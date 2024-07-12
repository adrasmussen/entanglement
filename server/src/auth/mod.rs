use std::collections::HashSet;

use async_trait::async_trait;

use api::MediaUuid;

use crate::service::*;

pub mod msg;
pub mod svc;

#[derive(Debug)]
enum AuthType {
    ProxyHeader,
    LDAP
}

// we only do authorization, not authentication
#[async_trait]
trait ESAuthService: ESInner {
    async fn clear_user_cache(&self, uid: Option<String>) -> anyhow::Result<()>;

    async fn clear_access_cache(&self, media: Option<MediaUuid>) -> anyhow::Result<()>;

    async fn is_valid_user(&self, auth_type: AuthType, user: String, password: String) -> anyhow::Result<bool>;

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool>;

    async fn can_access_media(&self, uid: String, uuid: MediaUuid) -> anyhow::Result<bool>;
}
