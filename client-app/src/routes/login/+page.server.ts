import { error, fail, redirect, isRedirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { setAuthCookie } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ServiceError } from '$lib/server/api';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ cookies, fetch }) => {
    if (cookies.get('auth_token')) {
        throw redirect(302, ROUTES.ADMIN.DASHBOARD);
    }

    const svc    = new AuthService(fetch);
    const status = await svc.getSetupStatus();
    if ((status?.user_count ?? 1) === 0) {
        throw redirect(302, ROUTES.SETUP);
    }

    return {};
};

export const actions: Actions = {
    login: async ({ request, fetch, cookies }) => {
        const data     = await request.formData();
        const username = (data.get('username') as string ?? '').trim();
        const password = (data.get('password') as string ?? '');

        if (!username || !password) {
            return fail(400, { error: 'Vui lòng nhập tên đăng nhập và mật khẩu.' });
        }

        const svc = new AuthService(fetch);
        try {
            const { token } = await svc.login(username, password);
            setAuthCookie(cookies, token);
            throw redirect(302, ROUTES.ADMIN.DASHBOARD);
        } catch (err) {
            if (isRedirect(err)) throw err;
            if (err instanceof ServiceError) {
                return fail(err.status, { error: err.message, locked: err.status === 423 });
            }
            throw error(503, 'Không thể kết nối máy chủ. Vui lòng thử lại.');
        }
    },
};
