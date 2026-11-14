use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "locations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub label: String,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
    pub is_active: bool,
    /// Loaded with the demo data, removed with it unless real data uses it.
    pub is_demo: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
