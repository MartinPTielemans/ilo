//! The Settings window: Arc's, a standard window of its own titled with the tab it shows, with the
//! tabs across its toolbar and each tab's settings in boxes of rows under them (docs/ux-spec.md ›
//! Settings). It opens on ⌘, and hides when closed, keeping its place and its tab until the
//! browser quits.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use browser::account::{self, Account};
use browser::agents::{self, Agents, Client};
use browser::build_info;
use browser::default_browser::{self, DefaultBrowser, Os};
use browser::dialog;
use browser::theme::Appearance;
use browser::import::ImportView;
use browser::link_rules::LinkRulesView;
use browser::log;
use browser::notes;
use browser::passwords::{self, Login};
use browser::shell::{self, CancelName, Command, Holder, KeepName, MenuEntry, Refused, UserKeys};
use browser::text_input::{self, TextInput};
use browser::widevine::{self, Widevine, WidevineRow};
use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontWeight,
    Global, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, PathPromptOptions, Rgba, SharedString, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions, actions,
    canvas, div, point, prelude::*, px, rgb, rgba, size, svg,
};

use crate::{bc, ns_view, platform, show_menu};
use browser::menu::Look;
use browser::palette::{Palette, STATE_HOVERED, STATE_PRESSED, state_alpha};
use browser::settings_rows::{
    Look as RowLook, SLIDE, button, group, popup, row, switch, tile,
};

actions!(settings, [CloseSettings]);

use browser::tokens::settings::WIDTH;
use browser::tokens::settings::CONTROL;
use browser::tokens::settings::CONTROL_RADIUS;
use browser::tokens::settings::HEIGHT;
use browser::tokens::settings::TITLE_BAR;
use browser::tokens::shell::WINDOW_BUTTON_GLYPH;
use browser::tokens::shell::WINDOW_BUTTON_SIZE;
use browser::tokens::shell::WINDOWS_BUTTON_GLYPH;
use browser::tokens::shell::WINDOWS_BUTTON_PLATE;
use browser::tokens::shell::WINDOWS_CLOSE_HOVERED;
use browser::tokens::settings::TOOLBAR;
use browser::tokens::settings::TAB;
use browser::tokens::settings::TAB_PADDING;
use browser::tokens::settings::PLATE_TOP;
use browser::tokens::settings::PLATE;
use browser::tokens::settings::ICON;
use browser::tokens::settings::ICON_TOP;
use browser::tokens::settings::LABEL_TOP;
use browser::tokens::settings::SIDE;
use browser::tokens::settings::FIRST;
use browser::tokens::settings::BOX_GAP;
use browser::tokens::settings::PADDING;
use browser::tokens::settings::RADIUS;
use browser::tokens::settings::CONTROL_HEIGHT;
use browser::tokens::settings::CONTROL_PADDING;
use browser::tokens::settings::POPUP_WIDTH;
use browser::tokens::settings::FAVORITES_POPUP_WIDTH;
use browser::tokens::settings::POPUP_END;
use browser::tokens::settings::BUTTONS_TOP;
use browser::tokens::settings::HEADER_GAP;
use browser::tokens::settings::SECTION_GAP;
use browser::tokens::settings::CHIP_HEIGHT;
use browser::tokens::settings::CHIP_PADDING;
use browser::tokens::settings::CHIP_WIDTH;
use browser::tokens::settings::CHIP_GAP;
use browser::tokens::settings::PROFILE_ROW;
use browser::tokens::settings::USERNAME_SIZE;

use browser::tokens::settings::BAR;
use browser::tokens::settings::BAR_LINE;
use browser::tokens::settings::BACKGROUND;
use browser::tokens::settings::PLATE_FILL;
use browser::tokens::settings::PLATE_EDGE;
use browser::tokens::settings::TITLE;
use browser::tokens::settings::TAB_TEXT;
use browser::tokens::settings::TEXT;
use browser::tokens::settings::HAPTICS_TILE;
use browser::tokens::settings::LINK_PREVIEWS_TILE;
use browser::tokens::settings::MINI_PLAYER_TILE;
use browser::tokens::settings::OPTION_REVEAL_TILE;
use browser::tokens::settings::RESTORE_WINDOWS_TILE;
use browser::tokens::settings::PROFILE_TILE;
use browser::tokens::settings::PASSWORD_TILE;
use browser::tokens::settings::AGENTS_TILE;
use browser::tokens::settings::CLIENT_TILE;
use browser::tokens::settings::EXPORT_TILE;
use browser::tokens::settings::DETAIL;
use browser::tokens::settings::CHIP;
/// The chip's plate, as Paper's alpha, for the ladder to go over.
const CHIP_ALPHA: f32 = (CHIP & 0xff) as f32 / 255.;
use browser::tokens::settings::CHIP_RADIUS;
use browser::tokens::settings::RESTORE_RADIUS;
use browser::tokens::text;

/// What Arc offers for how long a Today tab waits unseen before it's archived, in seconds.
const ARCHIVE_AFTER: [(&str, f64); 4] = [
    ("12 hours (default)", 12. * 3600.),
    ("24 hours", 24. * 3600.),
    ("7 days", 7. * 24. * 3600.),
    ("30 days", 30. * 24. * 3600.),
];

/// How long a Today tab waits unseen before it's archived, in words: "12 hours".
pub(crate) fn archive_after_words() -> String {
    let after = unsafe { bc::bc_archive_after() };
    match ARCHIVE_AFTER.iter().find(|&&(_, seconds)| seconds == after) {
        Some((label, _)) => label.trim_end_matches(" (default)").to_owned(),
        None => format!("{:.0} hours", after / 3600.),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    Links,
    Sync,
    Profiles,
    Passwords,
    Import,
    Shortcuts,
    Privacy,
    Agents,
    Advanced,
    About,
}

impl Tab {
    const ALL: [Tab; 11] = [
        Tab::General,
        Tab::Links,
        Tab::Sync,
        Tab::Profiles,
        Tab::Passwords,
        Tab::Import,
        Tab::Shortcuts,
        Tab::Privacy,
        Tab::Agents,
        Tab::Advanced,
        Tab::About,
    ];

    /// The tabs this build has: Privacy only holds crash reports, which a build without a collector
    /// never sends.
    fn shown() -> impl Iterator<Item = Tab> {
        let collector = !crash_reports_collector().is_empty();
        Tab::ALL.into_iter().filter(move |&tab| tab != Tab::Privacy || collector)
    }

    fn name(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Links => "Links",
            Tab::Sync => "Sync",
            Tab::Profiles => "Profiles",
            Tab::Passwords => "Passwords",
            Tab::Import => "Import",
            Tab::Shortcuts => "Shortcuts",
            Tab::Privacy => "Privacy",
            Tab::Agents => "Agents",
            Tab::Advanced => "Advanced",
            Tab::About => "About",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tab::General => "icons/settings.svg",
            Tab::Links => "icons/link.svg",
            Tab::Sync => "icons/cloud.svg",
            Tab::Profiles => "icons/user.svg",
            Tab::Passwords => "icons/key-round.svg",
            Tab::Import => "icons/arrow-down-to-line.svg",
            Tab::Shortcuts => "icons/keyboard.svg",
            Tab::Privacy => "icons/lock.svg",
            Tab::Agents => "icons/sparkles.svg",
            Tab::Advanced => "icons/sliders-horizontal.svg",
            Tab::About => "icons/info.svg",
        }
    }
}

/// The keys a row of the Shortcuts tab shows: a command's shortcut, or keys the app binds itself.
/// Or the switch for whether those keys count the favorites, or whether Last Tab is the ninth.
enum Keys {
    Of(Command),
    /// Keys no command holds, which can't be changed: a binding's keystroke.
    Fixed(&'static str),
    /// A run of them, from the first keystroke to the last.
    Span(&'static str, &'static str),
    FavoritesNumbered,
    LastTabNinth,
}

impl Keys {
    /// A row's keys that can't be changed, as a menu shows them, Ctrl+1 – Ctrl+8 off the Mac.
    fn fixed(&self) -> Option<String> {
        match *self {
            Keys::Of(_) | Keys::FavoritesNumbered | Keys::LastTabNinth => None,
            Keys::Fixed(keys) => Some(shell::keystroke_label(keys)),
            Keys::Span(first, last) => {
                Some(format!("{} – {}", shell::keystroke_label(first), shell::keystroke_label(last)))
            }
        }
    }
}

/// The choice that has a command's keys reach ilo before the page.
const PREFER_ILO: &str = concat!("Prefer ", env!("APP_NAME"));

/// The Shortcuts tab, every command the user can give keys, in the menus' order and names.
const SHORTCUTS: &[(&str, &[(&str, Keys)])] = &[
    (
        "Tabs",
        &[
            ("New Tab", Keys::Of(Command::NewTab)),
            ("New Window", Keys::Of(Command::NewWindow)),
            ("New Little ilo Window", Keys::Of(Command::NewLittleWindow)),
            ("Open in Main Window", Keys::Of(Command::OpenInMainWindow)),
            ("New Incognito Window", Keys::Of(Command::NewPrivateWindow)),
            ("New Note", Keys::Of(Command::NewNote)),
            ("Open Command Bar", Keys::Of(Command::EditUrl)),
            ("Search Tabs", Keys::Of(Command::SearchTabs)),
            ("Archive Tab", Keys::Of(Command::CloseTab)),
            ("Close Window", Keys::Of(Command::CloseWindow)),
            ("Restore Last Closed Tab", Keys::Of(Command::ReopenClosedTab)),
            ("Duplicate Current Tab", Keys::Of(Command::DuplicateTab)),
            ("Next Tab", Keys::Of(Command::NextTab)),
            ("Previous Tab", Keys::Of(Command::PreviousTab)),
            ("Last Used Tab", Keys::Of(Command::RecentTab)),
            ("Tabs 1 to 8", Keys::Span("cmd-1", "cmd-8")),
            ("Include Favorites in ⌘1–⌘8", Keys::FavoritesNumbered),
            ("Last Tab", Keys::Of(Command::SelectLastTab)),
            ("Last Tab Goes to the Ninth Tab", Keys::LastTabNinth),
            ("Pin/Unpin Tab", Keys::Of(Command::TogglePin)),
            ("Favorite Tab", Keys::Of(Command::FavoriteTab)),
            ("Back to Pinned URL", Keys::Of(Command::BackToPinnedUrl)),
            ("Reveal Tab in Sidebar", Keys::Of(Command::RevealTab)),
            ("Focus Sidebar", Keys::Of(Command::FocusSidebar)),
            ("Clear Today", Keys::Of(Command::Clear)),
            ("Reset all tabs in this Space", Keys::Of(Command::ResetSpaceTabs)),
            ("Copy URL", Keys::Of(Command::CopyUrl)),
            ("Copy URL as Markdown", Keys::Of(Command::CopyMarkdownLink)),
            ("Paste URL as New Tab", Keys::Of(Command::PasteUrlAsTab)),
            ("Mute/Unmute Tab", Keys::Of(Command::ToggleMute)),
            ("Rename Current Tab", Keys::Of(Command::RenameTab)),
            ("New Folder", Keys::Of(Command::NewFolder)),
            ("Collapse All Folders", Keys::Of(Command::CollapseFolders)),
            ("Expand All Folders", Keys::Of(Command::ExpandFolders)),
        ],
    ),
    (
        "Page",
        &[
            ("Go Back", Keys::Of(Command::Back)),
            ("Go Forward", Keys::Of(Command::Forward)),
            ("Refresh the Page", Keys::Of(Command::Reload)),
            ("Force Refresh the Page", Keys::Of(Command::ForceReload)),
            ("Stop Loading", Keys::Of(Command::Stop)),
            ("Find", Keys::Of(Command::Find)),
            ("Find Next", Keys::Of(Command::FindNext)),
            ("Find Previous", Keys::Of(Command::FindPrevious)),
            ("Jump to Selection", Keys::Of(Command::JumpToSelection)),
            ("Zoom In", Keys::Of(Command::ZoomIn)),
            ("Zoom Out", Keys::Of(Command::ZoomOut)),
            ("Actual Size", Keys::Of(Command::ActualSize)),
            ("Share", Keys::Of(Command::Share)),
            ("Capture", Keys::Of(Command::CapturePortion)),
            ("Capture Full Page", Keys::Of(Command::CaptureFullPage)),
            ("Save Page As", Keys::Of(Command::SavePage)),
            ("Print", Keys::Of(Command::Print)),
            ("Clear Cache and Refresh", Keys::Of(Command::ClearCacheAndRefresh)),
            ("Clear Cookies and Refresh", Keys::Of(Command::ClearCookiesAndRefresh)),
            ("Site Controls", Keys::Of(Command::SiteControls)),
        ],
    ),
    (
        "Spaces and Sidebar",
        &[
            ("Next Space", Keys::Of(Command::NextSpace)),
            ("Previous Space", Keys::Of(Command::PreviousSpace)),
            ("Go to Space 1", Keys::Of(Command::SelectSpace(0))),
            ("Go to Space 2", Keys::Of(Command::SelectSpace(1))),
            ("Go to Space 3", Keys::Of(Command::SelectSpace(2))),
            ("Go to Space 4", Keys::Of(Command::SelectSpace(3))),
            ("Go to Space 5", Keys::Of(Command::SelectSpace(4))),
            ("Go to Space 6", Keys::Of(Command::SelectSpace(5))),
            ("Go to Space 7", Keys::Of(Command::SelectSpace(6))),
            ("Go to Space 8", Keys::Of(Command::SelectSpace(7))),
            ("Go to Space 9", Keys::Of(Command::SelectSpace(8))),
            ("New Space", Keys::Of(Command::NewSpace)),
            ("Rename Space", Keys::Of(Command::RenameSpace)),
            ("Export Space as Markdown", Keys::Of(Command::ExportSpace)),
            ("Show/Hide Sidebar", Keys::Of(Command::ToggleSidebar)),
            ("Collapse Pinned Tabs", Keys::Of(Command::CollapsePinned)),
            ("Expand Pinned Tabs", Keys::Of(Command::ExpandPinned)),
            ("Automatic Appearance", Keys::Of(Command::Appearance(Appearance::Automatic))),
            ("Light Appearance", Keys::Of(Command::Appearance(Appearance::Light))),
            ("Dark Appearance", Keys::Of(Command::Appearance(Appearance::Dark))),
            ("View Library", Keys::Of(Command::ViewLibrary)),
            ("View Archive", Keys::Of(Command::ViewArchive)),
            ("Clear Archive", Keys::Of(Command::ClearArchive)),
            ("View Downloads", Keys::Of(Command::ViewDownloads)),
            ("View Media", Keys::Of(Command::ViewMedia)),
            ("View History", Keys::Of(Command::ViewHistory)),
            ("Clear Browsing Data", Keys::Of(Command::ClearBrowsingData)),
        ],
    ),
    (
        "Split View",
        &[
            ("Add Split View", Keys::Of(Command::AddSplit)),
            ("New Note Split", Keys::Of(Command::NewNoteSplit)),
            ("Close this Split Pane", Keys::Of(Command::CloseSplitPane)),
            ("Next Pane", Keys::Of(Command::NextPane)),
            ("Previous Pane", Keys::Of(Command::PreviousPane)),
            ("Separate Page from Split View", Keys::Of(Command::SeparatePane)),
            ("Expand Current Split", Keys::Of(Command::ExpandSplit)),
            ("Separate All Pages", Keys::Of(Command::SeparateSplit)),
            ("Flip Split View", Keys::Of(Command::FlipSplit)),
            ("Change Orientation", Keys::Of(Command::ChangeOrientation)),
            ("Move Pane Left", Keys::Of(Command::MovePaneLeft)),
            ("Move Pane Right", Keys::Of(Command::MovePaneRight)),
        ],
    ),
    (
        "Developer",
        &[
            ("View Source", Keys::Of(Command::ViewSource)),
            ("Developer Tools", Keys::Of(Command::DevTools)),
            ("Inspect Elements", Keys::Of(Command::InspectElements)),
            ("JavaScript Console", Keys::Of(Command::JavaScriptConsole)),
            ("Toggle Developer Mode", Keys::Of(Command::ToggleDeveloperMode)),
            ("Open Task Manager", Keys::Of(Command::TaskManager)),
            ("Record a Trace", Keys::Of(Command::RecordTrace)),
            ("Manage Extensions", Keys::Of(Command::ManageExtensions)),
            ("Add Extension", Keys::Of(Command::AddExtension)),
        ],
    ),
    (
        env!("APP_NAME"),
        &[
            (concat!("About ", env!("APP_NAME")), Keys::Of(Command::About)),
            ("Check for Updates", Keys::Of(Command::CheckForUpdates)),
            ("Settings", Keys::Of(Command::Settings)),
            ("Import from Other Browsers", Keys::Of(Command::Import)),
            ("Privacy Policy", Keys::Of(Command::PrivacyPolicy)),
            ("Passwords", Keys::Of(Command::Passwords)),
            ("Site Search Settings", Keys::Of(Command::SiteSearchSettings)),
            // main.rs binds these.
            (concat!("Hide ", env!("APP_NAME")), Keys::Fixed("cmd-h")),
            ("Hide Others", Keys::Fixed("alt-cmd-h")),
            ("Minimize", Keys::Fixed("cmd-m")),
            (concat!("Quit ", env!("APP_NAME")), Keys::Of(Command::Quit)),
        ],
    ),
    (
        "Help",
        &[
            (concat!("Welcome to ", env!("APP_NAME")), Keys::Of(Command::Welcome)),
            ("Help Center", Keys::Of(Command::HelpCenter)),
            ("What's New", Keys::Of(Command::WhatsNew)),
            ("Essential Keyboard Shortcuts", Keys::Of(Command::KeyboardShortcuts)),
            ("Report a Problem", Keys::Of(Command::ReportProblem)),
            ("Copy Build Info", Keys::Of(Command::CopyBuildInfo)),
        ],
    ),
];

/// The pop-up buttons.
#[derive(Clone, Copy, PartialEq)]
enum Popup {
    ArchiveAfter,
    Favorites,
    PasswordsProfile,
    /// A profile's, in the Profiles tab.
    SearchEngine(u64),
    ProfileArchiveAfter(u64),
    /// A profile's New Documents.
    Notes(u64),
}

pub struct Settings {
    tab: Tab,
    focus: FocusHandle,
    /// It was asked to show, or changed tab, and logs the tab it draws next.
    shown: bool,
    /// Which switch last changed, and when, for its knob's slide.
    switched: Option<(&'static str, Instant)>,
    /// The pop-up menu that is open, whose button shows pressed.
    choosing: Option<Popup>,
    /// The profile whose name is in a field for renaming.
    renaming: Option<Renaming>,
    /// Whether the field takes the keyboard once its row is drawn.
    focus_renaming: bool,
    /// The command whose next key the Shortcuts tab takes.
    recording: Option<Command>,
    /// Why the key pressed for this command wasn't given to it.
    refused: Option<(Command, SharedString)>,
    /// The Shortcuts tab's search, which narrows its rows to what it finds.
    search: Entity<TextInput>,
    default_browser: Entity<DefaultBrowser>,
    account: Entity<Account>,
    /// The sites with ad blocking off, as General last logged them for check.sh.
    off_sites_logged: Option<String>,
    /// Where each control check.sh clicks has been logged, so it logs only where it moves to.
    drawn_logged: Rc<RefCell<Vec<String>>>,
    /// Where each pop-up button's text starts, as last drawn, for its menu to put the chosen
    /// item's over.
    popup_texts: Rc<RefCell<Vec<(Popup, gpui::Point<gpui::Pixels>)>>>,
    /// The profile whose logins the Passwords tab lists, once the user has picked one.
    passwords_profile: Option<u64>,
    /// Each profile's logins, as the core last told them.
    logins: HashMap<u64, Vec<Login>>,
    /// Each profile's saved addresses and cards, as the core last told them.
    autofill: HashMap<u64, Vec<Saved>>,
    /// The profiles whose logins were asked for since the Passwords tab showed.
    asked: HashSet<u64>,
    /// Made when the tab first shows, so the other browsers' files are read only once the user
    /// asks.
    import: Option<Entity<ImportView>>,
    /// The Links tab's view, made again each time the tab shows.
    links: Option<Entity<LinkRulesView>>,
    /// Where it asks before deleting a profile, over all of it, as the browser window's dialog
    /// can't go over it on Linux.
    #[cfg(target_os = "linux")]
    dialog: Entity<dialog::Dialog>,
    widevine: Entity<WidevineRow>,
    /// Draws the Agents tab again as the agents change.
    _agents: Option<Subscription>,
}

/// Settings' dialog, which it draws in its own content, so there's nothing to show.
#[cfg(target_os = "linux")]
struct InWindow;

#[cfg(target_os = "linux")]
impl dialog::Host for InWindow {
    fn show(&self, _: Option<Bounds<gpui::Pixels>>) {}

    fn hide(&self) {}
}

/// The browser's account, as the Sync tab asks the core for it.
struct CoreAccount {
    /// Where it asks before signing out or showing the words on Linux, which has no alert:
    /// Settings' own dialog, or for a browser window's view, the browser's.
    #[cfg(target_os = "linux")]
    dialog: Option<Entity<dialog::Dialog>>,
}

/// The browser's account for a view of it in a browser window, as the first launch's step, which
/// asks before starting over in an alert, or on Linux in the browser's dialog.
pub fn browser_account() -> Rc<dyn account::Host> {
    Rc::new(CoreAccount {
        #[cfg(target_os = "linux")]
        dialog: None,
    })
}

impl account::Host for CoreAccount {
    fn set_server(&self, server: &str) -> bool {
        let server = CString::new(server).unwrap_or_default();
        unsafe { bc::bc_sync_set_server(server.as_ptr()) }
    }

    fn sign_in(&self) {
        unsafe { bc::bc_sync_sign_in() };
    }

    fn cancel(&self) {
        unsafe { bc::bc_sync_cancel() };
    }

    fn sign_out(&self) {
        unsafe { bc::bc_sync_sign_out() };
    }

    fn recovery_words(&self) -> String {
        // 24 words of at most 8 letters, and the spaces between them.
        let mut words = [0u8; 256];
        let length = unsafe { bc::bc_sync_recovery_words(words.as_mut_ptr().cast(), words.len()) };
        String::from_utf8_lossy(&words[..length.min(words.len() - 1)]).into_owned()
    }

    fn enter_recovery_words(&self, words: &str) -> bool {
        let words = CString::new(words).unwrap_or_default();
        unsafe { bc::bc_sync_enter_recovery_words(words.as_ptr()) }
    }

    fn start_over(&self) {
        unsafe { bc::bc_sync_start_over() }
    }

    fn request_pairing(&self) {
        unsafe { bc::bc_sync_request_pairing() };
    }

    fn read_devices(&self) {
        unsafe { bc::bc_sync_read_devices() };
    }

    fn remove_device(&self, id: &str) {
        let id = CString::new(id).unwrap_or_default();
        unsafe { bc::bc_sync_remove_device(id.as_ptr()) };
    }

    fn copy_secret(&self, text: &str, kept: Duration, cx: &mut App) {
        let secret = CString::new(text).unwrap_or_default();
        let seconds = u32::try_from(kept.as_secs()).unwrap_or(u32::MAX);
        if unsafe { platform::host_copy_secret(secret.as_ptr(), seconds) } {
            return;
        }
        account::copy_for(text, kept, cx);
    }

    /// In an alert, or on Linux in Settings' own dialog, as a profile's Delete asks.
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        on_return: dialog::Return,
        confirmed: Box<dyn FnOnce(&mut App)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        #[cfg(target_os = "linux")]
        match &self.dialog {
            Some(dialog) => {
                dialog::ask_in(dialog, message, detail, action, on_return, confirmed, window, cx)
            }
            None => crate::with_overlays(
                move |cx| dialog::ask_returning(message, detail, action, on_return, confirmed, cx),
                cx,
            ),
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            let returns = on_return == dialog::Return::Action;
            cx.spawn(async move |cx| {
                if crate::ask(&message, &detail, &[action, "Cancel"], returns) == 0 {
                    cx.update(confirmed);
                }
            })
            .detach();
        }
    }
}

/// The browser's account changed: Settings' Sync tab shows it, and the first launch's account
/// step follows it (account::Synced).
pub fn sync_changed(state: account::State, cx: &mut App) {
    cx.set_global(account::Synced(state.clone()));
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        let changed = move |settings: &mut Settings, _: &mut Window, cx: &mut Context<Settings>| {
            settings.account.update(cx, |account, cx| account.set_state(state, cx));
        };
        window.update(cx, changed).ok();
    }
}

/// The account's devices, which the Sync tab asked the core for.
pub fn devices_changed(devices: Vec<account::Device>, cx: &mut App) {
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        let changed = move |settings: &mut Settings, _: &mut Window, cx: &mut Context<Settings>| {
            settings.account.update(cx, |account, cx| account.set_devices(devices, cx));
        };
        window.update(cx, changed).ok();
    }
}
/// The activity the Agents tab lists, newest first: the log keeps more, for the run.
const ACTIVITY_SHOWN: usize = 50;

/// The system's default browser, for General's row.
struct System;

impl default_browser::System for System {
    fn is_default(&self) -> bool {
        unsafe { platform::host_is_default_browser() }
    }

    fn make_default(&self) {
        unsafe { platform::host_make_default_browser(default_browser_answered) };
    }
}

/// Reads the system's answer into General's row, whoever asked.
pub extern "C" fn default_browser_answered() {
    crate::update_app(check_default_browser);
}

/// Google's Widevine module, for Advanced's row.
struct Module;

impl widevine::Module for Module {
    fn state(&self) -> Widevine {
        Widevine::from_core(unsafe { bc::bc_widevine() })
    }

    fn download(&self) {
        unsafe { bc::bc_widevine_download() };
    }

    fn remove(&self) {
        unsafe { bc::bc_widevine_remove() };
    }
}

struct Renaming {
    profile: u64,
    name: Entity<TextInput>,
    /// Keeps the name once the field loses the keyboard, as the sidebar's renaming does.
    _blur: Option<Subscription>,
}

/// A profile as the Profiles tab lists it.
struct Profile {
    id: u64,
    name: SharedString,
    spaces: usize,
    /// As the core has it: not the one it started with, nor one a Space or a favorite is in.
    deletable: bool,
    /// Whether one of the user's Spaces is in it, for Clear Browsing Data's tab.
    users_space: bool,
}

/// An address or a card a profile keeps, as the Passwords tab lists it. A card's number stays in
/// the core.
#[derive(Clone)]
pub struct Saved {
    /// An address's name, or its email; a card's nickname, or the name on it.
    pub title: SharedString,
    /// An address's first line and city; a card's network and expiry.
    pub detail: SharedString,
    pub card: bool,
    /// The core's name for it.
    pub key: SharedString,
}

/// A site the user turned ad blocking off on, as General lists it.
struct OffSite {
    profile: u64,
    /// The profile's name, shown only when there are others.
    profile_name: Option<SharedString>,
    site: SharedString,
}

/// The sites with ad blocking off, the first profile's first, each profile's in the order they
/// were turned off.
fn off_sites() -> Vec<OffSite> {
    let profiles = crate::ids(bc::bc_profiles);
    let several = profiles.len() > 1;
    (profiles.into_iter())
        .flat_map(|profile| {
            let profile_name = several.then(|| crate::name(bc::bc_profile_name, profile));
            let sites = crate::name(bc::bc_ad_blocking_off_sites, profile);
            (sites.split('\n').filter(|site| !site.is_empty()))
                .map(|site| OffSite {
                    profile,
                    profile_name: profile_name.clone(),
                    site: site.to_owned().into(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The profiles, the one the browser started with first.
fn profiles() -> Vec<Profile> {
    let in_space = |space| unsafe { bc::bc_space_profile(space) };
    let spaces: Vec<u64> = crate::ids(bc::bc_spaces).into_iter().map(in_space).collect();
    let users = crate::ids(bc::bc_spaces).into_iter().filter(|&space| {
        crate::name(bc::bc_space_agent, space).is_empty()
    });
    let users: Vec<u64> = users.map(in_space).collect();
    let of_tab = |tab| unsafe { bc::bc_tab_profile(tab) };
    let favorites: Vec<u64> = crate::ids(bc::bc_favorites).into_iter().map(of_tab).collect();
    (crate::ids(bc::bc_profiles).into_iter().enumerate())
        .map(|(index, id)| {
            let count = spaces.iter().filter(|&&each| each == id).count();
            Profile {
                id,
                name: crate::name(bc::bc_profile_name, id),
                spaces: count,
                deletable: index > 0 && count == 0 && !favorites.contains(&id),
                users_space: users.contains(&id),
            }
        })
        .collect()
}

/// The archive pop-up's text for `seconds`, and which of ARCHIVE_AFTER it is.
fn archive_after(seconds: f64) -> (SharedString, Option<usize>) {
    let chosen = ARCHIVE_AFTER.iter().position(|&(_, each)| each == seconds);
    let label = match chosen {
        Some(index) => ARCHIVE_AFTER[index].0.into(),
        None => format!("{:.0} hours", seconds / 3600.).into(),
    };
    (label, chosen)
}

/// The search engines `profile`'s default is picked from, each its keyword and name, and the
/// keyword of the default, or None while it loads or a policy or an extension picks it.
fn search_engines(profile: u64) -> Option<(Vec<(SharedString, SharedString)>, SharedString)> {
    let engines = crate::name(bc::bc_profile_search_engines, profile);
    let engines: Vec<(SharedString, SharedString)> = (engines.lines())
        .filter_map(|line| line.split_once('\t'))
        .map(|(keyword, name)| (keyword.to_string().into(), name.to_string().into()))
        .collect();
    (!engines.is_empty()).then(|| (engines, crate::name(bc::bc_profile_search_engine, profile)))
}

/// Where `profile`'s downloads go, with ~ for the home folder, or None while it loads.
fn download_folder(profile: u64) -> Option<SharedString> {
    let folder = crate::name(bc::bc_profile_download_folder, profile);
    if folder.is_empty() {
        return None;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    Some(match folder.strip_prefix(home.as_str()) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}").into()
        }
        _ => folder,
    })
}

struct SettingsWindow(WindowHandle<Settings>);

impl Global for SettingsWindow {}

/// Shows the Settings window on its Shortcuts tab.
pub fn show_shortcuts(cx: &mut App) {
    show_on(Tab::Shortcuts, cx);
}

/// Shows the Settings window on its Agents tab, with each agent's activity.
pub fn show_agents(cx: &mut App) {
    show_on(Tab::Agents, cx);
}

/// Shows the Settings window on its About tab.
pub fn show_about(cx: &mut App) {
    show_on(Tab::About, cx);
}

/// Shows the Settings window on its Import tab.
pub fn show_import(cx: &mut App) {
    show_on(Tab::Import, cx);
}

fn show_on(tab: Tab, cx: &mut App) {
    cx.defer(move |cx| {
        show_now(cx);
        if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
            let show = |settings: &mut Settings, window: &mut Window, cx: &mut Context<Settings>| {
                settings.show_tab(tab, window, cx)
            };
            window.update(cx, show).ok();
        }
    });
}

/// Shows the Settings window on its Passwords tab, with `profile`'s logins, or those it showed.
pub fn show_passwords(profile: Option<u64>, cx: &mut App) {
    cx.defer(move |cx| {
        show_now(cx);
        if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
            let show =
                move |settings: &mut Settings, window: &mut Window, cx: &mut Context<Settings>| {
                    settings.passwords_profile = profile.or(settings.passwords_profile);
                    settings.show_tab(Tab::Passwords, window, cx)
                };
            window.update(cx, show).ok();
        }
    });
}

/// The profile's logins, which the Passwords tab asked the core for, as they are now.
pub fn logins_changed(profile: u64, logins: Vec<Login>, cx: &mut App) {
    // check.sh waits for this line. The app has no passwords to log, only sites and usernames.
    log!("browser: passwords {profile}: {}", passwords::describe(&logins));
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        let changed = move |settings: &mut Settings, _: &mut Window, cx: &mut Context<Settings>| {
            settings.logins.insert(profile, logins);
            cx.notify();
        };
        window.update(cx, changed).ok();
    }
}

/// The profile's saved addresses and cards, which the Passwords tab asked the core for.
pub fn autofill_changed(profile: u64, saved: Vec<Saved>, cx: &mut App) {
    // check.sh waits for this line, which names nothing saved.
    let cards = saved.iter().filter(|each| each.card).count();
    log!("browser: autofill {profile}: {} addresses, {cards} cards", saved.len() - cards);
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        let changed = move |settings: &mut Settings, _: &mut Window, cx: &mut Context<Settings>| {
            settings.autofill.insert(profile, saved);
            cx.notify();
        };
        window.update(cx, changed).ok();
    }
}

/// Shows the Settings window, opening it the first time, or brings it to the front.
pub fn show(cx: &mut App) {
    cx.defer(show_now);
}

/// Shows it now, which can't be from inside its own window's update: a command from the menu bar
/// while Settings is the key window runs in one, where reading the window panics and updating it
/// does nothing.
fn show_now(cx: &mut App) {
    if !cx.has_global::<SettingsWindow>() {
        open(cx);
    }
    let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) else {
        return;
    };
    window
        .update(cx, |settings, window, cx| {
            window.focus(&settings.focus, cx);
            settings.shown = true;
            if settings.tab == Tab::Agents {
                look_for_clients(cx);
            }
            cx.notify();
        })
        .ok();
    check_default_browser(cx);
    unsafe { platform::host_show_settings() };
    // check.sh waits for this line, which the Mac's and Linux's hosts log as they show the window.
    if cfg!(target_os = "windows") {
        log!("browser: settings shown {}", title());
    }
}

/// Looks again for the agent clients on this computer, as the Agents tab shows.
fn look_for_clients(cx: &mut App) {
    if let Some(agents) = agents::global(cx) {
        agents.update(cx, |agents, cx| agents.look_for_clients(cx));
    }
}

/// Reads again where Google's Widevine module is, as the core says it changed, once Settings has
/// been opened.
pub fn check_widevine(cx: &mut App) {
    let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) else {
        return;
    };
    if let Ok(row) = window.read_with(cx, |settings, _| settings.widevine.clone()) {
        row.update(cx, |row, cx| row.check(cx));
    }
}

/// Reads again whether the browser is the system's default, as Settings shows, the app becomes
/// active, or the user has answered macOS, once Settings has been opened.
pub fn check_default_browser(cx: &mut App) {
    let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) else {
        return;
    };
    let Ok(row) = window.read_with(cx, |settings, _| settings.default_browser.clone()) else {
        return;
    };
    let default = row.update(cx, |row, cx| {
        row.check(cx);
        row.is_default()
    });
    // check.sh waits for this line.
    log!("browser: default browser {}", if default { "yes" } else { "no" });
}

thread_local! {
    /// The window's title, the name of the tab it shows.
    static SHOWN_TAB: Cell<&'static str> = const { Cell::new("") };
}

/// The window's title, logged as the Mac's host reads it off the window.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn title() -> &'static str {
    SHOWN_TAB.get()
}

fn open(cx: &mut App) {
    // ⌘W, pressed as Ctrl+W off the Mac, as the browser's own shortcuts are.
    let close = if crate::shell::OFF_THE_MAC { "ctrl-w" } else { "cmd-w" };
    cx.bind_keys([gpui::KeyBinding::new(close, CloseSettings, Some("Settings"))]);
    account::bind_keys(cx);
    let synced = account::synced(cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WIDTH), px(HEIGHT)),
        ))),
        // The UI draws the title and the toolbar under the window's buttons.
        titlebar: Some(TitlebarOptions {
            title: Some(Tab::General.name().into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // Nor a title bar of the window manager's on Linux, where the UI draws the close button.
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui::WindowDecorations::Client),
        kind: WindowKind::Normal,
        show: false,
        focus: false,
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    SHOWN_TAB.set(Tab::General.name());
    let window = cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            // Closing it hides it, so it opens again where the user left it.
            window.on_window_should_close(cx, |_, cx| {
                hide(cx);
                false
            });
            let default_browser = cx.new(|_| DefaultBrowser::new(Rc::new(System), Os::current()));
            cx.observe(&default_browser, |_, _, cx| cx.notify()).detach();
            #[cfg(target_os = "linux")]
            let dialog = {
                let dialog = dialog::in_window(Rc::new(InWindow), cx);
                // Settings takes the keyboard back from the dialog once it has gone, as the
                // dialog's focus still counts as focused then, though nothing draws it.
                let mut asked = false;
                cx.observe_in(&dialog, window, move |settings: &mut Settings, dialog, window, cx| {
                    let asks = dialog::asks(&dialog, cx);
                    if asked && !asks {
                        window.focus(&settings.focus, cx);
                    }
                    asked = asks;
                })
                .detach();
                dialog
            };
            let host = CoreAccount {
                #[cfg(target_os = "linux")]
                dialog: Some(dialog.clone()),
            };
            let account = cx.new(|cx| {
                let mut account = Account::new(Rc::new(host), cx);
                account.set_state(synced, cx);
                account
            });
            cx.observe(&account, |_, _, cx| cx.notify()).detach();
            let widevine = cx.new(|_| WidevineRow::new(Rc::new(Module)));
            cx.observe(&widevine, |_, _, cx| cx.notify()).detach();
            let agents = agents::global(cx);
            let agents = agents.map(|agents| cx.observe(&agents, |_, _, cx| cx.notify()));
            let search = cx.new(|cx| {
                TextInput::new("Search", cx)
                    .placeholder_color(rgb(DETAIL))
                    .caret_color(rgb(TEXT))
            });
            let mut searched = String::new();
            cx.observe(&search, move |_, search, cx| {
                let typed = search.read(cx).text();
                if typed != searched {
                    searched = typed.to_string();
                    log_found(&searched, cx);
                }
                cx.notify();
            })
            .detach();
            Settings {
                account,
                #[cfg(target_os = "linux")]
                dialog,
                default_browser,
                widevine,
                _agents: agents,
                tab: Tab::General,
                focus: cx.focus_handle(),
                shown: false,
                switched: None,
                choosing: None,
                renaming: None,
                focus_renaming: false,
                recording: None,
                refused: None,
                search,
                off_sites_logged: None,
                drawn_logged: Rc::default(),
                popup_texts: Rc::default(),
                passwords_profile: None,
                logins: HashMap::new(),
                autofill: HashMap::new(),
                asked: HashSet::new(),
                import: None,
                links: None,
            }
        })
    });
    match window {
        Ok(window) => {
            if let Ok(view) = window.update(cx, |_, window, cx| {
                let view = ns_view(window);
                // The link preview over a browser window closes as Settings takes the keyboard.
                #[cfg(target_os = "linux")]
                crate::system::watch_settings_activation(window, view, cx);
                view
            }) {
                unsafe { platform::host_add_settings(view) };
                cx.set_global(SettingsWindow(window));
            }
        }
        Err(error) => log!("couldn't open the Settings window: {error}"),
    }
}

/// Shows what changed in the core, such as a profile made, renamed or deleted, if it's open.
pub fn refresh(cx: &mut App) {
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        window.update(cx, |_, _, cx| cx.notify()).ok();
    }
}

/// The window's close button, for a window without a frame to draw it: where the browser window's
/// is on Linux, and in the top right corner on Windows, where Windows puts close. Settings can't be
/// minimized or zoomed, so it's alone. `at` logs where it's drawn.
fn close_button(at: gpui::Canvas<()>) -> AnyElement {
    if cfg!(target_os = "windows") {
        return windows_close_button(at);
    }
    let (x, colour) = shell::WINDOW_BUTTONS[0];
    let button = div().id("close").absolute();
    let button = match cfg!(target_os = "windows") {
        true => button.right(px(x - WINDOW_BUTTON_SIZE / 2.)),
        false => button.left(px(x - WINDOW_BUTTON_SIZE / 2.)),
    };
    button
        .top(px((TITLE_BAR - WINDOW_BUTTON_SIZE) / 2.))
        .size(px(WINDOW_BUTTON_SIZE))
        .rounded_full()
        .bg(rgb(colour))
        .group("close")
        // A press here closes the window, and doesn't move it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| hide(cx))
        .child(
            svg()
                .path("icons/window-close.svg")
                .size(px(WINDOW_BUTTON_SIZE))
                .text_color(gpui::transparent_black())
                .group_hover("close", |style| style.text_color(rgba(WINDOW_BUTTON_GLYPH))),
        )
        .child(at)
        .into_any_element()
}

/// Windows' close, as the browser window's (shell.rs's `windows_buttons`): Settings' title's grey
/// glyph, on Windows' red under the pointer, as far in from the corner as the Mac's is.
fn windows_close_button(at: gpui::Canvas<()>) -> AnyElement {
    let (x, _) = shell::WINDOW_BUTTONS[0];
    div()
        .id("close")
        .absolute()
        .right(px(x - WINDOWS_BUTTON_PLATE / 2.))
        .top(px((TITLE_BAR - WINDOWS_BUTTON_PLATE) / 2.))
        .size(px(WINDOWS_BUTTON_PLATE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .group("close")
        .hover(|style| style.bg(rgb(WINDOWS_CLOSE_HOVERED)))
        // A press here closes the window, and doesn't move it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| hide(cx))
        .child(
            svg()
                .path("icons/window-windows-close.svg")
                .size(px(WINDOWS_BUTTON_GLYPH))
                .text_color(rgb(TITLE))
                .group_hover("close", |style| style.text_color(gpui::white())),
        )
        .child(at)
        .into_any_element()
}

fn hide(_: &mut App) {
    // check.sh waits for this line.
    log!("browser: settings hidden");
    unsafe { platform::host_hide_settings() };
}

impl Settings {
    fn show_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        if tab == self.tab {
            return;
        }
        self.keep_name(window, cx);
        self.recording = None;
        self.refused = None;
        // Read again each time it shows: a profile closed since has the core forget it was asked.
        self.asked.clear();
        // check.sh waits for this line.
        log!("browser: settings tab {}", tab.name());
        self.tab = tab;
        // A tab takes no click until it has drawn, so check.sh waits for its "settings drew".
        self.shown = true;
        if tab == Tab::Import {
            self.show_import(cx);
        }
        if tab == Tab::Agents {
            look_for_clients(cx);
        }
        self.links = (tab == Tab::Links).then(|| {
            let host = Rc::new(crate::link_rules_host::CoreLinkRules);
            cx.new(|cx| LinkRulesView::new(host, cx))
        });
        window.set_window_title(tab.name());
        SHOWN_TAB.set(tab.name());
        cx.notify();
    }

    /// The import's view, made the first time, else with the browsers found again, as they may
    /// have changed since.
    fn show_import(&mut self, cx: &mut Context<Self>) {
        match &self.import {
            Some(import) => import.update(cx, |import, cx| import.refresh(cx)),
            None => {
                let host = Rc::new(crate::import_host::CoreImport);
                self.import = Some(cx.new(|_| ImportView::new(host, false)));
            }
        }
    }

    /// The title, and the toolbar's tabs, under the close button where the window has no frame to
    /// draw it. Dragging either moves the window, as a toolbar does.
    fn bar(&self, accent: Rgba, frameless: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tabs = Tab::shown().map(|tab| {
            let shown = tab == self.tab;
            let color = if shown { accent } else { rgb(TAB_TEXT) };
            div()
                .id(tab.name())
                .min_w(px(TAB))
                .h(px(PLATE))
                .mt(px(PLATE_TOP))
                .px(px(TAB_PADDING))
                .flex()
                .flex_col()
                .items_center()
                .rounded(px(RADIUS))
                .when(shown, |plate| {
                    plate.bg(rgb(PLATE_FILL)).border(px(0.5)).border_color(rgb(PLATE_EDGE))
                })
                // check.sh clicks a tab where this says, as its labels' widths differ by platform.
                .relative()
                .child(self.drawn(format!("toolbar {}", tab.name())))
                // A press here picks the tab, and doesn't move the window.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |settings, _, window, cx| {
                    settings.show_tab(tab, window, cx)
                }))
                .child(svg().path(tab.icon()).size(px(ICON)).mt(px(ICON_TOP)).text_color(color))
                .child(
                    div()
                        .mt(px(LABEL_TOP))
                        .text_size(px(text::SMALL))
                        .text_color(color)
                        .child(tab.name()),
                )
        })
        .collect::<Vec<_>>();
        div()
            .flex_none()
            .h(px(TITLE_BAR + TOOLBAR))
            .bg(rgb(BAR))
            .border_b(px(0.5))
            .border_color(rgb(BAR_LINE))
            .relative()
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .children(frameless.then(|| close_button(self.drawn("close".into()))))
            .child(
                div()
                    .h(px(TITLE_BAR))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(TITLE))
                    .child(self.tab.name()),
            )
            .child(div().h(px(TOOLBAR)).flex().justify_center().gap(px(1.)).children(tabs))
    }

    fn general(&self, off_sites: &[OffSite], accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let (label, chosen) = archive_after(unsafe { bc::bc_archive_after() });
        let archive = self.popup(
            Popup::ArchiveAfter,
            label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_archive_after(None, chosen, window, cx)
            }),
        );
        let scope = favorites_scope();
        let favorites = self.popup(
            Popup::Favorites,
            MenuEntry::label(scope.title()),
            FAVORITES_POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_favorites_scope(scope, window, cx)
            }),
        );
        let warn = unsafe { bc::bc_warn_before_quitting() };
        let warn = self.toggle("warn before quitting", warn, set_warn_before_quitting, accent, cx);
        let unload = unsafe { bc::bc_unload_idle_tabs() };
        let unload = self.toggle("unload idle tabs", unload, set_unload_idle_tabs, accent, cx);
        let ad_blocking = unsafe { bc::bc_ad_blocking() };
        let ad_blocking = self.toggle("ad blocking", ad_blocking, set_ad_blocking, accent, cx);
        // The Mac's alone, the one with a host for the button so far.
        let menu_bar = cfg!(target_os = "macos").then(|| {
            let on = crate::little_in_menu_bar();
            let switch = self.toggle("little in the menu bar", on, set_little_in_menu_bar, accent, cx);
            row(None, "Show Little ilo in the menu bar", switch)
        });
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            // Unload under the archive's time, as both are about tabs left unseen.
            .child(group(&RowLook::settings(), [
                row(None, "Warn before quitting", warn),
                // Each profile then keeps its own, in Profiles.
                row(None, "New profiles archive tabs after", archive.into_any_element()),
                row(None, "Unload tabs you haven't used in a while", unload),
                row(None, "Favorites", favorites.into_any_element()),
                // Last, so the checks find the pop-ups where they were.
                row(None, "Block ads and trackers", ad_blocking),
            ].into_iter().chain(menu_bar)))
            .children(self.off_sites(off_sites, cx))
            .child(group(&RowLook::settings(), [self.default_browser_row(cx)]))
            .into_any_element()
    }

    /// The sites the user turned ad blocking off on in their Site Controls, each with the way to
    /// turn it on again, under Block ads and trackers. Nothing while there are none.
    fn off_sites(&self, sites: &[OffSite], cx: &mut Context<Self>) -> Option<gpui::Div> {
        if sites.is_empty() {
            return None;
        }
        let rows = sites.iter().enumerate().map(|(index, off)| {
            let (profile, site) = (off.profile, off.site.clone());
            let at = self.drawn(format!("turn on {}", off.site));
            let turn_on = button(&RowLook::settings(), ("ad-blocking-on", index as u64), "Turn On").relative().child(at);
            let turn_on = turn_on.on_click(cx.listener(move |_, _, _, cx| {
                let Ok(text) = CString::new(site.to_string()) else {
                    return;
                };
                unsafe { bc::bc_ad_blocking_turn_on_site(profile, text.as_ptr()) };
                // check.sh waits for this line.
                log!("browser: ad blocking on for {site}");
                cx.notify();
            }));
            let profile_name = (off.profile_name.clone())
                .map(|name| div().flex_none().ml(px(PADDING)).text_color(rgb(DETAIL)).child(name));
            div()
                .px(px(PADDING))
                .py(px(PADDING))
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().truncate().child(off.site.clone()))
                .children(profile_name)
                .child(div().flex_none().ml(px(PADDING)).child(turn_on))
        });
        let header = div()
            .mb(px(HEADER_GAP))
            .pl(px(PADDING))
            .font_weight(FontWeight::SEMIBOLD)
            .child("Ad blocking is off on");
        Some(div().child(header).child(group(&RowLook::settings(), rows)))
    }

    /// Logs the sites General lists with ad blocking off when they change, for check.sh: each
    /// site, and its profile when there are others.
    fn log_off_sites(&mut self, sites: &[OffSite]) {
        let listed: Vec<String> = (sites.iter())
            .map(|off| match &off.profile_name {
                Some(name) => format!("{} in {name}", off.site),
                None => off.site.to_string(),
            })
            .collect();
        let listed = listed.join(", ");
        if self.off_sites_logged.as_ref() != Some(&listed) {
            log!("browser: settings ad blocking off on {listed}");
            self.off_sites_logged = Some(listed);
        }
    }

    /// Make Default until the browser is the system's default, then that it is, with the way back
    /// under it, which is the system's. In a Flatpak, where to set it, and no button.
    fn default_browser_row(&self, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.default_browser.read(cx);
        let make = row.offers().then(|| {
            let make = cx.listener(|settings: &mut Self, _, _, cx| {
                // check.sh waits for this line.
                log!("browser: make default browser");
                settings.default_browser.read(cx).make_default();
            });
            button(&RowLook::settings(), "make-default-browser", "Make Default").on_click(make)
        });
        detailed_row(row.title(), row.detail(), make.map(IntoElement::into_any_element))
    }

    /// Logs where the element it fills is drawn (settings_rows::drawn). Put it in a relative one.
    fn drawn(&self, what: String) -> gpui::Canvas<()> {
        browser::settings_rows::drawn(what, self.drawn_logged.clone())
    }

    /// A pop-up button showing `label`. It opens on a press, as a pop-up button does, and shows
    /// pressed while its menu is open.
    fn popup(
        &self,
        which: Popup,
        label: SharedString,
        width: f32,
        press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> gpui::Div {
        let texts = self.popup_texts.clone();
        let at = canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                let text = point(bounds.origin.x + px(CONTROL_PADDING), bounds.origin.y);
                let mut texts = texts.borrow_mut();
                texts.retain(|&(each, _)| each != which);
                texts.push((which, text));
            },
        )
        .absolute()
        .inset_0();
        popup(&RowLook::settings(), label, width, self.choosing == Some(which))
            .relative()
            .mr(px(POPUP_END))
            .on_mouse_down(MouseButton::Left, press)
            .child(at)
    }

    /// The switch `name`, `on` now, that calls `set` with the other way when clicked.
    fn toggle(
        &self,
        name: &'static str,
        on: bool,
        set: impl Fn(bool, &mut App) + 'static,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let slid = match self.switched {
            Some((switched, at)) if switched == name => {
                (at.elapsed().as_secs_f32() / SLIDE).min(1.)
            }
            _ => 1.,
        };
        div()
            .id(name)
            .on_click(cx.listener(move |settings, _, _, cx| {
                // check.sh waits for this line.
                log!("browser: {name} {}", if on { "off" } else { "on" });
                set(!on, cx);
                settings.switched = Some((name, Instant::now()));
                cx.notify();
            }))
            .relative()
            .child(switch(&RowLook::settings(), on, slid, accent))
            .child(self.drawn(format!("switch {name}")))
            .into_any_element()
    }

    /// Opens the archive's pop-up menu over its button, with the time chosen now, `chosen`, over
    /// the button's text, as a pop-up button does, and sets the time picked: `profile`'s, or with
    /// None, the one new profiles start with.
    fn choose_archive_after(
        &mut self,
        profile: Option<u64>,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (ARCHIVE_AFTER.iter().enumerate())
            .map(|(index, &(title, _))| {
                let on = Some(index) == chosen;
                MenuEntry { checked: on, over: on, ..MenuEntry::new(title, true) }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            match (picked.map(|index| ARCHIVE_AFTER[index]), profile) {
                (Some((title, seconds)), Some(profile)) => {
                    // check.sh waits for this line.
                    log!("browser: profile {profile} archive tabs after {title}");
                    unsafe { bc::bc_profile_set_archive_after(profile, seconds) };
                }
                (Some((title, seconds)), None) => {
                    // check.sh waits for this line.
                    log!("browser: archive tabs after {title}");
                    unsafe { bc::bc_set_archive_after(seconds) };
                }
                (None, _) => {}
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        let which = profile.map_or(Popup::ArchiveAfter, Popup::ProfileArchiveAfter);
        self.pop_up(which, items, picked, window, cx);
    }

    /// Opens a profile's search engine pop-up menu over its button, as the archive's, and makes
    /// the engine picked its default.
    fn choose_search_engine(
        &mut self,
        profile: u64,
        engines: Vec<(SharedString, SharedString)>,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (engines.iter().enumerate())
            .map(|(index, (_, name))| {
                let on = Some(index) == chosen;
                MenuEntry { checked: on, over: on, ..MenuEntry::named(name.clone(), true) }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((keyword, name)) = picked.and_then(|index| engines.get(index)) {
                // check.sh waits for this line.
                log!("browser: profile {profile} searches with {name}");
                let keyword = CString::new(keyword.as_str()).unwrap_or_default();
                unsafe { bc::bc_profile_set_search_engine(profile, keyword.as_ptr()) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::SearchEngine(profile), items, picked, window, cx);
    }

    /// Opens the favorites' pop-up menu over its button, as the archive's, and sets the scope
    /// picked. The core keeps the favorites it had, copying or merging the lists, and tells the
    /// windows (`favorites_changed`).
    fn choose_favorites_scope(
        &mut self,
        chosen: shell::FavoritesScope,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (shell::FavoritesScope::ALL.iter())
            .map(|&scope| {
                let on = scope == chosen;
                MenuEntry { checked: on, over: on, ..MenuEntry::new(scope.title(), true) }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(scope) = picked.map(|index| shell::FavoritesScope::ALL[index]) {
                // check.sh waits for this line.
                log!("browser: favorites scope {}", scope.title());
                unsafe { bc::bc_set_favorites_scope(scope.to_core()) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::Favorites, items, picked, window, cx);
    }

    /// Shows `which` pop-up's menu of `items` in ilo's menu over Settings' window, dark as Settings
    /// is, its button pressed until `picked` is called.
    fn pop_up(
        &mut self,
        which: Popup,
        items: Vec<MenuEntry>,
        picked: Box<dyn FnOnce(Option<usize>, &mut App)>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let texts = self.popup_texts.borrow();
        let Some(at) = texts.iter().find(|&&(each, _)| each == which).map(|&(_, at)| at) else {
            return;
        };
        drop(texts);
        self.choosing = Some(which);
        cx.notify();
        show_menu(ns_view(window), items, at, false, Look::dark(), picked, cx);
    }

    /// The pop-up's menu closed.
    fn chose(&mut self, cx: &mut Context<Self>) {
        self.choosing = None;
        cx.notify();
    }

    /// A row for each profile, with how many Spaces are in it and Delete for one nothing is in, and
    /// New Profile under them, then each profile's New Documents. A double-click on a profile's name
    /// renames it, as on a Space's.
    fn profiles(&self, profiles: &[Profile], cx: &mut Context<Self>) -> AnyElement {
        let services: Vec<_> = profiles.iter().map(|profile| self.notes_row(profile, cx)).collect();
        let rows = profiles.iter().map(|profile| {
            let (id, name) = (profile.id, profile.name.clone());
            let label = match &self.renaming {
                Some(renaming) if renaming.profile == id => div()
                    .flex_1()
                    .min_w_0()
                    .key_context("Naming")
                    .on_action(cx.listener(|settings, _: &KeepName, window, cx| {
                        settings.keep_name(window, cx)
                    }))
                    .on_action(cx.listener(|settings, _: &CancelName, window, cx| {
                        settings.cancel_name(window, cx)
                    }))
                    .child(renaming.name.clone()),
                _ => div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.drawn(format!("profile {id}")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |settings, event: &MouseDownEvent, _, cx| {
                            if event.click_count == 2 {
                                settings.rename(id, name.clone(), cx);
                            }
                        }),
                    )
                    .child(profile.name.clone()),
            };
            let spaces: SharedString = match profile.spaces {
                0 => "No Spaces".into(),
                1 => "1 Space".into(),
                count => format!("{count} Spaces").into(),
            };
            let name = profile.name.clone();
            let delete = profile.deletable.then(|| {
                let button = button(&RowLook::settings(), ("delete-profile", id), "Delete");
                let button = button.relative().child(self.drawn(format!("delete profile {id}")));
                button.on_click(
                    cx.listener(move |settings, _, window, cx| {
                        settings.delete(id, name.clone(), window, cx)
                    }),
                )
            });
            let head = div()
                .h(px(PROFILE_ROW))
                .px(px(PADDING))
                .flex()
                .items_center()
                .child(tile("icons/user.svg", PROFILE_TILE))
                .child(label)
                .child(div().flex_none().ml(px(PADDING)).text_color(rgb(DETAIL)).child(spaces))
                .children(delete.map(|delete| delete.ml(px(PADDING))));
            // Its own settings under it, as Arc's Settings › Profiles keeps them: none to show
            // until the core has loaded it.
            let search = search_engines(id).map(|(engines, keyword)| {
                let chosen = engines.iter().position(|(each, _)| *each == keyword);
                let label = chosen.map_or(keyword, |index| engines[index].1.clone());
                let popup = self.popup(
                    Popup::SearchEngine(id),
                    label,
                    POPUP_WIDTH,
                    cx.listener(move |settings, _, window, cx| {
                        settings.choose_search_engine(id, engines.clone(), chosen, window, cx)
                    }),
                );
                let popup = popup.child(self.drawn(format!("search engine {id}")));
                row(None, "Search engine", popup.into_any_element())
            });
            let after = unsafe { bc::bc_profile_archive_after(id) };
            let archive = (after > 0.).then(|| {
                let (label, chosen) = archive_after(after);
                let popup = self.popup(
                    Popup::ProfileArchiveAfter(id),
                    label,
                    POPUP_WIDTH,
                    cx.listener(move |settings, _, window, cx| {
                        settings.choose_archive_after(Some(id), chosen, window, cx)
                    }),
                );
                let popup = popup.child(self.drawn(format!("archive {id}")));
                row(None, "Archive tabs after", popup.into_any_element())
            });
            let folder = download_folder(id).map(|folder| {
                let change = button(&RowLook::settings(), ("download-folder", id), "Change…")
                    .relative()
                    .child(self.drawn(format!("download folder {id}")))
                    .on_click(cx.listener(move |_, _, _, cx| choose_download_folder(id, cx)));
                detailed_row("Download folder", folder, Some(change.into_any_element()))
            });
            let clear = profile.users_space.then(|| {
                let clear = button(&RowLook::settings(), ("clear-browsing-data", id), "Clear…")
                    .relative()
                    .child(self.drawn(format!("clear {id}")))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if crate::clear_browsing_data(id) {
                            hide(cx);
                        }
                    }));
                row(None, "Browsing data", clear.into_any_element())
            });
            let rows = [Some(head), search, archive, folder, clear];
            group(&RowLook::settings(), rows.into_iter().flatten())
        });
        let new = button(&RowLook::settings(), "new-profile", "New Profile")
            .relative()
            .child(self.drawn("new profile".into()))
            .on_click(cx.listener(|settings, _, _, cx| settings.new_profile(cx)));
        let documents = div()
            // The column's gap is above it too.
            .mt(px(SECTION_GAP - BOX_GAP))
            .mb(px(HEADER_GAP))
            .pl(px(PADDING))
            .font_weight(FontWeight::SEMIBOLD)
            .child("New Documents");
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .children(rows)
            .child(div().mt(px(BUTTONS_TOP - BOX_GAP)).flex().child(new))
            .child(div().child(documents).child(group(&RowLook::settings(), services)))
            .into_any_element()
    }

    /// A profile's row under New Documents: the notes service New Note opens a new document in, in
    /// a pop-up, as Arc's Settings › Profiles has it.
    fn notes_row(&self, profile: &Profile, cx: &mut Context<Self>) -> gpui::Div {
        let id = profile.id;
        let chosen = notes::service(&crate::name(bc::bc_profile_notes_service, id));
        let label = notes::SERVICES[chosen].1.into();
        let popup = self.popup(
            Popup::Notes(id),
            label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| settings.choose_notes(id, chosen, window, cx)),
        );
        let icon = Some(("icons/user.svg", PROFILE_TILE));
        row(icon, profile.name.clone(), popup.into_any_element())
    }

    /// Opens a profile's New Documents pop-up menu over its button, as the archive's, and keeps the
    /// service picked for the profile.
    fn choose_notes(
        &mut self,
        profile: u64,
        chosen: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (notes::SERVICES.iter().enumerate())
            .map(|(index, &(_, title, _))| {
                let on = index == chosen;
                // A service's name, as written, as Google Docs keeps its capital off the Mac too.
                MenuEntry { checked: on, over: on, ..MenuEntry::named(title, true) }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(&(name, title, _)) = picked.and_then(|index| notes::SERVICES.get(index)) {
                // check.sh waits for this line.
                log!("browser: notes service {profile} {title}");
                let name = CString::new(name).unwrap_or_default();
                unsafe { bc::bc_profile_set_notes_service(profile, name.as_ptr()) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::Notes(profile), items, picked, window, cx);
    }

    /// Shows the profile's name in a field, all selected, to type a new one over.
    fn rename(&mut self, profile: u64, name: SharedString, cx: &mut Context<Self>) {
        let name = cx.new(|cx| {
            TextInput::new("Profile name", cx)
                .placeholder_color(rgb(DETAIL))
                .caret_color(rgb(TEXT))
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            profile,
            name,
            _blur: None,
        });
        self.focus_renaming = true;
        cx.notify();
    }

    /// Names the profile what the field holds, unless it's empty, and closes the field.
    fn keep_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let name = renaming.name.read(cx).text().trim().to_string();
        if !name.is_empty() {
            let name = CString::new(name).unwrap_or_default();
            unsafe { bc::bc_profile_rename(renaming.profile, name.as_ptr()) };
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            // check.sh waits for this line.
            log!("browser: rename cancelled");
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Makes a profile, named as Chrome numbers them, with its name in a field once it's listed,
    /// to name it at once, as Finder's New Folder does.
    fn new_profile(&mut self, cx: &mut Context<Self>) {
        let names: Vec<SharedString> = profiles().into_iter().map(|profile| profile.name).collect();
        let name = shell::new_profile_name(&names);
        let c_name = CString::new(name.as_str()).unwrap_or_default();
        let profile = unsafe { bc::bc_profile_create(c_name.as_ptr()) };
        // check.sh waits for this line.
        log!("browser: create profile {profile} {name}");
        if profile != 0 {
            self.rename(profile, name.into(), cx);
        }
    }

    /// Deletes the profile once the user confirms, in an alert, as the window has no room for the
    /// browser window's dialog. Linux has no alert, so it asks in its own dialog over Settings.
    fn delete(
        &mut self,
        profile: u64,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Delete “{name}”?");
        let detail = "Its history, cookies and logins go with it.";
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail.into(),
            "Delete",
            dialog::Return::Action,
            Box::new(move |_| {
                unsafe { bc::bc_profile_delete(profile) };
            }),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            cx.spawn(async move |_, _| {
                if crate::ask(&message, detail, &["Delete", "Cancel"], true) == 0 {
                    unsafe { bc::bc_profile_delete(profile) };
                }
            })
            .detach();
        }
    }

    /// Send Crash Reports, off by default, with where they go and what turning it off does under
    /// it, as the default browser's row has its way back.
    fn privacy(&self, accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let send = unsafe { bc::bc_send_crash_reports() };
        let send = self.toggle("send crash reports", send, set_send_crash_reports, accent, cx);
        let detail = format!(
            "Only sent while this is on, to {}. Turning it off deletes those not yet sent.",
            collector_name(&crash_reports_collector())
        );
        let row = detailed_row("Send crash reports", detail, Some(send));
        div().child(group(&RowLook::settings(), [row])).into_any_element()
    }

    /// The profile the Passwords tab lists: the one picked or asked for while it's there, or else
    /// the one the browser started with.
    fn passwords_profile(&self, profiles: &[Profile]) -> Option<u64> {
        let picked = self.passwords_profile.filter(|&id| profiles.iter().any(|each| each.id == id));
        picked.or(profiles.first().map(|profile| profile.id))
    }

    /// The profile's logins: the saved ones in a box, each with its username under its site, and
    /// the sites set to Never in another, each removable. A pop-up picks the profile, if there are
    /// more than one, as each keeps its own, over the switch for whether it offers to save them.
    /// Manage Passwords… under the logins opens Chrome's page.
    fn passwords(
        &mut self,
        profiles: &[Profile],
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(profile) = self.passwords_profile(profiles) else {
            return div().into_any_element();
        };
        // None until the core has loaded the profile, which has no logins to read before then.
        let offers = |what| {
            let mut offers = false;
            unsafe { bc::bc_profile_offers_to_save(profile, what, &mut offers) }.then_some(offers)
        };
        let passwords = offers(bc::BC_SAVE_PASSWORDS as i32);
        if passwords.is_some() && self.asked.insert(profile) {
            unsafe { bc::bc_profile_read_logins(profile) };
            unsafe { bc::bc_profile_read_autofill(profile) };
        }
        let logins = self.logins.get(&profile).cloned().unwrap_or_default();
        // As the Shortcuts tab's headers, the first at the top unless the pop-up is over it.
        let header = |title: &'static str, first: bool| {
            div()
                .when(!first, |header| header.mt(px(SECTION_GAP)))
                .mb(px(HEADER_GAP))
                .pl(px(PADDING))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title)
        };
        let login_row = |index: usize, login: &Login, cx: &mut Context<Self>| {
            let (site, username) = passwords::lines(login);
            let login = login.clone();
            let remove = button(&RowLook::settings(), ("remove-login", index), "Remove");
            let remove = remove.relative().child(self.drawn(format!("remove login {index}")));
            let remove = remove.on_click(cx.listener(move |settings, _, window, cx| {
                settings.remove(profile, login.clone(), window, cx)
            }));
            div()
                .h(px(PROFILE_ROW))
                .px(px(PADDING))
                .flex()
                .items_center()
                .child(tile(Tab::Passwords.icon(), PASSWORD_TILE))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().child(site))
                        .children(username.map(|username| {
                            div()
                                .truncate()
                                .text_size(px(USERNAME_SIZE))
                                .text_color(rgb(DETAIL))
                                .child(username)
                        })),
                )
                .child(remove.ml(px(PADDING)))
        };
        // An address or a card as a login's row: its title over its detail, and Remove.
        let autofill = self.autofill.get(&profile).cloned().unwrap_or_default();
        let saved_row = |index: usize, saved: &Saved, cx: &mut Context<Self>| {
            let icon = if saved.card { "icons/credit-card.svg" } else { "icons/map-pin.svg" };
            let saved = saved.clone();
            let remove = button(&RowLook::settings(), ("remove-autofill", index), "Remove");
            let remove = remove.relative().child(self.drawn(format!("remove autofill {index}")));
            let remove = remove.on_click(cx.listener({
                let saved = saved.clone();
                move |settings, _, window, cx| {
                    settings.remove_autofill(profile, saved.clone(), window, cx)
                }
            }));
            div()
                .h(px(PROFILE_ROW))
                .px(px(PADDING))
                .flex()
                .items_center()
                .child(tile(icon, PASSWORD_TILE))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().child(saved.title.clone()))
                        .when(!saved.detail.is_empty(), |text| {
                            text.child(
                                div()
                                    .truncate()
                                    .text_size(px(USERNAME_SIZE))
                                    .text_color(rgb(DETAIL))
                                    .child(saved.detail.clone()),
                            )
                        }),
                )
                .child(remove.ml(px(PADDING)))
        };
        let (cards, addresses): (Vec<_>, Vec<_>) =
            autofill.iter().enumerate().partition(|(_, saved)| saved.card);
        let addresses: Vec<gpui::Div> =
            addresses.into_iter().map(|(index, saved)| saved_row(index, saved, cx)).collect();
        let cards: Vec<gpui::Div> =
            cards.into_iter().map(|(index, saved)| saved_row(index, saved, cx)).collect();
        let (saved, never): (Vec<_>, Vec<_>) =
            logins.iter().enumerate().partition(|(_, login)| !login.never);
        let saved: Vec<gpui::Div> =
            saved.into_iter().map(|(index, login)| login_row(index, login, cx)).collect();
        let never: Vec<gpui::Div> =
            never.into_iter().map(|(index, login)| login_row(index, login, cx)).collect();
        let saved = match saved.is_empty() {
            true => vec![
                div()
                    .h(px(PROFILE_ROW))
                    .px(px(PADDING))
                    .flex()
                    .items_center()
                    .text_color(rgb(DETAIL))
                    .child("Passwords you save are listed here."),
            ],
            false => saved,
        };
        let chooser = (profiles.len() > 1).then(|| {
            let chosen = profiles.iter().position(|each| each.id == profile);
            let names: Vec<SharedString> = profiles.iter().map(|each| each.name.clone()).collect();
            let ids: Vec<u64> = profiles.iter().map(|each| each.id).collect();
            let label = chosen.map(|index| names[index].clone()).unwrap_or_default();
            let view = cx.entity().downgrade();
            let popup = self.popup(Popup::PasswordsProfile, label, POPUP_WIDTH, move |_, window, cx| {
                let (names, ids) = (names.clone(), ids.clone());
                let choose = |settings: &mut Settings, cx: &mut Context<Settings>| {
                    settings.choose_profile(names, ids, chosen, window, cx)
                };
                view.update(cx, choose).ok();
            });
            let icon = Some(("icons/user.svg", PROFILE_TILE));
            row(icon, "Profile", popup.into_any_element())
        });
        // Chrome's words for each, as its settings pages have them.
        let switches = [
            (bc::BC_SAVE_PASSWORDS, "offer to save passwords", "Offer to save passwords"),
            (bc::BC_SAVE_ADDRESSES, "save and fill addresses", "Save and fill addresses"),
            (bc::BC_SAVE_CARDS, "save and fill payment methods", "Save and fill payment methods"),
        ];
        let switches = switches.map(|(what, name, title)| {
            offers(what as i32).map(|on| {
                let set = move |offer, _: &mut App| unsafe {
                    bc::bc_profile_set_offers_to_save(profile, what as i32, offer);
                };
                row(None, title, self.toggle(name, on, set, accent, cx))
            })
        });
        let settings = [chooser].into_iter().chain(switches).flatten().collect::<Vec<_>>();
        let first = settings.is_empty();
        let manage = button(&RowLook::settings(), "manage-passwords", "Manage Passwords…")
            .relative()
            .child(self.drawn("manage passwords".into()))
            .on_click(move |_, _, cx| manage_passwords(profile, cx));
        div()
            .when(!first, |tab| tab.child(group(&RowLook::settings(), settings)))
            .child(header("Saved", first))
            .child(group(&RowLook::settings(), saved))
            .when(!never.is_empty(), |tab| {
                tab.child(header("Never Saved", false)).child(group(&RowLook::settings(), never))
            })
            .child(div().mt(px(BUTTONS_TOP)).flex().child(manage))
            // Each only while it has one, as Chrome offers to save them as forms are filled.
            .when(!addresses.is_empty(), |tab| {
                tab.child(header("Addresses", false)).child(group(&RowLook::settings(), addresses))
            })
            .when(!cards.is_empty(), |tab| {
                tab.child(header("Payment Methods", false)).child(group(&RowLook::settings(), cards))
            })
            .into_any_element()
    }

    /// Opens the profiles' pop-up menu over its button, as the archive's, and lists the profile
    /// picked's logins.
    fn choose_profile(
        &mut self,
        names: Vec<SharedString>,
        ids: Vec<u64>,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (names.iter().enumerate())
            .map(|(index, name)| {
                let on = Some(index) == chosen;
                MenuEntry { checked: on, over: on, ..MenuEntry::named(name.clone(), true) }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            settings
                .update(cx, |settings, cx| {
                    if let Some(&profile) = picked.and_then(|index| ids.get(index)) {
                        // check.sh waits for this line.
                        log!("browser: passwords profile {profile}");
                        settings.passwords_profile = Some(profile);
                    }
                    settings.chose(cx);
                })
                .ok();
        });
        self.pop_up(Popup::PasswordsProfile, items, picked, window, cx);
    }

    /// Removes a saved login once the user confirms, in an alert, or on Linux in Settings' own
    /// dialog, as a profile's Delete asks, as nothing brings it back. A site set to Never comes off
    /// it at once: the offer coming again is its way back.
    fn remove(&mut self, profile: u64, login: Login, window: &mut Window, cx: &mut Context<Self>) {
        if login.never {
            crate::remove_login(profile, &login);
            return;
        }
        let (message, detail) = passwords::question(&login);
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            "Remove",
            dialog::Return::Action,
            Box::new(move |_| crate::remove_login(profile, &login)),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            cx.spawn(async move |_, _| {
                if crate::ask(&message, &detail, &["Remove", "Cancel"], true) == 0 {
                    crate::remove_login(profile, &login);
                }
            })
            .detach();
        }
    }

    /// Removes a saved address or card once the user confirms, as a login, as nothing brings it
    /// back.
    fn remove_autofill(
        &mut self,
        profile: u64,
        saved: Saved,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = match saved.card {
            true => format!("Remove the card “{}”?", saved.title),
            false => format!("Remove the address for “{}”?", saved.title),
        };
        let detail = format!("{} won't fill it in again.", env!("APP_NAME"));
        let remove = move || {
            // check.sh waits for this line, which names nothing saved.
            log!("browser: remove {} {profile}", if saved.card { "card" } else { "address" });
            let key = CString::new(saved.key.as_str()).unwrap_or_default();
            unsafe { bc::bc_profile_remove_autofill(profile, key.as_ptr()) };
        };
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            "Remove",
            dialog::Return::Action,
            Box::new(move |_| remove()),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            cx.spawn(async move |_, _| {
                if crate::ask(&message, &detail, &["Remove", "Cancel"], true) == 0 {
                    remove();
                }
            })
            .detach();
        }
    }

    /// The switch that lets agents in, the clients to connect, and what the agents did in this
    /// run, newest first.
    fn agents(&self, agents: &Entity<Agents>, accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let on = agents.read(cx).is_on();
        let switch = {
            let agents = agents.clone();
            let set = move |on, cx: &mut App| agents.update(cx, |agents, cx| agents.set_on(on, cx));
            self.toggle("agents", on, set, accent, cx)
        };
        let model = agents.read(cx);
        let about: SharedString = match model.trouble() {
            Some(trouble) => trouble,
            None if on => {
                "On: a connected client reaches the browser through the bridge beside it.".into()
            }
            None => "They work in a Space of their own, which you can watch, and never see your \
                     passwords, cookies or autofill."
                .into(),
        };
        let listed = agents.read(cx).listed();
        let clients: Vec<gpui::Div> =
            listed.iter().map(|client| self.client(agents, *client, &listed, cx)).collect();
        // A list for each agent, as each has its own Space (decision 8): what the user let it do
        // or never do on a site, to revoke, then what it did.
        let model = agents.read(cx);
        let now = std::time::SystemTime::now();
        let lists: Vec<gpui::Div> = (Client::ALL.into_iter())
            .filter_map(|client| {
                let grants = self.grants(agents, client, cx);
                let entries: Vec<gpui::Div> = (model.activity(client.id()).take(ACTIVITY_SHOWN))
                    .map(|entry| {
                        let what = agents::tools::describe(&entry.action);
                        let refused = (entry.steps.iter().map(|step| &step.refused))
                            .find(|refused| !refused.is_empty())
                            .unwrap_or(&entry.refused);
                        let steps = (entry.steps.iter())
                            .map(|step| agents::tools::describe(&step.action));
                        let detail: SharedString = if !refused.is_empty() {
                            format!("Refused: {refused}").into()
                        } else if !entry.steps.is_empty() {
                            steps.collect::<Vec<_>>().join(" · ").into()
                        } else {
                            entry.site.clone()
                        };
                        let detail = Some(detail).filter(|detail| !detail.is_empty());
                        let ago = div().flex_none().ml(px(PADDING)).text_color(rgb(DETAIL));
                        described(None, what.into(), detail).child(ago.child(agents::ago(entry.at, now)))
                    })
                    .collect();
                (grants.is_some() || !entries.is_empty()).then(|| {
                    let entries = (!entries.is_empty()).then(|| group(&RowLook::settings(), entries));
                    let groups = div().flex().flex_col().gap(px(BOX_GAP));
                    let groups = groups.children(grants).children(entries);
                    div().flex().flex_col().child(header(client.name())).child(groups)
                })
            })
            .collect();
        let activity = match lists.is_empty() {
            true => div().flex().flex_col().child(header("Activity")).child(group(&RowLook::settings(), [described(
                None,
                "Nothing yet".into(),
                Some("What agents do while the browser runs shows here.".into()),
            )])),
            false => div().flex().flex_col().gap(px(SECTION_GAP)).children(lists),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .child(group(&RowLook::settings(), [described(
                Some(("icons/sparkles.svg", AGENTS_TILE)),
                "Let agents use the browser".into(),
                Some(about),
            )
            .child(div().flex_none().ml(px(PADDING)).child(switch))]))
            .child(group(&RowLook::settings(), clients))
            .child(activity.mt(px(SECTION_GAP - BOX_GAP)))
            .into_any_element()
    }

    /// The sites where the user answered the agent for good, each with Revoke, and Revoke All
    /// for several, then the tabs of its Space the user doesn't hold, with Take Back All; None
    /// for none.
    fn grants(&self, agents: &Entity<Agents>, client: Client, cx: &App) -> Option<gpui::Div> {
        let grants = agents.read(cx).grants(client.id());
        let revoke = |site: Option<SharedString>, title| {
            let id = format!("revoke-{}-{}", client.id(), site.as_deref().unwrap_or(""));
            let agents = agents.clone();
            button(&RowLook::settings(), gpui::ElementId::Name(id.into()), title).ml(px(PADDING)).on_click(
                move |_, _, cx| {
                    agents.update(cx, |agents, cx| {
                        agents.revoke(client.id(), site.as_deref(), cx)
                    })
                },
            )
        };
        let several = grants.len() > 1;
        let mut rows: Vec<gpui::Div> = (grants.into_iter())
            .map(|(site, may)| {
                let answer = if may { "Always Allow" } else { "Never" };
                let row = described(None, site.clone(), Some(answer.into()));
                row.child(revoke(Some(site), "Revoke"))
            })
            .collect();
        if several {
            rows.push(described(None, "All sites".into(), None).child(revoke(None, "Revoke All")));
        }
        let tabs = agents.read(cx).tabs_to_take(client.id(), cx).len();
        if tabs > 0 {
            let agents = agents.clone();
            let take = button(&RowLook::settings(), ("take-back-agent-tabs", client as usize), "Take Back All")
                .ml(px(PADDING))
                .on_click(move |_, _, cx| {
                    agents.update(cx, |agents, cx| agents.take_back_all(client.id(), cx))
                });
            let open = match tabs {
                1 => "1 tab it may act in".into(),
                _ => format!("{tabs} tabs it may act in"),
            };
            rows.push(described(None, "Its tabs".into(), Some(open.into())).child(take));
        }
        (!rows.is_empty()).then(|| group(&RowLook::settings(), rows))
    }

    /// A client's row: whether it's connected, and Connect, or Disconnect, with Copy Setup for a
    /// client the browser can't set up itself.
    fn client(
        &self,
        agents: &Entity<Agents>,
        client: Client,
        listed: &[Client],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let model = agents.read(cx);
        let connected = model.is_connected(client);
        let detail: SharedString = if let Some(why) = model.failure(client) {
            why
        } else if model.is_connecting(client) {
            "Connecting…".into()
        } else if connected && model.is_live(client) {
            "Connected, and at work".into()
        } else if connected && client == Client::Other {
            format!(
                "Runs {}. Copy Setup copies it with its token.",
                model.command()
            )
            .into()
        } else if connected {
            format!("Connected in {}", client.config()).into()
        } else if client == Client::Other {
            // Says which it looked for, so one missing from the list isn't a mystery.
            let missing: Vec<&str> = (Client::ALL.into_iter())
                .filter(|client| !listed.contains(client))
                .map(Client::name)
                .collect();
            match missing.as_slice() {
                [] => "Any MCP client".into(),
                [one] => format!("Any MCP client. {one} isn't on this computer.").into(),
                _ => format!("Any MCP client. {} aren't on this computer.", missing.join(" and "))
                    .into(),
            }
        } else {
            format!("Not connected. Connect adds the browser to {}", client.config()).into()
        };
        let index = client as usize;
        let setup = (connected && client == Client::Other).then(|| model.setup(client)).flatten();
        let copy = setup.map(|setup| {
            button(&RowLook::settings(), ("copy-agent-setup", index), "Copy Setup").on_click(move |_, _, cx| {
                // check.sh reads this line.
                log!("browser: agent setup copied");
                cx.write_to_clipboard(ClipboardItem::new_string(setup.clone()));
            })
        });
        let agents = agents.clone();
        let action = match connected {
            true => button(&RowLook::settings(), ("disconnect-agent", index), "Disconnect").on_click(move |_, _, cx| {
                agents.update(cx, |agents, cx| agents.disconnect(client, cx))
            }),
            false => button(&RowLook::settings(), ("connect-agent", index), "Connect").on_click(move |_, _, cx| {
                // check.sh reads this line.
                log!("browser: connect agent {}", client.id());
                agents.update(cx, |agents, cx| agents.connect(client, cx))
            }),
        };
        let icon = match client {
            Client::ClaudeCode => "icons/terminal.svg",
            Client::Codex => "icons/square-terminal.svg",
            Client::Other => "icons/cable.svg",
        };
        described(Some((icon, CLIENT_TILE)), client.name().into(), Some(detail))
            .min_h(px(PROFILE_ROW))
            .children(copy.map(|copy| copy.ml(px(PADDING))))
            .child(action.ml(px(PADDING)))
    }

    fn advanced(&self, accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let haptics = unsafe { bc::bc_haptics() };
        let haptics = self.toggle("haptics", haptics, set_haptics, accent, cx);
        let previews = unsafe { bc::bc_link_previews() };
        let previews = self.toggle("link previews", previews, set_link_previews, accent, cx);
        let mini_player = unsafe { bc::bc_auto_picture_in_picture() };
        let mini_player = self.toggle("mini player", mini_player, set_mini_player, accent, cx);
        let option = unsafe { bc::bc_option_reveals_sidebar() };
        let option = self.toggle("option reveals sidebar", option, set_option_reveals, accent, cx);
        // Where the host watches for it (host_watch_option_taps).
        let option = cfg!(any(target_os = "macos", target_os = "linux")).then(|| {
            row(
                Some(("icons/panel-left.svg", OPTION_REVEAL_TILE)),
                "Show the hidden sidebar when ⌥ is pressed alone",
                option,
            )
        });
        let restore = unsafe { bc::bc_restore_windows() };
        let restore = self.toggle("restore windows", restore, set_restore_windows, accent, cx);
        let more = button(&RowLook::settings(), "more-settings", "More Settings…")
            .on_click(|_, _, cx| more_settings(cx));
        let widevine = self.widevine.read(cx).shown().then(|| {
            group(&RowLook::settings(), [self.widevine_row(cx)]).mt(px(BOX_GAP))
        });
        let export = button(&RowLook::settings(), "export-spaces", "Export…")
            .on_click(|_, _, cx| crate::export_spaces(cx));
        let export = div()
            .h(px(PROFILE_ROW))
            .px(px(PADDING))
            .flex()
            .items_center()
            .child(tile("icons/file.svg", EXPORT_TILE))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child("Export each Space's favorites and pinned tabs as Markdown"),
            )
            .child(div().flex_none().ml(px(PADDING)).child(export));
        div()
            .child(group(
                &RowLook::settings(),
                [
                    row(
                        Some(("icons/vibrate.svg", HAPTICS_TILE)),
                        "Haptic feedback when swiping between Spaces",
                        haptics,
                    ),
                    row(
                        Some(("icons/app-window.svg", LINK_PREVIEWS_TILE)),
                        "Preview links to other sites from pinned tabs and favorites",
                        previews,
                    ),
                    row(
                        Some(("icons/picture-in-picture-2.svg", MINI_PLAYER_TILE)),
                        "Float a video or call in Picture in Picture as you leave its tab",
                        mini_player,
                    ),
                    row(
                        Some(("icons/rotate-ccw-clock.svg", RESTORE_WINDOWS_TILE)),
                        "Restore windows from last session",
                        restore,
                    ),
                ]
                .into_iter()
                .chain(option),
            ))
            .child(group(&RowLook::settings(), [export]).mt(px(BOX_GAP)))
            .children(widevine)
            .child(div().mt(px(BUTTONS_TOP)).flex().child(more))
            .into_any_element()
    }

    /// Google's Widevine module, where the core offers it: where it is, and the way to get it or
    /// remove it. The user's, as the note's yes is.
    fn widevine_row(&self, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.widevine.read(cx);
        let action = row.action().map(|(action, title)| {
            let act = cx.listener(move |settings: &mut Self, _, _, cx| {
                log!("browser: settings widevine {action:?}");
                settings.widevine.read(cx).act(action);
            });
            button(&RowLook::settings(), "widevine", title).on_click(act)
        });
        detailed_row(row.title(), row.detail(), action.map(IntoElement::into_any_element))
    }

    /// Gives the renaming field the keyboard once its profile is listed, which a new one is once
    /// Chrome has made it.
    fn focus_field(&mut self, profiles: &[Profile], window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = &mut self.renaming else {
            return;
        };
        if !profiles.iter().any(|profile| profile.id == renaming.profile)
            || !std::mem::take(&mut self.focus_renaming)
        {
            return;
        }
        let field = renaming.name.focus_handle(cx);
        window.focus(&field, cx);
        // check.sh waits for this line, which says the field has the keyboard.
        log!("browser: rename profile {}", renaming.profile);
        renaming._blur =
            Some(text_input::on_leave(&field, window, cx, |settings, window, cx| {
                settings.keep_name(window, cx)
            }));
    }

    /// The search, then the commands it finds by section, each with its keys in a chip. A click on
    /// a command's chip records its next keys; escape leaves them and ⌫ takes them away. ↺ gives
    /// a command its default back, and Restore Defaults every command, with the favorites numbered
    /// and ⌘9 the last tab.
    fn shortcuts(&self, accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let typed = self.search.read(cx).text().to_string();
        let found = found(&typed, cx);
        let sections = SHORTCUTS.iter().filter_map(|(section, rows)| {
            let rows: Vec<gpui::Div> = rows
                .iter()
                .filter(|(name, _)| found.contains(name))
                .map(|(name, keys)| {
                    let control = match (keys, keys.fixed()) {
                        (Keys::Of(command), _) => self.shortcut(*command, accent, cx),
                        (Keys::FavoritesNumbered, _) => {
                            let on = shell::user_keys(cx).favorites_numbered();
                            self.toggle("favorites numbered", on, set_favorites_numbered, accent, cx)
                        }
                        (Keys::LastTabNinth, _) => {
                            let on = shell::user_keys(cx).last_tab_ninth();
                            self.toggle("last tab ninth", on, set_last_tab_ninth, accent, cx)
                        }
                        (_, text) => chip(text.unwrap_or_default().into(), rgb(DETAIL))
                            .into_any_element(),
                    };
                    row(None, shown(name), control)
                })
                .collect();
            (!rows.is_empty()).then(|| {
                div().mt(px(SECTION_GAP)).child(
                    div()
                        .mb(px(HEADER_GAP))
                        .pl(px(PADDING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(*section),
                    )
                    .child(group(&RowLook::settings(), rows))
            })
        });
        let sections: Vec<gpui::Div> = sections.collect();
        // Ours: Arc's look for a search that finds nothing isn't recorded.
        let none = sections.is_empty().then(|| {
            div().mt(px(SECTION_GAP)).pl(px(PADDING)).text_color(rgb(DETAIL)).child("No Results")
        });
        let search = div()
            .h(px(CONTROL_HEIGHT))
            .px(px(CONTROL_PADDING))
            .flex()
            .items_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(rgb(CONTROL))
            // Keys typed here are the search's, not a command's being recorded.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|settings, _, _, cx| {
                    settings.recording = None;
                    settings.refused = None;
                    cx.notify();
                }),
            )
            .child(self.search.clone());
        let restore = button(&RowLook::settings(), "restore-shortcuts", "Restore Defaults");
        let restore = restore.on_click(cx.listener(|settings, _, _, cx| {
            shell::change_keys(cx, UserKeys::reset_all);
            // check.sh waits for this line.
            log!("browser: shortcuts restored");
            settings.recording = None;
            settings.refused = None;
            save_shortcuts(cx);
            cx.notify();
        }));
        div()
            .child(search)
            .children(sections)
            .children(none)
            .child(div().mt(px(BUTTONS_TOP)).flex().child(restore))
            .into_any_element()
    }

    /// `command`'s chip, which records its keys if they can be changed, after why the last keys
    /// were refused and the way back to its default, if it has others.
    fn shortcut(&self, command: Command, accent: Rgba, cx: &mut Context<Self>) -> AnyElement {
        let user = shell::user_keys(cx);
        if !shell::rebindable(command) {
            let keys = user.label(command).unwrap_or_default();
            return chip(keys.into(), rgb(DETAIL)).into_any_element();
        }
        let recording = self.recording == Some(command);
        let (keys, color): (SharedString, Rgba) = match user.label(command) {
            _ if recording => ("Type Keys".into(), accent),
            Some(keys) => (keys.into(), rgb(TEXT)),
            None => ("None".into(), rgb(DETAIL)),
        };
        let refused = (self.refused.as_ref())
            .filter(|(refused, _)| *refused == command)
            .map(|(_, why)| div().text_color(rgb(DETAIL)).child(why.clone()));
        // Who gets the keys first, once the user has chosen ilo, as the menu below says it.
        let preferred = user.prefers_ilo(command).then(|| {
            div().text_color(rgb(DETAIL)).child(MenuEntry::label(PREFER_ILO))
        });
        let restore = user.changed(command).then(|| {
            div()
                .id(SharedString::from(format!("restore-{}", command.key_name())))
                .size(px(CHIP_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RESTORE_RADIUS))
                .hover(|restore| restore.bg(Palette::on_dark(STATE_HOVERED)))
                .active(|restore| restore.bg(Palette::on_dark(STATE_PRESSED)))
                .relative()
                .child("↺")
                .child(self.drawn(format!("restore {}", command.key_name())))
                .on_click(cx.listener(move |settings, _, _, cx| settings.restore(command, cx)))
        });
        let chip = chip(keys, color)
            .id(SharedString::from(command.key_name()))
            .when(recording, |chip| chip.border_1().border_color(accent))
            .hover(|chip| chip.bg(Palette::on_dark(state_alpha(CHIP_ALPHA, STATE_HOVERED))))
            .active(|chip| chip.bg(Palette::on_dark(state_alpha(CHIP_ALPHA, STATE_PRESSED))))
            .on_click(cx.listener(move |settings, _, window, cx| {
                settings.start_recording(command, window, cx)
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |settings, event: &MouseDownEvent, window, cx| {
                    settings.choose_preference(command, event.position, window, cx)
                }),
            )
            .relative()
            .child(self.drawn(format!("shortcut {}", command.key_name())));
        div()
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .children(refused)
            .children(preferred)
            .children(restore)
            .child(chip)
            .into_any_element()
    }

    /// A right-click on `command`'s chip: who gets its keys first when a page uses them too,
    /// Prefer Websites, the default, or Prefer ilo, as Arc's Shortcuts offers. A reserved command's
    /// are always ilo's, which the menu shows. A page that keeps keys still gives them up to a
    /// second press.
    fn choose_preference(
        &mut self,
        command: Command,
        at: gpui::Point<gpui::Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let user = shell::user_keys(cx);
        let (ilo, can) = (user.before_page(command), UserKeys::can_prefer(command));
        let items = vec![
            MenuEntry { checked: !ilo, ..MenuEntry::new("Prefer Websites", can) },
            MenuEntry { checked: ilo, ..MenuEntry::new(PREFER_ILO, can) },
        ];
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            let Some(picked) = picked else {
                return;
            };
            let ilo = picked == 1;
            shell::change_keys(cx, |keys| keys.set_prefers_ilo(command, ilo));
            let who = if ilo { "ilo" } else { "websites" };
            // check.sh waits for this line.
            log!("browser: shortcut {} prefers {who}", command.key_name());
            save_shortcuts(cx);
            settings.update(cx, |_, cx| cx.notify()).ok();
        });
        show_menu(ns_view(window), items, at, false, Look::dark(), picked, cx);
    }

    /// Takes the next keys pressed for `command`, or stops if it's taking them already.
    fn start_recording(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.refused = None;
        self.recording = match self.recording == Some(command) {
            true => None,
            false => {
                // check.sh waits for this line.
                log!("browser: shortcut recording {}", command.key_name());
                Some(command)
            }
        };
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Gives the command being recorded `keystroke`, unless it's escape, which leaves its keys,
    /// or ⌫, which takes them away. Keys that can't be its stay recording, saying why.
    fn record(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(command) = self.recording else {
            return;
        };
        cx.stop_propagation();
        let plain = !keystroke.modifiers.modified();
        let keys = match keystroke.key.as_str() {
            "escape" if plain => {
                // check.sh waits for this line.
                log!("browser: shortcut recording stopped");
                self.recording = None;
                self.refused = None;
                return cx.notify();
            }
            "backspace" if plain => Ok(Vec::new()),
            _ => shell::recorded(keystroke),
        };
        let set = keys.and_then(|keys| shell::change_keys(cx, |user| user.set(command, keys)));
        match set {
            Ok(_) => {
                self.recording = None;
                self.refused = None;
                save_shortcuts(cx);
            }
            Err(refused) => {
                let why = refusal(refused);
                // check.sh waits for this line.
                log!("browser: shortcut refused {why}");
                self.refused = Some((command, why.into()));
            }
        }
        cx.notify();
    }

    /// Gives `command` its default keys back, unless another command keeps them.
    fn restore(&mut self, command: Command, cx: &mut Context<Self>) {
        match shell::change_keys(cx, |user| user.reset(command)) {
            Ok(_) => {
                self.refused = None;
                save_shortcuts(cx);
            }
            Err(refused) => self.refused = Some((command, refusal(refused).into())),
        }
        cx.notify();
    }
}

/// What the core has for who shares the favorites.
fn favorites_scope() -> shell::FavoritesScope {
    shell::FavoritesScope::from_core(unsafe { bc::bc_favorites_scope() })
}

// The shell's FavoritesScope numbers are the core's.
const _: () = {
    assert!(bc::BC_FAVORITES_EVERY == 0 && bc::BC_FAVORITES_PROFILE == 1);
    assert!(bc::BC_FAVORITES_SPACE == 2);
};

fn set_warn_before_quitting(warn: bool, _: &mut App) {
    unsafe { bc::bc_set_warn_before_quitting(warn) };
}

fn set_little_in_menu_bar(on: bool, _: &mut App) {
    crate::set_little_in_menu_bar(on);
}

fn set_unload_idle_tabs(unload: bool, _: &mut App) {
    unsafe { bc::bc_set_unload_idle_tabs(unload) };
}

/// In every profile; a site's own stays as the user set it in the Site Controls.
fn set_ad_blocking(on: bool, _: &mut App) {
    unsafe { bc::bc_set_ad_blocking(on) };
}

/// Off, the next launch opens one window with no Today tabs, theirs going in the archive.
fn set_restore_windows(restore: bool, _: &mut App) {
    unsafe { bc::bc_set_restore_windows(restore) };
}

fn set_haptics(haptics: bool, _: &mut App) {
    unsafe { bc::bc_set_haptics(haptics) };
}

pub(crate) fn set_send_crash_reports(send: bool, _: &mut App) {
    unsafe { bc::bc_set_send_crash_reports(send) };
}

/// What the Privacy tab calls the collector at `host`: Sentry, by its region, else its host.
pub(crate) fn collector_name(host: &str) -> String {
    match host {
        _ if host.ends_with(".ingest.de.sentry.io") => "Sentry (EU region)".into(),
        _ if host.ends_with(".ingest.sentry.io") || host.ends_with(".ingest.us.sentry.io") => {
            "Sentry".into()
        }
        _ => host.into(),
    }
}

/// The host crash reports go to, or nothing in a build that names none.
pub(crate) fn crash_reports_collector() -> String {
    let length = unsafe { bc::bc_crash_reports_collector(std::ptr::null_mut(), 0) };
    let mut host = vec![0u8; length + 1];
    unsafe { bc::bc_crash_reports_collector(host.as_mut_ptr().cast(), host.len()) };
    host.truncate(length);
    String::from_utf8_lossy(&host).into_owned()
}

fn set_link_previews(previews: bool, _: &mut App) {
    unsafe { bc::bc_set_link_previews(previews) };
}

/// In every profile; a site the user allows in its Site Controls still floats.
fn set_mini_player(on: bool, _: &mut App) {
    unsafe { bc::bc_set_auto_picture_in_picture(on) };
}

fn set_option_reveals(reveals: bool, _: &mut App) {
    unsafe { bc::bc_set_option_reveals_sidebar(reveals) };
}

thread_local! {
    static ABOUT_LOGGED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// The app's version and its engine's, the licence it's under, and the notices of what it's built
/// from, each of which opens in a tab.
fn about() -> AnyElement {
    let mut version = format!("Version {}", build_info::version());
    if let Some(engine) = build_info::ENGINE_VERSION.as_deref() {
        version = format!("{version}, on Chromium {engine}");
    }
    // check.sh reads the engine's version here.
    let line = format!("browser: settings about {version}");
    ABOUT_LOGGED.with_borrow_mut(|logged| {
        if logged.insert(line.clone()) {
            log!("{line}");
        }
    });
    let open = |id: &'static str, url: Option<String>| {
        url.map(|url| {
            // check.sh clicks Open where this line says it is.
            let at = canvas(
                |_, _, _| {},
                move |bounds, _, _, _| {
                    let (x, y) = (f32::from(bounds.center().x), f32::from(bounds.center().y));
                    let line = format!("browser: settings open {id} at {x:.0} {y:.0}");
                    ABOUT_LOGGED.with_borrow_mut(|logged| {
                        if logged.insert(line.clone()) {
                            log!("{line}");
                        }
                    });
                },
            )
            .absolute()
            .inset_0();
            let open = button(&RowLook::settings(), id, "Open").relative().child(at);
            open.on_click(move |_, _, cx| open_notice(&url, cx)).into_any_element()
        })
    };
    let notices = [
        detailed_row(
            "Licence",
            concat!(
                env!("APP_NAME"),
                " is free software: you may use, study, share and change it under the GNU General \
                 Public License, version 3."
            ),
            open("licence", carried("LICENSE")),
        ),
        detailed_row(
            "Notices",
            "The projects it adapts code, icons and words from.",
            open("notices", carried("NOTICE")),
        ),
        detailed_row(
            "Libraries",
            "The Rust libraries built into the app, and their licences.",
            open("libraries", carried("THIRD-PARTY.txt")),
        ),
        detailed_row(
            "Chromium",
            "The engine's credits, and the licences of what it's built from.",
            open("chromium", Some("chrome://credits".into())),
        ),
    ];
    div()
        .child(group(&RowLook::settings(), [detailed_row(env!("APP_NAME"), version, None)]))
        .child(group(&RowLook::settings(), notices).mt(px(BOX_GAP)))
        .into_any_element()
}

/// The URL of `name`, one of the licence files the app carries (tools/licences.sh): in the bundle's
/// Resources on the Mac, beside the binary elsewhere. None where the build left it out, as `cargo
/// run` does.
fn carried(name: &str) -> Option<String> {
    let mut folder = std::env::current_exe().ok()?.parent()?.to_path_buf();
    if cfg!(target_os = "macos") {
        folder = folder.parent()?.join("Resources");
    }
    let path = folder.join(name);
    path.is_file().then(|| shell::file_url(&path.to_string_lossy()))
}

/// Opens one of About's notices in a new tab and goes to it, as More Settings… does.
fn open_notice(url: &str, cx: &mut App) {
    crate::open_tab(crate::front().0, url);
    hide(cx);
}

/// Asks for the folder `profile`'s downloads go to, in the system's panel, and sends them there.
fn choose_download_folder(profile: u64, cx: &mut App) {
    let chosen = move |folder: Option<PathBuf>, cx: &mut App| {
        let Some(folder) = folder else {
            return;
        };
        let Ok(path) = CString::new(folder.to_string_lossy().as_bytes()) else {
            return;
        };
        let set = unsafe { bc::bc_profile_set_download_folder(profile, path.as_ptr()) };
        let done = if set { "set" } else { "refused" };
        // check.sh waits for this line.
        log!("browser: download folder of profile {profile} {} {done}", folder.display());
        refresh(cx);
    };
    // A check run names its made-up folder, as it can't drive the system's panel.
    if let Some(folder) = std::env::var_os("BROWSER_DOWNLOAD_FOLDER") {
        return chosen(Some(folder.into()), cx);
    }
    let asked = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Choose".into()),
    });
    cx.spawn(async move |cx| {
        let folder = asked.await.ok().and_then(Result::ok).flatten();
        let folder = folder.and_then(|folders| folders.into_iter().next());
        cx.update(|cx| chosen(folder, cx));
    })
    .detach();
}

/// Opens Chromium's settings in a new tab, for what this window doesn't offer yet, and goes to it.
fn more_settings(cx: &mut App) {
    // check.sh waits for this line.
    log!("browser: more settings");
    crate::open_tab(crate::front().0, "chrome://settings");
    hide(cx);
}

/// Opens Chrome's password manager page in a new tab of one of `profile`'s Spaces and goes to it.
/// It's the one place a password is shown, copied, edited or exported, after the device's check,
/// which the page asks for itself and keeps for five minutes, as Chrome's does.
fn manage_passwords(profile: u64, cx: &mut App) {
    // check.sh waits for this line.
    log!("browser: manage passwords {profile}");
    crate::open_in_profile(profile, "chrome://password-manager/passwords");
    hide(cx);
}

/// A row with a grey line under its name, and its control, if any, at the right.
fn detailed_row(
    name: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: Option<AnyElement>,
) -> gpui::Div {
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(name.into()))
                .child(
                    div().text_size(px(text::SMALL)).text_color(rgb(DETAIL)).child(detail.into()),
                ),
        )
        .children(control.map(|control| div().flex_none().ml(px(PADDING)).child(control)))
}

/// A section's header, over its box.
fn header(title: &'static str) -> gpui::Div {
    div()
        .mb(px(HEADER_GAP))
        .pl(px(PADDING))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

/// A row with a line under its name that says more, and room after for its controls.
fn described(
    icon: Option<(&'static str, u32)>,
    name: SharedString,
    detail: Option<SharedString>,
) -> gpui::Div {
    let detail = detail.map(|detail| {
        div().truncate().text_size(px(text::SMALL)).text_color(rgb(DETAIL)).child(detail)
    });
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .children(icon.map(|(glyph, color)| tile(glyph, color)))
        .child(div().flex_1().min_w_0().child(div().truncate().child(name)).children(detail))
}

/// Keys as a chip, as the command bar shows a command's, in `color`.
fn chip(keys: SharedString, color: Rgba) -> gpui::Div {
    div()
        .h(px(CHIP_HEIGHT))
        .min_w(px(CHIP_WIDTH))
        .px(px(CHIP_PADDING))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHIP_RADIUS))
        .bg(rgba(CHIP))
        .text_size(px(text::SMALL))
        .text_color(color)
        .child(keys)
}

/// Why keys weren't given to a command, as its row says it.
fn refusal(refused: Refused) -> String {
    let holder = match refused {
        // Off the Mac ⌘ is pressed as Ctrl and ⌃ as Alt.
        Refused::Plain if shell::OFF_THE_MAC => return "Add Ctrl or Alt".into(),
        Refused::Plain => return "Add ⌘ or ⌃".into(),
        Refused::Taken(Holder::System(name)) => name,
        Refused::Taken(Holder::Command(command)) => command_name(command),
    };
    format!("Used by {holder}")
}

/// `command`'s name in the Shortcuts tab.
fn command_name(command: Command) -> &'static str {
    let rows = SHORTCUTS.iter().flat_map(|(_, rows)| rows.iter());
    let named = rows.filter_map(|(name, keys)| match keys {
        Keys::Of(each) if *each == command => Some(*name),
        _ => None,
    });
    named.chain(match command {
        Command::SelectTab(_) => Some("Tabs 1 to 8"),
        Command::RecentTabBack => Some("Last Used Tab"),
        Command::Undo => Some("Undo"),
        Command::Redo => Some("Redo"),
        _ => None,
    })
    .next()
    .unwrap_or("another command")
}

/// Where the user's keys are kept: in the profile directory, beside Chromium's profiles, as the
/// core keeps no settings of the app's own.
/// The names of the Shortcuts tab's rows that a search for `typed` finds, by their names or the
/// keys their chips show.
fn found(typed: &str, cx: &App) -> Vec<&'static str> {
    let user = shell::user_keys(cx);
    let rows = SHORTCUTS.iter().flat_map(|(_, rows)| rows.iter());
    let found = rows.filter(|(name, keys)| {
        let label = match keys {
            Keys::Of(command) => user.label(*command),
            Keys::FavoritesNumbered | Keys::LastTabNinth => None,
            _ => keys.fixed(),
        };
        shell::shortcut_found(typed, &shown(name), label.as_deref())
    });
    found.map(|(name, _)| *name).collect()
}

/// A row's name as the tab shows it, with ⌘1–⌘8 in this OS's keys, Ctrl+1–Ctrl+8 off the Mac.
fn shown(name: &str) -> String {
    let tabs = format!("{}–{}", shell::keystroke_label("cmd-1"), shell::keystroke_label("cmd-8"));
    name.replace("⌘1–⌘8", &tabs)
}

fn log_found(typed: &str, cx: &App) {
    // check.sh waits for this line.
    match typed {
        "" => log!("browser: shortcuts search cleared"),
        _ => {
            let found: Vec<String> = found(typed, cx).into_iter().map(shown).collect();
            log!("browser: shortcuts search {typed:?} finds {}", found.join(", "))
        }
    }
}

fn shortcuts_path() -> String {
    format!("{}/Shortcuts", crate::profile_dir())
}

/// Reads the keys the user gave the commands.
pub fn read_shortcuts(cx: &mut App) {
    let text = std::fs::read_to_string(shortcuts_path()).unwrap_or_default();
    cx.set_global(UserKeys::parse(&text));
}

/// Where View › Appearance's choice is kept, beside the keys.
fn appearance_path() -> String {
    format!("{}/Appearance", crate::profile_dir())
}

/// Reads View › Appearance's choice, automatic if there's none.
pub fn read_appearance(cx: &mut App) {
    let text = std::fs::read_to_string(appearance_path()).unwrap_or_default();
    cx.set_global(shell::AppAppearance(Appearance::named(text.trim()).unwrap_or_default()));
}

pub fn save_appearance(appearance: Appearance) {
    if let Err(error) = std::fs::write(appearance_path(), appearance.name()) {
        log!("couldn't save the appearance: {error}");
    }
}

/// Include Favorites in ⌘1–⌘8, kept with the user's keys.
fn set_favorites_numbered(numbered: bool, cx: &mut App) {
    shell::change_keys(cx, |keys| keys.set_favorites_numbered(numbered));
    save_shortcuts(cx);
}

/// Last Tab Goes to the Ninth Tab, kept with the user's keys.
fn set_last_tab_ninth(ninth: bool, cx: &mut App) {
    shell::change_keys(cx, |keys| keys.set_last_tab_ninth(ninth));
    save_shortcuts(cx);
}

fn save_shortcuts(cx: &App) {
    if let Err(error) = std::fs::write(shortcuts_path(), shell::user_keys(cx).to_text()) {
        log!("couldn't save the shortcuts: {error}");
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The app's window, not a Space's, so ilo green rather than a Space's accent (#319).
        let accent = rgb(browser::palette::NO_THEME_ACCENT);
        // As Linux's client-side decorations leave it, and Windows', where GPUI hides the system's
        // title bar and its buttons for a transparent one; AppKit's frame is always there.
        let frameless = cfg!(target_os = "windows")
            || matches!(window.window_decorations(), gpui::Decorations::Client { .. });
        if self.switched.is_some_and(|(_, at)| at.elapsed().as_secs_f32() < SLIDE) {
            window.request_animation_frame();
        }
        if std::mem::take(&mut self.shown) {
            // check.sh waits for this line, the tab each show or switch draws, whichever window
            // was key.
            log!("browser: settings drew {}", self.tab.name());
        }
        let content = match self.tab {
            Tab::General => {
                let off_sites = off_sites();
                self.log_off_sites(&off_sites);
                self.general(&off_sites, accent, cx)
            }
            Tab::Profiles => {
                let profiles = profiles();
                self.focus_field(&profiles, window, cx);
                self.profiles(&profiles, cx)
            }
            Tab::Sync => self.account.clone().into_any_element(),
            Tab::Passwords => self.passwords(&profiles(), accent, cx),
            Tab::Shortcuts => self.shortcuts(accent, cx),
            Tab::Privacy => self.privacy(accent, cx),
            Tab::Links => match &self.links {
                Some(links) => links.clone().into_any_element(),
                None => div().into_any_element(),
            },
            Tab::Import => match &self.import {
                Some(import) => import.clone().into_any_element(),
                None => div().into_any_element(),
            },
            Tab::Agents => match agents::global(cx) {
                Some(agents) => self.agents(&agents, accent, cx),
                None => div().into_any_element(),
            },
            Tab::Advanced => self.advanced(accent, cx),
            Tab::About => about(),
        };
        div()
            // While a command's key is recorded, every key is the recording's, ⌘W too.
            .when(self.recording.is_none(), |root| root.key_context("Settings"))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|settings, event: &KeyDownEvent, _, cx| {
                settings.record(&event.keystroke, cx)
            }))
            .on_action(|_: &CloseSettings, _, cx| hide(cx))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BACKGROUND))
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(rgb(TEXT))
            .child(self.bar(accent, frameless, cx))
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(SIDE))
                    .py(px(FIRST))
                    .child(content),
            )
            .children(self.cover(cx))
    }
}

impl Settings {
    #[cfg(target_os = "linux")]
    fn cover(&self, cx: &App) -> Option<AnyElement> {
        dialog::cover(&self.dialog, cx)
    }

    #[cfg(not(target_os = "linux"))]
    fn cover(&self, _: &App) -> Option<AnyElement> {
        None
    }
}
