//! Baseline schema. DDL only: reference data is seeded at startup from the
//! catalogue in `application::catalogue`, demo data only on request.
//!
//! The value lists in the checks are frozen with this migration; the live
//! lists are in `domain::constants` (CATEGORY_TYPES, INQUIRY_STATUSES) and
//! `application::validation::is_valid_vn_phone`.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, add_unique, big_serial_id, by_user, create_extension, create_updated_on_trigger,
    flag, int, int_null, is_vn_phone, references, references_plain, run, serial_id, stamped,
    text_null, timestamp_null, varchar, varchar_null, AuditLogs, Businesses, CatalogueState,
    Categories, Ext, GeographyPoint, IndexMethod, Inquiries, Locations, PasswordResetTokens,
    Permissions, Products, Reviews, RolePermissions, Roles, Services, Users, SET_UPDATED_ON,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// The tables carrying an `updated_on` trigger, in creation order.
const TRIGGERED: &[&dyn Iden] = &[
    &Roles::Table,
    &Users::Table,
    &Permissions::Table,
    &Categories::Table,
    &Businesses::Table,
    &Services::Table,
    &Products::Table,
    &Inquiries::Table,
    &Reviews::Table,
];

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

/// One-time catalogue steps (e.g. the initial default-grant matrix). A row
/// here means "already done, never redo", which is what keeps an admin's
/// revocation from being undone at the next startup.
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

fn categories() -> TableCreateStatement {
    Table::create()
        .table(Categories::Table)
        .col(serial_id(Categories::Id))
        .col(varchar(Categories::Name, 100))
        .col(varchar(Categories::Slug, 100).unique_key())
        .col(varchar(Categories::Type, 20))
        .col(varchar_null(Categories::Icon, 100))
        .col(varchar_null(Categories::Color, 20))
        .col(varchar_null(Categories::Description, 300))
        .col(
            ColumnDef::new(Categories::DisplayOrder)
                .small_integer()
                .not_null()
                .default(0),
        )
        .col(flag(Categories::IsActive, true))
        .col(stamped(Categories::CreatedOn))
        .col(stamped(Categories::UpdatedOn))
        .col(int_null(Categories::CreatedBy))
        .col(int_null(Categories::UpdatedBy))
        .foreign_key(&mut by_user(Categories::Table, Categories::CreatedBy))
        .foreign_key(&mut by_user(Categories::Table, Categories::UpdatedBy))
        .to_owned()
}

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
        .col(int(Services::CategoryId))
        .col(varchar(Services::Name, 300))
        .col(text_null(Services::Description))
        .col(varchar_null(Services::PriceNote, 300))
        .col(flag(Services::IsActive, true))
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
            Services::CategoryId,
            Categories::Table,
            Categories::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut by_user(Services::Table, Services::CreatedBy))
        .foreign_key(&mut by_user(Services::Table, Services::UpdatedBy))
        .to_owned()
}

/// Images arrive with the media migration as a FK to `files`, never a URL.
fn products() -> TableCreateStatement {
    Table::create()
        .table(Products::Table)
        .col(serial_id(Products::Id))
        .col(int(Products::BusinessId))
        .col(int(Products::CategoryId))
        .col(varchar(Products::Name, 300))
        .col(text_null(Products::Description))
        .col(ColumnDef::new(Products::Price).big_integer())
        .col(varchar_null(Products::Unit, 50))
        .col(flag(Products::IsActive, true))
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
            Products::CategoryId,
            Categories::Table,
            Categories::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut by_user(Products::Table, Products::CreatedBy))
        .foreign_key(&mut by_user(Products::Table, Products::UpdatedBy))
        .to_owned()
}

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
        .foreign_key(&mut references(
            Inquiries::Table,
            Inquiries::ServiceId,
            Services::Table,
            Services::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            Inquiries::Table,
            Inquiries::ProductId,
            Products::Table,
            Products::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Inquiries::Table, Inquiries::UpdatedBy))
        .to_owned()
}

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
        .col(flag(Reviews::IsVisible, true))
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
        .foreign_key(&mut references(
            Reviews::Table,
            Reviews::InquiryId,
            Inquiries::Table,
            Inquiries::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Reviews::Table, Reviews::UpdatedBy))
        .to_owned()
}

/// Geo-search presets.
fn locations() -> TableCreateStatement {
    Table::create()
        .table(Locations::Table)
        .col(serial_id(Locations::Id))
        .col(varchar(Locations::Label, 150).unique_key())
        .col(ColumnDef::new(Locations::Lat).double().not_null())
        .col(ColumnDef::new(Locations::Lng).double().not_null())
        .col(int(Locations::DisplayOrder).default(0))
        .col(flag(Locations::IsActive, true))
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

fn checks() -> Vec<String> {
    vec![
        add_check(
            &Categories::Table,
            "chk_categories_type",
            Expr::col(Categories::Type).is_in(["service", "product"]),
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
            Expr::col(Inquiries::Status).is_in(["new", "contacted", "closed", "spam"]),
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
    ]
}

/// `is_active = TRUE AND deleted_at IS NULL` on `table`: the live rows.
fn live<A: IntoIden + 'static, D: IntoIden + 'static>(is_active: A, deleted_at: D) -> Condition {
    Condition::all()
        .add(Expr::col(is_active).eq(true))
        .add(Expr::col(deleted_at).is_null())
}

fn indexes() -> Vec<IndexCreateStatement> {
    vec![
        Index::create()
            .name("idx_users_role")
            .table(Users::Table)
            .col(Users::RoleId)
            .to_owned(),
        Index::create()
            .name("idx_password_reset_tokens_user_id")
            .table(PasswordResetTokens::Table)
            .col(PasswordResetTokens::UserId)
            .to_owned(),
        Index::create()
            .name("idx_role_permissions_permission")
            .table(RolePermissions::Table)
            .col(RolePermissions::PermissionId)
            .to_owned(),
        Index::create()
            .name("idx_categories_type")
            .table(Categories::Table)
            .col(Categories::Type)
            .to_owned(),
        Index::create()
            .name("idx_businesses_location")
            .table(Businesses::Table)
            .index_type(IndexType::Custom(IndexMethod::Gist.into_iden()))
            .col(Businesses::Location)
            .cond_where(live(Businesses::IsActive, Businesses::DeletedAt))
            .to_owned(),
        Index::create()
            .name("idx_businesses_active")
            .table(Businesses::Table)
            .col(Businesses::IsActive)
            .col(Businesses::IsVerified)
            .and_where(Expr::col(Businesses::DeletedAt).is_null())
            .to_owned(),
        Index::create()
            .name("idx_businesses_owner")
            .table(Businesses::Table)
            .col(Businesses::OwnerId)
            .to_owned(),
        Index::create()
            .name("idx_businesses_demo")
            .table(Businesses::Table)
            .col(Businesses::Id)
            .and_where(Expr::col(Businesses::IsDemo).into())
            .to_owned(),
        Index::create()
            .name("idx_services_business")
            .table(Services::Table)
            .col(Services::BusinessId)
            .to_owned(),
        Index::create()
            .name("idx_services_category")
            .table(Services::Table)
            .col(Services::CategoryId)
            .to_owned(),
        Index::create()
            .name("idx_services_live")
            .table(Services::Table)
            .col(Services::BusinessId)
            .cond_where(live(Services::IsActive, Services::DeletedAt))
            .to_owned(),
        Index::create()
            .name("idx_products_business")
            .table(Products::Table)
            .col(Products::BusinessId)
            .to_owned(),
        Index::create()
            .name("idx_products_category")
            .table(Products::Table)
            .col(Products::CategoryId)
            .to_owned(),
        Index::create()
            .name("idx_products_live")
            .table(Products::Table)
            .col(Products::BusinessId)
            .cond_where(live(Products::IsActive, Products::DeletedAt))
            .to_owned(),
        Index::create()
            .name("idx_inquiries_business")
            .table(Inquiries::Table)
            .col(Inquiries::BusinessId)
            .to_owned(),
        Index::create()
            .name("idx_inquiries_status")
            .table(Inquiries::Table)
            .col(Inquiries::Status)
            .to_owned(),
        Index::create()
            .name("idx_inquiries_created_on")
            .table(Inquiries::Table)
            .col(Inquiries::CreatedOn)
            .to_owned(),
        Index::create()
            .name("idx_reviews_business_visible")
            .table(Reviews::Table)
            .col(Reviews::BusinessId)
            .and_where(Expr::col(Reviews::IsVisible).eq(true))
            .to_owned(),
        Index::create()
            .name("ix_audit_logs_admin_id")
            .table(AuditLogs::Table)
            .col(AuditLogs::AdminId)
            .to_owned(),
        Index::create()
            .name("ix_audit_logs_entity")
            .table(AuditLogs::Table)
            .col(AuditLogs::EntityType)
            .col(AuditLogs::EntityId)
            .to_owned(),
        Index::create()
            .name("ix_audit_logs_created_on")
            .table(AuditLogs::Table)
            .col((AuditLogs::CreatedOn, IndexOrder::Desc))
            .to_owned(),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run(
            manager,
            &[
                create_extension(Ext::Postgis),
                SET_UPDATED_ON.create_or_replace(),
            ],
        )
        .await?;
        for table in [
            roles(),
            users(),
            password_reset_tokens(),
            permissions(),
            role_permissions(),
            catalogue_state(),
            categories(),
            businesses(),
            services(),
            products(),
            inquiries(),
            reviews(),
            locations(),
            audit_logs(),
        ] {
            manager.create_table(table).await?;
        }
        run(manager, &checks()).await?;
        for index in indexes() {
            manager.create_index(index).await?;
        }
        let triggers: Vec<String> = TRIGGERED
            .iter()
            .map(|table| create_updated_on_trigger(*table))
            .collect();
        run(manager, &triggers).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(AuditLogs::Table)
                    .table(Locations::Table)
                    .table(Reviews::Table)
                    .table(Inquiries::Table)
                    .table(Products::Table)
                    .table(Services::Table)
                    .table(Businesses::Table)
                    .table(Categories::Table)
                    .table(CatalogueState::Table)
                    .table(RolePermissions::Table)
                    .table(Permissions::Table)
                    .table(PasswordResetTokens::Table)
                    .table(Users::Table)
                    .table(Roles::Table)
                    .if_exists()
                    .cascade()
                    .to_owned(),
            )
            .await?;
        run(manager, &[SET_UPDATED_ON.drop_if_exists(true)]).await
    }
}
