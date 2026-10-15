# Agri-Connect Platform

## Project Vision

An aggregator platform for the Southern Vietnam agricultural ecosystem, solving information fragmentation between farmers and service/supply providers. Core values: **Local · Transparent · Timely**.

Target region: Mekong Delta (priority: durian farms in Cai Lay, rice fields in An Giang).

---

## Repository Structure

```
agri-connect-platform/
├── backend/                    # Cargo workspace; layering enforced by crate boundaries
│   ├── crates/
│   │   ├── domain/             # Models, repository traits, the one Error type (no I/O)
│   │   ├── application/        # Use cases, ports, inputs/outputs, validation, catalogue
│   │   ├── infrastructure/     # Sea-ORM entities + repos, JWT, bcrypt, SMTP, seeders
│   │   ├── web/                # Axum: handlers, middleware, routing, extractors, startup
│   │   │   └── locales/vi.toml # User-facing text for every error code
│   │   └── tests/              # Integration + convention tests (no workspace lints)
│   └── migration/              # Sea-ORM migrations: DDL only, append-only
└── client-app/                 # SvelteKit frontend (adapter-node)
    └── src/
        ├── routes/
        │   ├── (app)/          # Public + authenticated app routes
        │   └── (admin)/        # Admin panel routes
        └── lib/
            ├── components/     # Atomic design component library
            ├── server/
            │   └── api.ts      # Server-side API client
            ├── categoryIcons.ts
            ├── routes.ts       # Centralised URL/route constants (ROUTES)
            └── slugify.ts
```

---

## Tech Stack

### Backend

- **Language:** Rust (edition 2021)
- **Framework:** Axum 0.8
- **Async runtime:** Tokio
- **ORM:** Sea-ORM 1.0 with PostgreSQL (replaces raw SQLx)
- **Database:** PostgreSQL with PostGIS extension for geo queries
- **Auth:** JWT (jsonwebtoken 9) + bcrypt 0.15
- **Email:** Lettre 0.11 (SMTP)
- **Serialization:** Serde + serde_json
- **Validation:** validator 0.19
- **Logging:** tracing + tracing-subscriber
- **Error handling:** thiserror
- **IDs:** UUID v4
- **Config:** config + dotenvy

### Frontend

- **Framework:** SvelteKit 2 with Svelte 5, adapter-node
- **UI Framework:** Bootstrap 5.3
- **Icons:** FontAwesome 6 (fa-solid classes)
- **Design methodology:** Atomic Design (atoms → molecules → organisms → pages)
- **Target:** PWA with offline support for weak signal areas; optimized for low-end Android

### Infrastructure

- **Cloud:** Azure (Container Apps or Functions)
- **Integration:** Zalo OA & Mini App API (planned)

---

## Architecture

### Backend Layers

The backend follows a strict layered architecture. Dependencies flow inward only.

```
HTTP Request
    ↓
handlers/          — parse & validate HTTP input, call use case, return HTTP response
    ↓
application/usecases/  — orchestrate domain logic, call repository traits
    ↓
domain/repositories/   — trait definitions (interfaces)
    ↑
infrastructure/repositories/  — Sea-ORM implementations injected at startup
```

- **Handlers** hold no business logic — only HTTP marshalling.
- **Use cases** hold all business logic — they never touch Sea-ORM or HTTP types directly.
- **Repository traits** define what persistence operations exist; the infrastructure layer implements them.
- **`AppState`** wires every use case at startup and is injected into every handler via Axum's `State` extractor.

### Frontend Architecture — Atomic Design

All UI components live under `client-app/src/lib/components/` and are organized by atomic level:

| Level     | Folder       | Examples                                                                                               |
| --------- | ------------ | ------------------------------------------------------------------------------------------------------ |
| Atoms     | `atoms/`     | `Button`, `Badge`, `Tag`, `Spinner`, `Heading`, `Text`, `AlertBanner`                                  |
| Molecules | `molecules/` | `FormField`, `PageHero`, `SectionHeader`, `CategoryItem`, `PaginationNav`, `FilterChips`, `EmptyState` |
| Organisms | `organisms/` | `ProviderCard`, `ProductCard`, `CategoryGrid`, `ContactSheet`, `BottomNav`, `AdminShell`               |

**Rules:**

- Atoms have no dependencies on other components.
- Molecules may compose atoms only.
- Organisms may compose atoms and molecules.
- Page-level composition happens in route files (`+page.svelte`), not in organisms.
- Never add a new component at a lower level when an existing higher-level component already covers the need.

---

## Domain Model

### Core Entities

| Entity                  | Key Fields                                                                                             | Notes                                          |
| ----------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------- |
| `roles`                 | id, code, name, is_active                                                                              | Codes: `admin`, `provider`                     |
| `users`                 | id, username, email, password_hash, full_name, role_id, is_active, failed_login_attempts, locked_until | Soft-deletable                                 |
| `categories`            | id, name, slug, type, icon, display_order, is_active                                                   | Types: `service`, `product`                    |
| `businesses`            | id, name, phone, email, description, address, lat, lng, is_verified, is_active                         | Provider/business listing                      |
| `services`              | id, business_id, category_id, name, description, price_note, is_active                                 | Services offered by a business                 |
| `products`              | id, business_id, category_id, name, description, price, unit, is_active                                | Products sold by a business                    |
| `inquiries`             | id, business_id, service_id, topic, requester_name, requester_phone, note, status                      | Statuses: `new`, `contacted`, `closed`, `spam` |
| `reviews`               | id, business_id, reviewer_name, rating (1–5), comment, is_visible                                      | Moderated by admin                             |
| `password_reset_tokens` | id, user_id, token, expires_at, consumed_at                                                            | Single-use, 1-hour TTL                         |

### Geo Query Pattern

PostGIS `GEOGRAPHY` type (not `GEOMETRY`) is used so distance calculations are in metres on the spheroid. Coordinates are stored as `lat`/`lng` decimal columns plus a computed geography expression, or via `ST_GeographyFromText`.

Point WKT format: `SRID=4326;POINT(<lng> <lat>)` — **longitude comes first**.

```sql
ST_DWithin(location, ST_GeographyFromText($1), $radius_metres)
```

GIST index on `businesses.location` makes radius queries O(log n).

---

## API Endpoints

All routes are assembled in `backend/crates/web/src/routings/app_routing.rs`. Every error body is `{ code, message, field_errors? }`; `code` is the stable machine identifier.

### Public (no auth)

| Method | Path                          | Description                                                                        |
| ------ | ----------------------------- | ---------------------------------------------------------------------------------- |
| GET    | `/health`                     | Health check                                                                       |
| GET    | `/api/explore/providers`      | Geo-search (`lat`, `lng`, `radius?`, `service_id?`) or browse (paginated, `page?`) |
| GET    | `/api/explore/providers/{id}` | Provider detail                                                                    |
| GET    | `/api/explore/featured`       | Home featured data (providers, products, categories)                               |
| GET    | `/api/explore/categories`     | Category listing (filter by `type=service\|product`)                               |
| GET    | `/api/explore/locations`      | Location presets listing                                                           |
| GET    | `/api/explore/products`       | Browse products (paginated, filter by `category_id?`)                              |
| GET    | `/api/explore/products/{id}`  | Product detail                                                                     |
| POST   | `/api/explore/contact`        | Submit contact/inquiry (rate-limited: 30/min)                                      |
| POST   | `/contact`                   | Same inquiry as a page form (urlencoded); 303 + one-time flash, or the page re-shown with field errors |
| GET    | `/files/{id}`, `/files/{id}/thumb` | Uploaded image (JPEG, cached forever; ids are never reused)             |

### Auth (rate-limited: 10 req/min)

| Method | Path                                | Description                                        |
| ------ | ----------------------------------- | -------------------------------------------------- |
| POST   | `/api/auth/tokens`                  | Login → JWT                                        |
| POST   | `/api/users`                        | Provider self-registration                         |
| POST   | `/api/auth/password-resets`         | Send password reset email                          |
| PATCH  | `/api/auth/password-resets/{token}` | Consume reset token, set new password              |
| POST   | `/api/admin/setup`                  | Create first admin account (rate-limited, one-time) |

### User (session required)

| Method | Path                     | Description          |
| ------ | ------------------------ | -------------------- |
| GET    | `/api/users/me`          | Current user profile |
| PATCH  | `/api/users/me/password` | Change password      |

### Admin (each route checks its own permission key, e.g. `user.read`, `provider.verify`)

| Method | Path                                       | Description                       |
| ------ | ------------------------------------------ | --------------------------------- |
| GET    | `/api/admin/stats`                         | Dashboard statistics              |
| GET    | `/api/admin/users`                         | User list (paginated)             |
| GET    | `/api/admin/users/{id}`                    | User detail                       |
| PATCH  | `/api/admin/users/{id}/role`               | Update user role                  |
| PATCH  | `/api/admin/users/{id}/status`             | Toggle user active state          |
| GET    | `/api/admin/users/{id}/permissions`        | Effective permissions (via role)  |
| GET    | `/api/admin/roles`                         | Role listing                      |
| GET    | `/api/admin/roles/{id}`                    | Role detail                       |
| GET    | `/api/admin/permissions`                   | All available permissions         |
| GET    | `/api/admin/categories`                    | Category listing                  |
| POST   | `/api/admin/categories`                    | Create category                   |
| GET    | `/api/admin/categories/{id}`               | Category detail                   |
| PUT    | `/api/admin/categories/{id}`               | Update category                   |
| PATCH  | `/api/admin/categories/{id}/status`        | Toggle category active state      |
| GET    | `/api/admin/providers`                     | Provider list (paginated)         |
| GET    | `/api/admin/providers/{id}`                | Provider detail                   |
| PATCH  | `/api/admin/providers/{id}/verification`   | Toggle verification badge         |
| POST   | `/api/admin/providers/{id}/users`          | Assign user to provider           |
| GET    | `/api/admin/inquiries`                     | Inquiry list (paginated)          |
| PATCH  | `/api/admin/inquiries/{id}/status`         | Update inquiry status             |
| GET    | `/api/admin/reviews`                       | Review list (paginated)           |
| PATCH  | `/api/admin/reviews/{id}/visibility`       | Toggle review visibility          |
| GET    | `/api/admin/locations`                     | Location preset list              |
| POST   | `/api/admin/locations`                     | Create location preset            |
| GET    | `/api/admin/locations/{id}`                | Location preset detail            |
| PUT    | `/api/admin/locations/{id}`                | Update location preset            |
| PATCH  | `/api/admin/locations/{id}/status`         | Toggle location active state      |
| DELETE | `/api/admin/locations/{id}`                | Delete location preset            |
| GET    | `/api/admin/products`                      | Product list (paginated)          |
| GET    | `/api/admin/products/{id}`                 | Product detail                    |
| PATCH  | `/api/admin/products/{id}/active`          | Toggle product active state       |
| DELETE | `/api/admin/products/{id}`                 | Delete product                    |
| GET    | `/api/admin/services`                      | Service list (paginated)          |
| POST   | `/api/admin/services`                      | Create service                    |
| GET    | `/api/admin/services/{id}`                 | Service detail                    |
| PUT    | `/api/admin/services/{id}`                 | Update service                    |
| PATCH  | `/api/admin/services/{id}/active`          | Toggle service active state       |
| DELETE | `/api/admin/services/{id}`                 | Delete service                    |

### Provider Dashboard (`own.*` permissions; reads need only a session)

| Method | Path                          | Description                 |
| ------ | ----------------------------- | --------------------------- |
| GET    | `/api/provider/profile`       | View own business profile   |
| PUT    | `/api/provider/profile`       | Update own business profile |
| GET    | `/api/provider/products`      | List own products           |
| POST   | `/api/provider/products`      | Create product              |
| PUT    | `/api/provider/products/{id}` | Update product              |
| DELETE | `/api/provider/products/{id}` | Delete product              |
| PUT    | `/api/provider/products/{id}/image` | Replace product image (multipart field `image`; 20/min) |

### Setup (first-run / development only)

| Method | Path               | Description                      |
| ------ | ------------------ | -------------------------------- |
| GET    | `/api/setup/status` | Check whether first admin exists |
| POST   | `/api/setup/seed`  | Load demo data (flagged is_demo) |
| DELETE | `/api/setup/seed`  | Remove all demo data             |

---

## Security Model

- **JWT expiry:** 1 hour
- **Password reset token TTL:** 1 hour (single-use)
- **Account lockout:** 5 failed login attempts → 15-minute lockout
- **Password:** at least 8 characters, at most 72 bytes (bcrypt ignores the rest)
- **JWT secret:** at least 32 bytes; startup refuses a shorter one
- **Permissions:** granted to roles only (`role_permissions`); default grants applied once, so revocations stick
- **Username minimum:** 3 characters
- **Rate limiting:** per client address (behind `TRUSTED_PROXY_HOPS` proxies): auth 10/min, public writes 30/min, signed-in writes 120/min, uploads 20/min
- **Security headers:** `X-Content-Type-Options`, `X-Frame-Options`, `Content-Security-Policy`, `Strict-Transport-Security` (HTTPS only)
- **CORS:** configured via environment variable

---

## Frontend Route Map

All routes are defined as SvelteKit file-system pages under `client-app/src/routes/`.

### Auth (no layout group)

| Route              | Description                             |
| ------------------ | --------------------------------------- |
| `/login`           | Admin / provider login, first-run setup |
| `/register`        | Provider self-registration              |
| `/forgot-password` | Request password reset email            |
| `/reset-password`  | Consume reset token, set new password   |

### App `(app)/`

| Route                            | Description                                    |
| -------------------------------- | ---------------------------------------------- |
| `/`                              | Home: featured products, providers, categories (**served by Rust**, `web/src/pages/home.rs`) |
| `/products`                      | Product listing                                |
| `/products/[id]`                 | 301 → `/products/[id]/[slug]` (**Rust**)       |
| `/products/[id]/[slug]`          | Product detail (**Rust**, `web/src/pages/product.rs`; any other spelling 301s here) |
| `/products/category/[id]/[slug]` | Products filtered by category                  |
| `/services`                      | Service listing / search                       |
| `/services/category/[id]/[slug]` | Services filtered by category                  |
| `/agri-connect`                  | Provider map geo-discovery                     |
| `/providers/[id]`                | 301 → `/providers/[id]/[slug]` (**Rust**)      |
| `/providers/[id]/[slug]`         | Provider detail + contact form (**Rust**, `web/src/pages/provider.rs`) |
| `/account`                       | User profile (RequireAuth)                     |
| `/provider/dashboard`            | Provider product management (RequireProvider)  |

### Admin `(admin)/admin/`

| Route                      | Description                                     |
| -------------------------- | ----------------------------------------------- |
| `/admin`                   | Redirects to `/admin/dashboard`                 |
| `/admin/dashboard`         | Stats overview                                  |
| `/admin/users`             | User list                                       |
| `/admin/users/[id]`        | User detail, role & active management           |
| `/admin/roles`             | Role listing (read-only)                        |
| `/admin/roles/[id]`        | Role detail                                     |
| `/admin/categories`        | Category list + CRUD                            |
| `/admin/categories/[id]`   | Category detail & edit                          |
| `/admin/agri-connect`      | Provider list & verification                    |
| `/admin/agri-connect/[id]` | Provider detail                                 |
| `/admin/inquiries`         | Inquiry status management                       |
| `/admin/reviews`           | Review visibility management                    |
| `/admin/logout`            | POST action — clears cookie, redirects to login |

---

## Frontend URL Constants

All hardcoded URL strings are banned. Every `href`, `goto()`, and `redirect()` call must use the centralised constants in **`client-app/src/lib/routes.ts`**.

```ts
import { ROUTES } from '$lib/routes';

// Static routes
ROUTES.HOME              // '/'
ROUTES.LOGIN             // '/login'
ROUTES.LOGIN_RESET       // '/login?reset=1'
ROUTES.REGISTER          // '/register'
ROUTES.FORGOT_PASSWORD   // '/forgot-password'
ROUTES.RESET_PASSWORD    // '/reset-password'
ROUTES.ACCOUNT           // '/account'
ROUTES.ACCOUNT_SECURITY  // '/account#security'
ROUTES.AGRI_CONNECT      // '/agri-connect'
ROUTES.PRODUCTS          // '/products'
ROUTES.SERVICES          // '/services'
ROUTES.PROVIDER_DASHBOARD// '/provider/dashboard'

// Builder functions
ROUTES.PRODUCT(id, slug)          // '/products/:id/:slug'
ROUTES.PRODUCT_BY_ID(id)          // '/products/:id'
ROUTES.PRODUCT_CATEGORY(id, slug) // '/products/category/:id/:slug'
ROUTES.SERVICE_CATEGORY(id, slug) // '/services/category/:id/:slug'
ROUTES.PROVIDER(id, slug)         // '/providers/:id/:slug'
ROUTES.PROVIDER_BY_ID(id)         // '/providers/:id'

// Admin static
ROUTES.ADMIN.DASHBOARD    ROUTES.ADMIN.USERS    ROUTES.ADMIN.ROLES
ROUTES.ADMIN.CATEGORIES   ROUTES.ADMIN.AGRI_CONNECT
ROUTES.ADMIN.INQUIRIES    ROUTES.ADMIN.REVIEWS   ROUTES.ADMIN.LOGOUT

// Admin builders
ROUTES.ADMIN.USER(id)      ROUTES.ADMIN.ROLE(id)
ROUTES.ADMIN.CATEGORY(id)  ROUTES.ADMIN.PROVIDER(id)
```

The file works in both client and server contexts via the `$lib` alias. Never add a new navigation target without first adding it to `routes.ts`.

---

## Product Roadmap

### Phase 1 — MVP

- [ ] Provider directory with service tags (mud dredging, machine repair, fertilizer, drone spraying, etc.)
- [x] Geo-search: filter providers within configurable radius
- [x] Provider profile: real photos, capabilities, phone number
- [x] Click-to-call: direct connection, no middleman
- [x] Contact/inquiry form → stored in DB with status tracking
- [x] Admin panel: users, providers, categories, inquiries, reviews
- [x] Provider self-service dashboard: profile + product management
- [x] Auth: JWT login, registration, password reset, account lockout

### Phase 2 — Trust & Growth

- [ ] Review system: real feedback from local farmers
- [ ] Verification badges ("blue tick") for vetted providers
- [ ] Demand posts: farmers post a job, system pushes notifications to nearby providers

### Phase 3 — Ecosystem & Monetization

- [ ] Crop price feed: daily price tracking for rice and fruit
- [ ] Ads: promoted listings for equipment dealers and fertilizer brands
- [ ] Zalo Mini App: deep integration into Zalo ecosystem for fastest user acquisition

---

## Monetization Model

1. **Listing fee:** Monthly charge for verified/priority placement
2. **Lead gen:** Small fee when a provider receives a qualified contact request
3. **Display ads:** Banner placements for fertilizer, pesticide, and equipment brands

---

## Go-To-Market Strategy

1. **Field trip:** 1 week on-the-ground in target district to collect 50–100 initial provider listings manually
2. **Launch:** Minimal PWA + hyper-targeted Facebook Ads by district radius to drive first users
3. **Iterate:** Refine UI/UX based on farmer feedback (less text, more images, large buttons)
4. **Scale:** Expand to adjacent districts using oil-spill growth model

---

## KPIs

- New provider registrations per month
- Click-to-call conversion rate
- User retention rate (return visits for crop prices / weather)

---

## Development Guidelines

### General

- No hardcoding: all configuration (URLs, limits, secrets, feature flags) must come from environment variables or constants files. Never inline values that could vary between environments.
- No fake data, no stub implementations: every function must do real work. No `todo!()`, `unimplemented!()`, placeholder returns, or `// TODO: implement` stubs left in committed code.
- No code duplication: extract shared logic into helpers, utilities, or shared modules. If you write the same expression or pattern twice, factor it out.
- Functions that can be used interchangeably must be used consistently across the codebase — do not reimplement logic that already exists elsewhere.

### Backend (Rust)

- Layered architecture is strict: handlers call use cases; use cases call repository traits; infrastructure implements traits. Never skip a layer.
- Sea-ORM entities are in `infrastructure/src/entities/`; never define raw SQL structs that duplicate an entity.
- Business rules and validation belong in use cases (`application/src/validation.rs` for shared rules), not in handlers or request structs.
- Every failure is a `domain::Error` with an `ErrorCode`. Never put user-facing text in Rust: add the code to `domain/src/error.rs` and its Vietnamese text to `web/locales/vi.toml` (a test fails if either is missing). Never build an error response with an ad-hoc status code.
- Handlers extract with `ApiJson` / `ApiPath` / `ApiQuery` from `web::extract`, never axum's own, so malformed input gets a coded error.
- Environment variables are read only in `web/src/startup.rs`; everything else receives configuration as arguments.
- Migrations are DDL only and append-only: never edit or delete one that has run anywhere. Reference data (roles, permissions, categories, locations) lives in `application/src/catalogue.rs` and is seeded idempotently at startup; demo data only through `/api/setup/seed`.
- Keep internal logs in English.
- All location data uses **SRID 4326** (WGS84). Never mix geometry and geography types in the same query.
- Phone number pattern for Vietnamese mobile: `0\d{9}` (10 digits starting with 0).
- Radius validation bounds: 100 m minimum, 50 000 m maximum.

### Backend — DDD / Layer Integrity Rules

These rules were codified after a full architectural review. Violations are compile-time detectable — enforce them on every PR.

**Layer dependency direction** (dependencies may only point inward):
```
middleware / handlers  →  application (use cases, ports)  →  domain  ←  infrastructure
```

**Domain types**
- `UserRole` lives in `domain/models/role.rs`. No other layer redefines role semantics. Handlers and use cases both import `UserRole` from the domain — never from middleware.
- A domain model struct that is used as input to a use case must carry every field that the use case needs to enforce a business rule. Never rely on a silent SQL `WHERE` filter to enforce a domain invariant that belongs in the use case (e.g. `is_active` on `UserCredentials`).

**Application layer (`application/`)**
- Use cases depend only on domain models, repository traits, and the port traits in `application/ports.rs`. Never import from `middleware::`, `handlers::`, or concrete infrastructure types (e.g. `jsonwebtoken`, `bcrypt`).
- Infrastructure concerns that a use case needs (password hashing, token minting) must be abstracted behind a port trait in `application/ports.rs` and injected via `Arc<dyn PortTrait>` in the constructor — same pattern as repository traits.
- Shared validation rules live in `application/src/validation.rs` and return coded errors; forms collect them with `domain::Validation` so every bad field is reported at once. Never scatter the same rule as a private function in multiple use case files.
- Before any `user_repo.update_role()` call, check the code against an active row in `roles`, so arbitrary strings never reach the role column.

**Middleware layer (`middleware/`)**
- Middleware only finds out who is calling (`MaybeActor`, from the bearer token or the `auth_token` cookie); it never decides access. Handlers that need a session call `require_actor`, then pass `&actor` to the use case.
- Every state-changing use case, and every admin read, starts with `actor.require(PERM_...)?`. Permission keys live in `application/src/catalogue.rs`. Never compare role names: a convention test rejects it, and another fails when a use case taking an `Actor` checks nothing.
- Permissions are loaded from the database on every request, so a revocation or a deactivated account takes effect on the next request, not when the token expires.
- Non-GET requests with a foreign `Origin` are refused (`cross_origin_request`); `APP_URL` and `CORS_ORIGINS` are the allowed origins.
- Domain events (`domain/src/events.rs`) are published after a change is stored; the audit log is a subscriber. New side effects (notifications, webhooks) subscribe; use cases do not call them.

**Infrastructure layer (`infrastructure/`)**
- Concrete library types (`jsonwebtoken`, `bcrypt`, Sea-ORM) are confined to `infrastructure/`. Implement the port traits from `application/ports.rs` here.
- Uploads never reach storage as sent: `MediaUseCase` passes them through the `ImageProcessor` port (type sniffed from bytes, pixel cap checked before decoding, re-encoded as JPEG, EXIF dropped). Storage is the `FileStorage` port (`GCS_BUCKET`, else `UPLOAD_DIR`). Templates link images only with `file_url(id=…, variant="thumb")`. Old images are deleted by the file-cleanup event subscriber, never inline.
- `JwtTokenService` (in `infrastructure/jwt_token_service.rs`) is the only file that calls `jsonwebtoken::encode`. `BcryptPasswordHasher` (in `infrastructure/bcrypt_password_hasher.rs`) is the only file that calls `bcrypt::hash` / `bcrypt::verify`.

**Errors (`domain/src/error.rs`, `web/src/error_response.rs`)**
- `IntoResponse for Error` (behind domain's `axum` feature) only sets the status and parks the error; `render_errors` writes the localised body. Every router must sit under that layer.

### Rust pages (replacing SvelteKit route by route)

- A page is a handler in `web/src/pages/` that takes `Shell` (the frame: top bar, account menu, bottom tabs, built from the session) and renders through `views::render_page`. A stale session cookie renders the page signed out; it never fails it.
- Templates follow the same atomic levels as the Svelte components (`templates/atoms|molecules|organisms`, pages extend `layouts/app.html`). Links come from `url_for`, product and provider slugs from the `slugify` filter (identical to `slugify.js`; both are tested against `client-app/src/lib/slugify.fixtures.json`, run `npm test` in `client-app`), images from `file_url`.
- Interactive bits are Alpine components registered in `static/js/app.js`; markup must work without them (links and plain forms first).
- Page forms post urlencoded (`ApiForm`) and follow Post/Redirect/Get: success is a 303 with a `cookies::Flash`; a broken rule re-renders the page with the error's status, the typed values and one message per field (from `error_body`). The rules stay in the use case the JSON API uses.
- When a route moves to Rust, delete its SvelteKit `+page*` files in the same change.

### Frontend (SvelteKit)

- **Bootstrap first:** use Bootstrap 5 components, utilities, and layout system before writing any custom CSS. Only add custom styles for things Bootstrap cannot provide.
- **Atomic Design is mandatory:** place every new component at the correct atomic level (atoms, molecules, organisms). Do not add components directly to route files or skip levels.
- **All navigation must use `ROUTES`:** import from `$lib/routes` for every `href`, `goto()`, and `redirect()`. Never write a bare string URL path anywhere in the frontend codebase. Add new routes to `routes.ts` first.
- UI components must use **large tap targets** (min 44 px) and **minimal text** — primary users are farmers on low-end Android phones with poor connectivity.
- In Svelte templates, use `pattern={"0\\d{9}"}` for phone regex — raw curly braces in attribute strings are parsed as Svelte expressions.
- Server-side API calls go through `lib/server/api.ts`; never call the backend directly from `+page.svelte` client-side code without going through the server layer.
- Auth token is stored in an httpOnly cookie `auth_token`; never expose it to client-side JavaScript.

### SEO

Public-facing content pages must include a `<svelte:head>` block with `<title>` and `<meta name="description">`. Dynamic pages (provider detail, product detail, category listing) must derive these values from the loaded data — never use generic fallback strings.

| Route group                                                  | SEO required | Notes                                                       |
| ------------------------------------------------------------ | ------------ | ----------------------------------------------------------- |
| `/` (home)                                                   | Yes          | Static title + description                                  |
| `/products`, `/products/[id]/[slug]`                         | Yes          | Product name & description in meta                          |
| `/products/category/[id]/[slug]`                             | Yes          | Category name in title                                      |
| `/services/category/[id]/[slug]`                             | Yes          | Category name in title                                      |
| `/agri-connect`                                              | Yes          | Geo-discovery map page                                      |
| `/providers/[id]/[slug]`                                     | Yes          | Provider name & description in meta                         |
| `/agri-connect/providers/[id]/[slug]`                        | Yes          | Provider name & description in meta                         |
| `/account`, `/provider/dashboard`                            | No           | Authenticated; add `<meta name="robots" content="noindex">` |
| `/login`, `/register`, `/forgot-password`, `/reset-password` | No           | Auth flows; add `<meta name="robots" content="noindex">`    |
| `/admin/**`                                                  | No           | Admin panel; add `<meta name="robots" content="noindex">`   |

---

## Running Locally

### Backend

```powershell
cd backend
# Copy .env.example to .env; needs DATABASE_URL (PostgreSQL + PostGIS) and a 32-byte JWT_SECRET
cargo run --bin agri-connect

# Before every push: both must pass across the whole workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Database tests (each creates and drops its own database):
# TEST_DATABASE_URL=postgres://... cargo test --workspace -- --include-ignored
```

### Rust in front of SvelteKit (how production runs)

Set `LEGACY_FRONTEND_URL=http://localhost:5173` in `backend/.env`, start both, and browse `http://localhost:3000`: Rust serves its own pages and proxies the rest to SvelteKit, which calls the API back on :3000.

### Deployment

GitHub Actions (`.github/workflows/ci.yml`, `deploy.yml`) builds and deploys to one Compute Engine VM; one-time setup, backups and the restore procedure are in `deploy/README.md`.

### Frontend

```powershell
cd client-app
npm install
npm run dev
```

> **Windows note:** If `npm run dev` fails with a script execution policy error, run:
> `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`
