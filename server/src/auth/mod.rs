use std::collections::HashSet;

use async_trait::async_trait;

use common::api::media::MediaUuid;

use crate::service::*;

pub mod msg;
pub mod svc;

pub fn get_admin_groups() -> HashSet<String> {
    HashSet::from([String::from("admins")])
}

#[derive(Debug)]
pub enum AuthType {
    _ProxyHeader,
    _LDAP
}

#[async_trait]
trait ESAuthService: ESInner {
    async fn clear_user_cache(&self, uid: Vec<String>) -> anyhow::Result<()>;

    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()>;

    // used if the server is providing authn via some sort of backend
    async fn is_valid_user(&self, auth_type: AuthType, uid: String, password: String) -> anyhow::Result<bool>;

    // checks if the user is a member of any of the specified groups
    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool>;

    // a user can access media if they either are a member of a group that either owns a library
    // or album containing that media
    //
    // access allows users to download the media for viewing by the frontend as well as creating
    // tickets that reference that media
    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool>;

    // a user owns media if they are a member of a group that owns a library
    //
    // ownership allows adding media to albums and setting the hidden state
    //
    // in reality, we likely do not need the owner field, since everything we could want to use
    // that information for is already handled via group
    async fn owns_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool>;
}
