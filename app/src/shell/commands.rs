//! The browser's commands: their names, their keys and where the command bar offers
//! them, and what each does, run from a key, a menu, the command bar or the page.

use std::sync::{Arc, LazyLock};
use web_time::Instant;

use gpui::{
    Action, App, Bounds, Context, Global, Image, KeyBinding, MenuItem, Pixels, Point, SharedString,
    WeakEntity, actions, point,
};

use crate::build_info::{self, BuildInfo};
use crate::theme::Appearance;
use crate::usage::{self, Usage};

use super::{
    COPIED_TIME, Edit, Section, Shell, Space, Swapped, UserKeys, What, agent_bar, boost,
    browser::WindowButton,
    clean_link,
    drag::dropped_urls,
    drm_note, extensions, keys,
    little::little_runs,
    panes::pane_frames,
    sidebar_keys,
    swap::{CHECK, LINK},
    trace, translate, user_keys,
};

/// A browser command, from a key or the sidebar.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Command {
    NewTab,
    /// Opens the command bar on the open tabs alone, the Space shown's first, to switch to one.
    SearchTabs,
    /// Opens another window on the same Spaces.
    NewWindow,
    /// Opens a private window, in the profile's off-the-record session.
    NewPrivateWindow,
    /// Opens a Little ilo window, Arc's Little Arc, for the user's Space shown.
    NewLittleWindow,
    /// Opens a Blank Window, Arc's: one list of tabs that aren't kept or shown in other windows.
    NewBlankWindow,
    /// A Little ilo window's ⌘O: its page goes to the main window, live.
    OpenInMainWindow,
    /// Closes the window, its tabs going to the window in front of the others.
    CloseWindow,
    EditUrl,
    CloseTab,
    /// Reopens the Today tab closed last.
    ReopenClosedTab,
    NextTab,
    PreviousTab,
    /// The tab shown before this one, and on while ⌃ is held.
    RecentTab,
    /// The same walk, backwards.
    RecentTabBack,
    /// The tab at this index in sidebar order.
    SelectTab(usize),
    SelectLastTab,
    /// Pins the selected tab, or unpins it.
    TogglePin,
    /// Makes the selected tab a favorite, as its menu's Add to Favorites does.
    FavoriteTab,
    /// Copies the selected tab, with its history, under it, as its menu's Duplicate Tab does.
    DuplicateTab,
    /// Shows the selected tab's row with its name in a field, as its menu's Rename Tab does.
    RenameTab,
    /// Makes a folder at the bottom of the pinned tabs, and names it.
    NewFolder,
    /// Closes every folder of the Space shown.
    CollapseFolders,
    /// Hides the pinned rows of the Space shown but the tab shown's, as a closed folder does.
    CollapsePinned,
    /// Shows them again.
    ExpandPinned,
    /// Opens every folder of the Space shown.
    ExpandFolders,
    /// Shows the sidebar's row of the tab shown: opens its folders and scrolls the list to it.
    RevealTab,
    /// Gives the sidebar the keyboard, its arrows moving over the rows (sidebar_keys.rs).
    FocusSidebar,
    /// Loads the selected pinned tab's pinned URL.
    BackToPinnedUrl,
    /// Loads the pinned URL of each pinned tab of the Space shown that's away from it, as Arc's
    /// Reset all tabs in this Space does.
    ResetSpaceTabs,
    /// Mutes the selected tab, or unmutes it.
    ToggleMute,
    /// Turns Chrome's Battery Saver off until the computer's plugged in, while it's on (#686).
    TurnOffBatterySaver,
    /// Translates the page shown into the user's language on the device, where it's in another
    /// (#671).
    TranslatePage,
    /// Puts the translated page's own words back, as Arc's Use Original does.
    UseOriginal,
    /// Mutes every tab of the window (`true`), or unmutes them all: Arc's Mute All Tabs.
    MuteAll(bool),
    /// Puts the window full screen, or takes it out, as View's Enter Full Screen does.
    ToggleFullScreen,
    /// Arc's View › Exit (⎋): takes the window out of full screen, and a page full screen with
    /// it, as Exit Full Screen does.
    Exit,
    /// Closes the Today tabs of the Space shown, but the one shown.
    Clear,
    /// Undoes the last thing the user did that lost or moved something (`Shell::undo`).
    Undo,
    /// Does again what Undo undid.
    Redo,
    /// Copies the selected tab's URL.
    CopyUrl,
    /// Copies a Markdown link to the selected tab: its title, then its URL.
    CopyMarkdownLink,
    /// Opens the web link on the clipboard as a new tab.
    PasteUrlAsTab,
    /// Shares the selected tab's page through the system's share sheet.
    Share,
    Find,
    /// Opens the Site Controls for the selected tab's site, or closes them.
    SiteControls,
    /// Opens the selected tab's Cast dialog, or closes it.
    Cast,
    /// Starts capture mode over the selected tab's page, or leaves it.
    CapturePortion,
    /// Captures all of the selected tab's page, beyond its view, and copies it.
    CaptureFullPage,
    /// Opens Portrait Mode's card for the page the URL field shows, or closes it (portrait.rs).
    CapturePortrait,
    /// Portrait Mode on the background kept, without the card, as ⌥ on Site Controls' Capture
    /// Full Page runs it. Not in the command bar, which has the card.
    CapturePortraitNow,
    FindNext,
    FindPrevious,
    /// Scrolls the page to its selection.
    JumpToSelection,
    Back,
    Forward,
    Reload,
    /// Reloads without the cache.
    ForceReload,
    /// Deletes the page's site's cache, then reloads without the cache.
    ClearCacheAndRefresh,
    /// Deletes the page's site's cookies, then reloads.
    ClearCookiesAndRefresh,
    Stop,
    /// Zooms the page's site to 100 %.
    ActualSize,
    ZoomIn,
    ZoomOut,
    /// Opens the page's source in a tab under it.
    ViewSource,
    /// Asks where to save the page, and saves it.
    SavePage,
    /// Shows print preview for the page.
    Print,
    /// Opens the page's DevTools, or closes them.
    DevTools,
    /// Opens the page's DevTools picking an element.
    InspectElements,
    /// Opens the page's DevTools at the console, or closes them there.
    JavaScriptConsole,
    /// Opens Chrome's Task Manager.
    TaskManager,
    /// Starts recording a trace of the whole browser, or stops it and asks where to save it.
    RecordTrace,
    /// Keeps the window over other apps' windows, or lets it go back among them (Arc's Window ›
    /// Stay On Top).
    StayOnTop,
    /// Moves the sidebar to the window's other side, Settings' Sidebar position, in every window.
    /// The command bar's alone, with no key (#2375).
    MoveSidebar,
    /// Turns Developer Mode on or off for the page's site.
    ToggleDeveloperMode,
    /// Opens Chrome's extensions page in a new tab.
    ManageExtensions,
    /// Opens the Web Store in a new tab, as Dia's Add Extension… does.
    AddExtension,
    NewSpace,
    /// Shows the Space shown with its name in a field.
    RenameSpace,
    /// Opens the theme picker for the Space shown, as its menu's Edit Theme Color… does.
    EditTheme,
    /// Saves the Space shown's pinned tabs, folders and Today tabs as Markdown links.
    ExportSpace,
    /// Puts the Space shown in this profile, or with none in a new one.
    ChangeProfile(Option<u64>),
    /// Brings back the shown Space's profile's sidebar from its backup taken at this time, in
    /// milliseconds since 1970, once the user confirms.
    RestoreSidebar(u64),
    /// Lists the shown profile's sidebar backups in ilo's menu, to restore one as Help's does:
    /// the way in where there's no menu bar.
    RestoreEarlierSidebar,
    /// The Space at this index in the switcher.
    SelectSpace(usize),
    /// Moves the tab shown to this Space, as the tab menu's Move to does.
    MoveToSpace(u64),
    /// Pins the tab shown and moves it to this Space, at the bottom of its pinned tabs.
    PinToSpace(u64),
    /// Opens this folder of the Space shown, with the folders it's in, and scrolls to it.
    OpenFolder(u64),
    /// Pins the tab shown, or the rows chosen, at the bottom of this folder of the Space shown, as
    /// their menu's Add to Folder › does.
    MoveToFolder(u64),
    /// Runs the action of the extension with this id (`extensions::id_number`) on the tab shown,
    /// as the Extensions menu does.
    RunExtension(u128),
    NextSpace,
    PreviousSpace,
    /// Hides the docked sidebar, or shows it.
    ToggleSidebar,
    /// Opens the Library, or closes it.
    ViewLibrary,
    /// Opens the Library on the archive.
    ViewArchive,
    ClearArchive,
    /// Opens the Library on the downloads, or closes it there.
    ViewDownloads,
    /// Opens the Library on its media, or closes it there.
    ViewMedia,
    /// Opens the Library on its agents' past tasks, or closes it there.
    ViewAgents,
    /// Opens the card for a Boost of the site of the page shown, which has none yet (#695).
    NewBoost,
    /// Opens the card of the Boost of the site of the page shown.
    EditBoost,
    /// Opens the Library on the profile's Boosts, or closes it there.
    ViewBoosts,
    /// Opens the command bar over a new pane beside the pages shown, for a split view.
    AddSplit,
    /// Opens a new document in the notes service the Space's profile picks (`crate::notes`), in a
    /// tab, as Arc's New Note does.
    NewNote,
    /// The same, in a split view with the tab shown.
    NewNoteSplit,
    /// Closes the split view's focused page.
    CloseSplitPane,
    /// Takes the split view's focused page out of it, to show alone.
    SeparatePane,
    /// Takes every page of the split view out of it, and shows the focused one alone.
    SeparateSplit,
    /// Puts the split view's pages in the opposite order.
    FlipSplit,
    /// Stacks the split view's pages top to bottom, or puts them back side by side.
    ChangeOrientation,
    /// Gives the split view's focused page the card, leaving the others 220 pt each.
    ExpandSplit,
    /// Swaps the split view's focused page with the one on its left, or on its right.
    MovePaneLeft,
    MovePaneRight,
    /// Focuses the split view's page right of the focused one, or left of it.
    NextPane,
    PreviousPane,
    /// Focuses the split view's page at this place from the left, as Arc's ⌃⇧1–⌃⇧4 do.
    FocusPane(usize),
    /// Opens Chrome's history page in a new tab.
    ViewHistory,
    /// Opens Chromium's search engines page in a new tab, where the profile's site searches are
    /// added and edited: each a name, a keyword and a URL with %s for the query.
    SiteSearchSettings,
    /// Opens Chrome's Clear browsing data dialog in a new tab, for the Space's profile, as Arc's
    /// Settings › Profiles does for each profile.
    ClearBrowsingData,
    /// Opens the Settings window, or brings it to the front.
    Settings,
    /// Opens the Settings window on its Shortcuts tab.
    KeyboardShortcuts,
    /// Opens the Settings window on its About tab: the version, the engine's and the credits.
    About,
    /// Asks the updater to look, or says where the update stands, in a build that updates itself.
    CheckForUpdates,
    /// Opens the Settings window on its Import tab.
    Import,
    /// Opens the Settings window on its Passwords tab, with the Space's profile's logins.
    Passwords,
    /// Opens Chrome's autofill dropdown at the field of the page that has the keyboard, as a click
    /// in the field does, or with no field there, Settings › Passwords, as Edit › AutoFill's
    /// Contact… and Credit Card… do.
    AutoFill,
    /// The same, its dropdown with every login the profile has, as Chrome's Select Password and
    /// Edit › AutoFill › Passwords… do.
    AutoFillPasswords,
    /// Plays a new profile's first launch again.
    Welcome,
    /// Gives every window this appearance where its Space's theme doesn't choose one, in place of
    /// the system's (View › Appearance).
    Appearance(Appearance),
    /// Opens a new issue on the bug report form in a tab, its version and system fields filled
    /// with the build's details. Nothing is sent until the user submits it.
    ReportProblem,
    /// Copies the build's details: the version, the engine's, the system's and the chip.
    CopyBuildInfo,
    /// Opens the site's help in a tab.
    HelpCenter,
    /// Opens the site's release notes in a tab.
    WhatsNew,
    /// Opens the site's privacy policy in a tab.
    PrivacyPolicy,
    Quit,
}

/// The browser's shortcuts (docs/ux-spec.md › Keyboard): the keystroke, with modifiers in the order
/// ctrl, alt, shift, cmd, its command, and whether it's reserved. As in Chrome, reserved ones reach
/// the browser before the page sees them, and the others only if the page doesn't keep them.
pub(super) const SHORTCUTS: &[(&str, Command, bool)] = &[
    ("cmd-t", Command::NewTab, true),
    ("cmd-w", Command::CloseTab, true),
    ("cmd-n", Command::NewWindow, true),
    ("shift-cmd-n", Command::NewPrivateWindow, true),
    ("alt-cmd-n", Command::NewLittleWindow, true),
    ("ctrl-cmd-n", Command::NewBlankWindow, true),
    ("cmd-o", Command::OpenInMainWindow, false),
    ("shift-cmd-w", Command::CloseWindow, true),
    ("shift-cmd-t", Command::ReopenClosedTab, true),
    ("cmd-q", Command::Quit, true),
    ("cmd-,", Command::Settings, false),
    ("alt-cmd-down", Command::NextTab, true),
    ("alt-cmd-up", Command::PreviousTab, true),
    // Arc's ⌘} and ⌘{, Chrome's too. GPUI names them by their character and a page by the key with
    // shift, as ⌃⇧] below.
    ("shift-cmd-]", Command::NextTab, true),
    ("shift-cmd-}", Command::NextTab, true),
    ("shift-cmd-[", Command::PreviousTab, true),
    ("shift-cmd-{", Command::PreviousTab, true),
    ("ctrl-tab", Command::RecentTab, true),
    ("ctrl-shift-tab", Command::RecentTabBack, true),
    ("alt-cmd-right", Command::NextSpace, true),
    ("alt-cmd-left", Command::PreviousSpace, true),
    ("cmd-l", Command::EditUrl, false),
    ("shift-cmd-a", Command::SearchTabs, false),
    ("cmd-d", Command::TogglePin, false),
    ("cmd-enter", Command::BackToPinnedUrl, false),
    ("shift-cmd-k", Command::Clear, false),
    ("cmd-z", Command::Undo, false),
    ("shift-cmd-z", Command::Redo, false),
    ("shift-cmd-c", Command::CopyUrl, false),
    ("alt-shift-cmd-c", Command::CopyMarkdownLink, false),
    ("alt-cmd-v", Command::PasteUrlAsTab, false),
    ("cmd-f", Command::Find, false),
    ("cmd-g", Command::FindNext, false),
    ("shift-cmd-g", Command::FindPrevious, false),
    ("cmd-j", Command::JumpToSelection, false),
    // The first is the one the menu shows. ⇧2 is @ on US layouts, which GPUI names by its
    // character and a page by the key with shift.
    ("shift-cmd-2", Command::CapturePortion, false),
    ("cmd-@", Command::CapturePortion, false),
    ("shift-cmd-@", Command::CapturePortion, false),
    ("cmd-1", Command::SelectTab(0), false),
    ("cmd-2", Command::SelectTab(1), false),
    ("cmd-3", Command::SelectTab(2), false),
    ("cmd-4", Command::SelectTab(3), false),
    ("cmd-5", Command::SelectTab(4), false),
    ("cmd-6", Command::SelectTab(5), false),
    ("cmd-7", Command::SelectTab(6), false),
    ("cmd-8", Command::SelectTab(7), false),
    ("cmd-9", Command::SelectLastTab, false),
    ("cmd-[", Command::Back, false),
    ("cmd-]", Command::Forward, false),
    ("cmd-left", Command::Back, false),
    ("cmd-right", Command::Forward, false),
    ("cmd-r", Command::Reload, false),
    ("shift-cmd-r", Command::ForceReload, false),
    ("cmd-.", Command::Stop, false),
    ("cmd-0", Command::ActualSize, false),
    // The first is the one the menu shows. ⌘+ is ⇧⌘= on most layouts, which a page names either
    // way, and ⌘= is Chrome's hidden second ⌘+.
    ("cmd-+", Command::ZoomIn, false),
    ("cmd-=", Command::ZoomIn, false),
    ("shift-cmd-=", Command::ZoomIn, false),
    ("shift-cmd-+", Command::ZoomIn, false),
    ("cmd--", Command::ZoomOut, false),
    ("alt-cmd-u", Command::ViewSource, false),
    ("shift-cmd-s", Command::SavePage, false),
    ("cmd-p", Command::Print, false),
    ("alt-cmd-i", Command::DevTools, false),
    ("f12", Command::DevTools, false),
    ("alt-cmd-c", Command::InspectElements, false),
    ("alt-cmd-j", Command::JavaScriptConsole, false),
    // Chrome's off the Mac, where the Task Manager is a window of Chromium's own.
    ("shift-escape", Command::TaskManager, false),
    // Arc's, unreserved: a page or field that uses ⌃D, as a Mac text field's forward delete,
    // keeps it. Off the Mac it would be Alt+D, Chrome's, so it has none there.
    ("ctrl-d", Command::ToggleDeveloperMode, false),
    ("cmd-s", Command::ToggleSidebar, false),
    // Chrome's key for moving the keyboard out of the page, unreserved as Chrome's is, as Arc's ⌘.
    // is Stop Loading here.
    ("f6", Command::FocusSidebar, false),
    ("shift-cmd-l", Command::ViewLibrary, false),
    ("shift-cmd-j", Command::ViewDownloads, false),
    ("cmd-y", Command::ViewHistory, false),
    ("shift-cmd-backspace", Command::ClearBrowsingData, false),
    // ⌃+ is ⌃⇧= on most layouts, as ⌘+ is.
    ("ctrl-+", Command::AddSplit, false),
    ("ctrl-shift-=", Command::AddSplit, false),
    ("ctrl-shift-+", Command::AddSplit, false),
    ("ctrl-shift-n", Command::NewNote, false),
    ("ctrl-shift--", Command::CloseSplitPane, false),
    ("ctrl-shift-_", Command::CloseSplitPane, false),
    ("ctrl-shift-]", Command::NextPane, false),
    ("ctrl-shift-}", Command::NextPane, false),
    ("ctrl-shift-[", Command::PreviousPane, false),
    ("ctrl-shift-{", Command::PreviousPane, false),
    // A split view has at most four pages. GPUI may name the digit or what ⇧ makes of it.
    ("ctrl-shift-1", Command::FocusPane(0), false),
    ("ctrl-shift-!", Command::FocusPane(0), false),
    ("ctrl-shift-2", Command::FocusPane(1), false),
    ("ctrl-shift-@", Command::FocusPane(1), false),
    ("ctrl-shift-3", Command::FocusPane(2), false),
    ("ctrl-shift-#", Command::FocusPane(2), false),
    ("ctrl-shift-4", Command::FocusPane(3), false),
    ("ctrl-shift-$", Command::FocusPane(3), false),
    ("escape", Command::Stop, false),
    ("ctrl-1", Command::SelectSpace(0), false),
    ("ctrl-2", Command::SelectSpace(1), false),
    ("ctrl-3", Command::SelectSpace(2), false),
    ("ctrl-4", Command::SelectSpace(3), false),
    ("ctrl-5", Command::SelectSpace(4), false),
    ("ctrl-6", Command::SelectSpace(5), false),
    ("ctrl-7", Command::SelectSpace(6), false),
    ("ctrl-8", Command::SelectSpace(7), false),
    ("ctrl-9", Command::SelectSpace(8), false),
];

/// The name of the sidebar's second row in the command bar, which it lists only while there's a
/// sidebar to hide.
const HIDE_SIDEBAR: &str = "Hide Sidebar";

/// Window › Stay On Top, and the command bar's row that ends it while it's on.
pub const STAY_ON_TOP: &str = "Stay On Top";
const DONT_STAY_ON_TOP: &str = "Don't Stay On Top";

/// The command bar's rows for Sidebar position, of which it lists the one that would move it.
const SIDEBAR_TO_RIGHT: &str = "Move Sidebar to the Right";
const SIDEBAR_TO_LEFT: &str = "Move Sidebar to the Left";

/// The page's Developer Mode's rows in the command bar, of which it lists the one that would change
/// it (Arc's, in sentence case).
const DEVELOPER_MODE_ON: &str = "Turn on Developer Mode for this site";
const DEVELOPER_MODE_OFF: &str = "Turn off Developer Mode for this site";

/// What turns Developer Mode the other way, for a page in it or not.
pub(super) fn developer_mode_title(on: bool) -> &'static str {
    if on {
        DEVELOPER_MODE_OFF
    } else {
        DEVELOPER_MODE_ON
    }
}

/// The split's orientation's rows in the command bar, Arc's names, of which it lists the one that
/// would turn it: horizontal is side by side, vertical stacked (Arc 1.171).
const CONVERT_VERTICAL: &str = "Convert to Vertical Split View";
const CONVERT_HORIZONTAL: &str = "Convert to Horizontal Split View";

/// What turns a split the other way, for one `stacked` or side by side.
pub(super) fn orientation_title(stacked: bool) -> &'static str {
    if stacked {
        CONVERT_HORIZONTAL
    } else {
        CONVERT_VERTICAL
    }
}

/// The commands the command bar finds, with the words besides their names' that find them, in
/// lowercase: Arc's bar's names and words where they're known, else its menus' names and ours
/// (docs/ux-spec.md › Command bar › Commands). Among equal matches, the first here comes first.
pub(super) const BAR_COMMANDS: &[(&str, Command, &[&str])] = &[
    ("Pin Tab", Command::TogglePin, &["unpin"]),
    ("Next Tab", Command::NextTab, &[]),
    (
        "Search Tabs…",
        Command::SearchTabs,
        &["switch tab", "open tabs"],
    ),
    ("Archive Tab", Command::CloseTab, &["close tab"]),
    ("Clear Today", Command::Clear, &["unpinned tabs"]),
    (
        "Clear Cache and Refresh",
        Command::ClearCacheAndRefresh,
        &[],
    ),
    (
        "Clear Cookies and Refresh",
        Command::ClearCookiesAndRefresh,
        &[],
    ),
    ("Clear Archive", Command::ClearArchive, &[]),
    ("Favorite Tab", Command::FavoriteTab, &["add to favorites"]),
    ("Collapse All Folders", Command::CollapseFolders, &[]),
    (
        "Collapse Pinned Tabs",
        Command::CollapsePinned,
        &["hide pinned"],
    ),
    (
        "Expand Pinned Tabs",
        Command::ExpandPinned,
        &["show pinned"],
    ),
    ("Expand All Folders", Command::ExpandFolders, &[]),
    ("Reveal Tab in Sidebar", Command::RevealTab, &["locate"]),
    (
        "Focus Sidebar",
        Command::FocusSidebar,
        &["keyboard", "arrow keys"],
    ),
    ("New Folder…", Command::NewFolder, &[]),
    (
        "Back to Pinned URL",
        Command::BackToPinnedUrl,
        &["reset tab"],
    ),
    ("Mute/Unmute Tab", Command::ToggleMute, &["sound", "audio"]),
    (
        "Turn Off Battery Saver for Now",
        Command::TurnOffBatterySaver,
        &["energy saver", "power"],
    ),
    (
        "Translate Page",
        Command::TranslatePage,
        &["translation", "language"],
    ),
    (
        "Use Original",
        Command::UseOriginal,
        &["translation", "untranslate", "show original"],
    ),
    (
        "Mute All Tabs",
        Command::MuteAll(true),
        &["sound", "audio", "silence"],
    ),
    (
        "Unmute All Tabs",
        Command::MuteAll(false),
        &["sound", "audio"],
    ),
    ("Exit", Command::Exit, &["escape"]),
    ("Enter Full Screen", Command::ToggleFullScreen, &["toggle"]),
    ("Exit Full Screen", Command::ToggleFullScreen, &["toggle"]),
    (
        "New Profile",
        Command::ChangeProfile(None),
        &["add profile"],
    ),
    ("Copy URL", Command::CopyUrl, &["link"]),
    ("Copy URL as Markdown", Command::CopyMarkdownLink, &["link"]),
    (
        "Paste URL as New Tab",
        Command::PasteUrlAsTab,
        &["clipboard", "open"],
    ),
    ("Share…", Command::Share, &["send"]),
    ("New Window", Command::NewWindow, &["duplicate", "copy"]),
    ("Duplicate Current Tab", Command::DuplicateTab, &["copy"]),
    (
        "New Incognito Window",
        Command::NewPrivateWindow,
        &["private"],
    ),
    (
        "New Little ilo Window",
        Command::NewLittleWindow,
        &["little arc", "small window"],
    ),
    (
        "New Blank Window",
        Command::NewBlankWindow,
        &["empty window", "unsaved"],
    ),
    (
        "Open in Main Window",
        Command::OpenInMainWindow,
        &["little", "move"],
    ),
    ("Close Window", Command::CloseWindow, &[]),
    (
        "Restore Last Closed Tab",
        Command::ReopenClosedTab,
        &["reopen", "undo close"],
    ),
    ("Find…", Command::Find, &[]),
    ("Site Controls", Command::SiteControls, &["permissions"]),
    ("Cast…", Command::Cast, &["chromecast", "tv", "mirror"]),
    ("Go Back", Command::Back, &[]),
    ("Go Forward", Command::Forward, &[]),
    ("Refresh the Page", Command::Reload, &["reload"]),
    (
        "Force Refresh the Page",
        Command::ForceReload,
        &["reload", "hard reload"],
    ),
    ("Stop Loading", Command::Stop, &[]),
    ("Zoom In", Command::ZoomIn, &[]),
    ("Zoom Out", Command::ZoomOut, &[]),
    ("Zoom to Actual Size", Command::ActualSize, &["reset zoom"]),
    ("Add Split View", Command::AddSplit, &["side by side"]),
    ("Close this Split Pane", Command::CloseSplitPane, &[]),
    ("Separate Page from Split View", Command::SeparatePane, &[]),
    ("Expand Current Split", Command::ExpandSplit, &[]),
    // Dia's (docs/ux-spec.md › Split view).
    ("Flip Split View", Command::FlipSplit, &[]),
    // Arc's names, with Dia's among the words.
    (
        CONVERT_VERTICAL,
        Command::ChangeOrientation,
        &["change orientation", "stack"],
    ),
    (
        CONVERT_HORIZONTAL,
        Command::ChangeOrientation,
        &["change orientation", "side by side"],
    ),
    ("Move Pane Left", Command::MovePaneLeft, &[]),
    ("Move Pane Right", Command::MovePaneRight, &[]),
    (
        "Separate All Tabs",
        Command::SeparateSplit,
        &["separate all pages"],
    ),
    (
        "New Note",
        Command::NewNote,
        &["document", "notion", "google docs", "word", "confluence"],
    ),
    (
        "New Note Split",
        Command::NewNoteSplit,
        &["document", "side by side"],
    ),
    ("Toggle Sidebar", Command::ToggleSidebar, &["hide", "show"]),
    (
        "Automatic Appearance",
        Command::Appearance(Appearance::Automatic),
        &["system", "app theme"],
    ),
    (
        "Light Appearance",
        Command::Appearance(Appearance::Light),
        &["light mode", "app theme"],
    ),
    (
        "Dark Appearance",
        Command::Appearance(Appearance::Dark),
        &["dark mode", "app theme"],
    ),
    (HIDE_SIDEBAR, Command::ToggleSidebar, &[]),
    ("Capture…", Command::CapturePortion, &["screenshot"]),
    (
        "Capture Full Page",
        Command::CaptureFullPage,
        &["screenshot"],
    ),
    (
        "Capture in Portrait Mode",
        Command::CapturePortrait,
        &["screenshot", "share"],
    ),
    ("Save Page As", Command::SavePage, &[]),
    ("Print", Command::Print, &["pdf"]),
    ("View Source", Command::ViewSource, &[]),
    ("Developer Tools", Command::DevTools, &["devtools"]),
    ("Inspect Elements", Command::InspectElements, &[]),
    ("JavaScript Console", Command::JavaScriptConsole, &[]),
    (
        DEVELOPER_MODE_ON,
        Command::ToggleDeveloperMode,
        &["dev mode", "localhost"],
    ),
    (
        DEVELOPER_MODE_OFF,
        Command::ToggleDeveloperMode,
        &["dev mode", "localhost"],
    ),
    ("Open Task Manager", Command::TaskManager, &[]),
    (
        STAY_ON_TOP,
        Command::StayOnTop,
        &["float", "always on top", "pin window"],
    ),
    (
        DONT_STAY_ON_TOP,
        Command::StayOnTop,
        &["float", "always on top", "pin window"],
    ),
    (
        SIDEBAR_TO_RIGHT,
        Command::MoveSidebar,
        &["side", "position", "left", "right"],
    ),
    (
        SIDEBAR_TO_LEFT,
        Command::MoveSidebar,
        &["side", "position", "left", "right"],
    ),
    (
        trace::RECORD,
        Command::RecordTrace,
        &["performance", "tracing"],
    ),
    (
        trace::STOP_RECORDING,
        Command::RecordTrace,
        &["performance", "tracing"],
    ),
    (
        "Manage Extensions…",
        Command::ManageExtensions,
        &["plugins"],
    ),
    ("Add Extension…", Command::AddExtension, &[]),
    ("New Space…", Command::NewSpace, &[]),
    ("Rename Space", Command::RenameSpace, &[]),
    // Arc's bar's name, where its Space menu says Edit Theme Color…. "theme" finds it before the
    // Appearance rows, whose "app theme" it finds as a later word.
    (
        "Edit Theme…",
        Command::EditTheme,
        &["theme", "color", "colour"],
    ),
    (
        "Export Space as Markdown…",
        Command::ExportSpace,
        &["save", "links", "backup"],
    ),
    ("Rename Current Tab…", Command::RenameTab, &["title"]),
    ("Next Space", Command::NextSpace, &[]),
    ("Previous Space", Command::PreviousSpace, &[]),
    ("Previous Tab", Command::PreviousTab, &[]),
    ("Open Library…", Command::ViewLibrary, &["view library"]),
    ("View Archive…", Command::ViewArchive, &[]),
    ("View Downloads…", Command::ViewDownloads, &[]),
    (
        "View Media…",
        Command::ViewMedia,
        &["screenshots", "captures"],
    ),
    (
        "View Agents…",
        Command::ViewAgents,
        &["agent history", "past tasks"],
    ),
    ("New Boost", Command::NewBoost, &["zap", "css", "style"]),
    ("Edit Boost", Command::EditBoost, &["zap", "css", "style"]),
    (
        "View Boosts…",
        Command::ViewBoosts,
        &["zap", "css", "style"],
    ),
    ("View History", Command::ViewHistory, &[]),
    (
        "Site Search Settings",
        Command::SiteSearchSettings,
        &["search engines", "keywords"],
    ),
    (
        "Clear Browsing Data…",
        Command::ClearBrowsingData,
        &["history", "cookies", "cache"],
    ),
    ("Settings", Command::Settings, &["preferences"]),
    (
        "Essential Keyboard Shortcuts",
        Command::KeyboardShortcuts,
        &["keys"],
    ),
    (
        concat!("About ", env!("APP_NAME")),
        Command::About,
        &["version", "credits", "licence"],
    ),
    // The app menu's, which Linux shows no bar for, and its name once an update is ready.
    (
        crate::updates::CHECK_FOR_UPDATES,
        Command::CheckForUpdates,
        &["update", "upgrade"],
    ),
    (
        crate::updates::RESTART,
        Command::CheckForUpdates,
        &["update", "upgrade", "relaunch"],
    ),
    (
        "Import from Other Browsers…",
        Command::Import,
        &["bookmarks"],
    ),
    (
        "Privacy Policy",
        Command::PrivacyPolicy,
        &["data", "tracking"],
    ),
    ("Passwords", Command::Passwords, &["logins", "keychain"]),
    (
        concat!("Welcome to ", env!("APP_NAME")),
        Command::Welcome,
        &["onboarding", "intro"],
    ),
    ("Help Center", Command::HelpCenter, &["help", "support"]),
    (
        "What's New",
        Command::WhatsNew,
        &["release notes", "changelog"],
    ),
    (
        "Report a Problem…",
        Command::ReportProblem,
        &["bug", "feedback", "issue"],
    ),
    (
        "Restore Earlier Sidebar…",
        Command::RestoreEarlierSidebar,
        &["backup", "restore data"],
    ),
    ("Copy Build Info", Command::CopyBuildInfo, &["version"]),
    (concat!("Quit ", env!("APP_NAME")), Command::Quit, &[]),
];

/// BAR_COMMANDS' names in lowercase, made once, since the bar matches them on every key.
static LOWERCASE_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
    BAR_COMMANDS
        .iter()
        .map(|(name, ..)| name.to_lowercase())
        .collect()
});

/// The most rows the command bar lists, five shown and the rest scrolled (Arc), so the most
/// commands it's given.
pub const BAR_ROWS: usize = 15;

/// How well a command matches what's typed, the weakest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Match {
    /// What's typed has its letters in the name in order, from a word's start.
    Letters,
    /// A word of the name after its first, or of a keyword after its first, starts with it.
    Word,
    /// One of its keywords starts with it.
    Keyword,
    /// Its name starts with it.
    Name,
}

/// A command the command bar offers for what's typed.
pub struct BarCommand {
    pub name: SharedString,
    pub command: Command,
    /// The shortcut that runs it, as a menu shows it.
    pub shortcut: Option<SharedString>,
    pub matched: Match,
}

/// The most tabs the command bar lists before anything is typed (ours: Arc's limit isn't measured).
pub const BAR_TABS_SHOWN: usize = 10;

/// One of the Space's tabs, which the command bar lists before anything is typed, and puts first
/// when what's typed starts its name.
#[derive(Clone)]
pub struct BarTab {
    pub tab: u64,
    pub title: SharedString,
    pub url: SharedString,
    pub favicon: Option<Arc<Image>>,
    /// Whether its title is the name the user gave it, which counts more when what's typed starts
    /// it, as Arc's custom title does.
    pub renamed: bool,
    /// How much the user's switches to it count, which ranks the tabs matching what's typed.
    pub switched: f32,
}

/// One of the user's Spaces, which a Little ilo window's Spaces menu lists for its page to go to,
/// with the mark Move to Space's menu shows it by.
#[derive(Clone, Debug, PartialEq)]
pub struct BarSpace {
    pub space: u64,
    pub name: SharedString,
    pub swatch: crate::menu::Swatch,
}

/// The commands that do something now that match `typed`, best first.
pub fn bar_commands(typed: &str, cx: &App) -> Vec<BarCommand> {
    let shell = key_shell(cx).and_then(|shell| shell.upgrade());
    let Some(shell) = shell else {
        return Vec::new();
    };
    let shell = shell.read(cx);
    let keys = user_keys(cx);
    let picks = shell.bar_picks(cx);
    let found = find_ranked(typed, keys, picks, |name, command| {
        shell.bar_name(name, command, cx)
    });
    let mut named = shell.named_bar_rows();
    named.extend(shell.numbered_bar_row(typed, cx));
    named.extend(shell.extension_bar_rows());
    with_named_ranked(found, typed, &named, keys, picks)
}

/// The commands of BAR_COMMANDS that match `typed`, best first and then in the table's order, at
/// most BAR_ROWS, each with the name `named` gives it now, leaving out those it gives none. With
/// nothing typed, as the command bar's commands-only list starts, it's all of them in the table's
/// order.
pub fn find_commands(
    typed: &str,
    keys: &UserKeys,
    named: impl FnMut(&'static str, Command) -> Option<&'static str>,
) -> Vec<BarCommand> {
    find_ranked(typed, keys, None, named)
}

/// As `find_commands`, with equal matches ranked by the user's `picks` of them.
fn find_ranked(
    typed: &str,
    keys: &UserKeys,
    picks: Option<&Usage<String>>,
    mut named: impl FnMut(&'static str, Command) -> Option<&'static str>,
) -> Vec<BarCommand> {
    let typed = typed.trim().to_lowercase();
    let found = (BAR_COMMANDS.iter())
        .zip(LOWERCASE_NAMES.iter())
        .filter_map(|(&(name, command, keywords), lowercase)| {
            let matched = match typed.is_empty() {
                true => Match::Letters,
                false => match_of(lowercase, keywords, &typed)?,
            };
            // "close" means the tab, as Arc lists Archive Tab above Close Window for it, and ↩ on
            // the window's close would take every tab with it: Close Window comes before the
            // suggestions only once what's typed goes past its first word.
            let first_word = matched == Match::Name && !typed.contains(' ');
            let matched = match command == Command::CloseWindow && first_word {
                true => Match::Letters,
                false => matched,
            };
            Some((matched, SharedString::from(named(name, command)?), command))
        })
        .collect();
    ranked(found, &typed, keys, picks)
}

/// A command bar row the browser names now: its name, the words that find it as hidden words do,
/// such as what it's for, and its command.
pub type NamedRow = (SharedString, Vec<SharedString>, Command);

/// `found` with the rows of `rows` that match `typed`, ranked as `find_commands` ranks. None with
/// nothing typed, so the commands-only list starts with the table's commands alone (ours).
pub fn with_named_rows(
    found: Vec<BarCommand>,
    typed: &str,
    rows: &[NamedRow],
    keys: &UserKeys,
) -> Vec<BarCommand> {
    with_named_ranked(found, typed, rows, keys, None)
}

fn with_named_ranked(
    found: Vec<BarCommand>,
    typed: &str,
    rows: &[NamedRow],
    keys: &UserKeys,
    picks: Option<&Usage<String>>,
) -> Vec<BarCommand> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return found;
    }
    let found = found
        .into_iter()
        .map(|row| (row.matched, row.name, row.command));
    let named = rows.iter().filter_map(|(name, words, command)| {
        let words: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let matched = match_of(&name.to_lowercase(), &words, &typed)?;
        Some((matched, name.clone(), *command))
    });
    ranked(found.chain(named).collect(), &typed, keys, picks)
}

/// `found` best first, equals by how much the user picked them, then in their order, at most
/// BAR_ROWS unless nothing is `typed`, when it's the table's order alone, which the user learns by
/// place.
fn ranked(
    mut found: Vec<(Match, SharedString, Command)>,
    typed: &str,
    keys: &UserKeys,
    picks: Option<&Usage<String>>,
) -> Vec<BarCommand> {
    use std::cmp::Reverse;
    match picks.filter(|picks| !typed.is_empty() && !picks.is_empty()) {
        // A score is never negative, so its bits order as it does. Stable, so equals keep the
        // table's order.
        Some(picks) => {
            let now = usage::now();
            let picked = |name: &str| picks.score(name, now).to_bits();
            found.sort_by_cached_key(|(matched, name, _)| {
                (Reverse(*matched), Reverse(picked(name)))
            });
        }
        None => found.sort_by_key(|&(matched, ..)| Reverse(matched)),
    }
    if !typed.is_empty() {
        found.truncate(BAR_ROWS);
    }
    (found.into_iter())
        .map(|(matched, name, command)| BarCommand {
            name,
            command,
            // Exit is View › Exit's ⎋, which leaves full screen as it does, bound to no command.
            shortcut: (keys.label(command))
                .or_else(|| (command == Command::Exit).then(|| keystroke_label("escape")))
                .map(Into::into),
            matched,
        })
        .collect()
}

/// How `name`, lowercase, or its `keywords` match `typed`, which is lowercase too, if they do.
fn match_of(name: &str, keywords: &[&str], typed: &str) -> Option<Match> {
    let later_word = |text: &str| words(text).skip(1).any(|word| word.starts_with(typed));
    if name.starts_with(typed) {
        return Some(Match::Name);
    }
    if keywords.iter().any(|keyword| keyword.starts_with(typed)) {
        return Some(Match::Keyword);
    }
    if later_word(name) || keywords.iter().any(|keyword| later_word(keyword)) {
        return Some(Match::Word);
    }
    let first = typed.chars().next()?;
    let in_order = |word: &str| {
        let mut letters = word.chars();
        typed
            .chars()
            .all(|letter| letters.any(|other| other == letter))
    };
    words(name)
        .any(|word| word.starts_with(first) && in_order(word))
        .then_some(Match::Letters)
}

/// `text` from each of its words' starts on, a word starting after a space or a slash.
fn words(text: &str) -> impl Iterator<Item = &str> {
    (text.char_indices())
        .filter(move |&(at, _)| at == 0 || text[..at].ends_with([' ', '/']))
        .map(move |(at, _)| &text[at..])
}

/// How a menu shows `keystroke`, as SHORTCUTS writes it: ⇧⌘K for shift-cmd-k, and Ctrl+Shift+K
/// off the Mac, where it's pressed that way.
pub fn keystroke_label(keystroke: &str) -> String {
    keystroke_label_on(keystroke, super::keys::OFF_THE_MAC)
}

/// `keystroke_label`, on the Mac or `off_the_mac`, for each key chip that spells a chord, such as
/// Stop's ⌘. or Ctrl+., the way a menu does.
pub(super) fn keystroke_label_on(keystroke: &str, off_the_mac: bool) -> String {
    match off_the_mac {
        true => keystroke_keys_on(keystroke, true).join("+"),
        false => keystroke_keys_on(keystroke, false).concat(),
    }
}

/// The keys of `keystroke` as a menu shows them, one by one, for a chip each: ⇧, ⌘ and K for
/// shift-cmd-k, and Ctrl, Shift and K off the Mac. None if it can't be pressed here.
pub fn keystroke_keys(keystroke: &str) -> Vec<String> {
    keystroke_keys_on(keystroke, super::keys::OFF_THE_MAC)
}

fn keystroke_keys_on(keystroke: &str, off_the_mac: bool) -> Vec<String> {
    match off_the_mac {
        true => (super::keys::pressed_off_the_mac(keystroke).first())
            .map(|pressed| keys_named(pressed, true))
            .unwrap_or_default(),
        false => keys_named(keystroke, false),
    }
}

/// The keys of `keystroke`, its modifiers and key named as on the Mac, or `off_the_mac` in words.
fn keys_named(keystroke: &str, off_the_mac: bool) -> Vec<String> {
    let names = match off_the_mac {
        true => ["Ctrl", "Alt", "Shift", "Super"],
        false => ["⌃", "⌥", "⇧", "⌘"],
    };
    let mut rest = keystroke;
    let mut keys = Vec::new();
    for (modifier, name) in ["ctrl-", "alt-", "shift-", "cmd-"].into_iter().zip(names) {
        if let Some(after) = rest.strip_prefix(modifier) {
            keys.push(name.to_string());
            rest = after;
        }
    }
    keys.push(key_name(rest, off_the_mac));
    keys
}

/// How a menu names `key`, a binding's key such as "enter": by its symbol on the Mac, as macOS
/// does, and `off_the_mac` by Chrome's word for it on Linux and Windows, where the UI's font has no
/// keyboard symbols (#772).
pub fn key_name(key: &str, off_the_mac: bool) -> String {
    let (symbol, word) = match key {
        "enter" => ("↩", "Enter"),
        "escape" => ("⎋", "Esc"),
        "up" => ("↑", "Up Arrow"),
        "down" => ("↓", "Down Arrow"),
        "left" => ("←", "Left Arrow"),
        "right" => ("→", "Right Arrow"),
        "tab" => ("⇥", "Tab"),
        "backspace" => ("⌫", "Backspace"),
        "delete" => ("⌦", "Del"),
        "space" => ("Space", "Space"),
        "pageup" => ("⇞", "Page Up"),
        "pagedown" => ("⇟", "Page Down"),
        "home" => ("↖", "Home"),
        "end" => ("↘", "End"),
        key => return key.to_uppercase(),
    };
    match off_the_mac {
        true => word.into(),
        false => symbol.into(),
    }
}

/// Copies `text`, a sum's result chosen in the command bar, once the bar has gone, and says so in
/// the toast.
pub fn copy_from_bar(text: &str, cx: &mut App) {
    if let Some(shell) = key_shell(cx) {
        shell
            .update(cx, |shell, cx| {
                shell.command_bar_dismissed(cx);
                shell.copy(text, &format!("Copied {text}"), cx);
            })
            .ok();
    }
}

/// ⇧⌘C in the link preview copies its page's address, not the tab's under it, as Arc's Peek
/// does, with the toast ⇧⌘C shows.
pub fn copy_url_of_preview(url: &str, cx: &mut App) {
    let Some(shell) = key_shell(cx) else {
        return;
    };
    shell
        .update(cx, |shell, cx| {
            let url = shell.copied_link(url);
            if !url.is_empty() {
                shell.copy(&url, "Copied Current URL", cx);
            }
        })
        .ok();
}

/// Runs `command`, whose row in the command bar was chosen, once the bar has gone.
pub fn run_from_bar(command: Command, cx: &mut App) {
    if let Some(shell) = key_shell(cx) {
        shell
            .update(cx, |shell, cx| {
                shell.command_bar_dismissed(cx);
                shell.run(command, "command bar", cx);
            })
            .ok();
    }
}

impl Command {
    /// Whether it opens the command bar or the find bar with the keyboard, which the keys typed
    /// right after its shortcut belong in (docs/ux-spec.md › Command bar › Keys).
    pub fn opens_field(self) -> bool {
        matches!(
            self,
            Command::NewTab
                | Command::SearchTabs
                | Command::EditUrl
                | Command::Find
                | Command::AddSplit
        )
    }

    /// The command's name as Settings knows it, in the user's saved keys and the lines it logs:
    /// its name, with the number of a Space's, as each of ⌃1–⌃9 has keys of its own.
    pub fn key_name(self) -> String {
        match self {
            Command::SelectSpace(index) => format!("select-space-{}", index + 1),
            Command::FocusPane(index) => format!("focus-pane-{}", index + 1),
            _ => self.name().to_string(),
        }
    }

    /// The command's name in the log.
    pub fn name(self) -> &'static str {
        match self {
            Command::NewTab => "new-tab",
            Command::SearchTabs => "search-tabs",
            Command::NewWindow => "new-window",
            Command::NewPrivateWindow => "new-private-window",
            Command::NewLittleWindow => "new-little-window",
            Command::NewBlankWindow => "new-blank-window",
            Command::OpenInMainWindow => "open-in-main-window",
            Command::CloseWindow => "close-window",
            Command::EditUrl => "edit-url",
            Command::CloseTab => "close-tab",
            Command::ReopenClosedTab => "reopen-closed-tab",
            Command::NextTab => "next-tab",
            Command::PreviousTab => "previous-tab",
            Command::RecentTab => "recent-tab",
            Command::RecentTabBack => "recent-tab-back",
            Command::SelectTab(_) => "select-tab",
            Command::SelectLastTab => "select-last-tab",
            Command::TogglePin => "toggle-pin",
            Command::FavoriteTab => "add-to-favorites",
            Command::DuplicateTab => "duplicate-tab",
            Command::RenameTab => "rename-tab",
            Command::NewFolder => "new-folder",
            Command::CollapseFolders => "collapse-folders",
            Command::CollapsePinned => "collapse-pinned",
            Command::ExpandPinned => "expand-pinned",
            Command::ExpandFolders => "expand-folders",
            Command::RevealTab => "reveal-tab",
            Command::FocusSidebar => "focus-sidebar",
            Command::BackToPinnedUrl => "back-to-pinned-url",
            Command::ResetSpaceTabs => "reset-space-tabs",
            Command::ToggleMute => "toggle-mute",
            Command::TurnOffBatterySaver => "turn-off-battery-saver",
            Command::TranslatePage => "translate-page",
            Command::UseOriginal => "use-original",
            Command::MuteAll(true) => "mute-all-tabs",
            Command::MuteAll(false) => "unmute-all-tabs",
            Command::ToggleFullScreen => "toggle-full-screen",
            Command::Exit => "exit",
            Command::Clear => "clear",
            Command::Undo => "undo",
            Command::Redo => "redo",
            Command::CopyUrl => "copy-url",
            Command::CopyMarkdownLink => "copy-markdown-link",
            Command::PasteUrlAsTab => "paste-url-as-tab",
            Command::Share => "share",
            Command::Find => "find",
            Command::SiteControls => "site-controls",
            Command::Cast => "cast",
            Command::CapturePortion => "capture",
            Command::CaptureFullPage => "capture-full-page",
            Command::CapturePortrait => "capture-portrait",
            Command::CapturePortraitNow => "capture-portrait-now",
            Command::FindNext => "find-next",
            Command::FindPrevious => "find-previous",
            Command::JumpToSelection => "jump-to-selection",
            Command::Back => "back",
            Command::Forward => "forward",
            Command::Reload => "reload",
            Command::ForceReload => "force-reload",
            Command::ClearCacheAndRefresh => "clear-cache-and-refresh",
            Command::ClearCookiesAndRefresh => "clear-cookies-and-refresh",
            Command::Stop => "stop",
            Command::ActualSize => "actual-size",
            Command::ZoomIn => "zoom-in",
            Command::ZoomOut => "zoom-out",
            Command::ViewSource => "view-source",
            Command::SavePage => "save-page",
            Command::Print => "print",
            Command::DevTools => "devtools",
            Command::InspectElements => "inspect-elements",
            Command::JavaScriptConsole => "javascript-console",
            Command::TaskManager => "task-manager",
            Command::RecordTrace => "record-trace",
            Command::StayOnTop => "stay-on-top",
            Command::MoveSidebar => "move-sidebar",
            Command::ToggleDeveloperMode => "toggle-developer-mode",
            Command::ManageExtensions => "manage-extensions",
            Command::AddExtension => "add-extension",
            Command::NewSpace => "new-space",
            Command::RenameSpace => "rename-space",
            Command::EditTheme => "edit-theme",
            Command::ExportSpace => "export-space",
            Command::ChangeProfile(_) => "change-profile",
            Command::RestoreSidebar(_) => "restore-sidebar",
            Command::RestoreEarlierSidebar => "restore-earlier-sidebar",
            Command::SelectSpace(_) => "select-space",
            Command::MoveToSpace(_) => "move-to-space",
            Command::PinToSpace(_) => "pin-to-space",
            Command::OpenFolder(_) => "open-folder",
            Command::MoveToFolder(_) => "move-to-folder",
            Command::RunExtension(_) => "run-extension",
            Command::NextSpace => "next-space",
            Command::PreviousSpace => "previous-space",
            Command::ToggleSidebar => "toggle-sidebar",
            Command::ViewLibrary => "view-library",
            Command::ViewArchive => "view-archive",
            Command::ClearArchive => "clear-archive",
            Command::ViewDownloads => "view-downloads",
            Command::ViewMedia => "view-media",
            Command::ViewAgents => "view-agents",
            Command::NewBoost => "new-boost",
            Command::EditBoost => "edit-boost",
            Command::ViewBoosts => "view-boosts",
            Command::AddSplit => "add-split",
            Command::NewNote => "new-note",
            Command::NewNoteSplit => "new-note-split",
            Command::CloseSplitPane => "close-split-pane",
            Command::SeparatePane => "separate-pane",
            Command::SeparateSplit => "separate-split",
            Command::FlipSplit => "flip-split",
            Command::ChangeOrientation => "change-orientation",
            Command::ExpandSplit => "expand-split",
            Command::MovePaneLeft => "move-pane-left",
            Command::MovePaneRight => "move-pane-right",
            Command::NextPane => "next-pane",
            Command::PreviousPane => "previous-pane",
            Command::FocusPane(_) => "focus-pane",
            Command::ViewHistory => "view-history",
            Command::SiteSearchSettings => "site-search-settings",
            Command::ClearBrowsingData => "clear-browsing-data",
            Command::Appearance(Appearance::Automatic) => "appearance-automatic",
            Command::Appearance(Appearance::Light) => "appearance-light",
            Command::Appearance(Appearance::Dark) => "appearance-dark",
            Command::Settings => "settings",
            Command::KeyboardShortcuts => "keyboard-shortcuts",
            Command::About => "about",
            Command::CheckForUpdates => "check-for-updates",
            Command::Import => "import",
            Command::Passwords => "passwords",
            Command::AutoFill => "autofill",
            Command::AutoFillPasswords => "autofill-passwords",
            Command::Welcome => "welcome",
            Command::ReportProblem => "report-problem",
            Command::CopyBuildInfo => "copy-build-info",
            Command::HelpCenter => "help-center",
            Command::WhatsNew => "whats-new",
            Command::PrivacyPolicy => "privacy-policy",
            Command::Quit => "quit",
        }
    }
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = browser, no_json)]
pub struct Run(pub Command);

actions!(space_form, [CreateSpace]);
actions!(space_name, [KeepName, CancelName]);

pub(super) struct ShellHandle(pub(super) WeakEntity<Shell>);

impl Global for ShellHandle {}

thread_local! {
    /// The shell of the browser window made key last, from the moment the host says so until the
    /// shell has heard: a shortcut pressed in between acts on it, not on the window behind.
    static MADE_KEY: std::cell::RefCell<Option<WeakEntity<Shell>>> = const { std::cell::RefCell::new(None) };
}

/// The shell of the browser window last key, which the browser's shortcuts and menus act on.
pub(super) fn key_shell(cx: &App) -> Option<WeakEntity<Shell>> {
    let made_key = MADE_KEY.with_borrow(Clone::clone);
    made_key
        .filter(|shell| shell.upgrade().is_some())
        .or_else(|| {
            cx.try_global::<ShellHandle>()
                .map(|handle| handle.0.clone())
        })
}

/// A browser window became key, as the host says from inside AppKit's update that made it so,
/// where the app can't change (#819). The shortcuts act on it from now; what the shell does about
/// it is returned, for once that update is over.
pub fn window_became_key(shell: WeakEntity<Shell>) -> impl FnOnce(&mut App) + 'static {
    MADE_KEY.set(Some(shell.clone()));
    move |cx| {
        if shell.update(cx, |shell, cx| shell.became_key(cx)).is_err() {
            // Closed before it heard.
            heard_key(&shell);
        }
    }
}

/// Whether a window made key since `shell` was has yet to be heard, so `shell` hearing late mustn't
/// take the shortcuts back from it (#2648).
pub(super) fn key_moved_on(shell: &WeakEntity<Shell>) -> bool {
    MADE_KEY.with_borrow(|made_key| {
        made_key
            .as_ref()
            .is_some_and(|made_key| made_key != shell && made_key.upgrade().is_some())
    })
}

/// The shell made key has heard, so the ShellHandle names it.
pub(super) fn heard_key(shell: &WeakEntity<Shell>) {
    MADE_KEY.with_borrow_mut(|made_key| {
        if made_key.as_ref() == Some(shell) {
            *made_key = None;
        }
    });
}

/// The key context at the root of the browser's windows, where its shortcuts apply. GPUI ranks a
/// binding with no context above every other, so without one the shortcuts would take keys a
/// focused field binds for itself, such as escape and ⌘←.
pub const KEY_CONTEXT: &str = "Browser";

/// Binds the shortcuts for while the UI has the keyboard. The browser hands them over itself while
/// the page has it.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(keys::key_bindings(cx));
    cx.bind_keys([
        KeyBinding::new("enter", CreateSpace, Some("SpaceForm")),
        KeyBinding::new("enter", KeepName, Some("Naming")),
        KeyBinding::new("escape", CancelName, Some("Naming")),
    ]);
    super::bind_conversation_keys(cx);
    cx.on_action(|run: &Run, cx| {
        // The menu's keys reach it from the dialog's window too, and the dialog has to be
        // answered first, as an alert's does.
        if crate::dialog::asking(cx) {
            return;
        }
        // A shortcut in the UI opens its field before GPUI has read the keys typed after it, which
        // it holds for the field under Wayland, as for one the page has (gpui_linux::hold_keys).
        #[cfg(target_os = "linux")]
        if run.0.opens_field() {
            gpui_linux::hold_keys(None);
        }
        if let Some(shell) = key_shell(cx) {
            shell
                .update(cx, |shell, cx| shell.run(run.0, "ui", cx))
                .ok();
        }
    });
    extensions::bind(cx);
    agent_bar::bind(cx);
    drm_note::bind(cx);
    translate::bind(cx);
    boost::bind(cx);
    sidebar_keys::bind(cx);
}

/// The command bar is about to put `tab` in a split view with `with`: the pages shown take back the
/// room of the pane being added, if one is, so the split view shows at its place at once. ⌘Z takes
/// `tab` back out, or closes it if it was `opened` for the split.
pub fn pane_added(tab: u64, with: u64, opened: bool, cx: &mut App) {
    if let Some(shell) = key_shell(cx) {
        shell
            .update(cx, |shell, cx| {
                if shell.adding_pane.take().is_some() {
                    shell.lay_out_page(cx);
                    cx.notify();
                }
                match opened {
                    true => shell.split_opened(tab, cx),
                    false => shell.keep_split(tab, with, cx),
                }
            })
            .ok();
    }
}

/// The tab the window shows, or 0.
pub fn shown_tab(cx: &App) -> u64 {
    let shell = key_shell(cx).and_then(|shell| shell.upgrade());
    shell.map_or(0, |shell| shell.read(cx).selected)
}

/// Whether `tab` is a task's page the window shows in place, which the user may edit the address
/// of but which stays in the task's Space.
pub fn task_page_shown(tab: u64, cx: &App) -> bool {
    let shell = key_shell(cx).and_then(|shell| shell.upgrade());
    shell.is_some_and(|shell| shell.read(cx).task_page_live() == Some(tab))
}

/// A menu item that runs `command`, showing its shortcut.
pub fn menu_item(name: impl Into<SharedString>, command: Command) -> MenuItem {
    MenuItem::action(name, Run(command))
}

impl Shell {
    /// Runs a browser command and logs it. `from` says which side had the key: GPUI's bindings
    /// ("ui"), the browser handing it back ("page"), or the sidebar.
    pub fn run(&mut self, command: Command, from: &str, cx: &mut Context<Self>) {
        // check.sh waits for these lines.
        log!("browser: command {} via {from}", command.name());
        if self.one_space() && !private_runs(command) {
            return;
        }
        // ⌘T in a Little ilo window opens its command bar on its page, as ⌘L does (Arc): it holds
        // one page.
        let command = match command {
            Command::NewTab if self.little.is_some() => Command::EditUrl,
            command => command,
        };
        // Its Spaces menu sends its page to the Space picked, in the main window (Arc).
        if let (Command::MoveToSpace(space), Some(_)) = (command, &self.little) {
            return self.open_in_space(space, cx);
        }
        if self.little.is_some() && !little_runs(command) {
            return;
        }
        let rows = self.sidebar_rows();
        let position = rows
            .iter()
            .position(|&row| row == self.row_of(self.selected));
        let count = rows.len();
        let space = self.spaces.iter().position(|space| space.id == self.space);
        // Next and previous wrap around for tabs, as in Chrome, and stop at the ends for Spaces, as
        // in Arc.
        // Escape puts back a dragged row or dot, or lets chosen rows go, before it stops the page
        // (docs/ux-spec.md › Tab list).
        let put_back = |shell: &mut Self, cx: &mut Context<Self>| {
            shell.cancel_drag(cx) || shell.cancel_space_drag(cx) || shell.unchoose(cx)
        };
        if matches!(command, Command::Stop) && put_back(self, cx) {
            return;
        }
        if self.run_in_task_area(command, from, cx) {
            return;
        }
        // An Esc with nothing to stop closes a Little ilo window (Arc), as no menu's Stop does.
        let key = matches!(from, "ui" | "page");
        if matches!(command, Command::Stop) && key && self.escape_unused(cx) {
            return;
        }
        // The command bar opens over the page, and the reveal's panel makes way for it.
        if matches!(
            command,
            Command::NewTab | Command::SearchTabs | Command::EditUrl | Command::AddSplit
        ) {
            self.set_revealed(false, cx);
            self.read_bar_picks(cx);
        }
        let target = match command {
            Command::NewTab => {
                self.set_new_tab(true, cx);
                self.set_bar_over_card(true, cx);
                let shown = self.shown_urls();
                return self
                    .browser
                    .command_bar(None, shown, self.bar_tabs(cx), None, cx);
            }
            Command::SearchTabs => {
                self.set_new_tab(false, cx);
                self.set_bar_over_card(true, cx);
                return self.browser.search_tabs(self.open_tabs(cx), cx);
            }
            Command::EditUrl => {
                self.set_new_tab(false, cx);
                let tab = self.field_tab();
                let editing = (tab != 0).then(|| (tab, self.url(tab)));
                self.set_bar_over_card(editing.is_none(), cx);
                // A click on the URL field opens the bar over the field, and ⌘L centred (Arc). In a
                // Little ilo window a click on its bar's title opens it compact over the title, and
                // the keys inside the window.
                let field = match self.little_field_bounds(cx) {
                    Some(title) if from == "little bar" => Some(title),
                    Some(_) => {
                        let (bar, shown) = (self.little_bar_bounds(), self.shown_urls());
                        return self.browser.command_bar_in(
                            editing,
                            shown,
                            self.bar_tabs(cx),
                            bar,
                            cx,
                        );
                    }
                    None if from == "sidebar" => self.url_field_bounds(),
                    None => None,
                };
                let shown = self.shown_urls();
                return self
                    .browser
                    .command_bar(editing, shown, self.bar_tabs(cx), field, cx);
            }
            // A Little ilo window's page closes with it (Arc).
            Command::CloseTab if self.little.is_some() => {
                if !self.browser.close_other_window(cx) {
                    self.browser.close_window(cx);
                }
                return;
            }
            Command::CloseTab => {
                // From a window of Chrome's, such as its Task Manager, ⌘W closes that window, as in
                // Chrome, and leaves the tab alone. With no tab selected there is no page to
                // close, so it closes the window, as ⇧⌘W does.
                if !self.browser.close_other_window(cx) {
                    if !self.chosen.is_empty() {
                        self.close_chosen(cx);
                    } else if self.selected != 0 {
                        // A pinned split's pages all go, and it stays a pinned split, as its −
                        // does (Arc 1.171), and so do a favorite split's, its tile kept.
                        let tab = self.selected;
                        let kept =
                            self.pinned().contains(&tab) || self.favorite_split(tab).is_some();
                        let split = kept && self.panes(tab).len() > 1;
                        self.close_tab(tab, split, cx);
                    } else {
                        self.browser.close_window(cx);
                    }
                }
                return;
            }
            Command::NewWindow => return self.browser.new_window(cx),
            Command::NewPrivateWindow => return self.browser.new_private_window(cx),
            // Off in Settings › Features, ⌥⌘N does nothing.
            Command::NewLittleWindow if !self.little_windows => return,
            // And ⌃⇧N, or the keys the user gave either.
            Command::NewNote | Command::NewNoteSplit if !self.new_note => return,
            Command::NewBlankWindow => return self.browser.new_blank_window(cx),
            Command::NewLittleWindow => {
                let home = self.little().unwrap_or(self.space);
                return self.browser.new_little_window(home, cx);
            }
            Command::OpenInMainWindow => return self.open_in_main_window(cx),
            Command::CloseWindow => {
                // As ⌘W does, from a window of Chrome's it closes that one.
                if !self.browser.close_other_window(cx) {
                    self.browser.close_window(cx);
                }
                return;
            }
            Command::TogglePin if !self.chosen.is_empty() => return self.pin_chosen(cx),
            Command::TogglePin => {
                if self.selected != 0 {
                    let pinned = self.pinned().contains(&self.selected);
                    self.pin_tab(self.selected, !pinned, cx);
                }
                return;
            }
            Command::FavoriteTab => {
                return self.favorite_tab(self.selected, usize::MAX, cx);
            }
            Command::DuplicateTab => {
                self.browser.duplicate(self.selected, cx);
                return;
            }
            Command::RenameTab => return self.rename_tab(self.selected, cx),
            Command::NewFolder if !self.chosen.is_empty() => return self.folder_chosen(cx),
            Command::NewFolder => return self.new_folder(cx),
            Command::CollapseFolders => return self.fold_folders(false, cx),
            Command::ExpandFolders => return self.fold_folders(true, cx),
            Command::CollapsePinned => return self.set_pinned_open(false, cx),
            Command::ExpandPinned => return self.set_pinned_open(true, cx),
            Command::RevealTab => return self.reveal_tab(cx),
            Command::FocusSidebar => return self.focus_sidebar(cx),
            // In an agent's Space ⌘↩ is the bar's Take Over or Hand Back.
            Command::BackToPinnedUrl if self.agent_bar().is_some() => {
                return self.agent_bar_primary(cx);
            }
            Command::BackToPinnedUrl => return self.browser.back_to_pinned_url(self.selected, cx),
            Command::ResetSpaceTabs => {
                for tab in self.tabs_to_reset() {
                    self.browser.back_to_pinned_url(tab, cx);
                }
                return;
            }
            Command::ToggleMute => return self.toggle_mute(self.selected, cx),
            Command::MuteAll(muted) => return self.mute_all(muted, cx),
            Command::ToggleFullScreen => return self.browser.toggle_full_screen(cx),
            Command::Exit => return self.browser.toggle_full_screen(cx),
            Command::Clear => return self.clear(cx),
            Command::Undo => {
                self.undo(cx);
                return;
            }
            Command::Redo => {
                self.redo(cx);
                return;
            }
            Command::CopyUrl if !self.chosen.is_empty() => return self.copy_chosen(cx),
            Command::CopyUrl => return self.copy_url(false, cx),
            Command::TurnOffBatterySaver => return self.turn_off_battery_saver(cx),
            Command::TranslatePage => return self.translate_page(cx),
            Command::UseOriginal => return self.use_original(cx),
            Command::CopyMarkdownLink => return self.copy_url(true, cx),
            Command::PasteUrlAsTab => return self.paste_url_as_tab(cx),
            Command::Share => return self.share_page(cx),
            Command::ReopenClosedTab => return self.reopen_closed(cx),
            Command::Back => return self.browser.go_back(self.selected, cx),
            Command::Forward => return self.browser.go_forward(self.selected, cx),
            Command::Reload => return self.browser.reload(self.selected, false, cx),
            Command::ForceReload => return self.browser.reload(self.selected, true, cx),
            Command::ClearCacheAndRefresh => {
                return self.browser.clear_site(self.selected, false, cx);
            }
            Command::ClearCookiesAndRefresh => {
                return self.browser.clear_site(self.selected, true, cx);
            }
            // Escape closes the Library, after a picker over it, which has the keyboard, has
            // closed (the owner, #1074; Arc's doesn't).
            Command::Stop if self.library_open => return self.set_library(false, cx),
            // Escape leaves full screen when there's no load to stop, as the note there says.
            Command::Stop if self.full_screen && !self.page_fullscreen => {
                if !self.page(self.selected).loading {
                    return self.browser.press_window_button(WindowButton::Zoom, cx);
                }
                return self.browser.stop(self.selected, cx);
            }
            Command::Stop => return self.browser.stop(self.selected, cx),
            Command::ActualSize => return self.browser.zoom(self.selected, 0, cx),
            Command::ZoomIn => return self.browser.zoom(self.selected, 1, cx),
            Command::ZoomOut => return self.browser.zoom(self.selected, -1, cx),
            Command::ViewSource => return self.browser.view_source(self.selected, cx),
            Command::SavePage => return self.browser.save_page(self.selected, cx),
            Command::Print => return self.browser.print(self.selected, cx),
            Command::DevTools => return self.browser.devtools(self.selected, 0, cx),
            Command::JavaScriptConsole => return self.browser.devtools(self.selected, 1, cx),
            Command::InspectElements => return self.browser.devtools(self.selected, 2, cx),
            Command::TaskManager => return self.browser.task_manager(cx),
            Command::RecordTrace => {
                trace::toggle(self.browser.clone(), cx);
                return self.update_menus(cx);
            }
            Command::StayOnTop => return self.toggle_on_top(cx),
            Command::MoveSidebar => {
                return self
                    .browser
                    .keep_sidebar_right(!super::sidebar_right(cx), cx);
            }
            Command::ToggleDeveloperMode => {
                let on = !self.page(self.selected).developer_mode;
                return self.browser.set_developer_mode(self.selected, on, cx);
            }
            Command::ManageExtensions => return self.browser.manage_extensions(None, cx),
            Command::AddExtension => return self.browser.add_extension(cx),
            Command::Settings => return self.browser.settings(cx),
            Command::SiteSearchSettings => return self.browser.site_search_settings(cx),
            Command::ClearBrowsingData => return self.browser.clear_browsing_data(cx),
            Command::KeyboardShortcuts => return self.browser.shortcuts(cx),
            Command::About => return self.browser.about(cx),
            Command::CheckForUpdates => return self.browser.check_for_updates(cx),
            Command::Import => return self.browser.import(cx),
            Command::Passwords => return self.browser.passwords(self.space, cx),
            Command::AutoFill | Command::AutoFillPasswords => {
                let logins = command == Command::AutoFillPasswords;
                let shown = self.selected != 0 && self.browser.show_autofill(self.selected, logins);
                if !shown {
                    self.browser.passwords(self.space, cx);
                }
                return;
            }
            Command::Welcome => return self.welcome_again(cx),
            Command::ReportProblem => return self.report_problem(cx),
            Command::CopyBuildInfo => {
                return self.copy(&BuildInfo::here(cx).text(), "Copied Build Info", cx);
            }
            Command::HelpCenter => return self.browser.open_page(build_info::HELP_CENTER, cx),
            Command::WhatsNew => return self.browser.open_page(build_info::WHATS_NEW, cx),
            Command::PrivacyPolicy => {
                return self.browser.open_page(build_info::PRIVACY_POLICY, cx);
            }
            Command::ViewHistory => {
                // Chrome's ⌘Y shows the history page it has open rather than a second one.
                if !self.url(self.selected).starts_with("chrome://history") {
                    self.browser.view_history(cx);
                }
                return;
            }
            Command::Quit => return self.browser.quit(cx),
            Command::AddSplit => return self.add_pane(cx),
            Command::NewNote => return self.browser.new_note(self.space, None, cx),
            Command::NewNoteSplit => {
                let with = self.selected;
                if with != 0 && !self.split_full(with, cx) {
                    self.browser.new_note(self.space, Some(with), cx);
                }
                return;
            }
            Command::CloseSplitPane | Command::SeparatePane => {
                if self.panes(self.selected).len() > 1 {
                    match command {
                        Command::CloseSplitPane => self.close_tab(self.selected, false, cx),
                        _ => self.separate_tab(self.selected, cx),
                    }
                }
                return;
            }
            Command::SeparateSplit => return self.separate_all(self.selected, cx),
            Command::FlipSplit => {
                for (index, &tab) in self.panes(self.selected).iter().rev().enumerate() {
                    self.browser.move_in_split(tab, index, cx);
                }
                return;
            }
            Command::ChangeOrientation => {
                let stacked = self.stacked(self.selected);
                return self.stack_split(self.selected, !stacked, cx);
            }
            Command::ExpandSplit => return self.expand_split(cx),
            Command::MovePaneLeft | Command::MovePaneRight => {
                let panes = self.panes(self.selected);
                let at = panes
                    .iter()
                    .position(|&tab| tab == self.selected)
                    .unwrap_or_default();
                let to = match command {
                    Command::MovePaneLeft => at.checked_sub(1),
                    _ => Some(at + 1).filter(|&to| to < panes.len()),
                };
                if let Some(to) = to {
                    self.browser.move_in_split(self.selected, to, cx);
                }
                return;
            }
            Command::NextPane => return self.focus_pane(true, cx),
            Command::PreviousPane => return self.focus_pane(false, cx),
            Command::FocusPane(index) => return self.focus_pane_at(index, cx),
            Command::ToggleSidebar => return self.toggle_sidebar(cx),
            Command::Appearance(appearance) => return self.set_app_appearance(appearance, cx),
            Command::ViewLibrary => return self.set_library(!self.library_open, cx),
            Command::ViewArchive => return self.view_section(Section::Archive, cx),
            Command::ViewDownloads => return self.view_section(Section::Downloads, cx),
            Command::ViewMedia => return self.view_section(Section::Media, cx),
            Command::ViewAgents => return self.view_section(Section::Agents, cx),
            Command::NewBoost | Command::EditBoost => return self.open_boost(cx),
            Command::ViewBoosts => return self.view_section(Section::Boosts, cx),
            Command::ClearArchive => return self.clear_archive(cx),
            Command::NewSpace => return self.new_space(cx),
            Command::RenameSpace => return self.rename_space(self.space, cx),
            Command::EditTheme => return self.edit_space_theme(self.space, cx),
            Command::ExportSpace => return self.export_space(self.space, cx),
            Command::ChangeProfile(profile) => return self.change_profile(profile, cx),
            Command::RestoreSidebar(time) => return self.restore_sidebar(time, cx),
            Command::RestoreEarlierSidebar => return self.list_sidebar_backups(cx),
            Command::SelectSpace(index) => return self.show_space_at(Some(index), cx),
            // As the chosen rows' menu moves them (#1353).
            Command::MoveToSpace(space) if !self.chosen.is_empty() => {
                if self
                    .spaces_for_chosen()
                    .iter()
                    .any(|&(each, ..)| each == space)
                {
                    self.move_chosen_to_space(space, cx);
                }
                return;
            }
            Command::MoveToSpace(space) | Command::PinToSpace(space) => {
                let pin = matches!(command, Command::PinToSpace(_));
                if self.selected != 0 {
                    self.move_to_space(self.selected, space, pin, cx);
                }
                return;
            }
            Command::OpenFolder(folder) => return self.open_folder(folder, cx),
            Command::MoveToFolder(folder) => {
                let rows = match self.chosen.is_empty() {
                    true if self.selected == 0 => return,
                    true => vec![self.selected],
                    false => self.chosen_in_order(),
                };
                return self.add_to_folder(rows, folder, cx);
            }
            Command::RunExtension(id) => {
                return self.run_extension(&super::extensions::id_text(id), cx);
            }
            Command::NextSpace => return self.show_space_at(space.map(|index| index + 1), cx),
            Command::PreviousSpace => {
                return self.show_space_at(space.and_then(|index| index.checked_sub(1)), cx);
            }
            Command::Find => return self.browser.find(self.selected, cx),
            Command::SiteControls => return self.site_controls(cx),
            Command::Cast => return self.cast(cx),
            Command::CapturePortion => return self.capture(false, cx),
            Command::CaptureFullPage => return self.capture(true, cx),
            Command::CapturePortrait => return self.capture_in_portrait_mode(cx),
            Command::CapturePortraitNow => return self.capture_portrait_now(cx),
            Command::FindNext => return self.browser.find_next(self.selected, true, cx),
            Command::FindPrevious => return self.browser.find_next(self.selected, false, cx),
            Command::JumpToSelection => return self.browser.jump_to_selection(self.selected, cx),
            Command::RecentTab => return self.walk(rows, true, cx),
            Command::RecentTabBack => return self.walk(rows, false, cx),
            Command::NextTab => position.map(|index| (index + 1) % count),
            Command::PreviousTab => position.map(|index| (index + count - 1) % count),
            Command::SelectTab(index) => self.numbered(index, count, cx),
            // The ninth, as ⌘1–⌘8 count, if the user chose it: nothing if there are fewer.
            Command::SelectLastTab if user_keys(cx).last_tab_ninth() => self.numbered(8, count, cx),
            Command::SelectLastTab => count.checked_sub(1),
        };
        if let Some(index) = target {
            self.select(self.row_tab(rows[index]), cx);
        }
    }

    /// The row ⌘ and `index` + 1 goes to, of `count`: past the favorites, which sidebar_rows puts
    /// first, if the user left them out.
    fn numbered(&self, index: usize, count: usize, cx: &App) -> Option<usize> {
        let index = match user_keys(cx).favorites_numbered() {
            true => index,
            false => index + self.favorites.len(),
        };
        (index < count).then_some(index)
    }

    /// ⌃Tab, as in Arc: shows the Space's tab shown before this one at once, with no switcher.
    /// While ⌃ is held, each ⌃Tab goes on to the one shown before that, and ⌃⇧Tab back. A split's
    /// row counts once.
    fn walk(&mut self, rows: Vec<u64>, forward: bool, cx: &mut Context<Self>) {
        // Then the rows not shown yet, in the sidebar's order.
        let mut order: Vec<u64> = Vec::new();
        for row in self
            .recent
            .iter()
            .map(|&tab| self.row_of(tab))
            .chain(rows.iter().copied())
        {
            if rows.contains(&row) && !order.contains(&row) {
                order.push(row);
            }
        }
        let count = order.len();
        let next = match order
            .iter()
            .position(|&row| row == self.row_of(self.selected))
        {
            Some(at) if forward => (at + 1) % count,
            Some(at) => (at + count - 1) % count,
            // With no tab shown, the tab shown last (Arc).
            None if forward && count > 0 => 0,
            None => match count.checked_sub(1) {
                Some(last) => last,
                None => return,
            },
        };
        let tab = self.row_tab(order[next]);
        if tab == self.selected {
            return;
        }
        if self.walked.is_none() {
            self.browser.watch_control(cx);
        }
        self.walked = Some(tab);
        self.select(tab, cx);
    }

    /// Clears the Space shown, but the tab shown, unless that's all there is to clear. Its rows go
    /// in one frame, with no toast (Arc).
    fn clear(&mut self, cx: &mut Context<Self>) {
        let space = self.space;
        // The tasks at the foot are the window's, not the Space's, so they stay (Agent task).
        if let Some(clear) = self.clear_now(cx) {
            self.keep(What::CLEAR, Some(Edit::UndoClear { space, clear }), cx);
        }
    }

    /// Clears, and returns the core's id for the Clear if it closed any.
    pub(super) fn clear_now(&mut self, cx: &mut Context<Self>) -> Option<u64> {
        // A split view shown stays whole.
        let shown = self.panes(self.selected);
        let last = self.clears_last();
        let today = self.today().iter().copied();
        self.clearing = today.filter(|tab| last || !shown.contains(tab)).collect();
        if self.clearing.is_empty() {
            return None;
        }
        let clear = self.browser.clear(cx);
        // check.sh reads this line.
        log!("browser: cleared {}", self.clearing.len());
        (clear != 0).then_some(clear)
    }

    /// Runs `command` for a key the page didn't keep, and returns whether it was used. With
    /// nothing to undo or redo, ⌘Z and ⇧⌘Z go on, for the Edit menu to give the page.
    pub fn run_for_page(&mut self, command: Command, cx: &mut Context<Self>) -> bool {
        self.hide_hover(cx);
        let done = match command {
            Command::Undo => self.undo(cx),
            Command::Redo => self.redo(cx),
            _ => {
                self.run(command, "page", cx);
                return true;
            }
        };
        if done {
            // check.sh waits for this line.
            log!("browser: command {} via page", command.name());
        }
        done
    }

    /// Copies the selected tab's URL, or a Markdown link to it, without its tracking parameters,
    /// and says so in the toast.
    fn copy_url(&mut self, markdown: bool, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        let Some((text, toast)) = self.link_to_copy(tab, markdown) else {
            return;
        };
        match markdown {
            true => self.copy(&text, toast, cx),
            false => self.copy_page_url(tab, text, toast, cx),
        }
    }

    /// Copies `tab`'s link, `url`, and says so in `toast`, which shares the page too where there's
    /// a share sheet, with the capture toast's Share button: ⇧⌘C's and the URL field's button's.
    fn copy_page_url(&mut self, tab: u64, url: String, toast: &str, cx: &mut Context<Self>) {
        if !self.browser.has_share_sheet() {
            return self.copy(&url, toast, cx);
        }
        self.browser.copy(&url, cx);
        // check.sh reads this line.
        log!("browser: copied {url}");
        let (browser, title) = (self.browser.clone(), self.title(tab));
        let share = std::rc::Rc::new(move |at, cx: &mut App| {
            browser.share_from_toast(&url, &title, at, cx)
        });
        self.browser.toast_share(toast, share, COPIED_TIME, cx);
    }

    /// Opens the clipboard's web link at the top of the Today tabs and shows it, as Arc's ⌥⌘V
    /// does. The clipboard is read only now, and text that isn't one http(s) link opens nothing,
    /// as a drop of it doesn't. No agent path: the clipboard is the user's.
    fn paste_url_as_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(urls) = self.pasted_urls(cx) {
            self.browser.open_at(&urls, false, 0, 0, cx);
        }
    }

    /// The clipboard's web link, read now, as one URL to open, or None for anything else.
    pub(super) fn pasted_urls(&self, cx: &App) -> Option<Vec<String>> {
        // From the host, where Copy Link put it: a driven Mac run's pasteboard is its own.
        let text = self.browser.pasted(cx).unwrap_or_default();
        // Text that isn't a web link opens nothing here, not a search, as Arc's ⌥⌘V does.
        let urls = dropped_urls([(text.trim(), false)], |_| None);
        if urls.is_empty() {
            // check.sh reads this line.
            log!("browser: pasted no URL");
        }
        Some(urls).filter(|urls| !urls.is_empty())
    }

    /// File › Share… and the command bar's Share…: the system's share sheet for the page shown,
    /// as Site Controls' Share tile opens it, hanging from the URL field, else from the page's top.
    fn share_page(&mut self, cx: &mut Context<Self>) {
        let link = self.copied_link(&self.url(self.selected));
        if link.is_empty() || !self.browser.has_share_sheet() {
            return;
        }
        let at = match self.url_field_bounds() {
            Some(field) => point(field.center().x, field.bottom()),
            None => {
                let page = self.page_frame(self.viewport).0;
                point(page.center().x, page.top())
            }
        };
        let (tab, title) = (self.selected, self.title(self.selected));
        // After this update, as finding the tab's window reads every Shell (#2459).
        let browser = self.browser.clone();
        cx.defer(move |cx| browser.show_share_sheet(tab, &link, &title, at, cx));
    }

    /// `tab`'s link to copy, as a Markdown link if `markdown`, and the toast to say so with.
    pub(super) fn link_to_copy(&self, tab: u64, markdown: bool) -> Option<(String, &'static str)> {
        let url = self.copied_link(&self.url(tab));
        if url.is_empty() {
            return None;
        }
        let (text, toast) = match markdown {
            false => (url.clone(), "Copied Current URL"),
            true => {
                let title = markdown_text(&self.title(tab));
                (format!("[{title}]({url})"), "Copied URL as Markdown")
            }
        };
        Some((text, toast))
    }

    /// A click on the URL field's Copy URL button: copies with the toast, as Arc's does, and
    /// shows a checkmark in the glyph's place for `COPIED_TIME`.
    pub(super) fn copy_url_from_button(&mut self, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: command copy-url via sidebar");
        let tab = self.field_tab();
        let Some((text, toast)) = self.link_to_copy(tab, false) else {
            return;
        };
        self.copy_page_url(tab, text, toast, cx);
        if self.url_copied.is_none() {
            self.swaps
                .start(Swapped::CopyUrl, LINK, CHECK, Instant::now());
        }
        self.url_copied = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(COPIED_TIME).await;
            shell
                .update(cx, |shell, cx| {
                    shell.url_copied = None;
                    shell
                        .swaps
                        .start(Swapped::CopyUrl, CHECK, LINK, Instant::now());
                    cx.notify();
                })
                .ok();
        }));
        cx.notify();
    }

    /// Report a Problem…: the sheet with the build's details, whose Continue opens a new issue
    /// with them, titled with what the user typed. While another dialog shows, the issue opens
    /// at once, as the user can still read and change all of it in GitHub's form.
    fn report_problem(&mut self, cx: &mut Context<Self>) {
        let info = BuildInfo::here(cx);
        let question = info.report();
        let browser = self.browser.clone();
        let file = Box::new(move |typed: Vec<String>, cx: &mut App| {
            let title = typed.first().map_or("", String::as_str);
            browser.open_page(&info.issue_url(build_info::NEW_ISSUE, title), cx);
        });
        if !self.browser.tell(question, file, cx) {
            let url = BuildInfo::here(cx).issue_url(build_info::NEW_ISSUE, "");
            self.browser.open_page(&url, cx);
        }
    }

    /// Copies `text` and shows `toast`.
    pub(super) fn copy(&mut self, text: &str, toast: &str, cx: &mut Context<Self>) {
        self.browser.copy(text, cx);
        // check.sh reads this line.
        log!("browser: copied {text}");
        self.browser.toast(toast, None, COPIED_TIME, cx);
    }

    /// The tabs the command bar lists before anything is typed, as Arc's does: the favorites and
    /// the Space's pinned and Today tabs, those shown most recently first, then the rest in the
    /// sidebar's order. It leaves out the tab shown and what shows with it, as its suggestions do.
    /// All of them: the bar lists the first BAR_TABS_SHOWN, and looks through the rest for a name
    /// that what's typed starts.
    pub(super) fn bar_tabs(&self, cx: &App) -> Vec<BarTab> {
        let mut tabs = self.favorites.clone();
        if let Some(space) = self.shown_space() {
            tabs.extend(space_tabs(space));
        }
        self.as_bar_tabs(self.recent_first(tabs), cx)
    }

    /// The tabs Search Tabs lists: the command bar's, then the other Spaces' pinned and Today tabs,
    /// those shown most recently first, then the rest in the Spaces' order.
    pub(super) fn open_tabs(&self, cx: &App) -> Vec<BarTab> {
        let others = self.spaces.iter().filter(|space| space.id != self.space);
        let others = self.recent_first(others.flat_map(space_tabs).collect());
        let mut tabs = self.bar_tabs(cx);
        tabs.extend(self.as_bar_tabs(others, cx));
        tabs
    }

    /// `tabs`, those shown most recently first, then the rest in their order.
    fn recent_first(&self, tabs: Vec<u64>) -> Vec<u64> {
        let recent = self.recent.iter().copied().filter(|tab| tabs.contains(tab));
        let mut order: Vec<u64> = recent.collect();
        for tab in tabs {
            if !order.contains(&tab) {
                order.push(tab);
            }
        }
        order
    }

    /// `tabs` as the command bar lists them, without the tab shown and what shows with it, and
    /// without those that have no page yet.
    fn as_bar_tabs(&self, tabs: Vec<u64>, cx: &App) -> Vec<BarTab> {
        let shown = self.panes(self.selected);
        tabs.into_iter()
            .filter(|tab| !shown.contains(tab) && !self.url(*tab).is_empty())
            .map(|tab| BarTab {
                tab,
                title: self.title(tab),
                url: self.url(tab),
                favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
                renamed: !self.browser.tab_name(tab).is_empty(),
                switched: self.switch_score(tab, cx),
            })
            .collect()
    }

    /// The pages shown, the tab's and what shows with it, whose rows the command bar puts last.
    fn shown_urls(&self) -> Vec<SharedString> {
        let pages = self
            .panes(self.selected)
            .into_iter()
            .map(|tab| self.url(tab));
        pages.filter(|url| !url.is_empty()).collect()
    }

    /// Starts capture mode over the focused page, or with `full_page` captures all of it.
    fn capture(&mut self, full_page: bool, cx: &mut Context<Self>) {
        self.capture_pane(self.selected, full_page, None, cx);
    }

    /// Starts capture mode over the page of `tab`, a pane of the tab shown, dragging from `press`
    /// in it for a ⇧⌘-drag on the page (Arc's), else choosing; or with `full_page` captures all of
    /// it. Returns whether `tab`'s page shows here.
    pub fn capture_pane(
        &mut self,
        tab: u64,
        full_page: bool,
        press: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((pane, radius)) = self.pane_frame(tab) else {
            return false;
        };
        let (look, library) = (self.menu_look(), !self.private);
        self.browser
            .capture(tab, pane, radius, full_page, press, library, look, cx);
        true
    }

    /// Where `tab`'s page is in the window, a pane of the tab shown, and its corners' radius; none
    /// if it isn't shown here.
    pub(super) fn pane_frame(&self, tab: u64) -> Option<(Bounds<Pixels>, f32)> {
        if self.selected == 0 {
            return None;
        }
        let (frame, radius) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let index = panes.iter().position(|&pane| pane == tab);
        let index = index.or((tab == self.selected).then_some(0))?;
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        Some((
            pane_frames(frame, panes.len(), stacked, &shares)[index],
            radius,
        ))
    }

    /// Whether `tab`'s page is in Developer Mode, or None where the user can't change it: a page
    /// that isn't http(s), or a private window.
    pub(super) fn developer_mode(&self, tab: u64) -> Option<bool> {
        let page = self.page(tab);
        let web = page.url.starts_with("http://") || page.url.starts_with("https://");
        (tab != 0 && web && !self.private).then_some(page.developer_mode)
    }

    /// Settings › Features' New Note switch: off, the + menu, the command bar and its keys offer
    /// neither New Note nor New Note Split. The tabs it opened stay.
    pub fn set_new_note(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.new_note == on {
            return;
        }
        self.new_note = on;
        self.update_menus(cx);
        cx.notify();
    }

    /// Settings › Privacy's Copy links without tracking: off, every copy path, and the share sheet,
    /// gives the address as it is (#2351).
    pub fn set_clean_links(&mut self, on: bool, cx: &mut Context<Self>) {
        self.clean_links = on;
        cx.notify();
    }

    /// `url` as the copy paths give it: without its trackers, unless the user turned that off.
    pub(super) fn copied_link(&self, url: &str) -> String {
        match self.clean_links {
            true => clean_link(url),
            false => url.to_string(),
        }
    }

    /// Whether `command` would do something now, for the command bar to offer it.
    pub(super) fn offers(&self, command: Command) -> bool {
        if self.one_space() && !private_runs(command) {
            return false;
        }
        if self.little.is_some() && !little_runs(command) {
            return false;
        }
        let page = self.page(self.selected);
        let space = self.spaces.iter().position(|space| space.id == self.space);
        match command {
            Command::Back => page.can_go_back,
            Command::Forward => page.can_go_forward,
            Command::Stop => page.loading,
            // Only a web page has a site to clear, or to show in Developer Mode.
            Command::ClearCacheAndRefresh | Command::ClearCookiesAndRefresh => {
                page.url.starts_with("http://") || page.url.starts_with("https://")
            }
            Command::ToggleDeveloperMode => self.developer_mode(self.selected).is_some(),
            Command::BackToPinnedUrl => page.away_from_pinned_url || self.agent_bar().is_some(),
            Command::CloseSplitPane
            | Command::SeparatePane
            | Command::SeparateSplit
            | Command::FlipSplit
            | Command::ChangeOrientation
            | Command::ExpandSplit => self.panes(self.selected).len() > 1,
            Command::MovePaneLeft => self.panes(self.selected).first() != Some(&self.selected),
            Command::MovePaneRight => self.panes(self.selected).last() != Some(&self.selected),
            Command::CollapseFolders => self.shown_space().is_some_and(|space| {
                space
                    .folders
                    .folders
                    .keys()
                    .any(|&folder| space.is_open(folder))
            }),
            // Only when they'd hide or show a row now (docs/ux-principles.md: a control with nothing
            // to act on is noise).
            Command::CollapsePinned => self
                .shown_space()
                .is_some_and(|space| !space.folders.pinned_shut && space.collapsing_hides()),
            Command::ExpandPinned => self
                .shown_space()
                .is_some_and(|space| space.folders.pinned_shut && space.collapsing_hides()),
            // The Library and a page's full screen keep the sidebar away.
            Command::FocusSidebar => {
                self.library.is_none() && !self.page_fullscreen && !self.sidebar_rows().is_empty()
            }
            Command::ExpandFolders => self.shown_space().is_some_and(|space| {
                space
                    .folders
                    .folders
                    .keys()
                    .any(|&folder| !space.is_open(folder))
            }),
            Command::Clear => !self.today().is_empty(),
            Command::FavoriteTab => {
                let tab = self.selected;
                tab != 0 && !self.favorites.contains(&tab) && self.has_room_for(tab)
            }
            Command::Share => self.browser.has_share_sheet() && !self.url(self.selected).is_empty(),
            // A favorite's menu has no Rename, and the field is in the tab's row in the sidebar.
            Command::RenameTab => {
                let favorite = self.favorites.contains(&self.selected);
                self.selected != 0 && !favorite && !self.sidebar_hidden
            }
            Command::NextSpace => space.is_some_and(|index| index + 1 < self.spaces.len()),
            Command::TurnOffBatterySaver => self.battery_saver.is_some(),
            Command::TranslatePage | Command::UseOriginal => self.offers_translation(command),
            Command::NewBoost | Command::EditBoost | Command::ViewBoosts => {
                self.offers_boost(command)
            }
            Command::MuteAll(muted) => self.tabs.values().any(|tab| tab.page.muted != muted),
            // As the menus' New Profile: only an empty Space changes profile.
            Command::ChangeProfile(None) => {
                self.profile_choices(self.space).is_some_and(|(_, new)| new)
            }
            Command::ToggleFullScreen => true,
            // Only full screen, the page's too, has something to leave.
            Command::Exit => self.full_screen,
            Command::CheckForUpdates => self.browser.updates(),
            Command::RestoreEarlierSidebar => !self.shown_backups().is_empty(),
            Command::PreviousSpace => space.is_some_and(|index| index > 0),
            Command::NextTab | Command::PreviousTab => self.sidebar_rows().len() > 1,
            Command::OpenInMainWindow => self.little.is_some() && self.selected != 0,
            Command::NewLittleWindow => self.little_windows,
            Command::NewNote => self.new_note,
            Command::NewNoteSplit => self.new_note && self.selected != 0,
            Command::NewFolder
            | Command::PasteUrlAsTab
            | Command::SearchTabs
            | Command::ReopenClosedTab
            | Command::NewWindow
            | Command::NewPrivateWindow
            | Command::NewBlankWindow
            | Command::CloseWindow
            | Command::ToggleSidebar
            | Command::NewSpace
            | Command::RenameSpace
            | Command::EditTheme
            | Command::ExportSpace
            | Command::ViewLibrary
            | Command::ViewArchive
            | Command::ClearArchive
            | Command::ViewDownloads
            | Command::ViewMedia
            | Command::ViewAgents
            | Command::ManageExtensions
            | Command::AddExtension
            | Command::ViewHistory
            | Command::SiteSearchSettings
            | Command::ClearBrowsingData
            | Command::KeyboardShortcuts
            | Command::About
            | Command::Import
            | Command::Passwords
            | Command::AutoFill
            | Command::AutoFillPasswords
            | Command::Welcome
            | Command::Appearance(_)
            | Command::RecordTrace
            | Command::StayOnTop
            | Command::MoveSidebar
            | Command::ReportProblem
            | Command::CopyBuildInfo
            | Command::HelpCenter
            | Command::WhatsNew
            | Command::PrivacyPolicy
            | Command::Settings => true,
            _ => self.selected != 0,
        }
    }

    /// The pinned tabs of the Space shown that Reset all tabs in this Space takes back, those away
    /// from their pinned URL. An unloaded one waits until it's shown, as loading it unseen would
    /// cost what showing it does.
    fn tabs_to_reset(&self) -> Vec<u64> {
        let Some(space) = self.shown_space() else {
            return Vec::new();
        };
        let away = |tab: &u64| {
            let page = self.page(*tab);
            page.away_from_pinned_url && !page.unloaded
        };
        space
            .pinned
            .iter()
            .copied()
            .filter(|&row| !space.is_folder(row))
            .filter(away)
            .collect()
    }

    /// What the command bar calls `command`, listed as `name`, if it offers it now: a toggle's row
    /// says what it would do.
    pub(super) fn bar_name(
        &self,
        name: &'static str,
        command: Command,
        cx: &App,
    ) -> Option<&'static str> {
        if !self.offers(command) {
            return None;
        }
        match command {
            Command::TogglePin if self.pinned().contains(&self.selected) => Some("Unpin Tab"),
            // The appearance chosen already would do nothing.
            Command::Appearance(appearance) => (appearance != self.app_appearance).then_some(name),
            Command::ToggleSidebar if name == HIDE_SIDEBAR => {
                (!self.sidebar_hidden).then_some(name)
            }
            Command::ToggleDeveloperMode => {
                let on = self.page(self.selected).developer_mode;
                (name == developer_mode_title(on)).then_some(name)
            }
            Command::ChangeOrientation => {
                (name == orientation_title(self.stacked(self.selected))).then_some(name)
            }
            Command::RecordTrace => (name == trace::title(trace::recording(cx))).then_some(name),
            Command::ToggleFullScreen => {
                let title = match self.full_screen {
                    true => "Exit Full Screen",
                    false => "Enter Full Screen",
                };
                (name == title).then_some(name)
            }
            // The row that changes it, where the window can stay on top at all.
            Command::StayOnTop => (self.browser.can_stay_on_top()
                && name
                    == if self.on_top {
                        DONT_STAY_ON_TOP
                    } else {
                        STAY_ON_TOP
                    })
            .then_some(name),
            Command::CheckForUpdates => (name == crate::updates::command_title(cx)).then_some(name),
            Command::MoveSidebar => {
                let to = match super::sidebar_right(cx) {
                    true => SIDEBAR_TO_LEFT,
                    false => SIDEBAR_TO_RIGHT,
                };
                (name == to).then_some(name)
            }
            _ => Some(name),
        }
    }
}

impl Shell {
    /// The command bar's rows for the Spaces and the folders, each with what it's for and its
    /// command, as Arc's bar has them: Focus for each other Space, then Pin to and Move to for
    /// each other of the user's while a tab is shown, Open for each closed folder of the Space
    /// shown, and Move to for each of its folders but the tab's. With rows chosen, Move to is
    /// theirs, as their menu's, for the Spaces it lists and the folders. A private window has one
    /// Space and no folders.
    pub(super) fn named_bar_rows(&self) -> Vec<NamedRow> {
        if self.one_space() {
            return Vec::new();
        }
        let chosen = !self.chosen.is_empty();
        let tab = !chosen && self.selected != 0 && !self.favorites.contains(&self.selected);
        let for_chosen: Vec<u64> = match chosen {
            true => self
                .spaces_for_chosen()
                .into_iter()
                .map(|(space, ..)| space)
                .collect(),
            false => Vec::new(),
        };
        let mut rows = Vec::new();
        for (index, space) in self.spaces.iter().enumerate() {
            if space.id == self.space {
                continue;
            }
            let name = &space.name;
            let mut row = |verb: &str, command| {
                rows.push((format!("{verb} {name}").into(), vec![name.clone()], command));
            };
            row("Focus", Command::SelectSpace(index));
            if tab && space.agent.is_empty() {
                row("Pin to", Command::PinToSpace(space.id));
                row("Move to", Command::MoveToSpace(space.id));
            } else if for_chosen.contains(&space.id) {
                row("Move to", Command::MoveToSpace(space.id));
            }
        }
        if let Some(space) = self.shown_space() {
            let shut = |row: &&u64| space.is_folder(**row) && !space.is_open(**row);
            for &folder in space.pinned.iter().filter(shut) {
                let name = space.folders.folders[&folder].0.clone();
                let command = Command::OpenFolder(folder);
                rows.push((format!("Open {name}").into(), vec![name], command));
            }
            // Named by its path, as Add to Folder › names it, and found by its own name too.
            let held = |folder: u64| tab && space.folder_of(self.selected) == folder;
            if (tab || chosen) && space.agent.is_empty() {
                for (folder, path) in super::menus::folder_paths(space) {
                    if held(folder) {
                        continue;
                    }
                    let name = space.folders.folders[&folder].0.clone();
                    let command = Command::MoveToFolder(folder);
                    rows.push((format!("Move to {path}").into(), vec![name], command));
                }
            }
        }
        rows
    }

    /// The command bar's Go to Sidebar Item row for the number `typed` has, found by its name or
    /// as Arc's "3. tab" and "3. item". It goes where ⌘3 goes, and 9 to the ninth item, where ⌘9
    /// goes to the last. Only once the number is typed, so "tab" or "item" alone doesn't list
    /// nine, and none past the sidebar's rows.
    pub(super) fn numbered_bar_row(&self, typed: &str, cx: &App) -> Option<NamedRow> {
        let number = typed.chars().find(|c| ('1'..='9').contains(c))?;
        let index = number as usize - '1' as usize;
        self.numbered(index, self.sidebar_rows().len(), cx)?;
        let words = vec![
            format!("{number}. tab").into(),
            format!("{number}. item").into(),
        ];
        Some((
            format!("Go to Sidebar Item {number}").into(),
            words,
            Command::SelectTab(index),
        ))
    }
}

/// `space`'s pinned tabs, without its folders, and its Today tabs, in the sidebar's order.
pub(super) fn space_tabs(space: &Space) -> impl Iterator<Item = u64> + '_ {
    let pinned = space.pinned.iter().filter(|&&row| !space.is_folder(row));
    pinned.chain(&space.today).copied()
}

/// Whether a private window runs `command`. It has one Space, unsaved, so nothing that keeps a tab
/// or shows another Space, and none of what the regular windows keep: the closed tabs, the
/// Library's Spaces and archived tabs, the history. Its Library has its own session's downloads
/// and their media (#669).
pub(super) fn private_runs(command: Command) -> bool {
    !matches!(
        command,
        Command::ReopenClosedTab
            | Command::NewLittleWindow
            | Command::TogglePin
            | Command::FavoriteTab
            | Command::NewFolder
            | Command::CollapseFolders
            | Command::CollapsePinned
            | Command::ExpandPinned
            | Command::ExpandFolders
            | Command::BackToPinnedUrl
            | Command::ResetSpaceTabs
            | Command::NewSpace
            | Command::RenameSpace
            | Command::EditTheme
            | Command::ExportSpace
            | Command::ChangeProfile(_)
            | Command::RestoreSidebar(_)
            | Command::RestoreEarlierSidebar
            | Command::SelectSpace(_)
            | Command::MoveToSpace(_)
            | Command::PinToSpace(_)
            | Command::OpenFolder(_)
            | Command::MoveToFolder(_)
            | Command::NextSpace
            | Command::PreviousSpace
            | Command::ViewArchive
            | Command::ClearArchive
            | Command::ViewAgents
            // It keeps nothing, so it makes no Boost, though it shows its profile's.
            | Command::NewBoost
            | Command::EditBoost
            | Command::ViewBoosts
            | Command::ViewHistory
            | Command::SiteSearchSettings
            | Command::ClearBrowsingData
            | Command::ManageExtensions
            | Command::AddExtension
            | Command::ToggleDeveloperMode
            // The service's document needs the profile's sign-in, which a private window hasn't.
            | Command::NewNote
            | Command::NewNoteSplit
    )
}

/// `text` for a Markdown link's text, with what would end it escaped.
pub(super) fn markdown_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '[' | ']') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::UserKeys;

    /// A key that ran two commands would run the first of them, and the menu would show it for
    /// the other.
    #[test]
    fn no_key_is_bound_twice() {
        for (at, &(key, ..)) in SHORTCUTS.iter().enumerate() {
            assert!(
                SHORTCUTS[..at].iter().all(|&(each, ..)| each != key),
                "{key} is bound twice"
            );
        }
    }

    /// ⌃n shows the nth Space, as the Spaces menu lists them, and the ones past the ninth have
    /// no key.
    #[test]
    fn ctrl_and_a_digit_show_the_spaces_in_order() {
        let keys = UserKeys::default();
        for number in 1..=9 {
            let key = format!("ctrl-{number}");
            let command = keys.command_for(&key, false);
            assert_eq!(command, Some(Command::SelectSpace(number - 1)));
        }
        assert_eq!(keys.label(Command::SelectSpace(0)).as_deref(), Some("⌃1"));
        assert_eq!(keys.label(Command::SelectSpace(9)), None);
    }

    /// Each key chip that spells a chord after its words spells it as a menu does: Stop's and Take
    /// Over's in the conversation and under a task's page, and the command bar's Run and Ask at its
    /// input's end. Off the Mac they read Ctrl+. and Ctrl+Enter, not Ctrl. and CtrlEnter, and Enter
    /// and Tab in the words the UI's font has.
    #[test]
    fn key_chips_spell_their_chords_as_the_menus_do() {
        let stop = "cmd-.";
        let take_over = "cmd-enter";
        assert!(SHORTCUTS.contains(&(take_over, Command::BackToPinnedUrl, false)));
        assert_eq!(keystroke_label_on(stop, true), "Ctrl+.");
        assert_eq!(keystroke_label_on(take_over, true), "Ctrl+Enter");
        assert_eq!(keystroke_label_on(stop, false), "⌘.");
        assert_eq!(keystroke_label_on(take_over, false), "⌘↩");
        assert_eq!(super::super::agent_bar::stop_keys(), keystroke_label(stop));
        for (key, mac, off) in [("enter", "↩", "Enter"), ("tab", "⇥", "Tab")] {
            assert_eq!(key_name(key, false), mac);
            assert_eq!(key_name(key, true), off);
        }
    }

    /// Off the Mac, keys are named in Chrome's words, as the UI's font there has no keyboard
    /// symbols; the Mac keeps its symbols (#772).
    #[test]
    fn keys_are_named_in_words_off_the_mac() {
        assert_eq!(keys_named("shift-cmd-k", false), ["⇧", "⌘", "K"]);
        assert_eq!(keys_named("ctrl-shift-k", true), ["Ctrl", "Shift", "K"]);
        assert_eq!(keys_named("escape", false), ["⎋"]);
        assert_eq!(keys_named("escape", true), ["Esc"]);
        assert_eq!(keys_named("alt-left", true), ["Alt", "Left Arrow"]);
        assert_eq!(
            keys_named("ctrl-alt-backspace", true),
            ["Ctrl", "Alt", "Backspace"]
        );
        assert_eq!(keys_named("ctrl-pagedown", true), ["Ctrl", "Page Down"]);
        assert_eq!(key_name("enter", false), "↩");
        assert_eq!(key_name("enter", true), "Enter");
        assert_eq!(key_name("delete", true), "Del");
    }

    /// ⌘Y is a page's to keep, as Chrome's is, unlike the reserved keys.
    #[test]
    fn history_is_not_a_reserved_key() {
        let keys = UserKeys::default();
        assert_eq!(keys.command_for("cmd-y", false), Some(Command::ViewHistory));
        assert_eq!(keys.command_for("cmd-y", true), None);
    }

    /// ⌘N, ⇧⌘N and ⇧⌘W reach the browser before the page, as Chrome's do, so a page can't keep
    /// them.
    #[test]
    fn window_keys_are_reserved() {
        let keys = UserKeys::default();
        assert_eq!(keys.command_for("cmd-n", true), Some(Command::NewWindow));
        assert_eq!(
            keys.command_for("shift-cmd-n", true),
            Some(Command::NewPrivateWindow)
        );
        assert_eq!(
            keys.command_for("shift-cmd-w", true),
            Some(Command::CloseWindow)
        );
    }

    /// ⇧⌘A is a page's to keep, as Chrome's Search Tabs is, and the keys typed right after it wait
    /// for the bar it opens.
    #[test]
    fn search_tabs_is_not_a_reserved_key() {
        let keys = UserKeys::default();
        assert_eq!(
            keys.command_for("shift-cmd-a", false),
            Some(Command::SearchTabs)
        );
        assert_eq!(keys.command_for("shift-cmd-a", true), None);
        assert!(keys.holds_keys("shift-cmd-a"));
        assert_eq!(keys.label(Command::SearchTabs).as_deref(), Some("⇧⌘A"));
    }

    /// ⇧⌘A with the UI focused lists the open tabs but the one shown: the Space shown's first, as
    /// the command bar lists them, then the other Space's, the one shown last there first.
    #[gpui::test]
    fn search_tabs_lists_the_open_tabs_the_space_shown_first(cx: &mut gpui::TestAppContext) {
        use std::rc::Rc;

        use gpui::VisualTestContext;

        use crate::shell::{Folders, Page, recorder::Recorder};

        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(bind_keys);
        let (shell, cx): (gpui::Entity<Shell>, &mut VisualTestContext) =
            cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space =
                |id, name: &str| (id, name.into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space(1, "One"), space(2, "Two")], cx);
            shell.set_tabs(
                1,
                vec![10],
                vec![11, 12],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(
                2,
                Vec::new(),
                vec![20, 21],
                Vec::new(),
                Folders::default(),
                cx,
            );
            for tab in [10, 11, 12, 20, 21] {
                let page = Page {
                    url: format!("https://{tab}.test/").into(),
                    ..Page::default()
                };
                shell.set_page(tab, page, cx);
            }
            shell.set_space(2, cx);
            shell.set_selected(21, cx);
            shell.set_space(1, cx);
            shell.set_selected(12, cx);
            shell.set_selected(11, cx);
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.simulate_keystrokes("shift-cmd-a");
        assert_eq!(host.searched_tabs.take(), [vec![12, 10, 21, 20]]);
        assert!(
            host.command_bars.take().is_empty(),
            "it opened the bar for a new tab too"
        );
    }

    /// Reset all tabs in this Space takes back the pinned tabs away from their pinned URL, in a
    /// folder too, but neither an unloaded one, which waits to be shown, nor a Today tab.
    #[gpui::test]
    fn reset_all_tabs_takes_back_the_loaded_pinned_tabs(cx: &mut gpui::TestAppContext) {
        use std::{collections::HashMap, rc::Rc};

        use gpui::VisualTestContext;

        use crate::shell::{Folders, Page, recorder::Recorder};

        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx): (gpui::Entity<Shell>, &mut VisualTestContext) =
            cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "One".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            let folders = Folders {
                folders: HashMap::from([(5, ("Folder".into(), false))]),
                parents: HashMap::from([(4, 5)]),
                ..Default::default()
            };
            shell.set_tabs(1, vec![2, 3, 5, 4, 6], vec![7], Vec::new(), folders, cx);
            for tab in [2, 3, 4, 7] {
                let unloaded = tab == 3;
                let page = Page {
                    away_from_pinned_url: true,
                    unloaded,
                    ..Page::default()
                };
                shell.set_page(tab, page, cx);
            }
            shell.set_space(1, cx);
            shell.set_selected(6, cx);
            host.take();
            shell.run(Command::ResetSpaceTabs, "ui", cx);
        });
        assert_eq!(
            host.take(),
            ["back to pinned url 2", "back to pinned url 4"]
        );
    }

    /// Another Space shown while the command bar is open gives the bar that Space's tabs once its
    /// tab is selected, but the one shown, and New Tab keeps its plate there for the bar's pick.
    /// With the bar closed, the bar hears nothing.
    #[gpui::test]
    fn the_open_command_bar_follows_another_space(cx: &mut gpui::TestAppContext) {
        use std::rc::Rc;

        use crate::shell::{Folders, Page, recorder::Recorder};

        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space =
                |id, name: &str| (id, name.into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space(1, "One"), space(2, "Two")], cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![11, 12],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(
                2,
                Vec::new(),
                vec![21, 22],
                Vec::new(),
                Folders::default(),
                cx,
            );
            for tab in [11, 12, 21, 22] {
                let page = Page {
                    url: format!("https://{tab}.test/").into(),
                    ..Page::default()
                };
                shell.set_page(tab, page, cx);
            }
            shell.set_space(1, cx);
            shell.set_selected(11, cx);
            shell.run(Command::NewTab, "ui", cx);
            shell.set_space(2, cx);
            shell.set_selected(21, cx);
        });
        assert_eq!(host.followed_tabs.take(), [vec![22]]);
        assert!(
            shell.read_with(cx, |shell, _| shell.new_tab),
            "New Tab keeps its plate"
        );
        // A tab selected in the same Space isn't another Space.
        shell.update(cx, |shell, cx| shell.set_selected(22, cx));
        assert!(host.followed_tabs.take().is_empty());

        host.bar_open.set(false);
        shell.update(cx, |shell, cx| {
            shell.set_space(1, cx);
            shell.set_selected(11, cx);
        });
        assert!(host.followed_tabs.take().is_empty());
        assert!(!shell.read_with(cx, |shell, _| shell.new_tab));
    }

    /// The bar finds Site Search Settings by its name or by Chrome's, and it opens the search
    /// engines page. A private window keeps no site searches of its own, so it offers none.
    #[gpui::test]
    fn site_search_settings_open_from_the_bar_but_not_in_a_private_window(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::rc::Rc;

        use gpui::AppContext as _;

        use crate::shell::{Browser, recorder::Recorder};

        let host = Rc::new(Recorder::default());
        let browser: Rc<dyn Browser> = host.clone();
        let regular = cx.new(|cx| Shell::new(browser.clone(), cx));
        let private = cx.new(|cx| Shell::private_window(browser, 1, "Incognito".into(), 1, cx));
        let found = |shell: &gpui::Entity<Shell>, typed: &str, cx: &mut gpui::TestAppContext| {
            shell.read_with(cx, |shell, cx| {
                find_commands(typed, &UserKeys::default(), |name, command| {
                    shell.bar_name(name, command, cx)
                })
                .into_iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>()
            })
        };
        for typed in ["site search", "search engines", "keywords"] {
            let name = "Site Search Settings".to_string();
            assert_eq!(found(&regular, typed, cx).first(), Some(&name), "{typed}");
            assert!(!found(&private, typed, cx).contains(&name), "{typed}");
        }

        regular.update(cx, |shell, cx| {
            shell.run(Command::SiteSearchSettings, "bar", cx)
        });
        assert_eq!(host.site_search_settings.get(), 1);
        private.update(cx, |shell, cx| {
            shell.run(Command::SiteSearchSettings, "bar", cx)
        });
        assert_eq!(
            host.site_search_settings.get(),
            1,
            "a private window opened it"
        );
    }

    /// ⇧⌘⌫ and the bar open Chrome's Clear browsing data dialog for the Space's profile, as Dia's
    /// ⇧⌘⌫ does. A page may keep the key, as Chrome's isn't reserved. A private window's session
    /// keeps nothing once it ends, so it offers none.
    #[gpui::test]
    fn clear_browsing_data_opens_from_its_key_and_the_bar_but_not_in_a_private_window(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::rc::Rc;

        use gpui::AppContext as _;

        use crate::shell::{Browser, recorder::Recorder};

        let keys = UserKeys::default();
        let key = "shift-cmd-backspace";
        assert_eq!(
            keys.command_for(key, false),
            Some(Command::ClearBrowsingData)
        );
        assert_eq!(keys.command_for(key, true), None);
        assert_eq!(
            keys.label(Command::ClearBrowsingData).as_deref(),
            Some("⇧⌘⌫")
        );

        let host = Rc::new(Recorder::default());
        let browser: Rc<dyn Browser> = host.clone();
        let regular = cx.new(|cx| Shell::new(browser.clone(), cx));
        let private = cx.new(|cx| Shell::private_window(browser, 1, "Incognito".into(), 1, cx));
        let found = |shell: &gpui::Entity<Shell>, typed: &str, cx: &mut gpui::TestAppContext| {
            shell.read_with(cx, |shell, cx| {
                find_commands(typed, &UserKeys::default(), |name, command| {
                    shell.bar_name(name, command, cx)
                })
                .into_iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>()
            })
        };
        let name = "Clear Browsing Data…".to_string();
        for typed in ["clear browsing", "cookies"] {
            assert!(found(&regular, typed, cx).contains(&name), "{typed}");
            assert!(!found(&private, typed, cx).contains(&name), "{typed}");
        }

        regular.update(cx, |shell, cx| {
            shell.run(Command::ClearBrowsingData, "key", cx)
        });
        assert_eq!(host.clear_browsing_data.get(), 1);
        private.update(cx, |shell, cx| {
            shell.run(Command::ClearBrowsingData, "key", cx)
        });
        assert_eq!(
            host.clear_browsing_data.get(),
            1,
            "a private window opened it"
        );
    }

    /// ⌥⌘V opens the clipboard's web link as a tab at the top of Today, and text that isn't one
    /// opens nothing, as a drop of it doesn't. The clipboard is the host's, where Copy Link puts
    /// it, as a driven Mac run's pasteboard of its own isn't GPUI's.
    #[gpui::test]
    fn paste_url_as_new_tab_opens_only_a_web_link(cx: &mut gpui::TestAppContext) {
        let shell_with = crate::shell::drag::detents::shell_with;
        let (shell, host) = shell_with(cx, &[], &[2], Default::default());
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            "https://example.org/gpui".into(),
        ));
        let paste = |text: &str, cx: &mut gpui::TestAppContext| {
            host.copied.borrow_mut().push(text.into());
            host.take();
            shell.update(cx, |shell, cx| {
                shell.run(Command::PasteUrlAsTab, "page", cx)
            });
            host.take()
                .into_iter()
                .filter(|call| call.starts_with("open "))
                .collect::<Vec<_>>()
        };
        let opened = paste(" https://example.com/a?b \n", cx);
        assert_eq!(opened, ["open https://example.com/a?b false 0 0"]);
        for text in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "some words",
            "",
        ] {
            assert!(paste(text, cx).is_empty(), "{text:?} opened a tab");
        }
        let keys = UserKeys::default();
        assert_eq!(
            keys.command_for("alt-cmd-v", false),
            Some(Command::PasteUrlAsTab)
        );
    }

    /// The command bar offers to turn Developer Mode on for a web page and off for one in it, and
    /// neither for a page with no site or in a private window, which can't change it.
    #[gpui::test]
    fn developer_mode_s_row_says_what_it_would_do(cx: &mut gpui::TestAppContext) {
        use std::rc::Rc;

        use gpui::AppContext;

        use crate::shell::{Folders, Page, recorder::Recorder};

        let listed = |shell: &Shell, cx: &App| -> Vec<&str> {
            let named = |name, command| shell.bar_name(name, command, cx);
            (find_commands("developer mode", &UserKeys::default(), named).into_iter())
                .filter(|found| found.command == Command::ToggleDeveloperMode)
                .map(|found| {
                    if found.name == DEVELOPER_MODE_ON {
                        "on"
                    } else {
                        "off"
                    }
                })
                .collect()
        };
        let page = |url: &str, developer_mode| Page {
            url: url.to_string().into(),
            developer_mode,
            ..Page::default()
        };
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(browser, cx);
            let space = (1, "One".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_page(10, page("https://example.com/", false), cx);
            shell.set_selected(10, cx);
            shell
        });
        shell.update(cx, |shell, cx| {
            assert_eq!(listed(shell, cx), ["on"]);
            shell.run(Command::ToggleDeveloperMode, "bar", cx);
            shell.set_page(10, page("http://localhost:3000/", true), cx);
            assert_eq!(listed(shell, cx), ["off"]);
            shell.run(Command::ToggleDeveloperMode, "bar", cx);
            shell.set_page(10, page("chrome://settings/", false), cx);
            assert!(listed(shell, cx).is_empty());
        });
        let calls = host.take();
        let changes: Vec<&String> = calls
            .iter()
            .filter(|call| call.contains("developer"))
            .collect();
        assert_eq!(
            changes,
            ["developer mode 10 true", "developer mode 10 false"]
        );

        let browser = host.clone();
        let private = cx.new(|cx| {
            let mut shell = Shell::private_window(browser, 1, "Private".into(), 0, cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_page(10, page("http://localhost:3000/", true), cx);
            shell.set_selected(10, cx);
            shell
        });
        private.update(cx, |shell, cx| assert!(listed(shell, cx).is_empty()));
    }

    /// Record a Trace starts one, and while it records the command bar and Help › Troubleshooting
    /// offer Stop Recording Trace in its place, which stops it.
    #[gpui::test]
    fn record_a_trace_s_row_says_what_it_would_do(cx: &mut gpui::TestAppContext) {
        use std::rc::Rc;

        use gpui::AppContext;

        use crate::shell::recorder::Recorder;

        let listed = |shell: &Shell, cx: &App| -> Vec<&str> {
            let named = |name, command| shell.bar_name(name, command, cx);
            (find_commands("trace", &UserKeys::default(), named).into_iter())
                .filter(|found| found.command == Command::RecordTrace)
                .map(|found| {
                    if found.name == trace::RECORD {
                        "record"
                    } else {
                        "stop"
                    }
                })
                .collect()
        };
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let shell = cx.new(|cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            assert_eq!(listed(shell, cx), ["record"]);
            shell.run(Command::RecordTrace, "bar", cx);
            assert_eq!(listed(shell, cx), ["stop"]);
        });
        assert_eq!(host.menu_bar.borrow().trace_title(), trace::STOP_RECORDING);
        shell.update(cx, |shell, cx| {
            shell.run(Command::RecordTrace, "ui", cx);
            assert_eq!(listed(shell, cx), ["record"]);
        });
        assert_eq!(host.menu_bar.borrow().trace_title(), trace::RECORD);
        let calls = host.take();
        let traced: Vec<&String> = calls.iter().filter(|call| call.contains("trace")).collect();
        assert_eq!(
            traced,
            [
                "start trace",
                "trace note shown",
                "trace note hidden",
                "stop trace"
            ]
        );
    }

    /// ⌃⇧N opens a new note in the Space's profile's service, New Note Split opens it beside the
    /// tab shown, and neither runs in a private window, which has no sign-ins.
    #[gpui::test]
    fn new_note_opens_in_the_spaces_service_and_new_note_split_beside_the_tab(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::rc::Rc;

        use gpui::AppContext as _;

        use crate::shell::{Browser, Folders, recorder::Recorder};

        assert_eq!(
            UserKeys::default().command_for("ctrl-shift-n", false),
            Some(Command::NewNote)
        );
        let host = Rc::new(Recorder::default());
        let browser: Rc<dyn Browser> = host.clone();
        let regular = cx.new(|cx| Shell::new(browser.clone(), cx));
        regular.update(cx, |shell, cx| {
            let space = (2, "Work".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(2, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(2, cx);
            shell.set_selected(10, cx);
            shell.run(Command::NewNote, "keys", cx);
            shell.run(Command::NewNoteSplit, "command bar", cx);
        });
        assert_eq!(*host.notes.borrow(), [(2, None), (2, Some(10))]);

        let private = cx.new(|cx| Shell::private_window(browser, 1, "Incognito".into(), 1, cx));
        private.update(cx, |shell, cx| shell.run(Command::NewNote, "keys", cx));
        assert_eq!(
            host.notes.borrow().len(),
            2,
            "a private window opened a note"
        );
    }

    /// Off in Settings › Features, nothing offers or runs New Note or New Note Split: not the +
    /// menu, the command bar or ⌃⇧N. On again, ⌃⇧N opens one.
    #[gpui::test]
    fn new_note_off_is_in_no_menu_bar_or_key(cx: &mut gpui::TestAppContext) {
        use gpui::{point, px};

        let (shell, host) = shell_with_today(cx, vec![10]);
        shell.update(cx, |shell, cx| {
            shell.set_selected(10, cx);
            shell.set_new_note(false, cx);
            shell.plus_menu(point(px(100.), px(700.)), cx);
        });
        let titles: Vec<String> = host
            .menus
            .borrow()
            .last()
            .unwrap()
            .iter()
            .map(|(title, _)| title.to_string())
            .collect();
        assert!(titles.contains(&"New Split View".to_string()), "{titles:?}");
        assert!(!titles.contains(&"New Note".to_string()), "{titles:?}");
        shell.read_with(cx, |shell, cx| {
            assert!(!shell.offers(Command::NewNote));
            assert!(!shell.offers(Command::NewNoteSplit));
            assert_eq!(shell.bar_name("New Note", Command::NewNote, cx), None);
        });
        shell.update(cx, |shell, cx| {
            shell.run(Command::NewNote, "keys", cx);
            shell.run(Command::NewNoteSplit, "command bar", cx);
        });
        assert!(
            host.notes.borrow().is_empty(),
            "New Note ran while it's off"
        );
        shell.update(cx, |shell, cx| {
            shell.set_new_note(true, cx);
            shell.run(Command::NewNote, "keys", cx);
        });
        assert_eq!(
            host.notes.borrow().len(),
            1,
            "New Note didn't run once on again"
        );
    }

    /// A shell with one Space and Today tabs `tabs`, on a host that records what it asks.
    fn shell_with_today(
        cx: &mut gpui::TestAppContext,
        tabs: Vec<u64>,
    ) -> (
        gpui::Entity<Shell>,
        std::rc::Rc<super::super::recorder::Recorder>,
    ) {
        use gpui::AppContext;
        let host = std::rc::Rc::new(super::super::recorder::Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), tabs, Vec::new(), Default::default(), cx);
            shell.set_space(1, cx);
        });
        (shell, host)
    }

    /// A shortcut acts on the window just made key, though the shell hears of the change only
    /// once AppKit's update that made it is over: a click into another window and ⌘2 right away
    /// show that window's second tab, not the one behind's.
    #[gpui::test]
    fn a_shortcut_acts_on_the_window_just_made_key(cx: &mut gpui::TestAppContext) {
        cx.update(bind_keys);
        let (first, first_host) = shell_with_today(cx, vec![11, 12]);
        // Made last, it's the one the shortcuts act on, as a window just opened is.
        let (_second, second_host) = shell_with_today(cx, vec![21, 22]);
        cx.run_until_parked();
        first_host.take();
        second_host.take();

        // As the host's key change does, and the press comes before the executor's next turn.
        let later = window_became_key(first.downgrade());
        cx.update(|cx| cx.dispatch_action(&Run(Command::SelectTab(1))));
        let asked = |host: &super::super::recorder::Recorder| -> Vec<String> {
            let calls = host.take();
            calls
                .into_iter()
                .filter(|call| call.starts_with("select "))
                .collect()
        };
        assert_eq!(
            asked(&first_host),
            ["select 12"],
            "the window made key showed its tab"
        );
        assert!(
            asked(&second_host).is_empty(),
            "the window behind showed nothing"
        );

        cx.update(later);
        cx.update(|cx| cx.dispatch_action(&Run(Command::SelectTab(0))));
        assert_eq!(
            asked(&first_host),
            ["select 11"],
            "and still does once the shell has heard"
        );
    }

    /// A window opened in front keeps the shortcuts when the window made key before it, as one
    /// closed, hears only after it opened, as macOS reports no key change for the window opened
    /// in front: ⌘N right after ⇧⌘W, then ⇧⌘W, closes the new window, not the one behind (#2648).
    #[gpui::test]
    fn a_window_opened_in_front_keeps_the_shortcuts_from_one_made_key_before(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(bind_keys);
        let (behind, behind_host) = shell_with_today(cx, vec![11]);
        cx.run_until_parked();

        // A window closed and left this one key, and its shell hears once the executor turns.
        let behind_heard = window_became_key(behind.downgrade());
        // ⌘N's window opens before that turn, and is key from when it opens, as the app makes it.
        let (opened, opened_host) = shell_with_today(cx, vec![21]);
        let opened_heard = window_became_key(opened.downgrade());
        cx.update(behind_heard);
        behind_host.take();
        opened_host.take();
        let closed = |host: &super::super::recorder::Recorder| {
            host.take().iter().any(|call| call == "close window")
        };

        cx.update(|cx| cx.dispatch_action(&Run(Command::CloseWindow)));
        assert!(closed(&opened_host), "⇧⌘W closed the window opened");
        assert!(!closed(&behind_host), "and not the one behind");

        cx.update(opened_heard);
        cx.update(|cx| cx.dispatch_action(&Run(Command::CloseWindow)));
        assert!(
            closed(&opened_host),
            "and still does once the window opened has heard"
        );
        assert!(!closed(&behind_host), "the one behind still isn't closed");
    }
}
