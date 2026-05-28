/**
 * Auth guard for every route under /admin/*.
 *
 * Strategy (defence in depth):
 *  1. Redirect to /login immediately if no auth_token cookie exists.
 *  2. Call GET /api/users/me to validate the token with the backend server.
 *     - 401  → token expired/invalid → clear cookie and redirect.
 *     - Any other error → fail secure and redirect (e.g. backend unreachable).
 *  3. Check that the returned role is "admin".
 *     A logged-in provider who knows the admin URL is redirected to /login.
 *
 * Data returned here (`username`, `role`) is merged into every child layout's
 * and page's `data` object by SvelteKit, so sub-layouts need not repeat the
 * lookup.
 *
 * Note: The backend's RequireAdmin extractor is the authoritative security
 * boundary. This guard prevents providers from seeing the admin UI shell and
 * avoids unnecessary API calls from non-admin sessions, but it is NOT the
 * sole line of defence.
 */
import { isRedirect, redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { AuthService } from '$lib/server/services/auth.service';
import { ServiceError } from '$lib/server/api';
import { ROUTES } from '$lib/routes';

export const load: LayoutServerLoad = async ({ cookies, fetch }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.LOGIN);

    try {
        const me = await new AuthService(fetch).me(token);

        if (me.role !== 'admin') {
            // Valid token, wrong role (e.g. a provider). Clear and redirect so
            // the browser's next visit to /login starts from a clean state.
            cookies.delete('auth_token', { path: '/' });
            throw redirect(302, ROUTES.LOGIN);
        }

        return { username: me.username, role: me.role };
    } catch (e) {
        // Re-throw SvelteKit redirects (thrown from inside the try block above).
        if (isRedirect(e)) throw e;

        if (e instanceof ServiceError && e.status === 401) {
            cookies.delete('auth_token', { path: '/' });
        }

        // Fail secure: any unexpected error (backend unreachable, bad JSON,
        // network timeout) sends the user back to the login page.
        throw redirect(302, ROUTES.LOGIN);
    }
};
