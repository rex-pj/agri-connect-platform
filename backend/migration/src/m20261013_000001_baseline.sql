-- Baseline schema. DDL only: reference data is seeded at startup from the
-- catalogue in `application::catalogue`, demo data only on request.
--
-- Append-only from here: never edit this file once a database has applied it.
-- sea-orm records applied migrations by name, so changing this file does not
-- change existing databases; it only makes fresh ones differ from them.

CREATE EXTENSION IF NOT EXISTS postgis;

CREATE OR REPLACE FUNCTION fn_set_updated_on()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_on = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ── roles ─────────────────────────────────────────────────────────────────────
CREATE TABLE roles (
    id          SERIAL       PRIMARY KEY,
    code        VARCHAR(50)  NOT NULL UNIQUE,
    name        VARCHAR(100) NOT NULL,
    description TEXT,
    is_active   BOOLEAN      NOT NULL DEFAULT TRUE,
    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE TRIGGER trg_roles_updated_on BEFORE UPDATE ON roles
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── users ─────────────────────────────────────────────────────────────────────
CREATE TABLE users (
    id                    SERIAL       PRIMARY KEY,
    username              VARCHAR(100) NOT NULL UNIQUE,
    email                 VARCHAR(255) NOT NULL UNIQUE,
    password_hash         VARCHAR(255) NOT NULL,
    full_name             VARCHAR(200),
    role_id               INT          NOT NULL REFERENCES roles (id),
    is_active             BOOLEAN      NOT NULL DEFAULT TRUE,
    last_login_on         TIMESTAMPTZ,
    failed_login_attempts INT          NOT NULL DEFAULT 0,
    locked_until          TIMESTAMPTZ,
    created_on            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by            INT          REFERENCES users (id) ON DELETE SET NULL,
    updated_by            INT          REFERENCES users (id) ON DELETE SET NULL,
    deleted_at            TIMESTAMPTZ
);
CREATE INDEX idx_users_role ON users (role_id);
CREATE TRIGGER trg_users_updated_on BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── password_reset_tokens ─────────────────────────────────────────────────────
CREATE TABLE password_reset_tokens (
    id         SERIAL       PRIMARY KEY,
    user_id    INT          NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token      VARCHAR(255) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ  NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_password_reset_tokens_user_id ON password_reset_tokens (user_id);

-- ── permissions (role-based grants only) ──────────────────────────────────────
CREATE TABLE permissions (
    id         SERIAL       PRIMARY KEY,
    code       VARCHAR(100) NOT NULL UNIQUE,
    name       VARCHAR(200) NOT NULL,
    created_on TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by INT          REFERENCES users (id) ON DELETE SET NULL,
    updated_by INT          REFERENCES users (id) ON DELETE SET NULL
);
CREATE TRIGGER trg_permissions_updated_on BEFORE UPDATE ON permissions
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

CREATE TABLE role_permissions (
    role_id       INT         NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    permission_id INT         NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    granted_by    INT         REFERENCES users (id) ON DELETE SET NULL,
    granted_on    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);
CREATE INDEX idx_role_permissions_permission ON role_permissions (permission_id);

-- One-time catalogue steps (e.g. the initial default-grant matrix). A row here
-- means "already done, never redo", which is what keeps an admin's revocation
-- from being undone at the next startup.
CREATE TABLE catalogue_state (
    key        VARCHAR(100) PRIMARY KEY,
    applied_on TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- ── categories ────────────────────────────────────────────────────────────────
CREATE TABLE categories (
    id            SERIAL       PRIMARY KEY,
    name          VARCHAR(100) NOT NULL,
    slug          VARCHAR(100) NOT NULL UNIQUE,
    type          VARCHAR(20)  NOT NULL,
    icon          VARCHAR(100),
    color         VARCHAR(20),
    description   VARCHAR(300),
    display_order SMALLINT     NOT NULL DEFAULT 0,
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    created_on    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by    INT          REFERENCES users (id) ON DELETE SET NULL,
    updated_by    INT          REFERENCES users (id) ON DELETE SET NULL,
    -- Keep in sync with domain::constants::CATEGORY_TYPES.
    CONSTRAINT chk_categories_type CHECK (type IN ('service', 'product'))
);
CREATE INDEX idx_categories_type ON categories (type);
CREATE TRIGGER trg_categories_updated_on BEFORE UPDATE ON categories
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── businesses ────────────────────────────────────────────────────────────────
CREATE TABLE businesses (
    id          SERIAL                 PRIMARY KEY,
    name        VARCHAR(200)           NOT NULL,
    phone       VARCHAR(20)            NOT NULL,
    email       VARCHAR(255),
    description TEXT,
    address     VARCHAR(500),
    -- GEOGRAPHY (not GEOMETRY) so distances are metres on the spheroid.
    location    GEOGRAPHY(POINT, 4326),
    is_verified BOOLEAN                NOT NULL DEFAULT FALSE,
    is_active   BOOLEAN                NOT NULL DEFAULT TRUE,
    -- Sample listings; "remove demo data" deletes exactly these rows.
    is_demo     BOOLEAN                NOT NULL DEFAULT FALSE,
    owner_id    INT                    REFERENCES users (id) ON DELETE SET NULL,
    created_on  TIMESTAMPTZ            NOT NULL DEFAULT NOW(),
    updated_on  TIMESTAMPTZ            NOT NULL DEFAULT NOW(),
    created_by  INT                    REFERENCES users (id) ON DELETE SET NULL,
    updated_by  INT                    REFERENCES users (id) ON DELETE SET NULL,
    deleted_at  TIMESTAMPTZ,
    -- Keep in sync with application::validation::is_valid_vn_phone.
    CONSTRAINT chk_businesses_phone CHECK (phone ~ '^0[0-9]{9}$')
);
CREATE INDEX idx_businesses_location ON businesses USING GIST (location)
    WHERE is_active = TRUE AND deleted_at IS NULL;
CREATE INDEX idx_businesses_active ON businesses (is_active, is_verified)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_businesses_owner ON businesses (owner_id);
CREATE INDEX idx_businesses_demo ON businesses (id) WHERE is_demo;
CREATE TRIGGER trg_businesses_updated_on BEFORE UPDATE ON businesses
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── services ──────────────────────────────────────────────────────────────────
CREATE TABLE services (
    id          SERIAL       PRIMARY KEY,
    business_id INT          NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
    category_id INT          NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
    name        VARCHAR(300) NOT NULL,
    description TEXT,
    price_note  VARCHAR(300),
    is_active   BOOLEAN      NOT NULL DEFAULT TRUE,
    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by  INT          REFERENCES users (id) ON DELETE SET NULL,
    updated_by  INT          REFERENCES users (id) ON DELETE SET NULL,
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX idx_services_business ON services (business_id);
CREATE INDEX idx_services_category ON services (category_id);
CREATE INDEX idx_services_live ON services (business_id)
    WHERE is_active = TRUE AND deleted_at IS NULL;
CREATE TRIGGER trg_services_updated_on BEFORE UPDATE ON services
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── products ──────────────────────────────────────────────────────────────────
-- Images arrive with the media slice as a FK to a files table, never a URL.
CREATE TABLE products (
    id          SERIAL       PRIMARY KEY,
    business_id INT          NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
    category_id INT          NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
    name        VARCHAR(300) NOT NULL,
    description TEXT,
    price       BIGINT,
    unit        VARCHAR(50),
    is_active   BOOLEAN      NOT NULL DEFAULT TRUE,
    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by  INT          REFERENCES users (id) ON DELETE SET NULL,
    updated_by  INT          REFERENCES users (id) ON DELETE SET NULL,
    deleted_at  TIMESTAMPTZ,
    CONSTRAINT chk_products_price CHECK (price IS NULL OR price > 0)
);
CREATE INDEX idx_products_business ON products (business_id);
CREATE INDEX idx_products_category ON products (category_id);
CREATE INDEX idx_products_live ON products (business_id)
    WHERE is_active = TRUE AND deleted_at IS NULL;
CREATE TRIGGER trg_products_updated_on BEFORE UPDATE ON products
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── inquiries ─────────────────────────────────────────────────────────────────
CREATE TABLE inquiries (
    id              SERIAL       PRIMARY KEY,
    business_id     INT          NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
    service_id      INT          REFERENCES services (id) ON DELETE SET NULL,
    product_id      INT          REFERENCES products (id) ON DELETE SET NULL,
    topic           VARCHAR(300) NOT NULL,
    requester_name  VARCHAR(200) NOT NULL,
    requester_phone VARCHAR(20)  NOT NULL,
    note            TEXT,
    status          VARCHAR(20)  NOT NULL DEFAULT 'new',
    created_on      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_by      INT          REFERENCES users (id) ON DELETE SET NULL,
    -- Keep in sync with domain::constants::INQUIRY_STATUSES.
    CONSTRAINT chk_inquiries_status CHECK (status IN ('new', 'contacted', 'closed', 'spam')),
    CONSTRAINT chk_inquiries_phone CHECK (requester_phone ~ '^0[0-9]{9}$')
);
CREATE INDEX idx_inquiries_business ON inquiries (business_id);
CREATE INDEX idx_inquiries_status ON inquiries (status);
CREATE INDEX idx_inquiries_created_on ON inquiries (created_on);
CREATE TRIGGER trg_inquiries_updated_on BEFORE UPDATE ON inquiries
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── reviews ───────────────────────────────────────────────────────────────────
CREATE TABLE reviews (
    id                  SERIAL       PRIMARY KEY,
    business_id         INT          NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
    inquiry_id          INT          REFERENCES inquiries (id) ON DELETE SET NULL,
    rating              SMALLINT     NOT NULL,
    comment             TEXT,
    reviewer_name       VARCHAR(200) NOT NULL,
    reviewer_phone_hash VARCHAR(64)  NOT NULL,
    is_visible          BOOLEAN      NOT NULL DEFAULT TRUE,
    created_on          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_on          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_by          INT          REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT chk_reviews_rating CHECK (rating BETWEEN 1 AND 5),
    CONSTRAINT uq_reviews_business_reviewer UNIQUE (business_id, reviewer_phone_hash)
);
CREATE INDEX idx_reviews_business_visible ON reviews (business_id) WHERE is_visible = TRUE;
CREATE TRIGGER trg_reviews_updated_on BEFORE UPDATE ON reviews
    FOR EACH ROW EXECUTE FUNCTION fn_set_updated_on();

-- ── locations (geo-search presets) ────────────────────────────────────────────
CREATE TABLE locations (
    id            SERIAL           PRIMARY KEY,
    label         VARCHAR(150)     NOT NULL UNIQUE,
    lat           DOUBLE PRECISION NOT NULL,
    lng           DOUBLE PRECISION NOT NULL,
    display_order INT              NOT NULL DEFAULT 0,
    is_active     BOOLEAN          NOT NULL DEFAULT TRUE
);

-- ── audit_logs ────────────────────────────────────────────────────────────────
CREATE TABLE audit_logs (
    id          BIGSERIAL    PRIMARY KEY,
    -- Nullable so the log survives deletion of the admin who acted.
    admin_id    INT          REFERENCES users (id) ON DELETE SET NULL,
    action      VARCHAR(100) NOT NULL,
    entity_type VARCHAR(50)  NOT NULL,
    entity_id   INT          NOT NULL,
    reason      TEXT,
    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE INDEX ix_audit_logs_admin_id ON audit_logs (admin_id);
CREATE INDEX ix_audit_logs_entity ON audit_logs (entity_type, entity_id);
CREATE INDEX ix_audit_logs_created_on ON audit_logs (created_on DESC);
