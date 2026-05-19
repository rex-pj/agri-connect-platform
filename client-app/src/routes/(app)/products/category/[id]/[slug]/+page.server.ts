import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';

export type { Category, Product, ProductsResult } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ params, fetch, url }) => {
    const categoryId = Number(params.id);
    if (!categoryId) throw error(400, 'Danh mục không hợp lệ');

    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AgriConnectService(fetch);

    const [productsResult, categoriesResult] = await Promise.allSettled([
        svc.browseProducts(page, String(categoryId)),
        svc.getCategories('product'),
    ]);

    const categories      = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];
    const currentCategory = categories.find(c => c.id === categoryId) ?? null;

    if (productsResult.status === 'rejected') {
        return {
            result: { items: [], total: 0, page: 1, total_pages: 0 },
            categories,
            currentCategory,
            error: 'Không thể tải danh sách sản phẩm.',
        };
    }

    return { result: productsResult.value, categories, currentCategory, error: null };
};
