use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Business {
    #[iden = "businesses"]
    Table,
    Id,
    Name,
    Phone,
    Email,
    Description,
    Address,
    Location,
    IsVerified,
    IsActive,
    OwnerId,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}
