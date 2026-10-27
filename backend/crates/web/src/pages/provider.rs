//! GET /providers/{id}, /providers/{id}/{slug} — provider detail — and
//! POST /contact, the contact form on it.
//!
//! The form is a plain HTML form (Post/Redirect/Get): success answers 303 to
//! the provider page with a one-time flash; a rule broken re-renders the page
//! with the typed values and one message per field. The rules themselves are
//! `ExploreUseCase::contact_request`, the same ones the JSON API uses.

use std::collections::BTreeMap;

use application::{
    constants::{NOTE_MAX_LEN, REQUESTER_NAME_MAX_LEN, TOPIC_MAX_LEN},
    inputs::ContactInput,
    outputs::BusinessDetailDto,
};
use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode, ErrorKind};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::META_DESCRIPTION_MAX_CHARS,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    locale::Locale,
    pages::{
        canonical_url, forms::FormErrors, listing::parse_opt, moved_permanently, parse_id,
        see_other, service as service_page, summary,
    },
    shell::Shell,
    views::{render_page, render_page_status},
};

/// What the visitor typed, echoed back when the form is re-shown.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ContactValues {
    pub requester_name: String,
    pub requester_phone: String,
    pub topic: String,
    /// The chosen service's id as sent, so the select keeps it.
    pub service_id: String,
    pub note: String,
}

/// POST /contact body. Flat, not `#[serde(flatten)]`: flattening makes the
/// urlencoded deserializer read `business_id` as a string and reject it.
#[derive(Debug, Deserialize)]
pub struct ContactForm {
    pub business_id: i32,
    #[serde(default)]
    pub requester_name: String,
    #[serde(default)]
    pub requester_phone: String,
    #[serde(default)]
    pub topic: String,
    /// The service select (a provider with services); its name becomes the
    /// topic. Blank or not a number: none chosen.
    #[serde(default)]
    pub service_id: String,
    #[serde(default)]
    pub note: String,
    /// `service` when the form was on a service's page: the visitor lands
    /// back there. The page is rebuilt from `service_id`, never a posted URL.
    #[serde(default)]
    pub return_to: String,
}

/// `return_to` of the form on a service's own page.
pub const RETURN_TO_SERVICE: &str = "service";

impl ContactForm {
    fn values(&self) -> ContactValues {
        ContactValues {
            requester_name: self.requester_name.clone(),
            requester_phone: self.requester_phone.clone(),
            topic: self.topic.clone(),
            service_id: self.service_id.clone(),
            note: self.note.clone(),
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ContactState {
    pub values: ContactValues,
    /// Localised message per field name.
    pub errors: BTreeMap<&'static str, String>,
    /// Shown above the fields when the form as a whole failed.
    pub message: Option<String>,
    /// Re-shown after an error: the sheet opens by itself.
    pub open: bool,
}

/// Length caps the form shows as `maxlength`; the use case enforces the same.
#[derive(Serialize)]
pub struct ContactLimits {
    pub name_max: usize,
    pub topic_max: usize,
    pub note_max: usize,
}

pub(super) const LIMITS: ContactLimits = ContactLimits {
    name_max: REQUESTER_NAME_MAX_LEN,
    topic_max: TOPIC_MAX_LEN,
    note_max: NOTE_MAX_LEN,
};

#[derive(Serialize)]
pub struct ProviderPage {
    pub provider: BusinessDetailDto,
    pub meta_description: Option<String>,
    pub contact: ContactState,
    pub limits: ContactLimits,
    /// Where the provider works, or the region when the address is unknown.
    pub area: String,
    /// The text a share sheet (Zalo, Messenger) puts with the link.
    pub share_text: String,
    /// The form was just sent (flash from the redirect).
    pub sent: bool,
}

/// The region the platform serves (its text key); stands in for a missing
/// address.
pub(super) const DEFAULT_AREA: &str = "site.region";

async fn load(state: &AppState, raw_id: &str) -> Result<BusinessDetailDto, Error> {
    let id = parse_id(raw_id, ErrorCode::ProviderNotFound)?;
    state.usecases.explore.get_provider_detail(id).await
}

fn canonical(provider: &BusinessDetailDto) -> Result<String, Error> {
    canonical_url("provider", provider.id, &provider.name)
}

/// Where a provider works, or the region when the address is unknown.
pub(super) fn area_of(locale: &Locale, address: Option<&str>) -> String {
    address
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map_or_else(|| locale.text(DEFAULT_AREA), str::to_owned)
}

fn page(
    locale: &Locale,
    provider: BusinessDetailDto,
    contact: ContactState,
    sent: bool,
) -> ProviderPage {
    let area = area_of(locale, provider.address.as_deref());
    ProviderPage {
        share_text: locale.text_with(
            "provider.share_text",
            &[("name", &provider.name), ("area", &area)],
        ),
        area,
        meta_description: provider
            .description
            .as_deref()
            .and_then(|d| summary(d, META_DESCRIPTION_MAX_CHARS)),
        provider,
        contact,
        limits: LIMITS,
        sent,
    }
}

/// GET /providers/{id}
pub async fn by_id(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Response, Error> {
    let provider = load(&state, &id).await?;
    moved_permanently(&canonical(&provider)?)
}

/// GET /providers/{id}/{slug}
pub async fn show(
    shell: Shell,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath((id, _slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let provider = load(&state, &id).await?;
    let canonical = canonical(&provider)?;
    if shell.path != canonical {
        return moved_permanently(&canonical);
    }
    let sent = flash.0 == Some(Flash::ContactSent);
    let page = page(state.locale(), provider, ContactState::default(), sent);
    let mut response = render_page(&state, &shell, "pages/provider.html", &page)?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /contact
pub async fn submit_contact(
    shell: Shell,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ContactForm>,
) -> Result<Response, Error> {
    // Load first: a form for a provider that is gone is a 404, not a 400.
    let provider = state
        .usecases
        .explore
        .get_provider_detail(form.business_id)
        .await?;
    let v = form.values();
    // Sent from a service's page: that service, if it is still this
    // provider's and public; otherwise the visitor lands on the provider.
    let service = match (
        form.return_to == RETURN_TO_SERVICE,
        parse_opt::<i32>(&v.service_id),
    ) {
        (true, Some(id)) => match state.usecases.explore.get_service(id).await {
            Ok(s) if s.business_id == form.business_id => Some(s),
            Ok(_) => None,
            Err(e) if e.code() == ErrorCode::ServiceNotFound => None,
            Err(e) => return Err(e),
        },
        _ => None,
    };
    let result = state
        .usecases
        .explore
        .contact_request(ContactInput {
            business_id: form.business_id,
            topic: v.topic.clone(),
            requester_name: v.requester_name.clone(),
            requester_phone: v.requester_phone.clone(),
            note: Some(v.note.clone()),
            service_id: parse_opt(&v.service_id),
        })
        .await;

    match result {
        Ok(_) => {
            let back = match &service {
                Some(s) => service_page::canonical(s)?,
                None => canonical(&provider)?,
            };
            let mut response = see_other(&back)?;
            Flash::ContactSent.set_on(&mut response);
            Ok(response)
        }
        Err(err) if err.kind() == ErrorKind::BadRequest => {
            let form = FormErrors::from_error(&state, &err);
            let contact = ContactState {
                values: v,
                errors: form.fields,
                message: form.message,
                open: true,
            };
            if let Some(service) = service {
                let status = err.kind().status();
                return service_page::render(
                    &state, &shell, service, provider, contact, false, status,
                );
            }
            let page = page(state.locale(), provider, contact, false);
            render_page_status(
                &state,
                &shell,
                err.kind().status(),
                "pages/provider.html",
                &page,
            )
        }
        Err(err) => Err(err),
    }
}
