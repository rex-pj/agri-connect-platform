use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum Location {
    #[iden = "locations"]
    Table,
    Id,
    Label,
    Lat,
    Lng,
    DisplayOrder,
    IsActive,
}
