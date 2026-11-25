//! Where a business is, without anyone typing coordinates: the ward a point
//! lies in, said by name; the ward chosen from the list when no point was
//! taken on the spot (its centre, approximate); and the map's tiles
//! (`MapTiles`) a provider drags the pin on. Nothing here needs a session:
//! the wards and the map are public knowledge.

use std::{f64::consts::PI, sync::Arc};

use domain::{
    models::admin_unit::Ward, repositories::admin_unit_repository_trait::AdminUnitRepositoryTrait,
    Error, ErrorCode, Result,
};

use crate::{
    admin_units::{ward_label, AdminUnits, AreaKey},
    constants::{
        ADMIN_UNIT_SNAP_M, MAP_REGION_LAT_MAX, MAP_REGION_LAT_MIN, MAP_REGION_LNG_MAX,
        MAP_REGION_LNG_MIN, MAP_ZOOM_MAX, MAP_ZOOM_MIN, WARD_CENTRE_TOLERANCE_DEG,
        WARD_SEARCH_RESULTS,
    },
    inputs::UpdateProfileInput,
    outputs::{PlaceDto, PlaceFormDto, WardMatchDto, WardMatchesDto},
    ports::MapTiles,
    validation::{search_text, validate_coordinates},
};

#[derive(Clone)]
pub struct PlaceUseCase {
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    tiles: Arc<dyn MapTiles>,
}

impl PlaceUseCase {
    pub fn new(
        admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
        tiles: Arc<dyn MapTiles>,
    ) -> Self {
        Self {
            admin_unit_repo,
            tiles,
        }
    }

    /// What a location box shows: every active ward to choose from, and
    /// where the business is now (its stored ward and point).
    pub async fn form(
        &self,
        ward_code: Option<&str>,
        point: Option<(f64, f64)>,
    ) -> Result<PlaceFormDto> {
        let units = AdminUnits::load(self.admin_unit_repo.as_ref()).await?;
        Ok(PlaceFormDto {
            place: place_in(&units, ward_code, point),
            wards: units.choices(),
        })
    }

    /// The ward a point lies in, by name; `area` empty outside every ward
    /// (the sea, another country), which the box says.
    pub async fn ward_at(&self, lat: f64, lng: f64) -> Result<PlaceDto> {
        validate_coordinates(lat, lng)?;
        let Some(ward) = self
            .admin_unit_repo
            .ward_at(lat, lng, ADMIN_UNIT_SNAP_M)
            .await?
        else {
            return Ok(PlaceDto::default());
        };
        let provinces = self.admin_unit_repo.provinces().await?;
        let province = provinces.iter().find(|p| p.code == ward.province_code);
        Ok(PlaceDto {
            label: Some(ward_label(&ward, province)),
            area: AreaKey::Ward(ward.code).value(),
            approximate: false,
        })
    }

    /// A profile's position as it is stored: the point given, unless a ward
    /// is chosen and the point is missing or lies in another ward (the
    /// ward was changed without a new pin): then that ward's centre. A
    /// point in no ward at all (the sea, another country, a swapped pair)
    /// with no ward chosen is refused on `lat` (`place_outside_wards`): it
    /// would put the business on no near-me page. An unknown or turned-off
    /// ward is a field error on `ward`; a broken coordinate pair is left
    /// for the profile's rules to report.
    pub async fn placed(&self, mut input: UpdateProfileInput) -> Result<UpdateProfileInput> {
        let chosen = self
            .chosen_ward(input.ward.as_deref())
            .await
            .map_err(|e| match e.code() {
                ErrorCode::LocationNotFound => Error::field("ward", e.code()),
                _ => e,
            })?;
        let lies_in = match (input.lat, input.lng) {
            (None, None) => None,
            (Some(lat), Some(lng)) if validate_coordinates(lat, lng).is_ok() => Some(
                self.admin_unit_repo
                    .ward_at(lat, lng, ADMIN_UNIT_SNAP_M)
                    .await?,
            ),
            _ => return Ok(input),
        };
        match (chosen, lies_in) {
            (Some(ward), Some(Some(found))) if found.code == ward.code => {}
            (Some(ward), _) => (input.lat, input.lng) = (Some(ward.lat), Some(ward.lng)),
            (None, Some(None)) => {
                return Err(Error::field("lat", ErrorCode::PlaceOutsideWards));
            }
            (None, _) => {}
        }
        Ok(input)
    }

    /// The centre of the ward a form chose (`w28439`), `None` when none was
    /// chosen; an unknown or turned-off ward is `location_not_found`.
    pub async fn ward_centre(&self, area: Option<&str>) -> Result<Option<(f64, f64)>> {
        Ok(self.chosen_ward(area).await?.map(|w| (w.lat, w.lng)))
    }

    /// A ward box's value as its choice (a form shown again keeps its ward
    /// by name); `None` for none chosen or one not known.
    pub async fn ward_choice(&self, area: Option<&str>) -> Result<Option<WardMatchDto>> {
        let ward = match self.chosen_ward(area).await {
            Ok(Some(ward)) => ward,
            Ok(None) => return Ok(None),
            Err(e) if e.code() == ErrorCode::LocationNotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let provinces = self.admin_unit_repo.provinces().await?;
        let province = provinces.iter().find(|p| p.code == ward.province_code);
        Ok(Some(WardMatchDto {
            label: ward_label(&ward, province),
            value: AreaKey::Ward(ward.code).value(),
        }))
    }

    async fn chosen_ward(&self, area: Option<&str>) -> Result<Option<Ward>> {
        let Some(chosen) = area.map(str::trim).filter(|w| !w.is_empty()) else {
            return Ok(None);
        };
        match AreaKey::parse(chosen) {
            Some(AreaKey::Ward(code)) => self.admin_unit_repo.ward(&code).await?,
            _ => None,
        }
        .filter(|w| w.is_active)
        .map(Some)
        .ok_or_else(|| Error::new(ErrorCode::LocationNotFound))
    }

    /// The active wards whose name or province holds every word typed
    /// (accents ignored), by name, at most `WARD_SEARCH_RESULTS`: a ward
    /// box that loads its choices as the name is typed, so a farmer's page
    /// never carries all of them.
    pub async fn search_wards(&self, query: &str) -> Result<WardMatchesDto> {
        let Some(query) = search_text(Some(query.to_owned()))? else {
            return Ok(WardMatchesDto::default());
        };
        let words: Vec<String> = query.split_whitespace().map(str::to_owned).collect();
        let (wards, provinces) = tokio::try_join!(
            self.admin_unit_repo
                .search_wards(&words, WARD_SEARCH_RESULTS),
            self.admin_unit_repo.provinces()
        )?;
        Ok(WardMatchesDto {
            wards: wards
                .into_iter()
                .map(|w| {
                    let province = provinces.iter().find(|p| p.code == w.province_code);
                    WardMatchDto {
                        label: ward_label(&w, province),
                        value: AreaKey::Ward(w.code).value(),
                    }
                })
                .collect(),
        })
    }

    /// A map tile of the region, as the tile host draws it.
    pub async fn tile(&self, z: u8, x: u32, y: u32) -> Result<Vec<u8>> {
        if !tile_in_region(z, x, y) {
            return Err(Error::new(ErrorCode::MapTileNotFound));
        }
        self.tiles.tile(z, x, y).await
    }
}

/// Where a business is, from its stored ward and point.
fn place_in(units: &AdminUnits, ward_code: Option<&str>, point: Option<(f64, f64)>) -> PlaceDto {
    let Some(code) = ward_code else {
        return PlaceDto::default();
    };
    let key = AreaKey::Ward(code.to_owned());
    PlaceDto {
        label: units.label(&key),
        area: key.value(),
        approximate: units
            .ward(code)
            .zip(point)
            .is_some_and(|(ward, point)| at_centre(ward, point)),
    }
}

/// Whether a point is the ward's own centre: placed by choosing the ward.
fn at_centre(ward: &Ward, (lat, lng): (f64, f64)) -> bool {
    (ward.lat - lat).abs() < WARD_CENTRE_TOLERANCE_DEG
        && (ward.lng - lng).abs() < WARD_CENTRE_TOLERANCE_DEG
}

/// Whether tile `x`, `y` at zoom `z` exists and touches the region (the
/// web map's tiling: `2^z` columns from longitude -180, rows from the north
/// in the Mercator projection).
fn tile_in_region(z: u8, x: u32, y: u32) -> bool {
    if !(MAP_ZOOM_MIN..=MAP_ZOOM_MAX).contains(&z) {
        return false;
    }
    let n = f64::from(1u32 << z);
    let (x, y) = (f64::from(x), f64::from(y));
    if x >= n || y >= n {
        return false;
    }
    let lng = |x: f64| x / n * 360.0 - 180.0;
    let lat = |y: f64| (PI * (1.0 - 2.0 * y / n)).sinh().atan().to_degrees();
    let (west, east) = (lng(x), lng(x + 1.0));
    let (north, south) = (lat(y), lat(y + 1.0));
    east > MAP_REGION_LNG_MIN
        && west < MAP_REGION_LNG_MAX
        && north > MAP_REGION_LAT_MIN
        && south < MAP_REGION_LAT_MAX
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin_units::tests::units;

    /// The tile holding a point at zoom `z`.
    fn tile_of(lat: f64, lng: f64, z: u8) -> (u32, u32) {
        let n = f64::from(1u32 << z);
        let x = ((lng + 180.0) / 360.0 * n).floor();
        let rad = lat.to_radians();
        let y = ((1.0 - (rad.tan() + 1.0 / rad.cos()).ln() / PI) / 2.0 * n).floor();
        (x as u32, y as u32)
    }

    #[test]
    fn the_region_s_tiles_are_served_and_no_others() {
        let (x, y) = tile_of(10.4, 106.1, 16);
        assert!(tile_in_region(16, x, y), "Cai Lậy");
        let (x, y) = tile_of(21.0, 105.8, MAP_ZOOM_MAX);
        assert!(tile_in_region(MAP_ZOOM_MAX, x, y), "Hà Nội");
        let (x, y) = tile_of(48.85, 2.35, 12);
        assert!(!tile_in_region(12, x, y), "Paris");
        let (x, y) = tile_of(13.75, 100.5, 12);
        assert!(!tile_in_region(12, x, y), "Bangkok");
    }

    #[test]
    fn a_tile_outside_the_zooms_or_the_grid_is_refused() {
        let (x, y) = tile_of(10.4, 106.1, MAP_ZOOM_MIN - 1);
        assert!(!tile_in_region(MAP_ZOOM_MIN - 1, x, y));
        assert!(!tile_in_region(MAP_ZOOM_MAX + 1, 0, 0));
        assert!(!tile_in_region(10, 1 << 10, 0));
    }

    #[test]
    fn a_business_at_its_ward_s_centre_is_placed_approximately() {
        let units = units();
        let centre = units.ward("28441").map(|w| (w.lat, w.lng));
        let place = place_in(&units, Some("28441"), centre);
        assert_eq!(place.area, "w28441");
        assert_eq!(place.label.as_deref(), Some("Xã Bình Phú, Đồng Tháp"));
        assert!(place.approximate);

        let on_the_spot = centre.map(|(lat, lng)| (lat + 0.002, lng));
        assert!(!place_in(&units, Some("28441"), on_the_spot).approximate);
    }

    #[test]
    fn no_ward_is_no_place() {
        let place = place_in(&units(), None, None);
        assert!(place.area.is_empty());
        assert_eq!(place.label, None);
        assert!(!place.approximate);
    }
}
