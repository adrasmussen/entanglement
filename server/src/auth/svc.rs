use std::{collections::HashSet, sync::Arc, time::Duration};

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use regex::Regex;
use tokio::{sync::Mutex, task::spawn, time::timeout};
use tracing::{Instrument, Level, debug, error, info, instrument, span};

use crate::{
    auth::{ESAuthService, msg::AuthMsg},
    db::msg::DbMsg,
    service::{ESInner, ESMRegistry, EntanglementService, Esm, EsmReceiver, ServiceType},
};
use api::media::MediaUuid;
use common::{
    AwaitCache, GROUP_REGEX, USER_REGEX,
    auth::{
        AuthnProvider, AuthzProvider,
        cert::CertAuthn,
        ldap::LdapAuthz,
        proxy::ProxyAuth,
        tomlfile::{TomlAuthnFile, TomlAuthzFile},
    },
    config::{AuthnBackend, AuthzBackend, ESConfig},
};

// auth service
//
// the auth service is really two services in one -- a way to query several authentication (authn)
// and authorization (authz) providers, and a cache so we don't have to query them often
//
// the cache semantics are not ideal (need to be flushed manually when the group information
// changes), but the database service will flush individual media files when their collections change
//
// there are almost certainly several better ways of doing this, which will likely matter when we
// switch to using ldap instead of a fixed file, but the services should roughly stay the same
pub struct AuthService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<EsmReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for AuthService {
    type Inner = AuthCache;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<Esm>(1024);

        registry
            .insert(ServiceType::Auth, tx)
            .expect("failed to add auth sender to registry");

        AuthService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    #[instrument(skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> anyhow::Result<()> {
        info!("starting auth service");

        let config = self.config.clone();
        let receiver = self.receiver.clone();
        let state = Arc::new(AuthCache::new(config.clone(), registry.clone())?);

        // for the first pass, we don't need any further machinery for this service
        //
        // however, if we want things like timers, batching updates, or other optimizations,
        // they would all go here with corresponding handles in the AuthService struct
        //
        // example would use StreamExt's ready_chunks() method

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "auth", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg("auth service esm channel disconnected"))
            }
        };

        self.handle.set(spawn(serve));

        debug!("started auth service");
        Ok(())
    }
}

#[derive(Debug)]
pub struct AuthCache {
    registry: ESMRegistry,
    authn_provider: Box<dyn AuthnProvider>,
    authz_provider: Box<dyn AuthzProvider>,
    // uid: set(gid)
    user_cache: Arc<AwaitCache<String, HashSet<String>>>,
    // media_uuid: set(gid)
    access_cache: Arc<AwaitCache<MediaUuid, HashSet<String>>>,
    user_regex: Regex,
    group_regex: Regex,
}

#[async_trait]
impl ESInner for AuthCache {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> anyhow::Result<Self> {
        let authn_provider: Box<dyn AuthnProvider> = match config.authn_backend {
            AuthnBackend::ProxyHeader => Box::new(ProxyAuth::new(config.clone())?),
            AuthnBackend::TomlFile => Box::new(TomlAuthnFile::new(config.clone())?),
            AuthnBackend::X509Cert => Box::new(CertAuthn::new(config.clone())?),
        };

        let authz_provider: Box<dyn AuthzProvider> = match config.authz_backend {
            AuthzBackend::Ldap => Box::new(LdapAuthz::new(config.clone())?),
            AuthzBackend::TomlFile => Box::new(TomlAuthzFile::new(config.clone())?),
        };

        Ok(AuthCache {
            registry: registry.clone(),
            authn_provider,
            authz_provider,
            user_cache: Arc::new(AwaitCache::new()),
            access_cache: Arc::new(AwaitCache::new()),
            user_regex: Regex::new(USER_REGEX)?,
            group_regex: Regex::new(GROUP_REGEX)?,
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    async fn message_handler(&self, esm: Esm) -> anyhow::Result<()> {
        match esm {
            Esm::Auth(message) => match message {
                AuthMsg::_ClearUserCache { resp, uid } => {
                    self.respond(resp, self.clear_user_cache(uid)).await
                }
                AuthMsg::ClearAccessCache { resp, media_uuid } => {
                    self.respond(resp, self.clear_access_cache(media_uuid))
                        .await
                }
                AuthMsg::GroupsForUser { resp, uid } => {
                    self.respond(resp, self.groups_for_user(uid)).await
                }
                AuthMsg::UsersInGroup { resp, gid } => {
                    self.respond(resp, self.users_in_group(gid)).await
                }
                AuthMsg::IsGroupMember { resp, uid, gid } => {
                    self.respond(resp, self.is_group_member(uid, gid)).await
                }
                AuthMsg::CanAccessMedia {
                    resp,
                    uid,
                    media_uuid,
                } => {
                    self.respond(resp, self.can_access_media(uid, media_uuid))
                        .await
                }
                AuthMsg::OwnsMedia {
                    resp,
                    uid,
                    media_uuid,
                } => self.respond(resp, self.owns_media(uid, media_uuid)).await,
                AuthMsg::_IsValidUser { resp, uid } => {
                    self.respond(resp, self.is_valid_user(uid)).await
                }
                AuthMsg::_AuthenticateUser {
                    resp,
                    uid,
                    password,
                } => {
                    self.respond(resp, self.authenticate_user(uid, password))
                        .await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

// AuthCache-specifc logic
//
// these two functions are the interface with the particular details of the providers and
// the way the media authorization scheme works
impl AuthCache {
    // this is called to populate the user cache for a particular user
    //
    // thus, it is the best singular place to check group validity, as it ensure that only
    // valid groups make it into the cache
    async fn groups_from_providers(&self, uid: String) -> anyhow::Result<HashSet<String>> {
        if !self.is_valid_user(uid.clone()).await? {
            return Err(anyhow::Error::msg("invalid uid"));
        }

        let mut groups = self.authz_provider.groups_for_user(uid.clone()).await?;

        groups.retain(|s| self.group_regex.is_match(s));

        Ok(groups)
    }

    // for a given media_uuid, this is the db lookup that establishes which groups are allowed
    // to access that media.  it does not implement any caching itself, and should always be
    // up-to-date when called
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>> {
        let db_svc_sender = self.registry.get(&ServiceType::Db)?;
        let (tx, rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .clone()
            .send(
                DbMsg::MediaAccessGroups {
                    resp: tx,
                    media_uuid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }
}

#[async_trait]
impl ESAuthService for AuthCache {
    // cache management
    #[instrument(skip(self))]
    async fn clear_user_cache(&self, uid: Vec<String>) -> anyhow::Result<()> {
        let user_cache = self.user_cache.clone();

        match uid.len() {
            0 => {
                user_cache.clear();
            }
            _ => {
                for k in uid {
                    user_cache.remove(&k)
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()> {
        debug!("clearing access cache");

        let access_cache = self.access_cache.clone();

        match media_uuid.len() {
            0 => {
                access_cache.clear();
            }
            _ => {
                for k in media_uuid {
                    access_cache.remove(&k)
                }
            }
        }

        Ok(())
    }

    // authz
    #[instrument(skip_all)]
    // CACHE LOOKUP FUNCTION
    //
    // this is the primary access method for the user AwaitCache
    async fn groups_for_user(&self, uid: String) -> anyhow::Result<HashSet<String>> {
        let user_cache = self.user_cache.clone();

        let groups = user_cache
            .perhaps(
                uid.clone(),
                async {
                    let fut = timeout(Duration::from_secs(10), self.groups_from_providers(uid));
                    match fut.await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(err)) => Err(err),
                        Err(err) => Err(anyhow::Error::from(err)),
                    }
                }
                .instrument(span!(Level::INFO, "groups_for_user")),
            )
            .await?;

        Ok(groups.clone())
    }

    async fn users_in_group(&self, gid: String) -> anyhow::Result<HashSet<String>> {
        self.authz_provider.users_in_group(gid).await
    }

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool> {
        Ok(gid.intersection(&self.groups_for_user(uid).await?).count() > 0)
    }

    // CACHE LOOKUP FUNCTION
    //
    // this is the primary access method for the media AwaitCache
    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        let access_cache = self.access_cache.clone();

        let groups = access_cache
            .perhaps(
                media_uuid,
                async {
                    let fut = timeout(
                        Duration::from_secs(10),
                        self.media_access_groups(media_uuid),
                    );
                    match fut.await {
                        Ok(Ok(v)) => Ok(v),
                        Ok(Err(err)) => Err(err),
                        Err(err) => Err(anyhow::Error::from(err)),
                    }
                }
                .instrument(span!(Level::INFO, "can_access_media")),
            )
            .await?;

        Ok(self.is_group_member(uid, groups).await?)
    }

    // this should be a relatively uncommon operation, so having three independent database messages
    // is worth being able to use the existing messages to get the information
    async fn owns_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        let db_svc_sender = self.registry.get(&ServiceType::Db)?;
        let (media_tx, media_rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .clone()
            .send(
                DbMsg::GetMedia {
                    resp: media_tx,
                    media_uuid,
                }
                .into(),
            )
            .await?;

        let media = match media_rx.await?? {
            Some(result) => result.0,
            None => return Ok(false),
        };

        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .clone()
            .send(
                DbMsg::GetLibrary {
                    resp: library_tx,
                    library_uuid: media.library_uuid,
                }
                .into(),
            )
            .await?;

        let library = match library_rx.await?? {
            Some(result) => result,
            None => return Ok(false),
        };

        Ok(self
            .is_group_member(uid, HashSet::from([library.gid]))
            .await?)
    }

    // authn
    #[instrument(skip_all)]
    async fn authenticate_user(&self, uid: String, password: String) -> anyhow::Result<bool> {
        self.authn_provider
            .authenticate_user(uid.clone(), password.clone())
            .await
    }

    async fn is_valid_user(&self, uid: String) -> anyhow::Result<bool> {
        if !self.user_regex.is_match(&uid) {
            return Err(anyhow::Error::msg("invalid uid"));
        }

        self.authn_provider.is_valid_user(uid.clone()).await
    }
}
