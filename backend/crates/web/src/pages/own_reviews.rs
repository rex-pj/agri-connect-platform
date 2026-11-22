//! /provider/reviews — the reviews farmers wrote about the provider's
//! business, newest first, shown or still waiting for an admin — and
//! POST /provider/reviews/{id}/reply, the public answer to one.
//!
//! Each card's answer is a small form; a blank answer takes it down. A broken
//! rule (too long) re-renders the page with that card's box as typed and its
//! message.

use application::{
    authz::Actor, catalogue::PERM_OWN_REVIEW_REPLY, constants::REVIEW_REPLY_MAX,
    outputs::OwnReviewDto,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::is_refusal,
        forms::FormErrors,
        listing::{parse_opt, past_the_end, with_query, Pagination},
        parse_id, see_other,
    },
    routes,
    shell::Shell,
    views::render_page_status,
};

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("review links: {e}")))
}

#[derive(Serialize)]
pub struct ReviewRow {
    pub review: OwnReviewDto,
    pub action: String,
    /// The answer box: as stored, or as typed after a refusal.
    pub reply: String,
    pub errors: FormErrors,
}

#[derive(Serialize)]
pub struct ReviewsPage {
    pub rows: Vec<ReviewRow>,
    /// Every review of the business, all pages.
    pub total: i64,
    pub waiting: i64,
    pub pagination: Option<Pagination>,
    /// The page a card's answer returns to (empty for the first).
    pub page_param: String,
    pub can_reply: bool,
    pub reply_max: usize,
    pub dashboard_href: String,
    pub flash: Option<Flash>,
}

/// The review whose answer came back refused: its id, the box as typed and
/// the messages.
struct Refused {
    id: i32,
    reply: String,
    errors: FormErrors,
}

/// The page number as a query value: empty for the first page.
fn page_param(page: i64) -> String {
    Some(page)
        .filter(|p| *p > 1)
        .map(|p| p.to_string())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
async fn reviews_page(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    page: i64,
    refused: Option<Refused>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let review = &state.usecases.review;
    let (reviews, (_, waiting)) = tokio::try_join!(
        review.list_own_reviews(actor, page),
        review.count_own_reviews(actor)
    )?;
    let path = url("provider_reviews", &[])?;
    if let Some(redirect) = past_the_end(&path, &[], reviews.page, reviews.total_pages) {
        return redirect;
    }
    let mut refused = refused;
    let mut rows = Vec::with_capacity(reviews.items.len());
    for review in reviews.items {
        let (reply, errors) = match refused.take_if(|r| r.id == review.id) {
            Some(r) => (r.reply, r.errors),
            None => (
                review.reply.clone().unwrap_or_default(),
                FormErrors::default(),
            ),
        };
        rows.push(ReviewRow {
            action: url("provider_review_reply", &[("id", &review.id.to_string())])?,
            review,
            reply,
            errors,
        });
    }
    let page = ReviewsPage {
        rows,
        total: reviews.total,
        waiting,
        pagination: Pagination::new(&path, &[], reviews.page, reviews.total_pages),
        page_param: page_param(reviews.page),
        can_reply: actor.can(PERM_OWN_REVIEW_REPLY),
        reply_max: REVIEW_REPLY_MAX,
        dashboard_href: url("provider_dashboard", &[])?,
        flash,
    };
    render_page_status(state, shell, status, "pages/own_reviews.html", &page)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
}

/// GET /provider/reviews
pub async fn list(
    shell: Shell,
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let page = parse_opt(&q.page).unwrap_or(1);
    let mut response =
        reviews_page(&state, &shell, &actor, page, None, flash.0, StatusCode::OK).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReplyForm {
    pub reply: String,
    /// The list page the card was on, so saving returns to it.
    pub page: String,
}

/// POST /provider/reviews/{id}/reply
pub async fn reply(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ReplyForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ReviewNotFound)?;
    // Rebuilt from a number, never a posted URL.
    let page = parse_opt::<i64>(&form.page).unwrap_or(1).max(1);
    match state
        .usecases
        .review
        .reply_own_review(&actor, id, Some(form.reply.clone()))
        .await
    {
        Ok(()) => {
            let mut response = see_other(&with_query(
                &url("provider_reviews", &[])?,
                &[("page", &page_param(page))],
            ))?;
            Flash::ReviewReplied.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let refused = Refused {
                id,
                reply: form.reply,
                errors: FormErrors::from_error(&state, &err),
            };
            let status = err.kind().status();
            reviews_page(&state, &shell, &actor, page, Some(refused), None, status).await
        }
        Err(err) => Err(err),
    }
}
