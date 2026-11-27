//! /articles, /articles/news, /articles/guides and /articles/{id}/{slug}:
//! the news (what happens now) and the guides (what stays true), as the
//! admin published them (`application::usecases::post_usecase`).
//!
//! Each tab is an address of its own, so each is found by a search engine:
//! the hub previews both kinds, a kind's tab lists it paged. A list follows
//! the visitor's sector where its switch shows, and takes a province
//! (`?province=`); one article's page is `/articles/{id}/{slug of its
//! title}`, every other spelling 301s there. The whole part is the articles
//! feature (`features::Feature::Articles`): off, the gate answers 404.

use application::{
    constants::RELATED_POSTS_SHOWN,
    outputs::{NamedCodeDto, PostDto},
    posts::{KIND_GUIDE, KIND_NEWS},
};
use axum::{extract::State, response::Response};
use domain::{models::post::PostQuery, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    pages::{
        canonical_url,
        listing::{parse_opt, past_the_end, Pagination},
        moved_permanently, parse_id,
    },
    routes,
    shell::Shell,
    slug::slugify,
    views::render_page,
};

/// The articles a hub's section shows before "see all".
const HUB_SHOWN: i64 = 4;

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("article links: {e}")))
}

/// One article as a card or a line: its own page's address with it.
#[derive(Serialize)]
pub struct ArticleCard {
    #[serde(flatten)]
    pub post: PostDto,
    pub href: String,
}

pub fn cards(posts: Vec<PostDto>) -> Result<Vec<ArticleCard>, Error> {
    posts
        .into_iter()
        .map(|post| {
            Ok(ArticleCard {
                href: canonical_url("article", post.id, &post.title)?,
                post,
            })
        })
        .collect()
}

/// One of the part's tabs, each an address of its own.
#[derive(Serialize)]
pub struct ArticleTab {
    pub code: &'static str,
    pub href: String,
    pub current: bool,
}

/// The tabs: every kind, the news, the guides; `current` the code shown
/// (`all` for the hub).
fn tabs(current: &str) -> Result<Vec<ArticleTab>, Error> {
    [
        ("all", "articles"),
        (KIND_NEWS, "articles_news"),
        (KIND_GUIDE, "articles_guides"),
    ]
    .into_iter()
    .map(|(code, route)| {
        Ok(ArticleTab {
            code,
            href: url(route, &[])?,
            current: code == current,
        })
    })
    .collect()
}

#[derive(Serialize)]
struct HubSection {
    kind: &'static str,
    href: String,
    items: Vec<ArticleCard>,
}

#[derive(Serialize)]
struct HubPage {
    tabs: Vec<ArticleTab>,
    sections: Vec<HubSection>,
}

/// GET /articles
pub async fn hub(shell: Shell, State(state): State<AppState>) -> Result<Response, Error> {
    let query = |kind: &str| PostQuery {
        kind: Some(kind.to_owned()),
        sector: shell.sector.clone(),
        ..PostQuery::default()
    };
    let posts = &state.usecases.post;
    let (news_query, guide_query) = (query(KIND_NEWS), query(KIND_GUIDE));
    let (news, guides) = tokio::try_join!(
        posts.latest(&news_query, HUB_SHOWN),
        posts.latest(&guide_query, HUB_SHOWN),
    )?;
    let page = HubPage {
        tabs: tabs("all")?,
        sections: vec![
            HubSection {
                kind: KIND_NEWS,
                href: url("articles_news", &[])?,
                items: cards(news)?,
            },
            HubSection {
                kind: KIND_GUIDE,
                href: url("articles_guides", &[])?,
                items: cards(guides)?,
            },
        ],
    };
    render_page(&state, &shell, "pages/articles.html", &page)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    /// One province's articles and the whole region's.
    pub province: String,
}

#[derive(Serialize)]
struct ListPage {
    kind: &'static str,
    tabs: Vec<ArticleTab>,
    items: Vec<ArticleCard>,
    total: i64,
    provinces: Vec<NamedCodeDto>,
    province: String,
    /// The province's name in the title, when one is chosen.
    province_name: Option<String>,
    action: String,
    pagination: Option<Pagination>,
}

async fn list(
    shell: Shell,
    state: AppState,
    kind: &'static str,
    route: &str,
    q: ListQuery,
) -> Result<Response, Error> {
    let provinces = state.usecases.post.provinces().await?;
    let province = provinces
        .iter()
        .find(|p| p.code == q.province.trim())
        .map(|p| p.code.clone());
    let query = PostQuery {
        kind: Some(kind.to_owned()),
        sector: shell.sector.clone(),
        province_code: province.clone(),
        ..PostQuery::default()
    };
    let page_no = parse_opt::<i64>(&q.page).unwrap_or(1);
    let found = state.usecases.post.public_page(&query, page_no).await?;
    let path = url(route, &[])?;
    let province = province.unwrap_or_default();
    let params: Vec<(&str, &str)> = if province.is_empty() {
        Vec::new()
    } else {
        vec![("province", province.as_str())]
    };
    if let Some(redirect) = past_the_end(&path, &params, found.page, found.total_pages) {
        return redirect;
    }
    let page = ListPage {
        kind,
        tabs: tabs(kind)?,
        total: found.total,
        items: cards(found.items)?,
        province_name: provinces
            .iter()
            .find(|p| p.code == province)
            .map(|p| p.name.clone()),
        provinces,
        pagination: Pagination::new(&path, &params, found.page, found.total_pages),
        province,
        action: path,
    };
    render_page(&state, &shell, "pages/article_list.html", &page)
}

/// GET /articles/news
pub async fn news(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    list(shell, state, KIND_NEWS, "articles_news", q).await
}

/// GET /articles/guides
pub async fn guides(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    list(shell, state, KIND_GUIDE, "articles_guides", q).await
}

/// GET /articles/{id}: 301 to its own address.
pub async fn by_id(
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    let post = state.usecases.post.public_get(id).await?;
    moved_permanently(&canonical_url("article", post.id, &post.title)?)
}

#[derive(Serialize)]
struct ArticlePage {
    post: PostDto,
    /// Its kind's list, the way back.
    back_href: String,
    /// The price board of its commodity and the supplies for it, when it
    /// is about one (each only while that part is shown).
    prices_href: Option<String>,
    supplies_href: Option<String>,
    related: Vec<ArticleCard>,
    canonical: String,
}

/// GET /articles/{id}/{slug}
pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiPath((raw_id, slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    let post = state.usecases.post.public_get(id).await?;
    let canonical = canonical_url("article", post.id, &post.title)?;
    if slug != slugify(&post.title) {
        return moved_permanently(&canonical);
    }
    let back_href = if post.kind == KIND_GUIDE {
        url("articles_guides", &[])?
    } else {
        url("articles_news", &[])?
    };
    let commodity = post.commodity_id.map(|c| c.to_string());
    let with_commodity = |route: &str| -> Result<Option<String>, Error> {
        let Some(c) = commodity.as_deref() else {
            return Ok(None);
        };
        let href = format!("{}?commodity={c}", url(route, &[])?);
        Ok(shell.shows_href(&href).then_some(href))
    };
    let related = state
        .usecases
        .post
        .latest(
            &PostQuery {
                kind: Some(post.kind.clone()),
                except: Some(post.id),
                commodity_id: post.commodity_id,
                ..PostQuery::default()
            },
            RELATED_POSTS_SHOWN,
        )
        .await?;
    let page = ArticlePage {
        prices_href: with_commodity("prices")?,
        supplies_href: with_commodity("products")?,
        related: cards(related)?,
        back_href,
        canonical,
        post,
    };
    render_page(&state, &shell, "pages/article.html", &page)
}
