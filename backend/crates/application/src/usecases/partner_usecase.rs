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
        admin_list::{
            AdminFilter, CONTRACT_STATE_ENDED, CONTRACT_STATE_IN_FORCE, CONTRACT_STATE_PAUSED,
            CONTRACT_STATE_UPCOMING,
        },
        admin_unit::Ward,
        demand::DemoScope,
        partner::{
            ActiveContract, Contract, ContractArea, ContractFee, ContractFilter, NewContract,
            Partner, PartnerFields,
        },
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        partner_repository_trait::PartnerRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    admin_units::{AdminUnits, AreaKey},
    authz::Actor,
    catalogue::{PERM_PARTNER_MANAGE, PERM_PARTNER_REPORT_READ},
    constants::{
        ADMIN_PAGE_SIZE, ADMIN_UNIT_SNAP_M, CONTRACT_NOTE_MAX, PARTNER_NAME_MAX,
        SPONSOR_CANDIDATES_SHOWN,
    },
    inputs::{ContractInput, PartnerInput},
    outputs::{
        AdminPagedResponse, AreaGroupDto, AreaRevenueDto, ContractDto, NamedCodeDto, NamedDto,
        PartnerAccountDto, PartnerDetailDto, PartnerDto, PartnerKpiDto, PartnerReportDto,
        ProvinceRevenueDto, PublicServiceDto, SponsorCandidateDto,
    },
    pagination::{admin_page_of, paginate_admin, total_or_count},
    ports::{EventBus, PasswordHasher},
    settings::SettingsReader,
    usecases::{
        account_rules::account_for_role,
        demand_usecase::{vn_today, DemandReportUseCase, ReportGrant, ReportScope},
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

    /// The contracts in force today, without what they cover: what the
    /// revenue figures and the in-force marks read.
    async fn fees(&self) -> Result<Vec<ContractFee>> {
        self.partner_repo.fees_in_force(vn_today(Utc::now())?).await
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

    /// A page of the partners, on ones first, with their contracts in force
    /// (`filter`: the name, contact, phone or email, `SWITCH_LIST_STATES`,
    /// an order).
    pub async fn list_partners(
        &self,
        actor: &Actor,
        page: i64,
        filter: &AdminFilter,
    ) -> Result<AdminPagedResponse<PartnerDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let filter = &AdminFilter {
            today: Some(vn_today(Utc::now())?),
            ..filter.clone()
        };
        let (page, offset) = paginate_admin(page);
        let (partners, in_force) = tokio::try_join!(
            self.partner_repo.list(filter, ADMIN_PAGE_SIZE, offset),
            self.fees()
        )?;
        let total = total_or_count(offset, partners.len(), ADMIN_PAGE_SIZE, || {
            self.partner_repo.count(filter)
        })
        .await?;
        let items = partners
            .into_iter()
            .map(|p| partner_dto(p, &in_force))
            .collect();
        Ok(AdminPagedResponse::admin(items, total, page))
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
        let (contracts, fees, names, units) = tokio::try_join!(
            self.partner_repo.contracts(id),
            self.fees(),
            self.category_names(),
            self.units(),
        )?;
        let in_force = ids_of(&fees);
        Ok(PartnerDetailDto {
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names, &units))
                .collect(),
            partner: partner_dto(partner, &fees),
        })
    }

    /// One partner, with what its contracts in force bring in (its page's
    /// header and strip).
    pub async fn partner(&self, actor: &Actor, id: i32) -> Result<PartnerDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (partner, fees) = tokio::try_join!(self.partner_repo.find(id), self.fees())?;
        let partner = partner.ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        Ok(partner_dto(partner, &fees))
    }

    /// A page of a partner's contracts, the newest start first; `filter`: only
    /// those in the state, in the area, with a monthly fee in the range.
    pub async fn partner_contracts(
        &self,
        actor: &Actor,
        id: i32,
        page: i64,
        filter: &ContractFilter,
    ) -> Result<AdminPagedResponse<ContractDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        if !filter.is_empty() {
            let (contracts, fees, names, units) = tokio::try_join!(
                self.partner_repo.contracts(id),
                self.fees(),
                self.category_names(),
                self.units(),
            )?;
            let in_force = ids_of(&fees);
            let found = contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names, &units))
                .filter(|c| filter.state.is_none_or(|s| c.state == s))
                .filter(|c| filter.area.as_deref().is_none_or(|a| c.area == a))
                .filter(|c| filter.fee_min.is_none_or(|m| c.monthly_fee >= m))
                .filter(|c| filter.fee_max.is_none_or(|m| c.monthly_fee <= m))
                .filter(|c| filter.start_from.is_none_or(|d| c.starts_on >= d))
                .filter(|c| filter.start_to.is_none_or(|d| c.starts_on <= d))
                .filter(|c| filter.end_from.is_none_or(|d| c.ends_on >= d))
                .filter(|c| filter.end_to.is_none_or(|d| c.ends_on <= d))
                .filter(|c| {
                    filter
                        .category_id
                        .is_none_or(|id| c.categories.iter().any(|k| k.id == id))
                })
                .collect();
            return Ok(admin_page_of(found, page));
        }
        let (page, offset) = paginate_admin(page);
        let (contracts, fees, names, units) = tokio::try_join!(
            self.partner_repo
                .contracts_page(id, ADMIN_PAGE_SIZE, offset),
            self.fees(),
            self.category_names(),
            self.units(),
        )?;
        let total = total_or_count(offset, contracts.len(), ADMIN_PAGE_SIZE, || {
            self.partner_repo.count_contracts(id)
        })
        .await?;
        let in_force = ids_of(&fees);
        let items = contracts
            .into_iter()
            .map(|c| contract_dto(c, &in_force, &names, &units))
            .collect();
        Ok(AdminPagedResponse::admin(items, total, page))
    }

    /// The areas a partner's contracts are in, each once: what its contract
    /// list can be narrowed to.
    pub async fn contract_areas(&self, actor: &Actor, id: i32) -> Result<Vec<NamedCodeDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (contracts, units) = tokio::try_join!(self.partner_repo.contracts(id), self.units())?;
        let mut seen = HashSet::new();
        Ok(contracts
            .iter()
            .map(|c| AreaKey::of_contract(&c.area))
            .filter(|a| seen.insert(a.value()))
            .map(|a| NamedCodeDto {
                name: units.label(&a).unwrap_or_default(),
                code: a.value(),
            })
            .collect())
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
        email: String,
        password: String,
    ) -> Result<PartnerAccountDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        if self.partner_repo.find(id).await?.is_none() {
            return Err(Error::new(ErrorCode::PartnerNotFound));
        }
        if let Some(u) = self
            .user_repo
            .find_account_by_email(&email.trim().to_lowercase())
            .await?
        {
            let linked = self.partner_repo.find_by_account(u.id).await?;
            if u.role == ROLE_PARTNER && linked.is_some_and(|p| p != id) {
                return Err(Error::field("email", ErrorCode::PartnerAccountInUse));
            }
        }
        let (user_id, email) = account_for_role(
            self.user_repo.as_ref(),
            self.password_hasher.as_ref(),
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
                email: email.clone(),
            },
            None,
        )
        .await;
        Ok(PartnerAccountDto {
            partner_id: id,
            email,
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

    /// One contract of a partner, with the partner (its page's header).
    pub async fn get_contract(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
    ) -> Result<(PartnerDto, ContractDto)> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let partner = self
            .partner_repo
            .find(partner_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        let (contract, fees, names, units) = tokio::try_join!(
            self.contract_of(partner_id, contract_id),
            self.fees(),
            self.category_names(),
            self.units(),
        )?;
        Ok((
            partner_dto(partner, &fees),
            contract_dto(contract, &ids_of(&fees), &names, &units),
        ))
    }

    /// The providers a contract may sponsor: those of its area not
    /// sponsored by it yet, their name matching `search` when given; the
    /// first `SPONSOR_CANDIDATES_SHOWN` by name.
    pub async fn sponsor_candidates(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        search: Option<&str>,
    ) -> Result<Vec<SponsorCandidateDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let contract = self.contract_of(partner_id, contract_id).await?;
        let scope = self.sponsor_scope(partner_id).await?;
        let shops = self
            .partner_repo
            .sponsor_candidates(
                contract_id,
                &contract.area,
                scope,
                search,
                SPONSOR_CANDIDATES_SHOWN,
            )
            .await?;
        Ok(shops
            .into_iter()
            .map(|s| SponsorCandidateDto {
                id: s.business_id,
                name: s.name,
                phone: s.phone,
                address: s.address,
            })
            .collect())
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
        let Some(shop) = self
            .partner_repo
            .shop_in_area(business_id, &contract.area, scope)
            .await?
        else {
            return Err(Error::field("business_id", ErrorCode::SponsoredOutsideArea));
        };
        let about = self
            .contract_about(&contract.area, Some(&shop.name))
            .await?;
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
        let (in_force, units) = tokio::try_join!(self.fees(), self.units())?;
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
        let mine_ids: HashSet<i32> = mine.iter().map(|c| c.contract_id).collect();
        // Each contract grants its own categories in its own area, never
        // another contract's categories there.
        let mut scope = ReportScope {
            grants: Some(
                mine.iter()
                    .map(|c| ReportGrant {
                        area: AreaKey::of_contract(&c.area),
                        categories: c.category_ids.clone(),
                    })
                    .collect(),
            ),
            focus: None,
            // A real partner never sees demo figures; a demo one only those.
            demo: scope_of(&partner),
        };
        let areas = scope.areas().unwrap_or_default();
        scope.focus = focus.filter(|f| units.within(f, &areas));
        let report = self.demand.build(Utc::now(), &scope).await?;
        Ok(PartnerReportDto {
            partner_name: partner.name,
            report,
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &mine_ids, &names, &units))
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

/// The ids of the contracts in force.
fn ids_of(in_force: &[ContractFee]) -> HashSet<i32> {
    in_force.iter().map(|c| c.contract_id).collect()
}

fn partner_dto(p: Partner, in_force: &[ContractFee]) -> PartnerDto {
    let mine: Vec<&ContractFee> = in_force.iter().filter(|c| c.partner_id == p.id).collect();
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
        account_email: p.account_email,
        created_on: p.created_on,
    }
}

fn contract_dto(
    c: Contract,
    in_force: &HashSet<i32>,
    names: &HashMap<i32, String>,
    units: &AdminUnits,
) -> ContractDto {
    let area = AreaKey::of_contract(&c.area);
    let today = vn_today(Utc::now()).unwrap_or(c.starts_on);
    let live = in_force.contains(&c.id);
    let state = if live {
        CONTRACT_STATE_IN_FORCE
    } else if c.ends_on < today {
        CONTRACT_STATE_ENDED
    } else if c.starts_on > today {
        CONTRACT_STATE_UPCOMING
    } else {
        CONTRACT_STATE_PAUSED
    };
    ContractDto {
        in_force: live,
        state,
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
        sponsored_away: c
            .sponsored
            .iter()
            .filter(|s| !s.in_area)
            .map(|s| s.business_id)
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

/// The revenue figures of the contracts in force: paying partners, the
/// monthly fees, by province (the whole province and its wards together,
/// each partner once), each with its contract areas; the most first.
fn kpis_of(in_force: &[ContractFee], units: &AdminUnits) -> PartnerKpiDto {
    #[derive(Default)]
    struct Sum {
        partners: HashSet<i32>,
        revenue: i64,
    }
    let mut provinces: BTreeMap<String, (Sum, BTreeMap<AreaKey, Sum>)> = BTreeMap::new();
    for c in in_force {
        let (province, areas) = provinces.entry(c.area.province_code.clone()).or_default();
        let area = areas.entry(AreaKey::of_contract(&c.area)).or_default();
        for sum in [province, area] {
            sum.partners.insert(c.partner_id);
            sum.revenue += c.monthly_fee;
        }
    }
    let most_first = |a: &i64, b: &i64, x: &str, y: &str| b.cmp(a).then(x.cmp(y));
    let mut provinces: Vec<ProvinceRevenueDto> = provinces
        .into_iter()
        .map(|(code, (sum, areas))| {
            let key = AreaKey::Province(code);
            let mut areas: Vec<AreaRevenueDto> = areas
                .into_iter()
                .map(|(area, sum)| AreaRevenueDto {
                    label: units.label(&area).unwrap_or_default(),
                    area: area.value(),
                    partners: sum.partners.len(),
                    monthly_revenue: sum.revenue,
                })
                .collect();
            areas.sort_by(|a, b| {
                most_first(&a.monthly_revenue, &b.monthly_revenue, &a.area, &b.area)
            });
            ProvinceRevenueDto {
                label: units.label(&key).unwrap_or_default(),
                area: key.value(),
                partners: sum.partners.len(),
                monthly_revenue: sum.revenue,
                areas,
            }
        })
        .collect();
    provinces.sort_by(|a, b| most_first(&a.monthly_revenue, &b.monthly_revenue, &a.area, &b.area));
    PartnerKpiDto {
        paying_partners: in_force
            .iter()
            .map(|c| c.partner_id)
            .collect::<HashSet<_>>()
            .len(),
        monthly_revenue: in_force.iter().map(|c| c.monthly_fee).sum(),
        provinces,
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
        Some(shop) => format!("{label} · {shop}"),
        None => label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(
        id: i32,
        partner: i32,
        province: &str,
        ward: Option<&str>,
        fee: i64,
    ) -> ContractFee {
        ContractFee {
            contract_id: id,
            partner_id: partner,
            area: ContractArea {
                province_code: province.into(),
                ward_code: ward.map(str::to_owned),
            },
            monthly_fee: fee,
        }
    }

    #[test]
    fn revenue_is_counted_once_per_partner_by_province_and_area() {
        let kpis = kpis_of(
            &[
                contract(1, 7, "82", None, 2_000_000),
                contract(2, 7, "91", None, 1_000_000),
                contract(3, 8, "82", Some("28439"), 500_000),
                // The same partner in a ward of a province it already pays
                // for whole: one partner there.
                contract(4, 7, "82", Some("28441"), 300_000),
            ],
            &crate::admin_units::tests::units(),
        );
        assert_eq!(kpis.paying_partners, 2);
        assert_eq!(kpis.monthly_revenue, 3_800_000);
        let provinces: Vec<_> = kpis
            .provinces
            .iter()
            .map(|p| (p.label.as_str(), p.partners, p.monthly_revenue))
            .collect();
        assert_eq!(
            provinces,
            vec![("Đồng Tháp", 2, 2_800_000), ("An Giang", 1, 1_000_000)]
        );
        // A province's own areas, the most first: the whole of it, then its
        // wards.
        let areas: Vec<_> = kpis
            .provinces
            .first()
            .map(|p| p.areas.iter())
            .into_iter()
            .flatten()
            .map(|a| (a.area.as_str(), a.partners, a.monthly_revenue))
            .collect();
        assert_eq!(
            areas,
            vec![
                ("p82", 1, 2_000_000),
                ("w28439", 1, 500_000),
                ("w28441", 1, 300_000)
            ]
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
