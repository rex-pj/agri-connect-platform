//! User-facing text, resolved from machine codes.
//!
//! The files are compiled in, so a deploy can never ship without them; the
//! tests below fail when a code has no text or two locale files disagree on
//! their keys.

use std::collections::HashMap;

use domain::ErrorCode;
use serde::Deserialize;

const VI: &str = include_str!("../locales/vi.toml");

#[derive(Debug, Deserialize)]
struct LocaleFile {
    errors: HashMap<String, String>,
}

#[derive(Debug)]
pub struct Locale {
    errors: HashMap<String, String>,
}

impl Locale {
    /// Vietnamese, the only locale today. Fails only if `vi.toml` is malformed,
    /// which the tests catch before a build ships.
    pub fn vi() -> Result<Self, toml::de::Error> {
        Self::parse(VI)
    }

    fn parse(src: &str) -> Result<Self, toml::de::Error> {
        let file: LocaleFile = toml::from_str(src)?;
        Ok(Self {
            errors: file.errors,
        })
    }

    /// Text for `code` with `{name}` placeholders filled from `params`. Falls
    /// back to the wire code rather than an empty message, so a gap is visible.
    pub fn error_text(&self, code: ErrorCode, params: &[(&'static str, String)]) -> String {
        let Some(template) = self.errors.get(code.as_str()) else {
            return code.as_str().to_owned();
        };
        params.iter().fold(template.clone(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn keys(src: &str) -> Result<BTreeSet<String>, toml::de::Error> {
        Ok(Locale::parse(src)?.errors.into_keys().collect())
    }

    #[test]
    fn every_error_code_has_vietnamese_text() -> Result<(), toml::de::Error> {
        let vi = keys(VI)?;
        let missing: Vec<_> = ErrorCode::ALL
            .iter()
            .map(|c| c.as_str())
            .filter(|c| !vi.contains(*c))
            .collect();
        assert!(missing.is_empty(), "vi.toml has no text for {missing:?}");
        Ok(())
    }

    #[test]
    fn locale_has_no_stale_keys() -> Result<(), toml::de::Error> {
        let codes: BTreeSet<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        let stale: Vec<_> = keys(VI)?
            .into_iter()
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
        let text = Locale::vi()?.error_text(ErrorCode::PasswordTooShort, &[("min", "8".into())]);
        assert!(text.contains('8') && !text.contains("{min}"), "{text}");
        Ok(())
    }
}
