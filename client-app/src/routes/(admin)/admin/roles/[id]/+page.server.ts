import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { Role as RoleDetail } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'ID không hợp lệ.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const role = await svc.getRole(id);
        return { role };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Không tìm thấy vai trò.');
        }
        throw error(500, 'Không thể tải thông tin vai trò.');
    }
};
