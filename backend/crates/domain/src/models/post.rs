use chrono::{DateTime, Utc};
use uuid::Uuid;

/// An article: a news post (what happens now) or a guide (what stays true),
/// written by an admin in Markdown. A draft, published or hidden; it may
/// concern one sector, one province and one commodity.
#[derive(Debug, Clone)]
pub struct Post {
    pub id: i32,
    /// `news` or `guide`.
    pub kind: String,
    pub title: String,
    /// One or two sentences: the cards' line and the page's description.
    pub summary: String,
    pub body: String,
    pub cover_file_id: Option<Uuid>,
    pub sector: Option<String>,
    pub province_code: Option<String>,
    /// The province's name, when it has one.
    pub province_name: Option<String>,
    pub commodity_id: Option<i32>,
    /// The commodity's name, when it has one.
    pub commodity_name: Option<String>,
    /// `draft`, `published` or `hidden`.
    pub status: String,
    /// Set the first time it is published.
    pub published_on: Option<DateTime<Utc>>,
    pub is_pinned: bool,
    /// Loaded with the demo data.
    pub is_demo: bool,
    pub updated_on: DateTime<Utc>,
}

/// A post's boxes, validated (create and edit alike).
#[derive(Debug, Clone)]
pub struct PostFields {
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub sector: Option<String>,
    pub province_code: Option<String>,
    pub commodity_id: Option<i32>,
    pub is_pinned: bool,
}

/// What a public list of posts is narrowed to: published ones only.
#[derive(Debug, Clone, Default)]
pub struct PostQuery {
    /// `news` or `guide`; none: both.
    pub kind: Option<String>,
    /// That sector's and those of every sector; none: all.
    pub sector: Option<String>,
    /// That province's and those of the whole region; none: all.
    pub province_code: Option<String>,
    /// That commodity's only.
    pub commodity_id: Option<i32>,
    /// Leave this post out (the page's own, under "more").
    pub except: Option<i32>,
}
