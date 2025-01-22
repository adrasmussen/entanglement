use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};

use api::media::MediaUuid;
use common::{
    auth::{proxy::ProxyAuth, yamlfile::YamlGroupFile, AuthnBackend, AuthzBackend},
    config::ESConfig,
};

use crate::auth::msg::AuthMsg;
use crate::auth::ESAuthService;
use crate::db::msg::DbMsg;
use crate::service::*;

// auth service
//
// the auth service is really two services in one -- a way to query several authentication (authn)
// and authorization (authz) providers, and a cache so we don't have to query them often
//
// the cache semantics are not ideal (need to be flushed manually when the group information
// changes), but the database service will flush individual media files when their albums change
//
// there are almost certainly several better ways of doing this, which will likely matter when we
// switch to using ldap instead of a fixed file, but the services should roughly stay the same
pub struct AuthService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for AuthService {
    type Inner = AuthCache;

    fn create(config: Arc<ESConfig>, sender_map: &mut HashMap<ServiceType, ESMSender>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(65536);

        sender_map.insert(ServiceType::Auth, tx);

        AuthService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    async fn start(&self, senders: &HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        let config = self.config.clone();
        let receiver = self.receiver.clone();
        let state = Arc::new(AuthCache::new(config.clone(), senders.clone())?);

        // determine authn/authz providers from the global config file
        //
        // each provider is tied to a particular field that, if set, means that we should try
        // to connect to that provider.  connect() may use other parts of the config struct
        match config.authn_proxy_header {
            None => {}
            Some(_) => {
                state
                    .add_authn_provider(ProxyAuth::connect(config.clone()).await?)
                    .await?;
            }
        }

        match config.authz_yaml_groups {
            None => {}
            Some(_) => {
                state
                    .add_authz_provider(YamlGroupFile::connect(config.clone()).await?)
                    .await?;
            }
        }

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
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("cache service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        Ok(())
    }
}

pub struct AuthCache {
    db_svc_sender: ESMSender,
    authn_providers: Arc<Mutex<Vec<Box<dyn AuthnBackend>>>>,
    authz_providers: Arc<Mutex<Vec<Box<dyn AuthzBackend>>>>,
    // uid: set(gid)
    user_cache: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    // media_uuid: set(gid)
    access_cache: Arc<RwLock<HashMap<MediaUuid, HashSet<String>>>>,
}

// should we attempt to optimize the rwlock logic to batch writes?
//
// maybe batch with tokio::select! and recv_many -- have a separate writer
// process with a 1024-length queue
//
// may not need timer, actually -- if we while let Some(_) = recv_many(...)
// then we let things build up during each cycle

// alternatively, prime the cache at boot time and periodically refresh it
// in bulk, including after each scan
//
// then, after we commit changes to any images, refresh just that image
#[async_trait]
impl ESAuthService for AuthCache {
    // cache management
    async fn clear_user_cache(&self, uid: Vec<String>) -> anyhow::Result<()> {
        let user_cache = self.user_cache.clone();

        {
            let mut user_cache = user_cache.write().await;

            match uid.len() {
                0 => {
                    user_cache.drain();
                }
                _ => {
                    let _ = uid.into_iter().map(|uid| user_cache.remove(&uid));
                }
            }
        }

        Ok(())
    }

    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()> {
        let access_cache = self.access_cache.clone();

        {
            let mut access_cache = access_cache.write().await;

            match media_uuid.len() {
                0 => {
                    access_cache.drain();
                }
                _ => {
                    let _ = media_uuid
                        .into_iter()
                        .map(|media_uuid| access_cache.remove(&media_uuid));
                }
            }
        }

        Ok(())
    }

    // authz
    async fn add_authz_provider(&self, provider: impl AuthzBackend) -> anyhow::Result<()> {
        let authz_providers = self.authz_providers.clone();

        let mut authz_providers = authz_providers.lock().await;

        authz_providers.push(Box::new(provider));

        Ok(())
    }

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool> {
        let user_cache = self.user_cache.clone();

        // if there is an entry in the user cache, assume that it is correct
        //
        // this leads to the somewhat unfortunate effect that we need to manually
        // clear the user cache when group information changes
        //
        // when the groups were part of the db, this was easier -- but we can't
        // easily subscribe to ldap updates

        // many threads may race through this check initially, at least until one
        // grabs the write() lock below.  then they will all pile up waiting for
        // a read(), at which point the HashSet should exist
        //
        // in the event that the thread with the write lock panics, all the threads
        // stopped at the second read will get an Error; threads stopped at the
        // first read will race to the second and try again
        //
        // TODO -- in the event of an authz failure, each of the waiting messages
        // will attempt to populate the cache.  ideally, we detect this somehow
        // and either fail quickly or tell the http service to reject those requests
        {
            let user_cache = user_cache.read().await;

            match user_cache.get(&uid) {
                Some(groups) => {
                    // this is the fast path
                    return Ok(groups.intersection(&gid).count() > 0);
                }
                None => {}
            }
        }

        // on a cache miss, we still want to verify that the user is recognized by someone
        if !self.is_valid_user(uid.clone()).await? {
            return Err(anyhow::Error::msg("invalid user"));
        }

        // to ensure that only one thread attempts to populate the cache, we need a second
        // early return that also awaits the cell
        let mut user_cache = match user_cache.try_write() {
            Err(_) => {
                let user_cache = user_cache.read().await;

                // this is a somewhat awkward error to handle -- it would fire if the thread
                // that gets the write() lock fails to actually add the uid to the HashMap
                let groups = user_cache
                    .get(&uid)
                    .ok_or_else(|| anyhow::Error::msg("failed to initialize user_cache"))?;

                return Ok(groups.intersection(&gid).count() > 0);
            }
            Ok(lock) => lock,
        };

        // populate the cache
        let groups = {
            let mut groups = HashSet::new();

            let authz_providers = self.authz_providers.clone();

            let authz_providers = authz_providers.lock().await;

            for provider in authz_providers.iter() {
                for group in gid.iter() {
                    if provider
                        .is_group_member(uid.clone(), group.to_string())
                        .await?
                    {
                        groups.insert(group.clone());
                    }
                }
            }
            groups
        };

        user_cache.insert(uid, groups.clone());

        Ok(groups.intersection(&gid).count() > 0)
    }

    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        let access_cache = self.access_cache.clone();

        {
            let access_cache = access_cache.read().await;

            match access_cache.get(&media_uuid) {
                Some(groups) => return Ok(self.is_group_member(uid, groups.clone()).await?),
                None => {}
            }
        }

        let mut access_cache = match access_cache.try_write() {
            Err(_) => {
                let access_cache = access_cache.read().await;

                let groups = access_cache
                    .get(&media_uuid)
                    .ok_or_else(|| anyhow::Error::msg("failed to initialize access_cache"))?;

                return Ok(self.is_group_member(uid, groups.clone()).await?);
            }
            Ok(lock) => lock,
        };

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .clone()
            .send(
                DbMsg::MediaAccessGroups {
                    resp: tx,
                    media_uuid: media_uuid,
                }
                .into(),
            )
            .await?;

        let groups = rx.await??;

        access_cache.insert(media_uuid, groups.clone());

        Ok(self.is_group_member(uid, groups).await?)
    }

    // this should be a relatively uncommon operation, so having three independent database messages
    // is worth being able to use the existing messages to get the information
    async fn owns_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        let (media_tx, media_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .clone()
            .send(
                DbMsg::GetMedia {
                    resp: media_tx,
                    media_uuid: media_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let media = match media_rx.await?? {
            Some(result) => result.0,
            None => return Ok(false),
        };

        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
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
    async fn add_authn_provider(&self, provider: impl AuthnBackend) -> anyhow::Result<()> {
        let authn_providers = self.authn_providers.clone();

        let mut authn_providers = authn_providers.lock().await;

        authn_providers.push(Box::new(provider));

        Ok(())
    }

    async fn authenticate_user(&self, uid: String, password: String) -> anyhow::Result<bool> {
        let authn_providers = self.authn_providers.clone();

        let authn_providers = authn_providers.lock().await;

        for provider in authn_providers.iter() {
            if provider
                .authenticate_user(uid.clone(), password.clone())
                .await?
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn is_valid_user(&self, uid: String) -> anyhow::Result<bool> {
        let authn_providers = self.authn_providers.clone();

        let authn_providers = authn_providers.lock().await;

        for provider in authn_providers.iter() {
            if provider.is_valid_user(uid.clone()).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[async_trait]
impl ESInner for AuthCache {
    fn new(
        _config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(AuthCache {
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            authn_providers: Arc::new(Mutex::new(Vec::new())),
            authz_providers: Arc::new(Mutex::new(Vec::new())),
            user_cache: Arc::new(RwLock::new(HashMap::new())),
            access_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Auth(message) => match message {
                AuthMsg::ClearUserCache { resp, uid } => {
                    self.respond(resp, self.clear_user_cache(uid)).await
                }
                AuthMsg::ClearAccessCache { resp, uuid } => {
                    self.respond(resp, self.clear_access_cache(uuid)).await
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
                AuthMsg::IsValidUser { resp, uid } => {
                    self.respond(resp, self.is_valid_user(uid)).await
                }
                AuthMsg::AuthenticateUser {
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
