//! Uploaded images: the pipeline from untrusted bytes to stored objects, and
//! reading them back for `/files/{id}`.

use std::{future::Future, sync::Arc};

use domain::{
    events::{Change, DomainEvent, Entity},
    models::file::{FileVariant, NewFile},
    repositories::{
        file_repository_trait::FileRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
    },
    Error, ErrorCode, Result,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    authz::Actor,
    catalogue::{PERM_OWN_LISTING_UPDATE, PERM_OWN_SERVICE_UPDATE},
    hashing::sha256_hex,
    outputs::{ProviderProductDto, ProviderServiceDto},
    ports::{EventBus, FileStorage, ImageProcessor, ProcessedImage},
};

/// A stored object and the type to serve it with.
pub struct FileContent {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// Everything `MediaUseCase` depends on, named so wiring cannot swap two of
/// compatible types by position.
pub struct MediaDeps {
    pub products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub services: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    pub files: Arc<dyn FileRepositoryTrait + Send + Sync>,
    pub storage: Arc<dyn FileStorage>,
    pub images: Arc<dyn ImageProcessor>,
    pub events: Arc<dyn EventBus>,
    /// UPLOAD_MAX_BYTES. The router enforces it on the body too; this check
    /// keeps the rule true for callers that are not HTTP.
    pub max_upload_bytes: usize,
}

#[derive(Clone)]
pub struct MediaUseCase {
    products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    services: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    files: Arc<dyn FileRepositoryTrait + Send + Sync>,
    storage: Arc<dyn FileStorage>,
    images: Arc<dyn ImageProcessor>,
    events: Arc<dyn EventBus>,
    max_upload_bytes: usize,
}

const MEBIBYTE: usize = 1024 * 1024;

/// The error for an upload over the cap, with the cap in the message.
pub fn file_too_large(max_upload_bytes: usize) -> Error {
    Error::new(ErrorCode::FileTooLarge).with_param("max_mb", max_upload_bytes.div_ceil(MEBIBYTE))
}

/// Keys are derived from the id, so two uploads can never collide and no
/// user input reaches a storage path.
fn object_keys(id: Uuid) -> (String, String) {
    (format!("images/{id}.jpg"), format!("images/{id}-thumb.jpg"))
}

impl MediaUseCase {
    pub fn new(d: MediaDeps) -> Self {
        Self {
            products: d.products,
            services: d.services,
            files: d.files,
            storage: d.storage,
            images: d.images,
            events: d.events,
            max_upload_bytes: d.max_upload_bytes,
        }
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_bytes
    }

    /// Replaces the image of one of the actor's own products. The old image is
    /// removed by the cleanup subscriber once the product no longer points at it.
    pub async fn store_product_image(
        &self,
        actor: &Actor,
        product_id: i32,
        upload: Vec<u8>,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_UPDATE)?;
        // Ownership before the CPU work, so nobody can make us decode images
        // for products that are not theirs.
        let owned = self
            .products
            .list_by_user(actor.user_id)
            .await?
            .iter()
            .any(|p| p.id == product_id);
        if !owned {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }

        let image = self.prepare_image(upload).await?;
        self.attach_product_image(actor, product_id, image).await
    }

    /// Checks and re-encodes an upload without storing anything: type sniffed
    /// from the bytes, size and pixel caps, EXIF dropped. A form calls it
    /// before saving anything else, so a bad photo refuses the whole form
    /// instead of leaving a product half-saved. Callers check who is asking
    /// (and, for an existing product, that it is theirs) first.
    pub async fn prepare_image(&self, upload: Vec<u8>) -> Result<ProcessedImage> {
        if upload.is_empty() {
            return Err(Error::new(ErrorCode::ImageRequired));
        }
        if upload.len() > self.max_upload_bytes {
            return Err(file_too_large(self.max_upload_bytes));
        }
        self.images.process(upload).await
    }

    /// Stores a prepared image and makes it the image of one of the actor's
    /// own products.
    pub async fn attach_product_image(
        &self,
        actor: &Actor,
        product_id: i32,
        image: ProcessedImage,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_UPDATE)?;
        self.swap_image(
            actor,
            Entity::Product,
            product_id,
            image,
            ErrorCode::ProductNotFound,
            |file| self.products.set_image(product_id, actor.user_id, file),
        )
        .await?;

        self.products
            .find_provider_product_by_id(product_id)
            .await?
            .map(ProviderProductDto::from)
            .ok_or_else(|| Error::internal("product image set but product could not be reloaded"))
    }

    /// Replaces the image of one of the actor's own services. The old image
    /// is removed by the cleanup subscriber.
    pub async fn store_service_image(
        &self,
        actor: &Actor,
        service_id: i32,
        upload: Vec<u8>,
    ) -> Result<ProviderServiceDto> {
        actor.require(PERM_OWN_SERVICE_UPDATE)?;
        // Ownership before the CPU work, as for products.
        if self
            .services
            .find_owned(service_id, actor.user_id)
            .await?
            .is_none()
        {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }

        let image = self.prepare_image(upload).await?;
        self.attach_service_image(actor, service_id, image).await
    }

    /// Stores a prepared image and makes it the image of one of the actor's
    /// own services.
    pub async fn attach_service_image(
        &self,
        actor: &Actor,
        service_id: i32,
        image: ProcessedImage,
    ) -> Result<ProviderServiceDto> {
        actor.require(PERM_OWN_SERVICE_UPDATE)?;
        self.swap_image(
            actor,
            Entity::Service,
            service_id,
            image,
            ErrorCode::ServiceNotFound,
            |file| {
                self.services
                    .set_image_owned(service_id, actor.user_id, file)
            },
        )
        .await?;

        self.services
            .find_owned(service_id, actor.user_id)
            .await?
            .map(ProviderServiceDto::from)
            .ok_or_else(|| Error::internal("service image set but service could not be reloaded"))
    }

    /// Stores the image, points the record at it with `set` (which answers
    /// `None` when the record is not the actor's, e.g. deleted since the
    /// ownership check), and announces the replaced one for cleanup. A
    /// refused or failed `set` discards what was stored.
    async fn swap_image<F, Fut>(
        &self,
        actor: &Actor,
        entity: Entity,
        entity_id: i32,
        image: ProcessedImage,
        missing: ErrorCode,
        set: F,
    ) -> Result<()>
    where
        F: FnOnce(Uuid) -> Fut,
        Fut: Future<Output = Result<Option<Option<Uuid>>>>,
    {
        let file = self.store(actor, image).await?;
        let previous = match set(file).await {
            Ok(Some(previous)) => previous,
            Ok(None) => {
                self.discard(file).await;
                return Err(Error::new(missing));
            }
            Err(e) => {
                self.discard(file).await;
                return Err(e);
            }
        };
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity,
                entity_id,
                change: Change::ImageReplaced { previous },
                reason: None,
            })
            .await;
        Ok(())
    }

    /// Stores both objects, then the row. A failure after an object was
    /// written deletes what was written, so nothing is left without a row.
    /// For a use case that saves the image together with its owner; it
    /// checks who is asking and calls `discard` if that save fails.
    pub(crate) async fn store(&self, actor: &Actor, image: ProcessedImage) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let (storage_key, thumb_key) = object_keys(id);
        let row = NewFile {
            id,
            storage_key: storage_key.clone(),
            thumb_key: thumb_key.clone(),
            content_type: image.content_type.to_owned(),
            byte_size: i64::try_from(image.main.bytes.len())
                .map_err(|_| Error::internal("image size overflows i64"))?,
            width: i32::try_from(image.main.width)
                .map_err(|_| Error::internal("image width overflows i32"))?,
            height: i32::try_from(image.main.height)
                .map_err(|_| Error::internal("image height overflows i32"))?,
            sha256: sha256_hex(&image.main.bytes),
            uploaded_by: actor.user_id,
        };

        self.storage.put(&storage_key, image.main.bytes).await?;
        if let Err(e) = self.storage.put(&thumb_key, image.thumb.bytes).await {
            self.delete_objects(&[&storage_key]).await;
            return Err(e);
        }
        if let Err(e) = self.files.insert(&row).await {
            self.delete_objects(&[&storage_key, &thumb_key]).await;
            return Err(e);
        }
        Ok(id)
    }

    pub(crate) async fn discard(&self, id: Uuid) {
        if let Err(e) = release_file(self.files.as_ref(), self.storage.as_ref(), id).await {
            warn!(file = %id, error = %e, "could not discard an unused upload");
        }
    }

    async fn delete_objects(&self, keys: &[&str]) {
        for key in keys {
            if let Err(e) = self.storage.delete(key).await {
                warn!(key, error = %e, "could not delete an orphaned object");
            }
        }
    }

    /// Public: product and service images are shown to everyone, and ids are
    /// unguessable.
    pub async fn file(&self, id: Uuid, variant: FileVariant) -> Result<FileContent> {
        let file = self
            .files
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::FileNotFound))?;
        let bytes = self.storage.get(file.key(variant)).await?;
        Ok(FileContent {
            bytes,
            content_type: file.content_type,
        })
    }
}

/// Deletes the row first, so the URL stops working at once, then both objects.
/// An object that fails to delete is orphaned but unreachable.
pub(crate) async fn release_file(
    files: &(dyn FileRepositoryTrait + Send + Sync),
    storage: &dyn FileStorage,
    id: Uuid,
) -> Result<()> {
    let Some(file) = files.delete(id).await? else {
        return Ok(());
    };
    for variant in [FileVariant::Main, FileVariant::Thumb] {
        storage.delete(file.key(variant)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_come_from_the_id_only() {
        let id = Uuid::nil();
        assert_eq!(
            object_keys(id),
            (
                "images/00000000-0000-0000-0000-000000000000.jpg".to_owned(),
                "images/00000000-0000-0000-0000-000000000000-thumb.jpg".to_owned()
            )
        );
    }

    #[test]
    fn the_size_message_rounds_the_cap_up_to_whole_megabytes() {
        let e = file_too_large(5 * MEBIBYTE);
        assert_eq!(e.code(), ErrorCode::FileTooLarge);
        assert_eq!(e.params(), [("max_mb", "5".to_owned())]);
    }
}
