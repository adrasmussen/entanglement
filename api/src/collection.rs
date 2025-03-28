use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{endpoint, media::MediaUuid, search::SearchFilter};

// structs and types

pub type CollectionUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Collection {
    pub uid: String,
    pub gid: String,
    pub mtime: i64,
    pub name: String,
    pub note: String,
    pub tags: HashSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionUpdate {
    pub name: Option<String>,
    pub note: Option<String>,
    pub tags: Option<HashSet<String>>,
}

// messages

// create a new collection
endpoint!(AddCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddCollectionReq {
    pub collection: Collection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddCollectionResp {
    pub collection_uuid: CollectionUuid,
}

// get details on an collection
//
// note that we fetch the media with
// a blank filter in another call
endpoint!(GetCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCollectionReq {
    pub collection_uuid: CollectionUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCollectionResp {
    pub collection: Collection,
}

// delete an collection
endpoint!(DeleteCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteCollectionReq {
    pub collection_uuid: CollectionUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteCollectionResp {}

// change collection properties
endpoint!(UpdateCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub update: CollectionUpdate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateCollectionResp {}

// add media to an collection
endpoint!(AddMediaToCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToCollectionResp {}

// remove media from an collection
endpoint!(RmMediaFromCollection);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromCollectionResp {}

// search collections
//
// defaults to ""
endpoint!(SearchCollections);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchCollectionsReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchCollectionsResp {
    pub collections: Vec<CollectionUuid>,
}

// search media inside a particular collection
endpoint!(SearchMediaInCollection);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchMediaInCollectionReq {
    pub collection_uuid: CollectionUuid,
    pub filter: SearchFilter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInCollectionResp {
    pub media: Vec<MediaUuid>,
}
