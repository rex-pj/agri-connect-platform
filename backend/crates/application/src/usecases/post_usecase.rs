//! Articles: the news (what happens now) and the guides (what stays true),
//! written by an admin in Markdown (`content.post.manage`), for one sector
//! or all, one province or the whole region, maybe about one commodity.
//!
//! A post is made a draft, then published (it keeps that first day, also
//! when hidden and shown again) or hidden. Farmers read the published ones
//! while the articles feature is open (`features::Feature::Articles`, the
//! web gate). Its cover photo is stored before the post is saved and
//! discarded if the save fails; a replaced one goes to the file cleanup.
//! Nothing is deleted: a post is hidden.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::{
        admin_list::AdminFilter,
        post::{Post, PostFields, PostQuery},
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        coded_repository_trait::CodedRepositoryTrait,
        commodity_repository_trait::CommodityRepositoryTrait,
        post_repository_trait::PostRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_CONTENT_POST_MANAGE,
    constants::{ADMIN_PAGE_SIZE, PAGE_SIZE, POST_BODY_MAX, POST_SUMMARY_MAX, POST_TITLE_MAX},
    inputs::PostInput,
    outputs::{
        AdminPagedResponse, NamedCodeDto, NamedIdDto, PagedResponse, PostChoicesDto, PostDto,
    },
    pagination::{paginate_admin, paginate_public, total_or_count},
    ports::{EventBus, ProcessedImage},
    posts::{POST_KINDS, POST_STATUSES, STATUS_PUBLISHED},
    usecases::media_usecase::MediaUseCase,
    validation::{self, audit_reason, max_chars, one_line},
};

/// What a post's use case reads and writes.
pub struct PostDeps {
    pub post_repo: Arc<dyn PostRepositoryTrait + Send + Sync>,
    pub sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    pub admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    pub commodity_repo: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
    /// Stores and discards the cover photos.
    pub media: MediaUseCase,
    pub events: Arc<dyn EventBus>,
}

#[derive(Clone)]
pub struct PostUseCase {
    posts: Arc<dyn PostRepositoryTrait + Send + Sync>,
    sectors: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    admin_units: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    commodities: Arc<dyn CommodityRepositoryTrait + Send + Sync>,
    media: MediaUseCase,
    events: Arc<dyn EventBus>,
}

impl PostUseCase {
    pub fn new(d: PostDeps) -> Self {
        Self {
            posts: d.post_repo,
            sectors: d.sector_repo,
            admin_units: d.admin_unit_repo,
            commodities: d.commodity_repo,
            media: d.media,
            events: d.events,
        }
    }

    async fn publish(&self, actor: &Actor, id: i32, change: Change, reason: Option<String>) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Post,
                entity_id: id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// A page of every post (drafts and hidden ones too), the latest changed
    /// first: `filter`'s words, kind, sector and status.
    pub async fn list(
        &self,
        actor: &Actor,
        page: i64,
        filter: &AdminFilter,
    ) -> Result<AdminPagedResponse<PostDto>> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let (page, offset) = paginate_admin(page);
        let rows = self
            .posts
            .admin_list(filter, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, rows.len(), ADMIN_PAGE_SIZE, || {
            self.posts.admin_count(filter)
        })
        .await?;
        Ok(AdminPagedResponse::admin(
            rows.into_iter().map(post_dto).collect(),
            total,
            page,
        ))
    }

    pub async fn get(&self, actor: &Actor, id: i32) -> Result<PostDto> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        self.find(id).await.map(post_dto)
    }

    async fn find(&self, id: i32) -> Result<Post> {
        self.posts
            .find(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PostNotFound))
    }

    /// The provinces a post may concern, the shown ones: the form's
    /// choices and the public lists' filter. Public.
    pub async fn provinces(&self) -> Result<Vec<NamedCodeDto>> {
        Ok(self
            .admin_units
            .provinces()
            .await?
            .into_iter()
            .filter(|p| p.is_active)
            .map(|p| NamedCodeDto {
                code: p.code,
                name: p.name,
            })
            .collect())
    }

    /// The sectors, provinces and commodities a post may concern.
    pub async fn choices(&self, actor: &Actor) -> Result<PostChoicesDto> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let (sectors, provinces, commodities) = tokio::try_join!(
            self.sectors.list_all(),
            self.provinces(),
            self.commodities.list_all(),
        )?;
        Ok(PostChoicesDto {
            sectors: sectors
                .into_iter()
                .map(|s| NamedCodeDto {
                    code: s.code,
                    name: s.name,
                })
                .collect(),
            provinces,
            commodities: commodities
                .into_iter()
                .map(|c| NamedIdDto {
                    id: c.id,
                    name: c.name,
                })
                .collect(),
        })
    }

    /// A new draft, with its cover photo when one was chosen: its id. The
    /// photo is stored first and discarded when the post is refused.
    pub async fn create(
        &self,
        actor: &Actor,
        input: PostInput,
        cover: Option<ProcessedImage>,
    ) -> Result<i32> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let fields = self.validate(input).await?;
        let file = match cover {
            Some(image) => Some(self.media.store(actor, image).await?),
            None => None,
        };
        match self.posts.create(&fields, file, actor.user_id).await {
            Ok(id) => {
                self.publish(actor, id, Change::Created, None).await;
                Ok(id)
            }
            Err(e) => {
                if let Some(file) = file {
                    self.media.discard(file).await;
                }
                Err(e)
            }
        }
    }

    /// Its boxes, and its cover when a new photo was chosen (the former one
    /// goes to the file cleanup).
    pub async fn update(
        &self,
        actor: &Actor,
        id: i32,
        input: PostInput,
        cover: Option<ProcessedImage>,
    ) -> Result<()> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let fields = self.validate(input).await?;
        let file = match cover {
            Some(image) => Some(self.media.store(actor, image).await?),
            None => None,
        };
        let saved = self.posts.update(id, &fields, actor.user_id).await;
        let saved = match saved {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::new(ErrorCode::PostNotFound)),
            Err(e) => Err(e),
        };
        if let Err(e) = saved {
            if let Some(file) = file {
                self.media.discard(file).await;
            }
            return Err(e);
        }
        self.publish(actor, id, Change::Updated, None).await;
        if let Some(file) = file {
            let previous = self
                .posts
                .set_cover(id, Some(file), actor.user_id)
                .await?
                .ok_or_else(|| Error::new(ErrorCode::PostNotFound))?;
            self.publish(actor, id, Change::ImageReplaced { previous }, None)
                .await;
        }
        Ok(())
    }

    /// Takes its cover photo off (audited with `reason`); nothing to do when
    /// it has none.
    pub async fn remove_cover(&self, actor: &Actor, id: i32, reason: Option<String>) -> Result<()> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let previous = self
            .posts
            .set_cover(id, None, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PostNotFound))?;
        if let Some(previous) = previous {
            self.publish(actor, id, Change::ImageRemoved { previous }, reason)
                .await;
        }
        Ok(())
    }

    /// Publishes, hides or turns it back into a draft; the same status again
    /// changes nothing.
    pub async fn set_status(
        &self,
        actor: &Actor,
        id: i32,
        status: &str,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_CONTENT_POST_MANAGE)?;
        let status = POST_STATUSES
            .into_iter()
            .find(|s| *s == status.trim())
            .ok_or_else(|| Error::field("status", ErrorCode::InvalidPostStatus))?;
        let changed = self
            .posts
            .set_status(id, status, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::PostNotFound))?;
        if changed {
            let status = status.to_owned();
            self.publish(actor, id, Change::StatusChanged { status }, reason)
                .await;
        }
        Ok(())
    }

    /// A page of the published posts `query` asks for, pinned first, the
    /// newest first. Public.
    pub async fn public_page(
        &self,
        query: &PostQuery,
        page: i64,
    ) -> Result<PagedResponse<PostDto>> {
        let (page, offset) = paginate_public(page);
        let rows = self.posts.public_list(query, PAGE_SIZE, offset).await?;
        let total = total_or_count(offset, rows.len(), PAGE_SIZE, || {
            self.posts.public_count(query)
        })
        .await?;
        Ok(PagedResponse::public(
            rows.into_iter().map(post_dto).collect(),
            total,
            page,
        ))
    }

    /// The first `limit` published posts `query` asks for. Public.
    pub async fn latest(&self, query: &PostQuery, limit: i64) -> Result<Vec<PostDto>> {
        Ok(self
            .posts
            .public_list(query, limit, 0)
            .await?
            .into_iter()
            .map(post_dto)
            .collect())
    }

    /// A published post; any other is `post_not_found`. Public.
    pub async fn public_get(&self, id: i32) -> Result<PostDto> {
        let post = self.find(id).await?;
        if post.status != STATUS_PUBLISHED {
            return Err(Error::new(ErrorCode::PostNotFound));
        }
        Ok(post_dto(post))
    }

    /// Trims every box and reports every bad one at once.
    async fn validate(&self, input: PostInput) -> Result<PostFields> {
        let kind = input.kind.trim().to_owned();
        let title = one_line(&input.title);
        let summary = one_line(&input.summary);
        let body = input.body.trim().to_owned();
        let sector = input.sector.trim().to_owned();
        let province = input.province_code.trim().to_owned();
        let (sectors, provinces, commodity) = tokio::try_join!(
            self.sectors.list_all(),
            self.admin_units.provinces(),
            async {
                match input.commodity_id {
                    Some(id) => self.commodities.find(id).await,
                    None => Ok(None),
                }
            },
        )?;
        let known_province =
            province.is_empty() || provinces.iter().any(|p| p.code == province && p.is_active);
        Validation::new()
            .check(
                !POST_KINDS.contains(&kind.as_str()),
                "kind",
                ErrorCode::InvalidPostKind,
            )
            .check(title.is_empty(), "title", ErrorCode::PostTitleRequired)
            .rule("title", max_chars(&title, POST_TITLE_MAX))
            .rule("summary", max_chars(&summary, POST_SUMMARY_MAX))
            .check(body.is_empty(), "body", ErrorCode::PostBodyRequired)
            .rule("body", max_chars(&body, POST_BODY_MAX))
            .check(
                validation::sector(&sector, &sectors).is_err(),
                "sector",
                ErrorCode::InvalidSector,
            )
            .check(!known_province, "province_code", ErrorCode::InvalidProvince)
            .check(
                input.commodity_id.is_some() && commodity.is_none(),
                "commodity_id",
                ErrorCode::InvalidCommodity,
            )
            .finish()?;
        Ok(PostFields {
            kind,
            title,
            summary,
            body,
            sector: (!sector.is_empty()).then_some(sector),
            province_code: (!province.is_empty()).then_some(province),
            commodity_id: input.commodity_id,
            is_pinned: input.is_pinned,
        })
    }
}

fn post_dto(p: Post) -> PostDto {
    PostDto {
        id: p.id,
        kind: p.kind,
        title: p.title,
        summary: p.summary,
        body: p.body,
        cover_file_id: p.cover_file_id.map(|f| f.to_string()),
        sector: p.sector,
        province_code: p.province_code,
        province_name: p.province_name,
        commodity_id: p.commodity_id,
        commodity_name: p.commodity_name,
        status: p.status,
        published_on: p.published_on,
        is_pinned: p.is_pinned,
        is_demo: p.is_demo,
    }
}
