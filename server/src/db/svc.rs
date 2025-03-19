use std::marker::PhantomData;
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::db::{msg::DbMsg, ESDbRunner};
use crate::service::{ESInner, ESMReceiver, ESMRegistry, EntanglementService, ServiceType, ESM};
use common::config::ESConfig;
use common::db::DbBackend;

pub struct DbService<T: DbBackend> {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
    backend: PhantomData<T>,
}

#[async_trait]
impl<T: DbBackend> EntanglementService for DbService<T> {
    type Inner = DbRunner<T>;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        registry
            .insert(ServiceType::Db, tx)
            .expect("failed to add db sender to registry");

        DbService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
            backend: PhantomData::<T>,
        }
    }

    async fn start(&self, registry: &ESMRegistry) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(DbRunner::<T>::new(self.config.clone(), registry.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        let esmstr = format!("{msg:?}");

                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => println!(
                                "mysql_service failed to reply to message!\nError: {err}\nMessage: {esmstr}"
                            ),
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

pub struct DbRunner<T: DbBackend> {
    registry: ESMRegistry,
    backend: T,
}

#[async_trait]
impl<T: DbBackend> ESInner for DbRunner<T> {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> anyhow::Result<Self> {
        Ok(DbRunner {
            registry: registry.clone(),
            backend: T::new(config.clone())?,
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                // auth messages
                DbMsg::MediaAccessGroups { resp, media_uuid } => {
                    self.respond(resp, self.backend.media_access_groups(media_uuid))
                        .await
                }

                // media messages
                DbMsg::AddMedia { resp, media } => {
                    self.respond(resp, self.backend.add_media(media)).await
                }
                DbMsg::GetMedia { resp, media_uuid } => {
                    self.respond(resp, self.backend.get_media(media_uuid)).await
                }
                DbMsg::GetMediaUuidByPath { resp, path } => {
                    self.respond(resp, self.backend.get_media_uuid_by_path(path))
                        .await
                }
                DbMsg::UpdateMedia {
                    resp,
                    media_uuid,
                    update,
                } => {
                    self.respond(resp, self.backend.update_media(media_uuid, update))
                        .await
                }
                DbMsg::SearchMedia {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.backend.search_media(uid, gid, filter))
                        .await
                }
                DbMsg::SimilarMedia {
                    resp,
                    uid,
                    gid,
                    media_uuid,
                    distance,
                } => {
                    self.respond(
                        resp,
                        self.backend.similar_media(uid, gid, media_uuid, distance),
                    )
                    .await
                }

                // comment messages
                DbMsg::AddComment { resp, comment } => {
                    self.respond(resp, self.backend.add_comment(comment)).await
                }
                DbMsg::GetComment { resp, comment_uuid } => {
                    self.respond(resp, self.backend.get_comment(comment_uuid))
                        .await
                }
                DbMsg::DeleteComment { resp, comment_uuid } => {
                    self.respond(resp, self.backend.delete_comment(comment_uuid))
                        .await
                }
                DbMsg::UpdateComment {
                    resp,
                    comment_uuid,
                    text,
                } => {
                    self.respond(resp, self.backend.update_comment(comment_uuid, text))
                        .await
                }

                // collection messages
                DbMsg::AddCollection { resp, collection } => {
                    self.respond(resp, self.backend.add_collection(collection))
                        .await
                }
                DbMsg::GetCollection {
                    resp,
                    collection_uuid,
                } => {
                    self.respond(resp, self.backend.get_collection(collection_uuid))
                        .await
                }
                DbMsg::DeleteCollection {
                    resp,
                    collection_uuid,
                } => {
                    self.respond(resp, self.backend.delete_collection(collection_uuid))
                        .await
                }
                DbMsg::UpdateCollection {
                    resp,
                    collection_uuid,
                    update,
                } => {
                    self.respond(
                        resp,
                        self.backend.update_collection(collection_uuid, update),
                    )
                    .await
                }
                DbMsg::AddMediaToCollection {
                    resp,
                    media_uuid,
                    collection_uuid,
                } => {
                    self.respond(
                        resp,
                        self.backend
                            .add_media_to_collection(media_uuid, collection_uuid),
                    )
                    .await
                }
                DbMsg::RmMediaFromCollection {
                    resp,
                    media_uuid,
                    collection_uuid,
                } => {
                    self.respond(
                        resp,
                        self.backend
                            .rm_media_from_collection(media_uuid, collection_uuid),
                    )
                    .await
                }
                DbMsg::SearchCollections {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.backend.search_collections(uid, gid, filter))
                        .await
                }
                DbMsg::SearchMediaInCollection {
                    resp,
                    uid,
                    gid,
                    collection_uuid,
                    filter,
                } => {
                    self.respond(
                        resp,
                        self.backend
                            .search_media_in_collection(uid, gid, collection_uuid, filter),
                    )
                    .await
                }

                // library messages
                DbMsg::AddLibrary { resp, library } => {
                    self.respond(resp, self.backend.add_library(library)).await
                }
                DbMsg::GetLibrary { resp, library_uuid } => {
                    self.respond(resp, self.backend.get_library(library_uuid))
                        .await
                }
                DbMsg::UpdateLibrary {
                    resp,
                    library_uuid,
                    update,
                } => {
                    self.respond(resp, self.backend.update_library(library_uuid, update))
                        .await
                }
                DbMsg::SearchLibraries {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.backend.search_libraries(uid, gid, filter))
                        .await
                }
                DbMsg::SearchMediaInLibrary {
                    resp,
                    uid,
                    gid,
                    library_uuid,
                    filter,
                    hidden,
                } => {
                    self.respond(
                        resp,
                        self.backend.search_media_in_library(
                            uid,
                            gid,
                            library_uuid,
                            filter,
                            hidden,
                        ),
                    )
                    .await
                }
                _ => panic!(),
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}
