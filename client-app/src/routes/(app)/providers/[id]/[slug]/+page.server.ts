import { error, fail } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';
import { ServiceError, PHONE_REGEX } from '$lib/server/api';

export type { ProviderDetail, ProviderService } from '$lib/server/services/explore.service';

type FieldErrors = Record<string, string>;

function validateContactForm(
    requester_name: string,
    requester_phone: string,
    topic: string,
): FieldErrors | null {
    const errors: FieldErrors = {};
    if (!requester_name) errors.requester_name = 'Vui lòng nhập họ tên của bạn.';
    if (!requester_phone || !PHONE_REGEX.test(requester_phone))
        errors.requester_phone = 'Số điện thoại không hợp lệ (ví dụ: 0901234567).';
    if (!topic) errors.topic = 'Vui lòng chọn dịch vụ cần liên hệ.';
    return Object.keys(errors).length ? errors : null;
}

export const load: PageServerLoad = async ({ params, fetch }) => {
    const svc = new ExploreService(fetch);
    try {
        const provider = await svc.getProvider(params.id);
        return { provider };
    } catch (err) {
        if (err instanceof ServiceError && err.status === 404) throw error(404, 'Nhà cung cấp không tồn tại');
        throw error(500, 'Không thể tải thông tin nhà cung cấp');
    }
};

export const actions = {
    contact_request: async ({ request, fetch }) => {
        const form            = await request.formData();
        const business_id     = Number(form.get('business_id'));
        const requester_name  = String(form.get('requester_name')  ?? '').trim();
        const requester_phone = String(form.get('requester_phone') ?? '').trim();
        const topic           = String(form.get('topic') ?? '').trim();
        const note            = String(form.get('note')  ?? '').trim();

        const values = { requester_name, requester_phone, topic, note };

        const field_errors = validateContactForm(requester_name, requester_phone, topic);
        if (field_errors) {
            return fail(400, { error: 'Vui lòng kiểm tra lại thông tin.', field_errors, values });
        }

        const svc = new ExploreService(fetch);
        try {
            await svc.submitContact({ business_id, requester_name, requester_phone, topic, note: note || null });
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                return fail(err.status, { error: err.message, field_errors: err.fieldErrors ?? null, values });
            }
            return fail(500, { error: 'Máy chủ không phản hồi. Vui lòng thử lại.', values });
        }
    },
} satisfies Actions;
