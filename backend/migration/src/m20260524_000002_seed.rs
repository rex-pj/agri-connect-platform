use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        db.execute_unprepared(
            "INSERT INTO categories (name, slug, type, icon, display_order) VALUES
                ('Múc bùn',    'muc-bun',    'service', 'fa-solid fa-water',              1),
                ('Phun thuốc', 'phun-thuoc', 'service', 'fa-solid fa-droplet',            2),
                ('Sửa máy nổ', 'sua-may-no', 'service', 'fa-solid fa-gears',              3),
                ('Nông sản',   'nong-san',   'product', 'fa-solid fa-basket-shopping',    1),
                ('Thuốc BVTV', 'thuoc-bvtv', 'product', 'fa-solid fa-shield-halved',      2),
                ('Nông cụ',    'nong-cu',    'product', 'fa-solid fa-screwdriver-wrench', 3),
                ('Máy móc',    'may-moc',    'product', 'fa-solid fa-gears',              4),
                ('Phân bón',   'phan-bon',   'product', 'fa-solid fa-bag-shopping',       5)
             ON CONFLICT (slug) DO NOTHING",
        )
        .await?;

        // Mekong Delta + TP.HCM — reflects 2025 Vietnam administrative reorganisation
        // (63 → 34 provinces). Key mergers: Long An ← Tiền Giang, Bến Tre ← Trà Vinh,
        // Đồng Tháp ← Vĩnh Long, An Giang ← Kiên Giang, Cần Thơ ← Hậu Giang,
        // Sóc Trăng ← Bạc Liêu. Phú Quốc excluded (tourist island, not agricultural focus).
        db.execute_unprepared(
            "INSERT INTO locations (label, lat, lng, display_order) VALUES
                ('TP. Hồ Chí Minh',       10.7769, 106.7009,  1),
                ('Cần Thơ',               10.0452, 105.7469,  2),
                ('Cần Thơ — Vị Thanh',     9.7856, 105.4706,  3),
                ('Long An — Tân An',       10.5354, 106.4120,  4),
                ('Long An — Đức Hòa',      10.8608, 106.2873,  5),
                ('Long An — Cai Lậy',      10.4167, 105.9500,  6),
                ('Long An — Mỹ Tho',       10.3600, 106.3600,  7),
                ('Long An — Cái Bè',       10.3667, 105.8833,  8),
                ('Long An — Gò Công',      10.3667, 106.6667,  9),
                ('Bến Tre',               10.2433, 106.3752, 10),
                ('Bến Tre — Mỏ Cày',       10.1364, 106.3074, 11),
                ('Bến Tre — Giồng Trôm',   10.1775, 106.4431, 12),
                ('Bến Tre — Trà Vinh',      9.9477, 106.3422, 13),
                ('Đồng Tháp — Cao Lãnh',   10.4592, 105.6330, 14),
                ('Đồng Tháp — Sa Đéc',     10.2938, 105.7583, 15),
                ('Đồng Tháp — Hồng Ngự',   10.8167, 105.3167, 16),
                ('Đồng Tháp — Vĩnh Long',  10.2500, 105.9667, 17),
                ('An Giang — Long Xuyên',  10.3797, 105.4358, 18),
                ('An Giang — Châu Đốc',    10.7044, 105.1177, 19),
                ('An Giang — Tân Châu',    10.8026, 105.2337, 20),
                ('An Giang — Chợ Mới',     10.4516, 105.4477, 21),
                ('An Giang — Rạch Giá',    10.0125, 105.0809, 22),
                ('An Giang — Hà Tiên',     10.3833, 104.4833, 23),
                ('Sóc Trăng',              9.6031, 105.9739, 24),
                ('Sóc Trăng — Bạc Liêu',   9.2827, 105.7244, 25),
                ('Cà Mau',                 9.1769, 105.1524, 26)
             ON CONFLICT (label) DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO businesses (name, phone, description, address, location, is_verified, is_active) VALUES
                ('Dịch vụ Nguyễn Văn An', '0901234567',
                 'Chuyên múc bùn và sửa máy nổ vùng Cái Lậy, kinh nghiệm 10 năm.',
                 'Xã Mỹ Long, Cái Lậy, Tiền Giang',
                 ST_GeographyFromText('SRID=4326;POINT(106.0123 10.3456)'), TRUE, TRUE),

                ('Dịch vụ Trần Thị Bình', '0912345678',
                 'Phun thuốc bảo vệ thực vật bằng drone, phủ đều toàn ruộng.',
                 'Xã Tân Phong, Cái Lậy, Tiền Giang',
                 ST_GeographyFromText('SRID=4326;POINT(106.0200 10.3500)'), TRUE, TRUE),

                ('Cơ sở Lê Văn Cường', '0923456789',
                 'Tổng hợp dịch vụ nông nghiệp: múc bùn, phun thuốc, sửa máy.',
                 'Xã Long Trung, Cái Lậy, Tiền Giang',
                 ST_GeographyFromText('SRID=4326;POINT(106.0300 10.3400)'), FALSE, TRUE),

                ('Cơ sở Năm Hùng', '0901111001',
                 'Chuyên kinh doanh gạo ST25 và nông sản đặc sản Mekong.',
                 '12 Lê Lợi, Cái Lậy, Tiền Giang',
                 NULL, TRUE, TRUE),

                ('Vườn Tư Phát', '0901111002',
                 'Vườn sầu riêng Ri6 chuẩn xuất khẩu, bán buôn và lẻ.',
                 'Xã Tam Bình, Cai Lậy, Tiền Giang',
                 NULL, TRUE, TRUE),

                ('Đại lý Phân bón Thành', '0904444001',
                 'Đại lý cấp 1 phân bón Bình Điền, NPK, Urea và phân hữu cơ.',
                 '88 Hùng Vương, Cái Lậy, Tiền Giang',
                 NULL, FALSE, TRUE)
             ON CONFLICT DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO services (business_id, category_id, name, description, price_note)
             SELECT b.id, c.id, s.name, s.description, s.price_note
             FROM (VALUES
                ('Dịch vụ Nguyễn Văn An', 'muc-bun',
                 'Múc bùn kênh mương',
                 'Nạo vét kênh mương, ao hồ, ruộng lúa bằng máy múc mini.',
                 '800.000 ₫/ca (8 giờ), có thể thương lượng'),

                ('Dịch vụ Nguyễn Văn An', 'sua-may-no',
                 'Sửa máy nổ, máy bơm',
                 'Sửa chữa, bảo dưỡng máy bơm nước, máy gặt, động cơ nổ.',
                 '100.000–300.000 ₫ tuỳ lỗi'),

                ('Dịch vụ Trần Thị Bình', 'phun-thuoc',
                 'Phun thuốc bằng drone',
                 'Drone DJI Agras T40, tải 40 L, phủ 1 ha/15 phút.',
                 '80.000 ₫/công (1.000 m²)'),

                ('Cơ sở Lê Văn Cường', 'muc-bun',
                 'Múc bùn ao hồ',
                 'Múc bùn cải tạo ao nuôi tôm, cá; ruộng lúa vùng trũng.',
                 'Liên hệ báo giá'),

                ('Cơ sở Lê Văn Cường', 'phun-thuoc',
                 'Phun thuốc thủ công',
                 'Phun thuốc bảo vệ thực vật bằng bình đeo vai.',
                 '50.000 ₫/công'),

                ('Cơ sở Lê Văn Cường', 'sua-may-no',
                 'Sửa máy nông nghiệp',
                 'Bảo dưỡng định kỳ và sửa chữa các loại máy nông nghiệp.',
                 '150.000–500.000 ₫')
             ) AS s(business_name, category_slug, name, description, price_note)
             JOIN businesses b ON b.name = s.business_name
             JOIN categories c ON c.slug = s.category_slug
             ON CONFLICT DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO products (business_id, category_id, name, description, price, unit, image_url)
             SELECT b.id, c.id, p.name, p.description, p.price::BIGINT, p.unit, p.image_url
             FROM (VALUES
                ('Cơ sở Năm Hùng', 'nong-san',
                 'Gạo ST25',
                 'Gạo ngon nhất thế giới, sản xuất tại Sóc Trăng. Hạt dài, thơm tự nhiên, cơm dẻo.',
                 '28000', 'kg',
                 'https://images.unsplash.com/photo-1601493700631-2b16ec4b4716?w=400&q=80'),

                ('Vườn Tư Phát', 'nong-san',
                 'Sầu riêng Ri6',
                 'Sầu riêng Cái Lậy chín cây, múi vàng, hạt lép, ngọt béo, thu hoạch đúng mùa.',
                 '85000', 'kg',
                 'https://images.unsplash.com/photo-1601493700631-2b16ec4b4716?w=400&q=80'),

                ('Cơ sở Năm Hùng', 'nong-san',
                 'Xoài cát Hoà Lộc',
                 'Xoài đặc sản Tiền Giang, trái lớn, thịt vàng, ít xơ, vị ngọt thanh.',
                 '45000', 'kg',
                 'https://images.unsplash.com/photo-1601493700631-2b16ec4b4716?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'thuoc-bvtv',
                 'Thuốc trừ sâu Actara 25WG',
                 'Thiamethoxam 25%, đặc trị rầy nâu và bọ trĩ trên lúa. Gói 100g.',
                 '85000', 'gói',
                 'https://images.unsplash.com/photo-1530836369250-ef72a3f5cda8?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'thuoc-bvtv',
                 'Thuốc diệt cỏ Sofit 300EC',
                 'Pretilachlor 300g/L, kiểm soát cỏ dại ruộng lúa gieo sạ. Chai 1L.',
                 '120000', 'chai',
                 'https://images.unsplash.com/photo-1530836369250-ef72a3f5cda8?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân NPK 16-16-8 Bình Điền',
                 'Phân hỗn hợp cân đối dinh dưỡng cho lúa, bắp, rau màu. Bao 50 kg.',
                 '420000', 'bao',
                 'https://images.unsplash.com/photo-1416879595882-3373a0480b5b?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân Urea 46%',
                 'Phân đạm nguyên chất 46% N, dạng hạt trắng, tan nhanh, bao 50 kg.',
                 '380000', 'bao',
                 'https://images.unsplash.com/photo-1416879595882-3373a0480b5b?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân hữu cơ vi sinh',
                 'Phân ủ từ bã mía + vi khuẩn cố định đạm, cải tạo đất lâu dài. Bao 25 kg.',
                 '180000', 'bao',
                 'https://images.unsplash.com/photo-1416879595882-3373a0480b5b?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'nong-cu',
                 'Bình phun thuốc 16L',
                 'Bình xịt tay có van điều áp, vòi phun xoay 360°, chịu hoá chất tốt.',
                 '320000', 'cái',
                 'https://images.unsplash.com/photo-1592924357228-91a4daadcfea?w=400&q=80'),

                ('Đại lý Phân bón Thành', 'nong-cu',
                 'Liềm cắt lúa thép',
                 'Lưỡi thép carbon, cán gỗ chắc, răng cưa sắc bén cho gặt thủ công.',
                 '55000', 'cái',
                 'https://images.unsplash.com/photo-1592924357228-91a4daadcfea?w=400&q=80')
             ) AS p(business_name, category_slug, name, description, price, unit, image_url)
             JOIN businesses b ON b.name = p.business_name
             JOIN categories c ON c.slug = p.category_slug
             ON CONFLICT DO NOTHING",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        db.execute_unprepared("DELETE FROM products WHERE business_id IN (SELECT id FROM businesses WHERE name IN ('Cơ sở Năm Hùng','Vườn Tư Phát','Đại lý Phân bón Thành'))").await?;
        db.execute_unprepared("DELETE FROM services WHERE business_id IN (SELECT id FROM businesses WHERE name IN ('Dịch vụ Nguyễn Văn An','Dịch vụ Trần Thị Bình','Cơ sở Lê Văn Cường'))").await?;
        db.execute_unprepared("DELETE FROM businesses WHERE name IN ('Dịch vụ Nguyễn Văn An','Dịch vụ Trần Thị Bình','Cơ sở Lê Văn Cường','Cơ sở Năm Hùng','Vườn Tư Phát','Đại lý Phân bón Thành')").await?;
        db.execute_unprepared("DELETE FROM locations").await?;
        db.execute_unprepared("DELETE FROM categories").await?;

        Ok(())
    }
}
