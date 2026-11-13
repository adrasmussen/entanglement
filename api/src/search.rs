use std::{
    collections::HashSet,
    fmt::{Debug, Display},
};

use itertools::Itertools;
use regex::escape;
use serde::{Deserialize, Serialize};

use crate::{
    FOLDING_SEPARATOR,
    collection::{CollectionUuid, SearchMediaInCollectionReq},
    comment::CommentUuid,
    http_endpoint,
    library::{LibraryUuid, SearchMediaInLibraryReq},
    media::{Media, MediaUuid, SearchMediaReq},
    page::{PageMeta, PageRequest, PagedResp},
    query::{Clause, FieldSpec, FieldType, Op, SearchClass, SearchQuery, field_spec},
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
// WHERE clause) and push the parameters in the order: filter_params, limit, offset
#[derive(Clone, Debug)]
pub struct PgSearchSql {
    pub filter_sql: String,
    // every value is text (dates, tags, patterns and enum names too; the sql casts)
    pub filter_params: Vec<String>,
    pub order_sql: &'static str,
    pub page_sql: String,
    pub limit: Option<i64>,
    pub offset: i64,
}

// sql fragments for a mariadb search, using named parameters :limit and :offset plus the
// (name, value) filter_params, which are :filter for the older filters and :f0, :f1, ...
// for structured queries; an empty filter has none
#[derive(Clone, Debug)]
pub struct MariaSearchSql {
    pub filter_sql: String,
    pub filter_params: Vec<(String, String)>,
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
        let (filter_sql, filter_params) = self.filter.format_postgres(ts_col, first_param);

        let next = first_param + filter_params.len();

        PgSearchSql {
            filter_sql,
            filter_params,
            order_sql: self.order_sql(),
            // LIMIT NULL is equivalent to no limit in postgres
            page_sql: format!(" LIMIT ${} OFFSET ${}", next, next + 1),
            limit: self.page.limit.map(clamp_i64),
            offset: clamp_i64(self.page.offset),
        }
    }

    pub fn mariadb_sql(&self, cols: &str) -> MariaSearchSql {
        let (filter_sql, filter_params) = self.filter.format_mariadb(cols);

        MariaSearchSql {
            filter_sql,
            filter_params,
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
    // the structured query (typed fields plus free text); media only for now
    Query(SearchQuery),
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
            Self::Query(q) => {
                write!(f, "Query{{{}}}", q.to_text())
            }
        }
    }
}

impl SearchFilter {
    // postgres formatting for queries
    //
    // https://www.postgresql.org/docs/current/textsearch-controls.html
    //
    // returns (sql, params) where sql references the params as $idx, $idx+1, ...; user input
    // is never interpolated into the sql text itself
    fn format_postgres(&self, ts_col: &str, idx: usize) -> (String, Vec<String>) {
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
                        vec![q],
                    ),
                    None => (String::new(), Vec::new()),
                }
            }

            Self::SubstringAll { filter } => match tsquery(filter, " & ") {
                Some(q) => (
                    format!(" AND {ts_col} @@ to_tsquery('english', ${idx})"),
                    vec![q],
                ),
                None => (String::new(), Vec::new()),
            },

            // websearch_to_tsquery never raises on malformed input
            Self::Fulltext { filter } => {
                if filter.trim().is_empty() {
                    return (String::new(), Vec::new());
                }

                (
                    format!(" AND {ts_col} @@ websearch_to_tsquery('english', ${idx})"),
                    vec![filter.clone()],
                )
            }

            Self::Query(q) => q.postgres_filter::<Media>(ts_col, idx),
        }
    }

    // mariadb formatting for mysql_async queries
    //
    // returns (sql, params) where 'sql' is a fragment of an sql query
    // and params are its named parameters
    fn format_mariadb(&self, cols: &str) -> (String, Vec<(String, String)>) {
        // the older filters all bind a single :filter
        let single = |sql: String, value: String| (sql, vec![("filter".to_string(), value)]);

        match self {
            // match any of the strings using the normal regex logical OR |
            // (?i) enables case-insensitive matching
            Self::SubstringAny { filter } => {
                if filter.is_empty() {
                    return (String::new(), Vec::new());
                }

                // note that, while this is superficially the same as fold_set(), the pipe here is a
                // regex operator and thus specific to the query
                let regex = filter
                    .iter()
                    .map(|s| escape(s))
                    .fold(String::from("(?i)"), |a, b| a + &b + "|")
                    .trim_matches('|')
                    .to_string();

                single(
                    format!(" AND CONCAT_WS(\"|\", {cols}) RLIKE :filter"),
                    regex,
                )
            }

            // match all of the strings using an expensive lookahead assertion (?=) and word boundary \b
            Self::SubstringAll { filter } => {
                if filter.is_empty() {
                    return (String::new(), Vec::new());
                }

                let regex = filter
                    .iter()
                    .map(|s| escape(s))
                    .fold(String::from("(?i)"), |a, b| {
                        a + &format!("(?=.*?\\b{b}\\b)")
                    });

                single(
                    format!(" AND CONCAT_WS(\"|\", {cols}) RLIKE :filter"),
                    regex,
                )
            }

            // use mariadb's fulltext index/search mechanism with several sorts of operators built-in,
            // so we don't need to parse anything
            Self::Fulltext { filter } => {
                if filter.is_empty() {
                    return (String::new(), Vec::new());
                }

                single(
                    format!(" AND MATCH({cols}) AGAINST(:filter IN BOOLEAN MODE)"),
                    filter.clone(),
                )
            }

            // use the fulltext search as keywords, which expects a comma-separated list
            Self::Keyword { filter } => {
                if filter.is_empty() {
                    return (String::new(), Vec::new());
                }
                let keywords = filter
                    .iter()
                    .fold(String::from(""), |a, b| a + b + ",")
                    .trim_matches(',')
                    .to_string();
                single(
                    format!(" AND MATCH({cols}) AGAINST(:filter IN NATURAL LANGUAGE MODE)"),
                    keywords,
                )
            }

            Self::Query(q) => q.mariadb_filter::<Media>(cols),
        }
    }

    // the checks the server applies to a request before it reaches a backend: only
    // structured queries can be invalid, and every search filter today is over media
    pub fn validate_media(&self) -> Result<(), String> {
        match self {
            Self::Query(q) => q.validate::<Media>(),
            _ => Ok(()),
        }
    }
}

// the generic resolver: turns a SearchQuery into sql fragments by walking its clauses and
// choosing a fixed template from each field's declared type and operator.  column names
// come only from the class's static FIELDS table (and C::TABLE), operators and templates are
// literals in this file, and every user value is a bound parameter -- never sql text.
//
// the fragments each start with " AND ", for appending to a base query that ends in a WHERE
// clause.  a clause the declarations don't cover (which validate() rejects before a request
// gets here) resolves to FALSE, so a bad query can only ever match nothing
#[derive(Clone, Copy, PartialEq)]
enum Dialect {
    Postgres,
    MariaDb,
}

struct Resolver {
    dialect: Dialect,
    // the $N number of the first placeholder (postgres only)
    first: usize,
    params: Vec<String>,
}

impl Resolver {
    // bind a value, returning the placeholder to put in the sql: $N or :fN
    fn bind(&mut self, value: String) -> String {
        let n = self.params.len();
        self.params.push(value);
        match self.dialect {
            Dialect::Postgres => format!("${}", self.first + n),
            Dialect::MariaDb => format!(":f{n}"),
        }
    }

    // the "a OR b OR c" free-text condition, matching the older SubstringAny filter
    fn text_sql(&mut self, terms: &[String], cols: &str) -> Option<String> {
        let terms: Vec<&str> = terms
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .sorted()
            .dedup()
            .collect();

        if terms.is_empty() {
            return None;
        }

        match self.dialect {
            Dialect::Postgres => {
                let q = terms.iter().map(|t| quote_tsquery_lexeme(t)).join(" | ");
                let p = self.bind(q);
                Some(format!("{cols} @@ to_tsquery('english', {p})"))
            }
            Dialect::MariaDb => {
                let regex = format!("(?i){}", terms.iter().map(|t| escape(t)).join("|"));
                let p = self.bind(regex);
                Some(format!("CONCAT_WS(\"|\", {cols}) RLIKE {p}"))
            }
        }
    }

    // one clause as a condition, or None when no template covers it
    fn clause_sql<C: SearchClass>(&mut self, clause: &Clause) -> Option<String> {
        let spec: &FieldSpec = field_spec::<C>(&clause.field)?;

        if !spec.ops.contains(&clause.op) {
            return None;
        }

        let col = format!("{}.{}", C::TABLE, spec.column);
        let v = clause.value.clone();
        let pg = self.dialect == Dialect::Postgres;

        let sql = match (spec.ty, clause.op) {
            (FieldType::Text, Op::Contains) => {
                let p = self.bind(format!("%{}%", escape_like(&v)));
                let like = if pg { "ILIKE" } else { "LIKE" };
                format!("{col} {like} {p}")
            }

            // tags are an hstore on postgres and a folded string on mariadb; as with
            // GID_MATCH, delimiting both sides with the separator (which no tag contains)
            // makes the match exact
            (FieldType::Tag, Op::Has) => {
                let p = self.bind(v);
                if pg {
                    format!("{col} ? {p}")
                } else {
                    let sep = FOLDING_SEPARATOR;
                    format!(
                        "LOCATE(CONCAT('{sep}', {p}, '{sep}'), CONCAT('{sep}', {col}, '{sep}')) > 0"
                    )
                }
            }

            // the postgres enum has no text comparison, hence the cast; the only enum
            // field is media.media_type
            (FieldType::Enum(_), Op::Eq) => {
                let p = self.bind(v);
                if pg {
                    format!("{col} = {p}::text::media_type")
                } else {
                    format!("{col} = {p}")
                }
            }

            // dates are strings, and a partial date is a prefix: date:2020-06 is all of
            // June, so > and <= treat the value as a prefix rather than comparing past it
            (FieldType::Date, Op::Eq) => {
                let p = self.bind(format!("{}%", escape_like(&v)));
                format!("{col} LIKE {p}")
            }
            (FieldType::Date, Op::Ge) => {
                let p = self.bind(v);
                format!("{col} >= {p}")
            }
            (FieldType::Date, Op::Lt) => {
                let p = self.bind(v);
                format!("{col} < {p}")
            }
            (FieldType::Date, Op::Gt) => {
                let p = self.bind(v.clone());
                let q = self.bind(format!("{}%", escape_like(&v)));
                format!("({col} > {p} AND {col} NOT LIKE {q})")
            }
            (FieldType::Date, Op::Le) => {
                let p = self.bind(v.clone());
                let q = self.bind(format!("{}%", escape_like(&v)));
                format!("({col} <= {p} OR {col} LIKE {q})")
            }
            (FieldType::Date, Op::Range) => {
                let (a, b) = v.split_once("..")?;
                let pa = self.bind(a.to_string());
                let pb = self.bind(b.to_string());
                let pc = self.bind(format!("{}%", escape_like(b)));
                format!("({col} >= {pa} AND ({col} <= {pb} OR {col} LIKE {pc}))")
            }

            _ => return None,
        };

        Some(sql)
    }

    fn resolve<C: SearchClass>(&mut self, query: &SearchQuery, free_text_cols: &str) -> String {
        let mut sql = String::new();

        if let Some(t) = self.text_sql(&query.text, free_text_cols) {
            sql.push_str(" AND ");
            sql.push_str(&t);
        }

        for clause in &query.clauses {
            sql.push_str(" AND ");
            match self.clause_sql::<C>(clause) {
                Some(c) => sql.push_str(&c),
                None => sql.push_str("FALSE"),
            }
        }

        sql
    }
}

impl SearchQuery {
    // postgres fragment and parameters for this query; free text goes against ts_col (the
    // tsvector column) and the first placeholder is $first_param
    pub fn postgres_filter<C: SearchClass>(
        &self,
        ts_col: &str,
        first_param: usize,
    ) -> (String, Vec<String>) {
        let mut r = Resolver {
            dialect: Dialect::Postgres,
            first: first_param,
            params: Vec::new(),
        };
        let sql = r.resolve::<C>(self, ts_col);
        (sql, r.params)
    }

    // mariadb fragment and named parameters (:f0, :f1, ...); free text goes against the
    // comma-separated column list cols
    pub fn mariadb_filter<C: SearchClass>(&self, cols: &str) -> (String, Vec<(String, String)>) {
        let mut r = Resolver {
            dialect: Dialect::MariaDb,
            first: 0,
            params: Vec::new(),
        };
        let sql = r.resolve::<C>(self, cols);
        let params = r
            .params
            .into_iter()
            .enumerate()
            .map(|(n, v)| (format!("f{n}"), v))
            .collect();
        (sql, params)
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

http_endpoint!(GetCounts);

// exact counts for several scopes in one round trip.  each scope counts what the
// matching search endpoint would return for an empty filter, so the numbers always agree
// with the lists and the same access rules apply (a collection or library the user can't
// see counts as 0)
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum CountScope {
    // all media visible to the user
    Media,
    Collections,
    Libraries,
    MediaInCollection {
        collection_uuid: CollectionUuid,
    },
    MediaInLibrary {
        library_uuid: LibraryUuid,
        hidden: Option<bool>,
    },
}

// the most scopes a single request may ask for
pub const MAX_COUNT_SCOPES: usize = 16;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GetCountsReq {
    pub scopes: Vec<CountScope>,
}

// one entry per requested scope, in request order
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct GetCountsResp {
    pub counts: Vec<ScopeCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScopeCount {
    pub scope: CountScope,
    pub count: u64,
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
    fn count_scopes_wire_format() {
        let json = r#"{"scopes":["Media","Libraries",
            {"MediaInLibrary":{"library_uuid":"00000000-0000-0000-0000-000000000001","hidden":false}}]}"#;
        let req: GetCountsReq = serde_json::from_str(json).unwrap();
        assert_eq!(req.scopes.len(), 3);
        assert_eq!(req.scopes[0], CountScope::Media);
        assert!(matches!(
            req.scopes[2],
            CountScope::MediaInLibrary {
                hidden: Some(false),
                ..
            }
        ));
        let back: GetCountsReq =
            serde_json::from_value(serde_json::to_value(&req).unwrap()).unwrap();
        assert_eq!(back.scopes, req.scopes);
    }

    #[test]
    fn empty_filter_has_no_param() {
        let q = opts(SearchFilter::default()).postgres_sql("ts", 2);
        assert_eq!(q.filter_sql, "");
        assert!(q.filter_params.is_empty());
        assert_eq!(q.page_sql, " LIMIT $2 OFFSET $3");
        assert_eq!(q.limit, None);
        assert_eq!(q.offset, 0);

        let q = opts(SearchFilter::SubstringAny {
            filter: set(&["", "  "]),
        })
        .postgres_sql("ts", 2);
        assert!(q.filter_params.is_empty());

        let q = opts(SearchFilter::Fulltext {
            filter: "   ".into(),
        })
        .postgres_sql("ts", 2);
        assert!(q.filter_params.is_empty());
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
            assert!(q.filter_params[0].contains("DROP"));

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
        assert_eq!(q.filter_params, ["'a&!(b' | 'back\\\\slash' | 'it''s'"]);

        let q = opts(SearchFilter::SubstringAll {
            filter: set(&["x", "y"]),
        })
        .postgres_sql("ts", 2);
        assert_eq!(q.filter_params, ["'x' & 'y'"]);
    }

    fn q(text: &str) -> SearchQuery {
        let parsed = SearchQuery::parse::<Media>(text);
        assert!(parsed.issues.is_empty(), "{text}: {:?}", parsed.issues);
        parsed.query
    }

    fn pg(text: &str, first: usize) -> (String, Vec<String>) {
        q(text).postgres_filter::<Media>("media.ts_vec", first)
    }

    fn maria(text: &str) -> (String, Vec<(String, String)>) {
        q(text).mariadb_filter::<Media>("media.path, media.note")
    }

    fn named(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn query_free_text_and_tag_sql() {
        let (sql, params) = pg("beach tag:holiday", 2);
        assert_eq!(
            sql,
            " AND media.ts_vec @@ to_tsquery('english', $2) AND media.tags ? $3"
        );
        assert_eq!(params, ["'beach'", "holiday"]);

        let (sql, params) = maria("beach tag:holiday");
        assert_eq!(
            sql,
            " AND CONCAT_WS(\"|\", media.path, media.note) RLIKE :f0 \
             AND LOCATE(CONCAT('|', :f1, '|'), CONCAT('|', media.tags, '|')) > 0"
        );
        assert_eq!(params, named(&[("f0", "(?i)beach"), ("f1", "holiday")]));
    }

    #[test]
    fn query_description_and_date_range_sql() {
        let text = "description:50%_off date:2020-01..2020-06";

        let (sql, params) = pg(text, 1);
        assert_eq!(
            sql,
            " AND media.note ILIKE $1 \
             AND (media.date >= $2 AND (media.date <= $3 OR media.date LIKE $4))"
        );
        assert_eq!(params, ["%50\\%\\_off%", "2020-01", "2020-06", "2020-06%"]);

        let (sql, params) = maria(text);
        assert_eq!(
            sql,
            " AND media.note LIKE :f0 \
             AND (media.date >= :f1 AND (media.date <= :f2 OR media.date LIKE :f3))"
        );
        assert_eq!(
            params,
            named(&[
                ("f0", "%50\\%\\_off%"),
                ("f1", "2020-01"),
                ("f2", "2020-06"),
                ("f3", "2020-06%")
            ])
        );
    }

    #[test]
    fn query_type_and_date_comparisons_sql() {
        let text = "type:Image date:>2020 date:<=2021-03 date:>=2019 date:<2022 date:2020-06";

        let (sql, params) = pg(text, 4);
        assert_eq!(
            sql,
            " AND media.media_type = $4::text::media_type \
             AND (media.date > $5 AND media.date NOT LIKE $6) \
             AND (media.date <= $7 OR media.date LIKE $8) \
             AND media.date >= $9 \
             AND media.date < $10 \
             AND media.date LIKE $11"
        );
        assert_eq!(
            params,
            [
                "Image", "2020", "2020%", "2021-03", "2021-03%", "2019", "2022", "2020-06%"
            ]
        );

        let (sql, params) = maria(text);
        assert!(sql.starts_with(" AND media.media_type = :f0 AND (media.date > :f1"));
        assert!(sql.ends_with("AND media.date LIKE :f7"));
        assert_eq!(params.len(), 8);
        assert_eq!(params[7], ("f7".to_string(), "2020-06%".to_string()));
    }

    #[test]
    fn query_text_terms_are_or_and_deduped() {
        let (sql, params) = pg("b a b \"c d\"", 1);
        assert_eq!(sql, " AND media.ts_vec @@ to_tsquery('english', $1)");
        assert_eq!(params, ["'a' | 'b' | 'c d'"]);

        let (_, params) = maria("b a b \"c d\" x.y");
        assert_eq!(params, named(&[("f0", "(?i)a|b|c d|x\\.y")]));
    }

    #[test]
    fn empty_query_has_no_sql_or_params() {
        let opts = opts(SearchFilter::Query(SearchQuery::default()));
        let p = opts.postgres_sql("media.ts_vec", 2);
        assert_eq!(p.filter_sql, "");
        assert!(p.filter_params.is_empty());
        assert_eq!(p.page_sql, " LIMIT $2 OFFSET $3");

        let m = opts.mariadb_sql("cols");
        assert_eq!(m.filter_sql, "");
        assert!(m.filter_params.is_empty());
    }

    #[test]
    fn query_placeholders_number_after_all_filter_params() {
        let o = SearchOptions {
            filter: SearchFilter::Query(q("a tag:x date:2020")),
            ..Default::default()
        };
        let p = o.postgres_sql("media.ts_vec", 3);
        assert_eq!(p.filter_params.len(), 3);
        assert_eq!(p.page_sql, " LIMIT $6 OFFSET $7");

        let m = o.mariadb_sql("cols");
        let names: Vec<_> = m.filter_params.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["f0", "f1", "f2"]);
        for n in names {
            assert!(m.filter_sql.contains(&format!(":{n}")), "{n}");
        }
    }

    // whatever the user types, it is only ever in the parameters
    #[test]
    fn query_user_input_never_reaches_sql_text() {
        let evils = [
            "'; DROP TABLE media; --",
            "$1",
            "100%_\\",
            "\") OR 1=1 --",
            ":f0",
        ];
        for evil in evils {
            let query = SearchQuery {
                text: vec![evil.to_string()],
                clauses: [
                    ("tag", Op::Has),
                    ("description", Op::Contains),
                    ("date", Op::Eq),
                    ("date", Op::Gt),
                    ("date", Op::Le),
                    ("type", Op::Eq),
                ]
                .into_iter()
                .map(|(f, op)| Clause {
                    field: f.into(),
                    op,
                    value: evil.into(),
                })
                .chain([Clause {
                    field: "date".into(),
                    op: Op::Range,
                    value: format!("{evil}..{evil}"),
                }])
                .collect(),
            };

            let (sql, params) = query.postgres_filter::<Media>("media.ts_vec", 1);
            let (msql, mparams) = query.mariadb_filter::<Media>("cols");

            assert!(params.len() >= 10 && mparams.len() == params.len());
            for fragment in ["DROP", "OR 1=1", "100%", "\\"] {
                assert!(!sql.contains(fragment), "{fragment}: {sql}");
                assert!(!msql.contains(fragment), "{fragment}: {msql}");
            }
            // the only quote characters in the sql are the fixed templates' (the
            // mariadb tag match has four separator literals)
            assert_eq!(sql.matches('\'').count(), 2);
            assert_eq!(msql.matches('\'').count(), 8);
            // every $N is a bound parameter, and nothing past the last one exists
            assert!(sql.contains(&format!("${}", params.len())));
            assert!(!sql.contains(&format!("${}", params.len() + 1)));
        }
    }

    #[test]
    fn query_with_undeclared_fields_matches_nothing() {
        let query = SearchQuery {
            text: vec![],
            clauses: vec![
                Clause {
                    field: "tags; DROP TABLE media".into(),
                    op: Op::Has,
                    value: "x".into(),
                },
                // declared field, undeclared operator
                Clause {
                    field: "tag".into(),
                    op: Op::Gt,
                    value: "x".into(),
                },
                // a field of another class
                Clause {
                    field: "path".into(),
                    op: Op::Contains,
                    value: "x".into(),
                },
                // a range without both ends
                Clause {
                    field: "date".into(),
                    op: Op::Range,
                    value: "2020".into(),
                },
            ],
        };
        let (sql, params) = query.postgres_filter::<Media>("ts", 1);
        assert_eq!(sql, " AND FALSE AND FALSE AND FALSE AND FALSE");
        assert!(params.is_empty());
        let (sql, params) = query.mariadb_filter::<Media>("cols");
        assert_eq!(sql, " AND FALSE AND FALSE AND FALSE AND FALSE");
        assert!(params.is_empty());
    }

    #[test]
    fn query_filter_wire_format_and_validation() {
        let json = r#"{"Query":{"text":["a"],"clauses":[{"field":"tag","op":"Has","value":"x"}]}}"#;
        let f: SearchFilter = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&f).unwrap(), json);
        assert_eq!(f.validate_media(), Ok(()));
        assert_eq!(f.to_string(), "Query{a tag:x}");

        let bad: SearchFilter = serde_json::from_str(
            r#"{"Query":{"clauses":[{"field":"zzz","op":"Eq","value":"x"}]}}"#,
        )
        .unwrap();
        assert!(bad.validate_media().is_err());

        // the older filters are untouched
        let old: SearchFilter =
            serde_json::from_str(r#"{"SubstringAny":{"filter":["a"]}}"#).unwrap();
        assert_eq!(old.validate_media(), Ok(()));
        let p = opts(old.clone()).postgres_sql("ts", 1);
        assert_eq!(p.filter_sql, " AND ts @@ to_tsquery('english', $1)");
        assert_eq!(p.filter_params, ["'a'"]);
        let m = opts(old).mariadb_sql("cols");
        assert_eq!(m.filter_params, named(&[("filter", "(?i)a")]));
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
