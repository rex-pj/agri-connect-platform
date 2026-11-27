//! View models shared by the list pages. Templates get display-ready values;
//! the formatting rules live here, where they can be tested.

use application::{
    features::Feature,
    outputs::{BusinessBrowseDto, BusinessSearchDto, CategoryDto, PublicServiceDto, RatingDto},
};
use domain::Error;
use serde::Serialize;
use uuid::Uuid;

use crate::{app_state::AppState, pages::canonical_url, shell::Shell};

/// What `organisms/provider_card.html` shows.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProviderCard {
    pub id: i32,
    pub name: String,
    pub phone: String,
    /// The last two parts of the address, the commune or ward and the
    /// province ("phường Cai Lậy, Đồng Tháp"; no districts since July 2025):
    /// enough to judge the distance without a long street line on a small
    /// screen.
    pub locality: Option<String>,
    pub is_verified: bool,
    pub services: Vec<String>,
    /// Metres away, only on the near-me results; the template words it
    /// (`distance` filter).
    pub distance_m: Option<f64>,
    /// Its shown reviews, once any; set by `rated_providers`.
    pub rating: Option<RatingDto>,
}

impl From<BusinessBrowseDto> for ProviderCard {
    fn from(b: BusinessBrowseDto) -> Self {
        Self {
            locality: locality(b.address.as_deref()),
            id: b.id,
            name: b.name,
            phone: b.phone,
            is_verified: b.is_verified,
            services: b.services,
            rating: None,
            distance_m: None,
        }
    }
}

impl From<BusinessSearchDto> for ProviderCard {
    fn from(b: BusinessSearchDto) -> Self {
        Self {
            locality: locality(b.address.as_deref()),
            distance_m: Some(b.distance_m),
            id: b.id,
            name: b.name,
            phone: b.phone,
            is_verified: b.is_verified,
            services: b.services,
            rating: None,
        }
    }
}

/// What `organisms/service_card.html` shows: the service first (photo, name,
/// price), then who offers it and how to reach them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServiceCard {
    pub id: i32,
    pub name: String,
    /// The service's own page.
    pub href: String,
    pub category: String,
    pub category_icon: Option<String>,
    /// The category label is left out when the name already says it (a
    /// blank name took the category's), so "Phun thuốc" is not read twice.
    pub show_category: bool,
    pub price_note: Option<String>,
    pub image_file_id: Option<Uuid>,
    pub provider_name: String,
    pub provider_verified: bool,
    pub phone: String,
    pub locality: Option<String>,
    /// Only on the near-me results.
    pub distance_m: Option<f64>,
    /// The provider's shown reviews, once any; set by `rated_services`.
    pub rating: Option<RatingDto>,
    #[serde(skip)]
    pub business_id: i32,
}

impl ServiceCard {
    pub fn new(s: PublicServiceDto) -> Result<Self, Error> {
        Ok(Self {
            href: canonical_url("service", s.id, &s.name)?,
            show_category: !same_label(&s.name, &s.category),
            locality: locality(s.business_address.as_deref()),
            distance_m: s.distance_m,
            id: s.id,
            name: s.name,
            category: s.category,
            category_icon: s.category_icon,
            price_note: s.price_note,
            image_file_id: s.image_file_id,
            provider_name: s.business_name,
            provider_verified: s.business_verified,
            phone: s.business_phone,
            business_id: s.business_id,
            rating: None,
        })
    }

    pub fn all(services: Vec<PublicServiceDto>) -> Result<Vec<Self>, Error> {
        services.into_iter().map(Self::new).collect()
    }
}

/// Service cards with their providers' ratings, read in one query.
pub async fn rated_services(
    state: &AppState,
    shell: &Shell,
    services: Vec<PublicServiceDto>,
) -> Result<Vec<ServiceCard>, Error> {
    let mut cards = ServiceCard::all(services)?;
    if !shell.shows(Feature::Reviews) {
        return Ok(cards);
    }
    let ids: Vec<i32> = cards.iter().map(|c| c.business_id).collect();
    let ratings = state.usecases.review.ratings(&ids).await?;
    for card in &mut cards {
        card.rating = ratings.get(&card.business_id).copied();
    }
    Ok(cards)
}

/// Provider cards with their ratings, read in one query.
pub async fn rated_providers(
    state: &AppState,
    shell: &Shell,
    mut cards: Vec<ProviderCard>,
) -> Result<Vec<ProviderCard>, Error> {
    if !shell.shows(Feature::Reviews) {
        return Ok(cards);
    }
    let ids: Vec<i32> = cards.iter().map(|c| c.id).collect();
    let ratings = state.usecases.review.ratings(&ids).await?;
    for card in &mut cards {
        card.rating = ratings.get(&card.id).copied();
    }
    Ok(cards)
}

/// True when a service's name is just its category's ("Phun thuốc" in
/// "Phun thuốc"), ignoring case and surrounding spaces.
pub fn same_label(name: &str, category: &str) -> bool {
    name.trim().to_lowercase() == category.trim().to_lowercase()
}

fn locality(address: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = address?
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    match parts.as_slice() {
        [] => None,
        [only] => Some((*only).to_owned()),
        [.., a, b] => Some(format!("{a}, {b}")),
    }
}

#[cfg(test)]
mod tests {
    use super::locality;

    #[test]
    fn a_name_that_is_its_category_is_not_labelled_twice() {
        assert!(super::same_label("Phun thuốc", "Phun thuốc"));
        assert!(super::same_label(" phun THUỐC ", "Phun thuốc"));
        assert!(!super::same_label("Phun thuốc drone", "Phun thuốc"));
    }

    #[test]
    fn locality_is_the_last_two_address_parts() {
        assert_eq!(
            locality(Some("12 Quốc lộ 1, Khu phố 3, phường Cai Lậy, Đồng Tháp")),
            Some("phường Cai Lậy, Đồng Tháp".into())
        );
        assert_eq!(locality(Some("Long Xuyên")), Some("Long Xuyên".into()));
        assert_eq!(locality(Some(" , ")), None);
        assert_eq!(locality(None), None);
    }
}

/// One tile of `organisms/category_grid.html`.
#[derive(Debug, Serialize)]
pub struct CategoryTile {
    pub name: String,
    pub icon: String,
    pub href: String,
    pub active: bool,
    /// `service` or `product` (a category's type: its colour and shape), or
    /// blank.
    pub kind: &'static str,
    /// Where the tile waits behind its row's "+N" toggle.
    pub fold: Fold,
    /// The category's own colour (`#rrggbb`), its icon's only: the tile's
    /// background and shape stay its kind's, so the two errands stay apart.
    pub color: Option<String>,
}

/// Whether a tile of a folding row (the home page's) shows before the row
/// is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fold {
    /// Always shown.
    Shown,
    /// Shown folded from 992 px only, where the row has more room.
    Phone,
    /// Shown only once the row is opened.
    All,
}

/// A category's tile, leading to `href`: its name, icon (a tag when it has
/// none) and colour.
pub fn tile(c: &CategoryDto, href: String) -> CategoryTile {
    CategoryTile {
        name: c.name.clone(),
        icon: c.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        href,
        active: false,
        kind: "",
        fold: Fold::Shown,
        color: c.color.clone(),
    }
}

/// How many tiles a folded row of `per_row` places shows out of `total`:
/// all when they fit, else one place fewer, the last place being the toggle.
pub fn folded_count(total: usize, per_row: usize) -> usize {
    if total <= per_row {
        total
    } else {
        per_row.saturating_sub(1)
    }
}

/// The fold of the `i`th of `total` tiles in a row of `phone` places below
/// 992 px and `wide` from there (`phone` ≤ `wide`).
pub fn fold_of(i: usize, total: usize, phone: usize, wide: usize) -> Fold {
    if i < folded_count(total, phone) {
        Fold::Shown
    } else if i < folded_count(total, wide) {
        Fold::Phone
    } else {
        Fold::All
    }
}

#[cfg(test)]
mod fold_tests {
    use super::{fold_of, folded_count, Fold};

    #[test]
    fn a_row_that_fits_folds_nothing() {
        assert_eq!(folded_count(4, 4), 4);
        assert!((0..4).all(|i| fold_of(i, 4, 4, 5) == Fold::Shown));
    }

    #[test]
    fn a_longer_row_keeps_its_last_place_for_the_toggle() {
        assert_eq!(folded_count(9, 4), 3);
        assert_eq!(folded_count(9, 5), 4);
        let folds: Vec<_> = (0..6).map(|i| fold_of(i, 9, 4, 5)).collect();
        assert_eq!(
            folds,
            [
                Fold::Shown,
                Fold::Shown,
                Fold::Shown,
                Fold::Phone,
                Fold::All,
                Fold::All
            ]
        );
        // Five fit a wide row but not a phone's: the phone folds two.
        assert_eq!(fold_of(4, 5, 4, 5), Fold::Phone);
    }
}
