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
│   │   ├── infrastructure/     # Sea-ORM entities + repos, JWT, bcrypt, SMTP, storage, seeders
│   │   ├── web/                # Axum: pages, handlers, middleware, routing, extractors, startup
│   │   │   ├── src/pages/      # Server-rendered pages (one module per page or area)
│   │   │   ├── templates/      # Tera: atoms/ molecules/ organisms/ pages/ layouts/
│   │   │   ├── static/         # CSS, fonts, Bootstrap, Alpine, the committed islands.js
│   │   │   └── locales/vi.toml # User-facing text for every error code
│   │   └── tests/              # Integration + convention tests (no workspace lints)
│   ├── islands/                # Svelte custom elements → static/js/islands.js (Node only here)
│   └── migration/              # Sea-ORM migrations: DDL only, append-only
├── deploy/                     # compose, Caddy, VM deploy script, backups (see deploy/README.md)
└── docs/migration-plan.md      # How the SvelteKit app was replaced, slice by slice (done)
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

### Frontend (server-rendered by the Rust server)

- **Templates:** Tera, rendered by `web::views`; every page works without JavaScript
- **UI Framework:** Bootstrap 5.3 (vendored under `static/vendor`)
- **Interactivity:** Alpine.js (CSP build), components registered in `static/js/app.js`
- **Islands:** Svelte 5 custom elements for what Alpine cannot do (today: geolocation)
- **Icons:** FontAwesome 6 (fa-solid classes)
- **Design methodology:** Atomic Design (atoms → molecules → organisms → pages)
- **Target:** optimized for low-end Android on weak signal; pages work without JS

### Infrastructure

- **Cloud:** Google Compute Engine (one VM, docker compose), Artifact Registry, Cloud Storage for uploads; deployed by GitHub Actions
- **Integration:** Zalo OA & Mini App API (planned)

---

## Architecture

### Backend Layers

The backend follows a strict layered architecture. Dependencies flow inward only.

```
HTTP Request
    ↓
pages/ handlers/       — parse & validate HTTP input, call use case, render or return JSON
    ↓
application/usecases/  — orchestrate domain logic, call repository traits
    ↓
domain/repositories/   — trait definitions (interfaces)
    ↑
infrastructure/repositories/  — Sea-ORM implementations injected at startup
```

- **Pages and handlers** hold no business logic — only HTTP marshalling. Pages render HTML; handlers serve the JSON API. Both call the same use cases.
- **Use cases** hold all business logic — they never touch Sea-ORM or HTTP types directly.
- **Repository traits** define what persistence operations exist; the infrastructure layer implements them.
- **`AppState`** wires every use case at startup and is injected into every handler via Axum's `State` extractor.

### Frontend Architecture — Atomic Design

Templates live under `backend/crates/web/templates/` and are organized by atomic level (Tera macros):

| Level     | Folder       | Examples                                                                                       |
| --------- | ------------ | ---------------------------------------------------------------------------------------------- |
| Atoms     | `atoms/`     | `icon`, `alert`, `inquiry_status`                                                              |
| Molecules | `molecules/` | `form_field`, `page_hero`, `section_header`, `pagination_nav`, `empty_state`, `admin_header`   |
| Organisms | `organisms/` | `product_card`, `provider_card`, `category_grid`, `contact_sheet`, `confirm_sheet`, `*_fields` |
| Layouts   | `layouts/`   | `app.html` (public + provider), `admin.html` (admin panel)                                     |

**Rules:**

- Atoms have no dependencies on other templates.
- Molecules may compose atoms only.
- Organisms may compose atoms and molecules.
- Page-level composition happens in `templates/pages/`, not in organisms.
- Never add a new template at a lower level when an existing higher-level one already covers the need.

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

| Method | Path                               | Description                                                                                            |
| ------ | ---------------------------------- | ------------------------------------------------------------------------------------------------------ |
| GET    | `/health/live`, `/health/ready`    | Liveness; readiness (database reachable)                                                               |
| GET    | `/api/explore/providers`           | Geo-search (`lat`, `lng`, `radius?` default 10 km, `category_id?`) or browse (paginated, `page?`)      |
| GET    | `/api/explore/providers/{id}`      | Provider detail                                                                                        |
| GET    | `/api/explore/featured`            | Home featured data (providers, products, categories)                                                   |
| GET    | `/api/explore/categories`          | Category listing (filter by `type=service\|product`)                                                   |
| GET    | `/api/explore/locations`           | Location presets listing                                                                               |
| GET    | `/api/explore/products`            | Browse products (paginated, filter by `category_id?`)                                                  |
| GET    | `/api/explore/products/{id}`       | Product detail                                                                                         |
| POST   | `/api/explore/contact`             | Submit contact/inquiry (rate-limited: 30/min)                                                          |
| POST   | `/contact`                         | Same inquiry as a page form (urlencoded); 303 + one-time flash, or the page re-shown with field errors |
| GET    | `/files/{id}`, `/files/{id}/thumb` | Uploaded image (JPEG, cached forever; ids are never reused)                                            |

### Auth (rate-limited: 10 req/min)

| Method | Path                                | Description                                         |
| ------ | ----------------------------------- | --------------------------------------------------- |
| POST   | `/api/auth/tokens`                  | Login → JWT                                         |
| POST   | `/api/users`                        | Provider self-registration                          |
| POST   | `/api/auth/password-resets`         | Send password reset email                           |
| PATCH  | `/api/auth/password-resets/{token}` | Consume reset token, set new password               |
| POST   | `/api/admin/setup`                  | Create first admin account (rate-limited, one-time) |

### User (session required)

| Method | Path                     | Description          |
| ------ | ------------------------ | -------------------- |
| GET    | `/api/users/me`          | Current user profile |
| PATCH  | `/api/users/me/password` | Change password      |

### Admin (each route checks its own permission key, e.g. `user.read`, `provider.verify`)

| Method | Path                                     | Description                                                       |
| ------ | ---------------------------------------- | ----------------------------------------------------------------- |
| GET    | `/api/admin/stats`                       | Dashboard statistics                                              |
| GET    | `/api/admin/users`                       | User list (paginated)                                             |
| GET    | `/api/admin/users/{id}`                  | User detail                                                       |
| PATCH  | `/api/admin/users/{id}/role`             | Update user role                                                  |
| PATCH  | `/api/admin/users/{id}/status`           | Toggle user active state                                          |
| GET    | `/api/admin/users/{id}/permissions`      | Effective permissions (via role)                                  |
| GET    | `/api/admin/roles`                       | Role listing                                                      |
| GET    | `/api/admin/roles/{id}`                  | Role detail                                                       |
| GET    | `/api/admin/permissions`                 | All available permissions                                         |
| GET    | `/api/admin/categories`                  | Category listing                                                  |
| POST   | `/api/admin/categories`                  | Create category                                                   |
| GET    | `/api/admin/categories/{id}`             | Category detail                                                   |
| PUT    | `/api/admin/categories/{id}`             | Update category                                                   |
| PATCH  | `/api/admin/categories/{id}/status`      | Toggle category active state                                      |
| GET    | `/api/admin/providers`                   | Provider list (paginated)                                         |
| GET    | `/api/admin/providers/{id}`              | Provider detail                                                   |
| PATCH  | `/api/admin/providers/{id}/verification` | Set verification badge (`is_verified`, `reason?`)                 |
| POST   | `/api/admin/providers/{id}/users`        | Link a provider account (or create one); one business per account |
| GET    | `/api/admin/inquiries`                   | Inquiry list (paginated)                                          |
| PATCH  | `/api/admin/inquiries/{id}/status`       | Update inquiry status                                             |
| GET    | `/api/admin/reviews`                     | Review list (paginated)                                           |
| PATCH  | `/api/admin/reviews/{id}/visibility`     | Toggle review visibility                                          |
| GET    | `/api/admin/locations`                   | Location preset list                                              |
| POST   | `/api/admin/locations`                   | Create location preset                                            |
| GET    | `/api/admin/locations/{id}`              | Location preset detail                                            |
| PUT    | `/api/admin/locations/{id}`              | Update location preset                                            |
| PATCH  | `/api/admin/locations/{id}/status`       | Toggle location active state                                      |
| DELETE | `/api/admin/locations/{id}`              | Delete location preset                                            |
| GET    | `/api/admin/products`                    | Product list (paginated)                                          |
| GET    | `/api/admin/products/{id}`               | Product detail                                                    |
| PATCH  | `/api/admin/products/{id}/active`        | Toggle product active state                                       |
| DELETE | `/api/admin/products/{id}`               | Delete product                                                    |
| GET    | `/api/admin/services`                    | Service list (paginated)                                          |
| POST   | `/api/admin/services`                    | Create service                                                    |
| GET    | `/api/admin/services/{id}`               | Service detail                                                    |
| PUT    | `/api/admin/services/{id}`               | Update service                                                    |
| PATCH  | `/api/admin/services/{id}/active`        | Toggle service active state                                       |
| DELETE | `/api/admin/services/{id}`               | Delete service                                                    |

### Provider Dashboard (`own.*` permissions; reads need only a session, except the inquiry inbox: `own.inquiry.manage`)

| Method | Path                                | Description                                             |
| ------ | ----------------------------------- | ------------------------------------------------------- |
| GET    | `/api/provider/profile`             | View own business profile                               |
| PUT    | `/api/provider/profile`             | Update own business profile                             |
| GET    | `/api/provider/products`            | List own products                                       |
| POST   | `/api/provider/products`            | Create product                                          |
| PUT    | `/api/provider/products/{id}`       | Update product                                          |
| DELETE | `/api/provider/products/{id}`       | Delete product                                          |
| PUT    | `/api/provider/products/{id}/image` | Replace product image (multipart field `image`; 20/min) |

### Setup (first-run / development only)

| Method | Path                | Description                      |
| ------ | ------------------- | -------------------------------- |
| GET    | `/api/setup/status` | Check whether first admin exists |
| POST   | `/api/setup/seed`   | Load demo data (flagged is_demo) |
| DELETE | `/api/setup/seed`   | Remove all demo data             |

---

## Security Model

- **JWT expiry:** 1 hour
- **Browser sessions:** httpOnly `auth_token` (the JWT) + `refresh_token` cookies, `SameSite=Lax`, `Secure` behind TLS. Refresh tokens last `REFRESH_TOKEN_DAYS` (30), are stored as SHA-256 hashes, rotate on every use; a spent one reused after 30 s revokes its whole family. `session::renew` renews an expired access token before any handler runs. `POST /logout` (and `/admin/logout`) ends the family. A password change or reset ends every other session at once: it sets `users.sessions_valid_after`, and access tokens issued before it (`iat`) are refused.
- **Password reset token TTL:** 1 hour, single-use (claimed atomically), stored as a SHA-256 hash; a reset retires the account's other links
- **Account lockout:** 5 failed login attempts → 15-minute lockout
- **Password:** at least 8 characters, at most 72 bytes (bcrypt ignores the rest)
- **JWT secret:** at least 32 bytes; startup refuses a shorter one
- **Permissions:** granted to roles only (`role_permissions`); default grants applied once, so revocations stick
- **Username minimum:** 3 characters
- **Rate limiting:** per client address (behind `TRUSTED_PROXY_HOPS` proxies): auth 10/min, public writes 30/min, signed-in writes 120/min, uploads 20/min
- **Security headers:** `X-Content-Type-Options`, `X-Frame-Options`, `Content-Security-Policy`, `Strict-Transport-Security` (HTTPS only)
- **CORS:** configured via environment variable

---

## Page Route Map

Every page is a handler in `backend/crates/web/src/pages/`, routed in `web/src/routings/app_routing.rs`. Public and provider pages use `layouts/app.html`; admin pages use `layouts/admin.html`.

### Auth

| Route              | Description                                                                                  |
| ------------------ | -------------------------------------------------------------------------------------------- |
| `/login`           | Login (`web/src/pages/auth.rs`); signed in → own area by permission; no admin yet → `/setup` |
| `/register`        | Provider self-registration                                                                   |
| `/forgot-password` | Request password reset email                                                                 |
| `/reset-password`  | Consume reset token, set new password                                                        |
| `/setup`           | First admin + optional demo data (`web/src/pages/setup.rs`); closed once an admin exists     |
| `/setup/seed`      | Demo data page: load / remove (`system.demo.manage`)                                         |
| `/logout`          | POST: ends the session family, clears both cookies (`web/src/session.rs`)                    |

### App

| Route                                                    | Description                                                                                                                              |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `/`                                                      | Home: featured products, providers, categories (`web/src/pages/home.rs`)                                                                 |
| `/products`                                              | Product listing: GET filter form, page links (`web/src/pages/products.rs`)                                                               |
| `/products/[id]`                                         | 301 → `/products/[id]/[slug]`                                                                                                            |
| `/products/[id]/[slug]`                                  | Product detail (`web/src/pages/product.rs`; any other spelling 301s here)                                                                |
| `/products/category/[id]/[slug]`                         | Products in a category (slug is the category slug, others 301)                                                                           |
| `/services`                                              | Provider directory: GET filter form, page links (`web/src/pages/services.rs`)                                                            |
| `/services/category/[id]/[slug]`                         | Providers of one service (category slug canonical)                                                                                       |
| `/search`                                                | Providers + products for `q`; link tabs (`tab=`) work without JS (`web/src/pages/search.rs`)                                             |
| `/explore`                                               | Providers near a GPS position (`ac-geolocate` island) or a location preset (GET form); radius/service links (`web/src/pages/explore.rs`) |
| `/providers/[id]`                                        | 301 → `/providers/[id]/[slug]`                                                                                                           |
| `/providers/[id]/[slug]`                                 | Provider detail + contact form (`web/src/pages/provider.rs`)                                                                             |
| `/account`                                               | Profile, password change (signs out other devices), logout (`web/src/pages/account.rs`)                                                  |
| `/provider/dashboard`                                    | Products (photo thumbs) + business profile with GPS fill (`ac-geolocate` fill mode) (`web/src/pages/dashboard.rs`)                       |
| `/provider/products/new`, `/provider/products/[id]/edit` | Multipart product form with photo (checked before anything is saved), delete (confirm)                                                   |
| `/provider/inquiries`                                    | Farmers' contact requests to this business: call button, status per card (`own.inquiry.manage`) (`web/src/pages/inbox.rs`)              |

### Admin

| Route                    | Description                                                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `/admin`                 | 303 → `/admin/dashboard` (anonymous → `/login`)                                                                                |
| `/admin/dashboard`       | Stats + demo-data warning (`web/src/pages/admin/`; frame `layouts/admin.html`, menu filtered by permission)                    |
| `/admin/users`           | User list, paged (`web/src/pages/admin/users.rs`)                                                                              |
| `/admin/users/[id]`      | Role change, deactivate (confirm + reason) / reactivate, effective permissions                                                 |
| `/admin/roles`           | Roles with grant counts                                                                                                        |
| `/admin/roles/[id]`      | Editable permission grid, `POST …/permissions` (`role.permission.manage`; cannot revoke it from your own role; audited)        |
| `/admin/permissions`     | Permissions × roles, read-only                                                                                                 |
| `/admin/categories`      | Category list + create form (`web/src/pages/admin/categories.rs`)                                                              |
| `/admin/categories/[id]` | Edit; hide (confirm + reason) / show                                                                                           |
| `/admin/locations`       | Location presets + create form (`web/src/pages/admin/locations.rs`)                                                            |
| `/admin/locations/[id]`  | Edit, hide / show, delete (confirm + reason) → list                                                                            |
| `/admin/explore`         | Provider list, paged (`web/src/pages/admin/providers.rs`)                                                                      |
| `/admin/explore/[id]`    | Details, verify / unverify (confirm + reason), assign the managing account                                                     |
| `/admin/products`        | Products (filter `provider_id`): hide (confirm + reason) / show, delete (confirm + reason) (`web/src/pages/admin/products.rs`) |
| `/admin/services`        | Services (filter `provider_id`; create form only for one provider)                                                             |
| `/admin/services/[id]`   | Edit, hide / show, delete (confirm + reason)                                                                                   |
| `/admin/inquiries`       | Status per row (select + button; `autoSubmit` with JS)                                                                         |
| `/admin/reviews`         | Hide (confirm + reason) / show                                                                                                 |
| `/admin/logout`          | POST: same as `/logout`, lands on `/login`                                                                                     |

---

## URL Names

Hardcoded URL paths are banned in templates and pages. Every route has a name in **`backend/crates/web/src/routes.rs`** (`ROUTES`); templates link with `url_for(name="…", …)` and Rust code with `routes::url(name, params)`, which percent-encodes every parameter so a value cannot escape its segment. An unknown name or a missing parameter is an error when the page renders, never a broken link.

```
url_for(name="product", id=p.id, slug=p.name | slugify)   → /products/{id}/{slug}
url_for(name="admin_user", id=u.id)                        → /admin/users/{id}
routes::url("provider_product_edit", &[("id", "7")])       → /provider/products/7/edit
```

Canonical product, provider and category URLs end in a slug from the `slugify` filter (`web/src/slug.rs`, pinned by `slugify.fixtures.json`); any other spelling 301s to the canonical one. Never add a link target without adding its name to `routes.rs` first.

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
- Public provider lists (directory, near-me, search) include providers with no service yet, so a new sign-up can be found; a service filter (`category_id`) matches only a live service in a live category. The home page's featured providers need at least one live service.
- Every list sort ends with an `id` tiebreaker, and price sorts put unpriced products last, so pages never repeat or skip rows.

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
- A write that can hit a UNIQUE constraint maps the failure with `repositories::unique_or_internal` to a field error (e.g. `slug_taken`), never a 500.
- `JwtTokenService` (in `infrastructure/jwt_token_service.rs`) is the only file that calls `jsonwebtoken::encode`. `BcryptPasswordHasher` (in `infrastructure/bcrypt_password_hasher.rs`) is the only file that calls `bcrypt::hash` / `bcrypt::verify`.

**Errors (`domain/src/error.rs`, `web/src/error_response.rs`)**

- `IntoResponse for Error` (behind domain's `axum` feature) only sets the status and parks the error; `render_errors` writes the localised body. Every router must sit under that layer.

### Pages

- A page is a handler in `web/src/pages/` that takes `Shell` (the frame: top bar, account menu, bottom tabs, built from the session) and renders through `views::render_page`. Admin pages render through `pages::admin::render_admin` (the admin frame, menu filtered by permission). A stale session cookie renders the page signed out; it never fails it.
- Templates follow the atomic levels above. Links come from `url_for`, product and provider slugs from the `slugify` filter, images from `file_url(id=…, variant="thumb")`, field limits (`maxlength`, `minlength`) from the global `limits`, never typed numbers.
- **Bootstrap first:** Bootstrap 5 components and utilities before custom CSS; custom styles go in `static/css/app.css` only for what Bootstrap cannot do. No `style=""` attributes and no `<style>` elements: the page CSP is `style-src 'self'`.
- Interactive bits are Alpine components (CSP build) registered in `static/js/app.js`; attribute values name a component property, never an expression (convention test). Markup must work without them: links and plain forms first.
- What Alpine cannot do (today only geolocation) is an island: a Svelte custom element in `backend/islands/src` (light DOM, no `<style>`, all text passed as attributes), built with `npm run build` into `static/js/islands.js`, which is committed. CI rebuilds it and fails on a diff. Pages that use one load it in `{% block scripts %}`. Node is needed only for this.
- **Large tap targets** (min 44 px) and **minimal text**: the primary users are farmers on low-end Android phones with poor connectivity.
- Custom SQL in `Expr::cust_with_values` uses `$1`, `$2`: a `?` is left unbound and the query fails at runtime (convention test).
- Page forms post urlencoded (`ApiForm`) and follow Post/Redirect/Get: success is a 303 with a `cookies::Flash`; a broken rule re-renders the page with the error's status, the typed values and one message per field (from `error_body`). The rules stay in the use case the JSON API uses. A form with a file box posts `multipart/form-data`, read with `ApiMultipart::into_form`; a photo is checked with `MediaUseCase::prepare_image` before anything else is saved, and ownership is checked before the body is read.
- An action that needs a second look (deactivate, hide, delete) uses `organisms/confirm_sheet.html` inside an `x-data="dialog"` element; without JS it is an anchored form on the page. On a list, the row's link reloads the list with `?hide={id}` (or `?delete=`) and the server renders that confirmation open. Row forms send the list position back as `page` / `provider_id` (`pages::admin::ListPosition`), rebuilt from numbers, never a posted URL.
- The session lives in httpOnly cookies (`auth_token`, `refresh_token`); no script ever reads a token.

### SEO

Public content pages fill `{% block title %}` and `{% block meta %}` (a `<meta name="description">`) from the loaded data; never a generic fallback.

| Route group                                                  | SEO required | Notes                                                   |
| ------------------------------------------------------------ | ------------ | ------------------------------------------------------- |
| `/` (home)                                                   | Yes          | Static title + description                              |
| `/products`, `/products/[id]/[slug]`                         | Yes          | Product name & description in meta                      |
| `/products/category/[id]/[slug]`                             | Yes          | Category name in title                                  |
| `/services/category/[id]/[slug]`                             | Yes          | Category name in title                                  |
| `/explore`                                                   | Yes          | Near-me page                                            |
| `/providers/[id]/[slug]`                                     | Yes          | Provider name & description in meta                     |
| `/account`, `/provider/**`                                   | No           | Authenticated; `<meta name="robots" content="noindex">` |
| `/login`, `/register`, `/forgot-password`, `/reset-password` | No           | Auth flows; `noindex`                                   |
| `/admin/**`                                                  | No           | Admin panel; `noindex`                                  |

---

## Running Locally

```powershell
cd backend
# Copy .env.example to .env; needs DATABASE_URL (PostgreSQL + PostGIS) and a 32-byte JWT_SECRET
cargo run --bin agri-connect
# Browse http://localhost:5173: the Rust server serves every page and the API.

# Before every push: both must pass across the whole workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Database tests (each creates and drops its own database):
# TEST_DATABASE_URL=postgres://... cargo test --workspace -- --include-ignored

# Only after changing backend/islands/src: rebuild and commit the bundle
cd islands; npm ci; npm run build
```

> **Windows note:** If `npm` fails with a script execution policy error, run:
> `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`

### Deployment

GitHub Actions (`.github/workflows/ci.yml`, `deploy.yml`) builds one image and deploys it to one Compute Engine VM; one-time setup, backups and the restore procedure are in `deploy/README.md`.
