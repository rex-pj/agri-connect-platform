use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::{extension::postgres::PgExpr, Expr};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::product::{
        AdminProduct, NewProduct, ProductFilter, ProductView, ProviderProduct, UpdateProduct,
    },
    repositories::product_repository_trait::ProductRepositoryTrait,
};

use domain::Error;

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        category::{self, Entity as Category},
        product::{self, Entity as ProductEntity},
    },
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
    id: i32,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    unit: Option<String>,
    category: String,
    vendor_name: String,
    vendor_phone: String,
    is_available: bool,
    created_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct AdminProductRow {
    id: i32,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    unit: Option<String>,
    category: String,
    category_id: i32,
    business_id: i32,
    business_name: String,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct ProviderProductRow {
    id: i32,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    unit: Option<String>,
    category: String,
    category_id: i32,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
}

fn base_product_view_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
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
        .column_as(category::Column::Name, "category")
        .column(product::Column::CategoryId)
        .column(product::Column::IsActive)
        .column(product::Column::CreatedOn)
        .join(JoinType::InnerJoin, product::Relation::Category.def())
}

fn base_admin_product_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column_as(category::Column::Name, "category")
        .column(product::Column::CategoryId)
        .column(product::Column::BusinessId)
        .column_as(business::Column::Name, "business_name")
        .column(product::Column::IsActive)
        .column(product::Column::CreatedOn)
        .column(product::Column::UpdatedOn)
        .join(JoinType::InnerJoin, product::Relation::Business.def())
        .join(JoinType::InnerJoin, product::Relation::Category.def())
        .filter(product::Column::DeletedAt.is_null())
        .filter(business::Column::DeletedAt.is_null())
}

#[async_trait]
impl ProductRepositoryTrait for ProductRepository {
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminProduct>, Error> {
        let mut query = base_admin_product_query();
        if let Some(bid) = provider_id {
            query = query.filter(product::Column::BusinessId.eq(bid));
        }
        query
            .order_by_desc(product::Column::CreatedOn)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<AdminProductRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_admin_row).collect())
            .map_err(|e| {
                error!("product list_admin error: {e}");
                Error::internal("Failed to load products.")
            })
    }

    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error> {
        let mut query = ProductEntity::find()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(product::Column::DeletedAt.is_null())
            .filter(business::Column::DeletedAt.is_null());
        if let Some(bid) = provider_id {
            query = query.filter(product::Column::BusinessId.eq(bid));
        }
        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("product count_admin error: {e}");
                Error::internal("Failed to count products.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminProduct>, Error> {
        base_admin_product_query()
            .filter(product::Column::Id.eq(id))
            .into_model::<AdminProductRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(map_admin_row))
            .map_err(|e| {
                error!("product find_by_id_admin error (id={id}): {e}");
                Error::internal("Failed to load product.")
            })
    }

    async fn admin_toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error> {
        ProductEntity::update_many()
            .col_expr(product::Column::IsActive, Expr::value(is_active))
            .col_expr(product::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(id))
            .filter(product::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("product admin_toggle_active error (id={id}): {e}");
                Error::internal("Failed to update product.")
            })
    }

    async fn admin_delete(&self, id: i32) -> Result<bool, Error> {
        ProductEntity::update_many()
            .col_expr(product::Column::DeletedAt, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(id))
            .filter(product::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("product admin_delete error (id={id}): {e}");
                Error::internal("Failed to delete product.")
            })
    }

    async fn browse(
        &self,
        filter: &ProductFilter,
        sort: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error> {
        let query = apply_filter(
            base_product_view_query()
                .filter(product::Column::IsActive.eq(true))
                .filter(product::Column::DeletedAt.is_null()),
            filter,
        );

        let query = match sort.as_deref() {
            Some("price_asc") => query.order_by_asc(product::Column::Price),
            Some("price_desc") => query.order_by_desc(product::Column::Price),
            _ => query.order_by_desc(product::Column::CreatedOn),
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
                Error::internal("Failed to load products.")
            })
    }

    async fn count_browse(&self, filter: &ProductFilter) -> Result<i64, Error> {
        let query = apply_filter(
            ProductEntity::find()
                .join(JoinType::InnerJoin, product::Relation::Business.def())
                .join(JoinType::InnerJoin, product::Relation::Category.def())
                .filter(product::Column::IsActive.eq(true))
                .filter(product::Column::DeletedAt.is_null())
                .filter(business::Column::IsActive.eq(true))
                .filter(business::Column::DeletedAt.is_null())
                .filter(category::Column::IsActive.eq(true)),
            filter,
        );

        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("product count_browse error: {e}");
                Error::internal("Failed to load products.")
            })
    }

    async fn featured(&self, limit: i64) -> Result<Vec<ProductView>, Error> {
        base_product_view_query()
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .order_by_desc(product::Column::CreatedOn)
            .limit(limit as u64)
            .into_model::<ProductViewRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_view_row).collect())
            .map_err(|e| {
                error!("product featured error: {e}");
                Error::internal("Failed to load products.")
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<ProductView>, Error> {
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
                Error::internal("Failed to load product details.")
            })
    }

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, Error> {
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
                Error::internal("Failed to load products.")
            })
    }

    async fn check_category_is_product(&self, category_id: i32) -> Result<bool, Error> {
        Category::find_by_id(category_id)
            .filter(category::Column::Type.eq("product"))
            .filter(category::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("product check_category_is_product error: {e}");
                Error::internal("Failed to check category.")
            })
    }

    async fn create(&self, product: NewProduct) -> Result<i32, Error> {
        let new_product = product::ActiveModel {
            business_id: Set(product.business_id),
            category_id: Set(product.category_id),
            name: Set(product.name),
            description: Set(product.description),
            price: Set(product.price),
            unit: Set(product.unit),
            created_by: Set(Some(product.created_by)),
            ..Default::default()
        };

        ProductEntity::insert(new_product)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("product create error: {e}");
                Error::internal("Failed to create product.")
            })
    }

    async fn find_provider_product_by_id(&self, id: i32) -> Result<Option<ProviderProduct>, Error> {
        base_provider_product_query()
            .filter(product::Column::Id.eq(id))
            .into_model::<ProviderProductRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(map_provider_row))
            .map_err(|e| {
                error!("product find_provider_product_by_id error: {e}");
                Error::internal("Product created but failed to reload.")
            })
    }

    async fn update(&self, product: UpdateProduct) -> Result<bool, Error> {
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
                Error::internal("Failed to update product.")
            })?;

        if owned == 0 {
            return Ok(false);
        }

        ProductEntity::update_many()
            .col_expr(
                product::Column::CategoryId,
                Expr::value(product.category_id),
            )
            .col_expr(product::Column::Name, Expr::value(product.name))
            .col_expr(
                product::Column::Description,
                Expr::value(product.description),
            )
            .col_expr(product::Column::Price, Expr::value(product.price))
            .col_expr(product::Column::Unit, Expr::value(product.unit))
            .col_expr(product::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(product.id))
            .exec(db)
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!(
                    "product update error (id={}, user={}): {e}",
                    product.id, product.owner_user
                );
                Error::internal("Failed to update product.")
            })
    }

    async fn soft_delete(&self, id: i32, owner_user: i32) -> Result<bool, Error> {
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
                error!(
                    "product soft_delete ownership check error (id={id}, user={owner_user}): {e}"
                );
                Error::internal("Failed to delete product.")
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
                Error::internal("Failed to delete product.")
            })
    }

    async fn count_active(&self) -> Result<i64, Error> {
        ProductEntity::find()
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("product count_active error: {e}");
                Error::internal("Failed to load statistics.")
            })
    }
}

fn map_admin_row(r: AdminProductRow) -> AdminProduct {
    AdminProduct {
        id: r.id,
        name: r.name,
        description: r.description,
        price: r.price,
        unit: r.unit,
        category: r.category,
        category_id: r.category_id,
        business_id: r.business_id,
        business_name: r.business_name,
        is_active: r.is_active,
        created_on: r.created_on.with_timezone(&Utc),
        updated_on: r.updated_on.with_timezone(&Utc),
    }
}

fn map_view_row(r: ProductViewRow) -> ProductView {
    ProductView {
        id: r.id,
        name: r.name,
        description: r.description,
        price: r.price,
        unit: r.unit,
        category: r.category,
        vendor_name: r.vendor_name,
        vendor_phone: r.vendor_phone,
        is_available: r.is_available,
        created_on: r.created_on.with_timezone(&Utc),
    }
}

fn map_provider_row(r: ProviderProductRow) -> ProviderProduct {
    ProviderProduct {
        id: r.id,
        name: r.name,
        description: r.description,
        price: r.price,
        unit: r.unit,
        category: r.category,
        category_id: r.category_id,
        is_active: r.is_active,
        created_on: r.created_on.with_timezone(&Utc),
    }
}

/// The public search criteria, shared by `browse` and `count_browse` so the
/// page count can never disagree with the page contents.
fn apply_filter<Q: QueryFilter>(mut query: Q, filter: &ProductFilter) -> Q {
    if let Some(id) = filter.category_id {
        query = query.filter(product::Column::CategoryId.eq(id));
    }
    if let Some(q) = &filter.name {
        let pattern = format!("%{q}%");
        query = query.filter(
            Condition::any()
                .add(Expr::col((product::Entity, product::Column::Name)).ilike(pattern.clone()))
                .add(Expr::col((product::Entity, product::Column::Description)).ilike(pattern)),
        );
    }
    if let Some(min) = filter.min_price {
        query = query.filter(product::Column::Price.gte(min));
    }
    if let Some(max) = filter.max_price {
        query = query.filter(product::Column::Price.lte(max));
    }
    query
}
