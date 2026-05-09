import { error, fail } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';
import { ServiceError, PHONE_REGEX } from '$lib/server/api';

export type { Category, BrowseProvider, BrowseResult } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ params, fetch, url }) => {
    const categoryId = Number(params.id);
    if (!categoryId) throw error(400, 'Danh mục không hợp lệ');

    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AgriConnectService(fetch);

    const [providersResult, categoriesResult] = await Promise.allSettled([
        svc.browseProviders(page, String(categoryId)),
        svc.getCategories('service'),
    ]);

    const categories      = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];
    const currentCategory = categories.find(c => c.id === categoryId) ?? null;

    if (providersResult.status === 'rejected') {
        return {
            result: { providers: [], total: 0, page: 1, total_pages: 0 },
            categories,
            currentCategory,
            error: 'Không thể tải danh sách dịch vụ.',
        };
    }

    return { result: providersResult.value, categories, currentCategory, error: null };
};

export const actions: Actions = {
    contact_request: async ({ request, fetch }) => {
        const data = await request.formData();

        const business_id     = data.get('business_id');
        const requester_name  = (data.get('requester_name')  as string ?? '').trim();
        const requester_phone = (data.get('requester_phone') as string ?? '').trim();
        const topic           = (data.get('topic')           as string ?? '').trim();
        const note            = (data.get('note')            as string ?? '').trim();

        if (!business_id) return fail(400, { message: 'Thiếu thông tin nhà cung cấp.' });
        if (!requester_name) return fail(400, { message: 'Vui lòng nhập họ tên của bạn.' });
        if (!requester_phone || !PHONE_REGEX.test(requester_phone))
            return fail(400, { message: 'Số điện thoại không hợp lệ (ví dụ: 0901234567).' });
        if (!topic) return fail(400, { message: 'Vui lòng chọn dịch vụ cần liên hệ.' });

        const svc = new AgriConnectService(fetch);
        try {
            await svc.submitContact({
                business_id: Number(business_id),
                topic, requester_name, requester_phone,
                note: note || null,
            });
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) return fail(err.status, { message: err.message });
            return fail(500, { message: 'Máy chủ không phản hồi. Vui lòng thử lại.' });
        }
    },
};
