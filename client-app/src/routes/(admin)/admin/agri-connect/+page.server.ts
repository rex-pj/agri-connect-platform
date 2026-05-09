import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminProvider, AdminProviderPaged } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const result = await svc.listProviders(page);
        return { result, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return {
                result: { items: [], total: 0, page: 1, total_pages: 1 },
                error: err.message,
            };
        }
        return {
            result: { items: [], total: 0, page: 1, total_pages: 1 },
            error: 'Không thể tải danh sách nhà cung cấp.',
        };
    }
};

export const actions: Actions = {
    assign_user: async ({ request, fetch, cookies }) => {
        const data     = await request.formData();
        const id       = Number(data.get('business_id'));
        const username = (data.get('username') as string ?? '').trim();
        const password = (data.get('password') as string ?? '');

        if (!id || isNaN(id))    return fail(400, { error: 'ID doanh nghiệp không hợp lệ.' });
        if (!username)           return fail(400, { error: 'Vui lòng nhập tên đăng nhập.' });
        if (password.length < 8) return fail(400, { error: 'Mật khẩu phải có ít nhất 8 ký tự.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            const result = await svc.assignProviderUser(id, { username, password });
            return { success: true, message: result.message };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Gán tài khoản thất bại.' });
        }
    },

    toggle_verify: async ({ request, fetch, cookies }) => {
        const data = await request.formData();
        const id   = Number(data.get('id'));

        if (!id || isNaN(id)) return fail(400, { error: 'ID nhà cung cấp không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            const result = await svc.toggleProviderVerification(id);
            return { success: true, message: result.message, is_verified: result.is_verified };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật thất bại.' });
        }
    },
};
