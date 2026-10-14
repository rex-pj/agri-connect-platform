//! HTTP adapter: routing, handlers, middleware, request types and error
//! rendering. Handlers only marshal HTTP; business rules live in use cases.

pub mod app_state;
pub mod constants;
pub mod error_response;
pub mod extract;
pub mod handlers;
pub mod locale;
pub mod middleware;
pub mod requests;
pub mod routings;
pub mod startup;
