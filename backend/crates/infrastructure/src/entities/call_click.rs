use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "call_clicks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub business_id: i32,
    pub service_id: Option<i32>,
    pub product_id: Option<i32>,
    pub channel: String,
    pub created_on: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
