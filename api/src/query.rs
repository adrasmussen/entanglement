use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{FOLDING_SEPARATOR, collection::Collection, library::Library, media::Media};

// structured search queries
//
// every searchable object class declares its fields statically (name, type, operators, help
// text), and everything else is driven from that table: the parser below turns typed text
// such as `tag:holiday date:2020-06..2020-08 beach` into a SearchQuery, the server validates
// incoming queries against it, the sql resolvers in search.rs pick their templates from it,
// and the webapp can build completions and tooltips from the same declarations.
//
// the query text grammar, per whitespace-separated token:
//
//   word                  free text (plain words combine with OR)
//   field:value           a clause (clauses combine with AND)
//   field:>v >=v <v <=v   ordered comparisons, where the field allows them
//   field:a..b            an inclusive range, where the field allows it
//   "two words"           quotes keep a value (or a free word) together; \" and \\ escape
//
// problems in typed text are data (ParseIssue, for tooltips), never errors: an unknown field
// keeps the token as free text, and a bad operator or value drops the clause

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchKind {
    Media,
    Collection,
    Library,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    // free-form text, matched as a case-insensitive substring
    Text,
    // a member of a set of tags
    Tag,
    // a (possibly partial) date string such as 2020 or 2020-06-15
    Date,
    // one of a fixed list of values
    Enum(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Op {
    Contains,
    Has,
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
    Range,
}

impl Op {
    // the prefix this operator takes in query text, if it has one
    fn prefix(self) -> &'static str {
        match self {
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Lt => "<",
            Op::Le => "<=",
            _ => "",
        }
    }
}

// the static declaration of one searchable field; the webapp reads these directly
#[derive(Clone, Copy, Debug)]
pub struct FieldSpec {
    // what the user types: "tag", "date", ...
    pub name: &'static str,
    pub label: &'static str,
    pub ty: FieldType,
    pub ops: &'static [Op],
    // tooltip text
    pub help: &'static str,
    // e.g. "date:2020-06..2020-08"
    pub example: &'static str,
    // searchable from omnisearch
    pub in_global: bool,
    // the logical column on SearchClass::TABLE; the startup schema check verifies it
    pub column: &'static str,
}

impl FieldSpec {
    // the operator a bare `field:value` means
    pub fn default_op(&self) -> Op {
        match self.ty {
            FieldType::Text => Op::Contains,
            FieldType::Tag => Op::Has,
            FieldType::Date | FieldType::Enum(_) => Op::Eq,
        }
    }
}

// how a class matches plain words
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeText {
    // against the backend's fulltext column(s), which the caller supplies
    Fulltext,
    // any word as a case-insensitive substring of this column of SearchClass::TABLE
    Column(&'static str),
}

pub trait SearchClass {
    const KIND: SearchKind;
    const TABLE: &'static str;
    const FIELDS: &'static [FieldSpec];
    // what plain words match
    const FREE_TEXT: FreeText;
    const FREE_TEXT_HELP: &'static str;
}

pub fn field_spec<C: SearchClass>(name: &str) -> Option<&'static FieldSpec> {
    C::FIELDS.iter().find(|f| f.name == name)
}

// every (table, column) the class's fields refer to, for the startup schema check
pub fn declared_columns<C: SearchClass>() -> Vec<(&'static str, &'static str)> {
    let mut cols: Vec<_> = C::FIELDS.iter().map(|f| (C::TABLE, f.column)).collect();
    cols.dedup();
    cols
}

// the declared columns of every searchable class
pub fn all_declared_columns() -> Vec<(&'static str, &'static str)> {
    let mut cols = declared_columns::<Media>();
    cols.extend(declared_columns::<Collection>());
    cols.extend(declared_columns::<Library>());
    cols
}

const MEDIA_TYPES: &[&str] = &["Image", "Video", "VideoSlice", "Audio"];

impl SearchClass for Media {
    const KIND: SearchKind = SearchKind::Media;
    const TABLE: &'static str = "media";
    const FIELDS: &'static [FieldSpec] = &[
        FieldSpec {
            name: "tag",
            label: "Tag",
            ty: FieldType::Tag,
            ops: &[Op::Has],
            help: "Media that has exactly this tag",
            example: "tag:holiday",
            in_global: true,
            column: "tags",
        },
        FieldSpec {
            name: "description",
            label: "Description",
            ty: FieldType::Text,
            ops: &[Op::Contains],
            help: "Media whose description contains this text (any case)",
            example: "description:sunset",
            in_global: true,
            column: "note",
        },
        FieldSpec {
            name: "date",
            label: "Date",
            ty: FieldType::Date,
            ops: &[Op::Eq, Op::Gt, Op::Ge, Op::Lt, Op::Le, Op::Range],
            help: "Media from a date or date prefix; use > >= < <= or a..b for an inclusive range",
            example: "date:2020-06..2020-08",
            in_global: false,
            column: "date",
        },
        FieldSpec {
            name: "type",
            label: "Type",
            ty: FieldType::Enum(MEDIA_TYPES),
            ops: &[Op::Eq],
            help: "Media of one type: Image, Video, VideoSlice or Audio",
            example: "type:Image",
            in_global: false,
            column: "media_type",
        },
    ];
    const FREE_TEXT: FreeText = FreeText::Fulltext;
    const FREE_TEXT_HELP: &'static str =
        "Words are matched against the date, description and tags; any one word may match";
}

impl SearchClass for Collection {
    const KIND: SearchKind = SearchKind::Collection;
    const TABLE: &'static str = "collections";
    const FIELDS: &'static [FieldSpec] = &[
        FieldSpec {
            name: "name",
            label: "Name",
            ty: FieldType::Text,
            ops: &[Op::Contains],
            help: "Collections whose name contains this text",
            example: "name:holiday",
            in_global: true,
            column: "name",
        },
        FieldSpec {
            name: "description",
            label: "Description",
            ty: FieldType::Text,
            ops: &[Op::Contains],
            help: "Collections whose description contains this text",
            example: "description:trip",
            in_global: true,
            column: "note",
        },
        FieldSpec {
            name: "tag",
            label: "Tag",
            ty: FieldType::Tag,
            ops: &[Op::Has],
            help: "Collections that have exactly this tag",
            example: "tag:family",
            in_global: true,
            column: "tags",
        },
    ];
    const FREE_TEXT: FreeText = FreeText::Column("name");
    const FREE_TEXT_HELP: &'static str =
        "Words are matched against the name; any one word may match";
}

impl SearchClass for Library {
    const KIND: SearchKind = SearchKind::Library;
    const TABLE: &'static str = "libraries";
    const FIELDS: &'static [FieldSpec] = &[FieldSpec {
        name: "path",
        label: "Path",
        ty: FieldType::Text,
        ops: &[Op::Contains],
        help: "Libraries whose path contains this text",
        example: "path:photos",
        in_global: true,
        column: "path",
    }];
    const FREE_TEXT: FreeText = FreeText::Column("path");
    const FREE_TEXT_HELP: &'static str =
        "Words are matched against the library path; any one word may match";
}

// the structured query, as sent on the wire.  an empty query matches everything
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchQuery {
    // free-text terms, combined with OR
    #[serde(default)]
    pub text: Vec<String>,
    // field clauses, combined with AND (and with the text)
    #[serde(default)]
    pub clauses: Vec<Clause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Clause {
    pub field: String,
    pub op: Op,
    // Range values are "a..b"
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseIssue {
    // byte range of the offending token in the input
    pub span: Range<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub query: SearchQuery,
    pub issues: Vec<ParseIssue>,
}

// limits on what the server accepts, so a request can't ask for an absurd statement
pub const MAX_TEXT_TERMS: usize = 32;
pub const MAX_CLAUSES: usize = 32;
pub const MAX_VALUE_LEN: usize = 256;

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.clauses.is_empty()
    }

    // parse typed query text; see the grammar at the top of this file
    pub fn parse<C: SearchClass>(input: &str) -> Parsed {
        let mut parsed = Parsed::default();

        for (span, raw) in tokenize(input) {
            let mut issue = |message: String| {
                parsed.issues.push(ParseIssue {
                    span: span.clone(),
                    message,
                });
            };

            if raw.matches('"').count() % 2 == 1 {
                issue("unterminated quote".into());
            }

            let Some((head, rest)) = split_field(&raw) else {
                push_text(&mut parsed.query, unquote(&raw));
                continue;
            };

            let Some(spec) = field_spec::<C>(&head.to_ascii_lowercase()) else {
                issue(format!("unknown field '{head}'"));
                push_text(&mut parsed.query, unquote(&raw));
                continue;
            };

            // an operator prefix only counts before any quote
            let (prefix_op, rest) = split_op(rest);
            let mut value = unquote(rest);

            if value.is_empty() {
                issue(format!("{} needs a value", spec.name));
                continue;
            }

            let op = match prefix_op {
                Some(op) => op,
                None if spec.ops.contains(&Op::Range) && value.contains("..") => Op::Range,
                None => spec.default_op(),
            };

            if !spec.ops.contains(&op) {
                let how = match op.prefix() {
                    "" => "that kind of match".to_string(),
                    p => format!("'{p}'"),
                };
                issue(format!("{} does not support {how}", spec.name));
                continue;
            }

            if let FieldType::Enum(allowed) = spec.ty {
                // accept any case, store the declared spelling
                match allowed.iter().find(|a| a.eq_ignore_ascii_case(&value)) {
                    Some(a) => value = a.to_string(),
                    None => {
                        issue(format!(
                            "{} must be one of {}",
                            spec.name,
                            allowed.join(", ")
                        ));
                        continue;
                    }
                }
            }

            if let Err(message) = check_value(spec, op, &value) {
                issue(message);
                continue;
            }

            parsed.query.clauses.push(Clause {
                field: spec.name.to_string(),
                op,
                value,
            });
        }

        parsed
    }

    // the canonical text form, such that parse(to_text(q)) == q for any query that parse
    // can produce
    pub fn to_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        for t in &self.text {
            // a colon would read as a field
            parts.push(quote_if(t, t.contains(':')));
        }

        for c in &self.clauses {
            let v = &c.value;
            let (prefix, quoted) = match c.op {
                Op::Gt | Op::Ge | Op::Lt | Op::Le => (c.op.prefix(), false),
                // a leading operator character would read as an operator
                _ => ("", v.starts_with(['>', '<'])),
            };
            parts.push(format!("{}:{}{}", c.field, prefix, quote_if(v, quoted)));
        }

        parts.join(" ")
    }

    // server-side check of a wire query: unknown fields and operators are rejected, and so
    // are values that could never match (the resolver would otherwise match nothing)
    pub fn validate<C: SearchClass>(&self) -> Result<(), String> {
        if self.text.len() > MAX_TEXT_TERMS {
            return Err(format!("at most {MAX_TEXT_TERMS} search words"));
        }
        if self.clauses.len() > MAX_CLAUSES {
            return Err(format!("at most {MAX_CLAUSES} search clauses"));
        }
        if self.text.iter().any(|t| t.len() > MAX_VALUE_LEN) {
            return Err(format!("search words are limited to {MAX_VALUE_LEN} bytes"));
        }

        for c in &self.clauses {
            let spec = field_spec::<C>(&c.field)
                .ok_or_else(|| format!("unknown search field '{}'", c.field))?;

            if !spec.ops.contains(&c.op) {
                return Err(format!(
                    "search field '{}' does not support {:?}",
                    c.field, c.op
                ));
            }

            if let FieldType::Enum(allowed) = spec.ty
                && !allowed.contains(&c.value.as_str())
            {
                return Err(format!(
                    "search field '{}' must be one of {}",
                    c.field,
                    allowed.join(", ")
                ));
            }

            check_value(spec, c.op, &c.value)?;
        }

        Ok(())
    }
}

fn push_text(query: &mut SearchQuery, term: String) {
    if !term.is_empty() {
        query.text.push(term);
    }
}

// value rules shared by the parser and validate
fn check_value(spec: &FieldSpec, op: Op, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_VALUE_LEN {
        return Err(format!(
            "{} values must be 1 to {MAX_VALUE_LEN} bytes",
            spec.name
        ));
    }

    let single = |v: &str| -> Result<(), String> {
        match spec.ty {
            // a tag containing the separator could match across the folded tags column
            FieldType::Tag if v.contains(FOLDING_SEPARATOR) => Err(format!(
                "{} cannot contain '{FOLDING_SEPARATOR}'",
                spec.name
            )),
            FieldType::Date if !v.chars().all(|c| c.is_ascii_digit() || c == '-') => Err(format!(
                "{} must look like 2020, 2020-06 or 2020-06-15",
                spec.name
            )),
            _ => Ok(()),
        }
    };

    if op == Op::Range {
        let (a, b) = value
            .split_once("..")
            .ok_or_else(|| format!("{} ranges look like a..b", spec.name))?;
        if a.is_empty() || b.is_empty() {
            return Err(format!("{} ranges need both ends", spec.name));
        }
        single(a)?;
        single(b)
    } else {
        single(value)
    }
}

// split the input on whitespace outside quotes, returning each token's byte span and text
fn tokenize(input: &str) -> Vec<(Range<usize>, String)> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    let mut in_quote = false;
    let mut escaped = false;

    for (i, c) in input.char_indices() {
        if start.is_none() {
            if c.is_whitespace() {
                continue;
            }
            start = Some(i);
        }

        if in_quote && escaped {
            escaped = false;
        } else if in_quote && c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_quote = !in_quote;
        } else if !in_quote && c.is_whitespace() {
            let s = start.take().unwrap_or(i);
            tokens.push((s..i, input[s..i].to_string()));
        }
    }

    if let Some(s) = start {
        tokens.push((s..input.len(), input[s..].to_string()));
    }

    tokens
}

// `field:rest`, where field looks like an identifier; None for everything else
fn split_field(raw: &str) -> Option<(&str, &str)> {
    let (head, rest) = raw.split_once(':')?;
    let mut chars = head.chars();
    let ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some((head, rest))
}

fn split_op(rest: &str) -> (Option<Op>, &str) {
    for op in [Op::Ge, Op::Le, Op::Gt, Op::Lt] {
        if let Some(r) = rest.strip_prefix(op.prefix()) {
            return (Some(op), r);
        }
    }
    (None, rest)
}

// drop the quotes from a token and resolve \" and \\ inside them
fn unquote(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_quote = false;
    let mut chars = raw.chars();

    while let Some(c) = chars.next() {
        match c {
            '"' => in_quote = !in_quote,
            '\\' if in_quote => match chars.next() {
                Some(n @ ('"' | '\\')) => out.push(n),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            },
            _ => out.push(c),
        }
    }

    out
}

fn quote_if(v: &str, force: bool) -> String {
    let needs =
        force || v.is_empty() || v.contains(|c: char| c.is_whitespace() || c == '"' || c == '\\');

    if !needs {
        return v.to_string();
    }

    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::media::MediaMetadata;

    fn p(s: &str) -> Parsed {
        SearchQuery::parse::<Media>(s)
    }

    fn clause(field: &str, op: Op, value: &str) -> Clause {
        Clause {
            field: field.into(),
            op,
            value: value.into(),
        }
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_and_blank_input() {
        for s in ["", "   ", "\t\n"] {
            let r = p(s);
            assert!(r.query.is_empty());
            assert!(r.issues.is_empty());
        }
    }

    #[test]
    fn free_text_words() {
        let r = p("  beach   sunset ");
        assert_eq!(r.query.text, strs(&["beach", "sunset"]));
        assert!(r.query.clauses.is_empty());
        assert!(r.issues.is_empty());

        // quoted phrases stay together, and a colon in a non-field word is just text
        let r = p(r#""big wave" 12:30"#);
        assert_eq!(r.query.text, strs(&["big wave", "12:30"]));
        assert!(r.issues.is_empty());
    }

    #[test]
    fn every_operator() {
        let r = p("tag:holiday description:sun date:2020-06 type:image");
        assert_eq!(
            r.query.clauses,
            vec![
                clause("tag", Op::Has, "holiday"),
                clause("description", Op::Contains, "sun"),
                clause("date", Op::Eq, "2020-06"),
                clause("type", Op::Eq, "Image"),
            ]
        );
        assert!(r.issues.is_empty());

        let r = p("date:>2020 date:>=2020-02 date:<2021 date:<=2021-03 date:2020-01..2020-06");
        assert_eq!(
            r.query.clauses,
            vec![
                clause("date", Op::Gt, "2020"),
                clause("date", Op::Ge, "2020-02"),
                clause("date", Op::Lt, "2021"),
                clause("date", Op::Le, "2021-03"),
                clause("date", Op::Range, "2020-01..2020-06"),
            ]
        );
        assert!(r.issues.is_empty());
    }

    #[test]
    fn field_names_ignore_case() {
        let r = p("TAG:x Type:VIDEOSLICE");
        assert_eq!(
            r.query.clauses,
            vec![
                clause("tag", Op::Has, "x"),
                clause("type", Op::Eq, "VideoSlice")
            ]
        );
    }

    #[test]
    fn quoted_values() {
        let r = p(r#"description:"a big wave" tag:"two words" description:"say \"hi\" \\""#);
        assert_eq!(
            r.query.clauses,
            vec![
                clause("description", Op::Contains, "a big wave"),
                clause("tag", Op::Has, "two words"),
                clause("description", Op::Contains, r#"say "hi" \"#),
            ]
        );
        assert!(r.issues.is_empty());

        // a quoted value can start with an operator character literally
        let r = p(r#"description:">3""#);
        assert_eq!(
            r.query.clauses,
            vec![clause("description", Op::Contains, ">3")]
        );
        assert!(r.issues.is_empty());

        // an operator before a quoted value still applies
        let r = p(r#"date:>"2020""#);
        assert_eq!(r.query.clauses, vec![clause("date", Op::Gt, "2020")]);
    }

    #[test]
    fn unknown_field_keeps_free_text_and_flags_issue() {
        let input = "beach color:red";
        let r = p(input);
        assert_eq!(r.query.text, strs(&["beach", "color:red"]));
        assert!(r.query.clauses.is_empty());
        assert_eq!(r.issues.len(), 1);
        assert_eq!(&input[r.issues[0].span.clone()], "color:red");
        assert!(r.issues[0].message.contains("unknown field"));
    }

    #[test]
    fn bad_ops_and_values_drop_the_clause() {
        for (input, needle) in [
            ("tag:>x", "does not support"),
            ("description:<=x", "does not support"),
            ("type:>Image", "does not support"),
            ("type:Pdf", "must be one of"),
            ("date:soon", "must look like"),
            ("date:2020..", "both ends"),
            ("date:..2020", "both ends"),
            ("tag:", "needs a value"),
            ("tag:a|b", "cannot contain"),
        ] {
            let r = p(input);
            assert!(r.query.is_empty(), "{input}: {:?}", r.query);
            assert_eq!(r.issues.len(), 1, "{input}");
            assert!(
                r.issues[0].message.contains(needle),
                "{input}: {:?}",
                r.issues
            );
            assert_eq!(r.issues[0].span, 0..input.len());
        }

        // a range is only a range where the field allows it
        let r = p("tag:a..b");
        assert_eq!(r.query.clauses, vec![clause("tag", Op::Has, "a..b")]);
        assert!(r.issues.is_empty());
    }

    #[test]
    fn unterminated_quote_is_an_issue() {
        let r = p(r#"description:"abc def"#);
        assert_eq!(
            r.query.clauses,
            vec![clause("description", Op::Contains, "abc def")]
        );
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].message, "unterminated quote");
    }

    #[test]
    fn issue_spans_point_at_the_token() {
        let input = "a  bad:x  b tag:";
        let r = p(input);
        let spans: Vec<_> = r.issues.iter().map(|i| &input[i.span.clone()]).collect();
        assert_eq!(spans, vec!["bad:x", "tag:"]);
    }

    #[test]
    fn mixed_query() {
        let r = p("beach tag:holiday sunset date:2019..2020");
        assert_eq!(r.query.text, strs(&["beach", "sunset"]));
        assert_eq!(r.query.clauses.len(), 2);
    }

    #[test]
    fn to_text_round_trips() {
        let inputs = [
            "",
            "beach",
            "beach sunset tag:holiday",
            r#""big wave" description:"a b" date:2020-01..2020-06"#,
            "date:>2020 date:>=2020-02 date:<2021 date:<=2021-03 type:Audio",
            r#"description:">3" description:"say \"hi\" \\" "12:30""#,
            r#"tag:"<odd""#,
            "tag:a..b",
        ];
        for input in inputs {
            let q = p(input).query;
            let again = p(&q.to_text());
            assert!(again.issues.is_empty(), "{input} -> {}", q.to_text());
            assert_eq!(again.query, q, "{input} -> {}", q.to_text());
        }

        assert_eq!(
            p("beach   tag:holiday date:2020-01..2020-06")
                .query
                .to_text(),
            "beach tag:holiday date:2020-01..2020-06"
        );
    }

    #[test]
    fn validate_accepts_parsed_queries() {
        for input in [
            "",
            "a b",
            "tag:x description:y date:2020..2021 type:Video date:<=2020",
        ] {
            assert_eq!(p(input).query.validate::<Media>(), Ok(()), "{input}");
        }
    }

    #[test]
    fn validate_rejects_bad_queries() {
        let bad = |c: Clause| {
            SearchQuery {
                text: vec![],
                clauses: vec![c],
            }
            .validate::<Media>()
            .unwrap_err()
        };

        assert!(bad(clause("color", Op::Eq, "red")).contains("unknown search field"));
        // a field of another class
        assert!(bad(clause("path", Op::Contains, "x")).contains("unknown search field"));
        assert!(bad(clause("tag", Op::Gt, "x")).contains("does not support"));
        assert!(bad(clause("description", Op::Eq, "x")).contains("does not support"));
        assert!(bad(clause("type", Op::Eq, "image")).contains("must be one of"));
        assert!(bad(clause("date", Op::Eq, "x'; DROP")).contains("must look like"));
        assert!(bad(clause("date", Op::Range, "2020")).contains("a..b"));
        assert!(bad(clause("tag", Op::Has, "")).contains("values must be"));
        assert!(bad(clause("tag", Op::Has, "a|b")).contains("cannot contain"));
        assert!(
            bad(clause("tag", Op::Has, &"x".repeat(MAX_VALUE_LEN + 1))).contains("values must be")
        );

        let many = SearchQuery {
            text: vec!["a".into(); MAX_TEXT_TERMS + 1],
            clauses: vec![],
        };
        assert!(many.validate::<Media>().is_err());
        let many = SearchQuery {
            text: vec![],
            clauses: vec![clause("tag", Op::Has, "x"); MAX_CLAUSES + 1],
        };
        assert!(many.validate::<Media>().is_err());

        // other classes validate against their own fields
        let q = SearchQuery::parse::<Library>("path:photos").query;
        assert_eq!(q.validate::<Library>(), Ok(()));
        assert!(q.validate::<Media>().is_err());
    }

    #[test]
    fn query_wire_format() {
        let q = p("a tag:x date:>2020").query;
        let v = serde_json::to_value(&q).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "text": ["a"],
                "clauses": [
                    {"field": "tag", "op": "Has", "value": "x"},
                    {"field": "date", "op": "Gt", "value": "2020"},
                ],
            })
        );
        assert_eq!(serde_json::from_value::<SearchQuery>(v).unwrap(), q);

        // both lists are optional on the wire
        let q: SearchQuery = serde_json::from_str("{}").unwrap();
        assert!(q.is_empty());
    }

    #[test]
    fn field_tables_are_consistent() {
        fn check<C: SearchClass>() {
            let mut names: Vec<_> = C::FIELDS.iter().map(|f| f.name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), C::FIELDS.len(), "duplicate field names");

            for f in C::FIELDS {
                assert!(!f.ops.is_empty() && !f.help.is_empty() && !f.label.is_empty());
                assert!(f.ops.contains(&f.default_op()), "{}", f.name);
                // every example parses, to a clause on its own field, without issues
                let r = SearchQuery::parse::<C>(f.example);
                assert!(r.issues.is_empty(), "{}: {:?}", f.example, r.issues);
                assert_eq!(r.query.clauses.len(), 1, "{}", f.example);
                assert_eq!(r.query.clauses[0].field, f.name);
                assert_eq!(r.query.validate::<C>(), Ok(()));
            }
        }
        check::<Media>();
        check::<Collection>();
        check::<Library>();
    }

    #[test]
    fn media_type_values_match_the_enum() {
        for t in MEDIA_TYPES {
            assert!(MediaMetadata::from_str(t).is_ok(), "{t}");
        }
        // and the enum has no variant the field lacks
        for (i, t) in ["Image", "Video", "VideoSlice", "Audio"].iter().enumerate() {
            assert_eq!(MEDIA_TYPES[i], *t);
        }
    }

    #[test]
    fn declared_columns_cover_every_class() {
        let cols = all_declared_columns();
        assert!(cols.contains(&("media", "tags")));
        assert!(cols.contains(&("media", "media_type")));
        assert!(cols.contains(&("collections", "note")));
        assert!(cols.contains(&("libraries", "path")));
        // collections and libraries are covered by the startup check from phase 3
        assert_eq!(declared_columns::<Media>().len(), 4);
    }
}
