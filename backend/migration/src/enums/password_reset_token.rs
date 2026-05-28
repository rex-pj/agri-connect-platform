use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum PasswordResetToken {
    #[iden = "password_reset_tokens"]
    Table,
    Id,
    UserId,
    Token,
    ExpiresAt,
    UsedAt,
    CreatedAt,
}
