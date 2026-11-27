pub mod admin_unit_repository;
pub mod alert_repository;
pub mod audit_log_repository;
pub mod business_repository;
pub mod call_repository;
pub mod category_repository;
pub mod coded_repository;
pub mod commodity_repository;
pub mod demand_repository;
pub mod file_repository;
pub mod health_repository;
pub mod info_page_repository;
pub mod inquiry_repository;
pub mod job_repository;
pub mod location_repository;
pub mod partner_repository;
pub mod permission_repository;
pub mod post_repository;
pub mod price_repository;
pub mod product_repository;
pub mod refresh_token_repository;
pub mod review_repository;
pub mod role_repository;
pub mod service_repository;
pub mod setting_repository;
pub mod user_repository;

use domain::{Error, ErrorCode};
use sea_orm::{DatabaseTransaction, DbErr, EntityTrait, QuerySelect, SqlErr};

use crate::entities::business;

/// Locks a business's row until `txn` ends, so two requests adding to what
/// it lists (a service, a supply) count its rows in turn and a cap on them
/// holds.
pub(crate) async fn lock_business(
    txn: &DatabaseTransaction,
    business_id: i32,
) -> Result<(), DbErr> {
    business::Entity::find_by_id(business_id)
        .select_only()
        .column(business::Column::Id)
        .lock_exclusive()
        .into_tuple::<i32>()
        .one(txn)
        .await
        .map(|_| ())
}

/// Maps a write that hit a UNIQUE constraint to a field error, so two admins
/// racing for the same slug get the form message rather than a 500; any other
/// failure is logged and reported as internal.
pub(crate) fn unique_or_internal(
    e: DbErr,
    field: &'static str,
    code: ErrorCode,
    context: &str,
) -> Error {
    if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        return Error::field(field, code);
    }
    tracing::error!("{context} error: {e}");
    Error::internal("Failed to save.")
}
