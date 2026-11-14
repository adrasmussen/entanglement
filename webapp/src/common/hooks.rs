use std::{cell::Cell, rc::Rc};

use dioxus::prelude::*;

// run `f` every time `trigger` is written, but not for the effect's first run, which is
// just the mount
pub fn use_on_write(trigger: Signal<()>, mut f: impl FnMut() + 'static) {
    let first_run = use_hook(|| Rc::new(Cell::new(true)));
    use_effect(move || {
        trigger();
        if first_run.replace(false) {
            return;
        }
        f();
    });
}
