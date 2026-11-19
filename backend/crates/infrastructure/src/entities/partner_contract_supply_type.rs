use sea_orm::entity::prelude::*;

/// The supply types a partner contract covers.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "partner_contract_supply_types")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contract_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub supply_type_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
