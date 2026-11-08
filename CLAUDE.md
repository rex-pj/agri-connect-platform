# Agri-Connect Platform

## Project Vision

An aggregator platform for the Southern Vietnam agricultural ecosystem, solving information fragmentation between farmers and service/supply providers. Core values: **Local · Transparent · Timely**.

Target region: Mekong Delta (priority: durian farms around Cai Lậy, Đồng Tháp; rice fields in An Giang). Places use the units in force since 1 July 2025: 34 provinces, wards and communes, no districts (Tiền Giang is now part of Đồng Tháp, Kiên Giang of An Giang, Long An of Tây Ninh).

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
│   │   │   └── locales/       # vi.toml, en.toml: every user-facing word ([errors] by code, [text] by key)
│   │   └── tests/              # Integration + convention tests (no workspace lints)
│   ├── islands/                # Svelte custom elements → static/js/islands.js
│   └── migration/              # Sea-ORM migrations: DDL only, append-only
├── deploy/                     # compose, Caddy, VM deploy script, backups (see deploy/README.md)
└── zalo-mini-app/              # Zalo Mini App (React, zmp-ui, zmp-sdk): a JSON API client (see its README)
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
- **Integration:** Zalo Mini App (`zalo-mini-app/`: near me, prices, job post over the JSON API; position and phone tokens read with `ZALO_APP_SECRET`); Zalo OA (planned)

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
| Atoms     | `atoms/`     | `icon`, `logo`, `alert`, `inquiry_status`, `photo_placeholder`                                 |
| Molecules | `molecules/` | `form_field`, `page_hero`, `section_header`, `pagination_nav`, `empty_state`, `admin_header`   |
| Organisms | `organisms/` | `product_card`, `provider_card`, `category_grid`, `contact_sheet`, `confirm_sheet`, `*_fields` |
| Layouts   | `layouts/`   | `app.html` (public + provider), `admin.html` (admin panel), `partner.html` (partner report)   |

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
| `roles`                 | id, code, name, is_active                                                                              | Codes: `admin`, `provider`, `partner`          |
| `users`                 | id, username, email, password_hash, full_name, role_id, is_active, failed_login_attempts, locked_until | Soft-deletable                                 |
| `categories`            | id, name, slug, type, icon, display_order, is_active                                                   | Types: `service`, `product`                    |
| `businesses`            | id, name, phone, email, description, address, lat, lng, is_verified, is_active                         | Provider/business listing                      |
| `services`              | id, business_id, category_id, name, description, price_note, is_active, image_file_id                  | Offered by a business; `is_active` is admin's  |
| `products`              | id, business_id, category_id, name, description, price, unit, is_active                                | Products sold by a business                    |
| `inquiries`             | id, business_id, service_id, topic, requester_name, requester_phone, note, status                      | Statuses: `new`, `contacted`, `closed`, `spam` |
| `reviews`               | id, business_id, inquiry_id, reviewer_name, reviewer_phone_hash, rating (1–5), comment, reply, replied_at, is_visible | Hidden until an admin shows it |
| `call_clicks`           | id, business_id, service_id, product_id, channel, created_on                                           | Call / Zalo taps (`call`, `zalo`); never who   |
| `password_reset_tokens` | id, user_id, token, expires_at, consumed_at                                                            | Single-use, 1-hour TTL                         |
| `search_events`         | id, query, category_id, ward_code, results, created_on                                                 | What was searched and in which ward; never who |
| `partners`              | id, name, kind, contact_name, phone, email, account_id, is_active                                      | Kinds: `brand`, `dealer`, `cooperative`, `exporter` |
| `partner_contracts`     | id, partner_id, province_code, ward_code?, starts_on, ends_on, monthly_fee (+ `partner_contract_categories`, `partner_sponsored`) | A whole province or one ward of it; in force between its dates while the partner is on |
| `provinces`             | code (official, 2 digits), name, full_name, unit_type, center, is_active                               | The 34 since 1 July 2025; loaded from the bundled dataset, never deleted |
| `wards`                 | code (official, 5 digits), province_code, name, full_name, unit_type, center, boundary, is_active      | 3 321 wards, communes, special zones; `boundary` a simplified `GEOGRAPHY(MULTIPOLYGON)` |
| `crops`                 | id, name (UNIQUE), kind, unit, display_order, is_active                                                | Kinds: `rice`, `fruit`; seeded when empty, hidden never deleted |
| `crop_prices`           | id, crop_id, location_id, observed_on, price_low, price_high, note                                     | One per crop × area × day (again replaces it)  |
| `job_posts`             | id, category_id, location_id, location, description, requester_name, requester_phone, status         | Statuses: `open`, `closed`, `spam`; never public |

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
| GET    | `/api/explore/services`            | Same two modes as providers, over public services (`name?` = text, `sort?`); each with its business    |
| GET    | `/api/explore/services/{id}`       | Public service detail                                                                                  |
| GET    | `/api/explore/featured`            | Home featured data: services (one per verified provider), providers, products                          |
| GET    | `/api/explore/categories`          | Category listing (filter by `type=service\|product`)                                                   |
| GET    | `/api/explore/locations`           | Location presets listing                                                                               |
| POST   | `/api/explore/jobs`                | A job post (`category_id`, `lat`+`lng` or `location_id`, description, name, phone; 201 `{id}`; 30/min) |
| POST   | `/api/zalo/location`, `/api/zalo/phone` | The Zalo Mini App's `{access_token, token}` → `{lat, lng}` / `{phone}` through the Zalo Graph API (`ZALO_APP_SECRET`; unset: 503 `zalo_not_configured`; 30/min) |
| GET    | `/api/explore/prices`              | Crop prices: each shown crop's latest price per area within `PRICE_FRESH_DAYS`, its move (`location_id?`)  |
| GET    | `/api/explore/products`            | Browse products (paginated, filter by `category_id?`)                                                  |
| GET    | `/api/explore/products/{id}`       | Product detail                                                                                         |
| POST   | `/api/explore/contact`             | Submit contact/inquiry (rate-limited: 30/min)                                                          |
| GET    | `/api/explore/providers/{id}/reviews` | Shown reviews: `count`, `average`, the newest `PUBLIC_REVIEWS_SHOWN` with the provider's replies   |
| POST   | `/api/explore/reviews`             | A review (`business_id`, name, phone, `rating` 1–5, comment; 201, hidden until approved; 30/min)       |
| POST   | `/reviews`                         | Same review as a page form (urlencoded); 303 + flash, or the page re-shown with the sheet open          |
| POST   | `/api/explore/calls`               | A tap on a public call or Zalo button (`{kind, id, channel?}`: `call` default, `zalo`; 204; 30/min); the business is looked up from the record |
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
| GET    | `/api/admin/reports/demand`              | Demand report (`area?`: `p82`, `w28439`, `none`; each area's `value`, `level`, `label`; hidden figures are `value: null`; `report.demand.read`) |
| GET    | `/api/admin/partners`                    | Partners, with contracts in force and monthly revenue (`partner.manage`) |
| POST   | `/api/admin/partners`                    | Create a partner (201 + detail)                                   |
| GET    | `/api/admin/partners/kpis`               | Paying partners, monthly revenue, by area                         |
| GET    | `/api/admin/partners/{id}`               | Partner detail with contracts                                     |
| PUT    | `/api/admin/partners/{id}`               | Update a partner                                                  |
| PATCH  | `/api/admin/partners/{id}/status`        | Turn on / off (`is_active`, `reason?`; off: its contracts stop)   |
| POST   | `/api/admin/partners/{id}/account`       | Link or create its report account (role `partner`)                |
| POST   | `/api/admin/partners/{id}/contracts`     | Add a contract (`area`: `p82` / `w28439`, `category_ids`, `starts_on`, `ends_on`, `monthly_fee`, `note?`) |
| PUT / DELETE | `/api/admin/partners/{id}/contracts/{cid}` | Edit a contract (dates, fee, note, categories; not its area) / delete it (`reason?`) |
| POST / DELETE | `/api/admin/partners/{id}/contracts/{cid}/sponsored[/{bid}]` | Sponsor a shop of the area of the contract / stop (`reason?`, audited) |
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
| GET    | `/api/admin/categories/slug`             | Free slug for `text` (name or slug; `id?` = the one edited): `{slug, taken}` |
| PATCH  | `/api/admin/categories/{id}/status`      | Toggle category active state                                      |
| GET    | `/api/admin/providers`                   | Provider list (paginated)                                         |
| POST   | `/api/admin/providers`                   | Create a provider with no account (profile boxes; 201 + detail; `provider.manage`) |
| GET    | `/api/admin/providers/{id}`              | Provider detail                                                   |
| PUT    | `/api/admin/providers/{id}`              | Replace its profile (the owner's rules; detail; `provider.manage`) |
| PATCH  | `/api/admin/providers/{id}/verification` | Set verification badge (`is_verified`, `reason?`)                 |
| PATCH  | `/api/admin/providers/{id}/status`       | Turn a provider on / off (`is_active`, `reason?`; off: gone from public pages) |
| POST   | `/api/admin/providers/{id}/users`        | Link a provider account (or create one); one business per account |
| GET    | `/api/admin/inquiries`                   | Inquiry list (paginated, `provider_id?`)                          |
| PATCH  | `/api/admin/inquiries/{id}/status`       | Update inquiry status                                             |
| GET    | `/api/admin/reviews`                     | Review list (paginated, `provider_id?`)                           |
| PATCH  | `/api/admin/reviews/{id}/visibility`     | Toggle review visibility                                          |
| GET    | `/api/admin/jobs`                        | Job posts, newest first (`page?`; `job.moderate`)                 |
| PATCH  | `/api/admin/jobs/{id}/status`            | `open` / `closed` / `spam` (`reason?`)                            |
| GET / POST | `/api/admin/crops`                   | Crops (hidden too) / add one (201) (`price.manage`)               |
| GET / PUT | `/api/admin/crops/{id}`               | Crop detail / edit                                                |
| PATCH  | `/api/admin/crops/{id}/status`           | Show / hide a crop (`is_active`, `reason?`)                       |
| GET / POST | `/api/admin/prices`                  | Prices entered, newest day first (`page?`) / enter one (`crop_id`, `location_id`, `observed_on`, `price_low`, `price_high?`, `note?`; 201 `{id}`) |
| DELETE | `/api/admin/prices/{id}`                 | Delete a price (`reason?`)                                        |
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
| GET    | `/api/provider/services`            | List own services (admin-hidden ones too)               |
| POST   | `/api/provider/services`            | Create service (blank name → category name; max 20)     |
| PUT    | `/api/provider/services/{id}`       | Update service (never changes admin visibility)         |
| DELETE | `/api/provider/services/{id}`       | Delete service                                          |
| PUT    | `/api/provider/services/{id}/image` | Replace service image (multipart field `image`; 20/min) |
| GET    | `/api/provider/reviews`             | Reviews of the own business (waiting ones too)          |
| GET    | `/api/provider/jobs`                | Open job posts near the own business in its services' categories (`own.job.read`) |
| PUT    | `/api/provider/reviews/{id}/reply`  | Public answer (`{reply}`; blank removes it; 204; `own.review.reply`) |

### Partner (`partner.report.read`)

| Method | Path                  | Description                                                              |
| ------ | --------------------- | ------------------------------------------------------------------------ |
| GET    | `/api/partner/report` | The partner account's own demand report (`area?`), its contracts in force |

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

Every page is a handler in `backend/crates/web/src/pages/`, routed in `web/src/routings/app_routing.rs`. Public and provider pages use `layouts/app.html`; admin pages use `layouts/admin.html`; the partner's report uses `layouts/partner.html` (brand, language, logout: no farmer tabs, search or footer).

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
| `/language`        | POST only: the language choice (`lang` = vi / en, `back` = a local path, else home); `lang` cookie for a year, 303 back, so no language URL ever shows (`web/src/pages/language.rs`) |

### App

| Route                                                    | Description                                                                                                                              |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `/`                                                      | Home: service then product category tiles, near-me banner, featured services, featured products (`web/src/pages/home.rs`)                |
| `/products`                                              | Product listing: GET filter form, page links (`web/src/pages/products.rs`)                                                               |
| `/products/[id]`                                         | 301 → `/products/[id]/[slug]`                                                                                                            |
| `/products/[id]/[slug]`                                  | Product detail + the seller's services + contact form about it (`return_to=product`) (`product.rs`; other spellings 301)                                   |
| `/products/category/[id]/[slug]`                         | Products in a category (slug is the category slug, others 301)                                                                           |
| `/services`                                              | Service listing: one card per public service (photo, price, provider, call); GET filter form, page links (`web/src/pages/services.rs`)   |
| `/services/category/[id]/[slug]`                         | Services in one category (category slug canonical)                                                                                       |
| `/services/[id]`                                         | 301 → `/services/[id]/[slug]`                                                                                                            |
| `/services/[id]/[slug]`                                  | Service detail + the provider's other services + contact form with it chosen (`return_to=service`) (`web/src/pages/service.rs`)          |
| `/search`                                                | Services + products + shops for `q`; link tabs (`tab=`) work without JS (`web/src/pages/search.rs`)                                      |
| `/explore`                                               | Services, products and shops near a position (`ac-geolocate`) or preset, tabs in that order (as `/search`); featured services before one (`explore.rs`)     |
| `/jobs/new`                                              | Post a job (`POST /jobs`): service, where (`ac-geolocate` fills hidden boxes, else an area), what, name, phone; the home page's "Chưa tìm được thợ?" card leads here (`web/src/pages/jobs.rs`) |
| `/prices`                                                | Crop prices, rice then fruit, latest per area with its move; GET area filter (`autoSubmit`) (`web/src/pages/prices.rs`); home shows `HOME_PRICES_SHOWN` |
| `/providers`                                             | Provider directory (new sign-ups without a service too); found by name or by their services (`web/src/pages/directory.rs`)               |
| `/providers/[id]`                                        | 301 → `/providers/[id]/[slug]`                                                                                                           |
| `/providers/[id]/[slug]`                                 | Provider detail: services, products, shown reviews with replies + "write a review" sheet, contact form (`web/src/pages/provider.rs`)                       |
| `/account`                                               | Profile, password change (signs out other devices), logout (`web/src/pages/account.rs`)                                                  |
| `/provider/dashboard`                                    | Services + products (photo thumbs) + business profile with GPS fill (`ac-geolocate` fill mode) (`web/src/pages/dashboard.rs`)            |
| `/provider/products/new`, `/provider/products/[id]/edit` | Multipart product form with photo (checked before anything is saved), delete (confirm)                                                   |
| `/provider/services/new`, `/provider/services/[id]/edit` | Multipart service form: category cards, optional photo, delete (confirm) (`own.service.*`) (`web/src/pages/own_services.rs`)             |
| `/provider/inquiries`                                    | Farmers' contact requests to this business: call button, status per card (`own.inquiry.manage`) (`web/src/pages/inbox.rs`)              |
| `/provider/jobs`                                         | Job posts near the business in its services' categories, distance and call button (`own.job.read`) (`web/src/pages/own_jobs.rs`) |
| `/provider/reviews`                                      | Reviews of this business (shown or waiting), a public answer per card (`own.review.reply`) (`web/src/pages/own_reviews.rs`)              |
| `/partner`                                               | A partner account's home: the demand report of its contracts in force (areas × categories), its contracts; `/partner/report.csv` (`partner.report.read`) (`web/src/pages/partner.rs`) |

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
| `/admin/categories`      | Category list (row: hide (confirm + reason) / show, open) + create sheet (`web/src/pages/admin/categories.rs`) |
| `/admin/categories/[id]` | Edit; hide (confirm + reason) / show                                                                                           |
| `/admin/locations`       | Location presets (row: hide (confirm + reason) / show, delete (confirm), open) + create sheet (`web/src/pages/admin/locations.rs`) |
| `/admin/locations/[id]`  | Edit, hide (confirm + reason) / show, delete (confirm + reason) → list                                                                         |
| `/admin/explore`         | Provider list, paged + create sheet (`provider.manage`: a listing from name + phone, no account; GPS fill) (`web/src/pages/admin/providers.rs`) |
| `/admin/explore/[id]`    | Details, profile form (the owner's rules: `business_rules::validate_profile`; `organisms/business_fields.html`), turn off (confirm + reason) / on, verify / unverify (confirm + reason), assign the managing account; links to its services, products, inquiries, reviews |
| `/admin/products`        | Products (filter `provider_id`): hide (confirm + reason) / show, delete (confirm + reason) (`web/src/pages/admin/products.rs`) |
| `/admin/services`        | Services (filter `provider_id`; row: hide / show, delete, open; create sheet only for one provider) |
| `/admin/services/[id]`   | Edit, hide / show, delete (confirm + reason)                                                                                   |
| `/admin/inquiries`       | Status per row (select + button; `autoSubmit` with JS); filter `provider_id`                                                                       |
| `/admin/reviews`         | Hide (confirm + reason) / show; filter `provider_id`                                                                                               |
| `/admin/demand`          | Demand report: category × week for one area (`?area=` `p82` a province, `w28439` a ward, or `none`) or all, by channel, searches, unmet searches, steady areas; `/admin/demand.csv` (`report.demand.read`) (`web/src/pages/admin/demand.rs`) |
| `/admin/partners`        | Partners, paying ones and monthly revenue by area + create sheet (`partner.manage`) (`web/src/pages/admin/partners.rs`)      |
| `/admin/jobs`            | Job posts, state per row (select + button; `autoSubmit`) (`job.moderate`) (`web/src/pages/admin/jobs.rs`) |
| `/admin/prices`          | Prices entered, paged (row: delete, confirm + reason) + "enter a price" sheet (`price.manage`) (`web/src/pages/admin/prices.rs`) |
| `/admin/crops`, `/admin/crops/[id]` | Crops (row: hide (confirm + reason) / show, open) + create sheet; edit, hide / show (`web/src/pages/admin/crops.rs`) |
| `/admin/partners/[id]`   | Edit, turn off (confirm + reason) / on, report account (sheet), contracts (edit in place: dates, fee, categories, the area stays; delete confirm + reason) with the sponsored shops of their area (remove: confirm + reason), the add-contract form at the end (on the page, not a sheet: long form) |
| `/admin/logout`          | POST: same as `/logout`, lands on `/login`                                                                                     |

---

## URL Names

Hardcoded URL paths are banned in templates and pages. Every route has a name in **`backend/crates/web/src/routes.rs`** (`ROUTES`); templates link with `url_for(name="…", …)` and Rust code with `routes::url(name, params)`, which percent-encodes every parameter so a value cannot escape its segment. An unknown name or a missing parameter is an error when the page renders, never a broken link.

```
url_for(name="product", id=p.id, slug=p.name | slugify)   → /products/{id}/{slug}
url_for(name="admin_user", id=u.id)                        → /admin/users/{id}
routes::url("provider_product_edit", &[("id", "7")])       → /provider/products/7/edit
```

Canonical product, provider and category URLs end in a slug from the `slugify` filter (`application/src/slug.rs`, re-exported as `web::slug`, pinned by `slugify.fixtures.json`); any other spelling 301s to the canonical one. A category stores its slug (UNIQUE): left blank it is made from the name, numbered while taken (`muc-bun-2`, …), and a typed one another category has is refused (`slug_taken`) before the write; the form fills and checks it as the name is typed (Alpine `slugField`, `GET /api/admin/categories/slug`). Never add a link target without adding its name to `routes.rs` first.

---

## Product Roadmap

### Phase 1 — MVP

- [ ] Provider directory with service tags (mud dredging, machine repair, fertilizer, drone spraying, etc.)
- [x] Geo-search: filter providers within configurable radius
- [x] Provider profile: real photos, capabilities, phone number
- [x] Click-to-call: direct connection, no middleman
- [x] Zalo chat button on detail pages, counted as a lead like a call
- [x] Contact/inquiry form → stored in DB with status tracking
- [x] Admin panel: users, providers, categories, inquiries, reviews
- [x] Field onboarding: an admin lists a provider from its phone number (no account needed), edits its profile, assigns an account later
- [x] Weekly leads (calls, Zalo, inquiries) on the admin dashboard: the go / no-go figure for an area
- [x] Provider self-service dashboard: profile + product management
- [x] Auth: JWT login, registration, password reset, account lockout

### Phase 2 — Trust & Growth

Grow the rest only once one area sustains real leads week after week (the admin dashboard's weekly table). An area is a location preset ("Đồng Tháp — Cai Lậy") and the radius around it, not an administrative unit: there are no districts since July 2025.

- [x] Review system: only from a phone whose contact request the provider answered (`contacted` / `closed`: `INQUIRY_STATUSES_REVIEWABLE`, linked by `reviews.inquiry_id`), one per phone and provider (`reviewer_phone_hash`, SHA-256), hidden until an admin shows it, with the provider's public reply (`ReviewUseCase`)
- [ ] Verification badges ("blue tick") for vetted providers
- [x] Demand posts: farmers post a job, the providers nearby that offer it are emailed and see it on their jobs page

### Phase 3 — Ecosystem & Monetization (B2B)

- [x] Demand report: category × province / ward × week (calls, Zalo, inquiries, searches, unmet searches), aggregates only, small figures hidden; the admin's page, CSV and JSON, and each partner's view of its own areas and categories
- [x] Partners: brand, dealer, cooperative or exporter; contracts by area × categories × dates × monthly fee; a `partner` account that reads only its report; paying partners and revenue by area
- [x] Administrative units: 34 provinces and 3 321 wards with official codes, points and boundaries; the demand report and partner contracts by province and ward
- [x] Sponsored placement: a contract's providers of its area shown first on that area's near-me page, labelled "Tài trợ", moved up, never hiding the others, counted like any lead
- [x] Compliance directory: export-compliance service categories (growing-area codes, residue testing, traceability), sponsored like any other
- [x] Crop price feed: the admin enters the day's farm-gate price of a crop per area; `/prices` and the home page show the fresh ones
- [x] Zalo Mini App: services near the farmer with a call button, crop prices, a job post; the position and phone come from Zalo (`zalo-mini-app/`)

---

## Monetization Model

Farmers and small providers (an excavator owner, a mechanic, a spraying crew) use the platform for free: they are individual households that will not pay a monthly fee, and a listing that costs money empties the directory. The revenue comes from the companies behind them, who have marketing budgets and need what only the platform has, real local demand and a way to the right farmer at the right moment:

1. **Demand data (B2B):** fertilizer and crop-protection brands, machinery dealers, cooperatives pay for the demand report of their categories and areas. Aggregates only: a partner never sees a farmer's name or phone, nor a count small enough to point at one household (cells under a minimum are hidden).
2. **Sponsored placement (B2B):** a drone or machinery dealer pays to put the providers it sells to or trains first in its categories and areas (they need customers to pay off their machines); labelled, never exclusive.
3. **Compliance and export partners (B2B):** cooperatives and durian exporters sponsor the directory of export-compliance services their growers must use.
4. **Optional, later:** a provider's own paid boost, only once providers ask for more reach than the free listing gives.

The weekly leads table is the sales argument: "last week in Cai Lậy, X farmers looked for spraying". No partner is signed before one area sustains real leads.

---

## Go-To-Market Strategy

1. **Field trip:** 1 week on-the-ground in target area to collect 50–100 initial provider listings manually
2. **Launch:** Minimal PWA + hyper-targeted Facebook Ads by area radius to drive first users
3. **Iterate:** Refine UI/UX based on farmer feedback (less text, more images, large buttons)
4. **First partners:** with 8+ weeks of steady leads in one area, take the demand figures to 2–3 local dealers (drone, fertilizer, machinery) and one cooperative; a first sponsored category per area
5. **Scale:** Expand to adjacent areas using oil-spill growth model, each area opened with its own field trip and partners

---

## KPIs

- New provider registrations per month
- Leads per week (call taps + Zalo taps + inquiries, spam left out; admin dashboard)
- Areas with steady leads (8+ weeks in a row): the markets partners can be sold
- Paying partners, and revenue per area
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
- Every failure is a `domain::Error` with an `ErrorCode`. Never put user-facing text in Rust: add the code to `domain/src/error.rs` and its Vietnamese text to `web/locales/vi.toml` `[errors]` (a test fails if either is missing). Never build an error response with an ad-hoc status code.
- Handlers extract with `ApiJson` / `ApiPath` / `ApiQuery` from `web::extract`, never axum's own, so malformed input gets a coded error.
- Environment variables are read only in `web/src/startup.rs`; everything else receives configuration as arguments.
- Migrations are DDL only and append-only: never edit or delete one that has run anywhere. Reference data (roles, permissions, categories, locations) lives in `application/src/catalogue.rs` and is seeded idempotently at startup; demo data only through `/api/setup/seed`.
- Keep internal logs in English.
- All location data uses **SRID 4326** (WGS84). Never mix geometry and geography types in the same query.
- Phone number pattern for Vietnamese mobile: `0\d{9}` (10 digits starting with 0).
- Radius validation bounds: 100 m minimum, 50 000 m maximum.
- `/services` lists services, not providers; a service is public when it is shown, not deleted, in a shown category and its business is live (`live_public` in the service repository, the one place that says so). Searching a service matches its name, description and the business's name; searching a provider matches its name, description and its live services' text.
- Public provider lists (`/providers`, near-me shops tab, search's shops tab) include providers with no service yet, so a new sign-up can be found; a service filter (`category_id`) matches only a live service in a live category. Featured services (home, near-me before a position): one per verified provider, those with a photo first, providers rotated by day, every category once before a repeat (`pick_featured`). Service cards are compact rows at every width (a fixed 104/120 px square photo beside the text, one call button), and fewer than three keep the two-column width (`service-grid--1/--2`), never one card across a wide screen. The default service order and the near-me services take turns by provider (`with_provider_turn`: each provider's first service before anyone's second); an explicit sort is kept as chosen. The wide top bar has a "Cửa hàng" tab (`wide_only`); on a phone the directory is the second of the `/services` tabs. The home page opens with one strip of category tiles: services (green squares) then products (amber circles), `HOME_CATEGORY_TILES` of each (one fewer on a phone, so each kind is one row of four), each kind ending in an outlined "Tất cả" tile to its whole list; near-me has no tile (its tab and the banner cover it). The "Tài khoản" tab is `phone_only`: on a wide screen the top bar's account button covers it. `/services` and `/providers` open with "Dịch vụ | Cửa hàng" tabs (`services::view_tabs`: both read `ListQuery`, so a switch keeps the search, category and sort); their near-me banner comes after the third card (`near_me_href`, category and tab kept), and an empty search offers the other list as its second action.
- Providers manage their own services (`own.service.create` / `update` / `delete`). Every owner-scoped query matches the owner in the same statement (an `in_subquery` on the business), the business comes from the session, never the request, and someone else's service answers `service_not_found`. `is_active` is the admin's moderation switch: an owner's edit never changes it. A business lists at most `SERVICES_PER_BUSINESS_MAX` services (hidden ones count; the business row is locked while counting). Admin and owner forms share `usecases::service_rules::validate_service` (a blank name takes the category's); likewise the business profile: `usecases::business_rules::validate_profile` (owner dashboard, admin create / edit), one form `organisms/business_fields.html`.
- Reviews (`ReviewUseCase`): a farmer reviews a provider only from the phone of a contact request that provider answered (`review_needs_contact` otherwise), once (`review_already_sent`, from the UNIQUE on `reviewer_phone_hash`); the phone is kept only as `hashing::sha256_hex` (the one SHA-256 helper) and never shown. A new review is hidden; the admin's "show" on `/admin/reviews` is the approval ("Chờ duyệt" until then). The owner's answer (`reply_own_review`, owner-scoped in one statement) shows under the review once it is shown; a blank answer removes it. Stars render through `atoms/stars.html` everywhere. A provider's rating follows it: service and provider cards (`cards::rated_services` / `rated_providers`, one `ratings` query per list) and the service and product pages (`molecules/rating_line.html`, linking to its reviews, `REVIEWS_ANCHOR`). After a contact request the page says a review comes once the provider called back, and the inbox offers each answered request the review link to send (`share_button` with `url`).
- Administrative units (`provinces`, `wards`): the bundled dataset `infrastructure/data/admin_units/` (official codes, a point in each unit, each ward's boundary simplified as one coverage: see its README) is loaded at startup by `SetupUseCase::seed_admin_units` when its version is not the one in `catalogue_state`; a unit it no longer lists is turned off, never deleted. No hamlets (thôn, ấp): no open national list. The ward of a point is `admin_unit_repository::ward_code_at`, the one definition: the ward that `ST_Covers` it, else the nearest within `ADMIN_UNIT_SNAP_M` by the index's `<->` (never `ST_Distance` on a boundary: seconds). An area travels as text: `p82` (province), `w28439` (ward), `none` (`application::admin_units::AreaKey`); `AdminUnits` names them ("Đồng Tháp — Xã Bình Phú"). Location presets stay what the near-me page, crop prices and job posts choose from. Tests that place something in a ward call `load_admin_units`.
- The demand report (`DemandReportUseCase`) counts leads (call and Zalo taps, inquiries without spam) of non-demo businesses over the `DEMAND_WEEKS` whole weeks before this one, Monday to Sunday in Vietnam (`VN_UTC_OFFSET_SECS`, the one offset). A lead counts under the ward its business lies in and that ward's province (else "no area"); the area list shows provinces in code order, each followed by its wards with leads; steady areas count the top-level rows. A tap on a shop with one live service takes that service's category. Any figure above zero and under `DEMAND_REPORT_MIN_COUNT` is hidden, and so is a total that would give exactly one hidden figure away; every view of the figures (page, CSV, JSON, a partner's) goes through the same `aggregate`. Steady areas: every report week at `STEADY_WEEKLY_LEADS`. Searches (`record_search`) are stored in the background with no identity: words (lowercased, `SEARCH_QUERY_MAX`), category, ward (of the preset's point or the position), result count; a page never waits for them. A signed-in visitor's are skipped, and the same search by the same visitor within `SEARCH_REPEAT_SECS` counts once (`pages::visitor`: the address is hashed in memory only, never stored).
- Partners (`PartnerUseCase`): a contract covers a whole province or one ward of it (`ContractArea`; the area stays when it is edited), and is in force between its dates (both included, Vietnam days) while its partner is on; the revenue figures are the monthly fees of contracts in force today. A partner account (role `partner`) reads only `ReportScope { areas, categories }` of its contracts in force. Sponsored shops must lie in the contract's area; on `/explore` (whose position's ward is looked up only when a contract is in force) they are moved up under "Tài trợ" (`feature_sponsored`: one service per shop, at most `SPONSORED_SHOWN`), never hidden, never shown twice, counted by the same call beacon. "Assign an account" forms (provider, partner) share `account_rules::account_for_role`.
- Crop prices (`PriceUseCase`): the admin (`price.manage`) enters a low–high price in đồng per unit for a shown crop in a shown area on a day not after today (Vietnam), at most `PRICE_PER_UNIT_MAX`; the same crop, area and day again replaces it (UNIQUE, upsert). The board (`build_board`, one place) shows each shown crop's latest price per area only within `PRICE_FRESH_DAYS`, with the move of its middle since the price before (`PRICE_HISTORY_DAYS` back); a hidden crop or area leaves it. The tab is `wide_only`; on a phone the home strip (`molecules/price_line.html`) leads there.
- Job posts (`JobUseCase`): a farmer posts a job (a shown service category, the phone's position or else a location preset, a description, a name and a phone); one phone has at most `JOB_POSTS_OPEN_PER_PHONE` open posts within `JOB_POST_DAYS`. A post is never public: `JobNotificationSubscriber` emails the nearest `JOB_NOTIFY_MAX` providers within `JOB_NOTIFY_RADIUS_M` with a live service in its category (live, not demo, the managing account's address else the business's) in a background task, and those providers see it with the phone on `/provider/jobs` while it is open and at most `JOB_POST_DAYS` old (`OFFERS_THE_JOB` in the job repository, the one place that says so). The admin closes a post or marks it spam.
- The Zalo Mini App (`zalo-mini-app/`) is a client of the JSON API, never a second backend: no business rule lives there, its words are in `src/texts.js`, errors show the backend's message, the API origin is `VITE_API_URL`. Zalo's location and phone tokens are converted only on the server (`ZaloUseCase` over the `ZaloGateway` port, `ZaloGraphGateway` the one file that calls the Zalo Graph API); the secret never reaches the phone. Its origin (`https://h5.zdn.vn`) goes in `CORS_ORIGINS`. CI only checks it builds; it is deployed with zmp-cli.
- Repository queries use Sea-ORM's query builder (`find`, `update_many`, `Query::select` subqueries, `lock_exclusive` in a transaction), not raw SQL strings; `Expr::cust_with_values` only for a PostGIS fragment the builder cannot express.
- Every list sort ends with an `id` tiebreaker, and price sorts put unpriced products last, so pages never repeat or skip rows.

### Backend — DDD / Layer Integrity Rules

These rules were codified after a full architectural review. Violations are compile-time detectable — enforce them on every PR.

**Layer dependency direction** (dependencies may only point inward):

```
middleware / handlers  →  application (use cases, ports)  →  domain  ←  infrastructure
```

**Domain types**

- `UserRole` lives in `domain/src/models/role.rs`. No other layer redefines role semantics. Handlers and use cases both import `UserRole` from the domain — never from middleware.
- A domain model struct that is used as input to a use case must carry every field that the use case needs to enforce a business rule. Never rely on a silent SQL `WHERE` filter to enforce a domain invariant that belongs in the use case (e.g. `is_active` on `UserCredentials`).

**Application layer (`application/`)**

- Use cases depend only on domain models, repository traits, and the port traits in `application/src/ports.rs`. Never import from `middleware::`, `handlers::`, or concrete infrastructure types (e.g. `jsonwebtoken`, `bcrypt`).
- Infrastructure concerns that a use case needs (password hashing, token minting) must be abstracted behind a port trait in `application/src/ports.rs` and injected via `Arc<dyn PortTrait>` in the constructor — same pattern as repository traits.
- Shared validation rules live in `application/src/validation.rs` and return coded errors; forms collect them with `domain::Validation` so every bad field is reported at once. Never scatter the same rule as a private function in multiple use case files.
- Before any `user_repo.update_role()` call, check the code against an active row in `roles`, so arbitrary strings never reach the role column.

**Middleware layer (`middleware/`)**

- Middleware only finds out who is calling (`MaybeActor`, from the bearer token or the `auth_token` cookie); it never decides access. Handlers that need a session call `require_actor`, then pass `&actor` to the use case.
- Every state-changing use case, and every admin read, starts with `actor.require(PERM_...)?`. Permission keys live in `application/src/catalogue.rs`. Never compare role names: a convention test rejects it, and another fails when a use case taking an `Actor` checks nothing.
- Permissions are loaded from the database on every request, so a revocation or a deactivated account takes effect on the next request, not when the token expires.
- Non-GET requests with a foreign `Origin` are refused (`cross_origin_request`); `APP_URL` and `CORS_ORIGINS` are the allowed origins.
- Public call buttons are `molecules/call_button.html`: a plain `tel:` link that Alpine `callBeacon` counts with `sendBeacon` to `/api/explore/calls` (uncounted without JS); its label is always the phone number. Detail pages (provider, service, product) add `molecules/zalo_button.html` beside it: a link to the Zalo chat with the same number (`zalo` filter, `ZALO_CHAT_URL`), counted by the same beacon with `data-call-channel="zalo"`; cards keep the one call button. Counts cover `CALL_STATS_DAYS` across both channels (admin dashboard total, the provider's own on their dashboard), and the admin dashboard's weekly table is the demand report's (one definition of a lead); a signed-in visitor's taps are never stored (every account is an admin, a provider or a partner); nothing about the caller is stored.
- Domain events (`domain/src/events.rs`) are published after a change is stored; the audit log is a subscriber. New side effects (notifications, webhooks) subscribe; use cases do not call them. A visitor's event (a farmer's inquiry) has no `actor_id` and is not audited; the inquiry subscriber emails the provider (the managing account, else the business's address) in a background task, linking to `/provider/inquiries`.

**Infrastructure layer (`infrastructure/`)**

- Concrete library types (`jsonwebtoken`, `bcrypt`, Sea-ORM) are confined to `infrastructure/`. Implement the port traits from `application/src/ports.rs` here.
- Uploads never reach storage as sent: `MediaUseCase` passes them through the `ImageProcessor` port (type sniffed from bytes, pixel cap checked before decoding, re-encoded as JPEG, EXIF dropped). Storage is the `FileStorage` port (`GCS_BUCKET`, else `UPLOAD_DIR`). Templates link images only with `file_url(id=…, variant="thumb")`. Old images are deleted by the file-cleanup event subscriber, never inline. Products and services each have one optional image, replaced through `MediaUseCase::swap_image`.
- A write that can hit a UNIQUE constraint maps the failure with `repositories::unique_or_internal` to a field error (e.g. `slug_taken`), never a 500.
- `JwtTokenService` (in `infrastructure/src/jwt_token_service.rs`) is the only file that calls `jsonwebtoken::encode`. `BcryptPasswordHasher` (in `infrastructure/src/bcrypt_password_hasher.rs`) is the only file that calls `bcrypt::hash` / `bcrypt::verify`.

**Errors (`domain/src/error.rs`, `web/src/error_response.rs`)**

- `IntoResponse for Error` (behind domain's `axum` feature) only sets the status and parks the error; `render_errors` writes the localised body. Every router must sit under that layer.

### Pages

- A page is a handler in `web/src/pages/` that takes `Shell` (the frame: top bar, account menu, bottom tabs, footer, built from the session) and renders through `views::render_page`. The admin panel is never a tab: admins reach it from the account menu and the footer. The footer (`organisms/app_footer.html`) reuses the tabs for its browse links and the account menu's links for its account column, and shows the platform's help lines from `SITE_SUPPORT_PHONE` / `SITE_SUPPORT_ZALO_URL` / `SITE_SUPPORT_EMAIL`; each unset one is hidden, never a made-up number. Admin pages render through `pages::admin::render_admin` (the admin frame, menu filtered by permission). A stale session cookie renders the page signed out; it never fails it.
- Templates follow the atomic levels above. Links come from `url_for`, product and provider slugs from the `slugify` filter, images from `file_url(id=…, variant="thumb")`, field limits (`maxlength`, `minlength`) from the global `limits`, never typed numbers.
- **No words in code:** every user-facing word lives in `web/locales/vi.toml` and `en.toml` (same keys, a test compares them; `[text]` nested tables read as dotted keys). Templates say `t(key="area.name", count=…)` (an unknown key fails the render, like `url_for`); page code says `state.locale().text("area.name")` / `text_with(key, &[("name", &value)])`, the request's language; `Shell` takes the `Locale`, `Views` holds one Tera set per language; emails get their wording at startup (`EmailTexts`). A sentence that wraps markup is split into `_before` / `_after` keys (never `| safe`). Roles and permissions are worded by code (`role.<code>`, `role_description.<code>`, `permission.<key>`) through `labels::Worded`, in pages and the JSON API alike; the database `name` (seeded as the code) is only the fallback. Convention tests fail on a key with no text and on Vietnamese letters outside the locale files (category and location seeds in `catalogue.rs`, which admins edit, and the slug rule excepted).
- **Bootstrap first:** Bootstrap 5 components and utilities before custom CSS; custom styles go in `static/css/app.css` only for what Bootstrap cannot do. No `style=""` attributes and no `<style>` elements: the page CSP is `style-src 'self'`.
- Interactive bits are Alpine components (CSP build) registered in `static/js/app.js`; attribute values name a component property, never an expression (convention test). Markup must work without them: links and plain forms first.
- What Alpine cannot do (today only geolocation) is an island: a Svelte custom element in `backend/islands/src` (light DOM, no `<style>`, all text passed as attributes), built with `npm run build` into `static/js/islands.js`, which is committed. CI rebuilds it and fails on a diff. Pages that use one load it in `{% block scripts %}`. Node is needed only for this and the Zalo Mini App.
- **Large tap targets** (min 44 px) and **minimal text**: the primary users are farmers on low-end Android phones with poor connectivity.
- Custom SQL in `Expr::cust_with_values` uses `$1`, `$2`: a `?` is left unbound and the query fails at runtime (convention test).
- Page forms post urlencoded (`ApiForm`) and follow Post/Redirect/Get: success is a 303 with a `cookies::Flash`; a broken rule re-renders the page with the error's status, the typed values and one message per field (from `error_body`). The rules stay in the use case the JSON API uses. A form with a file box posts `multipart/form-data`, read with `ApiMultipart::into_form`; a photo is checked with `MediaUseCase::prepare_image` before anything else is saved, and ownership is checked before the body is read.
- A short create or assign form on an admin page (category, location, service for one provider, the provider's managing account) is a sheet: a `contact-sheet` section with `molecules/sheet_header.html`, in an `x-data="dialog"` scope whose `data-open` is true when the form came back with errors; the trigger (`admin_header`'s `action_opens=true`, or a button in its section) is an anchor to it, so without JS it is a card on the page. A row confirmation then gets its own nested `dialog` scope. Edit forms on a record's own page and large forms (the role permission grid) stay on the page. A sheet is a bottom sheet on a phone and a centred dialog from 768 px (`app.css`), never per page.
- An action that needs a second look (deactivate, hide, delete) uses `organisms/confirm_sheet.html` inside an `x-data="dialog"` element; without JS it is an anchored form on the page. It says what will happen (`effect="hide"` / `"delete"` with `what`, else `note`), and its reason box is optional, capped at `limits.audit_reason_max` (`AUDIT_REASON_MAX`, also cut server-side by `validation::audit_reason`). On a list, the row's link reloads the list with `?hide={id}` (or `?delete=`) and the server renders that confirmation open. Row forms send the list position back as `page` / `provider_id` (`pages::admin::ListPosition`), rebuilt from numbers, never a posted URL; for a record that also has its own page they add `back=list` (`ListReturn`), else its handler lands on that page.
- Every admin hide / deactivate / delete asks for confirmation and a reason (audited); show / activate is one button. Lists narrowed to one provider (`provider_id`: products, services, inquiries, reviews) use `ListPosition::provider_filter` / `pagination` and `molecules/admin_filter.html`.
- Admin lists end each row with `molecules/row_action.html` macros, one per kind (`show` green or `hide` amber, `delete` red, then `open`, or `public` for a record with no admin page), in that order; icon-only on a narrow screen, icon and a short word from 992 px. A table has only the ones its records support. States are `atoms/status_badge.html` (`visibility`, `account`, `verified`); a hidden or switched-off row is grey (`table-light text-body-secondary`), never amber, and green never marks anything but shown / active / verified / call. Rows start with `atoms/admin_thumb.html` where the record has a photo. Inquiry statuses are `inquiry_status::label(badge=true)` (new is blue). Detail pages are `admin-section` cards (title, then `admin-status` line, then one `admin-actions` row); forms are `form-stack`; deleting is its own card at the end. An admin page's one top action (add, public page) goes in `admin_header`'s `action_*`.
- The session lives in httpOnly cookies (`auth_token`, `refresh_token`); no script ever reads a token.
- **Languages:** Vietnamese by default; English when the `lang` cookie says so. The browser's `Accept-Language` is never applied (farmers' phones are often set to English), only offered: a visitor with no cookie whose browser prefers another language of ours sees a one-time strip under the top bar (`molecules/language_offer.html`, `shell.language_offer`), worded in that language with the way out in Vietnamese; either answer sets the cookie. `middleware::language` scopes every request with it (outermost of the page layers, so error bodies and the JSON API follow it); `locale::current()` reads it. One switch, `molecules/language_switch.html`, the same everywhere: a button with the language icon and the current code ("VI"), a `<details>` menu that opens without JS (Alpine `languageMenu`, sharing `topBarMenu` with the account menu, adds closing on Escape / outside). One place per page, never repeated (not in the footer, not in the account menu): the right end of the top bar on app pages and of the admin bar; under the card, at its right edge, on auth pages (`up=true`: the menu opens above it), never above the form nor pinned to a screen corner. Each language is named in itself, never a flag. Emails go out in the default language.

### SEO

Public content pages fill `{% block title %}` and `{% block meta %}` (a `<meta name="description">`) from the loaded data; never a generic fallback.

| Route group                                                  | SEO required | Notes                                                   |
| ------------------------------------------------------------ | ------------ | ------------------------------------------------------- |
| `/` (home)                                                   | Yes          | Static title + description                              |
| `/products`, `/products/[id]/[slug]`                         | Yes          | Product name & description in meta                      |
| `/products/category/[id]/[slug]`                             | Yes          | Category name in title                                  |
| `/services`, `/services/category/[id]/[slug]`                | Yes          | Category name in title                                  |
| `/services/[id]/[slug]`                                      | Yes          | Service + provider name in title, description in meta   |
| `/providers`                                                 | Yes          | Static title + description                              |
| `/prices`                                                    | Yes          | Area name in title when filtered                        |
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

# The Zalo Mini App (see zalo-mini-app/README.md)
cd ../../zalo-mini-app; npm ci; npm run build
```

> **Windows note:** If `npm` fails with a script execution policy error, run:
> `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`

### Deployment

GitHub Actions (`.github/workflows/ci.yml`, `deploy.yml`) builds one image and deploys it to one Compute Engine VM; one-time setup, backups and the restore procedure are in `deploy/README.md`.
