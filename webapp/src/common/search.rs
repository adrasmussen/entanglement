use std::collections::HashSet;

use crate::common::paging::{Page, Reconcile};
use api::{
    WebError,
    collection::{CollectionUuid, search_media_in_collection},
    library::search_media_in_library,
    media::{GetMediaReq, GetMediaResp, MediaUuid, get_media, search_media},
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
        opts.page.limit = None;
        opts.page.offset = 0;
        opts.page.with_total = false;
    }

    let uuids: Vec<MediaUuid> = match req {
        SearchRequest::Media(r) => search_media(&r).await?.media,
        SearchRequest::Collection(r) => search_media_in_collection(&r).await?.media,
        SearchRequest::Library(r) => search_media_in_library(&r).await?.media,
    };

    Ok(uuids.into_iter().collect())
}

// the collection and library search boxes match one literal substring, so the words of the
// box are rejoined with single spaces (collecting them straight into a String would glue
// them together: "my trip" would search for "mytrip")
pub fn literal_filter(search: &str) -> String {
    search.split_whitespace().collect::<Vec<_>>().join(" ")
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
        opts.page.offset = offset;
        opts.page.limit = Some(limit);
        opts.page.with_total = with_total;
    }

    let resp = batch_search_and_sort(&BatchSearchAndSortReq { req }).await?;

    Ok(Page {
        items: resp.media,
        total: resp.total,
        page_len: resp.page_len,
    })
}

// which grid an item sits in, for deciding what an edit of it means (see reconcile_media)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridScope {
    // the gallery and collections only ever show visible media
    Gallery,
    Collection(CollectionUuid),
    // the library grid shows either the visible or the hidden media, per its checkbox
    Library { hidden: bool },
}

// the effect of an edit on one grid: `old` is the item as cached, `fresh` what the server
// has now, and `filtered` whether a search filter is active in the grid
pub fn decide_reconcile(
    scope: GridScope,
    filtered: bool,
    old: &SearchResponse,
    fresh: &GetMediaResp,
) -> Reconcile<SearchResponse> {
    let belongs = match scope {
        GridScope::Gallery => !fresh.media.hidden,
        GridScope::Collection(uuid) => !fresh.media.hidden && fresh.collections.contains(&uuid),
        GridScope::Library { hidden } => fresh.media.hidden == hidden,
    };
    if !belongs {
        return Reconcile::Remove;
    }

    // sorted by date, so a new date can move the item
    if fresh.media.date != old.media.date {
        return Reconcile::Refresh;
    }

    // the search matches the note, tags and date; any of those changing might add or drop
    // the item from the results
    if filtered && (fresh.media.note != old.media.note || fresh.media.tags != old.media.tags) {
        return Reconcile::Refresh;
    }

    Reconcile::Patch(SearchResponse {
        media_uuid: old.media_uuid,
        media: fresh.media.clone(),
        collections: fresh.collections.clone(),
        comments: fresh.comments.clone(),
    })
}

// fetch the current record of an item that was edited behind the grid's back and decide what
// to do with it there.  a failed lookup (which is what a deleted media looks like) is left
// to the caller, who falls back to refreshing the grid
pub async fn reconcile_media(
    scope: GridScope,
    search: &str,
    old: SearchResponse,
) -> Result<Reconcile<SearchResponse>, WebError> {
    let fresh = get_media(&GetMediaReq {
        media_uuid: old.media_uuid,
    })
    .await?;

    let filtered = search.split_whitespace().next().is_some();
    Ok(decide_reconcile(scope, filtered, &old, &fresh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_filter_keeps_the_spaces() {
        assert_eq!(literal_filter("my trip"), "my trip");
        assert_eq!(literal_filter("  my   trip\t"), "my trip");
        assert_eq!(literal_filter("   "), "");
    }

    use api::{
        UuidSource,
        library::LibraryUuid,
        media::{Media, MediaMetadata},
    };

    struct Src;
    impl UuidSource for Src {}

    fn uuid_str(n: u8) -> String {
        format!("00000000-0000-0000-0000-0000000000{n:02}")
    }

    fn media(date: &str, note: &str, hidden: bool) -> Media {
        Media {
            library_uuid: LibraryUuid::try_parse(&Src, &uuid_str(9)).unwrap(),
            path: String::from("a.jpg"),
            size: 1,
            chash: String::new(),
            phash: String::new(),
            mtime: 0,
            hidden,
            date: date.to_owned(),
            note: note.to_owned(),
            tags: Default::default(),
            metadata: MediaMetadata::Image,
        }
    }

    fn old(m: Media, collections: Vec<CollectionUuid>) -> SearchResponse {
        SearchResponse {
            media_uuid: MediaUuid::try_parse(&Src, &uuid_str(1)).unwrap(),
            media: m,
            collections,
            comments: Vec::new(),
        }
    }

    fn fresh(m: Media, collections: Vec<CollectionUuid>) -> GetMediaResp {
        GetMediaResp {
            media: m,
            collections,
            comments: Vec::new(),
        }
    }

    fn coll(n: u8) -> CollectionUuid {
        CollectionUuid::try_parse(&Src, &uuid_str(n)).unwrap()
    }

    #[test]
    fn hidden_leaves_gallery_and_collection() {
        let o = old(media("2020-01-01", "", false), vec![coll(2)]);
        let f = fresh(media("2020-01-01", "", true), vec![coll(2)]);

        assert!(matches!(
            decide_reconcile(GridScope::Gallery, false, &o, &f),
            Reconcile::Remove
        ));
        assert!(matches!(
            decide_reconcile(GridScope::Collection(coll(2)), false, &o, &f),
            Reconcile::Remove
        ));
    }

    #[test]
    fn library_membership_follows_the_hidden_toggle() {
        let o = old(media("2020-01-01", "", false), vec![]);
        let f = fresh(media("2020-01-01", "", true), vec![]);

        assert!(matches!(
            decide_reconcile(GridScope::Library { hidden: false }, false, &o, &f),
            Reconcile::Remove
        ));
        assert!(matches!(
            decide_reconcile(GridScope::Library { hidden: true }, false, &o, &f),
            Reconcile::Patch(_)
        ));
    }

    #[test]
    fn leaving_the_collection_removes() {
        let o = old(media("2020-01-01", "", false), vec![coll(2), coll(3)]);
        let f = fresh(media("2020-01-01", "", false), vec![coll(3)]);

        assert!(matches!(
            decide_reconcile(GridScope::Collection(coll(2)), false, &o, &f),
            Reconcile::Remove
        ));
        // still a member of 3, and the gallery does not care at all
        assert!(matches!(
            decide_reconcile(GridScope::Collection(coll(3)), false, &o, &f),
            Reconcile::Patch(_)
        ));
        assert!(matches!(
            decide_reconcile(GridScope::Gallery, false, &o, &f),
            Reconcile::Patch(_)
        ));
    }

    #[test]
    fn date_change_refreshes() {
        let o = old(media("2020-01-01", "", false), vec![]);
        let f = fresh(media("2021-01-01", "", false), vec![]);

        assert!(matches!(
            decide_reconcile(GridScope::Gallery, false, &o, &f),
            Reconcile::Refresh
        ));
    }

    #[test]
    fn note_change_refreshes_only_when_filtered() {
        let o = old(media("2020-01-01", "a", false), vec![]);
        let f = fresh(media("2020-01-01", "b", false), vec![]);

        match decide_reconcile(GridScope::Gallery, false, &o, &f) {
            Reconcile::Patch(p) => assert_eq!(p.media.note, "b"),
            _ => panic!("expected a patch"),
        }
        assert!(matches!(
            decide_reconcile(GridScope::Gallery, true, &o, &f),
            Reconcile::Refresh
        ));
    }
}
