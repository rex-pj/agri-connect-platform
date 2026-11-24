//! GET /prices: today's farm-gate prices, a section per commodity (cây /
//! con the admin keeps: rice, durian, live pigs, shrimp…) of the visitor's
//! sector, each item with its latest price per area (only prices at most `PRICE_FRESH_DAYS` old) and
//! how it moved since the one before. A GET form narrows it to one area; it
//! sends itself on change with JS (`autoSubmit`).
//!
//! The area chosen is remembered (`price_area` cookie, a year): `?area=7`
//! keeps it, `?area=` (every area) forgets it, and `/prices` without `area`
//! opens on it. The home strip follows it and names it in its title, so the
//! choice never narrows a page unseen. An area that no longer has a fresh
//! price is passed over: the board shows every area.
//!
//! Chips over the board narrow it to one commodity (`?commodity=`), whatever
//! the visitor's sector, as a category chosen on the lists is; a supply for
//! a commodity with a fresh price links there (`commodity_href`). One with
//! no fresh price (an old link) is passed over.
//!
//! Each line with enough prices in the last `PRICE_TREND_DAYS` draws them
//! as a small line under its price (`PriceUseCase::trends`, `Sparkline`).

use std::collections::HashMap;

use application::{
    constants::PRICE_TREND_DAYS,
    features::Feature,
    outputs::{CommodityDto, CropBoardDto, NamedDto, PriceQuoteDto, PriceTrendDto},
};
use axum::{extract::State, http::HeaderMap, response::Response};
use chrono::{Duration, NaiveDate};
use domain::Error;
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::{PRICE_AREA_SEARCH_MIN, PRICE_STALE_DAYS},
    cookies::request_cookie,
    extract::ApiQuery,
    pages::{
        language::remember,
        listing::{parse_opt, with_query},
    },
    routes,
    shell::Shell,
    views::{format_number, format_vnd, render_page},
};

/// The area of the price board this browser chose last.
pub const PRICE_AREA_COOKIE: &str = "price_area";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PricesQuery {
    /// A location preset id, or empty for every area; absent: the area
    /// remembered.
    pub area: Option<String>,
    /// A commodity id: only its prices, whatever the visitor's sector.
    pub commodity: String,
}

/// One chip over the board: a commodity with a price (or every one, first),
/// a link that narrows the board to it.
#[derive(Debug, Serialize)]
pub struct CommodityChip {
    pub name: String,
    pub icon: Option<String>,
    pub href: String,
    pub current: bool,
}

/// The area this browser chose on the price board, when it is a number
/// (whether it still has prices is the board's to say).
pub fn remembered_area(headers: &HeaderMap) -> Option<i32> {
    request_cookie(headers, PRICE_AREA_COOKIE).and_then(parse_opt::<i32>)
}

/// One price as shown: the quote, which way it moved ("up", "down",
/// "same", or "" with nothing to compare), by how much, how many days old
/// it is (said "today", "yesterday", "n days ago") and whether it is old
/// enough to be drawn faded (`PRICE_STALE_DAYS`).
#[derive(Debug, Serialize)]
pub struct PriceLine {
    pub quote: PriceQuoteDto,
    pub direction: &'static str,
    pub change_abs: i64,
    pub days_ago: i64,
    pub is_stale: bool,
    /// The price as read: "6.200 ₫", or "6.200 – 6.400 ₫" for a range; the
    /// line and the text shared both say it so.
    pub range: String,
    /// Its last `PRICE_TREND_DAYS` as a small line (the price page only,
    /// with enough points).
    pub trend: Option<Sparkline>,
}

/// A trend drawn as a small line (`atoms/sparkline.html`): its points in a
/// `SPARK_W` × `SPARK_H` box, x by the day (a day without a price is a gap
/// in the spacing, not hidden), y from the lowest to the highest middle; the
/// last point marked; and the line in words for a screen reader.
#[derive(Debug, Serialize)]
pub struct Sparkline {
    pub view_box: String,
    pub points: String,
    pub last_x: String,
    pub last_y: String,
    pub label: String,
}

/// The sparkline's box, in its own units (its size on the page is CSS's,
/// the same ratio, so the dot stays round).
const SPARK_W: f64 = 100.0;
const SPARK_H: f64 = 25.0;
/// Room above and below, so the line's stroke and the dot are not cut.
const SPARK_PAD: f64 = 3.0;

/// The drawing of `trend` as of `today`; `None` without points. `label`
/// words it from its lowest and highest middle.
fn sparkline(
    trend: &PriceTrendDto,
    today: NaiveDate,
    label: impl Fn(i64, i64) -> String,
) -> Option<Sparkline> {
    let low = trend.points.iter().map(|p| p.middle).min()?;
    let high = trend.points.iter().map(|p| p.middle).max()?;
    let first_day = today - Duration::days(PRICE_TREND_DAYS - 1);
    let span = (PRICE_TREND_DAYS - 1) as f64;
    let at = |day: NaiveDate, middle: i64| {
        let x =
            ((day - first_day).num_days().clamp(0, PRICE_TREND_DAYS - 1) as f64) / span * SPARK_W;
        let y = if high == low {
            SPARK_H / 2.0
        } else {
            SPARK_H
                - SPARK_PAD
                - (middle - low) as f64 / (high - low) as f64 * (SPARK_H - 2.0 * SPARK_PAD)
        };
        (x, y)
    };
    let drawn: Vec<(f64, f64)> = trend.points.iter().map(|p| at(p.day, p.middle)).collect();
    let (last_x, last_y) = *drawn.last()?;
    Some(Sparkline {
        view_box: format!("0 0 {SPARK_W} {SPARK_H}"),
        points: drawn
            .iter()
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" "),
        last_x: format!("{last_x:.1}"),
        last_y: format!("{last_y:.1}"),
        label: label(low, high),
    })
}

/// A price as read: one figure in đồng, or low – high.
fn range_text(low: i64, high: i64) -> String {
    if low == high {
        format_vnd(low)
    } else {
        format!("{} – {}", format_number(low), format_vnd(high))
    }
}

impl PriceLine {
    pub fn new(quote: PriceQuoteDto, today: NaiveDate) -> Self {
        let direction = match quote.change {
            Some(c) if c > 0 => "up",
            Some(c) if c < 0 => "down",
            Some(_) => "same",
            None => "",
        };
        // Never after today; a negative age would only come from a clock
        // skew and reads as today.
        let days_ago = (today - quote.observed_on).num_days().max(0);
        Self {
            direction,
            change_abs: quote.change.map_or(0, i64::abs),
            days_ago,
            is_stale: days_ago > PRICE_STALE_DAYS,
            range: range_text(quote.price_low, quote.price_high),
            trend: None,
            quote,
        }
    }
}

/// The price page's trends, by crop and area.
type Trends = HashMap<(i32, i32), PriceTrendDto>;

#[derive(Debug, Serialize)]
pub struct CropCard {
    pub name: String,
    pub unit: String,
    pub lines: Vec<PriceLine>,
    /// What its share button sends: its prices in words, so the message
    /// read in a Zalo group says them before anyone opens the link.
    pub share_text: String,
    /// The board of its commodity, the link shared.
    pub share_url: String,
}

/// "Giá Lúa OM 18 tại nơi sản xuất: Cai Lậy 6.200 – 6.400 ₫/kg (22/11); …"
fn share_text(state: &AppState, crop: &CropBoardDto, lines: &[PriceLine]) -> String {
    let locale = state.locale();
    let lines: Vec<String> = lines
        .iter()
        .map(|l| {
            locale.text_with(
                "prices.share_line",
                &[
                    ("area", &l.quote.location_label),
                    ("price", &l.range),
                    ("unit", &l.quote.unit),
                    ("day", &l.quote.observed_on.format("%d/%m")),
                ],
            )
        })
        .collect();
    locale.text_with(
        "prices.share_text",
        &[("crop", &crop.name), ("lines", &lines.join("; "))],
    )
}

/// A commodity: its title, its icon, its items with a price, and the way to
/// the supplies for it.
#[derive(Debug, Serialize)]
pub struct PriceSection {
    pub title: String,
    pub icon: String,
    pub crops: Vec<CropCard>,
    /// `None` while supplies are hidden from this visitor.
    pub supplies_href: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PricesPage {
    pub sections: Vec<PriceSection>,
    /// The commodities with a price, every one first; none with fewer
    /// than two to choose from.
    pub chips: Vec<CommodityChip>,
    /// The commodity the board is narrowed to, kept by the area form.
    pub commodity_id: Option<i32>,
    pub areas: Vec<NamedDto>,
    /// The chosen area, when it has prices.
    pub area_id: Option<i32>,
    /// Every area at once: each price line names its area.
    pub show_area: bool,
    pub area_name: Option<String>,
    /// The visitor's sector, by name, when one narrows the board: an empty
    /// board then says so and offers every sector.
    pub sector_name: Option<String>,
    /// The area box takes typing (`PRICE_AREA_SEARCH_MIN` areas or more).
    pub area_search: bool,
    pub action: String,
    /// Every area, said outright (`?area=`), so the remembered one is
    /// forgotten rather than opened again.
    pub every_area_href: String,
    pub back_href: String,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("price links: {e}")))
}

/// The produce of `group` as cards; `None` when there is none. `supplies`:
/// the visitor sees supplies (the way to those for it).
fn section(
    state: &AppState,
    supplies: bool,
    group: &CommodityDto,
    crops: &[CropBoardDto],
    trends: &Trends,
    today: NaiveDate,
) -> Result<Option<PriceSection>, Error> {
    let share_url = commodity_href(group.id)?;
    let locale = state.locale();
    let worded = |low: i64, high: i64| {
        locale.text_with(
            "prices.trend",
            &[
                ("days", &PRICE_TREND_DAYS),
                ("low", &format_vnd(low)),
                ("high", &format_vnd(high)),
            ],
        )
    };
    let cards: Vec<CropCard> = crops
        .iter()
        .filter(|c| c.commodity_id == group.id)
        .map(|c| {
            let lines: Vec<PriceLine> = c
                .quotes
                .iter()
                .cloned()
                .map(|q| {
                    let trend = trends
                        .get(&(q.crop_id, q.location_id))
                        .and_then(|t| sparkline(t, today, worded));
                    PriceLine {
                        trend,
                        ..PriceLine::new(q, today)
                    }
                })
                .collect();
            CropCard {
                name: c.name.clone(),
                unit: c.unit.clone(),
                share_text: share_text(state, c, &lines),
                share_url: share_url.clone(),
                lines,
            }
        })
        .collect();
    if cards.is_empty() {
        return Ok(None);
    }
    Ok(Some(PriceSection {
        title: group.name.clone(),
        icon: group.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        crops: cards,
        supplies_href: match supplies {
            true => Some(with_query(
                &url("products")?,
                &[("commodity", &group.id.to_string())],
            )),
            false => None,
        },
    }))
}

/// The board's address narrowed to `commodity` (the area is the one
/// remembered, so it needs no parameter): what the chips and a supply for
/// that commodity link to.
pub fn commodity_href(commodity: i32) -> Result<String, Error> {
    Ok(with_query(
        &url("prices")?,
        &[("commodity", &commodity.to_string())],
    ))
}

/// The chips over the board: every commodity, then each one with a price;
/// none when there are fewer than two to choose from.
fn commodity_chips(
    state: &AppState,
    commodities: &[CommodityDto],
    chosen: Option<i32>,
) -> Result<Vec<CommodityChip>, Error> {
    if commodities.len() < 2 {
        return Ok(Vec::new());
    }
    let every = CommodityChip {
        name: state.locale().text("filters.all"),
        icon: None,
        href: url("prices")?,
        current: chosen.is_none(),
    };
    std::iter::once(Ok(every))
        .chain(commodities.iter().map(|g| {
            Ok(CommodityChip {
                name: g.name.clone(),
                icon: g.icon.clone(),
                href: commodity_href(g.id)?,
                current: chosen == Some(g.id),
            })
        }))
        .collect()
}

/// GET /prices
pub async fn show(
    shell: Shell,
    headers: HeaderMap,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<PricesQuery>,
) -> Result<Response, Error> {
    // Chosen now (kept or forgotten below), else the one remembered.
    let area = match &q.area {
        Some(raw) => parse_opt::<i32>(raw),
        None => remembered_area(&headers),
    };
    let prices = &state.usecases.price;
    // A commodity chosen is shown whatever the sector, as a category chosen
    // on the lists is (`listing::list_sector`).
    let requested = parse_opt::<i32>(&q.commodity);
    let sector = match requested {
        Some(_) => None,
        None => shell.sector.as_deref(),
    };
    let mut board = prices.board(area, sector).await?;
    let area_name = area.and_then(|id| {
        board
            .areas
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
    });
    // A remembered area with no fresh price left: every area instead of an
    // empty board the farmer did not ask for this time.
    let area = if area_name.is_none() && area.is_some() && q.area.is_none() {
        board = prices.board(None, sector).await?;
        None
    } else {
        area
    };
    // A commodity with no fresh price here (an old link): the board as if
    // none were chosen, the visitor's sector again.
    let commodity = requested.filter(|id| board.commodities.iter().any(|g| g.id == *id));
    if requested.is_some() && commodity.is_none() {
        board = prices.board(area, shell.sector.as_deref()).await?;
    }
    // Each line's last days, for the lines of this board's area.
    let trends: Trends = prices
        .trends(area)
        .await?
        .into_iter()
        .map(|t| ((t.crop_id, t.location_id), t))
        .collect();
    // One section per commodity with a price, in the commodities' order.
    let mut sections = Vec::new();
    for g in board
        .commodities
        .iter()
        .filter(|g| commodity.is_none_or(|id| id == g.id))
    {
        let supplies = shell.shows(Feature::Supplies);
        sections.extend(section(
            &state,
            supplies,
            g,
            &board.crops,
            &trends,
            board.today,
        )?);
    }
    let chips = commodity_chips(&state, &board.commodities, commodity)?;
    let sector_name = shell
        .sectors
        .iter()
        .find(|s| s.current && !s.value.is_empty())
        .map(|s| s.label.clone());
    let page = PricesPage {
        sections,
        chips,
        commodity_id: commodity,
        area_id: area,
        show_area: area_name.is_none(),
        area_name,
        sector_name,
        area_search: board.areas.len() >= PRICE_AREA_SEARCH_MIN,
        areas: board.areas,
        action: url("prices")?,
        every_area_href: format!("{}?area=", url("prices")?),
        back_href: url("home")?,
    };
    let mut response = render_page(&state, &shell, "pages/prices.html", &page)?;
    // Only a choice made now changes what is remembered: kept when it names
    // an area with prices, forgotten for every area.
    if q.area.is_some() {
        let kept = page.area_id.filter(|_| page.area_name.is_some());
        remember(
            &mut response,
            &state.cookies,
            PRICE_AREA_COOKIE,
            kept.map(|id| id.to_string()).as_deref(),
        );
    }
    Ok(response)
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
            previous_on: change.map(|_| on - chrono::Duration::days(1)),
        }
    }

    #[test]
    fn a_line_says_which_way_the_price_moved() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap_or_default();
        let down = PriceLine::new(quote(Some(-150), today), today);
        assert_eq!(
            (down.direction, down.change_abs, down.days_ago),
            ("down", 150, 0)
        );
        let first = PriceLine::new(quote(None, today - chrono::Duration::days(1)), today);
        assert_eq!((first.direction, first.days_ago), ("", 1));
        assert_eq!(
            PriceLine::new(quote(Some(0), today), today).direction,
            "same"
        );
    }

    #[test]
    fn an_older_price_is_faded_and_a_future_one_reads_as_today() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap_or_default();
        let at =
            |days: i64| PriceLine::new(quote(None, today - chrono::Duration::days(days)), today);
        assert!(!at(PRICE_STALE_DAYS).is_stale);
        assert!(at(PRICE_STALE_DAYS + 1).is_stale);
        assert_eq!(at(-1).days_ago, 0);
    }

    #[test]
    fn a_sparkline_spaces_its_points_by_day_and_spans_low_to_high() {
        use application::outputs::TrendPointDto;
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap_or_default();
        let point = |days_ago: i64, middle: i64| TrendPointDto {
            day: today - chrono::Duration::days(days_ago),
            middle,
        };
        let trend = PriceTrendDto {
            crop_id: 1,
            location_id: 1,
            points: vec![
                point(PRICE_TREND_DAYS - 1, 6_000),
                point(3, 6_100),
                point(0, 6_300),
            ],
        };
        // The window's first day at the left, the lowest at the bottom;
        // today at the right, the highest at the top.
        let drawn = sparkline(&trend, today, |low, high| format!("{low}-{high}"))
            .map(|s| (s.points, s.last_x, s.last_y, s.label));
        assert_eq!(
            drawn,
            Some((
                "0.0,22.0 76.9,15.7 100.0,3.0".to_owned(),
                "100.0".to_owned(),
                "3.0".to_owned(),
                "6000-6300".to_owned()
            ))
        );
        // A price that never moved: a flat line through the middle.
        let flat = PriceTrendDto {
            points: trend
                .points
                .iter()
                .map(|p| TrendPointDto {
                    middle: 5_000,
                    ..*p
                })
                .collect(),
            ..trend
        };
        let s = sparkline(&flat, today, |_, _| String::new());
        assert_eq!(s.map(|s| s.last_y), Some("12.5".to_owned()));
    }
}
