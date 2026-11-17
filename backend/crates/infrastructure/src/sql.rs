//! The typed SQL vocabulary of the repositories: database functions, cast
//! types, output-column aliases and the PostGIS column the entities cannot
//! map. Queries name these instead of spelling SQL, so a typo is a compile
//! error and every PostGIS call is built in one place.

use sea_orm::{
    sea_query::{
        Expr, Func, IntoColumnRef, IntoIden, Keyword, Query, SelectStatement, SimpleExpr,
        SubQueryStatement,
    },
    Condition, DeriveColumn, DeriveIden, EnumIter,
};

use crate::entities::{business, commodity, product, service_type, supply_type, user};

/// Database functions the query builder has no name for.
#[derive(DeriveIden, Clone, Copy)]
pub enum DbFn {
    #[sea_orm(iden = "ST_Covers")]
    StCovers,
    #[sea_orm(iden = "ST_Distance")]
    StDistance,
    #[sea_orm(iden = "ST_DWithin")]
    StDWithin,
    #[sea_orm(iden = "ST_GeographyFromText")]
    StGeographyFromText,
    #[sea_orm(iden = "ST_GeomFromTWKB")]
    StGeomFromTwkb,
    #[sea_orm(iden = "ST_MakePoint")]
    StMakePoint,
    #[sea_orm(iden = "ST_SetSRID")]
    StSetSrid,
    #[sea_orm(iden = "ST_X")]
    StX,
    #[sea_orm(iden = "ST_Y")]
    StY,
    #[sea_orm(iden = "pg_advisory_xact_lock")]
    AdvisoryXactLock,
    /// `date_part(field, value)`: `EXTRACT` spelled as a function.
    #[sea_orm(iden = "date_part")]
    DatePart,
    #[sea_orm(iden = "floor")]
    Floor,
    #[sea_orm(iden = "ROW_NUMBER")]
    RowNumber,
    #[sea_orm(iden = "NULLIF")]
    NullIf,
    #[sea_orm(iden = "TRIM")]
    Trim,
    /// `decode(text, 'hex')`: bytes from their hex spelling.
    #[sea_orm(iden = "decode")]
    Decode,
}

/// Types a value is cast to.
#[derive(DeriveIden, Clone, Copy)]
pub enum PgType {
    Geography,
    Geometry,
    Float8,
    Int4,
    Interval,
}

/// The PostGIS columns left out of the entities (Sea-ORM cannot decode
/// them): `location`, the `GEOGRAPHY(POINT, 4326)` of `businesses` and
/// `job_posts`; `center` (a point) and `boundary` (a multipolygon) of
/// `provinces` and `wards`.
#[derive(DeriveIden, Clone, Copy)]
pub enum Geo {
    Location,
    Center,
    Boundary,
}

/// A name a query gives a table: a second reference to one already in it
/// (`users` read again for an author's name), or a subquery it selects from
/// (rows numbered by a window, to keep the first few).
#[derive(DeriveIden, Clone, Copy)]
pub enum TableAlias {
    Author,
    Ranked,
}

/// The output columns a query names itself; each matches a field of the
/// `FromQueryResult` row that reads it.
#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum QueryAs {
    Area,
    Average,
    BusinessAddress,
    BusinessId,
    BusinessName,
    BusinessPhone,
    BusinessVerified,
    Category,
    CategoryIcon,
    CategoryId,
    CategorySlug,
    Commodity,
    Count,
    CreatedByName,
    DistanceM,
    Email,
    EmailVerified,
    Fresh,
    FromBusinessPhone,
    Lat,
    LinkedUserId,
    LinkedUsername,
    Lng,
    Oldest,
    ProviderTurn,
    RecipientEmail,
    RequestSentOn,
    Role,
    RoleCode,
    RoleName,
    Sector,
    ServiceName,
    ShopId,
    SvcName,
    Total,
    Turn,
    Unmet,
    UpdatedByName,
    VendorName,
    VendorPhone,
    VendorVerified,
    WardCode,
    Week,
    Words,
}

/// `ST_GeographyFromText(wkt)`, the WKT bound as a parameter
/// (`SRID=4326;POINT(<lng> <lat>)`).
pub fn geography_from_text(wkt: &str) -> SimpleExpr {
    Func::cust(DbFn::StGeographyFromText)
        .arg(Expr::val(wkt.to_owned()))
        .into()
}

/// `ST_MakePoint(lng, lat)::geography`.
pub fn make_point<L: Into<SimpleExpr>, A: Into<SimpleExpr>>(lng: L, lat: A) -> SimpleExpr {
    SimpleExpr::from(Func::cust(DbFn::StMakePoint).args([lng.into(), lat.into()]))
        .cast_as(PgType::Geography)
}

/// `ST_Distance(a, b)` in metres.
pub fn distance(a: SimpleExpr, b: SimpleExpr) -> SimpleExpr {
    Func::cust(DbFn::StDistance).args([a, b]).into()
}

/// `ST_DWithin(a, b, radius_m)`: `a` within `radius_m` metres of `b`.
pub fn within(a: SimpleExpr, b: SimpleExpr, radius_m: f64) -> SimpleExpr {
    Func::cust(DbFn::StDWithin)
        .args([a, b, Expr::val(radius_m).into()])
        .into()
}

/// `"businesses"."location"`.
pub fn business_location() -> SimpleExpr {
    Expr::col((business::Entity, Geo::Location)).into()
}

/// A business's latitude: `ST_Y(location::geometry)`.
pub fn business_lat() -> SimpleExpr {
    Func::cust(DbFn::StY)
        .arg(business_location().cast_as(PgType::Geometry))
        .into()
}

/// A business's longitude: `ST_X(location::geometry)`.
pub fn business_lng() -> SimpleExpr {
    Func::cust(DbFn::StX)
        .arg(business_location().cast_as(PgType::Geometry))
        .into()
}

/// The `NULL` keyword: untyped, so it fits any column (a bound NULL
/// parameter carries the type of its Rust value and would not).
pub fn null() -> SimpleExpr {
    SimpleExpr::Keyword(Keyword::Null)
}

/// `NULL::float8`: the distance column of a row with no distance.
pub fn null_float() -> SimpleExpr {
    null().cast_as(PgType::Float8)
}

/// `NULLIF(TRIM(expr), '')`: a blank text as NULL.
pub fn blank_as_null(expr: SimpleExpr) -> SimpleExpr {
    Func::cust(DbFn::NullIf)
        .args([
            Func::cust(DbFn::Trim).arg(expr).into(),
            Expr::val("").into(),
        ])
        .into()
}

/// `users.id = businesses.owner_id AND users.is_active AND users.deleted_at
/// IS NULL`: the business's managing account, while it is live (left-joined).
pub fn live_owner_on() -> Condition {
    Condition::all()
        .add(
            Expr::col((user::Entity, user::Column::Id))
                .equals((business::Entity, business::Column::OwnerId)),
        )
        .add(Expr::col((user::Entity, user::Column::IsActive)).eq(true))
        .add(Expr::col((user::Entity, user::Column::DeletedAt)).is_null())
}

/// A joined service type that serves `sector`: one of that sector, or one
/// of every sector (no sector set). The one definition of "a service in a
/// sector".
pub fn category_in_sector(sector: &str) -> Condition {
    Condition::any()
        .add(Expr::col((service_type::Entity, service_type::Column::Sector)).eq(sector))
        .add(Expr::col((service_type::Entity, service_type::Column::Sector)).is_null())
}

/// A supply (its type joined) of `sector`: for a commodity of that sector,
/// or for none and of a type of that sector or of every sector. The one
/// definition of "a supply in a sector": cá tra feed is aquatic even when
/// filed under a feed type that serves every sector.
pub fn supply_in_sector(sector: &str) -> Condition {
    let of_sector = Query::select()
        .column(commodity::Column::Id)
        .from(commodity::Entity)
        .and_where(Expr::col(commodity::Column::Sector).eq(sector))
        .to_owned();
    Condition::any()
        .add(Expr::col((product::Entity, product::Column::CommodityId)).in_subquery(of_sector))
        .add(
            Condition::all()
                .add(Expr::col((product::Entity, product::Column::CommodityId)).is_null())
                .add(
                    Condition::any()
                        .add(
                            Expr::col((supply_type::Entity, supply_type::Column::Sector))
                                .eq(sector),
                        )
                        .add(
                            Expr::col((supply_type::Entity, supply_type::Column::Sector)).is_null(),
                        ),
                ),
        )
}

/// The sector of a supply (its type and its commodity joined), as
/// `supply_in_sector` decides it: its commodity's, else its type's, else
/// none (every sector).
pub fn supply_sector() -> SimpleExpr {
    Func::coalesce([
        Expr::col((commodity::Entity, commodity::Column::Sector)).into(),
        Expr::col((supply_type::Entity, supply_type::Column::Sector)).into(),
    ])
    .into()
}

/// Where a notice to a business goes. A business with a managing account:
/// that account's address, once it is verified and while the account is live
/// (`live_owner_on`), else nowhere: an address nobody confirmed is never
/// written to, or a sign-up with a stranger's address would make us do so.
/// A business without an account (listed by an admin): its own address,
/// unless blank.
pub fn notice_email() -> SimpleExpr {
    Expr::case(
        Expr::col((business::Entity, business::Column::OwnerId)).is_null(),
        blank_as_null(Expr::col((business::Entity, business::Column::Email)).into()),
    )
    .case(
        Expr::col((user::Entity, user::Column::EmailVerifiedAt)).is_not_null(),
        Expr::col((user::Entity, user::Column::Email)),
    )
    .into()
}

/// `SELECT id FROM businesses WHERE owner_id = owner_user AND deleted_at IS
/// NULL`: the live business an account manages, for an `in_subquery` that
/// keeps the ownership check and the read or write in one statement.
pub fn businesses_of(owner_user: i32) -> SelectStatement {
    Query::select()
        .column((business::Entity, business::Column::Id))
        .from(business::Entity)
        .and_where(Expr::col((business::Entity, business::Column::OwnerId)).eq(owner_user))
        .and_where(Expr::col((business::Entity, business::Column::DeletedAt)).is_null())
        .to_owned()
}

/// `SELECT pg_advisory_xact_lock(key)`: serialises callers on `key` until
/// the transaction ends.
pub fn advisory_xact_lock(key: i64) -> SelectStatement {
    Query::select()
        .expr(Func::cust(DbFn::AdvisoryXactLock).arg(key))
        .to_owned()
}

/// `(SELECT author.username FROM users author WHERE author.id = user_id)`:
/// the username behind an audit column (`created_by`, `updated_by`).
pub fn username_of<C: IntoColumnRef>(user_id: C) -> SimpleExpr {
    SimpleExpr::SubQuery(
        None,
        Box::new(SubQueryStatement::SelectStatement(
            Query::select()
                .column((TableAlias::Author, user::Column::Username))
                .from_as(user::Entity, TableAlias::Author)
                .and_where(Expr::col((TableAlias::Author, user::Column::Id)).equals(user_id))
                .to_owned(),
        )),
    )
}

/// `(SELECT …)` as an expression.
pub fn scalar(query: SelectStatement) -> SimpleExpr {
    SimpleExpr::SubQuery(None, Box::new(SubQueryStatement::SelectStatement(query)))
}

/// `EXISTS (SELECT …)`.
pub fn exists(query: SelectStatement) -> SimpleExpr {
    Expr::exists(query)
}

/// An alias as an expression, for ORDER BY an output column.
pub fn output<I: IntoIden>(alias: I) -> SimpleExpr {
    Expr::col(alias.into_iden()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::{Iden, PostgresQueryBuilder};

    #[test]
    fn postgis_names_render_as_postgis_spells_them() {
        assert_eq!(DbFn::StDWithin.to_string(), "ST_DWithin");
        assert_eq!(QueryAs::DistanceM.to_string(), "distance_m");
        let sql = Query::select()
            .expr(within(business_location(), geography_from_text("x"), 5.0))
            .to_string(PostgresQueryBuilder);
        assert_eq!(
            sql,
            r#"SELECT ST_DWithin("businesses"."location", ST_GeographyFromText('x'), 5)"#
        );
    }
}
