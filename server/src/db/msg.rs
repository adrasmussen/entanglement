use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    _GetConn,
    _ImageListQuery {
        resp: ESMResp<Box<Vec<()>>>,
        user: String,
        filter: String, // eventually replace with ImageFIlter object from lib
    }
}
