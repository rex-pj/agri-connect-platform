//! Server-rendered HTML. `render_page` is the only way a handler produces a
//! page: it injects the shared context (site, locale) and sets the page
//! headers. Themes would attach here later; nothing else may call
//! `Tera::render` (a convention test enforces it).

use std::{collections::HashMap, sync::Arc};

use application::constants::{
    AUDIT_REASON_MAX, BUSINESS_ADDRESS_MAX, BUSINESS_DESCRIPTION_MAX, BUSINESS_NAME_MAX,
    FULL_NAME_MAX, PASSWORD_MIN_CHARS, PASSWORD_RESET_EXPIRY_HOURS, PRODUCT_DESCRIPTION_MAX,
    PRODUCT_NAME_MAX, PRODUCT_UNIT_MAX, USERNAME_MAX_LEN, USERNAME_MIN_LEN,
};
use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Datelike, FixedOffset, Utc};
use domain::{Error, ErrorCode};
use rust_embed::RustEmbed;
use serde::Serialize;
use tera::{Tera, Value};

use crate::{
    app_state::AppState,
    constants::EXPLORE_DEFAULT_RADIUS_M,
    extract::ApiPath,
    locale::{fill, Lang, Locale, Locales},
    routes,
    shell::Shell,
    slug::slugify,
};

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

/// Site identity, from `SITE_NAME` / `SITE_DESCRIPTION` (unset: the locale's
/// `site.description`), and the platform's own help lines for the footer
/// (`SITE_SUPPORT_*`); one left unset is not shown.
#[derive(Debug, Clone, Serialize)]
pub struct SiteConfig {
    pub name: String,
    pub description: Option<String>,
    pub support_phone: Option<String>,
    pub support_zalo_url: Option<String>,
    pub support_email: Option<String>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            name: "Agri-Connect".to_owned(),
            description: None,
            support_phone: None,
            support_zalo_url: None,
            support_email: None,
        }
    }
}

/// One template set per language: `t()` and the `distance` filter are bound
/// to their locale, and a page renders with the set of its request's language.
pub struct Views {
    vi: Tera,
    en: Tera,
    site: SiteConfig,
    locales: Arc<Locales>,
}

/// One entry of the language switch.
#[derive(Serialize)]
struct LanguageOption {
    code: &'static str,
    /// The language named in itself ("English", "Tiếng Việt").
    name: String,
    current: bool,
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
    audit_reason_max: usize,
    /// The near-me radius before the visitor picks one.
    near_me_km: i64,
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
    audit_reason_max: AUDIT_REASON_MAX,
    near_me_km: EXPLORE_DEFAULT_RADIUS_M / 1000,
};

/// What every template sees: `site`, `lang` (and `languages` for the
/// switch), this `year` in Vietnam (the
/// footer's copyright), the frame (`shell`: nav, tabs, account menu) and the
/// handler's `page`.
#[derive(Serialize)]
struct PageContext<'a, T: Serialize> {
    site: &'a SiteConfig,
    lang: &'static str,
    languages: Vec<LanguageOption>,
    year: i32,
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

/// A `t()` or `url_for()` argument as text: strings as they are, numbers as
/// written.
fn param_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Views {
    pub fn new(site: SiteConfig, locales: Arc<Locales>) -> Result<Self, Error> {
        Ok(Self {
            vi: Self::build(Arc::clone(locales.get(Lang::Vi)))?,
            en: Self::build(Arc::clone(locales.get(Lang::En)))?,
            site,
            locales,
        })
    }

    fn tera(&self, lang: Lang) -> &Tera {
        match lang {
            Lang::Vi => &self.vi,
            Lang::En => &self.en,
        }
    }

    /// Every template, with the functions and filters bound to `locale`.
    fn build(locale: Arc<Locale>) -> Result<Tera, Error> {
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

        // t(key="nav.home", count=3): the locale's text with its `{count}`
        // filled. An unknown key fails the render, like url_for, so a typo
        // never ships a page with a missing word.
        let texts = Arc::clone(&locale);
        tera.register_function("t", move |args: &HashMap<String, Value>| {
            let key = string_arg(args, "key", "t")?;
            let template = texts
                .lookup(key)
                .ok_or_else(|| tera::Error::msg(format!("t: no text for {key:?}")))?;
            let params = args
                .iter()
                .filter(|(k, _)| k.as_str() != "key")
                .map(|(k, v)| (k.as_str(), param_text(v)));
            Ok(Value::String(fill(template, params)))
        });

        tera.register_function("url_for", |args: &HashMap<String, Value>| {
            let name = string_arg(args, "name", "url_for")?;
            let params: Vec<(String, String)> = args
                .iter()
                .filter(|(k, _)| k.as_str() != "name")
                .map(|(k, v)| (k.clone(), param_text(v)))
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
                .ok_or_else(|| tera::Error::msg("vnd: expects an integer number of dong"))?;
            Ok(Value::String(format_vnd(amount)))
        });

        tera.register_filter("price_note", |value: &Value, _: &HashMap<String, Value>| {
            value
                .as_str()
                .map(|s| Value::String(format_price_note(s)))
                .ok_or_else(|| tera::Error::msg("price_note: expects a string"))
        });

        let distances = Arc::clone(&locale);
        tera.register_filter(
            "distance",
            move |value: &Value, _: &HashMap<String, Value>| {
                value
                    .as_f64()
                    .map(|m| Value::String(format_distance(&distances, m)))
                    .ok_or_else(|| tera::Error::msg("distance: expects metres"))
            },
        );

        tera.register_filter("phone", |value: &Value, _: &HashMap<String, Value>| {
            value
                .as_str()
                .map(|s| Value::String(format_phone(s)))
                .ok_or_else(|| tera::Error::msg("phone: expects a string"))
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

        Ok(tera)
    }

    fn render<T: Serialize>(
        &self,
        shell: &Shell,
        template: &str,
        page: &T,
    ) -> Result<String, Error> {
        let lang = crate::locale::current();
        let locale = self.locales.get(lang);
        let languages = Lang::ALL
            .into_iter()
            .map(|l| LanguageOption {
                code: l.code(),
                name: locale.text(&format!("language.{}", l.code())),
                current: l == lang,
            })
            .collect();
        let context = tera::Context::from_serialize(PageContext {
            site: &self.site,
            lang: lang.code(),
            languages,
            year: vn_year(Utc::now()),
            limits: FIELD_LIMITS,
            shell,
            page,
        })
        .map_err(|e| Error::internal(format!("page context for {template}: {}", chain(&e))))?;
        self.tera(lang)
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

/// Typical motorbike speed on rural roads, for the ride-time estimate.
const MOTORBIKE_KMH: f64 = 30.0;

/// Metres under a kilometre, otherwise kilometres with one decimal and the
/// ride time: farmers judge a provider by "how long to get here".
pub fn format_distance(locale: &Locale, metres: f64) -> String {
    if metres < 1000.0 {
        return locale.text_with("distance.metres", &[("metres", &metres.round())]);
    }
    let km = metres / 1000.0;
    let minutes = (km / MOTORBIKE_KMH * 60.0).round();
    locale.text_with(
        "distance.km_ride",
        &[("km", &format!("{km:.1}")), ("minutes", &minutes)],
    )
}

/// `28000` → `28.000 ₫`, the way Vietnamese shops write prices.
pub fn format_vnd(amount: i64) -> String {
    format!("{} ₫", format_number(amount))
}

/// A service's free-text price as shown: a bare amount ("100000", "100.000")
/// reads as `100.000 ₫`; anything with words ("Từ 500.000đ/công", "Liên
/// hệ") is the provider's own and shown as typed.
pub fn format_price_note(note: &str) -> String {
    let note = note.trim();
    let digits: String = note
        .chars()
        .filter(|c| !matches!(c, '.' | ',' | ' '))
        .collect();
    match digits.parse::<i64>() {
        Ok(amount) if digits.chars().all(|c| c.is_ascii_digit()) => format_vnd(amount),
        _ => note.to_owned(),
    }
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

/// `0912345678` → `0912 345 678`, easy to read aloud or dial by hand. Any
/// other shape is shown as stored.
pub fn format_phone(phone: &str) -> String {
    let p = phone.trim();
    if p.len() == 10 && p.bytes().all(|b| b.is_ascii_digit()) {
        format!("{} {} {}", &p[..4], &p[4..7], &p[7..])
    } else {
        p.to_owned()
    }
}

/// The calendar year in Vietnam at `instant`.
pub fn vn_year(instant: DateTime<Utc>) -> i32 {
    match FixedOffset::east_opt(VN_OFFSET_SECS) {
        Some(offset) => instant.with_timezone(&offset).year(),
        None => instant.year(),
    }
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
    // Pages show per-user state; a shared cache must never keep one.
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
    serve_static(&path, query.as_deref())
}

/// GET /favicon.ico: the address browsers and crawlers try without reading
/// the page's `<link rel="icon">`. Unversioned, so cached briefly.
pub async fn favicon() -> Result<Response, Error> {
    serve_static(FAVICON, None)
}

/// The static file the root `/favicon.ico` serves.
pub const FAVICON: &str = "favicon.ico";

/// The root address of the favicon; never a page, never needs a session.
pub const FAVICON_PATH: &str = "/favicon.ico";

fn serve_static(path: &str, query: Option<&str>) -> Result<Response, Error> {
    // Licence and provenance files are for the repository, not the web.
    if path.ends_with("VERSIONS") || path.contains("LICENSE") || path.ends_with("OFL.txt") {
        return Err(Error::new(ErrorCode::RouteNotFound));
    }
    let file = StaticFiles::get(path).ok_or_else(|| Error::new(ErrorCode::RouteNotFound))?;
    let hash: String = file
        .metadata
        .sha256_hash()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    let versioned = query == Some(&format!("v={hash}"));

    let mut response = file.data.into_owned().into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
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
    fn a_bare_amount_in_a_price_note_reads_as_dong() {
        assert_eq!(format_price_note("100000"), "100.000 ₫");
        assert_eq!(format_price_note(" 100.000 "), "100.000 ₫");
        assert_eq!(format_price_note("Từ 500.000đ/công"), "Từ 500.000đ/công");
        assert_eq!(format_price_note("Liên hệ"), "Liên hệ");
        assert_eq!(format_price_note("-5"), "-5");
        assert_eq!(format_price_note(""), "");
    }

    #[test]
    fn phones_are_grouped_four_three_three() {
        assert_eq!(format_phone("0912345678"), "0912 345 678");
        assert_eq!(format_phone(" 0912345678 "), "0912 345 678");
        assert_eq!(format_phone("+84912345678"), "+84912345678");
    }

    #[test]
    fn vietnam_time_is_utc_plus_seven() -> Result<(), chrono::ParseError> {
        let t = DateTime::parse_from_rfc3339("2026-10-13T17:30:00Z")?.with_timezone(&Utc);
        assert_eq!(format_vn_datetime(t), "14/10/2026 00:30");
        // New Year's Eve in UTC is already New Year's Day in Vietnam.
        let eve = DateTime::parse_from_rfc3339("2026-12-31T18:00:00Z")?.with_timezone(&Utc);
        assert_eq!(vn_year(eve), 2027);
        Ok(())
    }

    fn vi() -> Result<Arc<Locale>, Error> {
        Locale::vi()
            .map(Arc::new)
            .map_err(|e| Error::internal(e.to_string()))
    }

    fn views(site: SiteConfig) -> Result<Views, Error> {
        let locales = Locales::load().map_err(|e| Error::internal(e.to_string()))?;
        Views::new(site, Arc::new(locales))
    }

    #[test]
    fn each_language_renders_its_own_words() -> Result<(), Error> {
        let mut views = views(SiteConfig::default())?;
        let src = r#"{{ t(key="nav.home") }}"#;
        let ctx = tera::Context::new();
        let vi = views
            .vi
            .render_str(src, &ctx)
            .map_err(|e| Error::internal(chain(&e)))?;
        let en = views
            .en
            .render_str(src, &ctx)
            .map_err(|e| Error::internal(chain(&e)))?;
        assert_eq!(en, "Home");
        assert_ne!(vi, en);
        Ok(())
    }

    #[test]
    fn distances_round_and_add_the_ride_time() -> Result<(), Error> {
        let vi = vi()?;
        let ride = |km: &str, minutes: u32| {
            vi.text_with("distance.km_ride", &[("km", &km), ("minutes", &minutes)])
        };
        assert_eq!(
            format_distance(&vi, 420.4),
            vi.text_with("distance.metres", &[("metres", &420)])
        );
        assert_eq!(format_distance(&vi, 2_300.0), ride("2.3", 5));
        assert_eq!(format_distance(&vi, 15_000.0), ride("15.0", 30));
        Ok(())
    }

    #[test]
    fn every_template_parses_and_assets_are_hashed() -> Result<(), Error> {
        let views = views(SiteConfig::default())?;
        assert!(views.vi.get_template_names().count() > 0);
        assert!(asset_versions().contains_key("css/app.css"));
        assert!(asset_versions().contains_key(FAVICON));
        Ok(())
    }

    #[test]
    fn t_fills_placeholders_and_refuses_an_unknown_key() -> Result<(), Error> {
        let vi = vi()?;
        let mut tera = views(SiteConfig::default())?.vi;
        let mut render = |src: &str| {
            tera.render_str(src, &tera::Context::new())
                .map_err(|e| chain(&e))
        };
        assert_eq!(
            render(r#"{{ t(key="pagination.page", number=4) }}"#),
            Ok(vi.text_with("pagination.page", &[("number", &4)]))
        );
        assert!(render(r#"{{ t(key="no.such.key") }}"#).is_err());
        Ok(())
    }

    #[test]
    fn the_footer_offers_the_help_lines_that_are_set() -> Result<(), Error> {
        let vi = vi()?;
        let footer = |site: SiteConfig| -> Result<String, Error> {
            let mut views = views(site.clone())?;
            let mut ctx = tera::Context::new();
            ctx.insert("shell", &Shell::new(&vi, "/", None, None));
            ctx.insert("site", &site);
            ctx.insert("year", &2026);
            views
                .vi
                .render_str(
                    r#"{% import "organisms/app_footer.html" as f %}{{ f::app_footer(shell=shell, site=site, year=year) }}"#,
                    &ctx,
                )
                .map_err(|e| Error::internal(chain(&e)))
        };
        let bare = footer(SiteConfig::default())?;
        assert!(bare.contains("© 2026 Agri-Connect"));
        assert!(
            bare.contains(&vi.text("account_menu.register_provider")),
            "an anonymous visitor is invited to sell"
        );
        assert!(
            !bare.contains(&vi.text("footer.help_title")),
            "no help lines configured"
        );

        let helped = footer(SiteConfig {
            support_phone: Some("0912345678".to_owned()),
            support_email: Some("hotro@example.vn".to_owned()),
            ..SiteConfig::default()
        })?;
        assert!(helped.contains(r#"href="tel:0912345678""#));
        assert!(helped.contains("0912 345 678"));
        assert!(helped.contains("mailto:hotro@example.vn"));
        assert!(
            !helped.contains(&vi.text("footer.zalo")),
            "Zalo was not set"
        );
        Ok(())
    }

    #[test]
    fn file_url_points_at_our_own_file_routes() -> Result<(), Error> {
        let mut tera = views(SiteConfig::default())?.vi;
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
