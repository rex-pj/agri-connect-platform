import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { ServiceError, PASSWORD_MIN_LEN } from '$lib/server/api';
import { AuthService } from '$lib/server/services/auth.service';
import { ROUTES } from '$lib/routes';

interface TokenClaims {
    role: string;
    exp:  number;
}

function decodeToken(token: string): TokenClaims | null {
    try {
        const payload = token.split('.')[1];
        const json    = Buffer.from(payload, 'base64url').toString('utf8');
        const claims  = JSON.parse(json);
        if (!claims.role || !claims.exp) return null;
        return { role: claims.role, exp: claims.exp };
    } catch {
        return null;
    }
}

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.LOGIN);

    const claims = decodeToken(token);
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

    logout: async ({ cookies }) => {
        cookies.delete('auth_token', { path: '/' });
        throw redirect(302, ROUTES.HOME);
    },
};
