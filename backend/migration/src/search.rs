//! The text-folding expression shared by the search indexes and the search
//! queries. Postgres uses an expression index only when the query repeats the
//! indexed expression exactly; building both from `fold` keeps them in step
//! (`crates/tests/tests/search.rs` pins it with EXPLAIN).

/// Lower-cased, accent-free form of `column`: "Phân Bón Đầu Trâu" and
/// "phan bon dau trau" fold to the same text, so farmers typing without
/// diacritics still find listings.
pub fn fold(column: &str) -> String {
    format!("lower(f_unaccent({column}))")
}

/// (index name, table, column) for every folded search index.
pub const SEARCH_INDEXES: &[(&str, &str, &str)] = &[
    ("ix_businesses_name_fold", "businesses", "name"),
    (
        "ix_businesses_description_fold",
        "businesses",
        "description",
    ),
    ("ix_products_name_fold", "products", "name"),
    ("ix_products_description_fold", "products", "description"),
];

/// The folded search indexes on services (name, description), added after
/// `SEARCH_INDEXES` had run; kept apart so that migration's list never
/// changes under a database it already built.
pub const SERVICE_SEARCH_INDEXES: &[(&str, &str, &str)] = &[
    ("ix_services_name_fold", "services", "name"),
    ("ix_services_description_fold", "services", "description"),
];

/// The folded indexes for the admin's account search (username, name,
/// email), added after the lists above had run.
pub const USER_SEARCH_INDEXES: &[(&str, &str, &str)] = &[
    ("ix_users_username_fold", "users", "username"),
    ("ix_users_full_name_fold", "users", "full_name"),
    ("ix_users_email_fold", "users", "email"),
];
