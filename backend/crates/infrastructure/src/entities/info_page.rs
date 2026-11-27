use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "info_pages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// Its address (`/info/{code}`); never changes.
    pub code: String,
    pub title: String,
    pub icon: Option<String>,
    /// `on`, `preview` or `off`.
    pub stage: String,
    pub in_footer: bool,
    /// `about` or `policy`.
    pub footer_group: String,
    pub display_order: i32,
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
