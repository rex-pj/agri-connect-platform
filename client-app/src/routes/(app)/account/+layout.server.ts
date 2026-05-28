import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { ROUTES } from '$lib/routes';

export const load: LayoutServerLoad = async ({ cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.LOGIN);
    return { token };
};
