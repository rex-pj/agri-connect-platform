import type { PageServerLoad } from './$types';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';

export type { BrowseProvider, Product } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ fetch, url }) => {
    const q = url.searchParams.get('q')?.trim() ?? '';

    if (!q) {
        return { q, providers: [], products: [], providerTotal: 0, productTotal: 0 };
    }

    const svc = new AgriConnectService(fetch);

    const [providersResult, productsResult] = await Promise.allSettled([
        svc.browseProviders('1', undefined, q),
        svc.browseProducts('1', undefined, q),
    ]);

    const providers     = providersResult.status === 'fulfilled' ? providersResult.value.items : [];
    const providerTotal = providersResult.status === 'fulfilled' ? providersResult.value.total : 0;
    const products      = productsResult.status  === 'fulfilled' ? productsResult.value.items  : [];
    const productTotal  = productsResult.status  === 'fulfilled' ? productsResult.value.total  : 0;

    return { q, providers, products, providerTotal, productTotal };
};
