use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use tokio::sync::{Mutex, RwLock};

use api::auth::Group;

use crate::service::*;

use crate::auth::ESAuthService;
use crate::auth::{msg::AuthMsg, AuthType};

use crate::db::msg::DbMsg;

struct AuthCache {
    db_svc_sender: ESMSender,
    group_cache: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    access_cache: Arc<RwLock<HashMap<String, HashSet<String>>>>,
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
    async fn clear_group_cache(&self) -> anyhow::Result<()> {
        let group_cache = self.group_cache.clone();

        {
            let mut group_cache = group_cache.write().await;

            group_cache.drain();
        }

        Ok(())
    }

    async fn clear_access_cache(&self) -> anyhow::Result<()> {
        let access_cache = self.access_cache.clone();

        {
            let mut access_cache = access_cache.write().await;

            access_cache.drain();
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

    async fn is_group_member(&self, uid: String, gid: String) -> anyhow::Result<bool> {
        let group_cache = self.group_cache.clone();

        {
            let group_cache = group_cache.read().await;

            match group_cache.get(&gid) {
                Some(members) => return Ok(members.contains(&uid)),
                None => {}
            }
        }

        let db_svc_sender = self.db_svc_sender.clone();

        let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<Group>>();

        db_svc_sender
            .send(ESM::Db(DbMsg::GetGroup { resp: tx, gid: gid }))
            .await?;

        // this should fail if the group doesn't exist, so we are safe to add the group
        // to the cache after it returns
        let group = rx.await??;

        let in_group = group.members.contains(&uid);

        {
            let mut group_cache = group_cache.write().await;

            group_cache.insert(
                group.gid,
                if in_group {
                    HashSet::from([uid])
                } else {
                    HashSet::new()
                },
            );
        }

        Ok(in_group)
    }

    async fn can_access_file(&self, uid: String, file: String) -> anyhow::Result<bool> {
        let access_cache = self.access_cache.clone();

        {
            let access_cache = access_cache.read().await;
        }

        Ok(false)
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
            group_cache: Arc::new(RwLock::new(HashMap::new())),
            access_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Auth(message) => match message {
                AuthMsg::ClearGroupCache { resp } => {
                    self.respond(resp, self.clear_group_cache()).await
                }
                AuthMsg::ClearAccessCache { resp } => {
                    self.respond(resp, self.clear_access_cache()).await
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
                AuthMsg::CanAccessFile { resp, uid, file } => {
                    self.respond(resp, self.can_access_file(uid, file)).await
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
        // falliable stuff can happen here
        //
        // need to get senders for any cacheable data, starting with database service
        //
        // possibly want to also spin up the timer threads here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(AuthCache::new(senders)?);

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
