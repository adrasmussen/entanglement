use api::{
    omni::{GroupStatus, HitGroup, HitId, OmniSearchResp},
    query::SearchKind,
};

use crate::{Route, common::query_ui::q_param};

// the pure half of the ⌘K palette: turning an OmniSearch response into the rows the palette
// shows, where each row goes, and keyboard movement over them.  the component in
// components/palette.rs stays thin

#[derive(Clone, PartialEq)]
pub enum PaletteRow {
    // a group's heading, e.g. "Media · showing 5, 40 more match"; not selectable
    Heading {
        text: String,
    },
    // one hit: groups[group].hits[index]
    Hit {
        group: usize,
        index: usize,
        route: Route,
    },
    // the group's local page with the same search prefilled
    SeeAll {
        text: String,
        route: Route,
    },
    // why a group has nothing to show (not applicable, timed out, failed); not selectable
    Note {
        text: String,
    },
}

impl PaletteRow {
    pub fn selectable(&self) -> bool {
        matches!(self, PaletteRow::Hit { .. } | PaletteRow::SeeAll { .. })
    }

    pub fn route(&self) -> Option<Route> {
        match self {
            PaletteRow::Hit { route, .. } | PaletteRow::SeeAll { route, .. } => Some(route.clone()),
            _ => None,
        }
    }
}

pub fn kind_label(kind: SearchKind) -> &'static str {
    match kind {
        SearchKind::Media => "Media",
        SearchKind::Collection => "Collections",
        SearchKind::Library => "Libraries",
    }
}

// "Media · showing 5, 40 more match" when there is more than fits, "Media · 3" otherwise
pub fn group_heading(group: &HitGroup) -> String {
    let shown = group.hits.len() as u64;
    match group.meta.total {
        Some(total) if total > shown => format!(
            "{} \u{b7} showing {shown}, {} more match",
            group.label,
            total - shown
        ),
        Some(total) => format!("{} \u{b7} {total}", group.label),
        None => format!("{} \u{b7} {shown}", group.label),
    }
}

pub fn route_for(id: &HitId) -> Route {
    match id {
        HitId::Media(u) => Route::GalleryDetail {
            media_uuid: u.to_string(),
        },
        HitId::Collection(u) => Route::CollectionDetail {
            collection_uuid: u.to_string(),
            q: String::new(),
        },
        HitId::Library(u) => Route::LibraryDetail {
            library_uuid: u.to_string(),
            q: String::new(),
        },
    }
}

// the class's local page with the palette's text as its ?q= search
pub fn see_all_route(kind: SearchKind, text: &str) -> Route {
    let q = q_param(text);
    match kind {
        SearchKind::Media => Route::GallerySearch { q },
        SearchKind::Collection => Route::CollectionSearch { q },
        SearchKind::Library => Route::LibrarySearch { q },
    }
}

pub fn not_applicable_text(kind: SearchKind, field: &str) -> String {
    format!("{} have no {field}: field", kind_label(kind))
}

// the rows for a response to `text`, in the server's group order.  a group with nothing to
// show and nothing to say is left out
pub fn rows(resp: &OmniSearchResp, text: &str) -> Vec<PaletteRow> {
    let mut out = Vec::new();

    for (g, group) in resp.groups.iter().enumerate() {
        match &group.status {
            GroupStatus::Ok => {
                if group.hits.is_empty() {
                    continue;
                }
                out.push(PaletteRow::Heading {
                    text: group_heading(group),
                });
                for (i, hit) in group.hits.iter().enumerate() {
                    out.push(PaletteRow::Hit {
                        group: g,
                        index: i,
                        route: route_for(&hit.id),
                    });
                }
                let total = group.meta.total.unwrap_or(group.hits.len() as u64);
                out.push(PaletteRow::SeeAll {
                    text: format!("See all {total} in {}", kind_label(group.kind)),
                    route: see_all_route(group.kind, text),
                });
            }
            GroupStatus::NotApplicable { field } => out.push(PaletteRow::Note {
                text: not_applicable_text(group.kind, field),
            }),
            GroupStatus::TimedOut => out.push(PaletteRow::Note {
                text: format!("{} took too long to search", kind_label(group.kind)),
            }),
            GroupStatus::Error(_) => out.push(PaletteRow::Note {
                text: format!("{} couldn't be searched", kind_label(group.kind)),
            }),
        }
    }

    out
}

// the first selectable row, if any
pub fn first_active(rows: &[PaletteRow]) -> Option<usize> {
    rows.iter().position(PaletteRow::selectable)
}

// the active row after moving by delta (+1 down, -1 up), skipping rows that can't be selected
// and wrapping at the ends; None when nothing is selectable
pub fn move_active(rows: &[PaletteRow], active: Option<usize>, delta: i32) -> Option<usize> {
    let n = rows.len() as i32;
    if n == 0 || !rows.iter().any(PaletteRow::selectable) {
        return None;
    }
    let mut i = match active {
        Some(a) => a as i32,
        // nothing active: down starts at the top, up at the bottom
        None if delta > 0 => -1,
        None => n,
    };
    loop {
        i = (i + delta).rem_euclid(n);
        if rows[i as usize].selectable() {
            return Some(i as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api::{
        collection::CollectionUuid, library::LibraryUuid, media::MediaUuid, omni::SearchHit,
        page::PageMeta,
    };
    use std::str::FromStr;

    fn uuid(n: u8) -> String {
        format!("00000000-0000-0000-0000-0000000000{n:02}")
    }

    // the uuid newtypes only come from a UuidSource or the wire, so tests take the wire
    fn id<T: serde::de::DeserializeOwned>(n: u8) -> T {
        serde_json::from_str(&format!("\"{}\"", uuid(n))).unwrap()
    }

    fn hit(id: HitId) -> SearchHit {
        SearchHit {
            id,
            title: "t".into(),
            subtitle: String::new(),
            thumbnail: None,
        }
    }

    fn group(
        kind: SearchKind,
        label: &str,
        status: GroupStatus,
        hits: Vec<SearchHit>,
        total: u64,
    ) -> HitGroup {
        HitGroup {
            kind,
            label: label.into(),
            status,
            meta: PageMeta {
                total: Some(total),
                row_count: hits.len() as u64,
                ..Default::default()
            },
            hits,
        }
    }

    fn media(n: u8) -> HitId {
        HitId::Media(id::<MediaUuid>(n))
    }

    fn resp() -> OmniSearchResp {
        OmniSearchResp {
            groups: vec![
                group(
                    SearchKind::Media,
                    "Media",
                    GroupStatus::Ok,
                    vec![hit(media(1)), hit(media(2))],
                    42,
                ),
                // nothing found: left out entirely
                group(
                    SearchKind::Collection,
                    "Collections",
                    GroupStatus::Ok,
                    vec![],
                    0,
                ),
                group(
                    SearchKind::Library,
                    "Libraries",
                    GroupStatus::NotApplicable {
                        field: "tag".into(),
                    },
                    vec![],
                    0,
                ),
            ],
        }
    }

    fn shape(rows: &[PaletteRow]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                PaletteRow::Heading { text } => format!("H {text}"),
                PaletteRow::Hit {
                    group,
                    index,
                    route,
                } => format!("hit {group}.{index} {route}"),
                PaletteRow::SeeAll { text, route } => format!("all {text} {route}"),
                PaletteRow::Note { text } => format!("note {text}"),
            })
            .collect()
    }

    #[test]
    fn rows_follow_the_groups() {
        let r = rows(&resp(), "tag:beach sunset");
        assert_eq!(
            shape(&r),
            [
                "H Media \u{b7} showing 2, 40 more match".to_string(),
                format!("hit 0.0 /gallery/{}", uuid(1)),
                format!("hit 0.1 /gallery/{}", uuid(2)),
                format!(
                    "all See all 42 in Media {}",
                    see_all_route(SearchKind::Media, "tag:beach sunset")
                ),
                "note Libraries have no tag: field".to_string(),
            ]
        );
    }

    #[test]
    fn failures_are_notes() {
        let r = rows(
            &OmniSearchResp {
                groups: vec![
                    group(SearchKind::Media, "Media", GroupStatus::TimedOut, vec![], 0),
                    group(
                        SearchKind::Collection,
                        "Collections",
                        GroupStatus::Error("x".into()),
                        vec![],
                        0,
                    ),
                ],
            },
            "x",
        );
        assert_eq!(
            shape(&r),
            [
                "note Media took too long to search",
                "note Collections couldn't be searched"
            ]
        );
        assert_eq!(first_active(&r), None);
        assert_eq!(move_active(&r, None, 1), None);
    }

    #[test]
    fn headings_say_how_many_more() {
        let g = |shown: u8, total| {
            group(
                SearchKind::Collection,
                "Collections",
                GroupStatus::Ok,
                (0..shown)
                    .map(|n| hit(HitId::Collection(id::<CollectionUuid>(n))))
                    .collect(),
                total,
            )
        };
        assert_eq!(
            group_heading(&g(5, 45)),
            "Collections \u{b7} showing 5, 40 more match"
        );
        assert_eq!(group_heading(&g(3, 3)), "Collections \u{b7} 3");
    }

    #[test]
    fn routes_open_items_and_prefill_pages() {
        assert_eq!(
            route_for(&HitId::Collection(id::<CollectionUuid>(7))).to_string(),
            format!("/collections/{}?q=", uuid(7))
        );
        assert_eq!(
            route_for(&HitId::Library(id::<LibraryUuid>(8))).to_string(),
            format!("/library/{}?q=", uuid(8))
        );
        // see-all carries the text through ?q= and parses back to the same route
        for (kind, text) in [
            (SearchKind::Media, "tag:ct-mod7 sunset"),
            (SearchKind::Collection, "a b&c 50%"),
            (SearchKind::Library, "path:photos"),
        ] {
            let route = see_all_route(kind, text);
            let back = Route::from_str(&route.to_string()).ok().unwrap();
            assert!(back == route, "{route}");
        }
        assert_eq!(
            see_all_route(SearchKind::Media, "tag:ct-mod7 sunset").to_string(),
            "/gallery/?q=tag%3Act-mod7%20sunset"
        );
    }

    #[test]
    fn arrows_skip_headings_and_notes_and_wrap() {
        let r = rows(&resp(), "x");
        // rows: 0 heading, 1 hit, 2 hit, 3 see all, 4 note
        assert_eq!(first_active(&r), Some(1));
        assert_eq!(move_active(&r, None, 1), Some(1));
        assert_eq!(move_active(&r, None, -1), Some(3));
        assert_eq!(move_active(&r, Some(1), 1), Some(2));
        assert_eq!(move_active(&r, Some(3), 1), Some(1));
        assert_eq!(move_active(&r, Some(1), -1), Some(3));
        assert_eq!(move_active(&[], None, 1), None);
    }
}
