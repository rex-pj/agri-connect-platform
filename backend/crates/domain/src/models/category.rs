pub struct Category {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// The sector it serves (`sectors.code`); `None`: every sector.
    pub sector: Option<String>,
}

pub struct CategoryAdmin {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub category_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// The sector it serves (`sectors.code`); `None`: every sector.
    pub sector: Option<String>,
    pub is_active: bool,
    /// Demo data (`DemoDataRepositoryTrait`), not a real entry yet.
    pub is_demo: bool,
    pub created_on: chrono::DateTime<chrono::Utc>,
    pub updated_on: chrono::DateTime<chrono::Utc>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

pub struct NewCategory {
    pub name: String,
    pub slug: String,
    pub category_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// The sector it serves (`sectors.code`); `None`: every sector.
    pub sector: Option<String>,
    pub created_by: i32,
}

pub struct UpdateCategory {
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// The sector it serves (`sectors.code`); `None`: every sector.
    pub sector: Option<String>,
    pub updated_by: i32,
}
