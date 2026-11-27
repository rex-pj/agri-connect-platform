use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// `news` or `guide`.
    pub kind: String,
    pub title: String,
    pub summary: String,
    /// Markdown.
    #[sea_orm(column_type = "Text")]
    pub body: String,
    pub cover_file_id: Option<Uuid>,
    pub sector: Option<String>,
    pub province_code: Option<String>,
    pub commodity_id: Option<i32>,
    /// `draft`, `published` or `hidden`.
    pub status: String,
    pub published_on: Option<DateTimeWithTimeZone>,
    pub is_pinned: bool,
    /// Loaded with the demo data; removed with it.
    pub is_demo: bool,
    pub created_on: DateTimeWithTimeZone,
    pub updated_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
    pub updated_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
