//! The pieces every admin list's query shares: the order it was asked for
//! (`AdminSort`, else the list's own), the search by phone, and the shown /
//! on switches behind the state chips. One definition each, so the list and
//! its count can never disagree and every list orders the same way.

use domain::models::admin_list::{
    AdminFilter, AdminSort, LIST_STATE_HIDDEN, LIST_STATE_OFF, LIST_STATE_ON, LIST_STATE_SHOWN,
};
use sea_orm::{sea_query::Expr, ColumnTrait, Condition, QueryOrder};

use crate::search::escape_like;

/// Fewest digits a phone search takes: fewer are in nearly every number.
pub const PHONE_SEARCH_MIN_DIGITS: usize = 3;

/// The columns an admin list orders by when asked: when it was made, its
/// name, its id (the tiebreaker that keeps pages from repeating rows).
pub struct OrderColumns<C, N, I> {
    pub created: C,
    pub name: N,
    pub id: I,
}

/// `query` ordered by `sort`, else by `default` (which ends with its own id
/// tiebreaker); every asked order ends with the id, in its direction.
pub fn admin_order<Q, C, N, I>(
    query: Q,
    sort: Option<AdminSort>,
    columns: OrderColumns<C, N, I>,
    default: impl FnOnce(Q) -> Q,
) -> Q
where
    Q: QueryOrder,
    C: ColumnTrait,
    N: ColumnTrait,
    I: ColumnTrait,
{
    match sort {
        None => default(query),
        Some(AdminSort::Newest) => query
            .order_by_desc(columns.created)
            .order_by_desc(columns.id),
        Some(AdminSort::Oldest) => query.order_by_asc(columns.created).order_by_asc(columns.id),
        Some(AdminSort::Name) => query.order_by_asc(columns.name).order_by_asc(columns.id),
    }
}

/// The digits of a search typed as a phone number (spaces, dots, dashes
/// and a leading `+84` allowed, read as `0`), when there are enough of them.
pub fn phone_digits(search: &str) -> Option<String> {
    let typed = search.trim();
    let typed = typed.strip_prefix("+84").map_or_else(
        || typed.to_owned(),
        |rest| format!("0{}", rest.trim_start()),
    );
    let digits: String = typed
        .chars()
        .filter(|c| !matches!(c, ' ' | '.' | '-'))
        .collect();
    (digits.len() >= PHONE_SEARCH_MIN_DIGITS && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some(digits)
}

/// `column` (a phone, stored as its digits) contains `digits`.
pub fn phone_contains(column: impl ColumnTrait, digits: &str) -> Condition {
    Condition::all().add(Expr::col(column).like(format!("%{}%", escape_like(digits))))
}

/// The rows a shown / hidden chip keeps, by their `shown` switch; any other
/// state keeps every row.
pub fn shown_state(column: impl ColumnTrait, state: Option<&str>) -> Condition {
    match state {
        Some(LIST_STATE_SHOWN) => Condition::all().add(column.eq(true)),
        Some(LIST_STATE_HIDDEN) => Condition::all().add(column.eq(false)),
        _ => Condition::all(),
    }
}

/// The rows an on / off chip keeps, by their `active` switch.
pub fn switch_state(column: impl ColumnTrait, state: Option<&str>) -> Condition {
    match state {
        Some(LIST_STATE_ON) => Condition::all().add(column.eq(true)),
        Some(LIST_STATE_OFF) => Condition::all().add(column.eq(false)),
        _ => Condition::all(),
    }
}

/// The rows made within the days the filter names (Vietnam time, both ends
/// included), by `column` (the table's `created_on`).
pub fn created_between(
    column: impl sea_orm::sea_query::IntoColumnRef,
    filter: &AdminFilter,
) -> Condition {
    created_within(column, filter.created_from, filter.created_to)
}

/// `created_between` for a pair of days that is not an `AdminFilter`'s.
pub fn created_within(
    column: impl sea_orm::sea_query::IntoColumnRef,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> Condition {
    use chrono::{Duration, FixedOffset, NaiveTime, TimeZone};
    let vn = FixedOffset::east_opt(7 * 3600);
    let start_of = |day: chrono::NaiveDate| {
        vn.and_then(|tz| {
            tz.from_local_datetime(&day.and_time(NaiveTime::MIN))
                .single()
        })
    };
    let column = column.into_column_ref();
    let mut cond = Condition::all();
    if let Some(at) = from.and_then(start_of) {
        cond = cond.add(Expr::col(column.clone()).gte(at));
    }
    if let Some(at) = to
        .and_then(|d| d.checked_add_signed(Duration::days(1)))
        .and_then(start_of)
    {
        cond = cond.add(Expr::col(column).lt(at));
    }
    cond
}

/// How long a waiting record (a review not shown, a new request, an open
/// post) may sit before the admin's "waiting long" filter shows it: the
/// `stale_N` of the filter's `gap`, N days (3 or 7).
pub fn stale_cutoff(filter: &AdminFilter) -> Option<chrono::DateTime<chrono::Utc>> {
    let days: i64 = match filter.gap.as_deref()? {
        "stale_3" => 3,
        "stale_7" => 7,
        _ => return None,
    };
    Some(chrono::Utc::now() - chrono::Duration::days(days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_is_read_from_how_it_is_typed() {
        assert_eq!(phone_digits("0912 345 678").as_deref(), Some("0912345678"));
        assert_eq!(phone_digits("+84 912.345").as_deref(), Some("0912345"));
        assert_eq!(phone_digits("345").as_deref(), Some("345"));
        assert_eq!(phone_digits("09"), None);
        assert_eq!(phone_digits("Ba Tư"), None);
        assert_eq!(phone_digits("ST25"), None);
    }
}
