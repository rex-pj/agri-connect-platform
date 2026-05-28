import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { ExploreService } from '$lib/server/services/explore.service';
import { ServiceError, PHONE_REGEX } from '$lib/server/api';

export type { Category, Location, Provider, Product } from '$lib/server/services/explore.service';

// ── Load ──────────────────────────────────────────────────────────────────────

export const load: PageServerLoad = async ({ fetch, url }) => {
    const lat       = url.searchParams.get('lat');
    const lng       = url.searchParams.get('lng');
    const radius    = url.searchParams.get('radius')     ?? '10000';
    const serviceId = url.searchParams.get('service_id') ?? '';

    const svc = new ExploreService(fetch);

    const [exploreResult, svcCatResult, prodCatResult, locationsResult] = await Promise.allSettled([
        svc.getExploreData(),
        svc.getCategories('service'),
        svc.getCategories('product'),
        svc.getLocations(),
    ]);

    const featured_providers = exploreResult.status  === 'fulfilled' ? exploreResult.value.featured_providers : [];
    const featured_products  = exploreResult.status  === 'fulfilled' ? exploreResult.value.featured_products  : [];
    const serviceCategories  = svcCatResult.status   === 'fulfilled' ? svcCatResult.value   : [];
    const productCategories  = prodCatResult.status  === 'fulfilled' ? prodCatResult.value  : [];
    const locations          = locationsResult.status === 'fulfilled' ? locationsResult.value : [];

    if (!lat || !lng) {
        return {
            hasLocation: false,
            providers:   [],
            error:       null,
            featured_providers,
            featured_products,
            serviceCategories,
            productCategories,
            locations,
        };
    }

    try {
        const providers = await svc.searchProviders({
            lat, lng, radius, ...(serviceId ? { service_id: serviceId } : {}),
        });
        return {
            hasLocation: true,
            providers,
            error: null,
            featured_providers,
            featured_products,
            serviceCategories,
            productCategories,
            locations,
        };
    } catch (err) {
        const message = err instanceof ServiceError
            ? err.message
            : 'Máy chủ không phản hồi. Vui lòng thử lại.';
        return {
            hasLocation: true,
            providers:   [],
            error:       message,
            featured_providers,
            featured_products,
            serviceCategories,
            productCategories,
            locations,
        };
    }
};

// ── Helpers ───────────────────────────────────────────────────────────────────

type FieldErrors = Record<string, string>;

function validateContactForm(
    requester_name: string,
    requester_phone: string,
    topic: string,
    note: string,
): FieldErrors | null {
    const errors: FieldErrors = {};
    if (!requester_name)
        errors.requester_name = 'Vui lòng nhập họ tên của bạn.';
    else if (requester_name.length > 100)
        errors.requester_name = 'Họ tên tối đa 100 ký tự.';
    if (!requester_phone || !PHONE_REGEX.test(requester_phone))
        errors.requester_phone = 'Số điện thoại không hợp lệ (ví dụ: 0901234567).';
    if (!topic)
        errors.topic = 'Vui lòng chọn dịch vụ cần liên hệ.';
    else if (topic.length > 255)
        errors.topic = 'Chủ đề tối đa 255 ký tự.';
    if (note.length > 1000)
        errors.note = 'Ghi chú tối đa 1000 ký tự.';
    return Object.keys(errors).length ? errors : null;
}

// ── Actions ───────────────────────────────────────────────────────────────────

export const actions: Actions = {
    contact_request: async ({ request, fetch }) => {
        const data = await request.formData();

        const business_id     = data.get('business_id');
        const requester_name  = (data.get('requester_name')  as string ?? '').trim();
        const requester_phone = (data.get('requester_phone') as string ?? '').trim();
        const topic           = (data.get('topic')           as string ?? '').trim();
        const note            = (data.get('note')            as string ?? '').trim();

        const values = { requester_name, requester_phone, topic, note };

        if (!business_id) {
            return fail(400, { error: 'Thiếu thông tin nhà cung cấp.', values });
        }

        const field_errors = validateContactForm(requester_name, requester_phone, topic, note);
        if (field_errors) {
            return fail(400, { error: 'Vui lòng kiểm tra lại thông tin.', field_errors, values });
        }

        const svc = new ExploreService(fetch);
        try {
            await svc.submitContact({
                business_id:    Number(business_id),
                topic,
                requester_name,
                requester_phone,
                note: note || null,
            });
            return { success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                return fail(err.status, { error: err.message, field_errors: err.fieldErrors ?? null, values });
            }
            return fail(500, { error: 'Máy chủ không phản hồi. Vui lòng thử lại.', values });
        }
    },
};
