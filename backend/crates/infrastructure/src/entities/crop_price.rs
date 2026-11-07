use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "crop_prices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub crop_id: i32,
    pub location_id: i32,
    pub observed_on: Date,
    pub price_low: i64,
    pub price_high: i64,
    pub note: Option<String>,
    pub created_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
