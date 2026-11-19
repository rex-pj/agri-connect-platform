use chrono::{DateTime, Utc};

/// A setting an admin moved off its default, as stored: its key in the
/// registry (`application::settings`) and its value as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSetting {
    pub key: String,
    pub value: String,
    pub updated_on: DateTime<Utc>,
}

/// One change to the stored settings: `Some` stores the value, `None` drops
/// the row (the setting is back at its default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingWrite {
    pub key: String,
    pub value: Option<String>,
}
