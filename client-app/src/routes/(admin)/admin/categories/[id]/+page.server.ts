import { error, fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminCategory } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'ID không hợp lệ.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const category = await svc.getCategory(id);
        return { category };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Không tìm thấy danh mục.');
        }
        throw error(500, 'Không thể tải thông tin danh mục.');
    }
};

export const actions: Actions = {
    update: async ({ params, request, fetch, cookies }) => {
        const id            = Number(params.id);
        const data          = await request.formData();
        const name          = (data.get('name') as string ?? '').trim();
        const slug          = (data.get('slug') as string ?? '').trim();
        const icon          = (data.get('icon') as string ?? '').trim() || null;
        const display_order = Number(data.get('display_order') ?? 0);

        if (!name || !slug) return fail(400, { error: 'Tên và slug không được để trống.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateCategory(id, { name, slug, icon, display_order });
            return { success: true };
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
            await svc.toggleCategoryActive(id, is_active, reason);
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },
};
