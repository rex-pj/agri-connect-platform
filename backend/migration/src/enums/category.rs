use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Category {
    #[iden = "categories"]
    Table,
    Id,
    Name,
    Slug,
    #[iden = "type"]
    Type,
    Icon,
    DisplayOrder,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}
