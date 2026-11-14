//! Accent-insensitive "contains" search over folded text. The condition is
//! built from `migration::search::fold`, the same function that defines the
//! indexes, so a change to one cannot leave the other behind.

use domain::search_terms::query_variants;
use migration::search::fold;
use sea_orm::{
    sea_query::{BinOper, ColumnRef, Expr, IntoColumnRef, SimpleExpr},
    Condition,
};

use crate::entities::{business, product, service, user};

/// A text column the searches match.
#[derive(Clone, Copy, Debug)]
pub enum SearchColumn {
    BusinessName,
    BusinessDescription,
    ServiceName,
    ServiceDescription,
    ProductName,
    ProductDescription,
    UserUsername,
    UserFullName,
    UserEmail,
}

impl SearchColumn {
    /// The table-qualified column.
    pub fn column_ref(self) -> ColumnRef {
        match self {
            Self::BusinessName => (business::Entity, business::Column::Name).into_column_ref(),
            Self::BusinessDescription => {
                (business::Entity, business::Column::Description).into_column_ref()
            }
            Self::ServiceName => (service::Entity, service::Column::Name).into_column_ref(),
            Self::ServiceDescription => {
                (service::Entity, service::Column::Description).into_column_ref()
            }
            Self::ProductName => (product::Entity, product::Column::Name).into_column_ref(),
            Self::ProductDescription => {
                (product::Entity, product::Column::Description).into_column_ref()
            }
            Self::UserUsername => (user::Entity, user::Column::Username).into_column_ref(),
            Self::UserFullName => (user::Entity, user::Column::FullName).into_column_ref(),
            Self::UserEmail => (user::Entity, user::Column::Email).into_column_ref(),
        }
    }
}

use SearchColumn::*;

/// Searched by the provider directory (/providers, /search) and, for the
/// provider's name, by the service listing.
pub const BUSINESS_SEARCH_COLUMNS: &[SearchColumn] = &[BusinessName, BusinessDescription];
/// A service's own text: matched by the service listing, and by the provider
/// directory through the provider's live services.
pub const SERVICE_SEARCH_COLUMNS: &[SearchColumn] = &[ServiceName, ServiceDescription];
/// Searched by the service listing (/services, /search): the service's text and
/// the name of the business offering it.
pub const SERVICE_LISTING_SEARCH_COLUMNS: &[SearchColumn] =
    &[ServiceName, ServiceDescription, BusinessName];
/// Searched by the product listing (/products, /search).
pub const PRODUCT_SEARCH_COLUMNS: &[SearchColumn] = &[ProductName, ProductDescription];

/// The admin lists' name search: a record by its own name, a product or
/// service also by its business's name.
pub const ADMIN_BUSINESS_SEARCH_COLUMNS: &[SearchColumn] = &[BusinessName];
pub const ADMIN_PRODUCT_SEARCH_COLUMNS: &[SearchColumn] = &[ProductName, BusinessName];
pub const ADMIN_SERVICE_SEARCH_COLUMNS: &[SearchColumn] = &[ServiceName, BusinessName];
pub const ADMIN_USER_SEARCH_COLUMNS: &[SearchColumn] = &[UserUsername, UserFullName, UserEmail];

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

/// "`column` matches the bound LIKE `pattern`", folded on both sides. The
/// pattern carries its own `%` wildcards (see `contains_pattern`);
/// `lower`/`unaccent` leave `%` and `\` alone.
pub fn contains(column: SearchColumn, pattern: &str) -> SimpleExpr {
    fold(Expr::col(column.column_ref())).binary(BinOper::Like, fold(Expr::val(pattern.to_owned())))
}

/// The LIKE pattern for "contains `query`": escaped, then wrapped in `%`.
pub fn contains_pattern(query: &str) -> String {
    format!("%{}%", escape_like(query.trim()))
}

/// True when any of `columns` contains `query`, ignoring case and Vietnamese
/// diacritics.
pub fn text_contains(columns: &[SearchColumn], query: &str) -> Condition {
    // Every variant of the query (`heo` also finds `lợn`), in every column.
    query_variants(query)
        .iter()
        .map(|variant| contains_pattern(variant))
        .fold(Condition::any(), |cond, needle| {
            columns
                .iter()
                .fold(cond, |cond, column| cond.add(contains(*column, &needle)))
        })
}

/// `text_contains` for an optional search box: blank or absent matches
/// everything (an empty condition adds nothing to the WHERE clause).
pub fn search_condition(columns: &[SearchColumn], search: Option<&str>) -> Condition {
    match search.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => text_contains(columns, q),
        None => Condition::all(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_searched_column_has_a_folded_index() {
        // Without one, every search scans the whole table.
        let indexed: Vec<(String, String)> = migration::search::all_fold_indexes()
            .iter()
            .map(|i| (i.table.to_string(), i.column.to_string()))
            .collect();
        let searched = BUSINESS_SEARCH_COLUMNS
            .iter()
            .chain(PRODUCT_SEARCH_COLUMNS)
            .chain(SERVICE_SEARCH_COLUMNS)
            .chain(SERVICE_LISTING_SEARCH_COLUMNS)
            .chain(ADMIN_BUSINESS_SEARCH_COLUMNS)
            .chain(ADMIN_PRODUCT_SEARCH_COLUMNS)
            .chain(ADMIN_SERVICE_SEARCH_COLUMNS)
            .chain(ADMIN_USER_SEARCH_COLUMNS);
        for column in searched {
            // An unqualified column matches no index, so it fails below too.
            let key = match column.column_ref() {
                ColumnRef::TableColumn(table, col) => (table.to_string(), col.to_string()),
                _ => (String::new(), format!("{column:?}")),
            };
            assert!(
                indexed.contains(&key),
                "{}.{} is searched but has no folded index",
                key.0,
                key.1
            );
        }
    }

    #[test]
    fn wildcards_typed_by_users_are_literal() {
        assert_eq!(escape_like(r"50%_off\"), r"50\%\_off\\");
        assert_eq!(escape_like("phân bón"), "phân bón");
    }
}
