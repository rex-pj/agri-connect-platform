import type { PageServerLoad } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';
import type { Category } from '$lib/server/services/explore.service';

export type { FeaturedProvider, FeaturedProduct } from '$lib/server/services/explore.service';

export interface NavCategory {
    name:  string;
    icon:  string;
    href:  string;
    slug?: string;
}

const NAV_SHORTCUTS: NavCategory[] = [
    { name: 'Dịch vụ', icon: 'fa-border-all',   href: '/services' },
    { name: 'Gần tôi',  icon: 'fa-location-dot', href: '/explore' },
];

export const load: PageServerLoad = async ({ fetch }) => {
    const svc = new ExploreService(fetch);

    const [exploreResult, catResult] = await Promise.allSettled([
        svc.getExploreData(),
        svc.getCategories('product'),
    ]);

    const featuredProducts  = exploreResult.status === 'fulfilled' ? exploreResult.value.featured_products  : [];
    const featuredProviders = exploreResult.status === 'fulfilled' ? exploreResult.value.featured_providers : [];

    let categories: NavCategory[] = NAV_SHORTCUTS;
    if (catResult.status === 'fulfilled') {
        const dbCats: Category[] = catResult.value;
        categories = [
            ...dbCats.map(c => ({
                name: c.name,
                slug: c.slug,
                icon: '',
                href: `/products/category/${c.id}/${c.slug}`,
            })),
            ...NAV_SHORTCUTS,
        ];
    }

    return { categories, featuredProducts, featuredProviders };
};
