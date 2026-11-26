//! Settings › Shortcuts' rows: each command the user can give keys, by its name in
//! `Command::names`, and the keys the app binds itself.

use crate::shell::{Command, keystroke_label};
use crate::theme::Appearance;

/// The keys a row of the Shortcuts tab shows: a command's shortcut, or keys the app binds itself.
/// Or the switch for whether those keys count the favorites, or whether Last Tab is the ninth.
pub enum Keys {
    Of(Command),
    /// Keys no command holds, which can't be changed: a binding's keystroke.
    Fixed(&'static str),
    /// A run of them, from the first keystroke to the last.
    Span(&'static str, &'static str),
    /// The click that opens a link, a tab or + New Tab in a split view, which no command holds.
    SplitClick,
    FavoritesNumbered,
    LastTabNinth,
}

impl Keys {
    /// A row's keys that can't be changed, as a menu shows them, Ctrl+1 – Ctrl+8 off the Mac.
    pub fn fixed(&self) -> Option<String> {
        match *self {
            Keys::Of(_) | Keys::FavoritesNumbered | Keys::LastTabNinth => None,
            Keys::Fixed(keys) => Some(keystroke_label(keys)),
            Keys::SplitClick => Some(split_click().into()),
            Keys::Span(first, last) => Some(format!(
                "{} – {}",
                keystroke_label(first),
                keystroke_label(last)
            )),
        }
    }
}

/// The click that splits: ⌥-click, and on Linux Shift+Alt-click too, since Openbox and Xfce take
/// every Alt-click for themselves but leave one with Shift (#2755).
fn split_click() -> &'static str {
    match (cfg!(target_os = "macos"), cfg!(target_os = "linux")) {
        (true, _) => "⌥-click",
        (_, true) => "Alt-click or Shift+Alt-click",
        _ => "Alt-click",
    }
}

/// A Shortcuts row for `command`, by its name with no "…", as the tab lists a command it doesn't
/// open.
const fn of(command: Command) -> (&'static str, Keys) {
    let name = command.title();
    let name = match name.as_bytes() {
        [.., 0xE2, 0x80, 0xA6] => name.split_at(name.len() - "…".len()).0,
        _ => name,
    };
    (name, Keys::Of(command))
}

/// The Shortcuts tab, every command the user can give keys, in the menus' order and names.
pub const SHORTCUTS: &[(&str, &[(&str, Keys)])] = &[
    (
        "Tabs",
        &[
            of(Command::NewTab),
            of(Command::NewWindow),
            of(Command::NewLittleWindow),
            of(Command::OpenInMainWindow),
            of(Command::NewPrivateWindow),
            of(Command::NewNote),
            of(Command::NewBoard),
            of(Command::EditUrl),
            of(Command::SearchTabs),
            of(Command::CloseTab),
            of(Command::CloseWindow),
            of(Command::ReopenClosedTab),
            of(Command::DuplicateTab),
            of(Command::NextTab),
            of(Command::PreviousTab),
            of(Command::RecentTab),
            ("Tabs 1 to 8", Keys::Span("cmd-1", "cmd-8")),
            ("Include Favorites in ⌘1–⌘8", Keys::FavoritesNumbered),
            of(Command::SelectLastTab),
            ("Last Tab Goes to the Ninth Tab", Keys::LastTabNinth),
            of(Command::TogglePin),
            of(Command::FavoriteTab),
            of(Command::BackToPinnedUrl),
            of(Command::RevealTab),
            of(Command::FocusSidebar),
            of(Command::FocusAgentQuestion),
            of(Command::FocusMenu),
            of(Command::Clear),
            of(Command::SortBySite),
            of(Command::SortIntoFolders),
            of(Command::ResetSpaceTabs),
            of(Command::CopyUrl),
            of(Command::CopyMarkdownLink),
            of(Command::PasteUrlAsTab),
            of(Command::ToggleMute),
            of(Command::RenameTab),
            of(Command::NewFolder),
            of(Command::CollapseFolders),
            of(Command::ExpandFolders),
        ],
    ),
    (
        "Page",
        &[
            of(Command::Back),
            of(Command::Forward),
            of(Command::Reload),
            of(Command::ForceReload),
            of(Command::Stop),
            of(Command::Find),
            of(Command::FindNext),
            of(Command::FindPrevious),
            of(Command::JumpToSelection),
            of(Command::ZoomIn),
            of(Command::ZoomOut),
            of(Command::ActualSize),
            of(Command::Share),
            of(Command::CapturePortion),
            of(Command::CaptureFullPage),
            of(Command::CapturePortrait),
            of(Command::SavePage),
            of(Command::Print),
            of(Command::ClearCacheAndRefresh),
            of(Command::ClearCookiesAndRefresh),
            of(Command::SiteControls),
        ],
    ),
    (
        "Spaces and Sidebar",
        &[
            of(Command::NextSpace),
            of(Command::PreviousSpace),
            of(Command::SelectSpace(0)),
            of(Command::SelectSpace(1)),
            of(Command::SelectSpace(2)),
            of(Command::SelectSpace(3)),
            of(Command::SelectSpace(4)),
            of(Command::SelectSpace(5)),
            of(Command::SelectSpace(6)),
            of(Command::SelectSpace(7)),
            of(Command::SelectSpace(8)),
            of(Command::NewSpace),
            of(Command::RenameSpace),
            of(Command::ExportSpace),
            of(Command::ToggleSidebar),
            of(Command::CollapsePinned),
            of(Command::ExpandPinned),
            of(Command::Appearance(Appearance::Automatic)),
            of(Command::Appearance(Appearance::Light)),
            of(Command::Appearance(Appearance::Dark)),
            of(Command::ViewLibrary),
            of(Command::ViewArchive),
            of(Command::ClearArchive),
            of(Command::ViewDownloads),
            of(Command::ViewMedia),
            of(Command::ViewBoards),
            of(Command::ViewHistory),
            of(Command::ClearBrowsingData),
        ],
    ),
    (
        "Split View",
        &[
            of(Command::AddSplit),
            ("Open a Link or Tab in Split View", Keys::SplitClick),
            of(Command::NewNoteSplit),
            of(Command::CloseSplitPane),
            of(Command::NextPane),
            of(Command::PreviousPane),
            of(Command::FocusPane(0)),
            of(Command::FocusPane(1)),
            of(Command::FocusPane(2)),
            of(Command::FocusPane(3)),
            of(Command::SeparatePane),
            of(Command::ExpandSplit),
            of(Command::SeparateSplit),
            of(Command::FlipSplit),
            of(Command::ChangeOrientation),
            of(Command::MovePaneLeft),
            of(Command::MovePaneRight),
        ],
    ),
    (
        "Developer",
        &[
            of(Command::ViewSource),
            of(Command::DevTools),
            of(Command::InspectElements),
            of(Command::JavaScriptConsole),
            of(Command::ToggleDeveloperMode),
            of(Command::TaskManager),
            of(Command::RecordTrace),
            of(Command::ManageExtensions),
            of(Command::AddExtension),
        ],
    ),
    (
        env!("PRODUCT_NAME"),
        &[
            of(Command::About),
            of(Command::CheckForUpdates),
            of(Command::Settings),
            of(Command::Import),
            of(Command::PrivacyPolicy),
            of(Command::Passwords),
            of(Command::SiteSearchSettings),
            // main.rs binds these.
            (concat!("Hide ", env!("PRODUCT_NAME")), Keys::Fixed("cmd-h")),
            ("Hide Others", Keys::Fixed("alt-cmd-h")),
            ("Minimize", Keys::Fixed("cmd-m")),
            of(Command::Quit),
        ],
    ),
    (
        "Help",
        &[
            of(Command::Welcome),
            of(Command::HelpCenter),
            of(Command::WhatsNew),
            of(Command::KeyboardShortcuts),
            of(Command::ReportProblem),
            of(Command::RestoreEarlierSidebar),
            of(Command::CopyBuildInfo),
        ],
    ),
];
