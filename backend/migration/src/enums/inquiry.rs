use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Inquiry {
    #[iden = "inquiries"]
    Table,
    Id,
    BusinessId,
    ServiceId,
    ProductId,
    Topic,
    RequesterName,
    RequesterPhone,
    Note,
    Status,
    CreatedOn,
    UpdatedOn,
    UpdatedBy,
}
