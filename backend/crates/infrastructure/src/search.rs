//! Accent-insensitive "contains" search over folded text. The condition is
//! built from `migration::search::fold`, the same function that defines the
//! indexes, so a change to one cannot leave the other behind.

use migration::search::fold;
use sea_orm::{
    sea_query::{Expr, SimpleExpr},
    Condition,
};

/// Searched by the provider directory (/services, /search).
pub const BUSINESS_SEARCH_COLUMNS: &[(&str, &str)] =
    &[("businesses", "name"), ("businesses", "description")];
/// Searched by the product listing (/products, /search).
pub const PRODUCT_SEARCH_COLUMNS: &[(&str, &str)] =
    &[("products", "name"), ("products", "description")];

/// Postgres' default LIKE escape character.
const LIKE_ESCAPE: char = '\\';

/// `%`, `_` and the escape character typed by a user are literal characters,
/// not wildcards: a search for "50%" must not match every price.
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '%' | '_') || ch == LIKE_ESCAPE {
            out.push(LIKE_ESCAPE);
        }
        out.push(ch);
    }
    out
}

/// The SQL for "`column` matches the bound LIKE pattern", folded on both
/// sides. `$1` is the placeholder sea-query renumbers for Postgres (a `?` is
/// left in the SQL unbound). The pattern carries its own `%` wildcards (see
/// `contains_pattern`); `lower`/`unaccent` leave `%` and `\` alone.
pub fn contains_sql(table: &str, column: &str) -> String {
    format!(
        "{} LIKE {}",
        fold(&format!("\"{table}\".\"{column}\"")),
        fold("$1")
    )
}

/// The LIKE pattern for "contains `query`": escaped, then wrapped in `%`.
pub fn contains_pattern(query: &str) -> String {
    format!("%{}%", escape_like(query.trim()))
}

/// True when any of `columns` contains `query`, ignoring case and Vietnamese
/// diacritics.
pub fn text_contains(columns: &[(&str, &str)], query: &str) -> Condition {
    let needle = contains_pattern(query);
    columns
        .iter()
        .fold(Condition::any(), |cond, (table, column)| {
            let expr: SimpleExpr =
                Expr::cust_with_values(contains_sql(table, column), [needle.clone()]);
            cond.add(expr)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_searched_column_has_a_folded_index() {
        // Without one, every search scans the whole table.
        for (table, column) in BUSINESS_SEARCH_COLUMNS.iter().chain(PRODUCT_SEARCH_COLUMNS) {
            assert!(
                migration::search::SEARCH_INDEXES
                    .iter()
                    .any(|(_, t, c)| t == table && c == column),
                "{table}.{column} is searched but has no folded index"
            );
        }
    }

    #[test]
    fn wildcards_typed_by_users_are_literal() {
        assert_eq!(escape_like(r"50%_off\"), r"50\%\_off\\");
        assert_eq!(escape_like("phân bón"), "phân bón");
    }
}
