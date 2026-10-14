INSERT INTO products (business_id, category_id, name, description, price, unit)
             SELECT b.id, c.id, p.name, p.description, p.price::BIGINT, p.unit
             FROM (VALUES
                ('Cơ sở Năm Hùng', 'nong-san',
                 'Gạo ST25',
                 'Gạo ngon nhất thế giới, sản xuất tại Sóc Trăng. Hạt dài, thơm tự nhiên, cơm dẻo.',
                 '28000', 'kg'),

                ('Vườn Tư Phát', 'nong-san',
                 'Sầu riêng Ri6',
                 'Sầu riêng Cái Lậy chín cây, múi vàng, hạt lép, ngọt béo, thu hoạch đúng mùa.',
                 '85000', 'kg'),

                ('Cơ sở Năm Hùng', 'nong-san',
                 'Xoài cát Hoà Lộc',
                 'Xoài đặc sản Tiền Giang, trái lớn, thịt vàng, ít xơ, vị ngọt thanh.',
                 '45000', 'kg'),

                ('Đại lý Phân bón Thành', 'thuoc-bvtv',
                 'Thuốc trừ sâu Actara 25WG',
                 'Thiamethoxam 25%, đặc trị rầy nâu và bọ trĩ trên lúa. Gói 100g.',
                 '85000', 'gói'),

                ('Đại lý Phân bón Thành', 'thuoc-bvtv',
                 'Thuốc diệt cỏ Sofit 300EC',
                 'Pretilachlor 300g/L, kiểm soát cỏ dại ruộng lúa gieo sạ. Chai 1L.',
                 '120000', 'chai'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân NPK 16-16-8 Bình Điền',
                 'Phân hỗn hợp cân đối dinh dưỡng cho lúa, bắp, rau màu. Bao 50 kg.',
                 '420000', 'bao'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân Urea 46%',
                 'Phân đạm nguyên chất 46% N, dạng hạt trắng, tan nhanh, bao 50 kg.',
                 '380000', 'bao'),

                ('Đại lý Phân bón Thành', 'phan-bon',
                 'Phân hữu cơ vi sinh',
                 'Phân ủ từ bã mía + vi khuẩn cố định đạm, cải tạo đất lâu dài. Bao 25 kg.',
                 '180000', 'bao'),

                ('Đại lý Phân bón Thành', 'nong-cu',
                 'Bình phun thuốc 16L',
                 'Bình xịt tay có van điều áp, vòi phun xoay 360°, chịu hoá chất tốt.',
                 '320000', 'cái'),

                ('Đại lý Phân bón Thành', 'nong-cu',
                 'Liềm cắt lúa thép',
                 'Lưỡi thép carbon, cán gỗ chắc, răng cưa sắc bén cho gặt thủ công.',
                 '55000', 'cái')
             ) AS p(business_name, category_slug, name, description, price, unit)
             JOIN businesses b ON b.name = p.business_name AND b.is_demo
             JOIN categories c ON c.slug = p.category_slug
