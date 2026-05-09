use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum AgriPermission {
    #[iden = "agri_permissions"]
    Table,
    Id,
    Code,
    Name,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}
