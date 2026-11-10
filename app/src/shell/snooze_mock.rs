//! Mock only (#2350, not for merge): snoozing a Today tab, with PREVIEW_SNOOZE set. Its times are
//! read from a fixed clock, Tuesday 14:25, so the captures say the same whenever they're taken.

use std::{collections::HashMap, sync::Mutex};

use gpui::{IntoElement, div, prelude::*, px};

use super::{
    LOADING_SIZE, STATUS_DOT, Shell,
    menus::{MenuAction, MenuEntry},
};

/// The short list: each time's name and when it means at 14:25 on a Tuesday, as its row ends
/// with: 3 hours on, 18:00, 08:00 the next day and 08:00 on Monday (orchestrator's calls).
pub const TIMES: [(&str, &str); 4] = [
    ("Later Today", "17:25"),
    ("This Evening", "18:00"),
    ("Tomorrow Morning", "Wed 08:00"),
    ("Next Week", "Mon 08:00"),
];

static SNOOZED: Mutex<Option<HashMap<u64, &'static str>>> = Mutex::new(None);
static UNREAD: Mutex<Option<u64>> = Mutex::new(None);

/// Whether the preview mocks snoozing.
pub fn on() -> bool {
    std::env::var_os("PREVIEW_SNOOZE").is_some()
}

/// Marks archived tab `id` snoozed until `until`, such as "Wed 08:00".
pub fn snooze(id: u64, until: &'static str) {
    let mut snoozed = SNOOZED.lock().unwrap();
    snoozed.get_or_insert_with(HashMap::new).insert(id, until);
}

/// When archived tab `id` comes back, while it's snoozed.
pub fn until(id: u64) -> Option<&'static str> {
    SNOOZED.lock().unwrap().as_ref()?.get(&id).copied()
}

/// Whether any tab is snoozed, which shows History's Snoozed filter.
pub fn any() -> bool {
    SNOOZED.lock().unwrap().as_ref().is_some_and(|map| !map.is_empty())
}

/// Marks `tab` come back from a snooze and not yet shown.
pub fn back(tab: u64) {
    *UNREAD.lock().unwrap() = Some(tab);
}

/// The short list's entries under the item at `parent`: each time with when it means at its
/// end, then Pick a Date and Time….
pub(super) fn times_under(parent: usize) -> Vec<(MenuEntry, &'static str, MenuAction)> {
    let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = Vec::new();
    for (name, when) in TIMES {
        let mut entry = MenuEntry::new(name, true).under(parent);
        // The shortcut's place, which shows a `=` shortcut's words as they are.
        entry.shortcut = Some(format!("={when}"));
        items.push((entry, "snooze", Box::new(|_, _| {})));
    }
    items.push((MenuEntry::line().under(parent), "", Box::new(|_, _| {})));
    items.push((
        MenuEntry::new("Pick a Date and Time…", true).under(parent),
        "snooze-pick",
        Box::new(|_, _| {}),
    ));
    items
}

impl Shell {
    /// A tab back from a snooze and not yet shown: a steady dot in the Space's accent where the
    /// loading dot goes, which goes once the tab is shown.
    pub(super) fn unread_dot(&self, tab: u64) -> Option<impl IntoElement + use<>> {
        if *UNREAD.lock().unwrap() != Some(tab) || self.page(tab).loading {
            return None;
        }
        Some(
            div()
                .flex_none()
                .ml(px(4.))
                .size(px(LOADING_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(STATUS_DOT))
                        .rounded_full()
                        .bg(self.palette.accent),
                ),
        )
    }
}
