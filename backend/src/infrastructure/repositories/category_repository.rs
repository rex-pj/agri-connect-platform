use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use sea_orm::sea_query::Expr;
use std::sync::Arc;
use tracing::error;

use crate::{
    domain::{
        models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
        repositories::category_repository_trait::CategoryRepositoryTrait,
    },
    entities::category::{self, Entity as CategoryEntity},
    view_models::AppError,
};

pub struct CategoryRepository {
    db: Arc<DatabaseConnection>,
}

impl CategoryRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn to_category(r: category::Model) -> Category {
    Category {
        id:            r.id,
        name:          r.name,
        slug:          r.slug,
        icon:          r.icon,
        display_order: r.display_order,
    }
}

fn to_category_admin(r: category::Model) -> CategoryAdmin {
    CategoryAdmin {
        id:            r.id,
        name:          r.name,
        slug:          r.slug,
        category_type: r.r#type,
        icon:          r.icon,
        display_order: r.display_order,
        is_active:     r.is_active,
    }
}

#[async_trait]
impl CategoryRepositoryTrait for CategoryRepository {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, AppError> {
        let db = self.db.as_ref();
        let mut query = CategoryEntity::find()
            .filter(category::Column::IsActive.eq(true));

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
                AppError::Internal("Failed to load categories.".into())
            })
    }

    async fn count_active(&self) -> Result<i64, AppError> {
        CategoryEntity::find()
            .filter(category::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("category count_active error: {e}");
                AppError::Internal("Failed to read categories.".into())
            })
    }

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, AppError> {
        CategoryEntity::find()
            .order_by_asc(category::Column::Type)
            .order_by_asc(category::Column::DisplayOrder)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_category_admin).collect())
            .map_err(|e| {
                error!("category list_admin error: {e}");
                AppError::Internal("Failed to load categories.".into())
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, AppError> {
        CategoryEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(to_category_admin))
            .map_err(|e| {
                error!("category find_by_id_admin error: {e}");
                AppError::Internal("Failed to load category.".into())
            })
    }

    async fn create(&self, category: NewCategory) -> Result<i32, AppError> {
        let new_cat = category::ActiveModel {
            name:          Set(category.name),
            slug:          Set(category.slug),
            r#type:        Set(category.category_type),
            icon:          Set(category.icon),
            display_order: Set(category.display_order),
            ..Default::default()
        };

        CategoryEntity::insert(new_cat)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("category create error: {e}");
                AppError::Internal("Failed to create category.".into())
            })
    }

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), AppError> {
        let result = CategoryEntity::update_many()
            .col_expr(category::Column::Name,         Expr::value(category.name))
            .col_expr(category::Column::Slug,         Expr::value(category.slug))
            .col_expr(category::Column::Icon,         Expr::value(category.icon))
            .col_expr(category::Column::DisplayOrder, Expr::value(category.display_order))
            .filter(category::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("category update error: {e}");
                AppError::Internal("Failed to update category.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("Category not found.".into()));
        }
        Ok(())
    }

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), AppError> {
        let result = CategoryEntity::update_many()
            .col_expr(category::Column::IsActive, Expr::value(is_active))
            .filter(category::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("category toggle_active error: {e}");
                AppError::Internal("Failed to update category status.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("Category not found.".into()));
        }
        Ok(())
    }
}
