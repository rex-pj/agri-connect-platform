//! Use cases, port traits, input/output types and validation rules.
//!
//! Depends only on `domain`. No HTTP, ORM, JWT or bcrypt types may appear
//! here; infrastructure concerns arrive through the traits in `ports`.

pub mod catalogue;
pub mod constants;
pub mod inputs;
pub mod outputs;
pub mod pagination;
pub mod ports;
pub mod usecases;
pub mod validation;
