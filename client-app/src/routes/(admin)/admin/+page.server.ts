import { error, redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ cookies }) => {
    if (cookies.get('auth_token')) {
        throw redirect(302, ROUTES.ADMIN.DASHBOARD);
    }
    throw error(404);
};
