//! Partners, their contracts, each contract's categories and sponsored
//! providers. Lists are put together from a few queries by id (a partner
//! has a handful of contracts), never one per row.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sea_orm::{
    sea_query::{Expr, OnConflict, Query},
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select, Set,
    TransactionTrait,
};
use tracing::error;

use domain::{
    models::{
        admin_list::AdminFilter,
        demand::DemoScope,
        partner::{
            ActiveContract, AreaShop, Contract, ContractArea, ContractFee, NewContract, Partner,
            PartnerFields, SponsoredBusiness,
        },
    },
    repositories::partner_repository_trait::PartnerRepositoryTrait,
    Error,
};

use crate::{
    admin_list::{admin_order, switch_state, OrderColumns},
    entities::{
        business,
        partner::{self, Entity as PartnerEntity},
        partner_contract::{self, Entity as ContractEntity},
        partner_contract_service_type, partner_contract_supply_type,
        partner_sponsored::{self, Entity as Sponsored},
        service_type, user, ward,
    },
    repositories::demand_repository::business_in,
    search::{admin_partner_search, search_condition, BUSINESS_NAME_SEARCH_COLUMNS},
};

/// A contract's area: its province, and maybe one ward of it.
fn area_of(c: &partner_contract::Model) -> ContractArea {
    ContractArea {
        province_code: c.province_code.clone(),
        ward_code: c.ward_code.clone(),
    }
}

/// Files a contract's types: the ids of service types in one table, the rest
/// (supply types: one id sequence for both) in the other.
async fn write_contract_types<C: ConnectionTrait>(
    db: &C,
    contract_id: i32,
    ids: &[i32],
) -> Result<(), DbErr> {
    let services: Vec<i32> = service_type::Entity::find()
        .select_only()
        .column(service_type::Column::Id)
        .filter(service_type::Column::Id.is_in(ids.iter().copied()))
        .into_tuple()
        .all(db)
        .await?;
    let supplies: Vec<i32> = ids
        .iter()
        .copied()
        .filter(|id| !services.contains(id))
        .collect();
    if !services.is_empty() {
        partner_contract_service_type::Entity::insert_many(services.into_iter().map(|id| {
            partner_contract_service_type::ActiveModel {
                contract_id: Set(contract_id),
                service_type_id: Set(id),
            }
        }))
        .exec(db)
        .await?;
    }
    if !supplies.is_empty() {
        partner_contract_supply_type::Entity::insert_many(supplies.into_iter().map(|id| {
            partner_contract_supply_type::ActiveModel {
                contract_id: Set(contract_id),
                supply_type_id: Set(id),
            }
        }))
        .exec(db)
        .await?;
    }
    Ok(())
}

pub struct PartnerRepository {
    db: Arc<DatabaseConnection>,
}

impl PartnerRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("partner {what} error: {e}");
            Error::internal("Failed to load or save the partner.")
        }
    }

    /// Partners as the domain has them, with their account's email.
    async fn with_accounts(&self, rows: Vec<partner::Model>) -> Result<Vec<Partner>, Error> {
        let ids: Vec<i32> = rows.iter().filter_map(|p| p.account_id).collect();
        let names: HashMap<i32, String> = if ids.is_empty() {
            HashMap::new()
        } else {
            user::Entity::find()
                .filter(user::Column::Id.is_in(ids))
                .all(self.db.as_ref())
                .await
                .map_err(Self::fail("accounts"))?
                .into_iter()
                .map(|u| (u.id, u.email))
                .collect()
        };
        Ok(rows
            .into_iter()
            .map(|p| Partner {
                account_email: p.account_id.and_then(|id| names.get(&id).cloned()),
                account_id: p.account_id,
                id: p.id,
                name: p.name,
                kind: p.kind,
                contact_name: p.contact_name,
                phone: p.phone,
                email: p.email,
                is_active: p.is_active,
                is_demo: p.is_demo,
                created_on: p.created_on.with_timezone(&Utc),
            })
            .collect())
    }

    /// Contracts as the domain has them: area, categories, sponsored.
    async fn assemble(&self, rows: Vec<partner_contract::Model>) -> Result<Vec<Contract>, Error> {
        let db = self.db.as_ref();
        let ids: Vec<i32> = rows.iter().map(|c| c.id).collect();
        let mut categories: HashMap<i32, Vec<i32>> = HashMap::new();
        let services = partner_contract_service_type::Entity::find()
            .filter(partner_contract_service_type::Column::ContractId.is_in(ids.clone()))
            .all(db)
            .await
            .map_err(Self::fail("contract service types"))?
            .into_iter()
            .map(|c| (c.contract_id, c.service_type_id));
        let supplies = partner_contract_supply_type::Entity::find()
            .filter(partner_contract_supply_type::Column::ContractId.is_in(ids.clone()))
            .all(db)
            .await
            .map_err(Self::fail("contract supply types"))?
            .into_iter()
            .map(|c| (c.contract_id, c.supply_type_id));
        for (contract_id, category_id) in services.chain(supplies) {
            categories.entry(contract_id).or_default().push(category_id);
        }
        for ids in categories.values_mut() {
            ids.sort_unstable();
        }
        let sponsored_rows = Sponsored::find()
            .filter(partner_sponsored::Column::ContractId.is_in(ids))
            .all(db)
            .await
            .map_err(Self::fail("sponsored"))?;
        let shops: HashMap<i32, business::Model> = business::Entity::find()
            .filter(business::Column::Id.is_in(sponsored_rows.iter().map(|s| s.business_id)))
            .all(db)
            .await
            .map_err(Self::fail("sponsored names"))?
            .into_iter()
            .map(|b| (b.id, b))
            .collect();
        // The province of each sponsored shop's ward, to tell whether it
        // still lies in its contract's area.
        let provinces: HashMap<String, String> = ward::Entity::find()
            .filter(ward::Column::Code.is_in(shops.values().filter_map(|b| b.ward_code.clone())))
            .all(db)
            .await
            .map_err(Self::fail("sponsored wards"))?
            .into_iter()
            .map(|w| (w.code, w.province_code))
            .collect();
        let areas: HashMap<i32, ContractArea> = rows.iter().map(|c| (c.id, area_of(c))).collect();
        let mut sponsored: HashMap<i32, Vec<SponsoredBusiness>> = HashMap::new();
        for s in sponsored_rows {
            if let Some(shop) = shops.get(&s.business_id) {
                let in_area = shop
                    .ward_code
                    .as_ref()
                    .and_then(|w| provinces.get(w).map(|p| (w, p)))
                    .zip(areas.get(&s.contract_id))
                    .is_some_and(|((w, p), area)| area.covers(w, p));
                sponsored
                    .entry(s.contract_id)
                    .or_default()
                    .push(SponsoredBusiness {
                        business_id: s.business_id,
                        name: shop.name.clone(),
                        in_area,
                    });
            }
        }
        Ok(rows
            .into_iter()
            .map(|c| Contract {
                area: area_of(&c),
                category_ids: categories.remove(&c.id).unwrap_or_default(),
                sponsored: sponsored.remove(&c.id).unwrap_or_default(),
                id: c.id,
                partner_id: c.partner_id,
                starts_on: c.starts_on,
                ends_on: c.ends_on,
                monthly_fee: c.monthly_fee,
                note: c.note,
            })
            .collect())
    }
}

fn fields_model(f: &PartnerFields) -> partner::ActiveModel {
    partner::ActiveModel {
        name: Set(f.name.clone()),
        kind: Set(f.kind.clone()),
        contact_name: Set(f.contact_name.clone()),
        phone: Set(f.phone.clone()),
        email: Set(f.email.clone()),
        ..Default::default()
    }
}

/// A business lying in `area`: in its ward, or in a ward of its province.
fn business_in_area(area: &ContractArea) -> Condition {
    let ward = Expr::col((business::Entity, business::Column::WardCode));
    Condition::all().add(match &area.ward_code {
        Some(code) => ward.eq(code.clone()),
        None => ward.in_subquery(
            Query::select()
                .column(ward::Column::Code)
                .from(ward::Entity)
                .and_where(Expr::col(ward::Column::ProvinceCode).eq(area.province_code.clone()))
                .to_owned(),
        ),
    })
}

#[derive(FromQueryResult)]
struct ShopRow {
    id: i32,
    name: String,
    phone: String,
    address: Option<String>,
}

impl From<ShopRow> for AreaShop {
    fn from(r: ShopRow) -> Self {
        Self {
            business_id: r.id,
            name: r.name,
            phone: r.phone,
            address: r.address,
        }
    }
}

/// The live (not deleted) businesses of `scope` lying in `area`, read as
/// `ShopRow`s.
fn shops_in_area(area: &ContractArea, scope: DemoScope) -> Select<business::Entity> {
    business::Entity::find()
        .select_only()
        .column(business::Column::Id)
        .column(business::Column::Name)
        .column(business::Column::Phone)
        .column(business::Column::Address)
        .filter(business::Column::DeletedAt.is_null())
        .filter(business_in(scope))
        .filter(business_in_area(area))
}

#[async_trait]
impl PartnerRepositoryTrait for PartnerRepository {
    async fn list(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Partner>, Error> {
        let columns = OrderColumns {
            created: partner::Column::CreatedOn,
            name: partner::Column::Name,
            id: partner::Column::Id,
        };
        let query = PartnerEntity::find().filter(admin_condition(filter));
        let rows = admin_order(query, filter.sort, columns, |q| {
            q.order_by_desc(partner::Column::IsActive)
                .order_by_asc(partner::Column::Name)
                .order_by_asc(partner::Column::Id)
        })
        .limit(limit as u64)
        .offset(offset as u64)
        .all(self.db.as_ref())
        .await
        .map_err(Self::fail("list"))?;
        self.with_accounts(rows).await
    }

    async fn count(&self, filter: &AdminFilter) -> Result<i64, Error> {
        PartnerEntity::find()
            .filter(admin_condition(filter))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Self::fail("count"))
    }

    async fn find(&self, id: i32) -> Result<Option<Partner>, Error> {
        let row = PartnerEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("find"))?;
        Ok(self
            .with_accounts(row.into_iter().collect())
            .await?
            .into_iter()
            .next())
    }

    async fn find_by_account(&self, user_id: i32) -> Result<Option<i32>, Error> {
        PartnerEntity::find()
            .filter(partner::Column::AccountId.eq(user_id))
            .one(self.db.as_ref())
            .await
            .map(|p| p.map(|p| p.id))
            .map_err(Self::fail("find_by_account"))
    }

    async fn create(&self, fields: &PartnerFields, actor_id: i32) -> Result<i32, Error> {
        let mut model = fields_model(fields);
        model.is_active = Set(true);
        model.created_by = Set(Some(actor_id));
        model.updated_by = Set(Some(actor_id));
        PartnerEntity::insert(model)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(Self::fail("create"))
    }

    async fn update(&self, id: i32, f: &PartnerFields, actor_id: i32) -> Result<bool, Error> {
        PartnerEntity::update_many()
            .col_expr(partner::Column::Name, Expr::value(f.name.clone()))
            .col_expr(partner::Column::Kind, Expr::value(f.kind.clone()))
            .col_expr(
                partner::Column::ContactName,
                Expr::value(f.contact_name.clone()),
            )
            .col_expr(partner::Column::Phone, Expr::value(f.phone.clone()))
            .col_expr(partner::Column::Email, Expr::value(f.email.clone()))
            .col_expr(partner::Column::UpdatedBy, Expr::value(actor_id))
            .col_expr(partner::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(partner::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(Self::fail("update"))
    }

    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error> {
        let Some(current) = PartnerEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?
        else {
            return Ok(None);
        };
        if current.is_active == active {
            return Ok(Some((current.name, false)));
        }
        PartnerEntity::update_many()
            .col_expr(partner::Column::IsActive, Expr::value(active))
            .col_expr(partner::Column::UpdatedBy, Expr::value(actor_id))
            .col_expr(partner::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(partner::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?;
        Ok(Some((current.name, true)))
    }

    async fn assign_account(&self, id: i32, user_id: i32) -> Result<(), Error> {
        PartnerEntity::update_many()
            .col_expr(partner::Column::AccountId, Expr::value(user_id))
            .col_expr(partner::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(partner::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(Self::fail("assign_account"))
    }

    async fn contracts(&self, partner_id: i32) -> Result<Vec<Contract>, Error> {
        let rows = ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .order_by_desc(partner_contract::Column::StartsOn)
            .order_by_desc(partner_contract::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("contracts"))?;
        self.assemble(rows).await
    }

    async fn contracts_page(
        &self,
        partner_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Contract>, Error> {
        let rows = ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .order_by_desc(partner_contract::Column::StartsOn)
            .order_by_desc(partner_contract::Column::Id)
            .limit(u64::try_from(limit).unwrap_or(0))
            .offset(u64::try_from(offset).unwrap_or(0))
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("contracts_page"))?;
        self.assemble(rows).await
    }

    async fn count_contracts(&self, partner_id: i32) -> Result<i64, Error> {
        ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .count(self.db.as_ref())
            .await
            .map(|n| i64::try_from(n).unwrap_or(i64::MAX))
            .map_err(Self::fail("count_contracts"))
    }

    async fn fees_in_force(&self, day: NaiveDate) -> Result<Vec<ContractFee>, Error> {
        let on = Query::select()
            .column(partner::Column::Id)
            .from(PartnerEntity)
            .and_where(Expr::col(partner::Column::IsActive).eq(true))
            .to_owned();
        ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.in_subquery(on))
            .filter(partner_contract::Column::StartsOn.lte(day))
            .filter(partner_contract::Column::EndsOn.gte(day))
            .order_by_asc(partner_contract::Column::Id)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|c| ContractFee {
                        contract_id: c.id,
                        partner_id: c.partner_id,
                        area: area_of(&c),
                        monthly_fee: c.monthly_fee,
                    })
                    .collect()
            })
            .map_err(Self::fail("fees_in_force"))
    }

    async fn create_contract(&self, c: &NewContract, actor_id: i32) -> Result<i32, Error> {
        let fail = Self::fail("create_contract");
        let txn = self.db.begin().await.map_err(&fail)?;
        let id = ContractEntity::insert(partner_contract::ActiveModel {
            partner_id: Set(c.partner_id),
            province_code: Set(c.area.province_code.clone()),
            ward_code: Set(c.area.ward_code.clone()),
            starts_on: Set(c.starts_on),
            ends_on: Set(c.ends_on),
            monthly_fee: Set(c.monthly_fee),
            note: Set(c.note.clone()),
            created_by: Set(Some(actor_id)),
            ..Default::default()
        })
        .exec(&txn)
        .await
        .map_err(&fail)?
        .last_insert_id;
        write_contract_types(&txn, id, &c.category_ids)
            .await
            .map_err(&fail)?;
        txn.commit().await.map_err(&fail)?;
        Ok(id)
    }

    async fn find_contract(
        &self,
        partner_id: i32,
        contract_id: i32,
    ) -> Result<Option<Contract>, Error> {
        let rows = ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .filter(partner_contract::Column::Id.eq(contract_id))
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("find_contract"))?;
        Ok(self.assemble(rows).await?.into_iter().next())
    }

    async fn update_contract(
        &self,
        partner_id: i32,
        contract_id: i32,
        c: &NewContract,
    ) -> Result<bool, Error> {
        let fail = Self::fail("update_contract");
        let txn = self.db.begin().await.map_err(&fail)?;
        let updated = ContractEntity::update_many()
            .col_expr(partner_contract::Column::StartsOn, Expr::value(c.starts_on))
            .col_expr(partner_contract::Column::EndsOn, Expr::value(c.ends_on))
            .col_expr(
                partner_contract::Column::MonthlyFee,
                Expr::value(c.monthly_fee),
            )
            .col_expr(partner_contract::Column::Note, Expr::value(c.note.clone()))
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .filter(partner_contract::Column::Id.eq(contract_id))
            .exec(&txn)
            .await
            .map_err(&fail)?
            .rows_affected
            > 0;
        if !updated {
            return Ok(false);
        }
        partner_contract_service_type::Entity::delete_many()
            .filter(partner_contract_service_type::Column::ContractId.eq(contract_id))
            .exec(&txn)
            .await
            .map_err(&fail)?;
        partner_contract_supply_type::Entity::delete_many()
            .filter(partner_contract_supply_type::Column::ContractId.eq(contract_id))
            .exec(&txn)
            .await
            .map_err(&fail)?;
        write_contract_types(&txn, contract_id, &c.category_ids)
            .await
            .map_err(&fail)?;
        txn.commit().await.map_err(&fail)?;
        Ok(true)
    }

    async fn delete_contract(&self, partner_id: i32, contract_id: i32) -> Result<bool, Error> {
        ContractEntity::delete_many()
            .filter(partner_contract::Column::PartnerId.eq(partner_id))
            .filter(partner_contract::Column::Id.eq(contract_id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(Self::fail("delete_contract"))
    }

    async fn add_sponsored(&self, contract_id: i32, business_id: i32) -> Result<(), Error> {
        Sponsored::insert(partner_sponsored::ActiveModel {
            contract_id: Set(contract_id),
            business_id: Set(business_id),
        })
        .on_conflict(
            OnConflict::columns([
                partner_sponsored::Column::ContractId,
                partner_sponsored::Column::BusinessId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(self.db.as_ref())
        .await
        .map(|_| ())
        .map_err(Self::fail("add_sponsored"))
    }

    async fn sponsor_candidates(
        &self,
        contract_id: i32,
        area: &ContractArea,
        scope: DemoScope,
        search: Option<&str>,
        limit: u64,
    ) -> Result<Vec<AreaShop>, Error> {
        let sponsored = Query::select()
            .column(partner_sponsored::Column::BusinessId)
            .from(Sponsored)
            .and_where(Expr::col(partner_sponsored::Column::ContractId).eq(contract_id))
            .to_owned();
        shops_in_area(area, scope)
            .filter(business::Column::Id.not_in_subquery(sponsored))
            .filter(search_condition(BUSINESS_NAME_SEARCH_COLUMNS, search))
            .order_by_asc(business::Column::Name)
            .order_by_asc(business::Column::Id)
            .limit(limit)
            .into_model::<ShopRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(AreaShop::from).collect())
            .map_err(Self::fail("sponsor_candidates"))
    }

    async fn shop_in_area(
        &self,
        business_id: i32,
        area: &ContractArea,
        scope: DemoScope,
    ) -> Result<Option<AreaShop>, Error> {
        shops_in_area(area, scope)
            .filter(business::Column::Id.eq(business_id))
            .into_model::<ShopRow>()
            .one(self.db.as_ref())
            .await
            .map(|row| row.map(AreaShop::from))
            .map_err(Self::fail("shop_in_area"))
    }

    async fn remove_sponsored(&self, contract_id: i32, business_id: i32) -> Result<bool, Error> {
        Sponsored::delete_many()
            .filter(partner_sponsored::Column::ContractId.eq(contract_id))
            .filter(partner_sponsored::Column::BusinessId.eq(business_id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(Self::fail("remove_sponsored"))
    }

    async fn active_contracts(&self, day: NaiveDate) -> Result<Vec<ActiveContract>, Error> {
        let db = self.db.as_ref();
        let on: Vec<i32> = PartnerEntity::find()
            .filter(partner::Column::IsActive.eq(true))
            .all(db)
            .await
            .map_err(Self::fail("active partners"))?
            .into_iter()
            .map(|p| p.id)
            .collect();
        let rows = ContractEntity::find()
            .filter(partner_contract::Column::PartnerId.is_in(on))
            .filter(partner_contract::Column::StartsOn.lte(day))
            .filter(partner_contract::Column::EndsOn.gte(day))
            .order_by_asc(partner_contract::Column::Id)
            .all(db)
            .await
            .map_err(Self::fail("active contracts"))?;
        Ok(self
            .assemble(rows)
            .await?
            .into_iter()
            .map(|c| ActiveContract {
                contract_id: c.id,
                partner_id: c.partner_id,
                area: c.area,
                monthly_fee: c.monthly_fee,
                category_ids: c.category_ids,
                // Only the shops still in the area are moved up there.
                business_ids: c
                    .sponsored
                    .iter()
                    .filter(|s| s.in_area)
                    .map(|s| s.business_id)
                    .collect(),
            })
            .collect())
    }
}

/// The admin list's criteria, shared by the list and its count.
fn admin_condition(filter: &AdminFilter) -> sea_orm::Condition {
    admin_condition_base(filter).add(crate::admin_list::created_between(
        (
            crate::entities::partner::Entity,
            crate::entities::partner::Column::CreatedOn,
        ),
        filter,
    ))
}

fn admin_condition_base(filter: &AdminFilter) -> sea_orm::Condition {
    let cond = sea_orm::Condition::all()
        .add(admin_partner_search(filter.search()))
        .add(switch_state(partner::Column::IsActive, filter.state));
    let cond = match filter.kind.as_deref() {
        Some(kind) => cond.add(partner::Column::Kind.eq(kind)),
        None => cond,
    };
    match (filter.contract.as_deref(), filter.today) {
        (Some(code), Some(today)) => cond.add(contract_state(code, today)),
        _ => cond,
    }
}

/// How soon a contract in force counts as ending.
const EXPIRING_DAYS: i64 = 30;

/// The contracts of the partner of the outer query.
fn contracts_here() -> Select<ContractEntity> {
    ContractEntity::find()
        .select_only()
        .column(partner_contract::Column::Id)
        .filter(
            Expr::col((
                partner_contract::Entity,
                partner_contract::Column::PartnerId,
            ))
            .equals((partner::Entity, partner::Column::Id)),
        )
}

/// A partner by its contracts on `today`: one in force, one in force that
/// ends within `EXPIRING_DAYS`, contracts all over, or none at all.
fn contract_state(code: &str, today: NaiveDate) -> Condition {
    let in_force = || {
        contracts_here()
            .filter(partner_contract::Column::StartsOn.lte(today))
            .filter(partner_contract::Column::EndsOn.gte(today))
    };
    match code {
        "in_force" => Condition::all().add(Expr::exists(in_force().into_query())),
        "expiring" => {
            let soon = today + chrono::Duration::days(EXPIRING_DAYS);
            Condition::all().add(Expr::exists(
                in_force()
                    .filter(partner_contract::Column::EndsOn.lte(soon))
                    .into_query(),
            ))
        }
        "ended" => Condition::all()
            .add(Expr::exists(contracts_here().into_query()))
            .add(
                Expr::exists(
                    contracts_here()
                        .filter(partner_contract::Column::EndsOn.gte(today))
                        .into_query(),
                )
                .not(),
            ),
        "none" => Condition::all().add(Expr::exists(contracts_here().into_query()).not()),
        _ => Condition::all(),
    }
}
