import { redirect } from '@sveltejs/kit';
import { PRIVATE_API_URL } from '$env/static/private';
import type { Cookies } from '@sveltejs/kit';
import { ROUTES } from '$lib/routes';

// ── Base URL ──────────────────────────────────────────────────────────────────

export const API_BASE = PRIVATE_API_URL ?? 'http://localhost:3000';

// ── Auth cookie ───────────────────────────────────────────────────────────────

/** Matches JWT expiry configured in the backend (constants::JWT_EXPIRY_HOURS). */
export const AUTH_COOKIE_MAX_AGE = 60 * 60; // 1 hour

const IS_PROD = process.env.NODE_ENV === 'production';

export function authCookieOptions() {
    return {
        path:     '/' as const,
        httpOnly: true,
        sameSite: 'strict' as const,
        secure:   IS_PROD,
        maxAge:   AUTH_COOKIE_MAX_AGE,
    };
}

export function setAuthCookie(cookies: Cookies, token: string): void {
    cookies.set('auth_token', token, authCookieOptions());
}

// ── Request helpers ───────────────────────────────────────────────────────────

export function authHeader(cookies: Cookies): string {
    return `Bearer ${cookies.get('auth_token') ?? ''}`;
}

/** Redirects to /login when the backend returns 401 or 403. */
export function checkAuth(status: number): void {
    if (status === 401 || status === 403) throw redirect(302, ROUTES.LOGIN);
}

// ── Validation ────────────────────────────────────────────────────────────────

/** 10-digit Vietnamese mobile number starting with 0. */
export const PHONE_REGEX = /^0\d{9}$/;

/** Minimum password length — keep in sync with backend constants::PASSWORD_MIN_LEN. */
export const PASSWORD_MIN_LEN = 8;

// ── Response parsing helper ───────────────────────────────────────────────────

type ApiErrorBody = {
    message?: string;
    field_errors?: Record<string, string[]>;
};

/** Extracts the `message` field from a JSON response body, with a fallback. */
export async function extractError(res: Response, fallback: string): Promise<string> {
    const body = await res.json().catch(() => ({})) as ApiErrorBody;
    return body.message ?? fallback;
}

/** Converts backend `field_errors: Record<string, string[]>` to a flat map (first error per field). */
export function flattenFieldErrors(
    raw: Record<string, string[]> | undefined,
): Record<string, string> | undefined {
    if (!raw) return undefined;
    return Object.fromEntries(
        Object.entries(raw).map(([k, v]) => [k, v[0] ?? '']),
    );
}

// ── Service error ─────────────────────────────────────────────────────────────

/** Thrown by service classes when the backend returns a non-2xx response. */
export class ServiceError extends Error {
    constructor(
        public readonly status: number,
        message: string,
        public readonly fieldErrors?: Record<string, string>,
    ) {
        super(message);
        this.name = 'ServiceError';
    }
}

/** Parse a non-2xx response body into a ServiceError. */
export async function parseServiceError(res: Response, fallback: string): Promise<ServiceError> {
    const body = await res.json().catch(() => ({})) as ApiErrorBody;
    return new ServiceError(
        res.status,
        body.message ?? fallback,
        flattenFieldErrors(body.field_errors),
    );
}

export type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
