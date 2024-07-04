
use api::Visibility;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum CacheMsg {
    ClearAllCaches {
        resp: ESMResp<()>,
    },
    GetImageVisibility {
        resp: ESMResp<Visibility>,
        image: String,
    }
}
