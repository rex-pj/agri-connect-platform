//! GET /products — search and filter all products — and
//! GET /products/category/{id}/{slug} — one category's products.
//!
//! Both are plain GET forms and links: every filter and page is in the URL.

use application::outputs::{CategoryDto, ProductBrowseResponse};
use axum::{extract::State, response::Response};
use domain::{
    models::product::{ProductFilter, ProductSort},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    pages::{
        listing::{
            category_sub, chips, find_category, parse_opt, parse_price, past_the_end,
            redirect_to_canonical, with_query, FilterForm, Pagination, PriceRange, SortChip,
        },
        parse_id,
    },
    routes,
    shell::Shell,
    views::render_page,
};

const PRODUCT_CATEGORY_TYPE: &str = "product";

/// Everything as text: a hand-edited URL must give a page, not a 400.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub q: String,
    pub category_id: String,
    pub min_price: String,
    pub max_price: String,
    pub sort: String,
    pub page: String,
}

#[derive(Serialize)]
pub struct ProductsPage {
    pub title: String,
    pub icon: String,
    pub sub: String,
    pub back_href: String,
    pub back_label: String,
    /// Set on the category page.
    pub category: Option<CategoryDto>,
    /// Set on /products only; the category page has no filter panel.
    pub filters: Option<FilterForm>,
    pub result: ProductBrowseResponse,
    pub pagination: Option<Pagination>,
    /// Any filter or search is set: the empty state offers to clear them.
    pub filtered: bool,
}

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("products links: {e}")))
}

fn sort_label(sort: ProductSort) -> &'static str {
    match sort {
        ProductSort::Newest => "sort.newest",
        ProductSort::PriceAsc => "sort.price_asc",
        ProductSort::PriceDesc => "sort.price_desc",
    }
}

/// GET /products
pub async fn list(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let categories = state
        .usecases
        .explore
        .get_categories(Some(PRODUCT_CATEGORY_TYPE))
        .await?;
    // Only a category that exists filters; a stale id shows everything.
    let category_id =
        parse_opt::<i32>(&q.category_id).filter(|id| categories.iter().any(|c| c.id == *id));
    let min_price = parse_price(&q.min_price);
    let max_price = parse_price(&q.max_price);
    let sort = ProductSort::parse(Some(q.sort.trim()));
    let search = q.q.trim().to_owned();

    let result = state
        .usecases
        .product
        .browse_products(
            ProductFilter {
                category_id,
                name: (!search.is_empty()).then(|| search.clone()),
                min_price,
                max_price,
            },
            sort,
            parse_opt(&q.page).unwrap_or(1),
        )
        .await?;

    let path = url("products", &[])?;
    let category_text = category_id.map(|id| id.to_string()).unwrap_or_default();
    let min_text = min_price.map(|p| p.to_string()).unwrap_or_default();
    let max_text = max_price.map(|p| p.to_string()).unwrap_or_default();
    let sort_param = if sort == ProductSort::default() {
        ""
    } else {
        sort.as_str()
    };
    let kept = [
        ("q", search.as_str()),
        ("category_id", category_text.as_str()),
        ("min_price", min_text.as_str()),
        ("max_price", max_text.as_str()),
        ("sort", sort_param),
    ];
    let active = usize::from(category_id.is_some())
        + usize::from(min_price.is_some() || max_price.is_some())
        + usize::from(sort != ProductSort::default());

    if let Some(redirect) = past_the_end(&path, &kept, result.page, result.total_pages) {
        return redirect;
    }
    let page = ProductsPage {
        title: state.locale().text("products.title"),
        icon: "fa-bag-shopping".to_owned(),
        sub: state.locale().text_with(
            "products.sub",
            &[(
                "categories",
                &categories
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" · "),
            )],
        ),
        back_href: url("home", &[])?,
        back_label: state.locale().text("nav.home"),
        category: None,
        pagination: Pagination::new(&path, &kept, result.page, result.total_pages),
        filtered: !search.is_empty() || active > 0,
        filters: Some(FilterForm {
            reset_href: with_query(&path, &[("q", search.as_str())]),
            action: path,
            placeholder: state.locale().text("products.search_placeholder"),
            q: search,
            category_param: "category_id",
            category_label: state.locale().text("filters.category"),
            category_id,
            categories: chips(&categories),
            price: Some(PriceRange {
                min: min_text,
                max: max_text,
            }),
            sort: sort.as_str(),
            sorts: ProductSort::ALL
                .into_iter()
                .map(|s| SortChip {
                    value: s.as_str(),
                    label: state.locale().text(sort_label(s)),
                })
                .collect(),
            active,
        }),
        result,
    };
    render_page(&state, &shell, "pages/products.html", &page)
}

/// GET /products/category/{id}/{slug}
pub async fn category(
    shell: Shell,
    State(state): State<AppState>,
    ApiPath((raw_id, _slug)): ApiPath<(String, String)>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let categories = state
        .usecases
        .explore
        .get_categories(Some(PRODUCT_CATEGORY_TYPE))
        .await?;
    let category = find_category(&categories, id)?;
    let canonical = url(
        "product_category",
        &[("id", &id.to_string()), ("slug", &category.slug)],
    )?;
    let page_number: i64 = parse_opt(&q.page).unwrap_or(1);
    if let Some(redirect) = redirect_to_canonical(&shell.path, &canonical, page_number) {
        return redirect;
    }

    let result = state
        .usecases
        .product
        .browse_products(
            ProductFilter {
                category_id: Some(id),
                ..ProductFilter::default()
            },
            ProductSort::default(),
            page_number,
        )
        .await?;

    if let Some(redirect) = past_the_end(&canonical, &[], result.page, result.total_pages) {
        return redirect;
    }
    let page = ProductsPage {
        title: category.name.clone(),
        icon: category.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        sub: category_sub(state.locale(), &category, result.total, "products.count"),
        back_href: url("products", &[])?,
        back_label: state.locale().text("nav.products"),
        pagination: Pagination::new(&canonical, &[], result.page, result.total_pages),
        filtered: false,
        filters: None,
        category: Some(category),
        result,
    };
    render_page(&state, &shell, "pages/products.html", &page)
}
