//! Every integration test, as one test binary. Each file directly under
//! `tests/` would be its own binary linking the whole workspace: dozens of
//! links after every change, and dozens of processes each building the
//! templates and the template database again. Add a new test file here as a
//! module, never directly under `tests/`.

mod account_page;
mod admin_catalogue;
mod admin_moderation;
mod admin_pages;
mod admin_providers;
mod admin_users;
mod auth_pages;
mod authz;
mod call_clicks;
mod conventions;
mod dashboard_pages;
mod database;
mod demand;
mod explore_page;
mod home;
mod inbox;
mod jobs;
mod language;
mod lockout;
mod media;
mod ops;
mod own_services;
mod pages;
mod partners;
mod prices;
mod product_page;
mod products_page;
mod provider_page;
mod races;
mod reviews;
mod router_errors;
mod search;
mod search_page;
mod services_page;
mod sessions;
mod zalo;
