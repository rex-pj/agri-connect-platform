//! GET /prices: today's farm-gate prices, a section per commodity (cây /
//! con the admin keeps: rice, durian, live pigs, shrimp…) of the visitor's
//! sector, each item with its latest price per area (only prices at most `PRICE_FRESH_DAYS` old) and
//! how it moved since the one before. A GET form narrows it to one area; it
//! sends itself on change with JS (`autoSubmit`).

use application::outputs::{CommodityDto, CropBoardDto, NamedDto, PriceQuoteDto};
use axum::{extract::State, response::Response};
use chrono::NaiveDate;
use domain::Error;
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::ApiQuery,
    pages::listing::{parse_opt, with_query},
    routes,
    shell::Shell,
    views::render_page,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PricesQuery {
    /// A location preset id; anything else is every area.
    pub area: String,
}

/// One price as shown: the quote, which way it moved ("up", "down",
/// "same", or "" with nothing to compare), by how much, and whether it is
/// today's.
#[derive(Debug, Serialize)]
pub struct PriceLine {
    pub quote: PriceQuoteDto,
    pub direction: &'static str,
    pub change_abs: i64,
    pub is_today: bool,
}

impl PriceLine {
    pub fn new(quote: PriceQuoteDto, today: NaiveDate) -> Self {
        let direction = match quote.change {
            Some(c) if c > 0 => "up",
            Some(c) if c < 0 => "down",
            Some(_) => "same",
            None => "",
        };
        Self {
            direction,
            change_abs: quote.change.map_or(0, i64::abs),
            is_today: quote.observed_on == today,
            quote,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CropCard {
    pub name: String,
    pub unit: String,
    pub lines: Vec<PriceLine>,
}

/// A commodity: its title, its icon, its items with a price, and the way to
/// the supplies for it.
#[derive(Debug, Serialize)]
pub struct PriceSection {
    pub title: String,
    pub icon: String,
    pub crops: Vec<CropCard>,
    pub supplies_href: String,
}

#[derive(Debug, Serialize)]
pub struct PricesPage {
    pub sections: Vec<PriceSection>,
    pub areas: Vec<NamedDto>,
    /// The chosen area, when it has prices.
    pub area_id: Option<i32>,
    /// Every area at once: each price line names its area.
    pub show_area: bool,
    pub area_name: Option<String>,
    pub action: String,
    pub back_href: String,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("price links: {e}")))
}

/// The produce of `group` as cards; `None` when there is none.
fn section(
    group: &CommodityDto,
    crops: &[CropBoardDto],
    today: NaiveDate,
) -> Result<Option<PriceSection>, Error> {
    let cards: Vec<CropCard> = crops
        .iter()
        .filter(|c| c.commodity_id == group.id)
        .map(|c| CropCard {
            name: c.name.clone(),
            unit: c.unit.clone(),
            lines: c
                .quotes
                .iter()
                .cloned()
                .map(|q| PriceLine::new(q, today))
                .collect(),
        })
        .collect();
    if cards.is_empty() {
        return Ok(None);
    }
    Ok(Some(PriceSection {
        title: group.name.clone(),
        icon: group.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        crops: cards,
        supplies_href: with_query(&url("products")?, &[("commodity", &group.id.to_string())]),
    }))
}

/// GET /prices
pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<PricesQuery>,
) -> Result<Response, Error> {
    let area = parse_opt::<i32>(&q.area);
    let board = state
        .usecases
        .price
        .board(area, shell.sector.as_deref())
        .await?;
    let area_name = area.and_then(|id| {
        board
            .areas
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
    });
    // One section per commodity with a price, in the commodities' order.
    let mut sections = Vec::new();
    for g in &board.commodities {
        sections.extend(section(g, &board.crops, board.today)?);
    }
    let page = PricesPage {
        sections,
        area_id: area,
        show_area: area_name.is_none(),
        area_name,
        areas: board.areas,
        action: url("prices")?,
        back_href: url("home")?,
    };
    render_page(&state, &shell, "pages/prices.html", &page)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(change: Option<i64>, on: NaiveDate) -> PriceQuoteDto {
        PriceQuoteDto {
            crop_id: 1,
            crop_name: "crop".into(),
            unit: "kg".into(),
            location_id: 1,
            location_label: "area".into(),
            observed_on: on,
            price_low: 6_000,
            price_high: 6_200,
            note: None,
            change,
        }
    }

    #[test]
    fn a_line_says_which_way_the_price_moved() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap_or_default();
        let down = PriceLine::new(quote(Some(-150), today), today);
        assert_eq!(
            (down.direction, down.change_abs, down.is_today),
            ("down", 150, true)
        );
        let first = PriceLine::new(quote(None, today - chrono::Duration::days(1)), today);
        assert_eq!((first.direction, first.is_today), ("", false));
        assert_eq!(
            PriceLine::new(quote(Some(0), today), today).direction,
            "same"
        );
    }
}
