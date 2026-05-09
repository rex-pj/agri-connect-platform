use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum User {
    #[iden = "users"]
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    FullName,
    RoleId,
    IsActive,
    LastLoginOn,
    FailedLoginAttempts,
    LockedUntil,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}
