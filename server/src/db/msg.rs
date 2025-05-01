use std::collections::HashSet;

use api::{collection::*, comment::*, library::*, media::*, search::SearchFilter};

use crate::service::*;

#[derive(Debug)]
pub enum DbMsg {
    // auth messages
    MediaAccessGroups {
        resp: EsmResp<HashSet<String>>,
        media_uuid: MediaUuid,
    },

    // media messages
    AddMedia {
        resp: EsmResp<MediaUuid>,
        media: Media,
    },
    GetMedia {
        #[allow(clippy::type_complexity)]
        resp: EsmResp<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>>,
        media_uuid: MediaUuid,
    },
    GetMediaUuidByPath {
        resp: EsmResp<Option<MediaUuid>>,
        path: String,
    },
    GetMediaUuidByCHash {
        resp: EsmResp<Option<MediaUuid>>,
        library_uuid: LibraryUuid,
        chash: String,
    },
    UpdateMedia {
        resp: EsmResp<()>,
        media_uuid: MediaUuid,
        update: MediaUpdate,
    },
    ReplaceMediaPath {
        resp: EsmResp<()>,
        media_uuid: MediaUuid,
        path: String,
    },
    SearchMedia {
        resp: EsmResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        filter: SearchFilter,
    },
    SimilarMedia {
        resp: EsmResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    },

    // comment messages
    AddComment {
        resp: EsmResp<CommentUuid>,
        comment: Comment,
    },
    GetComment {
        resp: EsmResp<Option<Comment>>,
        comment_uuid: CommentUuid,
    },
    DeleteComment {
        resp: EsmResp<()>,
        comment_uuid: CommentUuid,
    },
    UpdateComment {
        resp: EsmResp<()>,
        comment_uuid: CommentUuid,
        text: Option<String>,
    },

    // collection messages
    AddCollection {
        resp: EsmResp<CollectionUuid>,
        collection: Collection,
    },
    GetCollection {
        resp: EsmResp<Option<Collection>>,
        collection_uuid: CollectionUuid,
    },
    DeleteCollection {
        resp: EsmResp<()>,
        collection_uuid: CollectionUuid,
    },
    UpdateCollection {
        resp: EsmResp<()>,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    },
    AddMediaToCollection {
        resp: EsmResp<()>,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    },
    RmMediaFromCollection {
        resp: EsmResp<()>,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    },
    SearchCollections {
        resp: EsmResp<Vec<CollectionUuid>>,
        gid: HashSet<String>,
        filter: SearchFilter,
    },
    SearchMediaInCollection {
        resp: EsmResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    },

    // library messages
    _AddLibrary {
        resp: EsmResp<LibraryUuid>,
        library: Library,
    },
    GetLibrary {
        resp: EsmResp<Option<Library>>,
        library_uuid: LibraryUuid,
    },
    UpdateLibrary {
        resp: EsmResp<()>,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    },
    SearchLibraries {
        resp: EsmResp<Vec<LibraryUuid>>,
        gid: HashSet<String>,
        filter: String,
    },
    SearchMediaInLibrary {
        resp: EsmResp<Vec<MediaUuid>>,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: bool,
        filter: SearchFilter,
    },
}

impl From<DbMsg> for Esm {
    fn from(value: DbMsg) -> Self {
        Esm::Db(value)
    }
}
