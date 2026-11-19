//! Pieces shared by the list pages: lenient query parsing, query strings and
//! pagination links. Lists are plain GET forms and links, so every state
//! (filters, page) is a URL that can be shared, bookmarked or reloaded.

use application::outputs::CategoryDto;
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{
    locale::Locale,
    pages::{moved_permanently, see_other},
    shell::SectorChoice,
};

/// Pages shown either side of the current one before an ellipsis.
const PAGE_WINDOW: i64 = 2;
/// Up to this many pages, every page gets a link.
const PAGE_LINKS_MAX_ALL: i64 = 7;

/// A query value a person may have typed or edited: blank or malformed means
/// "not set", never an error page.
pub fn parse_opt<T: std::str::FromStr>(raw: &str) -> Option<T> {
    let raw = raw.trim();
    (!raw.is_empty()).then(|| raw.parse().ok()).flatten()
}

/// A price filter: whole đồng, not negative.
pub fn parse_price(raw: &str) -> Option<i64> {
    parse_opt::<i64>(raw).filter(|p| *p >= 0)
}

/// `path?k=v&…` with blank values left out, encoded like a browser submits a
/// form (`application/x-www-form-urlencoded`), so the link and the form give
/// the same URL.
pub fn with_query(path: &str, params: &[(&str, &str)]) -> String {
    let query: Vec<String> = params
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("{}={}", encode(k), encode(v.trim())))
        .collect();
    if query.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(char::from(b))
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A category chip in a filter form.
#[derive(Debug, Serialize)]
pub struct CategoryChip {
    pub id: i32,
    pub name: String,
    pub icon: String,
}

/// The categories a visitor of `sector` is offered: that sector's and those
/// of every sector; all of them when no sector is chosen. The same rule as
/// the lists' `category_in_sector`, for what the page picks from.
pub fn in_sector(categories: &[CategoryDto], sector: Option<&str>) -> Vec<CategoryDto> {
    categories
        .iter()
        .filter(|c| serves(c, sector))
        .cloned()
        .collect()
}

/// Whether `category` is offered to a visitor of `sector` (`in_sector`).
pub fn serves(category: &CategoryDto, sector: Option<&str>) -> bool {
    match (sector, category.sector.as_deref()) {
        (Some(chosen), Some(own)) => chosen == own,
        _ => true,
    }
}

/// The sector a list is narrowed to: none once a category is chosen (it is
/// the filter, and a shared link still shows its rows) or a text is typed (a
/// search finds everything, unless its own `sector=` narrows it:
/// `typed_sector`); else the visitor's. The one rule for every list.
pub fn list_sector(sector: Option<&str>, category: Option<i32>, search: &str) -> Option<String> {
    sector
        .filter(|_| category.is_none() && search.trim().is_empty())
        .map(str::to_owned)
}

/// One link of `sector_switch::sector_links`: the same search in one sector.
#[derive(Debug, Serialize)]
pub struct SectorLink {
    pub label: String,
    pub icon: String,
    pub href: String,
    pub current: bool,
}

/// The sector a typed search is narrowed to by its `sector=` (`raw`): one of
/// the shown sectors, and only while `search` has words. Lenient, like every
/// typed query value: blank or unknown is every sector. Never the cookie: a
/// search is narrowed only by what its own address says.
pub fn typed_sector(choices: &[SectorChoice], raw: &str, search: &str) -> Option<String> {
    let raw = raw.trim();
    (!search.trim().is_empty() && !raw.is_empty())
        .then(|| choices.iter().find(|s| s.value == raw))
        .flatten()
        .map(|s| s.value.clone())
}

/// The links that narrow the typed search at `path` to one sector, "every
/// sector" first, `sector` marked; `kept` are its other parameters (without
/// `sector` and the page).
pub fn sector_links(
    choices: &[SectorChoice],
    path: &str,
    kept: &[(&str, &str)],
    sector: Option<&str>,
) -> Vec<SectorLink> {
    choices
        .iter()
        .map(|s| {
            let mut params = kept.to_vec();
            params.push(("sector", s.value.as_str()));
            SectorLink {
                label: s.label.clone(),
                icon: s.icon.clone(),
                href: with_query(path, &params),
                current: sector.unwrap_or_default() == s.value,
            }
        })
        .collect()
}

pub fn chips(categories: &[CategoryDto]) -> Vec<CategoryChip> {
    categories
        .iter()
        .map(|c| CategoryChip {
            id: c.id,
            name: c.name.clone(),
            icon: c.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct SortChip {
    pub value: &'static str,
    pub label: String,
}

/// The price inputs, as typed (only lists that have prices).
#[derive(Debug, Serialize)]
pub struct PriceRange {
    pub min: String,
    pub max: String,
}

/// The search-and-filter form of a list page, as the visitor left it. One
/// shape for every list, so one organism (`list_filters`) renders them all.
#[derive(Debug, Serialize)]
pub struct FilterForm {
    pub action: String,
    pub placeholder: String,
    pub q: String,
    /// Query name of the category radio group (`category_id`, `service_id`).
    pub category_param: &'static str,
    pub category_label: String,
    pub category_id: Option<i32>,
    pub categories: Vec<CategoryChip>,
    pub price: Option<PriceRange>,
    pub sort: &'static str,
    pub sorts: Vec<SortChip>,
    /// How many filters besides the search text are set (badge on the toggle).
    pub active: usize,
    /// The list with the filters cleared but the search text kept.
    pub reset_href: String,
    /// Narrowings the form has no box for, sent back as they are (the
    /// supplies' commodity), as (name, value).
    pub hidden: Vec<(&'static str, String)>,
}

/// The category `id` among `categories` (already narrowed to one type), or
/// a 404: a service id on a product page does not exist there.
pub fn find_category(categories: &[CategoryDto], id: i32) -> Result<CategoryDto, Error> {
    categories
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| Error::new(ErrorCode::CategoryNotFound))
}

/// 301 to the canonical category URL when `path` is another spelling of it,
/// keeping the page number.
pub fn redirect_to_canonical(
    path: &str,
    canonical: &str,
    page: i64,
) -> Option<Result<axum::response::Response, Error>> {
    if path == canonical {
        return None;
    }
    let page = page.to_string();
    let keep: &[(&str, &str)] = if page != "1" { &[("page", &page)] } else { &[] };
    Some(moved_permanently(&with_query(canonical, keep)))
}

/// A page number past the last page (an old link after listings were
/// removed, or a typed one): 303 to the last page, keeping the filters,
/// instead of an empty list. `None` when `page` is a page that exists.
pub fn past_the_end(
    path: &str,
    params: &[(&str, &str)],
    page: i64,
    total_pages: i64,
) -> Option<Result<axum::response::Response, Error>> {
    if total_pages < 1 || page <= total_pages {
        return None;
    }
    let last = total_pages.to_string();
    let mut all = params.to_vec();
    if total_pages > 1 {
        all.push(("page", &last));
    }
    Some(see_other(&with_query(path, &all)))
}

/// The hero line of a category page: its description and how many results,
/// worded by `count_key` (a text with a `{count}`).
pub fn category_sub(
    locale: &Locale,
    category: &CategoryDto,
    total: i64,
    count_key: &str,
) -> String {
    let count = (total > 0).then(|| locale.text_with(count_key, &[("count", &total)]));
    [category.description.clone(), count]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PageLink {
    /// `None` for an ellipsis.
    pub number: Option<i64>,
    pub href: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Pagination {
    pub prev: Option<String>,
    pub next: Option<String>,
    pub pages: Vec<PageLink>,
}

impl Pagination {
    /// `None` when everything fits on one page. `params` are the other query
    /// parameters to keep (filters); page 1 has no `page` parameter, so it
    /// shares its URL with the unpaginated list.
    pub fn new(path: &str, params: &[(&str, &str)], current: i64, total: i64) -> Option<Self> {
        if total <= 1 {
            return None;
        }
        let href = |n: i64| {
            let page = n.to_string();
            let mut all = params.to_vec();
            if n > 1 {
                all.push(("page", &page));
            }
            with_query(path, &all)
        };
        let pages = page_numbers(current, total)
            .into_iter()
            .map(|n| PageLink {
                href: n.map(&href).unwrap_or_default(),
                current: n == Some(current),
                number: n,
            })
            .collect();
        Some(Self {
            prev: (current > 1).then(|| href(current - 1)),
            next: (current < total).then(|| href(current + 1)),
            pages,
        })
    }
}

/// First, last, and a window around `current`, with `None` for each gap.
fn page_numbers(current: i64, total: i64) -> Vec<Option<i64>> {
    if total <= PAGE_LINKS_MAX_ALL {
        return (1..=total).map(Some).collect();
    }
    let low = (current - PAGE_WINDOW).max(2);
    let high = (current + PAGE_WINDOW).min(total - 1);
    let mut shown = vec![1];
    shown.extend(low..=high);
    shown.push(total);
    let mut out = Vec::with_capacity(shown.len() + 2);
    let mut last = 0;
    for n in shown {
        if n - last > 1 && last > 0 {
            out.push(None);
        }
        out.push(Some(n));
        last = n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_are_lenient() {
        assert_eq!(parse_opt::<i32>(" 4 "), Some(4));
        assert_eq!(parse_opt::<i32>("bốn"), None);
        assert_eq!(parse_opt::<i32>(""), None);
        assert_eq!(parse_price("-5"), None);
        assert_eq!(parse_price("28000"), Some(28000));
    }

    #[test]
    fn query_strings_match_what_a_form_submits() {
        assert_eq!(
            with_query(
                "/products",
                &[("q", "phân bón"), ("sort", ""), ("page", "2")]
            ),
            "/products?q=ph%C3%A2n+b%C3%B3n&page=2"
        );
        assert_eq!(with_query("/products", &[("q", "  ")]), "/products");
        assert_eq!(with_query("/p", &[("q", "a&b=c")]), "/p?q=a%26b%3Dc");
    }

    #[test]
    fn page_numbers_keep_both_ends_and_a_window() {
        let n = |v: &[i64]| {
            v.iter()
                .map(|&x| if x == 0 { None } else { Some(x) })
                .collect::<Vec<_>>()
        };
        assert_eq!(page_numbers(1, 5), n(&[1, 2, 3, 4, 5]));
        assert_eq!(page_numbers(1, 20), n(&[1, 2, 3, 0, 20]));
        assert_eq!(page_numbers(10, 20), n(&[1, 0, 8, 9, 10, 11, 12, 0, 20]));
        assert_eq!(page_numbers(20, 20), n(&[1, 0, 18, 19, 20]));
        assert_eq!(page_numbers(4, 20), n(&[1, 2, 3, 4, 5, 6, 0, 20]));
    }

    #[test]
    fn a_page_past_the_end_goes_to_the_last_one() {
        let location = |page, total| {
            past_the_end("/products", &[("q", "gạo")], page, total)
                .and_then(Result::ok)
                .map(|r| {
                    r.headers()[axum::http::header::LOCATION]
                        .to_str()
                        .unwrap_or_default()
                        .to_owned()
                })
        };
        assert_eq!(
            location(9, 3),
            Some("/products?q=g%E1%BA%A1o&page=3".into())
        );
        assert_eq!(location(4, 1), Some("/products?q=g%E1%BA%A1o".into()));
        assert_eq!(location(3, 3), None);
        assert_eq!(location(5, 0), None, "an empty list has no last page");
    }

    #[test]
    fn pagination_links_keep_filters_and_drop_page_one() {
        assert_eq!(Pagination::new("/products", &[], 1, 1), None);
        let p = Pagination::new("/products", &[("q", "gạo")], 2, 3);
        let p = p.as_ref();
        assert_eq!(
            p.and_then(|p| p.prev.clone()),
            Some("/products?q=g%E1%BA%A1o".into())
        );
        assert_eq!(
            p.and_then(|p| p.next.clone()),
            Some("/products?q=g%E1%BA%A1o&page=3".into())
        );
        assert_eq!(
            p.map(|p| p.pages.iter().filter(|l| l.current).count()),
            Some(1)
        );
    }
}
