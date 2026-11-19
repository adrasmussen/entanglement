use serde::{Deserialize, Serialize};

use crate::{
    collection::Collection,
    http_endpoint,
    library::Library,
    media::Media,
    query::{DataType, FieldSpec, FilterType, SearchKind, Values, check_filter_text, field_spec},
};

// slugs: the values a select or multiselect offers for one field
//
// a field declares where its choices come from (FilterType::Select / MultiSelect with a
// Values source).  Static values are answered from the metadata here; Distinct values are
// the distinct values in the user's own visible data, with how many records carry each, so
// the advanced search can offer "beach (120)" instead of making people guess tag spellings

http_endpoint!(GetSlugs);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetSlugsReq {
    pub kind: SearchKind,
    pub field: String,
    // case-insensitive prefix the values must start with; empty for all
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Slug {
    pub value: String,
    // visible records carrying the value; None for declared (static) values
    pub count: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetSlugsResp {
    pub slugs: Vec<Slug>,
}

pub const SLUG_LIMIT: u32 = 50;
pub const MAX_SLUG_LIMIT: u32 = 200;
pub const MAX_SLUG_PREFIX: usize = 64;

// what answering a request takes, once it is validated
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlugSource {
    // already answered from the metadata
    Static(Vec<Slug>),
    // the distinct tags of the visible records of this kind
    Tags {
        kind: SearchKind,
        prefix: String,
        limit: u32,
    },
}

pub fn field_for(kind: SearchKind, field: &str) -> Option<&'static FieldSpec> {
    match kind {
        SearchKind::Media => field_spec::<Media>(field),
        SearchKind::Collection => field_spec::<Collection>(field),
        SearchKind::Library => field_spec::<Library>(field),
    }
}

impl GetSlugsReq {
    // check the request against the field metadata (Err is a 400 message) and work out where
    // the answer comes from
    pub fn source(&self) -> Result<SlugSource, String> {
        let spec = field_for(self.kind, &self.field)
            .ok_or_else(|| format!("unknown {:?} field '{}'", self.kind, self.field))?;

        if self.prefix.len() > MAX_SLUG_PREFIX {
            return Err(format!(
                "slug prefixes are limited to {MAX_SLUG_PREFIX} bytes"
            ));
        }
        check_filter_text(&self.prefix)?;

        let limit = self.limit.unwrap_or(SLUG_LIMIT).clamp(1, MAX_SLUG_LIMIT);
        let prefix = self.prefix.trim().to_lowercase();

        match (spec.filter_type, spec.data_type) {
            (
                FilterType::Select(Values::Static(values))
                | FilterType::MultiSelect(Values::Static(values)),
                _,
            ) => Ok(SlugSource::Static(
                values
                    .iter()
                    .filter(|v| v.to_lowercase().starts_with(&prefix))
                    .take(limit as usize)
                    .map(|v| Slug {
                        value: v.to_string(),
                        count: None,
                    })
                    .collect(),
            )),
            (
                FilterType::Select(Values::Distinct) | FilterType::MultiSelect(Values::Distinct),
                DataType::Tag,
            ) => Ok(SlugSource::Tags {
                kind: self.kind,
                prefix,
                limit,
            }),
            _ => Err(format!(
                "{:?} field '{}' has no list of values",
                self.kind, spec.name
            )),
        }
    }
}

// the server's ordering, shared so both backends and the tests agree: most used first, then
// alphabetical
pub fn sort_slugs(slugs: &mut [Slug]) {
    slugs.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(kind: SearchKind, field: &str, prefix: &str, limit: Option<u32>) -> GetSlugsReq {
        GetSlugsReq {
            kind,
            field: field.into(),
            prefix: prefix.into(),
            limit,
        }
    }

    #[test]
    fn prefixes_are_checked_like_search_text() {
        assert!(
            req(SearchKind::Media, "tag", "be\0", None)
                .source()
                .unwrap_err()
                .contains("NUL")
        );
    }

    #[test]
    fn tags_come_from_the_data() {
        assert_eq!(
            req(SearchKind::Media, "tag", " Bea ", None).source(),
            Ok(SlugSource::Tags {
                kind: SearchKind::Media,
                prefix: "bea".into(),
                limit: SLUG_LIMIT,
            })
        );
        assert_eq!(
            req(SearchKind::Collection, "tag", "", Some(10_000)).source(),
            Ok(SlugSource::Tags {
                kind: SearchKind::Collection,
                prefix: String::new(),
                limit: MAX_SLUG_LIMIT,
            })
        );
    }

    #[test]
    fn static_values_are_answered_from_the_metadata() {
        let values =
            |prefix: &str, limit| match req(SearchKind::Media, "type", prefix, limit).source() {
                Ok(SlugSource::Static(s)) => s.into_iter().map(|s| s.value).collect::<Vec<_>>(),
                other => panic!("{other:?}"),
            };
        assert_eq!(values("", None), ["Image", "Video", "VideoSlice", "Audio"]);
        assert_eq!(values("vid", None), ["Video", "VideoSlice"]);
        assert_eq!(values("", Some(1)), ["Image"]);
        assert!(values("x", None).is_empty());
    }

    #[test]
    fn fields_without_values_and_bad_requests_are_rejected() {
        for (kind, field) in [
            (SearchKind::Media, "description"),
            (SearchKind::Media, "date"),
            (SearchKind::Media, "nope"),
            (SearchKind::Library, "tag"),
            (SearchKind::Library, "path"),
            (SearchKind::Collection, "name"),
        ] {
            assert!(
                req(kind, field, "", None).source().is_err(),
                "{kind:?} {field}"
            );
        }
        let long = "x".repeat(MAX_SLUG_PREFIX + 1);
        assert!(req(SearchKind::Media, "tag", &long, None).source().is_err());
    }

    #[test]
    fn wire_format_defaults() {
        let r: GetSlugsReq = serde_json::from_str(r#"{"kind":"Media","field":"tag"}"#).unwrap();
        assert_eq!(r.prefix, "");
        assert_eq!(r.limit, None);
        let s = serde_json::to_string(&Slug {
            value: "beach".into(),
            count: Some(3),
        })
        .unwrap();
        assert_eq!(s, r#"{"value":"beach","count":3}"#);
    }

    #[test]
    fn slugs_sort_by_count_then_value() {
        let mut s = vec![
            Slug {
                value: "b".into(),
                count: Some(1),
            },
            Slug {
                value: "a".into(),
                count: Some(1),
            },
            Slug {
                value: "c".into(),
                count: Some(5),
            },
        ];
        sort_slugs(&mut s);
        let v: Vec<_> = s.iter().map(|s| s.value.as_str()).collect();
        assert_eq!(v, ["c", "a", "b"]);
    }
}
