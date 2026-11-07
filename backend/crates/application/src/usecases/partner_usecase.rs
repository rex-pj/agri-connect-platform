//! Partners: the brands, dealers, cooperatives and exporters that pay for
//! the demand report of some areas and categories, and for sponsored places
//! for some providers there.
//!
//! A contract covers one area (a location preset), some categories, two
//! dates (both included) and a monthly fee. It is in force between its
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
    events::{Change, DomainEvent, Entity},
    models::partner::{ActiveContract, Contract, NewContract, Partner, PartnerFields},
    repositories::{
        category_repository_trait::CategoryRepositoryTrait,
        demand_repository_trait::DemandRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        partner_repository_trait::PartnerRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::{PERM_PARTNER_MANAGE, PERM_PARTNER_REPORT_READ},
    constants::{CONTRACT_NOTE_MAX, DEMAND_AREA_RADIUS_M, PARTNER_NAME_MAX, SPONSORED_SHOWN},
    inputs::{ContractInput, PartnerInput},
    outputs::{
        AreaRevenueDto, ContractDto, NamedDto, PartnerAccountDto, PartnerDetailDto, PartnerDto,
        PartnerKpiDto, PartnerReportDto, PublicServiceDto,
    },
    ports::{EventBus, PasswordHasher},
    usecases::{
        account_rules::account_for_role,
        demand_usecase::{vn_today, AreaKey, DemandReportUseCase, ReportScope},
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
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    demand: DemandReportUseCase,
    password_hasher: Arc<dyn PasswordHasher>,
    events: Arc<dyn EventBus>,
}

/// Everything `PartnerUseCase` depends on, named so wiring cannot swap two
/// repos of compatible types by position.
pub struct PartnerDeps {
    pub partner_repo: Arc<dyn PartnerRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    pub demand: DemandReportUseCase,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub events: Arc<dyn EventBus>,
}

impl PartnerUseCase {
    pub fn new(d: PartnerDeps) -> Self {
        Self {
            partner_repo: d.partner_repo,
            user_repo: d.user_repo,
            category_repo: d.category_repo,
            location_repo: d.location_repo,
            demand_repo: d.demand_repo,
            demand: d.demand,
            password_hasher: d.password_hasher,
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

    pub async fn get_partner(&self, actor: &Actor, id: i32) -> Result<PartnerDetailDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let partner = self
            .partner_repo
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PartnerNotFound))?;
        let (contracts, in_force, names) = tokio::try_join!(
            self.partner_repo.contracts(id),
            self.in_force(),
            self.category_names(),
        )?;
        Ok(PartnerDetailDto {
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names))
                .collect(),
            partner: partner_dto(partner, &in_force),
        })
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
        let (locations, categories) = tokio::try_join!(
            self.location_repo.list_all(),
            self.category_repo.list_admin()
        )?;
        let known: HashSet<i32> = categories.iter().map(|c| c.id).collect();
        let note = non_blank(input.note);
        let mut category_ids: Vec<i32> = input
            .category_ids
            .into_iter()
            .filter(|id| known.contains(id))
            .collect();
        category_ids.sort_unstable();
        category_ids.dedup();
        let location = input
            .location_id
            .filter(|id| locations.iter().any(|l| l.id == *id && l.is_active));
        let dates = match (input.starts_on, input.ends_on) {
            (Some(s), Some(e)) if e >= s => Some((s, e)),
            _ => None,
        };
        let fee = input.monthly_fee.filter(|f| *f >= 0);
        Validation::new()
            .check(
                location.is_none(),
                "location_id",
                ErrorCode::LocationNotFound,
            )
            .check(dates.is_none(), "ends_on", ErrorCode::InvalidContractDates)
            .check(fee.is_none(), "monthly_fee", ErrorCode::InvalidMonthlyFee)
            .check(
                category_ids.is_empty(),
                "category_ids",
                ErrorCode::ContractCategoriesRequired,
            )
            .rule("note", max_chars_opt(&note, CONTRACT_NOTE_MAX))
            .finish()?;
        let (Some(location_id), Some((starts_on, ends_on)), Some(monthly_fee)) =
            (location, dates, fee)
        else {
            return Err(Error::new(ErrorCode::ValidationFailed));
        };
        let id = self
            .partner_repo
            .create_contract(
                &NewContract {
                    partner_id,
                    location_id,
                    starts_on,
                    ends_on,
                    monthly_fee,
                    note,
                    category_ids,
                },
                actor.user_id,
            )
            .await?;
        self.publish(actor, Entity::Contract, id, Change::Created, None)
            .await;
        Ok(id)
    }

    pub async fn delete_contract(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        if !self
            .partner_repo
            .delete_contract(partner_id, contract_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ContractNotFound));
        }
        self.publish(
            actor,
            Entity::Contract,
            contract_id,
            Change::Deleted,
            reason,
        )
        .await;
        Ok(())
    }

    async fn contract_of(&self, partner_id: i32, contract_id: i32) -> Result<Contract> {
        self.partner_repo
            .find_contract(partner_id, contract_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ContractNotFound))
    }

    /// The providers a contract may sponsor: those counting under its area,
    /// not sponsored by it yet.
    pub async fn sponsor_candidates(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
    ) -> Result<Vec<NamedDto>> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let contract = self.contract_of(partner_id, contract_id).await?;
        let taken: HashSet<i32> = contract.sponsored.iter().map(|s| s.business_id).collect();
        let mut candidates: Vec<NamedDto> = self
            .demand_repo
            .business_areas(DEMAND_AREA_RADIUS_M)
            .await?
            .into_iter()
            .filter(|a| a.location_id == contract.location_id && !taken.contains(&a.business_id))
            .map(|a| NamedDto {
                id: a.business_id,
                name: a.name,
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
        let in_area = self
            .demand_repo
            .business_areas(DEMAND_AREA_RADIUS_M)
            .await?
            .iter()
            .any(|a| a.business_id == business_id && a.location_id == contract.location_id);
        if !in_area {
            return Err(Error::field("business_id", ErrorCode::SponsoredOutsideArea));
        }
        self.partner_repo
            .add_sponsored(contract_id, business_id)
            .await?;
        self.publish(actor, Entity::Contract, contract_id, Change::Updated, None)
            .await;
        Ok(())
    }

    pub async fn remove_sponsored(
        &self,
        actor: &Actor,
        partner_id: i32,
        contract_id: i32,
        business_id: i32,
    ) -> Result<()> {
        actor.require(PERM_PARTNER_MANAGE)?;
        self.contract_of(partner_id, contract_id).await?;
        if self
            .partner_repo
            .remove_sponsored(contract_id, business_id)
            .await?
        {
            self.publish(actor, Entity::Contract, contract_id, Change::Updated, None)
                .await;
        }
        Ok(())
    }

    /// Paying partners today, the monthly revenue, and by area.
    pub async fn kpis(&self, actor: &Actor) -> Result<PartnerKpiDto> {
        actor.require(PERM_PARTNER_MANAGE)?;
        let (in_force, locations) =
            tokio::try_join!(self.in_force(), self.location_repo.list_all())?;
        let labels: HashMap<i32, String> = locations.into_iter().map(|l| (l.id, l.label)).collect();
        Ok(kpis_of(&in_force, &labels))
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
        let (contracts, in_force, names) = tokio::try_join!(
            self.partner_repo.contracts(partner.id),
            self.in_force(),
            self.category_names(),
        )?;
        let mine: Vec<&ActiveContract> = in_force
            .iter()
            .filter(|c| c.partner_id == partner.id)
            .collect();
        let mut areas: Vec<AreaKey> = mine
            .iter()
            .map(|c| AreaKey::Location(c.location_id))
            .collect();
        areas.sort();
        areas.dedup();
        let mut categories: Vec<i32> = mine.iter().flat_map(|c| c.category_ids.clone()).collect();
        categories.sort_unstable();
        categories.dedup();
        let scope = ReportScope {
            focus: focus.filter(|f| areas.contains(f)),
            areas: Some(areas),
            categories: Some(categories),
        };
        let report = self.demand.build(Utc::now(), &scope).await?;
        Ok(PartnerReportDto {
            partner_name: partner.name,
            report,
            contracts: contracts
                .into_iter()
                .map(|c| contract_dto(c, &in_force, &names))
                .filter(|c| c.in_force)
                .collect(),
        })
    }

    // ── Sponsored places ──────────────────────────────────────────────────────

    /// The providers sponsored in an area for a category (any of the
    /// contract's when `None`), by contracts in force: what a near-me page
    /// shows first, labelled.
    pub async fn sponsored_in(
        &self,
        location_id: i32,
        category_id: Option<i32>,
    ) -> Result<Vec<i32>> {
        let mut ids: Vec<i32> = self
            .in_force()
            .await?
            .into_iter()
            .filter(|c| c.location_id == location_id)
            .filter(|c| category_id.is_none_or(|id| c.category_ids.contains(&id)))
            .flat_map(|c| c.business_ids)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// Near-me services split into the sponsored ones to show first, labelled
    /// (one per sponsored provider of the area, at most `SPONSORED_SHOWN`,
    /// in the order found) and the others, in their order. A sponsored card
    /// moves up; nothing is hidden and nothing is shown twice.
    pub async fn feature_sponsored(
        &self,
        location_id: Option<i32>,
        category_id: Option<i32>,
        services: Vec<PublicServiceDto>,
    ) -> Result<(Vec<PublicServiceDto>, Vec<PublicServiceDto>)> {
        let sponsored = match location_id {
            Some(id) => self.sponsored_in(id, category_id).await?,
            None => Vec::new(),
        };
        Ok(split_sponsored(services, &sponsored))
    }

    /// The location preset a point counts under (for a device position).
    pub async fn area_of_point(&self, lat: f64, lng: f64) -> Result<Option<i32>> {
        self.demand_repo
            .nearest_location(lat, lng, DEMAND_AREA_RADIUS_M)
            .await
    }
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
        account_username: p.account_username,
        created_on: p.created_on,
    }
}

fn contract_dto(
    c: Contract,
    in_force: &[ActiveContract],
    names: &HashMap<i32, String>,
) -> ContractDto {
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
        location_id: c.location_id,
        location_label: c.location_label,
        starts_on: c.starts_on,
        ends_on: c.ends_on,
        monthly_fee: c.monthly_fee,
        note: c.note,
    }
}

fn split_sponsored(
    services: Vec<PublicServiceDto>,
    sponsored: &[i32],
) -> (Vec<PublicServiceDto>, Vec<PublicServiceDto>) {
    let mut first = Vec::new();
    let mut rest = Vec::with_capacity(services.len());
    let mut seen: HashSet<i32> = HashSet::new();
    for s in services {
        if first.len() < SPONSORED_SHOWN
            && sponsored.contains(&s.business_id)
            && seen.insert(s.business_id)
        {
            first.push(s);
        } else {
            rest.push(s);
        }
    }
    (first, rest)
}

fn kpis_of(in_force: &[ActiveContract], labels: &HashMap<i32, String>) -> PartnerKpiDto {
    let paying: HashSet<i32> = in_force.iter().map(|c| c.partner_id).collect();
    let mut by_area: BTreeMap<i32, (HashSet<i32>, i64)> = BTreeMap::new();
    for c in in_force {
        let entry = by_area.entry(c.location_id).or_default();
        entry.0.insert(c.partner_id);
        entry.1 += c.monthly_fee;
    }
    let mut areas: Vec<AreaRevenueDto> = by_area
        .into_iter()
        .map(|(location_id, (partners, revenue))| AreaRevenueDto {
            label: labels.get(&location_id).cloned().unwrap_or_default(),
            location_id,
            partners: partners.len(),
            monthly_revenue: revenue,
        })
        .collect();
    areas.sort_by(|a, b| {
        b.monthly_revenue
            .cmp(&a.monthly_revenue)
            .then(a.location_id.cmp(&b.location_id))
    });
    PartnerKpiDto {
        paying_partners: paying.len(),
        monthly_revenue: in_force.iter().map(|c| c.monthly_fee).sum(),
        areas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(id: i32, partner: i32, location: i32, fee: i64) -> ActiveContract {
        ActiveContract {
            contract_id: id,
            partner_id: partner,
            location_id: location,
            monthly_fee: fee,
            category_ids: vec![1],
            business_ids: Vec::new(),
        }
    }

    #[test]
    fn revenue_is_counted_once_per_partner_and_by_area() {
        let labels = HashMap::from([(10, "A".to_owned()), (11, "B".to_owned())]);
        let kpis = kpis_of(
            &[
                contract(1, 7, 10, 2_000_000),
                contract(2, 7, 11, 1_000_000),
                contract(3, 8, 10, 500_000),
            ],
            &labels,
        );
        assert_eq!(kpis.paying_partners, 2);
        assert_eq!(kpis.monthly_revenue, 3_500_000);
        let areas: Vec<_> = kpis
            .areas
            .iter()
            .map(|a| (a.label.as_str(), a.partners, a.monthly_revenue))
            .collect();
        assert_eq!(areas, vec![("A", 2, 2_500_000), ("B", 1, 1_000_000)]);
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
