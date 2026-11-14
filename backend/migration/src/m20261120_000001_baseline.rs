//! Baseline schema. DDL only: roles and permissions are seeded at startup
//! from `application::catalogue`, provinces and wards from the bundled
//! dataset, demo data only on request.
//!
//! The catalogue has three separate parts, each its own table, under the
//! sectors the platform serves (`sectors`, kept by the admin like the rest):
//!
//! - `commodities` (cây trồng, vật nuôi): what a farmer raises, rice, durian, pigs,
//!   shrimp; each in one sector.
//! - `service_types` (loại dịch vụ) and `supply_types` (loại vật tư): what a
//!   provider is hired for and what one sells. One id sequence for both, so
//!   the demand report and a partner's contract name either by a bare id.
//! - `crops` (nông sản): the price board's items, each a variety or grade of
//!   one commodity ("Lúa ST25", "Heo hơi").
//!
//! What the application checks before a write, the schema holds on its own:
//!
//! - A row that stores a parent and something of that parent points to it
//!   with both columns (`references_within`): an inquiry's or a tap's service
//!   and supply are its own business's, a review's contact request was sent
//!   to the business reviewed, a contract's ward is a ward of its province.
//! - Two optional references of which a row names one (`at_most_one`): what
//!   an inquiry or a tap is about, a search's type filter.
//! - `service_types` and `supply_types` share their ids and slugs: the
//!   `distinct` triggers refuse a row the other table has.
//! - Text that must say something is never blank (`named`); usernames, emails,
//!   codes and slugs are stored in their one spelling, so their UNIQUE holds.
//!
//! The value lists in the checks are frozen with this migration; the live
//! lists are in `domain::constants` and
//! `application::validation::is_valid_vn_phone`.

use sea_orm_migration::prelude::*;

use crate::{
    schema::{
        add_check, add_reference, add_unique, at_most_one, big_serial_id, by_user,
        create_distinct_trigger, create_extension, create_sequence, create_updated_on_trigger,
        flag, int, int_null, is_lower, is_vn_phone, matches, not_blank, references,
        references_plain, run, sequence_id, serial_id, stamped, text_null, timestamp_null, varchar,
        varchar_null, AlertKinds, Alerts, AuditLogs, Businesses, CallClicks, CatalogueState,
        CodedLists, Commodities, CropPrices, Crops, Ext, Files, GeographyMultiPolygon,
        GeographyPoint, IndexMethod, Inquiries, JobPosts, Locations, OfferTypes,
        PartnerContractServiceTypes, PartnerContractSupplyTypes, PartnerContracts,
        PartnerSponsored, Partners, PasswordResetTokens, Permissions, Products, Provinces,
        RefreshTokens, Reviews, RolePermissions, Roles, SearchEvents, Sectors, Sequences,
        ServiceTypes, Services, SupplyTypes, Users, Wards, F_UNACCENT, OFFER_TYPE_DISTINCT,
        SET_UPDATED_ON, SHA256_HEX_PATTERN, SLUG_PATTERN,
    },
    search::{search_indexes, FoldIndex},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen here; the live lists are in `domain::constants`.
const INQUIRY_STATUSES: [&str; 4] = ["new", "contacted", "closed", "spam"];
const CALL_CHANNELS: [&str; 2] = ["call", "zalo"];
const PARTNER_KINDS: [&str; 4] = ["brand", "dealer", "cooperative", "exporter"];
const JOB_STATUSES: [&str; 3] = ["open", "closed", "spam"];

/// An official unit code: two digits for a province, five for a ward.
const UNIT_CODE_LEN: u32 = 5;
const PROVINCE_CODE_PATTERN: &str = "^[0-9]{2}$";
const WARD_CODE_PATTERN: &str = "^[0-9]{5}$";

/// The tables carrying an `updated_on` trigger, in creation order.
const TRIGGERED: &[&dyn Iden] = &[
    &Roles::Table,
    &Users::Table,
    &Permissions::Table,
    &Sectors::Table,
    &AlertKinds::Table,
    &Commodities::Table,
    &ServiceTypes::Table,
    &SupplyTypes::Table,
    &Businesses::Table,
    &Services::Table,
    &Products::Table,
    &Inquiries::Table,
    &Reviews::Table,
    &Partners::Table,
    &Alerts::Table,
];

// ── Accounts ──────────────────────────────────────────────────────────────────

fn roles() -> TableCreateStatement {
    Table::create()
        .table(Roles::Table)
        .col(serial_id(Roles::Id))
        .col(varchar(Roles::Code, 50).unique_key())
        .col(varchar(Roles::Name, 100))
        .col(text_null(Roles::Description))
        .col(flag(Roles::IsActive, true))
        .col(stamped(Roles::CreatedOn))
        .col(stamped(Roles::UpdatedOn))
        .to_owned()
}

fn users() -> TableCreateStatement {
    Table::create()
        .table(Users::Table)
        .col(serial_id(Users::Id))
        .col(varchar(Users::Username, 100).unique_key())
        .col(varchar(Users::Email, 255).unique_key())
        .col(varchar(Users::PasswordHash, 255))
        .col(varchar_null(Users::FullName, 200))
        .col(int(Users::RoleId))
        .col(flag(Users::IsActive, true))
        .col(timestamp_null(Users::LastLoginOn))
        .col(int(Users::FailedLoginAttempts).default(0))
        .col(timestamp_null(Users::LockedUntil))
        // Access tokens issued before it are refused (password change / reset).
        .col(timestamp_null(Users::SessionsValidAfter))
        .col(stamped(Users::CreatedOn))
        .col(stamped(Users::UpdatedOn))
        .col(int_null(Users::CreatedBy))
        .col(int_null(Users::UpdatedBy))
        .col(timestamp_null(Users::DeletedAt))
        .foreign_key(&mut references_plain(
            Users::Table,
            Users::RoleId,
            Roles::Table,
            Roles::Id,
        ))
        .foreign_key(&mut by_user(Users::Table, Users::CreatedBy))
        .foreign_key(&mut by_user(Users::Table, Users::UpdatedBy))
        .to_owned()
}

fn password_reset_tokens() -> TableCreateStatement {
    Table::create()
        .table(PasswordResetTokens::Table)
        .col(serial_id(PasswordResetTokens::Id))
        .col(int(PasswordResetTokens::UserId))
        .col(varchar(PasswordResetTokens::Token, 255).unique_key())
        .col(
            ColumnDef::new(PasswordResetTokens::ExpiresAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(timestamp_null(PasswordResetTokens::UsedAt))
        .col(stamped(PasswordResetTokens::CreatedAt))
        .foreign_key(&mut references(
            PasswordResetTokens::Table,
            PasswordResetTokens::UserId,
            Users::Table,
            Users::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

fn refresh_tokens() -> TableCreateStatement {
    Table::create()
        .table(RefreshTokens::Table)
        .col(ColumnDef::new(RefreshTokens::Id).uuid().primary_key())
        .col(ColumnDef::new(RefreshTokens::FamilyId).uuid().not_null())
        .col(int(RefreshTokens::UserId))
        .col(
            ColumnDef::new(RefreshTokens::TokenHash)
                .text()
                .not_null()
                .unique_key(),
        )
        .col(
            ColumnDef::new(RefreshTokens::ExpiresAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(stamped(RefreshTokens::CreatedOn))
        .col(timestamp_null(RefreshTokens::UsedAt))
        .col(timestamp_null(RefreshTokens::RevokedAt))
        .foreign_key(&mut references(
            RefreshTokens::Table,
            RefreshTokens::UserId,
            Users::Table,
            Users::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

fn permissions() -> TableCreateStatement {
    Table::create()
        .table(Permissions::Table)
        .col(serial_id(Permissions::Id))
        .col(varchar(Permissions::Code, 100).unique_key())
        .col(varchar(Permissions::Name, 200))
        .col(stamped(Permissions::CreatedOn))
        .col(stamped(Permissions::UpdatedOn))
        .col(int_null(Permissions::CreatedBy))
        .col(int_null(Permissions::UpdatedBy))
        .foreign_key(&mut by_user(Permissions::Table, Permissions::CreatedBy))
        .foreign_key(&mut by_user(Permissions::Table, Permissions::UpdatedBy))
        .to_owned()
}

/// Role-based grants only: no permission is ever given to a user directly.
fn role_permissions() -> TableCreateStatement {
    Table::create()
        .table(RolePermissions::Table)
        .col(int(RolePermissions::RoleId))
        .col(int(RolePermissions::PermissionId))
        .col(int_null(RolePermissions::GrantedBy))
        .col(stamped(RolePermissions::GrantedOn))
        .primary_key(
            Index::create()
                .col(RolePermissions::RoleId)
                .col(RolePermissions::PermissionId),
        )
        .foreign_key(&mut references(
            RolePermissions::Table,
            RolePermissions::RoleId,
            Roles::Table,
            Roles::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            RolePermissions::Table,
            RolePermissions::PermissionId,
            Permissions::Table,
            Permissions::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut by_user(
            RolePermissions::Table,
            RolePermissions::GrantedBy,
        ))
        .to_owned()
}

/// One-time platform steps (the initial default-grant matrix, the dataset
/// version). A row here means "already done, never redo", which is what
/// keeps an admin's revocation from being undone at the next startup.
fn catalogue_state() -> TableCreateStatement {
    Table::create()
        .table(CatalogueState::Table)
        .col(
            ColumnDef::new(CatalogueState::Key)
                .string_len(100)
                .primary_key(),
        )
        .col(stamped(CatalogueState::AppliedOn))
        .to_owned()
}

fn audit_logs() -> TableCreateStatement {
    Table::create()
        .table(AuditLogs::Table)
        .col(big_serial_id(AuditLogs::Id))
        // Nullable so the log survives deletion of the admin who acted.
        .col(int_null(AuditLogs::AdminId))
        .col(varchar(AuditLogs::Action, 100))
        .col(varchar(AuditLogs::EntityType, 50))
        .col(int(AuditLogs::EntityId))
        .col(text_null(AuditLogs::Reason))
        .col(stamped(AuditLogs::CreatedOn))
        .foreign_key(&mut by_user(AuditLogs::Table, AuditLogs::AdminId))
        .to_owned()
}

/// `column > 0`.
fn positive(c: Files) -> SimpleExpr {
    Expr::col(c).gt(0)
}

/// `TEXT NOT NULL`.
fn text(c: Files) -> ColumnDef {
    ColumnDef::new(c).text().not_null().to_owned()
}

/// An uploaded image, re-encoded; listings point to it, never to a URL.
fn files() -> TableCreateStatement {
    Table::create()
        .table(Files::Table)
        .col(ColumnDef::new(Files::Id).uuid().primary_key())
        .col(text(Files::StorageKey).unique_key())
        .col(text(Files::ThumbKey).unique_key())
        .col(text(Files::ContentType))
        .col(
            ColumnDef::new(Files::ByteSize)
                .big_integer()
                .not_null()
                .check(positive(Files::ByteSize)),
        )
        .col(int(Files::Width).check(positive(Files::Width)))
        .col(int(Files::Height).check(positive(Files::Height)))
        .col(text(Files::Sha256))
        .col(int_null(Files::UploadedBy))
        .col(stamped(Files::CreatedOn))
        .foreign_key(&mut references(
            Files::Table,
            Files::UploadedBy,
            Users::Table,
            Users::Id,
            ForeignKeyAction::SetNull,
        ))
        .to_owned()
}

// ── Places ────────────────────────────────────────────────────────────────────

/// The 34 provinces since 1 July 2025, from the bundled dataset.
fn provinces() -> TableCreateStatement {
    Table::create()
        .table(Provinces::Table)
        .col(varchar(Provinces::Code, UNIT_CODE_LEN).primary_key())
        .col(varchar(Provinces::Name, 100))
        .col(varchar(Provinces::FullName, 120))
        .col(varchar(Provinces::UnitType, 20))
        .col(
            ColumnDef::new(Provinces::Center)
                .custom(GeographyPoint)
                .not_null(),
        )
        .col(flag(Provinces::IsActive, true))
        .to_owned()
}

/// Wards, communes and special zones, each with its simplified boundary.
fn wards() -> TableCreateStatement {
    Table::create()
        .table(Wards::Table)
        .col(varchar(Wards::Code, UNIT_CODE_LEN).primary_key())
        .col(varchar(Wards::ProvinceCode, UNIT_CODE_LEN))
        .col(varchar(Wards::Name, 100))
        .col(varchar(Wards::FullName, 120))
        .col(varchar(Wards::UnitType, 20))
        .col(
            ColumnDef::new(Wards::Center)
                .custom(GeographyPoint)
                .not_null(),
        )
        .col(
            ColumnDef::new(Wards::Boundary)
                .custom(GeographyMultiPolygon)
                .not_null(),
        )
        .col(flag(Wards::IsActive, true))
        .foreign_key(&mut references(
            Wards::Table,
            Wards::ProvinceCode,
            Provinces::Table,
            Provinces::Code,
            ForeignKeyAction::Restrict,
        ))
        .to_owned()
}

/// The areas farmers pick from (near-me, prices, job posts).
fn locations() -> TableCreateStatement {
    Table::create()
        .table(Locations::Table)
        .col(serial_id(Locations::Id))
        .col(varchar(Locations::Label, 150).unique_key())
        .col(ColumnDef::new(Locations::Lat).double().not_null())
        .col(ColumnDef::new(Locations::Lng).double().not_null())
        .col(int(Locations::DisplayOrder).default(0))
        .col(flag(Locations::IsActive, true))
        .col(flag(Locations::IsDemo, false))
        .to_owned()
}

// ── Catalogue ─────────────────────────────────────────────────────────────────

/// A code's width: `sectors.code`, `alert_kinds.code` and every column
/// pointing to one.
const SECTOR_CODE_LEN: u32 = 40;

/// A short list the admin keeps (`sectors`: the sectors the platform serves,
/// the top of the catalogue; `alert_kinds`). `code` is made from the name
/// once and never changes (the sector cookie and `sector=` keep it); other
/// tables point to it.
fn coded_list<T: IntoIden + Copy + 'static>(table: T) -> TableCreateStatement {
    Table::create()
        .table(table)
        .col(serial_id(CodedLists::Id))
        .col(varchar(CodedLists::Code, SECTOR_CODE_LEN).unique_key())
        .col(varchar(CodedLists::Name, 100).unique_key())
        .col(varchar_null(CodedLists::Icon, 100))
        .col(
            ColumnDef::new(CodedLists::DisplayOrder)
                .small_integer()
                .not_null()
                .default(0),
        )
        .col(flag(CodedLists::IsActive, true))
        .col(flag(CodedLists::IsDemo, false))
        .col(stamped(CodedLists::CreatedOn))
        .col(stamped(CodedLists::UpdatedOn))
        .col(int_null(CodedLists::CreatedBy))
        .col(int_null(CodedLists::UpdatedBy))
        .foreign_key(&mut by_user(table, CodedLists::CreatedBy))
        .foreign_key(&mut by_user(table, CodedLists::UpdatedBy))
        .to_owned()
}

/// `column` of `table` names a sector by its code.
fn of_sector<T, C>(table: T, column: C) -> ForeignKeyCreateStatement
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
{
    references_plain(table, column, Sectors::Table, CodedLists::Code)
}

fn commodities() -> TableCreateStatement {
    Table::create()
        .table(Commodities::Table)
        .col(serial_id(Commodities::Id))
        .col(varchar(Commodities::Name, 100).unique_key())
        .col(varchar(Commodities::Sector, SECTOR_CODE_LEN))
        .col(varchar_null(Commodities::Icon, 100))
        .col(
            ColumnDef::new(Commodities::DisplayOrder)
                .small_integer()
                .not_null()
                .default(0),
        )
        .col(flag(Commodities::IsActive, true))
        .col(flag(Commodities::IsDemo, false))
        .col(stamped(Commodities::CreatedOn))
        .col(stamped(Commodities::UpdatedOn))
        .col(int_null(Commodities::CreatedBy))
        .col(int_null(Commodities::UpdatedBy))
        .foreign_key(&mut of_sector(Commodities::Table, Commodities::Sector))
        .foreign_key(&mut by_user(Commodities::Table, Commodities::CreatedBy))
        .foreign_key(&mut by_user(Commodities::Table, Commodities::UpdatedBy))
        .to_owned()
}

/// `service_types` or `supply_types`: the same columns, ids from one
/// sequence. `sector` none: it serves every sector (machine repair, tools).
fn offer_types<T: IntoIden + Copy + 'static>(table: T) -> TableCreateStatement {
    Table::create()
        .table(table)
        .col(sequence_id(OfferTypes::Id, Sequences::CatalogueTypeIds))
        .col(varchar(OfferTypes::Name, 100))
        .col(varchar(OfferTypes::Slug, 100).unique_key())
        .col(varchar_null(OfferTypes::Icon, 100))
        .col(varchar_null(OfferTypes::Color, 20))
        .col(varchar_null(OfferTypes::Description, 300))
        .col(
            ColumnDef::new(OfferTypes::DisplayOrder)
                .small_integer()
                .not_null()
                .default(0),
        )
        .col(varchar_null(OfferTypes::Sector, SECTOR_CODE_LEN))
        .col(flag(OfferTypes::IsActive, true))
        .col(flag(OfferTypes::IsDemo, false))
        .col(stamped(OfferTypes::CreatedOn))
        .col(stamped(OfferTypes::UpdatedOn))
        .col(int_null(OfferTypes::CreatedBy))
        .col(int_null(OfferTypes::UpdatedBy))
        .foreign_key(&mut of_sector(table, OfferTypes::Sector))
        .foreign_key(&mut by_user(table, OfferTypes::CreatedBy))
        .foreign_key(&mut by_user(table, OfferTypes::UpdatedBy))
        .to_owned()
}

/// A price board item: one commodity's variety or grade.
fn crops() -> TableCreateStatement {
    Table::create()
        .table(Crops::Table)
        .col(serial_id(Crops::Id))
        .col(int(Crops::CommodityId))
        .col(varchar(Crops::Name, 100).unique_key())
        .col(varchar(Crops::Unit, 20))
        .col(int(Crops::DisplayOrder).default(0))
        .col(flag(Crops::IsActive, true))
        .col(flag(Crops::IsDemo, false))
        .foreign_key(&mut references(
            Crops::Table,
            Crops::CommodityId,
            Commodities::Table,
            Commodities::Id,
            ForeignKeyAction::Restrict,
        ))
        .to_owned()
}

// ── Listings ──────────────────────────────────────────────────────────────────

fn businesses() -> TableCreateStatement {
    Table::create()
        .table(Businesses::Table)
        .col(serial_id(Businesses::Id))
        .col(varchar(Businesses::Name, 200))
        .col(varchar(Businesses::Phone, 20))
        .col(varchar_null(Businesses::Email, 255))
        .col(text_null(Businesses::Description))
        .col(varchar_null(Businesses::Address, 500))
        // GEOGRAPHY (not GEOMETRY) so distances are metres on the spheroid.
        .col(ColumnDef::new(Businesses::Location).custom(GeographyPoint))
        .col(flag(Businesses::IsVerified, false))
        .col(flag(Businesses::IsActive, true))
        // Sample listings; "remove demo data" deletes exactly these rows.
        .col(flag(Businesses::IsDemo, false))
        .col(int_null(Businesses::OwnerId))
        .col(stamped(Businesses::CreatedOn))
        .col(stamped(Businesses::UpdatedOn))
        .col(int_null(Businesses::CreatedBy))
        .col(int_null(Businesses::UpdatedBy))
        .col(timestamp_null(Businesses::DeletedAt))
        .foreign_key(&mut by_user(Businesses::Table, Businesses::OwnerId))
        .foreign_key(&mut by_user(Businesses::Table, Businesses::CreatedBy))
        .foreign_key(&mut by_user(Businesses::Table, Businesses::UpdatedBy))
        .to_owned()
}

fn services() -> TableCreateStatement {
    Table::create()
        .table(Services::Table)
        .col(serial_id(Services::Id))
        .col(int(Services::BusinessId))
        .col(int(Services::ServiceTypeId))
        .col(varchar(Services::Name, 300))
        .col(text_null(Services::Description))
        .col(varchar_null(Services::PriceNote, 300))
        .col(flag(Services::IsActive, true))
        .col(ColumnDef::new(Services::ImageFileId).uuid())
        .col(stamped(Services::CreatedOn))
        .col(stamped(Services::UpdatedOn))
        .col(int_null(Services::CreatedBy))
        .col(int_null(Services::UpdatedBy))
        .col(timestamp_null(Services::DeletedAt))
        .foreign_key(&mut references(
            Services::Table,
            Services::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            Services::Table,
            Services::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut references(
            Services::Table,
            Services::ImageFileId,
            Files::Table,
            Files::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Services::Table, Services::CreatedBy))
        .foreign_key(&mut by_user(Services::Table, Services::UpdatedBy))
        .to_owned()
}

/// A supply ("vật tư"); `commodity_id` is what it is for (lúa giống ST25:
/// rice), none for every commodity (NPK, a sprayer).
fn products() -> TableCreateStatement {
    Table::create()
        .table(Products::Table)
        .col(serial_id(Products::Id))
        .col(int(Products::BusinessId))
        .col(int(Products::SupplyTypeId))
        .col(int_null(Products::CommodityId))
        .col(varchar(Products::Name, 300))
        .col(text_null(Products::Description))
        .col(ColumnDef::new(Products::Price).big_integer())
        .col(varchar_null(Products::Unit, 50))
        .col(flag(Products::IsActive, true))
        .col(ColumnDef::new(Products::ImageFileId).uuid())
        .col(stamped(Products::CreatedOn))
        .col(stamped(Products::UpdatedOn))
        .col(int_null(Products::CreatedBy))
        .col(int_null(Products::UpdatedBy))
        .col(timestamp_null(Products::DeletedAt))
        .foreign_key(&mut references(
            Products::Table,
            Products::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            Products::Table,
            Products::SupplyTypeId,
            SupplyTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut references(
            Products::Table,
            Products::CommodityId,
            Commodities::Table,
            Commodities::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut references(
            Products::Table,
            Products::ImageFileId,
            Files::Table,
            Files::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Products::Table, Products::CreatedBy))
        .foreign_key(&mut by_user(Products::Table, Products::UpdatedBy))
        .to_owned()
}

// ── Farmers' contact ─────────────────────────────────────────────────────────

fn inquiries() -> TableCreateStatement {
    Table::create()
        .table(Inquiries::Table)
        .col(serial_id(Inquiries::Id))
        .col(int(Inquiries::BusinessId))
        .col(int_null(Inquiries::ServiceId))
        .col(int_null(Inquiries::ProductId))
        .col(varchar(Inquiries::Topic, 300))
        .col(varchar(Inquiries::RequesterName, 200))
        .col(varchar(Inquiries::RequesterPhone, 20))
        .col(text_null(Inquiries::Note))
        .col(varchar(Inquiries::Status, 20).default("new"))
        .col(stamped(Inquiries::CreatedOn))
        .col(stamped(Inquiries::UpdatedOn))
        .col(int_null(Inquiries::UpdatedBy))
        .foreign_key(&mut references(
            Inquiries::Table,
            Inquiries::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut by_user(Inquiries::Table, Inquiries::UpdatedBy))
        .to_owned()
}

/// A review, with the provider's public reply (NULL: none yet).
fn reviews() -> TableCreateStatement {
    Table::create()
        .table(Reviews::Table)
        .col(serial_id(Reviews::Id))
        .col(int(Reviews::BusinessId))
        .col(int_null(Reviews::InquiryId))
        .col(ColumnDef::new(Reviews::Rating).small_integer().not_null())
        .col(text_null(Reviews::Comment))
        .col(varchar(Reviews::ReviewerName, 200))
        .col(varchar(Reviews::ReviewerPhoneHash, 64))
        // Hidden until an admin shows it.
        .col(flag(Reviews::IsVisible, false))
        .col(text_null(Reviews::Reply))
        .col(timestamp_null(Reviews::RepliedAt))
        .col(stamped(Reviews::CreatedOn))
        .col(stamped(Reviews::UpdatedOn))
        .col(int_null(Reviews::UpdatedBy))
        .foreign_key(&mut references(
            Reviews::Table,
            Reviews::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut by_user(Reviews::Table, Reviews::UpdatedBy))
        .to_owned()
}

/// A tap on a public call or Zalo button: what was tapped and when, never
/// who tapped.
fn call_clicks() -> TableCreateStatement {
    Table::create()
        .table(CallClicks::Table)
        .col(big_serial_id(CallClicks::Id))
        .col(int(CallClicks::BusinessId))
        .col(int_null(CallClicks::ServiceId))
        .col(int_null(CallClicks::ProductId))
        .col(varchar(CallClicks::Channel, 10).default("call"))
        .col(stamped(CallClicks::CreatedOn))
        .foreign_key(&mut references(
            CallClicks::Table,
            CallClicks::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

/// What farmers looked for: the words (lowercased, capped), the service or
/// supply type filter, the ward, the result count; never who searched.
fn search_events() -> TableCreateStatement {
    Table::create()
        .table(SearchEvents::Table)
        .col(big_serial_id(SearchEvents::Id))
        .col(varchar_null(SearchEvents::Query, 100))
        .col(int_null(SearchEvents::ServiceTypeId))
        .col(int_null(SearchEvents::SupplyTypeId))
        .col(varchar_null(SearchEvents::WardCode, UNIT_CODE_LEN))
        .col(int(SearchEvents::Results))
        .col(stamped(SearchEvents::CreatedOn))
        .col(flag(SearchEvents::IsDemo, false))
        .foreign_key(&mut references(
            SearchEvents::Table,
            SearchEvents::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            SearchEvents::Table,
            SearchEvents::SupplyTypeId,
            SupplyTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            SearchEvents::Table,
            SearchEvents::WardCode,
            Wards::Table,
            Wards::Code,
            ForeignKeyAction::SetNull,
        ))
        .to_owned()
}

/// A farmer's job post, never public: providers nearby that offer its
/// service type see it.
fn job_posts() -> TableCreateStatement {
    Table::create()
        .table(JobPosts::Table)
        .col(serial_id(JobPosts::Id))
        .col(int(JobPosts::ServiceTypeId))
        .col(int_null(JobPosts::LocationId))
        .col(
            ColumnDef::new(JobPosts::Location)
                .custom(GeographyPoint)
                .not_null(),
        )
        .col(varchar(JobPosts::Description, 500))
        .col(varchar(JobPosts::RequesterName, 100))
        .col(varchar(JobPosts::RequesterPhone, 20))
        .col(varchar(JobPosts::Status, 20).default("open"))
        .col(stamped(JobPosts::CreatedOn))
        .col(flag(JobPosts::IsDemo, false))
        .foreign_key(&mut references(
            JobPosts::Table,
            JobPosts::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            JobPosts::Table,
            JobPosts::LocationId,
            Locations::Table,
            Locations::Id,
            ForeignKeyAction::SetNull,
        ))
        .to_owned()
}

// ── Prices and alerts ─────────────────────────────────────────────────────────

/// A crop's farm-gate price in one area on one day, low–high đồng per unit.
fn crop_prices() -> TableCreateStatement {
    Table::create()
        .table(CropPrices::Table)
        .col(serial_id(CropPrices::Id))
        .col(int(CropPrices::CropId))
        .col(int(CropPrices::LocationId))
        .col(ColumnDef::new(CropPrices::ObservedOn).date().not_null())
        .col(
            ColumnDef::new(CropPrices::PriceLow)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(CropPrices::PriceHigh)
                .big_integer()
                .not_null(),
        )
        .col(varchar_null(CropPrices::Note, 200))
        .col(stamped(CropPrices::CreatedOn))
        .col(int_null(CropPrices::CreatedBy))
        .col(flag(CropPrices::IsDemo, false))
        .foreign_key(&mut references(
            CropPrices::Table,
            CropPrices::CropId,
            Crops::Table,
            Crops::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            CropPrices::Table,
            CropPrices::LocationId,
            Locations::Table,
            Locations::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut by_user(CropPrices::Table, CropPrices::CreatedBy))
        .to_owned()
}

/// A disease outbreak, salt water or a storm: one sector or all, one
/// province or the whole region, between two days.
fn alerts() -> TableCreateStatement {
    Table::create()
        .table(Alerts::Table)
        .col(serial_id(Alerts::Id))
        .col(varchar(Alerts::Kind, SECTOR_CODE_LEN))
        .col(varchar_null(Alerts::Sector, SECTOR_CODE_LEN))
        .col(varchar_null(Alerts::ProvinceCode, UNIT_CODE_LEN))
        .col(varchar(Alerts::Title, 120))
        .col(ColumnDef::new(Alerts::Body).text().not_null())
        .col(ColumnDef::new(Alerts::StartsOn).date().not_null())
        .col(ColumnDef::new(Alerts::EndsOn).date().not_null())
        .col(flag(Alerts::IsActive, true))
        .col(flag(Alerts::IsDemo, false))
        .col(stamped(Alerts::CreatedOn))
        .col(stamped(Alerts::UpdatedOn))
        .col(int_null(Alerts::CreatedBy))
        .col(int_null(Alerts::UpdatedBy))
        .foreign_key(&mut references_plain(
            Alerts::Table,
            Alerts::ProvinceCode,
            Provinces::Table,
            Provinces::Code,
        ))
        .foreign_key(&mut of_sector(Alerts::Table, Alerts::Sector))
        .foreign_key(&mut references_plain(
            Alerts::Table,
            Alerts::Kind,
            AlertKinds::Table,
            CodedLists::Code,
        ))
        .foreign_key(&mut by_user(Alerts::Table, Alerts::CreatedBy))
        .foreign_key(&mut by_user(Alerts::Table, Alerts::UpdatedBy))
        .to_owned()
}

// ── Partners ──────────────────────────────────────────────────────────────────

fn partners() -> TableCreateStatement {
    Table::create()
        .table(Partners::Table)
        .col(serial_id(Partners::Id))
        .col(varchar(Partners::Name, 200))
        .col(varchar(Partners::Kind, 20))
        .col(varchar_null(Partners::ContactName, 200))
        .col(varchar_null(Partners::Phone, 20))
        .col(varchar_null(Partners::Email, 255))
        .col(int_null(Partners::AccountId).unique_key())
        .col(flag(Partners::IsActive, true))
        .col(flag(Partners::IsDemo, false))
        .col(stamped(Partners::CreatedOn))
        .col(stamped(Partners::UpdatedOn))
        .col(int_null(Partners::CreatedBy))
        .col(int_null(Partners::UpdatedBy))
        .foreign_key(&mut references(
            Partners::Table,
            Partners::AccountId,
            Users::Table,
            Users::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Partners::Table, Partners::CreatedBy))
        .foreign_key(&mut by_user(Partners::Table, Partners::UpdatedBy))
        .to_owned()
}

/// A contract covers a whole province or one ward of it.
fn partner_contracts() -> TableCreateStatement {
    Table::create()
        .table(PartnerContracts::Table)
        .col(serial_id(PartnerContracts::Id))
        .col(int(PartnerContracts::PartnerId))
        .col(varchar(PartnerContracts::ProvinceCode, UNIT_CODE_LEN))
        .col(varchar_null(PartnerContracts::WardCode, UNIT_CODE_LEN))
        .col(ColumnDef::new(PartnerContracts::StartsOn).date().not_null())
        .col(ColumnDef::new(PartnerContracts::EndsOn).date().not_null())
        .col(
            ColumnDef::new(PartnerContracts::MonthlyFee)
                .big_integer()
                .not_null(),
        )
        .col(text_null(PartnerContracts::Note))
        .col(stamped(PartnerContracts::CreatedOn))
        .col(int_null(PartnerContracts::CreatedBy))
        .foreign_key(&mut references(
            PartnerContracts::Table,
            PartnerContracts::PartnerId,
            Partners::Table,
            Partners::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerContracts::Table,
            PartnerContracts::ProvinceCode,
            Provinces::Table,
            Provinces::Code,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut by_user(
            PartnerContracts::Table,
            PartnerContracts::CreatedBy,
        ))
        .to_owned()
}

/// The service types a contract covers.
fn partner_contract_service_types() -> TableCreateStatement {
    Table::create()
        .table(PartnerContractServiceTypes::Table)
        .col(int(PartnerContractServiceTypes::ContractId))
        .col(int(PartnerContractServiceTypes::ServiceTypeId))
        .primary_key(
            Index::create()
                .col(PartnerContractServiceTypes::ContractId)
                .col(PartnerContractServiceTypes::ServiceTypeId),
        )
        .foreign_key(&mut references(
            PartnerContractServiceTypes::Table,
            PartnerContractServiceTypes::ContractId,
            PartnerContracts::Table,
            PartnerContracts::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerContractServiceTypes::Table,
            PartnerContractServiceTypes::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

/// The supply types a contract covers.
fn partner_contract_supply_types() -> TableCreateStatement {
    Table::create()
        .table(PartnerContractSupplyTypes::Table)
        .col(int(PartnerContractSupplyTypes::ContractId))
        .col(int(PartnerContractSupplyTypes::SupplyTypeId))
        .primary_key(
            Index::create()
                .col(PartnerContractSupplyTypes::ContractId)
                .col(PartnerContractSupplyTypes::SupplyTypeId),
        )
        .foreign_key(&mut references(
            PartnerContractSupplyTypes::Table,
            PartnerContractSupplyTypes::ContractId,
            PartnerContracts::Table,
            PartnerContracts::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerContractSupplyTypes::Table,
            PartnerContractSupplyTypes::SupplyTypeId,
            SupplyTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

fn partner_sponsored() -> TableCreateStatement {
    Table::create()
        .table(PartnerSponsored::Table)
        .col(int(PartnerSponsored::ContractId))
        .col(int(PartnerSponsored::BusinessId))
        .primary_key(
            Index::create()
                .col(PartnerSponsored::ContractId)
                .col(PartnerSponsored::BusinessId),
        )
        .foreign_key(&mut references(
            PartnerSponsored::Table,
            PartnerSponsored::ContractId,
            PartnerContracts::Table,
            PartnerContracts::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerSponsored::Table,
            PartnerSponsored::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

// ── Constraints and indexes ───────────────────────────────────────────────────

/// The names a row cannot do without: NOT NULL alone lets `''` through.
fn named() -> Vec<String> {
    let not_blank_of = |table: &dyn Iden, name: &str, check: SimpleExpr| {
        add_check(table, &format!("chk_{name}_not_blank"), check)
    };
    vec![
        not_blank_of(&Users::Table, "users_username", not_blank(Users::Username)),
        not_blank_of(
            &Locations::Table,
            "locations_label",
            not_blank(Locations::Label),
        ),
        not_blank_of(&Sectors::Table, "sectors_name", not_blank(CodedLists::Name)),
        not_blank_of(
            &AlertKinds::Table,
            "alert_kinds_name",
            not_blank(CodedLists::Name),
        ),
        not_blank_of(
            &Commodities::Table,
            "commodities_name",
            not_blank(Commodities::Name),
        ),
        not_blank_of(
            &ServiceTypes::Table,
            "service_types_name",
            not_blank(OfferTypes::Name),
        ),
        not_blank_of(
            &SupplyTypes::Table,
            "supply_types_name",
            not_blank(OfferTypes::Name),
        ),
        not_blank_of(&Crops::Table, "crops_name", not_blank(Crops::Name)),
        not_blank_of(&Crops::Table, "crops_unit", not_blank(Crops::Unit)),
        not_blank_of(
            &Businesses::Table,
            "businesses_name",
            not_blank(Businesses::Name),
        ),
        not_blank_of(&Services::Table, "services_name", not_blank(Services::Name)),
        not_blank_of(&Products::Table, "products_name", not_blank(Products::Name)),
        not_blank_of(
            &Inquiries::Table,
            "inquiries_topic",
            not_blank(Inquiries::Topic),
        ),
        not_blank_of(
            &Inquiries::Table,
            "inquiries_requester_name",
            not_blank(Inquiries::RequesterName),
        ),
        not_blank_of(
            &Reviews::Table,
            "reviews_reviewer_name",
            not_blank(Reviews::ReviewerName),
        ),
        not_blank_of(
            &JobPosts::Table,
            "job_posts_description",
            not_blank(JobPosts::Description),
        ),
        not_blank_of(
            &JobPosts::Table,
            "job_posts_requester_name",
            not_blank(JobPosts::RequesterName),
        ),
        not_blank_of(&Alerts::Table, "alerts_title", not_blank(Alerts::Title)),
        not_blank_of(&Alerts::Table, "alerts_body", not_blank(Alerts::Body)),
        not_blank_of(&Partners::Table, "partners_name", not_blank(Partners::Name)),
    ]
}

/// The keys a reference over two columns points to, then those references.
/// A row that stores both a parent and something of that parent (an inquiry's
/// business and its service) could otherwise name a service of another
/// business; with the parent in the reference, it cannot.
fn references_within() -> Vec<String> {
    let mut statements = vec![
        add_unique(
            &Wards::Table,
            "uq_wards_code_province",
            &[&Wards::Code, &Wards::ProvinceCode],
        ),
        add_unique(
            &Services::Table,
            "uq_services_id_business",
            &[&Services::Id, &Services::BusinessId],
        ),
        add_unique(
            &Products::Table,
            "uq_products_id_business",
            &[&Products::Id, &Products::BusinessId],
        ),
        add_unique(
            &Inquiries::Table,
            "uq_inquiries_id_business",
            &[&Inquiries::Id, &Inquiries::BusinessId],
        ),
        // A contract's ward is a ward of the contract's province.
        add_reference(
            &PartnerContracts::Table,
            "fk_partner_contracts_ward",
            &[&PartnerContracts::WardCode, &PartnerContracts::ProvinceCode],
            &Wards::Table,
            &[&Wards::Code, &Wards::ProvinceCode],
            &[],
        ),
        // A review's contact request was sent to the business it reviews.
        add_reference(
            &Reviews::Table,
            "fk_reviews_inquiry",
            &[&Reviews::InquiryId, &Reviews::BusinessId],
            &Inquiries::Table,
            &[&Inquiries::Id, &Inquiries::BusinessId],
            &[&Reviews::InquiryId],
        ),
    ];
    statements.extend(about_own(
        &Inquiries::Table,
        "inquiries",
        &Inquiries::BusinessId,
        &Inquiries::ServiceId,
        &Inquiries::ProductId,
    ));
    statements.extend(about_own(
        &CallClicks::Table,
        "call_clicks",
        &CallClicks::BusinessId,
        &CallClicks::ServiceId,
        &CallClicks::ProductId,
    ));
    statements
}

/// A contact request or a tap (`table`, named `name`) about a service or a
/// supply is about one of its own business's; when that one is deleted the
/// row stays, about the business alone.
fn about_own(
    table: &dyn Iden,
    name: &str,
    business_id: &dyn Iden,
    service_id: &dyn Iden,
    product_id: &dyn Iden,
) -> [String; 2] {
    [
        add_reference(
            table,
            &format!("fk_{name}_service"),
            &[service_id, business_id],
            &Services::Table,
            &[&Services::Id, &Services::BusinessId],
            &[service_id],
        ),
        add_reference(
            table,
            &format!("fk_{name}_product"),
            &[product_id, business_id],
            &Products::Table,
            &[&Products::Id, &Products::BusinessId],
            &[product_id],
        ),
    ]
}

fn checks() -> Vec<String> {
    vec![
        // The application folds both before every write and lookup; here so
        // that their UNIQUE cannot hold "An" beside "an".
        add_check(
            &Users::Table,
            "chk_users_username_lower",
            is_lower(Users::Username),
        ),
        add_check(
            &Users::Table,
            "chk_users_email_lower",
            is_lower(Users::Email),
        ),
        add_check(
            &Users::Table,
            "chk_users_failed_login_attempts",
            Expr::col(Users::FailedLoginAttempts).gte(0),
        ),
        add_check(
            &Files::Table,
            "chk_files_sha256",
            matches(Files::Sha256, SHA256_HEX_PATTERN),
        ),
        add_check(
            &Provinces::Table,
            "chk_provinces_code",
            matches(Provinces::Code, PROVINCE_CODE_PATTERN),
        ),
        add_check(
            &Wards::Table,
            "chk_wards_code",
            matches(Wards::Code, WARD_CODE_PATTERN),
        ),
        add_check(
            &Locations::Table,
            "chk_locations_point",
            Expr::col(Locations::Lat)
                .between(-90.0, 90.0)
                .and(Expr::col(Locations::Lng).between(-180.0, 180.0)),
        ),
        add_check(
            &Sectors::Table,
            "chk_sectors_code",
            matches(CodedLists::Code, SLUG_PATTERN),
        ),
        add_check(
            &AlertKinds::Table,
            "chk_alert_kinds_code",
            matches(CodedLists::Code, SLUG_PATTERN),
        ),
        add_check(
            &ServiceTypes::Table,
            "chk_service_types_slug",
            matches(OfferTypes::Slug, SLUG_PATTERN),
        ),
        add_check(
            &SupplyTypes::Table,
            "chk_supply_types_slug",
            matches(OfferTypes::Slug, SLUG_PATTERN),
        ),
        add_check(
            &Inquiries::Table,
            "chk_inquiries_about_one",
            at_most_one(Inquiries::ServiceId, Inquiries::ProductId),
        ),
        add_check(
            &CallClicks::Table,
            "chk_call_clicks_about_one",
            at_most_one(CallClicks::ServiceId, CallClicks::ProductId),
        ),
        // A search is over services or over supplies, never both.
        add_check(
            &SearchEvents::Table,
            "chk_search_events_one_type",
            at_most_one(SearchEvents::ServiceTypeId, SearchEvents::SupplyTypeId),
        ),
        add_check(
            &SearchEvents::Table,
            "chk_search_events_results",
            Expr::col(SearchEvents::Results).gte(0),
        ),
        add_check(
            &Reviews::Table,
            "chk_reviews_phone_hash",
            matches(Reviews::ReviewerPhoneHash, SHA256_HEX_PATTERN),
        ),
        // A reply and its time are set together and removed together.
        add_check(
            &Reviews::Table,
            "chk_reviews_reply",
            Expr::expr(Expr::col(Reviews::Reply).is_null())
                .eq(Expr::col(Reviews::RepliedAt).is_null()),
        ),
        add_check(
            &Provinces::Table,
            "chk_provinces_unit_type",
            Expr::col(Provinces::UnitType).is_in(["province", "city"]),
        ),
        add_check(
            &Wards::Table,
            "chk_wards_unit_type",
            Expr::col(Wards::UnitType).is_in(["ward", "commune", "special_zone"]),
        ),
        add_check(
            &Businesses::Table,
            "chk_businesses_phone",
            is_vn_phone(Businesses::Phone),
        ),
        add_check(
            &Products::Table,
            "chk_products_price",
            Expr::col(Products::Price)
                .is_null()
                .or(Expr::col(Products::Price).gt(0)),
        ),
        add_check(
            &Inquiries::Table,
            "chk_inquiries_status",
            Expr::col(Inquiries::Status).is_in(INQUIRY_STATUSES),
        ),
        add_check(
            &Inquiries::Table,
            "chk_inquiries_phone",
            is_vn_phone(Inquiries::RequesterPhone),
        ),
        add_check(
            &Reviews::Table,
            "chk_reviews_rating",
            Expr::col(Reviews::Rating).between(1, 5),
        ),
        add_unique(
            &Reviews::Table,
            "uq_reviews_business_reviewer",
            &[&Reviews::BusinessId, &Reviews::ReviewerPhoneHash],
        ),
        add_check(
            &CallClicks::Table,
            "chk_call_clicks_channel",
            Expr::col(CallClicks::Channel).is_in(CALL_CHANNELS),
        ),
        add_check(
            &JobPosts::Table,
            "chk_job_posts_status",
            Expr::col(JobPosts::Status).is_in(JOB_STATUSES),
        ),
        add_check(
            &JobPosts::Table,
            "chk_job_posts_phone",
            is_vn_phone(JobPosts::RequesterPhone),
        ),
        add_unique(
            &CropPrices::Table,
            "uq_crop_prices_day",
            &[
                &CropPrices::CropId,
                &CropPrices::LocationId,
                &CropPrices::ObservedOn,
            ],
        ),
        add_check(
            &CropPrices::Table,
            "chk_crop_prices_range",
            Expr::col(CropPrices::PriceLow)
                .gt(0)
                .and(Expr::col(CropPrices::PriceHigh).gte(Expr::col(CropPrices::PriceLow))),
        ),
        add_check(
            &Alerts::Table,
            "chk_alerts_dates",
            Expr::col(Alerts::EndsOn).gte(Expr::col(Alerts::StartsOn)),
        ),
        add_check(
            &Partners::Table,
            "chk_partners_kind",
            Expr::col(Partners::Kind).is_in(PARTNER_KINDS),
        ),
        add_check(
            &Partners::Table,
            "chk_partners_phone",
            Expr::col(Partners::Phone)
                .is_null()
                .or(is_vn_phone(Partners::Phone)),
        ),
        add_check(
            &PartnerContracts::Table,
            "chk_partner_contracts_dates",
            Expr::col(PartnerContracts::EndsOn).gte(Expr::col(PartnerContracts::StartsOn)),
        ),
        add_check(
            &PartnerContracts::Table,
            "chk_partner_contracts_fee",
            Expr::col(PartnerContracts::MonthlyFee).gte(0),
        ),
    ]
}

/// `is_active = TRUE AND deleted_at IS NULL`: the live rows.
fn live<A: IntoIden + 'static, D: IntoIden + 'static>(is_active: A, deleted_at: D) -> Condition {
    Condition::all()
        .add(Expr::col(is_active).eq(true))
        .add(Expr::col(deleted_at).is_null())
}

fn index<T: IntoIden + 'static>(name: &str, table: T) -> IndexCreateStatement {
    Index::create().name(name).table(table).to_owned()
}

fn gist<T: IntoIden + 'static, C: IntoIden + 'static>(
    name: &str,
    table: T,
    column: C,
) -> IndexCreateStatement {
    index(name, table)
        .index_type(IndexType::Custom(IndexMethod::Gist.into_iden()))
        .col(column)
        .to_owned()
}

fn indexes() -> Vec<IndexCreateStatement> {
    vec![
        index("idx_users_role", Users::Table)
            .col(Users::RoleId)
            .to_owned(),
        index(
            "idx_password_reset_tokens_user_id",
            PasswordResetTokens::Table,
        )
        .col(PasswordResetTokens::UserId)
        .to_owned(),
        index("ix_refresh_tokens_family", RefreshTokens::Table)
            .col(RefreshTokens::FamilyId)
            .to_owned(),
        index("ix_refresh_tokens_user", RefreshTokens::Table)
            .col(RefreshTokens::UserId)
            .to_owned(),
        // The hourly purge deletes by expiry.
        index("ix_refresh_tokens_expires", RefreshTokens::Table)
            .col(RefreshTokens::ExpiresAt)
            .to_owned(),
        index("idx_role_permissions_permission", RolePermissions::Table)
            .col(RolePermissions::PermissionId)
            .to_owned(),
        index("ix_wards_province", Wards::Table)
            .col(Wards::ProvinceCode)
            .to_owned(),
        gist("ix_wards_boundary", Wards::Table, Wards::Boundary),
        index("ix_crops_commodity", Crops::Table)
            .col(Crops::CommodityId)
            .to_owned(),
        gist(
            "idx_businesses_location",
            Businesses::Table,
            Businesses::Location,
        )
        .cond_where(live(Businesses::IsActive, Businesses::DeletedAt))
        .to_owned(),
        index("idx_businesses_active", Businesses::Table)
            .col(Businesses::IsActive)
            .col(Businesses::IsVerified)
            .and_where(Expr::col(Businesses::DeletedAt).is_null())
            .to_owned(),
        index("idx_businesses_owner", Businesses::Table)
            .col(Businesses::OwnerId)
            .to_owned(),
        // One live business per managing account.
        index("uq_businesses_live_owner", Businesses::Table)
            .col(Businesses::OwnerId)
            .unique()
            .and_where(Expr::col(Businesses::OwnerId).is_not_null())
            .and_where(Expr::col(Businesses::DeletedAt).is_null())
            .to_owned(),
        index("idx_businesses_demo", Businesses::Table)
            .col(Businesses::Id)
            .and_where(Expr::col(Businesses::IsDemo).into())
            .to_owned(),
        index("idx_services_business", Services::Table)
            .col(Services::BusinessId)
            .to_owned(),
        index("idx_services_type", Services::Table)
            .col(Services::ServiceTypeId)
            .to_owned(),
        index("idx_services_live", Services::Table)
            .col(Services::BusinessId)
            .cond_where(live(Services::IsActive, Services::DeletedAt))
            .to_owned(),
        // Deleting a file looks up the listings that reference it.
        index("ix_services_image_file_id", Services::Table)
            .col(Services::ImageFileId)
            .to_owned(),
        index("idx_products_business", Products::Table)
            .col(Products::BusinessId)
            .to_owned(),
        index("idx_products_type", Products::Table)
            .col(Products::SupplyTypeId)
            .to_owned(),
        index("idx_products_commodity", Products::Table)
            .col(Products::CommodityId)
            .to_owned(),
        index("idx_products_live", Products::Table)
            .col(Products::BusinessId)
            .cond_where(live(Products::IsActive, Products::DeletedAt))
            .to_owned(),
        index("ix_products_image_file_id", Products::Table)
            .col(Products::ImageFileId)
            .to_owned(),
        index("idx_inquiries_business", Inquiries::Table)
            .col(Inquiries::BusinessId)
            .to_owned(),
        index("idx_inquiries_status", Inquiries::Table)
            .col(Inquiries::Status)
            .to_owned(),
        index("idx_inquiries_created_on", Inquiries::Table)
            .col(Inquiries::CreatedOn)
            .to_owned(),
        index("idx_reviews_business_visible", Reviews::Table)
            .col(Reviews::BusinessId)
            .and_where(Expr::col(Reviews::IsVisible).eq(true))
            .to_owned(),
        // Counts are per business over a recent window.
        index("ix_call_clicks_business_created", CallClicks::Table)
            .col(CallClicks::BusinessId)
            .col(CallClicks::CreatedOn)
            .to_owned(),
        index("ix_search_events_created_on", SearchEvents::Table)
            .col(SearchEvents::CreatedOn)
            .to_owned(),
        gist("ix_job_posts_location", JobPosts::Table, JobPosts::Location),
        index("ix_job_posts_created", JobPosts::Table)
            .col((JobPosts::CreatedOn, IndexOrder::Desc))
            .col((JobPosts::Id, IndexOrder::Desc))
            .to_owned(),
        index("ix_job_posts_phone", JobPosts::Table)
            .col(JobPosts::RequesterPhone)
            .col(JobPosts::CreatedOn)
            .to_owned(),
        index("ix_crop_prices_observed", CropPrices::Table)
            .col((CropPrices::ObservedOn, IndexOrder::Desc))
            .col((CropPrices::Id, IndexOrder::Desc))
            .to_owned(),
        index("ix_alerts_live", Alerts::Table)
            .col(Alerts::EndsOn)
            .col(Alerts::StartsOn)
            .to_owned(),
        index("ix_partner_contracts_partner", PartnerContracts::Table)
            .col(PartnerContracts::PartnerId)
            .to_owned(),
        index("ix_audit_logs_admin_id", AuditLogs::Table)
            .col(AuditLogs::AdminId)
            .to_owned(),
        index("ix_audit_logs_entity", AuditLogs::Table)
            .col(AuditLogs::EntityType)
            .col(AuditLogs::EntityId)
            .to_owned(),
        index("ix_audit_logs_created_on", AuditLogs::Table)
            .col((AuditLogs::CreatedOn, IndexOrder::Desc))
            .to_owned(),
    ]
}

fn tables() -> Vec<TableCreateStatement> {
    vec![
        roles(),
        users(),
        password_reset_tokens(),
        refresh_tokens(),
        permissions(),
        role_permissions(),
        catalogue_state(),
        audit_logs(),
        files(),
        provinces(),
        wards(),
        locations(),
        coded_list(Sectors::Table),
        coded_list(AlertKinds::Table),
        commodities(),
        offer_types(ServiceTypes::Table),
        offer_types(SupplyTypes::Table),
        crops(),
        businesses(),
        services(),
        products(),
        inquiries(),
        reviews(),
        call_clicks(),
        search_events(),
        job_posts(),
        crop_prices(),
        alerts(),
        partners(),
        partner_contracts(),
        partner_contract_service_types(),
        partner_contract_supply_types(),
        partner_sponsored(),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // In production the extensions already exist (deploy/db-init runs as
        // the superuser); IF NOT EXISTS then needs no privilege from the app
        // role.
        run(
            manager,
            &[
                create_extension(Ext::Postgis),
                create_extension(Ext::Unaccent),
                create_extension(Ext::PgTrgm),
                SET_UPDATED_ON.create_or_replace(),
                F_UNACCENT.create_or_replace(),
                OFFER_TYPE_DISTINCT.create_or_replace(),
                create_sequence(Sequences::CatalogueTypeIds),
            ],
        )
        .await?;
        for table in tables() {
            manager.create_table(table).await?;
        }
        run(manager, &checks()).await?;
        run(manager, &named()).await?;
        run(manager, &references_within()).await?;
        run(
            manager,
            &[
                create_distinct_trigger(&ServiceTypes::Table, &SupplyTypes::Table),
                create_distinct_trigger(&SupplyTypes::Table, &ServiceTypes::Table),
            ],
        )
        .await?;
        for index in indexes() {
            manager.create_index(index).await?;
        }
        let mut statements: Vec<String> = TRIGGERED
            .iter()
            .map(|table| create_updated_on_trigger(*table))
            .collect();
        statements.extend(search_indexes().iter().map(FoldIndex::create));
        run(manager, &statements).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // The baseline is the whole schema: there is nothing before it to go
        // back to. `migration fresh` drops every table instead.
        Err(DbErr::Migration(
            "the baseline cannot be rolled back; use `fresh`".to_owned(),
        ))
    }
}
