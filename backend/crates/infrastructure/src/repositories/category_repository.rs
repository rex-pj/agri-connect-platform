//! Service types (`service_types`) and supply types (`supply_types`): two
//! tables with the same columns and one id sequence, so an id names a row of
//! exactly one of them. The code calls either a category; `category_type`
//! (`CATEGORY_TYPE_SERVICE` / `CATEGORY_TYPE_PRODUCT`) picks the table.

// std
use std::sync::Arc;

// external
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DbErr};
use tracing::error;

use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
    repositories::category_repository_trait::CategoryRepositoryTrait,
    Error, ErrorCode,
};

use crate::repositories::unique_or_internal;

pub struct CategoryRepository {
    db: Arc<DatabaseConnection>,
}

impl CategoryRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

/// The queries of one type table, written once for both: the two entities
/// have the same columns but are distinct Rust types.
macro_rules! type_table {
    ($table:ident, $kind:path) => {
        mod $table {
            use sea_orm::{
                sea_query::Expr, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult,
                PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set,
            };

            use domain::models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory};

            use crate::db_types::DateTimeWithTimeZone;
            use crate::entities::$table::{ActiveModel, Column, Entity, Model};
            use crate::sql::{username_of, QueryAs};

            pub(super) const KIND: &str = $kind;

            #[derive(FromQueryResult)]
            struct AdminRow {
                id: i32,
                name: String,
                slug: String,
                icon: Option<String>,
                color: Option<String>,
                description: Option<String>,
                display_order: i16,
                sector: Option<String>,
                is_active: bool,
                is_demo: bool,
                created_on: DateTimeWithTimeZone,
                updated_on: DateTimeWithTimeZone,
                created_by_name: Option<String>,
                updated_by_name: Option<String>,
            }

            fn to_category(r: Model) -> Category {
                Category {
                    id: r.id,
                    name: r.name,
                    slug: r.slug,
                    icon: r.icon,
                    color: r.color,
                    description: r.description,
                    display_order: r.display_order,
                    sector: r.sector,
                }
            }

            fn to_admin(r: AdminRow) -> CategoryAdmin {
                CategoryAdmin {
                    id: r.id,
                    name: r.name,
                    slug: r.slug,
                    category_type: KIND.to_owned(),
                    icon: r.icon,
                    color: r.color,
                    description: r.description,
                    display_order: r.display_order,
                    sector: r.sector,
                    is_active: r.is_active,
                    is_demo: r.is_demo,
                    created_on: r.created_on.with_timezone(&chrono::Utc),
                    updated_on: r.updated_on.with_timezone(&chrono::Utc),
                    created_by_name: r.created_by_name,
                    updated_by_name: r.updated_by_name,
                }
            }

            /// The shown ones, in their order.
            pub(super) async fn shown<C: ConnectionTrait>(db: &C) -> Result<Vec<Category>, DbErr> {
                Ok(Entity::find()
                    .filter(Column::IsActive.eq(true))
                    .order_by_asc(Column::DisplayOrder)
                    .order_by_asc(Column::Id)
                    .all(db)
                    .await?
                    .into_iter()
                    .map(to_category)
                    .collect())
            }

            /// Every one (hidden too), or the one of `id`, with who made and
            /// last changed it.
            pub(super) async fn admin<C: ConnectionTrait>(
                db: &C,
                id: Option<i32>,
            ) -> Result<Vec<CategoryAdmin>, DbErr> {
                let mut query = admin_select();
                if let Some(id) = id {
                    query = query.filter(Column::Id.eq(id));
                }
                fetch_admin(db, query).await
            }

            /// A page of every one (hidden too), in the same order.
            pub(super) async fn admin_page<C: ConnectionTrait>(
                db: &C,
                limit: i64,
                offset: i64,
            ) -> Result<Vec<CategoryAdmin>, DbErr> {
                let query = admin_select().limit(limit as u64).offset(offset as u64);
                fetch_admin(db, query).await
            }

            /// How many there are, hidden ones too.
            pub(super) async fn count<C: ConnectionTrait>(db: &C) -> Result<i64, DbErr> {
                Ok(Entity::find().count(db).await? as i64)
            }

            async fn fetch_admin<C: ConnectionTrait>(
                db: &C,
                query: Select<Entity>,
            ) -> Result<Vec<CategoryAdmin>, DbErr> {
                Ok(query
                    .into_model::<AdminRow>()
                    .all(db)
                    .await?
                    .into_iter()
                    .map(to_admin)
                    .collect())
            }

            /// The admin's columns, in display order then id.
            fn admin_select() -> Select<Entity> {
                Entity::find()
                    .select_only()
                    .column(Column::Id)
                    .column(Column::Name)
                    .column(Column::Slug)
                    .column(Column::Icon)
                    .column(Column::Color)
                    .column(Column::Description)
                    .column(Column::DisplayOrder)
                    .column(Column::Sector)
                    .column(Column::IsActive)
                    .column(Column::IsDemo)
                    .column(Column::CreatedOn)
                    .column(Column::UpdatedOn)
                    .expr_as(
                        username_of((Entity, Column::CreatedBy)),
                        QueryAs::CreatedByName,
                    )
                    .expr_as(
                        username_of((Entity, Column::UpdatedBy)),
                        QueryAs::UpdatedByName,
                    )
                    .order_by_asc(Column::DisplayOrder)
                    .order_by_asc(Column::Id)
            }

            pub(super) async fn create<C: ConnectionTrait>(
                db: &C,
                c: NewCategory,
            ) -> Result<i32, DbErr> {
                Entity::insert(ActiveModel {
                    name: Set(c.name),
                    slug: Set(c.slug),
                    icon: Set(c.icon),
                    color: Set(c.color),
                    description: Set(c.description),
                    display_order: Set(c.display_order),
                    sector: Set(c.sector),
                    created_by: Set(Some(c.created_by)),
                    ..Default::default()
                })
                .exec(db)
                .await
                .map(|r| r.last_insert_id)
            }

            /// The rows changed (0: not one of this table).
            pub(super) async fn update<C: ConnectionTrait>(
                db: &C,
                id: i32,
                c: UpdateCategory,
            ) -> Result<u64, DbErr> {
                Entity::update_many()
                    .col_expr(Column::Name, Expr::value(c.name))
                    .col_expr(Column::Slug, Expr::value(c.slug))
                    .col_expr(Column::Icon, Expr::value(c.icon))
                    .col_expr(Column::Color, Expr::value(c.color))
                    .col_expr(Column::Description, Expr::value(c.description))
                    .col_expr(Column::DisplayOrder, Expr::value(c.display_order))
                    .col_expr(Column::Sector, Expr::value(c.sector))
                    .col_expr(Column::UpdatedBy, Expr::value(Some(c.updated_by)))
                    .filter(Column::Id.eq(id))
                    .exec(db)
                    .await
                    .map(|r| r.rows_affected)
            }

            /// Which of `slugs` a row other than `exclude_id` uses.
            pub(super) async fn taken_slugs<C: ConnectionTrait>(
                db: &C,
                slugs: &[String],
                exclude_id: Option<i32>,
            ) -> Result<Vec<String>, DbErr> {
                let mut query = Entity::find()
                    .select_only()
                    .column(Column::Slug)
                    .filter(Column::Slug.is_in(slugs.iter().cloned()));
                if let Some(id) = exclude_id {
                    query = query.filter(Column::Id.ne(id));
                }
                query.into_tuple::<String>().all(db).await
            }

            /// The rows changed (0: not one of this table).
            pub(super) async fn set_active<C: ConnectionTrait>(
                db: &C,
                id: i32,
                is_active: bool,
                admin_id: i32,
            ) -> Result<u64, DbErr> {
                Entity::update_many()
                    .col_expr(Column::IsActive, Expr::value(is_active))
                    .col_expr(Column::UpdatedBy, Expr::value(Some(admin_id)))
                    .filter(Column::Id.eq(id))
                    .exec(db)
                    .await
                    .map(|r| r.rows_affected)
            }
        }
    };
}

type_table!(service_type, domain::constants::CATEGORY_TYPE_SERVICE);
type_table!(supply_type, domain::constants::CATEGORY_TYPE_PRODUCT);

fn db_error(context: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| {
        error!("category {context} error: {e}");
        Error::internal("Failed to load categories.")
    }
}

#[async_trait]
impl CategoryRepositoryTrait for CategoryRepository {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, Error> {
        let db = self.db.as_ref();
        let mut all = Vec::new();
        if type_filter.is_none_or(|t| t == service_type::KIND) {
            all.extend(service_type::shown(db).await.map_err(db_error("list"))?);
        }
        if type_filter.is_none_or(|t| t == supply_type::KIND) {
            all.extend(supply_type::shown(db).await.map_err(db_error("list"))?);
        }
        Ok(all)
    }

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, Error> {
        let db = self.db.as_ref();
        let (mut all, supplies) =
            tokio::try_join!(service_type::admin(db, None), supply_type::admin(db, None))
                .map_err(db_error("list_admin"))?;
        all.extend(supplies);
        Ok(all)
    }

    async fn used_ids(&self, category_type: &str) -> Result<Vec<i32>, Error> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        use crate::entities::{product, service};
        let db = self.db.as_ref();
        match category_type {
            service_type::KIND => {
                service::Entity::find()
                    .select_only()
                    .column(service::Column::ServiceTypeId)
                    .filter(service::Column::DeletedAt.is_null())
                    .distinct()
                    .into_tuple::<i32>()
                    .all(db)
                    .await
            }
            _ => {
                product::Entity::find()
                    .select_only()
                    .column(product::Column::SupplyTypeId)
                    .filter(product::Column::DeletedAt.is_null())
                    .distinct()
                    .into_tuple::<i32>()
                    .all(db)
                    .await
            }
        }
        .map_err(db_error("used_ids"))
    }

    async fn list_admin_page(
        &self,
        category_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CategoryAdmin>, Error> {
        let db = self.db.as_ref();
        match category_type {
            service_type::KIND => service_type::admin_page(db, limit, offset).await,
            _ => supply_type::admin_page(db, limit, offset).await,
        }
        .map_err(db_error("list_admin_page"))
    }

    async fn count_admin(&self, category_type: &str) -> Result<i64, Error> {
        let db = self.db.as_ref();
        match category_type {
            service_type::KIND => service_type::count(db).await,
            _ => supply_type::count(db).await,
        }
        .map_err(db_error("count_admin"))
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, Error> {
        let db = self.db.as_ref();
        let (services, supplies) = tokio::try_join!(
            service_type::admin(db, Some(id)),
            supply_type::admin(db, Some(id))
        )
        .map_err(db_error("find_by_id_admin"))?;
        Ok(services.into_iter().chain(supplies).next())
    }

    async fn create(&self, category: NewCategory) -> Result<i32, Error> {
        let db = self.db.as_ref();
        let created = match category.category_type.as_str() {
            CATEGORY_TYPE_SERVICE => service_type::create(db, category).await,
            CATEGORY_TYPE_PRODUCT => supply_type::create(db, category).await,
            _ => {
                return Err(Error::field(
                    "category_type",
                    ErrorCode::InvalidCategoryType,
                ))
            }
        };
        created.map_err(|e| unique_or_internal(e, "slug", ErrorCode::SlugTaken, "category create"))
    }

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), Error> {
        let db = self.db.as_ref();
        let updated = match self.find_by_id_admin(id).await?.map(|c| c.category_type) {
            Some(kind) if kind == service_type::KIND => {
                service_type::update(db, id, category).await
            }
            Some(_) => supply_type::update(db, id, category).await,
            None => return Err(Error::new(ErrorCode::CategoryNotFound)),
        }
        .map_err(|e| unique_or_internal(e, "slug", ErrorCode::SlugTaken, "category update"))?;
        if updated == 0 {
            return Err(Error::new(ErrorCode::CategoryNotFound));
        }
        Ok(())
    }

    async fn taken_slugs(
        &self,
        slugs: &[String],
        exclude_id: Option<i32>,
    ) -> Result<Vec<String>, Error> {
        // A slug names one type across both tables, so links never mix them up.
        let db = self.db.as_ref();
        let (mut taken, supplies) = tokio::try_join!(
            service_type::taken_slugs(db, slugs, exclude_id),
            supply_type::taken_slugs(db, slugs, exclude_id)
        )
        .map_err(db_error("taken_slugs"))?;
        taken.extend(supplies);
        Ok(taken)
    }

    async fn toggle_active(&self, id: i32, is_active: bool, admin_id: i32) -> Result<(), Error> {
        let db = self.db.as_ref();
        let (services, supplies) = tokio::try_join!(
            service_type::set_active(db, id, is_active, admin_id),
            supply_type::set_active(db, id, is_active, admin_id)
        )
        .map_err(|e| {
            error!("category toggle_active error: {e}");
            Error::internal("Failed to update category status.")
        })?;
        if services + supplies == 0 {
            return Err(Error::new(ErrorCode::CategoryNotFound));
        }
        Ok(())
    }
}
