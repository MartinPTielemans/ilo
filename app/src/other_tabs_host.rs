//! The tabs open on the account's other computers as the core tells them (#1234), which the
//! command bar lists for what's typed.

use super::*;

thread_local! {
    /// The profile that syncs and its other computers' tabs, as last told.
    pub(crate) static OTHER_TABS: RefCell<(bc::bc_id, Rc<Vec<command_bar::OtherTab>>)> =
        RefCell::default();
}

/// Kept for the bar to read as it's next typed in, so nothing runs for it until then.
pub(crate) unsafe extern "C" fn other_tabs_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    tabs: *const bc::bc_other_tab,
    count: usize,
) {
    // check.sh reads this line: how many, as their pages are the user's.
    log!("browser: other computers' tabs {count}");
    let tabs = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(tabs, count) },
    };
    let tabs = (tabs.iter())
        .map(|tab| command_bar::OtherTab {
            url: core_text(tab.url),
            title: core_text(tab.title),
            computer: core_text(tab.computer),
        })
        .collect();
    OTHER_TABS.set((profile, Rc::new(tabs)));
}

/// The other computers' tabs for the bar over `window`, if it shows a Space of the profile that
/// syncs.
pub(crate) fn other_tabs_over(window: bc::bc_id) -> Rc<Vec<command_bar::OtherTab>> {
    OTHER_TABS.with_borrow(|(profile, tabs)| {
        let shown = unsafe { bc::bc_space_profile(bc::bc_window_space(window)) };
        match *profile != 0 && shown == *profile {
            true => tabs.clone(),
            false => Rc::default(),
        }
    })
}
