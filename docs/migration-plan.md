# Migration plan: SvelteKit → Axum + Tera SSR

Status: **Phase 0 survey done; round-1 and round-2 decisions recorded in §8. Ready to start S0.** Nothing in the code has been changed yet. Sections 1–4 and 6 are the survey and stay as written. **§5 slices are revised to match §8. Where §8 conflicts with an earlier section, §8 wins.**

**Greenfield (round 2):** nothing has ever been deployed and every row in every database is sample data. So no data, URL, cookie or email link has to be preserved. S0 replaces the migration history with a single clean baseline, and every database is recreated from it. **From S0 on, migrations are append-only** (§8.4).

Everything below comes from reading the code at commit `bb4ec09`. Where the code differs from `CLAUDE.md`, the code wins and the difference is called out.

---

## 0. Headline findings

Read these first. They change the plan.

1. **Provider login is broken today.** `login/+page.server.ts` always redirects to `/admin/dashboard` after login. `(admin)/admin/+layout.server.ts` then sees `role !== 'admin'`, **deletes the cookie** and redirects back to `/login`. A provider can only get a session by registering. `/login`'s load does the same thing to any visitor who already has a cookie.
2. **First-run detection is broken today.** `/login` and `/setup` call `GET /api/setup/status` without a token, but that route needs `RequireAdmin`, so it always returns 401. As a result `/login` never redirects to `/setup`, and `/setup` never redirects away once users exist. The backend still blocks a second admin (`create_first_admin` checks the admin count), so this is a UX bug, not a security hole.
3. **Demo data ships in the primary migrator.** `m20260524_000002_seed` inserts six demo businesses with services and products, and it is part of `Migrator`, which runs at every startup. Every database, production included, gets fake listings. `SeedMigrator` lists the same four migrations, so `POST /api/setup/seed` re-runs migrations that are already applied: it does nothing, and it returns 409 because businesses exist. That makes `/setup/seed` dead in practice.
4. **Access is decided by role in extractors, not by permission.** `RequireAdmin` and `RequireProviderOrAdmin` compare `UserRole`. `ProviderDashboardUseCase::require_permission` skips the check when `role == Admin`. The admin permission codes (`AgriBusinessVerify`, `AgriReviewModerate`, …) are seeded but nothing checks them. `PermissionUseCase` and `AdminUseCase` do no permission checks at all.
5. **The JSON API reads only `Authorization: Bearer`.** Today that works because SvelteKit's Node server copies the cookie into a header. A browser island using `fetch` cannot do that with an httpOnly cookie, so the API must also accept the cookie, and then needs the CSRF `Origin` check.
6. **There are no tests.** Not a single `#[test]` exists in the backend or the migration crate.
7. **There is very little genuine client-side state.** Nearly every interaction in the SvelteKit app is "change URL params and reload" or "submit a form". Only browser geolocation (`/explore` and the dashboard's GPS button) needs JavaScript to function. See §5 and §7 Q3 before we commit to a Svelte island toolchain.

---

## 1. Route inventory

There is **no `hooks.server.ts` and no `+server.ts`** in the app. All server logic lives in `+page.server.ts` / `+layout.server.ts`. Every load and action calls the Rust JSON API over HTTP (`PRIVATE_API_URL`) through `lib/server/services/*.service.ts`. Nothing touches the database directly, and the server side calls no third parties. The browser loads Bootstrap CSS/JS from jsDelivr, Font Awesome from cdnjs, and Be Vietnam Pro from Google Fonts (`app.html`).

Auth column key: **—** public · **cookie** = any `auth_token` cookie · **admin** = verified via `/api/users/me` + `role === 'admin'` · **provider*** = JWT decoded **without signature verification**, `role === 'provider'`.

Class key: **SSR** = pure SSR · **SSR+i** = SSR + island(s) · **Rethink** = needs rethinking.

### 1.1 Root and shared

| File | URL | Loads | Auth | Interactive parts | Class |
|---|---|---|---|---|---|
| `+layout.svelte` | all | none; imports `app.css`, renders `NavProgress` | — | navigation progress bar (`$app/stores.navigating`) | SSR (bar is dropped) |
| `+error.svelte`, `(app)/+error.svelte`, `(admin)/+error.svelte` | errors | `page.status` | — | none | SSR |
| `(app)/+layout.server.ts` + `.svelte` | all `(app)` pages | `isLoggedIn`, `role` from **unverified** JWT decode | — | search toggle, profile dropdown, scroll reset, logout form (`/account?/logout`), `BottomNav` picks a tab by `role` | SSR (toggles via Bootstrap JS or Alpine, §7 Q2) |
| `(admin)/+layout.server.ts` + `.svelte` | `/admin/**` | nothing; adds `noindex` | — | none | SSR |
| `(admin)/admin/+layout.server.ts` | `/admin/**` | `/api/users/me`; requires `role === 'admin'` or clears the cookie and redirects | admin | none | SSR |
| `(admin)/admin/*/+layout.server.ts` (12 files) | each admin section | no-op `({})` | — | — | delete |
| `(admin)/admin/*/+layout.svelte` (12 files) | each admin section | wraps the page in `AdminShell` (duplicated 12×) | — | sidebar active state from `page.url` | SSR (one shared layout) |

### 1.2 Public app `(app)`

| Route | URL | Loads (all via Rust API) | Auth | Interactive parts | Class |
|---|---|---|---|---|---|
| `(app)/+page` | `/` | `/api/explore/featured`, `/api/explore/categories?type=product`; builds nav with hard-coded `'/services'`, `'/explore'` | — | none | SSR |
| `explore/+page` | `/explore?lat&lng&radius&service_id` | featured, service + product categories, locations; `GET /api/explore/providers?lat&lng&radius&service_id` when lat/lng are present | — | **`navigator.geolocation`** → `goto` with lat/lng; location-preset `<select>` → `goto`; radius buttons; service filter chips; `ContactSheet` modal + action `contact_request` | **SSR+i** (geolocate) |
| `products/+page` | `/products?q&category_id&min_price&max_price&sort&page` | `/api/explore/products?...`, product categories | — | search box, filter panel toggle, category/sort pills, price inputs, apply/reset, pagination; all via `goto` | SSR (GET form) |
| `products/[id]/+page.server` | `/products/{id}` | `/api/explore/products/{id}` → **301** `/products/{id}/{slugify(name)}` | — | — (`+page.svelte` here never renders: dead) | SSR (redirect) |
| `products/[id]/[slug]/+page` | `/products/{id}/{slug}` | `/api/explore/products/{id}`; **slug is never checked** | — | `tel:` link | SSR |
| `products/category/[id]/[slug]/+page` | `/products/category/{id}/{slug}?page` | products filtered by `category_id`, all product categories (finds the current one in JS; renders with `null` if not found) | — | pagination via `goto` | SSR |
| `services/+page` | `/services?q&service_id&sort&page` | `/api/explore/providers?page&service_id&name&sort` (browse mode), service categories | — | search, filter panel, pills, pagination, `ContactSheet` + action `contact_request` | SSR (contact form in a modal, §5) |
| `services/category/[id]/[slug]/+page` | `/services/category/{id}/{slug}?page` | providers by `service_id`, service categories | — | pagination, `ContactSheet` + `contact_request` | SSR |
| `providers/[id]/+page.server` | `/providers/{id}` | provider → **301** `/providers/{id}/{slug}` | — | — (`+page.svelte` dead) | SSR (redirect) |
| `providers/[id]/[slug]/+page` | `/providers/{id}/{slug}` | `/api/explore/providers/{id}`; slug not checked | — | `tel:`, **share button** (`navigator.share` / clipboard + `alert`), `ContactSheet` + `contact_request` | SSR (share is progressive enhancement) |
| `search/+page` | `/search?q` | providers browse `name=q` page 1 + products browse `name=q` page 1, preview capped at 6 | — | search box, tabs (all/services/products) in client state | SSR (tabs → Bootstrap tabs or `?tab=`) |
| `account/+layout.server` + `+page` | `/account` (`#security`) | cookie present, then `/api/users/me`; session expiry from **unverified** JWT `exp` | cookie | actions `change_password`, `logout` | SSR |
| `provider/+layout.server` + `dashboard/+page` | `/provider/dashboard` | `/api/provider/profile`, `/api/provider/products`, product categories | provider* | profile form with **GPS button**; add/edit product in **Bootstrap JS modals** (`window.bootstrap.Modal`); edit modal prefilled from client state; optimistic product list; actions `update_profile`, `add_product`, `edit_product`, `delete_product` | **Rethink** (§5 slice 15) |

### 1.3 Auth and setup (no layout group)

| Route | URL | Loads / actions | Auth | Class |
|---|---|---|---|---|
| `login/+page` | `/login` (`?reset=1`) | load: has cookie → `/admin/dashboard` (bug 0.1); `user_count == 0` → `/setup` (bug 0.2). Action `login`: sets cookie, always redirects to `/admin/dashboard`; 423 → `locked` flag | — | SSR |
| `register/+page` | `/register` | load: has cookie → `/provider/dashboard`. Default action: required fields, password confirmation, phone regex → `POST /api/users`, sets cookie | — | SSR |
| `forgot-password/+page` | `/forgot-password` | default action: `POST /api/auth/password-resets`, always reports success | — | SSR |
| `reset-password/+page` | `/reset-password?token` | load passes `token` through. Action: confirmation + min length → `PATCH /api/auth/password-resets/{token}` → `/login?reset=1` | — | SSR |
| `setup/+page` | `/setup` | load: cookie → admin dashboard; users exist → `/login` (broken, 0.2). Action `create`: `POST /api/admin/setup`, sets cookie, `go_seed` → `/setup/seed` | — | SSR |
| `setup/seed/+page` | `/setup/seed` | load: needs a cookie, `GET /api/setup/status`, already seeded → dashboard. Actions `seed`, `skip` | cookie | SSR |

### 1.4 Admin `(admin)/admin/**` (auth = admin for every row)

| Route | URL | Loads | Actions (form `?/name`) | Interactive parts | Class |
|---|---|---|---|---|---|
| `admin/+page.server` | `/admin` | cookie → 302 `/admin/dashboard`, else **404** | — | — | SSR (redirect) |
| `dashboard` | `/admin/dashboard` | `/api/admin/stats` | — | — | SSR |
| `users` | `/admin/users?page` | users page, roles | `update_role`, `toggle_active` (+reason) | role modal, `ConfirmModal` with reason, per-row spinners | SSR (§5 confirm pattern) |
| `users/[id]` | `/admin/users/{id}` | user, roles, all permissions, user permissions (4 parallel calls) | `update_role`, `toggle_active`, `assign_permission`, `revoke_permission` | role modal, deactivate confirm, **optimistic permission toggles** (`grantedIds`) | SSR (per-permission forms) |
| `roles` | `/admin/roles` | roles | — | — | SSR |
| `roles/[id]` | `/admin/roles/{id}` | role | — | — | SSR |
| `permissions` | `/admin/permissions` | all permissions | — | — | SSR |
| `categories` | `/admin/categories` | categories | `create`, `update`, `toggle_active` | create panel, inline edit, hide confirm | SSR |
| `categories/[id]` | `/admin/categories/{id}` | category | `update`, `toggle_active` | hide confirm | SSR |
| `explore` | `/admin/explore?page` | providers page | `assign_user`, `toggle_verify` (+reason) | assign-account modal with show-password toggle, unverify confirm, optimistic `overrides` map | SSR |
| `explore/[id]` | `/admin/explore/{id}` | provider detail with services and products | `toggle_verify` | confirm | SSR |
| `inquiries` | `/admin/inquiries?page` | inquiries page | `update_status` | status `<select onchange>` auto-submits | SSR (select + submit button; auto-submit as enhancement) |
| `reviews` | `/admin/reviews?page` | reviews page | `toggle_visible` (+reason) | hide confirm | SSR |
| `locations` | `/admin/locations` | locations | `create`, `update`, `toggle_active` | create panel, inline edit | SSR |
| `locations/[id]` | `/admin/locations/{id}` | location | `update`, `toggle_active`, `delete` → 303 list | delete confirm | SSR |
| `products` | `/admin/products?page&provider_id` | products page | `toggle_active`, `delete` (+reason) | confirms | SSR |
| `services` | `/admin/services?page&provider_id` | services page + all categories (filtered to active service categories in TS) | `create`, `update`, `toggle_active`, `delete` | create/edit panels, confirms | SSR |
| `logout` | `/admin/logout` (POST) | default action: deletes cookie → `/login` | — | — | SSR (POST handler) |

### 1.5 Shared components that carry behaviour

| Component | Behaviour | Replacement |
|---|---|---|
| `ContactSheet` (organism) | modal form posting `?/contact_request` with `use:enhance`; echoes field errors | Tera partial: a `<dialog>`/Bootstrap modal containing a plain POST form; with JS off it renders inline on the provider page |
| `ConfirmModal` (molecule) | modal with optional reason textarea, posts an action | Tera partial + a GET confirm page (§5 "confirm pattern") |
| `PaginationNav` (molecule) | page numbers via a callback → `goto` | Tera macro producing `<a href>` links |
| `FilterChips` (molecule) | callback → `goto` | links |
| `NavProgress` (atom) | SvelteKit navigation indicator | dropped: no client-side navigation remains |
| `AdminShell`, `BottomNav` (organisms) | active link from `page.url`; `BottomNav` compares `role` strings | server computes `nav.active` and `nav.show_admin` / `nav.show_dashboard` flags |
| `lib/categoryIcons.ts` | slug → icon/colour/description map, plus a reverse map keyed on **Vietnamese display names**; overrides the DB `icon` column for known slugs | §7 Q15 |
| `lib/slugify.ts` | Vietnamese slugify (đ → d, NFD, strip marks) | Rust port with fixture tests proving byte-identical output (existing URLs depend on it) |
| `lib/routes.ts` | URL constants | Rust `routes` module, also exposed to Tera as a registered function |

---

## 2. Logic in SvelteKit that must move to Rust

"Web" means the new `web` crate. Use cases live in `application`.

| # | Logic today | Where | New home |
|---|---|---|---|
| L1 | Auth guard for all of `/admin/**`: no cookie → `/login`; `/me` fails → clear cookie on 401, redirect on anything else (fail-secure); non-admin → clear cookie + redirect | `(admin)/admin/+layout.server.ts` | **Middleware** extracts `Option<AuthUser>` from the cookie. **Handler** redirects `None` → `/login`. **Use case** checks `has_perm("admin.…")` and returns `permission_denied` → 403 page. Logging the user out on 403 goes away (§7 Q6). |
| L2 | Nav state from an **unverified** JWT decode (`decodeJwtRole`) | `(app)/+layout.server.ts`, `lib/server/api.ts` | `render_page` builds `NavContext` from the verified `AuthUser` + permissions. Flags are computed server-side. |
| L3 | Provider guard via unverified decode, `role !== 'provider'` → home/login | `(app)/provider/+layout.server.ts` | handler redirect on `None`; `ProviderDashboardUseCase` checks `provider.profile.read` etc. |
| L4 | `/account` guard; session expiry read from unverified `exp` | `account/+layout.server.ts`, `+page.server.ts` | handler; expiry comes from the verified claims on `AuthUser` |
| L5 | `/admin` index: 302 to dashboard with a cookie, else 404 | `admin/+page.server.ts` | handler (302 to dashboard either way; dashboard handles login) |
| L6 | Login: non-empty check, sets cookie (httpOnly, `SameSite=Strict`, `Secure` when `NODE_ENV=production`, 1 h), redirects, maps 423 → "locked" | `login/+page.server.ts`, `api.ts:authCookieOptions` | `AuthUseCase::login` (already validates) + web `session` module that sets access + refresh cookies. Landing page decided by a pure `fn post_login_destination(&Permissions) -> Route` (fixes bug 0.1). |
| L7 | Login/setup first-run redirects (`user_count`) | `login`, `setup` loads | `SetupUseCase::status()` called directly by the handler (fixes bug 0.2; no HTTP hop, no auth needed internally) |
| L8 | Register: required fields, `password === password2`, phone regex | `register/+page.server.ts` | `AuthUseCase::register` (already does fields + phone); new `application::validators::password_confirmation` |
| L9 | Forgot password: always report success | `forgot-password` | use case already does this; handler renders the "sent" state |
| L10 | Reset password: confirmation, min length, redirect `?reset=1` | `reset-password` | `AuthUseCase::reset_password` + confirmation validator; handler redirects |
| L11 | Setup: confirmation, cookie, `go_seed` branch | `setup` | `AuthUseCase::create_first_admin` + handler |
| L12 | Seed / skip, "already seeded" redirect | `setup/seed` | `SetupUseCase` with permission `system.seed` (§7 Q4 decides what seeding means) |
| L13 | Logout (two copies: `/admin/logout`, `/account?/logout`) | both | one `POST /logout` handler; `AuthUseCase::logout` revokes the refresh token; both cookies cleared |
| L14 | Contact/inquiry validation, **four copies with three different rule sets** (only `/explore` checks the 100/255/1000 length limits). The backend also validates in the request DTO (`InquiryRequest` + `validator`) **and** in `ExploreUseCase::contact_request` | `explore`, `services`, `services/category`, `providers/[id]/[slug]` | one set of rules in `ExploreUseCase::submit_inquiry`; one web handler `POST /contact` with PRG and a flash message; field errors come back as error codes |
| L15 | Home nav assembly, with hard-coded `'/services'`, `'/explore'` | `(app)/+page.server.ts` | home page view model builder in web, using `routes` |
| L16 | "Current category" found by scanning the full list; renders a null category instead of 404 | both category pages | `ExploreUseCase::category(id, kind)` → `not_found` |
| L17 | Slug redirects (301) | `products/[id]`, `providers/[id]` | handler + `slugify` in `domain` (pure) |
| L18 | Query param parsing for filters, sort, page | listing loads | handler `Query<…>` structs; use cases clamp and validate (`paginate` already exists) |
| L19 | Admin form actions: trim, `Number()`, empty checks, Vietnamese messages | all admin `+page.server.ts` | handlers parse `Form<…>`; validation stays in use cases; messages resolved from error codes |
| L20 | Admin services page filters categories in TS (`type === 'service' && is_active`) | `admin/services` | use case query parameter |
| L21 | User detail aggregates four calls and computes granted IDs | `admin/users/[id]` | `AdminUseCase::user_detail_view(actor, id)` returns the user, roles, permissions with a `granted` flag, and `can_*` flags |
| L22 | 401/403 from the API → redirect to login (`checkAuth`) | everywhere | gone: handlers get typed errors from use cases |
| L23 | Error parsing (`ServiceError`, `flattenFieldErrors`) | `api.ts` | gone: single error type with field errors |
| L24 | Date (`toLocaleDateString('vi-VN')`, rendered in the **Node server's zone**) and price (`vi-VN` + `₫`) formatting | many pages | Tera filters: `vnd` for price; `<time datetime="UTC">` with browser localisation (§7 Q14) |
| L25 | Duplicated constants (`PHONE_REGEX`, `PASSWORD_MIN_LEN`, cookie max-age "keep in sync") | `api.ts` | deleted: Rust `constants` is the single source |
| L26 | Category display metadata (icon/colour/description) | `categoryIcons.ts` | §7 Q15 |
| L27 | Secrets/config: only `PRIVATE_API_URL` | `$env/static/private` | gone once Rust serves the pages |

---

## 3. Current backend shape vs the target

The backend is one crate (`agri_connect`) plus `migration`. Folders follow the layering, but the compiler enforces none of it. Every gap below is something the target needs and the code does not do today.

### 3.1 Crate structure and layering

| # | Gap | Evidence | Fix |
|---|---|---|---|
| G1 | Single crate, so layering is convention only | `Cargo.toml` `members = [".", "migration"]` | workspace: `crates/domain`, `crates/application`, `crates/infrastructure`, `crates/web` (bin), `migration` |
| G2 | Domain imports the web error type | all 12 `domain/repositories/*_trait.rs` import `crate::view_models::AppError` | `domain::DomainError` (machine `code`), with the `IntoResponse` impl behind an optional `axum` feature |
| G3 | Domain imports app constants | `domain/models/role.rs` uses `crate::constants::ROLE_*` | role codes move into domain |
| G4 | Use cases return HTTP DTOs | e.g. `auth_usecase.rs` returns `view_models::responses::{LoginResponse, MeResponse}`, and the same for every use case | application output types; web maps them to JSON/page view models |
| G5 | Infrastructure imports middleware | `jwt_token_service.rs` imports `middleware::auth::{encoding_key, Claims}`; JWT keys live in `OnceLock` statics in middleware | keys and `Claims` move into `infrastructure`; the token port gains `verify` |
| G6 | `application::shared` imports `validators` (crate root) and `view_models` | `shared.rs` | validators move into `application` |

### 3.2 Errors

| # | Gap | Evidence |
|---|---|---|
| G7 | Two error types (`AppError`, `HandlerError`) and no machine `code`; free-text messages in mixed English (use cases) and Vietnamese (provider dashboard). English strings reach farmers through SvelteKit's `err.message`. | `view_models/mod.rs` |
| G8 | Ad-hoc responses outside the error type: `json!({"message"})` in auth middleware and the rate limiter; `serde_json::to_value(result).unwrap()` in `explore_handler.rs` | `middleware/auth.rs`, `middleware/rate_limit.rs`, `handlers/explore_handler.rs:41,54` |

### 3.3 Auth and permissions

| # | Gap | Evidence |
|---|---|---|
| G9 | Access decided by role in extractors (`RequireAdmin`, `RequireProviderOrAdmin`) | every admin/provider handler |
| G10 | Admin bypass by role comparison inside a use case | `provider_dashboard_usecase.rs::require_permission` |
| G11 | State-changing use cases with no permission check | `PermissionUseCase::assign/revoke`, all of `AdminUseCase`, `SetupUseCase::run_seed` |
| G12 | Permission codes are CamelCase (`ProviderListingCreate`), not `x.y` keys. No role → permission mapping exists: grants are per-user only, and admins rely on the role bypass. | init migration, `constants.rs` |
| G13 | Bearer header only; no cookie; no `Option<AuthUser>` extractor | `middleware/auth.rs::bearer_token` |
| G14 | No refresh token; the JWT carries `role`, which goes stale for up to 1 h after a role change | `Claims` |
| G15 | Password policy (upper + digit + special) is stricter than `CLAUDE.md` ("min 8") and than the SvelteKit check | `shared.rs::validate_password` (§7 Q13) |

### 3.4 Ports and adapters

| # | Gap | Evidence |
|---|---|---|
| G16 | Email: a port exists, but the fallback is chosen **per send** inside the adapter (and warns each time), not at startup. `send_raw` maps every SMTP error to `MissingTo` with a "placeholder" comment. `APP_URL` defaults to `http://localhost:5173`. | `infrastructure/email_service.rs` |
| G17 | Rate limiting: no port. `governor` is used directly, keyed on `ConnectInfo` IP. Behind an Azure ingress every request shares the proxy IP, so one abusive client locks everyone out. Admin/provider write routes are not limited at all. | `middleware/rate_limit.rs` |
| G18 | Cache, storage and search ports don't exist. **Nothing needs them yet**, so the plan adds none unless §7 Q11/Q12 says otherwise. | — |
| G19 | No event bus. Audit logging is called inline from `AdminUseCase::audit` (failures are logged and ignored) | `admin_usecase.rs:85` |
| G20 | One `db` handle; no `db_read` | `startup.rs` |

### 3.5 Wiring and startup

| # | Gap | Evidence |
|---|---|---|
| G21 | `AppState` holds use-case structs by value and no ports; no Tera, site config or locale | `app_state.rs` |
| G22 | `startup.rs` panics (`expect`, `panic!`, `.unwrap()` on serve). It also **drops `seaql_migrations` automatically** when the `roles` table is missing, which is a silent destructive recovery. | `startup.rs:45-75` |
| G23 | Bind address is hard-coded (`0.0.0.0:3000`) | `constants.rs` |
| G24 | `CORS_ALLOW_ORIGINS` name differs from the target's `CORS_ORIGINS` | `app_routing.rs` (§7 Q18) |
| G25 | CSP is `default-src 'none'; script-src 'none'`. That is correct for JSON, but pages need a separate policy. | `app_routing.rs` |

### 3.6 Data and time

| # | Gap | Evidence |
|---|---|---|
| G26 | Migrations contain data: roles and permissions (init), categories + locations + demo businesses/services/products (`000002_seed`), the provider-permission backfill (`000004`, pure DML) | `migration/src/` |
| G27 | `backend/migrations/20260507000001_add_password_reset_tokens.sql` is a leftover sqlx file that nothing runs (it also uses `used_at`, while the entity uses `consumed_at`) | — |
| G28 | `.env` has `SEED_SECRET`; no code reads it | `backend/.env` keys |
| G29 | Timestamps are already `TIMESTAMPTZ` (28 columns), and repos convert `FixedOffset` → `Utc`. **No gap**, but `created_on` leaves use cases as RFC 3339 strings, not typed instants. | `db_types.rs`, `auth_usecase.rs::me` |
| G30 | `products.image_url` is a bare `VARCHAR(500)` with a user-supplied URL; no file entity | init migration (§7 Q12) |
| G31 | Search is `ILIKE '%q%'` on `name`/`description`, with no index and no accent folding. "phan bon" does not match "Phân bón". | `business_repository.rs:270`, `product_repository.rs:248` (§7 Q11) |

### 3.7 Health, ops and tooling

| # | Gap | Evidence |
|---|---|---|
| G32 | One `/health`, which checks nothing | `health_handler.rs` |
| G33 | JSON logs only when `LOG_FORMAT=json`; no `request_id`; no graceful shutdown | `startup.rs` |
| G34 | No `.env.example` for the backend; `client-app/.env.example` lists `PUBLIC_API_URL`, but the code reads `PRIVATE_API_URL` | — |
| G35 | No `[workspace.dependencies]`, no lints. `tokio = full` and `tower = full` with default features. 14 `unwrap`/`expect` calls in `src`. | `Cargo.toml` |
| G36 | Zero tests | — |
| G37 | No Dockerfile, CI config or README in the repo | — |

---

## 4. URL preservation

> **Superseded by §8.10.** Nothing was ever deployed, so this section is now the target URL map only. The `/agri-connect` 301s and the `/health` alias below are **not** built.

The Rust app answers every URL below. Paths and query parameter names stay identical; only the renderer changes.

### 4.1 Public pages: kept as-is

| URL | Query params kept | Note |
|---|---|---|
| `/` | — | |
| `/explore` | `lat`, `lng`, `radius`, `service_id` | |
| `/products` | `q`, `category_id`, `min_price`, `max_price`, `sort`, `page` | |
| `/products/{id}/{slug}` | — | wrong slug: §7 Q8 (recommend 301 to canonical) |
| `/products/category/{id}/{slug}` | `page` | same |
| `/services` | `q`, `service_id`, `sort`, `page` | |
| `/services/category/{id}/{slug}` | `page` | same |
| `/providers/{id}/{slug}` | — | same |
| `/search` | `q` | stays `noindex` |
| `/login` | `reset=1` | |
| `/register`, `/forgot-password` | — | |
| `/reset-password` | `token` | **this link is inside already-sent emails** (valid for 1 h) |
| `/setup`, `/setup/seed` | — | |
| `/account` (+ `#security` anchor), `/provider/dashboard` | — | |
| `/admin/dashboard`, `/admin/users`, `/admin/users/{id}`, `/admin/roles`, `/admin/roles/{id}`, `/admin/categories`, `/admin/categories/{id}`, `/admin/explore`, `/admin/explore/{id}`, `/admin/inquiries`, `/admin/reviews`, `/admin/locations`, `/admin/locations/{id}`, `/admin/products`, `/admin/services`, `/admin/permissions` | `page`; `provider_id` on products/services | |

### 4.2 Redirects

| From | To | Status |
|---|---|---|
| `/products/{id}` | `/products/{id}/{slug}` | 301 (unchanged) |
| `/providers/{id}` | `/providers/{id}/{slug}` | 301 (unchanged) |
| `/admin` | `/admin/dashboard` | 302 (today it is a 404 without a cookie) |
| `/agri-connect` (+ query) | `/explore` (+ same query) | 301, **only if** these URLs were ever live (§7 Q7) |
| `/agri-connect/providers/{id}/{slug}` | `/providers/{id}/{slug}` | 301, same condition |
| `/admin/agri-connect`, `/admin/agri-connect/{id}` | `/admin/explore`, `/admin/explore/{id}` | 301, same condition |

### 4.3 POST targets

SvelteKit form actions post to `?/action` on the page URL (`/account?/logout`, `/explore?/contact_request`, …). They are not bookmarkable and nothing outside the app links to them, so they get **new, explicit POST routes** (for example `POST /logout`, `POST /contact`, `POST /admin/users/{id}/role`). `/admin/logout` (POST) stays as an alias of `POST /logout`.

### 4.4 API and infrastructure URLs

- All `/api/**` routes stay byte-compatible.
- `/health` stays as an alias of `/health/live` until probes are moved (§7 Q19). New: `/health/live`, `/health/ready`.
- `/_app/**` (SvelteKit build assets) and `__data.json` endpoints disappear. They are cache-busted internals and nothing external references them.
- New static paths: `/static/css/…`, `/static/js/…`, `/static/fonts/…`, versioned through a Tera `asset_url()` function.

---

## 5. Ordered migration slices

### Running both apps during the migration

Rust becomes the front door from slice S1. Any non-`/api` path that Rust does not serve yet is reverse-proxied to the SvelteKit Node server (`LEGACY_FRONTEND_URL`; if unset, Rust returns 404 and logs a WARN at startup). Each slice moves routes from "proxied" to "served". The proxy and its HTTP client dependency are deleted in the last slice.

This keeps one origin, so the existing `auth_token` cookie works for both apps. Rust keeps minting JWTs with the same claim shape until `/admin/**` and `/provider/**` have left SvelteKit, because SvelteKit decodes `role` from the JWT. (§7 Q17 asks whether this topology fits your deployment.)

### Progressive-enhancement patterns used in every slice

- **Filters and pagination:** a plain `<form method="get">` and `<a href>` links. They work with JS off.
- **Confirm pattern:** a destructive or reasoned action is a link to `GET …/confirm-hide` (renders the confirm form, mutates nothing). That page `POST`s to the action, which redirects with 303 and a flash message. With JS on, the same partial opens in a modal. GET never mutates.
- **Forms:** POST → use case → 303 back with a flash (success) or re-render with field errors (failure). No optimistic UI.
- **Toggles, dropdowns, modals:** Alpine components (CSP build). Modals are native `<dialog>` elements styled with Bootstrap's modal classes. Bootstrap's JS is not shipped (§8.3).

### Slice definition of done

Every page slice ends with all of the following:

- The Rust page is live.
- The SvelteKit route is deleted.
- A render test (router `oneshot` → status, key data in the HTML, `noindex`/SEO tags as required) passes.
- A JS-disabled check passes: content is present in the raw HTML.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.

### Foundation slices (no user-visible page changes)

Revised for §8: CI/CD and deployment moved into S1 so every later slice ships through the pipeline; role permissions, search and media became their own slices.

| Slice | Content | Ships because |
|---|---|---|
| **S0 Workspace + errors + baseline** | Split into `domain` / `application` / `infrastructure` / `web` crates (G1–G6). `[workspace.dependencies]`, `[workspace.lints]` (deny unwrap/expect/panic/unreachable/todo/indexing), explicit features with reasons (G35). One error type with machine codes; `IntoResponse` behind `domain`'s `axum` feature (G7, G8). `vi` locale file for user text (§8.2), plus a convention test that every locale file has the same key set. Remove the startup `DROP TABLE seaql_migrations` (G22). **Replace all migrations with one DDL-only baseline; startup catalogue seeding of roles, permissions, default role grants, categories and locations (§8.4, §8.5).** | The JSON API is unchanged; fresh DBs come up with reference data and no demo data; first tests exist |
| **S1 Web shell + CI/CD** | Tera with `render_page` and typed `PageContext`. Registered functions `url_for`, `asset_url`, `file_url`; filters `vnd`, `time`. Base layouts as Tera macros organised by atomic level. Vendored Bootstrap CSS, Font Awesome, font and Alpine CSP build (§8.3). Page CSP. Legacy proxy fallback. `/health/live`, `/health/ready`. `request_id` JSON logs, graceful shutdown, `db` + `db_read`, `.env.example`. Template convention tests. **Dockerfile, GitHub Actions CI + deploy to Compute Engine (§8.9).** | Production now runs Rust in front and proxies every page to Node |
| **S2 Auth + permission checks** | `Option<AuthUser>` from cookie or Bearer. `actor.require(key)` in every state-changing use case; role decisions removed from extractors. Per-user permission API routes removed (§8.5). Refresh tokens. `Origin` allowlist. Rate-limit port grouped per router area. Event bus + audit subscriber. Password rule = min 8 chars / max 72 bytes (§8.8). | Same API; the API now also accepts the cookie |
| **S3 Accent-insensitive search** | `unaccent` + `pg_trgm`, `f_unaccent` wrapper, trigram indexes, matching query expression, `EXPLAIN` test (§8.6) | API search results improve immediately |
| **S4 Media storage** | `FileStorage` port with GCS and local-disk adapters, `files` table, `products.image_file_id` FK, image pipeline, `GET /files/{id}` (§8.7). No upload UI yet. | Product pages (S6+) can render images from our own origin |

### Page slices: read-only public pages first, most interactive last

| Slice | Routes | Alpine components / islands | Notes |
|---|---|---|---|
| **S5 App layout + home + errors** | `/`, 404/500 pages | `navSearch`, `profileMenu` | `BottomNav` and the profile menu driven by server flags |
| **S6 Product detail** | `/products/{id}`, `/products/{id}/{slug}` | none | slug parity tests against `slugify.ts` fixtures; wrong slug → 301 |
| **S7 Provider detail + contact** | `/providers/{id}`, `/providers/{id}/{slug}`, `POST /contact` | `shareButton`, `dialog` | consolidated inquiry rules (L14); contact partial reused later |
| **S8 Product listings** | `/products`, `/products/category/{id}/{slug}` | `collapse` (filter panel) | GET forms |
| **S9 Service listings** | `/services`, `/services/category/{id}/{slug}` | `collapse`, `dialog` (contact) | |
| **S10 Search page** | `/search` | `tabs` | |
| **S11 Explore** | `/explore` | **`ac-geolocate`** island | preset `<select>` + radius/service as GET params work without JS |
| **S12 Auth + setup pages** | `/login`, `/register`, `/forgot-password`, `/reset-password`, `/setup` (with the demo-data checkbox), `/setup/seed` (demo data page), `POST /logout`, `/admin/logout` | `passwordToggle` | fixes bugs 0.1 and 0.2; cookie becomes `SameSite=Lax` + refresh; demo loader (§8.4) |
| **S13 Account** | `/account` | none | |
| **S14 Admin shell + roles** | `/admin`, `/admin/dashboard`, `/admin/roles`, `/admin/roles/{id}` (**editable permission grid**), `/admin/permissions` | `collapse` (sidebar) | one admin layout replaces 12 wrappers |
| **S15 Admin users** | `/admin/users`, `/admin/users/{id}` | `dialog` (confirm) | role change + effective permissions (read-only) |
| **S16 Admin catalogue** | `/admin/categories(/{id})`, `/admin/locations(/{id})` | `dialog` | confirm pattern |
| **S17 Admin providers** | `/admin/explore(/{id})` | `dialog`, `passwordToggle` | assign account, verification |
| **S18 Admin moderation** | `/admin/products`, `/admin/services`, `/admin/inquiries`, `/admin/reviews` | `dialog`, `autoSubmit` | |
| **S19 Provider dashboard** | `/provider/dashboard`, `/provider/products/new`, `/provider/products/{id}/edit` | **`ac-geolocate`** (fills lat/lng), `imagePreview` | multipart product forms with image upload (§8.7) |
| **S20 Teardown** | — | — | delete `client-app/`, the proxy, its HTTP client, the `legacy` compose service and the legacy CI job; Node kept only to build `islands.js`; drop the JWT `role` claim; update README and `CLAUDE.md` (including removing the `/agri-connect` entries) |

**Islands in total: one (`ac-geolocate`).** Every other behaviour is an Alpine component registered in `static/js/app.js`.

---

## 6. Management surface

"End user" means a farmer (anonymous) or a provider (logged in), as stated per row. "Absent" means no tier controls it today.

| Feature | End user controls | Admin controls (where) | Ops-only (env/config) | Absent tiers, and why |
|---|---|---|---|---|
| Provider directory / profile | Provider: name, phone, description, address, lat/lng (`/provider/dashboard`) | verification badge, assign login account (`/admin/explore/{id}`) | — | Admin cannot **edit** a profile or deactivate a business: no API exists. Ask if intended. |
| Products | Provider: create/edit/delete own (`/provider/dashboard`) | hide/show, delete with reason (`/admin/products`) | — | |
| Services | Provider: **none** | create/edit/hide/delete (`/admin/services`) | — | Providers cannot list their own services (§7 Q16) |
| Categories | — | create, edit, hide (`/admin/categories`); no delete | — | Delete is absent, presumably because products/services FK them with `RESTRICT` |
| Category display (icon/colour/description) | — | `icon` column exists but is overridden by `categoryIcons.ts` for known slugs | colour/description hard-coded in TS | §7 Q15 |
| Location presets | — | full CRUD (`/admin/locations`) | — | |
| Geo search radius | Farmer picks 5/10/25/50 km | — | min/max/default in `constants.rs` | the frontend default (10 km) differs from the backend default (`RADIUS_DEFAULT_M` = 5 km) |
| Featured on home | — | — | counts in `constants.rs` | Selection is algorithmic; no admin "feature this" control |
| Inquiries | Farmer submits | status (`/admin/inquiries`) | rate limit (constant) | **Providers cannot see inquiries sent to them** (matters for lead-gen monetisation) |
| Reviews | **No submission UI or API** | visibility (`/admin/reviews`) | — | Reviews can only come from seed data today |
| Accounts | Provider: register, change password, reset password; **no profile/email edit** | role, active state, per-user permissions (`/admin/users/{id}`) | lockout thresholds, JWT expiry (constants), `JWT_SECRET` | |
| Roles | — | read-only (`/admin/roles`) | defined in code | deliberate: comment in `app_routing.rs` says roles are managed in code |
| Permissions | — | grant/revoke per user | catalogue in code (after S2) | no role-level defaults (§7 Q5) |
| Audit log | — | **no viewer** | — | Written but never shown |
| First admin / demo seed | — | `/setup`, `/setup/seed` | — | §7 Q4 |
| Email | — | — | `SMTP_*`, `FROM_EMAIL`, `APP_URL` | |
| Site identity (name, description, theme colour) | — | — | hard-coded in `app.html`; becomes `SITE_*` config injected by `render_page` | admin editing deliberately absent (no need yet) |
| Rate limits, CORS, HTTPS/HSTS, log format | — | — | env/constants | |

---

## 7. Open questions

Each question has my recommendation. **Round 1 is resolved:** answers are in §8.1, and questions you didn't answer take the recommended default listed in §8.2. The questions are kept below as the record of what was asked.

**Frontend approach**

1. **CSS** (the prompt left this open). The code is Bootstrap 5.3 + design tokens in `app.html` + `app.css` (519 lines) + 52 scoped `<style>` blocks. *Recommend:* keep Bootstrap 5; merge the tokens, `app.css` and the scoped styles into one `static/css/app.css` with BEM-style component classes; **self-host** Bootstrap, Font Awesome and Be Vietnam Pro. Self-hosting keeps `script-src 'self'` simple and cuts third-party requests on weak rural connections. OK?
2. **Bootstrap JS or Alpine?** The prompt says Alpine for dropdowns and toggles, but the repo already uses Bootstrap's JS (`bootstrap.bundle` in `app.html`, modals in the dashboard), which also covers dropdowns, collapse, modals and tabs. *Recommend:* self-hosted Bootstrap JS only, no Alpine. If you want Alpine anyway, I'll use the `@alpinejs/csp` build so we never need `'unsafe-eval'`.
3. **Island toolchain for one component?** Only geolocation needs client code. A Svelte custom-element build (Node + Vite at build time) for one ~40-line component is a lot of tooling. *Recommend:* still set up the islands build as you specified, because it's the seam for future interactive features (maps, demand posts). The alternative is a vanilla `static/js/geolocate.js`. Your call.

**Data and permissions**

4. **Demo data.** Should `m20260524_000002_seed`'s demo businesses/products stop being inserted at startup? And what should happen to the rows already in existing databases (keep them, or delete them by name in a one-off)? Should `/setup/seed` survive as an explicit admin-triggered demo loader, or be removed? *Recommend:* reference data (roles, permissions, categories, locations) is seeded idempotently at startup from code catalogues; demo data only through `/setup/seed`, and only when `APP_ENV != prod`.
5. **Permission model.** Today grants are per-user and admins bypass checks by role. To check by key everywhere I need either (a) a `role_permissions` table with defaults granted **once** when a role is first created (never re-applied, so revocations stick), or (b) per-user grants only, with every admin permission granted to each admin when the account is created or promoted. *Recommend (a).* Also: may I rename the codes to dotted keys (`provider.listing.create`)? Existing `user_permissions` rows reference permissions by id, so a rename is an in-place code update with no regrants.
6. **Wrong-role access.** Today a provider who opens `/admin` is logged out. *Recommend:* show a 403 page and keep the session.

**URLs**

7. **Were `/agri-connect…` and `/admin/agri-connect…` ever public** (deployed, shared, indexed)? If yes I add the 301s from §4.2; if no I skip them. `CLAUDE.md` also lists `/agri-connect/providers/[id]/[slug]`, which doesn't exist in the code.
8. **Slug mismatch.** `/providers/12/anything` renders today. *Recommend:* 301 to the canonical slug (good for SEO, and old links keep working after a rename).

**Auth**

9. **Refresh token:** lifetime (*recommend 30 days*), rotation on every use with reuse detection (*recommend yes*), and stored hashed in a new `refresh_tokens` table (DDL). Access JWT stays at 1 h?
10. **`SameSite`:** the repo uses `Strict`; the prompt says `Lax`. `Lax` is needed so a link opened from Zalo or Facebook arrives logged in, and the `Origin` check covers CSRF. Confirm the switch to `Lax`.
11. **Vietnamese search.** Do you want accent-insensitive search ("phan bon" finds "Phân bón")? If yes: an IMMUTABLE `f_unaccent` wrapper applied to both the indexed side and the query side, a trigram index, and an `EXPLAIN` test. This is DDL only, no search port. *Recommend yes;* farmers often type without diacritics.
12. **Product images.** Do uploads stay out of scope (the provider pastes a URL)? If they stay out, `img-src` must allow any `https:` origin, and those image hosts see our visitors' IPs. If uploads are in scope: a storage port (Azure Blob + local-disk fallback) and `products.image_file_id` as an FK.
13. **Password policy.** The backend requires an uppercase letter, a digit and a special character; `CLAUDE.md` says "min 8", and the forms only tell users about length. For farmers on phones, *recommend* min 8 characters only, unless you made the stricter rule deliberately.

**Presentation and product scope**

14. **Time display.** The server emits UTC in `<time datetime>`, and a small script localises it. What should the no-JS fallback text be? *Recommend:* format explicitly in `Asia/Ho_Chi_Minh` (UTC+7, no DST). This is a fixed named zone, not the server's local zone.
15. **Category display metadata.** `categoryIcons.ts` holds icon, colour and a one-line description per slug, and overrides the DB `icon`. *Recommend:* add `color` and `description` columns (DDL), backfill them from that map in the idempotent startup catalogue (only where the column is NULL), and make the admin category form edit them.
16. **Provider dashboard scope** (slice S17). OK to split it into sub-pages (`/provider/products/new`, `/provider/products/{id}/edit`) instead of prefilled modals? Should providers also gain **services management** and an **inbox of their inquiries** later? Not in this migration, just so I leave the right shape.

**Ops**

17. **Deployment during migration.** Where does this run today (Azure Container Apps?), and is it OK for Rust to be the single front door that proxies unmigrated pages to the Node process (§5)? Is there CI, and can it run Postgres + PostGIS for integration tests? There is no CI config in the repo.
18. **CORS variable name.** Keep `CORS_ALLOW_ORIGINS` (current), rename to `CORS_ORIGINS` (prompt), or read both for one release? *Recommend:* rename and keep reading the old name with a WARN until S18.
19. **`/health`.** Keep it as an alias of `/health/live` until your probes point at the new paths?
20. **PWA/offline** is listed in `CLAUDE.md` but doesn't exist (no manifest, no service worker). Out of scope for this migration?
21. **Client IP behind the ingress.** Rate limits need the real client IP. Which header does your ingress set, and how many proxy hops should we trust?

---

## 8. Round-1 decisions and the design that follows from them

### 8.1 Decision log

| # | Question (§7) | Decision | Where it lands |
|---|---|---|---|
| D1 | Q1 CSS | Keep Bootstrap 5 CSS; merge all styles into one `app.css`; self-host every asset | §8.3, S1 |
| D2 | Q2 JS behaviour | **Alpine.js** (not Bootstrap's JS) | §8.3, S1, every page slice |
| D3 | Q4 Demo data | Keep demo data for testing, but **opt-in from the UI**, never automatic | §8.4, S2, S12 |
| D4 | Q5 Permissions | **Role-based** grants | §8.5, S2, S14, S15 |
| D5 | Q7 `/agri-connect` URLs | Never deployed (round 2), so **no 301s**; the old names are only removed from `CLAUDE.md` | §8.10, S20 |
| D6 | Q11 Search | Accent-insensitive search: **yes** | §8.6, S3 |
| D7 | Q12 Images | **Uploads** | §8.7, S4, S19 |
| D8 | Q13 Password | Min 8 characters only | §8.8, S2 |
| D9 | Q17 Hosting/CI | **Google Compute Engine** + **GitHub Actions** | §8.9, S1 |
| D10 | Q-A Database | **Postgres on the VM** (container) | §8.9 |
| D11 | Q-B TLS | **Caddy on the VM** | §8.9 |
| D12 | Q-C Image URLs | Only demo data uses `image_url` → no import; the column is not in the baseline | §8.7 |
| D13 | Q-D Production | **Never deployed** → greenfield: squash migrations, preserve nothing | §8.4, §8.10 |
| D14 | Q-E Per-user grants | **Start fresh**: role grants only; `user_permissions` is not in the baseline | §8.5 |
| D15 | Q-F Demo data | Allowed **in every environment**; no feature flag | §8.4 |

### 8.2 Defaults assumed for questions you didn't answer

These follow my §7 recommendations. Say so if you want a different one.

| §7 | Default |
|---|---|
| Q3 Island toolchain | Set up the Svelte custom-element build as specified; one island (`ac-geolocate`) |
| Q6 Wrong role | Show a 403 page and keep the session |
| Q8 Wrong slug | 301 to the canonical slug |
| Q9 Refresh token | 30 days, rotated on every use, reuse detection revokes the whole family; stored hashed; access JWT stays at 1 h |
| Q10 Cookie | `SameSite=Lax` + `Origin` allowlist |
| Q14 Time fallback | Text rendered in `Asia/Ho_Chi_Minh`; `datetime` attribute in UTC; the browser re-localises |
| Q15 Category display | Add `color` and `description` columns; fill them from the `categoryIcons.ts` map only where they are NULL; editable in the admin category form |
| Q16 Dashboard | Split into sub-pages; provider services and inquiry inbox are out of scope |
| Q18 CORS variable | Rename to `CORS_ORIGINS`; no fallback, because no environment depends on the old name |
| Q19 `/health` | Removed; only `/health/live` and `/health/ready` exist (no probes depend on it) |
| Q20 PWA | Out of scope |
| Q21 Client IP | `TRUSTED_PROXY_HOPS` env var (§8.9) |
| Locales | `vi` only. Error codes resolve through `locales/vi.ftl`; the key-set convention test is in place for when a second locale arrives |

### 8.3 Frontend assets: Bootstrap CSS + Alpine (CSP build)

**Assets** (committed, all served from `/static/…` with versioned URLs from `asset_url()`):

```
crates/web/static/
  css/app.css                 tokens (from app.html) + app.css + the former scoped component styles
  vendor/bootstrap-5.3.3/bootstrap.min.css      CSS only, no bootstrap.bundle.js
  vendor/fontawesome-6.5.1/{css,webfonts}/      solid subset
  vendor/alpinejs-csp-3.x.y/cdn.min.js          exact version pinned in vendor/VERSIONS
  fonts/be-vietnam-pro/*.woff2                  400/500/600/700/800, vietnamese + latin subsets
  js/app.js                   Alpine.data(...) registrations (one per component listed in §5)
  js/time.js                  localises <time datetime>
  js/islands.js               built Svelte custom elements (ac-geolocate)
```

`vendor/VERSIONS` records each library's version, source URL and licence. Updating a vendored file is a reviewed diff.

**Why the CSP build:** the standard Alpine build evaluates attribute expressions with `new Function`, which needs `'unsafe-eval'`. The `@alpinejs/csp` build doesn't, so we stay on `script-src 'self'`. The cost is that templates can only reference **registered component properties and methods** (`x-data="navSearch"`, `@click="toggle"`, `x-show="open"`), not inline JS expressions. Two consequences:
- A comment at the CSP header definition records this: what breaks if someone swaps in the standard build.
- A convention test scans `templates/**` and fails on any `x-*` / `@*` / `:*` attribute value that isn't a plain identifier path, and on any `style="…"` attribute (`style-src 'self'` would block it).

**Page CSP:**
`default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' blob:; font-src 'self'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'`.
`blob:` is there only for the upload preview (§8.7). The JSON API keeps its current `default-src 'none'` policy.

**Modals** are native `<dialog>` elements opened by Alpine with `showModal()`, styled with Bootstrap's `.modal-*` classes. The browser provides focus trapping, Escape-to-close and the backdrop, so no focus-trap plugin is needed. Without JS, every dialog's content is reachable as a normal page (the confirm pattern in §5).

**Atomic design** carries over as Tera macros: `templates/atoms/*.html`, `templates/molecules/*.html`, `templates/organisms/*.html`, with pages in `templates/pages/**` and layouts in `templates/layouts/*`. The `CLAUDE.md` level rules still apply: atoms import nothing; molecules import atoms only.

### 8.4 Migration baseline and demo data

**Squash to one baseline (S0).** Nothing is deployed (D13), so the current history is replaced rather than patched.
- **Delete:** the four `migration/src/m2026052*` files and `backend/migrations/*.sql` (G27).
- **Add:** one `m20261013_000001_baseline`, DDL only, with:
  - the current schema, minus `user_permissions` (D14) and `products.image_url` (D12)
  - plus `role_permissions` and `catalogue_state` (§8.5)
  - plus `businesses.is_demo`
  - plus `categories.color` and `categories.description` (§8.2 Q15)
- **Later slices add their own migrations:** `refresh_tokens` (S2), search (S3), `files` + `products.image_file_id` (S4).
- **Reset:** every existing local database is dropped and recreated once. The S0 PR description says so, because sea-orm refuses to start against a database whose recorded migrations are missing (*"Migration file of version '…' is missing"*).
- **From then on, migrations are append-only.** An applied migration is never edited or deleted. `CLAUDE.md` gets this rule in S0, and a convention test checks that migration file names keep increasing.

**Reference data**, the data the app needs to run, is seeded at every startup from catalogues in code, inside one transaction:

| Catalogue | Rule |
|---|---|
| Roles, permissions | ensured by code: insert if missing |
| Default role grants | applied **once**, never again (§8.5) |
| Categories, location presets | inserted **only if the table is empty**, so an admin's renames and hides are never undone |

**Demo data** lives in `crates/infrastructure/src/demo/`: the six businesses with their services and products, as Rust data. It is loaded only by `SetupUseCase::load_demo(actor)`, which needs `system.demo.manage`.
- `businesses.is_demo BOOLEAN NOT NULL DEFAULT false` is in the baseline. Services and products reach it through their existing `ON DELETE CASCADE` FKs.
- Every demo row is inserted with `is_demo = true`.
- Loading is idempotent: if any `is_demo` row exists, `load_demo` returns `demo_already_loaded`.
- Demo products ship **without images**: the Unsplash URLs can't be shown under `img-src 'self'`, and the card already falls back to the category icon. Testers exercise the image upload flow themselves.

**UI** (available in every environment, D15):
- `/setup`: checkbox *"Nạp dữ liệu mẫu để thử nghiệm"*, **unchecked by default**. When ticked, the first admin is created and the demo data loaded in the same request.
- `/setup/seed` becomes the **demo data page** (admin, `system.demo.manage`). It shows the current demo row count and has **Load** and **Remove** buttons. Remove deletes every `is_demo` business through the confirm pattern; real listings are never touched, and a test proves it.

**Note for launch:** once real providers are listed, demo rows mix into public search results. Removing them is one click on `/setup/seed`, and the dashboard shows a warning banner while any demo rows exist.

### 8.5 Role-based permissions

**Schema (DDL):**
- `role_permissions(role_id → roles ON DELETE CASCADE, permission_id → permissions ON DELETE CASCADE, granted_by → users ON DELETE SET NULL NULL, granted_on TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (role_id, permission_id))`
- `catalogue_state(key TEXT PRIMARY KEY, applied_on TIMESTAMPTZ NOT NULL)`, which records one-time catalogue steps.

**Catalogue** (`application::authz::catalogue`, the single source). The old CamelCase codes and the unused `AgriBusinessCreate` / `AgriBusinessDelete` are not carried over; this list replaces them:

| Key | Default roles |
|---|---|
| `admin.dashboard.view` | admin |
| `user.read` | admin |
| `user.role.update` | admin |
| `user.status.update` | admin |
| `role.read` | admin |
| `role.permission.manage` | admin |
| `category.manage` | admin |
| `location.manage` | admin |
| `provider.read` | admin |
| `provider.verify` | admin |
| `provider.account.assign` | admin |
| `listing.moderate` | admin |
| `service.manage` | admin |
| `inquiry.read` | admin |
| `inquiry.manage` | admin |
| `review.moderate` | admin |
| `system.demo.manage` | admin |
| `own.profile.update` | provider |
| `own.listing.create` | provider |
| `own.listing.update` | provider |
| `own.listing.delete` | provider |

**Startup seeding (every boot, idempotent):**
1. Insert any missing permission (`ON CONFLICT DO NOTHING RETURNING id`).
2. **Default grants:**
   - If `catalogue_state` has no `role_grants.bootstrap.v1` row (a fresh database), apply the whole default matrix and record that key.
   - Otherwise, grant defaults only for permissions inserted in step 1 of this same run.
   - An admin's revocation is therefore never re-applied, and permissions added in later releases still reach the right roles.
3. A permission row whose key is no longer in the catalogue gets a WARN at startup, and nothing else. Deleting it would silently drop grants.

**No per-user grants** (D14). Effective permissions = the user's role's permissions. Between S0 and S2, the provider dashboard's existing `has_permission` check reads `role_permissions` instead of `user_permissions`. S2 removes the per-user permission API routes (`/api/admin/users/{id}/permissions…`) and the SvelteKit toggles that call them.

**Checks:**
- Middleware turns the JWT (`sub`) into `AuthUser { id }`; no access decision there.
- Per request, the web layer loads `Actor { user_id, role_id, perms }` with one indexed join on `db_read`. No cache: revocation takes effect on the next request.
- Every state-changing use case starts with `actor.require("…")?`. Admin-data reads do too.
- The decision is a pure `fn authorize(actor: Option<&Actor>, key: &str) -> Disposition { Allow, Unauthenticated, Forbidden }`, unit-tested on its own.
- Pages compute `can_*` flags with the same function.

**Lockout guards** (use case rules, each with a test):
- Refuse to remove `role.permission.manage` from a role if that would leave no active user holding it.
- An admin cannot change their own role or deactivate themselves.

**UI:**
- `/admin/roles/{id}` becomes editable: one form with a checkbox per permission and a save button. It works without JS; the use case diffs the submitted set against the current grants and emits one `RolePermissionsChanged` event, which the audit subscriber records.
- `/admin/users/{id}` shows the user's role and effective permissions, read-only.

`validate_role_code` checks against active rows in `roles`, not the hard-coded `admin`/`provider` pair. Roles themselves stay defined in code; admins edit only their permissions.

### 8.6 Accent-insensitive search

**Migration (DDL):**

```sql
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE FUNCTION f_unaccent(text) RETURNS text
  LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
  AS $$ SELECT public.unaccent('public.unaccent'::regdictionary, $1) $$;
CREATE INDEX ix_businesses_name_fold ON businesses USING gin (lower(f_unaccent(name)) gin_trgm_ops);
-- same for businesses.description, products.name, products.description
```

**One source for the expression.** `migration::search::fold(col) -> String` returns `lower(f_unaccent(<col>))`. The migration uses it to build the index; the repositories use it (through `Expr::cust`) to build `fold(col) LIKE '%' || lower(f_unaccent($1)) || '%'`. The same function produces both sides, so they can't drift apart. User input has `%`, `_` and `\` escaped before it reaches `LIKE`.

**Tests** (real Postgres in CI):
- `EXPLAIN` with `enable_seqscan = off` asserts a `Bitmap Index Scan` on each `ix_*_fold` index.
- Matching: `phan bon` → `Phân bón`, `dong thap` → `Đồng Tháp`, `DICH VU` → `dịch vụ`. The `đ`/`Đ` mapping is the one most likely to break, so it gets its own case.

**Applies to:** `/api/explore/providers?name=`, `/api/explore/products?name=`, and so `/services`, `/products`, `/search`.

### 8.7 Image uploads

**Port** (`application::ports::FileStorage`): `put(key, bytes, content_type)`, `get(key) -> byte stream`, `delete(key)`.

**Adapters** (both via the `object_store` crate, features `gcp` only), selected in `startup.rs`:

| Env | Adapter | Notes |
|---|---|---|
| `GCS_BUCKET` set | Google Cloud Storage | credentials from the VM's service account via the metadata server: no key file. Bucket private, uniform access. |
| unset | local disk at `UPLOAD_DIR` (default `./data/uploads`) | WARN at startup |

**Schema (DDL):**
- `files(id UUID PRIMARY KEY, storage_key TEXT UNIQUE NOT NULL, thumb_key TEXT UNIQUE NOT NULL, content_type TEXT NOT NULL, byte_size BIGINT NOT NULL, width INT NOT NULL, height INT NOT NULL, sha256 TEXT NOT NULL, uploaded_by INT REFERENCES users ON DELETE SET NULL, created_on TIMESTAMPTZ NOT NULL DEFAULT now())`
- `products.image_file_id UUID NULL REFERENCES files ON DELETE SET NULL`

**Pipeline** (`MediaUseCase::store_product_image`, CPU work on `spawn_blocking`):
1. **Size cap.** `UPLOAD_MAX_BYTES` (default 5 MiB), enforced twice: `DefaultBodyLimit` on the upload routes, and again in the use case.
2. **Sniff the type** from magic bytes: JPEG, PNG or WebP only. The client's `Content-Type` is ignored.
3. **Check dimensions before decoding.** Read the header first and reject images over 40 megapixels, so a decompression bomb never gets decoded.
4. **Decode and resize** with `image` (`default-features = false`, features `jpeg`, `png`, `webp`): longest side 1600 px for the main image, 400 px for the thumbnail.
5. **Re-encode to JPEG** (quality 80). This strips EXIF, including the GPS coordinates phones embed: farmers' photos would otherwise publish where their farm or house is. A test asserts the output has no EXIF segment.
6. **Store** both objects, then insert the `files` row. If the DB insert fails, the objects are deleted again.

**Serving:** `GET /files/{id}` and `GET /files/{id}/thumb` stream through Rust with:
- `Cache-Control: public, max-age=31536000, immutable` (an id is never reused; replacing an image creates a new id)
- `nosniff`
- the stored content type

Templates get URLs only from `file_url(id=…, variant="thumb")`. Everything stays same-origin, so `img-src 'self'` holds.

**Replace and delete:** when a product's image is replaced or the product is deleted, the use case emits `ProductImageReplaced` / `ProductDeleted`. A storage-cleanup subscriber on the event bus removes the old objects and row; failures are logged, not retried.

**Forms:**
- S19's create/edit product pages use `enctype="multipart/form-data"`, which works without JS.
- Alpine `imagePreview` shows the chosen file through a `blob:` URL.
- Upload routes get their own rate-limit group.
- Permission: `own.listing.create` / `own.listing.update`, plus the existing ownership check.

**No `image_url`.** The baseline has no `products.image_url` column (D12). Between S0 and S4, the API and the SvelteKit pages simply show no product images. The provider form's URL field goes away in S19, and its API field in S0.

### 8.8 Password policy

`validate_password`:
- At least 8 **characters** (counted with `chars()`).
- At most 72 **bytes**. bcrypt silently ignores everything after byte 72, so a longer password would *look* accepted while only its prefix counts. We reject it instead.
- The uppercase, digit and special-character rules are removed.

Tests cover a 7-character password, an 8-character Vietnamese password (multi-byte), and 73 bytes. `CLAUDE.md`'s Security Model section is updated to match.

### 8.9 Google Compute Engine + GitHub Actions

**Runtime image** (multi-stage `Dockerfile` at the repo root):
- Build stage: Rust on Debian bookworm, with `cargo-chef` for dependency caching.
- Runtime stage: `gcr.io/distroless/cc-debian12`, non-root.
- `templates/` and `static/` are embedded into the binary with `rust-embed`. In debug builds `rust-embed` reads from disk, so template edits reload during development. That makes one file the whole artifact, and `asset_url()` versions come from the embedded file hashes.
- No Node in the runtime image.

**On the VM** (a single Compute Engine instance), `docker compose` runs:
- `caddy`: TLS via Let's Encrypt (D11); the only service exposing ports 80/443. The `Caddyfile` is committed in `deploy/`.
- `web`: the Rust app on `:8080`, internal only, `stop_grace_period: 30s` so graceful shutdown can drain.
- `legacy`: the SvelteKit Node app, internal only. Its image is built by CI from `client-app/` (a small Dockerfile). Removed in S20.
- `db`: `postgis/postgis:16-3.4`, the same image CI tests against (D10).
  - **No published port:** reachable only on the compose network.
  - **Data** on a named volume, on a separate persistent disk, so the disk outlives the VM.
  - **Two roles:** `agri_app` (the app's role, no superuser) and `agri_admin` (migrations and extensions). `DATABASE_URL` uses `agri_app`. The baseline needs `CREATE EXTENSION` for `postgis` / `unaccent` / `pg_trgm`, so a one-time `deploy/db-init.sql` creates the extensions as `agri_admin`, and the app never needs superuser.
  - **`DATABASE_READ_URL`** points at the same server for now (the seam from §5).

**Backups** (Postgres on the VM makes this our job):
- A systemd timer on the VM runs `pg_dump -Fc` nightly and uploads the dump to a GCS bucket.
  - The bucket has a 30-day lifecycle rule and object versioning.
  - Uploads use the VM's service account; no key file.
- A weekly snapshot schedule of the data disk is a second layer.
- `deploy/RESTORE.md` documents the restore steps. The first restore drill happens before launch, so the steps are known to work.

Caddy appends the client IP to `X-Forwarded-For`. With `TRUSTED_PROXY_HOPS=1` the rate limiter keys on the right-most untrusted address; this lives in a pure function with tests. Migrations run at startup, as today, under a Postgres advisory lock so an overlapping restart can't run them twice.

**Secrets** live in Secret Manager. The deploy step renders them into `/etc/agri-connect/web.env` on the VM, mode 600. No secret ever appears in the image or the repo.

**CI** (`.github/workflows/ci.yml`, on pull requests and pushes to `master`):

| Job | Steps |
|---|---|
| `rust` | `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` against a `postgis/postgis:16-3.4` service container (it ships `unaccent` and `pg_trgm`); `Swatinem/rust-cache` |
| `islands` | `npm ci && npm run build` in `islands/`, then `git diff --exit-code crates/web/static/js/islands.js`: a stale committed bundle fails the build |
| `legacy` | `client-app`: `npm ci`, `npm run check`, `npm run build`. Removed in S20. |
| `image` | `docker build` (no push) |

**CD** (`.github/workflows/deploy.yml`, on a successful CI run on `master`):
1. **Auth:** `google-github-actions/auth` with Workload Identity Federation. No JSON service-account key.
2. **Build and push** the image to Artifact Registry, tagged with the git SHA.
3. **Deploy:** `gcloud compute ssh --tunnel-through-iap`, then `docker compose pull && docker compose up -d`.
4. **Gate:** poll `/health/ready` for 60 s. On failure, redeploy the previous SHA tag and fail the job.
5. **Approval:** the job runs in a GitHub `production` environment, with required reviewers if you want them.

**`.env.example`** lists every variable, one line each, with what breaks when it's wrong:
`APP_ENV`, `APP_URL`, `BIND_ADDR`, `DATABASE_URL`, `DATABASE_READ_URL`, `JWT_SECRET`, `REFRESH_TOKEN_DAYS`, `CORS_ORIGINS`, `HTTPS_ENABLED`, `TRUSTED_PROXY_HOPS`, `LOG_FORMAT`, `RUST_LOG`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `FROM_EMAIL`, `GCS_BUCKET`, `UPLOAD_DIR`, `UPLOAD_MAX_BYTES`, `LEGACY_FRONTEND_URL` (until S20), `SITE_NAME`, `SITE_DESCRIPTION`.

### 8.10 URLs: what greenfield changes

Nothing was ever public (D13), so §4's preservation list becomes the **target URL map**, not a compatibility promise:
- No `/agri-connect` 301s.
- No `/health` alias.
- No old cookie or reset-email compatibility.
- `CLAUDE.md`'s route map and SEO table drop the `/agri-connect` entries in S20.

Two redirects stay, because the app itself depends on them:
- `/products/{id}` → canonical slug
- `/providers/{id}` → canonical slug

Wrong-slug 301s stay too (§8.2 Q8).

### 8.11 What each decision adds, by slice

| Slice | Added by round 1 |
|---|---|
| S0 | migration baseline (D12, D13, D14); catalogue seeding with role grants (D3, D4) |
| S1 | vendored assets + Alpine CSP build + its template convention test (D1, D2); Dockerfile, CI, CD, Caddy + Postgres compose, backups (D9–D11) |
| S2 | permission checks by key; per-user permission routes removed (D4, D14); password rule (D8) |
| S3 | the whole slice (D6) |
| S4 | the whole slice (D7) |
| S12 | `/setup` demo checkbox, demo data page (D3, D15) |
| S14 | editable role permission grid (D4) |
| S19 | multipart image upload in provider product forms (D7) |

### 8.12 Round-2 answers

| Q | Answer | Effect |
|---|---|---|
| Q-A | Postgres on the VM | `db` compose service, two DB roles, nightly `pg_dump` to GCS + disk snapshots, restore drill before launch (§8.9) |
| Q-B | Caddy on the VM | Let's Encrypt; `TRUSTED_PROXY_HOPS=1` |
| Q-C | Only demo data | no image import; `image_url` not in the baseline |
| Q-D | Never deployed | greenfield: one squashed baseline, append-only afterwards; no URL/cookie/email compatibility |
| Q-E | Start fresh | role grants only, no `user_permissions` |
| Q-F | Demo data everywhere | no `DEMO_DATA_ENABLED` flag; removal is one click, with a warning banner while demo rows exist |

**Still needed before S1 deploys (not before S0):**
- the **domain name**, so Caddy can issue a certificate
- the **GCP project ID and region/zone**, for the VM, Artifact Registry, buckets and Workload Identity Federation

S0 can start now.

---

## 9. Progress log

### S0 — workspace, errors, baseline (branch `migration/s0-workspace-baseline`)

Done as planned:
- Workspace of `domain` / `application` / `infrastructure` / `web` / `migration`, plus a lint-free `crates/tests`.
- `[workspace.dependencies]` with `default-features = false` and a reason per feature; `[workspace.lints]` denies unwrap/expect/panic/unreachable/todo/unimplemented/indexing.
- One `domain::Error` with machine codes. Text lives in `web/locales/vi.toml`. The renderer middleware writes the body; extractor rejections are coded too.
- Single DDL-only baseline migration; the old migrations and the stray `.sql` file are deleted.
- Startup catalogue seeding with one-time default grants (`catalogue_state`), under an advisory lock.
- `DROP TABLE seaql_migrations` recovery removed; startup failures are typed errors instead of panics.

Pulled forward from later slices, because S0 touched the same code:

| Item | Planned in | Why now |
|---|---|---|
| Password rule: min 8 chars, max 72 bytes (D8) | S2 | `validation.rs` was rewritten for coded errors anyway |
| Per-user permission routes and the SvelteKit toggles removed | S2 | `user_permissions` is not in the baseline, so the routes had nothing to act on |
| Demo loader/remover in the backend (`is_demo`, `POST`/`DELETE /api/setup/seed`) | S12 (UI stays there) | the old seed endpoint depended on the deleted migrations; keeping it would have left a dead endpoint |
| `CORS_ALLOW_ORIGINS` → `CORS_ORIGINS`; malformed origin fails startup | S1 | env reads moved into `startup.rs` |
| `JwtTokenService` owns its keys, has `verify`, refuses secrets < 32 bytes | S2 | the middleware's global key statics had to go to move JWT into infrastructure |
| Email: SMTP vs log adapter chosen once at startup with a WARN | S1 | the old adapter's placeholder error mapping (G16) |
| Product search filters grouped into `ProductFilter`, shared by `browse` and `count_browse` | — | clippy's argument limit; the two had duplicated filter code |

Tests:
- Unit: error codes, validation, catalogue integrity, JWT, locale key sets, migration registration.
- Router: coded 404/400/401/403 bodies, security headers kept on error responses, internal detail hidden.
- Conventions: layer imports, env reads, extractors, ad-hoc statuses, local time, lints opt-in, workspace versions.
- Database (`--include-ignored` with `TEST_DATABASE_URL`): catalogue idempotence, revocation survives restart, later permissions reach their roles, admin category edits survive, demo load/remove spares real rows, registered providers hold exactly the provider permissions, registration reports all bad fields.

Verified against local PostgreSQL 18 + PostGIS: all 47 tests pass with `--include-ignored`. A server start on a fresh database passed an end-to-end check: create the admin, block a second admin, register, log in, add a product, reject a wrong password, load demo data once, remove it.

Bugs the smoke run caught and fixed:
- Field messages shared one parameter list, so the password error said "at least 3" (the username's limit). Parameters now belong to each field, with a regression test.
- An empty `RUST_LOG` parsed as a filter that hid WARN, silencing the fallback warnings. Empty now means "use the default".

### S1 — web shell, CI/CD, deployment (same branch)

Done:
- **Ops:** `/health/live`, `/health/ready` (503 `database_unavailable` only when the primary is down); `x-request-id` on every response and in every JSON log line; graceful shutdown; `Databases { primary, read }` with optional `DATABASE_READ_URL`; `BIND_ADDR`.
- **Web shell:**
  - `views::render_page` is the only renderer; page CSP `script-src 'self'` (Alpine CSP build, no `unsafe-eval`).
  - Templates and static files are embedded with rust-embed.
  - Tera functions `url_for` (port of `routes.ts`, typo-proof) and `asset_url` (content-hashed, immutable caching); filters `vnd`, `vn_datetime`, `utc_iso`.
  - Browsers get a localised HTML error page; API callers keep JSON.
- **Assets:** self-hosted Bootstrap 5.3.8 CSS, Font Awesome 6.5.1 solid, Be Vietnam Pro (latin + vietnamese), Alpine CSP 3.14.9; provenance and hashes in `static/vendor/VERSIONS`.
- **Proxy:** `LEGACY_FRONTEND_URL` forwards unmigrated pages to SvelteKit, keeping the browser Host so its CSRF check passes. Verified end to end against `vite dev`: home page, and a login form action whose API error comes back in Vietnamese.
- **Client IP:** `TRUSTED_PROXY_HOPS` resolves the real client behind Caddy; rate limits are per client, and a forged `X-Forwarded-For` prefix buys nothing.
- **Deployment:**
  - Images: `backend/Dockerfile` (distroless, non-root, built-in `healthcheck` subcommand) and `client-app/Dockerfile`.
  - On the VM: `deploy/compose.yaml` (Caddy, web, legacy, PostGIS on a separate disk, non-superuser app role) and `remote-deploy.sh` (secrets from Secret Manager, readiness gate, automatic rollback).
  - Backups: nightly `pg_dump` to GCS plus weekly disk snapshots.
  - GitHub Actions: CI (fmt, clippy, all tests incl. database, svelte-check/build, image builds, shellcheck, compose config) and Deploy (Workload Identity Federation, production environment approval).
- **Restore:** rehearsed locally. It caught a broken first draft (`spatial_ref_sys` data); the backup now excludes it.

Deviations from §5/§8:
- **Moved into S1:** `TRUSTED_PROXY_HOPS` (planned for S2). The first deploy sits behind Caddy, so it could not wait.
- **Deferred to the page slices:** the bulk of the Svelte component styles. Each one moves into `app.css` with the page that uses it, so no CSS ships unused. Likewise `js/app.js` arrives with the first Alpine component (S5) and `file_url` with media (S4).
- **Simpler build than planned:** Docker cache mounts instead of cargo-chef, one fewer tool to pin.
- **Image versions:** PostGIS `16-3.5` instead of `16-3.4`, in CI and production alike.
- **SvelteKit fixes:** the 6 pre-existing `svelte-check` errors are fixed so CI can gate on it. `CategoryGrid` got its `<script module>`, and the two never-rendered redirect pages were emptied.

Not verified here: no Docker on this machine, so the image builds, the compose stack and both workflows run for the first time on GitHub. Everything else ran locally: 88 Rust tests, svelte-check, npm build, shellcheck, YAML parsing, and the restore rehearsal.

Still needed from you before the first deploy: the domain, the GCP project, and the one-time setup in `deploy/README.md` (steps 1–8).
