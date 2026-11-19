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
    sync::Arc,
};

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Utc};
use domain::{
    models::{
        category::CategoryAdmin,
        demand::{DemoScope, LeadChannel, LeadCount, NewSearch, SearchCount, SoleCategory},
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
        ADMIN_UNIT_SNAP_M, DEMAND_REPORT_MIN_COUNT, DEMAND_WEEKS, EVENT_KEEP_WEEKS,
        SEARCH_QUERY_MAX, SEARCH_REPEAT_SECS, UNMET_SEARCHES_SHOWN, VN_UTC_OFFSET_SECS,
    },
    hashing::sha256_hex,
    inputs::{SearchInput, SearchPlace, Visitor},
    outputs::{
        DemandAreaDto, DemandReportDto, DemandRowDto, DemandWeekDto, Figure, UnmetSearchDto,
    },
    repeats::RepeatFilter,
    settings::SettingsReader,
};

/// What a report may show: every area and category (`None`), or only these
/// (a partner's contracts: provinces, wards). A province covers its wards.
/// `focus` narrows the rows to one area; the area list still shows them all.
#[derive(Debug, Clone, Default)]
pub struct ReportScope {
    pub areas: Option<Vec<AreaKey>>,
    pub categories: Option<Vec<i32>>,
    pub focus: Option<AreaKey>,
    /// Real data only by default; the admin's report counts demo data too.
    pub demo: DemoScope,
}

#[derive(Clone)]
pub struct DemandReportUseCase {
    demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    settings: SettingsReader,
    /// Fingerprints of recent searches (hashed, in memory only) and when
    /// they were made: a repeat within `SEARCH_REPEAT_SECS` counts once.
    recent_searches: RepeatFilter,
}

/// Everything `DemandReportUseCase` depends on, named so wiring cannot swap
/// two repos of compatible types by position.
pub struct DemandDeps {
    pub demand_repo: Arc<dyn DemandRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    pub settings: SettingsReader,
}

impl DemandReportUseCase {
    pub fn new(d: DemandDeps) -> Self {
        Self {
            demand_repo: d.demand_repo,
            category_repo: d.category_repo,
            location_repo: d.location_repo,
            admin_unit_repo: d.admin_unit_repo,
            settings: d.settings,
            recent_searches: RepeatFilter::default(),
        }
    }

    /// The report for every category of one area (`Some`) or of all.
    pub async fn report(&self, actor: &Actor, area: Option<AreaKey>) -> Result<DemandReportDto> {
        actor.require(PERM_REPORT_DEMAND_READ)?;
        let scope = ReportScope {
            focus: area,
            demo: DemoScope::All,
            ..ReportScope::default()
        };
        self.build(Utc::now(), &scope).await
    }

    /// The report as a scope allows; the caller has checked who may see it.
    pub(crate) async fn build(
        &self,
        now: DateTime<Utc>,
        scope: &ReportScope,
    ) -> Result<DemandReportDto> {
        let weeks = report_weeks(now)?;
        let (from, to) = window(&weeks)?;
        let (events, searches, areas, sole, categories, units, settings) = tokio::try_join!(
            self.demand_repo.lead_counts(from, to, scope.demo),
            self.demand_repo.search_counts(from, to, scope.demo),
            self.demand_repo.business_areas(scope.demo),
            self.demand_repo.sole_categories(),
            self.category_repo.list_admin(),
            AdminUnits::load(self.admin_unit_repo.as_ref()),
            self.settings.load(),
        )?;
        Ok(aggregate(Input {
            weeks,
            events: &events,
            searches: &searches,
            sole: &sole,
            // Demo businesses are among those counted: the page says so.
            with_demo: areas.iter().any(|a| a.is_demo),
            categories,
            units: &units,
            scope,
            steady_weekly_leads: settings.steady_weekly_leads(),
        }))
    }

    /// Counts the taps and searches older than `EVENT_KEEP_WEEKS` whole weeks
    /// into the weekly totals and deletes them, a week at a time, oldest
    /// first; the rows deleted. Run now and then by the server: nothing
    /// reads rows that old, and the two tables would otherwise grow for as
    /// long as the platform runs.
    pub async fn roll_up_old_events(&self, now: DateTime<Utc>) -> Result<u64> {
        let this_monday = monday_of(vn_today(now)?);
        let cutoff = vn_midnight(this_monday - Duration::weeks(EVENT_KEEP_WEEKS as i64))?;
        let mut deleted = 0;
        while let Some(oldest) = self.demand_repo.oldest_event_before(cutoff).await? {
            let starts_on = monday_of(vn_today(oldest)?);
            let (from, to) = (
                vn_midnight(starts_on)?,
                vn_midnight(starts_on + Duration::weeks(1))?,
            );
            let rows = self.demand_repo.roll_up_week(starts_on, from, to).await?;
            // The oldest row is in the week just rolled up: none deleted
            // means another process did it, and it will finish the rest.
            if rows == 0 {
                break;
            }
            deleted += rows;
        }
        Ok(deleted)
    }

    /// Forgets the searches whose repeat window has passed (bounded memory).
    pub fn forget_old_searches(&self) {
        self.recent_searches
            .forget_older_than(std::time::Duration::from_secs(SEARCH_REPEAT_SECS));
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
        let window = std::time::Duration::from_secs(SEARCH_REPEAT_SECS);
        if !self.recent_searches.first_in_window(fingerprint, window) {
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

/// The Monday of the week a Vietnam date falls in.
fn monday_of(day: NaiveDate) -> NaiveDate {
    day - Duration::days(i64::from(day.weekday().num_days_from_monday()))
}

/// Midnight in Vietnam at the start of `day`, as an instant.
fn vn_midnight(day: NaiveDate) -> Result<DateTime<Utc>> {
    vn()?
        .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap_or_default())
        .single()
        .map(|t| t.with_timezone(&Utc))
        .ok_or_else(|| Error::internal("Report week out of range."))
}

/// The `DEMAND_WEEKS` whole weeks before the one `now` falls in, oldest
/// first, as Vietnam dates.
pub fn report_weeks(now: DateTime<Utc>) -> Result<Vec<DemandWeekDto>> {
    let this_monday = monday_of(vn_today(now)?);
    Ok((1..=DEMAND_WEEKS as i64)
        .rev()
        .map(|back| {
            let starts_on = this_monday - Duration::weeks(back);
            DemandWeekDto {
                starts_on,
                ends_on: starts_on + Duration::weeks(1) - Duration::days(1),
            }
        })
        .collect())
}

/// The report's instants: Monday 00:00 of its first week to Monday 00:00
/// after its last, Vietnam time.
fn window(weeks: &[DemandWeekDto]) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    match (weeks.first(), weeks.last()) {
        (Some(first), Some(last)) => Ok((
            vn_midnight(first.starts_on)?,
            vn_midnight(last.ends_on + Duration::days(1))?,
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
    events: &'a [LeadCount],
    searches: &'a [SearchCount],
    sole: &'a [SoleCategory],
    with_demo: bool,
    categories: Vec<CategoryAdmin>,
    units: &'a AdminUnits,
    scope: &'a ReportScope,
    /// Leads a week an area needs (`market.steady_weekly_leads`).
    steady_weekly_leads: i64,
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
        sole,
        with_demo,
        categories,
        units,
        scope,
        steady_weekly_leads,
    } = input;
    let filter = AreaFilter { units, scope };
    let sole_of: HashMap<i32, i32> = sole
        .iter()
        .map(|s| (s.business_id, s.category_id))
        .collect();

    let mut cells: HashMap<(Option<String>, Option<i32>), Cell> = HashMap::new();
    for e in events {
        // A lead about nothing in particular is about its shop's one service.
        let category = e
            .category_id
            .or_else(|| sole_of.get(&e.business_id?).copied());
        let cell = cells.entry((e.ward_code.clone(), category)).or_default();
        // The database counted the report's window only.
        let Some(week) = cell.weeks.get_mut(e.week) else {
            continue;
        };
        *week += e.count;
        match e.channel {
            LeadChannel::Call => cell.calls += e.count,
            LeadChannel::Zalo => cell.zalo += e.count,
            LeadChannel::Inquiry => {
                cell.inquiries += e.count;
                if let Some(phone) = &e.requester_phone {
                    cell.people.insert(phone.clone());
                }
            }
        }
    }

    let in_categories = |c: &Option<i32>| match (&scope.categories, c) {
        (None, _) => true,
        (Some(s), Some(c)) => s.contains(c),
        (Some(_), None) => false,
    };

    let mut ordered: Vec<&CategoryAdmin> = categories.iter().collect();
    ordered.sort_by_key(|c| (c.category_type.clone(), c.display_order, c.id));
    let wanted = scope.categories.as_deref().unwrap_or_default();

    let area_rows = area_rows(
        &cells,
        &filter,
        &in_categories,
        &ordered,
        wanted,
        steady_weekly_leads,
    );
    let steady_areas = area_rows
        .iter()
        .filter(|a| a.steady && !a.nested && a.level != LEVEL_NONE)
        .count();

    // Rows: the categories of the areas in scope, or of the one in focus.
    let mut by_category = ByCategory::new();
    for ((ward, category), cell) in &cells {
        if filter.in_rows(ward.as_deref()) && in_categories(category) {
            by_category.entry(*category).or_default().add(cell);
        }
    }
    let (rows, all_row, _) = category_rows(&by_category, &ordered, wanted);

    let (searches, unmet) = search_figures(searches, &filter, &in_categories);

    DemandReportDto {
        weeks,
        areas: area_rows,
        rows,
        all: all_row,
        searches,
        unmet,
        steady_areas,
        min_count: DEMAND_REPORT_MIN_COUNT,
        steady_weekly_leads,
        with_demo,
    }
}

/// Leads by category (`None`: about no category).
type ByCategory = HashMap<Option<i32>, Cell>;

/// The report's rows for these leads: the catalogue's categories in order
/// (each of `wanted` even at zero), then the leads with no category; their
/// sum row, hidden so that no row or column sum gives a hidden figure away;
/// and the sum's raw counts.
fn category_rows(
    by_category: &ByCategory,
    ordered: &[&CategoryAdmin],
    wanted: &[i32],
) -> (Vec<DemandRowDto>, DemandRowDto, Cell) {
    let empty = Cell::default();
    let mut rows = Vec::new();
    let mut all = Cell::default();
    for c in ordered {
        let cell = by_category
            .get(&Some(c.id))
            .or_else(|| wanted.contains(&c.id).then_some(&empty));
        if let Some(cell) = cell {
            all.add(cell);
            let mut r = row(Some(c.id), Some(c.name.clone()), cell);
            r.sector = c.sector.clone();
            rows.push(r);
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
        hide_where(figure, shown_total(*raw, &column));
    }
    let totals: Vec<Figure> = rows.iter().map(|r| r.total).collect();
    hide_where(&mut all_row.total, shown_total(all.total(), &totals));
    (rows, all_row, all)
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
    ordered: &[&CategoryAdmin],
    wanted: &[i32],
    bar: i64,
) -> Vec<DemandAreaDto> {
    let units = filter.units;
    let mut by_ward: HashMap<&str, ByCategory> = HashMap::new();
    let mut by_province: HashMap<&str, ByCategory> = HashMap::new();
    let mut unplaced: Option<ByCategory> = None;
    for ((ward, category), cell) in cells {
        if !in_categories(category) {
            continue;
        }
        let add = |leads: &mut ByCategory| leads.entry(*category).or_default().add(cell);
        match ward.as_deref() {
            Some(w) => {
                add(by_ward.entry(w).or_default());
                if let Some(p) = units.province_of(w) {
                    add(by_province.entry(p).or_default());
                }
            }
            None => add(unplaced.get_or_insert_with(ByCategory::new)),
        }
    }
    let figures = |leads: Option<&ByCategory>| {
        category_rows(leads.unwrap_or(&ByCategory::new()), ordered, wanted)
    };
    let every_area = filter.scope.areas.is_none();
    let mut rows = Vec::new();
    for p in units.provinces() {
        let key = AreaKey::Province(p.code.clone());
        let leads = by_province.get(p.code.as_str());
        let province_shown = filter.listed(&key) || (every_area && leads.is_some());
        if province_shown {
            rows.push(area_dto(
                &key,
                LEVEL_PROVINCE,
                Some(p.name.clone()),
                None,
                false,
                figures(leads),
                bar,
            ));
        }
        for w in units.wards_of(&p.code) {
            let key = AreaKey::Ward(w.code.clone());
            let leads = by_ward.get(w.code.as_str());
            if filter.listed(&key) || (province_shown && leads.is_some()) {
                rows.push(area_dto(
                    &key,
                    LEVEL_WARD,
                    units.label(&key),
                    Some(w.full_name.clone()),
                    province_shown,
                    figures(leads),
                    bar,
                ));
            }
        }
    }
    if every_area || filter.listed(&AreaKey::Unplaced) {
        if let Some(leads) = &unplaced {
            rows.push(area_dto(
                &AreaKey::Unplaced,
                LEVEL_NONE,
                None,
                None,
                false,
                figures(Some(leads)),
                bar,
            ));
        }
    }
    rows
}

/// A figure as shown: hidden when it is above zero and under the minimum.
fn shown(value: i64) -> Figure {
    Figure {
        value: (value == 0 || value >= DEMAND_REPORT_MIN_COUNT).then_some(value),
        at_least: None,
    }
}

/// A total of `parts` as shown: hidden like any figure, and also when
/// exactly one part is hidden, since the total minus the shown parts would
/// give that one away; then it is at least the shown parts and one.
fn shown_total(total: i64, parts: &[Figure]) -> Figure {
    let hidden = parts.iter().filter(|f| f.value.is_none()).count();
    if hidden == 1 {
        let shown_sum: i64 = parts.iter().filter_map(|f| f.value).sum();
        Figure {
            value: None,
            at_least: Some(shown_sum + 1),
        }
    } else {
        shown(total)
    }
}

/// Hides `figure` where `check` (the same sum seen through other parts)
/// is hidden, keeping the larger "at least" of the two.
fn hide_where(figure: &mut Figure, check: Figure) {
    if check.value.is_none() {
        figure.value = None;
        figure.at_least = figure.at_least.max(check.at_least);
    }
}

fn row(category_id: Option<i32>, category: Option<String>, cell: &Cell) -> DemandRowDto {
    let weeks: Vec<Figure> = cell.weeks.iter().map(|v| shown(*v)).collect();
    let calls = shown(cell.calls);
    let zalo = shown(cell.zalo);
    let inquiries = shown(cell.inquiries);
    let mut total = shown_total(cell.total(), &weeks);
    hide_where(
        &mut total,
        shown_total(cell.total(), &[calls, zalo, inquiries]),
    );
    DemandRowDto {
        category_id,
        category,
        sector: None,
        total,
        weeks,
        calls,
        zalo,
        inquiries,
        people: shown(cell.people.len() as i64),
    }
}

/// Searches by week in the rows' areas and categories, and the words that
/// found nothing, asked at least `DEMAND_REPORT_MIN_COUNT` times.
fn search_figures(
    searches: &[SearchCount],
    filter: &AreaFilter<'_>,
    in_categories: &dyn Fn(&Option<i32>) -> bool,
) -> (Vec<Figure>, Vec<UnmetSearchDto>) {
    let mut weekly = [0_i64; DEMAND_WEEKS];
    let mut unmet: HashMap<&str, i64> = HashMap::new();
    for s in searches {
        if !filter.in_rows(s.ward_code.as_deref()) || !in_categories(&s.category_id) {
            continue;
        }
        let Some(week) = weekly.get_mut(s.week) else {
            continue;
        };
        *week += s.count;
        if let (true, Some(q)) = (s.unmet, s.query.as_deref()) {
            *unmet.entry(q).or_default() += s.count;
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

/// An area's line from its `category_rows`: the sum row its own report
/// (`?area=`, the same scope) shows, so the list gives away nothing that
/// report hides; and how many of its weeks reached the bar.
fn area_dto(
    key: &AreaKey,
    level: &str,
    label: Option<String>,
    name: Option<String>,
    nested: bool,
    (_, sum, raw): (Vec<DemandRowDto>, DemandRowDto, Cell),
    bar: i64,
) -> DemandAreaDto {
    let weeks_at_bar = raw.weeks.iter().filter(|w| **w >= bar).count();
    DemandAreaDto {
        value: key.value(),
        level: level.to_owned(),
        label,
        name,
        nested,
        weeks: sum.weeks,
        total: sum.total,
        weeks_at_bar,
        steady: weeks_at_bar == raw.weeks.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin_units::tests::units;
    use crate::constants::STEADY_WEEKLY_LEADS_DEFAULT;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0)
            .single()
            .unwrap_or_default()
    }

    /// The report week of an instant, as the database counts it: whole
    /// weeks from the first Monday midnight (Vietnam) of the report; `None`
    /// outside the report, which the database never reads.
    fn week_of(at: DateTime<Utc>) -> Option<usize> {
        let weeks = report_weeks(now()).unwrap_or_default();
        let (from, to) = window(&weeks).ok()?;
        if !(from..to).contains(&at) {
            return None;
        }
        usize::try_from((at - from).num_weeks()).ok()
    }

    /// One call tap, if `at` is in the report, as the database counts it:
    /// under the ward of its business (1 is in Cai Lậy, 28439, Đồng Tháp; 3
    /// in Long Xuyên, 30001, An Giang; 4 in Tân Phước, 28400, Đồng Tháp; 2
    /// has no ward), naming the business
    /// only when it is about no category.
    fn event(at: DateTime<Utc>, business_id: i32, category_id: Option<i32>) -> Option<LeadCount> {
        let ward_code = match business_id {
            1 => Some("28439".to_owned()),
            4 => Some("28400".to_owned()),
            3 => Some("30001".to_owned()),
            _ => None,
        };
        Some(LeadCount {
            week: week_of(at)?,
            ward_code,
            business_id: category_id.is_none().then_some(business_id),
            category_id,
            channel: LeadChannel::Call,
            count: 1,
            requester_phone: None,
        })
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
            sector: None,
            is_active: true,
            is_demo: false,
            created_on: Utc::now(),
            updated_on: Utc::now(),
            created_by_name: None,
            updated_by_name: None,
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

    /// Business 1 has one live service, in category 7.
    fn run(events: Vec<Option<LeadCount>>, scope: &ReportScope) -> DemandReportDto {
        let units = units();
        let events: Vec<LeadCount> = events.into_iter().flatten().collect();
        aggregate(Input {
            weeks: report_weeks(now()).unwrap_or_default(),
            events: &events,
            searches: &[],
            sole: &[SoleCategory {
                business_id: 1,
                category_id: 7,
            }],
            with_demo: false,
            categories: vec![category(7, "spray"), category(8, "dredge")],
            units: &units,
            scope,
            steady_weekly_leads: STEADY_WEEKLY_LEADS_DEFAULT,
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
        assert_eq!(
            report.rows.first().and_then(|r| r.total.at_least),
            Some(7),
            "never \"fewer than 5\" for a total of 9: at least the 6 shown and one"
        );
        assert_eq!(
            report
                .rows
                .first()
                .and_then(|r| r.weeks.get(DEMAND_WEEKS - 1))
                .map(|f| f.at_least),
            Some(None),
            "a small figure is just small"
        );
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
                demo: DemoScope::Real,
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
                demo: DemoScope::Real,
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
            for _ in 0..STEADY_WEEKLY_LEADS_DEFAULT {
                events.push(event(instant, 1, Some(7)));
            }
        }
        events.push(event(at(2026, 11, 3, 2), 3, Some(7)));
        let report = run(events, &ReportScope::default());
        let steady: Vec<(&str, bool, usize)> = report
            .areas
            .iter()
            .map(|a| (a.value.as_str(), a.steady, a.weeks_at_bar))
            .collect();
        assert_eq!(
            steady,
            vec![
                ("p82", true, DEMAND_WEEKS),
                ("w28439", true, DEMAND_WEEKS),
                ("p91", false, 0),
                ("w30001", false, 0)
            ]
        );
        assert_eq!(
            report.steady_areas, 1,
            "a steady ward under its province counts once"
        );
    }

    #[test]
    fn an_area_shows_by_week_what_its_own_report_sums() {
        // The last week: Cai Lậy 3 sprays (hidden) and 6 dredges, Tân Phước
        // 6 sprays; the week before, Cai Lậy 7 sprays.
        let mut events = Vec::new();
        let last = at(2026, 11, 3, 2);
        for (n, business, category) in [(3, 1, 7), (6, 1, 8), (6, 4, 7)] {
            for _ in 0..n {
                events.push(event(last, business, Some(category)));
            }
        }
        for _ in 0..7 {
            events.push(event(at(2026, 10, 27, 2), 1, Some(7)));
        }
        let whole = run(events.clone(), &ReportScope::default());
        let line = |value: &str| whole.areas.iter().find(|a| a.value == value).cloned();
        let cai_lay = line("w28439");
        assert_eq!(
            cai_lay
                .as_ref()
                .and_then(|a| a.weeks.get(DEMAND_WEEKS - 1))
                .map(|f| f.value),
            Some(None),
            "9 in all, but 9 − 6 dredges would give the 3 sprays away"
        );
        assert_eq!(
            cai_lay
                .as_ref()
                .and_then(|a| a.weeks.get(DEMAND_WEEKS - 2))
                .map(|f| f.value),
            Some(Some(7))
        );

        // Every line is its area's own report's sum row, figure for figure.
        for area in &whole.areas {
            let own = run(
                events.clone(),
                &ReportScope {
                    focus: AreaKey::parse(&area.value),
                    ..ReportScope::default()
                },
            );
            assert_eq!(area.weeks, own.all.weeks, "{}", area.value);
            assert_eq!(area.total, own.all.total, "{}", area.value);
        }
    }

    #[test]
    fn searches_that_found_nothing_are_listed_once_asked_enough() {
        // As the database counts them: the words only of what found nothing.
        let search = |query: Option<&str>, count: i64| SearchCount {
            week: week_of(at(2026, 11, 3, 2)).unwrap_or_default(),
            query: query.map(str::to_owned),
            category_id: None,
            ward_code: Some("28439".into()),
            unmet: query.is_some(),
            count,
        };
        let searches = vec![
            search(Some("drone sau rieng"), DEMAND_REPORT_MIN_COUNT),
            search(Some("may cay"), 1),
            search(None, 1),
        ];
        let units = units();
        let any = |_: &Option<i32>| true;
        let everywhere = ReportScope::default();
        let filter = AreaFilter {
            units: &units,
            scope: &everywhere,
        };
        let (weekly, unmet) = search_figures(&searches, &filter, &any);
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
        let (weekly, _) = search_figures(&searches, &filter, &any);
        assert_eq!(weekly.last().and_then(|f| f.value), Some(0));
    }
}
