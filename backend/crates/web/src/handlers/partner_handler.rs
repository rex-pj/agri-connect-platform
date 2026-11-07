//! The JSON API of partners: the admin's partners, contracts and sponsored
//! providers, and a partner's own report. The same use cases as the pages.

use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::auth::MaybeActor,
    requests::{
        AssignUserRequest, ContractRequest, DeleteWithReasonBody, DemandQuery, PartnerRequest,
        SponsorRequest, ToggleActiveRequest,
    },
};

pub struct PartnerHandler;

impl PartnerHandler {
    /// GET /api/admin/partners
    pub async fn list(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.partner.list_partners(&actor).await?))
    }

    /// GET /api/admin/partners/kpis — paying partners, monthly revenue, by area.
    pub async fn kpis(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.partner.kpis(&actor).await?))
    }

    /// POST /api/admin/partners — 201 with its details.
    pub async fn create(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<PartnerRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let partners = &state.usecases.partner;
        let id = partners.create_partner(&actor, req.into()).await?;
        Ok((
            StatusCode::CREATED,
            Json(partners.get_partner(&actor, id).await?),
        ))
    }

    /// GET /api/admin/partners/{id} — with its contracts.
    pub async fn get(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.partner.get_partner(&actor, id).await?))
    }

    /// PUT /api/admin/partners/{id}
    pub async fn update(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<PartnerRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let partners = &state.usecases.partner;
        partners.update_partner(&actor, id, req.into()).await?;
        Ok(Json(partners.get_partner(&actor, id).await?))
    }

    /// PATCH /api/admin/partners/{id}/status — `{is_active, reason?}`; 204.
    pub async fn set_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .partner
            .set_partner_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/admin/partners/{id}/account — `{partner_id, username}`.
    pub async fn assign_account(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<AssignUserRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let account = state
            .usecases
            .partner
            .assign_partner_account(&actor, id, req.username, req.email, req.password)
            .await?;
        Ok(Json(account))
    }

    /// POST /api/admin/partners/{id}/contracts — 201 with the partner's details.
    pub async fn create_contract(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ContractRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let partners = &state.usecases.partner;
        partners.create_contract(&actor, id, req.into()).await?;
        Ok((
            StatusCode::CREATED,
            Json(partners.get_partner(&actor, id).await?),
        ))
    }

    /// DELETE /api/admin/partners/{id}/contracts/{cid} — `{reason?}`; 204.
    pub async fn delete_contract(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath((id, cid)): ApiPath<(i32, i32)>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .partner
            .delete_contract(&actor, id, cid, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/admin/partners/{id}/contracts/{cid}/sponsored — 204.
    pub async fn add_sponsored(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath((id, cid)): ApiPath<(i32, i32)>,
        ApiJson(req): ApiJson<SponsorRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .partner
            .add_sponsored(&actor, id, cid, req.business_id)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// DELETE /api/admin/partners/{id}/contracts/{cid}/sponsored/{bid} —
    /// `{reason?}`; 204.
    pub async fn remove_sponsored(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath((id, cid, bid)): ApiPath<(i32, i32, i32)>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .partner
            .remove_sponsored(&actor, id, cid, bid, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// PUT /api/admin/partners/{id}/contracts/{cid} — dates, fee, note,
    /// categories (the area stays); the partner's details.
    pub async fn update_contract(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath((id, cid)): ApiPath<(i32, i32)>,
        ApiJson(req): ApiJson<ContractRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let partners = &state.usecases.partner;
        partners
            .update_contract(&actor, id, cid, req.into())
            .await?;
        Ok(Json(partners.get_partner(&actor, id).await?))
    }

    /// GET /api/partner/report — `area?`; the partner account's own report.
    pub async fn own_report(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<DemandQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(
            state.usecases.partner.own_report(&actor, q.focus()).await?,
        ))
    }
}
