use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Service {
    #[iden = "services"]
    Table,
    Id,
    BusinessId,
    CategoryId,
    Name,
    Description,
    PriceNote,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}
