//! GET /services/{id} and /services/{id}/{slug} — one service, as farmers see
//! it: photo, price, description, the provider (call button, their page),
//! the provider's other services, and the contact form with this service
//! already chosen.
//!
//! One canonical URL per service, `/services/{id}/{slug of its name}`; any
//! other spelling answers 301 to it. The form posts to /contact like the
//! provider page's, with `return_to=service`, so the visitor lands back here.

use application::outputs::{BusinessDetailDto, PublicServiceDto, ServiceDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::META_DESCRIPTION_MAX_CHARS,
    cookies::{Flash, TakeFlash},
    extract::ApiPath,
    pages::{
        canonical_url,
        cards::same_label,
        moved_permanently, parse_id,
        provider::{
            area_of, rating_line, ContactLimits, ContactState, RatingLine, LIMITS,
            RETURN_TO_SERVICE,
        },
        summary,
    },
    routes,
    shell::Shell,
    views::render_page_status,
};

/// A link to one of a provider's services (on a service's page: its siblings;
/// on a product's page: what the seller also does).
#[derive(Serialize)]
pub struct OtherService {
    pub name: String,
    pub price_note: Option<String>,
    pub category_icon: Option<String>,
    pub href: String,
}

#[derive(Serialize)]
pub struct ServicePage {
    /// The provider's rating and the way to its reviews.
    pub rating: RatingLine,
    pub service: PublicServiceDto,
    /// The category badge is left out when the name already says it.
    pub show_category: bool,
    /// The provider, with the services the contact form offers.
    pub provider: BusinessDetailDto,
    pub provider_href: String,
    pub category_href: String,
    pub others: Vec<OtherService>,
    pub meta_description: Option<String>,
    pub area: String,
    pub share_text: String,
    pub contact: ContactState,
    pub limits: ContactLimits,
    /// The form's hidden `return_to`.
    pub return_to: &'static str,
    /// The form was just sent (flash from the redirect).
    pub sent: bool,
}

async fn load(state: &AppState, raw_id: &str) -> Result<PublicServiceDto, Error> {
    let id = parse_id(raw_id, ErrorCode::ServiceNotFound)?;
    state.usecases.explore.get_service(id).await
}

pub(super) fn canonical(service: &PublicServiceDto) -> Result<String, Error> {
    canonical_url("service", service.id, &service.name)
}

/// Links to a provider's services, but `except` (the one already shown).
pub(super) fn service_links(
    services: &[ServiceDto],
    except: Option<i32>,
) -> Result<Vec<OtherService>, Error> {
    services
        .iter()
        .filter(|s| Some(s.id) != except)
        .map(|s| {
            Ok(OtherService {
                name: s.name.clone(),
                price_note: s.price_note.clone(),
                category_icon: s.category_icon.clone(),
                href: canonical_url("service", s.id, &s.name)?,
            })
        })
        .collect()
}

/// The page around `service`, with the contact form as `contact` left it
/// (fresh, or re-shown after a refusal). `provider` is the service's own.
pub(super) async fn render(
    state: &AppState,
    shell: &Shell,
    service: PublicServiceDto,
    provider: BusinessDetailDto,
    mut contact: ContactState,
    sent: bool,
    status: StatusCode,
) -> Result<Response, Error> {
    // A fresh form has this service chosen.
    if contact.values.service_id.is_empty() {
        contact.values.service_id = service.id.to_string();
    }
    let others = service_links(&provider.services, Some(service.id))?;
    let area = area_of(state.locale(), service.business_address.as_deref());
    let rating = rating_line(state, &provider).await?;
    let page = ServicePage {
        rating,
        show_category: !same_label(&service.name, &service.category),
        provider_href: canonical_url("provider", provider.id, &provider.name)?,
        category_href: routes::url(
            "service_category",
            &[
                ("id", &service.category_id.to_string()),
                ("slug", &service.category_slug),
            ],
        )
        .map_err(|e| Error::internal(format!("service page links: {e}")))?,
        others,
        meta_description: service
            .description
            .as_deref()
            .and_then(|d| summary(d, META_DESCRIPTION_MAX_CHARS)),
        share_text: state.locale().text_with(
            "service.share_text",
            &[
                ("name", &service.name),
                ("provider", &service.business_name),
                ("area", &area),
            ],
        ),
        area,
        service,
        provider,
        contact,
        limits: LIMITS,
        return_to: RETURN_TO_SERVICE,
        sent,
    };
    render_page_status(state, shell, status, "pages/service.html", &page)
}

/// GET /services/{id}
pub async fn by_id(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Response, Error> {
    let service = load(&state, &id).await?;
    moved_permanently(&canonical(&service)?)
}

/// GET /services/{id}/{slug}
pub async fn show(
    shell: Shell,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath((id, _slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let service = load(&state, &id).await?;
    let canonical = canonical(&service)?;
    if shell.path != canonical {
        return moved_permanently(&canonical);
    }
    let provider = state
        .usecases
        .explore
        .get_provider_detail(service.business_id)
        .await?;
    let sent = flash.0 == Some(Flash::ContactSent);
    let mut response = render(
        &state,
        &shell,
        service,
        provider,
        ContactState::default(),
        sent,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}
