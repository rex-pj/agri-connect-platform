use sea_orm_migration::prelude::*;

use crate::enums::{
    agri_permission::AgriPermission, business::Business, category::Category, inquiry::Inquiry,
    product::Product, review::Review, role::Role, service::Service, user::User,
    user_permission::UserPermission,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS postgis")
            .await?;

        db.execute_unprepared(
            r#"CREATE OR REPLACE FUNCTION fn_set_updated_on()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_on = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql"#,
        )
        .await?;

        // ── 1. roles ──────────────────────────────────────────────────────────
        // Created before users because users.role_id FKs here.
        // No created_by/updated_by to avoid circular FK with users.
        manager
            .create_table(
                Table::create()
                    .table(Role::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Role::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Role::Code).string_len(50).not_null().unique_key())
                    .col(ColumnDef::new(Role::Name).string_len(100).not_null())
                    .col(ColumnDef::new(Role::Description).text().null())
                    .col(ColumnDef::new(Role::IsActive).boolean().not_null().default(true))
                    .col(
                        ColumnDef::new(Role::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Role::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_roles_updated_on \
             BEFORE UPDATE ON roles \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 2. users ──────────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(User::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(User::Username).string_len(100).not_null().unique_key())
                    .col(ColumnDef::new(User::Email).string_len(255).not_null().unique_key())
                    .col(ColumnDef::new(User::PasswordHash).string_len(255).not_null())
                    .col(ColumnDef::new(User::FullName).string_len(200).null())
                    .col(ColumnDef::new(User::RoleId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_users_role_id")
                            .from(User::Table, User::RoleId)
                            .to(Role::Table, Role::Id),
                    )
                    .col(ColumnDef::new(User::IsActive).boolean().not_null().default(true))
                    .col(ColumnDef::new(User::LastLoginOn).timestamp_with_time_zone().null())
                    .col(
                        ColumnDef::new(User::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(User::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(User::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_users_created_by")
                            .from(User::Table, User::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(User::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_users_updated_by")
                            .from(User::Table, User::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(User::DeletedAt).timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_users_updated_on \
             BEFORE UPDATE ON users \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 3. categories ─────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(Category::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Category::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Category::Name).string_len(100).not_null())
                    .col(ColumnDef::new(Category::Slug).string_len(100).not_null().unique_key())
                    .col(ColumnDef::new(Category::Type).string_len(20).not_null())
                    .col(ColumnDef::new(Category::Icon).string_len(100).null())
                    .col(
                        ColumnDef::new(Category::DisplayOrder)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Category::IsActive).boolean().not_null().default(true))
                    .col(
                        ColumnDef::new(Category::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Category::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Category::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_categories_created_by")
                            .from(Category::Table, Category::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Category::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_categories_updated_by")
                            .from(Category::Table, Category::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_categories_type")
                    .table(Category::Table)
                    .col(Category::Type)
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_categories_updated_on \
             BEFORE UPDATE ON categories \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 3. businesses ─────────────────────────────────────────────────────
        // Uses GEOGRAPHY(POINT, 4326) which requires raw DDL; other columns via DSL.
        manager
            .create_table(
                Table::create()
                    .table(Business::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Business::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Business::Name).string_len(200).not_null())
                    .col(ColumnDef::new(Business::Phone).string_len(20).not_null())
                    .col(ColumnDef::new(Business::Email).string_len(255).null())
                    .col(ColumnDef::new(Business::Description).text().null())
                    .col(ColumnDef::new(Business::Address).string_len(500).null())
                    // Geography column: longitude first in WKT
                    .col(
                        ColumnDef::new(Business::Location)
                            .custom(Alias::new("GEOGRAPHY(POINT, 4326)"))
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Business::IsVerified)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Business::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    // owner_id: the user account that operates this business
                    .col(ColumnDef::new(Business::OwnerId).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_businesses_owner_id")
                            .from(Business::Table, Business::OwnerId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(
                        ColumnDef::new(Business::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Business::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // created_by: the admin/staff who created the record
                    .col(ColumnDef::new(Business::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_businesses_created_by")
                            .from(Business::Table, Business::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Business::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_businesses_updated_by")
                            .from(Business::Table, Business::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    // deleted_at = permanent soft-delete (admin); is_active = temporary hide (owner)
                    .col(ColumnDef::new(Business::DeletedAt).timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "ALTER TABLE businesses ADD CONSTRAINT chk_businesses_phone \
             CHECK (phone ~ '^0[0-9]{9}$')",
        )
        .await?;

        // Partial GIST index — only active non-deleted rows; ST_DWithin uses this
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_businesses_location \
             ON businesses USING GIST (location) \
             WHERE is_active = true AND deleted_at IS NULL",
        )
        .await?;

        // Partial composite index — listing queries filter active+verified over live rows
        db.execute_unprepared(
            "CREATE INDEX idx_businesses_active \
             ON businesses (is_active, is_verified) \
             WHERE deleted_at IS NULL",
        )
        .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_businesses_owner")
                    .table(Business::Table)
                    .col(Business::OwnerId)
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_businesses_updated_on \
             BEFORE UPDATE ON businesses \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 4. services ───────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(Service::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Service::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Service::BusinessId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_services_business_id")
                            .from(Service::Table, Service::BusinessId)
                            .to(Business::Table, Business::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(Service::CategoryId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_services_category_id")
                            .from(Service::Table, Service::CategoryId)
                            .to(Category::Table, Category::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .col(ColumnDef::new(Service::Name).string_len(300).not_null())
                    .col(ColumnDef::new(Service::Description).text().null())
                    .col(ColumnDef::new(Service::PriceNote).string_len(300).null())
                    .col(
                        ColumnDef::new(Service::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Service::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Service::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Service::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_services_created_by")
                            .from(Service::Table, Service::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Service::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_services_updated_by")
                            .from(Service::Table, Service::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Service::DeletedAt).timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_services_business")
                    .table(Service::Table)
                    .col(Service::BusinessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_services_category")
                    .table(Service::Table)
                    .col(Service::CategoryId)
                    .to_owned(),
            )
            .await?;

        // Partial index — live rows only; used by the business profile page query
        db.execute_unprepared(
            "CREATE INDEX idx_services_live \
             ON services (business_id) \
             WHERE is_active = true AND deleted_at IS NULL",
        )
        .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_services_updated_on \
             BEFORE UPDATE ON services \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 5. products ───────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(Product::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Product::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Product::BusinessId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_products_business_id")
                            .from(Product::Table, Product::BusinessId)
                            .to(Business::Table, Business::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(Product::CategoryId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_products_category_id")
                            .from(Product::Table, Product::CategoryId)
                            .to(Category::Table, Category::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .col(ColumnDef::new(Product::Name).string_len(300).not_null())
                    .col(ColumnDef::new(Product::Description).text().null())
                    // Price stored as integer VND (whole đồng, no decimals)
                    .col(ColumnDef::new(Product::Price).big_integer().null())
                    .col(ColumnDef::new(Product::Unit).string_len(50).null())
                    .col(ColumnDef::new(Product::ImageUrl).string_len(500).null())
                    .col(
                        ColumnDef::new(Product::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Product::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Product::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Product::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_products_created_by")
                            .from(Product::Table, Product::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Product::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_products_updated_by")
                            .from(Product::Table, Product::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(
                        ColumnDef::new(Product::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_business")
                    .table(Product::Table)
                    .col(Product::BusinessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_category")
                    .table(Product::Table)
                    .col(Product::CategoryId)
                    .to_owned(),
            )
            .await?;

        // Partial index — live rows only; used by the business profile page query
        db.execute_unprepared(
            "CREATE INDEX idx_products_live \
             ON products (business_id) \
             WHERE is_active = true AND deleted_at IS NULL",
        )
        .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_products_updated_on \
             BEFORE UPDATE ON products \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 6. inquiries ──────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(Inquiry::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Inquiry::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Inquiry::BusinessId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inquiries_business_id")
                            .from(Inquiry::Table, Inquiry::BusinessId)
                            .to(Business::Table, Business::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(Inquiry::ServiceId).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inquiries_service_id")
                            .from(Inquiry::Table, Inquiry::ServiceId)
                            .to(Service::Table, Service::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Inquiry::ProductId).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inquiries_product_id")
                            .from(Inquiry::Table, Inquiry::ProductId)
                            .to(Product::Table, Product::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Inquiry::Topic).string_len(300).not_null())
                    .col(ColumnDef::new(Inquiry::RequesterName).string_len(200).not_null())
                    .col(ColumnDef::new(Inquiry::RequesterPhone).string_len(20).not_null())
                    .col(ColumnDef::new(Inquiry::Note).text().null())
                    .col(
                        ColumnDef::new(Inquiry::Status)
                            .string_len(20)
                            .not_null()
                            .default("new"),
                    )
                    .col(
                        ColumnDef::new(Inquiry::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Inquiry::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Inquiry::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inquiries_updated_by")
                            .from(Inquiry::Table, Inquiry::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "ALTER TABLE inquiries ADD CONSTRAINT chk_inquiries_status \
             CHECK (status IN ('new', 'contacted', 'closed', 'spam'))",
        )
        .await?;

        db.execute_unprepared(
            "ALTER TABLE inquiries ADD CONSTRAINT chk_inquiries_phone \
             CHECK (requester_phone ~ '^0[0-9]{9}$')",
        )
        .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inquiries_business")
                    .table(Inquiry::Table)
                    .col(Inquiry::BusinessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inquiries_status")
                    .table(Inquiry::Table)
                    .col(Inquiry::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inquiries_created_on")
                    .table(Inquiry::Table)
                    .col(Inquiry::CreatedOn)
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_inquiries_updated_on \
             BEFORE UPDATE ON inquiries \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 7. reviews ────────────────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(Review::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Review::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Review::BusinessId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reviews_business_id")
                            .from(Review::Table, Review::BusinessId)
                            .to(Business::Table, Business::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(Review::InquiryId).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reviews_inquiry_id")
                            .from(Review::Table, Review::InquiryId)
                            .to(Inquiry::Table, Inquiry::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(Review::Rating).small_integer().not_null())
                    .col(ColumnDef::new(Review::Comment).text().null())
                    .col(ColumnDef::new(Review::ReviewerName).string_len(200).not_null())
                    // Salted hash of reviewer phone — never store raw phone in reviews
                    .col(ColumnDef::new(Review::ReviewerPhoneHash).string_len(64).not_null())
                    .col(
                        ColumnDef::new(Review::IsVisible)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Review::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Review::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Review::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reviews_updated_by")
                            .from(Review::Table, Review::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "ALTER TABLE reviews ADD CONSTRAINT chk_reviews_rating \
             CHECK (rating BETWEEN 1 AND 5)",
        )
        .await?;

        // One review per reviewer per business (prevents spam; uses salted phone hash)
        db.execute_unprepared(
            "ALTER TABLE reviews ADD CONSTRAINT uq_reviews_business_reviewer \
             UNIQUE (business_id, reviewer_phone_hash)",
        )
        .await?;

        // Partial index — visible reviews only, which is what all public queries filter on
        db.execute_unprepared(
            "CREATE INDEX idx_reviews_business_visible \
             ON reviews (business_id) \
             WHERE is_visible = true",
        )
        .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_reviews_updated_on \
             BEFORE UPDATE ON reviews \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 8. agri_permissions ───────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(AgriPermission::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgriPermission::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AgriPermission::Code)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(AgriPermission::Name).string_len(200).not_null())
                    .col(
                        ColumnDef::new(AgriPermission::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(AgriPermission::UpdatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(AgriPermission::CreatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_agri_permissions_created_by")
                            .from(AgriPermission::Table, AgriPermission::CreatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(ColumnDef::new(AgriPermission::UpdatedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_agri_permissions_updated_by")
                            .from(AgriPermission::Table, AgriPermission::UpdatedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE TRIGGER trg_agri_permissions_updated_on \
             BEFORE UPDATE ON agri_permissions \
             FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on()",
        )
        .await?;

        // ── 9. user_permissions ───────────────────────────────────────────────
        manager
            .create_table(
                Table::create()
                    .table(UserPermission::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserPermission::UserId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_permissions_user_id")
                            .from(UserPermission::Table, UserPermission::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(UserPermission::PermissionId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_permissions_permission_id")
                            .from(UserPermission::Table, UserPermission::PermissionId)
                            .to(AgriPermission::Table, AgriPermission::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(UserPermission::GrantedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(UserPermission::GrantedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_permissions_granted_by")
                            .from(UserPermission::Table, UserPermission::GrantedBy)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserPermission::UserId)
                            .col(UserPermission::PermissionId),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;

        manager
            .drop_table(Table::drop().table(UserPermission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AgriPermission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Review::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Inquiry::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Product::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Service::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Business::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Category::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Role::Table).to_owned())
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION IF EXISTS fn_set_updated_on CASCADE")
            .await?;

        Ok(())
    }
}
