//! /admin/partners/{id}/contracts/{cid}: one partner contract as a record
//! page. Its sponsored shops come first (the work done most often), with a
//! search of the shops of its area it may add (`?q=`: the first
//! `SPONSOR_CANDIDATES_SHOWN` by name, picked by the database, since an area
//! can hold thousands); its terms are the second part, one edit form (the
//! area stays). Deleting it is in the header's menu, confirmed with a reason,
//! and lands on the partner's contracts. A contract is added on the
//! partner's page.

use std::collections::HashMap;

use application::{
    authz::Actor,
    constants::SPONSOR_CANDIDATES_SHOWN,
    outputs::{ContractDto, NamedDto, PartnerDto, SponsorCandidateDto},
};
use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            chosen_tab, is_refusal,
            partners::{
                category_choices, contracts_href, redirect_to, CategoryOption, ContractForm,
            },
            record_tabs, render_admin, tab_href, url, AdminTab, Badge, Confirm, Outcome,
            ReasonForm, RecordAction, RecordHeader, SearchBox, TabSpec,
        },
        forms::{reason, FormErrors},
        listing::with_query,
        parse_id, see_other,
    },
};

/// The parts of a contract's page (`?tab=`): its sponsored shops, then its
/// terms.
const CONTRACT_TABS: [TabSpec; 2] = [
    TabSpec {
        code: TAB_SPONSORED,
        label: "partner.sponsored",
    },
    TabSpec {
        code: TAB_TERMS,
        label: "partner.tab_terms",
    },
];
const TAB_SPONSORED: &str = "sponsored";
const TAB_TERMS: &str = "terms";

const DELETE_SHEET: &str = "xoa-hop-dong";

/// The page's query: its part, and the words of the sponsor search.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ContractQuery {
    pub tab: String,
    pub q: String,
}

impl ContractQuery {
    fn search(&self) -> Option<&str> {
        Some(self.q.trim()).filter(|q| !q.is_empty())
    }
}

/// A sponsored shop, with its "stop" confirmation's form.
#[derive(Serialize)]
pub struct SponsoredRow {
    pub provider: NamedDto,
    pub remove_action: String,
    /// It has moved out of the contract's area since it was sponsored.
    pub away: bool,
}

#[derive(Serialize)]
pub struct ContractPage {
    pub partner: PartnerDto,
    pub contract: ContractDto,
    /// Its area, its partner, and deleting it in the menu.
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    pub tabs: Vec<AdminTab>,
    pub tab: &'static str,
    /// The one-time message after a change, worded.
    pub flash_text: Option<String>,
    /// A refused sponsorship or deletion, shown above everything.
    pub errors: FormErrors,
    pub sponsored: Vec<SponsoredRow>,
    /// The sponsor search, and the shops it found (sponsored part only).
    pub search: SearchBox,
    pub candidates: Vec<SponsorCandidateDto>,
    /// The list was cut at `shown`: more shops of the area may be found by
    /// name.
    pub more_candidates: bool,
    pub shown: u64,
    pub sponsor_action: String,
    /// The terms form (terms part only), as stored or as typed.
    pub form: ContractForm,
    pub form_errors: FormErrors,
    pub categories: Vec<CategoryOption>,
    pub edit_action: String,
}

/// The contract's page address.
fn contract_href(id: i32, cid: i32) -> Result<String, Error> {
    url(
        "admin_partner_contract",
        &[("id", &id.to_string()), ("cid", &cid.to_string())],
    )
}

/// 303 to the contract's page, on `tab`, the sponsor search kept.
fn redirect_to_contract(
    id: i32,
    cid: i32,
    tab: &str,
    search: &str,
    flash: Flash,
) -> Result<Response, Error> {
    let mut href = tab_href(&contract_href(id, cid)?, &CONTRACT_TABS, tab);
    if !search.trim().is_empty() {
        href = with_query(&href, &[("q", search.trim())]);
    }
    let mut response = see_other(&href)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// The header: the area as title, the partner beneath and as the way back,
/// deleting in the menu.
fn contract_controls(
    state: &AppState,
    partner: &PartnerDto,
    contract: &ContractDto,
    delete_action: String,
    confirm_open: bool,
) -> Result<(RecordHeader, Vec<Confirm>), Error> {
    let locale = state.locale();
    let mut badges = Vec::new();
    if partner.is_demo {
        badges.push(Badge::new("demo", true));
    }
    let header = RecordHeader {
        title: contract.area_label.clone(),
        icon: "fa-file-signature".to_owned(),
        sub: Some(partner.name.clone()),
        back_href: contracts_href(partner.id)?,
        back_label: partner.name.clone(),
        badges,
        actions: Vec::new(),
        menu: vec![RecordAction::sheet(
            locale.text("partner.delete_contract"),
            "fa-trash",
            DELETE_SHEET,
            "danger",
        )],
    };
    let delete = Confirm::delete(
        DELETE_SHEET,
        locale.text_with(
            "partner.delete_contract_title",
            &[("name", &contract.area_label)],
        ),
        "fa-trash",
        delete_action,
        locale.text("common.confirm_delete"),
    )
    .saying("delete", locale.text("partner.contract_what"))
    .opened(confirm_open);
    Ok((header, vec![delete]))
}

async fn contract_page(
    state: &AppState,
    actor: &Actor,
    (id, cid): (i32, i32),
    tab: &'static str,
    query: &ContractQuery,
    outcome: Outcome<ContractForm>,
) -> Result<Response, Error> {
    let partners = &state.usecases.partner;
    // A refused edit comes back on the terms.
    let tab = if outcome.form.is_some() {
        TAB_TERMS
    } else {
        tab
    };
    let search = query.search();
    // Each part loads only what it shows.
    let sponsored_part = tab == TAB_SPONSORED;
    let ((partner, contract), candidates, categories) = tokio::try_join!(
        partners.get_contract(actor, id, cid),
        async {
            if sponsored_part {
                partners.sponsor_candidates(actor, id, cid, search).await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if sponsored_part {
                Ok(Vec::new())
            } else {
                category_choices(state).await
            }
        },
    )?;
    let (id_text, cid_text) = (id.to_string(), cid.to_string());
    let path = contract_href(id, cid)?;
    let mut sponsored = Vec::with_capacity(contract.sponsored.len());
    for s in &contract.sponsored {
        sponsored.push(SponsoredRow {
            remove_action: url(
                "admin_partner_sponsored_remove",
                &[
                    ("id", &id_text),
                    ("cid", &cid_text),
                    ("bid", &s.id.to_string()),
                ],
            )?,
            away: contract.sponsored_away.contains(&s.id),
            provider: s.clone(),
        });
    }
    let delete_action = url(
        "admin_partner_contract_delete",
        &[("id", &id_text), ("cid", &cid_text)],
    )?;
    let (header, confirms) = contract_controls(
        state,
        &partner,
        &contract,
        delete_action,
        outcome.confirm_open,
    )?;
    let count = sponsored.len();
    let tabs = record_tabs(state, &path, &CONTRACT_TABS, tab, |code| {
        (code == TAB_SPONSORED).then_some(count)
    });
    // The messages go with the terms form when it was refused, else above.
    let (errors, form_errors) = if outcome.form.is_some() {
        (FormErrors::default(), outcome.errors)
    } else {
        (outcome.errors, FormErrors::default())
    };
    let body = ContractPage {
        header,
        confirms,
        tabs,
        tab,
        flash_text: outcome.flash.map(|f| {
            state
                .locale()
                .text(&format!("partner_flash.{}", f.as_str()))
        }),
        errors,
        sponsored,
        search: SearchBox::plain(path.clone(), search.unwrap_or_default()),
        more_candidates: u64::try_from(candidates.len()).unwrap_or(u64::MAX)
            >= SPONSOR_CANDIDATES_SHOWN,
        shown: SPONSOR_CANDIDATES_SHOWN,
        candidates,
        sponsor_action: url(
            "admin_partner_sponsored",
            &[("id", &id_text), ("cid", &cid_text)],
        )?,
        form: outcome
            .form
            .unwrap_or_else(|| ContractForm::from_dto(&contract)),
        form_errors,
        categories,
        edit_action: path.clone(),
        partner,
        contract,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/contract.html",
        &body,
    )
}

/// The ids of a contract's address.
fn ids(raw_id: &str, raw_cid: &str) -> Result<(i32, i32), Error> {
    Ok((
        parse_id(raw_id, ErrorCode::PartnerNotFound)?,
        parse_id(raw_cid, ErrorCode::ContractNotFound)?,
    ))
}

/// Where a refused change comes back: its part, the sponsor search as it
/// was, the terms as typed, its confirmation open.
#[derive(Default)]
struct Back {
    tab: &'static str,
    query: ContractQuery,
    form: Option<ContractForm>,
    confirm_open: bool,
}

impl Back {
    fn to(tab: &'static str) -> Self {
        Self {
            tab,
            ..Self::default()
        }
    }
}

/// The page again after a refusal, or the failure itself when it is not
/// one.
async fn refused(
    state: &AppState,
    actor: &Actor,
    ids: (i32, i32),
    err: Error,
    back: Back,
) -> Result<Response, Error> {
    if !is_refusal(&err) {
        return Err(err);
    }
    let outcome = Outcome::refused(state, &err, back.form, back.confirm_open);
    contract_page(state, actor, ids, back.tab, &back.query, outcome).await
}

/// GET /admin/partners/{id}/contracts/{cid} (`?tab=`, `?q=`)
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiQuery(query): ApiQuery<ContractQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let ids = ids(&raw_id, &raw_cid)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let tab = chosen_tab(&CONTRACT_TABS, &query.tab);
    let mut response = contract_page(&state, &actor, ids, tab, &query, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/partners/{id}/contracts/{cid}: the terms form.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiForm(raw): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let (id, cid) = ids(&raw_id, &raw_cid)?;
    let form = ContractForm::from_map(&raw);
    match state
        .usecases
        .partner
        .update_contract(&actor, id, cid, form.input())
        .await
    {
        Ok(()) => redirect_to_contract(id, cid, TAB_TERMS, "", Flash::ContractSaved),
        Err(err) => {
            let back = Back {
                form: Some(form),
                ..Back::to(TAB_TERMS)
            };
            refused(&state, &actor, (id, cid), err, back).await
        }
    }
}

/// POST /admin/partners/{id}/contracts/{cid}/delete: lands on the
/// partner's contracts.
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath((raw_id, raw_cid)): ApiPath<(String, String)>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let (id, cid) = ids(&raw_id, &raw_cid)?;
    match state
        .usecases
        .partner
        .delete_contract(&actor, id, cid, reason(&form.reason))
        .await
    {
        Ok(()) => redirect_to(id, Flash::ContractDeleted),
        Err(err) => {
            let back = Back {
                confirm_open: true,
                ..Back::to(TAB_SPONSORED)
            };
            refused(&state, &actor, (id, cid), err, back).await
        }
    }
}

/// The sponsor form: the shop, and the search it was found by (kept on the
/// way back).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SponsorForm {
    pub business_id: String,
    pub q: String,
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
    let (id, cid) = ids(&raw_id, &raw_cid)?;
    let result = match parse_id(&form.business_id, ErrorCode::ProviderNotFound) {
        Ok(business) => {
            state
                .usecases
                .partner
                .add_sponsored(&actor, id, cid, business)
                .await
        }
        Err(err) => Err(err),
    };
    match result {
        Ok(()) => redirect_to_contract(id, cid, TAB_SPONSORED, &form.q, Flash::SponsoredAdded),
        Err(err) => {
            let back = Back {
                query: ContractQuery {
                    q: form.q,
                    ..ContractQuery::default()
                },
                ..Back::to(TAB_SPONSORED)
            };
            refused(&state, &actor, (id, cid), err, back).await
        }
    }
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
    let (id, cid) = ids(&raw_id, &raw_cid)?;
    let bid = parse_id(&raw_bid, ErrorCode::ProviderNotFound)?;
    match state
        .usecases
        .partner
        .remove_sponsored(&actor, id, cid, bid, reason(&form.reason))
        .await
    {
        Ok(()) => redirect_to_contract(id, cid, TAB_SPONSORED, "", Flash::SponsoredRemoved),
        Err(err) => refused(&state, &actor, (id, cid), err, Back::to(TAB_SPONSORED)).await,
    }
}
