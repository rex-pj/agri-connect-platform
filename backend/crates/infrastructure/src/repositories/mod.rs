pub mod audit_log_repository;
pub mod business_repository;
pub mod call_repository;
pub mod category_repository;
pub mod demand_repository;
pub mod file_repository;
pub mod health_repository;
pub mod inquiry_repository;
pub mod location_repository;
pub mod partner_repository;
pub mod permission_repository;
pub mod product_repository;
pub mod refresh_token_repository;
pub mod review_repository;
pub mod role_repository;
pub mod service_repository;
pub mod user_repository;

use domain::{Error, ErrorCode};
use sea_orm::{DbErr, SqlErr};

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
