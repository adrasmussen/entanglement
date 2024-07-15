use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use tokio::sync::{Mutex, RwLock};

use api::{auth::User, MediaUuid};

use crate::service::*;

use crate::auth::ESAuthService;
use crate::auth::{msg::AuthMsg, AuthType};

use crate::db::msg::DbMsg;

pub struct AuthCache {
    db_svc_sender: ESMSender,
    // uid: set(gid)
    user_cache: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    // uuid: set(gid)
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
    async fn clear_user_cache(&self, uid: Option<String>) -> anyhow::Result<()> {
        let user_cache = self.user_cache.clone();

        {
            let mut user_cache = user_cache.write().await;

            match uid {
                Some(uid) => {
                    user_cache.remove(&uid);
                }
                None => {
                    user_cache.drain();
                }
            }
        }

        Ok(())
    }

    async fn clear_access_cache(&self, media: Option<MediaUuid>) -> anyhow::Result<()> {
        let access_cache = self.access_cache.clone();

        {
            let mut access_cache = access_cache.write().await;

            match media {
                Some(media) => {
                    access_cache.remove(&media);
                }
                None => {
                    access_cache.drain();
                }
            }
        }

        Ok(())
    }

    async fn is_valid_user(
        &self,
        auth_type: AuthType,
        _user: String,
        _password: String,
    ) -> anyhow::Result<bool> {
        match auth_type {
            AuthType::ProxyHeader => Ok(true),
            AuthType::LDAP => Err(anyhow::Error::msg("ldap auth not implemented")),
        }
    }

    async fn is_group_member(&self, uid: String, gid: HashSet<String>) -> anyhow::Result<bool> {
        let user_cache = self.user_cache.clone();

        {
            let user_cache = user_cache.read().await;

            match user_cache.get(&uid) {
                Some(members) => return Ok(members.intersection(&gid).count() > 0),
                None => {}
            }
        }

        let db_svc_sender = self.db_svc_sender.clone();

        let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<User>>();

        db_svc_sender
            .send(ESM::Db(DbMsg::GetUser { resp: tx, uid: uid }))
            .await?;

        // this should fail if the user doesn't exist, so we are safe to add the user
        // to the cache after it returns
        let user = rx.await??;

        let groups = user.groups;

        {
            let mut user_cache = user_cache.write().await;

            user_cache.insert(user.uid, groups.clone());
        }

        Ok(groups.intersection(&gid).count() > 0)
    }

    async fn can_access_media(&self, uid: String, uuid: MediaUuid) -> anyhow::Result<bool> {
        let access_cache = self.access_cache.clone();

        // in order to get the HashSet out of the HashMap, we need to match the outer get()
        // before cloning out of the block
        let cached_groups = {
            let access_cache = access_cache.read().await;

            let groups = match access_cache.get(&uuid) {
                None => None,
                Some(v) => Some(v.clone()),
            };

            groups
        };

        // since we have to compare the sets of groups, we have to handle the cache miss
        // before we check for an intersection
        //
        // this is outside the block with the read() mutex so we don't hold the lock any
        // longer than is strictly necessary
        let groups = match cached_groups {
            Some(v) => v,
            None => {
                let db_svc_sender = self.db_svc_sender.clone();

                let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<HashSet<String>>>();

                db_svc_sender
                    .send(ESM::Db(DbMsg::GetImageGroups {
                        resp: tx,
                        uuid: uuid,
                    }))
                    .await?;

                let groups = rx.await??;

                {
                    let mut access_cache = access_cache.write().await;

                    access_cache.insert(uuid, groups.clone());
                }

                groups
            }
        };

        Ok(self.is_group_member(uid, groups).await?)
    }
}

#[async_trait]
impl ESInner for AuthCache {
    fn new(senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<Self> {
        let db_svc_sender = senders
            .get(&ServiceType::Db)
            .ok_or_else(|| anyhow::Error::msg("failed to find Db sender for AuthCache"))?;

        Ok(AuthCache {
            db_svc_sender: db_svc_sender.clone(),
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
                AuthMsg::IsValidUser {
                    resp,
                    auth_type,
                    uid,
                    password,
                } => {
                    self.respond(resp, self.is_valid_user(auth_type, uid, password))
                        .await
                }
                AuthMsg::IsGroupMember { resp, uid, gid } => {
                    self.respond(resp, self.is_group_member(uid, gid)).await
                }
                AuthMsg::CanAccessMedia { resp, uid, uuid } => {
                    self.respond(resp, self.can_access_media(uid, uuid)).await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct AuthService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for AuthService {
    type Inner = AuthCache;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx.clone(),
            AuthService {
                config: config.clone(),
                sender: tx,
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(AuthCache::new(senders)?);

        // for the first pass, we don't need any further machinery for this service
        //
        // however, if we want things like timers, batching updates, or other optimizations,
        // they would all go here with corresponding handles in the AuthService struct

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
