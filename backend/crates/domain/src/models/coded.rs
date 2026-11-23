/// An entry of a short list the admin keeps, named elsewhere by its code:
/// the sectors the platform serves ("ngành": trồng trọt, chăn nuôi, thuỷ
/// sản) and the kinds of alert (dịch bệnh, xâm nhập mặn, thời tiết). Its
/// `code` is made from its name when it is added and never changes after
/// (cookies, `sector=` filters and the rows pointing to it keep it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodedEntry {
    pub id: i32,
    pub code: String,
    pub name: String,
    /// Font Awesome class.
    pub icon: Option<String>,
    pub display_order: i16,
    pub is_active: bool,
    /// Demo data (`DemoDataRepositoryTrait`), not a real entry yet.
    pub is_demo: bool,
}

/// An entry's boxes the admin edits (its code stays).
pub struct CodedFields {
    pub name: String,
    pub icon: Option<String>,
    pub display_order: i16,
}
