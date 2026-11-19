//! A request that runs longer than `REQUEST_TIMEOUT_SECS` is given up on:
//! a query that never comes back must not hold a database connection and a
//! task for as long as the client cares to wait. Not on the upload routes,
//! where a photo over a weak signal may fairly take longer.

use std::time::Duration;

use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};

use crate::constants::REQUEST_TIMEOUT_SECS;

pub async fn enforce(req: Request, next: Next) -> Response {
    let limit = Duration::from_secs(REQUEST_TIMEOUT_SECS);
    match tokio::time::timeout(limit, next.run(req)).await {
        Ok(response) => response,
        Err(_) => Error::new(ErrorCode::RequestTimedOut).into_response(),
    }
}
