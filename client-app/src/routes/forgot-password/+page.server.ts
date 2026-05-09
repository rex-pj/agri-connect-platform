import { fail } from '@sveltejs/kit';
import type { Actions } from './$types';
import { AuthService } from '$lib/server/services/auth.service';

export const actions: Actions = {
    default: async ({ request, fetch }) => {
        const data  = await request.formData();
        const email = (data.get('email') as string ?? '').trim().toLowerCase();

        if (!email) return fail(400, { error: 'Vui lòng nhập địa chỉ email.' });

        // Always return success — never reveal whether the email is registered.
        const svc = new AuthService(fetch);
        await svc.forgotPassword(email);

        return { sent: true };
    },
};
