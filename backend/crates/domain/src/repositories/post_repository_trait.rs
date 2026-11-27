use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    error::Error,
    models::{
        admin_list::AdminFilter,
        post::{Post, PostFields, PostQuery},
    },
};

#[async_trait]
pub trait PostRepositoryTrait {
    /// A page of every post (drafts and hidden ones too), the latest
    /// changed first: `filter`'s words in the title or summary, its kind
    /// (`kind`), sector and status (`state`).
    async fn admin_list(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Post>, Error>;

    async fn admin_count(&self, filter: &AdminFilter) -> Result<i64, Error>;

    async fn find(&self, id: i32) -> Result<Option<Post>, Error>;

    /// A draft; its id.
    async fn create(&self, fields: &PostFields, cover: Option<Uuid>, by: i32)
        -> Result<i32, Error>;

    /// False when there is no such post.
    async fn update(&self, id: i32, fields: &PostFields, by: i32) -> Result<bool, Error>;

    /// Its cover now `cover`; `Some(previous)`, or `None` when there is no
    /// such post.
    async fn set_cover(
        &self,
        id: i32,
        cover: Option<Uuid>,
        by: i32,
    ) -> Result<Option<Option<Uuid>>, Error>;

    /// `Some(changed)`, or `None` when there is no such post. Published, it
    /// keeps the day it was first published, else takes now.
    async fn set_status(&self, id: i32, status: &str, by: i32) -> Result<Option<bool>, Error>;

    /// A page of the published posts `query` asks for, pinned first, the
    /// newest first.
    async fn public_list(
        &self,
        query: &PostQuery,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Post>, Error>;

    async fn public_count(&self, query: &PostQuery) -> Result<i64, Error>;
}
