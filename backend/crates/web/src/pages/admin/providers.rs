//! /admin/providers and /admin/providers/{id}: the provider list with a create
//! form, and one provider's details, profile, verification badge and
//! managing account.
//!
//! The create form is a sheet on the list: an admin on a field visit lists a
//! business from its owner's phone number, with no account (one can be
//! assigned later on its page), and lands on its page to add its services.
//! Its page carries the same profile form its owner has on the dashboard.
//!
//! Withdrawing the badge asks for confirmation and a reason (a dialog with
//! JS, the same form on the page without); granting it is one button. Both
//! set an explicit state, so a form sent twice changes nothing. The account
//! form links an existing provider account by its email, or creates one.
//! Turning a provider off (gone from every public page with its listings)
//! also asks for confirmation and a reason; turning it on is one button.

use application::{
    authz::Actor,
    catalogue::{
        PERM_INQUIRY_READ, PERM_LISTING_MODERATE, PERM_PROVIDER_ACCOUNT_ASSIGN,
        PERM_PROVIDER_MANAGE, PERM_PROVIDER_STATUS_UPDATE, PERM_PROVIDER_VERIFY,
        PERM_REVIEW_MODERATE, PERM_SERVICE_MANAGE,
    },
    outputs::{
        AdminPagedResponse, AdminProviderDetailDto, AdminProviderDto, PlaceFormDto,
        ProviderActivityDto,
    },
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    models::admin_list::{AdminSort, LIST_STATE_UNVERIFIED, PROVIDER_LIST_STATES},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::MAP_POINT_URL,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::admin::{
        products::CREATE_SHEET as PRODUCT_CREATE_SHEET,
        services::CREATE_SHEET as SERVICE_CREATE_SHEET,
    },
    pages::{
        admin::{
            chosen_tab,
            dashboard::{card, StatCard},
            history_rows, is_refusal, record_tabs, render_admin, tab_href, to_tab, url, user_href,
            AdminTab, Badge, Confirm, HistoryRow, ListPosition, Outcome, ReasonForm, RecordAction,
            RecordHeader, SearchBox, TabQuery, TabSpec,
        },
        canonical_url,
        forms::{reason, FormErrors},
        listing::Pagination,
        parse_id,
        profile_form::ProfileForm,
        see_other,
        service_form::service_categories,
    },
};

#[derive(Serialize)]
pub struct ProviderRow {
    pub provider: AdminProviderDto,
    pub href: String,
}

#[derive(Serialize)]
pub struct ProvidersPage {
    pub total: i64,
    pub rows: Vec<ProviderRow>,
    pub pagination: Option<Pagination>,
    pub search: SearchBox,
    /// The unverified / verified / off chips.
    pub states: Vec<AdminTab>,
    /// The create form, `None` without `provider.manage`.
    pub create: Option<CreateSheet>,
}

/// The create sheet, with the values as typed after a refusal.
#[derive(Serialize)]
pub struct CreateSheet {
    pub action: String,
    pub form: ProfileForm,
    pub errors: FormErrors,
    /// The location box: every ward, and the one typed.
    pub place: PlaceFormDto,
}

/// The orders the provider list offers besides its own (verified first,
/// then the newest).
const PROVIDER_SORTS: &[AdminSort] = &[AdminSort::Newest, AdminSort::Oldest, AdminSort::Name];

async fn list_page(
    state: &AppState,
    actor: &Actor,
    position: &ListPosition,
    typed: Option<(ProfileForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let position = &position
        .clone()
        .checked(PROVIDER_LIST_STATES, PROVIDER_SORTS);
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_providers(
            actor,
            position.page(),
            &position.filter(PROVIDER_LIST_STATES),
        )
        .await?;
    let unverified = state
        .usecases
        .admin
        .count_unverified_providers(actor)
        .await?;
    let sectors = state.usecases.sector.named().await?;
    let path = url("admin_providers", &[])?;
    let mut rows = Vec::with_capacity(items.len());
    for provider in items {
        let href = url("admin_provider", &[("id", &provider.id.to_string())])?;
        rows.push(ProviderRow { provider, href });
    }
    let create = if actor.can(PERM_PROVIDER_MANAGE) {
        let (form, errors) = typed.unwrap_or_default();
        let place = state
            .usecases
            .place
            .form(form.ward_code().as_deref(), form.point())
            .await?;
        Some(CreateSheet {
            action: path.clone(),
            form,
            errors,
            place,
        })
    } else {
        None
    };
    let body = ProvidersPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        search: position
            .search_box("admin_providers")?
            .picked(
                "sector",
                state.locale().text("sector.label"),
                state.locale().text("admin_pick.all_sectors"),
                sectors
                    .iter()
                    .map(|s| (s.code.clone(), s.name.clone()))
                    .collect(),
            )
            .picked(
                "rating",
                state.locale().text("admin_pick.rating_from"),
                state.locale().text("admin_pick.all_ratings"),
                (3..=5)
                    .rev()
                    .map(|n| {
                        (
                            n,
                            state
                                .locale()
                                .text_with("admin_pick.stars_up", &[("n", &n.to_string())]),
                        )
                    })
                    .collect(),
            )
            .texted(
                "area",
                state.locale().text("admin_pick.address"),
                state.locale().text("admin_pick.address_hint"),
            )
            .picked(
                "category_id",
                state.locale().text("admin_pick.service_type"),
                state.locale().text("admin_pick.all_types"),
                service_categories(state)
                    .await?
                    .into_iter()
                    .map(|c| (c.id, c.name))
                    .collect(),
            )
            .picked(
                "gap",
                state.locale().text("admin_pick.gap"),
                state.locale().text("admin_pick.all_gaps"),
                ["no_services", "no_products", "no_account"]
                    .into_iter()
                    .map(|g| (g, state.locale().text(&format!("admin_pick.gap_{g}"))))
                    .collect(),
            )
            .dated(state.locale())
            .sorted(
                state.locale(),
                position,
                "admin_sort.verified",
                PROVIDER_SORTS,
            ),
        states: position.shared_state_chips(
            state.locale(),
            "admin_providers",
            PROVIDER_LIST_STATES,
            Some((LIST_STATE_UNVERIFIED, unverified)),
        )?,
        create,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/providers.html",
        &body,
    )
}

/// GET /admin/providers
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(position): ApiQuery<ListPosition>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    list_page(&state, &actor, &position, None, StatusCode::OK).await
}

/// POST /admin/providers
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state
        .usecases
        .admin
        .create_provider(&actor, form.input())
        .await
    {
        Ok(id) => redirect_to(id, Flash::ProviderCreated),
        Err(err) if is_refusal(&err) => {
            let typed = (form, FormErrors::from_error(&state, &err));
            let status = err.kind().status();
            list_page(
                &state,
                &actor,
                &ListPosition::default(),
                Some(typed),
                status,
            )
            .await
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
    Profile(ProfileForm),
    Account(AccountForm),
}

/// What farmers did about the provider lately, as the dashboard's cards:
/// taps, contact requests (to their list), reviews (to theirs); a figure
/// the visitor may not read is left out.
fn activity_cards(
    state: &AppState,
    activity: &ProviderActivityDto,
    inquiries_href: Option<&String>,
    reviews_href: Option<&String>,
) -> Vec<StatCard> {
    let locale = state.locale();
    let mut cards = vec![card(
        locale.text_with("admin_stats.calls", &[("days", &activity.days)]),
        activity.taps,
        "fa-phone",
        "green",
    )];
    if let Some(i) = &activity.inquiries {
        let note = if i.waiting > 0 {
            Some(locale.text_with("admin.inquiries_waiting", &[("count", &i.waiting)]))
        } else {
            (i.total > 0).then(|| locale.text("admin.inquiries_none_waiting"))
        };
        cards.push(StatCard {
            href: inquiries_href.cloned(),
            note,
            ..card(
                locale.text("contact.title"),
                i.total,
                "fa-envelope-open-text",
                "blue",
            )
        });
    }
    if let Some(r) = &activity.reviews {
        let mut parts = Vec::new();
        if let Some(average) = r.average {
            parts.push(locale.text_with("admin.reviews_average", &[("average", &average)]));
        }
        if r.waiting > 0 {
            parts.push(locale.text_with("admin.reviews_waiting", &[("count", &r.waiting)]));
        }
        cards.push(StatCard {
            href: reviews_href.cloned(),
            note: (!parts.is_empty()).then(|| parts.join(" · ")),
            ..card(
                locale.text("admin_menu.reviews"),
                r.shown,
                "fa-star",
                "amber",
            )
        });
    }
    cards
}

#[derive(Serialize)]
pub struct ProviderPage {
    pub provider: AdminProviderDetailDto,
    /// Its name, states and actions, and the confirmations they open.
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    /// The parts of the page (`?tab=`), and the one shown.
    pub tabs: Vec<AdminTab>,
    pub tab: &'static str,
    /// Its latest changes: who turned it off, verified or edited it, and why.
    pub history: Vec<HistoryRow>,
    /// Its recent activity, as the dashboard's cards.
    pub cards: Vec<StatCard>,
    /// Its position on a map, to check where it was placed.
    pub map_href: Option<String>,
    /// The managing account's admin page, when the visitor may read accounts.
    pub linked_user_href: Option<String>,
    pub can_verify: bool,
    pub can_assign: bool,
    pub can_edit: bool,
    pub account_action: String,
    /// The account sheet's title: "assign", or "change" once one is linked.
    pub account_label: String,
    /// The create sheets of its services and supplies (their lists narrowed
    /// to it), while the visitor may add them.
    pub services_add_href: Option<String>,
    pub products_add_href: Option<String>,
    pub profile_action: String,
    /// This provider's services and products in their admin lists, when the
    /// visitor may manage them.
    pub services_href: Option<String>,
    pub products_href: Option<String>,
    pub form: AccountForm,
    pub profile: ProfileForm,
    /// The profile form's messages, apart so they never show in the others.
    pub profile_errors: FormErrors,
    /// The profile's location box, on the profile tab only.
    pub place: Option<PlaceFormDto>,
    pub errors: FormErrors,
    /// The form `errors` belong to: "account", or "" (a refused confirmation,
    /// shown above everything); "profile" when they are in `profile_errors`.
    pub failed: &'static str,
    pub flash: Option<Flash>,
}

/// The parts of a provider's page, each a URL (`?tab=`): what decides about
/// it first, then what it offers, its profile form, its history.
const PROVIDER_TABS: [TabSpec; 4] = [
    TabSpec {
        code: TAB_OVERVIEW,
        label: "admin.tab_overview",
    },
    TabSpec {
        code: TAB_OFFERS,
        label: "admin.tab_offers",
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
const TAB_OFFERS: &str = "offers";
const TAB_PROFILE: &str = "profile";
const TAB_HISTORY: &str = "history";

/// A provider's page, or one of its parts.
fn provider_href(id: i32, tab: &str) -> Result<String, Error> {
    Ok(tab_href(
        &url("admin_provider", &[("id", &id.to_string())])?,
        &PROVIDER_TABS,
        tab,
    ))
}
const ACCOUNT_SHEET: &str = "gan-tai-khoan";
const UNLINK_SHEET: &str = "go-tai-khoan";
const UNVERIFY_SHEET: &str = "bo-xac-minh";
const TURN_OFF_SHEET: &str = "tat-nha-cung-cap";

/// The provider's header and the confirmations it opens. Buttons: granting
/// the badge and turning it back on (what an admin does to a new listing),
/// the public page. The menu: the managing account (assign or change, then
/// remove), withdrawing the badge, turning it off, each only for a visitor
/// allowed to.
fn provider_controls(
    state: &AppState,
    actor: &Actor,
    p: &AdminProviderDetailDto,
) -> Result<(RecordHeader, Vec<Confirm>), Error> {
    let locale = state.locale();
    let text = |key: &str| locale.text(key);
    let named = |key: &str, name: &str| locale.text_with(key, &[("name", &name)]);
    let id = p.id.to_string();
    let verification = url("admin_provider_verification", &[("id", &id)])?;
    let status = url("admin_provider_status", &[("id", &id)])?;
    let can_verify = actor.can(PERM_PROVIDER_VERIFY);
    let can_status = actor.can(PERM_PROVIDER_STATUS_UPDATE);
    let can_assign = actor.can(PERM_PROVIDER_ACCOUNT_ASSIGN);

    let mut actions = Vec::new();
    if can_verify && !p.is_verified {
        actions.push(RecordAction::post(
            text("provider.verify"),
            "fa-circle-check",
            verification.clone(),
            "is_verified",
            "true",
        ));
    }
    if can_status && !p.is_active {
        actions.push(RecordAction::post(
            text("admin.turn_on_provider"),
            "fa-store",
            status.clone(),
            "is_active",
            "true",
        ));
    }
    actions.push(RecordAction::link(
        text("admin.public_page"),
        "fa-arrow-up-right-from-square",
        canonical_url("provider", p.id, &p.name)?,
    ));

    let mut menu = Vec::new();
    let mut confirms = Vec::new();
    if can_assign {
        let label = if p.linked_email.is_some() {
            "admin.change_account"
        } else {
            "admin.assign"
        };
        menu.push(RecordAction::sheet(
            text(label),
            "fa-user-plus",
            ACCOUNT_SHEET,
            "success",
        ));
        if let Some(email) = &p.linked_email {
            menu.push(RecordAction::sheet(
                text("admin.unlink_account"),
                "fa-user-minus",
                UNLINK_SHEET,
                "warning",
            ));
            confirms.push(Confirm {
                button_class: "btn-warning",
                ..Confirm::delete(
                    UNLINK_SHEET,
                    named("admin.unlink_account_title", email),
                    "fa-user-minus",
                    url("admin_provider_account_delete", &[("id", &id)])?,
                    text("admin.unlink_account"),
                )
                .noting(text("admin.unlink_account_note"))
            });
        }
    }
    if can_verify && p.is_verified {
        menu.push(RecordAction::sheet(
            text("admin.unverify"),
            "fa-circle-xmark",
            UNVERIFY_SHEET,
            "warning",
        ));
        confirms.push(
            Confirm::off(
                UNVERIFY_SHEET,
                named("admin.unverify_title", &p.name),
                "fa-circle-xmark",
                verification,
                text("admin.confirm_unverify"),
                "is_verified",
            )
            .noting(text("admin.unverify_note")),
        );
    }
    if can_status && p.is_active {
        menu.push(RecordAction::sheet(
            text("admin.turn_off_provider"),
            "fa-store-slash",
            TURN_OFF_SHEET,
            "warning",
        ));
        confirms.push(
            Confirm::off(
                TURN_OFF_SHEET,
                named("admin.turn_off_provider_title", &p.name),
                "fa-store-slash",
                status,
                text("admin.confirm_off"),
                "is_active",
            )
            .noting(text("admin.turn_off_provider_note")),
        );
    }

    let mut badges = vec![
        Badge::new("account", p.is_active),
        Badge::new("verified", p.is_verified),
        Badge::new("managed", p.linked_email.is_some()),
    ];
    if p.is_demo {
        badges.push(Badge::new("demo", true));
    }
    let header = RecordHeader {
        title: p.name.clone(),
        icon: "fa-store".to_owned(),
        sub: None,
        back_href: url("admin_providers", &[])?,
        back_label: text("admin_menu.providers"),
        badges,
        actions,
        menu,
    };
    Ok((header, confirms))
}

async fn provider_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    tab: &'static str,
    outcome: Outcome<Typed>,
) -> Result<Response, Error> {
    let provider = state.usecases.admin.get_provider_detail(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_provider", &[("id", &id_text)])?;
    let (typed_profile, typed_account, failed) = match outcome.form {
        Some(Typed::Profile(f)) => (Some(f), None, "profile"),
        Some(Typed::Account(f)) => (None, Some(f), "account"),
        None => (None, None, ""),
    };
    // A refused profile is shown on its tab; the profile tab needs the
    // permission to edit.
    let can_edit = actor.can(PERM_PROVIDER_MANAGE);
    let tab = match (failed, tab) {
        ("profile", _) => TAB_PROFILE,
        (_, TAB_PROFILE) if !can_edit => TAB_OVERVIEW,
        (_, tab) => tab,
    };
    // The account it has, else the business's own address: usually the
    // owner's.
    let form = typed_account.unwrap_or_else(|| AccountForm {
        email: provider
            .linked_email
            .clone()
            .or_else(|| provider.email.clone())
            .unwrap_or_default(),
        ..AccountForm::default()
    });
    let (profile, profile_errors, errors) = match typed_profile {
        Some(typed) => (typed, outcome.errors, FormErrors::default()),
        None => (
            ProfileForm::from_admin(&provider),
            FormErrors::default(),
            outcome.errors,
        ),
    };
    let place = match tab {
        TAB_PROFILE => Some(
            state
                .usecases
                .place
                .form(profile.ward_code().as_deref(), profile.point())
                .await?,
        ),
        _ => None,
    };
    let own = ListPosition {
        provider_id: id_text.clone(),
        ..ListPosition::default()
    };
    let list_of = |perm: &str, route: &str| -> Result<Option<String>, Error> {
        actor.can(perm).then(|| own.href(route, &[])).transpose()
    };
    let inquiries_href = list_of(PERM_INQUIRY_READ, "admin_inquiries")?;
    let reviews_href = list_of(PERM_REVIEW_MODERATE, "admin_reviews")?;
    let offers = provider.services.len() + provider.products.len();
    let shown: Vec<TabSpec> = PROVIDER_TABS
        .into_iter()
        .filter(|t| t.code != TAB_PROFILE || can_edit)
        .collect();
    let tabs = record_tabs(state, &path, &shown, tab, |code| {
        (code == TAB_OFFERS).then_some(offers)
    });
    let (header, confirms) = provider_controls(state, actor, &provider)?;
    let account_label = state.locale().text(if provider.linked_email.is_some() {
        "admin.change_account"
    } else {
        "admin.assign"
    });
    let body = ProviderPage {
        header,
        confirms,
        tabs,
        tab,
        history: history_rows(state, actor, HistoryOf::Provider(id)).await?,
        cards: activity_cards(
            state,
            &provider.activity,
            inquiries_href.as_ref(),
            reviews_href.as_ref(),
        ),
        map_href: provider
            .lat
            .zip(provider.lng)
            .map(|(lat, lng)| format!("{MAP_POINT_URL}{lat},{lng}")),
        services_add_href: list_of(PERM_SERVICE_MANAGE, "admin_services")?
            .map(|href| format!("{href}#{SERVICE_CREATE_SHEET}")),
        products_add_href: list_of(PERM_LISTING_MODERATE, "admin_products")?
            .map(|href| format!("{href}#{PRODUCT_CREATE_SHEET}")),
        services_href: list_of(PERM_SERVICE_MANAGE, "admin_services")?,
        products_href: list_of(PERM_LISTING_MODERATE, "admin_products")?,
        linked_user_href: user_href(actor, provider.linked_user_id)?,
        account_label,
        provider,
        can_verify: actor.can(PERM_PROVIDER_VERIFY),
        can_assign: actor.can(PERM_PROVIDER_ACCOUNT_ASSIGN),
        can_edit,
        account_action: url("admin_provider_account", &[("id", &id_text)])?,
        profile_action: provider_href(id, TAB_PROFILE)?,
        form,
        profile,
        profile_errors,
        place,
        errors,
        failed,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/provider.html",
        &body,
    )
}

/// The page again after one of its forms was refused.
async fn refused(
    state: &AppState,
    actor: &Actor,
    id: i32,
    err: &Error,
    typed: Typed,
) -> Result<Response, Error> {
    let outcome = Outcome::refused(state, err, Some(typed), false);
    provider_page(state, actor, id, TAB_OVERVIEW, outcome).await
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    redirect_to_tab(id, TAB_OVERVIEW, flash)
}

fn redirect_to_tab(id: i32, tab: &str, flash: Flash) -> Result<Response, Error> {
    to_tab(
        &url("admin_provider", &[("id", &id.to_string())])?,
        &PROVIDER_TABS,
        tab,
        flash,
    )
}

/// GET /admin/providers/{id} (`?tab=` one of its parts)
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
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let tab = chosen_tab(&PROVIDER_TABS, &q.tab);
    let mut response = provider_page(&state, &actor, id, tab, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/providers/{id}: the profile form.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    match state
        .usecases
        .admin
        .update_provider(&actor, id, form.input())
        .await
    {
        Ok(()) => redirect_to_tab(id, TAB_PROFILE, Flash::ProviderSaved),
        Err(err) if is_refusal(&err) => {
            refused(&state, &actor, id, &err, Typed::Profile(form)).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VerificationForm {
    /// "true" to grant the badge, "false" to withdraw it.
    pub is_verified: String,
    pub reason: String,
}

/// POST /admin/providers/{id}/verification
pub async fn set_verification(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<VerificationForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let verified = form.is_verified == "true";
    state
        .usecases
        .admin
        .set_verification(&actor, id, verified, reason(&form.reason))
        .await?;
    let flash = if verified {
        Flash::ProviderVerified
    } else {
        Flash::ProviderUnverified
    };
    redirect_to(id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to turn the provider on, "false" to turn it off.
    pub is_active: String,
    pub reason: String,
}

/// POST /admin/providers/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let active = form.is_active == "true";
    state
        .usecases
        .admin
        .toggle_provider_active(&actor, id, active, reason(&form.reason))
        .await?;
    let flash = if active {
        Flash::ProviderActivated
    } else {
        Flash::ProviderDeactivated
    };
    redirect_to(id, flash)
}

/// POST /admin/providers/{id}/account
pub async fn assign_account(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<AccountForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let result = state
        .usecases
        .admin
        .assign_provider_user(&actor, id, form.email.clone(), form.password.clone())
        .await;
    match result {
        Ok(_) => redirect_to(id, Flash::ProviderAccountAssigned),
        Err(err) if is_refusal(&err) => {
            refused(&state, &actor, id, &err, Typed::Account(form)).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/providers/{id}/account/delete: the business no longer has a
/// managing account (confirmation with a reason); the account stays.
pub async fn unlink_account(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    state
        .usecases
        .admin
        .unlink_provider_user(&actor, id, reason(&form.reason))
        .await?;
    redirect_to(id, Flash::ProviderAccountUnlinked)
}
