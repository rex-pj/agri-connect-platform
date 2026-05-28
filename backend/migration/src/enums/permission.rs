use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Permission {
    #[iden = "permissions"]
    Table,
    Id,
    Code,
    Name,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}
