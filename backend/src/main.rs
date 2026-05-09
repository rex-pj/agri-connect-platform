mod app_state;
mod application;
mod constants;
mod domain;
mod entities;
mod handlers;
mod helpers;
mod infrastructure;
mod middleware;
mod routings;
mod startup;
mod validators;
mod view_models;

#[tokio::main]
async fn main() {
    startup::start().await;
}
