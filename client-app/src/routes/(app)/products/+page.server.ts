import type { PageServerLoad } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';

export type { Category, Product, ProductsResult } from '$lib/server/services/explore.service';

export const load: PageServerLoad = async ({ fetch, url }) => {
    const page        = url.searchParams.get('page') ?? '1';
    const searchQuery = url.searchParams.get('q')         || null;
    const categoryId  = url.searchParams.get('category_id') || undefined;
    const minPrice    = url.searchParams.get('min_price') || undefined;
    const maxPrice    = url.searchParams.get('max_price') || undefined;
    const sort        = url.searchParams.get('sort')      || undefined;
    const svc         = new ExploreService(fetch);

    const [productsResult, categoriesResult] = await Promise.allSettled([
        svc.browseProducts(page, categoryId, searchQuery ?? undefined, minPrice, maxPrice, sort),
        svc.getCategories('product'),
    ]);

    const categories = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];

    if (productsResult.status === 'rejected') {
        return {
            result: { items: [], total: 0, page: 1, total_pages: 0 },
            categories,
            searchQuery,
            categoryId:  categoryId  ?? null,
            minPrice:    minPrice    ?? null,
            maxPrice:    maxPrice    ?? null,
            sort:        sort        ?? null,
            error: 'Máy chủ không phản hồi. Vui lòng thử lại.',
        };
    }

    return {
        result:     productsResult.value,
        categories,
        searchQuery,
        categoryId:  categoryId  ?? null,
        minPrice:    minPrice    ?? null,
        maxPrice:    maxPrice    ?? null,
        sort:        sort        ?? null,
        error:       null,
    };
};
