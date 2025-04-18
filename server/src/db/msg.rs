use std::collections::HashSet;

use api::{collection::*, comment::*, library::*, media::*, search::SearchFilter};

use crate::service::*;

#[derive(Debug)]
pub enum DbMsg {
    // auth messages
    MediaAccessGroups {
        resp: ESMResp<HashSet<String>>,
        media_uuid: MediaUuid,
    },

    // media messages
    AddMedia {
        resp: ESMResp<MediaUuid>,
        media: Media,
    },
    GetMedia {
        resp: ESMResp<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>>,
        media_uuid: MediaUuid,
    },
    GetMediaUuidByPath {
        resp: ESMResp<Option<MediaUuid>>,
        path: String,
    },
    GetMediaUuidByCHash {
        resp: ESMResp<Option<MediaUuid>>,
        library_uuid: LibraryUuid,
        chash: String,
    },
    UpdateMedia {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        update: MediaUpdate,
    },
    ReplaceMediaPath {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        path: String,
    },
    SearchMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        filter: SearchFilter,
    },
    SimilarMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    },

    // comment messages
    AddComment {
        resp: ESMResp<CommentUuid>,
        comment: Comment,
    },
    GetComment {
        resp: ESMResp<Option<Comment>>,
        comment_uuid: CommentUuid,
    },
    DeleteComment {
        resp: ESMResp<()>,
        comment_uuid: CommentUuid,
    },
    UpdateComment {
        resp: ESMResp<()>,
        comment_uuid: CommentUuid,
        text: Option<String>,
    },

    // collection messages
    AddCollection {
        resp: ESMResp<CollectionUuid>,
        collection: Collection,
    },
    GetCollection {
        resp: ESMResp<Option<Collection>>,
        collection_uuid: CollectionUuid,
    },
    DeleteCollection {
        resp: ESMResp<()>,
        collection_uuid: CollectionUuid,
    },
    UpdateCollection {
        resp: ESMResp<()>,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    },
    AddMediaToCollection {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    },
    RmMediaFromCollection {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    },
    SearchCollections {
        resp: ESMResp<Vec<CollectionUuid>>,
        gid: HashSet<String>,
        filter: SearchFilter,
    },
    SearchMediaInCollection {
        resp: ESMResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    },

    // library messages
    AddLibrary {
        resp: ESMResp<LibraryUuid>,
        library: Library,
    },
    GetLibrary {
        resp: ESMResp<Option<Library>>,
        library_uuid: LibraryUuid,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    },
    SearchLibraries {
        resp: ESMResp<Vec<LibraryUuid>>,
        gid: HashSet<String>,
        filter: String,
    },
    SearchMediaInLibrary {
        resp: ESMResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: bool,
        filter: SearchFilter,
    },
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
