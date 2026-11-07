use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "partner_contracts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub partner_id: i32,
    pub location_id: i32,
    pub starts_on: Date,
    pub ends_on: Date,
    pub monthly_fee: i64,
    pub note: Option<String>,
    pub created_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
