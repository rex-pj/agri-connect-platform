export const ROUTES = {
    // ── Public auth ───────────────────────────────────────────────────────────
    HOME:            '/',
    LOGIN:           '/login',
    LOGIN_RESET:     '/login?reset=1',
    REGISTER:        '/register',
    FORGOT_PASSWORD: '/forgot-password',
    RESET_PASSWORD:  '/reset-password',

    // ── App (authenticated / public) ─────────────────────────────────────────
    ACCOUNT:          '/account',
    ACCOUNT_SECURITY: '/account#security',
    AGRI_CONNECT:     '/agri-connect',
    PRODUCTS:         '/products',
    SERVICES:         '/services',
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
        AGRI_CONNECT: '/admin/agri-connect',
        PROVIDER:    (id: string | number) => `/admin/agri-connect/${id}`,
        INQUIRIES:   '/admin/inquiries',
        REVIEWS:     '/admin/reviews',
        LOGOUT:      '/admin/logout',
    },
};
