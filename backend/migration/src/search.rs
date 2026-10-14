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
