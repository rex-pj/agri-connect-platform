import type { PageServerLoad } from './$types';
import { ServiceError, checkAuth } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { Permission } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const permissions = await svc.listPermissions();
        return { permissions };
    } catch (err) {
        if (err instanceof ServiceError) checkAuth(err.status);
        return { permissions: [] };
    }
};
