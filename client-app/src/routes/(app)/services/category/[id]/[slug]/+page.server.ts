import { error, fail } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';
import { ServiceError, PHONE_REGEX } from '$lib/server/api';

export type { Category, BrowseProvider, BrowseResult } from '$lib/server/services/explore.service';

export const load: PageServerLoad = async ({ params, fetch, url }) => {
    const categoryId = Number(params.id);
    if (!categoryId) throw error(400, 'Danh mục không hợp lệ');

    const page = url.searchParams.get('page') ?? '1';
    const svc  = new ExploreService(fetch);

    const [providersResult, categoriesResult] = await Promise.allSettled([
        svc.browseProviders(page, String(categoryId)),
        svc.getCategories('service'),
    ]);

    const categories      = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];
    const currentCategory = categories.find(c => c.id === categoryId) ?? null;

    if (providersResult.status === 'rejected') {
        return {
            result: { items: [], total: 0, page: 1, total_pages: 0 },
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

        const values = { requester_name, requester_phone, topic, note };

        if (!business_id) return fail(400, { error: 'Thiếu thông tin nhà cung cấp.', values });
        if (!requester_name) return fail(400, { error: 'Vui lòng nhập họ tên của bạn.', values });
        if (!requester_phone || !PHONE_REGEX.test(requester_phone))
            return fail(400, { error: 'Số điện thoại không hợp lệ (ví dụ: 0901234567).', values });
        if (!topic) return fail(400, { error: 'Vui lòng chọn dịch vụ cần liên hệ.', values });

        const svc = new ExploreService(fetch);
        try {
            await svc.submitContact({
                business_id: Number(business_id),
                topic, requester_name, requester_phone,
                note: note || null,
            });
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) return fail(err.status, { error: err.message, values });
            return fail(500, { error: 'Máy chủ không phản hồi. Vui lòng thử lại.', values });
        }
    },
};
