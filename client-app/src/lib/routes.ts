export const ROUTES = {
    // ── Public auth ───────────────────────────────────────────────────────────
    HOME:            '/',
    LOGIN:           '/login',
    LOGIN_RESET:     '/login?reset=1',
    REGISTER:        '/register',
    FORGOT_PASSWORD: '/forgot-password',
    RESET_PASSWORD:  '/reset-password',
    // Handled by Rust: ends the session and clears both session cookies.
    LOGOUT:          '/logout',

    // ── First-run setup wizard ────────────────────────────────────────────────
    SETUP:           '/setup',
    SETUP_SEED:      '/setup/seed',

    // ── App (authenticated / public) ─────────────────────────────────────────
    ACCOUNT:          '/account',
    ACCOUNT_SECURITY: '/account#security',
    EXPLORE:          '/explore',
    PRODUCTS:         '/products',
    SERVICES:         '/services',
    SEARCH:           '/search',
    PROVIDER_DASHBOARD: '/provider/dashboard',

    // ── Dynamic app routes ───────────────────────────────────────────────────
    PRODUCT:          (id: string | number, slug: string) => `/products/${id}/${slug}`,
    PRODUCT_BY_ID:    (id: string | number)               => `/products/${id}`,
    PRODUCT_CATEGORY: (id: string | number, slug: string) => `/products/category/${id}/${slug}`,
    SERVICE_CATEGORY: (id: string | number, slug: string) => `/services/category/${id}/${slug}`,
    PROVIDER:         (id: string | number, slug: string) => `/providers/${id}/${slug}`,
    PROVIDER_BY_ID:   (id: string | number)               => `/providers/${id}`,

    // ── Admin ─────────────────────────────────────────────────────────────────
    ADMIN: {
        DASHBOARD:   '/admin/dashboard',
        USERS:       '/admin/users',
        USER:        (id: string | number) => `/admin/users/${id}`,
        ROLES:       '/admin/roles',
        ROLE:        (id: string | number) => `/admin/roles/${id}`,
        CATEGORIES:  '/admin/categories',
        CATEGORY:    (id: string | number) => `/admin/categories/${id}`,
        EXPLORE:     '/admin/explore',
        PROVIDER:    (id: string | number) => `/admin/explore/${id}`,
        INQUIRIES:   '/admin/inquiries',
        REVIEWS:     '/admin/reviews',
        LOCATIONS:   '/admin/locations',
        LOCATION:    (id: string | number) => `/admin/locations/${id}`,
        PRODUCTS:    '/admin/products',
        SERVICES:    '/admin/services',
        PERMISSIONS: '/admin/permissions',
        LOGOUT:      '/admin/logout',
    },
};
