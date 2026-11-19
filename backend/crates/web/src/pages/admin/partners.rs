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
    outputs::{
        AreaGroupDto, CategoryDto, CodedDto, NamedDto, PartnerDetailDto, PartnerDto, PartnerKpiDto,
        SponsorCandidateDto,
    },
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::NaiveDate;
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE, PARTNER_KINDS},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::sector_name,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{
            chosen_tab, history_rows, is_refusal, record_tabs, render_admin, tab_href, to_tab, url,
            user_href, AdminTab, Badge, Confirm, HistoryRow, Outcome, ReasonForm, RecordAction,
            RecordHeader, Says, TabQuery, TabSpec, Toggle,
        },
        forms::{reason, FormErrors},
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
    /// `p82` (a whole province) or `w28439` (one ward).
    pub area: String,
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
            area: c.area.clone(),
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
            area: get("area"),
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
            area: Some(self.area.clone()),
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

/// The contract form's category boxes, by sector (in the order of
/// `sectors`, then those of every sector), each sector headed once.
fn category_options(
    locale: &Locale,
    sectors: &[CodedDto],
    mut categories: Vec<CategoryDto>,
) -> Vec<CategoryOption> {
    let rank = |c: &CategoryDto| {
        c.sector
            .as_deref()
            .and_then(|s| sectors.iter().position(|x| x.code == s))
            .unwrap_or(sectors.len())
    };
    // Stable: within a sector the categories keep their display order.
    categories.sort_by_key(rank);
    let mut last: Option<Option<String>> = None;
    categories
        .into_iter()
        .map(|c| {
            let group = (last.as_ref() != Some(&c.sector)).then(|| match c.sector.as_deref() {
                Some(s) => sector_name(sectors, s),
                None => locale.text("sector.all"),
            });
            last = Some(c.sector.clone());
            CategoryOption {
                field: format!("{CATEGORY_PREFIX}{}", c.id),
                id: c.id,
                name: c.name,
                group,
            }
        })
        .collect()
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
pub struct CategoryOption {
    pub id: i32,
    pub name: String,
    pub field: String,
    /// The sector heading to show before this box: set on the first of each
    /// sector, so a partner's categories read by sector.
    pub group: Option<String>,
}

/// The parts of a partner's page (`?tab=`): what decides about it first, then
/// its contracts (with the add form), its profile form, its history.
const PARTNER_TABS: [TabSpec; 4] = [
    TabSpec {
        code: TAB_OVERVIEW,
        label: "admin.tab_overview",
    },
    TabSpec {
        code: TAB_CONTRACTS,
        label: "partner.contracts",
    },
    TabSpec {
        code: TAB_PROFILE,
        label: "admin.tab_profile",
    },
    TabSpec {
        code: TAB_HISTORY,
        label: "admin.tab_history",
    },
];
const TAB_OVERVIEW: &str = "overview";
const TAB_CONTRACTS: &str = "contracts";
const TAB_PROFILE: &str = "profile";
const TAB_HISTORY: &str = "history";

const ACCOUNT_SHEET: &str = "gan-tai-khoan";
const TURN_OFF_SHEET: &str = "tat-doi-tac";

/// The part a change lands on: a contract's on the contracts (a new
/// partner's too: its first contract is next), the profile's on the profile.
fn tab_after(flash: Flash) -> &'static str {
    match flash {
        Flash::PartnerCreated
        | Flash::ContractCreated
        | Flash::ContractSaved
        | Flash::ContractDeleted
        | Flash::SponsoredAdded
        | Flash::SponsoredRemoved => TAB_CONTRACTS,
        Flash::PartnerSaved => TAB_PROFILE,
        _ => TAB_OVERVIEW,
    }
}

/// The partner's header: turning it back on and adding a contract are
/// buttons; the report account and switching it off (its contracts stop)
/// wait in the menu.
fn partner_controls(
    state: &AppState,
    p: &PartnerDto,
    contracts_href: &str,
) -> Result<(RecordHeader, Vec<Confirm>), Error> {
    let locale = state.locale();
    let id = p.id.to_string();
    let (toggle, off) = Toggle {
        is_on: p.is_active,
        action: url("admin_partner_status", &[("id", &id)])?,
        field: "is_active",
        sheet: TURN_OFF_SHEET,
        off_label: locale.text("partner.turn_off"),
        off_icon: "fa-ban",
        on_label: locale.text("partner.turn_on"),
        on_icon: "fa-handshake",
        title: locale.text_with("partner.turn_off_title", &[("name", &p.name)]),
        button: locale.text("admin.confirm_off"),
        says: Says::Note(locale.text("partner.turn_off_note")),
    }
    .split();
    let mut actions = Vec::new();
    let account_label = if p.account_username.is_some() {
        "admin.change_account"
    } else {
        "admin.assign"
    };
    let mut menu = vec![RecordAction::sheet(
        locale.text(account_label),
        "fa-user-plus",
        ACCOUNT_SHEET,
        "success",
    )];
    if off.is_some() {
        menu.push(toggle);
    } else {
        actions.push(toggle);
    }
    actions.push(
        RecordAction::link(
            locale.text("partner.add_contract"),
            "fa-file-signature",
            format!("{contracts_href}#them-hop-dong"),
        )
        .tinted("success"),
    );
    let mut badges = vec![
        Badge::new("account", p.is_active),
        Badge::new("managed", p.account_username.is_some()),
    ];
    if p.is_demo {
        badges.push(Badge::new("demo", true));
    }
    let header = RecordHeader {
        title: p.name.clone(),
        icon: "fa-handshake".to_owned(),
        sub: Some(kind_label(locale, &p.kind)),
        back_href: url("admin_partners", &[])?,
        back_label: locale.text("admin_menu.partners"),
        badges,
        actions,
        menu,
    };
    Ok((header, off.into_iter().collect()))
}

#[derive(Serialize)]
pub struct PartnerPage {
    pub partner: PartnerDto,
    /// Its name, states and actions, and the confirmation.
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    /// The parts of the page, and the one shown.
    pub tabs: Vec<AdminTab>,
    pub tab: &'static str,
    /// Its latest changes: who switched it off or edited it, and why.
    pub history: Vec<HistoryRow>,
    /// The account form's title: "assign", or "change" once one is linked.
    pub account_label: String,
    /// The report account's admin page, when the visitor may read accounts.
    pub account_href: Option<String>,
    pub kind_label: String,
    /// The one-time message after a change, worded.
    pub flash_text: Option<String>,
    pub contracts: Vec<ContractRow>,
    pub kinds: Vec<KindOption>,
    /// Each province (the whole of it), with its wards.
    pub areas: Vec<AreaGroupDto>,
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
    pub account_action: String,
    pub contract_action: String,
}

async fn partner_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    tab: &'static str,
    outcome: Outcome<Typed>,
) -> Result<Response, Error> {
    let partners = &state.usecases.partner;
    let PartnerDetailDto { partner, contracts } = partners.get_partner(actor, id).await?;
    // What a lead is counted under: services and supplies, never a produce
    // group of the price board.
    let explore = &state.usecases.explore;
    let (areas, mut categories, supplies, sectors) = tokio::try_join!(
        partners.area_choices(actor),
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_categories(Some(CATEGORY_TYPE_PRODUCT)),
        state.usecases.sector.named(),
    )?;
    categories.extend(supplies);
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
    // A refused form is shown on its part of the page.
    let tab = if profile.is_some() {
        TAB_PROFILE
    } else if contract.is_some() || any_edit {
        TAB_CONTRACTS
    } else {
        tab
    };
    let contracts_href = tab_href(&path, &PARTNER_TABS, TAB_CONTRACTS);
    let (header, confirms) = partner_controls(state, &partner, &contracts_href)?;
    let count = rows.len();
    let tabs = record_tabs(state, &path, &PARTNER_TABS, tab, |code| {
        (code == TAB_CONTRACTS).then_some(count)
    });
    let account_label = state.locale().text(if partner.account_username.is_some() {
        "admin.change_account"
    } else {
        "admin.assign"
    });
    let body = PartnerPage {
        header,
        confirms,
        tabs,
        tab,
        history: history_rows(state, actor, HistoryOf::Partner(id)).await?,
        account_label,
        account_href: user_href(actor, partner.account_id)?,
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
        areas,
        categories: category_options(state.locale(), &sectors, categories),
        profile_action: tab_href(&path, &PARTNER_TABS, TAB_PROFILE),
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

/// 303 to the partner's page, on the part the change belongs to.
fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let page = url("admin_partner", &[("id", &id.to_string())])?;
    to_tab(&page, &PARTNER_TABS, tab_after(flash), flash)
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
            partner_page(state, actor, id, TAB_OVERVIEW, outcome).await
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
    ApiQuery(q): ApiQuery<TabQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PartnerNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let tab = chosen_tab(&PARTNER_TABS, &q.tab);
    let mut response = partner_page(&state, &actor, id, tab, outcome).await?;
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
            ("area", "w28439"),
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
        assert_eq!(input.area.as_deref(), Some("w28439"));
        assert!(input.starts_on.is_some() && input.ends_on.is_some());
    }
}
