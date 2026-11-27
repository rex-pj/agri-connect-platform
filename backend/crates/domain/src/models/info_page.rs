use chrono::{DateTime, NaiveDate, Utc};

/// A page of what the platform is and the rules it keeps: the terms, the
/// privacy and personal data policies, the rules for providers and
/// listings, the complaints and takedown procedures, the legal entity, the
/// contact page. Its text is kept in versions (`InfoRevision`), the one in
/// force shown.
#[derive(Debug, Clone)]
pub struct InfoPage {
    pub id: i32,
    /// Its address (`/info/{code}`), a slug; never changes once made.
    pub code: String,
    pub title: String,
    pub icon: Option<String>,
    /// Who sees it, as stored: `on`, `preview` or `off`.
    pub stage: String,
    /// Listed in the footer, under `footer_group` (`about` or `policy`).
    pub in_footer: bool,
    pub footer_group: String,
    pub display_order: i32,
    /// Loaded with the demo data.
    pub is_demo: bool,
    pub updated_on: DateTime<Utc>,
}

/// A page's boxes, validated (create and edit alike; the code is only
/// stored when the page is made).
#[derive(Debug, Clone)]
pub struct InfoPageFields {
    pub code: String,
    pub title: String,
    pub icon: Option<String>,
    pub in_footer: bool,
    pub footer_group: String,
    pub display_order: i32,
}

/// One version of a page's text, in Markdown, in force from `effective_on`
/// until a later one comes into force. Never edited: a correction is a new
/// version.
#[derive(Debug, Clone)]
pub struct InfoRevision {
    pub id: i32,
    pub page_id: i32,
    pub body: String,
    pub effective_on: NaiveDate,
    pub change_note: Option<String>,
    pub created_on: DateTime<Utc>,
}

/// A new version's boxes, validated.
#[derive(Debug, Clone)]
pub struct NewInfoRevision {
    pub body: String,
    pub effective_on: NaiveDate,
    pub change_note: Option<String>,
}
