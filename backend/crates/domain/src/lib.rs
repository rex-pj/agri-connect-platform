//! Domain models, repository traits and the shared error type.
//!
//! No I/O lives here. The only framework integration is the optional `axum`
//! feature on the error type (see `error.rs` for why it must live here).

pub mod constants;
pub mod error;
pub mod models;
pub mod repositories;

pub use error::{Error, ErrorCode, ErrorKind, FieldError, Result, Validation};
