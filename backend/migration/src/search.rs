//! The text-folding expression shared by the search indexes and the search
//! queries. Postgres uses an expression index only when the query repeats the
//! indexed expression exactly; building both from `fold` keeps them in step
//! (`crates/tests/tests/search.rs` pins it with EXPLAIN).

use sea_orm_migration::prelude::*;

use crate::schema::{
    create_opclass_index, Businesses, DbFn, IndexMethod, Products, Services, Users,
};

/// Lower-cased, accent-free form of `expr`: "Phân Bón Đầu Trâu" and
/// "phan bon dau trau" fold to the same text, so farmers typing without
/// diacritics still find listings.
pub fn fold<E: Into<SimpleExpr>>(expr: E) -> SimpleExpr {
    Func::lower(Func::cust(DbFn::FUnaccent).arg(expr)).into()
}

/// A trigram index on `fold(table.column)`.
pub struct FoldIndex {
    pub name: &'static str,
    pub table: DynIden,
    pub column: DynIden,
}

impl FoldIndex {
    fn new<T: IntoIden, C: IntoIden>(name: &'static str, table: T, column: C) -> Self {
        Self {
            name,
            table: table.into_iden(),
            column: column.into_iden(),
        }
    }

    /// `CREATE INDEX name ON table USING gin ((fold(column)) gin_trgm_ops)`.
    pub fn create(&self) -> String {
        create_opclass_index(
            self.name,
            &*self.table,
            IndexMethod::Gin,
            fold(Expr::col(self.column.clone())),
            IndexMethod::GinTrgmOps,
        )
    }

    /// `DROP INDEX IF EXISTS name`.
    pub fn drop(&self) -> IndexDropStatement {
        Index::drop()
            .name(self.name)
            .table(self.table.clone())
            .if_exists()
            .to_owned()
    }
}

/// Every folded search index of the search migration.
pub fn search_indexes() -> Vec<FoldIndex> {
    vec![
        FoldIndex::new(
            "ix_businesses_name_fold",
            Businesses::Table,
            Businesses::Name,
        ),
        FoldIndex::new(
            "ix_businesses_description_fold",
            Businesses::Table,
            Businesses::Description,
        ),
        FoldIndex::new("ix_products_name_fold", Products::Table, Products::Name),
        FoldIndex::new(
            "ix_products_description_fold",
            Products::Table,
            Products::Description,
        ),
    ]
}

/// The folded search indexes on services (name, description), added after
/// `search_indexes` had run; kept apart so that migration's list never
/// changes under a database it already built.
pub fn service_search_indexes() -> Vec<FoldIndex> {
    vec![
        FoldIndex::new("ix_services_name_fold", Services::Table, Services::Name),
        FoldIndex::new(
            "ix_services_description_fold",
            Services::Table,
            Services::Description,
        ),
    ]
}

/// The folded indexes for the admin's account search (username, name,
/// email), added after the lists above had run.
pub fn user_search_indexes() -> Vec<FoldIndex> {
    vec![
        FoldIndex::new("ix_users_username_fold", Users::Table, Users::Username),
        FoldIndex::new("ix_users_full_name_fold", Users::Table, Users::FullName),
        FoldIndex::new("ix_users_email_fold", Users::Table, Users::Email),
    ]
}

/// Every folded index, for the check that each searched column has one.
pub fn all_fold_indexes() -> Vec<FoldIndex> {
    let mut all = search_indexes();
    all.extend(service_search_indexes());
    all.extend(user_search_indexes());
    all
}
