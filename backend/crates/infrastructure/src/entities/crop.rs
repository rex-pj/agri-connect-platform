use sea_orm::entity::prelude::*;

/// A farm produce item on the price board (nông sản): a variety or grade of
/// one commodity ("Lúa ST25", "Heo hơi").
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "crops")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub commodity_id: i32,
    pub name: String,
    pub unit: String,
    pub display_order: i32,
    pub is_active: bool,
    /// Loaded with the demo data, removed with it unless real data uses it.
    pub is_demo: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::commodity::Entity",
        from = "Column::CommodityId",
        to = "super::commodity::Column::Id"
    )]
    Commodity,
}

impl Related<super::commodity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Commodity.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
