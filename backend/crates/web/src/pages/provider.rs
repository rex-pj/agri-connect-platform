//! GET /providers/{id}, /providers/{id}/{slug} — provider detail — and
//! POST /contact, the contact form on it.
//!
//! The form is a plain HTML form (Post/Redirect/Get): success answers 303 to
//! the provider page with a one-time flash; a rule broken re-renders the page
//! with the typed values and one message per field. The rules themselves are
//! `ExploreUseCase::contact_request`, the same ones the JSON API uses.

use std::collections::BTreeMap;

use application::{
    constants::{
        NOTE_MAX_LEN, RATING_MAX, RATING_MIN, REQUESTER_NAME_MAX_LEN, REVIEW_COMMENT_MAX,
        TOPIC_MAX_LEN,
    },
    features::Feature,
    inputs::{ContactInput, ReviewInput},
    outputs::{BusinessDetailDto, ProductDto, ProviderReviewsDto, PublicServiceDto},
};
use axum::{extract::State, response::Response};
use domain::{
    models::product::{ProductFilter, ProductSort},
    Error, ErrorCode, ErrorKind,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::META_DESCRIPTION_MAX_CHARS,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::is_refusal,
        canonical_url,
        forms::FormErrors,
        listing::parse_opt,
        manage::{manage_link, Managed},
        moved_permanently, parse_id, product as product_page, see_other, service as service_page,
        summary,
    },
    shell::{Link, Shell},
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
    /// `service` or `product` when the form was on that record's page: the
    /// visitor lands back there. The page is rebuilt from `service_id` or
    /// `product_id`, never a posted URL.
    #[serde(default)]
    pub return_to: String,
    /// The product whose page the form was on (`return_to=product`).
    #[serde(default)]
    pub product_id: String,
}

/// `return_to` of the form on a service's own page.
pub const RETURN_TO_SERVICE: &str = "service";
/// `return_to` of the form on a product's own page.
pub const RETURN_TO_PRODUCT: &str = "product";

/// The page the form was sent from, when it was not the provider's own.
enum Origin {
    Service(PublicServiceDto),
    Product(ProductDto),
}

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
    pub phone_pattern: &'static str,
}

pub(super) const LIMITS: ContactLimits = ContactLimits {
    phone_pattern: application::validation::VN_PHONE_PATTERN,
    name_max: REQUESTER_NAME_MAX_LEN,
    topic_max: TOPIC_MAX_LEN,
    note_max: NOTE_MAX_LEN,
};

/// POST /reviews body: a farmer's review, from the phone their contact
/// request came from.
#[derive(Debug, Deserialize)]
pub struct ReviewForm {
    pub business_id: i32,
    #[serde(default)]
    pub reviewer_name: String,
    #[serde(default)]
    pub reviewer_phone: String,
    /// The chosen star, "1" to "5"; anything else is no rating.
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub comment: String,
}

/// What the visitor typed in the review form, echoed back when re-shown.
#[derive(Debug, Default, Serialize)]
pub struct ReviewValues {
    pub reviewer_name: String,
    pub reviewer_phone: String,
    pub rating: String,
    pub comment: String,
}

/// The review form: as typed and its messages after a refusal (the sheet
/// opens by itself), empty otherwise.
#[derive(Debug, Default, Serialize)]
pub struct ReviewState {
    pub values: ReviewValues,
    pub errors: FormErrors,
    pub open: bool,
}

#[derive(Serialize)]
pub struct ProviderPage {
    pub provider: BusinessDetailDto,
    /// What the provider sells (the first page, newest first).
    pub products: Vec<ProductDto>,
    pub meta_description: Option<String>,
    pub contact: ContactState,
    pub limits: ContactLimits,
    /// Its shown reviews (count, average, the newest) and the review form.
    pub reviews: ProviderReviewsDto,
    pub review: ReviewState,
    pub review_comment_max: usize,
    /// The id of the reviews section, which other pages link to.
    pub reviews_anchor: &'static str,
    /// The stars a rating can have, for the form's radio buttons.
    pub stars: Vec<i16>,
    /// A review was just sent (flash from the redirect): it waits for an admin.
    pub review_sent: bool,
    /// Where the provider works, or the region when the address is unknown.
    pub area: String,
    /// The text a share sheet (Zalo, Messenger) puts with the link.
    pub share_text: String,
    /// The form was just sent (flash from the redirect).
    pub sent: bool,
    /// Where the owner or an admin edits it (`pages::manage`).
    pub manage: Option<Link>,
}

/// Who is looking and what just happened, for the detail pages (provider,
/// service, product): the contact form was just sent, and the edit link for
/// the owner or an admin. A refused form re-shown is `default()`: the
/// farmer who sent it has nothing to edit.
#[derive(Debug, Default)]
pub(super) struct Visit {
    pub sent: bool,
    pub manage: Option<Link>,
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

/// A provider's public products, newest first (one page of them).
async fn products_of(state: &AppState, business_id: i32) -> Result<Vec<ProductDto>, Error> {
    let filter = ProductFilter {
        business_id: Some(business_id),
        ..ProductFilter::default()
    };
    Ok(state
        .usecases
        .product
        .browse_products(filter, ProductSort::default(), 1)
        .await?
        .items)
}

/// What the page shows besides the provider: its products and reviews, the
/// two forms' states, and which one was just sent.
struct Around {
    products: Vec<ProductDto>,
    reviews: ProviderReviewsDto,
    contact: ContactState,
    review: ReviewState,
    flash: Option<Flash>,
    manage: Option<Link>,
}

/// The products and shown reviews of a provider's page.
async fn around(
    state: &AppState,
    business_id: i32,
) -> Result<(Vec<ProductDto>, ProviderReviewsDto), Error> {
    tokio::try_join!(
        products_of(state, business_id),
        state.usecases.review.list_public(business_id),
    )
}

fn page(locale: &Locale, provider: BusinessDetailDto, a: Around) -> ProviderPage {
    let area = area_of(locale, provider.address.as_deref());
    let Around {
        products,
        reviews,
        contact,
        review,
        flash,
        manage,
    } = a;
    ProviderPage {
        manage,
        products,
        reviews,
        review,
        review_comment_max: REVIEW_COMMENT_MAX,
        reviews_anchor: REVIEWS_ANCHOR,
        stars: (RATING_MIN..=RATING_MAX).rev().collect(),
        review_sent: flash == Some(Flash::ReviewSent),
        sent: flash == Some(Flash::ContactSent),
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
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath((id, _slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let provider = load(&state, &id).await?;
    let canonical = canonical(&provider)?;
    if shell.path != canonical {
        return moved_permanently(&canonical);
    }
    let (products, reviews) = around(&state, provider.id).await?;
    let a = Around {
        products,
        reviews,
        contact: ContactState::default(),
        review: ReviewState::default(),
        flash: flash.0,
        manage: manage_link(&state, viewer.as_ref(), provider.id, Managed::Provider).await?,
    };
    let page = page(state.locale(), provider, a);
    let mut response = render_page(&state, &shell, "pages/provider.html", &page)?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// Sent from a service's or a product's page: that record, if it is still
/// this provider's and public; otherwise (`None`) the visitor lands on the
/// provider.
async fn origin_of(state: &AppState, form: &ContactForm) -> Result<Option<Origin>, Error> {
    let found = match form.return_to.as_str() {
        RETURN_TO_SERVICE => match parse_opt::<i32>(&form.service_id) {
            Some(id) => state
                .usecases
                .explore
                .get_service(id)
                .await
                .map(|s| (s.business_id == form.business_id).then_some(Origin::Service(s))),
            None => Ok(None),
        },
        RETURN_TO_PRODUCT => match parse_opt::<i32>(&form.product_id) {
            Some(id) => state
                .usecases
                .product
                .get_product_by_id(id)
                .await
                .map(|p| (p.business_id == form.business_id).then_some(Origin::Product(p))),
            None => Ok(None),
        },
        _ => Ok(None),
    };
    match found {
        Err(e)
            if matches!(
                e.code(),
                ErrorCode::ServiceNotFound | ErrorCode::ProductNotFound
            ) =>
        {
            Ok(None)
        }
        other => other,
    }
}

/// POST /contact
pub async fn submit_contact(
    shell: Shell,
    ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
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
    let origin = origin_of(&state, &form).await?;
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
            product_id: parse_opt(&form.product_id),
            client: crate::middleware::client_ip::client_key(ip.as_deref()),
        })
        .await;

    match result {
        Ok(_) => {
            let back = match &origin {
                Some(Origin::Service(s)) => service_page::canonical(s)?,
                Some(Origin::Product(p)) => product_page::canonical(p)?,
                None => canonical(&provider)?,
            };
            let mut response = see_other(&back)?;
            Flash::ContactSent.set_on(&mut response);
            Ok(response)
        }
        // Too many sent today: said in the form, which offers the call.
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::BadRequest | ErrorKind::TooManyRequests
            ) =>
        {
            let form = FormErrors::from_error(&state, &err);
            let contact = ContactState {
                values: v,
                errors: form.fields,
                message: form.message,
                open: true,
            };
            let status = err.kind().status();
            match origin {
                Some(Origin::Service(service)) => {
                    return service_page::render(
                        &state,
                        &shell,
                        service,
                        provider,
                        contact,
                        Visit::default(),
                        status,
                    )
                    .await;
                }
                Some(Origin::Product(product)) => {
                    return product_page::render(
                        &state,
                        &shell,
                        product,
                        provider,
                        contact,
                        Visit::default(),
                        status,
                    )
                    .await;
                }
                None => {}
            }
            let (products, reviews) = around(&state, provider.id).await?;
            let a = Around {
                products,
                reviews,
                contact,
                review: ReviewState::default(),
                flash: None,
                manage: None,
            };
            let page = page(state.locale(), provider, a);
            render_page_status(&state, &shell, status, "pages/provider.html", &page)
        }
        Err(err) => Err(err),
    }
}

/// POST /reviews: Post/Redirect/Get like the contact form. Sent, the
/// visitor lands back on the provider's page with a one-time flash (the
/// review waits for an admin); refused, the page comes back with the review
/// sheet open, the values as typed and one message per field.
pub async fn submit_review(
    shell: Shell,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ReviewForm>,
) -> Result<Response, Error> {
    // Load first: a review of a provider that is gone is a 404, not a 400.
    let provider = state
        .usecases
        .explore
        .get_provider_detail(form.business_id)
        .await?;
    let result = state
        .usecases
        .review
        .submit(ReviewInput {
            business_id: form.business_id,
            reviewer_name: form.reviewer_name.clone(),
            reviewer_phone: form.reviewer_phone.clone(),
            rating: parse_opt(&form.rating),
            comment: Some(form.comment.clone()),
        })
        .await;
    match result {
        Ok(_) => {
            let mut response = see_other(&canonical(&provider)?)?;
            Flash::ReviewSent.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let status = err.kind().status();
            let review = ReviewState {
                errors: FormErrors::from_error(&state, &err),
                values: ReviewValues {
                    reviewer_name: form.reviewer_name,
                    reviewer_phone: form.reviewer_phone,
                    rating: form.rating,
                    comment: form.comment,
                },
                open: true,
            };
            let (products, reviews) = around(&state, provider.id).await?;
            let a = Around {
                products,
                reviews,
                contact: ContactState::default(),
                review,
                flash: None,
                manage: None,
            };
            let page = page(state.locale(), provider, a);
            render_page_status(&state, &shell, status, "pages/provider.html", &page)
        }
        Err(err) => Err(err),
    }
}

/// A provider's rating on the pages that name it (a service's, a
/// product's), and where its reviews are, to read them or write one.
#[derive(Serialize)]
pub struct RatingLine {
    pub rating: Option<application::outputs::RatingDto>,
    pub href: String,
}

/// While reviews are hidden from this visitor, no rating and no way to
/// them (`href` empty).
pub(super) async fn rating_line(
    state: &AppState,
    shell: &Shell,
    provider: &BusinessDetailDto,
) -> Result<RatingLine, Error> {
    if !shell.shows(Feature::Reviews) {
        return Ok(RatingLine {
            rating: None,
            href: String::new(),
        });
    }
    let rating = state
        .usecases
        .review
        .ratings(&[provider.id])
        .await?
        .remove(&provider.id);
    Ok(RatingLine {
        rating,
        href: format!("{}#{REVIEWS_ANCHOR}", canonical(provider)?),
    })
}

/// The id of the reviews section on a provider's page.
pub(super) const REVIEWS_ANCHOR: &str = "danh-gia-list";
