use std::sync::Arc;

use domain::{
    models::{
        admin_unit::AdminUnitSync,
        catalogue::{Catalogue, DemoReport, DemoVolume, SeedReport},
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        business_repository_trait::BusinessRepositoryTrait,
        catalogue_repository_trait::CatalogueRepositoryTrait,
        demo_data_repository_trait::DemoDataRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result,
};
use tracing::{info, warn};

use crate::{
    authz::Actor,
    catalogue::{PERM_ADMIN_DASHBOARD_VIEW, PERM_SYSTEM_DEMO_MANAGE},
    outputs::SetupStatusDto,
    ports::PasswordHasher,
    usecases::media_usecase::MediaUseCase,
    validation::validate_password,
};

#[derive(Clone)]
pub struct SetupUseCase {
    catalogue_repo: Arc<dyn CatalogueRepositoryTrait + Send + Sync>,
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    demo_repo: Arc<dyn DemoDataRepositoryTrait + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    /// Hashes the one password of the demo accounts.
    password_hasher: Arc<dyn PasswordHasher>,
    /// Removes the photos of demo services and products when the demo data
    /// goes.
    media: MediaUseCase,
}

impl SetupUseCase {
    pub fn new(
        catalogue_repo: Arc<dyn CatalogueRepositoryTrait + Send + Sync>,
        admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
        demo_repo: Arc<dyn DemoDataRepositoryTrait + Send + Sync>,
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
        password_hasher: Arc<dyn PasswordHasher>,
        media: MediaUseCase,
    ) -> Self {
        Self {
            catalogue_repo,
            admin_unit_repo,
            demo_repo,
            business_repo,
            user_repo,
            password_hasher,
            media,
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
            "platform catalogue applied"
        );
        if !report.unknown_permissions.is_empty() {
            // Left in place on purpose: deleting them would silently drop
            // whatever grants an admin made.
            warn!(keys = ?report.unknown_permissions, "permissions in the database are not in the catalogue");
        }
        Ok(report)
    }

    /// Runs at every startup, after the catalogue: loads the bundled
    /// provinces and wards when their version is not the one stored (the
    /// first start, or a new dataset), else does nothing.
    pub async fn seed_admin_units(&self) -> Result<AdminUnitSync> {
        let sync = self.admin_unit_repo.sync().await?;
        if sync.loaded {
            info!(
                version = %sync.version,
                provinces = sync.provinces,
                wards = sync.wards,
                "administrative units loaded"
            );
        }
        Ok(sync)
    }

    pub async fn get_status(&self, actor: &Actor) -> Result<SetupStatusDto> {
        actor.require(PERM_ADMIN_DASHBOARD_VIEW)?;
        self.load_status().await
    }

    /// For the demo data page: who may load or remove demo data may see how
    /// much of it there is.
    pub async fn demo_status(&self, actor: &Actor) -> Result<SetupStatusDto> {
        actor.require(PERM_SYSTEM_DEMO_MANAGE)?;
        self.load_status().await
    }

    async fn load_status(&self) -> Result<SetupStatusDto> {
        let (business_count, user_count, demo, platform) = tokio::try_join!(
            self.business_repo.count_all(),
            self.user_repo.count_all(),
            self.demo_repo.count(),
            self.catalogue_repo.platform_counts(),
        )?;

        Ok(SetupStatusDto {
            database_connected: true,
            is_seeded: demo.any(),
            business_count,
            user_count,
            demo,
            platform,
        })
    }

    pub async fn load_demo(&self, actor: &Actor) -> Result<DemoReport> {
        actor.require(PERM_SYSTEM_DEMO_MANAGE)?;
        if self.demo_repo.count().await?.businesses > 0 {
            return Err(Error::new(ErrorCode::DemoAlreadyLoaded));
        }
        let report = self.demo_repo.load().await?;
        info!(
            categories = report.categories,
            locations = report.locations,
            crops = report.crops,
            businesses = report.businesses,
            services = report.services,
            products = report.products,
            "demo data loaded"
        );
        Ok(report)
    }

    /// The demo data at volume, for trying the pages and timing the reads
    /// on a database of realistic size. Run from the command line
    /// (`agri-connect demo-data`), never from a request: it writes for
    /// minutes. Refused while demo listings are present, as `load_demo` is,
    /// so a second run never doubles them (a demo catalogue already there
    /// is reused by name); every demo account signs in with `password`, which
    /// must pass the sign-up rules.
    pub async fn load_demo_volume(
        &self,
        volume: &DemoVolume,
        password: &str,
    ) -> Result<DemoReport> {
        validate_password(password)?;
        if self.demo_repo.count().await?.businesses > 0 {
            return Err(Error::new(ErrorCode::DemoAlreadyLoaded));
        }
        let hash = self.password_hasher.hash(password).await?;
        let report = self.demo_repo.load_volume(volume, &hash).await?;
        info!(
            providers = volume.providers,
            seed = volume.seed,
            businesses = report.businesses,
            accounts = report.accounts,
            services = report.services,
            products = report.products,
            inquiries = report.inquiries,
            reviews = report.reviews,
            "demo data at volume loaded"
        );
        Ok(report)
    }

    pub async fn remove_demo(&self, actor: &Actor) -> Result<u64> {
        actor.require(PERM_SYSTEM_DEMO_MANAGE)?;
        let removed = self.demo_repo.remove().await?;
        // Photos someone uploaded to a demo service or product go too; a
        // failure only orphans an object (logged), the rows are already gone.
        for file in &removed.image_files {
            self.media.discard(*file).await;
        }
        info!(
            businesses = removed.businesses,
            catalogue_removed = removed.catalogue_removed,
            catalogue_kept = removed.catalogue_kept,
            photos = removed.image_files.len(),
            "demo data removed"
        );
        Ok(removed.businesses)
    }
}
