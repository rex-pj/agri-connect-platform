import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminUser, AdminUserPaged, Role } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page  = url.searchParams.get('page') ?? '1';
    const token = cookies.get('auth_token') ?? '';
    const svc   = new AdminService(fetch, token);

    const [usersResult, rolesResult] = await Promise.allSettled([
        svc.listUsers(page),
        svc.listRoles(),
    ]);

    if (usersResult.status === 'rejected') {
        const err = usersResult.reason;
        if (err instanceof ServiceError) checkAuth(err.status);
    }
    if (rolesResult.status === 'rejected') {
        const err = rolesResult.reason;
        if (err instanceof ServiceError) checkAuth(err.status);
    }

    const result = usersResult.status === 'fulfilled'
        ? usersResult.value
        : { items: [], total: 0, page: 1, total_pages: 1 };
    const roles  = rolesResult.status === 'fulfilled' ? rolesResult.value : [];

    return {
        result,
        roles,
        error: usersResult.status === 'rejected' ? 'Failed to load users.' : null,
    };
};

export const actions: Actions = {
    update_role: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const id        = Number(data.get('id'));
        const role_code = (data.get('role_code') as string ?? '').trim();
        if (!id || !role_code) return fail(400, { error: 'Thông tin không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateUserRole(id, role_code);
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật vai trò thất bại.' });
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
            await svc.toggleUserActive(id, is_active, reason);
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
