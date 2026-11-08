use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "role_permissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: i32,
    pub granted_by: Option<i32>,
    pub granted_on: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
