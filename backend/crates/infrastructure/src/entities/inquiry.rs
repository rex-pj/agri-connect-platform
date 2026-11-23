use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "inquiries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub business_id: i32,
    pub service_id: Option<i32>,
    pub product_id: Option<i32>,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub status: String,
    pub created_on: DateTimeWithTimeZone,
    pub updated_on: DateTimeWithTimeZone,
    pub updated_by: Option<i32>,
    /// When it was first marked answered (`contacted` or `closed`).
    pub answered_on: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::business::Entity",
        from = "Column::BusinessId",
        to = "super::business::Column::Id"
    )]
    Business,
}

impl Related<super::business::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Business.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
