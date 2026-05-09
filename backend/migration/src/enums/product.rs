use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Product {
    #[iden = "products"]
    Table,
    Id,
    BusinessId,
    CategoryId,
    Name,
    Description,
    Price,
    Unit,
    ImageUrl,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}
