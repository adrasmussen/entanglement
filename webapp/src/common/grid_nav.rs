// previous/next navigation on the media detail page, over the grid the user came from.
//
// clicking a card snapshots the ids the grid had loaded, in the order its cards render, so
// the arrows walk what the UI showed even if the server's contents change meanwhile.  the
// snapshot is only used while its `cursor` is the media being shown: the card click sets
// it to the clicked card and an arrow move advances it, so any other way of arriving at a
// detail page (deep link, similar media, a collection link) finds a mismatch and drops it.
//
// the pure parts are testable on the host; only GRID_NAV is a signal.

use dioxus::prelude::*;

use crate::Route;
use api::media::MediaUuid;

#[derive(Clone, Debug, PartialEq)]
pub struct GridNav {
    // the page to return to, and what to call it
    pub source: Route,
    pub label: String,
    pub ids: Vec<MediaUuid>,
    // the media this snapshot was last pointed at
    pub cursor: MediaUuid,
}

pub static GRID_NAV: GlobalSignal<Option<GridNav>> = Signal::global(|| None);

// called by a card click, before navigating
pub fn set_grid_nav(source: Route, label: String, ids: Vec<MediaUuid>, clicked: MediaUuid) {
    *GRID_NAV.write() = Some(GridNav {
        source,
        label,
        ids,
        cursor: clicked,
    });
}

// the neighbours of `current` in `ids`; (None, None) when it isn't there, and None at the edges
pub fn neighbours<T: PartialEq + Copy>(ids: &[T], current: &T) -> (Option<T>, Option<T>) {
    let Some(i) = ids.iter().position(|id| id == current) else {
        return (None, None);
    };
    let prev = i.checked_sub(1).map(|p| ids[p]);
    let next = ids.get(i + 1).copied();
    (prev, next)
}

// what the detail page shows for `current`
#[derive(Clone, Debug, PartialEq)]
pub struct NavView {
    pub source: Route,
    pub label: String,
    pub prev: Option<MediaUuid>,
    pub next: Option<MediaUuid>,
    // 1-based position and the number of ids
    pub position: usize,
    pub total: usize,
}

// None when there is no usable snapshot: none at all, one made for another media, or one
// that doesn't contain `current`
pub fn view_for(nav: &Option<GridNav>, current: &MediaUuid) -> Option<NavView> {
    let nav = nav.as_ref()?;
    if nav.cursor != *current {
        return None;
    }
    let position = nav.ids.iter().position(|id| id == current)?;
    let (prev, next) = neighbours(&nav.ids, current);
    Some(NavView {
        source: nav.source.clone(),
        label: nav.label.clone(),
        prev,
        next,
        position: position + 1,
        total: nav.ids.len(),
    })
}

// an arrow move: the cursor follows so the snapshot stays valid for the new media
pub fn advance_cursor(to: MediaUuid) {
    if let Some(nav) = GRID_NAV.write().as_mut() {
        nav.cursor = to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_has_both_neighbours() {
        assert_eq!(neighbours(&[1, 2, 3], &2), (Some(1), Some(3)));
    }

    #[test]
    fn edges_have_none() {
        assert_eq!(neighbours(&[1, 2, 3], &1), (None, Some(2)));
        assert_eq!(neighbours(&[1, 2, 3], &3), (Some(2), None));
    }

    #[test]
    fn missing_or_empty_has_nothing() {
        assert_eq!(neighbours(&[1, 2, 3], &9), (None, None));
        assert_eq!(neighbours::<u32>(&[], &1), (None, None));
    }

    #[test]
    fn single_item_has_nothing() {
        assert_eq!(neighbours(&[7], &7), (None, None));
    }

    // MediaUuid can only be made through a UuidSource
    struct Src;
    impl api::UuidSource for Src {}
    fn id(n: u128) -> MediaUuid {
        MediaUuid::try_parse(&Src, &format!("00000000-0000-0000-0000-{n:012}")).unwrap()
    }

    fn nav(cursor: MediaUuid) -> Option<GridNav> {
        Some(GridNav {
            source: Route::GallerySearch {},
            label: "Gallery".into(),
            ids: vec![id(1), id(2), id(3)],
            cursor,
        })
    }

    #[test]
    fn view_for_the_cursor() {
        let v = view_for(&nav(id(2)), &id(2)).unwrap();
        assert_eq!(
            (v.prev, v.next, v.position, v.total),
            (Some(id(1)), Some(id(3)), 2, 3)
        );
    }

    #[test]
    fn view_for_a_stale_snapshot_is_none() {
        // arrived some other way: the cursor is elsewhere, even though the id is in the list
        assert_eq!(view_for(&nav(id(1)), &id(2)), None);
        assert_eq!(view_for(&None, &id(2)), None);
    }

    #[test]
    fn view_for_a_cursor_outside_the_ids_is_none() {
        assert_eq!(view_for(&nav(id(9)), &id(9)), None);
    }
}
