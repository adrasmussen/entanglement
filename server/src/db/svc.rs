use std::marker::PhantomData;
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument};

use crate::db::msg::DbMsg;
use crate::service::{ESInner, ESMReceiver, ESMRegistry, EntanglementService, ServiceType, ESM};
use common::config::ESConfig;
use common::db::DbBackend;

pub struct DbService<B: DbBackend> {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
    backend: PhantomData<B>,
}

#[async_trait]
impl<B: DbBackend> EntanglementService for DbService<B> {
    type Inner = DbRunner<B>;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        registry
            .insert(ServiceType::Db, tx)
            .expect("failed to add db sender to registry");

        DbService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
            backend: PhantomData::<B>,
        }
    }

    #[instrument(skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> anyhow::Result<()> {
        info!("starting db service");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(DbRunner::<B>::new(self.config.clone(), registry.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "task", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        debug!("started db service");

        Ok(())
    }
}

pub struct DbRunner<B: DbBackend> {
    registry: ESMRegistry,
    backend: B,
}

#[async_trait]
impl<B: DbBackend> ESInner for DbRunner<B> {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> anyhow::Result<Self> {
        Ok(DbRunner {
            registry: registry.clone(),
            backend: B::new(config.clone())?,
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
                DbMsg::GetMediaUuidByCHash {
                    resp,
                    library_uuid,
                    chash,
                } => {
                    self.respond(
                        resp,
                        self.backend.get_media_uuid_by_chash(library_uuid, chash),
                    )
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
                DbMsg::ReplaceMediaPath {
                    resp,
                    media_uuid,
                    path,
                } => {
                    self.respond(resp, self.backend.replace_media_path(media_uuid, path))
                        .await
                }
                DbMsg::SearchMedia { resp, gid, filter } => {
                    self.respond(resp, self.backend.search_media(gid, filter))
                        .await
                }
                DbMsg::SimilarMedia {
                    resp,
                    gid,
                    media_uuid,
                    distance,
                } => {
                    self.respond(resp, self.backend.similar_media(gid, media_uuid, distance))
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
                DbMsg::SearchCollections { resp, gid, filter } => {
                    self.respond(resp, self.backend.search_collections(gid, filter))
                        .await
                }
                DbMsg::SearchMediaInCollection {
                    resp,
                    gid,
                    collection_uuid,
                    filter,
                } => {
                    self.respond(
                        resp,
                        self.backend
                            .search_media_in_collection(gid, collection_uuid, filter),
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
                DbMsg::SearchLibraries { resp, gid, filter } => {
                    self.respond(resp, self.backend.search_libraries(gid, filter))
                        .await
                }
                DbMsg::SearchMediaInLibrary {
                    resp,
                    gid,
                    library_uuid,
                    hidden,
                    filter,
                } => {
                    self.respond(
                        resp,
                        self.backend
                            .search_media_in_library(gid, library_uuid, hidden, filter),
                    )
                    .await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}
