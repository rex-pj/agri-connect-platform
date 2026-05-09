use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum UserPermission {
    #[iden = "user_permissions"]
    Table,
    UserId,
    PermissionId,
    GrantedOn,
    GrantedBy,
}
