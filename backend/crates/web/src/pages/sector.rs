//! POST /sector — the visitor's sector (one of the shown `sectors`), from
//! `molecules/sector_switch.html`. Like the language: a plain form that keeps
//! the choice in the `sector` cookie for a year and goes back to the page it
//! was sent from. "Every sector" forgets it. The service and product lists
//! and the price board then show that sector's categories and those of every
//! sector (`category_in_sector`); the home page, which has no switch, never
//! follows it.

use application::outputs::CodedDto;
use axum::{extract::State, http::HeaderMap, response::Response, Extension};
use domain::Error;
use serde::Deserialize;

use crate::{
    app_state::AppState,
    cookies::request_cookie,
    extract::ApiForm,
    pages::{
        language::{back_to, remember},
        see_other,
    },
    session::CookiePolicy,
};

pub const SECTOR_COOKIE: &str = "sector";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SectorForm {
    /// A sector code, or empty for every sector.
    pub sector: String,
    /// The page the switch was on (path and query).
    pub back: String,
}

/// The sector this browser chose, when it is one of the shown `sectors`.
pub fn chosen_sector(headers: &HeaderMap, sectors: &[CodedDto]) -> Option<String> {
    let value = request_cookie(headers, SECTOR_COOKIE)?;
    sectors
        .iter()
        .find(|s| s.code == value)
        .map(|s| s.code.clone())
}

pub async fn switch(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<SectorForm>,
) -> Result<Response, Error> {
    let chosen = state.usecases.sector.chosen(&form.sector).await?;
    let mut response = see_other(&back_to(&form.back)?)?;
    remember(&mut response, &policy, SECTOR_COOKIE, chosen.as_deref());
    Ok(response)
}
