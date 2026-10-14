import { API_BASE, parseServiceError, type Fetch } from '$lib/server/api';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ProviderProfile {
    id:          number;
    name:        string;
    phone:       string;
    email:       string | null;
    description: string | null;
    address:     string | null;
    is_verified: boolean;
    lat:         number | null;
    lng:         number | null;
}

export interface ProviderProduct {
    id:          number;
    name:        string;
    description: string | null;
    price:       number | null;
    unit:        string | null;
    image_url:   string | null;
    category:    string;
    category_id: number;
    is_active:   boolean;
    created_on:  string;
}

// ── Service ───────────────────────────────────────────────────────────────────

export class ProviderService {
    private readonly auth: Record<string, string>;

    constructor(private readonly fetch: Fetch, token: string) {
        this.auth = { Authorization: `Bearer ${token}` };
    }

    async getProfile(): Promise<ProviderProfile | null> {
        const res = await this.fetch(`${API_BASE}/api/provider/profile`, { headers: this.auth });
        if (res.status === 404) return null;
        if (!res.ok) throw await parseServiceError(res, 'Failed to load profile.');
        return res.json();
    }

    async updateProfile(data: {
        name:        string;
        phone:       string;
        description: string | null;
        address:     string | null;
        lat:         number | null;
        lng:         number | null;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/profile`, {
            method:  'PUT',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Update failed.');
    }

    async listProducts(): Promise<ProviderProduct[]> {
        const res = await this.fetch(`${API_BASE}/api/provider/products`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load products.');
        return res.json();
    }

    async createProduct(data: {
        category_id: number;
        name:        string;
        description: string | null;
        price:       number | null;
        unit:        string | null;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products`, {
            method:  'POST',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to add product.');
    }

    async updateProduct(id: number, data: {
        category_id: number;
        name:        string;
        description: string | null;
        price:       number | null;
        unit:        string | null;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products/${id}`, {
            method:  'PUT',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update product.');
    }

    async deleteProduct(id: number): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products/${id}`, {
            method:  'DELETE',
            headers: this.auth,
        });
        if (!res.ok) throw await parseServiceError(res, 'Delete failed.');
    }
}
