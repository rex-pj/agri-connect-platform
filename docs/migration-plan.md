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

**One source for the expression.** `migration::search::fold(col) -> String` returns `lower(f_unaccent(<col>))`. The migration uses it to build the index; the repositories use it (through `Expr::cust_with_values`) to build `fold(col) LIKE fold($1)`. The same function produces both sides, so they can't drift apart. The bound pattern is `%<query>%`, with `%`, `_` and `\` in the query escaped first.

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

### S2 — sessions, permission checks, CSRF, events (same branch)

Done:
- **Sessions:** `MaybeActor` reads the session from `Authorization: Bearer` or the `auth_token` cookie. The cookie is the one SvelteKit sets, so Rust pages and islands share the login. Middleware never decides access.
- **Fresh permissions:** they are loaded from the database on every request (read pool). A revoked permission, or a deactivated account, takes effect on the very next request rather than when the token expires.
- **Permission checks:** every state-changing use case and every admin read starts with `actor.require(PERM_…)?`. `RequireAdmin` / `RequireProviderOrAdmin` and the admin role bypass are gone. Convention tests reject role comparisons and fail when a use case taking an `Actor` checks nothing.
- **Self-protection:** an admin cannot change their own role or deactivate themselves (`cannot_change_own_account`).
- **CSRF:** non-GET requests with a foreign `Origin` are refused (`cross_origin_request`); allowed origins are `APP_URL` plus `CORS_ORIGINS`; a request without `Origin` passes. `APP_URL` now defaults to `http://localhost:3000`, the Rust front door.
- **Rate limits:** a `RateLimiter` port with an in-process adapter. Three budgets: auth 10/min, public writes 30/min, signed-in writes 120/min (writes only). Idle entries are pruned every 5 minutes.
- **Events:** domain events go through an in-process bus; the audit log is a subscriber, and action names match the old history. Role changes are audited now; they were not before.
- **Verified end to end:**
  - Through SvelteKit: login form → cookie → admin dashboard.
  - The API on the cookie alone.
  - A cross-site form POST refused with 403.

Moved to S12 (the Rust login page), because nothing issues cookies from Rust before then:
- refresh tokens (table, rotation, reuse detection);
- the `SameSite=Lax` cookie and its test.

### S3 — accent-insensitive search (same branch)

Done:
- **Migration** `m20261014_000002_search`: the `unaccent` and `pg_trgm` extensions, the `f_unaccent` IMMUTABLE wrapper, and GIN trigram indexes on `lower(f_unaccent(..))` for business and product name and description.
- **One expression:** `migration::search::fold` builds the indexes; `infrastructure::search::text_contains` builds the query from the same function. Each listing names its searched columns in one place, and a unit test fails if a searched column has no index.
- **Applies to:** provider browse/count (`name=`) and product browse/count (`name=`). Description matches count too.
- **Literal wildcards:** a typed `%` or `_` matches itself, not everything.

Tests (database):
- `phan bon`, `dong thap` (the `đ` case), `DICH VU`, a description match, and two product cases.
- The accented query still works.
- `50%` and a bare `%` match only the row that contains `%`.
- `EXPLAIN` with `enable_seqscan = off` shows each `ix_*_fold` index in use.

Found on the way:
- sea-query 0.32 binds only `$1`-style placeholders in a custom Postgres expression; a `?` stays in the SQL unbound and Postgres reports a syntax error. So the `%` wildcards are part of the bound value, not SQL literals.

### S4 — media storage (same branch)

Done:
- **Migration** `m20261014_000003_files`: the `files` table, `products.image_file_id` (FK, `ON DELETE SET NULL`), and an index on it so deleting a file does not scan products.
- **Ports:** `FileStorage` (put/get/delete) and `ImageProcessor`. One `object_store` adapter covers GCS (`GCS_BUCKET`, VM service account through the metadata server), local disk (`UPLOAD_DIR`) and memory (tests).
- **Pipeline** (`MediaUseCase::store_product_image`):
  1. `own.listing.update` and ownership are checked before any decoding.
  2. Size cap: `UPLOAD_MAX_BYTES`, enforced by the body limit and again in the use case.
  3. Type sniffed from the magic bytes.
  4. Pixel cap read from the header before decoding.
  5. EXIF orientation applied, then the image is resized (1600/400) and re-encoded as JPEG q80. The test fixture's EXIF block does not survive.
  6. Both objects are stored, then the row; whatever was written is removed again on failure.
- **API:** `PUT /api/provider/products/{id}/image` (multipart field `image`), with its own rate-limit group (20/min). `image_file_id` appears in the product JSON. Admin and provider product DTOs are now built through `From` impls instead of four copies of the same mapping.
- **Serving:** `GET /files/{id}` and `/files/{id}/thumb`, with the stored type, `immutable` caching and `nosniff`. Templates use `file_url(id, variant)`. `/files/` is never proxied to SvelteKit.
- **Cleanup:** a `FileCleanupSubscriber` on the event bus removes the image a product replaced (`Change::ImageReplaced`) or the image of a product just deleted (`Change::Deleted`). Provider deletions now publish events too, so they are audited as well.
- **Deploy:** an `UPLOAD_BUCKET` GitHub variable becomes `GCS_BUCKET`, and deploys refuse to run without it. README step 7 creates the bucket: private, versioned, noncurrent versions kept 30 days so a restored database still finds its images.

Changed from §8.7, and why:
- `FileStorage::put` takes no content type. The type lives in the `files` row, and the local-disk store cannot keep one on the object.
- `get` returns the whole object rather than a stream. Stored images are re-encoded and capped at a few hundred KB, so streaming would buy nothing.
- The decoding sits behind an `ImageProcessor` port: application code must not name the `image` crate.
- There are no `ProductImageReplaced`/`ProductDeleted` event types. A `Change::ImageReplaced { previous }` variant was added instead, and the existing `Change::Deleted` is reused.
- Phone photos are rotated by their EXIF orientation before EXIF is dropped. Without that, portrait photos would come out sideways.

Tests:
- Unit: EXIF dropped and its rotation applied, resizing, transparent PNG onto white, WebP accepted, GIF/SVG/empty refused, truncated JPEG reported as invalid, a PNG header claiming 50 000 × 50 000 refused before decoding, storage round trips (memory and disk), `file_url`.
- Router: an anonymous upload gets 401 before its body is read.
- Database: upload and serve (headers, both variants, id in the public product JSON); a replaced image is removed; deleting the product removes its image; another provider's product answers 404 and nothing is stored; too large (both limits), a renamed script, a missing field and an empty file each get their code, and nothing is stored.

Not done: a run of the real server over TCP. There is no `psql` on this machine to create a throwaway database, and I did not want test accounts in the dev database. The router tests cover the same path in-process.

### S5 — app layout, home, error pages (same branch)

Done:
- **`/` is served by Rust** (`web/src/pages/home.rs`). It shows product-category tiles from the catalogue plus the two shortcuts, 6 featured products, the near-me banner and 3 featured providers, and the empty state when there is nothing to show. The SvelteKit `(app)/+page.svelte` and `+page.server.ts` are deleted; `svelte-check` reports 0 errors.
- **App frame** (`layouts/app.html`): top bar with search and account menu, plus the bottom tab bar. `web::shell::Shell` builds it on the server from the session:
  - the last tab and the menu entry point to the admin panel when the user has `admin.dashboard.view`, to the provider dashboard when they have `own.listing.create`, and to the account page otherwise. Permissions decide, never role names;
  - an expired or forged cookie renders the page signed out instead of a 401.
- **Error pages** now sit in the frame, without the account menu: the renderer does not resolve the session.
- **Alpine:** `static/js/app.js` registers `navSearch` and `profileMenu`. Without JS, the search button is a link to `/search` and the account button a link to `/account`. Opening one closes the other, and Escape closes the menu.
- **Templates by atomic level:**
  - atoms `badge`, `tag`;
  - molecules `category_item`, `section_header`, `geo_cta`, `empty_state`;
  - organisms `app_nav`, `bottom_nav`, `category_grid`, `product_card`, `provider_card`.
  Their styles moved from the Svelte `<style>` blocks into `app.css`.
- **Data:** products now carry `category_icon` from the catalogue, so a card without a photo shows its category icon. This replaces the hard-coded name→icon map in `categoryIcons.ts`. Cards with a photo use `file_url(variant="thumb")`.
- **Slugs:** `web::slug::slugify`, exposed as a Tera filter, mirrors `slugify.ts`. Its fixtures were run through the TS logic with Node and give the same output.
- **Logout** in the menu posts to SvelteKit's `/account?/logout` action (route `account_logout`) until S12 brings the Rust auth pages.

Tests:
- Unit: tab and menu choice by permission, a single active tab, query decoding for the search bar, slug fixtures, provider district.
- Database:
  - with demo data, the home page has product and provider links using the SvelteKit slugs, `tel:` links and category tiles;
  - an empty platform shows the empty state;
  - the frame for anonymous, provider and admin sessions;
  - a stale cookie renders the page signed out.
- Router: error pages have the frame but no account menu; `app.js` is served versioned (existing asset test).
- Smoke: the real server answered `/` over HTTP against the local dev database (read-only), with page CSP, category tiles and the empty state. That database has no demo data, so the populated case rests on the integration test.

Deferred: the distance line on provider cards waits for the explore page (S11), which is the only page that knows a distance.

### S6 — product detail (same branch)

Done:
- **`/products/{id}` and `/products/{id}/{slug}` are served by Rust** (`web/src/pages/product.rs`). The SvelteKit `products/[id]` routes are deleted.
- **One canonical URL per product:** `/products/{id}/{slug of the name}`. Every other spelling answers **301** (not axum's 308) to it: no slug, a stale slug after a rename, or a padded id such as `07`.
- **404 page** for a missing, deactivated or deleted product, and for an id that is not a number or overflows. A person typed the address, so "not found" fits better than "bad request".
- **Page:** hero with a back link and the category badge, price card, description (line breaks kept), and seller with a call button. With a photo it is two columns on a wide screen and shows the full-size image (`file_url` without variant).
- **SEO:** the title is `{name} — {site}`. The meta description is the description on one line, cut to 150 characters. A product without one gets "Mua … từ … trên …".
- **New molecule** `page_hero`, for S7's provider page too. Its styles and the detail styles moved into `app.css`.
- **Slug parity is now a test on both sides:**
  - `slugify.ts` became `slugify.js` with JSDoc types, so Node 20 runs it without a build step;
  - `slugify.fixtures.json` is read by `npm test` (new, added to CI) and by the Rust `web::slug` test;
  - the fixture moves into the backend in S20.
- **Test helpers:** `Env`, `register_provider`, `create_product`, `photo`/`multipart`/`upload_request`/`put_photo` now live in `crates/tests/src/lib.rs`. `authz`, `home` and `media` used to carry copies of them.

Tests (database):
- the three non-canonical spellings 301 to the canonical URL;
- the page shows price, unit, description, the seller and a `tel:` link, with the page CSP;
- a photo shows the full image;
- the meta fallback without a description;
- deactivated, missing, non-numeric and overflowing ids give a 404 page.

### S7 — provider detail and the contact form (same branch)

Done:
- **`/providers/{id}` and `/providers/{id}/{slug}` are served by Rust** (`web/src/pages/provider.rs`). They follow the same canonical-URL rules as products: 301 for any other spelling, a 404 page for missing or non-numeric ids. The SvelteKit `providers/[id]` routes are deleted.
- **Page:** hero with address, verified badge and share button; phone with call and "Gửi yêu cầu" buttons; description; services with their category icon (`category_icon` now comes with services too, replacing `getServiceDisplay`).
- **`POST /contact`:** a plain urlencoded form (`ApiForm`, new) under the public-write rate limit and the Origin check. It follows Post/Redirect/Get:
  - success → 303 to the provider page plus a one-time flash cookie (`cookies::Flash`). A cookie, not `?sent=1`, so a shared or reloaded URL does not say "sent" again;
  - a broken rule → the page re-rendered with the error's own status (400), the typed values and one localised message per field;
  - a missing provider → 404.
- **L14 closed:** the only rules are `ExploreUseCase::contact_request`, shared with the JSON endpoint. The SvelteKit form labelled the name "optional" while its server required it; the Rust form marks it required, as the rule is.
- **No-JS first:** the form is a section of the page (`#lien-he`) and "Gửi yêu cầu" is an anchor to it. The Alpine `dialog` component turns it into a bottom sheet, opens it by itself when the server re-shows errors, and focuses the phone field. `shareButton` uses the phone's share sheet or copies the link, and is hidden without JS.
- **New templates:**
  - atom `alert`;
  - molecules `form_field` (input/select/textarea with linked error text), `share_button`;
  - organism `contact_sheet`.
- **`web::cookies`** now holds the request-cookie reader that auth used privately, plus the flash.
- **Convention:** handlers may not use axum's `Form` either. The Alpine check caught a template-supplied `x-ref` value; the field macro now takes a `first` flag instead.

Tests (database):
- canonical 301s; the page shows services, price note, `tel:`, the form posting to `/contact` with the business id, and the anchor fallback;
- a valid post → 303 and the flash cookie, one inquiry stored; the next page shows the thanks once and clears the cookie;
- an invalid post → 400 page with messages linked to their fields, typed values kept, a `<script>` name echoed escaped, the sheet set to open, nothing stored;
- a missing provider → 404; a cross-site post → 403 and nothing stored;
- missing provider pages → 404.

### S8 — product listings (same branch)

Done:
- **`/products` and `/products/category/{id}/{slug}` are served by Rust** (`web/src/pages/products.rs`). The SvelteKit `products` route directory is gone.
- **One GET form** carries the search text, category, price range and sort, so every state is a URL. Category and sort chips are radio buttons styled with `:has(:checked)`. The filter panel is open without JS; the Alpine `collapse` component folds it behind the sliders button, which shows a count of active filters. "Xóa bộ lọc" is a link that keeps the search text.
- **Lenient queries:** a malformed number, a negative price, an unknown category id or an unknown sort is ignored, not a 400 page. A hand-edited link still shows products.
- **`ProductSort`** is now a domain enum (`newest`, `price_asc`, `price_desc`), used by the repository, the use case, the JSON API and the page. The repository no longer matches loose strings.
- **Pagination** (`pages/listing.rs`, ready for S9): real links that keep the filters. Page 1 has no `page` parameter. The window is first and last, ±2 around the current page, with ellipses, ported from `PaginationNav.svelte`.
- **Category pages** use the category's own slug as canonical (a stale one → 301, keeping `?page=`). A service category id or an unknown id → 404. The hero shows the category's catalogue description and the count.
- **`page_hero`** gained a list variant, title icon, watermark and subtitle. New: molecule `pagination_nav`, organism `product_filters`.
- **Tests:**
  - test databases are dropped `WITH (FORCE)`: a pooled connection still closing made the drop fail now and then;
  - the proxy tests now use `/provider/dashboard` as the not-yet-migrated page.

Tests:
- Unit: query encoding matches a browser form submit, lenient parsing, the page-number window, pagination links keep filters and drop page 1, `ProductSort` round-trips.
- Database:
  - text (accent-insensitive), category, price range and both sorts through the page; the form echoes its state and the active-filter badge;
  - hand-edited queries still list everything;
  - the empty state's reset link keeps the search;
  - 28 products page as 20 + 8, with filter-keeping links;
  - category canonical, 301 with `page` kept, and 404 for service, unknown and non-numeric ids.

### S9 — service listings (same branch)

Done:
- **`/services` and `/services/category/{id}/{slug}` are served by Rust** (`web/src/pages/services.rs`). The SvelteKit `services` route directory is gone.
- **The provider directory** works like the product list: one GET form (text, service, sort), lenient parsing and pagination links. Each card shows the provider's district and services, with call and detail buttons. The near-me banner sits above the results. A provider with no active service is never listed (unchanged rule).
- **`ProviderSort`** (`verified_first` default, `newest`, `name_asc`) is now a domain enum, like `ProductSort`, end to end through the JSON API.
- **One filter organism:** `list_filters` replaces `product_filters`. It is driven by `listing::FilterForm`, which carries the category parameter name, the sort chips with labels, and an optional price range. Category pages share `find_category`, `redirect_to_canonical` and `category_sub`.
- **SEO:** `/services` now has a title and a meta description; the SvelteKit page had neither.

Changed from the plan, and why:
- **No contact dialog on the list pages.** The SvelteKit list pages wired a `ContactSheet` to `ProviderCard`'s `onContact`, but the card never rendered a button for it, so no visitor could ever open it. Contact stays on the provider page (S7), one tap from every card through "Chi tiết", where the form also works without JS.

Tests (database):
- default order is verified first then newest; `name_asc`; accent-insensitive search; service filter with the chip kept; no price filter; a business without services hidden; district and banner shown;
- hand-edited queries still list everything; the empty state's reset link keeps the search;
- category canonical and 301; product-category and unknown ids → 404.
- The product listing tests run unchanged against the shared organism.

### S10 — search page (same branch)

Done:
- **`/search` is served by Rust** (`web/src/pages/search.rs`); the SvelteKit route is deleted. One query searches providers and products together (accent-insensitive, S3). Each kind shows six results, plus "Xem thêm" to `/services?q=` or `/products?q=` when there are more. The page is `noindex`, and the top bar's search box stays empty there, as before.
- **Tabs without JS:** the tabs are links (`?q=…&tab=services`). The server renders every section and marks the ones outside the chosen tab `hidden`. An unknown tab means "all".
- **Alpine `tabs`:** a click switches the panels in place (`:hidden`) and puts the tab's link in the address bar, so a reload or a shared link keeps the tab. The current tab is marked by `aria-current`, set by the server and then by Alpine, not by a class. Alpine's `:class` keeps static classes, so a server-rendered `active` class could never be removed.
- **New molecule** `search_bar`.

Tests (database):
- both counts, tab links carrying the query, `noindex`, all sections visible on "all", no "see more" under six;
- `tab=services` hides the products section in the HTML itself;
- 8 providers and 9 products preview six each with links to the full lists;
- no query asks for one; nothing found shows both empty states with links to the lists; the top-bar box is not pre-filled.

### S11 — near-me page and the island toolchain (same branch)

Done:
- **`/explore` is served by Rust** (`web/src/pages/explore.rs`); the SvelteKit route is deleted.
- **Position from two sources:**
  - **No JS:** a GET form with the catalogue's location presets (`location_id`).
  - **JS:** the `ac-geolocate` island asks for the device position and goes to `/explore?lat=…&lng=…`, keeping the radius and service.
  - Bad coordinates, an unknown preset or nothing at all show the onboarding, not an error. The coordinate bounds are the use case's own constants.
- **Filters are links:** radius (5/10/25/50 km, default 10) and the service tiles. Each keeps the position; the active tile clears the filter. Results show the distance on each card ("995 m", "8.0 km (~16 phút xe máy)"), the S5 card field deferred until now. An empty result offers the next larger radius.
- **Privacy:** coordinates travel in the query string, as before, but are never logged. The request span records the path only.
- **Island toolchain** (`backend/islands/`):
  - Svelte 5 custom elements with Vite, pinned to the versions client-app uses;
  - built as one minified IIFE, `static/js/islands.js` (15 KB gzip), committed so the Rust image needs no Node. A rebuild is byte-identical;
  - islands use light DOM and carry no `<style>`, so `style-src 'self'` holds; the bundle contains no `eval`/`new Function`;
  - all user-facing text is passed as attributes from the template;
  - only pages that use an island load the script (`{% block scripts %}` in `base.html`);
  - new CI job `islands`: `npm ci && npm run build`, then `git diff --exit-code` on the bundle.

Bug found and fixed:
- **Geo search and saving a location were broken.** `search_geo` (distance and radius) and `update_location` built their PostGIS SQL with `?` placeholders in `Expr::cust_with_values`. sea-query 0.32 leaves those unbound for Postgres (the S3 finding). The near-me search and a provider saving GPS coordinates both answered 500: confirmed by running the new tests against the old code. They now use `$1`/`$2`. A new convention test fails on any `?` inside `cust_with_values` in infrastructure; it fails on the old code and passes on the fix.

Tests (database):
- onboarding: island tag, versioned island script, preset form, service tiles linking to service pages;
- a preset search: nearest first, radius 5/10/25 km give 1/2/3 results, distance texts, preset name, links that keep `location_id`;
- device coordinates: island `href` keeps radius and service; the active tile clears the filter;
- malformed, off-globe and unknown positions → onboarding; nothing near → "Tìm trong 25 km";
- a provider saves coordinates through the API and the geo API then finds them within 1 km.

### S12a — browser sessions: refresh tokens, Lax cookies, logout (same branch)

S12 ships in two commits. This one is the session machinery; the auth and setup pages follow in S12b.

Done:
- **Migration** `m20261015_000004_refresh_tokens`: `id`, `family_id`, `user_id` (cascade), `token_hash` (unique), `expires_at`, `used_at`, `revoked_at`, indexed by family, user and expiry.
- **`AuthUseCase` sessions** (Q9):
  - `start_session` opens a new family;
  - `refresh_session` rotates. The claim is one conditional UPDATE, so of two concurrent rotations exactly one wins. Reuse of a spent token within `REFRESH_REUSE_GRACE_SECS` (30 s) is refused without revoking, because it is two tabs. Later reuse revokes the family. A deactivated or role-less account cannot renew;
  - `end_session` (logout) revokes the family; `purge_expired_sessions` runs hourly;
  - a password reset now ends every session of the account.
  - Only SHA-256 hashes are stored; the raw token lives in the cookie. The constructor became `AuthDeps`, like `AdminDeps`. `LoginResponse` carries `user_id`, not serialised, so the JSON API is unchanged.
- **`web::session`:**
  - `CookiePolicy` (from `HTTPS_ENABLED`) writes both cookies: httpOnly, `SameSite=Lax`, `Secure` behind TLS, `Max-Age` 1 h and 30 days;
  - **`renew` middleware**, inside the Origin check: when the access cookie is missing or invalid but a refresh cookie exists, it rotates the pair. It then rewrites this request's `Cookie` header, so Rust handlers and SvelteKit behind the proxy see the new session at once, and sets both cookies on the response;
  - a dead refresh token clears both cookies;
  - assets, files, health probes and bearer-token API calls never trigger it.
- **`POST /logout`** (→ `/`) and **`POST /admin/logout`** (→ `/login`) in Rust, under the public-write budget. The menu's logout and the SvelteKit account page, layout and admin shell now post there via `ROUTES.LOGOUT` / `ROUTES.ADMIN.LOGOUT`. SvelteKit's own logout action and `admin/logout` route are deleted: they cleared only `auth_token`, and the refresh cookie would have signed the user straight back in.
- **`REFRESH_TOKEN_DAYS`** (default 30) in `startup.rs` and `.env.example`.

Why Lax and not Strict: with Strict, a link opened from Zalo or Messenger arrives without cookies, so farmers would look signed out. Lax still withholds the cookies from cross-site POSTs, and every non-GET passes the Origin check too.

Tests:
- Unit: the rewritten Cookie header keeps other cookies; cookie attributes (Lax, HttpOnly, Secure only with TLS, Max-Age); assets never renew.
- Database:
  - rotation;
  - reuse within the grace period keeps the family, later reuse revokes it, including the legitimate token;
  - only 64-hex hashes are stored;
  - the middleware renews for the same request (200 from `/api/users/me` with only a refresh cookie) and rotates both cookies; a bad refresh → 401 and cleared cookies;
  - a valid access cookie or a bearer header never rotates;
  - both logout routes → 303, cleared cookies, dead family;
  - a password reset and a deactivation end sessions;
  - expired tokens are refused and purged.

### S12b — auth and setup pages (same branch)

Done:
- **`/login`, `/register`, `/forgot-password`, `/reset-password`, `/setup`, `/setup/seed` are served by Rust** (`web/src/pages/auth.rs`, `setup.rs`) on a new `layouts/auth.html` (one card, `noindex, nofollow`). The SvelteKit routes are deleted, and so is its now-unused cookie writer in `lib/server/api.ts`.
- **Bug 0.1 fixed:** a signed-in visitor on `/login` or `/register`, and every successful sign-in, lands on their own area by permission: admin dashboard, provider dashboard or account. This is the same rule as the last tab (`shell::home_area`). It used to be the admin dashboard for everyone.
- **Bug 0.2 fixed:** `/login` sends a fresh installation to `/setup`, and `/setup` goes to `/login` once an admin exists. Both call `AuthUseCase::needs_first_admin` directly. A setup that loses the race to another (`admin_already_exists`) also goes to `/login`.
- **Sign-in starts a session** (S12a cookies) everywhere: login, registration, setup.
- **Forms:**
  - fields are re-shown with one message each; passwords are never echoed;
  - a mistyped confirmation is `password_mismatch` on the confirmation field, checked before the use case runs, so a typo never creates an account;
  - the lockout replaces the form with its own panel (423) and a reset link;
  - `/forgot-password` gives the same answer for known and unknown addresses;
  - a used or expired reset link shows "Liên kết không hợp lệ";
  - after a reset, `/login` thanks the user once (flash, replacing `?reset=1`).
- **Setup:** the demo data box is unticked by default (§8.4). When ticked, the demo data loads in the same request. A failed load is logged and can be retried from `/setup/seed`; the admin exists either way.
- **`/setup/seed`:** a new `SetupUseCase::demo_status` requires `system.demo.manage`. The page shows the counts. Load is one button; Remove needs a ticked confirmation (`confirmation_required`), so it is safe without JS. A provider gets 403, and an anonymous visitor goes to `/login`.
- **Shared:**
  - `pages::forms::{FormErrors, is_form_error, confirm_password}`, also used by the contact form now;
  - `Viewer`, a lenient session extractor that `Shell` uses too;
  - `limits` (password and username minimums, reset link hours) in every template context, so forms show `minlength` without repeating numbers.
- **Alpine `passwordToggle`** on every password field (hidden without JS).
- **Rate limits:** the page forms use the auth budget (10/min) on writes only; `/setup/seed` uses the signed-in budget.

Bug found and fixed:
- **The account lockout never worked.** `record_failed_login` built its UPDATE with `?` placeholders in `Expr::cust_with_exprs`. sea-query left them unbound, the UPDATE failed, and the use case deliberately swallows that failure (a wrong password must not become a 500). Five wrong passwords therefore never locked anything. The new login test caught it. The SQL now uses `$1`/`$2`, and the placeholder convention test covers `cust_with_exprs` too; it fails on the old code.

Not done:
- `register` and `create_first_admin` do not validate the email address beyond the browser's `type=email`.
- `create_first_admin` is check-then-insert, so two simultaneous setups could both succeed. Both are rate-limited and one-time; noted for a later hardening pass.
- Password reset tokens are still stored raw (refresh tokens are hashed).

Tests (database):
- a fresh install redirects `/login` → `/setup`; setup opens a session, then closes; a signed-in admin on `/login` goes to `/admin/dashboard`;
- a mismatch → 400, nothing created, no password echoed; the demo box loads demo data and flashes;
- a wrong password → 401 with the username kept; a provider lands on `/provider/dashboard`; five failures → 423 lockout panel;
- registration: a field error under the phone, values kept; success signs in; signed-in `/register` and `/login` redirect;
- the reset flow: the same answer for unknown emails, the invalid-link state, success → `/login` with a one-time thanks, a token works once;
- the demo page: anonymous → `/login`, provider → 403, removal needs the tick, only demo rows are removed;
- a cross-site login post → 403 and no cookies.

### S13 — account page (same branch)

Done:
- **`/account` is served by Rust** (`web/src/pages/account.rs`) in the app frame, `noindex`. An anonymous visitor goes to `/login`. The SvelteKit `account` route is deleted.
- **Page:** name, username, email, role (display only), join date (`vn_datetime`), and a link to the visitor's area (admin or provider dashboard), the same rule as the tabs.
- **Password change:** `POST /account/password`, Post/Redirect/Get. A wrong current password (401) or a mismatched confirmation (400) is re-shown in place.
  - **It now signs out every other device:** `AuthUseCase::change_password` revokes all refresh tokens of the account, including through the JSON API. The browser that made the change gets a fresh session, so it stays signed in.
  - After the change, a one-time flash confirms it on `#security`.
- **Logout** posts to `/logout` (S12a).
- Dropped the SvelteKit "session expires at" row. With refresh tokens the 1-hour access expiry is renewed silently and no longer tells the user anything.

Tests (database):
- anonymous → `/login`; the profile, role, area link, logout form and `#security` anchor;
- wrong current password → 401; mismatch → 400;
- success → 303 to `#security` with new cookies and the flash; another device's refresh token is revoked; the new password logs in.

### S14 — admin frame, dashboard, roles and the permission grid (same branch)

Done:
- **One admin layout** (`layouts/admin.html`): top bar (site, user, home, logout) and a menu built by `pages::admin::AdminNav`. The menu shows **only the entries the visitor's permissions open**, and hides empty groups. On a phone it folds behind a header button (Alpine `collapse`); without JS it sits above the content. Every Rust admin page renders through `render_admin`. The remaining SvelteKit admin pages keep their `AdminShell` until S15–S18.
- **`/admin`** → `/admin/dashboard` (anonymous → `/login`). **The dashboard** shows four counts (`number` filter: `1.234`) and the §8.4 warning while demo listings are live, with a link to `/setup/seed`; it appears only for those who may remove them. The SvelteKit "sắp ra mắt" placeholder cards are not carried over.
- **`/admin/roles`** lists roles with their grant counts. **`/admin/roles/{id}`** is the **editable permission grid**, grouped by key prefix:
  - one checkbox per permission, named `perm.<key>` (an urlencoded form cannot post a repeated name into a list), plus an optional reason;
  - `POST /admin/roles/{id}/permissions` → `PermissionUseCase::set_role_grants`. It needs `role.permission.manage`, rejects unknown keys (`unknown_permission`), and **refuses to take `role.permission.manage` from the caller's own role** (`cannot_revoke_own_access`), because nobody could give it back;
  - read-only (disabled fieldsets) for visitors who may read roles but not manage them.
- **Grants apply on the next request** of every user of the role, since permissions are loaded per request (S2). Each save publishes `Change::PermissionsChanged { added, removed }`; the audit log records `change_role_permissions` with `+key, -key: reason`.
- **`/admin/permissions`**: permissions × roles, read-only, with role headers linking to their grids.
- **Writes:** `RoleGrantsRepositoryTrait::replace` on the primary pool, in one transaction (DELETE … RETURNING, INSERT … ON CONFLICT DO NOTHING RETURNING). `PermissionUseCase` reads grants from the primary too, because the page reads its own writes right after a save. The session's permission lookup stays on the read pool.
- **`/setup/seed`** moved into the admin layout and menu ("Dữ liệu mẫu").
- **Cleanup:** `RoleDto` is built through `From<RoleModel>` instead of two private mappers; `format_vnd` reuses the new `format_number`.

Tests:
- Unit: the menu filters by permission, hides empty groups and keeps a sub-page active; grouping by prefix; only `perm.*` fields count; number grouping.
- Database:
  - redirects for `/admin`; the admin menu; a provider gets 403 on the dashboard; the demo banner;
  - **a grant on the grid lets the provider's existing session into the dashboard on the next request**, its menu shows only that entry, the audit row reads `change_role_permissions +admin.dashboard.view: thử nghiệm`, and revoking closes it again;
  - self-lockout → 403 with the message; an unknown key → 400; a provider posting the grid → 403; nothing changed in each case;
  - the matrix shows every key and links each role.

### S15 — admin users (same branch)

Done:
- **`/admin/users`** (paged list, links to each account) and **`/admin/users/{id}`** are served by Rust in the admin frame. The SvelteKit `admin/users` routes are deleted.
- **Role change:** a select of the active roles posts to `POST /admin/users/{id}/role`. The choices come from a new `AdminUseCase::assignable_roles`, which needs `user.role.update` only, so changing roles does not require `role.read` as well. The role links to its grid when the visitor may read roles.
- **Status:**
  - deactivating goes through a confirmation with an optional reason, `POST /admin/users/{id}/status`. With JS it is the Alpine `dialog` sheet; without JS it is an anchored section of the page;
  - reactivating is one button;
  - a refused change re-opens the confirmation with the message.
- **Rules** stay in the use cases: invalid role codes, and an admin cannot change their own role or status (`cannot_change_own_account`). Effects are immediate because permissions and account status are read per request.
- **Effective permissions** (read-only) list what the role grants; editing them happens on the role page.
- Flashes for role changed, activated and deactivated.

Tests (database):
- the list and detail for an admin; a provider gets 403;
- an invalid role → 400; a role change opens the admin dashboard to the user's **existing session**;
- deactivation signs the user out on their next request; the audit row reads `deactivate_user: <reason>`; reactivation lets them sign in again;
- an admin's own deactivation and demotion → 403, the confirmation re-opened, nothing changed.

Note: while this slice was being tested, the local database stopped answering twice (the Windows service stopped while another process held port 5432). Database tests then fail with `PoolTimedOut` after 30 s each, which can look like a hang when the output is filtered.

### S16 — admin catalogue: categories and locations (same branch)

Done:
- **`/admin/categories`**, **`/admin/categories/{id}`**, **`/admin/locations`** and **`/admin/locations/{id}`** are served by Rust in the admin frame. The SvelteKit `admin/categories` and `admin/locations` routes are deleted.
- **Lists with a create form** below the table, reached by an anchor link. `POST /admin/categories` and `POST /admin/locations` → 303 to the new item, with a flash. A broken rule re-shows the list with the values as typed.
- **Edit forms** on each detail page post to the page's own URL. A category's type is chosen on create only.
- **Visibility and deletion:**
  - hiding a category goes through a confirmation with a reason (`POST …/status`), because it takes the category's listings off the public pages; showing it again is one button;
  - hiding or showing a location is one button;
  - deleting a location goes through a confirmation with a reason (`POST /admin/locations/{id}/delete`) → 303 to the list.
- **Rules**, all in the use case and reported per field:
  - name, slug and label are required; the slug must be lowercase ASCII words separated by single hyphens (`invalid_slug`);
  - every box has a maximum length (`field_too_long`, limits in `application::constants`, which the templates also use for `maxlength`);
  - the category type is now a field error rather than a whole-request error;
  - coordinates are checked by `validate_location`, which create and update now share.
- **UNIQUE constraints:** a write that loses to a concurrent one (same slug, same label) is mapped by `infrastructure::repositories::unique_or_internal` to a field error (`slug_taken`, `location_label_taken`) instead of a 500. There is a new `Error::field` constructor for this case.
- **Page form helpers:**
  - in `pages::forms`: `whole_number` (blank → 0, otherwise `invalid_number`), `coordinate` (accepts a comma; text that is not a number becomes NaN, so the coordinate rule reports it with the other messages) and `reason`;
  - in `pages::admin`: `is_refusal` and the `Outcome<F>` struct, used by the user pages as well.
- **`organisms/confirm_sheet.html`:** the confirmation sheet, now shared by user deactivation, category hiding and location deletion. `organisms/category_fields.html` and `location_fields.html` hold the boxes shared by the create and edit forms.

Tests (database):
- every broken category rule is reported at once, with the typed values kept; a bad display order is reported; a taken slug is reported under the slug box;
- create → flash → edit → hide: the audit row reads `hide_category: <reason>` and the category leaves `/api/explore/categories`;
- a location with a blank label and bad coordinates gets both messages; a comma is accepted as the decimal mark; a duplicate label is refused; hiding removes it from `/api/explore/locations`; deleting it is audited, and its page then returns 404;
- a provider gets 403 on both lists and on create; anonymous visitors are sent to `/login`.

Unit tests cover the number and reason helpers.

### S17 — admin providers (same branch)

Done:
- **`/admin/explore`** (paged list: phone, counts, linked account, badge) and **`/admin/explore/{id}`** are served by Rust in the admin frame. The SvelteKit `admin/explore` routes are deleted.
- **Verification** is now an explicit state, not a toggle:
  - `AdminUseCase::set_verification(id, is_verified, reason)`, backed by the repository's `set_verified`;
  - setting the state a provider already has changes nothing and is not audited, so a form sent twice is harmless;
  - the JSON API takes `{ is_verified, reason? }` (breaking; its only caller was the deleted SvelteKit page);
  - granting the badge is one button; withdrawing it goes through `confirm_sheet` with a reason (`unverify_provider: <reason>` in the audit log).
- **Account assignment** (`POST /admin/explore/{id}/account`): one form. The username starts as the business phone. An existing provider account is linked as-is, so only its username is needed; a new account needs an email and a password. Fixes in the use case:
  - the email is validated, and an email already in use is a field error. Before, the old form sent no email, so the second account it created hit `users.email UNIQUE` and returned a 500;
  - one account manages one business (`provider_account_in_use`), because the provider dashboard shows the business its account owns;
  - admin and other non-provider accounts are refused on the username field;
  - linking is audited (`Change::AccountAssigned` → `assign_provider_account: <username>`).
- **`validation::validate_email`:** a local part, one `@`, a dotted domain without empty labels, no spaces, at most 255 characters. It is used by setup, registration and assignment, which closes the S12b follow-up.
- **`UserRepository::create`** maps a UNIQUE violation, from a concurrent sign-up, to a field error on `email` or `username`.

Tests (database):
- a verification sent twice is audited once; withdrawing it is confirmed and audited with its reason; the JSON API sets the state explicitly;
- a new account with a bad email and a short password gets both messages (the typed email is kept, the password is not echoed); an email already in use goes under the email field; after assignment the owner can sign in, and the audit log names the account;
- an account that already manages a business is refused, and so is an admin username; a provider gets 403;
- the register page reports a malformed email together with the other errors.

### S18 — admin moderation: products, services, inquiries, reviews (same branch)

Done:
- **`/admin/products`**, **`/admin/services`** (+ **`/admin/services/{id}`**), **`/admin/inquiries`** and **`/admin/reviews`** are served by Rust. No admin page is left in SvelteKit, so the whole `(admin)` route group is deleted. Its shared components (`AdminShell`, …) and the `admin.service.ts` methods go in S20.
- **Confirmations on list pages without detail pages:** the "hide" or "delete" link reloads the list with `?hide={id}` or `?delete={id}`, and the server renders that row's `confirm_sheet` open. With JS it is the dialog; without JS it is an anchored form. Showing an item again is one button.
- **Back to the same list page:** row forms send `page` and `provider_id` as hidden fields (`confirm_sheet` gained a `keep` map for this). `pages::admin::ListPosition` rebuilds the redirect from those fields only when they are numbers, so a posted value can never redirect off the list.
- **Products:** hide/show and delete, both audited with the reason; an optional provider filter, linked from the provider page ("Kiểm duyệt sản phẩm").
- **Services:**
  - the list can be narrowed to one provider (linked from the provider page as "Quản lý dịch vụ"), and only then does it offer a create form, since a service belongs to a provider;
  - each service has its own page: edit, hide (confirmation) / show, delete (confirmation) → back to the provider's services;
  - the rules are now one `validate_service`, shared by create and update: name required, maximum lengths (`SERVICE_*_MAX`), an active service category reported on the `category_id` field, all in one round trip;
  - the category choices are the active service categories.
- **Inquiries:** each row is a status form with a select and a save button; the new Alpine `autoSubmit` component submits it on change. Visitors with `inquiry.read` but not `inquiry.manage` see the status only.
- **Reviews:** hide (confirmation with a reason) / show.
- **`domain::constants::CATEGORY_TYPE_SERVICE` / `_PRODUCT`** now name the two types; `CATEGORY_TYPES` and the service-category check use them.

Tests (database):
- a product: the confirmation opens from the list link and keeps the list position; hiding is audited with its reason and lands on the same filtered list; a non-numeric position is dropped (the redirect goes to the plain list); after deletion the list is empty;
- services: no create form without a provider; a blank name and an unparsable category are both reported, with the typed values kept; create → edit → delete returns to the provider's services, after which the service page returns 404;
- an inquiry's status is changed from the list, and an unknown status → 400; a review is hidden from the list's confirmation;
- a provider gets 403 on all four lists.

Unit test: `ListPosition` keeps only numbers.
