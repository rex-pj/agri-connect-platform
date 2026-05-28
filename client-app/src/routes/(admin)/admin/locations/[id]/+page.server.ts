import { error, fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';
import { ROUTES } from '$lib/routes';

export type { AdminLocation } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'ID không hợp lệ.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const location = await svc.getLocation(id);
        return { location };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Không tìm thấy khu vực.');
        }
        throw error(500, 'Không thể tải thông tin khu vực.');
    }
};

export const actions: Actions = {
    update: async ({ params, request, fetch, cookies }) => {
        const id            = Number(params.id);
        const data          = await request.formData();
        const label         = (data.get('label') as string ?? '').trim();
        const lat           = parseFloat(data.get('lat') as string ?? '');
        const lng           = parseFloat(data.get('lng') as string ?? '');
        const display_order = parseInt(data.get('display_order') as string ?? '0', 10);

        if (!label)                   return fail(400, { error: 'Tên khu vực không được để trống.' });
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
            return fail(500, { error: 'Cập nhật thất bại.' });
        }
    },

    toggle_active: async ({ params, request, fetch, cookies }) => {
        const id        = Number(params.id);
        const data      = await request.formData();
        const is_active = data.get('is_active') === 'true';
        const reason    = (data.get('reason') as string ?? '').trim() || null;

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleLocationActive(id, is_active, reason);
            return {
                success: true,
                message: is_active ? 'Khu vực đã được hiện.' : 'Khu vực đã được ẩn.',
            };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },

    delete: async ({ params, request, fetch, cookies }) => {
        const id     = Number(params.id);
        const data   = await request.formData();
        const reason = (data.get('reason') as string ?? '').trim() || null;

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.deleteLocation(id, reason);
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Xoá khu vực thất bại.' });
        }
        redirect(303, ROUTES.ADMIN.LOCATIONS);
    },
};
