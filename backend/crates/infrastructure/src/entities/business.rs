use sea_orm::entity::prelude::*;

// The `location` column (PostGIS GEOGRAPHY) is intentionally omitted — SeaORM
// cannot deserialise that type. All geo queries use raw Statement instead.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "businesses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id:          i32,
    pub name:        String,
    pub phone:       String,
    pub email:       Option<String>,
    pub description: Option<String>,
    pub address:     Option<String>,
    pub is_verified: bool,
    pub is_active:   bool,
    pub is_demo:     bool,
    pub owner_id:    Option<i32>,
    pub created_on:  DateTimeWithTimeZone,
    pub updated_on:  DateTimeWithTimeZone,
    pub created_by:  Option<i32>,
    pub updated_by:  Option<i32>,
    pub deleted_at:  Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::OwnerId",
        to = "super::user::Column::Id"
    )]
    Owner,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Owner.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
