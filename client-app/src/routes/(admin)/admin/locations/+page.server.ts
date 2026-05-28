import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminLocation } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const locations = await svc.listLocations();
        return { locations, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { locations: [], error: err.message };
        }
        return { locations: [], error: 'Không thể tải danh sách khu vực.' };
    }
};

export const actions: Actions = {
    create: async ({ request, fetch, cookies }) => {
        const data          = await request.formData();
        const label         = (data.get('label')         as string ?? '').trim();
        const lat           = parseFloat(data.get('lat')           as string ?? '');
        const lng           = parseFloat(data.get('lng')           as string ?? '');
        const display_order = parseInt(data.get('display_order')   as string ?? '0', 10);

        if (!label)             return fail(400, { error: 'Tên khu vực không được để trống.' });
        if (isNaN(lat) || isNaN(lng)) return fail(400, { error: 'Toạ độ không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.createLocation({ label, lat, lng, display_order });
            return { success: true, message: `Đã thêm khu vực "${label}".` };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Thêm khu vực thất bại.' });
        }
    },

    update: async ({ request, fetch, cookies }) => {
        const data          = await request.formData();
        const id            = Number(data.get('id'));
        const label         = (data.get('label') as string ?? '').trim();
        const lat           = parseFloat(data.get('lat') as string ?? '');
        const lng           = parseFloat(data.get('lng') as string ?? '');
        const display_order = parseInt(data.get('display_order') as string ?? '0', 10);

        if (!id || !label)            return fail(400, { error: 'Thông tin không hợp lệ.' });
        if (isNaN(lat) || isNaN(lng)) return fail(400, { error: 'Toạ độ không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateLocation(id, { label, lat, lng, display_order });
            return { success: true, message: 'Đã cập nhật khu vực.' };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật khu vực thất bại.' });
        }
    },

    toggle_active: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const id        = Number(data.get('id'));
        const is_active = data.get('is_active') === 'true';
        const reason    = (data.get('reason') as string ?? '').trim() || null;
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleLocationActive(id, is_active, reason);
            return { success: true, message: is_active ? 'Đã bật khu vực.' : 'Đã tắt khu vực.' };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },
};
