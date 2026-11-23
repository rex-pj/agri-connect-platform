use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::{Expr, NullOrdering, Order};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Set, TransactionTrait,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use domain::{
    models::{
        business::ListingEditor,
        product::{
            AdminProduct, AdminProductFilter, NewProduct, ProductFilter, ProductSort, ProductView,
            ProviderProduct, UpdateProduct, UpdateProductAdmin,
        },
    },
    repositories::product_repository_trait::ProductRepositoryTrait,
};

use domain::Error;

use crate::admin_list::{admin_order, shown_state, OrderColumns};
use crate::repositories::lock_business;
use crate::search::{admin_product_search, text_contains, PRODUCT_SEARCH_COLUMNS};
use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business, commodity,
        product::{self, Entity as ProductEntity},
        supply_type::{self as category, Entity as Category},
    },
    sql::{
        business_location, businesses_of, distance, geography_from_text, output, supply_in_sector,
        supply_sector, within, QueryAs,
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
    category_id: i32,
    category_slug: String,
    category_icon: Option<String>,
    commodity_id: Option<i32>,
    commodity: Option<String>,
    vendor_name: String,
    vendor_phone: String,
    vendor_verified: bool,
    business_id: i32,
    created_on: DateTimeWithTimeZone,
    image_file_id: Option<Uuid>,
}

#[derive(FromQueryResult)]
struct GeoProductRow {
    #[sea_orm(nested)]
    product: ProductViewRow,
    distance_m: f64,
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
    commodity: Option<String>,
    commodity_id: Option<i32>,
    sector: Option<String>,
    business_id: i32,
    business_name: String,
    is_active: bool,
    business_active: bool,
    category_active: bool,
    business_phone: String,
    business_verified: bool,
    created_by: Option<i32>,
    owner_id: Option<i32>,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    image_file_id: Option<Uuid>,
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
    commodity_id: Option<i32>,
    commodity: Option<String>,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    image_file_id: Option<Uuid>,
}

fn base_product_view_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column_as(category::Column::Name, QueryAs::Category)
        .column_as(product::Column::SupplyTypeId, QueryAs::CategoryId)
        .column_as(category::Column::Slug, QueryAs::CategorySlug)
        .column_as(category::Column::Icon, QueryAs::CategoryIcon)
        .column(product::Column::CommodityId)
        .column_as(commodity::Column::Name, QueryAs::Commodity)
        .column_as(business::Column::Name, QueryAs::VendorName)
        .column_as(business::Column::Phone, QueryAs::VendorPhone)
        .column_as(business::Column::IsVerified, QueryAs::VendorVerified)
        .column(product::Column::BusinessId)
        .column(product::Column::CreatedOn)
        .column(product::Column::ImageFileId)
        .join(JoinType::InnerJoin, product::Relation::Business.def())
        .join(JoinType::InnerJoin, product::Relation::SupplyType.def())
        .join(JoinType::LeftJoin, product::Relation::Commodity.def())
        .filter(business::Column::IsActive.eq(true))
        .filter(business::Column::DeletedAt.is_null())
        .filter(category::Column::IsActive.eq(true))
}

/// The live products whose seller lies within `radius` metres of
/// `point_wkt`: what the geo search pages through and what its count counts.
fn near_live(point_wkt: &str, radius: f64) -> sea_orm::Select<ProductEntity> {
    base_product_view_query()
        .filter(product::Column::IsActive.eq(true))
        .filter(product::Column::DeletedAt.is_null())
        .filter(within(
            business_location(),
            geography_from_text(point_wkt),
            radius,
        ))
}

fn base_provider_product_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column_as(category::Column::Name, QueryAs::Category)
        .column_as(product::Column::SupplyTypeId, QueryAs::CategoryId)
        .column(product::Column::CommodityId)
        .column_as(commodity::Column::Name, QueryAs::Commodity)
        .column(product::Column::IsActive)
        .column(product::Column::CreatedOn)
        .column(product::Column::ImageFileId)
        .join(JoinType::InnerJoin, product::Relation::SupplyType.def())
        .join(JoinType::LeftJoin, product::Relation::Commodity.def())
}

fn base_admin_product_query() -> sea_orm::Select<ProductEntity> {
    ProductEntity::find()
        .select_only()
        .column(product::Column::Id)
        .column(product::Column::Name)
        .column(product::Column::Description)
        .column(product::Column::Price)
        .column(product::Column::Unit)
        .column_as(category::Column::Name, QueryAs::Category)
        .column_as(product::Column::SupplyTypeId, QueryAs::CategoryId)
        .column_as(commodity::Column::Name, QueryAs::Commodity)
        .column(product::Column::CommodityId)
        .column(product::Column::BusinessId)
        .column_as(business::Column::Name, QueryAs::BusinessName)
        .column(product::Column::IsActive)
        .column_as(business::Column::IsActive, QueryAs::BusinessActive)
        .column_as(category::Column::IsActive, QueryAs::CategoryActive)
        .column_as(business::Column::Phone, QueryAs::BusinessPhone)
        .column_as(business::Column::IsVerified, QueryAs::BusinessVerified)
        .column(product::Column::CreatedBy)
        .column(business::Column::OwnerId)
        .column(product::Column::CreatedOn)
        .column(product::Column::UpdatedOn)
        .column(product::Column::ImageFileId)
        .expr_as(supply_sector(), QueryAs::Sector)
        .join(JoinType::InnerJoin, product::Relation::Business.def())
        .join(JoinType::InnerJoin, product::Relation::SupplyType.def())
        .join(JoinType::LeftJoin, product::Relation::Commodity.def())
        .filter(product::Column::DeletedAt.is_null())
        .filter(business::Column::DeletedAt.is_null())
}

#[async_trait]
impl ProductRepositoryTrait for ProductRepository {
    async fn list_admin(
        &self,
        filter: &AdminProductFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminProduct>, Error> {
        admin_order(
            apply_admin_filter(base_admin_product_query(), filter),
            filter.sort,
            OrderColumns {
                created: product::Column::CreatedOn,
                name: product::Column::Name,
                id: product::Column::Id,
            },
            |q| {
                q.order_by_desc(product::Column::CreatedOn)
                    .order_by_desc(product::Column::Id)
            },
        )
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

    async fn count_admin(&self, filter: &AdminProductFilter) -> Result<i64, Error> {
        let query = ProductEntity::find()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(product::Column::DeletedAt.is_null())
            .filter(business::Column::DeletedAt.is_null());
        apply_admin_filter(query, filter)
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
        sort: ProductSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error> {
        let query = apply_filter(
            base_product_view_query()
                .filter(product::Column::IsActive.eq(true))
                .filter(product::Column::DeletedAt.is_null()),
            filter,
        );

        let query = match sort {
            // Unpriced ("Liên hệ giá") last both ways: Postgres puts NULL first
            // in a descending sort.
            ProductSort::PriceAsc => {
                query.order_by_with_nulls(product::Column::Price, Order::Asc, NullOrdering::Last)
            }
            ProductSort::PriceDesc => {
                query.order_by_with_nulls(product::Column::Price, Order::Desc, NullOrdering::Last)
            }
            ProductSort::Newest => query.order_by_desc(product::Column::CreatedOn),
        };

        query
            // Ties (same price, or demo rows loaded in one transaction with
            // one created_on) need a fixed order, or rows repeat across pages.
            .order_by_desc(product::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<ProductViewRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(|r| map_view_row(r, None)).collect())
            .map_err(|e| {
                error!("product browse error: {e}");
                Error::internal("Failed to load products.")
            })
    }

    async fn count_browse(&self, filter: &ProductFilter) -> Result<i64, Error> {
        let query = apply_filter(
            ProductEntity::find()
                .join(JoinType::InnerJoin, product::Relation::Business.def())
                .join(JoinType::InnerJoin, product::Relation::SupplyType.def())
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

    async fn featured(&self, limit: i64, sector: Option<&str>) -> Result<Vec<ProductView>, Error> {
        let mut query = base_product_view_query();
        if let Some(sector) = sector {
            query = query.filter(supply_in_sector(sector));
        }
        query
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            // Those with a photo first, as the featured services: a home
            // row of placeholders reads as an empty catalogue.
            .order_by(
                Expr::col((product::Entity, product::Column::ImageFileId)).is_not_null(),
                Order::Desc,
            )
            .order_by_desc(product::Column::CreatedOn)
            .order_by_desc(product::Column::Id)
            .limit(limit as u64)
            .into_model::<ProductViewRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(|r| map_view_row(r, None)).collect())
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
            .map(|opt| opt.map(|r| map_view_row(r, None)))
            .map_err(|e| {
                error!("product find_by_id error: {e}");
                Error::internal("Failed to load product details.")
            })
    }

    // PostGIS calls built from `sql`, as in the business repository's geo search.
    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error> {
        let distance = distance(business_location(), geography_from_text(point_wkt));
        let mut query = near_live(point_wkt, radius)
            .expr_as(distance, QueryAs::DistanceM)
            .limit(limit as u64)
            .offset(offset as u64);
        QueryTrait::query(&mut query).order_by_expr(output(QueryAs::DistanceM), Order::Asc);
        query
            .order_by_asc(product::Column::Id)
            .into_model::<GeoProductRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| map_view_row(r.product, Some(r.distance_m)))
                    .collect()
            })
            .map_err(|e| Error::internal(format!("product search_geo: {e}")))
    }

    async fn count_geo(&self, point_wkt: &str, radius: f64) -> Result<i64, Error> {
        near_live(point_wkt, radius)
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| Error::internal(format!("product count_geo: {e}")))
    }

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, Error> {
        base_provider_product_query()
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .filter(product::Column::DeletedAt.is_null())
            .order_by_desc(product::Column::CreatedOn)
            .order_by_desc(product::Column::Id)
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
            .filter(category::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("product check_category_is_product error: {e}");
                Error::internal("Failed to check category.")
            })
    }

    async fn check_commodity_is_shown(&self, commodity_id: i32) -> Result<bool, Error> {
        commodity::Entity::find_by_id(commodity_id)
            .filter(commodity::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("product check_commodity_is_shown error: {e}");
                Error::internal("Failed to check commodity.")
            })
    }

    async fn create_capped(&self, product: NewProduct, cap: u64) -> Result<Option<i32>, Error> {
        let fail = |e: sea_orm::DbErr| {
            error!("product create error: {e}");
            Error::internal("Failed to create product.")
        };
        let business_id = product.business_id;
        let txn = self.db.begin().await.map_err(fail)?;
        lock_business(&txn, business_id).await.map_err(fail)?;
        let existing = ProductEntity::find()
            .filter(product::Column::BusinessId.eq(business_id))
            .filter(product::Column::DeletedAt.is_null())
            .count(&txn)
            .await
            .map_err(fail)?;
        if existing >= cap {
            txn.rollback().await.map_err(fail)?;
            return Ok(None);
        }
        let new_product = product::ActiveModel {
            business_id: Set(product.business_id),
            supply_type_id: Set(product.category_id),
            commodity_id: Set(product.commodity_id),
            name: Set(product.name),
            description: Set(product.description),
            price: Set(product.price),
            unit: Set(product.unit),
            image_file_id: Set(product.image_file_id),
            created_by: Set(Some(product.created_by)),
            ..Default::default()
        };

        let id = ProductEntity::insert(new_product)
            .exec(&txn)
            .await
            .map_err(fail)?
            .last_insert_id;
        txn.commit().await.map_err(fail)?;
        Ok(Some(id))
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
                product::Column::SupplyTypeId,
                Expr::value(product.category_id),
            )
            .col_expr(
                product::Column::CommodityId,
                Expr::value(product.commodity_id),
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

    async fn update_admin(&self, product: UpdateProductAdmin) -> Result<bool, Error> {
        ProductEntity::update_many()
            .col_expr(
                product::Column::SupplyTypeId,
                Expr::value(product.category_id),
            )
            .col_expr(
                product::Column::CommodityId,
                Expr::value(product.commodity_id),
            )
            .col_expr(product::Column::Name, Expr::value(product.name))
            .col_expr(
                product::Column::Description,
                Expr::value(product.description),
            )
            .col_expr(product::Column::Price, Expr::value(product.price))
            .col_expr(product::Column::Unit, Expr::value(product.unit))
            .col_expr(
                product::Column::UpdatedBy,
                Expr::value(Some(product.updated_by)),
            )
            .col_expr(product::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(product.id))
            .filter(product::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("product update_admin error (id={}): {e}", product.id);
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

    async fn set_image(
        &self,
        id: i32,
        editor: ListingEditor,
        file_id: Option<Uuid>,
    ) -> Result<Option<Option<Uuid>>, Error> {
        let fail = |e: sea_orm::DbErr| Error::internal(format!("product set_image (id={id}): {e}"));
        let txn = self.db.begin().await.map_err(fail)?;
        // FOR UPDATE, with the ownership check: a concurrent upload waits
        // here, then reads this upload's image as its previous one, so the
        // image it replaces is never orphaned.
        let mut query = ProductEntity::find_by_id(id)
            .select_only()
            .column(product::Column::ImageFileId)
            .filter(product::Column::DeletedAt.is_null());
        if let ListingEditor::Owner(owner_user) = editor {
            query =
                query.filter(product::Column::BusinessId.in_subquery(businesses_of(owner_user)));
        }
        let previous = query
            .lock_exclusive()
            .into_tuple::<Option<Uuid>>()
            .one(&txn)
            .await
            .map_err(fail)?;
        let Some(previous) = previous else {
            txn.rollback().await.map_err(fail)?;
            return Ok(None);
        };
        ProductEntity::update_many()
            .col_expr(product::Column::ImageFileId, Expr::value(file_id))
            .col_expr(
                product::Column::UpdatedBy,
                Expr::value(Some(editor.user_id())),
            )
            .col_expr(product::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(product::Column::Id.eq(id))
            .exec(&txn)
            .await
            .map_err(fail)?;
        txn.commit().await.map_err(fail)?;
        Ok(Some(previous))
    }

    async fn image_of(&self, id: i32) -> Result<Option<Uuid>, Error> {
        ProductEntity::find_by_id(id)
            .select_only()
            .column(product::Column::ImageFileId)
            .into_tuple::<Option<Uuid>>()
            .one(self.db.as_ref())
            .await
            .map(Option::flatten)
            .map_err(|e| Error::internal(format!("product image_of (id={id}): {e}")))
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
        commodity: r.commodity,
        commodity_id: r.commodity_id,
        sector: r.sector,
        business_id: r.business_id,
        business_name: r.business_name,
        is_active: r.is_active,
        business_active: r.business_active,
        category_active: r.category_active,
        business_phone: r.business_phone,
        business_verified: r.business_verified,
        created_by: r.created_by,
        owner_id: r.owner_id,
        created_on: r.created_on.with_timezone(&Utc),
        updated_on: r.updated_on.with_timezone(&Utc),
        image_file_id: r.image_file_id,
    }
}

fn map_view_row(r: ProductViewRow, distance_m: Option<f64>) -> ProductView {
    ProductView {
        id: r.id,
        name: r.name,
        description: r.description,
        price: r.price,
        unit: r.unit,
        category: r.category,
        category_id: r.category_id,
        category_slug: r.category_slug,
        category_icon: r.category_icon,
        commodity_id: r.commodity_id,
        commodity: r.commodity,
        vendor_name: r.vendor_name,
        vendor_phone: r.vendor_phone,
        vendor_verified: r.vendor_verified,
        business_id: r.business_id,
        created_on: r.created_on.with_timezone(&Utc),
        image_file_id: r.image_file_id,
        distance_m,
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
        commodity_id: r.commodity_id,
        commodity: r.commodity,
        is_active: r.is_active,
        created_on: r.created_on.with_timezone(&Utc),
        image_file_id: r.image_file_id,
    }
}

/// The admin's list criteria, shared by `list_admin` and `count_admin`.
fn apply_admin_filter<Q: QueryFilter>(mut query: Q, filter: &AdminProductFilter) -> Q {
    query = query.filter(admin_product_search(filter.search.as_deref()));
    if let Some(id) = filter.provider_id {
        query = query.filter(product::Column::BusinessId.eq(id));
    }
    if let Some(id) = filter.category_id {
        query = query.filter(product::Column::SupplyTypeId.eq(id));
    }
    if let Some(id) = filter.commodity_id {
        query = query.filter(product::Column::CommodityId.eq(id));
    }
    if let Some(sector) = filter.sector.as_deref() {
        query = query.filter(supply_in_sector(sector));
    }
    match filter.gap.as_deref() {
        Some("no_image") => query = query.filter(product::Column::ImageFileId.is_null()),
        Some("no_price") => query = query.filter(product::Column::Price.is_null()),
        _ => {}
    }
    if let Some(min) = filter.price_min {
        query = query.filter(product::Column::Price.gte(min));
    }
    if let Some(max) = filter.price_max {
        query = query.filter(product::Column::Price.lte(max));
    }
    query = query.filter(crate::admin_list::created_within(
        (product::Entity, product::Column::CreatedOn),
        filter.created_from,
        filter.created_to,
    ));
    query.filter(shown_state(product::Column::IsActive, filter.state))
}

/// The public search criteria, shared by `browse` and `count_browse` so the
/// page count can never disagree with the page contents.
fn apply_filter<Q: QueryFilter>(mut query: Q, filter: &ProductFilter) -> Q {
    if let Some(id) = filter.category_id {
        query = query.filter(product::Column::SupplyTypeId.eq(id));
    }
    if let Some(id) = filter.commodity_id {
        query = query.filter(
            sea_orm::Condition::any()
                .add(product::Column::CommodityId.eq(id))
                .add(product::Column::CommodityId.is_null()),
        );
    }
    if let Some(sector) = filter.sector.as_deref() {
        query = query.filter(supply_in_sector(sector));
    }
    if let Some(id) = filter.business_id {
        query = query.filter(product::Column::BusinessId.eq(id));
    }
    if let Some(q) = filter.name.as_deref().filter(|q| !q.trim().is_empty()) {
        query = query.filter(text_contains(PRODUCT_SEARCH_COLUMNS, q));
    }
    if let Some(min) = filter.min_price {
        query = query.filter(product::Column::Price.gte(min));
    }
    if let Some(max) = filter.max_price {
        query = query.filter(product::Column::Price.lte(max));
    }
    query
}
