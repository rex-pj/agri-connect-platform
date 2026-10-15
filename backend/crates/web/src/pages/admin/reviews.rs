//! /admin/reviews: farmers' reviews, newest first, with hide / show.
//!
//! Hiding asks for confirmation and a reason: the row's link reloads the
//! list with `?hide={id}`, which renders that confirmation open (a dialog
//! with JS, an anchored form without). Showing again is one button.

use application::outputs::{AdminPagedResponse, AdminReviewDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, ListPosition},
        forms::reason,
        listing::{parse_opt, Pagination},
        parse_id, see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    /// The review whose hide confirmation is open.
    pub hide: String,
}

#[derive(Serialize)]
pub struct ReviewRow {
    pub review: AdminReviewDto,
    pub provider_href: String,
    pub hide_href: String,
    pub action: String,
}

/// The hide confirmation open on this render.
#[derive(Serialize)]
pub struct Confirm {
    pub reviewer: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct ReviewsPage {
    pub total: i64,
    pub rows: Vec<ReviewRow>,
    pub pagination: Option<Pagination>,
    pub position: ListPosition,
    pub confirm: Option<Confirm>,
    pub flash: Option<Flash>,
}

/// GET /admin/reviews
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_reviews(&actor, parse_opt(&q.page).unwrap_or(1))
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..ListPosition::default()
    };
    let hide = parse_opt::<i32>(&q.hide);
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    for review in items {
        let id = review.id.to_string();
        let action = url("admin_review_visibility", &[("id", &id)])?;
        if hide == Some(review.id) && review.is_visible {
            confirm = Some(Confirm {
                reviewer: review.reviewer_name.clone(),
                action: action.clone(),
            });
        }
        rows.push(ReviewRow {
            provider_href: url("admin_provider", &[("id", &review.business_id.to_string())])?,
            hide_href: position.href("admin_reviews", &[("hide", &id)])?,
            action,
            review,
        });
    }
    let path = url("admin_reviews", &[])?;
    let body = ReviewsPage {
        total,
        rows,
        pagination: Pagination::new(&path, &[], page, total_pages),
        position,
        confirm,
        flash: flash.0,
    };
    let mut response = render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/reviews.html",
        &body,
    )?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VisibilityForm {
    /// "true" to show, "false" to hide.
    pub is_visible: String,
    pub reason: String,
    pub page: String,
}

/// POST /admin/reviews/{id}/visibility
pub async fn change_visibility(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<VisibilityForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ReviewNotFound)?;
    let show = form.is_visible == "true";
    state
        .usecases
        .admin
        .toggle_review_visible(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::ReviewShown
    } else {
        Flash::ReviewHidden
    };
    ListPosition {
        page: form.page,
        ..ListPosition::default()
    }
    .back_to("admin_reviews", flash)
}
