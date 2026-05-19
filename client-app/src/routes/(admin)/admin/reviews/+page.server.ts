import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminReview, AdminReviewPaged } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const result = await svc.listReviews(page);
        return { result, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { result: { items: [], total: 0, page: 1, total_pages: 1 }, error: err.message };
        }
        return { result: { items: [], total: 0, page: 1, total_pages: 1 }, error: 'Failed to load reviews.' };
    }
};

export const actions: Actions = {
    toggle_visible: async ({ request, fetch, cookies }) => {
        const data       = await request.formData();
        const id         = Number(data.get('id'));
        const is_visible = data.get('is_visible') === 'true';
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleReviewVisible(id, is_visible);
            return { success: true, message: `Đã ${is_visible ? 'hiện' : 'ẩn'} đánh giá.` };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật hiển thị thất bại.' });
        }
    },
};
