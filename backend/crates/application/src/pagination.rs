use crate::constants::MAX_PAGE;

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
