use sea_orm::entity::prelude::*;

// The `center` and `boundary` columns (PostGIS GEOGRAPHY) are left out:
// Sea-ORM cannot decode them. Queries read them as `sql::Geo::Center` and
// `sql::Geo::Boundary`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "wards")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub code: String,
    pub province_code: String,
    pub name: String,
    pub full_name: String,
    pub unit_type: String,
    pub is_active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
