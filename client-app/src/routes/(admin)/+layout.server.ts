import type { LayoutServerLoad } from './$types';

// Outer (admin) layout: no auth guard.
// The login page lives here (unauthenticated).
// Inner layouts at (admin)/admin/(cp)/ carry the auth guard + AdminShell.
export const load: LayoutServerLoad = async () => {
    return {};
};
