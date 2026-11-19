//! Farm produce prices ("nông sản"): the farm-gate price of a rice variety, a
//! fruit, live pigs or shrimp in one area (a location preset) on one day, as a
//! low–high range in đồng per unit.
//!
//! Admins keep the crops, each a variety or grade of one commodity (cây trồng, vật nuôi:
//! rice, durian, pigs), and enter the prices (`price.manage`);
//! entering the same crop, area and day again replaces that day's price. The
//! public board shows, commodity by commodity, each crop's latest price per area,
//! only while it is fresh: at most its commodity's `price_fresh_days` old
//! (durian moves daily, rice weekly; a stale price misleads more than none),
//! with how its middle moved since the price before.

use std::{collections::HashMap, sync::Arc};

use chrono::{Duration, NaiveDate, Utc};
use domain::{
    events::{Change, DomainEvent, Entity},
    models::commodity::Commodity,
    models::price::{Crop, CropFields, CropPrice, NewCropPrice},
    repositories::{
        commodity_repository_trait::CommodityRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        price_repository_trait::PriceRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_PRICE_MANAGE,
    constants::{
        ADMIN_PAGE_SIZE, CROP_NAME_MAX, CROP_UNIT_MAX, PRICE_FRESH_DAYS_DEFAULT,
        PRICE_HISTORY_DAYS, PRICE_NOTE_MAX, PRICE_PER_UNIT_MAX,
    },
    inputs::{CropInput, PriceInput},
    outputs::{
        AdminPagedResponse, CommodityDto, CropBoardDto, CropDto, CropPriceDto, NamedDto,
        PriceBoardDto, PriceChoicesDto, PriceQuoteDto,
    },
    pagination::{admin_total_pages, paginate_admin, total_or_count},
    ports::EventBus,
    usecases::demand_usecase::vn_today,
    validation::{audit_reason, max_chars, max_chars_opt, non_blank},
};

#[derive(Clone)]
pub struct PriceUseCase {
    price_repo: Arc<dyn PriceRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    commodity_repo: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

impl PriceUseCase {
    pub fn new(
        price_repo: Arc<dyn PriceRepositoryTrait + Send + Sync>,
        location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
        commodity_repo: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            price_repo,
            location_repo,
            commodity_repo,
            events,
        }
    }

    async fn publish(
        &self,
        actor: &Actor,
        entity: Entity,
        id: i32,
        change: Change,
        reason: Option<String>,
    ) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    // ── Crops (admin) ──────────────────────────────────────────────────────────

    /// Every crop, hidden ones too.
    pub async fn list_crops(&self, actor: &Actor) -> Result<Vec<CropDto>> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (crops, commodities) =
            tokio::try_join!(self.price_repo.list_crops(), self.commodity_repo.list_all())?;
        Ok(crops
            .into_iter()
            .map(|c| crop_dto(c, &commodities))
            .collect())
    }

    /// The crops of one commodity, hidden ones too, in their order.
    pub async fn crops_of(&self, actor: &Actor, commodity_id: i32) -> Result<Vec<CropDto>> {
        actor.require(PERM_PRICE_MANAGE)?;
        Ok(self
            .list_crops(actor)
            .await?
            .into_iter()
            .filter(|c| c.commodity_id == commodity_id)
            .collect())
    }

    pub async fn get_crop(&self, actor: &Actor, id: i32) -> Result<CropDto> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (crop, commodities) = tokio::try_join!(
            self.price_repo.find_crop(id),
            self.commodity_repo.list_all()
        )?;
        crop.map(|c| crop_dto(c, &commodities))
            .ok_or_else(|| Error::new(ErrorCode::CropNotFound))
    }

    pub async fn create_crop(&self, actor: &Actor, input: CropInput) -> Result<i32> {
        actor.require(PERM_PRICE_MANAGE)?;
        let fields = validate_crop(input, &self.commodity_repo.list_all().await?)?;
        let id = self.price_repo.create_crop(&fields).await?;
        self.publish(actor, Entity::Crop, id, Change::Created, None)
            .await;
        Ok(id)
    }

    pub async fn update_crop(&self, actor: &Actor, id: i32, input: CropInput) -> Result<()> {
        actor.require(PERM_PRICE_MANAGE)?;
        let fields = validate_crop(input, &self.commodity_repo.list_all().await?)?;
        if !self.price_repo.update_crop(id, &fields).await? {
            return Err(Error::new(ErrorCode::CropNotFound));
        }
        self.publish(actor, Entity::Crop, id, Change::Updated, None)
            .await;
        Ok(())
    }

    /// Hides a crop (its prices leave the public board, they are kept) or
    /// shows it again. The same state again changes nothing.
    pub async fn set_crop_active(
        &self,
        actor: &Actor,
        id: i32,
        active: bool,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (_, changed) = self
            .price_repo
            .set_crop_active(id, active)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::CropNotFound))?;
        if changed {
            self.publish(
                actor,
                Entity::Crop,
                id,
                Change::Visibility { visible: active },
                reason,
            )
            .await;
        }
        Ok(())
    }

    /// The shown commodities the crop form offers, in their order.
    pub async fn commodity_choices(&self, actor: &Actor) -> Result<Vec<CommodityDto>> {
        actor.require(PERM_PRICE_MANAGE)?;
        Ok(self
            .commodity_repo
            .list_all()
            .await?
            .into_iter()
            .filter(|c| c.is_active)
            .map(CommodityDto::from)
            .collect())
    }

    // ── Prices (admin) ─────────────────────────────────────────────────────────

    /// Every price entered (or `crop`'s only), the newest day first.
    pub async fn list_prices(
        &self,
        actor: &Actor,
        crop: Option<i32>,
        page: i64,
    ) -> Result<AdminPagedResponse<CropPriceDto>> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (page, offset) = paginate_admin(page);
        let rows = self
            .price_repo
            .list_prices(
                crop,
                u64::try_from(ADMIN_PAGE_SIZE).unwrap_or(0),
                u64::try_from(offset).unwrap_or(0),
            )
            .await?;
        let total = total_or_count(offset, rows.len(), ADMIN_PAGE_SIZE, || {
            self.price_repo.count_prices(crop)
        })
        .await?;
        Ok(AdminPagedResponse {
            items: rows.into_iter().map(price_dto).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: admin_total_pages(total),
        })
    }

    /// The shown crops and areas the price form offers.
    pub async fn price_choices(&self, actor: &Actor) -> Result<PriceChoicesDto> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (crops, areas) = tokio::try_join!(
            self.price_repo.list_crops(),
            self.location_repo.list_active()
        )?;
        Ok(PriceChoicesDto {
            crops: crops
                .into_iter()
                .filter(|c| c.is_active)
                .map(|c| NamedDto {
                    id: c.id,
                    name: c.name,
                })
                .collect(),
            areas: areas
                .into_iter()
                .map(|l| NamedDto {
                    id: l.id,
                    name: l.label,
                })
                .collect(),
        })
    }

    /// Stores a price of a shown crop in a shown area, on a day that is not
    /// after today (Vietnam); the same crop, area and day again replaces it.
    /// Its id.
    pub async fn record_price(&self, actor: &Actor, input: PriceInput) -> Result<i32> {
        actor.require(PERM_PRICE_MANAGE)?;
        let (crops, locations) =
            tokio::try_join!(self.price_repo.list_crops(), self.location_repo.list_all())?;
        let today = vn_today(Utc::now())?;
        let crop = input
            .crop_id
            .filter(|id| crops.iter().any(|c| c.id == *id && c.is_active));
        let location = input
            .location_id
            .filter(|id| locations.iter().any(|l| l.id == *id && l.is_active));
        let observed_on = input.observed_on.filter(|d| *d <= today);
        let in_range = |p: &i64| (1..=PRICE_PER_UNIT_MAX).contains(p);
        let low = input.price_low.filter(in_range);
        let high = input.price_high.filter(in_range);
        let note = non_blank(input.note);
        Validation::new()
            .check(crop.is_none(), "crop_id", ErrorCode::CropNotFound)
            .check(
                location.is_none(),
                "location_id",
                ErrorCode::LocationNotFound,
            )
            .check(
                observed_on.is_none(),
                "observed_on",
                ErrorCode::InvalidPriceDate,
            )
            .check(low.is_none(), "price_low", ErrorCode::InvalidPrice)
            .check(
                high.is_none() || matches!((low, high), (Some(l), Some(h)) if h < l),
                "price_high",
                ErrorCode::InvalidPriceRange,
            )
            .rule("note", max_chars_opt(&note, PRICE_NOTE_MAX))
            .finish()?;
        let (
            Some(crop_id),
            Some(location_id),
            Some(observed_on),
            Some(price_low),
            Some(price_high),
        ) = (crop, location, observed_on, low, high)
        else {
            return Err(Error::new(ErrorCode::ValidationFailed));
        };
        let id = self
            .price_repo
            .record_price(
                &NewCropPrice {
                    crop_id,
                    location_id,
                    observed_on,
                    price_low,
                    price_high,
                    note,
                },
                actor.user_id,
            )
            .await?;
        self.publish(actor, Entity::CropPrice, id, Change::Created, None)
            .await;
        Ok(id)
    }

    pub async fn delete_price(&self, actor: &Actor, id: i32, reason: Option<String>) -> Result<()> {
        actor.require(PERM_PRICE_MANAGE)?;
        if !self.price_repo.delete_price(id).await? {
            return Err(Error::new(ErrorCode::PriceNotFound));
        }
        self.publish(actor, Entity::CropPrice, id, Change::Deleted, reason)
            .await;
        Ok(())
    }

    // ── Public ─────────────────────────────────────────────────────────────────

    /// The price page: each shown crop's latest fresh price in `area`, or in
    /// every area, with the areas that have one; by commodity (shown ones, in
    /// their order), those of `sector` only when one is chosen.
    pub async fn board(&self, area: Option<i32>, sector: Option<&str>) -> Result<PriceBoardDto> {
        let today = vn_today(Utc::now())?;
        let (prices, crops, commodities) = tokio::try_join!(
            self.price_repo.latest_public_prices(
                today - Duration::days(PRICE_HISTORY_DAYS),
                PRICES_COMPARED as u64,
            ),
            self.price_repo.list_crops(),
            self.commodity_repo.list_all(),
        )?;
        let commodities: Vec<Commodity> = commodities
            .into_iter()
            .filter(|c| c.is_active)
            .filter(|c| sector.is_none_or(|chosen| chosen == c.sector))
            .collect();
        let (crops, areas) = build_board(&prices, &crops, &commodities, today, area);
        Ok(PriceBoardDto {
            commodities: commodities
                .into_iter()
                .filter(|g| crops.iter().any(|c| c.commodity_id == g.id))
                .map(CommodityDto::from)
                .collect(),
            crops,
            areas,
            today,
        })
    }

    /// The home page's few prices: each crop's newest fresh price (whatever
    /// the area), the most recent first, at most `limit`.
    pub async fn highlights(
        &self,
        limit: usize,
        sector: Option<&str>,
    ) -> Result<Vec<PriceQuoteDto>> {
        Ok(highlights_of(self.board(None, sector).await?.crops, limit))
    }
}

/// How many prices of a crop in an area the board reads: the latest, and
/// the one before it for the move.
const PRICES_COMPARED: usize = 2;

/// The middle of a range, what the move between two prices compares.
fn middle(p: &CropPrice) -> i64 {
    (p.price_low + p.price_high) / 2
}

/// The board from prices newest first: per crop (by commodity in the order
/// of `commodities`, then in the crops' order; shown crops of those only) the
/// latest price of each area if it is fresh (within its commodity's
/// `price_fresh_days`), compared with the one before; `area` keeps one area.
/// Also the areas with a fresh price, by label.
fn build_board(
    prices: &[CropPrice],
    crops: &[Crop],
    commodities: &[Commodity],
    today: NaiveDate,
    area: Option<i32>,
) -> (Vec<CropBoardDto>, Vec<NamedDto>) {
    // The first day a price of each crop is still fresh on: its commodity's
    // window, a crop of a commodity not on this board the default one.
    let fresh_from: HashMap<i32, NaiveDate> = crops
        .iter()
        .map(|c| {
            let days = commodities
                .iter()
                .find(|g| g.id == c.commodity_id)
                .map_or(PRICE_FRESH_DAYS_DEFAULT, |g| g.price_fresh_days);
            (c.id, today - Duration::days(i64::from(days)))
        })
        .collect();
    let is_fresh = |p: &CropPrice| {
        fresh_from
            .get(&p.crop_id)
            .is_some_and(|from| p.observed_on >= *from)
    };
    // (crop, area) → [latest, previous], in the newest-first order given:
    // the move is the latest against the one before.
    let mut pairs: HashMap<(i32, i32), Vec<&CropPrice>> = HashMap::new();
    for p in prices {
        let seen = pairs.entry((p.crop_id, p.location_id)).or_default();
        if seen.len() < PRICES_COMPARED {
            seen.push(p);
        }
    }
    let mut quotes: HashMap<i32, Vec<PriceQuoteDto>> = HashMap::new();
    let mut areas: HashMap<i32, String> = HashMap::new();
    for seen in pairs.values() {
        let Some(latest) = seen.first().filter(|p| is_fresh(p)) else {
            continue;
        };
        areas.insert(latest.location_id, latest.location_label.clone());
        if area.is_some_and(|a| a != latest.location_id) {
            continue;
        }
        quotes
            .entry(latest.crop_id)
            .or_default()
            .push(PriceQuoteDto {
                crop_id: latest.crop_id,
                crop_name: latest.crop_name.clone(),
                unit: latest.unit.clone(),
                location_id: latest.location_id,
                location_label: latest.location_label.clone(),
                observed_on: latest.observed_on,
                price_low: latest.price_low,
                price_high: latest.price_high,
                note: latest.note.clone(),
                change: seen.get(1).map(|before| middle(latest) - middle(before)),
            });
    }
    let board = commodities
        .iter()
        .flat_map(|g| crops.iter().filter(move |c| c.commodity_id == g.id))
        .filter(|c| c.is_active)
        .filter_map(|c| {
            let mut quotes = quotes.remove(&c.id)?;
            quotes.sort_by(|a, b| {
                a.location_label
                    .cmp(&b.location_label)
                    .then(a.location_id.cmp(&b.location_id))
            });
            Some(CropBoardDto {
                crop_id: c.id,
                name: c.name.clone(),
                commodity_id: c.commodity_id,
                unit: c.unit.clone(),
                quotes,
            })
        })
        .collect();
    let mut areas: Vec<NamedDto> = areas
        .into_iter()
        .map(|(id, name)| NamedDto { id, name })
        .collect();
    areas.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    (board, areas)
}

/// Each crop's newest quote (the first area by label on a tie), the most
/// recent first, then in the crops' order; at most `limit`.
fn highlights_of(crops: Vec<CropBoardDto>, limit: usize) -> Vec<PriceQuoteDto> {
    let mut newest: Vec<(usize, PriceQuoteDto)> = crops
        .into_iter()
        .enumerate()
        .filter_map(|(order, c)| {
            let mut best: Option<PriceQuoteDto> = None;
            for q in c.quotes {
                if best.as_ref().is_none_or(|b| q.observed_on > b.observed_on) {
                    best = Some(q);
                }
            }
            best.map(|q| (order, q))
        })
        .collect();
    newest.sort_by(|(oa, a), (ob, b)| b.observed_on.cmp(&a.observed_on).then(oa.cmp(ob)));
    newest.into_iter().take(limit).map(|(_, q)| q).collect()
}

/// A crop's boxes; its commodity must be one of `commodities`.
fn validate_crop(input: CropInput, commodities: &[Commodity]) -> Result<CropFields> {
    let name = input.name.trim().to_owned();
    let commodity_id = input
        .commodity_id
        .filter(|id| commodities.iter().any(|g| g.id == *id));
    let unit = input.unit.trim().to_owned();
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, CROP_NAME_MAX))
        .check(
            commodity_id.is_none(),
            "commodity_id",
            ErrorCode::InvalidCommodity,
        )
        .check(unit.is_empty(), "unit", ErrorCode::UnitRequired)
        .rule("unit", max_chars(&unit, CROP_UNIT_MAX))
        .finish()?;
    let Some(commodity_id) = commodity_id else {
        return Err(Error::field("commodity_id", ErrorCode::InvalidCommodity));
    };
    Ok(CropFields {
        name,
        commodity_id,
        unit,
        display_order: input.display_order,
    })
}

/// A crop with its commodity's name (from `commodities`, hidden ones too).
fn crop_dto(c: Crop, commodities: &[Commodity]) -> CropDto {
    CropDto {
        commodity: commodities
            .iter()
            .find(|g| g.id == c.commodity_id)
            .map(|g| g.name.clone()),
        commodity_id: c.commodity_id,
        id: c.id,
        name: c.name,
        unit: c.unit,
        display_order: c.display_order,
        is_active: c.is_active,
        is_demo: c.is_demo,
    }
}

fn price_dto(p: CropPrice) -> CropPriceDto {
    CropPriceDto {
        id: p.id,
        crop_id: p.crop_id,
        crop_name: p.crop_name,
        unit: p.unit,
        location_id: p.location_id,
        location_label: p.location_label,
        observed_on: p.observed_on,
        price_low: p.price_low,
        price_high: p.price_high,
        note: p.note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap_or_default()
    }

    fn crop(id: i32, active: bool) -> Crop {
        Crop {
            id,
            name: format!("crop {id}"),
            commodity_id: 1,
            unit: "kg".into(),
            display_order: id,
            is_active: active,
            is_demo: false,
        }
    }

    /// The one commodity the test crops are of.
    fn rice() -> Vec<Commodity> {
        vec![Commodity {
            id: 1,
            name: "rice".into(),
            sector: "crop".into(),
            icon: None,
            display_order: 1,
            price_fresh_days: PRICE_FRESH_DAYS_DEFAULT,
            is_active: true,
            is_demo: false,
        }]
    }

    fn price(id: i32, crop_id: i32, location_id: i32, on: u32, low: i64, high: i64) -> CropPrice {
        CropPrice {
            id,
            crop_id,
            crop_name: format!("crop {crop_id}"),
            unit: "kg".into(),
            location_id,
            location_label: format!("area {location_id}"),
            observed_on: day(on),
            price_low: low,
            price_high: high,
            note: None,
        }
    }

    #[test]
    fn each_commodity_keeps_its_prices_for_its_own_days() {
        let today = day(20);
        // Crop 1 is rice (14 days); crop 2 is durian, fresh for 2 days.
        let durian = Commodity {
            id: 2,
            name: "durian".into(),
            price_fresh_days: 2,
            ..rice().remove(0)
        };
        let commodities = vec![rice().remove(0), durian];
        let crops = vec![
            crop(1, true),
            Crop {
                commodity_id: 2,
                ..crop(2, true)
            },
        ];
        let prices = vec![
            price(2, 2, 10, 17, 90_000, 95_000),
            price(1, 1, 10, 10, 6_000, 6_200),
        ];
        let (board, _) = build_board(&prices, &crops, &commodities, today, None);
        let shown: Vec<i32> = board.iter().map(|c| c.crop_id).collect();
        assert_eq!(
            shown,
            vec![1],
            "a 10-day-old rice price shows, a 3-day-old durian one does not"
        );
        let (board, _) = build_board(&prices, &crops, &commodities, day(18), None);
        assert_eq!(board.len(), 2, "on the 18th the durian price is a day old");
    }

    #[test]
    fn the_board_shows_the_latest_fresh_price_and_its_move() {
        let today = day(20);
        // Newest first, as the repository gives them.
        let prices = vec![
            price(5, 1, 10, 20, 6_200, 6_400),
            price(4, 1, 11, 19, 6_000, 6_000),
            price(3, 1, 10, 18, 6_000, 6_200),
            price(2, 1, 10, 15, 5_000, 5_000),
            // Crop 2's only price is older than its commodity's 14 days.
            price(1, 2, 10, 1, 90_000, 95_000),
        ];
        let (board, areas) = build_board(
            &prices,
            &[crop(1, true), crop(2, true)],
            &rice(),
            today,
            None,
        );
        assert_eq!(board.len(), 1, "a stale price is not shown");
        let quotes = &board.first().map(|c| c.quotes.clone()).unwrap_or_default();
        assert_eq!(quotes.len(), 2);
        let area10 = quotes.iter().find(|q| q.location_id == 10);
        assert_eq!(area10.map(|q| q.observed_on), Some(day(20)));
        assert_eq!(
            area10.and_then(|q| q.change),
            Some(200),
            "6 300 against 6 100"
        );
        let area11 = quotes.iter().find(|q| q.location_id == 11);
        assert_eq!(
            area11.and_then(|q| q.change),
            None,
            "nothing to compare with"
        );
        assert_eq!(
            areas.iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![10, 11],
            "only areas with a fresh price"
        );

        let (one, areas) = build_board(&prices, &[crop(1, true)], &rice(), today, Some(11));
        assert_eq!(one.first().map(|c| c.quotes.len()), Some(1));
        assert_eq!(
            areas.len(),
            2,
            "the area list does not shrink with the filter"
        );

        let (hidden, _) = build_board(&prices, &[crop(1, false)], &rice(), today, None);
        assert!(hidden.is_empty(), "a hidden crop is not shown");

        let (no_group, _) = build_board(&prices, &[crop(1, true)], &[], today, None);
        assert!(
            no_group.is_empty(),
            "a crop of no shown commodity is not shown"
        );
    }

    #[test]
    fn highlights_take_each_crops_newest_price_most_recent_first() {
        let today = day(20);
        let prices = vec![
            price(4, 2, 10, 20, 80_000, 85_000),
            price(3, 1, 11, 19, 6_000, 6_000),
            price(2, 1, 10, 17, 6_100, 6_100),
            price(1, 3, 10, 18, 30_000, 30_000),
        ];
        let crops = [crop(1, true), crop(2, true), crop(3, true)];
        let (board, _) = build_board(&prices, &crops, &rice(), today, None);
        let top = highlights_of(board, 2);
        assert_eq!(
            top.iter()
                .map(|q| (q.crop_id, q.location_id))
                .collect::<Vec<_>>(),
            vec![(2, 10), (1, 11)]
        );
    }

    #[test]
    fn a_crop_needs_a_name_a_known_commodity_and_a_unit() {
        let commodities = rice();
        let err = validate_crop(
            CropInput {
                name: " ".into(),
                commodity_id: Some(99),
                unit: "".into(),
                display_order: 0,
            },
            &commodities,
        )
        .err();
        let fields: Vec<&str> = err
            .as_ref()
            .map(|e| e.fields().iter().map(|f| f.field).collect())
            .unwrap_or_default();
        assert_eq!(fields, vec!["name", "commodity_id", "unit"]);
    }
}
