use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    http_endpoint,
    media::MediaUuid,
    page::{PageMeta, PageRequest, PagedResp},
    search::SearchOptions,
    uuid_newtype,
};

// structs

uuid_newtype!(Collection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Collection {
    pub uid: String,
    pub gid: String,
    pub name: String,
    pub note: String,
    pub tags: HashSet<String>,
    pub cover: Option<MediaUuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CollectionUpdate {
    pub name: Option<String>,
    pub note: Option<String>,
    pub tags: Option<HashSet<String>>,
}

// messages

// create a new collection
http_endpoint!(AddCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddCollectionReq {
    pub collection: Collection,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddCollectionResp {
    pub collection_uuid: CollectionUuid,
}

// get details on an collection
//
// note that we fetch the media with
// a blank filter in another call
http_endpoint!(GetCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCollectionReq {
    pub collection_uuid: CollectionUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCollectionResp {
    pub collection: Collection,
}

// delete an collection
http_endpoint!(DeleteCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeleteCollectionReq {
    pub collection_uuid: CollectionUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeleteCollectionResp {}

// change collection properties
http_endpoint!(UpdateCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub update: CollectionUpdate,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateCollectionResp {}

// add media to an collection
http_endpoint!(AddMediaToCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddMediaToCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddMediaToCollectionResp {}

// remove media from an collection
http_endpoint!(RmMediaFromCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RmMediaFromCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RmMediaFromCollectionResp {}

// search collections
//
// filter defaults to "" (match everything) and is matched literally against the name;
// results are ordered by name.  limit = None returns every match on the uuid-only path,
// though callers that render cards should page with PAGE_SIZE
http_endpoint!(SearchCollections);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchCollectionsReq {
    pub filter: String,
    #[serde(flatten)]
    pub page: PageRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchCollectionsResp {
    pub collections: Vec<CollectionUuid>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub meta: PageMeta,
}

impl PagedResp for SearchCollectionsResp {
    type Item = CollectionUuid;

    fn into_parts(self) -> (Vec<CollectionUuid>, PageMeta) {
        (self.collections, self.meta)
    }
}

// search media inside a particular collection
http_endpoint!(SearchMediaInCollection);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMediaInCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub opts: SearchOptions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMediaInCollectionResp {
    pub media: Vec<MediaUuid>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub meta: PageMeta,
}

impl PagedResp for SearchMediaInCollectionResp {
    type Item = MediaUuid;

    fn into_parts(self) -> (Vec<MediaUuid>, PageMeta) {
        (self.media, self.meta)
    }
}
