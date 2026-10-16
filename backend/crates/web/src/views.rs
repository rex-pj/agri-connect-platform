//! Server-rendered HTML. `render_page` is the only way a handler produces a
//! page: it injects the shared context (site, locale) and sets the page
//! headers. Themes would attach here later; nothing else may call
//! `Tera::render` (a convention test enforces it).

use std::collections::HashMap;

use application::constants::{
    BUSINESS_ADDRESS_MAX, BUSINESS_DESCRIPTION_MAX, BUSINESS_NAME_MAX, FULL_NAME_MAX,
    PASSWORD_MIN_CHARS, PASSWORD_RESET_EXPIRY_HOURS, PRODUCT_DESCRIPTION_MAX, PRODUCT_NAME_MAX,
    PRODUCT_UNIT_MAX, USERNAME_MAX_LEN, USERNAME_MIN_LEN,
};
use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, FixedOffset, Utc};
use domain::{Error, ErrorCode};
use rust_embed::RustEmbed;
use serde::Serialize;
use tera::{Tera, Value};

use crate::{app_state::AppState, extract::ApiPath, routes, shell::Shell, slug::slugify};

#[derive(RustEmbed)]
#[folder = "templates/"]
struct TemplateFiles;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticFiles;

/// Policy for every HTML page. `script-src 'self'` holds because Alpine is
/// the CSP build (no `new Function`); swapping in the standard Alpine build
/// would need `'unsafe-eval'` and break every page instead. `style-src 'self'`
/// is why templates may not use `style=""` attributes. `blob:` in img-src is
/// for the upload preview. The JSON API keeps its stricter `default-src 'none'`.
pub const PAGE_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
     img-src 'self' blob:; font-src 'self'; connect-src 'self'; form-action 'self'; \
     frame-ancestors 'none'; base-uri 'none'; object-src 'none'";

/// Vietnam has had no daylight-saving time since 1975, so a fixed offset is
/// exact. Used only for the no-JS fallback text; `<time datetime>` stays UTC.
const VN_OFFSET_SECS: i32 = 7 * 3600;

/// Site identity, from `SITE_NAME` / `SITE_DESCRIPTION`.
#[derive(Debug, Clone, Serialize)]
pub struct SiteConfig {
    pub name: String,
    pub description: String,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            name: "Agri-Connect".to_owned(),
            description: "Tìm thợ múc bùn, phun thuốc, sửa máy nổ nhanh chóng gần bạn nhất."
                .to_owned(),
        }
    }
}

pub struct Views {
    tera: Tera,
    site: SiteConfig,
}

/// The use cases' field rules, so forms can say `minlength` without
/// repeating the numbers.
#[derive(Clone, Copy, Serialize)]
struct FieldLimits {
    password_min_chars: usize,
    username_min_len: usize,
    username_max_len: usize,
    full_name_max: usize,
    reset_link_hours: i64,
    business_name_max: usize,
    business_address_max: usize,
    business_description_max: usize,
    product_name_max: usize,
    product_unit_max: usize,
    product_description_max: usize,
}

const FIELD_LIMITS: FieldLimits = FieldLimits {
    password_min_chars: PASSWORD_MIN_CHARS,
    username_min_len: USERNAME_MIN_LEN,
    username_max_len: USERNAME_MAX_LEN,
    full_name_max: FULL_NAME_MAX,
    reset_link_hours: PASSWORD_RESET_EXPIRY_HOURS,
    business_name_max: BUSINESS_NAME_MAX,
    business_address_max: BUSINESS_ADDRESS_MAX,
    business_description_max: BUSINESS_DESCRIPTION_MAX,
    product_name_max: PRODUCT_NAME_MAX,
    product_unit_max: PRODUCT_UNIT_MAX,
    product_description_max: PRODUCT_DESCRIPTION_MAX,
};

/// What every template sees: `site`, `lang`, the frame (`shell`: nav, tabs,
/// account menu) and the handler's `page`.
#[derive(Serialize)]
struct PageContext<'a, T: Serialize> {
    site: &'a SiteConfig,
    lang: &'static str,
    limits: FieldLimits,
    shell: &'a Shell,
    page: &'a T,
}

fn asset_versions() -> HashMap<String, String> {
    StaticFiles::iter()
        .filter_map(|path| {
            let file = StaticFiles::get(&path)?;
            let hash = file.metadata.sha256_hash();
            let short: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
            Some((path.into_owned(), short))
        })
        .collect()
}

fn string_arg<'a>(
    args: &'a HashMap<String, Value>,
    key: &str,
    func: &str,
) -> tera::Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| tera::Error::msg(format!("{func}: `{key}` must be a string")))
}

impl Views {
    pub fn new(site: SiteConfig) -> Result<Self, Error> {
        let mut tera = Tera::default();
        tera.set_escape_fn(escape_html);
        let templates: Vec<(String, String)> = TemplateFiles::iter()
            .filter_map(|name| {
                let file = TemplateFiles::get(&name)?;
                let body = String::from_utf8(file.data.into_owned()).ok()?;
                Some((name.into_owned(), body))
            })
            .collect();
        tera.add_raw_templates(templates)
            .map_err(|e| Error::internal(format!("templates: {}", chain(&e))))?;

        let versions = asset_versions();
        tera.register_function("asset_url", move |args: &HashMap<String, Value>| {
            let path = string_arg(args, "path", "asset_url")?;
            // An unknown asset fails the render, so a typo never ships a page
            // that silently loads nothing.
            let version = versions
                .get(path)
                .ok_or_else(|| tera::Error::msg(format!("asset_url: no static file {path:?}")))?;
            Ok(Value::String(format!("/static/{path}?v={version}")))
        });

        tera.register_function("url_for", |args: &HashMap<String, Value>| {
            let name = string_arg(args, "name", "url_for")?;
            let params: Vec<(String, String)> = args
                .iter()
                .filter(|(k, _)| k.as_str() != "name")
                .map(|(k, v)| {
                    let value = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.clone(), value)
                })
                .collect();
            let borrowed: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            routes::url(name, &borrowed)
                .map(Value::String)
                .map_err(|e| tera::Error::msg(format!("url_for: {e}")))
        });

        // file_url(id=product.image_file_id, variant="thumb"): our own origin,
        // so pages keep img-src 'self'.
        tera.register_function("file_url", |args: &HashMap<String, Value>| {
            let id = string_arg(args, "id", "file_url")?;
            let route = match args.get("variant").and_then(Value::as_str) {
                None | Some("main") => "file",
                Some("thumb") => "file_thumb",
                Some(other) => {
                    return Err(tera::Error::msg(format!(
                        "file_url: variant {other:?} is not main or thumb"
                    )))
                }
            };
            routes::url(route, &[("id", id)])
                .map(Value::String)
                .map_err(|e| tera::Error::msg(format!("file_url: {e}")))
        });

        tera.register_filter("slugify", |value: &Value, _: &HashMap<String, Value>| {
            value
                .as_str()
                .map(|s| Value::String(slugify(s)))
                .ok_or_else(|| tera::Error::msg("slugify: expects a string"))
        });

        tera.register_filter("number", |value: &Value, _: &HashMap<String, Value>| {
            let n = value
                .as_i64()
                .ok_or_else(|| tera::Error::msg("number: expects an integer"))?;
            Ok(Value::String(format_number(n)))
        });

        tera.register_filter("vnd", |value: &Value, _: &HashMap<String, Value>| {
            let amount = value
                .as_i64()
                .ok_or_else(|| tera::Error::msg("vnd: expects an integer number of đồng"))?;
            Ok(Value::String(format_vnd(amount)))
        });

        tera.register_filter(
            "vn_datetime",
            |value: &Value, _: &HashMap<String, Value>| {
                let instant = parse_instant(value)?;
                Ok(Value::String(format_vn_datetime(instant)))
            },
        );

        tera.register_filter("utc_iso", |value: &Value, _: &HashMap<String, Value>| {
            let instant = parse_instant(value)?;
            Ok(Value::String(
                instant.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ))
        });

        Ok(Self { tera, site })
    }

    fn render<T: Serialize>(
        &self,
        shell: &Shell,
        template: &str,
        page: &T,
    ) -> Result<String, Error> {
        let context = tera::Context::from_serialize(PageContext {
            site: &self.site,
            lang: "vi",
            limits: FIELD_LIMITS,
            shell,
            page,
        })
        .map_err(|e| Error::internal(format!("page context for {template}: {}", chain(&e))))?;
        self.tera
            .render(template, &context)
            .map_err(|e| Error::internal(format!("render {template}: {}", chain(&e))))
    }
}

/// Escapes the five characters that can end a text node or a quoted attribute.
/// Tera's default also escapes `/`, which turns every URL into `&#x2F;` noise
/// without adding safety: we never emit unquoted attributes. Templates must
/// quote every attribute value for this to hold.
fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\x27' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Tera nests the useful message (missing variable, bad function arg) in
/// `source()`; the top level only says which template failed.
fn chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}

fn parse_instant(value: &Value) -> tera::Result<DateTime<Utc>> {
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("expects an RFC 3339 timestamp string"))?;
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| tera::Error::msg(format!("not an RFC 3339 timestamp: {text:?}: {e}")))
}

/// `28000` → `28.000 ₫`, the way Vietnamese shops write prices.
pub fn format_vnd(amount: i64) -> String {
    format!("{} ₫", format_number(amount))
}

/// `1234567` → `1.234.567`: Vietnamese groups thousands with dots.
pub fn format_number(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{grouped}")
}

pub fn format_vn_datetime(instant: DateTime<Utc>) -> String {
    match FixedOffset::east_opt(VN_OFFSET_SECS) {
        Some(offset) => instant
            .with_timezone(&offset)
            .format("%d/%m/%Y %H:%M")
            .to_string(),
        // Unreachable for a constant in range, but a wrong time beats a panic.
        None => instant.format("%d/%m/%Y %H:%M UTC").to_string(),
    }
}

fn page_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PAGE_CSP),
    );
    // Pages show per-user state from S2 on; a shared cache must never keep one.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
}

/// Renders `template` with the shared context and page headers.
pub fn render_page<T: Serialize>(
    state: &AppState,
    shell: &Shell,
    template: &str,
    page: &T,
) -> Result<Response, Error> {
    render_page_status(state, shell, StatusCode::OK, template, page)
}

pub fn render_page_status<T: Serialize>(
    state: &AppState,
    shell: &Shell,
    status: StatusCode,
    template: &str,
    page: &T,
) -> Result<Response, Error> {
    let html = state.views.render(shell, template, page)?;
    let mut response = (status, html).into_response();
    page_headers(response.headers_mut());
    Ok(response)
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "woff2" => "font/woff2",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

/// GET /static/{*path}. A request carrying the current `?v=` is cached for a
/// year; `asset_url` changes `v` whenever the file changes, so the cache can
/// never serve a stale stylesheet against new markup.
pub async fn static_file(
    ApiPath(path): ApiPath<String>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Result<Response, Error> {
    // Licence and provenance files are for the repository, not the web.
    if path.ends_with("VERSIONS") || path.contains("LICENSE") || path.ends_with("OFL.txt") {
        return Err(Error::new(ErrorCode::RouteNotFound));
    }
    let file = StaticFiles::get(&path).ok_or_else(|| Error::new(ErrorCode::RouteNotFound))?;
    let hash: String = file
        .metadata
        .sha256_hash()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    let versioned = query.as_deref() == Some(&format!("v={hash}"));

    let mut response = file.data.into_owned().into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(&path)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if versioned {
            "public, max-age=31536000, immutable"
        } else {
            "public, max-age=300"
        }),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_covers_text_and_quoted_attributes_but_keeps_urls_readable() {
        assert_eq!(
            escape_html(r#"<a href="x" title='y'>&/static/a.css"#),
            "&lt;a href=&quot;x&quot; title=&#x27;y&#x27;&gt;&amp;/static/a.css"
        );
    }

    #[test]
    fn vnd_groups_thousands_with_dots() {
        assert_eq!(format_vnd(0), "0 ₫");
        assert_eq!(format_vnd(28000), "28.000 ₫");
        assert_eq!(format_vnd(1_234_567), "1.234.567 ₫");
        assert_eq!(format_vnd(-5000), "-5.000 ₫");
        assert_eq!(format_number(1_234), "1.234");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn vietnam_time_is_utc_plus_seven() -> Result<(), chrono::ParseError> {
        let t = DateTime::parse_from_rfc3339("2026-10-13T17:30:00Z")?.with_timezone(&Utc);
        assert_eq!(format_vn_datetime(t), "14/10/2026 00:30");
        Ok(())
    }

    #[test]
    fn every_template_parses_and_assets_are_hashed() -> Result<(), Error> {
        let views = Views::new(SiteConfig::default())?;
        assert!(views.tera.get_template_names().count() > 0);
        assert!(asset_versions().contains_key("css/app.css"));
        Ok(())
    }

    #[test]
    fn file_url_points_at_our_own_file_routes() -> Result<(), Error> {
        let mut tera = Views::new(SiteConfig::default())?.tera;
        let mut render = |src: &str| {
            tera.render_str(src, &tera::Context::new())
                .map_err(|e| chain(&e))
        };
        assert_eq!(
            render(r#"{{ file_url(id="ab12") }}"#),
            Ok("/files/ab12".into())
        );
        assert_eq!(
            render(r#"{{ file_url(id="ab12", variant="thumb") }}"#),
            Ok("/files/ab12/thumb".into())
        );
        // A typo fails the render instead of shipping a broken image.
        assert!(render(r#"{{ file_url(id="ab12", variant="large") }}"#).is_err());
        Ok(())
    }
}
