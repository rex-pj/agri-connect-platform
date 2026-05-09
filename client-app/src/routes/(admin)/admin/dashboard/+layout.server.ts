import { error, redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { AuthService } from '$lib/server/services/auth.service';
import { ServiceError } from '$lib/server/api';
import { ROUTES } from '$lib/routes';

export const load: LayoutServerLoad = async ({ cookies, fetch }) => {
    const token = cookies.get('auth_token');
    if (!token) throw error(404);

    let username = 'Admin';
    try {
        const svc = new AuthService(fetch);
        const me  = await svc.me(token);
        username  = me.username;
    } catch (e) {
        if (e instanceof ServiceError && e.status === 401) {
            cookies.delete('auth_token', { path: '/' });
            throw redirect(302, ROUTES.LOGIN);
        }
        if (e && typeof e === 'object' && 'status' in e) throw e;
        // Backend unreachable — keep default, first protected API call will 401.
    }

    return { username };
};
