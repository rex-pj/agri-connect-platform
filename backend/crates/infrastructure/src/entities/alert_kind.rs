use sea_orm::entity::prelude::*;

/// A kind of alert (dịch bệnh, xâm nhập mặn, thời tiết); `alerts.kind`
/// names it by `code`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "alert_kinds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// Made from the name when it was added; never changes.
    pub code: String,
    pub name: String,
    pub icon: Option<String>,
    pub display_order: i16,
    pub is_active: bool,
    /// Loaded with the demo data, removed with it unless real data uses it.
    pub is_demo: bool,
    pub created_on: DateTimeWithTimeZone,
    pub updated_on: DateTimeWithTimeZone,
    pub created_by: Option<i32>,
    pub updated_by: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
