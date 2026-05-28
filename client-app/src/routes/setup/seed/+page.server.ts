import { fail, isRedirect, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.SETUP);

    const svc    = new AdminService(fetch, token);
    const status = await svc.getSetupStatus().catch(() => null);

    if (status?.is_seeded) {
        throw redirect(302, ROUTES.ADMIN.DASHBOARD);
    }

    return { status };
};

export const actions: Actions = {
    seed: async ({ fetch, cookies }) => {
        const token = cookies.get('auth_token') ?? '';
        const svc   = new AdminService(fetch, token);
        try {
            await svc.seedDemoData();
            throw redirect(302, ROUTES.ADMIN.DASHBOARD);
        } catch (err) {
            if (isRedirect(err)) throw err;
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Không thể nạp dữ liệu mẫu.' });
        }
    },

    skip: async () => {
        throw redirect(302, ROUTES.ADMIN.DASHBOARD);
    },
};
