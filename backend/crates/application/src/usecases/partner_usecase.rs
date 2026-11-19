//! Partners: the brands, dealers, cooperatives and exporters that pay for
//! the demand report of some areas and categories, and for sponsored places
//! for some providers there.
//!
//! A contract covers one area (a province, or one ward of it), some
//! categories, two dates (both included) and a monthly fee. It is in force between its
//! dates while its partner is on. A partner's account reads the report of
//! its contracts in force, and nothing else; the sponsored providers of a
//! contract must count under its area. The revenue figures are the monthly
//! fees of the contracts in force today.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use chrono::Utc;
use domain::{
    constants::{PARTNER_KINDS, ROLE_PARTNER},
    events::{Change, ContractChange, DomainEvent, Entity},
    models::{
        admin_unit::Ward,
        demand::DemoScope,
        partner::{ActiveContract, Contract, ContractArea, NewContract, Partner, PartnerFields},
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        demand_repository_trait::DemandRepositoryTrait,
        partner_repository_trait::PartnerRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    admin_units::{AdminUnits, AreaKey},
    authz::Actor,
    catalogue::{PERM_PARTNER_MANAGE, PERM_PARTNER_REPORT_READ},
    constants::{ADMIN_UNIT_SNAP_M, CONTRACT_NOTE_MAX, PARTNER_NAME_MAX},
    inputs::{ContractInput, PartnerInput},
    outputs::{
        AreaGroupDto, AreaRevenueDto, ContractDto, NamedDto, PartnerAccountDto, PartnerDetailDto,
        PartnerDto, PartnerKpiDto, PartnerReportDto, PublicServiceDto, SponsorCandidateDto,
    },
    ports::{EventBus, PasswordHasher},
    settings::SettingsReader,
    usecases::{
        account_rules::account_for_role,
        demand_usecase::{vn_today, DemandReportUseCase, ReportScope},
    },
    validation::{
        audit_reason, max_chars, max_chars_opt, non_blank, validate_email, validate_phone,
    },
};

#[derive(Clone)]
pub struct PartnerUseCase {
    partner_repo: Arc<dyn PartnerRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    demand: DemandReportUseCase,
    password_hasher: Arc<dyn PasswordHasher>,
    settings: SettingsReader,
    events: Arc<dyn EventBus>,
}

/// Everything `PartnerUseCase` depends on, named so wiring cannot swap two
/// repos of compatible types by position.
pub struct PartnerDeps {
    pub partner_repo: Arc<dyn PartnerRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    pub demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    pub demand: DemandReportUseCase,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub settings: SettingsReader,
    pub events: Arc<dyn EventBus>,
}

impl PartnerUseCase {
    pub fn new(d: PartnerDeps) -> Self {
        Self {
            partner_repo: d.partner_repo,
            user_repo: d.user_repo,
            category_repo: d.category_repo,
            admin_unit_repo: d.admin_unit_repo,
            demand_repo: d.demand_repo,
            demand: d.demand,
            password_hasher: d.password_hasher,
            settings: d.settings,
            events: d.events,
        }
    }

    async fn publish(
        &self,
        actor: &Actor,
        entity: Entity,
        id: i32,
        change: Change,
        reason: Option<String>,
    ) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    async fn in_force(&self) -> Result<Vec<ActiveContract>> {
        self.partner_repo
            .active_contracts(vn_today(Utc::now())?)
            .await
    }

    async fn units(&self) -> Result<AdminUnits> {
        AdminUnits::load(self.admin_unit_repo.as_ref()).await
    }

    /// How a contract is named in its partner's history: its area, and the
    /// shop a sponsorship is about.
    async fn contract_about(&self, area: &ContractArea, shop: Option<&str>) -> Result<String> {
        Ok(area_about(&self.units().await?, area, shop))
    }

    /// A change to one of the partner's contracts, audited on the partner.
    async fn contract_event(
        &self,
        actor: &Actor,
        partner_id: i32,
        change: ContractChange,
        about: String,
        reason: Option<String>,
    ) {
        self.publish(
            actor,
            Entity::Partner,
            partner_id,
            Change::Contract { change, about },
            reason,
        )
        .await;
    }

    async fn category_names(&self) -> Result<HashMap<i32, String>> {
        Ok(self
            .category_repo
            .list_admin()
            .await?
            .into_iter()
            .map(|c| (c.id, c.name))
            .collect())
    }

    // ── Admin ──────────────────────────────────────────────────────────────────

    pub async fn list_partners(&self, actor: &Actor) -> Result<Vec<PartnerDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (partners, in_force) = tokio::try_join!(self.partner_repo.list(), self.in_force())?;
        Ok(partners
            .into_iter()
            .map(|p| partner_dto(p, &in_force))
            .collect())
    }

    /// The partner whose report account `user_id` is, for the link from the
    /// account's admin page.
    pub async fn partner_of_account(&self, actor: &Actor, user_id: i32) -> Result<Option<i32>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        self.partner_repo.find_by_account(user_id).await
    }

    pub async fn get_partner(&self, actor: &Actor, id: i32) -> Result<PartnerDetailDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let partner = self
            .partner_repo
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        let (contracts, in_force, names, units) = tokio::try_join!(
            self.partner_repo.contracts(id),
            self.in_force(),
            self.category_names(),
            self.units(),
        )?;
        Ok(PartnerDetailDto {
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names, &units))
                .collect(),
            partner: partner_dto(partner, &in_force),
        })
    }

    /// What a contract may cover: each active province, and its wards.
    pub async fn area_choices(&self, actor: &Actor) -> Result<Vec<AreaGroupDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        Ok(self.units().await?.choices())
    }

    pub async fn create_partner(&self, actor: &Actor, input: PartnerInput) -> Result<i32> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let fields = validate_partner(input)?;
        let id = self.partner_repo.create(&fields, actor.user_id).await?;
        self.publish(actor, Entity::Partner, id, Change::Created, None)
            .await;
        Ok(id)
    }

    pub async fn update_partner(&self, actor: &Actor, id: i32, input: PartnerInput) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let fields = validate_partner(input)?;
        if !self.partner_repo.update(id, &fields, actor.user_id).await? {
            return Err(Error::new(ErrorCode::PartnerNotFound));
        }
        self.publish(actor, Entity::Partner, id, Change::Updated, None)
            .await;
        Ok(())
    }

    /// Turns a partner off (its contracts stop being in force: no report, no
    /// sponsored places) or on again. The same state again changes nothing.
    pub async fn set_partner_active(
        &self,
        actor: &Actor,
        id: i32,
        active: bool,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (_, changed) = self
            .partner_repo
            .set_active(id, active, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        if changed {
            self.publish(
                actor,
                Entity::Partner,
                id,
                Change::Visibility { visible: active },
                reason,
            )
            .await;
        }
        Ok(())
    }

    /// Gives a partner the account that signs in to read its report: an
    /// existing partner account, or a new one. One account, one partner.
    pub async fn assign_partner_account(
        &self,
        actor: &Actor,
        id: i32,
        username: String,
        email: String,
        password: String,
    ) -> Result<PartnerAccountDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        if self.partner_repo.find(id).await?.is_none() {
            return Err(Error::new(ErrorCode::PartnerNotFound));
        }
        if let Some(u) = self
            .user_repo
            .find_by_username(&username.trim().to_lowercase())
            .await?
        {
            let linked = self.partner_repo.find_by_account(u.id).await?;
            if u.role == ROLE_PARTNER && linked.is_some_and(|p| p != id) {
                return Err(Error::field("username", ErrorCode::PartnerAccountInUse));
            }
        }
        let (user_id, username) = account_for_role(
            self.user_repo.as_ref(),
            self.password_hasher.as_ref(),
            &username,
            &email,
            &password,
            ROLE_PARTNER,
        )
        .await?;
        self.partner_repo.assign_account(id, user_id).await?;
        self.publish(
            actor,
            Entity::Partner,
            id,
            Change::AccountAssigned {
                username: username.clone(),
            },
            None,
        )
        .await;
        Ok(PartnerAccountDto {
            partner_id: id,
            username,
        })
    }

    pub async fn create_contract(
        &self,
        actor: &Actor,
        partner_id: i32,
        input: ContractInput,
    ) -> Result<i32> {
        actor.require(PERM_PARTNER_MANAGE)?;
        if self.partner_repo.find(partner_id).await?.is_none() {
            return Err(Error::new(ErrorCode::PartnerNotFound));
        }
        let contract = self.checked_contract(partner_id, input, None).await?;
        let about = self.contract_about(&contract.area, None).await?;
        let id = self
            .partner_repo
            .create_contract(&contract, actor.user_id)
            .await?;
        self.contract_event(actor, partner_id, ContractChange::Added, about, None)
            .await;
        Ok(id)
    }

    /// Changes a contract's dates, fee, note and categories (renewing it, a
    /// new price). Its area stays: its sponsored shops belong to it.
    pub async fn update_contract(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        input: ContractInput,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let current = self.contract_of(partner_id, contract_id).await?;
        let about = self.contract_about(&current.area, None).await?;
        let contract = self
            .checked_contract(partner_id, input, Some(current.area))
            .await?;
        if !self
            .partner_repo
            .update_contract(partner_id, contract_id, &contract)
            .await?
        {
            return Err(Error::new(ErrorCode::ContractNotFound));
        }
        self.contract_event(actor, partner_id, ContractChange::Edited, about, None)
            .await;
        Ok(())
    }

    /// Every rule of a contract's boxes, reported together. `area` is the
    /// area of the contract being edited (it cannot change); a new contract
    /// takes the one sent, which must be an active province or ward.
    async fn checked_contract(
        &self,
        partner_id: i32,
        input: ContractInput,
        area: Option<ContractArea>,
    ) -> Result<NewContract> {
        let (units, categories) = tokio::try_join!(self.units(), self.category_repo.list_admin())?;
        let known: HashSet<i32> = categories.iter().map(|c| c.id).collect();
        let note = non_blank(input.note);
        let mut category_ids: Vec<i32> = input
            .category_ids
            .into_iter()
            .filter(|id| known.contains(id))
            .collect();
        category_ids.sort_unstable();
        category_ids.dedup();
        let area = area.or_else(|| {
            input
                .area
                .as_deref()
                .and_then(AreaKey::parse)
                .and_then(|key| units.contract_area(&key))
        });
        let dates = match (input.starts_on, input.ends_on) {
            (Some(s), Some(e)) if e >= s => Some((s, e)),
            _ => None,
        };
        let fee = input.monthly_fee.filter(|f| *f >= 0);
        Validation::new()
            .check(area.is_none(), "area", ErrorCode::LocationNotFound)
            .check(dates.is_none(), "ends_on", ErrorCode::InvalidContractDates)
            .check(fee.is_none(), "monthly_fee", ErrorCode::InvalidMonthlyFee)
            .check(
                category_ids.is_empty(),
                "category_ids",
                ErrorCode::ContractCategoriesRequired,
            )
            .rule("note", max_chars_opt(&note, CONTRACT_NOTE_MAX))
            .finish()?;
        let (Some(area), Some((starts_on, ends_on)), Some(monthly_fee)) = (area, dates, fee) else {
            return Err(Error::new(ErrorCode::ValidationFailed));
        };
        Ok(NewContract {
            partner_id,
            area,
            starts_on,
            ends_on,
            monthly_fee,
            note,
            category_ids,
        })
    }

    pub async fn delete_contract(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        // Named before it is gone: its line in the partner's history.
        let current = self.contract_of(partner_id, contract_id).await?;
        let about = self.contract_about(&current.area, None).await?;
        if !self
            .partner_repo
            .delete_contract(partner_id, contract_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ContractNotFound));
        }
        self.contract_event(actor, partner_id, ContractChange::Deleted, about, reason)
            .await;
        Ok(())
    }

    async fn contract_of(&self, partner_id: i32, contract_id: i32) -> Result<Contract> {
        self.partner_repo
            .find_contract(partner_id, contract_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ContractNotFound))
    }

    /// The shops a partner's contracts may sponsor: real ones for a real
    /// partner, demo ones for a demo partner, so demo data never mixes into
    /// what a paying partner gets.
    async fn sponsor_scope(&self, partner_id: i32) -> Result<DemoScope> {
        let partner = self
            .partner_repo
            .find(partner_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        Ok(scope_of(&partner))
    }

    /// The providers a contract may sponsor: those counting under its area,
    /// not sponsored by it yet.
    pub async fn sponsor_candidates(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
    ) -> Result<Vec<SponsorCandidateDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let contract = self.contract_of(partner_id, contract_id).await?;
        let taken: HashSet<i32> = contract.sponsored.iter().map(|s| s.business_id).collect();
        let scope = self.sponsor_scope(partner_id).await?;
        let (areas, units) =
            tokio::try_join!(self.demand_repo.business_areas(scope), self.units())?;
        let mut candidates: Vec<SponsorCandidateDto> = areas
            .into_iter()
            .filter(|a| in_area(&contract.area, &a.ward_code, &units))
            .filter(|a| !taken.contains(&a.business_id))
            .map(|a| SponsorCandidateDto {
                id: a.business_id,
                name: a.name,
                phone: a.phone,
                address: a.address,
            })
            .collect();
        candidates.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(candidates)
    }

    /// Sponsors a provider of the contract's area; twice changes nothing.
    pub async fn add_sponsored(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        business_id: i32,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let contract = self.contract_of(partner_id, contract_id).await?;
        let scope = self.sponsor_scope(partner_id).await?;
        let (areas, units) =
            tokio::try_join!(self.demand_repo.business_areas(scope), self.units())?;
        let Some(shop) = areas.iter().find(|a| {
            a.business_id == business_id && in_area(&contract.area, &a.ward_code, &units)
        }) else {
            return Err(Error::field("business_id", ErrorCode::SponsoredOutsideArea));
        };
        let about = area_about(&units, &contract.area, Some(&shop.name));
        self.partner_repo
            .add_sponsored(contract_id, business_id)
            .await?;
        self.contract_event(actor, partner_id, ContractChange::Sponsored, about, None)
            .await;
        Ok(())
    }

    /// Stops sponsoring a shop (asked to confirm, with a reason: it changes
    /// what a paying partner gets).
    pub async fn remove_sponsored(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        business_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let contract = self.contract_of(partner_id, contract_id).await?;
        let shop = contract
            .sponsored
            .iter()
            .find(|s| s.business_id == business_id)
            .map(|s| s.name.clone());
        let about = self.contract_about(&contract.area, shop.as_deref()).await?;
        if self
            .partner_repo
            .remove_sponsored(contract_id, business_id)
            .await?
        {
            self.contract_event(
                actor,
                partner_id,
                ContractChange::Unsponsored,
                about,
                reason,
            )
            .await;
        }
        Ok(())
    }

    /// Paying partners today, the monthly revenue, and by area.
    pub async fn kpis(&self, actor: &Actor) -> Result<PartnerKpiDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (in_force, units) = tokio::try_join!(self.in_force(), self.units())?;
        Ok(kpis_of(&in_force, &units))
    }

    // ── The partner's own report ──────────────────────────────────────────────

    /// The report over the areas and categories of the actor's partner's
    /// contracts in force, one area in `focus` or all of them.
    pub async fn own_report(
        &self,
        actor: &Actor,
        focus: Option<AreaKey>,
    ) -> Result<PartnerReportDto> {
        actor.require(PERM_PARTNER_REPORT_READ)?;
        let partner = match self.partner_repo.find_by_account(actor.user_id).await? {
            Some(id) => self.partner_repo.find(id).await?,
            None => None,
        }
        .filter(|p| p.is_active)
        .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        let (contracts, in_force, names, units) = tokio::try_join!(
            self.partner_repo.contracts(partner.id),
            self.in_force(),
            self.category_names(),
            self.units(),
        )?;
        let mine: Vec<&ActiveContract> = in_force
            .iter()
            .filter(|c| c.partner_id == partner.id)
            .collect();
        let mut areas: Vec<AreaKey> = mine.iter().map(|c| AreaKey::of_contract(&c.area)).collect();
        areas.sort();
        areas.dedup();
        let mut categories: Vec<i32> = mine.iter().flat_map(|c| c.category_ids.clone()).collect();
        categories.sort_unstable();
        categories.dedup();
        let scope = ReportScope {
            focus: focus.filter(|f| units.within(f, &areas)),
            areas: Some(areas),
            categories: Some(categories),
            // A real partner never sees demo figures; a demo one only those.
            demo: scope_of(&partner),
        };
        let report = self.demand.build(Utc::now(), &scope).await?;
        Ok(PartnerReportDto {
            partner_name: partner.name,
            report,
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names, &units))
                .filter(|c| c.in_force)
                .collect(),
        })
    }

    // ── Sponsored places ──────────────────────────────────────────────────────

    /// The providers sponsored where a point lies (its ward, or its whole
    /// province) for a category (any of the contract's when `None`), by
    /// contracts in force: what a near-me page shows first, labelled. The
    /// point's ward is looked up only when some contract is in force.
    pub async fn sponsored_at(
        &self,
        lat: f64,
        lng: f64,
        category_id: Option<i32>,
    ) -> Result<Vec<i32>> {
        let in_force = self.in_force().await?;
        if in_force.is_empty() {
            return Ok(Vec::new());
        }
        let Some(place) = self
            .admin_unit_repo
            .ward_at(lat, lng, ADMIN_UNIT_SNAP_M)
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(sponsored_of(&in_force, &place, category_id))
    }

    /// Near-me services split into the sponsored ones to show first, labelled
    /// (one per sponsored provider of the area, at most `market.sponsored_shown`,
    /// in the order found) and the others, in their order. A sponsored card
    /// moves up; nothing is hidden and nothing is shown twice.
    pub async fn feature_sponsored(
        &self,
        point: Option<(f64, f64)>,
        category_id: Option<i32>,
        services: Vec<PublicServiceDto>,
    ) -> Result<(Vec<PublicServiceDto>, Vec<PublicServiceDto>)> {
        let sponsored = match point {
            Some((lat, lng)) => self.sponsored_at(lat, lng, category_id).await?,
            None => Vec::new(),
        };
        let shown = self.settings.load().await?.sponsored_shown();
        Ok(split_sponsored(services, &sponsored, shown))
    }
}

/// The providers the contracts in force sponsor where `place` lies.
fn sponsored_of(in_force: &[ActiveContract], place: &Ward, category_id: Option<i32>) -> Vec<i32> {
    let mut ids: Vec<i32> = in_force
        .iter()
        .filter(|c| c.area.covers(&place.code, &place.province_code))
        .filter(|c| category_id.is_none_or(|id| c.category_ids.contains(&id)))
        .flat_map(|c| c.business_ids.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Whether a business in this ward counts under a contract's area.
fn in_area(area: &ContractArea, ward_code: &str, units: &AdminUnits) -> bool {
    units
        .province_of(ward_code)
        .is_some_and(|province| area.covers(ward_code, province))
}

/// Every rule of a partner's boxes, reported together.
fn validate_partner(input: PartnerInput) -> Result<PartnerFields> {
    let name = input.name.trim().to_owned();
    let kind = input.kind.trim().to_owned();
    let contact_name = non_blank(input.contact_name);
    let phone = non_blank(input.phone);
    let email = non_blank(input.email).map(|e| e.to_lowercase());
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, PARTNER_NAME_MAX))
        .check(
            !PARTNER_KINDS.contains(&kind.as_str()),
            "kind",
            ErrorCode::InvalidPartnerKind,
        )
        .rule(
            "contact_name",
            max_chars_opt(&contact_name, PARTNER_NAME_MAX),
        )
        .rule("phone", phone.as_deref().map_or(Ok(()), validate_phone))
        .rule("email", email.as_deref().map_or(Ok(()), validate_email))
        .finish()?;
    Ok(PartnerFields {
        name,
        kind,
        contact_name,
        phone,
        email,
    })
}

fn partner_dto(p: Partner, in_force: &[ActiveContract]) -> PartnerDto {
    let mine: Vec<&ActiveContract> = in_force.iter().filter(|c| c.partner_id == p.id).collect();
    PartnerDto {
        contracts_in_force: mine.len(),
        monthly_revenue: mine.iter().map(|c| c.monthly_fee).sum(),
        id: p.id,
        name: p.name,
        kind: p.kind,
        contact_name: p.contact_name,
        phone: p.phone,
        email: p.email,
        is_active: p.is_active,
        is_demo: p.is_demo,
        account_id: p.account_id,
        account_username: p.account_username,
        created_on: p.created_on,
    }
}

fn contract_dto(
    c: Contract,
    in_force: &[ActiveContract],
    names: &HashMap<i32, String>,
    units: &AdminUnits,
) -> ContractDto {
    let area = AreaKey::of_contract(&c.area);
    ContractDto {
        in_force: in_force.iter().any(|a| a.contract_id == c.id),
        categories: c
            .category_ids
            .iter()
            .filter_map(|id| {
                names.get(id).map(|name| NamedDto {
                    id: *id,
                    name: name.clone(),
                })
            })
            .collect(),
        sponsored: c
            .sponsored
            .into_iter()
            .map(|s| NamedDto {
                id: s.business_id,
                name: s.name,
            })
            .collect(),
        id: c.id,
        area_label: units.label(&area).unwrap_or_default(),
        area: area.value(),
        starts_on: c.starts_on,
        ends_on: c.ends_on,
        monthly_fee: c.monthly_fee,
        note: c.note,
    }
}

fn split_sponsored(
    services: Vec<PublicServiceDto>,
    sponsored: &[i32],
    shown: usize,
) -> (Vec<PublicServiceDto>, Vec<PublicServiceDto>) {
    let mut first = Vec::new();
    let mut rest = Vec::with_capacity(services.len());
    let mut seen: HashSet<i32> = HashSet::new();
    for s in services {
        if first.len() < shown && sponsored.contains(&s.business_id) && seen.insert(s.business_id) {
            first.push(s);
        } else {
            rest.push(s);
        }
    }
    (first, rest)
}

fn kpis_of(in_force: &[ActiveContract], units: &AdminUnits) -> PartnerKpiDto {
    let paying: HashSet<i32> = in_force.iter().map(|c| c.partner_id).collect();
    let mut by_area: BTreeMap<AreaKey, (HashSet<i32>, i64)> = BTreeMap::new();
    for c in in_force {
        let entry = by_area.entry(AreaKey::of_contract(&c.area)).or_default();
        entry.0.insert(c.partner_id);
        entry.1 += c.monthly_fee;
    }
    let mut areas: Vec<AreaRevenueDto> = by_area
        .into_iter()
        .map(|(area, (partners, revenue))| AreaRevenueDto {
            label: units.label(&area).unwrap_or_default(),
            area: area.value(),
            partners: partners.len(),
            monthly_revenue: revenue,
        })
        .collect();
    areas.sort_by(|a, b| {
        b.monthly_revenue
            .cmp(&a.monthly_revenue)
            .then(a.area.cmp(&b.area))
    });
    PartnerKpiDto {
        paying_partners: paying.len(),
        monthly_revenue: in_force.iter().map(|c| c.monthly_fee).sum(),
        areas,
    }
}

/// The data a partner's report and sponsored shops come from: real data for
/// a real partner, demo data for a demo one.
fn scope_of(partner: &Partner) -> DemoScope {
    if partner.is_demo {
        DemoScope::Demo
    } else {
        DemoScope::Real
    }
}

/// A contract's area by name, with the shop of a sponsorship after it.
fn area_about(units: &AdminUnits, area: &ContractArea, shop: Option<&str>) -> String {
    let label = units.label(&AreaKey::of_contract(area)).unwrap_or_default();
    match shop {
        Some(shop) => format!("{label} — {shop}"),
        None => label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(id: i32, partner: i32, province: &str, fee: i64) -> ActiveContract {
        ActiveContract {
            contract_id: id,
            partner_id: partner,
            area: ContractArea {
                province_code: province.into(),
                ward_code: None,
            },
            monthly_fee: fee,
            category_ids: vec![1],
            business_ids: Vec::new(),
        }
    }

    #[test]
    fn revenue_is_counted_once_per_partner_and_by_area() {
        let kpis = kpis_of(
            &[
                contract(1, 7, "82", 2_000_000),
                contract(2, 7, "91", 1_000_000),
                contract(3, 8, "82", 500_000),
            ],
            &crate::admin_units::tests::units(),
        );
        assert_eq!(kpis.paying_partners, 2);
        assert_eq!(kpis.monthly_revenue, 3_500_000);
        let areas: Vec<_> = kpis
            .areas
            .iter()
            .map(|a| (a.label.as_str(), a.partners, a.monthly_revenue))
            .collect();
        assert_eq!(
            areas,
            vec![("Đồng Tháp", 2, 2_500_000), ("An Giang", 1, 1_000_000)]
        );
    }

    #[test]
    fn a_partner_needs_a_known_kind_and_valid_contacts() {
        let fields = |kind: &str, phone: &str| {
            validate_partner(PartnerInput {
                name: " Dai ly Drone ".into(),
                kind: kind.into(),
                contact_name: None,
                phone: Some(phone.into()),
                email: Some(" ".into()),
            })
        };
        assert!(fields("dealer", "0912345678").is_ok_and(|f| f.name == "Dai ly Drone"));
        let codes = |r: Result<PartnerFields>| {
            r.err()
                .map(|e| e.fields().iter().map(|f| f.field).collect::<Vec<_>>())
        };
        assert_eq!(codes(fields("bank", "12")), Some(vec!["kind", "phone"]));
    }
}
