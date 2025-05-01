use std::{
    collections::HashSet,
    fmt::{Debug, Display},
};

use regex::escape;
use serde::{Deserialize, Serialize};

use crate::{
    collection::{CollectionUuid, SearchMediaInCollectionReq},
    comment::CommentUuid,
    endpoint,
    library::SearchMediaInLibraryReq,
    media::{Media, MediaUuid, SearchMediaReq},
    sort::SortMethod,
};

// this struct is a first attempt at making a more generalized search mechanism that is
// still agnostic to the particular database backend
//
// as of this writing, we only support mariadb -- and its fulltext search is not very
// useful to us since it doesn't match partial words.  it is very likely that we will
// need to have more fine-grained control over both the queries and the structure, so
// all of this should be considered work-in-progress
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
    // mariadb formatting for mysql_async queries
    //
    // returns (sql, filter) where 'sql' is a fragment of an sql query
    // and filter is the named parameter
    pub fn format_mariadb(&self, cols: &str) -> (String, String) {
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

// batch searching
//
// sending individual GetMedia requests for each of the referenced media uuids returned by
// the search requests causes the reverse proxy (apache) to fall over, as there is no
// throttling or other flow control from the frontend
//
// unfortunately, the current implementation isn't particularly performant... and this will
// generally be one of the most important functions in the system.  thus, we will need to
// think very carefully about the architecture and optimizations involved.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchRequest {
    Media(SearchMediaReq),
    Collection(SearchMediaInCollectionReq),
    Library(SearchMediaInLibraryReq),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub media_uuid: MediaUuid,
    pub media: Media,
    pub collections: Vec<CollectionUuid>,
    pub comments: Vec<CommentUuid>,
}

endpoint!(BatchSearchAndSort);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSearchAndSortReq {
    pub req: SearchRequest,
    pub sort: SortMethod,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSearchAndSortResp {
    pub media: Vec<SearchResponse>,
}
