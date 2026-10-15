import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { ServiceError, PASSWORD_MIN_LEN, decodeJwtClaims } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ fetch, parent }) => {
    const { token } = await parent();

    const claims = decodeJwtClaims(token);
    if (!claims) throw redirect(302, ROUTES.LOGIN);

    const svc = new AuthService(fetch);
    try {
        const me = await svc.me(token);
        return {
            username:         me.username,
            email:            me.email,
            fullName:         me.full_name,
            createdOn:        me.created_on,
            role:             claims.role,
            sessionExpiresAt: new Date(claims.exp * 1000).toISOString(),
        };
    } catch (err) {
        if (err instanceof ServiceError && (err.status === 401 || err.status === 403)) {
            throw redirect(302, ROUTES.LOGIN);
        }
        throw redirect(302, ROUTES.LOGIN);
    }
};

export const actions: Actions = {

    change_password: async ({ request, fetch, cookies }) => {
        const data             = await request.formData();
        const current_password = (data.get('current_password') as string ?? '');
        const new_password     = (data.get('new_password')     as string ?? '');
        const password2        = (data.get('password2')        as string ?? '');

        if (!current_password || !new_password) {
            return fail(400, { action: 'change_password', error: 'Vui lòng điền đầy đủ thông tin.' });
        }
        if (new_password !== password2) {
            return fail(400, { action: 'change_password', error: 'Mật khẩu xác nhận không khớp.' });
        }
        if (new_password.length < PASSWORD_MIN_LEN) {
            return fail(400, { action: 'change_password', error: `Mật khẩu mới phải có ít nhất ${PASSWORD_MIN_LEN} ký tự.` });
        }

        const token = cookies.get('auth_token') ?? '';
        const svc   = new AuthService(fetch);
        try {
            await svc.changePassword(token, current_password, new_password);
            return { action: 'change_password', success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                if (err.status === 401 || err.status === 403) throw redirect(302, ROUTES.LOGIN);
                return fail(err.status, { action: 'change_password', error: err.message });
            }
            return fail(500, { action: 'change_password', error: 'Đổi mật khẩu thất bại.' });
        }
    },
};
