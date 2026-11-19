use serde::{Deserialize, Serialize};

// shared paging types
//
// PageRequest is flattened into every paged request, so on the wire the keys are still
// the top-level `limit`, `offset` and `with_total` (and `cursor`, only when set).
// PageMeta is attached to every paged response as `meta`, next to the legacy `total`
// and `page_len` fields, which are still filled in until the webapp has migrated.

// opaque keyset cursor; reserved for cursor paging and not yet produced or accepted
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct PageCursor(pub String);

// limit = None means "no limit"; limit = Some(0) skips the page query entirely, so
// together with with_total it is a count-only request.  with_total asks the backend to
// run an additional COUNT(*) over the filtered set (without ORDER/LIMIT)
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PageRequest {
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<PageCursor>,
    #[serde(default)]
    pub with_total: bool,
}

impl PageRequest {
    // no rows, just the total
    pub fn count_only() -> Self {
        Self {
            limit: Some(0),
            with_total: true,
            ..Default::default()
        }
    }

    // the first page of `limit` rows
    pub fn first(limit: u64, with_total: bool) -> Self {
        Self {
            limit: Some(limit),
            with_total,
            ..Default::default()
        }
    }
}

// metadata describing the page that was returned
//
// total: size of the full filtered set; only when the request set with_total
// row_count: rows in this page (before any hydration)
// offset/limit: what was actually used, i.e. after any server-side clamp
// has_more: whether rows exist beyond this page; None when unknown
// next_cursor: reserved for cursor paging
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PageMeta {
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub row_count: u64,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub has_more: Option<bool>,
    #[serde(default)]
    pub next_cursor: Option<PageCursor>,
}

// a uniform way to read any paged response
pub trait PagedResp {
    type Item;

    fn into_parts(self) -> (Vec<Self::Item>, PageMeta);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        let c = PageRequest::count_only();
        assert_eq!(c.limit, Some(0));
        assert!(c.with_total);
        assert_eq!(c.offset, 0);
        assert!(c.cursor.is_none());

        let f = PageRequest::first(60, false);
        assert_eq!(f.limit, Some(60));
        assert!(!f.with_total);
    }

    #[test]
    fn request_defaults_and_skips_cursor() {
        let r: PageRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(r, PageRequest::default());

        let v = serde_json::to_value(PageRequest::first(5, true)).unwrap();
        assert!(v.get("cursor").is_none());

        let r = PageRequest {
            cursor: Some(PageCursor("abc".into())),
            ..Default::default()
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["cursor"], "abc");
        assert_eq!(serde_json::from_value::<PageRequest>(v).unwrap(), r);
    }

    #[test]
    fn meta_defaults_when_fields_missing() {
        let m: PageMeta = serde_json::from_str("{}").unwrap();
        assert_eq!(m, PageMeta::default());
        let m: PageMeta = serde_json::from_str(r#"{"total":7,"row_count":2}"#).unwrap();
        assert_eq!(m.total, Some(7));
        assert_eq!(m.row_count, 2);
        assert_eq!(m.has_more, None);
    }
}
