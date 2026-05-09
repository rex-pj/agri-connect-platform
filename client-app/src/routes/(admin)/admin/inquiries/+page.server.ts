import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminInquiry, AdminInquiryPaged } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const result = await svc.listInquiries(page);
        return { result, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { result: { items: [], total: 0, page: 1, total_pages: 1 }, error: err.message };
        }
        return { result: { items: [], total: 0, page: 1, total_pages: 1 }, error: 'Không thể tải danh sách yêu cầu.' };
    }
};

export const actions: Actions = {
    update_status: async ({ request, fetch, cookies }) => {
        const data   = await request.formData();
        const id     = Number(data.get('id'));
        const status = (data.get('status') as string ?? '').trim();
        if (!id || !status) return fail(400, { error: 'Thông tin không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateInquiryStatus(id, status);
            return { success: true, message: 'Đã cập nhật trạng thái yêu cầu.' };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },
};
