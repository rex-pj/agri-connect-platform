import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { ROUTES } from '$lib/routes';
import { decodeJwtRole } from '$lib/server/api';

export const load: LayoutServerLoad = async ({ cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.LOGIN);
    const role = decodeJwtRole(token);
    if (role !== 'provider') throw redirect(302, role ? ROUTES.HOME : ROUTES.LOGIN);
    return { token };
};
