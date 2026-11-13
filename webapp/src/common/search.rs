use std::collections::HashSet;

use crate::common::paging::Page;
use api::{
    WebError,
    collection::search_media_in_collection,
    library::search_media_in_library,
    media::{MediaUuid, search_media},
    search::{
        BatchSearchAndSortReq, SearchFilter, SearchOptions, SearchRequest, SearchResponse,
        SortMethod, batch_search_and_sort,
    },
};

// fetch the uuids of every match for a search (ignoring any paging in the request),
// using the uuid-only endpoints so that no media records need to be hydrated
pub async fn fetch_all_uuids(mut req: SearchRequest) -> Result<HashSet<MediaUuid>, WebError> {
    {
        let opts = req.opts_mut();
        opts.limit = None;
        opts.offset = 0;
        opts.with_total = false;
    }

    let uuids: Vec<MediaUuid> = match req {
        SearchRequest::Media(r) => search_media(&r).await?.media,
        SearchRequest::Collection(r) => search_media_in_collection(&r).await?.media,
        SearchRequest::Library(r) => search_media_in_library(&r).await?.media,
    };

    Ok(uuids.into_iter().collect())
}

// the (unpaged) options for a media search box string
pub fn search_opts(search: &str) -> SearchOptions {
    SearchOptions {
        filter: SearchFilter::SubstringAny {
            filter: search.split_whitespace().map(|s| s.to_owned()).collect(),
        },
        order: SortMethod::DateDesc,
        ..Default::default()
    }
}

// fetch one page of hydrated media for a request, whose paging options are overwritten
pub async fn fetch_media_page(
    mut req: SearchRequest,
    offset: u64,
    limit: u64,
    with_total: bool,
) -> Result<Page<SearchResponse>, WebError> {
    {
        let opts = req.opts_mut();
        opts.offset = offset;
        opts.limit = Some(limit);
        opts.with_total = with_total;
    }

    let resp = batch_search_and_sort(&BatchSearchAndSortReq { req }).await?;

    Ok(Page {
        items: resp.media,
        total: resp.total,
    })
}
