//! /admin/partners and /admin/partners/{id}: the partners (brands, dealers,
//! cooperatives, exporters), what they bring in, and one partner's details,
//! account, contracts and sponsored providers.
//!
//! The create form is a sheet on the list. On a partner's page, turning it
//! off and deleting a contract ask for confirmation and a reason; turning it
//! on is one button. A contract's categories are checkboxes (`cat.<id>`), so
//! the form works without JS; its sponsored providers are chosen from those
//! counting under its area.

use std::collections::HashMap;

use application::{
    authz::Actor,
    inputs::{ContractInput, PartnerInput},
    outputs::{NamedDto, PartnerDetailDto, PartnerDto, PartnerKpiDto, SponsorCandidateDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::NaiveDate;
use domain::{constants::PARTNER_KINDS, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, Outcome},
        forms::{reason, FormErrors},
        listing::parse_opt,
        parse_id, see_other,
    },
};

/// The checkbox name prefix of a contract's categories.
const CATEGORY_PREFIX: &str = "cat.";

/// The partner form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PartnerForm {
    pub name: String,
    pub kind: String,
    pub contact_name: String,
    pub phone: String,
    pub email: String,
}

impl PartnerForm {
    fn input(&self) -> PartnerInput {
        PartnerInput {
            name: self.name.clone(),
            kind: self.kind.clone(),
            contact_name: Some(self.contact_name.clone()),
            phone: Some(self.phone.clone()),
            email: Some(self.email.clone()),
        }
    }

    fn from_dto(p: &PartnerDto) -> Self {
        Self {
            name: p.name.clone(),
            kind: p.kind.clone(),
            contact_name: p.contact_name.clone().unwrap_or_default(),
            phone: p.phone.clone().unwrap_or_default(),
            email: p.email.clone().unwrap_or_default(),
        }
    }
}

/// The contract form as typed: the plain boxes, and the ticked categories.
#[derive(Debug, Default, Serialize)]
pub struct ContractForm {
    pub location_id: String,
    pub starts_on: String,
    pub ends_on: String,
    pub monthly_fee: String,
    pub note: String,
    pub category_ids: Vec<i32>,
}

impl ContractForm {
    /// A contract as stored, for its edit form.
    fn from_dto(c: &application::outputs::ContractDto) -> Self {
        Self {
            location_id: c.location_id.to_string(),
            starts_on: c.starts_on.to_string(),
            ends_on: c.ends_on.to_string(),
            monthly_fee: c.monthly_fee.to_string(),
            note: c.note.clone().unwrap_or_default(),
            category_ids: c.categories.iter().map(|cat| cat.id).collect(),
        }
    }

    fn from_map(form: &HashMap<String, String>) -> Self {
        let get = |k: &str| form.get(k).cloned().unwrap_or_default();
        let mut category_ids: Vec<i32> = form
            .keys()
            .filter_map(|k| k.strip_prefix(CATEGORY_PREFIX))
            .filter_map(|id| id.parse().ok())
            .collect();
        category_ids.sort_unstable();
        Self {
            location_id: get("location_id"),
            starts_on: get("starts_on"),
            ends_on: get("ends_on"),
            monthly_fee: get("monthly_fee"),
            note: get("note"),
            category_ids,
        }
    }

    fn input(&self) -> ContractInput {
        let date = |s: &str| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok();
        // "2.000.000 đ" or "2,000,000": the digits are the amount.
        let digits: String = self
            .monthly_fee
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        ContractInput {
            location_id: parse_opt(&self.location_id),
            starts_on: date(&self.starts_on),
            ends_on: date(&self.ends_on),
            monthly_fee: if digits.is_empty() {
                None
            } else {
                digits.parse().ok()
            },
            note: Some(self.note.clone()),
            category_ids: self.category_ids.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct KindOption {
    pub label: String,
    pub value: &'static str,
}

/// A kind's name in the visitor's language.
fn kind_label(locale: &Locale, kind: &str) -> String {
    locale.text(&format!("partner_kind.{kind}"))
}

fn kinds(locale: &Locale) -> Vec<KindOption> {
    PARTNER_KINDS
        .iter()
        .map(|k| KindOption {
            label: kind_label(locale, k),
            value: k,
        })
        .collect()
}

#[derive(Serialize)]
pub struct PartnerRow {
    pub partner: PartnerDto,
    pub href: String,
    pub kind_label: String,
}

#[derive(Serialize)]
pub struct PartnersPage {
    pub rows: Vec<PartnerRow>,
    pub kpis: PartnerKpiDto,
    pub kinds: Vec<KindOption>,
    pub create_action: String,
    pub form: PartnerForm,
    pub errors: FormErrors,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    typed: Option<(PartnerForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let partners = &state.usecases.partner;
    let (list, kpis) = tokio::try_join!(partners.list_partners(actor), partners.kpis(actor))?;
    let mut rows = Vec::with_capacity(list.len());
    for partner in list {
        rows.push(PartnerRow {
            href: url("admin_partner", &[("id", &partner.id.to_string())])?,
            kind_label: kind_label(state.locale(), &partner.kind),
            partner,
        });
    }
    let path = url("admin_partners", &[])?;
    let (form, errors) = typed.unwrap_or_default();
    let body = PartnersPage {
        rows,
        kpis,
        kinds: kinds(state.locale()),
        create_action: path.clone(),
        form,
        errors,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/partners.html",
        &body,
    )
}

/// GET /admin/partners
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    list_page(&state, &actor, None, StatusCode::OK).await
}

/// POST /admin/partners
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<PartnerForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state
        .usecases
        .partner
        .create_partner(&actor, form.input())
        .await
    {
        Ok(id) => redirect_to(id, Flash::PartnerCreated),
        Err(err) if is_refusal(&err) => {
            let typed = (form, FormErrors::from_error(&state, &err));
            list_page(&state, &actor, Some(typed), err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

/// The account form as typed. The password is never sent back.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AccountForm {
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
}

/// The form of the page that came back refused, with its values as typed.
pub enum Typed {
    Profile(PartnerForm),
    Account(AccountForm),
    Contract(ContractForm),
    /// The edit form of the contract with this id.
    ContractEdit(i32, ContractForm),
}

#[derive(Serialize)]
pub struct SponsoredRow {
    pub provider: NamedDto,
    pub remove_action: String,
}

/// One contract on the page: its edit form (open again after a refusal),
/// its actions and the providers it may sponsor.
#[derive(Serialize)]
pub struct ContractRow {
    pub contract: application::outputs::ContractDto,
    pub edit_action: String,
    pub form: ContractForm,
    pub errors: FormErrors,
    pub edit_open: bool,
    pub delete_action: String,
    pub sponsor_action: String,
    pub candidates: Vec<SponsorCandidateDto>,
    /// Its sponsored providers, each with its "remove" action.
    pub sponsored: Vec<SponsoredRow>,
}

#[derive(Serialize)]
pub struct AreaOption {
    pub id: i32,
    pub label: String,
}

#[derive(Serialize)]
pub struct CategoryOption {
    pub id: i32,
    pub name: String,
    pub field: String,
}

#[derive(Serialize)]
pub struct PartnerPage {
    pub partner: PartnerDto,
    pub kind_label: String,
    /// The one-time message after a change, worded.
    pub flash_text: Option<String>,
    pub contracts: Vec<ContractRow>,
    pub kinds: Vec<KindOption>,
    pub areas: Vec<AreaOption>,
    pub categories: Vec<CategoryOption>,
    pub profile: PartnerForm,
    pub profile_errors: FormErrors,
    pub account: AccountForm,
    pub account_errors: FormErrors,
    pub contract: ContractForm,
    pub contract_errors: FormErrors,
    /// A refused confirmation, shown above everything.
    pub errors: FormErrors,
    pub profile_action: String,
    pub status_action: String,
    pub account_action: String,
    pub contract_action: String,
}

async fn partner_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<Typed>,
) -> Result<Response, Error> {
    let partners = &state.usecases.partner;
    let PartnerDetailDto { partner, contracts } = partners.get_partner(actor, id).await?;
    let (locations, categories) = tokio::try_join!(
        state.usecases.explore.get_locations(),
        state.usecases.explore.get_categories(None),
    )?;
    let id_text = id.to_string();
    // The messages go with the form that was refused.
    let refused = |typed: bool| {
        if typed {
            FormErrors {
                fields: outcome.errors.fields.clone(),
                message: outcome.errors.message.clone(),
            }
        } else {
            FormErrors::default()
        }
    };
    let (mut profile, mut account, mut contract, mut edited) = (None, None, None, None);
    match outcome.form {
        Some(Typed::Profile(f)) => profile = Some(f),
        Some(Typed::Account(f)) => account = Some(f),
        Some(Typed::Contract(f)) => contract = Some(f),
        Some(Typed::ContractEdit(cid, f)) => edited = Some((cid, f)),
        None => {}
    }
    let mut rows = Vec::with_capacity(contracts.len());
    for contract in contracts {
        let (form, edit_open) = match edited.take_if(|(cid, _)| *cid == contract.id) {
            Some((_, f)) => (f, true),
            None => (ContractForm::from_dto(&contract), false),
        };
        let cid = contract.id.to_string();
        let mut sponsored = Vec::with_capacity(contract.sponsored.len());
        for s in &contract.sponsored {
            let action = url(
                "admin_partner_sponsored_remove",
                &[("id", &id_text), ("cid", &cid), ("bid", &s.id.to_string())],
            )?;
            sponsored.push(SponsoredRow {
                provider: s.clone(),
                remove_action: action,
            });
        }
        rows.push(ContractRow {
            edit_action: url("admin_partner_contract", &[("id", &id_text), ("cid", &cid)])?,
            errors: refused(edit_open),
            edit_open,
            form,
            candidates: partners.sponsor_candidates(actor, id, contract.id).await?,
            delete_action: url(
                "admin_partner_contract_delete",
                &[("id", &id_text), ("cid", &cid)],
            )?,
            sponsor_action: url(
                "admin_partner_sponsored",
                &[("id", &id_text), ("cid", &cid)],
            )?,
            sponsored,
            contract,
        });
    }
    let any_edit = rows.iter().any(|r| r.edit_open);
    let refused_form = profile.is_some() || account.is_some() || contract.is_some() || any_edit;
    let path = url("admin_partner", &[("id", &id_text)])?;
    let body = PartnerPage {
        profile_errors: refused(profile.is_some()),
        account_errors: refused(account.is_some()),
        contract_errors: refused(contract.is_some()),
        errors: refused(!refused_form),
        profile: profile.unwrap_or_else(|| PartnerForm::from_dto(&partner)),
        account: account.unwrap_or_else(|| AccountForm {
            username: partner.account_username.clone().unwrap_or_default(),
            ..AccountForm::default()
        }),
        contract: contract.unwrap_or_default(),
        kind_label: kind_label(state.locale(), &partner.kind),
        flash_text: outcome.flash.map(|f| {
            state
                .locale()
                .text(&format!("partner_flash.{}", f.as_str()))
        }),
        partner,
        contracts: rows,
        kinds: kinds(state.locale()),
        areas: locations
            .into_iter()
            .map(|l| AreaOption {
                id: l.id,
                label: l.label,
            })
            .collect(),
        categories: categories
            .into_iter()
            .map(|c| CategoryOption {
                field: format!("{CATEGORY_PREFIX}{}", c.id),
                id: c.id,
                name: c.name,
            })
            .collect(),
        profile_action: path.clone(),
        status_action: url("admin_partner_status", &[("id", &id_text)])?,
        account_action: url("admin_partner_account", &[("id", &id_text)])?,
        contract_action: url("admin_partner_contracts", &[("id", &id_text)])?,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/partner.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_partner", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// The page again with the refused form's values and messages, or the
/// failure itself when it is not a refusal.
async fn after(
    state: &AppState,
    actor: &Actor,
    id: i32,
    result: Result<(), Error>,
    flash: Flash,
    typed: Option<Typed>,
) -> Result<Response, Error> {
    match result {
        Ok(()) => redirect_to(id, flash),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(state, &err, typed, false);
            partner_page(state, actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}

/// GET /admin/partners/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = partner_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/partners/{id}: the profile form.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<PartnerForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let result = state
        .usecases
        .partner
        .update_partner(&actor, id, form.input())
        .await;
    after(
        &state,
        &actor,
        id,
        result,
        Flash::PartnerSaved,
        Some(Typed::Profile(form)),
    )
    .await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to turn the partner on, "false" to turn it off.
    pub is_active: String,
    pub reason: String,
}

/// POST /admin/partners/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let active = form.is_active == "true";
    let result = state
        .usecases
        .partner
        .set_partner_active(&actor, id, active, reason(&form.reason))
        .await;
    let flash = if active {
        Flash::PartnerActivated
    } else {
        Flash::PartnerDeactivated
    };
    after(&state, &actor, id, result, flash, None).await
}

/// POST /admin/partners/{id}/account
pub async fn assign_account(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<AccountForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let result = state
        .usecases
        .partner
        .assign_partner_account(
            &actor,
            id,
            form.username.clone(),
            form.email.clone(),
            form.password.clone(),
        )
        .await
        .map(|_| ());
    after(
        &state,
        &actor,
        id,
        result,
        Flash::PartnerAccountAssigned,
        Some(Typed::Account(form)),
    )
    .await
}

/// POST /admin/partners/{id}/contracts
pub async fn create_contract(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(raw): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let form = ContractForm::from_map(&raw);
    let result = state
        .usecases
        .partner
        .create_contract(&actor, id, form.input())
        .await
        .map(|_| ());
    after(
        &state,
        &actor,
        id,
        result,
        Flash::ContractCreated,
        Some(Typed::Contract(form)),
    )
    .await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReasonForm {
    pub reason: String,
}

/// POST /admin/partners/{id}/contracts/{cid}: the contract's edit form.
pub async fn update_contract(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiForm(raw): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let cid = parse_id(&raw_cid, ErrorCode::ContractNotFound)?;
    let form = ContractForm::from_map(&raw);
    let result = state
        .usecases
        .partner
        .update_contract(&actor, id, cid, form.input())
        .await;
    after(
        &state,
        &actor,
        id,
        result,
        Flash::ContractSaved,
        Some(Typed::ContractEdit(cid, form)),
    )
    .await
}

/// POST /admin/partners/{id}/contracts/{cid}/delete
pub async fn delete_contract(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let cid = parse_id(&raw_cid, ErrorCode::ContractNotFound)?;
    let result = state
        .usecases
        .partner
        .delete_contract(&actor, id, cid, reason(&form.reason))
        .await;
    after(&state, &actor, id, result, Flash::ContractDeleted, None).await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SponsorForm {
    pub business_id: String,
}

/// POST /admin/partners/{id}/contracts/{cid}/sponsored
pub async fn add_sponsored(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiForm(form): ApiForm<SponsorForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let cid = parse_id(&raw_cid, ErrorCode::ContractNotFound)?;
    let business = parse_id(&form.business_id, ErrorCode::ProviderNotFound)?;
    let result = state
        .usecases
        .partner
        .add_sponsored(&actor, id, cid, business)
        .await;
    after(&state, &actor, id, result, Flash::SponsoredAdded, None).await
}

/// POST /admin/partners/{id}/contracts/{cid}/sponsored/{bid}/delete
pub async fn remove_sponsored(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid, raw_bid)): ApiPath<(String, String, String)>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let cid = parse_id(&raw_cid, ErrorCode::ContractNotFound)?;
    let bid = parse_id(&raw_bid, ErrorCode::ProviderNotFound)?;
    let result = state
        .usecases
        .partner
        .remove_sponsored(&actor, id, cid, bid, reason(&form.reason))
        .await;
    after(&state, &actor, id, result, Flash::SponsoredRemoved, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_contract_form_reads_its_ticked_categories_and_the_fee_digits() {
        let form: HashMap<String, String> = [
            ("location_id", "3"),
            ("starts_on", "2026-11-01"),
            ("ends_on", "2027-10-31"),
            ("monthly_fee", "2.000.000 d"),
            ("cat.7", "on"),
            ("cat.2", "on"),
            ("cat.x", "on"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let parsed = ContractForm::from_map(&form);
        assert_eq!(parsed.category_ids, vec![2, 7]);
        let input = parsed.input();
        assert_eq!(input.monthly_fee, Some(2_000_000));
        assert_eq!(input.location_id, Some(3));
        assert!(input.starts_on.is_some() && input.ends_on.is_some());
    }
}
