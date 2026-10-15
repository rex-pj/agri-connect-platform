//! View models shared by the list pages. Templates get display-ready values;
//! the formatting rules live here, where they can be tested.

use application::outputs::{BusinessBrowseDto, BusinessSearchDto};
use serde::Serialize;

/// What `organisms/provider_card.html` shows.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderCard {
    pub id: i32,
    pub name: String,
    pub phone: String,
    /// The last two parts of the address ("Cai Lậy, Tiền Giang"): enough to
    /// judge the distance without a long street line on a small screen.
    pub district: Option<String>,
    pub is_verified: bool,
    pub services: Vec<String>,
    /// "850 m" or "2.3 km (~5 phút xe máy)"; only on the near-me results.
    pub distance: Option<String>,
}

impl From<BusinessBrowseDto> for ProviderCard {
    fn from(b: BusinessBrowseDto) -> Self {
        Self {
            district: district(b.address.as_deref()),
            id: b.id,
            name: b.name,
            phone: b.phone,
            is_verified: b.is_verified,
            services: b.services,
            distance: None,
        }
    }
}

impl From<BusinessSearchDto> for ProviderCard {
    fn from(b: BusinessSearchDto) -> Self {
        Self {
            district: district(b.address.as_deref()),
            distance: Some(distance(b.distance_m)),
            id: b.id,
            name: b.name,
            phone: b.phone,
            is_verified: b.is_verified,
            services: b.services,
        }
    }
}

/// Typical motorbike speed on rural roads, for the "~N phút" estimate.
const MOTORBIKE_KMH: f64 = 30.0;

/// Metres under a kilometre, otherwise kilometres with one decimal and the
/// ride time: farmers judge a provider by "how long to get here".
fn distance(metres: f64) -> String {
    if metres < 1000.0 {
        return format!("{} m", metres.round());
    }
    let km = metres / 1000.0;
    let minutes = (km / MOTORBIKE_KMH * 60.0).round();
    format!("{km:.1} km (~{minutes} phút xe máy)")
}

fn district(address: Option<&str>) -> Option<String> {
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
    use super::district;

    #[test]
    fn distance_reads_as_metres_or_km_with_ride_time() {
        assert_eq!(super::distance(849.6), "850 m");
        assert_eq!(super::distance(2_300.0), "2.3 km (~5 phút xe máy)");
        assert_eq!(super::distance(15_000.0), "15.0 km (~30 phút xe máy)");
    }

    #[test]
    fn district_is_the_last_two_address_parts() {
        assert_eq!(
            district(Some("12 Quốc lộ 1, Ấp 3, Cai Lậy, Tiền Giang")),
            Some("Cai Lậy, Tiền Giang".into())
        );
        assert_eq!(district(Some("Long Xuyên")), Some("Long Xuyên".into()));
        assert_eq!(district(Some(" , ")), None);
        assert_eq!(district(None), None);
    }
}

/// One tile of `organisms/category_grid.html`.
#[derive(Debug, Serialize)]
pub struct CategoryTile {
    pub name: String,
    pub icon: String,
    pub href: String,
    pub active: bool,
}

pub fn tile(name: &str, icon: &str, href: String) -> CategoryTile {
    CategoryTile {
        name: name.to_owned(),
        icon: icon.to_owned(),
        href,
        active: false,
    }
}
