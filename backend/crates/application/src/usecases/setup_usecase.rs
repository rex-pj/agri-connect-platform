use std::sync::Arc;

use domain::{
    models::catalogue::{Catalogue, DemoReport, SeedReport},
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        catalogue_repository_trait::CatalogueRepositoryTrait,
        demo_data_repository_trait::DemoDataRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result,
};
use tracing::{info, warn};

use crate::outputs::SetupStatusDto;

#[derive(Clone)]
pub struct SetupUseCase {
    catalogue_repo: Arc<dyn CatalogueRepositoryTrait + Send + Sync>,
    demo_repo: Arc<dyn DemoDataRepositoryTrait + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
}

impl SetupUseCase {
    pub fn new(
        catalogue_repo: Arc<dyn CatalogueRepositoryTrait + Send + Sync>,
        demo_repo: Arc<dyn DemoDataRepositoryTrait + Send + Sync>,
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            catalogue_repo,
            demo_repo,
            business_repo,
            user_repo,
        }
    }

    /// Runs at every startup, before the server accepts requests. Without it a
    /// fresh database has no roles, so the first registration would fail.
    pub async fn seed_catalogue(&self, catalogue: &Catalogue) -> Result<SeedReport> {
        let report = self.catalogue_repo.apply(catalogue).await?;
        info!(
            roles = report.roles_inserted,
            permissions = report.permissions_inserted,
            grants = report.grants_inserted,
            bootstrapped_grants = report.bootstrapped_grants,
            categories = report.categories_inserted,
            locations = report.locations_inserted,
            "reference catalogue applied"
        );
        if !report.unknown_permissions.is_empty() {
            // Left in place on purpose: deleting them would silently drop
            // whatever grants an admin made.
            warn!(keys = ?report.unknown_permissions, "permissions in the database are not in the catalogue");
        }
        Ok(report)
    }

    pub async fn get_status(&self) -> Result<SetupStatusDto> {
        let (business_count, user_count, demo_business_count) = tokio::try_join!(
            self.business_repo.count_all(),
            self.user_repo.count_all(),
            self.demo_repo.count(),
        )?;

        Ok(SetupStatusDto {
            database_connected: true,
            is_seeded: demo_business_count > 0,
            demo_business_count,
            business_count,
            user_count,
        })
    }

    pub async fn load_demo(&self) -> Result<DemoReport> {
        if self.demo_repo.count().await? > 0 {
            return Err(Error::new(ErrorCode::DemoAlreadyLoaded));
        }
        let report = self.demo_repo.load().await?;
        info!(
            businesses = report.businesses,
            services = report.services,
            products = report.products,
            "demo data loaded"
        );
        Ok(report)
    }

    pub async fn remove_demo(&self) -> Result<u64> {
        let removed = self.demo_repo.remove().await?;
        info!(businesses = removed, "demo data removed");
        Ok(removed)
    }
}
