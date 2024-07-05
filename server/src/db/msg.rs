use api::*;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    AddImage {
        resp: ESMResp<()>,
        image: Image,
    },
    GetImage {
        resp: ESMResp<Image>,
        uuid: ImageUUID,
    },
    UpdateImage {
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUUID,
        change: ImageMetadata,
    },
    FilterImages {
        resp: ESMResp<()>,
        user: String,
        filter: String, // eventually replace with ImageFIlter object from lib
    },
    AddAlbum {
        resp: ESMResp<()>,
        uuid: Album,
    },
    GetAlbum {
        resp: ESMResp<Album>,
        uuid: AlbumUUID,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUUID,
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
        uuid: LibraryUUID,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUUID,
        change: LibraryMetadata,
    },
}
