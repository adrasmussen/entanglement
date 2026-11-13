use std::{
    collections::HashSet,
    fmt::{Debug, Display},
};

use itertools::Itertools;
use regex::escape;
use serde::{Deserialize, Serialize};

use crate::{
    collection::{CollectionUuid, SearchMediaInCollectionReq},
    comment::CommentUuid,
    http_endpoint,
    library::SearchMediaInLibraryReq,
    media::{Media, MediaUuid, SearchMediaReq},
    page::{PageMeta, PageRequest, PagedResp},
};

// paging limits
//
// PAGE_SIZE is what the webapp requests per infinite-scroll page (divisible by the
// common 2/3/4/5/6 column grid layouts), while MAX_PAGE is the server-side clamp for
// the endpoint that hydrates full media records (BatchSearchAndSort).  the uuid-only
// searches, SearchCollections included, are deliberately left unbounded since the
// pickers ask for everything at once
pub const PAGE_SIZE: u64 = 60;
pub const MAX_PAGE: u64 = 200;

// search options
//
// page.limit = None means "no limit"; page.with_total asks the backend to run an
// additional COUNT(*) over the filtered set (without ORDER/LIMIT), which the webapp only
// does for the first page of a search so that subsequent pages stay cheap top-N index
// walks.  the page fields are flattened, so the wire keys are unchanged
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SearchOptions {
    pub filter: SearchFilter,
    pub order: SortMethod,
    #[serde(flatten)]
    pub page: PageRequest,
}

// sql fragments for a postgres search, with every piece of user input bound as a
// numbered parameter -- callers append these to their base query (which must end in a
// WHERE clause) and push the parameters in the order: filter_param (if Some), limit, offset
#[derive(Clone, Debug)]
pub struct PgSearchSql {
    pub filter_sql: String,
    pub filter_param: Option<String>,
    pub order_sql: &'static str,
    pub page_sql: String,
    pub limit: Option<i64>,
    pub offset: i64,
}

// sql fragments for a mariadb search, using named parameters :filter, :limit, :offset
#[derive(Clone, Debug)]
pub struct MariaSearchSql {
    pub filter_sql: String,
    pub filter_param: String,
    pub order_sql: &'static str,
    pub page_sql: &'static str,
    pub limit: u64,
    pub offset: u64,
}

impl SearchOptions {
    // the media_uuid tiebreaker makes the ordering total, which offset paging depends on --
    // media.date is a varchar with many duplicates, and without it rows sharing a date can
    // repeat or vanish between pages.  these match the ordered indexes in db/schema.sql
    fn order_sql(&self) -> &'static str {
        match self.order {
            SortMethod::DateAsc => " ORDER BY media.date ASC, media.media_uuid ASC",
            SortMethod::DateDesc => " ORDER BY media.date DESC, media.media_uuid DESC",
            SortMethod::Path => " ORDER BY media.path ASC, media.media_uuid ASC",
        }
    }

    // first_param is the $N index of the first placeholder this function may use, i.e.
    // one more than the number of parameters already used by the base query
    pub fn postgres_sql(&self, ts_col: &str, first_param: usize) -> PgSearchSql {
        let (filter_sql, filter_param) = self.filter.format_postgres(ts_col, first_param);

        let next = first_param + usize::from(filter_param.is_some());

        PgSearchSql {
            filter_sql,
            filter_param,
            order_sql: self.order_sql(),
            // LIMIT NULL is equivalent to no limit in postgres
            page_sql: format!(" LIMIT ${} OFFSET ${}", next, next + 1),
            limit: self.page.limit.map(clamp_i64),
            offset: clamp_i64(self.page.offset),
        }
    }

    pub fn mariadb_sql(&self, cols: &str) -> MariaSearchSql {
        let (filter_sql, filter_param) = self.filter.format_mariadb(cols);

        MariaSearchSql {
            filter_sql,
            filter_param,
            order_sql: self.order_sql(),
            page_sql: " LIMIT :limit OFFSET :offset",
            // mariadb has no LIMIT NULL/ALL; the documented idiom is the max u64
            limit: self.page.limit.unwrap_or(u64::MAX),
            offset: self.page.offset,
        }
    }
}

fn clamp_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

// escape LIKE wildcards so that user input is matched literally; both postgres and
// mariadb use backslash as the default LIKE escape character
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    for c in input.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// quote a single term as a tsquery lexeme so that operator characters (& | ! ( ) : *)
// inside user input are treated as text rather than tsquery syntax; embedded quotes and
// backslashes must be doubled per the tsvector/tsquery input rules
fn quote_tsquery_lexeme(term: &str) -> String {
    let escaped = term.replace('\\', "\\\\").replace('\'', "''");
    format!("'{escaped}'")
}

// this struct is a first attempt at making a more generalized search mechanism that is
// still agnostic to the particular database backend
//
// note that several places rely on an empty filter (of any sort) matching everything
//
// TODO -- to use the Substring filters more optimally, we need a better splitting
// algorithm than whitespace so as to keep quoted phrases together
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchFilter {
    SubstringAny { filter: HashSet<String> },
    SubstringAll { filter: HashSet<String> },
    Fulltext { filter: String },
    Keyword { filter: HashSet<String> },
}

impl Default for SearchFilter {
    fn default() -> Self {
        Self::SubstringAny {
            filter: HashSet::new(),
        }
    }
}

impl Display for SearchFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SubstringAny { filter } => {
                write!(f, "SubstringAny{filter:?}")
            }
            Self::SubstringAll { filter } => {
                write!(f, "SubstringAll{filter:?}")
            }
            Self::Fulltext { filter } => {
                write!(f, "FullText{{{filter}}}")
            }
            Self::Keyword { filter } => {
                write!(f, "Keyword{filter:?}")
            }
        }
    }
}

impl SearchFilter {
    // postgres formatting for queries
    //
    // https://www.postgresql.org/docs/current/textsearch-controls.html
    //
    // returns (sql, param) where sql references the param as $idx; user input is never
    // interpolated into the sql text itself
    fn format_postgres(&self, ts_col: &str, idx: usize) -> (String, Option<String>) {
        let tsquery = |filter: &HashSet<String>, op: &str| -> Option<String> {
            let terms = filter
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .sorted()
                .map(quote_tsquery_lexeme)
                .join(op);

            (!terms.is_empty()).then_some(terms)
        };

        match self {
            // use the fulltext search as keywords
            Self::SubstringAny { filter } | Self::Keyword { filter } => {
                match tsquery(filter, " | ") {
                    Some(q) => (
                        format!(" AND {ts_col} @@ to_tsquery('english', ${idx})"),
                        Some(q),
                    ),
                    None => (String::new(), None),
                }
            }

            Self::SubstringAll { filter } => match tsquery(filter, " & ") {
                Some(q) => (
                    format!(" AND {ts_col} @@ to_tsquery('english', ${idx})"),
                    Some(q),
                ),
                None => (String::new(), None),
            },

            // websearch_to_tsquery never raises on malformed input
            Self::Fulltext { filter } => {
                if filter.trim().is_empty() {
                    return (String::new(), None);
                }

                (
                    format!(" AND {ts_col} @@ websearch_to_tsquery('english', ${idx})"),
                    Some(filter.clone()),
                )
            }
        }
    }

    // mariadb formatting for mysql_async queries
    //
    // returns (sql, filter) where 'sql' is a fragment of an sql query
    // and filter is the named parameter
    fn format_mariadb(&self, cols: &str) -> (String, String) {
        match self {
            // match any of the strings using the normal regex logical OR |
            // (?i) enables case-insensitive matching
            Self::SubstringAny { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }

                // note that, while this is superficially the same as fold_set(), the pipe here is a
                // regex operator and thus specific to the query
                let regex = filter
                    .iter()
                    .map(|s| escape(s))
                    .fold(String::from("(?i)"), |a, b| a + &b + "|")
                    .trim_matches('|')
                    .to_string();

                (
                    format!(" AND CONCAT_WS(\"|\", {cols}) RLIKE :filter"),
                    regex,
                )
            }

            // match all of the strings using an expensive lookahead assertion (?=) and word boundary \b
            Self::SubstringAll { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }

                let regex = filter
                    .iter()
                    .map(|s| escape(s))
                    .fold(String::from("(?i)"), |a, b| {
                        a + &format!("(?=.*?\\b{b}\\b)")
                    });

                (
                    format!(" AND CONCAT_WS(\"|\", {cols}) RLIKE :filter"),
                    regex,
                )
            }

            // use mariadb's fulltext index/search mechanism with several sorts of operators built-in,
            // so we don't need to parse anything
            Self::Fulltext { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }

                (
                    format!(" AND MATCH({cols}) AGAINST(:filter IN BOOLEAN MODE)"),
                    filter.clone(),
                )
            }

            // use the fulltext search as keywords, which expects a comma-separated list
            Self::Keyword { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }
                let keywords = filter
                    .iter()
                    .fold(String::from(""), |a, b| a + b + ",")
                    .trim_matches(',')
                    .to_string();
                (
                    format!(" AND MATCH({cols}) AGAINST(:filter IN NATURAL LANGUAGE MODE)"),
                    keywords,
                )
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum SortMethod {
    DateAsc,
    #[default]
    DateDesc,
    Path,
}

// batch searching
//
// sending individual GetMedia requests for each of the referenced media uuids returned by
// the search requests causes the reverse proxy (apache) to fall over, as there is no
// throttling or other flow control from the frontend
//
// the server pages the uuid search in sql (limit/offset from the inner request's opts,
// clamped to MAX_PAGE) and hydrates only that page with a single batched media lookup
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchRequest {
    Media(SearchMediaReq),
    Collection(SearchMediaInCollectionReq),
    Library(SearchMediaInLibraryReq),
}

impl SearchRequest {
    pub fn opts(&self) -> &SearchOptions {
        match self {
            Self::Media(r) => &r.opts,
            Self::Collection(r) => &r.opts,
            Self::Library(r) => &r.opts,
        }
    }

    pub fn opts_mut(&mut self) -> &mut SearchOptions {
        match self {
            Self::Media(r) => &mut r.opts,
            Self::Collection(r) => &mut r.opts,
            Self::Library(r) => &mut r.opts,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub media_uuid: MediaUuid,
    pub media: Media,
    pub collections: Vec<CollectionUuid>,
    pub comments: Vec<CommentUuid>,
}

http_endpoint!(BatchSearchAndSort);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSearchAndSortReq {
    pub req: SearchRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSearchAndSortResp {
    pub media: Vec<SearchResponse>,
    // only populated when the request set opts.with_total
    #[serde(default)]
    pub total: Option<u64>,
    // how many uuids the search page held before hydration.  media deleted in between is
    // skipped, so `media` can come up short of the limit without this being the last page;
    // None from older servers, in which case media.len() is the best guess
    #[serde(default)]
    pub page_len: Option<u64>,
    // paging metadata; meta.row_count is the same pre-hydration count as page_len
    #[serde(default)]
    pub meta: PageMeta,
}

impl PagedResp for BatchSearchAndSortResp {
    type Item = SearchResponse;

    fn into_parts(self) -> (Vec<SearchResponse>, PageMeta) {
        (self.media, self.meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[&str]) -> HashSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn opts(filter: SearchFilter) -> SearchOptions {
        SearchOptions {
            filter,
            ..Default::default()
        }
    }

    #[test]
    fn empty_filter_has_no_param() {
        let q = opts(SearchFilter::default()).postgres_sql("ts", 2);
        assert_eq!(q.filter_sql, "");
        assert_eq!(q.filter_param, None);
        assert_eq!(q.page_sql, " LIMIT $2 OFFSET $3");
        assert_eq!(q.limit, None);
        assert_eq!(q.offset, 0);

        let q = opts(SearchFilter::SubstringAny {
            filter: set(&["", "  "]),
        })
        .postgres_sql("ts", 2);
        assert_eq!(q.filter_param, None);

        let q = opts(SearchFilter::Fulltext {
            filter: "   ".into(),
        })
        .postgres_sql("ts", 2);
        assert_eq!(q.filter_param, None);
    }

    #[test]
    fn placeholders_are_numbered_after_filter() {
        let q = SearchOptions {
            filter: SearchFilter::SubstringAll {
                filter: set(&["cat"]),
            },
            page: PageRequest {
                limit: Some(60),
                offset: 120,
                ..Default::default()
            },
            ..Default::default()
        }
        .postgres_sql("media.ts_vec", 3);

        assert_eq!(
            q.filter_sql,
            " AND media.ts_vec @@ to_tsquery('english', $3)"
        );
        assert_eq!(q.page_sql, " LIMIT $4 OFFSET $5");
        assert_eq!(q.limit, Some(60));
        assert_eq!(q.offset, 120);
    }

    #[test]
    fn injection_attempts_never_reach_sql_text() {
        let evil = "'); DROP TABLE media; --";
        for filter in [
            SearchFilter::SubstringAny {
                filter: set(&[evil]),
            },
            SearchFilter::SubstringAll {
                filter: set(&[evil]),
            },
            SearchFilter::Keyword {
                filter: set(&[evil]),
            },
            SearchFilter::Fulltext {
                filter: evil.into(),
            },
        ] {
            let q = opts(filter).postgres_sql("ts", 2);
            assert!(!q.filter_sql.contains("DROP"), "{}", q.filter_sql);
            assert!(!q.filter_sql.contains(evil));
            assert!(q.filter_param.unwrap().contains("DROP"));

            let m = opts(SearchFilter::Fulltext {
                filter: evil.into(),
            })
            .mariadb_sql("cols");
            assert!(!m.filter_sql.contains("DROP"));
        }
    }

    #[test]
    fn tsquery_terms_are_quoted_lexemes() {
        let q = opts(SearchFilter::SubstringAny {
            filter: set(&["it's", "a&!(b", "back\\slash"]),
        })
        .postgres_sql("ts", 2);

        // terms are sorted for deterministic output
        assert_eq!(
            q.filter_param.as_deref(),
            Some("'a&!(b' | 'back\\\\slash' | 'it''s'")
        );

        let q = opts(SearchFilter::SubstringAll {
            filter: set(&["x", "y"]),
        })
        .postgres_sql("ts", 2);
        assert_eq!(q.filter_param.as_deref(), Some("'x' & 'y'"));
    }

    #[test]
    fn order_has_uuid_tiebreaker() {
        for order in [SortMethod::DateAsc, SortMethod::DateDesc, SortMethod::Path] {
            let q = SearchOptions {
                order,
                ..Default::default()
            }
            .postgres_sql("ts", 1);
            assert!(
                q.order_sql.ends_with("media.media_uuid ASC")
                    || q.order_sql.ends_with("media.media_uuid DESC")
            );
        }
    }

    #[test]
    fn huge_limits_are_clamped() {
        let q = SearchOptions {
            page: PageRequest {
                limit: Some(u64::MAX),
                offset: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        }
        .postgres_sql("ts", 1);
        assert_eq!(q.limit, Some(i64::MAX));
        assert_eq!(q.offset, i64::MAX);

        let m = SearchOptions::default().mariadb_sql("cols");
        assert_eq!(m.limit, u64::MAX);
    }

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(escape_like("100%_off\\"), "100\\%\\_off\\\\");
        assert_eq!(escape_like("plain"), "plain");
    }

    #[test]
    fn page_len_defaults_to_none_when_missing() {
        let r: BatchSearchAndSortResp = serde_json::from_str(r#"{"media":[],"total":5}"#).unwrap();
        assert_eq!(r.total, Some(5));
        assert_eq!(r.page_len, None);
    }

    #[test]
    fn with_total_defaults_false_when_missing() {
        let o: SearchOptions =
            serde_json::from_str(r#"{"filter":{"SubstringAny":{"filter":[]}},"order":"DateDesc","limit":null,"offset":0}"#)
                .unwrap();
        assert!(!o.page.with_total);
    }

    // the flattened PageRequest must not change the wire format of the requests
    #[test]
    fn options_keep_flat_page_keys() {
        let o: SearchOptions = serde_json::from_str(
            r#"{"filter":{"SubstringAny":{"filter":[]}},"order":"DateDesc","limit":60,"offset":120,"with_total":true}"#,
        )
        .unwrap();
        assert_eq!(o.page.limit, Some(60));
        assert_eq!(o.page.offset, 120);
        assert!(o.page.with_total);

        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["limit"], 60);
        assert_eq!(v["offset"], 120);
        assert_eq!(v["with_total"], true);
        assert!(v.get("page").is_none());
        assert!(v.get("cursor").is_none());

        let back: SearchOptions = serde_json::from_value(v).unwrap();
        assert_eq!(back.page, o.page);
    }

    #[test]
    fn other_requests_keep_flat_page_keys() {
        use crate::{collection::SearchCollectionsReq, library::SearchLibrariesReq};

        let r: SearchCollectionsReq =
            serde_json::from_str(r#"{"filter":"a","limit":5,"offset":10,"with_total":true}"#)
                .unwrap();
        assert_eq!(r.page.limit, Some(5));
        assert_eq!(r.page.offset, 10);
        assert!(r.page.with_total);

        // every page field is optional
        let r: SearchCollectionsReq = serde_json::from_str(r#"{"filter":"a"}"#).unwrap();
        assert_eq!(r.page, PageRequest::default());
        let r: SearchLibrariesReq = serde_json::from_str(r#"{"filter":""}"#).unwrap();
        assert_eq!(r.page, PageRequest::default());

        let v = serde_json::to_value(SearchLibrariesReq {
            filter: "x".into(),
            page: PageRequest::first(3, true),
        })
        .unwrap();
        assert_eq!(v["limit"], 3);
        assert_eq!(v["with_total"], true);
    }

    #[test]
    fn responses_without_meta_get_a_default() {
        use crate::{
            collection::{SearchCollectionsResp, SearchMediaInCollectionResp},
            library::{SearchLibrariesResp, SearchMediaInLibraryResp},
            media::SearchMediaResp,
        };

        let r: BatchSearchAndSortResp =
            serde_json::from_str(r#"{"media":[],"total":5,"page_len":0}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());

        let r: SearchMediaResp = serde_json::from_str(r#"{"media":[],"total":1}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());
        let r: SearchMediaInCollectionResp = serde_json::from_str(r#"{"media":[]}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());
        let r: SearchMediaInLibraryResp = serde_json::from_str(r#"{"media":[]}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());
        let r: SearchCollectionsResp = serde_json::from_str(r#"{"collections":[]}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());
        let r: SearchLibrariesResp = serde_json::from_str(r#"{"libraries":[]}"#).unwrap();
        assert_eq!(r.meta, PageMeta::default());
    }

    #[test]
    fn meta_round_trips_and_into_parts() {
        use crate::{media::SearchMediaResp, page::PageCursor};

        let meta = PageMeta {
            total: Some(10),
            row_count: 2,
            offset: 4,
            limit: Some(2),
            has_more: Some(true),
            next_cursor: Some(PageCursor("c".into())),
        };
        let resp = SearchMediaResp {
            media: Vec::new(),
            total: Some(10),
            meta: meta.clone(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: SearchMediaResp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
        assert_eq!(back.into_parts().1, meta);
    }
}
