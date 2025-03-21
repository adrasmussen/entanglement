use std::collections::HashSet;

use serde::{Deserialize, Serialize};

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

                let regex = filter
                    .into_iter()
                    .fold(String::new(), |a, b| a + b + "|")
                    .trim_matches('|').to_string();

                ("AND RLIKE (?i) :filter".to_string(), regex)
            }

            // match all of the strings using an expensive lookahead assertion (?=) and word boundary \b
            Self::SubstringAll { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }

                let regex = filter
                    .into_iter()
                    .fold(String::new(), |a, b| a + &format!("(?=.*?\\b{b}\\b)"));

                ("AND RLIKE (?i) :filter".to_string(), regex)
            }

            Self::Fulltext { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }

                (format!("MATCHES({cols}) AGAINST(:filter IN BOOLEAN MODE)"), filter.clone())
            }
            Self::Keyword { filter } => {
                if filter.is_empty() {
                    return (String::new(), String::new());
                }
                let keywords = filter
                    .into_iter()
                    .fold(String::from(""), |a, b| a + b + ",")
                    .trim_matches(',').to_string();
                (format!("MATCHES({cols}) AGAINST(:filter IN NATURAL LANGUAGE MODE)"), keywords)
            }
        }
    }
}
