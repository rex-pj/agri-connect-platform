//! The short lists the admin keeps, each entry named elsewhere by its code:
//! the sectors ("ngành": trồng trọt, chăn nuôi, thuỷ sản; `category.manage`,
//! with the rest of the catalogue) and the alert kinds (dịch bệnh, xâm nhập
//! mặn, thời tiết; `alert.manage`). An entry's code is made from its name
//! when it is added and never changes after (the visitor's sector cookie,
//! every `sector=` filter and the rows pointing to it carry it). A hidden
//! entry leaves the public choices (the sector switch and filters, the alert
//! form); what is filed under it stays.
//!
//! One use case, configured per list (`CodedList`): the same rules, two
//! tables.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::coded::{CodedEntry, CodedFields},
    repositories::coded_repository_trait::CodedRepositoryTrait,
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::{PERM_ALERT_MANAGE, PERM_CATEGORY_MANAGE},
    constants::{CODED_CODE_MAX, CODED_ICON_MAX, CODED_NAME_MAX},
    inputs::CodedInput,
    outputs::CodedDto,
    ports::EventBus,
    slug::slugify,
    validation::{self, audit_reason, max_chars, max_chars_opt, non_blank},
};

/// What tells one list from the other: who keeps it, how its changes are
/// audited, and its errors.
#[derive(Debug, Clone, Copy)]
pub struct CodedList {
    pub permission: &'static str,
    pub entity: Entity,
    pub not_found: ErrorCode,
    /// A code that is not one of the list's (`chosen`).
    pub invalid: ErrorCode,
}

/// The sectors the platform serves.
pub const SECTOR_LIST: CodedList = CodedList {
    permission: PERM_CATEGORY_MANAGE,
    entity: Entity::Sector,
    not_found: ErrorCode::SectorNotFound,
    invalid: ErrorCode::InvalidSector,
};

/// The kinds of alert.
pub const ALERT_KIND_LIST: CodedList = CodedList {
    permission: PERM_ALERT_MANAGE,
    entity: Entity::AlertKind,
    not_found: ErrorCode::AlertKindNotFound,
    invalid: ErrorCode::InvalidAlertKind,
};

#[derive(Clone)]
pub struct CodedUseCase {
    list: CodedList,
    repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

impl CodedUseCase {
    pub fn new(
        list: CodedList,
        repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self { list, repo, events }
    }

    async fn publish(&self, actor: &Actor, id: i32, change: Change, reason: Option<String>) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: self.list.entity,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    async fn shown_entries(&self) -> Result<Vec<CodedEntry>> {
        Ok(self
            .repo
            .list_all()
            .await?
            .into_iter()
            .filter(|s| s.is_active)
            .collect())
    }

    /// The shown entries, in their order: the sector switch, the public
    /// filters, the alert form and the JSON API.
    pub async fn shown(&self) -> Result<Vec<CodedDto>> {
        Ok(self
            .shown_entries()
            .await?
            .into_iter()
            .map(CodedDto::from)
            .collect())
    }

    /// A visitor's choice (a `sector=` filter, the switch, the cookie):
    /// `None` for blank, the code of a shown entry, else the list's
    /// `invalid` error.
    pub async fn chosen(&self, text: &str) -> Result<Option<String>> {
        validation::coded(text, &self.shown_entries().await?, self.list.invalid)
    }

    /// Every entry, hidden ones too, in their order: what names a row filed
    /// under a hidden one, and what an admin form offers (a row may stay
    /// under a hidden entry). The names are public.
    pub async fn named(&self) -> Result<Vec<CodedDto>> {
        Ok(self
            .repo
            .list_all()
            .await?
            .into_iter()
            .map(CodedDto::from)
            .collect())
    }

    /// Every entry, hidden ones too, for the list's admin page.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CodedDto>> {
        actor.require(self.list.permission)?;
        self.named().await
    }

    pub async fn get(&self, actor: &Actor, id: i32) -> Result<CodedDto> {
        actor.require(self.list.permission)?;
        self.repo
            .find(id)
            .await?
            .map(CodedDto::from)
            .ok_or_else(|| Error::new(self.list.not_found))
    }

    /// Adds one; its code is the slug of its name.
    pub async fn create(&self, actor: &Actor, input: CodedInput) -> Result<i32> {
        actor.require(self.list.permission)?;
        let fields = validate(input)?;
        let id = self
            .repo
            .create(&code_of(&fields.name), &fields, actor.user_id)
            .await?;
        self.publish(actor, id, Change::Created, None).await;
        Ok(id)
    }

    /// Its name, icon and order; the code stays.
    pub async fn update(&self, actor: &Actor, id: i32, input: CodedInput) -> Result<()> {
        actor.require(self.list.permission)?;
        let fields = validate(input)?;
        if !self.repo.update(id, &fields, actor.user_id).await? {
            return Err(Error::new(self.list.not_found));
        }
        self.publish(actor, id, Change::Updated, None).await;
        Ok(())
    }

    /// Hides an entry (it leaves the public choices) or shows it again. The
    /// same state changes nothing.
    pub async fn set_active(
        &self,
        actor: &Actor,
        id: i32,
        active: bool,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(self.list.permission)?;
        let (_, changed) = self
            .repo
            .set_active(id, active, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(self.list.not_found))?;
        if changed {
            self.publish(actor, id, Change::Visibility { visible: active }, reason)
                .await;
        }
        Ok(())
    }
}

/// A new entry's code: the slug of its name, at most `CODED_CODE_MAX`.
fn code_of(name: &str) -> String {
    slugify(name)
        .chars()
        .take(CODED_CODE_MAX)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

/// An entry's boxes: a name with letters (its code comes from them), an
/// optional icon.
fn validate(input: CodedInput) -> Result<CodedFields> {
    let name = input.name.trim().to_owned();
    let icon = non_blank(input.icon);
    Validation::new()
        .check(code_of(&name).is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, CODED_NAME_MAX))
        .rule("icon", max_chars_opt(&icon, CODED_ICON_MAX))
        .finish()?;
    Ok(CodedFields {
        name,
        icon,
        display_order: input.display_order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_the_slug_of_the_name() {
        assert_eq!(code_of("Thuỷ hải sản"), "thuy-hai-san");
        assert_eq!(code_of("Xâm nhập mặn"), "xam-nhap-man");
        assert!(code_of(&"Trồng trọt ".repeat(10)).len() <= CODED_CODE_MAX);
        assert!(!code_of(&"a ".repeat(40)).ends_with('-'));
    }

    #[test]
    fn an_entry_needs_a_name_with_letters() {
        let err = validate(CodedInput {
            name: " -- ".into(),
            icon: None,
            display_order: 0,
        })
        .err();
        let fields: Vec<&str> = err
            .as_ref()
            .map(|e| e.fields().iter().map(|f| f.field).collect())
            .unwrap_or_default();
        assert_eq!(fields, vec!["name"]);
    }
}
