use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_permissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id:       i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: i32,
    pub granted_on:    DateTimeWithTimeZone,
    pub granted_by:    Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
