use sea_orm::entity::prelude::*;

/// The taps of one week on one business, type and channel, counted from the
/// `call_clicks` rows that were then deleted.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "call_click_weeks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// The Monday (Vietnam) the week starts on.
    pub week_starts_on: Date,
    pub business_id: i32,
    pub service_type_id: Option<i32>,
    pub supply_type_id: Option<i32>,
    pub channel: String,
    pub taps: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
