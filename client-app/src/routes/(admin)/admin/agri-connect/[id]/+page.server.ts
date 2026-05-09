import { error } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminProviderDetail, AdminServiceItem, AdminProductItem } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'ID không hợp lệ.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const provider = await svc.getProvider(id);
        return { provider };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Không tìm thấy nhà cung cấp.');
        }
        throw error(500, 'Không thể tải thông tin nhà cung cấp.');
    }
};

export const actions: Actions = {
    toggle_verify: async ({ params, fetch, cookies }) => {
        const id  = Number(params.id);
        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            const result = await svc.toggleProviderVerification(id);
            return { success: true, message: result.message };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return { success: false, error: err.message };
            }
            return { success: false, error: 'Cập nhật thất bại.' };
        }
    },
};
