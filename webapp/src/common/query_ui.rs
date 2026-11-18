use api::{
    query::{
        Clause, FilterType, IssueKind, Op, ParseIssue, SearchClass, SearchQuery, Values,
        any_values, field_spec, tokenize,
    },
    slug::Slug,
};

use crate::common::slugs::pickable;

// the pure half of the search bar: chips, completions, the filter builder and the ?q= URL
// parameter.  everything here works from the class's FIELDS metadata and the api parser, so
// the components stay thin and the logic is testable without a browser

// chips

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipTarget {
    // all the free-text words, shown as one chip
    Words,
    // the clause at this index in SearchQuery::clauses
    Clause(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    // the tooltip: the field's help text
    pub help: &'static str,
    pub target: ChipTarget,
}

// how an operator reads in a chip or a select
pub fn op_label(op: Op) -> &'static str {
    match op {
        Op::Contains => "contains",
        Op::Has => "has",
        Op::Eq => "is",
        Op::Gt => "after (>)",
        Op::Ge => "from (>=)",
        Op::Lt => "before (<)",
        Op::Le => "up to (<=)",
        Op::Range => "between (a..b)",
        Op::HasAny => "any of (a,b)",
    }
}

fn clause_value_label(clause: &Clause) -> String {
    match clause.op {
        Op::Range => match clause.value.split_once("..") {
            Some((a, b)) => format!("{a} \u{2026} {b}"),
            None => clause.value.clone(),
        },
        Op::Gt => format!("> {}", clause.value),
        Op::Ge => format!(">= {}", clause.value),
        Op::Lt => format!("< {}", clause.value),
        Op::Le => format!("<= {}", clause.value),
        Op::HasAny => any_values(&clause.value).join(" or "),
        _ => clause.value.clone(),
    }
}

// one chip per clause, preceded by a chip for the free-text words when there are any
pub fn chips<C: SearchClass>(query: &SearchQuery) -> Vec<Chip> {
    let mut out = Vec::new();

    if !query.text.is_empty() {
        out.push(Chip {
            label: format!("Words: {}", query.text.join(", ")),
            help: C::FREE_TEXT_HELP,
            target: ChipTarget::Words,
        });
    }

    for (i, clause) in query.clauses.iter().enumerate() {
        let (label, help) = match field_spec::<C>(&clause.field) {
            Some(spec) => (spec.label, spec.help),
            None => ("?", ""),
        };
        out.push(Chip {
            label: format!("{label}: {}", clause_value_label(clause)),
            help,
            target: ChipTarget::Clause(i),
        });
    }

    out
}

// the query without one chip
pub fn remove_chip(query: &SearchQuery, target: ChipTarget) -> SearchQuery {
    let mut q = query.clone();
    match target {
        ChipTarget::Words => q.text.clear(),
        ChipTarget::Clause(i) => {
            if i < q.clauses.len() {
                q.clauses.remove(i);
            }
        }
    }
    q
}

// the text with one chip removed, re-serialized
pub fn text_without_chip<C: SearchClass>(text: &str, target: ChipTarget) -> String {
    remove_chip(&SearchQuery::parse::<C>(text).query, target).to_text()
}

// the text with one clause added, for the filter builder.  `to` is only used by Range
pub fn add_clause<C: SearchClass>(
    text: &str,
    field: &str,
    op: Op,
    value: &str,
    to: &str,
) -> Result<String, String> {
    let spec = field_spec::<C>(field).ok_or_else(|| format!("unknown field '{field}'"))?;
    let (value, to) = (value.trim(), to.trim());

    let value = if op == Op::Range {
        if value.is_empty() || to.is_empty() {
            return Err(format!("{} ranges need both ends", spec.label));
        }
        format!("{value}..{to}")
    } else if value.is_empty() {
        return Err(format!("{} needs a value", spec.label));
    } else {
        value.to_string()
    };

    let mut query = SearchQuery::parse::<C>(text).query;
    query.clauses.push(Clause {
        field: spec.name.to_string(),
        op,
        value,
    });
    query.validate::<C>()?;
    Ok(query.to_text())
}

// issues

// one line for a parse issue, naming what the search did with the token
pub fn issue_line(input: &str, issue: &ParseIssue) -> String {
    let token = input.get(issue.span.clone()).unwrap_or("");
    match issue.kind {
        IssueKind::UnknownField => format!("{}; searched as text", issue.message),
        IssueKind::Syntax => format!("{token}: {}", issue.message),
        IssueKind::BadOp | IssueKind::BadValue => format!("{token}: {}; ignored", issue.message),
    }
}

// the search box placeholder: what plain words match, and one example clause
pub fn placeholder<C: SearchClass>() -> String {
    match C::FIELDS.first() {
        Some(f) => format!("{} (try {})", C::FREE_TEXT_HELP, f.example),
        None => C::FREE_TEXT_HELP.to_string(),
    }
}

// completions

// value completions shown at once
pub const MAX_SLUG_SUGGESTIONS: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub title: String,
    pub help: String,
    pub example: String,
    // what replaces the token under the cursor
    pub insert: String,
}

// the token being typed: the last one, when the input doesn't end in whitespace or an open
// quote (the cursor is taken to be at the end, which is where typing happens)
fn last_token(draft: &str) -> Option<(usize, String)> {
    let (span, raw) = tokenize(draft).pop()?;
    if span.end != draft.len() || raw.matches('"').count() % 2 == 1 {
        return None;
    }
    Some((span.start, raw))
}

// the field and typed prefix whose values (GetSlugs) the completions need: the last token
// is `field:` or `field:a,b,pre` on a field whose values come from the data.  None otherwise
pub fn slug_query<C: SearchClass>(draft: &str) -> Option<(&'static str, String)> {
    let (_, token) = last_token(draft)?;
    let (head, rest) = token.split_once(':')?;
    let spec = field_spec::<C>(&head.to_ascii_lowercase())?;
    if spec.filter_type.values() != Some(Values::Distinct) || rest.starts_with('"') {
        return None;
    }
    let typed = rest.rsplit(',').next().unwrap_or("");
    Some((spec.name, typed.to_lowercase()))
}

// completions for the token being typed.  `slugs` are the values GetSlugs returned for
// slug_query(draft); they may lag behind the typing, so they are filtered again here
pub fn suggest<C: SearchClass>(draft: &str, slugs: &[Slug]) -> Vec<Suggestion> {
    let Some((_, token)) = last_token(draft) else {
        return Vec::new();
    };

    let Some((head, rest)) = token.split_once(':') else {
        // a field name being typed
        let typed = token.to_ascii_lowercase();
        return C::FIELDS
            .iter()
            .filter(|f| f.name.starts_with(&typed))
            .map(|f| Suggestion {
                title: f.label.to_string(),
                help: f.help.to_string(),
                example: f.example.to_string(),
                insert: format!("{}:", f.name),
            })
            .collect();
    };

    let Some(spec) = field_spec::<C>(&head.to_ascii_lowercase()) else {
        return Vec::new();
    };

    match spec.filter_type {
        FilterType::Select(Values::Static(allowed))
        | FilterType::MultiSelect(Values::Static(allowed)) => {
            let typed = rest.to_ascii_lowercase();
            allowed
                .iter()
                .filter(|v| v.to_ascii_lowercase().starts_with(&typed))
                .map(|v| Suggestion {
                    title: v.to_string(),
                    help: spec.help.to_string(),
                    example: String::new(),
                    insert: format!("{}:{v}", spec.name),
                })
                .collect()
        }

        // values from the data: complete the last item of `tag:a,b,pre`, leaving out what is
        // already listed; a multiselect field takes the rest as any-of
        FilterType::Select(Values::Distinct) | FilterType::MultiSelect(Values::Distinct) => {
            if rest.starts_with('"') {
                return Vec::new();
            }
            let (listed, typed) = match rest.rsplit_once(',') {
                Some((listed, typed)) => (any_values(listed), typed),
                None => (Vec::new(), rest),
            };
            let typed = typed.to_lowercase();
            let multi = matches!(spec.filter_type, FilterType::MultiSelect(_));
            slugs
                .iter()
                .filter(|s| s.value.to_lowercase().starts_with(&typed))
                .filter(|s| !listed.contains(&s.value.as_str()))
                .filter(|s| pickable(&s.value))
                .take(MAX_SLUG_SUGGESTIONS)
                .map(|s| {
                    let mut values: Vec<&str> = if multi { listed.clone() } else { Vec::new() };
                    values.push(&s.value);
                    Suggestion {
                        title: s.value.clone(),
                        help: match s.count {
                            Some(1) => "1 match".to_string(),
                            Some(n) => format!("{n} matches"),
                            None => spec.help.to_string(),
                        },
                        example: String::new(),
                        insert: format!("{}:{}", spec.name, values.join(",")),
                    }
                })
                .collect()
        }

        FilterType::Date => {
            let mut out = Vec::new();

            if rest.is_empty() {
                for op in spec.ops {
                    let prefix = match op {
                        Op::Gt => ">",
                        Op::Ge => ">=",
                        Op::Lt => "<",
                        Op::Le => "<=",
                        _ => continue,
                    };
                    out.push(Suggestion {
                        title: prefix.to_string(),
                        help: op_label(*op).to_string(),
                        example: format!("{}:{prefix}2020-06", spec.name),
                        insert: format!("{}:{prefix}", spec.name),
                    });
                }
                if spec.ops.contains(&Op::Range) {
                    out.push(Suggestion {
                        title: "a..b".to_string(),
                        help: op_label(Op::Range).to_string(),
                        example: spec.example.to_string(),
                        insert: spec.example.to_string(),
                    });
                }
            } else if spec.ops.contains(&Op::Range)
                && !rest.contains("..")
                && rest.chars().all(|c| c.is_ascii_digit() || c == '-')
            {
                out.push(Suggestion {
                    title: format!("{rest}.."),
                    help: op_label(Op::Range).to_string(),
                    example: spec.example.to_string(),
                    insert: format!("{}:{rest}..", spec.name),
                });
            }

            out
        }

        // free-form values have nothing to complete
        FilterType::Text => Vec::new(),
    }
}

// the draft with the token under the cursor replaced by a suggestion; a finished value
// (as opposed to a field name or an operator) gets a space after it
// every field of the class, for the hint list a double-click on the search bar opens
pub fn all_fields<C: SearchClass>() -> Vec<Suggestion> {
    C::FIELDS
        .iter()
        .map(|f| Suggestion {
            title: f.label.to_string(),
            help: f.help.to_string(),
            example: f.example.to_string(),
            insert: format!("{}:", f.name),
        })
        .collect()
}

// add a suggestion after what is already typed (rather than replacing the last token, as
// accept does), separated by a space
pub fn append(draft: &str, suggestion: &Suggestion) -> String {
    let mut out = draft.to_string();
    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
        out.push(' ');
    }
    out.push_str(&suggestion.insert);
    out
}

pub fn accept(draft: &str, suggestion: &Suggestion) -> String {
    let start = last_token(draft).map(|(s, _)| s).unwrap_or(draft.len());
    let mut out = format!("{}{}", &draft[..start], suggestion.insert);
    if !suggestion.insert.ends_with([':', '>', '<', '=', '.']) && !suggestion.insert.contains("..")
    {
        out.push(' ');
    }
    out
}

// the ?q= route parameter
//
// the router writes `q` with percent-encoding, but on the way back it decodes the whole query
// string *before* splitting it on `&` and `=`.  everything round-trips except `&`, which would
// cut the value short, so the text is escaped here first: `%` becomes %25 and `&` becomes %26,
// and the router's own encoding is then applied on top of that
pub fn q_param(text: &str) -> String {
    text.replace('%', "%25").replace('&', "%26")
}

// the inverse of q_param; an escape it doesn't know is left alone, so a hand-typed
// `?q=50%off` still reads as typed
pub fn q_from_param(param: &str) -> String {
    let mut out = String::with_capacity(param.len());
    let mut rest = param;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(t) = tail.strip_prefix("%25") {
            out.push('%');
            rest = t;
        } else if let Some(t) = tail.strip_prefix("%26") {
            out.push('&');
            rest = t;
        } else {
            out.push('%');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn all_fields_lists_every_field_and_append_keeps_the_text() {
        use api::media::Media;
        let all = all_fields::<Media>();
        assert_eq!(
            all.iter().map(|s| s.insert.as_str()).collect::<Vec<_>>(),
            Media::FIELDS
                .iter()
                .map(|f| format!("{}:", f.name))
                .collect::<Vec<_>>()
        );
        let tag = all.iter().find(|s| s.insert == "tag:").unwrap();
        assert_eq!(append("", tag), "tag:");
        assert_eq!(append("beach", tag), "beach tag:");
        assert_eq!(append("beach ", tag), "beach tag:");
        assert_eq!(append("date:2020 be", tag), "date:2020 be tag:");
    }

    use super::*;
    use api::{collection::Collection, library::Library, media::Media, query::DataType};

    fn q(text: &str) -> SearchQuery {
        SearchQuery::parse::<Media>(text).query
    }

    fn labels(text: &str) -> Vec<String> {
        chips::<Media>(&q(text))
            .into_iter()
            .map(|c| c.label)
            .collect()
    }

    #[test]
    fn chips_show_every_clause_and_the_words() {
        assert_eq!(
            labels("beach tag:ct-mod7 date:2022 date:2020-01..2020-06 date:>2019 type:image"),
            [
                "Words: beach",
                "Tag: ct-mod7",
                "Date: 2022",
                "Date: 2020-01 \u{2026} 2020-06",
                "Date: > 2019",
                "Type: Image",
            ]
        );
        assert!(labels("").is_empty());

        // the tooltip is the field's help
        let c = chips::<Media>(&q("tag:x"));
        assert_eq!(c[0].help, field_spec::<Media>("tag").unwrap().help);
        let c = chips::<Collection>(&SearchQuery::parse::<Collection>("name:x trip").query);
        assert_eq!(c[0].help, Collection::FREE_TEXT_HELP);
        assert_eq!(c[1].label, "Name: x");
    }

    #[test]
    fn removing_a_chip_rewrites_the_text() {
        let text = "beach tag:ct-mod7 date:2022";

        assert_eq!(
            text_without_chip::<Media>(text, ChipTarget::Clause(0)),
            "beach date:2022"
        );
        assert_eq!(
            text_without_chip::<Media>(text, ChipTarget::Clause(1)),
            "beach tag:ct-mod7"
        );
        assert_eq!(
            text_without_chip::<Media>(text, ChipTarget::Words),
            "tag:ct-mod7 date:2022"
        );
        // out of range is a no-op, and the last chip leaves an empty search
        assert_eq!(
            text_without_chip::<Media>("tag:a", ChipTarget::Clause(5)),
            "tag:a"
        );
        assert_eq!(
            text_without_chip::<Media>("tag:a", ChipTarget::Clause(0)),
            ""
        );
    }

    #[test]
    fn adding_a_clause_round_trips_through_the_text() {
        let t = add_clause::<Media>("beach", "type", Op::Eq, "Image", "").unwrap();
        assert_eq!(t, "beach type:Image");

        // values with spaces and quotes survive, and re-adding what was removed is the identity
        let t =
            add_clause::<Media>(&t, "description", Op::Contains, r#"a "big" wave"#, "").unwrap();
        let parsed = SearchQuery::parse::<Media>(&t);
        assert!(parsed.issues.is_empty());
        assert_eq!(parsed.query.clauses[1].value, r#"a "big" wave"#);
        assert_eq!(
            text_without_chip::<Media>(&t, ChipTarget::Clause(1)),
            "beach type:Image"
        );

        let t = add_clause::<Media>("", "date", Op::Range, " 2020-01 ", "2020-06").unwrap();
        assert_eq!(t, "date:2020-01..2020-06");
        let t = add_clause::<Media>("", "date", Op::Ge, "2020", "").unwrap();
        assert_eq!(t, "date:>=2020");

        // every clause the builder makes parses back to itself
        for f in Media::FIELDS {
            for op in f.ops {
                let value = match (f.data_type, op) {
                    (DataType::Enum(a), _) => a[0],
                    (DataType::Date, _) => "2020-06",
                    (_, Op::HasAny) => "x,y",
                    _ => "x y",
                };
                let t = add_clause::<Media>("", f.name, *op, value, "2021").unwrap();
                let p = SearchQuery::parse::<Media>(&t);
                assert!(p.issues.is_empty(), "{t}: {:?}", p.issues);
                assert_eq!(p.query.clauses.len(), 1, "{t}");
                assert_eq!(p.query.clauses[0].op, *op, "{t}");
                assert_eq!(p.query.to_text(), t);
            }
        }
    }

    #[test]
    fn adding_a_bad_clause_is_an_error() {
        let bad = |field, op, value, to| add_clause::<Media>("", field, op, value, to).unwrap_err();
        assert!(bad("tag", Op::Has, "  ", "").contains("needs a value"));
        assert!(bad("date", Op::Range, "2020", "").contains("both ends"));
        assert!(bad("date", Op::Eq, "soon", "").contains("must look like"));
        assert!(bad("tag", Op::Gt, "x", "").contains("does not support"));
        assert!(bad("color", Op::Eq, "x", "").contains("unknown field"));
        assert!(bad("type", Op::Eq, "Pdf", "").contains("must be one of"));
        // a field of another class
        assert!(add_clause::<Library>("", "tag", Op::Has, "x", "").is_err());
        assert_eq!(
            add_clause::<Library>("", "path", Op::Contains, "photos", "").unwrap(),
            "path:photos"
        );
    }

    #[test]
    fn issue_lines_say_what_happened() {
        let input = "beach colour:red date:soon description:\"abc";
        let parsed = SearchQuery::parse::<Media>(input);
        let lines: Vec<_> = parsed.issues.iter().map(|i| issue_line(input, i)).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "unknown field 'colour'; searched as text");
        assert!(lines[1].starts_with("date:soon: date must look like"));
        assert!(lines[1].ends_with("; ignored"));
        assert_eq!(lines[2], "description:\"abc: unterminated quote");
    }

    fn inserts(draft: &str) -> Vec<String> {
        suggest::<Media>(draft, &[])
            .into_iter()
            .map(|s| s.insert)
            .collect()
    }

    #[test]
    fn typing_a_field_name_suggests_fields() {
        assert_eq!(inserts("ta"), ["tag:"]);
        assert_eq!(inserts("beach TA"), ["tag:"]);
        assert_eq!(inserts("t"), ["tag:", "type:"]);
        assert_eq!(inserts("d"), ["description:", "date:"]);

        // the metadata comes along for the tooltip
        let s = &suggest::<Media>("ta", &[])[0];
        assert_eq!(s.title, "Tag");
        assert_eq!(s.example, "tag:holiday");
        assert!(!s.help.is_empty());

        // nothing while between words, inside quotes, or for words that aren't field-like
        assert!(inserts("").is_empty());
        assert!(inserts("beach ").is_empty());
        assert!(inserts(r#"description:"ta"#).is_empty());
        assert!(inserts("zzz").is_empty());
        assert_eq!(suggest::<Collection>("na", &[])[0].insert, "name:");
        assert_eq!(suggest::<Library>("p", &[])[0].insert, "path:");
    }

    #[test]
    fn after_a_colon_the_values_are_suggested() {
        assert_eq!(
            inserts("type:"),
            ["type:Image", "type:Video", "type:VideoSlice", "type:Audio"]
        );
        assert_eq!(inserts("type:v"), ["type:Video", "type:VideoSlice"]);
        assert_eq!(inserts("beach TYPE:AU"), ["type:Audio"]);
        assert!(inserts("type:Pdf").is_empty());

        assert_eq!(
            inserts("date:"),
            [
                "date:>",
                "date:>=",
                "date:<",
                "date:<=",
                "date:2020-06..2020-08"
            ]
        );
        assert_eq!(inserts("date:2020"), ["date:2020.."]);
        assert!(inserts("date:2020..").is_empty());
        assert!(inserts("date:>2020").is_empty());

        // free-form values have nothing to complete, and tags have nothing until the
        // values arrive
        assert!(inserts("tag:").is_empty());
        assert!(inserts("description:su").is_empty());
        assert!(inserts("nope:").is_empty());
    }

    fn slugs(v: &[(&str, u64)]) -> Vec<Slug> {
        v.iter()
            .map(|(value, n)| Slug {
                value: value.to_string(),
                count: Some(*n),
            })
            .collect()
    }

    #[test]
    fn tag_values_come_from_the_slugs() {
        let have = slugs(&[("beach", 120), ("bees", 1), ("sunset", 7), ("a b", 3)]);
        let ins = |draft: &str| -> Vec<String> {
            suggest::<Media>(draft, &have)
                .into_iter()
                .map(|s| s.insert)
                .collect()
        };

        // which values to fetch for the token being typed
        assert_eq!(slug_query::<Media>("tag:"), Some(("tag", String::new())));
        assert_eq!(slug_query::<Media>("x TAG:Be"), Some(("tag", "be".into())));
        assert_eq!(
            slug_query::<Media>("tag:beach,Su"),
            Some(("tag", "su".into()))
        );
        assert_eq!(
            slug_query::<Collection>("tag:"),
            Some(("tag", String::new()))
        );
        assert_eq!(slug_query::<Media>("type:"), None);
        assert_eq!(slug_query::<Media>("description:"), None);
        assert_eq!(slug_query::<Media>(r#"tag:"be"#), None);
        assert_eq!(slug_query::<Library>("path:"), None);

        // values that can't travel unquoted in a list are left out
        assert_eq!(ins("tag:"), ["tag:beach", "tag:bees", "tag:sunset"]);
        assert_eq!(ins("tag:BE"), ["tag:beach", "tag:bees"]);
        // the last item of a list is completed, keeping the rest, never repeating one
        assert_eq!(
            ins("tag:sunset,be"),
            ["tag:sunset,beach", "tag:sunset,bees"]
        );
        assert_eq!(ins("tag:beach,"), ["tag:beach,bees", "tag:beach,sunset"]);

        let s = &suggest::<Media>("tag:be", &have)[0];
        assert_eq!(
            (s.title.as_str(), s.help.as_str()),
            ("beach", "120 matches")
        );
        assert_eq!(suggest::<Media>("tag:bees", &have)[0].help, "1 match");

        // an accepted list parses to one any-of clause
        let t = accept(
            "tag:sunset,be",
            &suggest::<Media>("tag:sunset,be", &have)[0],
        );
        assert_eq!(t, "tag:sunset,beach ");
        let q = SearchQuery::parse::<Media>(&t);
        assert!(q.issues.is_empty());
        assert_eq!(q.query.clauses[0].op, Op::HasAny);
    }

    #[test]
    fn any_of_chips_read_as_alternatives() {
        let q = SearchQuery::parse::<Media>("tag:beach,sunset").query;
        assert_eq!(chips::<Media>(&q)[0].label, "Tag: beach or sunset");
        assert_eq!(op_label(Op::HasAny), "any of (a,b)");
        // the builder writes the same clause a multiselect produces
        assert_eq!(
            add_clause::<Media>("x", "tag", Op::HasAny, "beach,sunset", "").unwrap(),
            "x tag:beach,sunset"
        );
        assert!(add_clause::<Media>("", "tag", Op::HasAny, "beach,beach", "").is_err());
    }

    #[test]
    fn accepting_replaces_only_the_last_token() {
        let pick = |draft: &str, i: usize| accept(draft, &suggest::<Media>(draft, &[])[i]);

        assert_eq!(pick("beach ta", 0), "beach tag:");
        assert_eq!(pick("type:", 1), "type:Video ");
        assert_eq!(pick("tag:a type:vi", 0), "tag:a type:Video ");
        assert_eq!(pick("date:", 1), "date:>=");
        assert_eq!(pick("date:", 4), "date:2020-06..2020-08");
        assert_eq!(pick("date:20", 0), "date:20..");

        // an accepted value parses cleanly
        let t = pick("beach type:", 0);
        assert!(SearchQuery::parse::<Media>(&t).issues.is_empty());
    }

    #[test]
    fn placeholder_names_the_help_and_an_example() {
        let p = placeholder::<Media>();
        assert!(p.starts_with(Media::FREE_TEXT_HELP));
        assert!(p.ends_with("(try tag:holiday)"));
        assert!(placeholder::<Library>().contains("path:photos"));
    }

    #[test]
    fn q_param_escapes_only_percent_and_ampersand() {
        assert_eq!(q_param("tag:a b"), "tag:a b");
        assert_eq!(q_param("50%&x"), "50%25%26x");

        for s in [
            "",
            "tag:beach",
            r#"description:"a b" tag:c%d&e"#,
            "100% & more",
            "%25 %26 %",
            "%2526",
            "&&&%%%",
            "\u{e9}\u{4e2d}",
        ] {
            assert_eq!(q_from_param(&q_param(s)), s, "{s}");
        }

        // a hand-typed % that isn't one of our escapes is left alone
        assert_eq!(q_from_param("50%off"), "50%off");
        assert_eq!(q_from_param("trailing%"), "trailing%");
        assert_eq!(q_from_param("%2"), "%2");
    }
}
