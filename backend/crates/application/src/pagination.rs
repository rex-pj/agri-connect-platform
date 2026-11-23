use std::future::Future;

use domain::Result;

use crate::constants::{ADMIN_MAX_PAGE, ADMIN_PAGE_SIZE, MAX_PAGE, PAGE_SIZE, PUBLIC_MAX_PAGE};

/// Clamps `page` to `[1, MAX_PAGE]` and returns `(page, offset)`.
pub fn paginate(page: i64, page_size: i64) -> (i64, i64) {
    let page = page.clamp(1, MAX_PAGE);
    let offset = (page - 1) * page_size;
    (page, offset)
}

/// Ceiling-divides `total` by `page_size` to get the number of pages.
pub fn total_pages(total: i64, page_size: i64) -> i64 {
    (total + page_size - 1) / page_size
}

/// A page of a public list: `page` clamped to `[1, PUBLIC_MAX_PAGE]`, with
/// its offset.
pub fn paginate_public(page: i64) -> (i64, i64) {
    paginate(page.min(PUBLIC_MAX_PAGE), PAGE_SIZE)
}

/// The pages a public list of `total` rows offers: never past
/// `PUBLIC_MAX_PAGE`, so no link leads where `paginate_public` will not go.
pub fn public_total_pages(total: i64) -> i64 {
    total_pages(total, PAGE_SIZE).min(PUBLIC_MAX_PAGE)
}

/// The total of a list when the page just read tells it: a page that is not
/// full is the last one, and an empty first page is an empty list. `None`:
/// the page is full (or past the end), and the rows must be counted.
pub fn total_from_page(offset: i64, rows: usize, page_size: i64) -> Option<i64> {
    let rows = i64::try_from(rows).ok()?;
    match rows {
        0 if offset == 0 => Some(0),
        0 => None,
        n if n < page_size => Some(offset + n),
        _ => None,
    }
}

/// The total of a list whose page at `offset` has `rows` rows: what the page
/// tells (`total_from_page`), else what `count` counts. Every paged list
/// reads its page first and asks this, so most pages cost one query.
pub async fn total_or_count<Fut>(
    offset: i64,
    rows: usize,
    page_size: i64,
    count: impl FnOnce() -> Fut,
) -> Result<i64>
where
    Fut: Future<Output = Result<i64>>,
{
    match total_from_page(offset, rows, page_size) {
        Some(total) => Ok(total),
        None => count().await,
    }
}

/// A page of an admin list: `page` clamped to `[1, ADMIN_MAX_PAGE]`, with
/// its offset.
pub fn paginate_admin(page: i64) -> (i64, i64) {
    paginate(page.min(ADMIN_MAX_PAGE), ADMIN_PAGE_SIZE)
}

/// One page of a short list read whole and already narrowed (a search over a
/// catalogue): `rows` are all the matches in their order.
pub fn admin_page_of<T: serde::Serialize>(
    rows: Vec<T>,
    page: i64,
) -> crate::outputs::AdminPagedResponse<T> {
    let (page, offset) = paginate_admin(page);
    let total = i64::try_from(rows.len()).unwrap_or(i64::MAX);
    let items = rows
        .into_iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(ADMIN_PAGE_SIZE).unwrap_or(0))
        .collect();
    crate::outputs::AdminPagedResponse::admin(items, total, page)
}

/// The pages an admin list of `total` rows offers: never past
/// `ADMIN_MAX_PAGE`.
pub fn admin_total_pages(total: i64) -> i64 {
    total_pages(total, ADMIN_PAGE_SIZE).min(ADMIN_MAX_PAGE)
}

/// Puts a short list read whole in the order an admin asked for: by name
/// (case ignored), the newest made first (the highest id) or the oldest. No
/// order asked leaves it as it came (the list's own).
pub fn order_admin_list<T>(
    rows: &mut [T],
    sort: Option<domain::models::admin_list::AdminSort>,
    name: impl Fn(&T) -> &str,
    id: impl Fn(&T) -> i32,
) {
    use domain::models::admin_list::AdminSort;
    match sort {
        Some(AdminSort::Name) => rows.sort_by_key(|r| (name(r).to_lowercase(), id(r))),
        Some(AdminSort::Newest) => rows.sort_by_key(|r| std::cmp::Reverse(id(r))),
        Some(AdminSort::Oldest) => rows.sort_by_key(|r| id(r)),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_public_list_stops_at_its_last_page() {
        assert_eq!(paginate_public(0), (1, 0));
        assert_eq!(paginate_public(3), (3, 2 * PAGE_SIZE));
        assert_eq!(
            paginate_public(PUBLIC_MAX_PAGE + 7),
            (PUBLIC_MAX_PAGE, (PUBLIC_MAX_PAGE - 1) * PAGE_SIZE)
        );
        assert_eq!(public_total_pages(PAGE_SIZE + 1), 2);
        assert_eq!(
            public_total_pages(PAGE_SIZE * PUBLIC_MAX_PAGE * 3),
            PUBLIC_MAX_PAGE
        );
    }

    #[test]
    fn an_admin_list_stops_at_its_last_page() {
        assert_eq!(paginate_admin(2), (2, ADMIN_PAGE_SIZE));
        assert_eq!(paginate_admin(ADMIN_MAX_PAGE + 1).0, ADMIN_MAX_PAGE);
        assert_eq!(
            admin_total_pages(ADMIN_PAGE_SIZE * ADMIN_MAX_PAGE * 2),
            ADMIN_MAX_PAGE
        );
    }

    #[tokio::test]
    async fn a_list_is_counted_only_when_its_page_does_not_tell() {
        let told = total_or_count(0, 7, 20, || async {
            Err(domain::Error::internal("must not be counted"))
        });
        assert_eq!(told.await.ok(), Some(7));
        let counted = total_or_count(0, 20, 20, || async { Ok(95) });
        assert_eq!(counted.await.ok(), Some(95));
    }

    #[test]
    fn a_page_that_is_not_full_tells_the_total() {
        assert_eq!(total_from_page(0, 0, 20), Some(0), "an empty list");
        assert_eq!(total_from_page(0, 7, 20), Some(7));
        assert_eq!(total_from_page(40, 3, 20), Some(43), "the last page");
        assert_eq!(total_from_page(0, 20, 20), None, "a full page: count");
        assert_eq!(total_from_page(40, 0, 20), None, "past the end: count");
    }
}
