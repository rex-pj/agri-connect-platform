use std::sync::Arc;

use domain::{
    models::{business::BusinessBrowse, inquiry::NewInquiry, product::ProductView},
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    constants::{
        FEATURED_PRODUCTS_COUNT, FEATURED_PROVIDERS_COUNT, NOTE_MAX_LEN, PAGE_SIZE, RADIUS_MAX_M,
        RADIUS_MIN_M, REQUESTER_NAME_MAX_LEN, TOPIC_MAX_LEN,
    },
    inputs::{BrowseParams, ContactInput, GeoSearchParams},
    outputs::{
        BrowseResponse, BusinessBrowseDto, BusinessDetailDto, BusinessSearchDto, CategoryDto,
        ExploreData, InquiryCreatedDto, LocationDto, ProductDto, ServiceDto,
    },
    pagination::{paginate, total_pages},
    validation::{non_blank, validate_coordinates, validate_phone},
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ExploreUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
}

impl ExploreUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
        inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
        location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
        product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            business_repo,
            category_repo,
            inquiry_repo,
            location_repo,
            product_repo,
        }
    }

    pub async fn search_providers(
        &self,
        params: GeoSearchParams,
    ) -> Result<Vec<BusinessSearchDto>> {
        validate_coordinates(params.lat, params.lng)?;
        if !(RADIUS_MIN_M..=RADIUS_MAX_M).contains(&params.radius) {
            return Err(Error::new(ErrorCode::RadiusOutOfRange)
                .with_param("min", RADIUS_MIN_M)
                .with_param("max", RADIUS_MAX_M));
        }

        let point_wkt = format!("SRID=4326;POINT({} {})", params.lng, params.lat);
        let results = self
            .business_repo
            .search_geo(&point_wkt, params.radius, params.service_id)
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
        let (page, offset) = paginate(params.page, PAGE_SIZE);

        let (providers, total) = tokio::try_join!(
            self.business_repo.browse(
                params.service_id,
                params.name.clone(),
                params.sort,
                PAGE_SIZE,
                offset
            ),
            self.business_repo
                .count_browse(params.service_id, params.name),
        )?;

        Ok(BrowseResponse {
            items: providers.into_iter().map(map_business_browse).collect(),
            total,
            page,
            per_page: PAGE_SIZE,
            total_pages: total_pages(total, PAGE_SIZE),
        })
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
                })
                .collect(),
        })
    }

    pub async fn get_explore_data(&self) -> Result<ExploreData> {
        let (featured_businesses, featured_product_views) = tokio::try_join!(
            self.business_repo.featured(FEATURED_PROVIDERS_COUNT),
            self.product_repo.featured(FEATURED_PRODUCTS_COUNT),
        )?;

        Ok(ExploreData {
            featured_providers: featured_businesses
                .into_iter()
                .map(map_business_browse)
                .collect(),
            featured_products: featured_product_views
                .into_iter()
                .map(map_product_dto)
                .collect(),
        })
    }

    /// The single home of the inquiry rules; every contact form posts here.
    pub async fn contact_request(&self, input: ContactInput) -> Result<InquiryCreatedDto> {
        let requester_name = input.requester_name.trim().to_owned();
        let requester_phone = input.requester_phone.trim().to_owned();
        let topic = input.topic.trim().to_owned();
        let note = non_blank(input.note);

        Validation::new()
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

        if !self.business_repo.check_exists(input.business_id).await? {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }

        let id = self
            .inquiry_repo
            .create(NewInquiry {
                business_id: input.business_id,
                service_id: input.service_id,
                topic,
                requester_name,
                requester_phone,
                note,
            })
            .await?;

        Ok(InquiryCreatedDto { id })
    }

    pub async fn get_categories(&self, type_filter: Option<&str>) -> Result<Vec<CategoryDto>> {
        let categories = self.category_repo.list(type_filter).await?;

        Ok(categories
            .into_iter()
            .map(|c| CategoryDto {
                id: c.id,
                name: c.name,
                slug: c.slug,
                icon: c.icon,
                color: c.color,
                description: c.description,
                display_order: c.display_order,
            })
            .collect())
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

fn map_product_dto(p: ProductView) -> ProductDto {
    ProductDto {
        id: p.id,
        name: p.name,
        description: p.description,
        price: p.price,
        unit: p.unit,
        category: p.category,
        vendor_name: p.vendor_name,
        vendor_phone: p.vendor_phone,
        is_available: p.is_available,
        created_on: p.created_on,
    }
}
