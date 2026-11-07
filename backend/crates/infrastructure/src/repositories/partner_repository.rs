//! Partners, their contracts, each contract's categories and sponsored
//! providers. Lists are put together from a few queries by id (a partner
//! has a handful of contracts), never one per row.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use tracing::error;

use domain::{
    models::partner::{
        ActiveContract, Contract, NewContract, Partner, PartnerFields, SponsoredBusiness,
    },
    repositories::partner_repository_trait::PartnerRepositoryTrait,
    Error,
};

use crate::entities::{
    business, location,
    partner::{self, Entity as PartnerEntity},
    partner_contract::{self, Entity as ContractEntity},
    partner_contract_category::{self, Entity as ContractCategory},
    partner_sponsored::{self, Entity as Sponsored},
    user,
};

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

    /// Partners as the domain has them, with their account's username.
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
                .map(|u| (u.id, u.username))
                .collect()
        };
        Ok(rows
            .into_iter()
            .map(|p| Partner {
                account_username: p.account_id.and_then(|id| names.get(&id).cloned()),
                id: p.id,
                name: p.name,
                kind: p.kind,
                contact_name: p.contact_name,
                phone: p.phone,
                email: p.email,
                is_active: p.is_active,
                created_on: p.created_on.with_timezone(&Utc),
            })
            .collect())
    }

    /// Contracts as the domain has them: area label, categories, sponsored.
    async fn assemble(&self, rows: Vec<partner_contract::Model>) -> Result<Vec<Contract>, Error> {
        let db = self.db.as_ref();
        let ids: Vec<i32> = rows.iter().map(|c| c.id).collect();
        let location_ids: Vec<i32> = rows.iter().map(|c| c.location_id).collect();
        let labels: HashMap<i32, String> = location::Entity::find()
            .filter(location::Column::Id.is_in(location_ids))
            .all(db)
            .await
            .map_err(Self::fail("contract areas"))?
            .into_iter()
            .map(|l| (l.id, l.label))
            .collect();
        let mut categories: HashMap<i32, Vec<i32>> = HashMap::new();
        for c in ContractCategory::find()
            .filter(partner_contract_category::Column::ContractId.is_in(ids.clone()))
            .order_by_asc(partner_contract_category::Column::CategoryId)
            .all(db)
            .await
            .map_err(Self::fail("contract categories"))?
        {
            categories
                .entry(c.contract_id)
                .or_default()
                .push(c.category_id);
        }
        let sponsored_rows = Sponsored::find()
            .filter(partner_sponsored::Column::ContractId.is_in(ids))
            .all(db)
            .await
            .map_err(Self::fail("sponsored"))?;
        let business_names: HashMap<i32, String> = business::Entity::find()
            .filter(business::Column::Id.is_in(sponsored_rows.iter().map(|s| s.business_id)))
            .all(db)
            .await
            .map_err(Self::fail("sponsored names"))?
            .into_iter()
            .map(|b| (b.id, b.name))
            .collect();
        let mut sponsored: HashMap<i32, Vec<SponsoredBusiness>> = HashMap::new();
        for s in sponsored_rows {
            if let Some(name) = business_names.get(&s.business_id) {
                sponsored
                    .entry(s.contract_id)
                    .or_default()
                    .push(SponsoredBusiness {
                        business_id: s.business_id,
                        name: name.clone(),
                    });
            }
        }
        Ok(rows
            .into_iter()
            .map(|c| Contract {
                location_label: labels.get(&c.location_id).cloned().unwrap_or_default(),
                category_ids: categories.remove(&c.id).unwrap_or_default(),
                sponsored: sponsored.remove(&c.id).unwrap_or_default(),
                id: c.id,
                partner_id: c.partner_id,
                location_id: c.location_id,
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

#[async_trait]
impl PartnerRepositoryTrait for PartnerRepository {
    async fn list(&self) -> Result<Vec<Partner>, Error> {
        let rows = PartnerEntity::find()
            .order_by_desc(partner::Column::IsActive)
            .order_by_asc(partner::Column::Name)
            .order_by_asc(partner::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?;
        self.with_accounts(rows).await
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

    async fn create_contract(&self, c: &NewContract, actor_id: i32) -> Result<i32, Error> {
        let fail = Self::fail("create_contract");
        let txn = self.db.begin().await.map_err(&fail)?;
        let id = ContractEntity::insert(partner_contract::ActiveModel {
            partner_id: Set(c.partner_id),
            location_id: Set(c.location_id),
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
        ContractCategory::insert_many(c.category_ids.iter().map(|category_id| {
            partner_contract_category::ActiveModel {
                contract_id: Set(id),
                category_id: Set(*category_id),
            }
        }))
        .exec(&txn)
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
                location_id: c.location_id,
                monthly_fee: c.monthly_fee,
                category_ids: c.category_ids,
                business_ids: c.sponsored.iter().map(|s| s.business_id).collect(),
            })
            .collect())
    }
}
