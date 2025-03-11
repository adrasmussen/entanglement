use std::collections::HashSet;

use async_trait::async_trait;

use api::media::MediaUuid;
use common::auth::{AuthnBackend, AuthzBackend};

use crate::service::*;

pub mod msg;
pub mod svc;

#[async_trait]
trait ESAuthService: ESInner {
    // cache management
    async fn clear_user_cache(&self, uid: Vec<String>) -> anyhow::Result<()>;

    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()>;

    // authz
    async fn add_authz_provider(&self, provider: impl AuthzBackend) -> anyhow::Result<()>;

    async fn groups_for_user(&self, uid: String) -> anyhow::Result<HashSet<String>>;

    async fn users_in_group(&self, gid: String) -> anyhow::Result<HashSet<String>>;

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool>;

    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool>;

    async fn owns_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool>;

    // authn
    async fn add_authn_provider(&self, provider: impl AuthnBackend) -> anyhow::Result<()>;

    async fn authenticate_user(&self, uid: String, password: String) -> anyhow::Result<bool>;

    async fn is_valid_user(&self, uid: String) -> anyhow::Result<bool>;
}
