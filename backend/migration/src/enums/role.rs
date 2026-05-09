use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Role {
    #[iden = "roles"]
    Table,
    Id,
    Code,
    Name,
    Description,
    IsActive,
    CreatedOn,
    UpdatedOn,
}
