//! Concrete adapters: Sea-ORM repositories, JWT, bcrypt, SMTP, seeders.
//!
//! The only crate allowed to name `sea_orm`, `jsonwebtoken`, `bcrypt` or
//! `lettre` types.

pub mod bcrypt_password_hasher;
pub mod catalogue_repository;
pub mod db_types;
pub mod demo_data_repository;
pub mod email_service;
pub mod entities;
pub mod file_storage;
pub mod image_processor;
pub mod jwt_token_service;
pub mod rate_limiter;
pub mod repositories;
pub mod search;
pub mod sql;
pub mod zalo_gateway;
