import type { PageServerLoad } from './$types';
import { ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminStats } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const token = cookies.get('auth_token') ?? '';
    const svc   = new AdminService(fetch, token);
    try {
        const stats = await svc.getStats();
        return { stats, statsError: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            return { stats: null, statsError: err.message };
        }
        return { stats: null, statsError: 'Không thể tải thống kê.' };
    }
};
