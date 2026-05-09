import { redirect, error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';
import { ServiceError } from '$lib/server/api';
import { slugify } from '$lib/slugify';
import { ROUTES } from '$lib/routes';

export const load: PageServerLoad = async ({ params, fetch }) => {
    const svc = new AgriConnectService(fetch);
    try {
        const provider = await svc.getProvider(params.id);
        throw redirect(301, ROUTES.PROVIDER(params.id, slugify(provider.name)));
    } catch (err) {
        if (err instanceof Response) throw err; // re-throw redirects
        if (err instanceof ServiceError && err.status === 404) throw error(404, 'Nhà cung cấp không tồn tại');
        throw error(500, 'Không thể tải thông tin nhà cung cấp');
    }
};
