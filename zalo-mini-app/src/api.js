// The backend's JSON API (the same one the website's pages use). Every
// error body is `{ code, message, field_errors? }`; `message` is already in
// Vietnamese and is what the app shows.

const BASE = (import.meta.env.VITE_API_URL || "").replace(/\/+$/, "");

export class ApiError extends Error {
  constructor(status, body) {
    super(body?.message || `HTTP ${status}`);
    this.status = status;
    this.code = body?.code || "";
    // field → first message
    this.fields = Object.fromEntries(
      Object.entries(body?.field_errors || {}).map(([field, messages]) => [
        field,
        Array.isArray(messages) ? messages[0] : String(messages),
      ]),
    );
  }
}

async function request(path, { method = "GET", body } = {}) {
  const headers = { Accept: "application/json" };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const res = await fetch(`${BASE}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (res.status === 204) return null;
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(res.status, data);
  return data;
}

const query = (params) => {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== "") q.set(k, String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : "";
};

export const api = {
  categories: () => request("/api/explore/categories?type=service"),
  areas: () => request("/api/explore/locations"),
  prices: (locationId) => request(`/api/explore/prices${query({ location_id: locationId })}`),
  servicesNear: ({ lat, lng, categoryId }) =>
    request(`/api/explore/services${query({ lat, lng, category_id: categoryId })}`),
  // A tap on a call button, counted like the website's (no one's identity).
  recordCall: (serviceId) =>
    request("/api/explore/calls", { method: "POST", body: { kind: "service", id: serviceId } }),
  postJob: (job) => request("/api/explore/jobs", { method: "POST", body: job }),
  zaloLocation: (accessToken, token) =>
    request("/api/zalo/location", { method: "POST", body: { access_token: accessToken, token } }),
  zaloPhone: (accessToken, token) =>
    request("/api/zalo/phone", { method: "POST", body: { access_token: accessToken, token } }),
  // The alerts in force today (disease, salt water, weather), every sector.
  alerts: () => request("/api/explore/alerts"),
  thumbUrl: (fileId) => `${BASE}/files/${fileId}/thumb`,
};
