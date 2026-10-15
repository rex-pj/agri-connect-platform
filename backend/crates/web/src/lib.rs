//! HTTP adapter: routing, handlers, middleware, request types and error
//! rendering. Handlers only marshal HTTP; business rules live in use cases.

pub mod app_state;
pub mod constants;
pub mod cookies;
pub mod error_response;
pub mod extract;
pub mod handlers;
pub mod legacy;
pub mod locale;
pub mod middleware;
pub mod pages;
pub mod requests;
pub mod routes;
pub mod routings;
pub mod session;
pub mod shell;
pub mod slug;
pub mod startup;
pub mod views;
