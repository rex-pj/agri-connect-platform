use sea_orm::entity::prelude::*;

/// What a farmer raises (cây trồng, vật nuôi): rice, durian, pigs, shrimp. It fixes
/// the sector of the price board items and the supplies that point to it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "commodities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    /// `crop`, `livestock` or `aquatic`.
    pub sector: String,
    pub icon: Option<String>,
    pub display_order: i16,
    /// Days a price of its crops stays on the board.
    pub price_fresh_days: i16,
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
