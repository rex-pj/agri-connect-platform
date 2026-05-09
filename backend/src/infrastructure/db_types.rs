/// SeaORM returns timestamps as `DateTime<FixedOffset>` from the database wire format.
/// Repositories use this type in their private `*Row` query-result structs, then convert
/// to `DateTime<Utc>` before returning domain models.
pub type DateTimeWithTimeZone = chrono::DateTime<chrono::FixedOffset>;
