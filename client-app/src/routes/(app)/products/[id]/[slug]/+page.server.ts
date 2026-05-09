import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';
import { ServiceError } from '$lib/server/api';

export type { ProductDetail } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ params, fetch }) => {
    const svc = new AgriConnectService(fetch);
    try {
        const product = await svc.getProduct(params.id);
        return { product };
    } catch (err) {
        if (err instanceof ServiceError && err.status === 404) throw error(404, 'Sản phẩm không tồn tại');
        throw error(500, 'Không thể tải thông tin sản phẩm');
    }
};
