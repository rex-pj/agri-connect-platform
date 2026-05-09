import { error, fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminUserDetail, Role } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'ID không hợp lệ.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');

    const [userResult, rolesResult] = await Promise.allSettled([
        svc.getUser(id),
        svc.listRoles(),
    ]);

    if (userResult.status === 'rejected') {
        const err = userResult.reason;
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Không tìm thấy người dùng.');
        }
        throw error(500, 'Không thể tải thông tin người dùng.');
    }

    const roles = rolesResult.status === 'fulfilled' ? rolesResult.value : [];
    return { user: userResult.value, roles };
};

export const actions: Actions = {
    update_role: async ({ params, request, fetch, cookies }) => {
        const id        = Number(params.id);
        const data      = await request.formData();
        const role_code = (data.get('role_code') as string ?? '').trim();
        if (!role_code) return fail(400, { error: 'Vui lòng chọn vai trò.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateUserRole(id, role_code);
            return { success: true, message: 'Đã cập nhật vai trò.' };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật vai trò thất bại.' });
        }
    },

    toggle_active: async ({ params, request, fetch, cookies }) => {
        const id        = Number(params.id);
        const data      = await request.formData();
        const is_active = data.get('is_active') === 'true';

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleUserActive(id, is_active);
            return { success: true, message: `Đã ${is_active ? 'kích hoạt' : 'vô hiệu hoá'} tài khoản.` };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },
};
