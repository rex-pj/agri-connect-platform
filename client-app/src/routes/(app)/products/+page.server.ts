import type { PageServerLoad } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';

export type { Category, Product, ProductsResult } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ fetch, url }) => {
    const page = url.searchParams.get('page') ?? '1';
    const svc  = new AgriConnectService(fetch);

    const [productsResult, categoriesResult] = await Promise.allSettled([
        svc.browseProducts(page),
        svc.getCategories('product'),
    ]);

    const categories = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];

    if (productsResult.status === 'rejected') {
        return {
            result: { products: [], total: 0, page: 1, total_pages: 0 },
            categories,
            error: 'Máy chủ không phản hồi. Vui lòng thử lại.',
        };
    }

    return { result: productsResult.value, categories, error: null };
};
