use serde::{Deserialize, Serialize};

use crate::{
    collection::{Collection, CollectionUuid},
    http_endpoint,
    library::{Library, LibraryUuid},
    media::{Media, MediaUuid},
    page::PageMeta,
    query::{IssueKind, SearchClass, SearchKind, SearchQuery},
};

// omnisearch: one query text searched across every class at once
//
// each class (a provider) parses the text with its own field metadata and runs its ordinary
// local search -- the same gid-filtered statement as its page -- as a separate concurrent
// query with its own timeout, so a slow provider never holds up the others.  each group
// carries an exact total of what matches plus the first few hits, for "showing 5, 40 more
// match"; picking a hit (or "see all") hands the same text to that class's page as ?q=.
//
// a query that uses a field a class doesn't declare (tag: for libraries) leaves that class
// out as NotApplicable rather than searching the token as text, which would show matches
// that ignore the filter

http_endpoint!(OmniSearch);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OmniSearchReq {
    pub query: String,
    // the classes to search; empty means all of them
    #[serde(default)]
    pub kinds: Vec<SearchKind>,
    // hits per group; PROVIDERS' default when missing, at most MAX_PER_GROUP
    #[serde(default)]
    pub per_group: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum HitId {
    Media(MediaUuid),
    Collection(CollectionUuid),
    Library(LibraryUuid),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchHit {
    pub id: HitId,
    pub title: String,
    pub subtitle: String,
    pub thumbnail: Option<MediaUuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum GroupStatus {
    Ok,
    // the query uses a field this class doesn't have
    NotApplicable { field: String },
    // the provider didn't answer within PROVIDER_TIMEOUT_MS
    TimedOut,
    Error(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HitGroup {
    pub kind: SearchKind,
    pub label: String,
    pub status: GroupStatus,
    pub hits: Vec<SearchHit>,
    // total is the exact count of everything that matches; row_count is hits.len()
    pub meta: PageMeta,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OmniSearchResp {
    pub groups: Vec<HitGroup>,
}

// the providers, in the order their groups are returned
#[derive(Clone, Copy, Debug)]
pub struct ProviderSpec {
    pub kind: SearchKind,
    pub label: &'static str,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        kind: SearchKind::Media,
        label: "Media",
    },
    ProviderSpec {
        kind: SearchKind::Collection,
        label: "Collections",
    },
    ProviderSpec {
        kind: SearchKind::Library,
        label: "Libraries",
    },
];

pub const DEFAULT_PER_GROUP: u32 = 5;
pub const MAX_PER_GROUP: u32 = 20;
pub const MAX_OMNI_QUERY: usize = 256;
pub const PROVIDER_TIMEOUT_MS: u64 = 1500;

// what to do for one provider, decided before anything runs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupPlan {
    Search(SearchQuery),
    NotApplicable { field: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmniPlan {
    pub per_group: u32,
    pub groups: Vec<(ProviderSpec, GroupPlan)>,
}

impl PartialEq for ProviderSpec {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for ProviderSpec {}

fn plan_for<C: SearchClass>(text: &str) -> Result<GroupPlan, String> {
    let parsed = SearchQuery::parse::<C>(text);

    if let Some(issue) = parsed
        .issues
        .iter()
        .find(|i| i.kind == IssueKind::UnknownField)
    {
        // the field name is the token's text before the colon
        let token = text.get(issue.span.clone()).unwrap_or("");
        let field = token.split(':').next().unwrap_or(token).to_lowercase();
        return Ok(GroupPlan::NotApplicable { field });
    }

    // other issues dropped a clause; the local page would show the same text with the same
    // issue line, so what runs here is what the page will run
    parsed.query.validate::<C>()?;
    Ok(GroupPlan::Search(parsed.query))
}

impl OmniSearchReq {
    // check the request (Err is a 400 message) and decide what each provider does
    pub fn plan(&self) -> Result<OmniPlan, String> {
        if self.query.len() > MAX_OMNI_QUERY {
            return Err(format!(
                "omnisearch queries are limited to {MAX_OMNI_QUERY} bytes"
            ));
        }

        let per_group = self
            .per_group
            .unwrap_or(DEFAULT_PER_GROUP)
            .clamp(1, MAX_PER_GROUP);

        let mut groups = Vec::new();
        for p in PROVIDERS {
            if !self.kinds.is_empty() && !self.kinds.contains(&p.kind) {
                continue;
            }
            let plan = match p.kind {
                SearchKind::Media => plan_for::<Media>(&self.query)?,
                SearchKind::Collection => plan_for::<Collection>(&self.query)?,
                SearchKind::Library => plan_for::<Library>(&self.query)?,
            };
            groups.push((*p, plan));
        }

        Ok(OmniPlan { per_group, groups })
    }
}

// the file name of a path, which is a media hit's title
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// "2021-06-01 · a note", leaving out whichever is empty
pub fn media_subtitle(date: &str, note: &str) -> String {
    match (date.trim(), note.trim()) {
        ("", n) => n.to_string(),
        (d, "") => d.to_string(),
        (d, n) => format!("{d} \u{b7} {n}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{Clause, Op};

    fn req(query: &str) -> OmniSearchReq {
        OmniSearchReq {
            query: query.into(),
            kinds: vec![],
            per_group: None,
        }
    }

    fn kinds(plan: &OmniPlan) -> Vec<SearchKind> {
        plan.groups.iter().map(|(p, _)| p.kind).collect()
    }

    #[test]
    fn every_provider_in_order_with_the_text_parsed_per_class() {
        let plan = req("beach").plan().unwrap();
        assert_eq!(plan.per_group, DEFAULT_PER_GROUP);
        assert_eq!(
            kinds(&plan),
            [
                SearchKind::Media,
                SearchKind::Collection,
                SearchKind::Library
            ]
        );
        for (_, g) in &plan.groups {
            assert_eq!(
                g,
                &GroupPlan::Search(SearchQuery {
                    text: vec!["beach".into()],
                    clauses: vec![]
                })
            );
        }
    }

    #[test]
    fn a_field_a_class_lacks_leaves_that_class_out() {
        let plan = req("tag:beach sunset").plan().unwrap();
        let by_kind: Vec<_> = plan
            .groups
            .iter()
            .map(|(p, g)| (p.kind, g.clone()))
            .collect();
        let tag = Clause {
            field: "tag".into(),
            op: Op::Has,
            value: "beach".into(),
        };
        assert_eq!(
            by_kind[0],
            (
                SearchKind::Media,
                GroupPlan::Search(SearchQuery {
                    text: vec!["sunset".into()],
                    clauses: vec![tag.clone()]
                })
            )
        );
        assert_eq!(
            by_kind[1],
            (
                SearchKind::Collection,
                GroupPlan::Search(SearchQuery {
                    text: vec!["sunset".into()],
                    clauses: vec![tag]
                })
            )
        );
        assert_eq!(
            by_kind[2],
            (
                SearchKind::Library,
                GroupPlan::NotApplicable {
                    field: "tag".into()
                }
            )
        );

        // and a field only libraries have
        let plan = req("PATH:photos").plan().unwrap();
        let na: Vec<_> = plan
            .groups
            .iter()
            .filter(|(_, g)| matches!(g, GroupPlan::NotApplicable { .. }))
            .map(|(p, _)| p.kind)
            .collect();
        assert_eq!(na, [SearchKind::Media, SearchKind::Collection]);
    }

    #[test]
    fn kinds_and_per_group_are_honoured_and_clamped() {
        let mut r = req("x");
        r.kinds = vec![SearchKind::Library, SearchKind::Media];
        r.per_group = Some(1000);
        let plan = r.plan().unwrap();
        // provider order, whatever order the request lists them in
        assert_eq!(kinds(&plan), [SearchKind::Media, SearchKind::Library]);
        assert_eq!(plan.per_group, MAX_PER_GROUP);
        r.per_group = Some(0);
        assert_eq!(r.plan().unwrap().per_group, 1);
    }

    #[test]
    fn bad_requests_are_rejected() {
        assert!(req(&"x".repeat(MAX_OMNI_QUERY + 1)).plan().is_err());
        // a valid-looking clause the class can't run is a 400, like on the local pages
        let many = (0..=crate::query::MAX_ANY_VALUES)
            .map(|i| format!("t{i}"))
            .collect::<Vec<_>>()
            .join(",");
        // the parser drops an over-long any-of list as an issue, so it never reaches
        // validation: the plan still searches, without the clause
        let plan = req(&format!("tag:{many}")).plan().unwrap();
        assert!(matches!(plan.groups[0].1, GroupPlan::Search(ref q) if q.clauses.is_empty()));
        // an empty query lists the first records of every class
        let plan = req("").plan().unwrap();
        assert!(
            plan.groups
                .iter()
                .all(|(_, g)| g == &GroupPlan::Search(SearchQuery::default()))
        );
    }

    #[test]
    fn hit_text_helpers() {
        assert_eq!(file_name("/photos/2021/IMG_1.jpg"), "IMG_1.jpg");
        assert_eq!(file_name("plain.jpg"), "plain.jpg");
        assert_eq!(
            media_subtitle("2021-06-01", "beach"),
            "2021-06-01 \u{b7} beach"
        );
        assert_eq!(media_subtitle("2021-06-01", " "), "2021-06-01");
        assert_eq!(media_subtitle("", "beach"), "beach");
    }

    #[test]
    fn wire_format() {
        let r: OmniSearchReq = serde_json::from_str(r#"{"query":"beach"}"#).unwrap();
        assert!(r.kinds.is_empty() && r.per_group.is_none());
        let g = HitGroup {
            kind: SearchKind::Library,
            label: "Libraries".into(),
            status: GroupStatus::NotApplicable {
                field: "tag".into(),
            },
            hits: vec![],
            meta: PageMeta::default(),
        };
        let back: HitGroup = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(back.status, g.status);
        assert_eq!(
            serde_json::to_string(&GroupStatus::TimedOut).unwrap(),
            r#""TimedOut""#
        );
    }
}
