//! GET /prices: today's farm-gate prices, rice then fruit, each crop with its
//! latest price per area (only prices at most `PRICE_FRESH_DAYS` old) and
//! how it moved since the one before. A GET form narrows it to one area; it
//! sends itself on change with JS (`autoSubmit`).

use application::outputs::{CropBoardDto, NamedDto, PriceQuoteDto};
use axum::{extract::State, response::Response};
use chrono::NaiveDate;
use domain::{
    constants::{CROP_KIND_FRUIT, CROP_KIND_RICE},
    Error,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState, extract::ApiQuery, pages::listing::parse_opt, routes, shell::Shell,
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

/// Rice or fruit: its title and its crops with a price.
#[derive(Debug, Serialize)]
pub struct PriceSection {
    pub title: String,
    pub icon: &'static str,
    pub crops: Vec<CropCard>,
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
    pub fresh_days: i64,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("price links: {e}")))
}

/// The crops of `kind` as cards; `None` when there are none.
fn section(
    state: &AppState,
    crops: &[CropBoardDto],
    kind: &str,
    title_key: &str,
    icon: &'static str,
    today: NaiveDate,
) -> Option<PriceSection> {
    let cards: Vec<CropCard> = crops
        .iter()
        .filter(|c| c.kind == kind)
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
    (!cards.is_empty()).then(|| PriceSection {
        title: state.locale().text(title_key),
        icon,
        crops: cards,
    })
}

/// GET /prices
pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<PricesQuery>,
) -> Result<Response, Error> {
    let area = parse_opt::<i32>(&q.area);
    let board = state.usecases.price.board(area).await?;
    let area_name = area.and_then(|id| {
        board
            .areas
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
    });
    let sections = [
        section(
            &state,
            &board.crops,
            CROP_KIND_RICE,
            "prices.rice",
            "fa-wheat-awn",
            board.today,
        ),
        section(
            &state,
            &board.crops,
            CROP_KIND_FRUIT,
            "prices.fruit",
            "fa-lemon",
            board.today,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let page = PricesPage {
        sections,
        area_id: area,
        show_area: area_name.is_none(),
        area_name,
        areas: board.areas,
        action: url("prices")?,
        back_href: url("home")?,
        fresh_days: board.fresh_days,
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
