//! Partners: the companies that pay for the demand report and sponsored
//! places. A contract covers one area (a location preset), some categories,
//! two dates and a monthly fee; it sponsors some providers of that area.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Keep the kinds in sync with domain::constants::PARTNER_KINDS.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE partners (
                    id           SERIAL       PRIMARY KEY,
                    name         VARCHAR(200) NOT NULL,
                    kind         VARCHAR(20)  NOT NULL,
                    contact_name VARCHAR(200),
                    phone        VARCHAR(20),
                    email        VARCHAR(255),
                    account_id   INT          UNIQUE REFERENCES users (id) ON DELETE SET NULL,
                    is_active    BOOLEAN      NOT NULL DEFAULT TRUE,
                    created_on   TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
                    updated_on   TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
                    created_by   INT          REFERENCES users (id) ON DELETE SET NULL,
                    updated_by   INT          REFERENCES users (id) ON DELETE SET NULL,
                    CONSTRAINT chk_partners_kind
                        CHECK (kind IN ('brand', 'dealer', 'cooperative', 'exporter')),
                    CONSTRAINT chk_partners_phone CHECK (phone IS NULL OR phone ~ '^0[0-9]{9}$')
                );
                CREATE TABLE partner_contracts (
                    id          SERIAL      PRIMARY KEY,
                    partner_id  INT         NOT NULL REFERENCES partners (id) ON DELETE CASCADE,
                    location_id INT         NOT NULL REFERENCES locations (id) ON DELETE RESTRICT,
                    starts_on   DATE        NOT NULL,
                    ends_on     DATE        NOT NULL,
                    monthly_fee BIGINT      NOT NULL,
                    note        TEXT,
                    created_on  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    created_by  INT         REFERENCES users (id) ON DELETE SET NULL,
                    CONSTRAINT chk_partner_contracts_dates CHECK (ends_on >= starts_on),
                    CONSTRAINT chk_partner_contracts_fee CHECK (monthly_fee >= 0)
                );
                CREATE INDEX ix_partner_contracts_partner ON partner_contracts (partner_id);
                CREATE TABLE partner_contract_categories (
                    contract_id INT NOT NULL REFERENCES partner_contracts (id) ON DELETE CASCADE,
                    category_id INT NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
                    PRIMARY KEY (contract_id, category_id)
                );
                CREATE TABLE partner_sponsored (
                    contract_id INT NOT NULL REFERENCES partner_contracts (id) ON DELETE CASCADE,
                    business_id INT NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
                    PRIMARY KEY (contract_id, business_id)
                );",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE IF EXISTS partner_sponsored, partner_contract_categories, \
                 partner_contracts, partners;",
            )
            .await?;
        Ok(())
    }
}
