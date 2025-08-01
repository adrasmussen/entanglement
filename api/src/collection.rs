use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{http_endpoint, media::MediaUuid, search::SearchFilter};

// structs and types

pub type CollectionUuid = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Collection {
    pub uid: String,
    pub gid: String,
    pub mtime: u64,
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
// defaults to ""
http_endpoint!(SearchCollections);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchCollectionsReq {
    pub filter: SearchFilter,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchCollectionsResp {
    pub collections: Vec<CollectionUuid>,
}

// search media inside a particular collection
http_endpoint!(SearchMediaInCollection);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchMediaInCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub filter: SearchFilter,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMediaInCollectionResp {
    pub media: Vec<MediaUuid>,
}
