use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "search_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub location_id: Option<i32>,
    pub results: i32,
    pub created_on: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
