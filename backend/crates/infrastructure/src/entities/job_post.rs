use sea_orm::entity::prelude::*;

// The `location` column (PostGIS GEOGRAPHY) is left out, as on businesses:
// Sea-ORM cannot decode it. Queries name it as `sql::Geo::Location`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "job_posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub service_type_id: i32,
    pub location_id: Option<i32>,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub status: String,
    pub created_on: DateTimeWithTimeZone,
    /// Demo data: shown only to a demo business, removed with the demo data.
    pub is_demo: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
