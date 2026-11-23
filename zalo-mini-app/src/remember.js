// What this phone chose last, kept in the WebView's storage: the area of the
// price board, as the website keeps it in its `price_area` cookie. Storage
// can be missing or refused (a private WebView, cleared data), so every read
// and write is guarded and the app works the same without it.

const PRICE_AREA = "price_area";

/** The area chosen on the price board, as its id text, or "" for every area. */
export function rememberedArea() {
  try {
    return window.localStorage.getItem(PRICE_AREA) || "";
  } catch {
    return "";
  }
}

/** Keep `area` (an id text), or forget it when "". */
export function rememberArea(area) {
  try {
    if (area) window.localStorage.setItem(PRICE_AREA, area);
    else window.localStorage.removeItem(PRICE_AREA);
  } catch {
    // Nothing kept: the next visit opens on every area.
  }
}
