//! The short lists the admin keeps, each entry named elsewhere by its code:
//! the sectors (`sectors`) and the alert kinds (`alert_kinds`). The two
//! tables have the same columns; one macro writes both repositories, so the
//! queries cannot drift apart.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use tracing::error;

use domain::{
    models::coded::{CodedEntry, CodedFields},
    repositories::coded_repository_trait::CodedRepositoryTrait,
    Error, ErrorCode,
};

use crate::{
    entities::{alert_kind, sector},
    repositories::unique_or_internal,
};

macro_rules! coded_repository {
    ($repo:ident, $table:ident, $taken:expr, $what:literal) => {
        pub struct $repo {
            db: Arc<DatabaseConnection>,
        }

        impl $repo {
            pub fn new(db: Arc<DatabaseConnection>) -> Self {
                Self { db }
            }

            fn fail(step: &'static str) -> impl Fn(DbErr) -> Error {
                move |e| {
                    error!(concat!($what, " {} error: {}"), step, e);
                    Error::internal(concat!("Failed to load or save the ", $what, "."))
                }
            }

            fn entry(m: $table::Model) -> CodedEntry {
                CodedEntry {
                    id: m.id,
                    code: m.code,
                    name: m.name,
                    icon: m.icon,
                    display_order: m.display_order,
                    is_active: m.is_active,
                    is_demo: m.is_demo,
                }
            }
        }

        #[async_trait]
        impl CodedRepositoryTrait for $repo {
            async fn list_all(&self) -> Result<Vec<CodedEntry>, Error> {
                $table::Entity::find()
                    .order_by_asc($table::Column::DisplayOrder)
                    .order_by_asc($table::Column::Name)
                    .order_by_asc($table::Column::Id)
                    .all(self.db.as_ref())
                    .await
                    .map(|all| all.into_iter().map(Self::entry).collect())
                    .map_err(Self::fail("list"))
            }

            async fn find(&self, id: i32) -> Result<Option<CodedEntry>, Error> {
                $table::Entity::find_by_id(id)
                    .one(self.db.as_ref())
                    .await
                    .map(|m| m.map(Self::entry))
                    .map_err(Self::fail("find"))
            }

            async fn create(
                &self,
                code: &str,
                f: &CodedFields,
                actor_id: i32,
            ) -> Result<i32, Error> {
                $table::Entity::insert($table::ActiveModel {
                    code: Set(code.to_owned()),
                    name: Set(f.name.clone()),
                    icon: Set(f.icon.clone()),
                    display_order: Set(f.display_order),
                    created_by: Set(Some(actor_id)),
                    ..Default::default()
                })
                .exec(self.db.as_ref())
                .await
                .map(|r| r.last_insert_id)
                .map_err(|e| unique_or_internal(e, "name", $taken, concat!($what, " create")))
            }

            async fn update(&self, id: i32, f: &CodedFields, actor_id: i32) -> Result<bool, Error> {
                $table::Entity::update_many()
                    .col_expr($table::Column::Name, Expr::value(f.name.clone()))
                    .col_expr($table::Column::Icon, Expr::value(f.icon.clone()))
                    .col_expr($table::Column::DisplayOrder, Expr::value(f.display_order))
                    .col_expr($table::Column::UpdatedBy, Expr::value(Some(actor_id)))
                    .filter($table::Column::Id.eq(id))
                    .exec(self.db.as_ref())
                    .await
                    .map(|r| r.rows_affected > 0)
                    .map_err(|e| unique_or_internal(e, "name", $taken, concat!($what, " update")))
            }

            async fn set_active(
                &self,
                id: i32,
                active: bool,
                actor_id: i32,
            ) -> Result<Option<(String, bool)>, Error> {
                let Some(current) = $table::Entity::find_by_id(id)
                    .one(self.db.as_ref())
                    .await
                    .map_err(Self::fail("set_active"))?
                else {
                    return Ok(None);
                };
                if current.is_active == active {
                    return Ok(Some((current.name, false)));
                }
                $table::Entity::update_many()
                    .col_expr($table::Column::IsActive, Expr::value(active))
                    .col_expr($table::Column::UpdatedBy, Expr::value(Some(actor_id)))
                    .filter($table::Column::Id.eq(id))
                    .exec(self.db.as_ref())
                    .await
                    .map_err(Self::fail("set_active"))?;
                Ok(Some((current.name, true)))
            }
        }
    };
}

coded_repository!(
    SectorRepository,
    sector,
    ErrorCode::SectorNameTaken,
    "sector"
);
coded_repository!(
    AlertKindRepository,
    alert_kind,
    ErrorCode::AlertKindNameTaken,
    "alert kind"
);
