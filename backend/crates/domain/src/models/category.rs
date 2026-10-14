pub struct Category {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
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
    pub is_active: bool,
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
    pub created_by: i32,
}

pub struct UpdateCategory {
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    pub updated_by: i32,
}
