import { API_BASE, parseServiceError, type Fetch } from '$lib/server/api';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface Category {
    id:            number;
    name:          string;
    slug:          string;
    icon:          string | null;
    display_order: number;
}

export interface Provider {
    id:          number;
    name:        string;
    phone:       string;
    address:     string | null;
    services:    string[];
    is_verified: boolean;
    distance_m?: number;
}

export interface BrowseProvider {
    id:          number;
    name:        string;
    phone:       string;
    address:     string | null;
    services:    string[];
    is_verified: boolean;
}

export interface ProviderService {
    id:          number;
    name:        string;
    description: string | null;
    price_note:  string | null;
    category:    string;
}

export interface ProviderDetail {
    id:          number;
    name:        string;
    phone:       string;
    description: string | null;
    address:     string | null;
    is_verified: boolean;
    services:    ProviderService[];
}

export interface BrowseResult {
    items:       BrowseProvider[];
    total:       number;
    page:        number;
    total_pages: number;
}

export interface Product {
    id:           number;
    name:         string;
    description:  string | null;
    price:        number | null;
    unit:         string | null;
    image_url:    string | null;
    category:     string;
    vendor_name:  string;
    vendor_phone: string;
    is_available: boolean;
}

export interface ProductDetail extends Product {
    created_on: string;
}

export interface ProductsResult {
    items:       Product[];
    total:       number;
    page:        number;
    total_pages: number;
}

export type FeaturedProvider = Provider;
export type FeaturedProduct  = Product;

export interface ExploreData {
    featured_providers: FeaturedProvider[];
    featured_products:  FeaturedProduct[];
}

export interface ContactInput {
    business_id:     number;
    topic:           string;
    requester_name:  string;
    requester_phone: string;
    note:            string | null;
    service_id?:     number | null;
}

// ── Service ───────────────────────────────────────────────────────────────────

export class AgriConnectService {
    constructor(private readonly fetch: Fetch) {}

    async getExploreData(): Promise<ExploreData> {
        const res = await this.fetch(`${API_BASE}/api/agri-connect/explore`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load home data.');
        return res.json();
    }

    async getCategories(type: 'product' | 'service'): Promise<Category[]> {
        const res = await this.fetch(`${API_BASE}/api/agri-connect/categories?type=${type}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load categories.');
        return res.json();
    }

    async searchProviders(params: {
        lat: string; lng: string; radius: string; service_id?: string;
    }): Promise<Provider[]> {
        const qs = new URLSearchParams({ lat: params.lat, lng: params.lng, radius: params.radius });
        if (params.service_id) qs.set('service_id', params.service_id);
        const res = await this.fetch(`${API_BASE}/api/agri-connect/providers?${qs}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load providers.');
        return res.json();
    }

    async browseProviders(page: string, serviceId?: string, name?: string, sort?: string): Promise<BrowseResult> {
        const qs = new URLSearchParams({ page });
        if (serviceId) qs.set('service_id', serviceId);
        if (name)      qs.set('name', name);
        if (sort)      qs.set('sort', sort);
        const res = await this.fetch(`${API_BASE}/api/agri-connect/providers/browse?${qs}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load providers.');
        return res.json();
    }

    async getProvider(id: number | string): Promise<ProviderDetail> {
        const res = await this.fetch(`${API_BASE}/api/agri-connect/providers/${id}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load provider.');
        return res.json();
    }

    async browseProducts(
        page: string,
        categoryId?: string,
        name?: string,
        minPrice?: string,
        maxPrice?: string,
        sort?: string,
    ): Promise<ProductsResult> {
        const qs = new URLSearchParams({ page });
        if (categoryId) qs.set('category_id', categoryId);
        if (name)       qs.set('name', name);
        if (minPrice)   qs.set('min_price', minPrice);
        if (maxPrice)   qs.set('max_price', maxPrice);
        if (sort)       qs.set('sort', sort);
        const res = await this.fetch(`${API_BASE}/api/agri-connect/products/browse?${qs}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load products.');
        return res.json();
    }

    async getProduct(id: number | string): Promise<ProductDetail> {
        const res = await this.fetch(`${API_BASE}/api/agri-connect/products/${id}`);
        if (!res.ok) throw await parseServiceError(res, 'Failed to load product.');
        return res.json();
    }

    async submitContact(data: ContactInput): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/agri-connect/contact`, {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to submit request.');
    }
}
