//! Little ilo from the operating system's own places, beside the Mac's Dock menu and menu bar
//! (#693): the taskbar's jump list on Windows and the launcher's action on Linux, which run the
//! browser with a switch, and the notification area's icon on Windows, which Settings › Features
//! turns on in the row that turns on the Mac's menu-bar button.

/// What the jump list's task and the launcher's action are called, as the Dock's item is.
pub const NEW_LITTLE_WINDOW: &str = concat!("New Little ", env!("APP_NAME"), " Window");

/// The switch they run the browser with, which the core hands to the app (core/main_delegate.cc).
pub const NEW_LITTLE_SWITCH: &str = "--new-little-window";

/// Whether a launch's arguments ask for a new Little ilo window.
pub fn asks_for_little(arguments: impl IntoIterator<Item = String>) -> bool {
    arguments
        .into_iter()
        .any(|argument| argument == NEW_LITTLE_SWITCH)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

/// The system the browser runs on.
pub const fn os() -> Os {
    if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else {
        Os::Linux
    }
}

/// Where the button that opens a new Little ilo window goes, by the place's own name: the Mac's
/// menu bar, Windows' notification area. Linux has no place every desktop shows (GNOME has no
/// tray), so none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Button {
    /// Settings' row for it, and the line under it.
    pub row: &'static str,
    pub line: &'static str,
    /// What check.sh's line says opened a window from it.
    pub place: &'static str,
}

pub fn button(os: Os) -> Option<Button> {
    match os {
        Os::Mac => Some(Button {
            row: "Show in the menu bar",
            line: "A button there opens a new one.",
            place: "menu bar",
        }),
        Os::Windows => Some(Button {
            row: "Show in the notification area",
            line: "An icon there opens a new one.",
            place: "notification area",
        }),
        Os::Linux => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The jump list's task and the launcher's action say what the Dock's item says, and run the
    /// switch the core takes; only that switch asks.
    #[test]
    fn the_task_is_the_docks_item_and_runs_its_switch() {
        assert_eq!(
            NEW_LITTLE_WINDOW,
            concat!("New Little ", env!("APP_NAME"), " Window")
        );
        let launch = |arguments: &[&str]| asks_for_little(arguments.iter().map(|a| a.to_string()));
        assert!(launch(&["chrome.exe", "--new-little-window"]));
        assert!(!launch(&[
            "chrome.exe",
            "--single-argument",
            "https://example.com/"
        ]));
        assert!(!launch(&["chrome.exe", "--new-little-window=no"]));
    }

    /// The button's row is the Mac's on the Mac and Windows, in each one's words for its place,
    /// off at first in both; Linux has none.
    #[test]
    fn the_buttons_row_names_its_place() {
        assert_eq!(button(Os::Mac).map(|b| b.row), Some("Show in the menu bar"));
        let windows = button(Os::Windows).expect("Windows has a notification area");
        assert_eq!(windows.row, "Show in the notification area");
        assert_eq!(windows.line, "An icon there opens a new one.");
        assert_eq!(button(Os::Linux), None);
    }
}
