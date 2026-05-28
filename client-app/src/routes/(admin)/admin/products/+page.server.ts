import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminProduct } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies, url }) => {
    const page       = url.searchParams.get('page') ?? '1';
    const providerIdRaw = url.searchParams.get('provider_id');
    const providerId = providerIdRaw ? Number(providerIdRaw) : undefined;

    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const result = await svc.listProducts(page, providerId);
        return { result, providerId: providerId ?? null, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { result: null, providerId: providerId ?? null, error: err.message };
        }
        return { result: null, providerId: providerId ?? null, error: 'Tải sản phẩm thất bại.' };
    }
};

export const actions: Actions = {
    toggle_active: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const id        = Number(data.get('id'));
        const is_active = data.get('is_active') === 'true';
        const reason    = (data.get('reason') as string ?? '').trim() || null;
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleProductActive(id, is_active, reason);
            return { success: true, message: is_active ? 'Sản phẩm đã được hiển thị.' : 'Sản phẩm đã bị ẩn.' };
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
            await svc.deleteProduct(id, reason);
            return { success: true, message: 'Sản phẩm đã được xoá.' };
        } catch (err) {
            if (err instanceof ServiceError) { checkAuth(err.status); return fail(err.status, { error: err.message }); }
            return fail(500, { error: 'Xoá sản phẩm thất bại.' });
        }
    },
};
