
use api::ImageVisibility;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum CacheMsg {
    ClearAllCaches {
        resp: ESMResp<()>,
    },
    _GetImageVisibility {
        resp: ESMResp<ImageVisibility>,
        image: String,
    }
}
