import { API_BASE, parseServiceError, type Fetch } from '$lib/server/api';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AdminStats {
    total_businesses:    number;
    verified_businesses: number;
    active_products:     number;
    total_users:         number;
}

export interface AdminUser {
    id:               number;
    username:         string;
    email:            string;
    full_name:        string | null;
    role_code:        string;
    role_name:        string;
    is_active:        boolean;
    created_on:       string;
    updated_on:       string;
    created_by_name:  string | null;
    updated_by_name:  string | null;
}

export interface AdminUserDetail {
    id:         number;
    username:   string;
    email:      string;
    full_name:  string | null;
    role_code:  string;
    role_name:  string;
    is_active:  boolean;
    created_on: string;
}

export interface AdminUserPaged {
    items:       AdminUser[];
    total:       number;
    page:        number;
    total_pages: number;
}

export interface Role {
    id:          number;
    code:        string;
    name:        string;
    description: string | null;
    is_active:   boolean;
}

export interface AdminCategory {
    id:               number;
    name:             string;
    slug:             string;
    category_type:    string;
    icon:             string | null;
    display_order:    number;
    is_active:        boolean;
    created_on:       string;
    updated_on:       string;
    created_by_name:  string | null;
    updated_by_name:  string | null;
}

export interface AdminCategoryDetail extends AdminCategory {}

export interface AdminProvider {
    id:               number;
    name:             string;
    phone:            string;
    address:          string | null;
    is_verified:      boolean;
    is_active:        boolean;
    service_count:    number;
    product_count:    number;
    created_on:       string;
    updated_on:       string;
    linked_username:  string | null;
    created_by_name:  string | null;
    updated_by_name:  string | null;
}

export interface AdminProviderPaged {
    items:       AdminProvider[];
    total:       number;
    page:        number;
    total_pages: number;
}

export interface AdminServiceItem {
    id:         number;
    name:       string;
    category:   string;
    price_note: string | null;
    is_active:  boolean;
}

export interface AdminProductItem {
    id:        number;
    name:      string;
    price:     number | null;
    unit:      string | null;
    category:  string;
    is_active: boolean;
}

export interface AdminProviderDetail {
    id:              number;
    name:            string;
    phone:           string;
    email:           string | null;
    description:     string | null;
    address:         string | null;
    is_verified:     boolean;
    is_active:       boolean;
    created_on:      string;
    linked_username: string | null;
    services:        AdminServiceItem[];
    products:        AdminProductItem[];
}

export interface AdminInquiry {
    id:              number;
    business_id:     number;
    business_name:   string;
    requester_name:  string;
    requester_phone: string;
    topic:           string;
    status:          string;
    created_on:      string;
    updated_on:      string;
    updated_by_name: string | null;
}

export interface AdminInquiryPaged {
    items:       AdminInquiry[];
    total:       number;
    page:        number;
    total_pages: number;
}

export interface AdminReview {
    id:              number;
    business_id:     number;
    business_name:   string;
    reviewer_name:   string;
    rating:          number;
    comment:         string | null;
    is_visible:      boolean;
    created_on:      string;
    updated_on:      string;
    updated_by_name: string | null;
}

export interface AdminReviewPaged {
    items:       AdminReview[];
    total:       number;
    page:        number;
    total_pages: number;
}

// ── Service ───────────────────────────────────────────────────────────────────

export class AdminService {
    private readonly auth: Record<string, string>;

    constructor(private readonly fetch: Fetch, token: string) {
        this.auth = { Authorization: `Bearer ${token}` };
    }

    // ── Stats ──────────────────────────────────────────────────────────────

    async getStats(): Promise<AdminStats> {
        const res = await this.fetch(`${API_BASE}/api/admin/stats`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load stats.');
        return res.json();
    }

    // ── Users ──────────────────────────────────────────────────────────────

    async listUsers(page: string): Promise<AdminUserPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/users?page=${page}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load users.');
        return res.json();
    }

    async getUser(id: number): Promise<AdminUserDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load user.');
        return res.json();
    }

    async updateUserRole(id: number, roleCode: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}/role`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ role_code: roleCode }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update role.');
    }

    async toggleUserActive(id: number, isActive: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}/active`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_active: isActive }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update status.');
    }

    // ── Roles ──────────────────────────────────────────────────────────────

    async listRoles(): Promise<Role[]> {
        const res = await this.fetch(`${API_BASE}/api/admin/roles`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load roles.');
        return res.json();
    }

    async getRole(id: number): Promise<Role> {
        const res = await this.fetch(`${API_BASE}/api/admin/roles/${id}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load role.');
        return res.json();
    }

    // ── Categories ─────────────────────────────────────────────────────────

    async listCategories(): Promise<AdminCategory[]> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load categories.');
        return res.json();
    }

    async getCategory(id: number): Promise<AdminCategoryDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories/${id}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load category.');
        return res.json();
    }

    async createCategory(data: {
        name: string; slug: string; category_type: string;
        icon: string | null; display_order: number;
    }): Promise<AdminCategory> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories`, {
            method:  'POST',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to create category.');
        return res.json();
    }

    async updateCategory(id: number, data: {
        name: string; slug: string; icon: string | null; display_order: number;
    }): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories/${id}`, {
            method:  'PUT',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update category.');
    }

    async toggleCategoryActive(id: number, isActive: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories/${id}/active`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_active: isActive }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update status.');
    }

    // ── Providers ──────────────────────────────────────────────────────────

    async listProviders(page: string): Promise<AdminProviderPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers?page=${page}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load providers.');
        return res.json();
    }

    async getProvider(id: number): Promise<AdminProviderDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers/${id}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load provider.');
        return res.json();
    }

    async toggleProviderVerification(id: number): Promise<{ message: string; is_verified: boolean }> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers/${id}/verification`, {
            method:  'PATCH',
            headers: this.auth,
        });
        if (!res.ok) throw await parseServiceError(res, 'Update failed.');
        return res.json();
    }

    async assignProviderUser(id: number, data: {
        username: string; password: string; email?: string;
    }): Promise<{ message: string }> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers/${id}/users`, {
            method:  'POST',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to assign account.');
        return res.json();
    }

    // ── Inquiries ──────────────────────────────────────────────────────────

    async listInquiries(page: string): Promise<AdminInquiryPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/inquiries?page=${page}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load inquiries.');
        return res.json();
    }

    async updateInquiryStatus(id: number, status: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/inquiries/${id}/status`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ status }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update status.');
    }

    // ── Reviews ────────────────────────────────────────────────────────────

    async listReviews(page: string): Promise<AdminReviewPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/reviews?page=${page}`, { headers: this.auth });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load reviews.');
        return res.json();
    }

    async toggleReviewVisible(id: number, isVisible: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/reviews/${id}/visible`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_visible: isVisible }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to update visibility.');
    }
}
