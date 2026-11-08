//! Areas as the demand report and partner contracts see them: a province, a
//! ward of one, or nowhere (a business with no position in reach of a ward).
//! A lead in a ward counts under that ward and under its province.
//!
//! An area travels in URLs and forms as text: `p82` (a province), `w28439`
//! (a ward), `none` (no area), so one select or `?area=` carries any of them.

use std::collections::{BTreeMap, HashMap};

use domain::{
    models::{
        admin_unit::{Province, Ward},
        partner::ContractArea,
    },
    repositories::admin_unit_repository_trait::AdminUnitRepositoryTrait,
    Result,
};

use crate::{
    outputs::{AreaChoiceDto, AreaGroupDto},
    slug::slugify,
};

/// The text of a province area: `p` and its code.
pub const PROVINCE_PREFIX: &str = "p";
/// The text of a ward area: `w` and its code.
pub const WARD_PREFIX: &str = "w";
/// The text of "no area".
pub const UNPLACED_AREA: &str = "none";
/// Official codes are two (province) or five (ward) digits.
const CODE_MAX: usize = 5;

/// Where a lead counts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AreaKey {
    Province(String),
    Ward(String),
    Unplaced,
}

impl AreaKey {
    /// The area a text names; anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text == UNPLACED_AREA {
            return Some(Self::Unplaced);
        }
        let code = |prefix: &str| {
            text.strip_prefix(prefix)
                .filter(|c| !c.is_empty() && c.len() <= CODE_MAX)
                .filter(|c| c.bytes().all(|b| b.is_ascii_digit()))
                .map(str::to_owned)
        };
        code(PROVINCE_PREFIX)
            .map(Self::Province)
            .or_else(|| code(WARD_PREFIX).map(Self::Ward))
    }

    /// The area as text, the inverse of `parse`.
    pub fn value(&self) -> String {
        match self {
            Self::Province(code) => format!("{PROVINCE_PREFIX}{code}"),
            Self::Ward(code) => format!("{WARD_PREFIX}{code}"),
            Self::Unplaced => UNPLACED_AREA.to_owned(),
        }
    }

    /// The area of a contract.
    pub fn of_contract(area: &ContractArea) -> Self {
        area.ward_code.as_ref().map_or_else(
            || Self::Province(area.province_code.clone()),
            |w| Self::Ward(w.clone()),
        )
    }
}

/// Every province and ward (without boundaries): names, and which province
/// a ward belongs to.
pub struct AdminUnits {
    /// By code: the official order, north to south.
    provinces: BTreeMap<String, Province>,
    wards: HashMap<String, Ward>,
    /// Each province's ward codes, in alphabetical order of their names.
    wards_by_province: HashMap<String, Vec<String>>,
}

impl AdminUnits {
    pub fn new(provinces: Vec<Province>, wards: Vec<Ward>) -> Self {
        let mut wards_by_province: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for w in &wards {
            wards_by_province
                .entry(w.province_code.clone())
                .or_default()
                .push((slugify(&w.full_name), w.code.clone()));
        }
        let wards_by_province = wards_by_province
            .into_iter()
            .map(|(province, mut list)| {
                list.sort();
                (province, list.into_iter().map(|(_, code)| code).collect())
            })
            .collect();
        Self {
            provinces: provinces.into_iter().map(|p| (p.code.clone(), p)).collect(),
            wards: wards.into_iter().map(|w| (w.code.clone(), w)).collect(),
            wards_by_province,
        }
    }

    pub async fn load(repo: &(dyn AdminUnitRepositoryTrait + Send + Sync)) -> Result<Self> {
        let (provinces, wards) = tokio::try_join!(repo.provinces(), repo.wards())?;
        Ok(Self::new(provinces, wards))
    }

    /// Provinces in the official order.
    pub fn provinces(&self) -> impl Iterator<Item = &Province> {
        self.provinces.values()
    }

    /// A province's wards, by name.
    pub fn wards_of(&self, province_code: &str) -> impl Iterator<Item = &Ward> {
        self.wards_by_province
            .get(province_code)
            .into_iter()
            .flatten()
            .filter_map(|code| self.wards.get(code))
    }

    pub fn province_of(&self, ward_code: &str) -> Option<&str> {
        self.wards.get(ward_code).map(|w| w.province_code.as_str())
    }

    /// The areas a lead in this ward counts under: the ward and its
    /// province; no ward is "no area".
    pub fn keys_of(&self, ward_code: Option<&str>) -> Vec<AreaKey> {
        match ward_code {
            None => vec![AreaKey::Unplaced],
            Some(w) => {
                let mut keys = vec![AreaKey::Ward(w.to_owned())];
                if let Some(p) = self.province_of(w) {
                    keys.push(AreaKey::Province(p.to_owned()));
                }
                keys
            }
        }
    }

    /// Whether `area` lies within one of `scope` (itself, or a ward of a
    /// province there).
    pub fn within(&self, area: &AreaKey, scope: &[AreaKey]) -> bool {
        match area {
            AreaKey::Ward(w) => self.keys_of(Some(w)).iter().any(|k| scope.contains(k)),
            other => scope.contains(other),
        }
    }

    /// An area's name: "Đồng Tháp", "Đồng Tháp — Phường Cai Lậy"; `None`
    /// for "no area" or a code that is not known.
    pub fn label(&self, area: &AreaKey) -> Option<String> {
        match area {
            AreaKey::Province(code) => self.provinces.get(code).map(|p| p.name.clone()),
            AreaKey::Ward(code) => {
                self.wards
                    .get(code)
                    .map(|w| match self.provinces.get(&w.province_code) {
                        Some(p) => format!("{} — {}", p.name, w.full_name),
                        None => w.full_name.clone(),
                    })
            }
            AreaKey::Unplaced => None,
        }
    }

    /// A ward's own name, without its province ("Phường Cai Lậy").
    pub fn ward_name(&self, code: &str) -> Option<&str> {
        self.wards.get(code).map(|w| w.full_name.as_str())
    }

    /// The contract area an area names, if it is an active province or an
    /// active ward of an active province.
    pub fn contract_area(&self, area: &AreaKey) -> Option<ContractArea> {
        let active_province = |code: &str| self.provinces.get(code).is_some_and(|p| p.is_active);
        match area {
            AreaKey::Province(code) if active_province(code) => Some(ContractArea {
                province_code: code.clone(),
                ward_code: None,
            }),
            AreaKey::Ward(code) => self
                .wards
                .get(code)
                .filter(|w| w.is_active && active_province(&w.province_code))
                .map(|w| ContractArea {
                    province_code: w.province_code.clone(),
                    ward_code: Some(w.code.clone()),
                }),
            _ => None,
        }
    }

    /// What a contract form offers: each active province (the whole of it),
    /// then its active wards.
    pub fn choices(&self) -> Vec<AreaGroupDto> {
        self.provinces()
            .filter(|p| p.is_active)
            .map(|p| AreaGroupDto {
                value: AreaKey::Province(p.code.clone()).value(),
                label: p.name.clone(),
                wards: self
                    .wards_of(&p.code)
                    .filter(|w| w.is_active)
                    .map(|w| AreaChoiceDto {
                        value: AreaKey::Ward(w.code.clone()).value(),
                        label: w.full_name.clone(),
                    })
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn province(code: &str, name: &str) -> Province {
        Province {
            code: code.into(),
            name: name.into(),
            full_name: format!("Tỉnh {name}"),
            unit_type: "province".into(),
            lat: 0.0,
            lng: 0.0,
            is_active: true,
        }
    }

    pub fn ward(code: &str, province_code: &str, full_name: &str) -> Ward {
        Ward {
            code: code.into(),
            province_code: province_code.into(),
            name: full_name.into(),
            full_name: full_name.into(),
            unit_type: "ward".into(),
            lat: 0.0,
            lng: 0.0,
            is_active: true,
        }
    }

    /// Two provinces; 82 has two wards (named out of code order), 91 one.
    pub fn units() -> AdminUnits {
        AdminUnits::new(
            vec![province("91", "An Giang"), province("82", "Đồng Tháp")],
            vec![
                ward("28439", "82", "Phường Cai Lậy"),
                ward("28400", "82", "Xã Tân Phước"),
                ward("28441", "82", "Xã Bình Phú"),
                ward("30001", "91", "Phường Long Xuyên"),
            ],
        )
    }

    #[test]
    fn an_area_reads_back_what_it_writes() {
        for key in [
            AreaKey::Province("82".into()),
            AreaKey::Ward("28439".into()),
            AreaKey::Unplaced,
        ] {
            assert_eq!(AreaKey::parse(&key.value()), Some(key));
        }
        for junk in ["", "82", "p", "w", "px2", "p123456", "12", "w28439x"] {
            assert_eq!(AreaKey::parse(junk), None, "{junk}");
        }
    }

    #[test]
    fn a_ward_counts_under_itself_and_its_province() {
        let u = units();
        assert_eq!(
            u.keys_of(Some("28439")),
            vec![
                AreaKey::Ward("28439".into()),
                AreaKey::Province("82".into())
            ]
        );
        assert_eq!(u.keys_of(None), vec![AreaKey::Unplaced]);
        let scope = [AreaKey::Province("82".into())];
        assert!(u.within(&AreaKey::Ward("28439".into()), &scope));
        assert!(!u.within(&AreaKey::Ward("30001".into()), &scope));
        assert!(!u.within(&AreaKey::Province("91".into()), &scope));
    }

    #[test]
    fn labels_and_choices_follow_the_official_and_alphabetical_order() {
        let u = units();
        assert_eq!(
            u.label(&AreaKey::Ward("28439".into())).as_deref(),
            Some("Đồng Tháp — Phường Cai Lậy")
        );
        assert_eq!(
            u.label(&AreaKey::Province("82".into())).as_deref(),
            Some("Đồng Tháp")
        );
        let choices = u.choices();
        let provinces: Vec<_> = choices.iter().map(|g| g.value.as_str()).collect();
        assert_eq!(provinces, vec!["p82", "p91"], "by code");
        let wards: Vec<_> = choices
            .first()
            .map(|g| g.wards.iter().map(|w| w.label.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(
            wards,
            vec!["Phường Cai Lậy", "Xã Bình Phú", "Xã Tân Phước"],
            "by name, accents ignored"
        );
    }

    #[test]
    fn a_contract_area_is_an_active_unit() {
        let mut u = units();
        assert_eq!(
            u.contract_area(&AreaKey::Ward("28439".into())),
            Some(ContractArea {
                province_code: "82".into(),
                ward_code: Some("28439".into())
            })
        );
        assert_eq!(u.contract_area(&AreaKey::Unplaced), None);
        assert_eq!(u.contract_area(&AreaKey::Ward("99999".into())), None);
        if let Some(w) = u.wards.get_mut("28439") {
            w.is_active = false;
        }
        assert_eq!(u.contract_area(&AreaKey::Ward("28439".into())), None);
    }
}
