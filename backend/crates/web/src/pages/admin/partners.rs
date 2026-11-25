//! /admin/partners and /admin/partners/{id}: the partners (brands, dealers,
//! cooperatives, exporters), what they bring in, and one partner's details,
//! account and contracts (each a short line to its own page:
//! `contracts.rs`).
//!
//! The create form is a sheet on the list. On a partner's page, turning it
//! off asks for confirmation and a reason; turning it on is one button. A
//! contract's categories are checkboxes (`cat.<id>`), so the form works
//! without JS.

use std::collections::HashMap;

use application::{
    authz::Actor,
    inputs::{ContractInput, PartnerInput},
    outputs::{AreaGroupDto, CategoryDto, CodedDto, ContractDto, PartnerDto, PartnerKpiDto},
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::NaiveDate;
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE, PARTNER_KINDS},
    models::{
        admin_list::{AdminSort, CONTRACT_LIST_STATES, SWITCH_LIST_STATES},
        partner::ContractFilter,
    },
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
            user_href, AdminTab, Badge, Confirm, HistoryRow, ListPosition, Outcome, RecordAction,
            RecordHeader, Says, SearchBox, TabSpec, Toggle,
        },
        forms::{reason, FormErrors},
        listing::{past_the_end, with_query, Pagination},
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
    pub(super) fn from_dto(c: &ContractDto) -> Self {
        Self {
            area: c.area.clone(),
            starts_on: c.starts_on.to_string(),
            ends_on: c.ends_on.to_string(),
            monthly_fee: c.monthly_fee.to_string(),
            note: c.note.clone().unwrap_or_default(),
            category_ids: c.categories.iter().map(|cat| cat.id).collect(),
        }
    }

    pub(super) fn from_map(form: &HashMap<String, String>) -> Self {
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

    pub(super) fn input(&self) -> ContractInput {
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

/// The contract form's category boxes: what a lead is counted under,
/// services and supplies (never a produce group of the price board), by
/// sector.
pub(super) async fn category_choices(state: &AppState) -> Result<Vec<CategoryOption>, Error> {
    let explore = &state.usecases.explore;
    let (mut categories, supplies, sectors) = tokio::try_join!(
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_categories(Some(CATEGORY_TYPE_PRODUCT)),
        state.usecases.sector.named(),
    )?;
    categories.extend(supplies);
    Ok(category_options(state.locale(), &sectors, categories))
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
    /// Each revenue area's demand report, by its value (`p82`, `w28439`).
    pub demand_hrefs: HashMap<String, String>,
    pub kinds: Vec<KindOption>,
    pub create_action: String,
    pub form: PartnerForm,
    pub errors: FormErrors,
    pub pagination: Option<Pagination>,
    pub search: SearchBox,
    /// The on / off chips.
    pub states: Vec<AdminTab>,
    /// How many partners the list shows (narrowed).
    pub total: i64,
}

/// The orders the partner list offers besides its own (on first, by name).
const PARTNER_SORTS: &[AdminSort] = &[AdminSort::Newest, AdminSort::Name];

/// The demand report of each area the revenue figures name: what the
/// partners pay for, next to what farmers there ask for.
fn demand_hrefs(kpis: &PartnerKpiDto) -> Result<HashMap<String, String>, Error> {
    let report = url("admin_demand", &[])?;
    Ok(kpis
        .provinces
        .iter()
        .flat_map(|p| p.areas.iter())
        .map(|a| (a.area.clone(), with_query(&report, &[("area", &a.area)])))
        .collect())
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    position: ListPosition,
    typed: Option<(PartnerForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let position = position.checked(SWITCH_LIST_STATES, PARTNER_SORTS);
    let partners = &state.usecases.partner;
    let filter = position.filter(SWITCH_LIST_STATES);
    let (list, kpis) = tokio::try_join!(
        partners.list_partners(actor, position.page(), &filter),
        partners.kpis(actor)
    )?;
    let path = url("admin_partners", &[])?;
    let mut kept = position.query();
    kept.retain(|(k, _)| *k != "page");
    let kept: Vec<(&str, &str)> = kept.iter().map(|(k, v)| (*k, v.as_str())).collect();
    if let Some(redirect) = past_the_end(&path, &kept, list.page, list.total_pages) {
        return redirect;
    }
    let mut rows = Vec::with_capacity(list.items.len());
    for partner in list.items {
        rows.push(PartnerRow {
            href: url("admin_partner", &[("id", &partner.id.to_string())])?,
            kind_label: kind_label(state.locale(), &partner.kind),
            partner,
        });
    }
    let (form, errors) = typed.unwrap_or_default();
    let body = PartnersPage {
        pagination: position.pagination(&path, list.page, list.total_pages),
        search: position
            .search_box("admin_partners")?
            .picked(
                "contract",
                state.locale().text("admin_pick.contract"),
                state.locale().text("admin_pick.all_contracts"),
                ["in_force", "expiring", "ended", "none"]
                    .into_iter()
                    .map(|c| (c, state.locale().text(&format!("admin_pick.contract_{c}"))))
                    .collect(),
            )
            .picked(
                "kind",
                state.locale().text("admin_pick.partner_kind"),
                state.locale().text("admin_pick.all_partner_kinds"),
                PARTNER_KINDS
                    .iter()
                    .map(|k| (*k, kind_label(state.locale(), k)))
                    .collect(),
            )
            .dated(state.locale())
            .sorted(
                state.locale(),
                &position,
                "admin_sort.on_first",
                PARTNER_SORTS,
            ),
        states: position.shared_state_chips(
            state.locale(),
            "admin_partners",
            SWITCH_LIST_STATES,
            None,
        )?,
        total: list.total,
        rows,
        demand_hrefs: demand_hrefs(&kpis)?,
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

/// GET /admin/partners (`?page=`)
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(position): ApiQuery<ListPosition>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    list_page(&state, &actor, position, None, StatusCode::OK).await
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
            let position = ListPosition::default();
            list_page(&state, &actor, position, Some(typed), err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

/// The account form as typed. The password is never sent back.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AccountForm {
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
}

/// The form of the page that came back refused, with its values as typed.
pub enum Typed {
    Profile(PartnerForm),
    Account(AccountForm),
    Contract(ContractForm),
}

/// One contract on the partner's page: a short line leading to its own
/// page (`contracts::show`), where it is edited and its shops sponsored.
#[derive(Serialize)]
pub struct ContractRow {
    pub contract: ContractDto,
    pub href: String,
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
    let account_label = if p.account_email.is_some() {
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
        Badge::new("managed", p.account_email.is_some()),
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
    /// A page of its contracts, newest start first, and the links to the
    /// others.
    pub contracts: Vec<ContractRow>,
    /// The contracts' toolbar and its state chips (contracts part only).
    pub contract_search: SearchBox,
    pub contract_states: Vec<AdminTab>,
    pub pagination: Option<Pagination>,
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
    (tab, asked): (&'static str, ListPosition),
    outcome: Outcome<Typed>,
) -> Result<Response, Error> {
    let partners = &state.usecases.partner;
    // A refused form is shown on its part of the page.
    let tab = match &outcome.form {
        Some(Typed::Profile(_)) => TAB_PROFILE,
        Some(Typed::Contract(_)) => TAB_CONTRACTS,
        _ => tab,
    };
    // The add form's choices (every ward, every category) only where it is.
    let on_contracts = tab == TAB_CONTRACTS;
    let asked = if on_contracts {
        asked.checked(CONTRACT_LIST_STATES, &[])
    } else {
        ListPosition::default()
    };
    let criteria = ContractFilter {
        state: asked.state_in(CONTRACT_LIST_STATES),
        area: asked.param("area"),
        fee_min: asked.fee(&asked.fee_min),
        fee_max: asked.fee(&asked.fee_max),
        start_from: ListPosition::day(&asked.start_from),
        start_to: ListPosition::day(&asked.start_to),
        end_from: ListPosition::day(&asked.end_from),
        end_to: ListPosition::day(&asked.end_to),
        category_id: asked.category_id(),
    };
    let (partner, contracts, areas, categories, contract_areas) = tokio::try_join!(
        partners.partner(actor, id),
        partners.partner_contracts(actor, id, asked.page(), &criteria),
        async {
            if on_contracts {
                partners.area_choices(actor).await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if on_contracts {
                category_choices(state).await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if on_contracts {
                partners.contract_areas(actor, id).await
            } else {
                Ok(Vec::new())
            }
        },
    )?;
    let id_text = id.to_string();
    let path = url("admin_partner", &[("id", &id_text)])?;
    let position = ListPosition {
        tab: TAB_CONTRACTS.to_owned(),
        ..asked
    };
    if on_contracts {
        let wanted_query = position.query();
        let params: Vec<(&str, &str)> = wanted_query
            .iter()
            .filter(|(k, _)| *k != "page")
            .map(|(k, v)| (*k, v.as_str()))
            .collect();
        if let Some(redirect) = past_the_end(&path, &params, contracts.page, contracts.total_pages)
        {
            return redirect;
        }
    }
    let locale = state.locale();
    let contract_search = position
        .search_box_at(&path, &[])?
        .without_words()
        .picked(
            "area",
            locale.text("partner.contract_area"),
            locale.text("partner.all_areas"),
            contract_areas
                .into_iter()
                .map(|a| (a.code, a.name))
                .collect(),
        )
        .ranged(
            ("fee_min", "fee_max"),
            locale.text("partner.contract_fee"),
            (
                locale.text("admin_search.from"),
                locale.text("admin_search.to"),
            ),
        );
    let contract_states = position.state_chips_at(
        &path,
        CONTRACT_LIST_STATES,
        locale.text("filters.all"),
        |s| locale.text(&format!("contract_state.{s}")),
        None,
    )?;
    let pagination = position.pagination(&path, contracts.page, contracts.total_pages);
    let contract_count = usize::try_from(contracts.total).unwrap_or_default();
    let contracts = contracts.items;
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
    let (mut profile, mut account, mut contract) = (None, None, None);
    match outcome.form {
        Some(Typed::Profile(f)) => profile = Some(f),
        Some(Typed::Account(f)) => account = Some(f),
        Some(Typed::Contract(f)) => contract = Some(f),
        None => {}
    }
    let mut rows = Vec::with_capacity(contracts.len());
    for contract in contracts {
        rows.push(ContractRow {
            href: url(
                "admin_partner_contract",
                &[("id", &id_text), ("cid", &contract.id.to_string())],
            )?,
            contract,
        });
    }
    let refused_form = profile.is_some() || account.is_some() || contract.is_some();
    let contracts_href = tab_href(&path, &PARTNER_TABS, TAB_CONTRACTS);
    let (header, confirms) = partner_controls(state, &partner, &contracts_href)?;
    let tabs = record_tabs(state, &path, &PARTNER_TABS, tab, |code| {
        (code == TAB_CONTRACTS).then_some(contract_count)
    });
    let account_label = state.locale().text(if partner.account_email.is_some() {
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
            email: partner.account_email.clone().unwrap_or_default(),
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
        pagination,
        contract_search,
        contract_states,
        kinds: kinds(state.locale()),
        areas,
        categories,
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
/// The partner's contracts: where a contract's page leads back to.
pub(super) fn contracts_href(id: i32) -> Result<String, Error> {
    let page = url("admin_partner", &[("id", &id.to_string())])?;
    Ok(tab_href(&page, &PARTNER_TABS, TAB_CONTRACTS))
}

pub(super) fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
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
            partner_page(
                state,
                actor,
                id,
                (TAB_OVERVIEW, ListPosition::default()),
                outcome,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

/// GET /admin/partners/{id} (`?tab=`, the contracts' `?page=`)
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(q): ApiQuery<ListPosition>,
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
    let mut response = partner_page(&state, &actor, id, (tab, q), outcome).await?;
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
