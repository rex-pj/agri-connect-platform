import { API_BASE, ServiceError, type Fetch } from '$lib/server/api';

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
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải hồ sơ.');
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
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Cập nhật thất bại.');
        }
    }

    async listProducts(): Promise<ProviderProduct[]> {
        const res = await this.fetch(`${API_BASE}/api/provider/products`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách sản phẩm.');
        return res.json();
    }

    async createProduct(data: {
        category_id: number;
        name:        string;
        description: string | null;
        price:       number | null;
        unit:        string | null;
        image_url:   string | null;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products`, {
            method:  'POST',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Thêm sản phẩm thất bại.');
        }
    }

    async updateProduct(id: number, data: {
        category_id: number;
        name:        string;
        description: string | null;
        price:       number | null;
        unit:        string | null;
        image_url:   string | null;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products/${id}`, {
            method:  'PUT',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Cập nhật sản phẩm thất bại.');
        }
    }

    async deleteProduct(id: number): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/provider/products/${id}`, {
            method:  'DELETE',
            headers: this.auth,
        });
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Xoá thất bại.');
        }
    }
}
