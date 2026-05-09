use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "roles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id:          i32,
    pub code:        String,
    pub name:        String,
    pub description: Option<String>,
    pub is_active:   bool,
    pub created_on:  DateTimeWithTimeZone,
    pub updated_on:  DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
