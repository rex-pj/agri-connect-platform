//! Information pages: the terms, the privacy and personal data policies,
//! the rules for providers and listings, the complaints and takedown
//! procedures, the legal entity, the contact page (`content.page.manage`).
//!
//! A page is on, in preview (only who holds `feature.preview` sees it, as a
//! feature) or off, and is listed in the footer or not. Its text is kept in
//! versions written in Markdown, never edited: the one in force is the
//! latest whose day has come, so a policy can be written ahead of the day it
//! applies, and the former ones stay readable. A page with no version in
//! force shows nowhere. Nothing is deleted: a page is turned off.

use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use domain::{
    events::{Change, DomainEvent, Entity},
    models::info_page::{InfoPage, InfoPageFields, InfoRevision, NewInfoRevision},
    repositories::info_page_repository_trait::InfoPageRepositoryTrait,
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_CONTENT_PAGE_MANAGE,
    constants::{INFO_PAGE_BODY_MAX, INFO_PAGE_CODE_MAX, INFO_PAGE_NOTE_MAX, INFO_PAGE_TITLE_MAX},
    features::Stage,
    info_pages::{FOOTER_GROUPS, FOOTER_GROUP_POLICY},
    inputs::{InfoPageInput, InfoRevisionInput},
    outputs::{
        InfoLinkDto, InfoPageDetailDto, InfoPageDto, InfoPageViewDto, InfoRevisionDto,
        InfoVersionDto,
    },
    ports::EventBus,
    slug::{numbered, slugify},
    usecases::demand_usecase::vn_today,
    validation::{self, audit_reason, max_chars, one_line, validate_slug},
};

/// Where a version stands on a day (`InfoRevisionDto.state`).
pub const REVISION_IN_FORCE: &str = "in_force";
pub const REVISION_UPCOMING: &str = "upcoming";
pub const REVISION_PAST: &str = "past";

#[derive(Clone)]
pub struct InfoPageUseCase {
    repo: Arc<dyn InfoPageRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

impl InfoPageUseCase {
    pub fn new(
        repo: Arc<dyn InfoPageRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self { repo, events }
    }

    async fn publish(&self, actor: &Actor, id: i32, change: Change, reason: Option<String>) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::InfoPage,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// Every page, off ones too, by footer group and order.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<InfoPageDto>> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let today = vn_today(Utc::now())?;
        let (pages, with_text) =
            tokio::try_join!(self.repo.list_all(), self.repo.with_text(today))?;
        Ok(pages
            .into_iter()
            .map(|page| {
                let has_text = with_text.contains(&page.id);
                page_dto(page, has_text)
            })
            .collect())
    }

    /// A page with every version, for its admin page.
    pub async fn get(&self, actor: &Actor, id: i32) -> Result<InfoPageDetailDto> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let today = vn_today(Utc::now())?;
        let page = self
            .repo
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::InfoPageNotFound))?;
        let revisions = revision_dtos(self.repo.revisions(id).await?, today);
        let has_text = revisions.iter().any(|r| r.state == REVISION_IN_FORCE);
        Ok(InfoPageDetailDto {
            page: page_dto(page, has_text),
            revisions,
        })
    }

    /// A new page, off and with no text yet: its id.
    pub async fn create(&self, actor: &Actor, input: InfoPageInput) -> Result<i32> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let fields = validate_page(input, true)?;
        let id = self.repo.create(&fields, actor.user_id).await?;
        self.publish(actor, id, Change::Created, None).await;
        Ok(id)
    }

    /// Its title, icon and place in the footer; its code stays.
    pub async fn update(&self, actor: &Actor, id: i32, input: InfoPageInput) -> Result<()> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let current = self
            .repo
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::InfoPageNotFound))?;
        let fields = validate_page(
            InfoPageInput {
                code: current.code,
                ..input
            },
            false,
        )?;
        if !self.repo.update(id, &fields, actor.user_id).await? {
            return Err(Error::new(ErrorCode::InfoPageNotFound));
        }
        self.publish(actor, id, Change::Updated, None).await;
        Ok(())
    }

    /// Who sees it: `on`, `preview` or `off` (a feature's stages). The same
    /// stage again changes nothing.
    pub async fn set_stage(
        &self,
        actor: &Actor,
        id: i32,
        stage: &str,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let stage = Stage::parse(stage.trim())
            .filter(|s| s.code() == stage.trim())
            .ok_or_else(|| Error::field("stage", ErrorCode::InvalidInfoPageStage))?;
        let changed = self
            .repo
            .set_stage(id, stage.code(), actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::InfoPageNotFound))?;
        if changed {
            let status = stage.code().to_owned();
            self.publish(actor, id, Change::StatusChanged { status }, reason)
                .await;
        }
        Ok(())
    }

    /// A new version of its text, in force from its day (today when none
    /// is given, never a day before today).
    pub async fn add_revision(
        &self,
        actor: &Actor,
        id: i32,
        input: InfoRevisionInput,
    ) -> Result<()> {
        actor.require(PERM_CONTENT_PAGE_MANAGE)?;
        let today = vn_today(Utc::now())?;
        let revision = validate_revision(input, today)?;
        if self.repo.find(id).await?.is_none() {
            return Err(Error::new(ErrorCode::InfoPageNotFound));
        }
        self.repo.add_revision(id, &revision, actor.user_id).await?;
        let effective_on = revision.effective_on.format("%Y-%m-%d").to_string();
        self.publish(
            actor,
            id,
            Change::Revised { effective_on },
            revision.change_note,
        )
        .await;
        Ok(())
    }

    /// The pages a visitor sees (`may_preview`: holds `feature.preview`),
    /// each with a version in force: the footer's and the hub's. Public.
    pub async fn shown(&self, may_preview: bool) -> Result<Vec<InfoLinkDto>> {
        let today = vn_today(Utc::now())?;
        let stages = visible_stages(may_preview);
        let (pages, with_text) =
            tokio::try_join!(self.repo.with_stage(&stages), self.repo.with_text(today))?;
        Ok(pages
            .into_iter()
            .filter(|page| with_text.contains(&page.id))
            .map(|page| InfoLinkDto {
                preview: page.stage == Stage::Preview.code(),
                code: page.code,
                title: page.title,
                icon: page.icon,
                footer_group: page.footer_group,
                in_footer: page.in_footer,
            })
            .collect())
    }

    /// The page at `code` for a visitor: the version in force, or `version`
    /// when it is one already in force at some point. A page off, in
    /// preview for one who may not preview, with no text yet, or a version
    /// not public is `info_page_not_found`. Public.
    pub async fn page(
        &self,
        code: &str,
        may_preview: bool,
        version: Option<i32>,
    ) -> Result<InfoPageViewDto> {
        let not_found = || Error::new(ErrorCode::InfoPageNotFound);
        let page = self
            .repo
            .find_by_code(code)
            .await?
            .filter(|p| stage_of(p).shown_to(may_preview))
            .ok_or_else(not_found)?;
        let today = vn_today(Utc::now())?;
        // Every version in force at some point, the latest first.
        let public: Vec<InfoRevision> = self
            .repo
            .revisions(page.id)
            .await?
            .into_iter()
            .filter(|r| r.effective_on <= today)
            .collect();
        let shown_at = match version {
            None => 0,
            Some(id) => public
                .iter()
                .position(|r| r.id == id)
                .ok_or_else(not_found)?,
        };
        let shown = public.get(shown_at).ok_or_else(not_found)?;
        // An earlier version stopped applying when the next one came.
        let superseded_on = shown_at
            .checked_sub(1)
            .and_then(|i| public.get(i))
            .map(|next| next.effective_on);
        Ok(InfoPageViewDto {
            preview: page.stage == Stage::Preview.code(),
            id: page.id,
            code: page.code,
            title: page.title,
            icon: page.icon,
            body: shown.body.clone(),
            effective_on: shown.effective_on,
            change_note: shown.change_note.clone(),
            superseded_on,
            versions: public
                .iter()
                .map(|r| InfoVersionDto {
                    id: r.id,
                    effective_on: r.effective_on,
                    change_note: r.change_note.clone(),
                    current: r.id == shown.id,
                })
                .collect(),
        })
    }
}

/// The stages a visitor sees.
fn visible_stages(may_preview: bool) -> Vec<&'static str> {
    Stage::ALL
        .into_iter()
        .filter(|s| s.shown_to(may_preview))
        .map(Stage::code)
        .collect()
}

/// A page's stage as stored; a value the schema refuses never comes back,
/// and anything unreadable is off.
fn stage_of(page: &InfoPage) -> Stage {
    Stage::parse(&page.stage)
        .filter(|s| s.code() == page.stage)
        .unwrap_or(Stage::Off)
}

fn page_dto(page: InfoPage, has_text: bool) -> InfoPageDto {
    InfoPageDto {
        stage: stage_of(&page).code(),
        id: page.id,
        code: page.code,
        title: page.title,
        icon: page.icon,
        in_footer: page.in_footer,
        footer_group: page.footer_group,
        display_order: page.display_order,
        is_demo: page.is_demo,
        has_text,
    }
}

/// Each version's state on `today`; `revisions` come the latest to come
/// into force first, so the first come is in force.
fn revision_dtos(revisions: Vec<InfoRevision>, today: NaiveDate) -> Vec<InfoRevisionDto> {
    let mut in_force_seen = false;
    revisions
        .into_iter()
        .map(|r| {
            let state = if r.effective_on > today {
                REVISION_UPCOMING
            } else if in_force_seen {
                REVISION_PAST
            } else {
                in_force_seen = true;
                REVISION_IN_FORCE
            };
            InfoRevisionDto {
                id: r.id,
                body: r.body,
                effective_on: r.effective_on,
                change_note: r.change_note,
                created_on: r.created_on,
                state,
            }
        })
        .collect()
}

/// The code of a new page: as typed, else made from its title.
fn page_code(typed: &str, title: &str) -> String {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        numbered(&slugify(title), 1, INFO_PAGE_CODE_MAX)
    } else {
        typed
    }
}

/// Trims every box and reports every bad one at once; `new`: the code is
/// checked (it is only stored when the page is made).
fn validate_page(input: InfoPageInput, new: bool) -> Result<InfoPageFields> {
    let title = one_line(&input.title);
    let code = if new {
        page_code(&input.code, &title)
    } else {
        input.code
    };
    let icon = validation::icon(Some(&input.icon));
    let footer_group = match input.footer_group.trim() {
        "" => FOOTER_GROUP_POLICY.to_owned(),
        group => group.to_owned(),
    };
    let mut check = Validation::new();
    check
        .check(title.is_empty(), "title", ErrorCode::InfoPageTitleRequired)
        .rule("title", max_chars(&title, INFO_PAGE_TITLE_MAX))
        .check(
            !FOOTER_GROUPS.contains(&footer_group.as_str()),
            "footer_group",
            ErrorCode::InvalidFooterGroup,
        )
        .rule("icon", icon.clone().map(|_| ()));
    if new && !title.is_empty() {
        check
            .rule("code", validate_slug(&code))
            .rule("code", max_chars(&code, INFO_PAGE_CODE_MAX));
    }
    check.finish()?;
    Ok(InfoPageFields {
        code,
        title,
        icon: icon.ok().flatten(),
        in_footer: input.in_footer,
        footer_group,
        display_order: input.display_order.unwrap_or(0),
    })
}

/// A new version's boxes: a text, a day not before `today`, a short note.
fn validate_revision(input: InfoRevisionInput, today: NaiveDate) -> Result<NewInfoRevision> {
    let body = input.body.trim().to_owned();
    let note = one_line(&input.change_note);
    let effective_on = input.effective_on.unwrap_or(today);
    Validation::new()
        .check(body.is_empty(), "body", ErrorCode::InfoPageBodyRequired)
        .rule("body", max_chars(&body, INFO_PAGE_BODY_MAX))
        .check(
            effective_on < today,
            "effective_on",
            ErrorCode::InvalidEffectiveDate,
        )
        .rule("change_note", max_chars(&note, INFO_PAGE_NOTE_MAX))
        .finish()?;
    Ok(NewInfoRevision {
        body,
        effective_on,
        change_note: (!note.is_empty()).then_some(note),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap_or_default()
    }

    fn revision(id: i32, on: u32) -> InfoRevision {
        InfoRevision {
            id,
            page_id: 1,
            body: "text".into(),
            effective_on: day(on),
            change_note: None,
            created_on: Utc::now(),
        }
    }

    #[test]
    fn the_latest_version_come_is_in_force() {
        let states: Vec<&str> = revision_dtos(
            vec![
                revision(4, 30),
                revision(3, 20),
                revision(2, 20),
                revision(1, 1),
            ],
            day(25),
        )
        .iter()
        .map(|r| r.state)
        .collect();
        assert_eq!(
            states,
            [
                REVISION_UPCOMING,
                REVISION_IN_FORCE,
                REVISION_PAST,
                REVISION_PAST
            ]
        );
    }

    #[test]
    fn a_new_page_takes_its_code_from_its_title_and_checks_every_box() {
        let fields = validate_page(
            InfoPageInput {
                title: "  Terms   of use ".into(),
                ..InfoPageInput::default()
            },
            true,
        );
        let fields = fields.ok();
        assert_eq!(
            fields.as_ref().map(|f| f.code.as_str()),
            Some("terms-of-use")
        );
        assert_eq!(
            fields.as_ref().map(|f| f.title.as_str()),
            Some("Terms of use")
        );
        assert_eq!(
            fields.as_ref().map(|f| f.footer_group.as_str()),
            Some(FOOTER_GROUP_POLICY)
        );
        let refused = validate_page(
            InfoPageInput {
                code: "Bad Code!".into(),
                title: String::new(),
                footer_group: "side".into(),
                icon: "no-such-icon-anywhere".into(),
                ..InfoPageInput::default()
            },
            true,
        );
        let fields: Vec<&str> = refused
            .err()
            .map(|e| e.fields().iter().map(|f| f.field).collect())
            .unwrap_or_default();
        assert_eq!(fields, ["title", "footer_group", "icon"]);
        let bad_code = validate_page(
            InfoPageInput {
                code: "Bad Code!".into(),
                title: "Terms".into(),
                ..InfoPageInput::default()
            },
            true,
        );
        assert!(bad_code
            .err()
            .is_some_and(|e| e.fields().iter().any(|f| f.field == "code")));
    }

    #[test]
    fn a_version_never_applies_before_today() {
        let today = day(10);
        let input = |on: Option<NaiveDate>| InfoRevisionInput {
            body: " text ".into(),
            effective_on: on,
            change_note: "  typo\nfixed ".into(),
        };
        let ok = validate_revision(input(None), today).ok();
        assert_eq!(ok.as_ref().map(|r| r.effective_on), Some(today));
        assert_eq!(ok.as_ref().map(|r| r.body.as_str()), Some("text"));
        assert_eq!(
            ok.as_ref().and_then(|r| r.change_note.as_deref()),
            Some("typo fixed")
        );
        assert!(validate_revision(input(Some(day(11))), today).is_ok());
        assert!(validate_revision(input(Some(day(9))), today).is_err());
        assert!(validate_revision(
            InfoRevisionInput {
                body: "  ".into(),
                ..input(None)
            },
            today
        )
        .is_err());
    }
}
