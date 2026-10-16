//! GET /services — the provider directory, searchable and filterable by
//! service — and GET /services/category/{id}/{slug} — one service's providers.
//!
//! Plain GET forms and links, like the product lists. Contacting a provider
//! happens on their page (S7), one tap away from each card.

use application::{inputs::BrowseParams, outputs::CategoryDto};
use axum::{extract::State, response::Response};
use domain::{models::business::ProviderSort, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    pages::{
        cards::ProviderCard,
        listing::{
            category_sub, chips, find_category, parse_opt, past_the_end, redirect_to_canonical,
            with_query, FilterForm, Pagination, SortChip,
        },
        parse_id,
    },
    routes,
    shell::Shell,
    views::render_page,
};

const SERVICE_CATEGORY_TYPE: &str = "service";

/// Everything as text: a hand-edited URL must give a page, not a 400.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub q: String,
    pub service_id: String,
    pub sort: String,
    pub page: String,
}

#[derive(Serialize)]
pub struct ServicesPage {
    pub title: String,
    pub icon: String,
    pub sub: String,
    pub back_href: String,
    pub back_label: &'static str,
    /// The near-me banner's second line.
    pub geo_sub: &'static str,
    pub category: Option<CategoryDto>,
    pub filters: Option<FilterForm>,
    pub providers: Vec<ProviderCard>,
    pub total: i64,
    pub pagination: Option<Pagination>,
    pub filtered: bool,
}

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("services links: {e}")))
}

fn sort_label(sort: ProviderSort) -> &'static str {
    match sort {
        ProviderSort::VerifiedFirst => "Ưu tiên xác minh",
        ProviderSort::Newest => "Mới nhất",
        ProviderSort::NameAsc => "A → Z",
    }
}

/// GET /services
pub async fn list(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let categories = explore.get_categories(Some(SERVICE_CATEGORY_TYPE)).await?;
    let service_id =
        parse_opt::<i32>(&q.service_id).filter(|id| categories.iter().any(|c| c.id == *id));
    let sort = ProviderSort::parse(Some(q.sort.trim()));
    let search = q.q.trim().to_owned();

    let result = explore
        .browse_providers(BrowseParams {
            service_id,
            name: (!search.is_empty()).then(|| search.clone()),
            sort,
            page: parse_opt(&q.page).unwrap_or(1),
        })
        .await?;

    let path = url("services", &[])?;
    let service_text = service_id.map(|id| id.to_string()).unwrap_or_default();
    let sort_param = if sort == ProviderSort::default() {
        ""
    } else {
        sort.as_str()
    };
    let kept = [
        ("q", search.as_str()),
        ("service_id", service_text.as_str()),
        ("sort", sort_param),
    ];
    let active = usize::from(service_id.is_some()) + usize::from(sort != ProviderSort::default());

    if let Some(redirect) = past_the_end(&path, &kept, result.page, result.total_pages) {
        return redirect;
    }
    let page = ServicesPage {
        title: "Danh mục dịch vụ".to_owned(),
        icon: "fa-border-all".to_owned(),
        sub: "Tất cả nhà cung cấp — không cần bật định vị".to_owned(),
        back_href: url("home", &[])?,
        back_label: "Trang chủ",
        geo_sub: "Cơ khí · Phun thuốc · Múc bùn — trong 10 km",
        category: None,
        pagination: Pagination::new(&path, &kept, result.page, result.total_pages),
        filtered: !search.is_empty() || active > 0,
        filters: Some(FilterForm {
            reset_href: with_query(&path, &[("q", search.as_str())]),
            action: path,
            placeholder: "Tìm nhà cung cấp...",
            q: search,
            category_param: "service_id",
            category_label: "Loại dịch vụ",
            category_id: service_id,
            categories: chips(&categories),
            price: None,
            sort: sort.as_str(),
            sorts: ProviderSort::ALL
                .into_iter()
                .map(|s| SortChip {
                    value: s.as_str(),
                    label: sort_label(s),
                })
                .collect(),
            active,
        }),
        total: result.total,
        providers: result.items.into_iter().map(ProviderCard::from).collect(),
    };
    render_page(&state, &shell, "pages/services.html", &page)
}

/// GET /services/category/{id}/{slug}
pub async fn category(
    shell: Shell,
    State(state): State<AppState>,
    ApiPath((raw_id, _slug)): ApiPath<(String, String)>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let explore = &state.usecases.explore;
    let categories = explore.get_categories(Some(SERVICE_CATEGORY_TYPE)).await?;
    let category = find_category(&categories, id)?;
    let canonical = url(
        "service_category",
        &[("id", &id.to_string()), ("slug", &category.slug)],
    )?;
    let page_number: i64 = parse_opt(&q.page).unwrap_or(1);
    if let Some(redirect) = redirect_to_canonical(&shell.path, &canonical, page_number) {
        return redirect;
    }

    let result = explore
        .browse_providers(BrowseParams {
            service_id: Some(id),
            name: None,
            sort: ProviderSort::default(),
            page: page_number,
        })
        .await?;

    if let Some(redirect) = past_the_end(&canonical, &[], result.page, result.total_pages) {
        return redirect;
    }
    let page = ServicesPage {
        title: category.name.clone(),
        icon: category.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        sub: category_sub(&category, result.total, "nhà cung cấp"),
        back_href: url("services", &[])?,
        back_label: "Dịch vụ",
        geo_sub: "Trong 10 km — bật định vị để lọc theo khoảng cách",
        pagination: Pagination::new(&canonical, &[], result.page, result.total_pages),
        filtered: false,
        filters: None,
        category: Some(category),
        total: result.total,
        providers: result.items.into_iter().map(ProviderCard::from).collect(),
    };
    render_page(&state, &shell, "pages/services.html", &page)
}
