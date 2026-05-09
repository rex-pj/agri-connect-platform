pub struct Category {
    pub id:            i32,
    pub name:          String,
    pub slug:          String,
    pub icon:          Option<String>,
    pub display_order: i16,
}

pub struct CategoryAdmin {
    pub id:            i32,
    pub name:          String,
    pub slug:          String,
    pub category_type: String,
    pub icon:          Option<String>,
    pub display_order: i16,
    pub is_active:     bool,
}

pub struct NewCategory {
    pub name:          String,
    pub slug:          String,
    pub category_type: String,
    pub icon:          Option<String>,
    pub display_order: i16,
}

pub struct UpdateCategory {
    pub name:          String,
    pub slug:          String,
    pub icon:          Option<String>,
    pub display_order: i16,
}
