//! The JSON API of the short lists kept by code: the sectors (the public
//! list every `sector=` filter takes, and the admin's) and the alert kinds.
//! The same use cases as the pages; one macro writes both handlers.

use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::MaybeActor,
    requests::{CodedRequest, ToggleActiveRequest},
};

macro_rules! coded_handler {
    ($handler:ident, $usecase:ident) => {
        pub struct $handler;

        impl $handler {
            /// GET: the shown ones, in their order. Public.
            pub async fn shown(State(state): State<AppState>) -> Result<impl IntoResponse, Error> {
                Ok(Json(state.usecases.$usecase.shown().await?))
            }

            /// GET (admin): every one, hidden ones too.
            pub async fn list(
                MaybeActor(actor): MaybeActor,
                State(state): State<AppState>,
            ) -> Result<impl IntoResponse, Error> {
                let actor = require_actor(actor)?;
                Ok(Json(state.usecases.$usecase.list(&actor).await?))
            }

            /// POST (admin): 201 with the entry (its code made from the name).
            pub async fn create(
                MaybeActor(actor): MaybeActor,
                State(state): State<AppState>,
                ApiJson(req): ApiJson<CodedRequest>,
            ) -> Result<impl IntoResponse, Error> {
                let actor = require_actor(actor)?;
                let list = &state.usecases.$usecase;
                let id = list.create(&actor, req.into()).await?;
                Ok((StatusCode::CREATED, Json(list.get(&actor, id).await?)))
            }

            /// GET (admin) one entry.
            pub async fn get(
                MaybeActor(actor): MaybeActor,
                State(state): State<AppState>,
                ApiPath(id): ApiPath<i32>,
            ) -> Result<impl IntoResponse, Error> {
                let actor = require_actor(actor)?;
                Ok(Json(state.usecases.$usecase.get(&actor, id).await?))
            }

            /// PUT (admin): its name, icon and order; the code stays.
            pub async fn update(
                MaybeActor(actor): MaybeActor,
                State(state): State<AppState>,
                ApiPath(id): ApiPath<i32>,
                ApiJson(req): ApiJson<CodedRequest>,
            ) -> Result<impl IntoResponse, Error> {
                let actor = require_actor(actor)?;
                let list = &state.usecases.$usecase;
                list.update(&actor, id, req.into()).await?;
                Ok(Json(list.get(&actor, id).await?))
            }

            /// PATCH (admin) `{is_active, reason?}`; 204.
            pub async fn set_status(
                MaybeActor(actor): MaybeActor,
                State(state): State<AppState>,
                ApiPath(id): ApiPath<i32>,
                ApiJson(req): ApiJson<ToggleActiveRequest>,
            ) -> Result<impl IntoResponse, Error> {
                let actor = require_actor(actor)?;
                state
                    .usecases
                    .$usecase
                    .set_active(&actor, id, req.is_active, req.reason)
                    .await?;
                Ok(StatusCode::NO_CONTENT)
            }
        }
    };
}

coded_handler!(SectorHandler, sector);
coded_handler!(AlertKindHandler, alert_kind);
