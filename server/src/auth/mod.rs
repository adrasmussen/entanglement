use std::collections::HashSet;

use anyhow::Result;
use async_trait::async_trait;

use crate::service::*;
use api::media::MediaUuid;

pub mod check;
pub mod msg;
pub mod svc;

#[async_trait]
trait ESAuthService: ESInner {
    // cache management
    async fn clear_user_cache(&self, uid: Vec<String>) -> Result<()>;

    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> Result<()>;

    // authz
    async fn groups_for_user(&self, uid: String) -> Result<HashSet<String>>;

    async fn users_in_group(&self, gid: String) -> Result<HashSet<String>>;

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> Result<bool>;

    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> Result<bool>;

    async fn owns_media(&self, uid: String, media_uuid: MediaUuid) -> Result<bool>;

    // authn
    async fn authenticate_user(&self, uid: String, password: String) -> Result<bool>;

    async fn is_valid_user(&self, uid: String) -> Result<bool>;
}
