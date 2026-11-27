use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "info_page_revisions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub page_id: i32,
    /// Markdown.
    #[sea_orm(column_type = "Text")]
    pub body: String,
    pub effective_on: Date,
    pub change_note: Option<String>,
    pub created_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
