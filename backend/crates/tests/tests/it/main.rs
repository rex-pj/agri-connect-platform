//! Every integration test, as one test binary. Each file directly under
//! `tests/` would be its own binary linking the whole workspace: dozens of
//! links after every change, and dozens of processes each building the
//! templates and the template database again. Add a new test file here as a
//! module, never directly under `tests/`.

mod account_page;
mod admin_catalogue;
mod admin_layout;
mod admin_moderation;
mod admin_pages;
mod admin_providers;
mod admin_toolbar;
mod admin_users;
mod auth_pages;
mod authz;
mod call_clicks;
mod coded_lists;
mod conventions;
mod dashboard_pages;
mod database;
mod demand;
mod demo_data;
mod explore_page;
mod features;
mod home;
mod inbox;
mod jobs;
mod language;
mod lockout;
mod media;
mod navigation;
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
mod scale_probe;
mod search;
mod search_page;
mod sectors;
mod services_page;
mod sessions;
mod settings;
mod supplies;
mod zalo;
