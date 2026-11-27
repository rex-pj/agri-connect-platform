//! GET / — the home page: category shortcuts in two labelled rows, one per
//! errand ("Thuê làm": services first, the farmer's main errand; "Mua vật
//! tư": supplies), the near-me banner, featured services (one per verified
//! provider), today's few crop prices and featured products.
//!
//! Always every sector's: the page has no sector switch (it is on the lists
//! and the price board), so a sector chosen there never narrows it unseen.
//! The price strip alone follows the area chosen on the price board
//! (`prices::remembered_area`), and its title then names that area.

use application::{
    constants::HOME_POSTS_SHOWN,
    features::Feature,
    outputs::{CategoryDto, ProductDto},
};
use axum::{extract::State, http::HeaderMap, response::Response};
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    models::post::PostQuery,
    Error,
};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::{
        HOME_PRICES_ON_PHONE, HOME_PRICES_SHOWN, HOME_PRODUCT_COUNT, HOME_SERVICES_ON_PHONE,
        HOME_TILES_PHONE, HOME_TILES_WIDE,
    },
    labels::{alert_reach, SECTOR_ICON_DEFAULT},
    pages::{
        articles::{cards, ArticleCard},
        cards::{fold_of, folded_count, rated_services, tile, CategoryTile, ServiceCard},
        prices::{remembered_area, PriceLine},
    },
    routes,
    shell::{Link, ProviderCta, Shell},
    views::render_page,
};

#[derive(Serialize)]
pub struct HomePage {
    /// The alerts in force, every sector's, first on the page.
    pub alerts: Vec<AlertCard>,
    /// The category shortcuts: a row per errand (hire, buy), each named.
    pub rows: Vec<TileRow>,
    pub services: Vec<ServiceCard>,
    pub products: Vec<ProductDto>,
    /// "Found no one? Post the job": the way to the job form; none while job
    /// posts are paused.
    pub job_cta: Option<ProviderCta>,
    /// The few newest prices (each crop's), under the near-me banner.
    /// On a phone the only way to the price board, so above the services.
    pub prices: Vec<PriceLine>,
    /// The area of those prices when they are the one the visitor chose on
    /// the price board (`prices::remembered_area`): the strip's title names
    /// it and its lines leave it out.
    pub prices_area: Option<String>,
    /// How many of those prices a phone shows (the rest from 768 px).
    pub prices_on_phone: usize,
    /// How many featured services a phone shows (the rest from 768 px).
    pub services_on_phone: usize,
    /// The newest articles, every sector's, while the part is shown.
    pub articles: Vec<ArticleCard>,
}

/// One row of category tiles and what it is for ("Thuê làm", "Mua vật tư"),
/// folded to one line until its "+N" toggle opens it.
#[derive(Serialize)]
pub struct TileRow {
    pub title: String,
    /// `service` / `product`: the tiles' kind, and the toggle's id.
    pub kind: &'static str,
    /// The kind's whole list, from the row's header.
    pub all_href: String,
    /// Tiles hidden while folded below 992 px (0: no toggle there).
    pub folded_phone: usize,
    /// The same from 992 px.
    pub folded_wide: usize,
    pub tiles: Vec<CategoryTile>,
}

/// An alert in force, as the home page shows it.
#[derive(Serialize)]
pub struct AlertCard {
    pub icon: String,
    pub title: String,
    pub body: String,
    /// For whom and where: "Chăn nuôi · Đồng Tháp".
    pub reach: String,
    /// Its last day.
    pub until: chrono::NaiveDate,
}

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("home links: {e}")))
}

/// Every category of `kind` as a row (`title`), each tile linking to its
/// listing (`route`), the row's header to the kind's whole list
/// (`all_route`). Folded, the row is one line (`HOME_TILES_PHONE` places,
/// `HOME_TILES_WIDE` from 992 px), its last place the "+N" toggle when the
/// rest do not fit; opened, every tile. No categories, no row.
fn category_row(
    state: &AppState,
    title: &str,
    categories: &[CategoryDto],
    kind: &'static str,
    route: &str,
    all_route: &str,
) -> Result<Option<TileRow>, Error> {
    if categories.is_empty() {
        return Ok(None);
    }
    let total = categories.len();
    let tiles = categories
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let href = url(route, &[("id", &c.id.to_string()), ("slug", &c.slug)])?;
            Ok(CategoryTile {
                kind,
                fold: fold_of(i, total, HOME_TILES_PHONE, HOME_TILES_WIDE),
                ..tile(&c.name, c.icon.as_deref().unwrap_or("fa-tag"), href)
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Some(TileRow {
        title: state.locale().text(title),
        kind,
        all_href: url(all_route, &[])?,
        folded_phone: total - folded_count(total, HOME_TILES_PHONE),
        folded_wide: total - folded_count(total, HOME_TILES_WIDE),
        tiles,
    }))
}

pub async fn show(
    shell: Shell,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let area = remembered_area(&headers);
    // A hidden feature is not read at all: its part of the page is empty,
    // and an empty part is not drawn.
    let supplies = shell.shows(Feature::Supplies);
    let price = &state.usecases.price;
    let show_prices = shell.shows(Feature::Prices);
    let highlights = move |area| async move {
        match show_prices {
            true => price
                .highlights(HOME_PRICES_SHOWN, area, None)
                .await
                .map(Some),
            false => Ok(None),
        }
    };
    let (featured, service_categories, product_categories, prices, alerts, sectors) = tokio::try_join!(
        explore.get_explore_data(None),
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        async {
            match supplies {
                true => explore.get_categories(Some(CATEGORY_TYPE_PRODUCT)).await,
                false => Ok(Vec::new()),
            }
        },
        highlights(area),
        state.usecases.alert.live(None),
        state.usecases.sector.named(),
    )?;
    // The visitor's area, named by its prices; none fresh there: every area's.
    let prices_area = area
        .and_then(|_| prices.as_ref()?.quotes.first())
        .map(|q| q.location_label.clone());
    let prices = match prices {
        Some(p) if area.is_some() && p.quotes.is_empty() => highlights(None).await?,
        other => other,
    };

    // The newest articles: read only while the part is shown.
    let articles = if shell.shows(Feature::Articles) {
        cards(
            state
                .usecases
                .post
                .latest(&PostQuery::default(), HOME_POSTS_SHOWN)
                .await?,
        )?
    } else {
        Vec::new()
    };

    // Near-me needs no tile: it has a tab and the banner under the rows.
    let rows = [
        category_row(
            &state,
            "home.row_services",
            &service_categories,
            CATEGORY_TYPE_SERVICE,
            "service_category",
            "services",
        )?,
        category_row(
            &state,
            "home.row_supplies",
            &product_categories,
            CATEGORY_TYPE_PRODUCT,
            "product_category",
            "products",
        )?,
    ]
    .into_iter()
    .flatten()
    .collect();

    let page = HomePage {
        alerts: alerts
            .iter()
            .map(|a| AlertCard {
                icon: a
                    .kind_icon
                    .clone()
                    .unwrap_or_else(|| SECTOR_ICON_DEFAULT.to_owned()),
                reach: alert_reach(state.locale(), &sectors, a),
                until: a.ends_on,
                title: a.title.clone(),
                body: a.body.clone(),
            })
            .collect(),
        rows,
        // Not offered while hire requests are hidden from this visitor.
        job_cta: shell.shows(Feature::Jobs).then_some(ProviderCta {
            link: Link {
                href: url("job_new", &[])?,
                icon: "fa-bullhorn",
                label: state.locale().text("job.cta_button"),
            },
            title: state.locale().text("job.cta_title"),
            sub: state.locale().text("job.cta_sub"),
        }),
        prices: prices
            .map(|p| {
                p.quotes
                    .into_iter()
                    .map(|q| PriceLine::new(q, p.today))
                    .collect()
            })
            .unwrap_or_default(),
        prices_area,
        prices_on_phone: HOME_PRICES_ON_PHONE,
        services_on_phone: HOME_SERVICES_ON_PHONE,
        articles,
        services: rated_services(&state, &shell, featured.featured_services).await?,
        products: featured
            .featured_products
            .into_iter()
            .take(if supplies { HOME_PRODUCT_COUNT } else { 0 })
            .collect(),
    };
    render_page(&state, &shell, "pages/home.html", &page)
}
