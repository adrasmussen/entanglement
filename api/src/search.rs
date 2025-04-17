use std::collections::HashSet;

use regex::escape;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl SearchFilter {
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
                    .into_iter()
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
                    .into_iter()
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
                    .into_iter()
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
