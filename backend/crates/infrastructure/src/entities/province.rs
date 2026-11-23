use sea_orm::entity::prelude::*;

// The `center` column (PostGIS GEOGRAPHY) is left out: Sea-ORM cannot decode
// it. Queries read it as `sql::Geo::Center`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "provinces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub code: String,
    pub name: String,
    pub full_name: String,
    pub unit_type: String,
    pub is_active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
