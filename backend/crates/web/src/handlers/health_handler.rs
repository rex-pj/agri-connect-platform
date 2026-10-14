use axum::http::StatusCode;

pub struct HealthHandler;

impl HealthHandler {
    pub async fn health() -> StatusCode {
        StatusCode::OK
    }
}
