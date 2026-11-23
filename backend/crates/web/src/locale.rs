//! User-facing text, resolved from machine codes and keys.
//!
//! `[errors]` holds the text of every `domain::ErrorCode`; `[text]` every
//! other word the app shows, in nested tables read as dotted keys
//! (`[text.nav] home` is `nav.home`). Templates say `t(key="nav.home")`, Rust
//! says `locale.text("nav.home")`; no Vietnamese is written anywhere else.
//!
//! The files are compiled in, so a deploy can never ship without them; the
//! tests below fail when a code has no text or two locale files disagree on
//! their keys, and a convention test fails when a key used in code or a
//! template has no text.

use std::{collections::HashMap, fmt::Display, sync::Arc};

use domain::ErrorCode;
use serde::Deserialize;

const VI: &str = include_str!("../locales/vi.toml");
const EN: &str = include_str!("../locales/en.toml");

/// A language the site is written in. Vietnamese is the default: the people
/// the site is for read it, whatever language their phone is set to, so the
/// browser's `Accept-Language` is not consulted; only an explicit choice
/// (the `lang` cookie, set by the language switch) changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Vi,
    En,
}

impl Lang {
    /// In the order the language switch lists them.
    pub const ALL: [Lang; 2] = [Lang::Vi, Lang::En];
    pub const DEFAULT: Lang = Lang::Vi;

    /// The BCP 47 tag: cookie value, `<html lang>` and the switch's field.
    pub const fn code(self) -> &'static str {
        match self {
            Lang::Vi => "vi",
            Lang::En => "en",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }
}

tokio::task_local! {
    static CURRENT: Lang;
}

/// The language of the request being served (`language::scope_request` sets
/// it around every request); the default outside one, e.g. in a background
/// task.
pub fn current() -> Lang {
    CURRENT.try_with(|l| *l).unwrap_or(Lang::DEFAULT)
}

/// Runs `f` with `lang` as the current language.
pub async fn scope<F: std::future::Future>(lang: Lang, f: F) -> F::Output {
    CURRENT.scope(lang, f).await
}

/// Every locale, compiled in.
#[derive(Debug)]
pub struct Locales {
    vi: Arc<Locale>,
    en: Arc<Locale>,
}

impl Locales {
    /// Fails only if a locale file is malformed, which the tests catch first.
    pub fn load() -> Result<Self, toml::de::Error> {
        Ok(Self {
            vi: Arc::new(Locale::vi()?),
            en: Arc::new(Locale::en()?),
        })
    }

    pub fn get(&self, lang: Lang) -> &Arc<Locale> {
        match lang {
            Lang::Vi => &self.vi,
            Lang::En => &self.en,
        }
    }

    /// The current request's locale.
    pub fn current(&self) -> &Arc<Locale> {
        self.get(current())
    }
}

#[derive(Debug, Deserialize)]
struct LocaleFile {
    errors: HashMap<String, String>,
    text: toml::Table,
}

#[derive(Debug)]
pub struct Locale {
    /// The language tag pages declare in `<html lang>`.
    lang: &'static str,
    errors: HashMap<String, String>,
    text: HashMap<String, String>,
}

/// `[text]`'s nested tables as `a.b.c` keys. Anything but a string or a
/// table is a mistake in the file.
fn flatten(
    prefix: &str,
    table: &toml::Table,
    out: &mut HashMap<String, String>,
) -> Result<(), toml::de::Error> {
    for (name, value) in table {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        match value {
            toml::Value::String(s) => {
                out.insert(key, s.clone());
            }
            toml::Value::Table(t) => flatten(&key, t, out)?,
            _ => {
                return Err(<toml::de::Error as serde::de::Error>::custom(format!(
                    "text key {key} must be a string or a table"
                )))
            }
        }
    }
    Ok(())
}

/// `template` with each `{name}` replaced by its value.
pub fn fill<'a>(template: &str, params: impl IntoIterator<Item = (&'a str, String)>) -> String {
    params
        .into_iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), &value)
        })
}

impl Locale {
    /// Vietnamese. Fails only if `vi.toml` is malformed, which the tests catch
    /// before a build ships.
    pub fn vi() -> Result<Self, toml::de::Error> {
        Self::parse(Lang::Vi.code(), VI)
    }

    pub fn en() -> Result<Self, toml::de::Error> {
        Self::parse(Lang::En.code(), EN)
    }

    fn parse(lang: &'static str, src: &str) -> Result<Self, toml::de::Error> {
        let file: LocaleFile = toml::from_str(src)?;
        let mut text = HashMap::new();
        flatten("", &file.text, &mut text)?;
        Ok(Self {
            lang,
            errors: file.errors,
            text,
        })
    }

    pub fn lang(&self) -> &'static str {
        self.lang
    }

    /// Text for `code` with `{name}` placeholders filled from `params`. Falls
    /// back to the wire code rather than an empty message, so a gap is visible.
    pub fn error_text(&self, code: ErrorCode, params: &[(&'static str, String)]) -> String {
        let Some(template) = self.errors.get(code.as_str()) else {
            return code.as_str().to_owned();
        };
        fill(template, params.iter().map(|(n, v)| (*n, v.clone())))
    }

    /// The raw text of `key`, placeholders unfilled; `None` when the file has
    /// no such key. Templates' `t()` fails the render on `None`.
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.text.get(key).map(String::as_str)
    }

    /// The text of `key`, or `fallback` when the file has none: for names
    /// looked up by a code stored in the database (a role, a permission),
    /// where a row the catalogue no longer has keeps its stored name.
    pub fn label(&self, key: &str, fallback: &str) -> String {
        self.lookup(key).unwrap_or(fallback).to_owned()
    }

    /// Every `[text]` key, dotted.
    pub fn text_keys(&self) -> impl Iterator<Item = &str> {
        self.text.keys().map(String::as_str)
    }

    /// The text of `key`. Falls back to the key itself, so a gap is visible
    /// on the page instead of failing it; the convention tests catch it first.
    pub fn text(&self, key: &str) -> String {
        self.text_with(key, &[])
    }

    /// The text of `key` with `{name}` placeholders filled from `params`.
    pub fn text_with(&self, key: &str, params: &[(&str, &dyn Display)]) -> String {
        match self.lookup(key) {
            Some(template) => fill(template, params.iter().map(|(n, v)| (*n, v.to_string()))),
            None => {
                tracing::warn!(key, "no text for this key");
                key.to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn keys(src: &str) -> Result<BTreeSet<String>, toml::de::Error> {
        let locale = Locale::parse("test", src)?;
        Ok(locale
            .errors
            .into_keys()
            .map(|k| format!("errors.{k}"))
            .chain(locale.text.into_keys().map(|k| format!("text.{k}")))
            .collect())
    }

    #[test]
    fn every_error_code_has_vietnamese_text() -> Result<(), toml::de::Error> {
        let vi = Locale::vi()?;
        let missing: Vec<_> = ErrorCode::ALL
            .iter()
            .map(|c| c.as_str())
            .filter(|c| !vi.errors.contains_key(*c))
            .collect();
        assert!(missing.is_empty(), "vi.toml has no text for {missing:?}");
        Ok(())
    }

    #[test]
    fn locale_has_no_stale_error_keys() -> Result<(), toml::de::Error> {
        let codes: BTreeSet<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        let stale: Vec<_> = Locale::vi()?
            .errors
            .into_keys()
            .filter(|k| !codes.contains(k.as_str()))
            .collect();
        assert!(
            stale.is_empty(),
            "vi.toml has keys for codes that no longer exist: {stale:?}"
        );
        Ok(())
    }

    #[test]
    fn all_locale_files_share_one_key_set() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let reference = keys(VI)?;
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let other = keys(&std::fs::read_to_string(&path)?)?;
                assert_eq!(other, reference, "{} differs from vi.toml", path.display());
            }
        }
        Ok(())
    }

    #[test]
    fn placeholders_are_filled() -> Result<(), toml::de::Error> {
        let vi = Locale::vi()?;
        let text = vi.error_text(ErrorCode::PasswordTooShort, &[("min", "8".into())]);
        assert!(text.contains('8') && !text.contains("{min}"), "{text}");
        let page = vi.text_with("pagination.page", &[("number", &3)]);
        assert!(page.contains('3') && !page.contains("{number}"), "{page}");
        Ok(())
    }

    #[test]
    fn nested_tables_read_as_dotted_keys_and_gaps_show_the_key() -> Result<(), toml::de::Error> {
        let locale = Locale::parse(
            "test",
            "[errors]\n[text.nav]\nhome = \"Home\"\n[text.a.b]\nc = \"C\"\n",
        )?;
        assert_eq!(locale.lookup("nav.home"), Some("Home"));
        assert_eq!(locale.text("a.b.c"), "C");
        assert_eq!(locale.text("nav.nope"), "nav.nope");
        assert!(Locale::parse("test", "[errors]\n[text]\nn = 1\n").is_err());
        Ok(())
    }
}
