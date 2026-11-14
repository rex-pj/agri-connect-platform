//! Alerts: what farmers must know at once (a disease outbreak, salt water
//! coming up the rivers, a storm), posted by an admin (`alert.manage`) for
//! one sector or all, one province or the whole region, between two days.
//!
//! The home page shows the ones in force today, every sector's (`live`; the
//! JSON API takes a sector), the newest first, at most `ALERTS_SHOWN`: "Kịp thời", timely,
//! is one of the platform's three promises.

use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use domain::{
    events::{Change, DomainEvent, Entity},
    models::alert::{Alert, AlertFields},
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        alert_repository_trait::AlertRepositoryTrait, coded_repository_trait::CodedRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_ALERT_MANAGE,
    constants::{ALERTS_SHOWN, ALERT_BODY_MAX, ALERT_TITLE_MAX},
    inputs::AlertInput,
    outputs::{AlertDto, NamedCodeDto},
    ports::EventBus,
    usecases::demand_usecase::vn_today,
    validation::{self, audit_reason, max_chars},
};

#[derive(Clone)]
pub struct AlertUseCase {
    alert_repo: Arc<dyn AlertRepositoryTrait + Send + Sync>,
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    kind_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

impl AlertUseCase {
    pub fn new(
        alert_repo: Arc<dyn AlertRepositoryTrait + Send + Sync>,
        admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
        sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
        kind_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            alert_repo,
            admin_unit_repo,
            sector_repo,
            kind_repo,
            events,
        }
    }

    async fn publish(&self, actor: &Actor, id: i32, change: Change, reason: Option<String>) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Alert,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// Every alert, switched off ones too, the latest to end first.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<AlertDto>> {
        actor.require(PERM_ALERT_MANAGE)?;
        let today = vn_today(Utc::now())?;
        Ok(self
            .alert_repo
            .list()
            .await?
            .into_iter()
            .map(|a| alert_dto(a, today))
            .collect())
    }

    pub async fn get(&self, actor: &Actor, id: i32) -> Result<AlertDto> {
        actor.require(PERM_ALERT_MANAGE)?;
        let today = vn_today(Utc::now())?;
        self.alert_repo
            .find(id)
            .await?
            .map(|a| alert_dto(a, today))
            .ok_or_else(|| Error::new(ErrorCode::AlertNotFound))
    }

    /// The provinces an alert may concern, for the form.
    pub async fn province_choices(&self, actor: &Actor) -> Result<Vec<NamedCodeDto>> {
        actor.require(PERM_ALERT_MANAGE)?;
        Ok(self
            .admin_unit_repo
            .provinces()
            .await?
            .into_iter()
            .filter(|p| p.is_active)
            .map(|p| NamedCodeDto {
                code: p.code,
                name: p.name,
            })
            .collect())
    }

    pub async fn create(&self, actor: &Actor, input: AlertInput) -> Result<i32> {
        actor.require(PERM_ALERT_MANAGE)?;
        let fields = self.validate(input).await?;
        let id = self.alert_repo.create(&fields, actor.user_id).await?;
        self.publish(actor, id, Change::Created, None).await;
        Ok(id)
    }

    pub async fn update(&self, actor: &Actor, id: i32, input: AlertInput) -> Result<()> {
        actor.require(PERM_ALERT_MANAGE)?;
        let fields = self.validate(input).await?;
        if !self.alert_repo.update(id, &fields, actor.user_id).await? {
            return Err(Error::new(ErrorCode::AlertNotFound));
        }
        self.publish(actor, id, Change::Updated, None).await;
        Ok(())
    }

    /// Turns an alert off (it leaves the home page at once) or on again. The
    /// same state again changes nothing.
    pub async fn set_active(
        &self,
        actor: &Actor,
        id: i32,
        active: bool,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_ALERT_MANAGE)?;
        let changed = self
            .alert_repo
            .set_active(id, active, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AlertNotFound))?;
        if changed {
            self.publish(actor, id, Change::Visibility { visible: active }, reason)
                .await;
        }
        Ok(())
    }

    /// The alerts in force today for a visitor of `sector` (`None`: every
    /// alert), the newest first, at most `ALERTS_SHOWN`. Public.
    pub async fn live(&self, sector: Option<&str>) -> Result<Vec<AlertDto>> {
        let today = vn_today(Utc::now())?;
        Ok(self
            .alert_repo
            .live(today, sector, ALERTS_SHOWN)
            .await?
            .into_iter()
            .map(|a| alert_dto(a, today))
            .collect())
    }

    /// Trims every box and reports every bad one at once.
    async fn validate(&self, input: AlertInput) -> Result<AlertFields> {
        let kind = input.kind.trim().to_owned();
        let sector = input.sector.trim().to_owned();
        let province = input.province_code.trim().to_owned();
        let title = input.title.trim().to_owned();
        let body = input.body.trim().to_owned();
        let known_province = province.is_empty()
            || self
                .admin_unit_repo
                .provinces()
                .await?
                .iter()
                .any(|p| p.code == province && p.is_active);
        Validation::new()
            .check(
                validation::coded(
                    &kind,
                    &self.kind_repo.list_all().await?,
                    ErrorCode::InvalidAlertKind,
                )
                .ok()
                .flatten()
                .is_none(),
                "kind",
                ErrorCode::InvalidAlertKind,
            )
            .check(
                validation::sector(&sector, &self.sector_repo.list_all().await?).is_err(),
                "sector",
                ErrorCode::InvalidSector,
            )
            .check(!known_province, "province_code", ErrorCode::InvalidProvince)
            .check(title.is_empty(), "title", ErrorCode::AlertTitleRequired)
            .rule("title", max_chars(&title, ALERT_TITLE_MAX))
            .check(body.is_empty(), "body", ErrorCode::AlertBodyRequired)
            .rule("body", max_chars(&body, ALERT_BODY_MAX))
            .check(
                input.starts_on.is_none(),
                "starts_on",
                ErrorCode::InvalidAlertDates,
            )
            .check(
                match (input.starts_on, input.ends_on) {
                    (Some(start), Some(end)) => end < start,
                    _ => true,
                },
                "ends_on",
                ErrorCode::InvalidAlertDates,
            )
            .finish()?;
        let (Some(starts_on), Some(ends_on)) = (input.starts_on, input.ends_on) else {
            return Err(Error::field("ends_on", ErrorCode::InvalidAlertDates));
        };
        Ok(AlertFields {
            kind,
            sector: (!sector.is_empty()).then_some(sector),
            province_code: (!province.is_empty()).then_some(province),
            title,
            body,
            starts_on,
            ends_on,
        })
    }
}

fn alert_dto(a: Alert, today: NaiveDate) -> AlertDto {
    AlertDto {
        live: a.is_active && a.starts_on <= today && today <= a.ends_on,
        id: a.id,
        kind: a.kind,
        kind_name: a.kind_name,
        kind_icon: a.kind_icon,
        sector: a.sector,
        province_code: a.province_code,
        province_name: a.province_name,
        title: a.title,
        body: a.body,
        starts_on: a.starts_on,
        ends_on: a.ends_on,
        is_active: a.is_active,
        is_demo: a.is_demo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap_or_default()
    }

    fn alert(active: bool, from: u32, to: u32) -> Alert {
        Alert {
            id: 1,
            kind: "disease".into(),
            kind_name: "Disease".into(),
            kind_icon: None,
            sector: Some("livestock".into()),
            province_code: None,
            province_name: None,
            title: "t".into(),
            body: "b".into(),
            starts_on: day(from),
            ends_on: day(to),
            is_active: active,
            is_demo: false,
            updated_on: Utc::now(),
        }
    }

    #[test]
    fn an_alert_is_live_while_on_and_between_its_dates() {
        assert!(alert_dto(alert(true, 1, 10), day(1)).live);
        assert!(alert_dto(alert(true, 1, 10), day(10)).live);
        assert!(!alert_dto(alert(true, 1, 10), day(11)).live);
        assert!(!alert_dto(alert(false, 1, 10), day(5)).live);
    }
}
