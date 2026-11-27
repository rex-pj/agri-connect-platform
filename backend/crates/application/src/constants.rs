// ── Pagination ─────────────────────────────────────────────────────────────────
pub const PAGE_SIZE: i64 = 20;
pub const ADMIN_PAGE_SIZE: i64 = 25;
pub const MAX_PAGE: i64 = 10_000;
/// The last page of an admin list: past it an admin narrows the list with
/// its search box or filter, since every page further costs the database all
/// the rows before it.
pub const ADMIN_MAX_PAGE: i64 = 500;
/// The last page of a public list (services, providers, supplies): a farmer
/// narrows a search long before it, and every page further costs the
/// database all the rows before it.
pub const PUBLIC_MAX_PAGE: i64 = 50;

// ── Featured counts ────────────────────────────────────────────────────────────
pub const FEATURED_PROVIDERS_COUNT: i64 = 6;
pub const FEATURED_PRODUCTS_COUNT: i64 = 8;
/// Services on the home page and the near-me page before a position is
/// known: one per verified provider.
pub const FEATURED_SERVICES_COUNT: usize = 6;

// ── Authentication ─────────────────────────────────────────────────────────────
pub const JWT_EXPIRY_HOURS: i64 = 1;
pub const PASSWORD_RESET_EXPIRY_HOURS: i64 = 1;
/// Reset links of one account that may be valid at once; a request beyond it
/// sends nothing until one runs out.
pub const PASSWORD_RESET_LINKS_MAX: u64 = 3;
/// SETUP_TOKEN is typed once by whoever deploys; a shorter one is guessable.
pub const SETUP_TOKEN_MIN_CHARS: usize = 16;
/// How long the link that confirms an email address works.
pub const EMAIL_VERIFICATION_EXPIRY_HOURS: i64 = 24;
/// Confirmation links of one account that may be valid at once: within a
/// day, an address is written to at most this often before it is confirmed.
pub const EMAIL_VERIFICATION_LINKS_MAX: u64 = 3;
/// Wrong passwords from one client (address, an IPv6 /64) for one account
/// before that client is kept out of that account for `LOGIN_LOCK_MINUTES`.
/// Per client, so a stranger who knows an address cannot lock its owner out.
pub const LOGIN_FAIL_MAX_ATTEMPTS: u32 = 5;
/// Wrong passwords for one account from every client together before the
/// account itself is locked: the backstop against guessing spread over many
/// addresses, high enough that no single stranger reaches it.
pub const LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS: i32 = 50;
pub const LOGIN_LOCK_MINUTES: i64 = 15;
/// Job posts from one client (address) within a day; the cap per phone does
/// not hold alone, since the phone is whatever the poster types. The
/// default of `settings::JOB_POSTS_PER_CLIENT_PER_DAY`.
pub const JOB_POSTS_PER_CLIENT_PER_DAY_DEFAULT: i64 = 5;
/// The day the cap above counts over.
pub const JOB_POST_CLIENT_WINDOW_SECS: u64 = 24 * 3600;
/// Sign-ups one client (address) may try within `SIGNUP_CLIENT_WINDOW_SECS`,
/// counted once the form is valid: each such try tells whether the email has
/// an account, so the cap bounds how fast anyone can probe
/// addresses. A security limit, never a setting. Mobile carriers share one
/// address among many phones (CGNAT); sign-ups are rare enough for this.
pub const SIGNUP_ATTEMPTS_PER_CLIENT: u32 = 10;
pub const SIGNUP_CLIENT_WINDOW_SECS: u64 = 24 * 3600;
/// Contact requests one client (address) may send to one provider within
/// `CONTACT_CLIENT_WINDOW_SECS`: each emails the provider and counts as a
/// lead in the demand report partners pay for, so one address must not
/// flood either. Per provider, not per address alone: mobile carriers share
/// one address among many phones (CGNAT), and neighbours writing to
/// different providers never share a count. A security limit, never a
/// setting.
pub const CONTACT_REQUESTS_PER_CLIENT_AND_PROVIDER: u32 = 3;
pub const CONTACT_CLIENT_WINDOW_SECS: u64 = 24 * 3600;

// ── Validation ─────────────────────────────────────────────────────────────────
/// The `users.full_name` column.
pub const FULL_NAME_MAX: usize = 200;
/// `users.email` is VARCHAR(255).
pub const EMAIL_MAX: usize = 255;
/// The platform's name (`brand.name`): it sits beside the logo in a phone's
/// top bar, so it stays short.
pub const SITE_NAME_MAX: usize = 40;
/// The platform's tagline (`brand.tagline`): the home page's title after the
/// name ("Name | Tagline"), which search engines cut near 60 in all.
pub const SITE_TAGLINE_MAX: usize = 60;
/// The platform's one-line description (`brand.description`): the meta
/// description search engines show, which they cut near 160. Never shown on
/// a page.
pub const SITE_DESCRIPTION_MAX: usize = 160;
/// Shorter than this, search engines often show another bit of the page
/// instead: the settings page says so (a nudge, never a refusal).
pub const SITE_DESCRIPTION_ADVISED_MIN: usize = 50;
/// Counted in characters, so a Vietnamese password is not penalised for its
/// multi-byte letters.
pub const PASSWORD_MIN_CHARS: usize = 8;
/// bcrypt silently ignores everything after byte 72, so a longer password
/// would look accepted while only its prefix counts. Reject instead.
pub const PASSWORD_MAX_BYTES: usize = 72;
pub const REQUESTER_NAME_MAX_LEN: usize = 100;
/// The `inquiries.topic` column. The contact form's topic is a service name,
/// so this must be at least `SERVICE_NAME_MAX` (asserted at compile time).
pub const TOPIC_MAX_LEN: usize = 300;
pub const NOTE_MAX_LEN: usize = 1000;

// ── Geography ─────────────────────────────────────────────────────────────────
pub const LAT_MIN: f64 = -90.0;
pub const LAT_MAX: f64 = 90.0;
pub const LNG_MIN: f64 = -180.0;
pub const LNG_MAX: f64 = 180.0;

// ── The map a provider places its business on ─────────────────────────────────
/// The map's zoom levels: 5 shows the whole country, 19 a house (the most
/// the tile host draws). Tiles outside are refused.
pub const MAP_ZOOM_MIN: u8 = 5;
pub const MAP_ZOOM_MAX: u8 = 19;
/// The map opens on the Mekong Delta when nothing is placed yet…
pub const MAP_START_LAT: f64 = 10.25;
pub const MAP_START_LNG: f64 = 105.9;
pub const MAP_START_ZOOM: u8 = 9;
/// …on a ward's centre at this zoom, on a placed point at the next.
pub const MAP_WARD_ZOOM: u8 = 14;
pub const MAP_POINT_ZOOM: u8 = 17;
/// The tiles served: those touching Vietnam's mainland and near islands, so
/// the tile route is no proxy for the rest of the world.
pub const MAP_REGION_LAT_MIN: f64 = 8.0;
pub const MAP_REGION_LAT_MAX: f64 = 23.5;
pub const MAP_REGION_LNG_MIN: f64 = 102.0;
pub const MAP_REGION_LNG_MAX: f64 = 110.0;
/// A position the browser gives with an error larger than this (a computer
/// placed by its internet address, a phone indoors without GPS) is not
/// taken for a business: the provider picks the ward or the map instead.
pub const GEOLOCATE_ACCURACY_MAX_M: u32 = 500;
/// A business this close to its ward's centre (in degrees, about a metre)
/// was placed by choosing the ward, not on the spot: its place is said to
/// be approximate.
pub const WARD_CENTRE_TOLERANCE_DEG: f64 = 0.00001;
/// The most wards a ward box offers for the words typed: enough to see the
/// one meant, few enough for a weak signal.
pub const WARD_SEARCH_RESULTS: u64 = 20;

pub const RADIUS_MIN_M: f64 = 100.0;
pub const RADIUS_MAX_M: f64 = 50_000.0;
/// Metres in a kilometre (radii are stored in metres, shown and set in km).
pub const METRES_PER_KM: i64 = 1_000;
/// A farm visit by motorbike; the near-me page and the JSON API share it.
pub const RADIUS_DEFAULT_M: f64 = 10_000.0;
/// Most results one near-me search returns (providers or services), nearest
/// first: enough for a district without a page that never ends.
pub const GEO_RESULTS_MAX: i64 = 50;
/// One visitor's taps on the same business and channel within this long are
/// one lead: a second tap is a redial, and without it a script could write
/// the figures partners pay for.
pub const CALL_REPEAT_SECS: u64 = 3_600;
/// A review written this soon after the contact request it rests on is
/// flagged for the moderator: no job is done and judged that fast (the
/// default of `settings::REVIEW_SUSPECT_MINUTES`).
pub const REVIEW_SUSPECT_MINUTES_DEFAULT: i64 = 60;
/// The window the call counts cover (admin dashboard, provider dashboard).
pub const CALL_STATS_DAYS: i64 = 30;
/// How many of a record's latest changes its admin page lists (who, what,
/// why).
pub const HISTORY_SHOWN: u64 = 10;
/// The window of the admin dashboard's "new providers" figure.
pub const NEW_BUSINESS_DAYS: i64 = 7;

// ── Demand report ─────────────────────────────────────────────────────────────
/// Vietnam is UTC+7 all year (no daylight saving since 1975), so a fixed
/// offset is exact: report weeks, dates shown on pages.
pub const VN_UTC_OFFSET_SECS: i32 = 7 * 3600;
/// How many whole weeks the demand report covers, ending last Sunday.
pub const DEMAND_WEEKS: usize = 8;
/// How many whole weeks before this one taps and searches are kept as rows.
/// Older ones are counted into the weekly totals and deleted. Longer than
/// anything reads rows for (`DEMAND_WEEKS`, `CALL_STATS_DAYS`), with room to
/// look into a figure a partner questions.
pub const EVENT_KEEP_WEEKS: usize = 26;
/// A figure under this (and above zero) is shown as "fewer than": it could
/// point at one household.
pub const DEMAND_REPORT_MIN_COUNT: i64 = 5;
/// A point counts under the ward it lies in; one just outside every ward
/// (a coastline, a sliver between two simplified boundaries) under the
/// nearest within this reach. The stored boundaries are within a few metres
/// of the official ones, so this only catches what falls between them.
pub const ADMIN_UNIT_SNAP_M: f64 = 500.0;
/// An area is steady when every one of the report's weeks has at least this
/// many leads: ready to show partners (the default of
/// `settings::STEADY_WEEKLY_LEADS`).
pub const STEADY_WEEKLY_LEADS_DEFAULT: i64 = 20;
/// A stored search keeps at most this many characters of its words.
pub const SEARCH_QUERY_MAX: usize = 100;
/// The fewest characters a public search takes (`validation::search_text`).
pub const SEARCH_QUERY_MIN: usize = 2;
/// How many searches that found nothing the report lists, most asked first.
pub const UNMET_SEARCHES_SHOWN: usize = 10;
/// The same search by the same visitor within this many seconds counts once
/// (a reload, a back button).
pub const SEARCH_REPEAT_SECS: u64 = 600;

// ── Partners ──────────────────────────────────────────────────────────────────
pub const PARTNER_NAME_MAX: usize = 200;
pub const CONTRACT_NOTE_MAX: usize = 500;
/// How many sponsored services a near-me page shows above its results: the
/// default of `settings::SPONSORED_SHOWN`, and the most it may be set to.
pub const SPONSORED_SHOWN_DEFAULT: usize = 2;
pub const SPONSORED_SHOWN_MAX: usize = 5;

// ── Coded lists (sectors, alert kinds) ───────────────────────────────────────
pub const CODED_NAME_MAX: usize = 100;
/// A code's length (`sectors.code`, `alert_kinds.code`): the slug of its
/// name, cut.
pub const CODED_CODE_MAX: usize = 40;

// ── Commodities (cây trồng, vật nuôi) ──────────────────────────────────────────────────
pub const COMMODITY_NAME_MAX: usize = 100;

// ── Crop prices ───────────────────────────────────────────────────────────────
pub const CROP_NAME_MAX: usize = 100;
pub const CROP_UNIT_MAX: usize = 20;
pub const PRICE_NOTE_MAX: usize = 200;
/// An alert's title and body, short enough to read on a phone at a glance.
pub const ALERT_TITLE_MAX: usize = 120;
pub const ALERT_BODY_MAX: usize = 600;
/// Alerts shown on the home page at once, the newest first.
pub const ALERTS_SHOWN: u64 = 3;
/// The highest price per unit an admin can enter, in đồng: a typo with an
/// extra zero or two is refused rather than published.
pub const PRICE_PER_UNIT_MAX: i64 = 10_000_000;
/// A price older than its commodity's `price_fresh_days` is not shown: a
/// stale price misleads more than none. This is a new commodity's
/// (`chk_commodities_price_fresh_days` holds the bounds too).
pub const PRICE_FRESH_DAYS_DEFAULT: i16 = 14;
/// The most a commodity's freshness may be: the board reads no further back.
pub const PRICE_FRESH_DAYS_MAX: i16 = 60;
/// How far back the price before the latest is looked for, to show the move.
pub const PRICE_HISTORY_DAYS: i64 = 60;
const _: () = assert!(PRICE_FRESH_DAYS_MAX as i64 <= PRICE_HISTORY_DAYS);
/// How many days back the price page draws each crop's trend in an area
/// (today included): at most one price a day, so as many points.
pub const PRICE_TREND_DAYS: i64 = 14;
/// Fewer prices than this in the window draw no trend: two points are a
/// move, which the line already says.
pub const PRICE_TREND_MIN_POINTS: usize = 3;
const _: () = assert!(PRICE_TREND_DAYS <= PRICE_HISTORY_DAYS);

// ── Reviews ───────────────────────────────────────────────────────────────────
pub const RATING_MIN: i16 = 1;
pub const RATING_MAX: i16 = 5;
pub const REVIEW_COMMENT_MAX: usize = 500;
pub const REVIEW_REPLY_MAX: usize = 500;
/// How many shown reviews a provider's page lists, newest first.
pub const PUBLIC_REVIEWS_SHOWN: i64 = 20;

// ── Uploads ───────────────────────────────────────────────────────────────────
/// Used when UPLOAD_MAX_BYTES is unset. A phone photo is 2–4 MB.
pub const DEFAULT_UPLOAD_MAX_BYTES: usize = 5 * 1024 * 1024;
/// Checked from the header before decoding: a 50 KB PNG can claim 100 000 ×
/// 100 000 pixels and would take 40 GB to decode.
pub const IMAGE_MAX_PIXELS: u64 = 40_000_000;
/// Longest side of the stored image; enough for a phone screen at 2x.
pub const IMAGE_MAIN_MAX_SIDE: u32 = 1600;
/// Longest side of the list/card thumbnail.
pub const IMAGE_THUMB_MAX_SIDE: u32 = 400;
pub const IMAGE_JPEG_QUALITY: u8 = 80;
/// The platform's logo and browser icon (`brand.logo`, `brand.icon`): a square
/// at least this wide, so the phone home-screen icon (180 px) stays sharp.
pub const ICON_MIN_SIDE: u32 = 192;
/// Its large copy: the phone home-screen icon, on white (iOS paints a
/// transparent background black).
pub const ICON_MAIN_SIDE: u32 = 512;
/// Its small copy: the browser tab and the top bars, at twice their size.
pub const ICON_THUMB_SIDE: u32 = 96;

// ── Sessions ──────────────────────────────────────────────────────────────────
/// Used when REFRESH_TOKEN_DAYS is unset: a farmer opening the app weekly
/// stays signed in; one who stops for a month signs in again.
pub const REFRESH_TOKEN_DEFAULT_DAYS: i64 = 30;
/// A spent token presented again this soon is two tabs refreshing at once,
/// not theft: refuse it without revoking the family, or both tabs log out.
pub const REFRESH_REUSE_GRACE_SECS: i64 = 30;

// ── Catalogue fields (the VARCHAR sizes in the baseline schema) ────────────────
pub const CATEGORY_NAME_MAX: usize = 100;
pub const CATEGORY_SLUG_MAX: usize = 100;
/// Icon names an icon box's search gives back at once (`GET /api/admin/icons`).
pub const ICON_SEARCH_RESULTS: usize = 60;
/// Shops a contract's sponsor search lists at once (its page, `?q=`): the
/// first by name, or those matching the words; an area can hold thousands.
pub const SPONSOR_CANDIDATES_SHOWN: u64 = 20;
pub const CATEGORY_DESCRIPTION_MAX: usize = 300;
/// Numbered slug candidates (`base`, `base-2`, …) checked per query while
/// looking for a free one.
pub const SLUG_CANDIDATES_PER_QUERY: u32 = 10;
pub const LOCATION_LABEL_MAX: usize = 150;
/// Wards one "add several" sheet may turn into presets at once: more than
/// the wards of the largest province, so one province is always one go.
pub const LOCATION_BULK_MAX: usize = 200;
pub const SERVICE_NAME_MAX: usize = 300;
pub const SERVICE_PRICE_NOTE_MAX: usize = 300;
/// `services.description` is TEXT; this keeps a listing readable on a phone.
pub const SERVICE_DESCRIPTION_MAX: usize = 2000;
/// How many services one business may list (hidden ones count), so deleting
/// and re-adding cannot flood the directory: the default of
/// `settings::SERVICES_PER_BUSINESS`, and the most it may be set to (the
/// featured picks read that many per provider).
pub const SERVICES_PER_BUSINESS_DEFAULT: u64 = 20;
pub const SERVICES_PER_BUSINESS_MAX: u64 = 50;
/// How many supplies one business may list (hidden ones count): a dealer's
/// whole shelf, and still a list its owner's page shows at once: the default
/// of `settings::PRODUCTS_PER_BUSINESS`, and the most it may be set to.
pub const PRODUCTS_PER_BUSINESS_DEFAULT: u64 = 200;
pub const PRODUCTS_PER_BUSINESS_MAX: u64 = 500;
pub const PRODUCT_NAME_MAX: usize = 300;
pub const PRODUCT_UNIT_MAX: usize = 50;
pub const PRODUCT_DESCRIPTION_MAX: usize = 2000;
pub const BUSINESS_NAME_MAX: usize = 200;
pub const BUSINESS_ADDRESS_MAX: usize = 500;
pub const BUSINESS_DESCRIPTION_MAX: usize = 2000;
/// `audit_log.reason` is TEXT; an admin's "why" is a sentence, not an essay.
pub const AUDIT_REASON_MAX: usize = 500;

// Every service name must fit as a contact topic.
const _: () = assert!(TOPIC_MAX_LEN >= SERVICE_NAME_MAX);

// ── Job posts ─────────────────────────────────────────────────────────────────
pub const JOB_DESCRIPTION_MAX: usize = 500;
// The defaults of the job settings (`settings::JOB_*`).
/// How long a job post stays open to providers, in days.
pub const JOB_POST_DAYS_DEFAULT: i64 = 7;
/// How long a job post is kept at all, in days. Providers see one while it
/// is open; after this it is deleted, whatever its status, so the table and
/// the phone numbers in it do not pile up for ever.
pub const JOB_POST_KEEP_DAYS: i64 = 365;
/// Open posts one phone may have while they are open.
pub const JOB_POSTS_OPEN_PER_PHONE_DEFAULT: i64 = 3;
/// How far from a job a provider is told about it, in km (the near-me
/// radius before the visitor picks one).
pub const JOB_NOTIFY_RADIUS_KM_DEFAULT: i64 = RADIUS_DEFAULT_M as i64 / METRES_PER_KM;
/// At most this many providers (the nearest) are emailed about one job.
pub const JOB_NOTIFY_MAX_DEFAULT: i64 = 20;

// ── Information pages ─────────────────────────────────────────────────────────
/// An information page's address (`/info/{code}`) and title.
pub const INFO_PAGE_CODE_MAX: usize = 60;
pub const INFO_PAGE_TITLE_MAX: usize = 160;
/// A page's text in Markdown: room for a full policy, not a book.
pub const INFO_PAGE_BODY_MAX: usize = 60_000;
/// What changed in a new version, as the page's version list says it.
pub const INFO_PAGE_NOTE_MAX: usize = 300;

// ── The legal entity (`legal.*` settings) ─────────────────────────────────────
/// A company's or a person's name.
pub const LEGAL_NAME_MAX: usize = 160;
/// A tax code: 10 digits, 13 with a branch's suffix.
pub const LEGAL_TAX_CODE_MAX: usize = 20;
/// An address, or the line of the business registration.
pub const LEGAL_LINE_MAX: usize = 250;

// ── Articles (news and guides) ────────────────────────────────────────────────
/// An article's title and its summary, the cards' line and the page's
/// description.
pub const POST_TITLE_MAX: usize = 160;
pub const POST_SUMMARY_MAX: usize = 300;
/// An article's text in Markdown.
pub const POST_BODY_MAX: usize = 60_000;
/// The newest articles the home page shows, while the feature is on.
pub const HOME_POSTS_SHOWN: i64 = 4;
/// The other articles of the same kind under one.
pub const RELATED_POSTS_SHOWN: i64 = 4;
