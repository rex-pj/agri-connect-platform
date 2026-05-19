import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { ServiceError } from '$lib/server/api';
import { ROUTES } from '$lib/routes';
import { AgriConnectService } from '$lib/server/services/agri-connect.service';
import { ProviderService } from '$lib/server/services/provider.service';

export type { ProviderProfile, ProviderProduct } from '$lib/server/services/provider.service';
export type { Category } from '$lib/server/services/agri-connect.service';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const token = cookies.get('auth_token');
    if (!token) throw redirect(302, ROUTES.LOGIN);

    const providerSvc = new ProviderService(fetch, token);
    const publicSvc   = new AgriConnectService(fetch);

    const [profileResult, productsResult, categoriesResult] = await Promise.allSettled([
        providerSvc.getProfile(),
        providerSvc.listProducts(),
        publicSvc.getCategories('product'),
    ]);

    if (profileResult.status === 'rejected') {
        const err = profileResult.reason;
        if (err instanceof ServiceError && (err.status === 401 || err.status === 403)) {
            throw redirect(302, ROUTES.LOGIN);
        }
    }
    if (productsResult.status === 'rejected') {
        const err = productsResult.reason;
        if (err instanceof ServiceError && (err.status === 401 || err.status === 403)) {
            throw redirect(302, ROUTES.LOGIN);
        }
    }

    const profile      = profileResult.status === 'fulfilled'  ? profileResult.value  : null;
    const products     = productsResult.status === 'fulfilled' ? productsResult.value : [];
    const categories   = categoriesResult.status === 'fulfilled' ? categoriesResult.value : [];
    const profileError = profileResult.status === 'rejected'
        ? (profileResult.reason instanceof ServiceError ? profileResult.reason.message : 'Failed to load profile.')
        : null;

    return { profile, products, categories, profileError };
};

export const actions: Actions = {

    update_profile: async ({ request, fetch, cookies }) => {
        const data = await request.formData();
        const lat  = data.get('lat') as string | null;
        const lng  = data.get('lng') as string | null;

        const body = {
            name:        (data.get('name')        as string ?? '').trim(),
            phone:       (data.get('phone')       as string ?? '').trim(),
            description: (data.get('description') as string ?? '').trim() || null,
            address:     (data.get('address')     as string ?? '').trim() || null,
            lat:  lat && lat.trim()  ? parseFloat(lat)  : null,
            lng:  lng && lng.trim()  ? parseFloat(lng)  : null,
        };

        if (!body.name)  return fail(400, { action: 'update_profile', error: 'Tên không được để trống.' });
        if (!body.phone) return fail(400, { action: 'update_profile', error: 'Số điện thoại không được để trống.' });

        const svc = new ProviderService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateProfile(body);
            return { action: 'update_profile', success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                if (err.status === 401 || err.status === 403) throw redirect(302, ROUTES.LOGIN);
                return fail(err.status, { action: 'update_profile', error: err.message });
            }
            return fail(500, { action: 'update_profile', error: 'Cập nhật thất bại.' });
        }
    },

    add_product: async ({ request, fetch, cookies }) => {
        const data = await request.formData();

        const name        = (data.get('name')        as string ?? '').trim();
        const category_id = Number(data.get('category_id'));
        const priceRaw    = (data.get('price')       as string ?? '').trim();
        const unit        = (data.get('unit')        as string ?? '').trim() || null;
        const description = (data.get('description') as string ?? '').trim() || null;
        const image_url   = (data.get('image_url')   as string ?? '').trim() || null;

        if (!name)        return fail(400, { action: 'add_product', error: 'Tên sản phẩm không được để trống.' });
        if (!category_id) return fail(400, { action: 'add_product', error: 'Vui lòng chọn danh mục.' });

        const price = priceRaw ? parseInt(priceRaw, 10) : null;
        if (price !== null && (isNaN(price) || price <= 0)) {
            return fail(400, { action: 'add_product', error: 'Giá phải là số dương.' });
        }

        const svc = new ProviderService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.createProduct({ category_id, name, description, price, unit, image_url });
            return { action: 'add_product', success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                if (err.status === 401 || err.status === 403) throw redirect(302, ROUTES.LOGIN);
                return fail(err.status, { action: 'add_product', error: err.message });
            }
            return fail(500, { action: 'add_product', error: 'Thêm sản phẩm thất bại.' });
        }
    },

    edit_product: async ({ request, fetch, cookies }) => {
        const data = await request.formData();

        const id          = Number(data.get('id'));
        const name        = (data.get('name')        as string ?? '').trim();
        const category_id = Number(data.get('category_id'));
        const priceRaw    = (data.get('price')       as string ?? '').trim();
        const unit        = (data.get('unit')        as string ?? '').trim() || null;
        const description = (data.get('description') as string ?? '').trim() || null;
        const image_url   = (data.get('image_url')   as string ?? '').trim() || null;

        if (!id || isNaN(id))  return fail(400, { action: 'edit_product', error: 'ID sản phẩm không hợp lệ.' });
        if (!name)             return fail(400, { action: 'edit_product', error: 'Tên sản phẩm không được để trống.' });
        if (!category_id)      return fail(400, { action: 'edit_product', error: 'Vui lòng chọn danh mục.' });

        const price = priceRaw ? parseInt(priceRaw, 10) : null;
        if (price !== null && (isNaN(price) || price <= 0)) {
            return fail(400, { action: 'edit_product', error: 'Giá phải là số dương.' });
        }

        const svc = new ProviderService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.updateProduct(id, { category_id, name, description, price, unit, image_url });
            return { action: 'edit_product', success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                if (err.status === 401 || err.status === 403) throw redirect(302, ROUTES.LOGIN);
                return fail(err.status, { action: 'edit_product', error: err.message });
            }
            return fail(500, { action: 'edit_product', error: 'Cập nhật sản phẩm thất bại.' });
        }
    },

    delete_product: async ({ request, fetch, cookies }) => {
        const data = await request.formData();
        const id   = Number(data.get('id'));

        if (!id || isNaN(id)) {
            return fail(400, { action: 'delete_product', error: 'ID sản phẩm không hợp lệ.' });
        }

        const svc = new ProviderService(fetch, cookies.get('auth_token') ?? '');
        try {
            await svc.deleteProduct(id);
            return { action: 'delete_product', success: true };
        } catch (err) {
            if (err instanceof ServiceError) {
                if (err.status === 401 || err.status === 403) throw redirect(302, ROUTES.LOGIN);
                return fail(err.status, { action: 'delete_product', error: err.message });
            }
            return fail(500, { action: 'delete_product', error: 'Xoá thất bại.' });
        }
    },
};
