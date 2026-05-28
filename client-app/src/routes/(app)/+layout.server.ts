import type { LayoutServerLoad } from './$types';
import { decodeJwtRole } from '$lib/server/api';

export const load: LayoutServerLoad = async ({ cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) return { isLoggedIn: false, role: null };
    const role = decodeJwtRole(token);
    return { isLoggedIn: true, role };
};
