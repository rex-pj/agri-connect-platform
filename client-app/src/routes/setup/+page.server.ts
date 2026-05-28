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
    if ((status?.user_count ?? 0) > 0) {
        throw redirect(302, ROUTES.LOGIN);
    }

    return {};
};

export const actions: Actions = {
    create: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const username  = (data.get('username')  as string ?? '').trim();
        const email     = (data.get('email')      as string ?? '').trim();
        const password  = (data.get('password')   as string ?? '');
        const password2 = (data.get('password2')  as string ?? '');
        const full_name = (data.get('full_name')   as string ?? '').trim() || null;
        const goSeed    = data.get('go_seed') === '1';

        if (!username || !email || !password) {
            return fail(400, { error: 'Vui lòng điền đầy đủ thông tin bắt buộc.' });
        }
        if (password !== password2) {
            return fail(400, { error: 'Mật khẩu xác nhận không khớp.' });
        }

        const svc = new AuthService(fetch);
        try {
            const { token } = await svc.createFirstAdmin({ username, email, password, full_name });
            setAuthCookie(cookies, token);
            throw redirect(302, goSeed ? ROUTES.SETUP_SEED : ROUTES.ADMIN.DASHBOARD);
        } catch (err) {
            if (isRedirect(err)) throw err;
            if (err instanceof ServiceError) return fail(err.status, { error: err.message });
            throw error(503, 'Không thể kết nối máy chủ.');
        }
    },
};
