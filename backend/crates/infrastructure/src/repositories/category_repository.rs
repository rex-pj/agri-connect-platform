// std
use std::sync::Arc;

// external
use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tracing::error;

use domain::{Error, ErrorCode};

use crate::db_types::DateTimeWithTimeZone;

use domain::{
    models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
    repositories::category_repository_trait::CategoryRepositoryTrait,
};

use crate::entities::category::{self, Entity as CategoryEntity};
use crate::repositories::unique_or_internal;
use crate::sql::{username_of, QueryAs};

pub struct CategoryRepository {
    db: Arc<DatabaseConnection>,
}

impl CategoryRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct CategoryAdminRow {
    id: i32,
    name: String,
    slug: String,
    category_type: String,
    icon: Option<String>,
    color: Option<String>,
    description: Option<String>,
    display_order: i16,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    created_by_name: Option<String>,
    updated_by_name: Option<String>,
}

fn to_category(r: category::Model) -> Category {
    Category {
        id: r.id,
        name: r.name,
        slug: r.slug,
        icon: r.icon,
        color: r.color,
        description: r.description,
        display_order: r.display_order,
    }
}

fn to_category_admin(r: CategoryAdminRow) -> CategoryAdmin {
    CategoryAdmin {
        id: r.id,
        name: r.name,
        slug: r.slug,
        category_type: r.category_type,
        icon: r.icon,
        color: r.color,
        description: r.description,
        display_order: r.display_order,
        is_active: r.is_active,
        created_on: r.created_on.with_timezone(&chrono::Utc),
        updated_on: r.updated_on.with_timezone(&chrono::Utc),
        created_by_name: r.created_by_name,
        updated_by_name: r.updated_by_name,
    }
}

#[async_trait]
impl CategoryRepositoryTrait for CategoryRepository {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, Error> {
        let db = self.db.as_ref();
        let mut query = CategoryEntity::find().filter(category::Column::IsActive.eq(true));

        if let Some(cat_type) = type_filter {
            query = query
                .filter(category::Column::Type.eq(cat_type))
                .order_by_asc(category::Column::DisplayOrder);
        } else {
            query = query
                .order_by_asc(category::Column::Type)
                .order_by_asc(category::Column::DisplayOrder);
        }

        query
            .all(db)
            .await
            .map(|rows| rows.into_iter().map(to_category).collect())
            .map_err(|e| {
                error!("category list error: {e}");
                Error::internal("Failed to load categories.")
            })
    }

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, Error> {
        CategoryEntity::find()
            .select_only()
            .column(category::Column::Id)
            .column(category::Column::Name)
            .column(category::Column::Slug)
            .column_as(category::Column::Type, QueryAs::CategoryType)
            .column(category::Column::Icon)
            .column(category::Column::Color)
            .column(category::Column::Description)
            .column(category::Column::DisplayOrder)
            .column(category::Column::IsActive)
            .column(category::Column::CreatedOn)
            .column(category::Column::UpdatedOn)
            .expr_as(
                username_of((category::Entity, category::Column::CreatedBy)),
                QueryAs::CreatedByName,
            )
            .expr_as(
                username_of((category::Entity, category::Column::UpdatedBy)),
                QueryAs::UpdatedByName,
            )
            .order_by_asc(category::Column::Type)
            .order_by_asc(category::Column::DisplayOrder)
            .into_model::<CategoryAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_category_admin).collect())
            .map_err(|e| {
                error!("category list_admin error: {e}");
                Error::internal("Failed to load categories.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, Error> {
        CategoryEntity::find_by_id(id)
            .select_only()
            .column(category::Column::Id)
            .column(category::Column::Name)
            .column(category::Column::Slug)
            .column_as(category::Column::Type, QueryAs::CategoryType)
            .column(category::Column::Icon)
            .column(category::Column::Color)
            .column(category::Column::Description)
            .column(category::Column::DisplayOrder)
            .column(category::Column::IsActive)
            .column(category::Column::CreatedOn)
            .column(category::Column::UpdatedOn)
            .expr_as(
                username_of((category::Entity, category::Column::CreatedBy)),
                QueryAs::CreatedByName,
            )
            .expr_as(
                username_of((category::Entity, category::Column::UpdatedBy)),
                QueryAs::UpdatedByName,
            )
            .into_model::<CategoryAdminRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(to_category_admin))
            .map_err(|e| {
                error!("category find_by_id_admin error: {e}");
                Error::internal("Failed to load category.")
            })
    }

    async fn create(&self, category: NewCategory) -> Result<i32, Error> {
        let new_cat = category::ActiveModel {
            name: Set(category.name),
            slug: Set(category.slug),
            r#type: Set(category.category_type),
            icon: Set(category.icon),
            color: Set(category.color),
            description: Set(category.description),
            display_order: Set(category.display_order),
            created_by: Set(Some(category.created_by)),
            ..Default::default()
        };

        CategoryEntity::insert(new_cat)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| unique_or_internal(e, "slug", ErrorCode::SlugTaken, "category create"))
    }

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), Error> {
        let result = CategoryEntity::update_many()
            .col_expr(category::Column::Name, Expr::value(category.name))
            .col_expr(category::Column::Slug, Expr::value(category.slug))
            .col_expr(category::Column::Icon, Expr::value(category.icon))
            .col_expr(category::Column::Color, Expr::value(category.color))
            .col_expr(
                category::Column::Description,
                Expr::value(category.description),
            )
            .col_expr(
                category::Column::DisplayOrder,
                Expr::value(category.display_order),
            )
            .col_expr(
                category::Column::UpdatedBy,
                Expr::value(Some(category.updated_by)),
            )
            .filter(category::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| unique_or_internal(e, "slug", ErrorCode::SlugTaken, "category update"))?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::CategoryNotFound));
        }
        Ok(())
    }

    async fn taken_slugs(
        &self,
        slugs: &[String],
        exclude_id: Option<i32>,
    ) -> Result<Vec<String>, Error> {
        let mut query = CategoryEntity::find()
            .select_only()
            .column(category::Column::Slug)
            .filter(category::Column::Slug.is_in(slugs.iter().cloned()));
        if let Some(id) = exclude_id {
            query = query.filter(category::Column::Id.ne(id));
        }
        query
            .into_tuple::<String>()
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("category taken_slugs error: {e}");
                Error::internal("Failed to load categories.")
            })
    }

    async fn toggle_active(&self, id: i32, is_active: bool, admin_id: i32) -> Result<(), Error> {
        let result = CategoryEntity::update_many()
            .col_expr(category::Column::IsActive, Expr::value(is_active))
            .col_expr(category::Column::UpdatedBy, Expr::value(Some(admin_id)))
            .filter(category::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("category toggle_active error: {e}");
                Error::internal("Failed to update category status.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::CategoryNotFound));
        }
        Ok(())
    }
}
