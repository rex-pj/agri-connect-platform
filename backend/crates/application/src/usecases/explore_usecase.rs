use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::Utc;

use domain::{
    constants::{
        CALL_CHANNELS, CALL_CHANNEL_PHONE, CALL_KIND_PRODUCT, CALL_KIND_PROVIDER, CALL_KIND_SERVICE,
    },
    events::{Change, DomainEvent, Entity},
    models::{
        business::BusinessBrowse,
        call::NewCallClick,
        inquiry::NewInquiry,
        service::{PublicService, PublicServiceFilter},
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        call_repository_trait::CallRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    constants::{
        CALL_REPEAT_SECS, FEATURED_PRODUCTS_COUNT, FEATURED_PROVIDERS_COUNT,
        FEATURED_SERVICES_COUNT, GEO_RESULTS_MAX, NOTE_MAX_LEN, PAGE_SIZE, RADIUS_MAX_M,
        RADIUS_MIN_M, REQUESTER_NAME_MAX_LEN, SERVICES_PER_BUSINESS_MAX, TOPIC_MAX_LEN,
    },
    hashing::sha256_hex,
    inputs::{BrowseParams, CallInput, ContactInput, GeoSearchParams, Visitor},
    outputs::{
        BrowseResponse, BusinessBrowseDto, BusinessDetailDto, BusinessSearchDto, CategoryDto,
        ExploreData, InquiryCreatedDto, LocationDto, PagedResponse, ProductDto, PublicServiceDto,
        ServiceBrowseResponse, ServiceDto,
    },
    pagination::{paginate_public, public_total_pages, total_or_count},
    ports::EventBus,
    repeats::RepeatFilter,
    validation::{non_blank, point_wkt, search_text, validate_coordinates, validate_phone},
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ExploreUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    /// Announces new inquiries (the provider is emailed).
    events: Arc<dyn EventBus>,
    /// Call and Zalo taps counted lately: a repeat within `CALL_REPEAT_SECS`
    /// is not another lead.
    recent_taps: RepeatFilter,
}

/// Everything `ExploreUseCase` depends on, named so wiring cannot swap two
/// repos of compatible types by position.
pub struct ExploreDeps {
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    pub call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    pub events: Arc<dyn EventBus>,
}

impl ExploreUseCase {
    pub fn new(d: ExploreDeps) -> Self {
        Self {
            business_repo: d.business_repo,
            category_repo: d.category_repo,
            inquiry_repo: d.inquiry_repo,
            location_repo: d.location_repo,
            product_repo: d.product_repo,
            service_repo: d.service_repo,
            call_repo: d.call_repo,
            events: d.events,
            recent_taps: RepeatFilter::default(),
        }
    }

    /// The nearest providers, at most `GEO_RESULTS_MAX` (the JSON API).
    pub async fn search_providers(
        &self,
        params: GeoSearchParams,
    ) -> Result<Vec<BusinessSearchDto>> {
        let point_wkt = geo_point(&params)?;
        self.providers_near(&point_wkt, &params, GEO_RESULTS_MAX, 0)
            .await
    }

    /// One page of the providers near a point, nearest first (the near-me
    /// page's shops tab).
    pub async fn search_providers_page(
        &self,
        params: GeoSearchParams,
        page: i64,
    ) -> Result<PagedResponse<BusinessSearchDto>> {
        let point_wkt = geo_point(&params)?;
        let (page, offset) = paginate_public(page);
        let items = self
            .providers_near(&point_wkt, &params, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), PAGE_SIZE, || {
            self.business_repo
                .count_geo(&point_wkt, params.radius, params.service_id)
        })
        .await?;
        Ok(PagedResponse::public(items, total, page))
    }

    async fn providers_near(
        &self,
        point_wkt: &str,
        params: &GeoSearchParams,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessSearchDto>> {
        let results = self
            .business_repo
            .search_geo(point_wkt, params.radius, params.service_id, limit, offset)
            .await?;

        Ok(results
            .into_iter()
            .map(|b| BusinessSearchDto {
                id: b.id,
                name: b.name,
                phone: b.phone,
                address: b.address,
                is_verified: b.is_verified,
                distance_m: b.distance_m,
                services: b.services,
            })
            .collect())
    }

    pub async fn browse_providers(&self, params: BrowseParams) -> Result<BrowseResponse> {
        let (page, offset) = paginate_public(params.page);

        let filter = PublicServiceFilter {
            category_id: params.service_id,
            sector: params.sector,
            text: search_text(params.name)?,
        };
        let providers = self
            .business_repo
            .browse(&filter, params.sort, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, providers.len(), PAGE_SIZE, || {
            self.business_repo.count_browse(&filter)
        })
        .await?;

        Ok(BrowseResponse {
            items: providers.into_iter().map(map_business_browse).collect(),
            total,
            page,
            per_page: PAGE_SIZE,
            total_pages: public_total_pages(total),
        })
    }

    // ── Services as farmers see them ─────────────────────────────────────────

    /// One page of the service listing; `service_id` in the params is the
    /// category, `name` the typed text (service name, description, business).
    pub async fn browse_services(&self, params: BrowseParams) -> Result<ServiceBrowseResponse> {
        let (page, offset) = paginate_public(params.page);
        let filter = PublicServiceFilter {
            category_id: params.service_id,
            sector: params.sector,
            text: search_text(params.name)?,
        };
        let items = self
            .service_repo
            .browse_public(&filter, params.sort, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), PAGE_SIZE, || {
            self.service_repo.count_public(&filter)
        })
        .await?;
        Ok(ServiceBrowseResponse {
            items: items.into_iter().map(PublicServiceDto::from).collect(),
            total,
            page,
            per_page: PAGE_SIZE,
            total_pages: public_total_pages(total),
        })
    }

    /// Services whose business is within the radius, nearest first, at most
    /// `GEO_RESULTS_MAX` (the JSON API).
    pub async fn search_services(&self, params: GeoSearchParams) -> Result<Vec<PublicServiceDto>> {
        let point_wkt = geo_point(&params)?;
        self.services_near(&point_wkt, &params, GEO_RESULTS_MAX, 0)
            .await
    }

    /// One page of the services near a point, nearest first (the near-me
    /// page's services tab).
    pub async fn search_services_page(
        &self,
        params: GeoSearchParams,
        page: i64,
    ) -> Result<PagedResponse<PublicServiceDto>> {
        let point_wkt = geo_point(&params)?;
        let (page, offset) = paginate_public(page);
        let items = self
            .services_near(&point_wkt, &params, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), PAGE_SIZE, || {
            self.service_repo
                .count_geo_public(&point_wkt, params.radius, params.service_id)
        })
        .await?;
        Ok(PagedResponse::public(items, total, page))
    }

    async fn services_near(
        &self,
        point_wkt: &str,
        params: &GeoSearchParams,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PublicServiceDto>> {
        let results = self
            .service_repo
            .search_geo_public(point_wkt, params.radius, params.service_id, limit, offset)
            .await?;
        Ok(results.into_iter().map(PublicServiceDto::from).collect())
    }

    pub async fn get_service(&self, id: i32) -> Result<PublicServiceDto> {
        self.service_repo
            .find_public(id)
            .await?
            .map(PublicServiceDto::from)
            .ok_or_else(|| Error::new(ErrorCode::ServiceNotFound))
    }

    /// Records a tap on a public call button. The business is looked up from
    /// the service, product or provider the button was on, so a request can
    /// only count a call for something farmers can actually see.
    pub async fn record_call(&self, input: CallInput) -> Result<()> {
        let channel = input
            .channel
            .unwrap_or_else(|| CALL_CHANNEL_PHONE.to_owned());
        if !CALL_CHANNELS.contains(&channel.as_str()) {
            return Err(Error::new(ErrorCode::InvalidCallTarget));
        }
        let (business_id, service_id, product_id) = match input.kind.as_str() {
            CALL_KIND_SERVICE => {
                let service = self
                    .service_repo
                    .find_public(input.id)
                    .await?
                    .ok_or_else(|| Error::new(ErrorCode::ServiceNotFound))?;
                (service.business_id, Some(service.id), None)
            }
            CALL_KIND_PRODUCT => {
                let product = self
                    .product_repo
                    .find_by_id(input.id)
                    .await?
                    .ok_or_else(|| Error::new(ErrorCode::ProductNotFound))?;
                (product.business_id, None, Some(product.id))
            }
            CALL_KIND_PROVIDER => {
                let provider = self
                    .business_repo
                    .find_detail(input.id)
                    .await?
                    .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;
                (provider.id, None, None)
            }
            _ => return Err(Error::new(ErrorCode::InvalidCallTarget)),
        };
        // An admin, provider or partner trying a button is not demand.
        let Visitor::Anonymous { key } = &input.visitor else {
            return Ok(());
        };
        // The same visitor ringing the same business again is the same lead.
        // A visitor with no address to tell apart is always counted.
        if !key.is_empty() {
            let fingerprint = sha256_hex(format!("{key}|{business_id}|{channel}"));
            let window = Duration::from_secs(CALL_REPEAT_SECS);
            if !self.recent_taps.first_in_window(fingerprint, window) {
                return Ok(());
            }
        }
        self.call_repo
            .record(NewCallClick {
                business_id,
                service_id,
                product_id,
                channel,
            })
            .await
    }

    pub async fn get_provider_detail(&self, id: i32) -> Result<BusinessDetailDto> {
        let detail = self
            .business_repo
            .find_detail(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;

        Ok(BusinessDetailDto {
            id: detail.id,
            name: detail.name,
            phone: detail.phone,
            description: detail.description,
            address: detail.address,
            is_verified: detail.is_verified,
            services: detail
                .services
                .into_iter()
                .map(|s| ServiceDto {
                    id: s.id,
                    name: s.name,
                    description: s.description,
                    price_note: s.price_note,
                    category: s.category,
                    category_icon: s.category_icon,
                    image_file_id: s.image_file_id,
                })
                .collect(),
        })
    }

    /// The featured services, providers and products; `sector` keeps those of
    /// that sector and of every sector, as the lists do.
    pub async fn get_explore_data(&self, sector: Option<&str>) -> Result<ExploreData> {
        // Enough candidates that even providers listing the most services
        // cannot crowd out the others before one per provider is kept.
        let candidates = FEATURED_SERVICES_COUNT as i64 * SERVICES_PER_BUSINESS_MAX as i64;
        let (featured_businesses, featured_product_views, service_candidates) = tokio::try_join!(
            self.business_repo
                .featured(FEATURED_PROVIDERS_COUNT, sector),
            self.product_repo.featured(FEATURED_PRODUCTS_COUNT, sector),
            self.service_repo.featured_candidates(candidates, sector),
        )?;

        Ok(ExploreData {
            featured_services: pick_featured(service_candidates, FEATURED_SERVICES_COUNT, today())
                .into_iter()
                .map(PublicServiceDto::from)
                .collect(),
            featured_providers: featured_businesses
                .into_iter()
                .map(map_business_browse)
                .collect(),
            featured_products: featured_product_views
                .into_iter()
                .map(ProductDto::from)
                .collect(),
        })
    }

    /// One page of the live products whose seller is within the radius,
    /// nearest first (the near-me page's supplies tab). Products have no
    /// service category, so `service_id` in the params is not used.
    pub async fn search_products_page(
        &self,
        params: GeoSearchParams,
        page: i64,
    ) -> Result<PagedResponse<ProductDto>> {
        let point_wkt = geo_point(&params)?;
        let (page, offset) = paginate_public(page);
        let rows = self
            .product_repo
            .search_geo(&point_wkt, params.radius, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, rows.len(), PAGE_SIZE, || {
            self.product_repo.count_geo(&point_wkt, params.radius)
        })
        .await?;
        let items = rows.into_iter().map(ProductDto::from).collect();
        Ok(PagedResponse::public(items, total, page))
    }

    /// The single home of the inquiry rules; every contact form posts here.
    /// A chosen service must be one the provider shows publicly; its name is
    /// the topic when none was typed. A request sent from a supply's page is
    /// stored as about that supply.
    pub async fn contact_request(&self, input: ContactInput) -> Result<InquiryCreatedDto> {
        let requester_name = input.requester_name.trim().to_owned();
        let requester_phone = input.requester_phone.trim().to_owned();
        let note = non_blank(input.note);

        let provider = self.business_repo.find_detail(input.business_id).await?;
        let service = input.service_id.map(|id| {
            provider
                .as_ref()
                .and_then(|p| p.services.iter().find(|s| s.id == id))
        });
        let topic = match (input.topic.trim(), service.flatten()) {
            ("", Some(s)) => s.name.clone(),
            (typed, _) => typed.to_owned(),
        };

        Validation::new()
            .check(
                provider.is_some() && service.is_some_and(|s| s.is_none()),
                "service_id",
                ErrorCode::ServiceNotOffered,
            )
            .check(
                requester_name.is_empty(),
                "requester_name",
                ErrorCode::RequesterNameRequired,
            )
            .check_with(
                requester_name.chars().count() > REQUESTER_NAME_MAX_LEN,
                "requester_name",
                ErrorCode::RequesterNameTooLong,
                ("name_max", REQUESTER_NAME_MAX_LEN),
            )
            .rule("requester_phone", validate_phone(&requester_phone))
            .check(topic.is_empty(), "topic", ErrorCode::TopicRequired)
            .check_with(
                topic.chars().count() > TOPIC_MAX_LEN,
                "topic",
                ErrorCode::TopicTooLong,
                ("topic_max", TOPIC_MAX_LEN),
            )
            .check_with(
                note.as_deref()
                    .is_some_and(|n| n.chars().count() > NOTE_MAX_LEN),
                "note",
                ErrorCode::NoteTooLong,
                ("note_max", NOTE_MAX_LEN),
            )
            .finish()?;

        if provider.is_none() {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }

        // The supply the form was on, when it is still this provider's. The
        // farmer chose no supply in the form, so one that is gone since is
        // left out, never an error; a chosen service is what the request is
        // about.
        let product_id = match input.product_id.filter(|_| input.service_id.is_none()) {
            Some(id) => self
                .product_repo
                .find_by_id(id)
                .await?
                .filter(|p| p.business_id == input.business_id)
                .map(|p| p.id),
            None => None,
        };

        let id = self
            .inquiry_repo
            .create(NewInquiry {
                business_id: input.business_id,
                service_id: input.service_id,
                product_id,
                topic,
                requester_name,
                requester_phone,
                note,
            })
            .await?;

        // No actor: the farmer is not signed in.
        self.events
            .publish(DomainEvent {
                actor_id: None,
                entity: Entity::Inquiry,
                entity_id: id,
                change: Change::Created,
                reason: None,
            })
            .await;

        Ok(InquiryCreatedDto { id })
    }

    pub async fn get_categories(&self, type_filter: Option<&str>) -> Result<Vec<CategoryDto>> {
        let categories = self.category_repo.list(type_filter).await?;

        Ok(categories.into_iter().map(CategoryDto::from).collect())
    }

    /// Forgets the taps whose repeat window has passed (bounded memory).
    pub fn forget_old_taps(&self) {
        self.recent_taps
            .forget_older_than(Duration::from_secs(CALL_REPEAT_SECS));
    }

    pub async fn get_locations(&self) -> Result<Vec<LocationDto>> {
        let locations = self.location_repo.list_active().await?;

        Ok(locations
            .into_iter()
            .map(|l| LocationDto {
                id: l.id,
                label: l.label,
                lat: l.lat,
                lng: l.lng,
            })
            .collect())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn map_business_browse(b: BusinessBrowse) -> BusinessBrowseDto {
    BusinessBrowseDto {
        id: b.id,
        name: b.name,
        phone: b.phone,
        address: b.address,
        is_verified: b.is_verified,
        services: b.services,
        created_on: b.created_on,
    }
}

/// The search point of a near-me query, once its coordinates and radius pass.
fn geo_point(params: &GeoSearchParams) -> Result<String> {
    validate_coordinates(params.lat, params.lng)?;
    if !(RADIUS_MIN_M..=RADIUS_MAX_M).contains(&params.radius) {
        return Err(Error::new(ErrorCode::RadiusOutOfRange)
            .with_param("min", RADIUS_MIN_M)
            .with_param("max", RADIUS_MAX_M));
    }
    Ok(point_wkt(params.lat, params.lng))
}

/// The featured services, at most `count`, chosen from `candidates` (photo
/// first, then newest):
/// - one per provider (its first candidate): a provider listing many
///   services takes one place, not all;
/// - providers take turns: the order is rotated by `day`, so every verified
///   provider reaches the home page in its turn, not only the newest few;
///   those with a photo still come before those without;
/// - every category once before any repeats, so six spraying crews never
///   hide the only dredger.
fn pick_featured(candidates: Vec<PublicService>, count: usize, day: i64) -> Vec<PublicService> {
    let mut seen = HashSet::new();
    let (mut with_photo, mut without): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .filter(|s| seen.insert(s.business_id))
        .partition(|s| s.image_file_id.is_some());
    for group in [&mut with_photo, &mut without] {
        if !group.is_empty() {
            let shift = day.rem_euclid(group.len() as i64) as usize;
            group.rotate_left(shift);
        }
    }
    with_photo.append(&mut without);

    let mut categories = HashSet::new();
    let (mut picked, repeats): (Vec<_>, Vec<_>) = with_photo
        .into_iter()
        .partition(|s| categories.insert(s.category_id));
    picked.extend(repeats);
    picked.truncate(count);
    picked
}

/// Whole days since the Unix epoch: the turn the featured providers are in.
fn today() -> i64 {
    Utc::now().timestamp().div_euclid(SECONDS_PER_DAY)
}

const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

#[cfg(test)]
mod tests {
    use super::*;

    fn service(id: i32, business_id: i32, category_id: i32, photo: bool) -> PublicService {
        PublicService {
            id,
            name: format!("Dịch vụ {id}"),
            description: None,
            price_note: None,
            category_id,
            category: String::new(),
            category_slug: String::new(),
            category_icon: None,
            image_file_id: photo.then(uuid::Uuid::new_v4),
            business_id,
            business_name: String::new(),
            business_phone: String::new(),
            business_address: None,
            business_verified: true,
            created_on: Utc::now(),
            distance_m: None,
        }
    }

    fn ids(services: &[PublicService]) -> Vec<i32> {
        services.iter().map(|s| s.id).collect()
    }

    #[test]
    fn one_place_per_provider() {
        let picked = pick_featured(
            vec![
                service(1, 10, 1, true),
                service(2, 10, 2, true),
                service(3, 20, 3, true),
            ],
            6,
            0,
        );
        assert_eq!(ids(&picked), [1, 3]);
    }

    #[test]
    fn providers_take_turns_day_by_day_photos_first() {
        let candidates = || {
            vec![
                service(1, 10, 1, true),
                service(2, 20, 2, true),
                service(3, 30, 3, true),
                service(4, 40, 4, false),
            ]
        };
        assert_eq!(ids(&pick_featured(candidates(), 2, 0)), [1, 2]);
        assert_eq!(ids(&pick_featured(candidates(), 2, 1)), [2, 3]);
        assert_eq!(ids(&pick_featured(candidates(), 2, 2)), [3, 1]);
        // Without a photo: after every provider with one.
        assert_eq!(ids(&pick_featured(candidates(), 6, 1)), [2, 3, 1, 4]);
    }

    #[test]
    fn every_category_before_a_repeat() {
        let picked = pick_featured(
            vec![
                service(1, 10, 1, true),
                service(2, 20, 1, true),
                service(3, 30, 2, true),
            ],
            2,
            0,
        );
        assert_eq!(ids(&picked), [1, 3], "the second spraying crew waits");
    }
}
