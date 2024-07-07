use std::collections::HashMap;

use api::*;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    AddImage {
        resp: ESMResp<ImageUuid>,
        image: Image,
    },
    GetImage {
        resp: ESMResp<Image>,
        uuid: ImageUuid,
    },
    UpdateImage {
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    },
    FilterImages {
        resp: ESMResp<HashMap<ImageUuid, Image>>,
        user: String,
        filter: ImageFilter,
    },
    AddAlbum {
        resp: ESMResp<()>,
        uuid: Album,
    },
    GetAlbum {
        resp: ESMResp<Album>,
        uuid: AlbumUuid,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    },
    FilterAlbums {
        resp: ESMResp<()>,
        user: String,
        filter: String, // eventually replace with AlbumFIlter object from lib
    },
    AddLibrary {
        resp: ESMResp<()>,
        library: Library,
    },
    GetLibary {
        resp: ESMResp<Library>,
        uuid: LibraryUuid,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUuid,
        change: LibraryMetadata,
    },
}
