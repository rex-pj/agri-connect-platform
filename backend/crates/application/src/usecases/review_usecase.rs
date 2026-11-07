//! Reviews of providers, by the farmers who reached them through the
//! platform.
//!
//! Only someone the provider actually answered may review it: a contact
//! request from the same phone that the provider marked contacted or done.
//! A review is stored hidden and shown once an admin approves it (the
//! moderation switch in `AdminUseCase`); one per phone and provider. The
//! provider may answer each review publicly. In a village everyone knows
//! everyone, so the rules keep a review tied to a real job and give the
//! provider a voice.

use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::review::NewReview,
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        review_repository_trait::ReviewRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::PERM_OWN_REVIEW_REPLY,
    constants::{
        PUBLIC_REVIEWS_SHOWN, RATING_MAX, RATING_MIN, REQUESTER_NAME_MAX_LEN, REVIEW_COMMENT_MAX,
        REVIEW_REPLY_MAX,
    },
    hashing::sha256_hex,
    inputs::ReviewInput,
    outputs::{OwnReviewDto, ProviderReviewsDto, PublicReviewDto, ReviewSentDto},
    ports::EventBus,
    validation::{max_chars, max_chars_opt, non_blank, validate_phone},
};

#[derive(Clone)]
pub struct ReviewUseCase {
    review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

/// Everything `ReviewUseCase` depends on, named so wiring cannot swap two
/// repos of compatible types by position.
pub struct ReviewDeps {
    pub review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub events: Arc<dyn EventBus>,
}

impl ReviewUseCase {
    pub fn new(d: ReviewDeps) -> Self {
        Self {
            review_repo: d.review_repo,
            inquiry_repo: d.inquiry_repo,
            business_repo: d.business_repo,
            events: d.events,
        }
    }

    /// A live provider's id, or `provider_not_found`.
    async fn live_provider(&self, business_id: i32) -> Result<i32> {
        self.business_repo
            .find_detail(business_id)
            .await?
            .map(|b| b.id)
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))
    }

    /// Stores a farmer's review, hidden until an admin shows it. The boxes'
    /// rules are reported together; then the phone must have a contact
    /// request this provider answered (`review_needs_contact`), and must not
    /// have reviewed it already (`review_already_sent`).
    pub async fn submit(&self, input: ReviewInput) -> Result<ReviewSentDto> {
        let business_id = self.live_provider(input.business_id).await?;
        let reviewer_name = input.reviewer_name.trim().to_owned();
        let phone = input.reviewer_phone.trim().to_owned();
        let comment = non_blank(input.comment);
        let rating = input
            .rating
            .filter(|r| (RATING_MIN..=RATING_MAX).contains(r));
        Validation::new()
            .check(
                reviewer_name.is_empty(),
                "reviewer_name",
                ErrorCode::RequesterNameRequired,
            )
            .rule(
                "reviewer_name",
                max_chars(&reviewer_name, REQUESTER_NAME_MAX_LEN),
            )
            .rule("reviewer_phone", validate_phone(&phone))
            .check(rating.is_none(), "rating", ErrorCode::InvalidRating)
            .rule("comment", max_chars_opt(&comment, REVIEW_COMMENT_MAX))
            .finish()?;
        let Some(rating) = rating else {
            return Err(Error::field("rating", ErrorCode::InvalidRating));
        };

        let inquiry_id = self
            .inquiry_repo
            .find_reviewable(business_id, &phone)
            .await?
            .ok_or_else(|| Error::field("reviewer_phone", ErrorCode::ReviewNeedsContact))?;
        let id = self
            .review_repo
            .create(NewReview {
                business_id,
                inquiry_id,
                rating,
                comment,
                reviewer_name,
                reviewer_phone_hash: sha256_hex(&phone),
            })
            .await?;
        // A visitor's change: not audited, there for notifications.
        self.events
            .publish(DomainEvent {
                actor_id: None,
                entity: Entity::Review,
                entity_id: id,
                change: Change::Created,
                reason: None,
            })
            .await;
        Ok(ReviewSentDto {
            id,
            is_visible: false,
        })
    }

    /// A live provider's shown reviews: their count, average and the newest.
    pub async fn list_public(&self, business_id: i32) -> Result<ProviderReviewsDto> {
        let business_id = self.live_provider(business_id).await?;
        let (summary, items) = tokio::try_join!(
            self.review_repo.summary_public(business_id),
            self.review_repo
                .list_public(business_id, PUBLIC_REVIEWS_SHOWN),
        )?;
        Ok(ProviderReviewsDto {
            count: summary.count,
            average: summary.average.map(|a| (a * 10.0).round() / 10.0),
            items: items.into_iter().map(PublicReviewDto::from).collect(),
        })
    }

    /// Every review of the actor's business, shown or waiting. A session is
    /// enough: they are about the provider's own business.
    pub async fn list_own_reviews(&self, actor: &Actor) -> Result<Vec<OwnReviewDto>> {
        Ok(self
            .review_repo
            .list_for_owner(actor.user_id)
            .await?
            .into_iter()
            .map(OwnReviewDto::from)
            .collect())
    }

    /// Answers a review of the actor's business publicly; a blank answer
    /// takes it down. Someone else's review is `review_not_found`.
    pub async fn reply_own_review(
        &self,
        actor: &Actor,
        id: i32,
        reply: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_OWN_REVIEW_REPLY)?;
        let reply = non_blank(reply);
        Validation::new()
            .rule("reply", max_chars_opt(&reply, REVIEW_REPLY_MAX))
            .finish()?;
        if !self
            .review_repo
            .set_reply_for_owner(actor.user_id, id, reply)
            .await?
        {
            return Err(Error::new(ErrorCode::ReviewNotFound));
        }
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Review,
                entity_id: id,
                change: Change::Updated,
                reason: None,
            })
            .await;
        Ok(())
    }
}
