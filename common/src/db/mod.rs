use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::config::ESConfig;
use api::{
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaUpdate, MediaUuid},
    page::{PageMeta, PageRequest},
    search::SearchOptions,
};

pub mod mariadb;
pub use mariadb::MariaDBBackend;

pub mod postgres;
pub use postgres::PostgresBackend;

// these are the database RPC calls that any backend server must be able to process
#[async_trait]
pub trait DbBackend: Send + Sync + 'static {
    async fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized;

    // get this from checking all collections that contain the media + owning group of the library
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> Result<HashSet<String>>;

    // media functions
    async fn add_media(&self, media: Media) -> Result<MediaUuid>;

    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>>;

    // fetch full details for a page of media in one round of queries, preserving the
    // order of the input; uuids that no longer exist are skipped
    async fn get_media_batch(&self, media_uuids: Vec<MediaUuid>) -> Result<Vec<MediaDetails>>;

    async fn get_media_uuids(&self) -> Result<Vec<MediaUuid>>;

    async fn get_media_by_path(&self, path: String) -> Result<Option<MediaByPath>>;

    async fn get_media_by_chash(
        &self,
        library_uuid: LibraryUuid,
        chash: String,
    ) -> Result<Option<MediaByCHash>>;

    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()>;

    async fn replace_media_path(
        &self,
        media_uuid: MediaUuid,
        path: String,
        hash: String,
        mtime: u64,
    ) -> Result<()>;

    async fn search_media(
        &self,
        gid: HashSet<String>,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>>;

    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> Result<Vec<MediaUuid>>;

    // comment functions
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid>;

    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>>;

    async fn get_comment_uuids(&self) -> Result<Vec<CommentUuid>>;

    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()>;

    async fn update_comment(&self, comment_uuid: CommentUuid, update: Option<String>)
    -> Result<()>;

    // collection functions
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid>;

    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>>;

    async fn get_collection_uuids(&self) -> Result<Vec<CollectionUuid>>;

    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()>;

    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> Result<()>;

    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()>;

    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()>;

    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: String,
        page: PageRequest,
    ) -> Result<SearchPage<CollectionUuid>>;

    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>>;

    // library functions
    async fn add_library(&self, library: Library) -> Result<LibraryUuid>;

    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>>;

    async fn get_library_uuids(&self) -> Result<Vec<LibraryUuid>>;

    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()>;

    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
        page: PageRequest,
    ) -> Result<SearchPage<LibraryUuid>>;

    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: Option<bool>,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>>;
}

// paged search results
//
// items is the requested page (in sort order); total is the size of the full filtered
// set, and is only computed (via a separate COUNT) when the caller asked for it.
// has_more says whether rows exist past this page: backends fetch one extra row as a
// probe and trim it, so it is exact whenever a page query ran, and None only for a
// count-only request (limit = 0), where no page query runs
#[derive(Clone, Debug)]
pub struct SearchPage<T> {
    pub items: Vec<T>,
    pub total: Option<u64>,
    pub has_more: Option<bool>,
}

impl<T> SearchPage<T> {
    // the page for a count-only request: no rows, and nothing known about what follows
    pub fn count_only(total: Option<u64>) -> Self {
        Self {
            items: Vec::new(),
            total,
            has_more: None,
        }
    }

    // how many rows the page query should ask for: one more than the limit, so that an
    // extra row proves there is more.  None stays unbounded
    pub fn probe_limit(limit: Option<u64>) -> Option<u64> {
        limit.map(|l| l.saturating_add(1))
    }

    // build the page from the rows fetched with probe_limit, trimming the probe row
    pub fn from_probe(mut rows: Vec<T>, total: Option<u64>, limit: Option<u64>) -> Self {
        let has_more = match limit {
            None => false,
            Some(l) => {
                let more = rows.len() as u64 > l;
                rows.truncate(usize::try_from(l).unwrap_or(usize::MAX));
                more
            }
        };

        Self {
            items: rows,
            total,
            has_more: Some(has_more),
        }
    }

    // the metadata for the response to req (whose limit has already been clamped by
    // the caller, if it clamps)
    pub fn meta(&self, req: &PageRequest) -> PageMeta {
        PageMeta {
            total: self.total,
            row_count: self.items.len() as u64,
            offset: req.offset,
            limit: req.limit,
            has_more: self.has_more,
            next_cursor: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MediaDetails {
    pub media_uuid: MediaUuid,
    pub media: Media,
    pub collections: Vec<CollectionUuid>,
    pub comments: Vec<CommentUuid>,
}

// structs needed to do media updates
#[derive(Debug)]
pub struct MediaByPath {
    pub media_uuid: MediaUuid,
    pub hash: String,
    pub mtime: u64,
}

#[derive(Debug)]
pub struct MediaByCHash {
    pub media_uuid: MediaUuid,
    pub path: String,
    pub mtime: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_asks_for_one_extra_row() {
        assert_eq!(SearchPage::<u8>::probe_limit(None), None);
        assert_eq!(SearchPage::<u8>::probe_limit(Some(60)), Some(61));
        assert_eq!(
            SearchPage::<u8>::probe_limit(Some(u64::MAX)),
            Some(u64::MAX)
        );
    }

    #[test]
    fn from_probe_trims_the_probe_row() {
        let p = SearchPage::from_probe(vec![1, 2, 3], Some(9), Some(2));
        assert_eq!(p.items, vec![1, 2]);
        assert_eq!(p.has_more, Some(true));
        assert_eq!(p.total, Some(9));

        // a page that exactly fits the limit is the last one
        let p = SearchPage::from_probe(vec![1, 2], None, Some(2));
        assert_eq!(p.items, vec![1, 2]);
        assert_eq!(p.has_more, Some(false));

        let p = SearchPage::from_probe(vec![1], None, Some(2));
        assert_eq!(p.has_more, Some(false));

        let p = SearchPage::from_probe(vec![1, 2, 3], None, None);
        assert_eq!(p.items.len(), 3);
        assert_eq!(p.has_more, Some(false));
    }

    #[test]
    fn count_only_page_has_no_rows_and_unknown_has_more() {
        let p = SearchPage::<u8>::count_only(Some(4));
        assert!(p.items.is_empty());
        assert_eq!(p.has_more, None);
        assert_eq!(p.total, Some(4));
    }

    #[test]
    fn meta_echoes_the_request() {
        let p = SearchPage::from_probe(vec![1, 2, 3], Some(10), Some(2));
        let req = PageRequest {
            limit: Some(2),
            offset: 4,
            with_total: true,
            ..Default::default()
        };
        assert_eq!(
            p.meta(&req),
            PageMeta {
                total: Some(10),
                row_count: 2,
                offset: 4,
                limit: Some(2),
                has_more: Some(true),
                next_cursor: None,
            }
        );
    }
}
