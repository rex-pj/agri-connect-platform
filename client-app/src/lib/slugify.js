/**
 * Converts a string to a URL-safe slug.
 * Handles Vietnamese diacritics and the special letter đ/Đ.
 * Example: "Gạo Thơm ST25 An Giang" → "gao-thom-st25-an-giang"
 *
 * Plain JS (types in JSDoc) so `node --test` can run it on Node 20; the Rust
 * port (backend web::slug) is checked against the same slugify.fixtures.json.
 *
 * @param {string} text
 * @returns {string}
 */
export function slugify(text) {
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
