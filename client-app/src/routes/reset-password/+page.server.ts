import { fail, redirect, isRedirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { ServiceError, PASSWORD_MIN_LEN } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ url }) => {
    const token = url.searchParams.get('token') ?? '';
    return { token };
};

export const actions: Actions = {
    default: async ({ request, fetch }) => {
        const data         = await request.formData();
        const token        = (data.get('token')        as string ?? '').trim();
        const new_password = (data.get('new_password') as string ?? '');
        const password2    = (data.get('password2')    as string ?? '');

        if (!token)        return fail(400, { token, error: 'Token không hợp lệ.' });
        if (!new_password) return fail(400, { token, error: 'Vui lòng nhập mật khẩu mới.' });
        if (new_password !== password2) {
            return fail(400, { token, error: 'Mật khẩu xác nhận không khớp.' });
        }
        if (new_password.length < PASSWORD_MIN_LEN) {
            return fail(400, { token, error: `Mật khẩu phải có ít nhất ${PASSWORD_MIN_LEN} ký tự.` });
        }

        const svc = new AuthService(fetch);
        try {
            await svc.resetPassword(token, new_password);
            throw redirect(302, ROUTES.LOGIN_RESET);
        } catch (err) {
            if (isRedirect(err)) throw err;
            if (err instanceof ServiceError) return fail(err.status, { token, error: err.message });
            return fail(503, { token, error: 'Không thể kết nối máy chủ. Vui lòng thử lại.' });
        }
    },
};
