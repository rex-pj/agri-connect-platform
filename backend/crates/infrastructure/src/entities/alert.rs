use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "alerts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// `disease`, `salinity` or `weather`.
    pub kind: String,
    /// The sector it concerns; none: every sector.
    pub sector: Option<String>,
    /// The province it concerns; none: the whole region.
    pub province_code: Option<String>,
    pub title: String,
    pub body: String,
    pub starts_on: Date,
    pub ends_on: Date,
    pub is_active: bool,
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
