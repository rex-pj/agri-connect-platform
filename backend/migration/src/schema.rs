//! The typed names of every table, column and database function the
//! migrations use, and the few Postgres statements sea-query 0.32 cannot
//! build (named CHECK constraints, operator-class indexes, triggers and
//! functions), each assembled from those names so no migration spells SQL.
//!
//! A variant here names a column some migration created: renaming one would
//! rewrite history, so a renamed column gets a new variant instead.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbErr},
    sea_query::{
        extension::postgres::{Extension, PgBinOper},
        FunctionCall, QueryBuilder,
    },
};

// ── Tables ──────────────────────────────────────────────────────────────────

#[derive(DeriveIden, Clone, Copy)]
pub enum Roles {
    Table,
    Id,
    Code,
    Name,
    Description,
    IsActive,
    CreatedOn,
    UpdatedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Users {
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    FullName,
    RoleId,
    IsActive,
    LastLoginOn,
    FailedLoginAttempts,
    LockedUntil,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
    SessionsValidAfter,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PasswordResetTokens {
    Table,
    Id,
    UserId,
    Token,
    ExpiresAt,
    UsedAt,
    CreatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Permissions {
    Table,
    Id,
    Code,
    Name,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum RolePermissions {
    Table,
    RoleId,
    PermissionId,
    GrantedBy,
    GrantedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum CatalogueState {
    Table,
    Key,
    AppliedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Categories {
    Table,
    Id,
    Name,
    Slug,
    Type,
    Icon,
    Color,
    Description,
    DisplayOrder,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Businesses {
    Table,
    Id,
    Name,
    Phone,
    Email,
    Description,
    Address,
    Location,
    IsVerified,
    IsActive,
    IsDemo,
    OwnerId,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Services {
    Table,
    Id,
    BusinessId,
    CategoryId,
    Name,
    Description,
    PriceNote,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Products {
    Table,
    Id,
    BusinessId,
    CategoryId,
    Name,
    Description,
    Price,
    Unit,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
    ImageFileId,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Inquiries {
    Table,
    Id,
    BusinessId,
    ServiceId,
    ProductId,
    Topic,
    RequesterName,
    RequesterPhone,
    Note,
    Status,
    CreatedOn,
    UpdatedOn,
    UpdatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Reviews {
    Table,
    Id,
    BusinessId,
    InquiryId,
    Rating,
    Comment,
    ReviewerName,
    ReviewerPhoneHash,
    IsVisible,
    CreatedOn,
    UpdatedOn,
    UpdatedBy,
    Reply,
    RepliedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Locations {
    Table,
    Id,
    Label,
    Lat,
    Lng,
    DisplayOrder,
    IsActive,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum AuditLogs {
    Table,
    Id,
    AdminId,
    Action,
    EntityType,
    EntityId,
    Reason,
    CreatedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Files {
    Table,
    Id,
    StorageKey,
    ThumbKey,
    ContentType,
    ByteSize,
    Width,
    Height,
    Sha256,
    UploadedBy,
    CreatedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum RefreshTokens {
    Table,
    Id,
    FamilyId,
    UserId,
    TokenHash,
    ExpiresAt,
    CreatedOn,
    UsedAt,
    RevokedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum CallClicks {
    Table,
    Channel,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SearchEvents {
    Table,
    Id,
    Query,
    CategoryId,
    LocationId,
    Results,
    CreatedOn,
    WardCode,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Partners {
    Table,
    Id,
    Name,
    Kind,
    ContactName,
    Phone,
    Email,
    AccountId,
    IsActive,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerContracts {
    Table,
    Id,
    PartnerId,
    LocationId,
    StartsOn,
    EndsOn,
    MonthlyFee,
    Note,
    CreatedOn,
    CreatedBy,
    ProvinceCode,
    WardCode,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerContractCategories {
    Table,
    ContractId,
    CategoryId,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerSponsored {
    Table,
    ContractId,
    BusinessId,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Crops {
    Table,
    Id,
    Name,
    Kind,
    Unit,
    DisplayOrder,
    IsActive,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum CropPrices {
    Table,
    Id,
    CropId,
    LocationId,
    ObservedOn,
    PriceLow,
    PriceHigh,
    Note,
    CreatedOn,
    CreatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum JobPosts {
    Table,
    Id,
    CategoryId,
    LocationId,
    Location,
    Description,
    RequesterName,
    RequesterPhone,
    Status,
    CreatedOn,
}

// ── Database vocabulary: functions, types, index methods ──────────────────────

/// Functions called in defaults, indexes and checks.
#[derive(DeriveIden, Clone, Copy)]
pub enum DbFn {
    /// `now()`: the transaction's timestamp (column defaults).
    Now,
    /// Our IMMUTABLE wrapper of `unaccent`, usable in an index.
    FUnaccent,
    /// Sets `NEW.updated_on`; called by every `updated_on` trigger.
    FnSetUpdatedOn,
}

/// Extensions the schema needs.
#[derive(DeriveIden, Clone, Copy)]
pub enum Ext {
    Postgis,
    Unaccent,
    PgTrgm,
}

/// Index access methods and operator classes the builder has no name for.
#[derive(DeriveIden, Clone, Copy)]
pub enum IndexMethod {
    Gist,
    Gin,
    GinTrgmOps,
}

/// A PostGIS point on the WGS84 spheroid: distances in metres (see CLAUDE.md,
/// "Geo Query Pattern"). Rendered as the column type itself.
pub struct GeographyPoint;

impl Iden for GeographyPoint {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        // SRID 4326 is the project's only spatial reference.
        let _ = write!(s, "GEOGRAPHY(POINT, 4326)");
    }
}

/// A PostGIS multipolygon on the WGS84 spheroid: an administrative unit's
/// boundary, so a point-in-area test agrees with the distances.
pub struct GeographyMultiPolygon;

impl Iden for GeographyMultiPolygon {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        let _ = write!(s, "GEOGRAPHY(MULTIPOLYGON, 4326)");
    }
}

/// `now()`, for `DEFAULT now()` columns.
pub fn now() -> FunctionCall {
    Func::cust(DbFn::Now)
}

/// The Vietnamese mobile pattern (`application::validation::is_valid_vn_phone`).
pub const VN_PHONE_PATTERN: &str = "^0[0-9]{9}$";

/// `column ~ VN_PHONE_PATTERN`.
pub fn is_vn_phone<C: IntoIden + 'static>(column: C) -> SimpleExpr {
    Expr::col(column).binary(PgBinOper::Regex, Expr::val(VN_PHONE_PATTERN))
}

// ── Column shorthands (the baseline's column kinds) ───────────────────────────

/// `id SERIAL PRIMARY KEY`.
pub fn serial_id<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .integer()
        .auto_increment()
        .primary_key()
        .to_owned()
}

/// `id BIGSERIAL PRIMARY KEY`.
pub fn big_serial_id<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .big_integer()
        .auto_increment()
        .primary_key()
        .to_owned()
}

/// `TIMESTAMPTZ NOT NULL DEFAULT now()`.
pub fn stamped<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .default(now())
        .to_owned()
}

/// `TIMESTAMPTZ` (nullable).
pub fn timestamp_null<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c).timestamp_with_time_zone().to_owned()
}

/// `VARCHAR(len) NOT NULL`.
pub fn varchar<C: IntoIden>(c: C, len: u32) -> ColumnDef {
    ColumnDef::new(c).string_len(len).not_null().to_owned()
}

/// `VARCHAR(len)` (nullable).
pub fn varchar_null<C: IntoIden>(c: C, len: u32) -> ColumnDef {
    ColumnDef::new(c).string_len(len).to_owned()
}

/// `BOOLEAN NOT NULL DEFAULT value`.
pub fn flag<C: IntoIden>(c: C, value: bool) -> ColumnDef {
    ColumnDef::new(c)
        .boolean()
        .not_null()
        .default(value)
        .to_owned()
}

/// `INT` (nullable).
pub fn int_null<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c).integer().to_owned()
}

/// `INT NOT NULL`.
pub fn int<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c).integer().not_null().to_owned()
}

/// `TEXT` (nullable).
pub fn text_null<C: IntoIden>(c: C) -> ColumnDef {
    ColumnDef::new(c).text().to_owned()
}

/// `FOREIGN KEY (from) REFERENCES to (id) ON DELETE action`, unnamed so
/// Postgres names it `<table>_<column>_fkey` as an inline REFERENCES would.
pub fn references<T, C, R, I>(
    table: T,
    column: C,
    to: R,
    to_id: I,
    on_delete: ForeignKeyAction,
) -> ForeignKeyCreateStatement
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
    R: IntoIden + 'static,
    I: IntoIden + 'static,
{
    ForeignKey::create()
        .from(table, column)
        .to(to, to_id)
        .on_delete(on_delete)
        .to_owned()
}

/// `FOREIGN KEY (from) REFERENCES to (id)` with Postgres' default action.
pub fn references_plain<T, C, R, I>(
    table: T,
    column: C,
    to: R,
    to_id: I,
) -> ForeignKeyCreateStatement
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
    R: IntoIden + 'static,
    I: IntoIden + 'static,
{
    ForeignKey::create()
        .from(table, column)
        .to(to, to_id)
        .to_owned()
}

/// The `created_by` / `updated_by` audit columns: a user, kept NULL when the
/// user is deleted.
pub fn by_user<T, C>(table: T, column: C) -> ForeignKeyCreateStatement
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
{
    references(
        table,
        column,
        Users::Table,
        Users::Id,
        ForeignKeyAction::SetNull,
    )
}

// ── Statements the builder cannot express ─────────────────────────────────────

fn quoted(iden: &dyn Iden) -> String {
    iden.quoted(PostgresQueryBuilder.quote())
}

/// An expression as Postgres SQL, values inlined (DDL takes no parameters).
fn expr_sql(expr: &SimpleExpr) -> String {
    let mut sql = String::new();
    PostgresQueryBuilder.prepare_simple_expr(expr, &mut sql);
    sql
}

/// `ALTER TABLE table ADD CONSTRAINT name CHECK (expr)`: a named check (sea-query
/// 0.32 only writes unnamed ones, which Postgres would call `<table>_<col>_check`).
pub fn add_check(table: &dyn Iden, name: &str, check: SimpleExpr) -> String {
    format!(
        "ALTER TABLE {} ADD CONSTRAINT {} CHECK ({})",
        quoted(table),
        quoted(&Alias::new(name)),
        expr_sql(&check)
    )
}

/// `ALTER TABLE table ADD CONSTRAINT name UNIQUE (columns)`: a named unique
/// constraint (ON CONFLICT and error mapping rely on a constraint, not a bare
/// unique index).
pub fn add_unique(table: &dyn Iden, name: &str, columns: &[&dyn Iden]) -> String {
    let columns: Vec<String> = columns.iter().map(|c| quoted(*c)).collect();
    format!(
        "ALTER TABLE {} ADD CONSTRAINT {} UNIQUE ({})",
        quoted(table),
        quoted(&Alias::new(name)),
        columns.join(", ")
    )
}

/// `CREATE INDEX name ON table USING method ((expr) opclass)`: an expression
/// index with an operator class, which the index builder cannot attach.
pub fn create_opclass_index(
    name: &str,
    table: &dyn Iden,
    method: IndexMethod,
    expr: SimpleExpr,
    opclass: IndexMethod,
) -> String {
    format!(
        "CREATE INDEX {} ON {} USING {} (({}) {})",
        quoted(&Alias::new(name)),
        quoted(table),
        method.to_string(),
        expr_sql(&expr),
        opclass.to_string()
    )
}

/// The `updated_on` trigger of `table`: `trg_<table>_updated_on`.
pub fn create_updated_on_trigger(table: &dyn Iden) -> String {
    format!(
        "CREATE TRIGGER {} BEFORE UPDATE ON {} FOR EACH ROW EXECUTE FUNCTION {}()",
        quoted(&Alias::new(format!("trg_{}_updated_on", table.to_string()))),
        quoted(table),
        DbFn::FnSetUpdatedOn.to_string()
    )
}

/// A function definition: its body is code in the function's own language,
/// the one thing here that stays text.
pub struct FunctionDef {
    pub name: DbFn,
    pub args: &'static str,
    pub returns: &'static str,
    pub language: &'static str,
    pub attributes: &'static str,
    pub body: &'static str,
}

impl FunctionDef {
    pub fn create_or_replace(&self) -> String {
        format!(
            "CREATE OR REPLACE FUNCTION {}({}) RETURNS {} LANGUAGE {} {} AS $${}$$",
            self.name.to_string(),
            self.args,
            self.returns,
            self.language,
            self.attributes,
            self.body
        )
    }

    pub fn drop_if_exists(&self, cascade: bool) -> String {
        format!(
            "DROP FUNCTION IF EXISTS {}({}){}",
            self.name.to_string(),
            self.args,
            if cascade { " CASCADE" } else { "" }
        )
    }
}

/// Sets `updated_on` on every UPDATE of a table carrying the trigger.
pub const SET_UPDATED_ON: FunctionDef = FunctionDef {
    name: DbFn::FnSetUpdatedOn,
    args: "",
    returns: "TRIGGER",
    language: "plpgsql",
    attributes: "",
    body: "\nBEGIN\n    NEW.updated_on = NOW();\n    RETURN NEW;\nEND;\n",
};

/// `unaccent()` is only STABLE (its dictionary could change), so it cannot
/// appear in an index. This wrapper pins the dictionary and is declared
/// IMMUTABLE; changing unaccent.rules would require a REINDEX of the folded
/// search indexes.
pub const F_UNACCENT: FunctionDef = FunctionDef {
    name: DbFn::FUnaccent,
    args: "text",
    returns: "text",
    language: "sql",
    attributes: "IMMUTABLE PARALLEL SAFE STRICT",
    body: " SELECT public.unaccent('public.unaccent'::regdictionary, $1) ",
};

/// `CREATE EXTENSION IF NOT EXISTS ext`.
pub fn create_extension(ext: Ext) -> String {
    Extension::create()
        .name(ext.to_string())
        .if_not_exists()
        .to_string(PostgresQueryBuilder)
}

/// Runs statements the builder rendered (or the helpers above assembled).
pub async fn run(manager: &SchemaManager<'_>, statements: &[String]) -> Result<(), DbErr> {
    let db = manager.get_connection();
    for statement in statements {
        db.execute_unprepared(statement).await?;
    }
    Ok(())
}
