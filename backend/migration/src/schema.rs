//! The typed names of every table, column and database function the
//! migrations use, and the few Postgres statements sea-query 0.32 cannot
//! build (named CHECK constraints, operator-class indexes, sequences,
//! triggers and functions), each assembled from those names so no migration
//! spells SQL.
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
    EmailVerifiedAt,
    IsDemo,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum EmailVerificationTokens {
    Table,
    Id,
    UserId,
    TokenHash,
    ExpiresAt,
    UsedAt,
    CreatedOn,
    NewEmail,
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
pub enum Provinces {
    Table,
    Code,
    Name,
    FullName,
    UnitType,
    Center,
    IsActive,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Wards {
    Table,
    Code,
    ProvinceCode,
    Name,
    FullName,
    UnitType,
    Center,
    Boundary,
    IsActive,
}

/// A sector the platform serves: crops, livestock, aquaculture, as the
/// admin keeps them; named elsewhere by its `code` (columns: `CodedLists`).
#[derive(DeriveIden, Clone, Copy)]
pub enum Sectors {
    Table,
}

/// A kind of alert (disease, salt water, weather), as the admin keeps them;
/// `alerts.kind` names it by its `code` (columns: `CodedLists`).
#[derive(DeriveIden, Clone, Copy)]
pub enum AlertKinds {
    Table,
}

/// The columns `sectors` and `alert_kinds` share: short lists the admin
/// keeps, each entry named elsewhere by a code made from its name once.
#[derive(DeriveIden, Clone, Copy)]
pub enum CodedLists {
    Id,
    Code,
    Name,
    Icon,
    DisplayOrder,
    IsActive,
    IsDemo,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

/// What a farmer raises (cây trồng, vật nuôi): rice, durian, pigs, shrimp. It fixes
/// its sector; the price board's items and a supply's use point to it.
#[derive(DeriveIden, Clone, Copy)]
pub enum Commodities {
    Table,
    Id,
    Name,
    Sector,
    Icon,
    DisplayOrder,
    /// Days a price of its crops stays on the board (`m20261123_000004`).
    PriceFreshDays,
    IsActive,
    IsDemo,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

/// The platform settings an admin moved off their defaults (the registry in
/// `application::settings`); a key with no row is at its default.
#[derive(DeriveIden, Clone, Copy)]
pub enum PlatformSettings {
    Table,
    Key,
    Value,
    UpdatedOn,
    UpdatedBy,
}

/// The columns `service_types` and `supply_types` share: the kinds of work
/// a provider is hired for, and the kinds of supplies one sells. Two tables,
/// one id sequence (`Sequences::CatalogueTypeIds`), so a bare id names one
/// row of either.
#[derive(DeriveIden, Clone, Copy)]
pub enum OfferTypes {
    Id,
    Name,
    Slug,
    Icon,
    Color,
    Description,
    DisplayOrder,
    Sector,
    IsActive,
    IsDemo,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum ServiceTypes {
    Table,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SupplyTypes {
    Table,
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
    /// The ward its position lies in, kept with the position.
    WardCode,
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
    ServiceTypeId,
    Name,
    Description,
    PriceNote,
    IsActive,
    ImageFileId,
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
    SupplyTypeId,
    CommodityId,
    Name,
    Description,
    Price,
    Unit,
    IsActive,
    ImageFileId,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
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
    AnsweredOn,
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
    Reply,
    RepliedAt,
    CreatedOn,
    UpdatedOn,
    UpdatedBy,
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
    IsDemo,
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
    Id,
    BusinessId,
    ServiceId,
    ProductId,
    Channel,
    CreatedOn,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SearchEvents {
    Table,
    Id,
    Query,
    ServiceTypeId,
    SupplyTypeId,
    WardCode,
    Results,
    CreatedOn,
    IsDemo,
}

/// Taps older than the report keeps as rows, counted by week.
#[derive(DeriveIden, Clone, Copy)]
pub enum CallClickWeeks {
    Table,
    Id,
    WeekStartsOn,
    BusinessId,
    ServiceTypeId,
    SupplyTypeId,
    Channel,
    Taps,
}

/// Searches older than the report keeps as rows, counted by week.
#[derive(DeriveIden, Clone, Copy)]
pub enum SearchEventWeeks {
    Table,
    Id,
    WeekStartsOn,
    ServiceTypeId,
    SupplyTypeId,
    WardCode,
    Searches,
    Unmet,
    IsDemo,
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
    IsDemo,
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
    ProvinceCode,
    WardCode,
    StartsOn,
    EndsOn,
    MonthlyFee,
    Note,
    CreatedOn,
    CreatedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerContractServiceTypes {
    Table,
    ContractId,
    ServiceTypeId,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerContractSupplyTypes {
    Table,
    ContractId,
    SupplyTypeId,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum PartnerSponsored {
    Table,
    ContractId,
    BusinessId,
}

/// A farm produce item on the price board ("Lúa ST25", "Heo hơi"): a
/// variety or grade of one commodity.
#[derive(DeriveIden, Clone, Copy)]
pub enum Crops {
    Table,
    Id,
    CommodityId,
    Name,
    Unit,
    DisplayOrder,
    IsActive,
    IsDemo,
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
    IsDemo,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum JobPosts {
    Table,
    Id,
    ServiceTypeId,
    LocationId,
    Location,
    Description,
    RequesterName,
    RequesterPhone,
    Status,
    CreatedOn,
    IsDemo,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Alerts {
    Table,
    Id,
    Kind,
    Sector,
    ProvinceCode,
    Title,
    Body,
    StartsOn,
    EndsOn,
    IsActive,
    IsDemo,
    CreatedOn,
    UpdatedOn,
    CreatedBy,
    UpdatedBy,
}

/// Sequences created on their own (not a column's SERIAL).
#[derive(DeriveIden, Clone, Copy)]
pub enum Sequences {
    /// The one id sequence of `service_types` and `supply_types`.
    CatalogueTypeIds,
}

// ── Database vocabulary: functions, types, index methods ──────────────────────

/// Functions called in defaults, indexes and checks.
#[derive(DeriveIden, Clone, Copy)]
pub enum DbFn {
    /// `now()`: the transaction's timestamp (column defaults).
    Now,
    /// `nextval(sequence)`: a shared sequence's next value (column defaults).
    Nextval,
    /// Our IMMUTABLE wrapper of `unaccent`, usable in an index.
    FUnaccent,
    /// Sets `NEW.updated_on`; called by every `updated_on` trigger.
    FnSetUpdatedOn,
    /// Refuses a service or supply type whose id or slug the other table
    /// already has; called by both tables' `distinct` triggers.
    FnOfferTypeDistinct,
    /// `btrim(text)`: the text without its surrounding spaces (checks).
    Btrim,
    /// `num_nonnulls(a, b, …)`: how many of its arguments are set (checks).
    NumNonnulls,
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
    /// Block ranges: a few pages for a table written in time order.
    Brin,
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
    matches(column, VN_PHONE_PATTERN)
}

/// A slug or a code (`application::validation::SLUG_PATTERN`): lower-case
/// letters and digits in groups joined by single hyphens.
pub const SLUG_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

/// A SHA-256 as `hashing::sha256_hex` writes it: 64 lower-case hex digits.
pub const SHA256_HEX_PATTERN: &str = "^[0-9a-f]{64}$";

/// `column ~ pattern`.
pub fn matches<C: IntoIden + 'static>(column: C, pattern: &str) -> SimpleExpr {
    Expr::col(column).binary(PgBinOper::Regex, Expr::val(pattern))
}

/// `btrim(column) <> ''`: a NOT NULL text that says something. NOT NULL
/// alone lets an empty name through.
pub fn not_blank<C: IntoIden + 'static>(column: C) -> SimpleExpr {
    Expr::expr(Func::cust(DbFn::Btrim).arg(Expr::col(column))).ne("")
}

/// `column = lower(column)`: stored folded, so its UNIQUE ignores case.
pub fn is_lower<C: IntoIden + Copy + 'static>(column: C) -> SimpleExpr {
    Expr::col(column).eq(Func::lower(Expr::col(column)))
}

/// `num_nonnulls(a, b) <= 1`: two optional references of which a row names
/// at most one.
pub fn at_most_one<A, B>(a: A, b: B) -> SimpleExpr
where
    A: IntoIden + 'static,
    B: IntoIden + 'static,
{
    Expr::expr(
        Func::cust(DbFn::NumNonnulls)
            .arg(Expr::col(a))
            .arg(Expr::col(b)),
    )
    .lte(1)
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

/// `ALTER TABLE table DROP CONSTRAINT IF EXISTS name`: undoes `add_check`.
pub fn drop_constraint(table: &dyn Iden, name: &str) -> String {
    format!(
        "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
        quoted(table),
        quoted(&Alias::new(name))
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

/// `ALTER TABLE table ADD CONSTRAINT name FOREIGN KEY (columns) REFERENCES to
/// (to_columns) ON DELETE …`: a reference over several columns, so the row
/// pointed to must agree with this one on all of them (an inquiry's service
/// is one of the inquiry's own business). `clear`: the columns set to NULL
/// when that row is deleted (`ON DELETE SET NULL (clear)`, which the builder
/// cannot write); none: the delete is refused.
pub fn add_reference(
    table: &dyn Iden,
    name: &str,
    columns: &[&dyn Iden],
    to: &dyn Iden,
    to_columns: &[&dyn Iden],
    clear: &[&dyn Iden],
) -> String {
    let list = |columns: &[&dyn Iden]| -> String {
        let columns: Vec<String> = columns.iter().map(|c| quoted(*c)).collect();
        columns.join(", ")
    };
    let on_delete = if clear.is_empty() {
        "RESTRICT".to_owned()
    } else {
        format!("SET NULL ({})", list(clear))
    };
    format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {}",
        quoted(table),
        quoted(&Alias::new(name)),
        list(columns),
        quoted(to),
        list(to_columns),
        on_delete
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

/// The trigger of `table` that keeps its ids and slugs apart from `other`'s:
/// `trg_<table>_distinct`.
pub fn create_distinct_trigger(table: &dyn Iden, other: &dyn Iden) -> String {
    format!(
        "CREATE TRIGGER {} BEFORE INSERT OR UPDATE OF {}, {} ON {} FOR EACH ROW EXECUTE FUNCTION {}('{}')",
        quoted(&Alias::new(format!("trg_{}_distinct", table.to_string()))),
        quoted(&OfferTypes::Id),
        quoted(&OfferTypes::Slug),
        quoted(table),
        DbFn::FnOfferTypeDistinct.to_string(),
        other.to_string()
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

/// `service_types` and `supply_types` are two tables whose rows are named by
/// one id and one slug (a contract's category, a category page's address).
/// A UNIQUE covers one table only, so this refuses a row whose id or slug
/// the other table (the trigger's argument) has. The lock makes two writers
/// take turns, so neither misses the other's uncommitted row; types are
/// written by an admin, a few times a year. Raised as a unique violation:
/// the application answers it like the tables' own UNIQUE (`slug_taken`).
pub const OFFER_TYPE_DISTINCT: FunctionDef = FunctionDef {
    name: DbFn::FnOfferTypeDistinct,
    args: "",
    returns: "TRIGGER",
    language: "plpgsql",
    attributes: "",
    body: "
DECLARE
    clash BOOLEAN;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtext('catalogue_type_ids'));
    EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I WHERE id = $1 OR slug = $2)', TG_ARGV[0])
        INTO clash USING NEW.id, NEW.slug;
    IF clash THEN
        RAISE EXCEPTION 'id % or slug % of % violates unique constraint \"uq_catalogue_types_distinct\": % has it',
            NEW.id, NEW.slug, TG_TABLE_NAME, TG_ARGV[0]
            USING ERRCODE = 'unique_violation', CONSTRAINT = 'uq_catalogue_types_distinct';
    END IF;
    RETURN NEW;
END;
",
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

/// `CREATE SEQUENCE name AS integer`: an id sequence several tables draw from.
pub fn create_sequence(name: Sequences) -> String {
    format!("CREATE SEQUENCE {} AS integer", quoted(&name))
}

/// `INT PRIMARY KEY DEFAULT nextval('sequence')`: an id from a shared sequence.
pub fn sequence_id<C: IntoIden>(c: C, sequence: Sequences) -> ColumnDef {
    ColumnDef::new(c)
        .integer()
        .not_null()
        .primary_key()
        .default(Func::cust(DbFn::Nextval).arg(sequence.to_string()))
        .to_owned()
}

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
