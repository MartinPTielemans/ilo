//! The menu bar and its menus.

pub(crate) use browser::menu_bar_items::{
    CheckForUpdates, Hide, HideOthers, Minimize, ShowAll, menus,
};

use super::*;

/// What the menu bar lists besides what never changes: what the key window's shell says of it, and
/// the titles Undo and Redo have for what the key window would undo and redo.
#[derive(Default)]
pub(crate) struct Menus {
    pub(crate) bar: shell::MenuBar,
    pub(crate) undo: Option<String>,
    pub(crate) redo: Option<String>,
    /// Check for Updates… reads Restart to Update once an update is ready.
    pub(crate) update_ready: bool,
}

impl Menus {
    pub(crate) fn build(&self) -> Vec<Menu> {
        menus(
            &self.bar,
            self.undo.as_deref(),
            self.redo.as_deref(),
            self.update_ready,
            updates_available(),
        )
    }
}

/// Puts up the menu bar built from MENUS, or, while a menu is open, once none is, as AppKit can
/// crash when the main menu is replaced under an open menu. What changes meanwhile goes in the one
/// rebuild.
pub(crate) fn show_menus(cx: &mut App) {
    if MENUS_HELD.get() {
        return;
    }
    if unsafe { platform::host_after_menus(menus_closed) } {
        // check.sh reads these lines.
        log!("browser: menu bar waits for the menu open");
        MENUS_HELD.set(true);
        return;
    }
    cx.set_menus(MENUS.with_borrow(Menus::build));
}

pub(crate) extern "C" fn menus_closed() {
    log!("browser: menu bar put up once the menus closed");
    MENUS_HELD.set(false);
    update_app(show_menus);
}

/// View › Appearance's choice as the core has it (bc_appearance).
pub(crate) fn appearance_code(appearance: Appearance) -> i32 {
    match appearance {
        Appearance::Automatic => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    }
}

/// Gives the host View › Appearance's choice, for the app's native parts, and the core, which keeps
/// it and gives it to pages.
pub(crate) fn give_host_appearance(appearance: Appearance) {
    let code = appearance_code(appearance);
    unsafe { platform::host_set_app_appearance(code) };
    unsafe { bc::bc_set_appearance(code) };
}
