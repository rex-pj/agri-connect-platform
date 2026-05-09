import type { PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { Role } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const roles = await svc.listRoles();
        return { roles, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { roles: [], error: err.message };
        }
        return { roles: [], error: 'Không thể tải danh sách vai trò.' };
    }
};
