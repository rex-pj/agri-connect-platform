//! Commodities ("cây trồng, vật nuôi"): what farmers raise, rice, durian, pigs,
//! shrimp, each in one sector. The admin keeps them (`category.manage`, with
//! the service and supply types); the price board groups its items by them,
//! and a supply may say which one it is for.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::{coded::CodedEntry, commodity::CommodityFields},
    repositories::{
        coded_repository_trait::CodedRepositoryTrait,
        commodity_repository_trait::CommodityRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_CATEGORY_MANAGE,
    constants::{COMMODITY_NAME_MAX, PRICE_FRESH_DAYS_MAX},
    inputs::CommodityInput,
    outputs::CommodityDto,
    ports::EventBus,
    validation::{self, audit_reason, max_chars, sector},
};

#[derive(Clone)]
pub struct CommodityUseCase {
    commodity_repo: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
    sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

impl CommodityUseCase {
    pub fn new(
        commodity_repo: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
        sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            commodity_repo,
            sector_repo,
            events,
        }
    }

    async fn publish(&self, actor: &Actor, id: i32, change: Change, reason: Option<String>) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Commodity,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// The shown commodities, in their order: what the public pages and the
    /// forms offer.
    pub async fn shown(&self) -> Result<Vec<CommodityDto>> {
        Ok(self
            .commodity_repo
            .list_all()
            .await?
            .into_iter()
            .filter(|c| c.is_active)
            .map(CommodityDto::from)
            .collect())
    }

    /// Every commodity, hidden ones too.
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CommodityDto>> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        Ok(self
            .commodity_repo
            .list_all()
            .await?
            .into_iter()
            .map(CommodityDto::from)
            .collect())
    }

    pub async fn get(&self, actor: &Actor, id: i32) -> Result<CommodityDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        self.commodity_repo
            .find(id)
            .await?
            .map(CommodityDto::from)
            .ok_or_else(|| Error::new(ErrorCode::CommodityNotFound))
    }

    pub async fn create(&self, actor: &Actor, input: CommodityInput) -> Result<i32> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let fields = validate(input, &self.sector_repo.list_all().await?)?;
        let id = self.commodity_repo.create(&fields, actor.user_id).await?;
        self.publish(actor, id, Change::Created, None).await;
        Ok(id)
    }

    pub async fn update(&self, actor: &Actor, id: i32, input: CommodityInput) -> Result<()> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let fields = validate(input, &self.sector_repo.list_all().await?)?;
        if !self
            .commodity_repo
            .update(id, &fields, actor.user_id)
            .await?
        {
            return Err(Error::new(ErrorCode::CommodityNotFound));
        }
        self.publish(actor, id, Change::Updated, None).await;
        Ok(())
    }

    /// Hides a commodity (its price board items and the supply filter leave
    /// the public pages) or shows it again. The same state changes nothing.
    pub async fn set_active(
        &self,
        actor: &Actor,
        id: i32,
        active: bool,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let (_, changed) = self
            .commodity_repo
            .set_active(id, active, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::CommodityNotFound))?;
        if changed {
            self.publish(actor, id, Change::Visibility { visible: active }, reason)
                .await;
        }
        Ok(())
    }
}

/// A commodity's boxes: a name, one of the `known` sectors, an optional icon.
fn validate(input: CommodityInput, known: &[CodedEntry]) -> Result<CommodityFields> {
    let name = input.name.trim().to_owned();
    let icon = validation::icon(input.icon.as_deref());
    let chosen = sector(&input.sector, known).ok().flatten();
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, COMMODITY_NAME_MAX))
        .check(chosen.is_none(), "sector", ErrorCode::InvalidSector)
        .check(icon.is_err(), "icon", ErrorCode::InvalidIcon)
        .check_with(
            !(1..=PRICE_FRESH_DAYS_MAX).contains(&input.price_fresh_days),
            "price_fresh_days",
            ErrorCode::InvalidFreshDays,
            ("max", PRICE_FRESH_DAYS_MAX as usize),
        )
        .finish()?;
    let Some(chosen) = chosen else {
        return Err(Error::field("sector", ErrorCode::InvalidSector));
    };
    Ok(CommodityFields {
        name,
        sector: chosen,
        icon: icon.ok().flatten(),
        display_order: input.display_order,
        price_fresh_days: input.price_fresh_days,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crops_sector() -> Vec<CodedEntry> {
        vec![CodedEntry {
            id: 1,
            code: "crop".into(),
            name: "Crops".into(),
            icon: None,
            display_order: 1,
            is_active: true,
            is_demo: false,
        }]
    }

    #[test]
    fn a_commodity_needs_a_name_a_sector_and_a_freshness() {
        let err = validate(
            CommodityInput {
                name: " ".into(),
                sector: "".into(),
                icon: None,
                display_order: 0,
                price_fresh_days: 0,
            },
            &crops_sector(),
        )
        .err();
        let fields: Vec<&str> = err
            .as_ref()
            .map(|e| e.fields().iter().map(|f| f.field).collect())
            .unwrap_or_default();
        assert_eq!(fields, vec!["name", "sector", "price_fresh_days"]);
    }

    #[test]
    fn a_commodity_keeps_its_sector_and_trims() {
        let fields = validate(
            CommodityInput {
                name: " Durian ".into(),
                sector: "crop".into(),
                icon: Some(" ".into()),
                display_order: 2,
                price_fresh_days: 7,
            },
            &crops_sector(),
        );
        let fields = fields.ok();
        assert_eq!(fields.as_ref().map(|f| f.name.as_str()), Some("Durian"));
        assert_eq!(fields.as_ref().map(|f| f.sector.as_str()), Some("crop"));
        assert!(fields.is_some_and(|f| f.icon.is_none() && f.price_fresh_days == 7));
    }
}
