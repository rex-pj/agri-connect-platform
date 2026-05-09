import { error } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw error(404);

    let username = 'Admin';
    try {
        const payload = JSON.parse(atob(token.split('.')[1]));
        username = payload.username ?? 'Admin';
    } catch { /* malformed token */ }

    return { username };
};
