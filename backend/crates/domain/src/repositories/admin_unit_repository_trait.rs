use async_trait::async_trait;

use crate::{
    error::Error,
    models::admin_unit::{AdminUnitSync, Province, Ward},
};

/// Provinces and wards: the bundled dataset, and the one question that needs
/// the boundaries (which ward a point lies in).
#[async_trait]
pub trait AdminUnitRepositoryTrait {
    /// Loads the bundled dataset unless its version is the one stored:
    /// names, points and boundaries are replaced, units it no longer lists
    /// are turned off (never deleted), and older rows that only knew a
    /// location preset are given the ward of its point.
    async fn sync(&self) -> Result<AdminUnitSync, Error>;

    /// Every province, by code (turned-off ones too).
    async fn provinces(&self) -> Result<Vec<Province>, Error>;

    /// Every ward, by code (turned-off ones too), without its boundary.
    async fn wards(&self) -> Result<Vec<Ward>, Error>;

    /// One ward by its code (a turned-off one too), without its boundary.
    async fn ward(&self, code: &str) -> Result<Option<Ward>, Error>;

    /// The active wards whose name or province's name holds every word
    /// (accents ignored), by name, at most `limit`.
    async fn search_wards(&self, words: &[String], limit: u64) -> Result<Vec<Ward>, Error>;

    /// The active ward a point lies in, else the nearest one within
    /// `snap_m` metres (a point on a coastline or in a sliver between two
    /// simplified boundaries); `None` beyond that.
    async fn ward_at(&self, lat: f64, lng: f64, snap_m: f64) -> Result<Option<Ward>, Error>;
}
