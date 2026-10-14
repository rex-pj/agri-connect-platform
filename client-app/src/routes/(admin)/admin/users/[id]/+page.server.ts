import { error, fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminUserDetail, Role, Permission, UserPermissions } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'Invalid ID.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');

    const [userResult, rolesResult, allPermsResult, userPermsResult] = await Promise.allSettled([
        svc.getUser(id),
        svc.listRoles(),
        svc.listPermissions(),
        svc.getUserPermissions(id),
    ]);

    if (userResult.status === 'rejected') {
        const err = userResult.reason;
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'User not found.');
        }
        throw error(500, 'Failed to load user.');
    }

    const roles       = rolesResult.status      === 'fulfilled' ? rolesResult.value      : [];
    const allPerms    = allPermsResult.status    === 'fulfilled' ? allPermsResult.value    : [];
    const userPermIds = userPermsResult.status   === 'fulfilled'
        ? new Set(userPermsResult.value.permissions.map((p) => p.id))
        : new Set<number>();

    return { user: userResult.value, roles, allPerms, userPermIds: [...userPermIds] };
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
        const reason    = (data.get('reason') as string ?? '').trim() || null;

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleUserActive(id, is_active, reason);
            return {
                success: true,
                action: 'toggle_active',
                message: is_active ? 'Tài khoản đã được kích hoạt.' : 'Tài khoản đã bị vô hiệu hoá.',
            };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message, action: 'toggle_active' });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.', action: 'toggle_active' });
        }
    },

};
