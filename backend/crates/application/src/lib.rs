//! Use cases, port traits, input/output types and validation rules.
//!
//! Depends only on `domain`. No HTTP, ORM, JWT or bcrypt types may appear
//! here; infrastructure concerns arrive through the traits in `ports`.

pub mod admin_units;
pub mod authz;
pub mod catalogue;
pub mod constants;
pub mod events;
pub mod features;
pub mod hashing;
pub mod icons;
pub mod inputs;
pub mod outputs;
pub mod pagination;
pub mod ports;
pub mod repeats;
pub mod settings;
pub mod slug;
pub mod usecases;
pub mod validation;
