import { redirect } from '@sveltejs/kit';
import type { Actions } from './$types';
import { ROUTES } from '$lib/routes';

export const actions: Actions = {
    default: async ({ cookies }) => {
        cookies.delete('auth_token', { path: '/' });
        throw redirect(302, ROUTES.LOGIN);
    },
};
