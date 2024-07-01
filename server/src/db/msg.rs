use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    _GetConn,
    ImageListQuery {
        resp: ESMResp<()>,
        user: String,
        filter: String, // eventually replace with ImageFIlter object from lib
    },
    EditAlbum {
        resp: ESMResp<()>,
        user: String,
        album: String,
        data: (),
    }

}
