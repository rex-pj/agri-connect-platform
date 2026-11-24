//! Uploaded images: the pipeline from untrusted bytes to stored objects, and
//! reading them back for `/files/{id}`.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::{
        business::ListingEditor,
        file::{FileVariant, NewFile},
    },
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
    catalogue::{
        PERM_LISTING_MODERATE, PERM_OWN_LISTING_UPDATE, PERM_OWN_SERVICE_UPDATE,
        PERM_SERVICE_MANAGE,
    },
    hashing::sha256_hex,
    outputs::{ProviderProductDto, ProviderServiceDto},
    ports::{EventBus, FileStorage, ImageProcessor, ProcessedImage},
    validation::audit_reason,
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
fn object_keys(id: Uuid, content_type: &str) -> (String, String) {
    let ext = match content_type {
        "image/png" => "png",
        _ => "jpg",
    };
    (
        format!("images/{id}.{ext}"),
        format!("images/{id}-thumb.{ext}"),
    )
}

/// A photo uploaded for one listing but not yet shown on it: an edit form
/// stages its photo before saving anything (the upload is what fails on a
/// weak link or a storage outage), saves its boxes, then commits the photo
/// (`MediaUseCase::commit_staged`), or abandons it when the boxes were
/// refused (`abandon_staged`). A storage failure so leaves the listing as
/// it was, never with its new words and its old photo.
#[must_use = "commit or abandon a staged photo, or its file is left behind"]
pub struct StagedImage {
    file: Uuid,
    listing: Listing,
    editor: ListingEditor,
    /// The permission it was staged under, asked again on commit.
    permission: &'static str,
}

/// The two kinds of listing that carry one image.
#[derive(Debug, Clone, Copy)]
enum Listing {
    Product(i32),
    Service(i32),
}

impl Listing {
    fn id(self) -> i32 {
        match self {
            Self::Product(id) | Self::Service(id) => id,
        }
    }

    fn entity(self) -> Entity {
        match self {
            Self::Product(_) => Entity::Product,
            Self::Service(_) => Entity::Service,
        }
    }

    fn missing(self) -> ErrorCode {
        match self {
            Self::Product(_) => ErrorCode::ProductNotFound,
            Self::Service(_) => ErrorCode::ServiceNotFound,
        }
    }
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
        self.check_size(&upload)?;
        self.images.process(upload).await
    }

    /// `prepare_image` for the platform's logo or browser icon: a PNG square
    /// with its transparency (`ImageProcessor::process_icon`).
    pub async fn prepare_icon(&self, upload: Vec<u8>) -> Result<ProcessedImage> {
        self.check_size(&upload)?;
        self.images.process_icon(upload).await
    }

    fn check_size(&self, upload: &[u8]) -> Result<()> {
        if upload.is_empty() {
            return Err(Error::new(ErrorCode::ImageRequired));
        }
        if upload.len() > self.max_upload_bytes {
            return Err(file_too_large(self.max_upload_bytes));
        }
        Ok(())
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
        let editor = ListingEditor::Owner(actor.user_id);
        self.swap_image(actor, Listing::Product(product_id), editor, image)
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
        let editor = ListingEditor::Owner(actor.user_id);
        self.swap_image(actor, Listing::Service(service_id), editor, image)
            .await?;

        self.services
            .find_owned(service_id, actor.user_id)
            .await?
            .map(ProviderServiceDto::from)
            .ok_or_else(|| Error::internal("service image set but service could not be reloaded"))
    }

    /// Uploads the photo of one of the actor's own products, to show once
    /// its edit is saved (`StagedImage`).
    pub async fn stage_product_image(
        &self,
        actor: &Actor,
        product_id: i32,
        image: ProcessedImage,
    ) -> Result<StagedImage> {
        actor.require(PERM_OWN_LISTING_UPDATE)?;
        let editor = ListingEditor::Owner(actor.user_id);
        self.stage(
            actor,
            Listing::Product(product_id),
            editor,
            PERM_OWN_LISTING_UPDATE,
            image,
        )
        .await
    }

    /// The same for one of the actor's own services.
    pub async fn stage_service_image(
        &self,
        actor: &Actor,
        service_id: i32,
        image: ProcessedImage,
    ) -> Result<StagedImage> {
        actor.require(PERM_OWN_SERVICE_UPDATE)?;
        let editor = ListingEditor::Owner(actor.user_id);
        self.stage(
            actor,
            Listing::Service(service_id),
            editor,
            PERM_OWN_SERVICE_UPDATE,
            image,
        )
        .await
    }

    /// The same for a moderator's photo of any product.
    pub async fn stage_product_image_admin(
        &self,
        actor: &Actor,
        product_id: i32,
        image: ProcessedImage,
    ) -> Result<StagedImage> {
        actor.require(PERM_LISTING_MODERATE)?;
        let editor = ListingEditor::Moderator(actor.user_id);
        self.stage(
            actor,
            Listing::Product(product_id),
            editor,
            PERM_LISTING_MODERATE,
            image,
        )
        .await
    }

    /// The same for a moderator's photo of any service.
    pub async fn stage_service_image_admin(
        &self,
        actor: &Actor,
        service_id: i32,
        image: ProcessedImage,
    ) -> Result<StagedImage> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let editor = ListingEditor::Moderator(actor.user_id);
        self.stage(
            actor,
            Listing::Service(service_id),
            editor,
            PERM_SERVICE_MANAGE,
            image,
        )
        .await
    }

    async fn stage(
        &self,
        actor: &Actor,
        listing: Listing,
        editor: ListingEditor,
        permission: &'static str,
        image: ProcessedImage,
    ) -> Result<StagedImage> {
        Ok(StagedImage {
            file: self.store(actor, image).await?,
            listing,
            editor,
            permission,
        })
    }

    /// Shows a staged photo on its listing, once the edit it came with is
    /// saved. Refused (the listing gone, or no longer the editor's), the
    /// file is discarded.
    pub async fn commit_staged(&self, actor: &Actor, staged: StagedImage) -> Result<()> {
        actor.require(staged.permission)?;
        self.point_at(actor, staged.listing, staged.editor, staged.file)
            .await
    }

    /// Deletes a staged photo whose edit was refused.
    pub async fn abandon_staged(&self, staged: StagedImage) {
        self.discard(staged.file).await;
    }

    /// A moderator takes a product's photo off, the product staying listed
    /// (audited with `reason`; the stored image goes to the cleanup
    /// subscriber).
    pub async fn remove_product_image(
        &self,
        actor: &Actor,
        product_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_LISTING_MODERATE)?;
        self.remove_image(actor, Listing::Product(product_id), reason)
            .await
    }

    /// A moderator takes a service's photo off, as for a product.
    pub async fn remove_service_image(
        &self,
        actor: &Actor,
        service_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_SERVICE_MANAGE)?;
        self.remove_image(actor, Listing::Service(service_id), reason)
            .await
    }

    /// Points `listing` at `file` (`None`: no image) if `editor` may change
    /// it: the image it had before, or `None` when it may not (missing,
    /// deleted, someone else's).
    async fn set_image(
        &self,
        listing: Listing,
        editor: ListingEditor,
        file: Option<Uuid>,
    ) -> Result<Option<Option<Uuid>>> {
        match listing {
            Listing::Product(id) => self.products.set_image(id, editor, file).await,
            Listing::Service(id) => self.services.set_image(id, editor, file).await,
        }
    }

    /// Stores the image, points the listing at it (refused when `editor` may
    /// not change it, e.g. deleted since the ownership check), and announces
    /// the replaced one for cleanup. A refused or failed write discards what
    /// was stored.
    async fn swap_image(
        &self,
        actor: &Actor,
        listing: Listing,
        editor: ListingEditor,
        image: ProcessedImage,
    ) -> Result<()> {
        let file = self.store(actor, image).await?;
        self.point_at(actor, listing, editor, file).await
    }

    /// Points the listing at a stored file and announces the replaced one
    /// for cleanup; a refused or failed write discards the file.
    async fn point_at(
        &self,
        actor: &Actor,
        listing: Listing,
        editor: ListingEditor,
        file: Uuid,
    ) -> Result<()> {
        let previous = match self.set_image(listing, editor, Some(file)).await {
            Ok(Some(previous)) => previous,
            Ok(None) => {
                self.discard(file).await;
                return Err(Error::new(listing.missing()));
            }
            Err(e) => {
                self.discard(file).await;
                return Err(e);
            }
        };
        self.announce(actor, listing, Change::ImageReplaced { previous }, None)
            .await;
        Ok(())
    }

    /// Takes the image off a listing as a moderator; nothing to announce when
    /// it had none.
    async fn remove_image(
        &self,
        actor: &Actor,
        listing: Listing,
        reason: Option<String>,
    ) -> Result<()> {
        let editor = ListingEditor::Moderator(actor.user_id);
        let previous = self
            .set_image(listing, editor, None)
            .await?
            .ok_or_else(|| Error::new(listing.missing()))?;
        if let Some(previous) = previous {
            self.announce(actor, listing, Change::ImageRemoved { previous }, reason)
                .await;
        }
        Ok(())
    }

    async fn announce(
        &self,
        actor: &Actor,
        listing: Listing,
        change: Change,
        reason: Option<String>,
    ) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: listing.entity(),
                entity_id: listing.id(),
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// Stores both objects, then the row. A failure after an object was
    /// written deletes what was written, so nothing is left without a row.
    /// For a use case that saves the image together with its owner; it
    /// checks who is asking and calls `discard` if that save fails.
    pub(crate) async fn store(&self, actor: &Actor, image: ProcessedImage) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let (storage_key, thumb_key) = object_keys(id, image.content_type);
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
            object_keys(id, "image/jpeg"),
            (
                "images/00000000-0000-0000-0000-000000000000.jpg".to_owned(),
                "images/00000000-0000-0000-0000-000000000000-thumb.jpg".to_owned()
            )
        );
        assert_eq!(
            object_keys(id, "image/png").1,
            "images/00000000-0000-0000-0000-000000000000-thumb.png"
        );
    }

    #[test]
    fn the_size_message_rounds_the_cap_up_to_whole_megabytes() {
        let e = file_too_large(5 * MEBIBYTE);
        assert_eq!(e.code(), ErrorCode::FileTooLarge);
        assert_eq!(e.params(), [("max_mb", "5".to_owned())]);
    }
}
