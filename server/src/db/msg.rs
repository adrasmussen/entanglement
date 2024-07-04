use api::{Album, AlbumMetadata, Image, ImageMetadata, Library, LibraryMetadata};

use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    _GetConn,
    AddImage {
        resp: ESMResp<()>,
        image: Image,
    },
    AddAlbum {
        resp: ESMResp<()>,
        album: Album,
    },
    AddLibrary {
        resp: ESMResp<()>,
        library: Library,
    },
    ImageListQuery {
        resp: ESMResp<()>,
        user: String,
        filter: String, // eventually replace with ImageFIlter object from lib
    },
    UpdateImage {
        resp: ESMResp<()>,
        user: String,
        image: String,
        change: ImageMetadata,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        user: String,
        album: String,
        change: AlbumMetadata,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        user: String,
        library: String,
        change: LibraryMetadata,
    },
}
