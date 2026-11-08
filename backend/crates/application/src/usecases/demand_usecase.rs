//! The demand report: what farmers asked for, by category × area × week.
//!
//! A lead is a call or Zalo tap, or a contact request that is not spam, of a
//! business that is not demo data. It counts under the ward its business
//! lies in (the nearest within `ADMIN_UNIT_SNAP_M` for one just outside
//! every ward; none: "no area") and under that ward's province, and under
//! the category of the service or product it was about; a tap on the page of
//! a shop with a single live service is about that service, one on a shop
//! with several has no category. Weeks run Monday to Sunday in Vietnam; the
//! report covers the `DEMAND_WEEKS` whole weeks before this one.
//!
//! The figures are meant to be shown to partners, so a figure above zero but
//! under `DEMAND_REPORT_MIN_COUNT` is hidden, and so is a total that would
//! give exactly one hidden figure away (total minus the shown ones).

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Utc};
use domain::{
    models::{
        category::CategoryAdmin,
        demand::{BusinessArea, LeadChannel, LeadEvent, NewSearch, SearchEvent, SoleCategory},
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        demand_repository_trait::DemandRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
    },
    Error, Result,
};
use tracing::warn;

use crate::{
    admin_units::{AdminUnits, AreaKey},
    authz::Actor,
    catalogue::PERM_REPORT_DEMAND_READ,
    constants::{
        ADMIN_UNIT_SNAP_M, DEMAND_REPORT_MIN_COUNT, DEMAND_WEEKS, SEARCH_QUERY_MAX,
        SEARCH_REPEAT_SECS, STEADY_WEEKLY_LEADS, UNMET_SEARCHES_SHOWN, VN_UTC_OFFSET_SECS,
    },
    hashing::sha256_hex,
    inputs::{SearchInput, SearchPlace, Visitor},
    outputs::{
        DemandAreaDto, DemandReportDto, DemandRowDto, DemandWeekDto, Figure, UnmetSearchDto,
    },
};

/// What a report may show: every area and category (`None`), or only these
/// (a partner's contracts: provinces, wards). A province covers its wards.
/// `focus` narrows the rows to one area; the area list still shows them all.
#[derive(Debug, Clone, Default)]
pub struct ReportScope {
    pub areas: Option<Vec<AreaKey>>,
    pub categories: Option<Vec<i32>>,
    pub focus: Option<AreaKey>,
}

#[derive(Clone)]
pub struct DemandReportUseCase {
    demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    /// Fingerprints of recent searches (hashed, in memory only) and when
    /// they were made: a repeat within `SEARCH_REPEAT_SECS` counts once.
    recent_searches: Arc<Mutex<HashMap<String, Instant>>>,
}

/// Everything `DemandReportUseCase` depends on, named so wiring cannot swap
/// two repos of compatible types by position.
pub struct DemandDeps {
    pub demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
}

impl DemandReportUseCase {
    pub fn new(d: DemandDeps) -> Self {
        Self {
            demand_repo: d.demand_repo,
            category_repo: d.category_repo,
            location_repo: d.location_repo,
            admin_unit_repo: d.admin_unit_repo,
            recent_searches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The report for every category of one area (`Some`) or of all.
    pub async fn report(&self, actor: &Actor, area: Option<AreaKey>) -> Result<DemandReportDto> {
        actor.require(PERM_REPORT_DEMAND_READ)?;
        let scope = ReportScope {
            focus: area,
            ..ReportScope::default()
        };
        self.build(Utc::now(), &scope).await
    }

    /// True the first time `fingerprint` is seen within `SEARCH_REPEAT_SECS`;
    /// older ones are forgotten as it goes, so the memory stays small.
    fn first_in_window(&self, fingerprint: String) -> bool {
        let window = std::time::Duration::from_secs(SEARCH_REPEAT_SECS);
        let now = Instant::now();
        let Ok(mut recent) = self.recent_searches.lock() else {
            return true;
        };
        recent.retain(|_, seen| now.duration_since(*seen) < window);
        if recent.contains_key(&fingerprint) {
            return false;
        }
        recent.insert(fingerprint, now);
        true
    }

    /// The report as a scope allows; the caller has checked who may see it.
    pub(crate) async fn build(
        &self,
        now: DateTime<Utc>,
        scope: &ReportScope,
    ) -> Result<DemandReportDto> {
        let weeks = report_weeks(now)?;
        let (from, to) = window(&weeks)?;
        let (events, searches, areas, sole, categories, units) = tokio::try_join!(
            self.demand_repo.lead_events(from, to),
            self.demand_repo.search_events(from, to),
            self.demand_repo.business_areas(ADMIN_UNIT_SNAP_M),
            self.demand_repo.sole_categories(),
            self.category_repo.list_admin(),
            AdminUnits::load(self.admin_unit_repo.as_ref()),
        )?;
        Ok(aggregate(Input {
            weeks,
            events,
            searches,
            areas,
            sole,
            categories,
            units: &units,
            scope,
        }))
    }

    /// Counts a visitor's search for the report, in the background: a page
    /// never waits for it nor fails on it. Words are trimmed, lowercased and
    /// capped; a search with no words, category or place is not one, nor is
    /// a signed-in visitor's, nor the same search again by the same visitor
    /// within `SEARCH_REPEAT_SECS`. Its area is the ward of the place.
    pub fn record_search(&self, input: SearchInput) {
        let Visitor::Anonymous { key } = &input.visitor else {
            return;
        };
        let query: String = input
            .query
            .trim()
            .to_lowercase()
            .chars()
            .take(SEARCH_QUERY_MAX)
            .collect();
        let query = (!query.is_empty()).then_some(query);
        if query.is_none()
            && input.category_id.is_none()
            && matches!(input.place, SearchPlace::Anywhere)
        {
            return;
        }
        let place = match input.place {
            SearchPlace::Preset(id) => format!("p{id}"),
            SearchPlace::Point { lat, lng } => format!("{lat:.3},{lng:.3}"),
            SearchPlace::Anywhere => String::new(),
        };
        let fingerprint = sha256_hex(format!(
            "{key}|{}|{:?}|{place}",
            query.as_deref().unwrap_or_default(),
            input.category_id
        ));
        if !self.first_in_window(fingerprint) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let stored = match this.ward_of_place(&input.place).await {
                Ok(ward_code) => {
                    this.demand_repo
                        .record_search(NewSearch {
                            query,
                            category_id: input.category_id,
                            ward_code,
                            results: input.results,
                        })
                        .await
                }
                Err(e) => Err(e),
            };
            if let Err(e) = stored {
                warn!(error = %e, "search not recorded");
            }
        });
    }

    /// The ward a search was made in: of the preset's point, or of the
    /// searcher's position.
    async fn ward_of_place(&self, place: &SearchPlace) -> Result<Option<String>> {
        let (lat, lng) = match place {
            SearchPlace::Anywhere => return Ok(None),
            SearchPlace::Point { lat, lng } => (*lat, *lng),
            SearchPlace::Preset(id) => match self.location_repo.find_by_id(*id).await? {
                Some(l) => (l.lat, l.lng),
                None => return Ok(None),
            },
        };
        Ok(self
            .admin_unit_repo
            .ward_at(lat, lng, ADMIN_UNIT_SNAP_M)
            .await?
            .map(|w| w.code))
    }
}

fn vn() -> Result<FixedOffset> {
    FixedOffset::east_opt(VN_UTC_OFFSET_SECS)
        .ok_or_else(|| Error::internal("Vietnam offset out of range."))
}

/// Today in Vietnam.
pub fn vn_today(now: DateTime<Utc>) -> Result<NaiveDate> {
    Ok(now.with_timezone(&vn()?).date_naive())
}

/// The `DEMAND_WEEKS` whole weeks before the one `now` falls in, oldest
/// first, as Vietnam dates.
pub fn report_weeks(now: DateTime<Utc>) -> Result<Vec<DemandWeekDto>> {
    let today = vn_today(now)?;
    let this_monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    Ok((1..=DEMAND_WEEKS as i64)
        .rev()
        .map(|back| {
            let starts_on = this_monday - Duration::weeks(back);
            DemandWeekDto {
                starts_on,
                ends_on: starts_on + Duration::days(6),
            }
        })
        .collect())
}

/// The report's instants: Monday 00:00 of its first week to Monday 00:00
/// after its last, Vietnam time.
fn window(weeks: &[DemandWeekDto]) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let offset = vn()?;
    let at_midnight = |d: NaiveDate| -> Result<DateTime<Utc>> {
        offset
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
            .single()
            .map(|t| t.with_timezone(&Utc))
            .ok_or_else(|| Error::internal("Report week out of range."))
    };
    match (weeks.first(), weeks.last()) {
        (Some(first), Some(last)) => Ok((
            at_midnight(first.starts_on)?,
            at_midnight(last.ends_on + Duration::days(1))?,
        )),
        _ => Err(Error::internal("Empty report window.")),
    }
}

/// The raw counts of one area and category: a count per report week.
#[derive(Clone)]
struct Cell {
    weeks: Vec<i64>,
    calls: i64,
    zalo: i64,
    inquiries: i64,
    people: HashSet<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            weeks: vec![0; DEMAND_WEEKS],
            calls: 0,
            zalo: 0,
            inquiries: 0,
            people: HashSet::new(),
        }
    }
}

impl Cell {
    fn add(&mut self, other: &Cell) {
        for (w, o) in self.weeks.iter_mut().zip(&other.weeks) {
            *w += o;
        }
        self.calls += other.calls;
        self.zalo += other.zalo;
        self.inquiries += other.inquiries;
        self.people.extend(other.people.iter().cloned());
    }

    fn total(&self) -> i64 {
        self.weeks.iter().sum()
    }
}

struct Input<'a> {
    weeks: Vec<DemandWeekDto>,
    events: Vec<LeadEvent>,
    searches: Vec<SearchEvent>,
    areas: Vec<BusinessArea>,
    sole: Vec<SoleCategory>,
    categories: Vec<CategoryAdmin>,
    units: &'a AdminUnits,
    scope: &'a ReportScope,
}

/// Which wards (by their keys) a report counts: those in its areas, and of
/// those, the ones in its focus.
struct AreaFilter<'a> {
    units: &'a AdminUnits,
    scope: &'a ReportScope,
}

impl AreaFilter<'_> {
    /// A lead in this ward (or in none) is one of the scope's areas.
    fn in_areas(&self, ward: Option<&str>) -> bool {
        self.scope
            .areas
            .as_ref()
            .is_none_or(|areas| self.units.keys_of(ward).iter().any(|k| areas.contains(k)))
    }

    /// A lead in this ward goes into the rows: in scope, and in the focus.
    fn in_rows(&self, ward: Option<&str>) -> bool {
        self.in_areas(ward)
            && self
                .scope
                .focus
                .as_ref()
                .is_none_or(|f| self.units.keys_of(ward).contains(f))
    }

    /// The scope names this area itself.
    fn listed(&self, area: &AreaKey) -> bool {
        self.scope.areas.as_ref().is_some_and(|a| a.contains(area))
    }
}

fn aggregate(input: Input<'_>) -> DemandReportDto {
    let Input {
        weeks,
        events,
        searches,
        areas,
        sole,
        categories,
        units,
        scope,
    } = input;
    let filter = AreaFilter { units, scope };
    let first_day = weeks.first().map(|w| w.starts_on);
    let ward_of: HashMap<i32, String> = areas
        .into_iter()
        .map(|a| (a.business_id, a.ward_code))
        .collect();
    let sole_of: HashMap<i32, i32> = sole
        .iter()
        .map(|s| (s.business_id, s.category_id))
        .collect();

    let mut cells: HashMap<(Option<String>, Option<i32>), Cell> = HashMap::new();
    for e in events {
        let Some(first_day) = first_day else {
            break;
        };
        let Some(index) = week_of(e.at, first_day) else {
            continue;
        };
        let ward = ward_of.get(&e.business_id).cloned();
        let category = e
            .category_id
            .or_else(|| sole_of.get(&e.business_id).copied());
        let cell = cells.entry((ward, category)).or_default();
        if let Some(week) = cell.weeks.get_mut(index) {
            *week += 1;
        }
        match e.channel {
            LeadChannel::Call => cell.calls += 1,
            LeadChannel::Zalo => cell.zalo += 1,
            LeadChannel::Inquiry => {
                cell.inquiries += 1;
                if let Some(phone) = e.requester_phone {
                    cell.people.insert(phone);
                }
            }
        }
    }

    let in_categories = |c: &Option<i32>| match (&scope.categories, c) {
        (None, _) => true,
        (Some(s), Some(c)) => s.contains(c),
        (Some(_), None) => false,
    };

    let area_rows = area_rows(&cells, &filter, &in_categories);
    let steady_areas = area_rows
        .iter()
        .filter(|a| a.steady && !a.nested && a.level != LEVEL_NONE)
        .count();

    // Rows: the categories of the areas in scope, or of the one in focus.
    let mut by_category: HashMap<Option<i32>, Cell> = HashMap::new();
    for ((ward, category), cell) in &cells {
        if filter.in_rows(ward.as_deref()) && in_categories(category) {
            by_category.entry(*category).or_default().add(cell);
        }
    }
    if let Some(wanted) = &scope.categories {
        for c in wanted {
            by_category.entry(Some(*c)).or_default();
        }
    }
    let mut ordered: Vec<&CategoryAdmin> = categories.iter().collect();
    ordered.sort_by_key(|c| (c.category_type.clone(), c.display_order, c.id));
    let mut rows = Vec::new();
    let mut all = Cell::default();
    for c in ordered {
        if let Some(cell) = by_category.get(&Some(c.id)) {
            all.add(cell);
            rows.push(row(Some(c.id), Some(c.name.clone()), cell));
        }
    }
    if let Some(cell) = by_category.get(&None) {
        all.add(cell);
        rows.push(row(None, None, cell));
    }
    let mut all_row = row(None, None, &all);
    // A column sum gives a hidden figure away like a row sum does.
    for ((i, figure), raw) in all_row.weeks.iter_mut().enumerate().zip(&all.weeks) {
        let column: Vec<Figure> = rows
            .iter()
            .filter_map(|r| r.weeks.get(i).copied())
            .collect();
        if shown_total(*raw, &column).value.is_none() {
            figure.value = None;
        }
    }
    let totals: Vec<Figure> = rows.iter().map(|r| r.total).collect();
    if shown_total(all.total(), &totals).value.is_none() {
        all_row.total.value = None;
    }

    let (searches, unmet) = search_figures(&searches, first_day, &filter, &in_categories);

    DemandReportDto {
        weeks,
        areas: area_rows,
        rows,
        all: all_row,
        searches,
        unmet,
        steady_areas,
        min_count: DEMAND_REPORT_MIN_COUNT,
        steady_weekly_leads: STEADY_WEEKLY_LEADS,
    }
}

const LEVEL_PROVINCE: &str = "province";
const LEVEL_WARD: &str = "ward";
const LEVEL_NONE: &str = "none";

/// The area list: provinces in the official order, each followed by its
/// wards with leads (by name), then "no area" when it has leads. With every
/// area in scope, a province is listed once it has leads; for a partner,
/// the provinces and wards of its contracts are listed even at zero, and a
/// ward of its own contract is listed alone when its province is not its.
fn area_rows(
    cells: &HashMap<(Option<String>, Option<i32>), Cell>,
    filter: &AreaFilter<'_>,
    in_categories: &dyn Fn(&Option<i32>) -> bool,
) -> Vec<DemandAreaDto> {
    let units = filter.units;
    let mut by_ward: HashMap<&str, Cell> = HashMap::new();
    let mut by_province: HashMap<&str, Cell> = HashMap::new();
    let mut unplaced: Option<Cell> = None;
    for ((ward, category), cell) in cells {
        if !in_categories(category) {
            continue;
        }
        match ward.as_deref() {
            Some(w) => {
                by_ward.entry(w).or_default().add(cell);
                if let Some(p) = units.province_of(w) {
                    by_province.entry(p).or_default().add(cell);
                }
            }
            None => unplaced.get_or_insert_with(Cell::default).add(cell),
        }
    }
    let every_area = filter.scope.areas.is_none();
    let empty = Cell::default();
    let mut rows = Vec::new();
    for p in units.provinces() {
        let key = AreaKey::Province(p.code.clone());
        let cell = by_province.get(p.code.as_str());
        let province_shown = filter.listed(&key) || (every_area && cell.is_some());
        if province_shown {
            rows.push(area_dto(
                &key,
                LEVEL_PROVINCE,
                Some(p.name.clone()),
                None,
                false,
                cell.unwrap_or(&empty),
            ));
        }
        for w in units.wards_of(&p.code) {
            let key = AreaKey::Ward(w.code.clone());
            let cell = by_ward.get(w.code.as_str());
            if filter.listed(&key) || (province_shown && cell.is_some()) {
                rows.push(area_dto(
                    &key,
                    LEVEL_WARD,
                    units.label(&key),
                    Some(w.full_name.clone()),
                    province_shown,
                    cell.unwrap_or(&empty),
                ));
            }
        }
    }
    if every_area || filter.listed(&AreaKey::Unplaced) {
        if let Some(cell) = &unplaced {
            rows.push(area_dto(
                &AreaKey::Unplaced,
                LEVEL_NONE,
                None,
                None,
                false,
                cell,
            ));
        }
    }
    rows
}

/// A figure as shown: hidden when it is above zero and under the minimum.
fn shown(value: i64) -> Figure {
    Figure {
        value: (value == 0 || value >= DEMAND_REPORT_MIN_COUNT).then_some(value),
    }
}

/// A total of `parts` as shown: hidden like any figure, and also when
/// exactly one part is hidden, since the total minus the shown parts would
/// give that one away.
fn shown_total(total: i64, parts: &[Figure]) -> Figure {
    let hidden = parts.iter().filter(|f| f.value.is_none()).count();
    if hidden == 1 {
        Figure { value: None }
    } else {
        shown(total)
    }
}

fn row(category_id: Option<i32>, category: Option<String>, cell: &Cell) -> DemandRowDto {
    let weeks: Vec<Figure> = cell.weeks.iter().map(|v| shown(*v)).collect();
    let calls = shown(cell.calls);
    let zalo = shown(cell.zalo);
    let inquiries = shown(cell.inquiries);
    let by_week = shown_total(cell.total(), &weeks);
    let by_channel = shown_total(cell.total(), &[calls, zalo, inquiries]);
    DemandRowDto {
        category_id,
        category,
        total: if by_week.value.is_none() || by_channel.value.is_none() {
            Figure { value: None }
        } else {
            by_week
        },
        weeks,
        calls,
        zalo,
        inquiries,
        people: shown(cell.people.len() as i64),
    }
}

/// The week index of a Vietnam date in the report, if it falls in it.
fn week_of(at: DateTime<Utc>, first_day: NaiveDate) -> Option<usize> {
    let day = at.with_timezone(&vn().ok()?).date_naive();
    usize::try_from((day - first_day).num_days() / 7)
        .ok()
        .filter(|i| *i < DEMAND_WEEKS && day >= first_day)
}

/// Searches by week in the rows' areas and categories, and the words that
/// found nothing, asked at least `DEMAND_REPORT_MIN_COUNT` times.
fn search_figures(
    searches: &[SearchEvent],
    first_day: Option<NaiveDate>,
    filter: &AreaFilter<'_>,
    in_categories: &dyn Fn(&Option<i32>) -> bool,
) -> (Vec<Figure>, Vec<UnmetSearchDto>) {
    let mut weekly = [0_i64; DEMAND_WEEKS];
    let mut unmet: HashMap<&str, i64> = HashMap::new();
    let Some(first_day) = first_day else {
        return (weekly.iter().map(|v| shown(*v)).collect(), Vec::new());
    };
    for s in searches {
        if !filter.in_rows(s.ward_code.as_deref()) || !in_categories(&s.category_id) {
            continue;
        }
        let Some(index) = week_of(s.at, first_day) else {
            continue;
        };
        if let Some(week) = weekly.get_mut(index) {
            *week += 1;
        }
        if let (0, Some(q)) = (s.results, s.query.as_deref()) {
            *unmet.entry(q).or_default() += 1;
        }
    }
    let mut unmet: Vec<UnmetSearchDto> = unmet
        .into_iter()
        .filter(|(_, n)| *n >= DEMAND_REPORT_MIN_COUNT)
        .map(|(query, count)| UnmetSearchDto {
            query: query.to_owned(),
            count,
        })
        .collect();
    unmet.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.query.cmp(&b.query)));
    unmet.truncate(UNMET_SEARCHES_SHOWN);
    (weekly.iter().map(|v| shown(*v)).collect(), unmet)
}

fn area_dto(
    key: &AreaKey,
    level: &str,
    label: Option<String>,
    name: Option<String>,
    nested: bool,
    cell: &Cell,
) -> DemandAreaDto {
    DemandAreaDto {
        value: key.value(),
        level: level.to_owned(),
        label,
        name,
        nested,
        total: shown(cell.total()),
        steady: cell.weeks.iter().all(|w| *w >= STEADY_WEEKLY_LEADS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin_units::tests::units;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0)
            .single()
            .unwrap_or_default()
    }

    fn event(at: DateTime<Utc>, business_id: i32, category_id: Option<i32>) -> LeadEvent {
        LeadEvent {
            at,
            business_id,
            category_id,
            channel: LeadChannel::Call,
            requester_phone: None,
        }
    }

    fn category(id: i32, name: &str) -> CategoryAdmin {
        CategoryAdmin {
            id,
            name: name.into(),
            slug: name.into(),
            category_type: "service".into(),
            icon: None,
            color: None,
            description: None,
            display_order: 0,
            is_active: true,
            created_on: Utc::now(),
            updated_on: Utc::now(),
            created_by_name: None,
            updated_by_name: None,
        }
    }

    fn placed(business_id: i32, ward_code: &str) -> BusinessArea {
        BusinessArea {
            business_id,
            ward_code: ward_code.into(),
            name: "shop".into(),
            phone: "0912345678".into(),
            address: None,
        }
    }

    fn province(code: &str) -> AreaKey {
        AreaKey::Province(code.into())
    }

    fn ward(code: &str) -> AreaKey {
        AreaKey::Ward(code.into())
    }

    // Wednesday 2026-11-11, so the report's last week is 2–8 November.
    fn now() -> DateTime<Utc> {
        at(2026, 11, 11, 3)
    }

    /// Business 1 is in Cai Lậy (28439, Đồng Tháp), business 3 in Long
    /// Xuyên (30001, An Giang); business 2 has no ward.
    fn run(events: Vec<LeadEvent>, scope: &ReportScope) -> DemandReportDto {
        let units = units();
        aggregate(Input {
            weeks: report_weeks(now()).unwrap_or_default(),
            events,
            searches: Vec::new(),
            areas: vec![placed(1, "28439"), placed(3, "30001")],
            sole: vec![SoleCategory {
                business_id: 1,
                category_id: 7,
            }],
            categories: vec![category(7, "spray"), category(8, "dredge")],
            units: &units,
            scope,
        })
    }

    fn values(report: &DemandReportDto) -> Vec<&str> {
        report.areas.iter().map(|a| a.value.as_str()).collect()
    }

    #[test]
    fn weeks_are_whole_vietnam_weeks_before_this_one() {
        let weeks = report_weeks(now()).unwrap_or_default();
        assert_eq!(weeks.len(), DEMAND_WEEKS);
        let last = weeks.last().cloned().unwrap_or(DemandWeekDto {
            starts_on: NaiveDate::MIN,
            ends_on: NaiveDate::MIN,
        });
        assert_eq!(
            last.starts_on,
            NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default()
        );
        assert_eq!(
            last.ends_on,
            NaiveDate::from_ymd_opt(2026, 11, 8).unwrap_or_default()
        );
        // Sunday 23:30 in Vietnam is still Sunday, though it is 16:30 UTC.
        let late = report_weeks(at(2026, 11, 8, 16)).unwrap_or_default();
        assert_eq!(
            late.last().map(|w| w.ends_on.weekday()),
            Some(chrono::Weekday::Sun)
        );
    }

    #[test]
    fn small_figures_are_hidden_and_never_given_away() {
        // Week 8: 3 sprays (hidden), week 7: 6 (shown). The row total (9)
        // would give the hidden 3 away, so it is hidden too.
        let mut events = Vec::new();
        for _ in 0..3 {
            events.push(event(at(2026, 11, 3, 2), 1, Some(7)));
        }
        for _ in 0..6 {
            events.push(event(at(2026, 10, 27, 2), 1, Some(7)));
        }
        let report = run(events, &ReportScope::default());
        let week = |i: usize| {
            report
                .rows
                .first()
                .and_then(|r| r.weeks.get(i))
                .and_then(|f| f.value)
        };
        assert_eq!(week(DEMAND_WEEKS - 1), None);
        assert_eq!(week(DEMAND_WEEKS - 2), Some(6));
        assert_eq!(week(0), Some(0), "zero is shown");
        assert_eq!(report.rows.first().map(|r| r.total.value), Some(None));
    }

    #[test]
    fn a_lead_counts_under_its_ward_and_its_province() {
        let events = vec![
            event(at(2026, 11, 3, 2), 1, None),
            event(at(2026, 11, 3, 2), 2, Some(8)),
            event(at(2026, 11, 3, 2), 3, Some(8)),
            // Before the window and in the current week: not counted.
            event(at(2026, 9, 1, 2), 1, Some(7)),
            event(at(2026, 11, 10, 2), 1, Some(7)),
        ];
        let report = run(events.clone(), &ReportScope::default());
        let names: Vec<_> = report.rows.iter().map(|r| r.category_id).collect();
        assert_eq!(
            names,
            vec![Some(7), Some(8)],
            "a shop tap takes its sole service"
        );
        assert_eq!(
            values(&report),
            vec!["p82", "w28439", "p91", "w30001", "none"],
            "provinces with leads by code, each with its wards, then no area"
        );
        let cai_lay = report.areas.get(1);
        assert_eq!(cai_lay.map(|a| a.nested), Some(true));
        assert_eq!(
            cai_lay.and_then(|a| a.name.as_deref()),
            Some("Phường Cai Lậy")
        );

        // The province in focus takes its wards' leads; a ward only its own.
        for focus in [province("82"), ward("28439")] {
            let one = run(
                events.clone(),
                &ReportScope {
                    focus: Some(focus),
                    ..ReportScope::default()
                },
            );
            let rows: Vec<_> = one.rows.iter().map(|r| r.category_id).collect();
            assert_eq!(rows, vec![Some(7)]);
            assert_eq!(one.areas.len(), 5, "the area list is not narrowed");
        }
    }

    #[test]
    fn a_partner_sees_only_its_areas_and_categories() {
        let events = vec![
            event(at(2026, 11, 3, 2), 1, Some(7)),
            event(at(2026, 11, 3, 2), 3, Some(8)),
            event(at(2026, 11, 3, 2), 2, Some(8)),
        ];
        // A ward of a province it does not have: listed alone, even at zero.
        let partner = run(
            events.clone(),
            &ReportScope {
                areas: Some(vec![ward("28400")]),
                categories: Some(vec![8]),
                focus: None,
            },
        );
        assert_eq!(values(&partner), vec!["w28400"]);
        assert_eq!(partner.areas.first().map(|a| a.nested), Some(false));
        assert_eq!(
            partner.areas.first().and_then(|a| a.label.as_deref()),
            Some("Đồng Tháp — Xã Tân Phước")
        );
        let rows: Vec<_> = partner.rows.iter().map(|r| r.category_id).collect();
        assert_eq!(rows, vec![Some(8)], "its category, even with no leads");
        assert_eq!(partner.all.total.value, Some(0));

        // A whole province: its wards with leads come under it.
        let whole = run(
            events,
            &ReportScope {
                areas: Some(vec![province("91")]),
                categories: Some(vec![8]),
                focus: None,
            },
        );
        assert_eq!(values(&whole), vec!["p91", "w30001"]);
    }

    #[test]
    fn an_area_is_steady_when_every_week_reaches_the_bar() {
        let weeks = report_weeks(now()).unwrap_or_default();
        let mut events = Vec::new();
        for w in &weeks {
            let day = w.starts_on + Duration::days(2);
            let instant = Utc
                .with_ymd_and_hms(day.year(), day.month(), day.day(), 3, 0, 0)
                .single()
                .unwrap_or_default();
            for _ in 0..STEADY_WEEKLY_LEADS {
                events.push(event(instant, 1, Some(7)));
            }
        }
        events.push(event(at(2026, 11, 3, 2), 3, Some(7)));
        let report = run(events, &ReportScope::default());
        let steady: Vec<(&str, bool)> = report
            .areas
            .iter()
            .map(|a| (a.value.as_str(), a.steady))
            .collect();
        assert_eq!(
            steady,
            vec![
                ("p82", true),
                ("w28439", true),
                ("p91", false),
                ("w30001", false)
            ]
        );
        assert_eq!(
            report.steady_areas, 1,
            "a steady ward under its province counts once"
        );
    }

    #[test]
    fn searches_that_found_nothing_are_listed_once_asked_enough() {
        let search = |query: &str, results: i64| SearchEvent {
            at: at(2026, 11, 3, 2),
            query: Some(query.into()),
            category_id: None,
            ward_code: Some("28439".into()),
            results,
        };
        let mut searches = Vec::new();
        for _ in 0..DEMAND_REPORT_MIN_COUNT {
            searches.push(search("drone sau rieng", 0));
        }
        searches.push(search("may cay", 0));
        searches.push(search("phun thuoc", 3));
        let first = report_weeks(now())
            .unwrap_or_default()
            .first()
            .map(|w| w.starts_on);
        let units = units();
        let any = |_: &Option<i32>| true;
        let everywhere = ReportScope::default();
        let filter = AreaFilter {
            units: &units,
            scope: &everywhere,
        };
        let (weekly, unmet) = search_figures(&searches, first, &filter, &any);
        assert_eq!(weekly.last().and_then(|f| f.value), Some(7));
        let words: Vec<_> = unmet.iter().map(|u| (u.query.as_str(), u.count)).collect();
        assert_eq!(
            words,
            vec![("drone sau rieng", DEMAND_REPORT_MIN_COUNT)],
            "a single ask is never shown"
        );

        // Searches elsewhere than the focus do not count.
        let elsewhere = ReportScope {
            focus: Some(province("91")),
            ..ReportScope::default()
        };
        let filter = AreaFilter {
            units: &units,
            scope: &elsewhere,
        };
        let (weekly, _) = search_figures(&searches, first, &filter, &any);
        assert_eq!(weekly.last().and_then(|f| f.value), Some(0));
    }
}
