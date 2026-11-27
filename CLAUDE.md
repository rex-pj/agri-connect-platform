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
│   │   └── tests/              # Integration + convention tests (no workspace lints): one binary, tests/it/ (a module per file in main.rs)
│   ├── islands/                # Svelte custom elements → static/js/islands.js
│   └── migration/              # Sea-ORM migrations: DDL only, append-only
├── deploy/                     # compose, Caddy, VM deploy script, backups (see deploy/README.md)
└── zalo-mini-app/              # Zalo Mini App (React, zmp-ui, zmp-sdk): a JSON API client (see its README)
```

---

## Tech Stack

### Backend

- **Language:** Rust (edition 2021; toolchain pinned in `backend/rust-toolchain.toml`)
- **Framework:** Axum 0.8, with tower-http (CORS, security headers, request ids, tracing)
- **Async runtime:** Tokio
- **ORM:** Sea-ORM 1.1 with PostgreSQL; migrations with sea-orm-migration
- **Database:** PostgreSQL with PostGIS extension for geo queries
- **Auth:** JWT (jsonwebtoken 9) + bcrypt 0.15
- **Rate limiting:** governor 0.6
- **Email:** Lettre 0.11 (SMTP)
- **Uploads:** image 0.25 (re-encoding), object_store 0.12 (Cloudflare R2, Cloud Storage or local disk)
- **HTTP client:** reqwest 0.12 (the Zalo Graph API only)
- **Serialization:** Serde + serde_json; toml for the locale files
- **Validation:** our own rules (`application/src/validation.rs`, collected with `domain::Validation`); no validation crate
- **Logging:** tracing + tracing-subscriber
- **Error handling:** thiserror 2 (`domain::Error`)
- **IDs:** integer keys; UUID v4 only for uploaded files
- **Config:** environment variables read in `web/src/startup.rs`, `.env` through dotenvy; no config crate
- **Versions:** every third-party version and kept feature is in `backend/Cargo.toml` `[workspace.dependencies]`, the one place to change them

### Frontend (server-rendered by the Rust server)

- **Templates:** Tera, rendered by `web::views`; every page works without JavaScript
- **UI Framework:** Bootstrap 5.3 (vendored under `static/vendor`)
- **Interactivity:** Alpine.js (CSP build), components registered in `static/js/app.js`
- **Admin list toolbar:** every admin list is one GET form (`molecules/admin_search.html`, built by `SearchBox` in `web/src/pages/admin/mod.rs`), so the result is a URL. Four rows, each only when the list has it: the words, the status chips (`chips=`, worded like the badge in the row), the filters folded behind a "Filters" button (`picked` selects, `toggled` checkboxes, `texted` words, `ranged` / `date_ranged` ranges, `dated` the days made; open when one is on, Alpine `adminToolbar`), and the criteria in force as removable chips. The order is not a filter: it is chosen on the result bar (`admin_results`, `sorted`), a form of its own that carries every criterion. A criterion is one field of `ListPosition` (`param` / `without` / `query`) that `AdminFilter` (or the list's own filter) carries to the repository; a handler must pass the whole position on, never pick fields (a dropped field means a filter that does nothing). `it/conventions.rs` `admin_lists_share_one_toolbar` keeps the lists on this one macro.
- **Islands:** Svelte 5 custom elements for what Alpine cannot do (today: geolocation)
- **Searchable selects:** Tom Select 2.6 (vendored, `static/vendor/VERSIONS`) over the province / ward selects (`molecules/form_field.html` `area_select`, Alpine `searchableSelect`, accents ignored; every option listed, provinces in Vietnamese dictionary order and each ward by its own name: `slug::sort_key`, `AdminUnits::choices`; the demand report keeps the official code order); a page loads it with `searchable_select_assets()` in its `vendor` block, and without JS the select is a plain one
- **Placing a business:** nobody types coordinates. The location box (`organisms/business_fields.html`, the provider's profile and the admin's create sheet and profile tab) asks for the address and the ward (typed to search; alone it places the business at the ward's centre, said to be approximate: `PlaceUseCase::placed`, `PlaceDto.approximate`), then "I am where I work now" (`ac-geolocate`, refusing a position rougher than `GEOLOCATE_ACCURACY_MAX_M`) or a pin on a map (Alpine `placePicker` over Leaflet 1.9, vendored and loaded only when the map opens; tiles from our own `/map/tiles/{z}/{x}/{y}`, OpenStreetMap credited). Each new point is named by its ward (`GET /api/places/ward`, the same `ward_at` as the reports), which the list then shows. A ward chosen with a point in another ward wins (the ward was changed without a new pin); a point in no ward with none chosen is refused (`place_outside_wards`). Folded under "Dán toạ độ hoặc link Google Maps", one box (`coords`) takes "10.4048, 106.1180", comma decimals, a pair written longitude first, or a full Google Maps link (`forms::typed_point`, the place's `!3d…!4d…` before the map's `@…`, then `?q=` / `?query=` / `?ll=`; mirrored by `typedPoint` in `app.js`; a maps.app.goo.gl short link is said to hold no point); without JS it and the ward list are the form
- **Icon picker:** every icon box (sectors, alert kinds, commodities, service and supply types) is `molecules/form_field.html` `icon_select`: a text box holding the Font Awesome name, which Alpine `iconSelect` (Tom Select) turns into a picker loading its matches as the name is typed from `GET /api/admin/icons` (`icons::search`, best first, `ICON_SEARCH_RESULTS`), each drawn with its glyph; without JS the name is typed. `application::icons::ICONS` is every name the vendored Font Awesome defines, aliases too (`icons/names.rs`, generated from `fontawesome.min.css`; `it/conventions.rs` `the_icon_list_is_the_vendored_fonts` fails when they differ, so a Font Awesome update regenerates it). Stored as `fa-…` (`cow` and an older `fa-solid fa-cow` are taken as `fa-cow`); `validation::icon` refuses any other name (`invalid_icon`); a stored one no longer listed is said under the box
- **Icons:** FontAwesome 6 (fa-solid classes)
- **Design methodology:** Atomic Design (atoms → molecules → organisms → pages)
- **Target:** optimized for low-end Android on weak signal; pages work without JS

### Infrastructure

- **Cloud:** Google Compute Engine (one VM, docker compose), Artifact Registry, Cloud Storage for backups; Cloudflare in front (proxy, Origin Certificate) and R2 for uploads; deployed by GitHub Actions
- **Map tiles:** OpenStreetMap's tile server (`MAP_TILE_URL` for another host), fetched by the server (`MapTiles` port, `OsmMapTiles`), only the tiles touching Vietnam (`MAP_REGION_*`, zoom `MAP_ZOOM_MIN`–`MAP_ZOOM_MAX`), kept a week by the browser and Cloudflare, never on the server
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

| Level | Folder | Examples |
| --- | --- | --- |
| Atoms | `atoms/` | `icon`, `logo`, `alert`, `inquiry_status`, `photo_placeholder` |
| Molecules | `molecules/` | `form_field`, `page_hero`, `section_header`, `pagination_nav`, `empty_state`, `admin_header` |
| Organisms | `organisms/` | `product_card`, `provider_card`, `category_grid`, `contact_sheet`, `confirm_sheet`, `*_fields` |
| Layouts | `layouts/` | `app.html` (public + provider), `admin.html` (admin panel), `partner.html` (partner report) |

**Rules:**

- Atoms have no dependencies on other templates.
- Molecules may compose atoms only.
- Organisms may compose atoms and molecules.
- Page-level composition happens in `templates/pages/`, not in organisms.
- Never add a new template at a lower level when an existing higher-level one already covers the need.

---

## Domain Model

### Core Entities

| Entity | Key Fields | Notes |
| --- | --- | --- |
| `roles` | id, code, name, is_active | Codes: `admin`, `provider`, `partner` |
| `users` | id, email, password_hash, full_name, role_id, is_active, failed_login_attempts, locked_until, email_verified_at, sessions_valid_after, is_demo | Soft-deletable; named by its email alone (no username since `m20261127_000008_account_email`); `is_demo`: an account of the demo data at volume |
| `sectors` | id, code (UNIQUE), name (UNIQUE), icon, display_order, is_active, is_demo | What the platform serves ("ngành": trồng trọt, chăn nuôi, thuỷ sản), kept by the admin; `code` made from the name once, never changed (the sector cookie and `sector=` carry it); the other tables' `sector` columns point to it |
| `commodities` | id, name (UNIQUE), sector, icon, display_order, price_fresh_days, is_active, is_demo | What a farmer raises ("cây trồng, vật nuôi": lúa, sầu riêng, heo, tôm); fixes the sector of its crops and of the supplies for it; `price_fresh_days` (1–60, default 14) how long a price of its crops stays on the board |
| `service_types` | id, name, slug, icon, display_order, sector?, is_active, is_demo | What a provider is hired for ("loại dịch vụ"); `sector` none = every sector |
| `supply_types` | id, name, slug, icon, display_order, sector?, is_active, is_demo | What a provider sells ("loại vật tư"); one id sequence (`catalogue_type_ids`) with `service_types`, so a bare id names one row of either |
| `businesses` | id, name, phone, email, description, address, lat, lng, ward_code, is_verified, is_active | Provider/business listing; `ward_code` the ward of its position, written with it |
| `services` | id, business_id, service_type_id, name, description, price_note, is_active, image_file_id | Offered by a business; `is_active` is admin's |
| `products` | id, business_id, supply_type_id, commodity_id?, name, description, price, unit, is_active | Supplies ("vật tư") a business sells; `commodity_id` what it is for (lúa giống ST25: Lúa), none = every one |
| `inquiries` | id, business_id, service_id, product_id, topic, requester_name, requester_phone, note, status, answered_on | Statuses: `new`, `contacted`, `closed`, `spam`; about a service or a supply of its own business, never both |
| `reviews` | id, business_id, inquiry_id, reviewer_name, reviewer_phone_hash, rating (1–5), comment, reply, replied_at, is_visible | Hidden until an admin shows it |
| `call_clicks` | id, business_id, service_id, product_id, channel, created_on | Call / Zalo taps (`call`, `zalo`); never who |
| `password_reset_tokens` | id, user_id, token, expires_at, used_at | Single-use, 1-hour TTL |
| `email_verification_tokens` | id, user_id, token_hash, expires_at, used_at | Single-use, 24-hour TTL; stamps `users.email_verified_at` |
| `search_events` | id, query, service_type_id?, supply_type_id?, ward_code, results, created_on | What was searched and in which ward; never who |
| `call_click_weeks`, `search_event_weeks` | week_starts_on, (business_id, channel, taps) / (ward_code, searches, unmet, is_demo), service_type_id?, supply_type_id? | Taps and searches older than `EVENT_KEEP_WEEKS`, counted by week; the rows they were counted from are deleted |
| `partners` | id, name, kind, contact_name, phone, email, account_id, is_active | Kinds: `brand`, `dealer`, `cooperative`, `exporter` |
| `partner_contracts` | id, partner_id, province_code, ward_code?, starts_on, ends_on, monthly_fee (+ `partner_contract_service_types`, `partner_contract_supply_types`, `partner_sponsored`) | A whole province or one ward of it; in force between its dates while the partner is on |
| `provinces` | code (official, 2 digits), name, full_name, unit_type, center, is_active | The 34 since 1 July 2025; loaded from the bundled dataset, never deleted |
| `wards` | code (official, 5 digits), province_code, name, full_name, unit_type, center, boundary, is_active | 3 321 wards, communes, special zones; `boundary` a simplified `GEOGRAPHY(MULTIPOLYGON)` |
| `crops` | id, commodity_id, name (UNIQUE), unit, display_order, is_active, is_demo | Farm produce ("nông sản") on the price board: a variety or grade of one commodity ("Lúa ST25", "Heo hơi"); demo ones from `/setup/seed`, hidden never deleted |
| `crop_prices` | id, crop_id, location_id, observed_on, price_low, price_high, note | One per crop × area × day (again replaces it) |
| `job_posts` | id, service_type_id, location_id, location, description, requester_name, requester_phone, status | Statuses: `open`, `closed`, `spam`; never public |
| `alert_kinds` | id, code (UNIQUE), name (UNIQUE), icon, display_order, is_active, is_demo | What an alert warns of (dịch bệnh, xâm nhập mặn, thời tiết), kept by the admin like the sectors (same columns, `CodedUseCase`) |
| `alerts` | id, kind, sector?, province_code?, title, body, starts_on, ends_on, is_active | `kind` one of `alert_kinds` (by code); never deleted, turned off |
| `platform_settings` | key (PK, dotted: `jobs.open_days`), value (text), updated_on, updated_by | The settings an admin moved off their defaults (`application::settings`); no row = the default; never seeded |
| `info_pages`, `info_page_revisions` | code (UNIQUE, the address), title, icon, stage (`on` / `preview` / `off`), in_footer, footer_group (`about` / `policy`), display_order, is_demo / page_id, body (Markdown), effective_on, change_note | The information pages (terms, policies, rules, contact, the legal entity); a version is never edited, the one in force is the latest whose day has come |

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

| Method | Path | Description |
| --- | --- | --- |
| GET | `/health/live`, `/health/ready` | Liveness; readiness (database reachable) |
| GET | `/api/explore/providers` | Geo-search (`lat`, `lng`, `radius?` default 10 km, `category_id?`) or browse (paginated, `page?`) |
| GET | `/api/explore/providers/{id}` | Provider detail |
| GET | `/api/explore/services` | Same two modes as providers, over public services (`name?` = text, `sort?`, `sector?`); each with its business |
| GET | `/api/explore/services/{id}` | Public service detail |
| GET | `/api/explore/featured` | Home featured data: services (one per verified provider), providers, products (`sector?`) |
| GET | `/api/explore/alerts` | Alerts in force today (`sector?`: that sector's and every sector's), newest first, as the home page |
| GET | `/api/explore/features` | Which parts the caller sees, `{prices, near_me, supplies, directory, reviews, jobs}` (a part in preview only for `feature.preview`): the Zalo Mini App offers only those |
| GET | `/api/explore/categories` | Category listing (filter by `type=service\|product`) |
| GET | `/api/explore/locations` | Location presets listing |
| GET | `/api/explore/sectors` | The shown sectors, in their order: what every `sector=` filter takes |
| GET | `/api/explore/alert-kinds` | The shown alert kinds, in their order |
| GET | `/api/explore/commodities` | The shown commodities (cây trồng, vật nuôi), by sector and order: what `commodity_id` filters take |
| POST | `/api/explore/jobs` | A job post (`category_id`, `lat`+`lng`, else `ward` `w28439` (its centre), else `location_id`; description, name, phone; 201 `{id}`; 30/min) |
| POST | `/api/zalo/location`, `/api/zalo/phone` | The Zalo Mini App's `{access_token, token}` → `{lat, lng}` / `{phone}` through the Zalo Graph API (`ZALO_APP_SECRET`; unset: 503 `zalo_not_configured`; 30/min) |
| GET | `/api/explore/prices` | Farm prices by commodity (`commodities`, each crop's `commodity_id`): each shown crop's latest price per area within its commodity's `price_fresh_days`, its move (`location_id?`, `sector?`) |
| GET | `/api/explore/prices/latest` | The few newest prices the website's home strip shows (each crop's newest fresh one, most recent first, `HOME_PRICES_SHOWN`) with the board's `today` (`location_id?`, `sector?`): the Zalo Mini App's home, which picks nothing itself |
| GET | `/api/explore/products` | Browse products (paginated, `category_id?`, `commodity_id?`) |
| GET | `/api/explore/products/{id}` | Product detail |
| POST | `/api/explore/contact` | Submit contact/inquiry (`service_id?` or `product_id?`: what it is about; rate-limited: 30/min) |
| GET | `/api/explore/providers/{id}/reviews` | Shown reviews: `count`, `average`, the newest `PUBLIC_REVIEWS_SHOWN` with the provider's replies |
| POST | `/api/explore/reviews` | A review (`business_id`, name, phone, `rating` 1–5, comment; 201, hidden until approved; 30/min) |
| POST | `/reviews` | Same review as a page form (urlencoded); 303 + flash, or the page re-shown with the sheet open |
| POST | `/api/explore/calls` | A tap on a public call or Zalo button (`{kind, id, channel?}`: `call` default, `zalo`; 204; 30/min); the business is looked up from the record |
| POST | `/contact` | Same inquiry as a page form (urlencoded); 303 + one-time flash, or the page re-shown with field errors |
| GET | `/files/{id}`, `/files/{id}/thumb` | Uploaded image (JPEG, cached forever; ids are never reused) |
| GET | `/api/places/wards` | The active wards whose name or province holds every word of `q` (accents ignored, at least `SEARCH_QUERY_MIN` characters), by name, at most `WARD_SEARCH_RESULTS`: `{wards: [{value, label}]}`; a ward box that loads as it is typed (searches budget) |
| GET | `/api/places/ward` | The ward a point lies in (`lat`, `lng`): `{area, label, approximate}` (`w28439`, "Xã Bình Phú, Đồng Tháp"; `area` empty outside every ward); the location box's pin (searches budget) |
| GET | `/map/tiles/{z}/{x}/{y}` | A map tile of the region, PNG, a week in cache (`map_tile_not_found` outside it, `map_unavailable` when the host fails; 600/min) |

### Auth (rate-limited: 10 req/min)

| Method | Path | Description |
| --- | --- | --- |
| POST | `/api/auth/tokens` | Login → JWT (in a session of its own): `{email, password}` |
| DELETE | `/api/auth/tokens` | Logout: the caller's token stops working at once (204) |
| POST | `/api/users` | Provider self-registration |
| POST | `/api/auth/password-resets` | Send password reset email |
| PATCH | `/api/auth/password-resets/{token}` | Consume reset token, set new password |
| POST | `/api/admin/setup` | Create first admin account (rate-limited, one-time) |

### User (session required)

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/users/me` | Current user profile |
| PATCH | `/api/users/me` | The name the account is shown by (`full_name`, blank clears it; 204) |
| PATCH | `/api/users/me/password` | Change password |
| POST | `/api/users/me/email` | Move to `new_email` (`current_password`; 202: nothing changes until the link sent there is opened) |
| DELETE | `/api/users/me/sessions` | Every session of the account ends (204) |

### Admin (each route checks its own permission key, e.g. `user.read`, `provider.verify`)

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/admin/stats` | Dashboard statistics (with `previous_calls`, the `CALL_STATS_DAYS` before, and `new_businesses`, real ones of the last `NEW_BUSINESS_DAYS`) |
| GET | `/api/admin/reports/demand` | Demand report (`area?`: `p82`, `w28439`, `none`; each area's `value`, `level`, `label`, `weeks`, `weeks_at_bar`; hidden figures are `value: null`; `report.demand.read`) |
| GET | `/api/admin/partners` | Partners, a page (`page?`), with contracts in force and monthly revenue (`partner.manage`) |
| POST | `/api/admin/partners` | Create a partner (201 + detail) |
| GET | `/api/admin/partners/kpis` | Paying partners, monthly revenue, by province (`provinces`, the most first, each with its contract `areas`) |
| GET | `/api/admin/partners/{id}` | Partner detail with contracts |
| PUT | `/api/admin/partners/{id}` | Update a partner |
| PATCH | `/api/admin/partners/{id}/status` | Turn on / off (`is_active`, `reason?`; off: its contracts stop) |
| POST | `/api/admin/partners/{id}/account` | Link or create its report account (role `partner`) |
| POST | `/api/admin/partners/{id}/contracts` | Add a contract (`area`: `p82` / `w28439`, `category_ids`, `starts_on`, `ends_on`, `monthly_fee`, `note?`) |
| PUT / DELETE | `/api/admin/partners/{id}/contracts/{cid}` | Edit a contract (dates, fee, note, categories; not its area) / delete it (`reason?`) |
| POST / DELETE | `/api/admin/partners/{id}/contracts/{cid}/sponsored[/{bid}]` | Sponsor a shop of the area of the contract / stop (`reason?`, audited) |
| GET | `/api/admin/users` | User list (paginated) |
| GET | `/api/admin/users/{id}` | User detail |
| PATCH | `/api/admin/users/{id}/role` | Update user role |
| PATCH | `/api/admin/users/{id}/status` | Toggle user active state |
| PUT | `/api/admin/users/{id}` | Correct its name and email (`full_name?`, `email?`: another address waits for its link, 202; `reason?`; `user.update`, never one's own) |
| POST | `/api/admin/users/{id}/password-resets` | A reset link to its address (`reason?`; 204; `user.update`) |
| GET | `/api/admin/users/{id}/permissions` | Effective permissions (via role) |
| GET | `/api/admin/roles` | Role listing |
| GET | `/api/admin/roles/{id}` | Role detail |
| GET | `/api/admin/permissions` | All available permissions |
| GET | `/api/admin/categories` | Category listing |
| POST | `/api/admin/categories` | Create category |
| GET | `/api/admin/categories/{id}` | Category detail |
| PUT | `/api/admin/categories/{id}` | Update category |
| GET | `/api/admin/categories/slug` | Free slug for `text` (name or slug; `id?` = the one edited): `{slug, taken}` |
| GET | `/api/admin/icons` | Font Awesome names matching `q` (`fa-` and dashes optional), best first, at most `ICON_SEARCH_RESULTS`: `{icons}` (`category.manage` or `alert.manage`) |
| PATCH | `/api/admin/categories/{id}/status` | Toggle category active state |
| GET | `/api/admin/providers` | Provider list (paginated) |
| POST | `/api/admin/providers` | Create a provider with no account (profile boxes; 201 + detail; `provider.manage`) |
| GET | `/api/admin/providers/{id}` | Provider detail |
| PUT | `/api/admin/providers/{id}` | Replace its profile (the owner's rules; detail; `provider.manage`) |
| PATCH | `/api/admin/providers/{id}/verification` | Set verification badge (`is_verified`, `reason?`) |
| PATCH | `/api/admin/providers/{id}/status` | Turn a provider on / off (`is_active`, `reason?`; off: gone from public pages) |
| POST | `/api/admin/providers/{id}/users` | Link a provider account (or create one); one business per account |
| GET | `/api/admin/inquiries` | Inquiry list (paginated, `provider_id?`, `status?`) |
| PATCH | `/api/admin/inquiries/{id}/status` | Update inquiry status (`status`, `reason?`) |
| GET | `/api/admin/reviews` | Review list (paginated, `provider_id?`) |
| PATCH | `/api/admin/reviews/{id}/visibility` | Toggle review visibility |
| GET | `/api/admin/jobs` | Job posts, newest first (`page?`, `status?`; `job.moderate`) |
| GET / PATCH | `/api/admin/settings` | Every setting by group (value, default, bounds, `changed_on`) / change some (`{values: {key: text}, reason?}`; 204; each broken value a field error on its key; `unknown_setting`) (`setting.manage`) |
| PATCH | `/api/admin/jobs/{id}/status` | `open` / `closed` / `spam` (`reason?`) |
| GET / POST, GET / PUT, PATCH | `/api/admin/alert-kinds`, `…/{id}`, `…/{id}/status` | Alert kinds, as the sectors (`alert.manage`) |
| GET / POST | `/api/admin/sectors` | Sectors (hidden too) / add one (`name`, `icon?`, `display_order?`; the code from the name; 201) (`category.manage`) |
| GET / PUT | `/api/admin/sectors/{id}` | Sector detail / edit (name, icon, order; the code stays) |
| PATCH | `/api/admin/sectors/{id}/status` | Show / hide a sector (`is_active`, `reason?`) |
| GET / POST | `/api/admin/commodities` | Commodities (hidden too) / add one (`name`, `sector`, `icon?`, `display_order?`; 201) (`category.manage`) |
| GET / PUT | `/api/admin/commodities/{id}` | Commodity detail / edit |
| PATCH | `/api/admin/commodities/{id}/status` | Show / hide a commodity (`is_active`, `reason?`) |
| GET / POST | `/api/admin/crops` | Crops (hidden too) / add one (201) (`price.manage`) |
| GET / PUT | `/api/admin/crops/{id}` | Crop detail / edit |
| PATCH | `/api/admin/crops/{id}/status` | Show / hide a crop (`is_active`, `reason?`) |
| GET / POST | `/api/admin/prices` | Prices entered, newest day first (`page?`) / enter one (`crop_id`, `location_id`, `observed_on`, `price_low`, `price_high?`, `note?`; 201 `{id}`) |
| DELETE | `/api/admin/prices/{id}` | Delete a price (`reason?`) |
| GET | `/api/admin/locations` | Location preset list |
| POST | `/api/admin/locations` | Create location preset (`ward?` `w28471` fills a blank label and a missing lat/lng) |
| POST | `/api/admin/locations/bulk` | A preset for each ward (`{wards: ["w28439", …]}`, at most `LOCATION_BULK_MAX`), named and placed as the one-ward form does; a name already there is passed over; one unknown ward refuses them all (201 `{created, skipped}`) |
| GET | `/api/admin/locations/{id}` | Location preset detail |
| PUT | `/api/admin/locations/{id}` | Update location preset |
| PATCH | `/api/admin/locations/{id}/status` | Toggle location active state |
| DELETE | `/api/admin/locations/{id}` | Delete location preset |
| GET | `/api/admin/products` | Product list (paginated) |
| POST | `/api/admin/products` | List a supply for a provider (`business_id` + the product boxes; the owner's rules and cap; 201 + detail) |
| GET | `/api/admin/products/{id}` | Product detail |
| PATCH | `/api/admin/products/{id}/active` | Toggle product active state |
| DELETE | `/api/admin/products/{id}` | Delete product |
| GET | `/api/admin/services` | Service list (paginated) |
| POST | `/api/admin/services` | Create service |
| GET | `/api/admin/services/{id}` | Service detail |
| PUT | `/api/admin/services/{id}` | Update service |
| PATCH | `/api/admin/services/{id}/active` | Toggle service active state |
| DELETE | `/api/admin/services/{id}` | Delete service |

### Provider Dashboard (`own.*` permissions; reads need only a session, except the inquiry inbox: `own.inquiry.manage`)

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/provider/profile` | View own business profile |
| PUT | `/api/provider/profile` | Update own business profile (`ward?` `w28439`: without a point, or with one in another ward, placed at its centre) |
| GET | `/api/provider/products` | List own products |
| POST | `/api/provider/products` | Create product |
| PUT | `/api/provider/products/{id}` | Update product |
| DELETE | `/api/provider/products/{id}` | Delete product |
| PUT | `/api/provider/products/{id}/image` | Replace product image (multipart field `image`; 20/min) |
| GET | `/api/provider/services` | List own services (admin-hidden ones too) |
| POST | `/api/provider/services` | Create service (blank name → category name; max 20) |
| PUT | `/api/provider/services/{id}` | Update service (never changes admin visibility) |
| DELETE | `/api/provider/services/{id}` | Delete service |
| PUT | `/api/provider/services/{id}/image` | Replace service image (multipart field `image`; 20/min) |
| GET | `/api/provider/reviews` | A page of the own business's reviews (waiting ones too; `page?`) |
| GET | `/api/provider/jobs` | A page of the open job posts near the own business (`page?`; `own.job.read`) |
| PUT | `/api/provider/reviews/{id}/reply` | Public answer (`{reply}`; blank removes it; 204; `own.review.reply`) |

### Partner (`partner.report.read`)

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/partner/report` | The partner account's own demand report (`area?`), its contracts in force |

### Setup (first-run / development only)

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/setup/status` | Check whether first admin exists |
| POST | `/api/setup/seed` | Load demo data (flagged is_demo) |
| DELETE | `/api/setup/seed` | Remove all demo data |

---

## Security Model

- **JWT expiry:** 1 hour
- **Browser sessions:** httpOnly `auth_token` (the JWT) + `refresh_token` cookies, `SameSite=Lax`; behind TLS `Secure` and named `__Host-auth_token` / `__Host-refresh_token`, so no other site under the domain can plant one (`CookiePolicy::session_name` / `refresh_name`, `AppState.cookies`: every reader takes the names from there). Refresh tokens last `REFRESH_TOKEN_DAYS` (30), are stored as SHA-256 hashes, rotate on every use; a spent one reused after 30 s revokes its whole family. `session::renew` renews an expired access token before any handler runs. `POST /logout` (and `/admin/logout`) ends the family. A password change or reset ends every other session at once: it sets `users.sessions_valid_after`, and access tokens issued before it (`iat`) are refused.
- **Password reset token TTL:** 1 hour, single-use, stored as a SHA-256 hash; the link is spent, the password stored and the account's other links retired in one transaction (`reset_password_with_token`), and a made-up link is refused before the password is hashed; expired links are purged with the expired refresh tokens; at most `PASSWORD_RESET_LINKS_MAX` valid links per account, and the email is sent in the background so a known address answers as fast as an unknown one
- **Account lockout:** per client: `LOGIN_FAIL_MAX_ATTEMPTS` (5) wrong passwords from one address (an IPv6 /64) keep that client out of that account for `LOGIN_LOCK_MINUTES` (15), in memory (`repeats::WindowCounter`), while the owner elsewhere still gets in; the account itself locks only after `LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS` (50) from every client together (`users.locked_until`). The use case is told the client by `client_ip::client_key`, never stored
- **Sign-in name:** an account is named by its email alone (stored lower-cased, UNIQUE): the sign-in box, the "assign an account" forms (`account_rules::account_for_role`) and the JSON API take it; pages show the full name, else the email (`domain::models::user::display_name`, `sql::account_name` in queries). A box that is not an address is `invalid_email` before anything is looked up, never counted. The session answers with the stored address
- **Login answers:** a wrong password is always `invalid_credentials`, after the same bcrypt check (an unknown email is verified against a decoy hash); only the right password is told `account_deactivated` / `account_locked`. Guesses at a locked or deactivated account are not counted
- **First admin:** with `SETUP_TOKEN` set, `/setup` and `POST /api/admin/setup` need it (`setup_token_invalid`); in production (`APP_ENV=prod`) startup refuses while there is no admin and no token, and refuses without `SMTP_HOST`
- **Access tokens end with their session:** every JWT carries its session id (`sid`, the refresh-token family; `TokenService::mint` takes no token outside one, and a token without `sid` is `invalid_token`), and `session_actor` refuses it once the family is revoked (`family_live`): logout (`/logout`, or `DELETE /api/auth/tokens` for a bearer), a password change, a reused refresh token and an admin deactivating the account (`revoke_user` + `sessions_valid_after`) all take effect on the next request. Login, sign-up and first-admin setup start the session in the use case (`LoginResponse.session`): a page puts both tokens in cookies, the JSON API answers only the access token
- **Email confirmation:** a sign-up's address is unconfirmed (`users.email_verified_at`) until the emailed link's form is posted (`/verify-email`: the GET only shows the button; token stored as SHA-256, single-use, `EMAIL_VERIFICATION_EXPIRY_HOURS`, at most `EMAIL_VERIFICATION_LINKS_MAX` valid at once; `/account` offers it again). Accounts an admin creates start confirmed. A notice to a business (`sql::notice_email`, the one definition: contact requests, job posts) goes to its managing account's address only once confirmed, to the business's own address only when it has no account
- **Who is calling** (`find_actor`: account, permissions, `sessions_valid_after`) is read from the primary database, never `DATABASE_READ_URL`
- **Password:** at least 8 characters, at most 72 bytes (bcrypt ignores the rest), not on `application/src/common_passwords.txt` nor one character repeated (`password_too_common`)
- **Job posts per client:** at most the `jobs.posts_per_client_per_day` setting from one address a day (`job_post_daily_limit`, 429), on top of the cap per phone
- **Sign-ups per client:** a sign-up says whether its email is taken, so one address (`client_ip::client_key`, hashed in memory) tries at most `SIGNUP_ATTEMPTS_PER_CLIENT` valid sign-ups in `SIGNUP_CLIENT_WINDOW_SECS` (`signup_daily_limit`, 429; a form refused for its own mistakes is not counted). A constant, never a setting
- **Contact requests per client:** one address (`client_ip::client_key`, hashed with the provider in memory) sends one provider at most `CONTACT_REQUESTS_PER_CLIENT_AND_PROVIDER` requests in `CONTACT_CLIENT_WINDOW_SECS` (`contact_daily_limit`, 429; the page form says it in the sheet): each emails the provider and counts as a lead. Per provider, never per address alone, since carriers share one address among many phones (CGNAT). A constant, never a setting
- **Password change:** `/account/password`, `PATCH /api/users/me/password` and the email change (`/account/email`, `POST /api/users/me/email`) are on the auth budget, and `LOGIN_FAIL_MAX_ATTEMPTS` wrong current passwords for one account, on either, stop the checks for `LOGIN_LOCK_MINUTES` (`too_many_requests`, before bcrypt), so a session in other hands cannot guess the password it does not have; a right one clears the count
- **Review signs:** `/admin/reviews` marks a review whose contact request came from the business's own phone, that was written within the `listings.review_suspect_minutes` setting of it, or whose request the provider marked answered within that window (`inquiries.answered_on`, the first time it was marked `contacted` or `closed`; `AdminReviewDto::suspect`, `quick_answer`), for the moderator to weigh before showing it
- **Database roles:** the schema belongs to `agri_owner`, which only runs the migrations at startup (`MIGRATION_DATABASE_URL`); the server answers as `agri_app`, rows only (`deploy/db-init/roles.psql`, tested by `it/database.rs` `the_app_role_cannot_change_the_schema`). Runtime code never changes the schema
- **CI:** every GitHub Action is pinned to a commit SHA, its tag in a comment. `.github/workflows/audit.yml` runs `cargo audit` and `npm audit` (islands, Mini App) on every change and weekly; an advisory is ignored only in `backend/.cargo/audit.toml`, with the reason (never compiled, or input that is not an attacker's). The Mini App's `overrides` lift zmp-ui's react-router and zmp-vite-plugin's vite to fixed releases (its README)
- **JWT secret:** at least 32 bytes; startup refuses a shorter one
- **Permissions:** granted to roles only (`role_permissions`); default grants applied once, so revocations stick. Only a role with `any_permission` (the admins) can be granted every key; the others take only the keys meant for them (`catalogue::grantable`, `permission_not_for_role`; the grid disables the rest): the provider role is every sign-up
- **Changing the account:** `/account` (every frame) changes the name (`update_profile`, no password), the email and the password, and signs out the other devices (`end_other_sessions`). The email moves only through the link sent to the new address (`request_email_change`: the current password, then `email_verification_tokens.new_email`; the old address keeps signing in and getting notices until it is opened); opening it confirms the new one, spends every other confirmation and reset link of the account in the same transaction, and tells the former address, when it was confirmed, the new one masked (`mask_email`). Whether the new address has an account is never said (nothing is sent; the link would be `email_taken`). The password change and the email change count wrong current passwords together (`reauth_key`), `LOGIN_FAIL_MAX_ATTEMPTS` in `LOGIN_LOCK_MINUTES`. An admin (`user.update`, never on their own account) corrects another's name and email the same way (`admin_update_user`: the move waits for the link) and has a reset link sent (`admin_send_password_reset`): an admin never sets or learns a password. Assigning a record to an existing account needs its email confirmed (`account_email_unconfirmed`): anyone may sign up with a stranger's address
- **Rate limiting:** per client address (behind `TRUSTED_PROXY_HOPS` proxies; an IPv6 client is its /64, `rate_limit::budget_key`): auth 10/min (the password change too), public writes 30/min, searches 240/min (`/search`, `/explore`, the JSON provider / service / product lists), signed-in writes 120/min, uploads 20/min, map tiles 600/min. Every request but an upload is given up on after `REQUEST_TIMEOUT_SECS` (`request_timed_out`)
- **Security headers:** `X-Content-Type-Options`, `X-Frame-Options`, `Content-Security-Policy`, `Strict-Transport-Security` (HTTPS only)
- **CORS:** configured via environment variable (`CORS_ORIGINS`). Those origins may also send non-GET requests, but only without a session cookie (`origin::Origins`: `own` is `APP_URL`, `api` the CORS list): the Zalo Mini App's host is shared by every Mini App, so a page there never acts as someone signed in here
- **Sign-in audit:** `AuthUseCase` publishes `Change::Auth` events for the account concerned (`login`, `login_failed`, `change_password`, `reset_password`, `refresh_token_reused`, `verify_email`, `request_email_change`, `change_email`, `update_profile`, `end_other_sessions`, `send_password_reset`; the last three also by an admin, audited as them), which the audit log stores like any admin action; a guess at an email nobody has is about no account and is not logged
- **Request ids:** a client's `x-request-id` is kept only when it is a plain id (letters, digits, `-`, `_`, at most `REQUEST_ID_MAX_LEN`); otherwise the server makes its own
- **CSV exports:** a cell that would start a formula (`=`, `+`, `-`, `@`) gets a leading apostrophe (`csv_field`)

---

## Page Route Map

Every page is a handler in `backend/crates/web/src/pages/`, routed in `web/src/routings/app_routing.rs`. Public and provider pages use `layouts/app.html`; admin pages use `layouts/admin.html`; the partner's report uses `layouts/partner.html` (brand back to the report, language, the account menu: no farmer tabs, search or footer). Pages every account shares stay in the visitor's frame (`shell::Area`, by permission): `/account` (`organisms/account_sections.html`) and the error page, which inside an area (`Area::covers`: `/admin`, `/setup/seed`, `/partner`, `/provider/`) keeps its frame and leads back to it, elsewhere is an app page with the session. The provider's tab lights on every `/provider/` page; on a phone, a wide-only tab's page lights its stand-in (`/providers` → services, `/prices` → home, `active_on_phone`). A service or product page goes back to its category's list; a public provider, service or product page shows its owner and an admin who may open it the way to edit it (`pages::manage`, `molecules/manage_link.html`). Admin pages link a record and its account both ways (`admin::user_href`; the user page to the business or partner it manages), and a record's own admin page offers its public page while it is shown.

### Auth

| Route | Description |
| --- | --- |
| `/login` | Login (`web/src/pages/auth.rs`); signed in → own area by permission; no admin yet → `/setup` |
| `/register` | Provider self-registration |
| `/forgot-password` | Request password reset email |
| `/reset-password` | Consume reset token, set new password |
| `/verify-email` | The emailed confirmation link: GET shows the button, POST confirms the address (`pages/auth.rs`) |
| `/setup` | First admin + optional demo data (`web/src/pages/setup.rs`); closed once an admin exists |
| `/setup/seed` | Demo data page: the demo data (load / remove) apart from the platform data it never touches (counts, each linked to its admin page) (`system.demo.manage`) |
| `/logout` | POST: ends the session family, clears both cookies (`web/src/session.rs`) |
| `/language` | POST only: the language choice (`lang` = vi / en, `back` = a local path, else home); `lang` cookie for a year, 303 back, so no language URL ever shows (`web/src/pages/language.rs`) |
| `/sector` | POST only: the visitor's sector (`crop` / `livestock` / `aquatic`, empty = every sector, `back`); `sector` cookie for a year, 303 back (`web/src/pages/sector.rs`) |

### App

| Route | Description |
| --- | --- |
| `/` | Home: alerts, two named rows of category tiles ("Thuê làm" services, "Mua vật tư" supplies), near-me banner, featured services, prices, featured supplies (`web/src/pages/home.rs`) |
| `/supplies` | Supplies ("vật tư") listing: GET filter form, page links, `commodity=` (the supplies for one cây trồng, vật nuôi and those for every one; the price board leads here) (`web/src/pages/products.rs`); the former `/products…` addresses 301 here (`pages::moved`) |
| `/supplies/[id]` | 301 → `/supplies/[id]/[slug]` |
| `/supplies/[id]/[slug]` | Supply detail + the seller's services + contact form about it (`return_to=product`); "Dùng cho" links the supplies for that commodity and, while it has a fresh price, its price board (`product.rs`; other spellings 301) |
| `/supplies/category/[id]/[slug]` | Supplies in a category (slug is the category slug, others 301) |
| `/services` | Service listing: one card per public service (photo, price, provider, call); GET filter form, page links (`web/src/pages/services.rs`) |
| `/services/category/[id]/[slug]` | Services in one category (category slug canonical) |
| `/services/[id]` | 301 → `/services/[id]/[slug]` |
| `/services/[id]/[slug]` | Service detail + the provider's other services + contact form with it chosen (`return_to=service`) (`web/src/pages/service.rs`) |
| `/search` | Services + products + shops for `q`, only the kinds with a result (no section, no tab for an empty kind; tabs only when two kinds have one; nothing at all: one message leading to every sector, a job post, near me); link tabs (`tab=`) work without JS; "all" previews each kind, a kind's tab is its whole list paged (`tab=`+`page=`); sector links (`sector=`) narrow the one search (`web/src/pages/search.rs`) |
| `/explore` | Services, products and shops near a position (`ac-geolocate`) or preset, tabs in that order (as `/search`), each list paged on its own (`tab=`+`page=`; sponsored on the first page); featured services before one (`explore.rs`) |
| `/jobs/new` | Post a job (`POST /jobs`): service, where (`ac-geolocate` fills hidden boxes, else a commune found by typing its name (Alpine `wardSearch`, choices from `/api/places/wards`, JS only), else an area preset), what, name, phone; the home page's "Chưa tìm được nhà cung cấp?" card leads here (`web/src/pages/jobs.rs`) |
| `/info`, `/info/[code]` | The information pages a visitor may open, by footer group; one page: the version in force (its day, an outline of its `##` headings, the text), the versions in force so far (`?version=` an earlier one, said no longer in force, `noindex`); in preview only to `feature.preview`, with the strip (`web/src/pages/info.rs`) |
| `/prices` | Farm prices, a section per commodity of the visitor's sector, latest per area with its move (in đồng, "so với" the earlier price's day: `previous_on`), each line's age in words ("Hôm nay", "3 ngày trước") and faded past `PRICE_STALE_DAYS`, each section with a link to the supplies for it, the move a ▲ / ▼ in the text colour (never green / red); under it the line's last `PRICE_TREND_DAYS` as a small server-drawn SVG line (`atoms/sparkline.html`, `PriceUseCase::trends`: the board's own query over a shorter window, only lines with `PRICE_TREND_MIN_POINTS`, worded for a screen reader); chips per commodity with a price (`?commodity=`, whatever the sector, as a chosen category; an id with no fresh price passed over), which a supply for that commodity links to (`prices::commodity_href`, only while it has a fresh price); sector switch (an empty board offers every sector); GET area filter (`autoSubmit`; it takes typing from `PRICE_AREA_SEARCH_MIN` areas, Tom Select loaded only then), the area remembered (`price_area` cookie: `?area=7` keeps it, `?area=` forgets it, none opens on it; one with no fresh price left is passed over) (`web/src/pages/prices.rs`); home shows `HOME_PRICES_SHOWN`, of the remembered area when it has some, its title then naming it |
| `/providers` | Provider directory (new sign-ups without a service too); found by name or by their services (`web/src/pages/directory.rs`) |
| `/providers/[id]` | 301 → `/providers/[id]/[slug]` |
| `/providers/[id]/[slug]` | Provider detail: services, products, shown reviews with replies + "write a review" sheet, contact form (`web/src/pages/provider.rs`) |
| `/account` | Its name (`/account/profile`), its sign-in email with its state and the "change email" sheet (`/account/email`: the current password; the address moves once the link is opened, `#email`), password change and "sign out the other devices" (`/account/sessions`; both keep this device, `#security`), logout, in the visitor's frame (admin panel, partner, app) (`web/src/pages/account.rs`, `organisms/account_sections.html`) |
| `/provider/dashboard` | Services + products (photo thumbs) + business profile with its location box (address, ward, GPS, map pin); while it has no position a reminder on top leads there (`web/src/pages/dashboard.rs`) |
| `/provider/supplies/new`, `/provider/supplies/[id]/edit` | Multipart supply form with photo (checked before anything is saved), delete (confirm) |
| `/provider/services/new`, `/provider/services/[id]/edit` | Multipart service form: category cards, optional photo, delete (confirm) (`own.service.*`) (`web/src/pages/own_services.rs`) |
| `/provider/inquiries` | Farmers' contact requests to this business: call button, the status's next steps per card (the admin list's buttons, spam confirmed) (`own.inquiry.manage`) (`web/src/pages/inbox.rs`) |
| `/provider/jobs` | Job posts near the business in its services' categories, distance and call button (`own.job.read`) (`web/src/pages/own_jobs.rs`) |
| `/provider/reviews` | Reviews of this business (shown or waiting), a public answer per card (`own.review.reply`) (`web/src/pages/own_reviews.rs`) |
| `/partner` | A partner account's home: the demand report of its contracts in force (areas × categories), its contracts; `/partner/report.csv` (`partner.report.read`) (`web/src/pages/partner.rs`) |

### Admin

| Route | Description |
| --- | --- |
| `/admin` | 303 → `/admin/dashboard` (anonymous → `/login`) |
| `/admin/dashboard` | Stats + demo-data warning (`web/src/pages/admin/`; frame `layouts/admin.html`, menu filtered by permission) |
| `/admin/users` | User list, paged (`web/src/pages/admin/users.rs`) |
| `/admin/users/[id]` | Name and email (`/profile`, reason; a new email waits for its link), role change, deactivate (confirm + reason) / reactivate, a password-reset link (menu, confirm + reason, `/password-reset`), effective permissions; never its own name, email or reset (`user.update`) |
| `/admin/roles` | Two tabs (`molecules/admin_tabs.html`): roles with grant counts; `?tab=permissions` every permission × role, read-only (`/admin/permissions` 301s there); one menu entry "Vai trò & quyền" |
| `/admin/roles/[id]` | Editable permission grid, `POST …/permissions` (`role.permission.manage`; cannot revoke it from your own role; audited) |
| `/admin/sectors`, `/admin/sectors/[id]` | Ngành: name, its code (shown, never edited), icon, order (row: hide (confirm + reason) / show, open) + create sheet; edit, hide / show; the first tab of the classification screen, where its menu entry leads (`category.manage`) (`web/src/pages/admin/sectors.rs`) |
| `/admin/commodities`, `/admin/commodities/[id]` | Cây trồng, vật nuôi: name, sector, icon, order (row: hide (confirm + reason) / show, open) + create sheet; edit, hide / show; its page gathers what hangs on it: its crops (each to its page) with an add-crop sheet, the commodity chosen, the supplies for it (`/admin/supplies?commodity_id=`), the public board (`category.manage`) (`web/src/pages/admin/commodities.rs`) |
| `/admin/categories` | The classification screen's last two tabs (`?type=` `service` (service types) / `product` (supply types)); the tab's list (row: hide (confirm + reason) / show, open) + create sheet (`web/src/pages/admin/categories.rs`) |
| `/admin/categories/[id]` | Edit; hide (confirm + reason) / show; its header leads to what is filed under it (a supply type: `/admin/supplies?category_id=`; a service type: its public list) |
| `/admin/locations` | Location presets (row: hide (confirm + reason) / show, delete (confirm), open) + create sheet: choosing a ward fills the name ("Bình Phú, Đồng Tháp") and point (Alpine `wardFill`; without JS the blank boxes take the ward's, `AdminUseCase::create_location`); an "add several" sheet: a province (`?bulk=p82`, a GET form sent on change), its wards ticked (`organisms/ward_checklist.html`, Alpine `wardChecklist`: filter as typed, tick every one shown, the count on the button), one a preset already names said so and not offered; POST `/admin/locations/bulk` (`AdminUseCase::create_locations_from_wards`, `ward_preset` the rule both share, one `INSERT … ON CONFLICT (label) DO NOTHING`), back on the list newest first (`web/src/pages/admin/locations.rs`) |
| `/admin/locations/[id]` | Edit (the same `organisms/location_fields.html` as the create sheet, ward picker included; the same `AdminUseCase` rule), hide (confirm + reason) / show, delete (confirm + reason) → list |
| `/admin/providers` | Provider list, paged + create sheet (`provider.manage`: a listing from name + phone, no account; the location box: ward, GPS, map pin) (`web/src/pages/admin/providers.rs`) |
| `/admin/providers/[id]` | A record page (`provider_controls`: verify, turn on and the public page as buttons; the account (assign / change, remove), unverify and turn off in the menu; the strip says who turned it off, verified it, manages it), its parts as tabs (`ProviderTab`: overview, offers, profile, history). A summary first: its states as badges (on, verified, account, demo), phone with call and Zalo links, its area (`AdminProviderDetailDto.area`, the ward of its position, else "no position: not on near-me") with a map link (`MAP_POINT_URL`), and its activity as the dashboard's cards (`molecules/stat_card.html`: taps over `CALL_STATS_DAYS`, contact requests waiting / all but spam, shown reviews with their average and the waiting ones; each to its list, each only with the permission to read it: `AdminUseCase::provider_activity`). Then one "status" card (turn off (confirm + reason) / on, verify / unverify (confirm + reason), assign or change the managing account, remove it (confirm + reason; the account stays; an unconfirmed email is said, since notices reach only a confirmed one)), its services and products (thumbs, count against the `listings.*` cap, "add" straight to the create sheet of the list narrowed to it), and the profile form last (the owner's rules: `business_rules::validate_profile`; `organisms/business_fields.html`) |
| `/admin/supplies` | Supplies (filters `provider_id`, `commodity_id`, `category_id`: `AdminProductFilter`, each one bar of `molecules/admin_filter.html`; each row its supply type (linked to `?category_id=`) and its sector (`AdminProductDto.sector`: `sql::supply_sector`, the select-side twin of `supply_in_sector`); each row says what it is for; `/admin/products` 301s here): hide (confirm + reason) / show, delete (confirm + reason); create sheet only for one provider (`?provider_id=`), as services (`web/src/pages/admin/products.rs`) |
| `/admin/supplies/[id]`, `/admin/services/[id]` | A record page (`listing_record::listing_controls`: hide / show and the public page as buttons; view or remove the photo and delete in the menu). Whose it is on top (`listing_facts::provider_line`: the provider to its page, its phone, its badge, "off" while it is). Edit (multipart: the boxes and `molecules/photo_picker.html`, the photo checked and uploaded before anything is saved, shown once the boxes are: `MediaUseCase::stage_*_image_admin`, `commit_staged`), remove the photo (confirm + reason, `…/image/delete`, `Change::ImageRemoved`), hide / show, delete (confirm + reason). The status line is whether farmers see it (`is_public`: shown, its provider on, its type shown, the admin read of `live_public`), and `molecules/offstage_note.html` says what else holds it back with the link to fix it; the public-page link only while public. Above the form, its sector, what it is for (supplies), when added and changed, and whether its provider listed it (`molecules/listing_facts.html`; editing a provider's own listing says the change shows in its history); under its state, its taps and contact requests (`AdminUseCase::listing_activity`, `ListingRef`). Opened from a list, the list's position rides on its URL and every form and redirect of the page (`ListPosition::record_href` / `to_record`), so "back" and a deletion land on that list page; opened bare, on the provider's list |
| `/admin/services` | Services (each row its service type and that type's sector, "Mọi ngành" for none; filter `provider_id`; row: hide / show, delete, open; create sheet only for one provider; an admin's service counts against `listings.services_per_business` like the owner's) |
| `/admin/inquiries` | Status as a badge, the row's next steps as `row_action` buttons (`pages::status_steps`: contacted, done, reopen; spam confirm + reason, `?spam=`); status chips (`?state=`, `molecules/status_chips.html`); filter `provider_id` |
| `/admin/reviews` | Hide (confirm + reason) / show; filter `provider_id` |
| `/admin/demand` | Demand report: category × week for one area (`?area=` `p82` a province, `w28439` a ward, or `none`) or all, by channel, searches, unmet searches, steady areas; `/admin/demand.csv` (`report.demand.read`) (`web/src/pages/admin/demand.rs`) |
| `/admin/partners` | Partners, paying ones and monthly revenue by province (at most 34 rows, each a `<details>` of its contract areas linking to their demand report; `fees_in_force`, the contracts in force without what they cover) + create sheet (`partner.manage`) (`web/src/pages/admin/partners.rs`) |
| `/admin/jobs` | Job posts, state as a badge (`atoms/job_status.html`), next steps as buttons like the contact requests (done, reopen; spam confirm + reason), state chips (`?state=`) (`job.moderate`) (`web/src/pages/admin/jobs.rs`) |
| `/admin/settings` | The platform settings, one tab per group (`?tab=` brand (the default: one form beside a live preview, Alpine `brandPreview`: the 360 px top bar at its real sizes, the sign-in card, the browser tab, the home screen on a dark wallpaper, the Google result; an empty box shows its default as its placeholder; `field::input` `counter` counts its characters, Alpine `charCount`), support, features (sign-ups, then each feature: on / preview / off as three radio buttons with where it shows and, in preview, the way to try its page), jobs, listings, market; `molecules/admin_tabs.html`, each tab counting the settings off their defaults): the group as one `admin-section` card (status badge, `form-stack`, an optional reason, save), then, while one is off its default, a "back to defaults" card listing "now, default"; every box says its default and bounds; saved, back on its tab with a flash (`setting.manage`, menu "Hệ thống") (`web/src/pages/admin/settings.rs`) |
| `/admin/prices` | The price board, one menu entry, two tabs: the prices entered, paged, each row linking to its crop and area, `?crop_id=` one crop's (row: delete, confirm + reason) + "enter a price" sheet (the crop chosen when filtered); `?tab=crops` the produce followed (`crops::list_page`) (`price.manage`) (`web/src/pages/admin/prices.rs`) |
| `/admin/crops/[id]` | A produce item, of a commodity (linked): its recent prices, "enter today's price" (the price list with it chosen), every price of it; `/admin/crops` 301s to `/admin/prices?tab=crops`; the crop pages light the price board's entry (`MENU_OWNERS`). Each (row: hide (confirm + reason) / show, open) + create sheet; edit, hide / show (`web/src/pages/admin/crops.rs`) |
| `/admin/info-pages`, `/admin/info-pages/[id]` | The information pages, a whole list + create sheet (off, no text); a page's record page: turn on (one button) / admins only / off (confirm + reason), its text as a new version (Markdown, a day not before today; "Xem trước" posts the same form and saves nothing), its footer place and icon, its versions, its history (`content.page.manage`) (`web/src/pages/admin/info_pages.rs`) |
| `/admin/alerts`, `/admin/alerts/[id]` | Two tabs: the alerts (row: turn off (confirm + reason) / on, open) + create sheet, each row with its kind's name; `?tab=kinds` the alert kinds (`alert_kinds.rs`: create sheet POST `/admin/alert-kinds`, a kind's page `/admin/alert-kinds/[id]` lighting the alerts entry, `/admin/alert-kinds` 301s to the tab); an alert's page: edit, turn off / on (`alert.manage`) (`web/src/pages/admin/alerts.rs`) |
| `/admin/partners/[id]` | Edit, turn off (confirm + reason) / on, report account (sheet), contracts (one line each: area, in force, dates, fee, how many categories and sponsored shops, to its own page; paged, `?tab=contracts&page=`), the add-contract form (its ward and category choices loaded on that tab only) at the end (on the page, not a sheet: long form)
| `/admin/partners/[id]/contracts/[cid]` | A contract's record page (`web/src/pages/admin/contracts.rs`): its partner as the way back, delete in the menu (confirm + reason, lands on the partner's contracts); two parts (`?tab=`): its sponsored shops (remove: confirm + reason) and the search of the shops of its area to add (`?q=`, a GET form: the first `SPONSOR_CANDIDATES_SHOWN` by name, picked by the database, `PartnerRepositoryTrait::sponsor_candidates`; adding keeps the search), and its terms (dates, fee, categories; the area stays). Each part loads only what it shows |
| `/admin/logout` | POST: same as `/logout`, lands on `/login` |

---

## URL Names

Hardcoded URL paths are banned in templates and pages. Every route has a name in **`backend/crates/web/src/routes.rs`** (`ROUTES`); templates link with `url_for(name="…", …)` and Rust code with `routes::url(name, params)`, which percent-encodes every parameter so a value cannot escape its segment. An unknown name or a missing parameter is an error when the page renders, never a broken link.

```
url_for(name="product", id=p.id, slug=p.name | slugify)   → /supplies/{id}/{slug}
url_for(name="admin_user", id=u.id)                        → /admin/users/{id}
routes::url("provider_product_edit", &[("id", "7")])       → /provider/supplies/7/edit
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

Grow the rest only once one area sustains real leads week after week (the admin dashboard's weekly table). An area is a location preset ("Cai Lậy, Đồng Tháp") and the radius around it, not an administrative unit: there are no districts since July 2025.

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
- `build_state` takes a `Wiring` (the adapters and settings) and builds the exact production graph; tests pass the same adapters except bcrypt at `BcryptPasswordHasher::MIN_COST` and one `Views` shared by the whole test process (`integration_tests::views`).
- Scale (what keeps a page's cost from growing with the tables). Decide an index from numbers, never from a guess, since an index is paid on every write: `pg_stat_statements` in production (`deploy/README.md`), and `it/scale_probe.rs`, which times the main reads against 20 000 providers, 60 000 services and a million taps (`SCALE_PROBE=1`; it does nothing otherwise). Its last run: every public read under 0.6 s (the slowest are text searches), the demand report 1 to 3.5 s.
  - **No server-side cache.** Every read goes to the database: a copy kept in memory has to be cleared by every write that could change it, and one write forgotten is a page showing stale data. A read that is slow is made cheaper (the points below); what a browser may keep (static files, images) is said with `Cache-Control`. Nothing measured so far needs more.
  - **The database counts, Rust never does.** A report reads rows already grouped by what it shows (`DemandRepository::lead_counts`: by week, ward and category, the business only where a lead names no category; `search_counts`; weeks with `week_since`), never by business or by tap: grouped by business a million taps came back as 800 000 rows. A board only the rows it shows (`latest_public_prices`: the newest two per crop and area). Never load a window of rows to count or pick from them in Rust.
  - **What is derived from a position is stored with it.** `businesses.ward_code` is written by the same statement as `location` (`ward_code_at`; the profile update, the demo insert) and refilled for every business once per dataset version (`AdminUnitRepository::sync`, `catalogue_state` key `business_wards.<version>`). A new way to write a business's position must set it too.
  - **Public lists** (`/services`, `/providers`, `/supplies` and their JSON) end at `PUBLIC_MAX_PAGE` (`paginate_public`, `public_total_pages`), admin lists at `ADMIN_MAX_PAGE` (`paginate_admin`, `admin_total_pages`; past it the admin narrows with the search box or filter). Every paged list reads its page first and counts its rows only when that page is full (`pagination::total_or_count`), so a repository has a list method and a count method, never one returning both; a use case answers `PagedResponse::public` / `::admin`. Every list that can grow is paged, one way: `?page=`, `listing::Pagination` and `molecules/pagination_nav.html` under it, `past_the_end` to the last page, a row's form sending its page back (admin lists, the alerts and partners included; the provider's inquiries, reviews and jobs). The admin's commodities, service and supply types (each tab paged on its own, `?type=` kept) and area presets are paged like the rest (`list_page` + `count`; the JSON `/api/admin/commodities`, `/categories`, `/locations` still answer the whole list). Only the shortest catalogues (sectors, alert kinds, crops, roles) are whole lists. A page of several lists (`/search`, `/explore`) pages each on its own: `page` belongs to the chosen tab (`listing::tab_page`), each list has its own page links carrying its `tab`, and `/search`'s "all" tab previews each kind with "Xem thêm" to its tab (its tabs load, `tab_nav` `in_place=false`). The JSON near-me lists (`/api/explore/providers`, `/services` with a position) still answer the nearest `GEO_RESULTS_MAX`.
  - **What a business lists is capped**: the `listings.services_per_business` services and `listings.products_per_business` supplies settings (never past `SERVICES_PER_BUSINESS_MAX` / `PRODUCTS_PER_BUSINESS_MAX`; `create_capped` with `repositories::lock_business`; `service_limit_reached`, `product_limit_reached`), so an owner's page shows its whole list at once.
  - **Text search across two tables is two subqueries**, each over one table's folded indexes (`search::service_listing_matches`, `admin_service_search`, `admin_product_search`, the directory's `matches_text`), never one OR over the joined rows. A public search takes at least `SEARCH_QUERY_MIN` characters (`validation::search_text`, `search_too_short`; the boxes carry it as `minlength` from `limits.search_min`): one letter is in nearly every name and no index answers it.
  - **Taps and searches are kept as rows for `EVENT_KEEP_WEEKS`**; older ones are counted into `call_click_weeks` / `search_event_weeks` and deleted (`DemandReportUseCase::roll_up_old_events`, daily). Nothing may read a tap or a search older than that. Job posts older than `JOB_POST_KEEP_DAYS` are deleted by the same daily task, whatever their status (`JobUseCase::purge_old_posts`).
  - **The pool**: `DB_MAX_CONNECTIONS`, a statement cancelled after `DB_STATEMENT_TIMEOUT_SECS`, a request failing after `DB_ACQUIRE_TIMEOUT_SECS` without a connection; migrations run on their own connection without the timeout. The repeat filters and rate limiters are pruned by one background task (`RATE_LIMIT_PRUNE_EVERY_SECS`), never on the request.
- Integration tests are one binary (`crates/tests/tests/it/`, each file a module of `main.rs`; a convention test fails on a file elsewhere or undeclared). Third-party crates build at `opt-level = 2` in dev and every crate with line tables only (`[profile.dev]`); `CARGO_PROFILE_DEV_DEBUG=full` for a debugger session.
- Migrations are DDL only and append-only from the baseline (`m20261120_000001_baseline`, the whole schema; nothing before it ever ran in production; then `m20261121_000002_email_verification`, `m20261122_000003_scale`, `m20261123_000004_settings`, `m20261124_000005_username_no_at`, `m20261125_000006_user_demo`, `m20261126_000007_inquiry_answered_on`, `m20261127_000008_account_email`, `m20261128_000009_user_reference_indexes`: the account columns removing the demo accounts sets to NULL, `m20261129_000010_info_pages`): never edit or delete one that has run anywhere. Platform data and demo data never mix. Platform data is only what the system runs on: roles and permissions (`application/src/catalogue.rs`, seeded idempotently at startup) and provinces and wards (the bundled dataset), the base an admin picks the supported areas from. Sectors, commodities, service and supply types, location presets and crops are demo data like the demo businesses (`infrastructure/src/demo/data.json`, `is_demo`); startup seeds none, and an admin adds the real ones. Demo data only through `/setup/seed` (`DemoDataRepositoryTrait`, `demo/data.json`, `demo_activity.rs`): a set that agrees with itself for testing: the catalogue, businesses with services and products, and around them inquiries, reviews (only from an answered request's phone), call and Zalo taps and searches by report week (counted back from the day of loading with `report_weeks`), prices, job posts, alerts, partners with contracts over the ward or province a demo shop lies in (`ward_code_at`) and the shops of that area they sponsor. The demo data at volume (`agri-connect demo-data [--providers N] [--seed S]`, default 5 000 providers, development only: refused with `APP_ENV=prod`) is the same file grown by `infrastructure/src/demo_volume.rs`, a pure, seeded generator over the words of `demo/vocabulary.json` (no Vietnamese in Rust) and the wards of the Mekong Delta provinces, written by the same loader in one transaction (`DemoDataRepositoryTrait::load_volume`, `SetupUseCase::load_demo_volume`): about 5 000 of each kind (accounts, services, supplies, reviews from 10 000 contact requests, job posts, alerts, prices) and 200 partners, busier wards getting more shops and leads; its provider and partner accounts carry `users.is_demo`, sign in with `DEMO_ACCOUNT_PASSWORD`, and go when the demo data is removed. Unit tests check every reference and the forms' rules on the file and on a generated sample (`datasets`); `it/demo_data.rs` reads it back through the reports, KPIs, price board and reviews. Loading reuses a commodity, type, area or crop already there (same name, slug, label, name); removing deletes the demo businesses (services, products, their photos, inquiries, reviews, call taps by cascade), the demo partners (contracts, sponsored shops), prices, job posts and searches, then every demo crop, type, commodity and area no real row uses (services, products, job posts, partner contracts, crops, prices), and the ones real data uses stay, unflagged, as real. An admin's price for a demo price's day makes it real. A demo job post is shown only to a demo business and never counts against a farmer's phone. Admin lists mark demo rows (`status_badge.html` `demo`). A new kind of demo row must belong to a demo business or carry its own `is_demo`. Tests start from the demo catalogue (`Env`, `DemoDataRepository::load_catalogue`).
- Keep internal logs in English.
- All location data uses **SRID 4326** (WGS84). Never mix geometry and geography types in the same query.
- Phone number pattern for Vietnamese mobile: `0\d{9}` (10 digits starting with 0).
- Radius validation bounds: 100 m minimum, 50 000 m maximum.
- `/services` lists services, not providers; a service is public when it is shown, not deleted, in a shown category and its business is live (`live_public` in the service repository, the one place that says so). Searching a service matches its name, description and the business's name; searching a provider matches its name, description and its live services' text.
- Public provider lists (`/providers`, near-me shops tab, search's shops tab) include providers with no service yet, so a new sign-up can be found; a service filter (`category_id`) matches only a live service in a live category. Featured services (home, near-me before a position): one per verified provider, those with a photo first, providers rotated by day, every category once before a repeat (`pick_featured`). Service cards are compact rows at every width (a fixed 104/120 px square photo beside the text, one call button), and fewer than three keep the two-column width (`service-grid--1/--2`), never one card across a wide screen. The default service order and the near-me services take turns by provider (`with_provider_turn`: each provider's first service before anyone's second); an explicit sort is kept as chosen. The wide top bar has a "Nhà cung cấp" tab (`wide_only`); on a phone the directory is the second of the `/services` tabs. The home page opens with two named rows of category tiles, one per errand: "Thuê làm" (services, green squares) then "Mua vật tư" (supplies, amber circles), `HOME_CATEGORY_TILES` of each (one fewer on a phone, so each row is four), each ending in an outlined "Tất cả" tile to its whole list; each row is its own section (`home-errand`), stacked on a phone, side by side from 992 px, every label two lines tall (clamped, the whole name in `title`); near-me has no tile (its tab and the banner cover it). The "Tài khoản" tab is `phone_only`: on a wide screen the top bar's account button covers it. `/services` and `/providers` open with "Dịch vụ | Nhà cung cấp" tabs (`services::view_tabs`: both read `ListQuery`, so a switch keeps the search, category and sort); their near-me banner comes after the third card (`near_me_href`, category and tab kept), and an empty search offers the other list as its second action.
- Providers manage their own services (`own.service.create` / `update` / `delete`). Every owner-scoped query matches the owner in the same statement (an `in_subquery` on the business), the business comes from the session, never the request, and someone else's service answers `service_not_found`. `is_active` is the admin's moderation switch: an owner's edit never changes it. A business lists at most the `listings.services_per_business` setting's services (hidden ones count; the business row is locked while counting). Admin and owner forms share `usecases::service_rules::validate_service` (a blank name takes the category's); likewise the business profile: `usecases::business_rules::validate_profile` (owner dashboard, admin create / edit), one form `organisms/business_fields.html`.
- Reviews (`ReviewUseCase`): a farmer reviews a provider only from the phone of a contact request that provider answered, once (the UNIQUE on `reviewer_phone_hash`, `review_already_sent` from the repository); a review that breaks either is quietly not stored, and the answer is the same as for a stored one (`ReviewSentDto`, no id), so nobody typing a number learns whom it contacted or reviewed (the sheet states the rule beforehand); the phone is kept only as `hashing::sha256_hex` (the one SHA-256 helper) and never shown. A new review is hidden; the admin's "show" on `/admin/reviews` is the approval ("Chờ duyệt" until then). The owner's answer (`reply_own_review`, owner-scoped in one statement) shows under the review once it is shown; a blank answer removes it. Stars render through `atoms/stars.html` everywhere. A provider's rating follows it: service and provider cards (`cards::rated_services` / `rated_providers`, one `ratings` query per list) and the service and product pages (`molecules/rating_line.html`, linking to its reviews, `REVIEWS_ANCHOR`). After a contact request the page says a review comes once the provider called back, and the inbox offers each answered request the review link to send (`share_button` with `url`).
- Administrative units (`provinces`, `wards`): the bundled dataset `infrastructure/data/admin_units/` (official codes, a point in each unit, each ward's boundary simplified as one coverage: see its README) is loaded at startup by `SetupUseCase::seed_admin_units` when its version is not the one in `catalogue_state`; a unit it no longer lists is turned off, never deleted. No hamlets (thôn, ấp): no open national list. The ward of a point is `admin_unit_repository::ward_code_at`, the one definition: the ward that `ST_Covers` it, else the nearest within `ADMIN_UNIT_SNAP_M` by the index's `<->` (never `ST_Distance` on a boundary: seconds). An area travels as text: `p82` (province), `w28439` (ward), `none` (`application::admin_units::AreaKey`); `AdminUnits` names them ("Xã Bình Phú, Đồng Tháp"). Location presets stay what the near-me page, crop prices and job posts choose from. Tests that place something in a ward call `load_admin_units`.
- The demand report (`DemandReportUseCase`) counts leads (call and Zalo taps, inquiries without spam) over the `DEMAND_WEEKS` whole weeks before this one, Monday to Sunday in Vietnam (`VN_UTC_OFFSET_SECS`, the one offset). A lead counts under the ward its business lies in and that ward's province (else "no area"); the area list shows provinces in code order, each followed by its wards with leads; steady areas count the top-level rows. A tap on a shop with one live service takes that service's category. Any figure above zero and under `DEMAND_REPORT_MIN_COUNT` is hidden, and so is a total that would give exactly one hidden figure away; every view of the figures (page, CSV, JSON, a partner's) goes through the same `aggregate`; a sum hidden only for that is never shown as "fewer than": it carries `Figure.at_least` (its shown parts plus one, "≥ n"), which anyone can add up from the page. An area's line (`organisms/area_progress.html`: its average a week against the goal, its weeks as squares filled at the goal, its total; each links to its `?area=` report) is exactly the sum row of its own report (`category_rows`), so the list never shows what that report hides. The dashboard lists the `DASHBOARD_NEAR_AREAS` areas with the most weeks at the bar (`weeks_at_bar`). Steady areas: every report week at the `market.steady_weekly_leads` setting. Which rows count is the report's `DemoScope`: the admin's report (page, CSV, JSON, dashboard table) counts demo data too while it is loaded and says so (`with_demo`); a real partner's never does; a demo partner's counts demo data only (its sponsored shops likewise). Searches (`record_search`) are stored in the background with no identity: words (lowercased, `SEARCH_QUERY_MAX`), category, ward (of the preset's point or the position), result count; a page never waits for them. The JSON near-me lists (`/api/explore/services`, `/providers` with a position: the Zalo Mini App's) are recorded like the near-me page (`explore_handler::record_near`), the two lists of one place one search. A signed-in visitor's are skipped, and the same search by the same visitor within `SEARCH_REPEAT_SECS` counts once (`pages::visitor`: the address is hashed in memory only, never stored).
- Partners (`PartnerUseCase`): a contract covers a whole province or one ward of it (`ContractArea`; the area stays when it is edited), and is in force between its dates (both included, Vietnam days) while its partner is on; the revenue figures are the monthly fees of contracts in force today. A partner account (role `partner`) reads only what its contracts in force grant: `ReportScope.grants`, one `ReportGrant` (area, categories) per contract, each lead checked against one grant (`AreaFilter::covers`), never the areas of one contract with the categories of another. Sponsored shops must lie in the contract's area; on `/explore` (whose position's ward is looked up only when a contract is in force) they are moved up under "Tài trợ" (`feature_sponsored`: one service per shop, at most the `market.sponsored_shown` setting), never hidden, never shown twice, counted by the same call beacon. "Assign an account" forms (provider, partner) share `account_rules::account_for_role`.
- The catalogue has three separate parts, each its own table, never one shared "category" table: commodities (`commodities`, "cây trồng, vật nuôi", `CommodityUseCase`, `/admin/commodities`: what a farmer raises, each in one sector), service types (`service_types`) and supply types (`supply_types`) (the code calls either a category: `CategoryRepository` picks the table by `category_type`, `CATEGORY_TYPE_SERVICE` / `CATEGORY_TYPE_PRODUCT`; slugs are unique across both). A supply may say which commodity it is for (`products.commodity_id`, optional: NPK is for every crop); the supply list filters by it (`commodity=`, those for every one included) and the price board leads there.
- Farm prices (`PriceUseCase`): a crop ("nông sản") is a variety or grade of one commodity; hiding a commodity takes its crops off the board. The admin (`price.manage`) enters a low–high price in đồng per unit for a shown crop in a shown area on a day not after today (Vietnam), at most `PRICE_PER_UNIT_MAX`; the same crop, area and day again replaces it (UNIQUE, upsert). The board (`build_board`, one place) shows, commodity by commodity in their order (those of the visitor's sector when one is chosen), each shown crop's latest price per area only within its commodity's `price_fresh_days` (1 to `PRICE_FRESH_DAYS_MAX`, a new one `PRICE_FRESH_DAYS_DEFAULT`), with the move of its middle since the price before (`PRICE_HISTORY_DAYS` back); a hidden crop or area leaves it. The tab is `wide_only`; on a phone the home strip (`molecules/price_line.html`) leads there.
- Sectors and alert kinds are data, never a list in code: two coded lists (`sectors`, `alert_kinds`: the same columns, `coded_list` in the baseline), one use case configured per list (`CodedUseCase` with `SECTOR_LIST` / `ALERT_KIND_LIST`: permission, audit entity, errors), one repository macro (`coded_repository.rs`), one JSON handler macro (`coded_handler.rs`) and one set of admin pages (`pages::admin::coded`, `CodedPages`: routes and locale keys per list; `coded_list.html`, `coded_entry.html`, `organisms/coded_fields.html`). Sectors (`/admin/sectors`): the demo data has crops, livestock and aquaculture (`crop`, `livestock`, `aquatic`), an admin adds the real ones. A sector's `code` is the slug of its name when it is added and never changes; commodities, service and supply types and alerts point to it (foreign keys). The public filters, the switch and the sector cookie take only a shown sector (`SectorUseCase::chosen`, `invalid_sector` otherwise); the admin forms take a hidden one too (`validation::sector` over the stored list); a row is named by its sector's stored name (`labels::sector_name`, from `SectorUseCase::named`). With no sector shown there is no switch. Every commodity is in one; a service or supply type may be (`sector`; none = every sector, e.g. machine repair); `/prices` has one section per commodity with a price. The visitor picks a sector with `molecules/sector_switch.html` (`/services`, `/providers`, `/supplies`, `/prices`; POST `/sector`, cookie, `shell.sector`; never on the home page nor `/explore`, which always show every sector: a farmer picks an errand there, not a classification, and a page follows the cookie only where its switch shows, so a choice never narrows a page unseen). `/search` ignores the cookie too; over its results `sector_switch::sector_links` are links (`sector=`, keeping the words and the tab) that narrow that one search and are never remembered, and only the search of every sector is recorded for the demand report. A list with words typed shows the same links in place of the switch (`listing::typed_sector`, `sector_links`; the filter form, page links and the search page's "Xem thêm" keep `sector=`); without words `sector=` is nothing: the list chips and the lists show that sector's categories and those of every sector (`listing::in_sector` / `serves` for what a page picks from, `sql::category_in_sector` for services and `sql::supply_in_sector` for supplies in queries: a supply for a commodity is in that commodity's sector, one for none in its type's, the one definition); a chosen category or a typed search is never narrowed by the sector (`listing::list_sector`, the one rule, pages and JSON API alike: `sector=` on `/api/explore/services`, `/providers`, `/featured`, `/alerts`). The JSON API's featured services, providers and products follow `sector=` too; the directory (`/providers`) keeps the providers serving the sector and those with nothing listed yet. The admin's demand report names each row's sector (page, CSV); the contract form groups its categories by sector, service and supply types (filed in their two tables).
- Information pages (`InfoPageUseCase`, `content.page.manage`): the footer lists the shown ones the admin put there, by `footer_group` (`about`, `policy`; `Shell::info`, read with the settings on each page), with the way to `/info` (`Shell::info_hub`), and under the copyright the legal entity (`legal.*` settings, the "Pháp nhân" tab: company, tax code, address, representative, registration, the Ministry of Industry and Trade notice link; `Shell::legal`, each line only once set, never made up). Each page on, in preview (only `feature.preview`) or off by its own `stage` (`features::Stage`, never a setting), shown only once a version of its text is in force; versions are rows of `info_page_revisions`, never edited (a correction is a new version), the one in force the latest whose `effective_on` (Vietnam day, never before today) has come. Text is Markdown, rendered only by the `MarkdownRenderer` port (`infrastructure::markdown_renderer`, the one file naming pulldown-cmark: typed HTML becomes text, links only `https:` / `http:` / `mailto:` / `tel:` / this site / `#`, images only `/files/…`, headings anchored), which templates call as `markdown(text=…)`, the one output never escaped (no `| safe`), and `outline(text=…)` for the headings. `/info*` is `ALWAYS_OPEN` in the gate: each page is gated by its own row. Nothing is deleted: a page is turned off.
- Alerts (`AlertUseCase`, `alert.manage`): of one of the alert kinds (a disease outbreak, salt water, a storm in the demo data; the alerts page's second tab), each named and drawn by its kind (`Alert.kind_name` / `kind_icon`, read by the repository; pages and the JSON API alike), for one sector or all and one province or the whole region, between two days (Vietnam); the home page shows the ones in force, every sector's, first (`molecules/alert_banner.html`, at most `ALERTS_SHOWN`, newest first), as the Zalo Mini App's home does; the JSON API (`/api/explore/alerts`) takes `sector=` for one sector's. Demo alerts carry their own `is_demo` (over a demo shop's province). Turned off (confirm + reason, audited), never deleted.
- Settings (`application::settings`, `SettingsUseCase`, `setting.manage`): what an admin tunes without a deploy is a registry in code (key, group, kind `Number { default, min, max }` / `Switch` / `Stage` / `Text(Phone | HttpsUrl | Email)`), its defaults the former constants (`*_DEFAULT`; the `*_MAX` beside them are the hard bounds). Storage keeps only the values moved off their defaults (`platform_settings`; one back on its default drops its row), so a new key needs no migration; a stored row no longer valid, or of a key no longer listed, is skipped with a warning. Use cases and pages read them through `SettingsReader` on each use (one query over a tiny table; no copy in memory, per the no-cache rule; a request that may draw a page reads them once, `middleware::page_settings`, which the frame and `views::render` share, and an unreadable read falls back on the defaults), never a constant: the platform's name, description, logo and browser icon (`brand.*`; the name and description one line each: `TextRule::Line`; empty: `SITE_NAME` / `SITE_DESCRIPTION`, `SiteConfig::branded`; every frame, the error page included, and every email), the help lines (`support.*`, `Shell::support`), sign-ups (`access.registration_open`: `registration_closed`; the register link and the invitation hidden) and each feature's stage (`features.*`, below), the job limits (`jobs.*`), the listing caps and the suspect-review window (`listings.*`), the steady-area bar and the sponsored count (`market.*`). Each save is audited as one `change_settings` row ("key: before → after; …: reason"). Never a setting: secrets, infrastructure, security limits (rate limits, lockout, passwords), the demand report's privacy floor (`DEMAND_REPORT_MIN_COUNT`), retention (`EVENT_KEEP_WEEKS`, `JOB_POST_KEEP_DAYS`) and the field lengths: an admin account taken over must not be able to loosen them.
- Features (`application::features`, `web::features`, the settings' "Tính năng" tab, `setting.manage`): the parts of the platform an admin opens to farmers when they are ready: prices, near me, supplies, the provider directory (the list only; a provider's page stays), reviews, hire requests (its key stays `access.job_posts_open`, a former switch read as a stage). Each a `Stage` setting: on, preview (only an account with `feature.preview`, the admins, sees it: a strip under the top bar says farmers do not, `X-Robots-Tag: noindex`) or off (its pages answer `route_not_found` like a page that does not exist). One gate (`features::gate`, a layer inside the error renderer) owns the addresses: `RULES` lists each feature's pages, former addresses, JSON routes and the provider's own pages (the JSON near-me and directory lists by their query); a test fails on a public route given to no feature and not listed `ALWAYS_OPEN`. The admin's pages of a feature are never gated, so its data is prepared first. The ways in are hidden by whoever draws them, from `Shell::features` (templates `shell.features.<code>`, code `Shell::shows` / `shows_href`): the tabs (so the footer), the error page's frame, and every page's link, card, tab and section of it; a hidden kind is not even read (search, near me, home). Writes check `Settings::is_open` in the use case (`feature_off`: a review, a provider's supply, a hire request). What reaches a farmer from outside the page (the inbox's review link, the job emails) waits for "on". `it/features.rs` turns each feature off and checks no page a visitor opens links to it. The admin dashboard says how many are not on. What shows only with data (alerts, sponsored shops, partners) needs no switch. Where a feature's page is led to from is chosen too (`features::PLACEMENTS`, switches `nav.<feature>.top` / `.phone` / `.footer`, defaults the frame as it was): the wide bar, the phone's tab bar and the wide top bar (from 1200 px), each with its room (`Place::room`: `PHONE_TABS_MAX` beside home, services and the account; `TOP_TABS_MAX` beside home and services, what a 1280 px bar holds: `phone_tabs_full`, `top_tabs_full`, checked in `SettingsUseCase::update` on the places as they would stand) and the footer's browse column (`Shell::footer`, no longer the tabs); home and services are always there. `Shell::apply_settings` rebuilds both from them (`shell::BROWSE`, each page's feature and the phone tab it lights when it has none there). The features tab draws each feature's three boxes in its row and, beside the form, the menus as they stand (`molecules/nav_preview.html`, Alpine `navPreview`: follows each tick, greys a bar's boxes once it is full; an off feature's boxes are faded, never disabled, so its places are kept).
- Search synonyms (`domain::search_terms::SYNONYMS`): heo / lợn, bắp / ngô and a few more; `search::text_contains` searches every variant of a query, so every list and admin search finds a thing under either name.
- Job posts (`JobUseCase`): a farmer posts a job (a shown service category, the phone's position or else a location preset, a description, a name and a phone); one phone has at most `jobs.open_per_phone` open posts within `jobs.open_days` (settings; counted and inserted under a lock on the phone, `JobRepositoryTrait::create_capped`, and the per-client day taken with `WindowCounter::take`, so posts sent at once never pass a cap); while the hire-request feature is off none is taken (`feature_off`), its pages answer 404 and the ways to the form (home card, empty search, the provider's jobs card) are hidden; in preview a post emails no provider. A post is never public: `JobNotificationSubscriber` emails the nearest `jobs.notify_max` providers within `jobs.notify_radius_km` with a live service in its category (live, not demo, the managing account's address else the business's) in a background task, and those providers see it with the phone on `/provider/jobs` while it is open and at most `jobs.open_days` old (`OFFERS_THE_JOB` in the job repository, the one place that says so). The admin closes a post or marks it spam.
- The Zalo Mini App (`zalo-mini-app/`) is a client of the JSON API, never a second backend: no business rule lives there, its words are in `src/texts.js`, errors show the backend's message, the API origin is `VITE_API_URL`. Zalo's location and phone tokens are converted only on the server (`ZaloUseCase` over the `ZaloGateway` port, `ZaloGraphGateway` the one file that calls the Zalo Graph API); the secret never reaches the phone. The phone it reads back only fills the box: a job post carries it as text like a typed one, so nothing may treat a post's phone as checked by Zalo (see its README). Its origin (`https://h5.zdn.vn`) goes in `CORS_ORIGINS`. CI only checks it builds; it is deployed with zmp-cli.
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
- Non-GET requests with a foreign `Origin` are refused (`cross_origin_request`); `APP_URL` is allowed always, a `CORS_ORIGINS` origin only on a request without a session cookie.
- Public call buttons are `molecules/call_button.html`: a plain `tel:` link that Alpine `callBeacon` counts with `sendBeacon` to `/api/explore/calls` (uncounted without JS); its label is always the phone number. One visitor's taps on the same business and channel within `CALL_REPEAT_SECS` are one lead (`repeats::RepeatFilter`, the same in-memory filter as repeated searches; a visitor is `client_ip::client_key`, an IPv6 client its /64, and a stale session cookie is nobody). Detail pages (provider, service, product) add `molecules/zalo_button.html` beside it: a link to the Zalo chat with the same number (`zalo` filter, `ZALO_CHAT_URL`), counted by the same beacon with `data-call-channel="zalo"`; cards keep the one call button. Counts cover `CALL_STATS_DAYS` across both channels (admin dashboard total, the provider's own on their dashboard), and the admin dashboard's weekly table is the demand report's (one definition of a lead); a signed-in visitor's taps are never stored (every account is an admin, a provider or a partner); nothing about the caller is stored.
- Domain events (`domain/src/events.rs`) are published after a change is stored; the audit log is a subscriber. A record's admin page (provider, service, supply) reads its latest `HISTORY_SHOWN` entries back (`AdminUseCase::history` with `HistoryOf`, the permission of that page; `molecules/history_list.html`: a "history" card, and under a hidden / switched-off / verified state the entry that explains it), each worded by its audit code (`[text.audit]`, the code itself for one with no words). The verification block lists what to check before granting the badge (position, a shown service, then the phone call and real photos only the admin can tell). New side effects (notifications, webhooks) subscribe; use cases do not call them. A visitor's event (a farmer's inquiry) has no `actor_id` and is not audited; the inquiry subscriber emails the provider (the managing account, else the business's address) in a background task, linking to `/provider/inquiries`.

**Infrastructure layer (`infrastructure/`)**

- Concrete library types (`jsonwebtoken`, `bcrypt`, Sea-ORM) are confined to `infrastructure/`. Implement the port traits from `application/src/ports.rs` here.
- Uploads never reach storage as sent: `MediaUseCase` passes them through the `ImageProcessor` port (type sniffed from bytes, pixel cap checked before decoding, re-encoded as JPEG, EXIF dropped). Storage is the `FileStorage` port (`R2_ACCOUNT_ID` and its bucket and token, else `GCS_BUCKET`, else `UPLOAD_DIR`). Templates link images only with `file_url(id=…, variant="thumb")`. Old images are deleted by the file-cleanup event subscriber, never inline. Products and services each have one optional image, replaced through `MediaUseCase::swap_image`. The platform's logo and browser icon (`brand.logo`, `brand.icon`, `SettingKind::Image`) are uploaded on the settings page only, in the identity tab's one multipart form with its words (`POST /admin/settings/brand`, `SettingsUseCase::save_brand`: every broken box at once, the images stored before the one audited write, discarded if it fails; `ApiMultipart::into_form_files`; a typed file id is `setting_is_an_image`; "back to defaults" never deletes an upload, each is removed behind its own confirmation), through `ImageProcessor::process_icon`: never SVG, at least `ICON_MIN_SIDE`, PNG squares: `ICON_THUMB_SIDE` with its transparency (the bars, the tab), `ICON_MAIN_SIDE` on white with a margin (the home screen: iOS paints transparency black); the file a change stops using rides on `Change::SettingsChanged.released` to the cleanup subscriber. An uploaded logo shows on a white rounded tile (`brand-logo--upload`), the name always written beside it; `/favicon.ico` redirects to the icon (else the logo) for an hour.
- A write that can hit a UNIQUE constraint maps the failure with `repositories::unique_or_internal` to a field error (e.g. `slug_taken`), never a 500.
- The schema holds its own rules; the use case's check before a write is for the message, never the only guard. A row that stores a parent and something of that parent references both columns (`add_reference`: an inquiry's or a tap's service and supply are its own business's, a review's contact request was sent to the business reviewed, a contract's ward is in its province); two optional references of which a row names one have a CHECK (`at_most_one`: what an inquiry or a tap is about, a search's type filter); `service_types` and `supply_types` keep their ids and slugs apart by trigger (`OFFER_TYPE_DISTINCT`, raised as a unique violation); a name is never blank (`not_blank`); emails are stored lower-cased, codes and slugs match `SLUG_PATTERN`, hashes are 64 hex digits. A new table follows the same rules, and `it/database.rs` (`the_schema_refuses_rows_that_contradict_each_other`) gets its cases.
- `JwtTokenService` (in `infrastructure/src/jwt_token_service.rs`) is the only file that calls `jsonwebtoken::encode`. `BcryptPasswordHasher` (in `infrastructure/src/bcrypt_password_hasher.rs`) is the only file that calls `bcrypt::hash` / `bcrypt::verify`.

**Errors (`domain/src/error.rs`, `web/src/error_response.rs`)**

- `IntoResponse for Error` (behind domain's `axum` feature) only sets the status and parks the error; `render_errors` writes the localised body. Every router must sit under that layer.

### Pages

- A page is a handler in `web/src/pages/` that takes `Shell` (the frame: top bar, account menu, bottom tabs, footer, built from the session) and renders through `views::render_page`. The admin panel is never a tab: admins reach it from the account menu and the footer. The footer (`organisms/app_footer.html`) reuses the tabs for its browse links and the account menu's links for its account column, and shows the platform's help lines from the `support.*` settings (`Shell::support`); each unset one is hidden, never a made-up number. Admin pages render through `pages::admin::render_admin` (the admin frame, menu filtered by permission). A stale session cookie renders the page signed out; it never fails it.
- Templates follow the atomic levels above. Links come from `url_for`, product and provider slugs from the `slugify` filter, images from `file_url(id=…, variant="thumb")`, field limits (`maxlength`, `minlength`) from the global `limits`, never typed numbers, and the browser's checks too (`limits.phone_pattern`, `limits.slug_pattern`: `validation::VN_PHONE_PATTERN`, `SLUG_PATTERN`, beside the rules they mirror); an example in a box (`placeholder`) is locale text (`form.phone_placeholder`, …), never typed in a template.
- **No words in code:** every user-facing word lives in `web/locales/vi.toml` and `en.toml` (same keys, a test compares them; `[text]` nested tables read as dotted keys). Templates say `t(key="area.name", count=…)` (an unknown key fails the render, like `url_for`); page code says `state.locale().text("area.name")` / `text_with(key, &[("name", &value)])`, the request's language; `Shell` takes the `Locale`, `Views` holds one Tera set per language; emails get their wording at startup (`EmailTexts`). A sentence that wraps markup is split into `_before` / `_after` keys (never `| safe`). Roles and permissions are worded by code (`role.<code>`, `role_description.<code>`, `permission.<key>`) through `labels::Worded`, in pages and the JSON API alike; the database `name` (seeded as the code) is only the fallback. Convention tests fail on a key with no text and on Vietnamese letters outside the locale files (category and location seeds in `catalogue.rs`, which admins edit, the slug rule and the search synonyms excepted). One word per concept, the same for farmers and admins: **nhà cung cấp** (every provider: mechanic, dealer, vet, hatchery; "thợ", "đại lý" only as examples), **bà con** (farmers), **tin cần mướn** (a farmer's job post: the Southern "mướn"), **yêu cầu liên hệ**, **vật tư** (what a provider sells and a farmer buys: fertiliser, feed, seed, tools; the code's `product`), **nông sản** (what a farmer grows and sells: the price board's items, never a supply), **giá tại nơi sản xuất**, **rác** (spam); English: provider, farmers, hire request, contact request, farm supplies, farm produce, farm-gate price. The two directions never share a word: a farmer hires ("Thuê làm"), buys ("Mua vật tư"), sells (the price board). The Zalo Mini App's `texts.js` follows the same words.
- **Bootstrap first:** Bootstrap 5 components and utilities before custom CSS; custom styles go in `static/css/app.css` only for what Bootstrap cannot do. No `style=""` attributes and no `<style>` elements: the page CSP is `style-src 'self'`. Bootstrap (and Tom Select) draw form icons as `data:` images, which `img-src 'self'` blocks: `app.css` sets the ones the templates use (select chevron, checkbox tick, invalid mark) from `static/img/forms/` as `background-image` itself, never through a `--bs-*` variable (a relative `url()` there resolves against Bootstrap's stylesheet under `vendor/`); a new Bootstrap control with an icon needs its file there too.
- Interactive bits are Alpine components (CSP build) registered in `static/js/app.js`; attribute values name a component property, never an expression (convention test). Markup must work without them: links and plain forms first.
- What Alpine cannot do (today only geolocation) is an island: a Svelte custom element in `backend/islands/src` (light DOM, no `<style>`, all text passed as attributes), built with `npm run build` into `static/js/islands.js`, which is committed. CI rebuilds it and fails on a diff. Pages that use one load it in `{% block scripts %}`. Node is needed only for this and the Zalo Mini App.
- **Large tap targets** (min 44 px) and **minimal text**: the primary users are farmers on low-end Android phones with poor connectivity.
- Custom SQL in `Expr::cust_with_values` uses `$1`, `$2`: a `?` is left unbound and the query fails at runtime (convention test).
- Page forms post urlencoded (`ApiForm`) and follow Post/Redirect/Get: success is a 303 with a `cookies::Flash`; a broken rule re-renders the page with the error's status, the typed values and one message per field (from `error_body`). The rules stay in the use case the JSON API uses. A form with a file box posts `multipart/form-data`, read with `ApiMultipart::into_form`; a photo is checked with `MediaUseCase::prepare_image` and, on an edit, uploaded (`stage_*_image`, a `StagedImage`) before anything else is saved, then shown once the boxes are (`commit_staged`; `abandon_staged` when they are refused), so a storage failure never leaves the new words with the old photo; ownership and permission are checked before the body is read.
- A short create or assign form on an admin page (category, location, service for one provider, the provider's managing account) is a sheet: a `contact-sheet` section with `molecules/sheet_header.html`, in an `x-data="dialog"` scope whose `data-open` is true when the form came back with errors; the trigger (`admin_header`'s `action_opens=true`, or a button in its section) is an anchor to it, so without JS it is a card on the page. A row confirmation then gets its own nested `dialog` scope. Edit forms on a record's own page and large forms (the role permission grid) stay on the page. A sheet is a bottom sheet on a phone and a centred dialog from 768 px (`app.css`), never per page.
- An action that needs a second look (deactivate, hide, delete) uses `organisms/confirm_sheet.html` inside an `x-data="dialog"` element; without JS it is an anchored form on the page. It says what will happen (`effect="hide"` / `"delete"` with `what`, else `note`), and its reason box is optional, capped at `limits.audit_reason_max` (`AUDIT_REASON_MAX`, also cut server-side by `validation::audit_reason`). On a list, the row's link reloads the list with `?hide={id}` (or `?delete=`) and the server renders that confirmation open. Row forms send the list position back as `page` / `provider_id` (`pages::admin::ListPosition`), rebuilt from numbers, never a posted URL; for a record that also has its own page they add `back=list` (`ListReturn`), else its handler lands on that page.
- The admin menu (`pages::admin::MENU`) is grouped by which way a thing flows, so related entries sit together: what farmers sent in ("Bà con gửi về": contact requests, job posts, reviews), what the admin publishes to them ("Đăng cho bà con": the price board, alerts), the directory (providers, their services and supplies), business, the setup (the classification, areas), the system (users, roles & permissions, demo data, settings). The classification is one entry ("Ngành & phân loại") and one screen of four tabs, each list at its own address: sectors, then what is filed under a sector: commodities, service types, supply types (`catalogue_tabs`, `CatalogueTab`). Each group folds (a `<details>`, so without JS too; Alpine `adminMenu` remembers the folded ones in the browser, the current page's group always open), and the sidebar opens with a search box (with JS only): typing keeps the entries whose name, group or tab lists (`MENU_KEYWORDS`, an entry's `data-keywords`) hold every word, accents ignored, and Enter opens the first. A page with no entry of its own lights its owner's (`MENU_OWNERS`). A page showing one of several lists uses `molecules/admin_tabs.html` (`AdminTab`), each tab a URL (`?tab=`), never a second menu entry; `ListPosition` carries the tab and the list's filters (`crop_id`, `commodity_id`, `category_id`) so row actions and page links keep them.
- Every admin hide / deactivate / delete asks for confirmation and a reason (audited); show / activate is one button. Lists narrowed to one provider (`provider_id`: products, services, inquiries, reviews) use `ListPosition::provider_filter` / `pagination` and `molecules/admin_filter.html`.
- Admin list tools, one layout everywhere: the header, then what the list is narrowed to (`admin_filter`: a provider, commodity or type, cleared by its own bar), then the search bar (`molecules/admin_search.html`, `ListPosition::search_box`: one GET form, the words beside an order select that sends itself, `SearchBox::sorted`; it sends every other criterion back as hidden fields and starts at page 1; "Bỏ tìm kiếm" clears the words, "Bỏ lọc" the words, chip and order but not the narrowing), then the state chips (`status_chips`, `ListPosition::state_chips` / `shared_state_chips`), then the table (`admin_search::empty` says why it is empty) and the page links. Which lists search what: providers (name, or a phone typed as digits: `admin_list::phone_digits`; chips unverified / verified / off), services and supplies (own or provider name; shown / hidden), accounts (name, email; on / off), contact requests (sender name or phone, provider name; their statuses), reviews (reviewer, comment, provider; waiting / shown), job posts (farmer name or phone, description; their statuses), partners (name, contact, phone, email; on / off). The short whole lists (sectors, alert kinds, crops, roles) have none. One criteria type for all of them, `domain::models::admin_list::AdminFilter` (provider, search, state, `AdminSort`), applied by one `admin_condition` per repository (the list and its count share it; the search's OR is wrapped in an AND) and ordered by `infrastructure::admin_list::admin_order` (`None` = the list's own order, every order ending with the id). A list offers only the orders that help it (`QUEUE_SORTS`: the oldest first, to work a queue through; `newest` / `oldest` / `name` elsewhere); `ListPosition::checked` drops a state or order the list does not take, so no link carries it. `ListPosition` carries `q`, `state` and `sort` with the page through chips, page links, row forms and record links, so every action lands back on the same view. The JSON lists take the same criteria (`AdminListQuery`: `name`, `status`, `sort`).
- Admin lists end each row with `molecules/row_action.html` macros, one per kind (`show` green or `hide` amber, `delete` red, then `open`), in that order; icon-only on a narrow screen, icon and a short word from 992 px. A table has only the ones its records support. States are `atoms/status_badge.html` (`visibility`, `account`, `verified`); a hidden or switched-off row is grey (`table-light text-body-secondary`), never amber, and green never marks anything but shown / active / verified / call. Rows start with `atoms/admin_thumb.html` where the record has a photo. Inquiry statuses are `inquiry_status::label(badge=true)` (new is blue, contacted soft blue; every badge filled, never bordered like a button). A record with a status (contact request, job post) offers its next steps (`status_steps`): the one it waits for filled (`Step.primary`), the others outlined, spam never stretched on a card; its status chips count the waiting status (`state_chips` `waiting`), the chosen chip tinted, not filled. Amber row buttons use the dark amber text (`--a800`), never Bootstrap's unreadable yellow. Detail pages are `admin-section` cards (title, then `admin-status` line, then one `admin-actions` row); forms are `form-stack`; deleting is its own card at the end. Every record's own admin page (provider, service, supply, crop, alert, sector, alert kind, location, partner, user, role) is a record page, in three layers: `organisms/record_header.html` (`RecordHeader`, built in Rust: its states once as badges, the actions done often as buttons, the rare or harmful ones in a "more" menu, harmful last), one read-only status strip (`record-strip`: what the states mean, who changed them and why, its leads), then the content (a two-column form and details on a wide screen, or `?tab=` parts); its confirmations (`Confirm`, `opened` to come back open after a refusal) close the page, each in its own `dialog`, opened by any link to the sheet's id (the `dialog` component follows the URL hash). An on / off pair is `Toggle` (`visibility` for shown / hidden: off behind its confirmation, on one button), the page placing it among the buttons or in the menu (turning off a provider, partner or account is in the menu); parts are `TabSpec` (`chosen_tab`, `record_tabs`, `to_tab`: a change lands on its part); every one ends with its history (`HistoryOf`). An admin page's one top action (add, public page) goes in `admin_header`'s `action_*`.
- The session lives in httpOnly cookies (`auth_token`, `refresh_token`); no script ever reads a token.
- **Languages:** Vietnamese by default; English when the `lang` cookie says so. The browser's `Accept-Language` is never applied (farmers' phones are often set to English), only offered: a visitor with no cookie whose browser prefers another language of ours sees a one-time strip under the top bar (`molecules/language_offer.html`, `shell.language_offer`), worded in that language with the way out in Vietnamese; either answer sets the cookie. `middleware::language` scopes every request with it (outermost of the page layers, so error bodies and the JSON API follow it); `locale::current()` reads it. One switch, `molecules/language_switch.html`, the same everywhere: a button with the language icon and the current code ("VI"), a `<details>` menu that opens without JS (Alpine `languageMenu`, sharing `topBarMenu` with the account menu, adds closing on Escape / outside). One place per page, never repeated (not in the footer, not in the account menu): the right end of the top bar on app pages and of the admin bar; under the card, at its right edge, on auth pages (`up=true`: the menu opens above it), never above the form nor pinned to a screen corner. Each language is named in itself, never a flag. Emails go out in the default language.

### SEO

Public content pages fill `{% block title %}` and `{% block meta %}` (a `<meta name="description">`) from the loaded data; never a generic fallback.

| Route group | SEO required | Notes |
| --- | --- | --- |
| `/` (home) | Yes | Static title + description |
| `/supplies`, `/supplies/[id]/[slug]` | Yes | Supply name & description in meta |
| `/supplies/category/[id]/[slug]` | Yes | Category name in title |
| `/services`, `/services/category/[id]/[slug]` | Yes | Category name in title |
| `/services/[id]/[slug]` | Yes | Service + provider name in title, description in meta |
| `/providers` | Yes | Static title + description |
| `/prices` | Yes | Area name in title when filtered |
| `/explore` | Yes | Near-me page |
| `/providers/[id]/[slug]` | Yes | Provider name & description in meta |
| `/account`, `/provider/**` | No | Authenticated; `<meta name="robots" content="noindex">` |
| `/login`, `/register`, `/forgot-password`, `/reset-password` | No | Auth flows; `noindex` |
| `/admin/**` | No | Admin panel; `noindex` |

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
# Database tests (each copies its own database from a template kept until the migrations change, and drops it):
# TEST_DATABASE_URL=postgres://... cargo test --workspace -- --include-ignored
# One file's tests: cargo test -p integration-tests --test it search_page:: -- --include-ignored

# Demo data at volume (about 5 000 rows of each kind; development only):
DEMO_ACCOUNT_PASSWORD=... cargo run --bin agri-connect -- demo-data [--providers 5000] [--seed 2026]
# Remove it from /setup/seed like the small demo set.

# Only after changing backend/islands/src: rebuild and commit the bundle
cd islands; npm ci; npm run build

# The Zalo Mini App (see zalo-mini-app/README.md)
cd ../../zalo-mini-app; npm ci; npm run build
```

> **Windows note:** If `npm` fails with a script execution policy error, run:
> `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`

### Deployment

GitHub Actions (`.github/workflows/ci.yml`, `deploy.yml`) builds one image and deploys it to one Compute Engine VM; one-time setup, backups and the restore procedure are in `deploy/README.md`.
