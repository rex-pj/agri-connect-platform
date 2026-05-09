use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Review {
    #[iden = "reviews"]
    Table,
    Id,
    BusinessId,
    InquiryId,
    Rating,
    Comment,
    ReviewerName,
    ReviewerPhoneHash,
    IsVisible,
    CreatedOn,
    UpdatedOn,
    UpdatedBy,
}
