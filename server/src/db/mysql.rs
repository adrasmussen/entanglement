use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use mysql_async::Pool;
use tokio::sync::Mutex;

use crate::auth::msg::*;
use crate::db::{msg::DbMsg, ESDbService};
use crate::service::*;
use api::{album::*, comment::*, library::*, media::*};
use common::config::ESConfig;

// mysql database backend
//
// initially, the choice to roll a manual ORM was due to diesel async having spotty support
// and a desire to see what is going on under the hood
//
// needless to say, everything about this is terrible
pub struct MySQLService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for MySQLService {
    type Inner = MySQLState;

    fn create(config: Arc<ESConfig>, sender_map: &mut HashMap<ServiceType, ESMSender>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        sender_map.insert(ServiceType::Db, tx);

        MySQLService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    async fn start(&self, senders: &HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(MySQLState::new(self.config.clone(), senders.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("mysql_service failed to reply to message"),
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

pub struct MySQLState {
    auth_svc_sender: ESMSender,
    pool: Pool,
}

impl MySQLState {
    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::ClearAccessCache {
                    resp: tx,
                    uuid: media_uuid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }
}

// database RPC handler functions
#[async_trait]
impl ESDbService for MySQLState {
    // auth queries
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>> {
        common::db::mysql::media_access_groups(self.pool.clone(), media_uuid).await
    }

    // media queries
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid> {
        common::db::mysql::add_media(self.pool.clone(), media).await
    }

    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> anyhow::Result<Option<(Media, Vec<AlbumUuid>, Vec<CommentUuid>)>> {
        common::db::mysql::get_media(self.pool.clone(), media_uuid).await
    }

    async fn get_media_uuid_by_path(&self, path: String) -> anyhow::Result<Option<MediaUuid>> {
        common::db::mysql::get_media_uuid_by_path(self.pool.clone(), path).await
    }

    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> anyhow::Result<()> {
        common::db::mysql::update_media(self.pool.clone(), media_uuid, update).await
    }

    async fn search_media(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        common::db::mysql::search_media(self.pool.clone(), uid, gid, filter).await
    }

    // comment queries
    async fn add_comment(&self, comment: Comment) -> anyhow::Result<CommentUuid> {
        common::db::mysql::add_comment(self.pool.clone(), comment).await
    }

    async fn get_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<Option<Comment>> {
        common::db::mysql::get_comment(self.pool.clone(), comment_uuid).await
    }

    async fn delete_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<()> {
        common::db::mysql::delete_comment(self.pool.clone(), comment_uuid).await
    }

    async fn update_comment(
        &self,
        comment_uuid: CommentUuid,
        text: Option<String>,
    ) -> anyhow::Result<()> {
        common::db::mysql::update_comment(self.pool.clone(), comment_uuid, text).await
    }

    // album queries
    async fn add_album(&self, album: Album) -> anyhow::Result<AlbumUuid> {
        common::db::mysql::add_album(self.pool.clone(), album).await
    }

    async fn get_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<Option<Album>> {
        common::db::mysql::get_album(self.pool.clone(), album_uuid).await
    }

    async fn delete_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<()> {
        common::db::mysql::delete_album(self.pool.clone(), album_uuid).await

        // should the function return a list of affected media to clear the cache?
    }

    async fn update_album(&self, album_uuid: AlbumUuid, update: AlbumUpdate) -> anyhow::Result<()> {
        common::db::mysql::update_album(self.pool.clone(), album_uuid, update).await
    }

    async fn add_media_to_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        common::db::mysql::add_media_to_album(self.pool.clone(), media_uuid, album_uuid).await?;

        self.clear_access_cache(Vec::from([media_uuid.clone()]))
            .await?;

        Ok(())
    }

    async fn rm_media_from_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        common::db::mysql::rm_media_from_album(self.pool.clone(), media_uuid, album_uuid).await?;

        self.clear_access_cache(Vec::from([media_uuid.clone()]))
            .await?;

        Ok(())
    }

    async fn search_albums(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<AlbumUuid>> {
        common::db::mysql::search_albums(self.pool.clone(), uid, gid, filter).await
    }

    async fn search_media_in_album(
        &self,
        uid: String,
        gid: HashSet<String>,
        album_uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        common::db::mysql::search_media_in_album(self.pool.clone(), uid, gid, album_uuid, filter)
            .await
    }

    // library queries
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid> {
        common::db::mysql::add_library(self.pool.clone(), library).await
    }

    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Option<Library>> {
        common::db::mysql::get_library(self.pool.clone(), library_uuid).await
    }

    async fn update_library(
        &self,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    ) -> anyhow::Result<()> {
        common::db::mysql::update_library(self.pool.clone(), library_uuid, update).await
    }

    async fn search_libraries(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<LibraryUuid>> {
        common::db::mysql::search_libraries(self.pool.clone(), uid, gid, filter).await
    }

    async fn search_media_in_library(
        &self,
        uid: String,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        common::db::mysql::search_media_in_library(
            self.pool.clone(),
            uid,
            gid,
            library_uuid,
            filter,
            hidden,
        )
        .await
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(MySQLState {
            auth_svc_sender: senders.get(&ServiceType::Auth).unwrap().clone(),
            pool: Pool::new(config.mysql_url.clone().as_str()),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                // auth messages
                DbMsg::MediaAccessGroups { resp, media_uuid } => {
                    self.respond(resp, self.media_access_groups(media_uuid))
                        .await
                }

                // media messages
                DbMsg::AddMedia { resp, media } => self.respond(resp, self.add_media(media)).await,
                DbMsg::GetMedia { resp, media_uuid } => {
                    self.respond(resp, self.get_media(media_uuid)).await
                }
                DbMsg::GetMediaUuidByPath { resp, path } => {
                    self.respond(resp, self.get_media_uuid_by_path(path)).await
                }
                DbMsg::UpdateMedia {
                    resp,
                    media_uuid,
                    update,
                } => {
                    self.respond(resp, self.update_media(media_uuid, update))
                        .await
                }
                DbMsg::SearchMedia {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.search_media(uid, gid, filter))
                        .await
                }

                // comment messages
                DbMsg::AddComment { resp, comment } => {
                    self.respond(resp, self.add_comment(comment)).await
                }
                DbMsg::GetComment { resp, comment_uuid } => {
                    self.respond(resp, self.get_comment(comment_uuid)).await
                }
                DbMsg::DeleteComment { resp, comment_uuid } => {
                    self.respond(resp, self.delete_comment(comment_uuid)).await
                }
                DbMsg::UpdateComment {
                    resp,
                    comment_uuid,
                    text,
                } => {
                    self.respond(resp, self.update_comment(comment_uuid, text))
                        .await
                }

                // album messages
                DbMsg::AddAlbum { resp, album } => self.respond(resp, self.add_album(album)).await,
                DbMsg::GetAlbum { resp, album_uuid } => {
                    self.respond(resp, self.get_album(album_uuid)).await
                }
                DbMsg::DeleteAlbum { resp, album_uuid } => {
                    self.respond(resp, self.delete_album(album_uuid)).await
                }
                DbMsg::UpdateAlbum {
                    resp,
                    album_uuid,
                    update,
                } => {
                    self.respond(resp, self.update_album(album_uuid, update))
                        .await
                }
                DbMsg::AddMediaToAlbum {
                    resp,
                    media_uuid,
                    album_uuid,
                } => {
                    self.respond(resp, self.add_media_to_album(media_uuid, album_uuid))
                        .await
                }
                DbMsg::RmMediaFromAlbum {
                    resp,
                    media_uuid,
                    album_uuid,
                } => {
                    self.respond(resp, self.rm_media_from_album(media_uuid, album_uuid))
                        .await
                }
                DbMsg::SearchAlbums {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.search_albums(uid, gid, filter))
                        .await
                }
                DbMsg::SearchMediaInAlbum {
                    resp,
                    uid,
                    gid,
                    album_uuid,
                    filter,
                } => {
                    self.respond(
                        resp,
                        self.search_media_in_album(uid, gid, album_uuid, filter),
                    )
                    .await
                }

                // library messages
                DbMsg::AddLibrary { resp, library } => {
                    self.respond(resp, self.add_library(library)).await
                }
                DbMsg::GetLibrary { resp, library_uuid } => {
                    self.respond(resp, self.get_library(library_uuid)).await
                }
                DbMsg::UpdateLibrary {
                    resp,
                    library_uuid,
                    update,
                } => {
                    self.respond(resp, self.update_library(library_uuid, update))
                        .await
                }
                DbMsg::SearchLibraries {
                    resp,
                    uid,
                    gid,
                    filter,
                } => {
                    self.respond(resp, self.search_libraries(uid, gid, filter))
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
                        self.search_media_in_library(uid, gid, library_uuid, filter, hidden),
                    )
                    .await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}
