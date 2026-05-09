import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { setAuthCookie, ServiceError, PHONE_REGEX } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ cookies }) => {
    if (cookies.get('auth_token')) throw redirect(302, ROUTES.PROVIDER_DASHBOARD);
    return {};
};

export const actions: Actions = {
    default: async ({ request, fetch, cookies }) => {
        const data = await request.formData();

        const username       = (data.get('username')       as string ?? '').trim();
        const email          = (data.get('email')          as string ?? '').trim();
        const password       = (data.get('password')       as string ?? '');
        const password2      = (data.get('password2')      as string ?? '');
        const full_name      = (data.get('full_name')      as string ?? '').trim() || null;
        const business_name  = (data.get('business_name')  as string ?? '').trim();
        const business_phone = (data.get('business_phone') as string ?? '').trim();

        if (!username || !email || !password || !business_name || !business_phone) {
            return fail(400, { error: 'Vui lòng điền đầy đủ thông tin bắt buộc.' });
        }
        if (password !== password2) {
            return fail(400, { error: 'Mật khẩu xác nhận không khớp.' });
        }
        if (!PHONE_REGEX.test(business_phone)) {
            return fail(400, { error: 'Số điện thoại không hợp lệ (phải là 10 số, bắt đầu bằng 0).' });
        }

        const svc = new AuthService(fetch);
        try {
            const { token } = await svc.register({ username, email, password, full_name, business_name, business_phone });
            setAuthCookie(cookies, token);
            throw redirect(302, ROUTES.PROVIDER_DASHBOARD);
        } catch (err) {
            if (err instanceof Response) throw err;
            if (err instanceof ServiceError) return fail(err.status, { error: err.message });
            return fail(503, { error: 'Không thể kết nối máy chủ. Vui lòng thử lại.' });
        }
    },
};
