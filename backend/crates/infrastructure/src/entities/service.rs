use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "services")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub business_id: i32,
    pub service_type_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
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
        belongs_to = "super::service_type::Entity",
        from = "Column::ServiceTypeId",
        to = "super::service_type::Column::Id"
    )]
    ServiceType,
}

impl Related<super::business::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Business.def()
    }
}

impl Related<super::service_type::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ServiceType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
