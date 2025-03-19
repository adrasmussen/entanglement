use std::collections::HashSet;
use std::fmt::Debug;

use anyhow::Result;
use async_trait::async_trait;
use tracing::{instrument, Level};

use crate::auth::msg::AuthMsg;
use crate::db::msg::DbMsg;
use crate::service::{ESInner, ServiceType};
use api::{
    collection::CollectionUuid, comment::CommentUuid, library::LibraryUuid, media::MediaUuid,
};

#[async_trait]
pub trait AuthCheck: ESInner + Debug {
    #[instrument(level=Level::DEBUG)]
    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> Result<()> {
        let auth_svc_sender = self.registry().get(&ServiceType::Auth)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        auth_svc_sender
            .send(
                AuthMsg::ClearAccessCache {
                    resp: tx,
                    media_uuid: media_uuid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn groups_for_user(&self, uid: &String) -> Result<HashSet<String>> {
        let auth_svc_sender = self.registry().get(&ServiceType::Auth)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        auth_svc_sender
            .send(
                AuthMsg::GroupsForUser {
                    resp: tx,
                    uid: uid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn is_group_member(&self, uid: &String, gid: HashSet<String>) -> Result<bool> {
        let auth_svc_sender = self.registry().get(&ServiceType::Auth)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        auth_svc_sender
            .send(
                AuthMsg::IsGroupMember {
                    resp: tx,
                    uid: uid.clone(),
                    gid: gid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_media(&self, uid: &String, media_uuid: &MediaUuid) -> Result<bool> {
        let auth_svc_sender = self.registry().get(&ServiceType::Auth)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        auth_svc_sender
            .send(
                AuthMsg::CanAccessMedia {
                    resp: tx,
                    uid: uid.clone(),
                    media_uuid: media_uuid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_media(&self, uid: &String, media_uuid: &MediaUuid) -> Result<bool> {
        let auth_svc_sender = self.registry().get(&ServiceType::Auth)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        auth_svc_sender
            .send(
                AuthMsg::OwnsMedia {
                    resp: tx,
                    uid: uid.clone(),
                    media_uuid: media_uuid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_comment(&self, uid: &String, comment_uuid: &CommentUuid) -> Result<bool> {
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetComment {
                    resp: tx,
                    comment_uuid: comment_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let comment = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

        self.can_access_media(uid, &comment.media_uuid).await
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_comment(&self, uid: &String, comment_uuid: &CommentUuid) -> Result<bool> {
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetComment {
                    resp: tx,
                    comment_uuid: comment_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let comment = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

        Ok(uid.to_owned() == comment.uid)
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_collection(
        &self,
        uid: &String,
        collection_uuid: &CollectionUuid,
    ) -> Result<bool> {
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetCollection {
                    resp: tx,
                    collection_uuid: collection_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let collection = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown collection_uuid"))?;

        self.is_group_member(&uid, HashSet::from([collection.gid]))
            .await
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_collection(
        &self,
        uid: &String,
        collection_uuid: &CollectionUuid,
    ) -> Result<bool> {
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetCollection {
                    resp: tx,
                    collection_uuid: collection_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let collection = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown collection_uuid"))?;

        Ok(uid.to_owned() == collection.uid)
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_library(&self, uid: &String, library_uuid: &LibraryUuid) -> Result<bool> {
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: tx,
                    library_uuid: library_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let library = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown library_uuid"))?;

        self.is_group_member(&uid, HashSet::from([library.gid]))
            .await
    }
}
