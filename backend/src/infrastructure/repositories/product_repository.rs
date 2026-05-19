use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use sea_orm::sea_query::{Expr, extension::postgres::PgExpr};
use std::sync::Arc;
use tracing::error;

use crate::{
    infrastructure::db_types::DateTimeWithTimeZone,
    domain::{
        models::product::{NewProduct, ProductView, ProviderProduct, UpdateProduct},
        repositories::product_repository_trait::ProductRepositoryTrait,
    },
    entities::{
        business,
        category::{self, Entity as Category},
        product::{self, Entity as ProductEntity},
    },
    view_models::AppError,
};

pub struct ProductRepository {
    db: Arc<DatabaseConnection>,
}

impl ProductRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct ProductViewRow {
    id:           i32,
    name:         String,
    description:  Option<String>,
    price:        Option<i64>,
    unit:         Option<String>,
    image_url:    Option<String>,
    category:     String,
    vendor_name:  String,
    vendor_phone: String,
    is_available: bool,
    created_on:   DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct ProviderProductRow {
    id:          i32,
    name:        String,
    description: Option<String>,
    price:       Option<i64>,
    unit:        Option<String>,
    image_url:   Option<String>,
    category:    String,
    category_id: i32,
    is_active:   bool,
    created_on:  DateTimeWithTimeZone,
}

fn base_product_view_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column(product::Column::ImageUrl)
        .column_as(category::Column::Name, "category")
        .column_as(business::Column::Name, "vendor_name")
        .column_as(business::Column::Phone, "vendor_phone")
        .column_as(product::Column::IsActive, "is_available")
        .column(product::Column::CreatedOn)
        .join(JoinType::InnerJoin, product::Relation::Business.def())
        .join(JoinType::InnerJoin, product::Relation::Category.def())
        .filter(business::Column::IsActive.eq(true))
        .filter(business::Column::DeletedAt.is_null())
        .filter(category::Column::IsActive.eq(true))
}

fn base_provider_product_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column(product::Column::ImageUrl)
        .column_as(category::Column::Name, "category")
        .column(product::Column::CategoryId)
        .column(product::Column::IsActive)
        .column(product::Column::CreatedOn)
        .join(JoinType::InnerJoin, product::Relation::Category.def())
}

#[async_trait]
impl ProductRepositoryTrait for ProductRepository {
    async fn browse(
        &self,
        category_id: Option<i32>,
        name: Option<String>,
        min_price: Option<i64>,
        max_price: Option<i64>,
        sort: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, AppError> {
        let mut query = base_product_view_query()
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null());

        if let Some(id) = category_id {
            query = query.filter(product::Column::CategoryId.eq(id));
        }
        if let Some(ref q) = name {
            let pattern = format!("%{}%", q);
            query = query.filter(
                Condition::any()
                    .add(Expr::col((product::Entity, product::Column::Name)).ilike(pattern.clone()))
                    .add(Expr::col((product::Entity, product::Column::Description)).ilike(pattern)),
            );
        }
        if let Some(min) = min_price {
            query = query.filter(product::Column::Price.gte(min));
        }
        if let Some(max) = max_price {
            query = query.filter(product::Column::Price.lte(max));
        }

        let query = match sort.as_deref() {
            Some("price_asc")  => query.order_by_asc(product::Column::Price),
            Some("price_desc") => query.order_by_desc(product::Column::Price),
            _                  => query.order_by_desc(product::Column::CreatedOn),
        };

        query
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<ProductViewRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_view_row).collect())
            .map_err(|e| {
                error!("product browse error: {e}");
                AppError::Internal("Failed to load products.".into())
            })
    }

    async fn count_browse(
        &self,
        category_id: Option<i32>,
        name: Option<String>,
        min_price: Option<i64>,
        max_price: Option<i64>,
    ) -> Result<i64, AppError> {
        let mut query = ProductEntity::find()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .join(JoinType::InnerJoin, product::Relation::Category.def())
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(category::Column::IsActive.eq(true));

        if let Some(id) = category_id {
            query = query.filter(product::Column::CategoryId.eq(id));
        }
        if let Some(ref q) = name {
            let pattern = format!("%{}%", q);
            query = query.filter(
                Condition::any()
                    .add(Expr::col((product::Entity, product::Column::Name)).ilike(pattern.clone()))
                    .add(Expr::col((product::Entity, product::Column::Description)).ilike(pattern)),
            );
        }
        if let Some(min) = min_price {
            query = query.filter(product::Column::Price.gte(min));
        }
        if let Some(max) = max_price {
            query = query.filter(product::Column::Price.lte(max));
        }

        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("product count_browse error: {e}");
                AppError::Internal("Failed to load products.".into())
            })
    }

    async fn featured(&self, limit: i64) -> Result<Vec<ProductView>, AppError> {
        base_product_view_query()
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .order_by_desc(product::Column::IsActive)
            .order_by_desc(product::Column::CreatedOn)
            .limit(limit as u64)
            .into_model::<ProductViewRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_view_row).collect())
            .map_err(|e| {
                error!("product featured error: {e}");
                AppError::Internal("Failed to load products.".into())
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<ProductView>, AppError> {
        base_product_view_query()
            .filter(product::Column::Id.eq(id))
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .into_model::<ProductViewRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(map_view_row))
            .map_err(|e| {
                error!("product find_by_id error: {e}");
                AppError::Internal("Failed to load product details.".into())
            })
    }

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, AppError> {
        base_provider_product_query()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .filter(product::Column::DeletedAt.is_null())
            .order_by_desc(product::Column::CreatedOn)
            .into_model::<ProviderProductRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_provider_row).collect())
            .map_err(|e| {
                error!("product list_by_user error (user_id={user_id}): {e}");
                AppError::Internal("Failed to load products.".into())
            })
    }

    async fn check_category_is_product(&self, category_id: i32) -> Result<bool, AppError> {
        Category::find_by_id(category_id)
            .filter(category::Column::Type.eq("product"))
            .filter(category::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("product check_category_is_product error: {e}");
                AppError::Internal("Failed to check category.".into())
            })
    }

    async fn create(&self, product: NewProduct) -> Result<i32, AppError> {
        let new_product = product::ActiveModel {
            business_id:  Set(product.business_id),
            category_id:  Set(product.category_id),
            name:         Set(product.name),
            description:  Set(product.description),
            price:        Set(product.price),
            unit:         Set(product.unit),
            image_url:    Set(product.image_url),
            created_by:   Set(Some(product.created_by)),
            ..Default::default()
        };

        ProductEntity::insert(new_product)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("product create error: {e}");
                AppError::Internal("Failed to create product.".into())
            })
    }

    async fn find_provider_product_by_id(
        &self,
        id: i32,
    ) -> Result<Option<ProviderProduct>, AppError> {
        base_provider_product_query()
            .filter(product::Column::Id.eq(id))
            .into_model::<ProviderProductRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(map_provider_row))
            .map_err(|e| {
                error!("product find_provider_product_by_id error: {e}");
                AppError::Internal("Product created but failed to reload.".into())
            })
    }

    async fn update(&self, product: UpdateProduct) -> Result<bool, AppError> {
        let db = self.db.as_ref();

        // Verify ownership before updating
        let owned = ProductEntity::find()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(product::Column::Id.eq(product.id))
            .filter(product::Column::DeletedAt.is_null())
            .filter(business::Column::OwnerId.eq(product.owner_user))
            .filter(business::Column::DeletedAt.is_null())
            .count(db)
            .await
            .map_err(|e| {
                error!(
                    "product update ownership check error (id={}, user={}): {e}",
                    product.id, product.owner_user
                );
                AppError::Internal("Failed to update product.".into())
            })?;

        if owned == 0 {
            return Ok(false);
        }

        ProductEntity::update_many()
            .col_expr(product::Column::CategoryId, Expr::value(product.category_id))
            .col_expr(product::Column::Name,        Expr::value(product.name))
            .col_expr(product::Column::Description, Expr::value(product.description))
            .col_expr(product::Column::Price,       Expr::value(product.price))
            .col_expr(product::Column::Unit,        Expr::value(product.unit))
            .col_expr(product::Column::ImageUrl,    Expr::value(product.image_url))
            .col_expr(product::Column::UpdatedOn,   Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(product.id))
            .exec(db)
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!(
                    "product update error (id={}, user={}): {e}",
                    product.id, product.owner_user
                );
                AppError::Internal("Failed to update product.".into())
            })
    }

    async fn soft_delete(&self, id: i32, owner_user: i32) -> Result<bool, AppError> {
        let db = self.db.as_ref();

        // Verify ownership before deleting
        let owned = ProductEntity::find()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(product::Column::Id.eq(id))
            .filter(product::Column::DeletedAt.is_null())
            .filter(business::Column::OwnerId.eq(owner_user))
            .filter(business::Column::DeletedAt.is_null())
            .count(db)
            .await
            .map_err(|e| {
                error!("product soft_delete ownership check error (id={id}, user={owner_user}): {e}");
                AppError::Internal("Failed to delete product.".into())
            })?;

        if owned == 0 {
            return Ok(false);
        }

        ProductEntity::update_many()
            .col_expr(product::Column::DeletedAt, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(id))
            .exec(db)
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("product soft_delete error (id={id}, user={owner_user}): {e}");
                AppError::Internal("Failed to delete product.".into())
            })
    }

    async fn count_active(&self) -> Result<i64, AppError> {
        ProductEntity::find()
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("product count_active error: {e}");
                AppError::Internal("Failed to load statistics.".into())
            })
    }
}

fn map_view_row(r: ProductViewRow) -> ProductView {
    ProductView {
        id:           r.id,
        name:         r.name,
        description:  r.description,
        price:        r.price,
        unit:         r.unit,
        image_url:    r.image_url,
        category:     r.category,
        vendor_name:  r.vendor_name,
        vendor_phone: r.vendor_phone,
        is_available: r.is_available,
        created_on:   r.created_on.with_timezone(&Utc),
    }
}

fn map_provider_row(r: ProviderProductRow) -> ProviderProduct {
    ProviderProduct {
        id:          r.id,
        name:        r.name,
        description: r.description,
        price:       r.price,
        unit:        r.unit,
        image_url:   r.image_url,
        category:    r.category,
        category_id: r.category_id,
        is_active:   r.is_active,
        created_on:  r.created_on.with_timezone(&Utc),
    }
}

