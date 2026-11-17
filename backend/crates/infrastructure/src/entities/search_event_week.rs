use sea_orm::entity::prelude::*;

/// The searches of one week with one type filter in one ward, counted from
/// the `search_events` rows that were then deleted; the words are not kept.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "search_event_weeks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// The Monday (Vietnam) the week starts on.
    pub week_starts_on: Date,
    pub service_type_id: Option<i32>,
    pub supply_type_id: Option<i32>,
    pub ward_code: Option<String>,
    pub searches: i32,
    /// How many of them found nothing.
    pub unmet: i32,
    /// Counted from demo searches: removed with the demo data.
    pub is_demo: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
