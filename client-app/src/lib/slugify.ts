/**
 * Converts a string to a URL-safe slug.
 * Handles Vietnamese diacritics and the special letter đ/Đ.
 * Example: "Gạo Thơm ST25 An Giang" → "gao-thom-st25-an-giang"
 */
export function slugify(text: string): string {
    return text
        .replace(/[đĐ]/g, 'd')
        .normalize('NFD')
        .replace(/[̀-ͯ]/g, '')
        .toLowerCase()
        .replace(/[^a-z0-9\s-]/g, '')
        .trim()
        .replace(/\s+/g, '-')
        .replace(/-+/g, '-');
}
