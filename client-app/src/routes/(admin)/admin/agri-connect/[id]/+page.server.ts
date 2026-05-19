import { error, fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminProviderDetail, AdminServiceItem, AdminProductItem } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, params }) => {
    const id = Number(params.id);
    if (!id || isNaN(id)) throw error(400, 'Invalid ID.');

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const provider = await svc.getProvider(id);
        return { provider };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            if (err.status === 404) throw error(404, 'Provider not found.');
        }
        throw error(500, 'Failed to load provider.');
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
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Update failed.' });
        }
    },
};
