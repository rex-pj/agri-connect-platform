import { redirect, error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';
import { ServiceError } from '$lib/server/api';
import { slugify } from '$lib/slugify';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ params, fetch }) => {
    const svc = new ExploreService(fetch);
    try {
        const product = await svc.getProduct(params.id);
        throw redirect(301, ROUTES.PRODUCT(params.id, slugify(product.name)));
    } catch (err) {
        if (err instanceof Response) throw err; // re-throw redirects
        if (err instanceof ServiceError && err.status === 404) throw error(404, 'Sản phẩm không tồn tại');
        throw error(500, 'Không thể tải thông tin sản phẩm');
    }
};
