use sea_orm::entity::prelude::*;

/// A kind of work a provider is hired for (loại dịch vụ) Its id comes from the sequence it shares with the other kind
/// (`catalogue_type_ids`), so a bare id names one row of either table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "service_types")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// The sector it serves (`crop`, `livestock`, `aquatic`); `None`: every one.
    pub sector: Option<String>,
    pub is_active: bool,
    /// Loaded with the demo data, removed with it unless real data uses it.
    pub is_demo: bool,
    pub created_on: DateTimeWithTimeZone,
    pub updated_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
    pub updated_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
