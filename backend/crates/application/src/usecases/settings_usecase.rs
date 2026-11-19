//! The settings page (`setting.manage`): every setting with its value,
//! default and bounds; saving a group's values; putting a group back on its
//! defaults. Each change is audited as "key: before → after". Other use
//! cases read the settings through `SettingsReader`, never through here.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::setting::SettingWrite,
    repositories::setting_repository_trait::SettingRepositoryTrait,
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_SETTING_MANAGE,
    outputs::{SettingDto, SettingGroupDto},
    ports::EventBus,
    settings::{
        definition, SettingGroup, SettingKind, SettingState, SettingValue, Settings,
        SettingsReader, TextRule,
    },
    validation::audit_reason,
};

/// The audit log names the settings as one record.
const SETTINGS_ENTITY_ID: i32 = 0;

#[derive(Clone)]
pub struct SettingsUseCase {
    repo: Arc<dyn SettingRepositoryTrait + Send + Sync>,
    reader: SettingsReader,
    events: Arc<dyn EventBus>,
}

impl SettingsUseCase {
    pub fn new(
        repo: Arc<dyn SettingRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            reader: SettingsReader::new(repo.clone()),
            repo,
            events,
        }
    }

    /// The settings as they stand, for anyone (the page frame reads the help
    /// lines and whether sign-ups are open).
    pub async fn current(&self) -> Result<Settings> {
        self.reader.load().await
    }

    /// Every setting, by group in the page's order.
    pub async fn groups(&self, actor: &Actor) -> Result<Vec<SettingGroupDto>> {
        actor.require(PERM_SETTING_MANAGE)?;
        Ok(group_dtos(&self.current().await?))
    }

    /// Stores the `values` sent (key, text), each checked by its kind; a key
    /// left out keeps its value, one set back to its default drops its row.
    /// Every broken value is reported at once, on its key.
    pub async fn update(
        &self,
        actor: &Actor,
        values: &[(String, String)],
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_SETTING_MANAGE)?;
        let current = self.current().await?;
        let mut check = Validation::new();
        let mut parsed = Vec::new();
        for (key, raw) in values {
            let def = definition(key)
                .ok_or_else(|| Error::new(ErrorCode::UnknownSetting).with_param("key", key))?;
            match def.parse(raw) {
                Ok(value) => parsed.push((def, value)),
                Err(e) => {
                    check.rule(def.key, Err(e));
                }
            }
        }
        check.finish()?;
        let writes = parsed
            .into_iter()
            .filter_map(|(def, value)| {
                let before = current.value(def.key)?;
                (before != &value).then(|| (def, before.clone(), value))
            })
            .collect::<Vec<_>>();
        self.store(
            actor,
            writes
                .into_iter()
                .map(|(def, before, after)| {
                    let stored = (after != def.default_value()).then(|| after.as_text());
                    (def.key, before, after, stored)
                })
                .collect(),
            reason,
        )
        .await
    }

    /// Puts every setting of `group` back on its default.
    pub async fn reset(&self, actor: &Actor, group: &str, reason: Option<String>) -> Result<()> {
        actor.require(PERM_SETTING_MANAGE)?;
        let group = SettingGroup::parse(group)
            .ok_or_else(|| Error::new(ErrorCode::UnknownSetting).with_param("key", group))?;
        let current = self.current().await?;
        let changes = current
            .states()
            .iter()
            .filter(|s| s.def.group == group && s.changed_on.is_some())
            .map(|s| (s.def.key, s.value.clone(), s.def.default_value(), None))
            .collect();
        self.store(actor, changes, reason).await
    }

    /// Writes the changes (key, before, after, the text to store or `None`
    /// for the default) and audits them; nothing changed, nothing written.
    async fn store(
        &self,
        actor: &Actor,
        changes: Vec<(&'static str, SettingValue, SettingValue, Option<String>)>,
        reason: Option<String>,
    ) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        let writes: Vec<SettingWrite> = changes
            .iter()
            .map(|(key, _, _, stored)| SettingWrite {
                key: (*key).to_owned(),
                value: stored.clone(),
            })
            .collect();
        self.repo.save(&writes, actor.user_id).await?;
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Setting,
                entity_id: SETTINGS_ENTITY_ID,
                change: Change::SettingsChanged {
                    changes: changes
                        .iter()
                        .map(|(key, before, after, _)| {
                            format!("{key}: {} → {}", shown(before), shown(after))
                        })
                        .collect(),
                },
                reason: audit_reason(reason),
            })
            .await;
        Ok(())
    }
}

/// A value as the audit log writes it: empty text as "-".
fn shown(value: &SettingValue) -> String {
    match value.as_text() {
        t if t.is_empty() => "-".to_owned(),
        t => t,
    }
}

fn setting_dto(state: &SettingState) -> SettingDto {
    let (kind, min, max) = match state.def.kind {
        SettingKind::Number { min, max, .. } => ("number", Some(min), Some(max)),
        SettingKind::Switch { .. } => ("switch", None, None),
        SettingKind::Text(TextRule::Phone) => ("phone", None, None),
        SettingKind::Text(TextRule::HttpsUrl) => ("url", None, None),
        SettingKind::Text(TextRule::Email) => ("email", None, None),
    };
    SettingDto {
        key: state.def.key,
        kind,
        value: state.value.as_text(),
        default: state.def.default_value().as_text(),
        min,
        max,
        changed_on: state.changed_on,
    }
}

fn group_dtos(settings: &Settings) -> Vec<SettingGroupDto> {
    SettingGroup::ALL
        .into_iter()
        .map(|group| {
            let settings: Vec<SettingDto> = settings
                .states()
                .iter()
                .filter(|s| s.def.group == group)
                .map(setting_dto)
                .collect();
            SettingGroupDto {
                code: group.code(),
                changed: settings.iter().filter(|s| s.changed_on.is_some()).count(),
                settings,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{SETTINGS, STEADY_WEEKLY_LEADS, SUPPORT_PHONE};

    #[test]
    fn every_setting_is_in_one_group_of_the_page() {
        let groups = group_dtos(&Settings::default());
        let listed: usize = groups.iter().map(|g| g.settings.len()).sum();
        assert_eq!(listed, SETTINGS.len());
        assert!(groups.iter().all(|g| !g.settings.is_empty()));
        assert!(groups.iter().all(|g| g.changed == 0));
    }

    #[test]
    fn the_page_shows_defaults_and_bounds() {
        let groups = group_dtos(&Settings::default());
        let find = |key: &str| {
            groups
                .iter()
                .flat_map(|g| g.settings.iter())
                .find(|s| s.key == key)
                .cloned()
        };
        let steady = find(STEADY_WEEKLY_LEADS);
        assert_eq!(steady.as_ref().map(|s| s.kind), Some("number"));
        assert_eq!(steady.as_ref().and_then(|s| s.min), Some(1));
        let phone = find(SUPPORT_PHONE);
        assert_eq!(
            phone.map(|s| (s.kind, s.value)),
            Some(("phone", String::new()))
        );
        assert_eq!(shown(&SettingValue::Text(String::new())), "-");
    }
}
