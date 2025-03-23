use std::collections::HashSet;

use api::{collection::*, comment::*, library::*, media::*, task::*, search::SearchFilter};

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
    UpdateMedia {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        update: MediaUpdate,
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
        filter: String,
    },
    SearchMediaInCollection {
        resp: ESMResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: String,
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
        filter: String,
        hidden: bool,
    },
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
