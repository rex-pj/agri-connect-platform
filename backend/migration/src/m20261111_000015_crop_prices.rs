//! Crop prices: the farm-gate price of a crop (a rice variety, a fruit) in
//! one area (a location preset) on one day, as a low–high range in đồng per
//! unit. One row per crop, area and day: entering the day's price again
//! replaces it. The crops themselves are reference data (seeded when the
//! table is empty, then edited by admins).

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Keep the kinds in sync with domain::constants::CROP_KINDS.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE crops (
                    id            SERIAL       PRIMARY KEY,
                    name          VARCHAR(100) NOT NULL UNIQUE,
                    kind          VARCHAR(20)  NOT NULL,
                    unit          VARCHAR(20)  NOT NULL,
                    display_order INT          NOT NULL DEFAULT 0,
                    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
                    CONSTRAINT chk_crops_kind CHECK (kind IN ('rice', 'fruit'))
                );
                CREATE TABLE crop_prices (
                    id          SERIAL       PRIMARY KEY,
                    crop_id     INT          NOT NULL REFERENCES crops (id) ON DELETE CASCADE,
                    location_id INT          NOT NULL REFERENCES locations (id) ON DELETE CASCADE,
                    observed_on DATE         NOT NULL,
                    price_low   BIGINT       NOT NULL,
                    price_high  BIGINT       NOT NULL,
                    note        VARCHAR(200),
                    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
                    created_by  INT          REFERENCES users (id) ON DELETE SET NULL,
                    CONSTRAINT uq_crop_prices_day UNIQUE (crop_id, location_id, observed_on),
                    CONSTRAINT chk_crop_prices_range CHECK (price_low > 0 AND price_high >= price_low)
                );
                CREATE INDEX ix_crop_prices_observed ON crop_prices (observed_on DESC, id DESC);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS crop_prices, crops;")
            .await?;
        Ok(())
    }
}
