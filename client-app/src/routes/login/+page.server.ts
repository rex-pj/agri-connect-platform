import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { setAuthCookie } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ServiceError } from '$lib/server/api';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ cookies, fetch, url }) => {
    if (cookies.get('auth_token')) {
        throw redirect(302, ROUTES.ADMIN.DASHBOARD);
    }

    const passwordReset = url.searchParams.get('reset') === '1';
    const svc           = new AuthService(fetch);
    const status        = await svc.getSetupStatus();
    const firstRun      = (status?.user_count ?? 1) === 0;

    return { firstRun, passwordReset };
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
            if (err instanceof Response) throw err; // re-throw redirects
            if (err instanceof ServiceError) {
                return fail(err.status, { error: err.message, locked: err.status === 423 });
            }
            return fail(503, { error: 'Không thể kết nối máy chủ. Vui lòng thử lại.' });
        }
    },

    setup: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const username  = (data.get('username')  as string ?? '').trim();
        const password  = (data.get('password')  as string ?? '');
        const password2 = (data.get('password2') as string ?? '');
        const full_name = (data.get('full_name')  as string ?? '').trim() || null;

        if (!username || !password) {
            return fail(400, { error: 'Vui lòng điền đầy đủ thông tin.' });
        }
        if (password !== password2) {
            return fail(400, { error: 'Mật khẩu xác nhận không khớp.' });
        }

        const svc = new AuthService(fetch);
        try {
            const { token } = await svc.createFirstAdmin({ username, password, full_name });
            setAuthCookie(cookies, token);
            throw redirect(302, ROUTES.ADMIN.DASHBOARD);
        } catch (err) {
            if (err instanceof Response) throw err;
            if (err instanceof ServiceError) return fail(err.status, { error: err.message });
            return fail(503, { error: 'Không thể kết nối máy chủ.' });
        }
    },
};
