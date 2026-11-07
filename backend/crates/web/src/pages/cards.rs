//! View models shared by the list pages. Templates get display-ready values;
//! the formatting rules live here, where they can be tested.

use application::outputs::{BusinessBrowseDto, BusinessSearchDto, PublicServiceDto, RatingDto};
use domain::Error;
use serde::Serialize;
use uuid::Uuid;

use crate::{app_state::AppState, pages::canonical_url};

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
    services: Vec<PublicServiceDto>,
) -> Result<Vec<ServiceCard>, Error> {
    let mut cards = ServiceCard::all(services)?;
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
    mut cards: Vec<ProviderCard>,
) -> Result<Vec<ProviderCard>, Error> {
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
    /// Shown from 992 px only, where the row has room for one more tile.
    pub wide_only: bool,
    /// `service` or `product` (a category's type: its shape and colour), or
    /// blank.
    pub kind: &'static str,
    /// The "all" tile ending a kind's run: outlined, not filled.
    pub more: bool,
}

pub fn tile(name: &str, icon: &str, href: String) -> CategoryTile {
    CategoryTile {
        name: name.to_owned(),
        icon: icon.to_owned(),
        href,
        active: false,
        wide_only: false,
        kind: "",
        more: false,
    }
}
