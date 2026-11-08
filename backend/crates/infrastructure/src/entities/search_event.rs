use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "search_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub query: Option<String>,
    pub category_id: Option<i32>,
    /// The preset of a search stored before wards; new ones keep `ward_code`.
    pub location_id: Option<i32>,
    pub ward_code: Option<String>,
    pub results: i32,
    pub created_on: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
