use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        db.execute_unprepared(
            "INSERT INTO roles (code, name) VALUES
                ('admin',    'Quản trị viên'),
                ('staff',    'Nhân viên'),
                ('provider', 'Nhà cung cấp')
             ON CONFLICT (code) DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO categories (name, slug, type, icon, display_order) VALUES
                ('Múc bùn',    'muc-bun',    'service', 'bi-water',               1),
                ('Phun thuốc', 'phun-thuoc', 'service', 'bi-droplet-fill',        2),
                ('Sửa máy nổ', 'sua-may-no', 'service', 'bi-gear-fill',           3),
                ('Nông sản',   'nong-san',   'product', 'bi-basket2-fill',        1),
                ('Thuốc BVTV', 'thuoc-bvtv', 'product', 'bi-shield-fill',         2),
                ('Nông cụ',    'nong-cu',    'product', 'bi-tools',               3),
                ('Máy móc',    'may-moc',    'product', 'bi-gear-wide-connected', 4),
                ('Phân bón',   'phan-bon',   'product', 'bi-bag-fill',            5)
             ON CONFLICT (slug) DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO agri_permissions (code, name) VALUES
                ('AgriBusinessRead',      'Xem danh sách doanh nghiệp'),
                ('AgriBusinessCreate',    'Thêm doanh nghiệp'),
                ('AgriBusinessUpdate',    'Cập nhật doanh nghiệp'),
                ('AgriBusinessDelete',    'Xoá doanh nghiệp'),
                ('AgriBusinessVerify',    'Xác minh doanh nghiệp'),
                ('AgriListingManage',     'Quản lý danh sách dịch vụ/sản phẩm'),
                ('AgriInquiryView',       'Xem yêu cầu liên hệ'),
                ('AgriInquiryManage',     'Cập nhật trạng thái yêu cầu'),
                ('AgriReviewModerate',    'Duyệt đánh giá'),
                ('ProviderProfileUpdate', 'Cập nhật hồ sơ doanh nghiệp (chủ sở hữu)'),
                ('ProviderListingCreate', 'Thêm dịch vụ / sản phẩm (chủ sở hữu)'),
                ('ProviderListingUpdate', 'Chỉnh sửa dịch vụ / sản phẩm (chủ sở hữu)'),
                ('ProviderListingDelete', 'Xoá dịch vụ / sản phẩm (chủ sở hữu)')
             ON CONFLICT (code) DO NOTHING",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        db.execute_unprepared("DELETE FROM agri_permissions").await?;
        db.execute_unprepared("DELETE FROM categories").await?;
        db.execute_unprepared("DELETE FROM roles").await?;

        Ok(())
    }
}
