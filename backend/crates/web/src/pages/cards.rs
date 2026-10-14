//! View models shared by the list pages. Templates get display-ready values;
//! the formatting rules live here, where they can be tested.

use application::outputs::BusinessBrowseDto;
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
        }
    }
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
