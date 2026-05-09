import { API_BASE, ServiceError, type Fetch } from '$lib/server/api';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AdminStats {
    total_businesses:    number;
    verified_businesses: number;
    active_products:     number;
    total_users:         number;
}

export interface AdminUser {
    id:        number;
    username:  string;
    email:     string;
    full_name: string | null;
    role_code: string;
    role_name: string;
    is_active: boolean;
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
    id:            number;
    name:          string;
    slug:          string;
    category_type: string;
    icon:          string | null;
    display_order: number;
    is_active:     boolean;
}

export interface AdminCategoryDetail extends AdminCategory {}

export interface AdminProvider {
    id:              number;
    name:            string;
    phone:           string;
    address:         string | null;
    is_verified:     boolean;
    is_active:       boolean;
    service_count:   number;
    product_count:   number;
    created_on:      string;
    linked_username: string | null;
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
}

export interface AdminInquiryPaged {
    items:       AdminInquiry[];
    total:       number;
    page:        number;
    total_pages: number;
}

export interface AdminReview {
    id:            number;
    business_id:   number;
    business_name: string;
    reviewer_name: string;
    rating:        number;
    comment:       string | null;
    is_visible:    boolean;
    created_on:    string;
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
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải thống kê.');
        return res.json();
    }

    // ── Users ──────────────────────────────────────────────────────────────

    async listUsers(page: string): Promise<AdminUserPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/users?page=${page}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách người dùng.');
        return res.json();
    }

    async getUser(id: number): Promise<AdminUserDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải thông tin người dùng.');
        return res.json();
    }

    async updateUserRole(id: number, roleCode: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}/role`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ role_code: roleCode }),
        });
        if (!res.ok) throw new ServiceError(res.status, 'Cập nhật vai trò thất bại.');
    }

    async toggleUserActive(id: number, isActive: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/users/${id}/active`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_active: isActive }),
        });
        if (!res.ok) throw new ServiceError(res.status, 'Cập nhật trạng thái thất bại.');
    }

    // ── Roles ──────────────────────────────────────────────────────────────

    async listRoles(): Promise<Role[]> {
        const res = await this.fetch(`${API_BASE}/api/admin/roles`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách vai trò.');
        return res.json();
    }

    async getRole(id: number): Promise<Role> {
        const res = await this.fetch(`${API_BASE}/api/admin/roles/${id}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải thông tin vai trò.');
        return res.json();
    }

    // ── Categories ─────────────────────────────────────────────────────────

    async listCategories(): Promise<AdminCategory[]> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh mục.');
        return res.json();
    }

    async getCategory(id: number): Promise<AdminCategoryDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories/${id}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải thông tin danh mục.');
        return res.json();
    }

    async createCategory(data: {
        name: string; slug: string; category_type: string;
        icon: string | null; display_order: number;
    }): Promise<{ id: number }> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories`, {
            method:  'POST',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Tạo danh mục thất bại.');
        }
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
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Cập nhật danh mục thất bại.');
        }
    }

    async toggleCategoryActive(id: number, isActive: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/categories/${id}/active`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_active: isActive }),
        });
        if (!res.ok) throw new ServiceError(res.status, 'Cập nhật trạng thái thất bại.');
    }

    // ── Providers ──────────────────────────────────────────────────────────

    async listProviders(page: string): Promise<AdminProviderPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers?page=${page}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách nhà cung cấp.');
        return res.json();
    }

    async getProvider(id: number): Promise<AdminProviderDetail> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers/${id}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải thông tin nhà cung cấp.');
        return res.json();
    }

    async toggleProviderVerification(id: number): Promise<{ message: string; is_verified: boolean }> {
        const res = await this.fetch(`${API_BASE}/api/admin/providers/${id}/verification`, {
            method:  'PATCH',
            headers: this.auth,
        });
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Cập nhật thất bại.');
        }
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
        if (!res.ok) {
            const body = await res.json().catch(() => ({}));
            throw new ServiceError(res.status, (body as { error?: string }).error ?? 'Gán tài khoản thất bại.');
        }
        return res.json();
    }

    // ── Inquiries ──────────────────────────────────────────────────────────

    async listInquiries(page: string): Promise<AdminInquiryPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/inquiries?page=${page}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách yêu cầu.');
        return res.json();
    }

    async updateInquiryStatus(id: number, status: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/inquiries/${id}/status`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ status }),
        });
        if (!res.ok) throw new ServiceError(res.status, 'Cập nhật trạng thái thất bại.');
    }

    // ── Reviews ────────────────────────────────────────────────────────────

    async listReviews(page: string): Promise<AdminReviewPaged> {
        const res = await this.fetch(`${API_BASE}/api/admin/reviews?page=${page}`, { headers: this.auth });
        if (!res.ok) throw new ServiceError(res.status, 'Không thể tải danh sách đánh giá.');
        return res.json();
    }

    async toggleReviewVisible(id: number, isVisible: boolean): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/admin/reviews/${id}/visible`, {
            method:  'PATCH',
            headers: { ...this.auth, 'Content-Type': 'application/json' },
            body:    JSON.stringify({ is_visible: isVisible }),
        });
        if (!res.ok) throw new ServiceError(res.status, 'Cập nhật hiển thị thất bại.');
    }
}
