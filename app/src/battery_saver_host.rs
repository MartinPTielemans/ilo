//! Chrome's Battery Saver as the core tells it (#686): every window's shell keeps whether it's on,
//! for its leaf and its command, and the window in front says so as it turns on.

use super::*;

thread_local! {
    /// Battery Saver's percent while it's on, for the windows opened after it changed.
    pub(crate) static BATTERY_SAVER: Cell<Option<i32>> = const { Cell::new(None) };
}

// Each after the update under way: the core says it as it's asked, such as from Turn Off.
pub(crate) unsafe extern "C" fn battery_saver_changed(
    _: *mut c_void,
    active: bool,
    percent: c_int,
) {
    let on = active.then_some(percent);
    BATTERY_SAVER.set(on);
    update_app_later(move |cx| {
        update_shells_in(cx, |shell, cx| shell.set_battery_saver(on, cx));
        if !active {
            return;
        }
        let front = front().0;
        let shell = WINDOWS.with_borrow(|windows| {
            (windows.iter())
                .find(|window| window.id == front)
                .map(|window| window.shell.clone())
        });
        if let Some(shell) = shell {
            shell
                .update(cx, |shell, cx| shell.show_battery_notice(cx))
                .ok();
        }
    });
}

pub(crate) unsafe extern "C" fn tab_frozen(_: *mut c_void, tab: bc::bc_id, frozen: bool) {
    update_app_later(move |cx| {
        update_shells_in(cx, |shell, cx| shell.set_tab_frozen(tab, frozen, cx))
    });
}
