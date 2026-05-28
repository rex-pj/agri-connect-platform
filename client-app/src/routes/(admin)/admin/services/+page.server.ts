import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminServiceEntry } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page          = url.searchParams.get('page') ?? '1';
    const providerIdRaw = url.searchParams.get('provider_id');
    const providerId    = providerIdRaw ? Number(providerIdRaw) : undefined;

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const [result, categories] = await Promise.all([
            svc.listServices(page, providerId),
            svc.listCategories(),
        ]);
        const serviceCategories = categories.filter(c => c.category_type === 'service' && c.is_active);
        return { result, serviceCategories, providerId: providerId ?? null, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { result: null, serviceCategories: [], providerId: providerId ?? null, error: err.message };
        }
        return { result: null, serviceCategories: [], providerId: providerId ?? null, error: 'Tải dịch vụ thất bại.' };
    }
};

export const actions: Actions = {
    create: async ({ request, fetch, cookies }) => {
        const data        = await request.formData();
        const business_id = Number(data.get('business_id'));
        const category_id = Number(data.get('category_id'));
        const name        = (data.get('name') as string ?? '').trim();
        const description = (data.get('description') as string ?? '').trim() || null;
        const price_note  = (data.get('price_note') as string ?? '').trim() || null;

        if (!business_id || !category_id || !name)
            return fail(400, { error: 'ID nhà cung cấp, danh mục và tên không được để trống.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.createService({ business_id, category_id, name, description, price_note });
            return { success: true, message: 'Dịch vụ đã được tạo.' };
        } catch (err) {
            if (err instanceof ServiceError) { checkAuth(err.status); return fail(err.status, { error: err.message }); }
            return fail(500, { error: 'Tạo dịch vụ thất bại.' });
        }
    },

    update: async ({ request, fetch, cookies }) => {
        const data        = await request.formData();
        const id          = Number(data.get('id'));
        const category_id = Number(data.get('category_id'));
        const name        = (data.get('name') as string ?? '').trim();
        const description = (data.get('description') as string ?? '').trim() || null;
        const price_note  = (data.get('price_note') as string ?? '').trim() || null;

        if (!id || !category_id || !name) return fail(400, { error: 'Thông tin không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateService(id, { category_id, name, description, price_note });
            return { success: true, message: 'Dịch vụ đã được cập nhật.' };
        } catch (err) {
            if (err instanceof ServiceError) { checkAuth(err.status); return fail(err.status, { error: err.message }); }
            return fail(500, { error: 'Cập nhật dịch vụ thất bại.' });
        }
    },

    toggle_active: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const id        = Number(data.get('id'));
        const is_active = data.get('is_active') === 'true';
        const reason    = (data.get('reason') as string ?? '').trim() || null;
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleServiceActive(id, is_active, reason);
            return { success: true, message: is_active ? 'Dịch vụ đã được hiển thị.' : 'Dịch vụ đã bị ẩn.' };
        } catch (err) {
            if (err instanceof ServiceError) { checkAuth(err.status); return fail(err.status, { error: err.message }); }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },

    delete: async ({ request, fetch, cookies }) => {
        const data   = await request.formData();
        const id     = Number(data.get('id'));
        const reason = (data.get('reason') as string ?? '').trim() || null;
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.deleteService(id, reason);
            return { success: true, message: 'Dịch vụ đã được xoá.' };
        } catch (err) {
            if (err instanceof ServiceError) { checkAuth(err.status); return fail(err.status, { error: err.message }); }
            return fail(500, { error: 'Xoá dịch vụ thất bại.' });
        }
    },
};
