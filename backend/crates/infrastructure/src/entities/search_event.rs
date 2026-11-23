use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "search_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub query: Option<String>,
    /// The service type filter of a search on services.
    pub service_type_id: Option<i32>,
    /// The supply type filter of a search on supplies.
    pub supply_type_id: Option<i32>,
    pub ward_code: Option<String>,
    pub results: i32,
    pub created_on: DateTimeWithTimeZone,
    /// Demo data: counted only where demo data is (the admin's report).
    pub is_demo: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
