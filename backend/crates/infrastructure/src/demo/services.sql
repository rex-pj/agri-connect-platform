INSERT INTO services (business_id, category_id, name, description, price_note)
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
             JOIN businesses b ON b.name = s.business_name AND b.is_demo
             JOIN categories c ON c.slug = s.category_slug
