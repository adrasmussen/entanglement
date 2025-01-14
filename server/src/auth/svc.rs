use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};

use common::{
    api::media::MediaUuid,
    auth::{AuthnBackend, AuthzBackend},
    config::ESConfig,
};

use crate::auth::ESAuthService;
use crate::auth::msg::AuthMsg;
use crate::db::msg::DbMsg;
use crate::service::*;

pub struct AuthCache {
    db_svc_sender: ESMSender,
    authn_providers: Vec<Box<dyn AuthnBackend>>,
    authz_providers: Vec<Box<dyn AuthzBackend>>,
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
        self.authz_providers.push(Box::new(provider));

        Ok(())
    }

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool> {
        let user_cache = self.user_cache.clone();

        // check the cache first using a read lock (since this function gets called for practically
        // every single api call and needs to be fast)
        //
        // this cache must be manually cleared
        {
            let user_cache = user_cache.read().await;

            match user_cache.get(&uid) {
                Some(groups) => return Ok(groups.intersection(&gid).count() > 0),
                None => {}
            }
        }

        // on a cache miss, we still want to verify that the user is recognized by someone
        if !self.is_valid_user(uid).await? {
            return Err(anyhow::Error::msg("invalid user"));
        }

        // since this is the cache population step, we check all of the known authz providers
        //
        // clients may make many more requests while this is running, and would need to refresh
        // after the cache populates in the current model
        //
        // we could try to grab the write() lock sooner and hold it, but we would have to be
        // really fast... but user-specific locks would keep this from blocking for too long
        let groups = HashSet::new();

        for provider in self.authz_providers {
            for group in gid.iter() {
                if provider.is_group_member(uid, group.to_string()).await? {
                    groups.insert(group.clone());
                }
            }
        }

        {
            let mut user_cache = user_cache.write().await;

            user_cache.insert(uid, groups);
        }

        Ok(groups.intersection(&gid).count() > 0)
    }

    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        let access_cache = self.access_cache.clone();

        // in order to get the HashSet out of the HashMap, we need to match the outer get()
        // before cloning out of the block
        let cached_groups = {
            let access_cache = access_cache.read().await;

            let cached_groups = match access_cache.get(&media_uuid) {
                None => None,
                Some(v) => Some(v.clone()),
            };

            cached_groups
        };

        // since we have to compare the sets of groups, we have to handle the cache miss
        // before we check for an intersection
        //
        // this is outside the block with the read() mutex so we don't hold the lock any
        // longer than is strictly necessary
        let groups = match cached_groups {
            Some(v) => v,
            None => {
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
                    .await
                    .context("Failed to send MediaAccessGroups from can_access_media")?;

                let groups = rx.await.context(
                    "Failed to receive MediaAccessGroups response at can_access_media",
                )??;

                // lock and unlock the write mutex with as little work as possible
                {
                    let mut access_cache = access_cache.write().await;

                    access_cache.insert(media_uuid, groups.clone());
                }

                groups
            }
        };

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
            .await
            .context("Failed to send GetMedia mesaage in owns_media")?;

        let media = match media_rx
            .await
            .context("Failed to receive GetMedia response in owns_media")??
        {
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
            .await
            .context("Failed to send GetLibrary mesaage in owns_media")?;

        let library = match library_rx
            .await
            .context("Failed to receive GetLibrary response in owns_media")??
        {
            Some(result) => result,
            None => return Ok(false),
        };

        Ok(self
            .is_group_member(uid, HashSet::from([library.gid]))
            .await?)
    }

    // authn
    async fn add_authn_provider(&self, provider: impl AuthnBackend) -> anyhow::Result<()> {
        self.authn_providers.push(Box::new(provider));

        Ok(())
    }

    async fn authenticate_user(&self, uid: String, password: String) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn is_valid_user(&self, _user: String) -> anyhow::Result<bool> {
        Ok(true)
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
            authn_providers: Vec::new(),
            authz_providers: Vec::new(),
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
                AuthMsg::_IsValidUser {
                    resp,
                    auth_type,
                    uid,
                    password,
                } => self.respond(resp, self.is_valid_user(uid)).await,
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
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct AuthService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for AuthService {
    type Inner = AuthCache;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx,
            AuthService {
                config: config.clone(),
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(AuthCache::new(self.config.clone(), senders)?);

        // for the first pass, we don't need any further machinery for this service
        //
        // however, if we want things like timers, batching updates, or other optimizations,
        // they would all go here with corresponding handles in the AuthService struct
        //
        // example would use StreamExt's ready_chunks() method

        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
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
