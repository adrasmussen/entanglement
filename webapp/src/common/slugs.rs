use gloo_timers::future::TimeoutFuture;

use api::{
    query::{Op, SearchKind, any_values},
    slug::{GetSlugsReq, Slug, get_slugs},
};

// field values for the search UI: fetching them, and the pure selection logic of the
// multiselect, which writes one any-of clause (`tag:a,b`)

// how long typing must settle before a values request goes out
pub const SLUG_DEBOUNCE_MS: u32 = 150;

// fetch the values of a field; callers run this inside a use_resource keyed on the field and
// prefix, so a newer key drops this future (debounce first, so dropped ones never send).
// a failure shows as no values: the box still takes typed text
pub async fn fetch_slugs(
    kind: SearchKind,
    field: &str,
    prefix: String,
    debounce: bool,
) -> Vec<Slug> {
    if debounce {
        TimeoutFuture::new(SLUG_DEBOUNCE_MS).await;
    }
    get_slugs(&GetSlugsReq {
        kind,
        field: field.to_string(),
        prefix,
        limit: None,
    })
    .await
    .map(|r| r.slugs)
    .unwrap_or_default()
}

// whether a value can be picked: the selection travels as `a,b` in unquoted query text, so a
// value with a comma, whitespace or a quote (possible in older data) can't be one of them
pub fn pickable(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(|c: char| c.is_whitespace() || matches!(c, ',' | '"' | '\\'))
}

// the selection, kept as the comma-joined value the clause will carry
pub fn selected(value: &str) -> Vec<&str> {
    any_values(value)
}

pub fn is_selected(value: &str, item: &str) -> bool {
    selected(value).contains(&item)
}

// the selection with one item toggled, in the order they were picked
pub fn toggle(value: &str, item: &str) -> String {
    let mut items = selected(value);
    match items.iter().position(|v| *v == item) {
        Some(i) => {
            items.remove(i);
        }
        None => items.push(item),
    }
    items.join(",")
}

// the operator a selection makes: one value is a plain Has, more are any-of
pub fn op_for(value: &str) -> Op {
    if selected(value).len() > 1 {
        Op::HasAny
    } else {
        Op::Has
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_keeps_pick_order() {
        let v = toggle("", "beach");
        let v = toggle(&v, "sunset");
        assert_eq!(v, "beach,sunset");
        assert!(is_selected(&v, "sunset") && !is_selected(&v, "sun"));
        assert_eq!(toggle(&v, "beach"), "sunset");
        assert_eq!(toggle("sunset", "sunset"), "");
    }

    #[test]
    fn values_that_cant_travel_unquoted_are_not_pickable() {
        assert!(pickable("summer_trip") && pickable("ct-mod7"));
        for v in ["", "a b", "a,b", "a\"b", "a\\b"] {
            assert!(!pickable(v), "{v:?}");
        }
    }

    #[test]
    fn one_value_is_has_and_more_are_any_of() {
        assert_eq!(op_for("beach"), Op::Has);
        assert_eq!(op_for("beach,sunset"), Op::HasAny);
        assert_eq!(op_for(""), Op::Has);
    }
}
