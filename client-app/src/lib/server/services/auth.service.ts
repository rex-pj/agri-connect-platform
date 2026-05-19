import { API_BASE, parseServiceError, type Fetch } from '$lib/server/api';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface LoginResult {
    token:    string;
    username: string;
    role:     string;
}

export interface MeResponse {
    username:   string;
    email:      string;
    full_name:  string | null;
    created_on: string;
}

export interface SetupStatus {
    user_count: number;
}

// ── Service ───────────────────────────────────────────────────────────────────

export class AuthService {
    constructor(private readonly fetch: Fetch) {}

    async getSetupStatus(): Promise<SetupStatus | null> {
        try {
            const res = await this.fetch(`${API_BASE}/api/setup/status`);
            if (!res.ok) return null;
            return res.json();
        } catch {
            return null;
        }
    }

    async login(username: string, password: string): Promise<LoginResult> {
        const res = await this.fetch(`${API_BASE}/api/auth/login`, {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ username, password }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Login failed.');
        return res.json();
    }

    async register(data: {
        username:       string;
        email:          string;
        password:       string;
        full_name:      string | null;
        business_name:  string;
        business_phone: string;
    }): Promise<LoginResult> {
        const res = await this.fetch(`${API_BASE}/api/auth/register`, {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Registration failed.');
        return res.json();
    }

    async createFirstAdmin(data: {
        username:  string;
        email:     string;
        password:  string;
        full_name: string | null;
    }): Promise<LoginResult> {
        const res = await this.fetch(`${API_BASE}/api/admin/initialize`, {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify(data),
        });
        if (!res.ok) throw await parseServiceError(res, 'Account creation failed.');
        return res.json();
    }

    async me(token: string): Promise<MeResponse> {
        const res = await this.fetch(`${API_BASE}/api/users/me`, {
            headers: { Authorization: `Bearer ${token}` },
        });
        if (!res.ok) throw await parseServiceError(res, 'Failed to load user profile.');
        return res.json();
    }

    async changePassword(token: string, currentPassword: string, newPassword: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/users/me/password`, {
            method:  'PATCH',
            headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
            body:    JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Password change failed.');
    }

    async forgotPassword(email: string): Promise<void> {
        await this.fetch(`${API_BASE}/api/auth/password-resets`, {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ email }),
        });
        // Intentionally ignore errors — never reveal whether the email is registered.
    }

    async resetPassword(token: string, newPassword: string): Promise<void> {
        const res = await this.fetch(`${API_BASE}/api/auth/password-resets/${token}`, {
            method:  'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ new_password: newPassword }),
        });
        if (!res.ok) throw await parseServiceError(res, 'Password reset failed.');
    }
}
