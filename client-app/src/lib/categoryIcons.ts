/**
 * Single source of truth for category icons, colours, and descriptions.
 * Keyed by the slug stored in the `categories` table.
 * Import these maps wherever a category needs a visual representation.
 */

export interface CategoryDisplay {
    icon:  string;   // Font Awesome solid class, e.g. "fa-basket-shopping"
    color: string;   // hex, used for tint/accent
    desc:  string;   // one-line Vietnamese description
}

export const PRODUCT_DISPLAY: Record<string, CategoryDisplay> = {
    'nong-san':   { icon: 'fa-basket-shopping',    color: '#065F46', desc: 'Gạo, trái cây, nông sản các loại' },
    'nong-cu':    { icon: 'fa-screwdriver-wrench', color: '#92400E', desc: 'Dụng cụ canh tác, bình xịt, liềm, cuốc' },
    'may-moc':    { icon: 'fa-gears',              color: '#1E3A8A', desc: 'Máy bơm, máy gặt, drone phun thuốc' },
    'phan-bon':   { icon: 'fa-bag-shopping',       color: '#7F1D1D', desc: 'Phân NPK, phân hữu cơ, vôi bột' },
    'thuoc-bvtv': { icon: 'fa-shield-halved',      color: '#14532D', desc: 'Thuốc trừ sâu, diệt cỏ, bảo vệ thực vật' },
};

export const SERVICE_DISPLAY: Record<string, CategoryDisplay> = {
    'muc-bun':    { icon: 'fa-water',   color: '#1E40AF', desc: 'Nạo vét kênh mương, ao hồ, ruộng lúa' },
    'phun-thuoc': { icon: 'fa-droplet', color: '#065F46', desc: 'Phun thuốc bảo vệ thực vật, drone phun' },
    'sua-may-no': { icon: 'fa-gears',   color: '#92400E', desc: 'Bảo dưỡng, sửa chữa máy bơm, máy gặt' },
};

export const DEFAULT_DISPLAY: CategoryDisplay = {
    icon: 'fa-tag', color: '#374151', desc: '',
};

/** Lookup by category display name (as returned by the API's `category` field on products). */
export const PRODUCT_NAME_ICON: Record<string, string> = Object.fromEntries(
    Object.entries(PRODUCT_DISPLAY).map(([slug, d]) => {
        const nameMap: Record<string, string> = {
            'nong-san':   'Nông sản',
            'nong-cu':    'Nông cụ',
            'may-moc':    'Máy móc',
            'phan-bon':   'Phân bón',
            'thuoc-bvtv': 'Thuốc BVTV',
        };
        return [nameMap[slug] ?? slug, d.icon];
    })
);

const PRODUCT_SLUG_BY_NAME: Record<string, string> = {
    'Nông sản':   'nong-san',
    'Nông cụ':    'nong-cu',
    'Máy móc':    'may-moc',
    'Phân bón':   'phan-bon',
    'Thuốc BVTV': 'thuoc-bvtv',
};

const SERVICE_SLUG_BY_NAME: Record<string, string> = {
    'Múc bùn':    'muc-bun',
    'Phun thuốc': 'phun-thuoc',
    'Sửa máy nổ': 'sua-may-no',
};

/** Get CategoryDisplay for a product category name (as returned by the API). */
export function getProductDisplay(categoryName: string): CategoryDisplay {
    const slug = PRODUCT_SLUG_BY_NAME[categoryName];
    return (slug ? PRODUCT_DISPLAY[slug] : undefined) ?? DEFAULT_DISPLAY;
}

/** Get CategoryDisplay for a service category name (as returned by the API). */
export function getServiceDisplay(categoryName: string): CategoryDisplay {
    const slug = SERVICE_SLUG_BY_NAME[categoryName];
    return (slug ? SERVICE_DISPLAY[slug] : undefined) ?? DEFAULT_DISPLAY;
}
