use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub business_id: i32,
    pub supply_type_id: i32,
    /// The commodity it is for (lúa giống ST25: rice); `None`: every one.
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub is_active: bool,
    pub created_on: DateTimeWithTimeZone,
    pub updated_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
    pub updated_by: Option<i32>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub image_file_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::business::Entity",
        from = "Column::BusinessId",
        to = "super::business::Column::Id"
    )]
    Business,
    #[sea_orm(
        belongs_to = "super::supply_type::Entity",
        from = "Column::SupplyTypeId",
        to = "super::supply_type::Column::Id"
    )]
    SupplyType,
    #[sea_orm(
        belongs_to = "super::commodity::Entity",
        from = "Column::CommodityId",
        to = "super::commodity::Column::Id"
    )]
    Commodity,
}

impl Related<super::business::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Business.def()
    }
}

impl Related<super::supply_type::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SupplyType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
