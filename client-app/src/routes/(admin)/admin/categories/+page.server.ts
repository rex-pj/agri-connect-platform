import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { checkAuth, ServiceError } from '$lib/server/api';
import { AdminService } from '$lib/server/services/admin.service';

export type { AdminCategory } from '$lib/server/services/admin.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
    try {
        const categories = await svc.listCategories();
        return { categories, error: null };
    } catch (err) {
        if (err instanceof ServiceError) {
            checkAuth(err.status);
            return { categories: [], error: err.message };
        }
        return { categories: [], error: 'Failed to load categories.' };
    }
};

export const actions: Actions = {
    create: async ({ request, fetch, cookies }) => {
        const data          = await request.formData();
        const name          = (data.get('name')          as string ?? '').trim();
        const slug          = (data.get('slug')          as string ?? '').trim();
        const category_type = (data.get('category_type') as string ?? '').trim();
        const icon          = (data.get('icon')          as string ?? '').trim() || null;
        const display_order = Number(data.get('display_order') ?? 0);

        if (!name || !slug || !category_type)
            return fail(400, { error: 'Tên, slug và loại danh mục không được để trống.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.createCategory({ name, slug, category_type, icon, display_order });
            return { success: true, message: `Đã tạo danh mục «${name}».` };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Tạo danh mục thất bại.' });
        }
    },

    update: async ({ request, fetch, cookies }) => {
        const data          = await request.formData();
        const id            = Number(data.get('id'));
        const name          = (data.get('name') as string ?? '').trim();
        const slug          = (data.get('slug') as string ?? '').trim();
        const icon          = (data.get('icon') as string ?? '').trim() || null;
        const display_order = Number(data.get('display_order') ?? 0);

        if (!id || !name || !slug) return fail(400, { error: 'Thông tin không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateCategory(id, { name, slug, icon, display_order });
            return { success: true, message: 'Đã cập nhật danh mục.' };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật danh mục thất bại.' });
        }
    },

    toggle_active: async ({ request, fetch, cookies }) => {
        const data      = await request.formData();
        const id        = Number(data.get('id'));
        const is_active = data.get('is_active') === 'true';
        if (!id) return fail(400, { error: 'ID không hợp lệ.' });

        const svc = new AdminService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.toggleCategoryActive(id, is_active);
            return { success: true, message: `Đã ${is_active ? 'bật' : 'tắt'} danh mục.` };
        } catch (err) {
            if (err instanceof ServiceError) {
                checkAuth(err.status);
                return fail(err.status, { error: err.message });
            }
            return fail(500, { error: 'Cập nhật trạng thái thất bại.' });
        }
    },
};
