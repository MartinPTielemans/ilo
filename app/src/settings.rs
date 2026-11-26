//! The Settings window (docs/ux-spec.md › Settings): a window of the browser's own, its sections
//! down a column on the window's ground and the one shown on a card, as a page is, its settings in
//! groups of rows. It opens on ⌘, and hides when closed, keeping its place and its section until
//! the browser quits.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use browser::account::{self, Account};
use browser::ad_block::choice::{self as ad_blocker, EngineRow};
use browser::agents::runner::Cli;
use browser::agents::{self, Agents, Client};
use browser::build_info;
use browser::default_browser::{self, DefaultBrowser, Os};
use browser::dialog;
use browser::import::ImportView;
use browser::link_rules::LinkRulesView;
use browser::list_keys::{self, Named};
use browser::little_launch;
use browser::log;
use browser::notes;
use browser::passwords::{self, Login};
use browser::product::PRODUCT;
use browser::shell::{self, CancelName, Command, Holder, KeepName, MenuEntry, Refused, UserKeys};
use browser::text_input::{self, Colors, TextInput};
use browser::theme::Appearance;
use browser::updates::{self, NightlyRow, Track, VersionRow};
use browser::widevine::{self, Widevine, WidevineRow};
use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontWeight,
    Global, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, PathPromptOptions, Rgba,
    SharedString, Subscription, Window, WindowHandle, WindowOptions, actions, canvas, div, point,
    prelude::*, px, rgb, rgba, size, svg,
};

use crate::{bc, ns_view, platform, show_menu};
use browser::menu::{self, Look};
use browser::palette::{INK, PAPER};
use browser::settings_card::{self, Card, Paging};
use browser::settings_column::{self, Section};
use browser::settings_pictures::{appearance_title, title as side_title};
use browser::settings_rows::{
    Act, Chip, Look as RowLook, SLIDE, assistant_name_alone, assistant_row, chevron_slot, chip,
    command_export, command_text, detailed_row, end, folder_popup, frame, glyph_plate, group,
    group_lined, mark_row, named_button, named_popup, ring, row, segmented, sent_part, stop_button,
    stop_chip, stop_popup, stop_restore, stop_switch, text,
};
use browser::settings_search::{
    self, EndSearch, Entry, NextControl, PageBottom, PageDown, PageTop, PageUp, PreviousControl,
    Search,
};
use browser::settings_shortcuts::{Keys, SHORTCUTS};
use browser::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
use browser::tokens::radius;
use browser::tokens::settings::form;
use browser::tokens::settings::{
    BUTTONS_TOP, CHIP_GAP, CONTROL_GAP, CONTROL_HEIGHT, CONTROL_PADDING, CONTROL_RADIUS,
    DETAIL_LINE, DETAIL_TOP, END_GAP, FIELD_PADDING, FIRST_GROUP, FOUND_FLASH, FOUND_FLASH_TIME,
    GLYPH_PLATE, GLYPH_PLATE_GAP, GROUP_GAP, HEADING_BOTTOM, HEADING_LINE, HEADING_TOP, HEADING_X,
    HEIGHT, LEDE_LINE, LEDE_TOP, NAME, PAGE_WIDTH, POPUP_WIDTH, ROW_X, SEARCH_PLACEHOLDER,
    SECTION_TEXT, SENT_TUCK, SHARE_TOP, SHORT_PADDING, SHORT_ROW, SIDE_ROOM, TEXT_X, TITLE, WIDTH,
};
use browser::tokens::shell::{
    WINDOW_BUTTON_GLYPH, WINDOW_BUTTON_SIZE, WINDOW_BUTTON_Y, WINDOWS_BUTTON_GLYPH,
    WINDOWS_BUTTON_PLATE, WINDOWS_CLOSE_HOVERED,
};
use browser::tokens::text;

mod assistants;
mod overview;

actions!(settings, [CloseSettings, ReleaseHeldProfile]);

/// What Arc offers for how long a Today tab waits unseen before it's archived, in seconds.
const ARCHIVE_AFTER: [(&str, f64); 5] = [
    ("12 hours (default)", 12. * 3600.),
    ("24 hours", 24. * 3600.),
    ("7 days", 7. * 24. * 3600.),
    ("30 days", 30. * 24. * 3600.),
    // Ours (#1636), Arc had none: Clear and ⌘W still archive (BC_ARCHIVE_NEVER).
    ("Never", ARCHIVE_NEVER),
];

/// The core's BC_ARCHIVE_NEVER.
const ARCHIVE_NEVER: f64 = -1.;

/// The times Settings offers a profile's tabs to wait before the archive, in its order, as first
/// run's bar has them.
pub(crate) fn archive_times() -> Vec<f64> {
    ARCHIVE_AFTER.iter().map(|&(_, seconds)| seconds).collect()
}

/// Chrome's page zoom choices, as its settings page's.
const PAGE_ZOOM: [(&str, f64); 11] = [
    ("50%", 0.5),
    ("67%", 2. / 3.),
    ("75%", 0.75),
    ("80%", 0.8),
    ("90%", 0.9),
    ("100% (default)", 1.),
    ("110%", 1.1),
    ("125%", 1.25),
    ("150%", 1.5),
    ("175%", 1.75),
    ("200%", 2.),
];

/// What's offered for how long a Little ilo window that isn't in use stays open before it closes
/// into the archive, in seconds. Arc's default is 6 hours; the rest are ours.
const LITTLE_ARCHIVE_AFTER: [(&str, f64); 4] = [
    ("1 hour", 3600.),
    ("6 hours (default)", 6. * 3600.),
    ("12 hours", 12. * 3600.),
    ("24 hours", 24. * 3600.),
];

/// How long a Today tab waits unseen before it's archived, in words: "12 hours".
pub(crate) fn archive_after_words() -> String {
    let after = unsafe { bc::bc_archive_after() };
    match ARCHIVE_AFTER.iter().find(|&&(_, seconds)| seconds == after) {
        Some((label, _)) => label.trim_end_matches(" (default)").to_owned(),
        None => format!("{:.0} hours", after / 3600.),
    }
}

/// One of a page's parts, which the page spaces by what comes before it, as a heading sits 20
/// under a group and 7 over its own.
enum Block {
    /// A heading over the group after it.
    Heading(SharedString),
    /// A heading with what's inside it after a ›, quieter, as Import › Arc.
    Crumb(SharedString, SharedString),
    /// A heading with the chip saying whose settings its group shows at its end.
    Scoped(SharedString, AnyElement),
    Group(gpui::Div),
    /// A group of rows the search can find.
    Rows(Vec<Row>),
    /// Buttons under the group before, such as New Profile.
    Under(gpui::Div),
    /// A view that spaces its own groups, such as the import's.
    View(AnyElement),
}

/// A setting's row, with what Settings' search reads of it.
struct Row {
    name: SharedString,
    line: Option<SharedString>,
    /// The keys its chips show, a shortcut's, a line each.
    keys: Option<String>,
    /// A feature's own setting, under it in its group with its text where the feature's name is.
    nested: bool,
    div: gpui::Div,
}

/// A section's page as its section builds it: the sentence under its name, what sits beside
/// them, and its blocks, which the card draws, or the search reads the rows of.
struct Page {
    section: Section,
    lede: SharedString,
    action: Option<AnyElement>,
    blocks: Vec<Block>,
}

impl Page {
    fn new(section: Section, lede: impl Into<SharedString>, blocks: Vec<Block>) -> Page {
        Page {
            section,
            lede: lede.into(),
            action: None,
            blocks,
        }
    }
}

/// The choice that has a command's keys reach ilo before the page.
const PREFER_ILO: &str = concat!("Prefer ", env!("PRODUCT_NAME"));

/// General's rows, as drawn, which their controls are named by too.
const DEFAULT_BROWSER: &str = "Default browser";
const SEARCH_WITH: &str = "Search with";
const SAVE_TO: &str = "Save downloads to";
/// Little Window's name (the words table's Z41a).
const LITTLE_WINDOW: &str = "Little Window";

/// The pop-up buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Popup {
    LittleArchiveAfter,
    PasswordsProfile,
    /// A profile's, in General.
    SearchEngine(u64),
    /// A profile's, in Tabs & Sidebar.
    ArchiveAfter(u64),
    /// A profile's New Note service, in Features.
    Notes(u64),
    /// A signed-in assistant's ⋯ menu, in Agents › Your assistant.
    Assistant(Cli),
    /// An assistant's Model, under its row in Agents › Your assistant.
    Model(Cli),
    /// Agents › Notifications' Notify.
    Notify,
    /// General's Page zoom.
    PageZoom,
    /// The chip saying whose settings a group shows.
    Scope,
}

impl Popup {
    /// What an assistive app calls it: its row's name.
    fn spoken(self) -> SharedString {
        match self {
            Popup::LittleArchiveAfter => "Archive windows left unused after".into(),
            Popup::PasswordsProfile | Popup::Scope => "Profile".into(),
            Popup::SearchEngine(_) => SEARCH_WITH.into(),
            Popup::ArchiveAfter(_) => "Archive Today tabs after".into(),
            Popup::Notes(_) => "New Note".into(),
            Popup::Assistant(cli) => {
                format!("{} options", browser::settings_rows::assistant_name(cli)).into()
            }
            Popup::Model(_) => browser::agents::task::MODEL.into(),
            Popup::Notify => browser::needs_you::WHEN_TO_NOTIFY.into(),
            Popup::PageZoom => "Page zoom".into(),
        }
    }
}

pub struct Settings {
    section: Section,
    focus: FocusHandle,
    /// Whether the window is dark, as View › Appearance or the system has it, which its rows,
    /// pop-up menus and the views it shows follow.
    dark: bool,
    /// View › Appearance's choice, which General's Appearance rings.
    appearance: Appearance,
    look: RowLook,
    /// The card the section shows on, which a section starts at the top of.
    card: Card,
    /// The column's sections, which scroll at the window's least height.
    sections: settings_column::Sections,
    /// It was asked to show, or changed section, and logs the section it draws next.
    shown: bool,
    /// An assistive app asks for the tree, so Settings names what it draws (#1381).
    a11y: bool,
    /// Which switch last changed, and when, for its knob's slide.
    switched: Option<(SharedString, Instant)>,
    /// The pop-up menu that is open, whose button shows pressed.
    choosing: Option<Popup>,
    /// The profile whose name is in a field for renaming.
    renaming: Option<Renaming>,
    /// Whether the field takes the keyboard once its row is drawn.
    focus_renaming: bool,
    /// The command whose next key Shortcuts takes, and which of its chips the key goes in.
    recording: Option<(Command, usize)>,
    /// Why the key pressed for this command wasn't given to it.
    refused: Option<(Command, SharedString)>,
    /// The search at the column's top, whose rows found replace the section's page while it
    /// holds any words.
    search: Entity<TextInput>,
    /// Ad Blocking's field that adds a filter to My filters.
    filter: Entity<TextInput>,
    /// Agents' field that adds a site to Always ask first, and whether it's open.
    fence_field: Entity<TextInput>,
    fence_adding: bool,
    /// What the search last logged it found, for check.sh.
    searched: String,
    /// The row a click on a found section's name showed, which flashes once, and when.
    flash: Option<(SharedString, Instant)>,
    /// That row, which gives its control the keyboard once it has drawn, and the handle the row
    /// takes for it, which is no stop of its own: the control is the stop after it.
    focus_row: Option<SharedString>,
    row_focus: FocusHandle,
    row_drawn: Rc<Cell<bool>>,
    default_browser: Entity<DefaultBrowser>,
    account: Entity<Account>,
    /// The sites with ad blocking off, as Privacy last logged them for check.sh.
    off_sites_logged: Option<String>,
    /// Where each control check.sh clicks has been logged, so it logs only where it moves to.
    drawn_logged: browser::settings_rows::Drawn,
    /// Where each pop-up button's text starts, as last drawn, for its menu to put the chosen
    /// item's over.
    popup_texts: Rc<RefCell<Vec<(Popup, gpui::Point<gpui::Pixels>)>>>,
    /// The profile whose logins Passwords lists, once the user has picked one.
    passwords_profile: Option<u64>,
    /// The profile General's search engine and downloads are shown for, once the user has picked
    /// one from its chip.
    scope: Option<u64>,
    /// Each profile's logins, as the core last told them.
    logins: HashMap<u64, Vec<Login>>,
    /// Each profile's saved addresses and cards, as the core last told them.
    autofill: HashMap<u64, Vec<Saved>>,
    /// The profiles whose logins were asked for since Passwords showed.
    asked: HashSet<u64>,
    /// The loose ends Overview's ✕ put away, read from the profile's folder at Overview's first
    /// draw, never at launch, when the search's index makes Settings.
    put_away: Option<browser::settings_overview::PutAway>,
    /// The assistants the user clicked on Models while the inside switch is off, looked up and
    /// shown as usual until the page is left.
    looked_up: HashSet<Cli>,
    /// Made when Import & Export first shows, so the other browsers' files are read only once the
    /// user asks.
    import: Option<Entity<ImportView>>,
    /// Draws the page again as the import changes, for its heading's crumb.
    _import: Option<Subscription>,
    /// The window's size as last drawn, logged as it changes for check.sh.
    drawn_size: Option<(f32, f32)>,
    /// Links' view, made again each time it shows.
    links: Option<Entity<LinkRulesView>>,
    /// Where it asks before deleting a profile, over all of it, as the browser window's dialog
    /// can't go over it on Linux.
    #[cfg(target_os = "linux")]
    dialog: Entity<dialog::Dialog>,
    widevine: Entity<WidevineRow>,
    /// Ad Blocking's way between the browser's own blocker and uBlock Origin.
    ad_blocker: Entity<EngineRow>,
    /// About's first row, which says where the update stands.
    version: Entity<VersionRow>,
    /// About's second row, where the build carries a nightly feed: Get nightly builds.
    nightly: Entity<NightlyRow>,
    /// Draws Agents again as the agents change.
    _agents: Option<Subscription>,
    /// The agents' requests opened in their activity, by entry, to show where each went and what
    /// it sent (#2326), and those of them showing every line.
    requests_opened: HashSet<u64>,
    requests_all: HashSet<u64>,
    /// The site notes opened to read, by host (#4122).
    notes_opened: HashSet<&'static str>,
}

/// Settings' dialog, which it draws in its own content, so there's nothing to show.
#[cfg(target_os = "linux")]
struct InWindow;

#[cfg(target_os = "linux")]
impl dialog::Host for InWindow {
    fn show(&self, _: Option<u64>, _: Option<Bounds<gpui::Pixels>>, _: bool) {}

    fn hide(&self) {}
}

/// The browser's account, as Sync asks the core for it.
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

    fn choose(&self, what: account::Choice, on: bool) {
        let what = match what {
            account::Choice::Settings => bc::BC_SYNC_SETTINGS,
            account::Choice::Extensions => bc::BC_SYNC_EXTENSIONS,
        };
        unsafe { bc::bc_sync_choose(what as i32, on) };
    }

    fn activity(&self) -> account::Activity {
        let mut activity = bc::bc_sync_activity::default();
        unsafe { bc::bc_sync_read_activity(&mut activity) };
        let mut day_starts = [0.; 4];
        unsafe { platform::host_day_starts(day_starts.as_mut_ptr()) };
        let kind = match activity.refused_kind as u32 {
            bc::BC_SYNC_KIND_SPACES => account::Kind::Spaces,
            bc::BC_SYNC_KIND_BOOKMARKS => account::Kind::Bookmarks,
            bc::BC_SYNC_KIND_PASSWORDS => account::Kind::Passwords,
            bc::BC_SYNC_KIND_OPEN_TABS => account::Kind::OpenTabs,
            bc::BC_SYNC_KIND_SETTINGS => account::Kind::Settings,
            bc::BC_SYNC_KIND_EXTENSIONS => account::Kind::Extensions,
            _ => account::Kind::Other,
        };
        let trouble = match activity.trouble as u32 {
            bc::BC_SYNC_TROUBLE_OFFLINE => Some(account::Trouble::Offline),
            bc::BC_SYNC_TROUBLE_SERVER => Some(account::Trouble::ServerError),
            bc::BC_SYNC_TROUBLE_REFUSED => Some(account::Trouble::Refused {
                kind,
                count: activity.refused.max(0) as u32,
            }),
            bc::BC_SYNC_TROUBLE_SIGN_IN => Some(account::Trouble::SignIn),
            bc::BC_SYNC_TROUBLE_THROTTLED => Some(account::Trouble::Throttled),
            bc::BC_SYNC_TROUBLE_STOPPED => Some(account::Trouble::Stopped),
            bc::BC_SYNC_TROUBLE_UPDATE => Some(account::Trouble::Update),
            _ => None,
        };
        // check.sh waits for this line.
        log!(
            "browser: sync activity {} {trouble:?}",
            if activity.last_synced > 0. {
                "synced"
            } else {
                "never"
            }
        );
        account::Activity {
            synced: account::synced_when(activity.last_synced, day_starts).into(),
            trouble,
            since: account::since_when(activity.since, day_starts).into(),
        }
    }

    fn open_sync_details(&self, cx: &mut App) {
        // The account syncs the profile the core started with, the first bc_profiles gives.
        let profile = crate::ids(bc::bc_profiles).first().copied().unwrap_or(0);
        crate::open_in_users_window(profile, "chrome://sync-internals");
        hide(cx);
    }

    fn delete_account(&self) {
        unsafe { bc::bc_sync_delete_account() };
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
            Some(dialog) => dialog::ask_in(
                dialog, message, detail, action, on_return, confirmed, window, cx,
            ),
            None => crate::with_overlays(
                move |cx| {
                    dialog::ask_returning(None, message, detail, action, on_return, confirmed, cx)
                },
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
    let changed = move |settings: &mut Settings, cx: &mut Context<Settings>| {
        settings
            .account
            .update(cx, |account, cx| account.set_state(state, cx));
    };
    update_made(cx, changed);
}

/// The account's devices, which the Sync tab asked the core for.
pub fn devices_changed(devices: Vec<account::Device>, cx: &mut App) {
    let changed = move |settings: &mut Settings, cx: &mut Context<Settings>| {
        settings
            .account
            .update(cx, |account, cx| account.set_devices(devices, cx));
    };
    update_made(cx, changed);
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

    fn restart(&self, cx: &mut App) {
        crate::restart_browser(cx);
    }
}

struct Renaming {
    profile: u64,
    name: Entity<TextInput>,
    /// Keep the name once the field loses the keyboard, however it goes, as the sidebar's do.
    _watches: Vec<Subscription>,
}

/// A profile as Profiles lists it, and the rows a profile keeps name it.
struct Profile {
    id: u64,
    name: SharedString,
    spaces: usize,
    /// As the core has it: not the one it started with, nor one a Space is in.
    deletable: bool,
    /// The favorites its tabs hold, which go with it.
    favorites: usize,
    /// Why it has no Delete, for the line under its name.
    kept: Option<SharedString>,
    /// Whether one of the user's Spaces is in it, for Clear Browsing Data's tab to open in.
    users_space: bool,
}

/// An address or a card a profile keeps, as Passwords lists it. A card's number stays in
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

/// A site the user turned ad blocking off on, as Privacy lists it.
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
    let spaces: Vec<u64> = crate::ids(bc::bc_spaces)
        .into_iter()
        .map(in_space)
        .collect();
    // An agent's Space, a task's or an outside client's, is never listed as a Space (#1080), so its
    // profile's count leaves it out, though it keeps the profile from being deleted.
    let users = crate::ids(bc::bc_spaces)
        .into_iter()
        .filter(|&space| crate::name(bc::bc_space_agent, space).is_empty());
    let users: Vec<u64> = users.map(in_space).collect();
    let listed = &users;
    let of_tab = |tab| unsafe { bc::bc_tab_profile(tab) };
    let favorites: Vec<u64> = crate::ids(bc::bc_favorites)
        .into_iter()
        .map(of_tab)
        .collect();
    let ids = crate::ids(bc::bc_profiles);
    let total = ids.len();
    (ids.into_iter().enumerate())
        .map(|(index, id)| {
            let count = spaces.iter().filter(|&&each| each == id).count();
            let listed = listed.iter().filter(|&&each| each == id).count();
            let favorites = favorites.iter().filter(|&&each| each == id).count();
            let kept = shell::profile_kept(index == 0, total, listed, count > listed);
            Profile {
                id,
                name: crate::name(bc::bc_profile_name, id),
                spaces: listed,
                deletable: index > 0 && count == 0,
                favorites,
                kept: kept.map(Into::into),
                users_space: users.contains(&id),
            }
        })
        .collect()
}

/// The archive pop-up's text for `seconds`, and which of ARCHIVE_AFTER it is.
fn archive_after(seconds: f64) -> (SharedString, Option<usize>) {
    chosen_time(&ARCHIVE_AFTER, seconds)
}

/// A time pop-up's text for `seconds`, and which of `times` it is.
fn chosen_time(times: &[(&'static str, f64)], seconds: f64) -> (SharedString, Option<usize>) {
    let chosen = times.iter().position(|&(_, each)| each == seconds);
    let label = match chosen {
        Some(index) => times[index].0.into(),
        None => format!("{:.0} hours", seconds / 3600.).into(),
    };
    (label, chosen)
}

/// The search engines `profile`'s default is picked from, each its keyword and name, and the
/// keyword of the default, or None while it loads or a policy or an extension picks it.
pub(crate) fn search_engines(
    profile: u64,
) -> Option<(Vec<(SharedString, SharedString)>, SharedString)> {
    let engines = crate::name(bc::bc_profile_search_engines, profile);
    let engines: Vec<(SharedString, SharedString)> = (engines.lines())
        .filter_map(|line| line.split_once('\t'))
        .map(|(keyword, name)| (keyword.to_string().into(), name.to_string().into()))
        .collect();
    (!engines.is_empty()).then(|| (engines, crate::name(bc::bc_profile_search_engine, profile)))
}

/// The keyword of the engine a profile searches with until the user picks one: DuckDuckGo, which
/// the core puts in the place of the engine ungoogled-chromium removed
/// (engine/patches/ours/chromium-search-default-duckduckgo.patch).
const FALLBACK_ENGINE: &str = "duckduckgo.com";

/// Makes the engine of `keyword`, named `name`, `profile`'s default, as Settings' pop-up and the
/// first launch's step do.
pub(crate) fn set_search_engine(profile: u64, keyword: &str, name: &str) {
    // check.sh waits for this line.
    log!("browser: profile {profile} searches with {name}");
    let keyword = CString::new(keyword).unwrap_or_default();
    unsafe { bc::bc_profile_set_search_engine(profile, keyword.as_ptr()) };
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

/// Shows the Settings window on Shortcuts.
pub fn show_shortcuts(cx: &mut App) {
    show_on(Section::Shortcuts, cx);
}

/// Shows the Settings window on Other apps, with each agent's activity.
pub fn show_agents(cx: &mut App) {
    show_on(Section::OtherApps, cx);
}

/// Shows the Settings window on About.
pub fn show_about(cx: &mut App) {
    show_on(Section::About, cx);
}

/// Shows the Settings window on Sync & Import.
pub fn show_import(cx: &mut App) {
    show_on(Section::SyncImport, cx);
}

fn show_on(section: Section, cx: &mut App) {
    cx.defer(move |cx| {
        show_now(cx);
        if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
            let show =
                |settings: &mut Settings, window: &mut Window, cx: &mut Context<Settings>| {
                    settings.show_section(section, window, cx)
                };
            window.update(cx, show).ok();
        }
    });
}

/// Shows the Settings window on Passwords, with `profile`'s logins, or those it showed.
pub fn show_passwords(profile: Option<u64>, cx: &mut App) {
    cx.defer(move |cx| {
        show_now(cx);
        if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
            let show =
                move |settings: &mut Settings, window: &mut Window, cx: &mut Context<Settings>| {
                    settings.passwords_profile = profile.or(settings.passwords_profile);
                    settings.show_section(Section::Passwords, window, cx)
                };
            window.update(cx, show).ok();
        }
    });
}

/// The profile's logins, which Passwords asked the core for, as they are now.
pub fn logins_changed(profile: u64, logins: Vec<Login>, cx: &mut App) {
    // check.sh waits for this line. The app has no passwords to log, only sites and usernames.
    log!(
        "browser: passwords {profile}: {}",
        passwords::describe(&logins)
    );
    let changed = move |settings: &mut Settings, cx: &mut Context<Settings>| {
        settings.logins.insert(profile, logins);
        cx.notify();
    };
    update_made(cx, changed);
}

/// The profile's saved addresses and cards, which Passwords asked the core for.
pub fn autofill_changed(profile: u64, saved: Vec<Saved>, cx: &mut App) {
    // check.sh waits for this line, which names nothing saved.
    let cards = saved.iter().filter(|each| each.card).count();
    log!(
        "browser: autofill {profile}: {} addresses, {cards} cards",
        saved.len() - cards
    );
    let changed = move |settings: &mut Settings, cx: &mut Context<Settings>| {
        settings.autofill.insert(profile, saved);
        cx.notify();
    };
    update_made(cx, changed);
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
            // On Settings itself, unless it opens at a row: typing goes to the search, as a
            // type-to-search list's does (type_to_search), while ⌘Z, Esc and the rest are
            // Settings' and its views' as they were.
            if settings.focus_row.is_none() {
                window.focus(&settings.focus, cx);
            }
            settings.shown = true;
            // Each show logs where its controls are drawn again, so check.sh can take a place from
            // this show rather than from one before it, which a hidden window's scroll left (#3841).
            settings.drawn_logged.borrow_mut().clear();
            // A show is the page shown again: Models' rows show the name alone again.
            settings.looked_up.clear();
            if settings.section == Section::OtherApps {
                look_for_clients(cx);
            }
            if settings.section == Section::Models {
                models_shown(cx);
            }
            if settings.section == Section::SyncImport {
                settings
                    .account
                    .update(cx, |account, cx| account.read_activity(cx));
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

/// Looks again for the agent clients on this computer, as Agents shows.
fn look_for_clients(cx: &mut App) {
    if let Some(agents) = agents::global(cx) {
        agents.update(cx, |agents, cx| {
            agents.look_for_clients(cx);
            agents.look_for_task_agents(cx);
            // Whether the command's skill files are still in place, for its row.
            agents.look_skills(cx);
        });
    }
}

/// Looks for the assistants as Models opens, so its rows say what's on this computer.
fn models_shown(cx: &mut App) {
    if let Some(agents) = agents::global(cx) {
        agents.update(cx, |agents, cx| agents.models_shown(cx));
    }
}

/// Reads again where Google's Widevine module is, as the core says it changed, once Settings has
/// been made, which the command bar can make before its window.
pub fn check_widevine(cx: &mut App) {
    let row = match cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        Some(window) => window
            .read_with(cx, |settings, _| settings.widevine.clone())
            .ok(),
        None => cx
            .try_global::<SettingsView>()
            .map(|view| view.0.read(cx).widevine.clone()),
    };
    if let Some(row) = row {
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
    log!(
        "browser: default browser {}",
        if default { "yes" } else { "no" }
    );
}

thread_local! {
    /// The window's title, the name of the section it shows.
    static SHOWN_SECTION: Cell<&'static str> = const { Cell::new("") };
}

/// The window's title, logged as the Mac's host reads it off the window.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn title() -> &'static str {
    SHOWN_SECTION.get()
}

fn open(cx: &mut App) {
    // ⌘W, pressed as Ctrl+W off the Mac, as the browser's own shortcuts are.
    let close = if crate::shell::OFF_THE_MAC {
        "ctrl-w"
    } else {
        "cmd-w"
    };
    cx.bind_keys([gpui::KeyBinding::new(
        close,
        CloseSettings,
        Some("Settings"),
    )]);
    // ⌘Z, Ctrl+Z off the Mac, which has no menu bar to send it; a field's own comes first.
    let undo = Box::new(browser::text_input::Undo);
    cx.bind_keys([shell::view_binding("cmd-z", undo, "Settings", cx)]);
    // A check run's key that lets the profile it holds back take its Spaces (import_host).
    if crate::import_host::holding() {
        let release = gpui::KeyBinding::new("shift-f12", ReleaseHeldProfile, Some("Settings"));
        cx.bind_keys([release]);
    }
    account::bind_keys(cx);
    settings_search::bind_keys(cx);
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    let title = Section::General.name().into();
    let options = WindowOptions {
        show: false,
        focus: false,
        ..browser::settings_column::window_options(bounds, title)
    };
    SHOWN_SECTION.set(Section::General.name());
    let window = cx.open_window(options, |window, cx| {
        let settings = view(cx);
        settings.update(cx, |settings, cx| {
            // Closing it hides it, so it opens again where the user left it.
            window.on_window_should_close(cx, |_, cx| {
                hide(cx);
                false
            });
            // The window follows View › Appearance, else the system, and the views it shows with
            // it.
            let dark = settings.dark;
            settings.follow_appearance(window, cx);
            // check.sh reads this line, and the one each change logs, which a change here did.
            if settings.dark == dark {
                log!(
                    "browser: settings look {}",
                    if dark { "dark" } else { "light" }
                );
            }
            // A control that had the keyboard from ⇥ and went, as a Remove's row does once its
            // Space or Return removed it, gives it back to Settings, or to the import it was in,
            // so ⌘W, Esc and ⌘Z still reach them.
            cx.on_focus_out(&settings.focus.clone(), window, |_, _, window, cx| {
                cx.defer_in(window, |settings, window, cx| {
                    settings.keep_the_keyboard(window, cx)
                });
            })
            .detach();
            cx.observe_window_appearance(window, |settings: &mut Settings, window, cx| {
                settings.follow_appearance(window, cx)
            })
            .detach();
            // A show's "settings drew" waits for the platform to report the window on screen
            // (#3270). The line says, when it doesn't come, whether a report came and whether the
            // window had a screen then; the frame asked here doesn't rely on a frame already asked.
            cx.observe_window_visibility(window, |_: &mut Settings, visibility, window, cx| {
                log!(
                    "browser: settings visibility {} screen {}",
                    if visibility.is_visible() {
                        "visible"
                    } else {
                        "hidden"
                    },
                    if window.display(cx).is_some() {
                        "yes"
                    } else {
                        "no"
                    }
                );
                cx.notify();
            })
            .detach();
            cx.observe_global_in::<shell::AppAppearance>(window, |settings, window, cx| {
                settings.follow_appearance(window, cx)
            })
            .detach();
            // The keyboard goes back to what had it as the dialog asked, or to Settings, once the
            // dialog has gone, as the dialog's focus still counts as focused then, though nothing
            // draws it.
            #[cfg(target_os = "linux")]
            {
                let mut asked = false;
                cx.observe_in(
                    &settings.dialog.clone(),
                    window,
                    move |settings: &mut Settings, dialog, window, cx| {
                        let asks = dialog::asks(&dialog, cx);
                        if asked && !asks {
                            dialog::give_back(&dialog, &settings.focus, window, cx);
                        }
                        asked = asks;
                    },
                )
                .detach();
            }
        });
        settings
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

/// Settings itself, made the first time it's needed: by its window as it opens, or by the
/// command bar, which finds its rows by what its own search reads (`index`), before any window.
struct SettingsView(Entity<Settings>);

impl Global for SettingsView {}

/// Changes Settings with what the core reported, once it's made: the command bar makes it before
/// its window for its index (#1873), and the window would open on what it read then. Through its
/// window once that's open, which does nothing from inside the window's own update, as a core
/// callback can come.
fn update_made(cx: &mut App, change: impl FnOnce(&mut Settings, &mut Context<Settings>)) {
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        window
            .update(cx, |settings, _, cx| change(settings, cx))
            .ok();
    } else if let Some(view) = cx.try_global::<SettingsView>().map(|view| view.0.clone()) {
        view.update(cx, change);
    }
}

fn view(cx: &mut App) -> Entity<Settings> {
    if let Some(view) = cx.try_global::<SettingsView>() {
        return view.0.clone();
    }
    let synced = account::synced(cx);
    let view = cx.new(|cx| {
        // As the system is until its window opens and follows View › Appearance, else the
        // system's, with the views it shows.
        let dark = dark_in(cx.window_appearance(), cx);
        let look = RowLook::settings(dark);
        let default_browser = cx.new(|_| DefaultBrowser::new(Rc::new(System), Os::current()));
        cx.observe(&default_browser, |_, _, cx| cx.notify())
            .detach();
        #[cfg(target_os = "linux")]
        let dialog = dialog::in_window(Rc::new(InWindow), cx);
        let host = CoreAccount {
            #[cfg(target_os = "linux")]
            dialog: Some(dialog.clone()),
        };
        let account = cx.new(|cx| {
            let mut account = Account::new(Rc::new(host), cx);
            account.set_state(synced, cx);
            account.set_look(look, cx);
            account
        });
        cx.observe(&account, |_, _, cx| cx.notify()).detach();
        let widevine = cx.new(|_| WidevineRow::new(Rc::new(Module)));
        cx.observe(&widevine, |_, _, cx| cx.notify()).detach();
        let launch = crate::ad_block_host::launch();
        let ad_blocker = cx.new(|_| EngineRow::new(Rc::new(crate::ad_block_host::Choice), launch));
        cx.observe(&ad_blocker, |_, _, cx| cx.notify()).detach();
        let version = cx.new(|cx| {
            let mut row = VersionRow::new(version(cx), cx);
            row.set_look(look, cx);
            row
        });
        let nightly = cx.new(|cx| {
            let mut row = NightlyRow::new(cx);
            row.set_look(look, cx);
            row
        });
        // The row shows only where the updater can follow its choice, which it says at launch.
        updates::observe_view(cx, |_, cx| cx.notify()).detach();
        let agents = agents::global(cx);
        let agents = agents.map(|agents| cx.observe(&agents, |_, _, cx| cx.notify()));
        // Each field takes the look Settings has as it draws, which follows the window.
        let settings: gpui::WeakEntity<Settings> = cx.weak_entity();
        let search = cx.new(|cx| {
            TextInput::new("Search", cx).follows(move |cx| {
                let Some(settings) = settings.upgrade() else {
                    return Colors::default();
                };
                let settings = settings.read(cx);
                // The field's "Search" over the field's own plate, not over a group's.
                Colors {
                    placeholder: settings.base().opacity(SEARCH_PLACEHOLDER).into(),
                    ..settings.look.field()
                }
            })
        });
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        let colors = browser::settings_rows::field_colors(cx, |settings: &Settings| settings.look);
        let filter = cx.new(|cx| {
            TextInput::new("example.com##.banner", cx)
                .follows(colors)
                .spoken("New filter")
        });
        let colors = browser::settings_rows::field_colors(cx, |settings: &Settings| settings.look);
        let fence_field = cx.new(|cx| {
            TextInput::new(agents::fence::PLACEHOLDER, cx)
                .follows(colors)
                .spoken(agents::fence::SPOKEN)
        });
        // ⇥ stops at the field, after the sections.
        let _ = search.read(cx).focus_handle(cx).tab_stop(true);
        Settings {
            account,
            #[cfg(target_os = "linux")]
            dialog,
            default_browser,
            widevine,
            ad_blocker,
            version,
            nightly,
            _agents: agents,
            section: Section::General,
            focus: cx.focus_handle(),
            dark,
            appearance: shell::app_appearance(cx),
            look,
            card: Card::default(),
            sections: Default::default(),
            shown: false,
            a11y: false,
            switched: None,
            choosing: None,
            renaming: None,
            focus_renaming: false,
            recording: None,
            refused: None,
            search,
            filter,
            fence_field,
            fence_adding: false,
            searched: String::new(),
            flash: None,
            focus_row: None,
            row_focus: cx.focus_handle().tab_stop(false),
            row_drawn: Rc::default(),
            off_sites_logged: None,
            drawn_logged: Rc::default(),
            popup_texts: Rc::default(),
            passwords_profile: None,
            scope: None,
            logins: HashMap::new(),
            autofill: HashMap::new(),
            asked: HashSet::new(),
            put_away: None,
            looked_up: HashSet::new(),
            requests_opened: HashSet::new(),
            notes_opened: HashSet::new(),
            requests_all: HashSet::new(),
            import: None,
            _import: None,
            drawn_size: None,
            links: None,
        }
    });
    cx.set_global(SettingsView(view.clone()));
    view
}

/// What Settings' search reads of its rows, for the command bar to find settings by the same
/// names, other names and keys. The bar asks once a time it opens, as it's typed in.
pub fn index() -> Option<Rc<Vec<Entry>>> {
    INDEX.with_borrow(Clone::clone)
}

/// Builds the index again in a task of its own, never on a frame of the command bar's: once after
/// launch, and once Settings hides or the core changes what it holds. The bar goes on reading the
/// one before until it's done.
pub fn index_later(cx: &mut App) {
    // Not before launch's own, after the UI shows, as a change the core reports while the
    // window opens would put the build on launch's way.
    if !INDEX_AFTER_LAUNCH.get() || INDEXING.replace(true) {
        return;
    }
    cx.spawn(async move |cx| {
        cx.update(|cx| {
            INDEXING.set(false);
            // Never while the bar shows, opening or typed in: it builds once the bar has gone.
            if browser::command_bar::is_open(cx) {
                INDEX_WANTED.set(true);
                return;
            }
            build_index(cx);
        })
    })
    .detach();
}

/// The command bar has gone: a build that waited for it starts.
pub fn bar_hidden() {
    if INDEX_WANTED.take() {
        crate::browser_windows::update_app_later(index_later);
    }
}

fn build_index(cx: &mut App) {
    let started = Instant::now();
    // Its rows are built only to be read, so in an arena freed as it ends: the fallback one GPUI
    // gives elements built outside a draw is never cleared, and each build added to it (#2138).
    let entries = gpui::with_scratch_element_arena(|| {
        view(cx).update(cx, |settings, cx| {
            settings.searched_rows(settings.look.on, cx).0
        })
    });
    // check.sh reads this line.
    log!(
        "browser: settings index {} rows in {:.1} ms",
        entries.len(),
        started.elapsed().as_secs_f64() * 1000.
    );
    INDEX.set(Some(Rc::new(entries)));
}

thread_local! {
    /// The index as last built: building every section's rows takes milliseconds, and the first
    /// time makes Settings, which the command bar's frames can't spare.
    static INDEX: RefCell<Option<Rc<Vec<Entry>>>> = const { RefCell::new(None) };
    /// A build is on its way.
    static INDEXING: Cell<bool> = const { Cell::new(false) };
    /// A build waits for the command bar to go.
    static INDEX_WANTED: Cell<bool> = const { Cell::new(false) };
    /// Launch has shown the UI and made the overlays, so builds may start.
    static INDEX_AFTER_LAUNCH: Cell<bool> = const { Cell::new(false) };
}

/// Launch's build, once the UI shows and the overlays are made: the first, which makes Settings.
pub fn index_after_launch(cx: &mut App) {
    INDEX_AFTER_LAUNCH.set(true);
    index_later(cx);
}

/// Shows Settings on `section` at the row named `row`, scrolled to it and flashed once, as a click
/// on a found section's name does: the command bar's settings rows.
pub fn show_row(section: Section, row: SharedString, cx: &mut App) {
    cx.defer(move |cx| {
        show_now(cx);
        let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) else {
            return;
        };
        window
            .update(cx, |settings, window, cx| {
                settings.show_found(section, row, window, cx)
            })
            .ok();
    });
}

/// Shows what changed in the core, such as a profile made, renamed or deleted, if it's open.
pub fn refresh(cx: &mut App) {
    index_later(cx);
    if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
        window.update(cx, |_, _, cx| cx.notify()).ok();
    }
}

/// Whether Settings is dark: as View › Appearance chose, else as the system is.
fn dark(window: &Window, cx: &App) -> bool {
    dark_in(window.appearance(), cx)
}

/// Whether Settings is dark in a window or a system of `appearance`.
fn dark_in(appearance: gpui::WindowAppearance, cx: &App) -> bool {
    match shell::app_appearance(cx) {
        Appearance::Light => false,
        Appearance::Dark => true,
        Appearance::Automatic => matches!(
            appearance,
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        ),
    }
}

/// The window's close button, for a window without a frame to draw it: in the column's top left,
/// where the browser window's is, on Linux and on Windows alike, as the browser window's buttons
/// stay top left on Windows for now. Settings can't be minimized or zoomed, so it's alone. `at`
/// logs where it's drawn.
fn close_button(base: Rgba, at: gpui::Canvas<()>, a11y: bool) -> AnyElement {
    if cfg!(target_os = "windows") {
        return windows_close_button(base, at, a11y);
    }
    let (x, colour) = shell::WINDOW_BUTTONS[0];
    div()
        .id("close")
        .when(a11y, |close| named_button(close, "Close"))
        .absolute()
        .left(px(x - WINDOW_BUTTON_SIZE / 2.))
        .top(px(WINDOW_BUTTON_Y - WINDOW_BUTTON_SIZE / 2.))
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

/// Windows' close, as the browser window's (shell.rs's `windows_buttons`): the column's grey
/// glyph, on Windows' red under the pointer, centred where the Mac's close is.
fn windows_close_button(base: Rgba, at: gpui::Canvas<()>, a11y: bool) -> AnyElement {
    let (x, _) = shell::WINDOW_BUTTONS[0];
    div()
        .id("close")
        .when(a11y, |close| named_button(close, "Close"))
        .absolute()
        .left(px(x - WINDOWS_BUTTON_PLATE / 2.))
        .top(px(WINDOW_BUTTON_Y - WINDOWS_BUTTON_PLATE / 2.))
        .size(px(WINDOWS_BUTTON_PLATE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .group("close")
        .hover(|style| style.bg(rgb(WINDOWS_CLOSE_HOVERED)))
        // A press here closes the window, and doesn't move it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| hide(cx))
        .child(
            svg()
                .path("icons/window-windows-close.svg")
                .size(px(WINDOWS_BUTTON_GLYPH))
                .text_color(base.opacity(SECTION_TEXT))
                .group_hover("close", |style| style.text_color(gpui::white())),
        )
        .child(at)
        .into_any_element()
}

fn hide(cx: &mut App) {
    // check.sh waits for this line.
    log!("browser: settings hidden");
    index_later(cx);
    unsafe { platform::host_hide_settings() };
    // Later, as it can come from inside the window's own update. What Links' ⌘Z would bring back
    // goes with the window, as the issue's "until Settings closes" (#1377).
    cx.defer(|cx| {
        if let Some(window) = cx.try_global::<SettingsWindow>().map(|settings| settings.0) {
            window
                .update(cx, |settings, _, cx| settings.forget_undo(cx))
                .ok();
        }
    });
}

impl Settings {
    /// ⌘Z: on Links, the last rule its trash took comes back. The other sections' changes show
    /// as they're made and are made back the same way.
    fn undo(&mut self, cx: &mut Context<Self>) {
        let links = (self.links.as_ref()).filter(|_| self.section == Section::Links);
        let undone = links.is_some_and(|links| links.update(cx, |links, cx| links.undo(cx)));
        let what = if undone {
            "brought a rule back"
        } else {
            "had nothing"
        };
        // check.sh reads this line.
        log!("browser: settings undo {what}");
    }

    fn forget_undo(&mut self, cx: &mut Context<Self>) {
        if let Some(links) = &self.links {
            links.update(cx, |links, _| links.forget_removed());
        }
    }

    fn show_section(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if section == self.section {
            return;
        }
        self.keep_name(window, cx);
        self.recording = None;
        self.refused = None;
        // Read again each time it shows: a profile closed since has the core forget it was asked.
        self.asked.clear();
        // Models' rows show the name alone again once left, while the inside switch is off.
        self.looked_up.clear();
        // check.sh waits for this line.
        log!("browser: settings tab {}", section.name());
        self.section = section;
        // A section takes no click until it has drawn, so check.sh waits for its "settings drew".
        self.shown = true;
        // Each section opens at its top.
        self.card.reset();
        if section == Section::SyncImport {
            self.show_import(cx);
        }
        if section == Section::OtherApps {
            look_for_clients(cx);
        }
        if section == Section::Models {
            models_shown(cx);
        }
        if section == Section::SyncImport {
            self.account
                .update(cx, |account, cx| account.read_activity(cx));
        }
        let look = self.look;
        self.links = (section == Section::Links).then(|| {
            let host = Rc::new(crate::link_rules_host::CoreLinkRules);
            cx.new(|cx| {
                let mut links = LinkRulesView::new(host, cx);
                links.set_look(look, cx);
                links
            })
        });
        window.set_window_title(section.name());
        SHOWN_SECTION.set(section.name());
        cx.notify();
    }

    /// The import's view, made the first time, else with the browsers found again, as they may
    /// have changed since.
    fn show_import(&mut self, cx: &mut Context<Self>) {
        match &self.import {
            Some(import) => import.update(cx, |import, cx| import.refresh(cx)),
            None => {
                let (host, look) = (Rc::new(crate::import_host::CoreImport), self.look);
                // Its heading is the page's, naming the browser it's inside of, and it takes Esc.
                let import = cx.new(|cx| {
                    let mut import = ImportView::new(host, false).headed();
                    import.set_look(look, cx);
                    import.take_keys(cx);
                    import
                });
                self._import = Some(cx.observe(&import, |_, _, cx| cx.notify()));
                self.import = Some(import);
            }
        }
    }

    /// Takes the look View › Appearance, else the system, gives it now, and hands it to the views
    /// it shows.
    fn follow_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A choice that leaves the look as it was, such as Automatic after Light on a light
        // system, still moves Appearance's ring.
        let appearance = shell::app_appearance(cx);
        if appearance != self.appearance {
            self.appearance = appearance;
            // check.sh waits for this line.
            log!(
                "browser: settings appearance {}",
                appearance_title(appearance)
            );
            cx.notify();
        }
        let dark = dark(window, cx);
        if dark == self.dark {
            return;
        }
        let look = RowLook::settings(dark);
        (self.dark, self.look) = (dark, look);
        self.account
            .update(cx, |account, cx| account.set_look(look, cx));
        self.version.update(cx, |row, cx| row.set_look(look, cx));
        self.nightly.update(cx, |row, cx| row.set_look(look, cx));
        if let Some(import) = &self.import {
            import.update(cx, |import, cx| import.set_look(look, cx));
        }
        if let Some(links) = &self.links {
            links.update(cx, |links, cx| links.set_look(look, cx));
        }
        // check.sh waits for this line.
        log!(
            "browser: settings look {}",
            if dark { "dark" } else { "light" }
        );
        cx.notify();
    }

    /// The paper in dark and the ink in light, which the window's text is alphas of.
    fn base(&self) -> Rgba {
        if self.dark { rgb(PAPER) } else { rgb(INK) }
    }

    /// Pop-up menus light or dark as the window is.
    fn menu_look(&self) -> Look {
        Look::plain(!self.dark)
    }

    /// A section's page: its name in the display face, or for About the app's, the sentence that
    /// says what it's for, its action beside them, and its blocks, each spaced by what's before it.
    fn page(&self, page: Page, cx: &mut App) -> gpui::Div {
        let Page {
            section,
            lede,
            action,
            blocks,
        } = page;
        // The one page whose heading is the app's name, with the line the first launch shows
        // under the wordmark.
        let title = match section {
            Section::About => env!("PRODUCT_NAME"),
            section => section.name(),
        };
        let base = self.base();
        // In the heading face, 22 on 28, semibold (spec-settings.md › The window).
        let title = shell::display_text(title, form::TITLE_SIZE, form::TITLE_LINE, cx)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(base.opacity(TITLE));
        let lede = (!lede.is_empty()).then(|| {
            div()
                .mt(px(LEDE_TOP))
                .line_height(px(LEDE_LINE))
                .text_color(self.look.detail)
                .child(lede)
        });
        let head = div()
            .px(px(TEXT_X))
            .flex()
            .items_start()
            .child(div().flex_1().min_w_0().child(title).children(lede))
            .children(action.map(|action| div().flex_none().ml(px(END_GAP)).child(action)));
        let mut after_heading = None;
        let blocks = blocks.into_iter().map(|block| {
            let top = match (after_heading, &block) {
                (_, Block::Heading(_) | Block::Crumb(..) | Block::Scoped(..)) => HEADING_TOP,
                (None, _) => FIRST_GROUP,
                (Some(true), _) => HEADING_BOTTOM,
                (_, Block::Under(_)) => BUTTONS_TOP,
                _ => GROUP_GAP,
            };
            after_heading = Some(matches!(
                block,
                Block::Heading(_) | Block::Crumb(..) | Block::Scoped(..)
            ));
            let block = match block {
                Block::Heading(heading) => self.heading(heading).into_any_element(),
                Block::Crumb(heading, inside) => {
                    let inside = div()
                        .text_color(self.look.detail)
                        .child(format!(" › {inside}"));
                    self.heading(heading)
                        .flex()
                        .child(inside)
                        .into_any_element()
                }
                Block::Scoped(heading, chip) => self
                    .heading(heading)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(chip)
                    .into_any_element(),
                Block::Group(group) | Block::Under(group) => group.into_any_element(),
                Block::Rows(rows) => self.rows(rows).into_any_element(),
                Block::View(view) => view,
            };
            div().mt(px(top)).child(block)
        });
        let blocks: Vec<gpui::Div> = blocks.collect();
        // One column, centred on the card (#4092).
        div()
            .w_full()
            .max_w(px(PAGE_WIDTH))
            .mx_auto()
            .flex()
            .flex_col()
            .child(head)
            .children(blocks)
    }

    /// A heading over the group after it, 4 in from the groups' edge, as drawn.
    fn heading(&self, heading: SharedString) -> gpui::Div {
        div()
            .relative()
            .child(self.card.mark(heading.clone()))
            .px(px(HEADING_X))
            .line_height(px(HEADING_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.look.heading)
            .child(heading)
    }

    /// A group of `rows`, a feature's own settings under it with the hairline from their text, and
    /// the row a click on a found section's name went to flashing once.
    fn rows(&self, rows: Vec<Row>) -> gpui::Div {
        let x = ROW_X + GLYPH_PLATE + GLYPH_PLATE_GAP;
        let rows = rows.into_iter().map(|row| {
            let div = self.flashed(&row.name, row.div);
            match row.nested {
                true => (div.pl(px(x)), x),
                false => (div, 0.),
            }
        });
        group_lined(&self.look, rows)
    }

    /// `name`'s row once a profile, by what `control` gives each: a plain row with one profile,
    /// else one a profile, its name in grey before its control, the line under the first alone.
    fn per_profile(
        &self,
        profiles: &[Profile],
        name: &'static str,
        line: Option<SharedString>,
        mut control: impl FnMut(&Profile) -> Option<AnyElement>,
    ) -> Vec<Row> {
        let several = profiles.len() > 1;
        let mut rows = Vec::new();
        for profile in profiles {
            let Some(control) = control(profile) else {
                continue;
            };
            let named = several.then(|| {
                let name = div()
                    .max_w(px(POPUP_WIDTH))
                    .truncate()
                    .child(profile.name.clone());
                name.text_color(self.look.detail).into_any_element()
            });
            let line = line.clone().filter(|_| rows.is_empty());
            rows.push(setting(
                &self.look,
                name,
                line,
                named.into_iter().chain([control]).collect(),
            ));
        }
        rows
    }

    /// General, as drawn (S2): whether the browser is the default, with Make Default while it
    /// isn't; how it opens and quits; the search engine and downloads of the profile its chip
    /// shows, which picks another while there are several; and Little Window's button where the
    /// OS has a place every desktop shows.
    fn general(&self, profiles: &[Profile], accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let default = self.default_browser.read(cx);
        let (line, make) = match (default.is_default(), default.offers()) {
            (true, _) => (
                Some(format!("{PRODUCT} is your default browser").into()),
                None,
            ),
            (false, offers) => {
                let line = format!(
                    "{PRODUCT} isn't your default, so links from other apps open somewhere else."
                );
                let line: SharedString = match offers {
                    true => line.into(),
                    false => format!("{line} {}", default.detail()).into(),
                };
                let make = offers.then(|| {
                    let make = cx.listener(|settings: &mut Self, _, _, cx| {
                        log!("browser: make default browser");
                        settings.default_browser.read(cx).make_default();
                    });
                    // The page's one main button, a stop as every button here is.
                    browser::settings_rows::primary(look, "make-default-browser", "Make Default")
                        .when(self.a11y, |make| named_button(make, "Make Default"))
                        .tab_index(0)
                        .focus_from_keys_only()
                        .focus_visible(ring(look.ink))
                        .relative()
                        .child(self.drawn("make default".into()))
                        .on_click(make)
                        .into_any_element()
                });
                (Some(line), make)
            }
        };
        let default = Row {
            div: mark_row(
                look,
                DEFAULT_BROWSER,
                line.clone(),
                make.into_iter().collect(),
            ),
            name: DEFAULT_BROWSER.into(),
            line,
            keys: None,
            nested: false,
        };
        let restore = unsafe { bc::bc_restore_windows() };
        let opens: [SharedString; 2] = ["Bring back my windows".into(), "Start fresh".into()];
        let opens = segmented(
            look,
            "when-opens",
            &opens,
            Some(if restore { 0 } else { 1 }),
            self.a11y,
            |index, segment| {
                let back = index == 0;
                segment
                    .child(self.drawn(format!("segment {}", opens[index])))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if back == restore {
                            return;
                        }
                        // check.sh waits for this line.
                        log!(
                            "browser: restore windows {}",
                            if back { "on" } else { "off" }
                        );
                        set_restore_windows(back, cx);
                        cx.notify();
                    }))
            },
        );
        let warn = self.toggle(
            "warn before quitting",
            "Ask before quitting",
            unsafe { bc::bc_warn_before_quitting() },
            set_warn_before_quitting,
            accent,
            cx,
        );
        let opening = vec![
            setting(
                look,
                format!("When {PRODUCT} opens"),
                None,
                vec![opens.into_any_element()],
            ),
            setting(look, "Ask before quitting", None, vec![warn]),
        ];
        // The profile the chip shows; each keeps its own, once the core has loaded it.
        let mut found = Vec::new();
        if let Some(id) = self.scoped(profiles) {
            if let Some((engines, keyword)) = search_engines(id) {
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
                found.push(setting(
                    look,
                    SEARCH_WITH,
                    None,
                    vec![popup.into_any_element()],
                ));
            }
            if let Some(folder) = download_folder(id) {
                // The folder's own name, as drawn; an assistive app reads where it is.
                let name = std::path::Path::new(folder.as_ref())
                    .file_name()
                    .map_or(folder.clone(), |name| {
                        name.to_string_lossy().into_owned().into()
                    });
                let picker = folder_popup(look, ("download-folder", id), SAVE_TO, name, self.a11y)
                    .relative()
                    .child(self.drawn(format!("download folder {id}")))
                    .on_click(cx.listener(move |_, _, _, cx| choose_download_folder(id, cx)));
                found.push(setting(
                    look,
                    SAVE_TO,
                    None,
                    vec![picker.into_any_element()],
                ));
            }
        }
        let ask = self.toggle(
            "ask where to save",
            "Ask where to save each file",
            unsafe { bc::bc_ask_where_to_save() },
            set_ask_where_to_save,
            accent,
            cx,
        );
        found.push(setting(
            look,
            "Ask where to save each file",
            None,
            vec![ask],
        ));
        // The Mac's menu bar and Windows' notification area, each by its own name; Linux has no
        // place every desktop shows (browser::little_launch). With Little Window off, none (#1636).
        let little_on = unsafe { bc::bc_little_windows() };
        let os = little_launch::os();
        let menu_bar = (little_launch::button(os).filter(|_| little_on)).map(|button| {
            let switch = self.toggle(
                "little in the menu bar",
                button.row,
                crate::little_in_menu_bar(),
                set_little_in_menu_bar,
                accent,
                cx,
            );
            let line = match os {
                little_launch::Os::Mac => {
                    "A small window from the menu bar, for a quick look without the browser."
                }
                _ => button.line,
            };
            let row = setting(look, LITTLE_WINDOW, Some(line.into()), vec![switch]);
            (format!("In the {}", button.place), row)
        });
        let downloads: SharedString = "Search and downloads".into();
        let mut blocks = vec![
            Block::Rows(vec![default]),
            Block::Heading("Opening and quitting".into()),
            Block::Rows(opening),
            match self.scope_chip(profiles, cx) {
                Some(chip) => Block::Scoped(downloads, chip),
                None => Block::Heading(downloads),
            },
            Block::Rows(found),
        ];
        if let Some((heading, row)) = menu_bar {
            blocks.push(Block::Heading(heading.into()));
            blocks.push(Block::Rows(vec![row]));
        }
        Page::new(Section::General, "", blocks)
    }

    /// The profile whose settings General shows: the one picked from its chip while it's there,
    /// or else the one the browser started with.
    fn scoped(&self, profiles: &[Profile]) -> Option<u64> {
        let picked = self
            .scope
            .filter(|&id| profiles.iter().any(|each| each.id == id));
        picked.or(profiles.first().map(|profile| profile.id))
    }

    /// The chip saying whose settings a group shows, which lists the profiles to pick another
    /// from, while there are several.
    fn scope_chip(&self, profiles: &[Profile], cx: &mut Context<Self>) -> Option<AnyElement> {
        if profiles.len() < 2 {
            return None;
        }
        let shown = self.scoped(profiles)?;
        let chosen = profiles.iter().position(|each| each.id == shown);
        let names: Vec<SharedString> = profiles.iter().map(|each| each.name.clone()).collect();
        let ids: Vec<u64> = profiles.iter().map(|each| each.id).collect();
        let name = chosen.map(|index| names[index].clone()).unwrap_or_default();
        let open = self.choosing == Some(Popup::Scope);
        let (keyed_names, keyed_ids) = (names.clone(), ids.clone());
        let pressed = cx.listener(move |settings: &mut Self, _: &MouseDownEvent, window, cx| {
            let (names, ids) = (names.clone(), ids.clone());
            settings.choose_profile(Popup::Scope, names, ids, chosen, window, cx)
        });
        // Space or Return opens its menu as a press does.
        let keyed = cx.listener(
            move |settings: &mut Self, event: &gpui::ClickEvent, window, cx| {
                if event.is_keyboard() {
                    let (names, ids) = (keyed_names.clone(), keyed_ids.clone());
                    settings.choose_profile(Popup::Scope, names, ids, chosen, window, cx)
                }
            },
        );
        let chip = browser::settings_rows::scope(&self.look, name.clone(), open)
            .relative()
            .id("scope")
            .when(self.a11y, |chip| {
                named_popup(chip, Popup::Scope.spoken(), name)
            })
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(ring(self.look.ink))
            .child(self.menu_at(Popup::Scope))
            .child(self.drawn("scope".into()))
            .on_mouse_down(MouseButton::Left, pressed)
            .on_click(keyed);
        Some(chip.into_any_element())
    }

    /// Appearance (#4092): the settings whose choice is a picture, Appearance and Sidebar
    /// position, with Page zoom beside them.
    fn appearance_page(&self, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let appearance = self.appearance;
        let appearance_line: SharedString = match cfg!(target_os = "macos") {
            true => "Light, dark, or as macOS is. View › Appearance has it too.",
            false => "Light, dark, or as the system is.",
        }
        .into();
        let appearances = browser::settings_pictures::appearance_picker(
            look,
            self.base(),
            appearance,
            self.a11y,
            self.drawn_logged.clone(),
            |appearance, _, cx| {
                crate::browser_windows::update_shells_in(cx, |shell, cx| {
                    shell.run(Command::Appearance(appearance), "settings", cx)
                })
            },
        )
        .relative()
        // Drawn once for each choice the first time it's shown, which check.sh waits for.
        .child(self.drawn(format!("appearance {}", appearance_title(appearance))));
        let zoom = unsafe { bc::bc_page_zoom() };
        let zoom_chosen = PAGE_ZOOM
            .iter()
            .position(|&(_, each)| (each - zoom).abs() < 0.001);
        let zoom_label: SharedString = match zoom_chosen {
            Some(index) => PAGE_ZOOM[index].0.into(),
            None => format!("{:.0}%", zoom * 100.).into(),
        };
        let zoom_popup = self.popup(
            Popup::PageZoom,
            zoom_label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_page_zoom(zoom_chosen, window, cx)
            }),
        );
        let zoom = setting(
            look,
            "Page zoom",
            Some("Every page's size, until you zoom a site, which keeps its own.".into()),
            vec![
                zoom_popup
                    .child(self.drawn("page zoom".into()))
                    .into_any_element(),
            ],
        );
        // Transparency, directly under Appearance, where the window has glass to show through.
        let transparency = browser::transparency::row(
            look,
            browser::transparency::has_glass(),
            self.drawn_logged.clone(),
            cx,
        )
        .map(|div| Row {
            div,
            name: browser::transparency::NAME.into(),
            line: Some(browser::transparency::ABOUT.into()),
            keys: None,
            nested: false,
        });
        let mut looks = vec![pictured(look, "Appearance", appearance_line, appearances)];
        // On the Mac the system's Reduce Transparency is said under the group, so Page zoom
        // starts the next one.
        let mut groups = match transparency {
            Some(transparency) if cfg!(target_os = "macos") => {
                looks.push(transparency);
                let foot = foot(look, browser::transparency::REDUCE_TRANSPARENCY_WINS);
                vec![
                    Block::Rows(looks),
                    Block::Under(foot),
                    Block::Rows(vec![zoom]),
                ]
            }
            transparency => {
                looks.extend(transparency);
                looks.push(zoom);
                vec![Block::Rows(looks)]
            }
        };
        let right = shell::sidebar_right(cx);
        let pair = browser::settings_pictures::picker(
            look,
            self.base(),
            self.dark,
            right,
            self.a11y,
            self.drawn_logged.clone(),
            |side, _, cx| set_sidebar_right(side, cx),
        )
        .relative()
        // Drawn again once the side changes, which check.sh waits for.
        .child(self.drawn(format!("sidebar position {}", side_title(right))));
        let position = pictured(
            look,
            "Sidebar position",
            "The side of the window the sidebar, the Library and the window's buttons are on."
                .into(),
            pair,
        );
        groups.extend([Block::Heading("Layout".into()), Block::Rows(vec![position])]);
        Page::new(Section::Window, "", groups)
    }

    /// Tabs & Sidebar: when Today tabs are archived and unloaded, whether Battery Saver comes on, who
    /// shares the favorites and how the sidebar comes while hidden.
    fn tabs_and_sidebar(&self, profiles: &[Profile], accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let reopen = shell::keystroke_label("shift-cmd-t");
        let archive = self.per_profile(
            profiles,
            "Archive Today tabs after",
            Some(
                format!("Tabs you haven't looked at go to the archive. {reopen} brings one back.")
                    .into(),
            ),
            |profile| {
                let id = profile.id;
                // Nothing to show until the core has loaded it, which Never (-1) isn't.
                let after = unsafe { bc::bc_profile_archive_after(id) };
                (after != 0.).then(|| {
                    let (label, chosen) = archive_after(after);
                    let popup = self.popup(
                        Popup::ArchiveAfter(id),
                        label,
                        POPUP_WIDTH,
                        cx.listener(move |settings, _, window, cx| {
                            settings.choose_archive_after(id, chosen, window, cx)
                        }),
                    );
                    popup
                        .child(self.drawn(format!("archive {id}")))
                        .into_any_element()
                })
            },
        );
        let unload = unsafe { bc::bc_unload_idle_tabs() };
        let unload = self.toggle(
            "unload idle tabs",
            "Unload tabs you haven't looked at",
            unload,
            set_unload_idle_tabs,
            accent,
            cx,
        );
        let unload = setting(
            look,
            "Unload tabs you haven't looked at",
            Some(
                "After 2 hours out of sight a tab frees its memory, and loads again when you \
                 show it."
                    .into(),
            ),
            vec![unload],
        );
        let battery = self.toggle(
            "battery saver setting",
            "Battery Saver",
            unsafe { bc::bc_battery_saver_enabled() },
            set_battery_saver,
            accent,
            cx,
        );
        let battery = setting(
            look,
            "Battery Saver",
            Some(
                "Below 20% on battery, pages draw less often and tabs out of sight that keep \
                 busy pause."
                    .into(),
            ),
            vec![battery],
        );
        let scope = favorites_scope();
        let settings = cx.weak_entity();
        let shared = browser::settings_pictures::favorites_picker(
            look,
            self.base(),
            self.dark,
            scope,
            self.a11y,
            self.drawn_logged.clone(),
            // The core keeps the favorites it had, copying or merging the lists, and tells the
            // windows (`favorites_changed`).
            move |scope, _, cx| {
                // check.sh waits for this line.
                log!("browser: favorites scope {}", scope.title());
                unsafe { bc::bc_set_favorites_scope(scope.to_core()) };
                settings.update(cx, |_, cx| cx.notify()).ok();
            },
        )
        .relative()
        // Drawn again once the scope changes, which check.sh waits for.
        .child(self.drawn(format!("share favorites {}", scope.title())));
        let favorites_line: SharedString = "Which Spaces show the same favorites.".into();
        // As Sidebar position's: the drawings under the row's line, their ring's room outside it.
        let favorites = Row {
            div: frame()
                .flex_col()
                .items_stretch()
                .child(text(look, "Share favorites", Some(favorites_line.clone())))
                .child(
                    div()
                        .flex()
                        .mt(px(SHARE_TOP))
                        .ml(px(-SIDE_ROOM))
                        .child(shared),
                ),
            name: "Share favorites".into(),
            line: Some(favorites_line),
            keys: None,
            nested: false,
        };
        let moving = self.toggle(
            "warn moving profiles",
            "Ask before moving a tab to another profile",
            unsafe { bc::bc_warn_moving_profiles() },
            set_warn_moving_profiles,
            accent,
            cx,
        );
        let moving = setting(
            look,
            "Ask before moving a tab to another profile",
            Some("Its page loads again there, without this profile's logins and cookies.".into()),
            vec![moving],
        );
        // Drawn only where the broom can be, which is also where the core keeps the switch.
        let broom = crate::broom_host::can_be(cx);
        let sort = self.per_profile(
            profiles,
            "Sort tabs into folders",
            Some(
                format!(
                    "When you sort, {PRODUCT} sends the titles and site names of today's tabs to \
                     your assistant."
                )
                .into(),
            ),
            |profile| {
                let id = profile.id;
                broom.then(|| {
                    self.toggle(
                        format!("sort tabs {id}"),
                        "Sort tabs into folders",
                        crate::broom_host::on(id),
                        move |on, cx| crate::broom_host::set_on(id, on, cx),
                        accent,
                        cx,
                    )
                })
            },
        );
        let mut blocks = vec![
            Block::Heading("Today tabs".into()),
            Block::Rows(
                (archive.into_iter())
                    .chain([unload, battery, moving])
                    .chain(sort)
                    .collect(),
            ),
            Block::Rows(vec![favorites]),
        ];
        let right = shell::sidebar_right(cx);
        let edge = self.toggle(
            "edge reveals sidebar",
            "Show the hidden sidebar at the window's edge",
            !off(NO_EDGE_REVEAL),
            set_edge_reveals,
            accent,
            cx,
        );
        let toggle_keys = shell::keystroke_label("cmd-s");
        let side = if right { "right" } else { "left" };
        let mut hidden = vec![setting(
            look,
            "Show the hidden sidebar at the window's edge",
            Some(
                format!(
                    "Move the pointer along the {side} edge. {toggle_keys} shows it either way."
                )
                .into(),
            ),
            vec![edge],
        )];
        // Where the host watches for it (host_watch_option_taps), or on Windows the core.
        if cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        )) {
            let option = unsafe { bc::bc_option_reveals_sidebar() };
            let spoken = match cfg!(target_os = "macos") {
                true => "Show the hidden sidebar when ⌥ is pressed alone",
                false => "Show the hidden sidebar when Alt is pressed alone",
            };
            let option = self.toggle(
                "option reveals sidebar",
                spoken,
                option,
                set_option_reveals,
                accent,
                cx,
            );
            let (name, line) = match cfg!(target_os = "macos") {
                true => (
                    "Show the hidden sidebar when ⌥ is pressed alone",
                    "Press and let go of ⌥ with no other key.",
                ),
                false => (
                    "Show the hidden sidebar when Alt is pressed alone",
                    "Press and let go of Alt with no other key.",
                ),
            };
            hidden.push(setting(look, name, Some(line.into()), vec![option]));
        }
        blocks.push(Block::Heading("The sidebar".into()));
        blocks.push(Block::Rows(hidden));
        // The trackpad's, which the Mac alone has a host for.
        if cfg!(target_os = "macos") {
            let haptics = unsafe { bc::bc_haptics() };
            let haptics = self.toggle(
                "haptics",
                "Haptic feedback",
                haptics,
                set_haptics,
                accent,
                cx,
            );
            let row = setting(
                look,
                "Haptic feedback",
                Some(
                    "A tap from the trackpad as a swipe changes Space or a tab drops into place."
                        .into(),
                ),
                vec![haptics],
            );
            blocks.push(Block::Rows(vec![row]));
        }
        // Once the core has backed the sidebar up, which it does as it saves the Spaces.
        if front_shell().is_some_and(|shell| shell.read(cx).has_sidebar_backups()) {
            let restore = self
                .button("restore-sidebar", "Restore…")
                .relative()
                .child(self.drawn("restore sidebar".into()))
                .on_click(|_, _, cx| restore_earlier_sidebar(cx));
            blocks.push(Block::Rows(vec![setting(
                look,
                "Restore an earlier sidebar",
                Some("Brings back the Spaces and tabs from a backup, newest first.".into()),
                vec![restore.into_any_element()],
            )]));
        }
        Page::new(
            Section::TabsAndSpaces,
            "When Today tabs leave, and how the sidebar comes and goes.",
            blocks,
        )
    }

    /// Features: ilo's own tools, each its glyph, its name, its ways in and its switch, and its
    /// own settings under it.
    /// Links: the cards ⇧ shows of a link in a page, then where a link from another app opens.
    fn links(&self, accent: Rgba, cx: &mut Context<Self>) -> Page {
        let shift = if cfg!(target_os = "macos") {
            "⇧"
        } else {
            "Shift"
        };
        let cards = self.toggle(
            "link cards",
            "Link cards",
            crate::link_cards::on(),
            set_link_cards,
            accent,
            cx,
        );
        // The privacy page's line, as the switch's (#3080).
        let cards = setting(
            &self.look,
            "Link cards",
            Some(
                format!(
                    "Press {shift} on a link to see its page's title, description and picture, \
                     asked without your cookies."
                )
                .into(),
            ),
            vec![cards],
        );
        let rules = self
            .links
            .clone()
            .map(|links| Block::View(links.into_any_element()));
        Page::new(
            Section::Links,
            "What a link in a page shows, and where a link from another app opens.",
            [
                Block::Rows(vec![cards]),
                Block::Heading("From other apps".into()),
            ]
            .into_iter()
            .chain(rules)
            .collect(),
        )
    }

    fn features(&self, profiles: &[Profile], accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let mac = cfg!(target_os = "macos");
        let (little_keys, little_click, peek_click) = match mac {
            true => ("⌥⌘N", "⌥⌘-click", "⇧-click"),
            false => ("Ctrl+Alt+N", "Ctrl+Alt-click", "Shift-click"),
        };
        let little_on = unsafe { bc::bc_little_windows() };
        let little_switch = self.toggle(
            "little windows",
            "Little Window",
            little_on,
            set_little_windows,
            accent,
            cx,
        );
        // Off, the line is the receipt: what's gone, and where things go instead.
        let little_line = match little_on {
            true => format!(
                "A small window for a quick look: {little_keys}, {little_click} a link, or a link \
                 from another app."
            ),
            false => format!(
                "Off: {little_keys} does nothing, {little_click} opens a tab behind, and links \
                 from other apps open in the Space in front."
            ),
        };
        let little = feature(
            look,
            "icons/app-window.svg",
            "Little Window",
            Some(little_line.into()),
            vec![little_switch],
        );
        let (label, chosen) = chosen_time(&LITTLE_ARCHIVE_AFTER, unsafe {
            bc::bc_little_archive_after()
        });
        let little_archive = self.popup(
            Popup::LittleArchiveAfter,
            label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_little_archive_after(chosen, window, cx)
            }),
        );
        let little_archive = nested(setting(
            look,
            "Archive windows left unused after",
            Some("Counted from when you leave the window.".into()),
            vec![little_archive.into_any_element()],
        ));
        let sign_in_close = self.toggle(
            "sign-in pages close",
            "Close sign-in pages when done",
            crate::sign_in_close::sign_in_close(),
            set_sign_in_close,
            accent,
            cx,
        );
        let sign_in_close = nested(setting(
            look,
            "Close sign-in pages when done",
            Some("When an app's sign-in finishes in a Little Window, the window closes.".into()),
            vec![sign_in_close],
        ));
        // Its own settings only while it's on.
        let little: Vec<Row> = [little]
            .into_iter()
            .chain(little_on.then_some(little_archive))
            .chain(little_on.then_some(sign_in_close))
            .collect();
        let previews = unsafe { bc::bc_link_previews() };
        let previews = self.toggle(
            "link previews",
            "Peek",
            previews,
            set_link_previews,
            accent,
            cx,
        );
        let peek = feature(
            look,
            "icons/eye.svg",
            "Peek",
            Some(
                format!(
                    "Links from pinned tabs and favorites open in a preview, and so do \
                     {peek_click} on any link and a click on an archived tab."
                )
                .into(),
            ),
            vec![previews],
        );
        let recent = self.toggle(
            "recent in hover cards",
            "What's recent in hover cards",
            browser::recent::on(cx),
            set_recent,
            accent,
            cx,
        );
        let recent = feature(
            look,
            "icons/git-pull-request.svg",
            "What's recent in hover cards",
            Some(
                "A pinned GitHub tab's or favorite's card shows your open pull requests, \
                 asked of GitHub with your own sign-in as you rest on it."
                    .into(),
            ),
            vec![recent],
        );
        let mini_player = unsafe { bc::bc_auto_picture_in_picture() };
        let mini_player = self.toggle(
            "mini player",
            "Mini Player",
            mini_player,
            set_mini_player,
            accent,
            cx,
        );
        let mini_player = feature(
            look,
            "icons/picture-in-picture-2.svg",
            "Mini Player",
            Some("A playing video or call floats over your work when you leave its tab.".into()),
            vec![mini_player],
        );
        let note_on = new_note_on(cx);
        let note_switch = self.toggle("new note", "New Note", note_on, set_new_note, accent, cx);
        let note_keys = shell::user_keys(cx).label(Command::NewNote);
        // Off, the line is the receipt, as Little ilo's is.
        let note_line: SharedString = match (note_on, note_keys) {
            (true, Some(keys)) => {
                format!("{keys} opens a new document in the service you pick.").into()
            }
            (true, None) => "New Note opens a new document in the service you pick.".into(),
            (false, Some(keys)) => format!(
                "Off: {keys} does nothing, and the + menu and the command bar have no New Note. \
                 The documents it opened stay."
            )
            .into(),
            (false, None) => "Off: the + menu and the command bar have no New Note. The \
                              documents it opened stay."
                .into(),
        };
        // One profile's service on the feature's row, else each profile's under it, and its
        // services only while it's on.
        let note = match profiles {
            _ if !note_on => vec![feature(
                look,
                "icons/pencil.svg",
                "New Note",
                Some(note_line),
                vec![note_switch],
            )],
            [profile] => vec![feature(
                look,
                "icons/pencil.svg",
                "New Note",
                Some(note_line),
                vec![self.notes_popup(profile.id, cx), note_switch],
            )],
            _ => {
                let services = profiles.iter().map(|profile| {
                    nested(setting(
                        look,
                        profile.name.clone(),
                        None,
                        vec![self.notes_popup(profile.id, cx)],
                    ))
                });
                let head = feature(
                    look,
                    "icons/pencil.svg",
                    "New Note",
                    Some(note_line),
                    vec![note_switch],
                );
                [head].into_iter().chain(services).collect()
            }
        };
        let boosts_on = !crate::boosts_host::all_off();
        let boosts = self.toggle(
            "boosts",
            "Boosts",
            boosts_on,
            crate::boosts_host::set_all_on,
            accent,
            cx,
        );
        // Off, the line is the receipt, as Little ilo's is.
        let boosts_line = match boosts_on {
            true => {
                "Your CSS and Zap on the sites you boost. New Boost in the command bar makes one."
            }
            false => "Off: no site shows its Boost. They stay in each profile's Boosts folder.",
        };
        let boosts = feature(
            look,
            "icons/paintbrush.svg",
            "Boosts",
            Some(boosts_line.into()),
            vec![boosts],
        );
        let mut blocks = vec![Block::Rows(little), Block::Rows(vec![peek])];
        // Not before this build's core can ask a site, where the switch would turn nothing off.
        if browser::recent::REACHED {
            blocks.push(Block::Rows(vec![recent]));
        }
        blocks.extend([
            Block::Rows(vec![mini_player]),
            Block::Rows(vec![boosts]),
            Block::Rows(note),
        ]);
        if self.widevine.read(cx).shown() {
            blocks.push(Block::Rows(vec![self.widevine_row(cx)]));
        }
        let always = cx.try_global::<shell::AlwaysTranslated>().cloned();
        if let Some(always) = always.filter(|always| !always.0.is_empty()) {
            blocks.push(Block::Heading("Always translated".into()));
            blocks.push(Block::Group(self.always_translated(&always.0, cx)));
        }
        Page::new(
            Section::General,
            concat!(
                env!("PRODUCT_NAME"),
                "'s own tools. Off takes every way into one away, and its switch brings it back."
            ),
            blocks,
        )
    }

    /// A profile's New Note service, in a pop-up, as Arc's Settings › Profiles has it.
    fn notes_popup(&self, id: u64, cx: &mut Context<Self>) -> AnyElement {
        let chosen = notes::service(&crate::name(bc::bc_profile_notes_service, id));
        let label = notes::SERVICES[chosen].1.into();
        let popup = self.popup(
            Popup::Notes(id),
            label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_notes(id, chosen, window, cx)
            }),
        );
        popup
            .child(self.drawn(format!("notes {id}")))
            .into_any_element()
    }

    /// Privacy: whether copied links leave out trackers, whether sites may ask to send
    /// notifications, whether calls are only relayed, each
    /// profile's browsing data, crash reports where the build names a collector, and the daily
    /// usage count where it names a server.
    fn privacy(&self, profiles: &[Profile], accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let clean_links = self.toggle(
            "clean links",
            "Copy links without tracking",
            clean_links_on(cx),
            set_clean_links,
            accent,
            cx,
        );
        let line = concat!(
            "Copied and shared links leave out what only tells a site where a click came from. ",
            "A video's time stays. The URL field's own text copies whole."
        );
        let clean_links = setting(
            look,
            "Copy links without tracking",
            Some(line.into()),
            vec![clean_links],
        );
        let prompts = unsafe { bc::bc_notification_prompts() };
        let prompts_switch = self.toggle(
            "notification prompts",
            "Let sites ask to send notifications",
            prompts,
            set_notification_prompts,
            accent,
            cx,
        );
        let prompts_line = match prompts {
            true => "Calls and chat need it. A site you allowed or blocked keeps its choice.",
            false => "Off: no site can ask. A site you allowed or blocked keeps its choice.",
        };
        let prompts = setting(
            look,
            "Let sites ask to send notifications",
            Some(prompts_line.into()),
            vec![prompts_switch],
        );
        let mut blocks = vec![Block::Rows(vec![clean_links]), Block::Rows(vec![prompts])];
        let relay = unsafe { bc::bc_relay_calls() };
        let relay = self.toggle(
            "relay calls",
            "Relay calls",
            relay,
            set_relay_calls,
            accent,
            cx,
        );
        let line = "Calls always go through a relay. Some apps won't connect, and calls may lag.";
        let relay = setting(look, "Relay calls", Some(line.into()), vec![relay]);
        blocks.push(Block::Rows(vec![relay]));
        // Only a profile one of the user's Spaces is in, for Clear Browsing Data's tab to open in.
        let clear = profiles
            .iter()
            .filter(|profile| profile.users_space)
            .collect::<Vec<_>>();
        let several = profiles.len() > 1;
        let clear = clear.into_iter().enumerate().map(|(index, profile)| {
            let id = profile.id;
            let button = self
                .button(("clear-browsing-data", id), "Clear…")
                .relative()
                .child(self.drawn(format!("clear {id}")))
                .on_click(cx.listener(move |_, _, _, cx| {
                    if crate::clear_browsing_data(id) {
                        hide(cx);
                    }
                }));
            let named = several.then(|| {
                let name = div()
                    .max_w(px(POPUP_WIDTH))
                    .truncate()
                    .child(profile.name.clone());
                name.text_color(look.detail).into_any_element()
            });
            let line = (index == 0).then(|| "History, cookies and cache, for one profile.".into());
            let controls = named
                .into_iter()
                .chain([button.into_any_element()])
                .collect();
            setting(look, "Browsing data", line, controls)
        });
        let clear: Vec<Row> = clear.collect();
        if !clear.is_empty() {
            blocks.push(Block::Rows(clear));
        }
        // What the browser sends of its own, one row each.
        let mut sending = Vec::new();
        let collector = crash_reports_collector();
        if !collector.is_empty() {
            let send = unsafe { bc::bc_send_crash_reports() };
            let send = self.toggle(
                "send crash reports",
                "Send crash reports",
                send,
                set_send_crash_reports,
                accent,
                cx,
            );
            let line = format!(
                "Only while this is on, to {}. Turning it off deletes those not yet sent.",
                collector_name(&collector)
            );
            let row = setting(look, "Send crash reports", Some(line.into()), vec![send]);
            sending.push(row);
        }
        // Only in a build that names a server, as only that build sends it (#1977).
        if crate::daily_count_host::named() {
            let count = self.toggle(
                "send a daily usage count",
                "Send a daily usage count",
                crate::daily_count_host::on(),
                crate::daily_count_host::set_on,
                accent,
                cx,
            );
            let line = browser::daily_count::words(env!("PRODUCT_NAME"));
            let row = setting(
                look,
                "Send a daily usage count",
                Some(line.into()),
                vec![count],
            );
            sending.push(row);
        }
        if !sending.is_empty() {
            blocks.push(Block::Rows(sending));
        }
        // What it never does, in a line, beside the policy that names every service it talks to,
        // as the owner asked (#684).
        let policy = self
            .button("privacy-policy", "Open")
            .relative()
            .child(self.drawn("open privacy policy".into()))
            .on_click(|_, _, cx| open_notice(build_info::PRIVACY_POLICY, cx));
        let line = concat!(
            env!("PRODUCT_NAME"),
            " never sees the sites you visit or what you type, and never sells your data. ",
            "The policy names every service it talks to."
        );
        let policy = setting(
            look,
            "Privacy Policy",
            Some(line.into()),
            vec![policy.into_any_element()],
        );
        blocks.push(Block::Rows(vec![policy]));
        Page::new(
            Section::Privacy,
            concat!(
                "What sites may do, and what ",
                env!("PRODUCT_NAME"),
                " sends."
            ),
            blocks,
        )
    }

    /// Ad Blocking: the switch for it everywhere, the sites it's off on, the user's own filters,
    /// and its filter lists by group, each with its switch, or as an administrator's policy chose
    /// them. The browser's model (bc_ad_block_lists), which the blocker applies.
    fn ad_blocking(&self, off_sites: &[OffSite], accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let on = unsafe { bc::bc_ad_blocking() };
        const NAME: &str = "Block ads and trackers";
        let switch = self.toggle("ad blocking", NAME, on, set_ad_blocking, accent, cx);
        let line = "A site's Site Controls turn it off there, and say how much it blocked.";
        let mut rows = vec![setting(look, NAME, Some(line.into()), vec![switch])];
        if self.ad_blocker.read(cx).shown() {
            rows.push(self.ad_blocker_row(accent, cx));
        }
        let mut blocks = vec![Block::Rows(rows)];
        if !off_sites.is_empty() {
            blocks.push(Block::Heading("Ads are shown on".into()));
            blocks.push(Block::Group(self.off_sites(off_sites, cx)));
        }
        blocks.push(Block::Heading("My filters".into()));
        blocks.push(Block::Group(self.my_filters(cx)));
        let lists = ad_block_lists();
        let locked = unsafe { bc::bc_ad_block_lists_locked() };
        for (group, heading) in AD_BLOCK_GROUPS {
            let rows: Vec<Row> = (lists.iter().filter(|list| list.group == group))
                .map(|list| {
                    let control = match locked {
                        true => div()
                            .text_color(look.detail)
                            .child(if list.on { "On" } else { "Off" })
                            .into_any_element(),
                        false => {
                            let key = list.key.clone();
                            let name = format!("ad block list {}", list.key);
                            let set = move |on, _: &mut App| set_ad_block_list(&key, on);
                            self.toggle(name, list.title.clone(), list.on, set, accent, cx)
                        }
                    };
                    setting(look, list.title.clone(), None, vec![control])
                })
                .collect();
            if !rows.is_empty() {
                blocks.push(Block::Heading(heading.into()));
                blocks.push(Block::Rows(rows));
            }
        }
        let native = crate::ad_block_host::launch().engine == ad_blocker::Engine::Native;
        let lede = match (native, locked) {
            (false, true) => {
                "uBlock Origin, built in, with the filter lists your organization chose."
            }
            (false, false) => "uBlock Origin, built in, with the filter lists you turn on.",
            (true, true) => concat!(
                env!("PRODUCT_NAME"),
                "'s own ad blocker, with the filter lists your organization chose."
            ),
            (true, false) => concat!(
                env!("PRODUCT_NAME"),
                "'s own ad blocker, with the filter lists you turn on."
            ),
        };
        Page::new(Section::Privacy, lede, blocks)
    }

    /// Block with uBlock Origin: the way back to the extension and forward again, each taking
    /// effect at the next launch, with the restart beside the switch once one is chosen.
    fn ad_blocker_row(&self, accent: Rgba, cx: &mut Context<Self>) -> Row {
        let row = self.ad_blocker.read(cx);
        let (title, detail, on) = (row.title(), row.detail(), row.on());
        let mut controls = Vec::new();
        if row.restart_offered() {
            let restart = cx.listener(|settings: &mut Self, _, _, cx| {
                log!("browser: settings ad blocker restart");
                settings.ad_blocker.update(cx, |row, cx| row.restart(cx));
            });
            let button = self
                .button("ad blocker restart", ad_blocker::RESTART)
                .relative()
                .child(self.drawn("ad blocker restart".into()))
                .on_click(restart);
            controls.push(button.into_any_element());
        }
        let entity = self.ad_blocker.clone();
        let set = move |on, cx: &mut App| {
            entity.update(cx, |row, cx| {
                row.set(on);
                cx.notify();
            })
        };
        controls.push(self.toggle("ad blocker", title.clone(), on, set, accent, cx));
        setting(&self.look, title, Some(detail), controls)
    }

    /// My filters: the user's own, one a row with Remove, then the field that adds one; or an
    /// administrator's, as their policy set them.
    fn my_filters(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let locked = unsafe { bc::bc_ad_block_filters_locked() };
        let filters = ad_block_filters();
        let count = filters.len();
        let rows = filters.into_iter().enumerate().map(|(index, filter)| {
            let remove = (!locked).then(|| {
                let remove = self
                    .button(("ad-block-filter-remove", index as u64), "Remove")
                    .relative()
                    .child(self.drawn(format!("remove filter {index}")))
                    .on_click(cx.listener(
                        move |settings, click: &gpui::ClickEvent, window, cx| {
                            let lines: Vec<usize> = (0..count).collect();
                            let keys = click.is_keyboard();
                            list_keys::removing(&index, &lines, Named::ByLine, keys, window, cx);
                            settings.remove_filter(index, cx)
                        },
                    ));
                remove.into_any_element()
            });
            line_row(look, filter, None, remove.into_iter().collect())
        });
        let mut rows: Vec<gpui::Div> = rows.collect();
        if locked {
            rows.push(line_row(look, "Set by your organization", None, Vec::new()));
            return group(look, rows);
        }
        let field = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(CONTROL_HEIGHT))
            .px(px(FIELD_PADDING))
            .flex()
            .items_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(look.control)
            .key_context("Naming")
            .on_action(cx.listener(|settings, _: &KeepName, window, cx| {
                if browser::dismissal::submits([&settings.filter.clone()], cx) {
                    settings.add_filter(window, cx)
                }
            }))
            .on_action(cx.listener(|settings, _: &CancelName, window, cx| {
                let filter = settings.filter.clone();
                if browser::dismissal::Close::escape_from([&filter], cx).is_some() {
                    filter.update(cx, |field, cx| field.reset("".into(), window, cx));
                }
            }))
            .child(self.drawn("filter field".into()))
            .child(self.filter.clone());
        let add = self
            .button("ad-block-filter-add", "Add")
            .relative()
            .child(self.drawn("add filter".into()))
            .on_click(cx.listener(|settings, _, window, cx| settings.add_filter(window, cx)));
        rows.push(frame().gap(px(CONTROL_GAP)).child(field).child(add));
        group(look, rows)
    }

    /// Adds the filter typed in My filters' field, after the others, and empties the field.
    fn add_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.filter.read(cx).text().trim().to_string();
        if typed.is_empty() {
            return;
        }
        let mut filters = ad_block_filters();
        filters.push(typed.clone());
        set_ad_block_filters(&filters);
        // check.sh waits for this line.
        log!("browser: ad block filter added {typed}");
        (self.filter).update(cx, |field, cx| field.reset("".into(), window, cx));
        cx.notify();
    }

    /// My filters' Remove on the filter at `index`.
    fn remove_filter(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut filters = ad_block_filters();
        if index < filters.len() {
            let removed = filters.remove(index);
            set_ad_block_filters(&filters);
            // check.sh waits for this line.
            log!("browser: ad block filter removed {removed}");
            cx.notify();
        }
    }

    /// The languages a page's translate card said to always translate, each with Remove, after
    /// which a page in it asks again (#671).
    fn always_translated(&self, languages: &[SharedString], cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let count = languages.len();
        let rows = languages.iter().enumerate().map(|(index, language)| {
            let at = self.drawn(format!("always translated {language}"));
            let gone = language.clone();
            let remove = self
                .button(("always-translated-remove", index as u64), "Remove")
                .relative()
                .child(at)
                .on_click(cx.listener(move |_, click: &gpui::ClickEvent, window, cx| {
                    let lines: Vec<usize> = (0..count).collect();
                    let keys = click.is_keyboard();
                    list_keys::removing(&index, &lines, Named::ByLine, keys, window, cx);
                    let mut languages = (cx.try_global::<shell::AlwaysTranslated>())
                        .map(|always| always.0.clone())
                        .unwrap_or_default();
                    languages.retain(|each| *each != gone);
                    crate::translate_host::save_always(&languages);
                    // check.sh waits for this line.
                    log!("browser: always translate {gone} off");
                    cx.set_global(shell::AlwaysTranslated(languages));
                    cx.refresh_windows();
                }));
            let name = crate::translate_host::language_name(language);
            line_row(look, name, None, vec![remove.into_any_element()])
        });
        group(look, rows)
    }

    /// The sites the user turned ad blocking off on in their Site Controls, each with the way to
    /// turn it on again, and its profile's name where there are others.
    fn off_sites(&self, sites: &[OffSite], cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let rows = sites.iter().enumerate().map(|(index, off)| {
            let (profile, site) = (off.profile, off.site.clone());
            let at = self.drawn(format!("turn on {}", off.site));
            let turn_on = self
                .button(("ad-blocking-on", index as u64), "Turn On")
                .relative()
                .child(at);
            let turn_on = turn_on.on_click(cx.listener(move |_, _, _, cx| {
                let Ok(text) = CString::new(site.to_string()) else {
                    return;
                };
                unsafe { bc::bc_ad_blocking_turn_on_site(profile, text.as_ptr()) };
                // check.sh waits for this line.
                log!("browser: ad blocking on for {site}");
                cx.notify();
            }));
            let named = (off.profile_name.clone()).map(|name| {
                let name = div().max_w(px(POPUP_WIDTH)).truncate().child(name);
                name.text_color(look.detail).into_any_element()
            });
            let controls = named
                .into_iter()
                .chain([turn_on.into_any_element()])
                .collect();
            line_row(look, off.site.clone(), None, controls)
        });
        let rows: Vec<gpui::Div> = rows.collect();
        group(look, rows)
    }

    /// Logs the sites Ad Blocking lists with ad blocking off when they change, for check.sh: each
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

    /// A push button with `title`, which an assistive app reads by it.
    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        title: impl Into<SharedString>,
    ) -> gpui::Stateful<gpui::Div> {
        stop_button(&self.look, id, title, self.a11y)
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
    ) -> gpui::Stateful<gpui::Div> {
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
        let press = Rc::new(press);
        let pressed = press.clone();
        let id = SharedString::from(format!("popup-{which:?}"));
        let open = self.choosing == Some(which);
        stop_popup(
            &self.look,
            id,
            which.spoken(),
            label,
            width,
            open,
            self.a11y,
        )
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            press(event, window, cx)
        })
        // Space or Return opens its menu as a press does.
        .on_click(move |event, window, cx| {
            if event.is_keyboard() {
                pressed(&MouseDownEvent::default(), window, cx)
            }
        })
        .child(at)
    }

    /// The switch `name`, `on` now, that calls `set` with the other way when clicked.
    fn toggle(
        &self,
        name: impl Into<SharedString>,
        spoken: impl Into<SharedString>,
        on: bool,
        set: impl Fn(bool, &mut App) + 'static,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name: SharedString = name.into();
        let slid = match &self.switched {
            Some((switched, at)) if *switched == name => {
                (at.elapsed().as_secs_f32() / SLIDE).min(1.)
            }
            _ => 1.,
        };
        let drawn = self.drawn(format!("switch {name}"));
        stop_switch(
            &self.look,
            name.clone(),
            spoken,
            on,
            slid,
            accent,
            self.a11y,
        )
        .on_click(cx.listener(move |settings, _, _, cx| {
            // check.sh waits for this line.
            log!("browser: {name} {}", if on { "off" } else { "on" });
            set(!on, cx);
            settings.switched = Some((name.clone(), browser::motion::start(cx)));
            cx.notify();
        }))
        .child(drawn)
        .into_any_element()
    }

    /// Opens a profile's archive pop-up menu over its button, with the time chosen now, `chosen`,
    /// over the button's text, as a pop-up button does, and sets the time picked.
    fn choose_archive_after(
        &mut self,
        profile: u64,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (ARCHIVE_AFTER.iter().enumerate())
            .map(|(index, &(title, _))| {
                let on = Some(index) == chosen;
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::new(title, true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((title, seconds)) = picked.map(|index| ARCHIVE_AFTER[index]) {
                // check.sh waits for this line.
                log!("browser: profile {profile} archive tabs after {title}");
                unsafe { bc::bc_profile_set_archive_after(profile, seconds) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::ArchiveAfter(profile), items, picked, window, cx);
    }

    /// Opens the Little ilo windows' archive pop-up menu over its button, as the archive's, and
    /// sets how long one not in use stays open.
    fn choose_little_archive_after(
        &mut self,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (LITTLE_ARCHIVE_AFTER.iter().enumerate())
            .map(|(index, &(title, _))| {
                let on = Some(index) == chosen;
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::new(title, true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((title, seconds)) = picked.map(|index| LITTLE_ARCHIVE_AFTER[index]) {
                log!("browser: little windows archive after {title}");
                unsafe { bc::bc_set_little_archive_after(seconds) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::LittleArchiveAfter, items, picked, window, cx);
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
        // The engine ilo searches with until the user picks one is marked, as Page zoom's 100% is.
        let items: Vec<MenuEntry> = (engines.iter().enumerate())
            .map(|(index, (keyword, name))| {
                let on = Some(index) == chosen;
                let name = match keyword.as_str() {
                    FALLBACK_ENGINE => format!("{name} (default)").into(),
                    _ => name.clone(),
                };
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::named(name, true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((keyword, name)) = picked.and_then(|index| engines.get(index)) {
                set_search_engine(profile, keyword, name);
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::SearchEngine(profile), items, picked, window, cx);
    }

    /// General's Page zoom: Chrome's choices, which every profile's pages take.
    fn choose_page_zoom(&mut self, chosen: Option<usize>, window: &Window, cx: &mut Context<Self>) {
        let items: Vec<MenuEntry> = (PAGE_ZOOM.iter().enumerate())
            .map(|(index, &(title, _))| {
                let on = Some(index) == chosen;
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::new(title, true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((title, factor)) = picked.map(|index| PAGE_ZOOM[index]) {
                // check.sh waits for this line.
                log!("browser: page zoom {title}");
                unsafe { bc::bc_set_page_zoom(factor) };
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::PageZoom, items, picked, window, cx);
    }

    /// Shows `which` pop-up's menu of `items` in ilo's menu over Settings' window, light or dark as
    /// the window is, its button pressed until `picked` is called.
    fn pop_up(
        &mut self,
        which: Popup,
        items: Vec<MenuEntry>,
        picked: Box<dyn FnOnce(Option<usize>, &mut App)>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let texts = self.popup_texts.borrow();
        let Some(at) = texts
            .iter()
            .find(|&&(each, _)| each == which)
            .map(|&(_, at)| at)
        else {
            return;
        };
        drop(texts);
        self.choosing = Some(which);
        cx.notify();
        show_menu(
            ns_view(window),
            items,
            at,
            false,
            self.menu_look(),
            menu::picked(picked),
            cx,
        );
    }

    /// The pop-up's menu closed.
    fn chose(&mut self, cx: &mut Context<Self>) {
        self.choosing = None;
        cx.notify();
    }

    /// Profiles: a row for each profile, with how many Spaces are in it, Rename, and Delete for one
    /// nothing is in, else why it has none, and New Profile under them. A double-click on a
    /// profile's name renames it too, as on a Space's.
    fn profiles(&self, profiles: &[Profile], cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let rows = profiles.iter().map(|profile| {
            let (id, name) = (profile.id, profile.name.clone());
            let naming = self
                .renaming
                .as_ref()
                .is_some_and(|renaming| renaming.profile == id);
            let label = match &self.renaming {
                Some(renaming) if renaming.profile == id => div()
                    .key_context("Naming")
                    .on_action(cx.listener(|settings, _: &KeepName, window, cx| {
                        let name = settings.renaming.as_ref().map(|r| r.name.clone());
                        if browser::dismissal::submits(&name, cx) {
                            settings.keep_name(window, cx)
                        }
                    }))
                    .on_action(cx.listener(|settings, _: &CancelName, window, cx| {
                        let name = settings.renaming.as_ref().map(|r| r.name.clone());
                        if browser::dismissal::Close::escape_from(&name, cx).is_some() {
                            settings.cancel_name(window, cx)
                        }
                    }))
                    .child(renaming.name.clone()),
                _ => div()
                    .relative()
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
            let spaces = match profile.spaces {
                0 => "No Spaces".to_string(),
                1 => "1 Space".to_string(),
                count => format!("{count} Spaces"),
            };
            // Why it has no Delete, after how many Spaces are in it.
            let line = match &profile.kept {
                Some(why) => format!("{spaces}. {why}"),
                None => spaces,
            };
            let line = div()
                .mt(px(DETAIL_TOP))
                .text_size(px(text::SMALL))
                .line_height(px(DETAIL_LINE))
                .text_color(look.detail)
                .child(line);
            let label = div().flex_1().min_w_0().child(label).child(line);
            let rename =
                (!naming).then(|| {
                    let name = profile.name.clone();
                    self.button(("rename-profile", id), "Rename")
                        .relative()
                        .child(self.drawn(format!("rename profile {id}")))
                        .on_click(cx.listener(move |settings, _, _, cx| {
                            settings.rename(id, name.clone(), cx)
                        }))
                        .into_any_element()
                });
            let (name, favorites) = (profile.name.clone(), profile.favorites);
            let delete = profile.deletable.then(|| {
                self.button(("delete-profile", id), "Delete")
                    .relative()
                    .child(self.drawn(format!("delete profile {id}")))
                    .on_click(cx.listener(move |settings, _, window, cx| {
                        settings.delete(id, name.clone(), favorites, window, cx)
                    }))
                    .into_any_element()
            });
            let controls: Vec<AnyElement> = rename.into_iter().chain(delete).collect();
            frame()
                .child(glyph_plate(look, "icons/user.svg", true))
                .child(label)
                .when(!controls.is_empty(), |row| row.child(end(controls)))
        });
        let rows: Vec<gpui::Div> = rows.collect();
        let new = self
            .button("new-profile", "New Profile")
            .relative()
            .child(self.drawn("new profile".into()))
            .on_click(cx.listener(|settings, _, _, cx| settings.new_profile(cx)));
        let blocks = vec![
            Block::Group(group(look, rows)),
            Block::Under(div().flex().child(new)),
        ];
        Page::new(
            Section::TabsAndSpaces,
            "A profile keeps its own logins, cookies, history and extensions. Several Spaces can \
             share one.",
            blocks,
        )
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
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::named(title, true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(&(name, title, _)) = picked.and_then(|index| notes::SERVICES.get(index)) {
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
        let colors = browser::settings_rows::field_colors(cx, |settings: &Self| settings.look);
        let name = cx.new(|cx| {
            TextInput::new("Profile name", cx)
                .follows(colors)
                .with_text(name)
        });
        self.renaming = Some(Renaming {
            profile,
            name,
            _watches: Vec::new(),
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
        if let Some(folder) = crate::bar_picks_host::folder(profile) {
            log!("browser: profile {profile} folder {}", folder.display());
        }
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
        favorites: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Delete “{name}”?");
        let detail = shell::profile_goes_with(
            favorites_scope(),
            favorites,
            crate::platform::keeps_passkeys(),
        );
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
                if crate::ask(&message, &detail, &["Delete", "Cancel"], true) == 0 {
                    unsafe { bc::bc_profile_delete(profile) };
                }
            })
            .detach();
        }
    }

    /// The profile Passwords lists: the one picked or asked for while it's there, or else the one
    /// the browser started with.
    fn passwords_profile(&self, profiles: &[Profile]) -> Option<u64> {
        let picked = self
            .passwords_profile
            .filter(|&id| profiles.iter().any(|each| each.id == id));
        picked.or(profiles.first().map(|profile| profile.id))
    }

    /// Passwords: a pop-up picks the profile, if there are more than one, as each keeps its own,
    /// over its switches for what it offers to save; then its saved logins, each with its username
    /// under its site, the sites set to Never, each removable, and Manage Passwords…, which opens
    /// Chrome's page; then its addresses and cards, while it has any.
    fn passwords(&mut self, profiles: &[Profile], accent: Rgba, cx: &mut Context<Self>) -> Page {
        const LEDE: &str = concat!(
            "What each profile saves and fills in. ",
            env!("PRODUCT_NAME"),
            " shows a login's site and username, never its password."
        );
        let Some(profile) = self.passwords_profile(profiles) else {
            return Page::new(Section::Passwords, LEDE, Vec::new());
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
        let look = self.look;
        let logins = self.logins.get(&profile).cloned().unwrap_or_default();
        let login_row = |index: usize, login: &Login, cx: &mut Context<Self>| {
            let (site, username) = passwords::lines(login);
            let login = login.clone();
            // Named by its login's key, so another's Remove never takes its name, and the keyboard
            // with it, as a row goes (list_keys::removing); hashed, as the key holds the username.
            let named = {
                use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
                BuildHasherDefault::<DefaultHasher>::default().hash_one(&login.key)
            };
            let remove = self
                .button(("remove-login", named), "Remove")
                .relative()
                .child(self.drawn(format!("remove login {index}")))
                .on_click(
                    cx.listener(move |settings, click: &gpui::ClickEvent, window, cx| {
                        let keys = click.is_keyboard();
                        settings.remove(profile, login.clone(), keys, window, cx)
                    }),
                );
            feature_row(
                &look,
                Section::Passwords.glyph(),
                site,
                username.map(Into::into),
                vec![remove.into_any_element()],
            )
        };
        // An address or a card as a login's row: its title over its detail, and Remove.
        let autofill = self.autofill.get(&profile).cloned().unwrap_or_default();
        let saved_row = |index: usize, saved: &Saved, cx: &mut Context<Self>| {
            let glyph = if saved.card {
                "icons/credit-card.svg"
            } else {
                "icons/map-pin.svg"
            };
            let remove = self
                .button(("remove-autofill", index), "Remove")
                .relative()
                .child(self.drawn(format!("remove autofill {index}")))
                .on_click(cx.listener({
                    let saved = saved.clone();
                    move |settings, _, window, cx| {
                        settings.remove_autofill(profile, saved.clone(), window, cx)
                    }
                }));
            let detail = Some(saved.detail.clone()).filter(|detail| !detail.is_empty());
            feature_row(
                &look,
                glyph,
                saved.title.clone(),
                detail,
                vec![remove.into_any_element()],
            )
        };
        let (cards, addresses): (Vec<_>, Vec<_>) = autofill
            .iter()
            .enumerate()
            .partition(|(_, saved)| saved.card);
        let addresses: Vec<gpui::Div> = addresses
            .into_iter()
            .map(|(index, saved)| saved_row(index, saved, cx))
            .collect();
        let cards: Vec<gpui::Div> = cards
            .into_iter()
            .map(|(index, saved)| saved_row(index, saved, cx))
            .collect();
        let (saved, never): (Vec<_>, Vec<_>) = logins
            .iter()
            .enumerate()
            .partition(|(_, login)| !login.never);
        let saved: Vec<gpui::Div> = saved
            .into_iter()
            .map(|(index, login)| login_row(index, login, cx))
            .collect();
        let never: Vec<gpui::Div> = never
            .into_iter()
            .map(|(index, login)| login_row(index, login, cx))
            .collect();
        let saved = match saved.is_empty() {
            true => vec![
                frame()
                    .text_color(look.detail)
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
            let popup = self.popup(
                Popup::PasswordsProfile,
                label,
                POPUP_WIDTH,
                move |_, window, cx| {
                    let (names, ids) = (names.clone(), ids.clone());
                    let choose = |settings: &mut Settings, cx: &mut Context<Settings>| {
                        let which = Popup::PasswordsProfile;
                        settings.choose_profile(which, names, ids, chosen, window, cx)
                    };
                    view.update(cx, choose).ok();
                },
            );
            row("Profile", popup.into_any_element())
        });
        // Chrome's words for each, as its settings pages have them.
        let switches = [
            (
                bc::BC_SAVE_PASSWORDS,
                "offer to save passwords",
                "Offer to save passwords",
            ),
            (
                bc::BC_SAVE_ADDRESSES,
                "save and fill addresses",
                "Save and fill addresses",
            ),
            (
                bc::BC_SAVE_CARDS,
                "save and fill payment methods",
                "Save and fill payment methods",
            ),
        ];
        let switches = switches.map(|(what, name, title)| {
            offers(what as i32).map(|on| {
                let set = move |offer, _: &mut App| unsafe {
                    bc::bc_profile_set_offers_to_save(profile, what as i32, offer);
                };
                row(title, self.toggle(name, title, on, set, accent, cx))
            })
        });
        let switches: Vec<gpui::Div> = switches.into_iter().flatten().collect();
        let manage = self
            .button("manage-passwords", "Manage Passwords…")
            .relative()
            .child(self.drawn("manage passwords".into()))
            .on_click(move |_, _, cx| manage_passwords(profile, cx));
        // The passkeys kept on this Mac, which Chrome's password manager links to only once there's
        // one, and which have no other list: where the user sees and deletes them.
        let passkeys = cfg!(target_os = "macos").then(|| {
            self.button("manage-passkeys", "Manage Passkeys…")
                .relative()
                .child(self.drawn("manage passkeys".into()))
                .on_click(move |_, _, cx| manage_passkeys(profile, cx))
        });
        let mut blocks = Vec::new();
        if let Some(chooser) = chooser {
            blocks.push(Block::Group(group(&look, [chooser])));
        }
        if !switches.is_empty() {
            blocks.push(Block::Group(group(&look, switches)));
        }
        blocks.push(Block::Heading("Saved".into()));
        blocks.push(Block::Group(group(&look, saved)));
        if !never.is_empty() {
            blocks.push(Block::Heading("Never saved".into()));
            blocks.push(Block::Group(group(&look, never)));
        }
        blocks.push(Block::Under(
            div()
                .flex()
                .gap(px(CONTROL_GAP))
                .child(manage)
                .children(passkeys),
        ));
        // Each only while it has one, as Chrome offers to save them as forms are filled.
        if !addresses.is_empty() {
            blocks.push(Block::Heading("Addresses".into()));
            blocks.push(Block::Group(group(&look, addresses)));
        }
        if !cards.is_empty() {
            blocks.push(Block::Heading("Payment methods".into()));
            blocks.push(Block::Group(group(&look, cards)));
        }
        Page::new(Section::Passwords, LEDE, blocks)
    }

    /// Opens the profiles' pop-up menu over `which`, as the archive's, and shows the profile
    /// picked's logins, for Passwords', or its settings, for a group's chip.
    fn choose_profile(
        &mut self,
        which: Popup,
        names: Vec<SharedString>,
        ids: Vec<u64>,
        chosen: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<MenuEntry> = (names.iter().enumerate())
            .map(|(index, name)| {
                let on = Some(index) == chosen;
                MenuEntry {
                    checked: on,
                    over: on,
                    ..MenuEntry::named(name.clone(), true)
                }
            })
            .collect();
        let settings = cx.weak_entity();
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            settings
                .update(cx, |settings, cx| {
                    if let Some(&profile) = picked.and_then(|index| ids.get(index)) {
                        // check.sh waits for these lines.
                        match which {
                            Popup::PasswordsProfile => {
                                log!("browser: passwords profile {profile}");
                                settings.passwords_profile = Some(profile);
                            }
                            _ => {
                                log!("browser: settings scope profile {profile}");
                                settings.scope = Some(profile);
                            }
                        }
                    }
                    settings.chose(cx);
                })
                .ok();
        });
        self.pop_up(which, items, picked, window, cx);
    }

    /// Removes a saved login once the user confirms, in an alert, or on Linux in Settings' own
    /// dialog, as a profile's Delete asks, as nothing brings it back. A site set to Never comes off
    /// it at once: the offer coming again is its way back. Its Remove pressed from the keys
    /// (`from_keys`) hands the keyboard to the Remove of the row that takes its place, among the
    /// saved or the Never rows it's listed in.
    fn remove(
        &mut self,
        profile: u64,
        login: Login,
        from_keys: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let listed: Vec<Login> = (self.logins.get(&profile).into_iter().flatten())
            .filter(|each| each.never == login.never)
            .cloned()
            .collect();
        if login.never {
            list_keys::removing(&login, &listed, Named::ByRow, from_keys, window, cx);
            crate::remove_login(profile, &login);
            return;
        }
        let (message, detail) = passwords::question(&login);
        // Settings' observer has given the keyboard back to this Remove by the time the answer
        // runs, so it hands on from there.
        #[cfg(target_os = "linux")]
        let handle = window.window_handle();
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            "Remove",
            dialog::Return::Action,
            Box::new(move |cx| {
                (handle.update(cx, |_, window, cx| {
                    list_keys::removing(&login, &listed, Named::ByRow, from_keys, window, cx)
                }))
                .ok();
                crate::remove_login(profile, &login);
            }),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        cx.spawn_in(window, async move |settings, cx| {
            if crate::ask(&message, &detail, &["Remove", "Cancel"], true) == 0 {
                (settings.update_in(cx, |_, window, cx| {
                    list_keys::removing(&login, &listed, Named::ByRow, from_keys, window, cx)
                }))
                .ok();
                crate::remove_login(profile, &login);
            }
        })
        .detach();
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
        let detail = format!("{} won't fill it in again.", env!("PRODUCT_NAME"));
        let remove = move || {
            // check.sh waits for this line, which names nothing saved.
            log!(
                "browser: remove {} {profile}",
                if saved.card { "card" } else { "address" }
            );
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

    /// Agents: the switch that lets agents in, the tasks ⇥ asks in ⌘T, the apps that connect,
    /// and each agent's permissions and what it did in this run, newest first.
    /// Agents' pages, Threads, Models, Access and Other apps, in that order: what was one page of
    /// agents, each part on the page the owner drew it on (spec-settings.md › S8 to S11).
    fn agents(&self, agents: &Entity<Agents>, accent: Rgba, cx: &mut Context<Self>) -> [Page; 4] {
        let look = &self.look;
        let on = agents.read(cx).threads();
        // The inside switch: the user's own assistant, for threads. Other apps have theirs.
        let name = format!("Let your assistant use {PRODUCT}");
        let switch = {
            let agents = agents.clone();
            let set =
                move |on, cx: &mut App| agents.update(cx, |agents, cx| agents.set_threads(on, cx));
            self.toggle("agents", name.clone(), on, set, accent, cx)
        };
        // Off, the drawn line says what off does; on, the row is bare. The agents' trouble comes
        // first, as it did.
        let about: Option<SharedString> = match agents.read(cx).trouble() {
            Some(trouble) => Some(trouble),
            None => (!on).then(|| "Off: no thread starts.".into()),
        };
        let master = setting(look, name, about, vec![switch]);
        let listed = agents.read(cx).listed();
        let clients: Vec<gpui::Div> = listed
            .iter()
            .map(|client| self.client(agents, *client, &listed, cx))
            .collect();
        // Under the inside switch, while it's on: what is kept of them (#4105).
        let keeps = on.then(|| self.kept_tasks(agents, cx));
        // And under that, each one kept, which is the list of what is.
        let kept = match on {
            true => self.kept_rows(agents, cx),
            false => Vec::new(),
        };
        let rows = [master].into_iter().chain(keeps).chain(kept);
        let mut threads = vec![Block::Rows(rows.collect())];
        let (mut models, mut access) = (Vec::new(), Vec::new());
        // Every assistant, in its state, whether agents and tasks are on or not (S9).
        models.push(Block::Heading("Your assistant".into()));
        models.push(Block::Group(self.assistants(agents, cx)));
        models.push(Block::Under(foot(
            look,
            browser::settings_rows::SIGN_INS_STAY,
        )));
        if on {
            access.extend(self.access());
            // What a thread may reach: files and folders too, with the folders it holds to revoke.
            access.extend(self.files_not_here(agents, cx));
            access.extend(self.folders_in_use(agents, cx));
        }
        // ⌘T's tasks and outside agents' sessions notify alike, so it's here while agents are on.
        // The Mac, Windows and Linux each post them (needs_you_host.rs).
        if on {
            threads.push(Block::Heading(browser::needs_you::NOTIFICATIONS.into()));
            threads.push(Block::Rows(vec![self.when_to_notify(agents, cx)]));
        }
        let mut blocks = vec![Block::Rows(vec![self.other_apps(agents, accent, cx)])];
        blocks.push(Block::Heading(APPS_THAT_CONNECT.into()));
        let apps = SharedString::from(APPS_THAT_CONNECT);
        blocks.push(Block::Group(self.flashed(&apps, group(look, clients))));
        // Shown as Apps that connect are, whether agents are on or off: the command says they're
        // off when it's run.
        if agents.read(cx).command_offered() {
            blocks.push(Block::Heading("Command line".into()));
            blocks.push(Block::Rows(vec![self.command_line(agents, cx)]));
        }
        access.extend(self.fenced_sites(agents, cx));
        access.extend(self.site_notes(agents, accent, cx));
        // AppleScript is the Mac's alone.
        if cfg!(target_os = "macos") {
            blocks.extend(self.scripts(agents, accent, cx));
        }
        // A list for each agent, as each has its own Space (decision 8): what the user let it do
        // or never do on a site, to revoke, then what it did.
        let model = agents.read(cx);
        let now = std::time::SystemTime::now();
        let mut listed_any = false;
        for client in Client::ALL {
            let grants = self.grants(agents, client, cx);
            let entries: Vec<gpui::Div> = (model.activity(client.id()).take(ACTIVITY_SHOWN))
                .map(|entry| {
                    // Each entry in the words a task's trace says its steps in.
                    let title = |entry: &agents::Activity| {
                        let did = agents::tools::Did {
                            tool: entry.action.to_string(),
                            site: entry.site.to_string(),
                            target: entry.target.to_string(),
                            ..Default::default()
                        };
                        let tense = match entry.refused.is_empty() {
                            true => agents::tools::Tense::Done,
                            false => agents::tools::Tense::Failed,
                        };
                        let title = agents::tools::step_title(&did, tense);
                        // The call's answer carried a site's notes (#4122).
                        match entry.notes.is_empty() {
                            true => title,
                            false => agents::notes::logged(&title),
                        }
                    };
                    let what = title(entry);
                    // Why it was refused, but for a permission the browser refused the page,
                    // whose title says it whole.
                    let shown = |entry: &agents::Activity| {
                        agents::tools::refusal_shown(&entry.action, &entry.refused).to_string()
                    };
                    let refused = (entry.steps.iter().map(shown))
                        .find(|refused| !refused.is_empty())
                        .unwrap_or_else(|| shown(entry));
                    let steps = entry.steps.iter().map(title);
                    let detail: SharedString = if !refused.is_empty() {
                        format!("Refused: {refused}").into()
                    } else if !entry.steps.is_empty() {
                        steps.collect::<Vec<_>>().join(" · ").into()
                    } else {
                        SharedString::default()
                    };
                    let detail = Some(detail).filter(|detail| !detail.is_empty());
                    let ago = div()
                        .text_color(look.detail)
                        .child(agents::ago(entry.at, now));
                    let said =
                        (!entry.said.is_empty()).then(|| agents::said(client.name(), &entry.said));
                    self.request_row(agents, entry, what, detail, said, ago, cx)
                })
                .collect();
            if grants.is_none() && entries.is_empty() {
                continue;
            }
            listed_any = true;
            // An app with no name of its own is "this app" here, as Another app would read as a
            // name (table OA1).
            let heading = match model.nameless(client.id()) {
                true => "What this app may do".into(),
                false => format!("What {} may do", model.agent_name(client.id())).into(),
            };
            blocks.push(Block::Heading(heading));
            blocks.extend(grants.map(Block::Group));
            if !entries.is_empty() {
                blocks.push(Block::Group(group(look, entries)));
            }
        }
        // ⌘T tasks' answers are their CLI's, as each task is new (#1913).
        let bar = shell::keystroke_label("cmd-t");
        for cli in Cli::ALL {
            let rows = self.answers(agents, &agents::task_answers(cli), cx);
            if rows.is_empty() {
                continue;
            }
            access.push(Block::Heading(
                format!("What {} may do from {bar}", cli.name()).into(),
            ));
            access.push(Block::Group(group(look, rows)));
        }
        if !listed_any {
            blocks.push(Block::Heading("Activity".into()));
            blocks.push(Block::Group(group(
                look,
                [line_row(
                    look,
                    "Nothing yet",
                    Some("What agents do while the browser runs shows here.".into()),
                    Vec::new(),
                )],
            )));
        }
        [
            Page::new(Section::Threads, "", threads),
            Page::new(
                Section::Models,
                Section::Models.lede().unwrap_or_default(),
                models,
            ),
            Page::new(Section::Access, agents::tier::NEVER_GETS, access),
            Page::new(
                Section::OtherApps,
                Section::OtherApps.lede().unwrap_or_default(),
                blocks,
            ),
        ]
    }

    /// Other apps' own switch (#4260), at the top of their page, where it is kept: only the user's
    /// click sets it.
    fn other_apps(&self, agents: &Entity<Agents>, accent: Rgba, cx: &mut Context<Self>) -> Row {
        let name = format!("Let other apps' agents use {PRODUCT}");
        let on = agents.read(cx).other_apps();
        let switch = {
            let agents = agents.clone();
            let set = move |on, cx: &mut App| {
                agents.update(cx, |agents, cx| agents.set_other_apps(on, cx))
            };
            self.toggle("other apps", name.clone(), on, set, accent, cx)
        };
        // S30, batch 12: off says what off does; on, the row is bare.
        let line = (!on).then(|| "Off: no app connects.".into());
        setting(&self.look, name, line, vec![switch])
    }

    /// Access: what each tier lets a task do, in words only, since only a task's own menu sets
    /// one. The level's menu opens Settings here, flashing the group (settings::show_row).
    fn access(&self) -> Vec<Block> {
        use agents::tier::{ACCESS, Tier};
        let look = &self.look;
        let rows = Tier::ALL.map(|tier| {
            setting(
                look,
                tier.label(),
                Some(tier.explained().into()),
                Vec::new(),
            )
            .div
        });
        let heading: SharedString = ACCESS.into();
        vec![
            Block::Heading(heading.clone()),
            Block::Group(self.flashed(&heading, group(look, rows))),
        ]
    }

    /// Files and folders (#4101) on a system that can't wall a task in with them: why they aren't
    /// offered (X9), under its own heading, where a user who looks for them finds it. A block of
    /// its own, which leans on no other row, so it can move as Settings is regrouped.
    fn files_not_here(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Vec<Block> {
        use agents::files::words;
        if agents.read(cx).files_available() {
            return Vec::new();
        }
        let why = line_row(&self.look, words::NOT_AVAILABLE, None, Vec::new());
        vec![
            Block::Heading(words::ON_THIS_COMPUTER.into()),
            Block::Group(group(&self.look, [why])),
        ]
    }

    /// Each folder a live task holds (#4101), with Revoke, which acts at once, under its own
    /// heading; not drawn while no task holds one, nor where files aren't available. A block of
    /// its own, which leans on no other row, so it can move as Settings is regrouped.
    fn folders_in_use(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Vec<Block> {
        use agents::files::words;
        let look = &self.look;
        let model = agents.read(cx);
        if !model.files_available() {
            return Vec::new();
        }
        let rows: Vec<gpui::Div> = (model.folders_in_use().into_iter().enumerate())
            .map(|(index, held)| {
                let (agents, agent) = (agents.clone(), held.agent.clone());
                let revoke = self
                    .button(("agent-folder-revoke", index as u64), words::REVOKE)
                    .relative()
                    // By its place, never the folder's name, as this line is logged.
                    .child(self.drawn(format!("revoke folder {index}")))
                    .on_click(move |_, _, cx| {
                        agents.update(cx, |agents, cx| agents.revoke_folder(&agent, cx))
                    });
                line_row(
                    look,
                    held.folder,
                    Some(held.task),
                    vec![revoke.into_any_element()],
                )
            })
            .collect();
        if rows.is_empty() {
            return Vec::new();
        }
        vec![
            Block::Heading(words::FOLDERS_IN_USE.into()),
            Block::Group(group(look, rows)),
        ]
    }

    /// Always ask first (#3724): the sites the user fenced, each with Remove, then Add Site…, which
    /// opens the field that adds one, and what the list does.
    fn fenced_sites(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Vec<Block> {
        use agents::fence;
        let look = &self.look;
        let fenced = agents.read(cx).fenced().to_vec();
        let mut rows: Vec<gpui::Div> = (fenced.iter().enumerate())
            .map(|(index, host)| {
                let (agents, gone) = (agents.clone(), host.clone());
                let remove = self
                    .button(("agent-fence-remove", index as u64), "Remove")
                    .relative()
                    .child(self.drawn(format!("unfence {host}")))
                    .on_click(move |_, _, cx| {
                        agents.update(cx, |agents, cx| agents.unfence(&gone, cx))
                    });
                line_row(look, host.clone(), None, vec![remove.into_any_element()])
            })
            .collect();
        if fenced.is_empty() && !self.fence_adding {
            rows.push(line_row(
                look,
                fence::NONE,
                Some(fence::NONE_LINE.into()),
                Vec::new(),
            ));
        }
        if self.fence_adding {
            let field = div()
                .relative()
                .flex_1()
                .min_w_0()
                .h(px(CONTROL_HEIGHT))
                .px(px(FIELD_PADDING))
                .flex()
                .items_center()
                .rounded(px(CONTROL_RADIUS))
                .bg(look.control)
                .key_context("Naming")
                .on_action(cx.listener(|settings, _: &KeepName, window, cx| {
                    if browser::dismissal::submits([&settings.fence_field.clone()], cx) {
                        settings.add_fence(window, cx)
                    }
                }))
                .on_action(cx.listener(|settings, _: &CancelName, window, cx| {
                    let field = settings.fence_field.clone();
                    if browser::dismissal::Close::escape_from([&field], cx).is_some() {
                        settings.close_fence(window, cx)
                    }
                }))
                .child(self.drawn("fence field".into()))
                .child(self.fence_field.clone());
            let cancel = self
                .button("agent-fence-cancel", "Cancel")
                .relative()
                .child(self.drawn("fence cancel".into()))
                .on_click(cx.listener(|settings, _, window, cx| settings.close_fence(window, cx)));
            let add = self
                .button("agent-fence-add", "Add")
                .relative()
                .child(self.drawn("fence add".into()))
                .on_click(cx.listener(|settings, _, window, cx| settings.add_fence(window, cx)));
            let adding = frame()
                .gap(px(CONTROL_GAP))
                .child(field)
                .child(cancel)
                .child(add);
            rows.push(adding);
        }
        // Its last line's foot, where check.sh scrolls it into sight before it captures it whole.
        let end = div().absolute().left_0().right_0().bottom_0().h(px(1.));
        let foot = foot(look, fence::FOOT)
            .relative()
            .child(end.child(self.drawn("fence foot".into())));
        let under = match self.fence_adding {
            true => div().child(foot),
            false => {
                let open = self
                    .button("agent-fence-open", fence::ADD_SITE)
                    .relative()
                    .child(self.drawn("fence open".into()))
                    .on_click(
                        cx.listener(|settings, _, window, cx| settings.open_fence(window, cx)),
                    );
                div()
                    .flex()
                    .flex_col()
                    .child(div().flex().child(open))
                    .child(foot.mt(px(BUTTONS_TOP)))
            }
        };
        // An ilo line's link opens here, flashing the list (Browser::off_limits).
        let heading: SharedString = fence::HEADING.into();
        vec![
            Block::Heading(heading.clone()),
            Block::Group(self.flashed(&heading, group(look, rows))),
            Block::Under(under),
        ]
    }

    /// Site notes (#4122): the switch, then a row for each site ilo has a note for, which opens
    /// to the note's own text, as the agent gets it. The list is the app's, so nothing adds to it.
    fn site_notes(
        &self,
        agents: &Entity<Agents>,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> Vec<Block> {
        use agents::notes;
        let look = &self.look;
        let on = agents.read(cx).notes_on();
        let switch = {
            let agents = agents.clone();
            let set =
                move |on, cx: &mut App| agents.update(cx, |agents, cx| agents.set_notes_on(on, cx));
            self.toggle("agent site notes", notes::HEADING, on, set, accent, cx)
        };
        let line = match on {
            true => notes::SWITCH_LINE,
            false => notes::SWITCH_OFF_LINE,
        };
        let row = setting(look, notes::HEADING, Some(line.into()), vec![switch]);
        let rows: Vec<gpui::Div> = (notes::NOTES.iter())
            .map(|note| {
                let host = note.host;
                let opened = self.notes_opened.contains(host);
                let toggle = cx.listener(move |settings, _, _, cx| {
                    if !settings.notes_opened.remove(host) {
                        settings.notes_opened.insert(host);
                    }
                    cx.notify();
                });
                let logged = self.drawn_logged.clone();
                browser::settings_rows::site_note_row(look, note, opened, self.a11y, logged, toggle)
            })
            .collect();
        vec![
            Block::Rows(vec![row]),
            Block::Group(group(look, rows)),
            Block::Under(foot(look, notes::FOOT)),
        ]
    }

    /// Add Site…: opens Always ask first's field, empty, with the keyboard.
    fn open_fence(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fence_adding = true;
        (self.fence_field).update(cx, |field, cx| field.reset("".into(), window, cx));
        let field = self.fence_field.focus_handle(cx);
        window.focus(&field, cx);
        // check.sh waits for this line.
        log!("browser: settings fence field open");
        cx.notify();
    }

    /// Adds the site typed in Always ask first's field and closes it. A field holding no site
    /// stays open, as it was.
    fn add_fence(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.fence_field.read(cx).text().to_string();
        let Some(agents) = agents::global(cx) else {
            return;
        };
        if agents
            .update(cx, |agents, cx| agents.fence(&typed, cx))
            .is_none()
        {
            log!("browser: settings fence field holds no site");
            return;
        }
        self.close_fence(window, cx);
    }

    /// Cancel, or Escape in the field: closes it, adding nothing.
    fn close_fence(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fence_adding = false;
        (self.fence_field).update(cx, |field, cx| field.reset("".into(), window, cx));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Scripts (#704): the switch that lets AppleScript in, off at first, and what each script
    /// did, refused or not, under the app that sent it.
    fn scripts(&self, agents: &Entity<Agents>, accent: Rgba, cx: &mut Context<Self>) -> Vec<Block> {
        let look = &self.look;
        let on = agents.read(cx).scripts_on();
        let switch = {
            let agents = agents.clone();
            let set = move |on, cx: &mut App| {
                agents.update(cx, |agents, cx| agents.set_scripts_on(on, cx))
            };
            self.toggle(
                "scripts",
                "Let scripts control the browser",
                on,
                set,
                accent,
                cx,
            )
        };
        let row = feature_row(
            look,
            "icons/square-terminal.svg",
            "Let scripts control the browser",
            Some(
                "AppleScript lists windows, Spaces and tabs, switches them and opens pages. It \
                 never reads what's in a page."
                    .into(),
            ),
            vec![switch],
        );
        let mut blocks = vec![
            Block::Heading("Scripts".into()),
            Block::Group(group(look, [row])),
        ];
        let now = std::time::SystemTime::now();
        let entries: Vec<gpui::Div> = (agents.read(cx).activity(agents::scripts::AGENT))
            .take(ACTIVITY_SHOWN)
            .map(|entry| {
                let ago = div()
                    .text_color(look.detail)
                    .child(agents::ago(entry.at, now));
                line_row(
                    look,
                    agents::scripts::title(entry),
                    Some(agents::scripts::detail(entry).into()),
                    vec![ago.into_any_element()],
                )
            })
            .collect();
        if !entries.is_empty() {
            blocks.push(Block::Heading("What scripts did".into()));
            blocks.push(Block::Group(group(look, entries)));
        }
        blocks
    }

    /// An entry of an agent's activity, its time at its end and then the chevron's place, which a
    /// request fills: a press opens it to show where it went and what it sent (#2326). Every row
    /// keeps the place, so every time ends on one edge.
    fn request_row(
        &self,
        agents: &Entity<Agents>,
        entry: &agents::Activity,
        what: String,
        detail: Option<SharedString>,
        said: Option<String>,
        ago: gpui::Div,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let look = &self.look;
        let id = entry.id;
        let opens = !entry.url.is_empty();
        let opened = self.requests_opened.contains(&id);
        let slot = chevron_slot(opens.then_some(opened), look.detail.into());
        let controls = vec![ago.into_any_element(), slot.into_any_element()];
        let head = line_row(look, what.clone(), detail, controls);
        if !opens {
            return head;
        }
        let head = head
            .id(("request", id))
            .relative()
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(browser::settings_rows::ring(look.ink))
            .when(self.a11y, |head| {
                named_button(head, what.clone()).aria_expanded(opened)
            })
            // Its method, which check.sh tells reads from changes by, and no path: its "/" would
            // end the run's pattern.
            .child(self.drawn(format!("request {id} {}", method(&entry.target))))
            .on_click({
                let (agents, agent) = (agents.clone(), entry.agent.clone());
                cx.listener(move |settings, _, _, cx| {
                    settings.open_request(&agents, &agent, id, cx)
                })
            });
        let all = self.requests_all.contains(&id);
        let refused = !entry.refused.is_empty();
        let colors = (look.ink.into(), look.detail.into());
        let more = cx.listener(move |settings: &mut Self, _: &gpui::ClickEvent, _, cx| {
            settings.requests_all.insert(id);
            // check.sh waits for this line.
            log!("browser: settings request {id} shows all");
            cx.notify();
        });
        let part = opened.then(|| {
            sent_part(
                ("request-all", id),
                said,
                &entry.url,
                &entry.sent,
                refused,
                all,
                colors,
                self.a11y,
                move |window, cx| more(&gpui::ClickEvent::default(), window, cx),
            )
            .px(px(ROW_X))
            .pb(px(browser::tokens::settings::PADDING))
            .mt(px(-SENT_TUCK))
        });
        div().child(head).children(part)
    }

    /// Opens an agent's request in its activity, or closes it, and says what it shows.
    fn open_request(
        &mut self,
        agents: &Entity<Agents>,
        agent: &str,
        id: u64,
        cx: &mut Context<Self>,
    ) {
        if !self.requests_opened.remove(&id) {
            self.requests_opened.insert(id);
        }
        let opened = self.requests_opened.contains(&id);
        let said = |sent: &[agents::request::Sent]| {
            let lines = sent.iter().map(|line| match line.name.is_empty() {
                true => line.value.clone(),
                false => format!("{}: {}", line.name, line.value),
            });
            lines.collect::<Vec<_>>().join(" · ")
        };
        let found = (agents.read(cx).activity(agent))
            .find(|entry| entry.id == id)
            .map(|entry| {
                (
                    entry.url.clone(),
                    entry.refused.is_empty(),
                    said(&entry.sent),
                    entry.said.clone(),
                )
            });
        if let (true, Some((.., words))) = (opened, &found)
            && !words.is_empty()
        {
            // check.sh reads this line: the agent's words, first, as plain text.
            log!("browser: settings request {id} said: {words}");
        }
        // check.sh reads this line: where it went, and what it sent, secrets hidden.
        match (opened, found.map(|(url, ok, sent, _)| (url, ok, sent))) {
            (true, Some((url, true, sent))) => {
                log!("browser: settings request {id} opened to {url}, sent: {sent}")
            }
            (true, Some((url, false, sent))) => {
                log!("browser: settings request {id} opened to {url}, not sent: {sent}")
            }
            _ => log!("browser: settings request {id} closed"),
        }
        cx.notify();
    }

    /// What is kept of the threads, which no setting turns off: the row is named by how many ilo
    /// holds a conversation's id for, which isn't a promise each still takes a reply, and has
    /// Remove All…, which asks first, as nothing brings a conversation back. With none the row
    /// stays, so what a Remove All did can be seen.
    fn kept_tasks(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Row {
        let look = &self.look;
        let (kept, stuck) = agents.read(cx).kept_tasks();
        let (name, removes) = browser::settings_rows::kept_row(kept);
        // Every id held is counted, so those are among them.
        let stuck = (stuck > 0).then(|| {
            let said = format!("{stuck} of them couldn't be removed");
            div().text_color(look.detail).child(said).into_any_element()
        });
        let remove = removes.then(|| {
            let agents = agents.clone();
            self.button("remove-kept-tasks", REMOVE_ALL)
                .on_click(move |_, _, cx| ask_remove_kept(agents.clone(), kept, None, cx))
                .into_any_element()
        });
        let controls = stuck.into_iter().chain(remove).collect();
        nested(setting(look, name, Some(KEPT_AS.into()), controls))
    }

    /// The finished tasks kept, the last to end first, each by its title and the Archive's own
    /// words for it, with Remove, which asks first as Remove All… does: the list of what ilo
    /// keeps of them, and the way out for one.
    fn kept_rows(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Vec<Row> {
        let kept = agents.read(cx).kept_list(cx);
        (kept.into_iter())
            .map(|(task, title, about)| {
                let agents = agents.clone();
                let remove = self
                    .button(SharedString::from(format!("remove-kept-{task}")), REMOVE)
                    .on_click(move |_, _, cx| {
                        ask_remove_kept(agents.clone(), 1, Some(task.clone()), cx)
                    })
                    .into_any_element();
                nested(setting(&self.look, title, Some(about), vec![remove]))
            })
            .collect()
    }

    /// Asks before the `kept` finished tasks' conversations are removed, or `only` that task's,
    /// as nothing brings them back: in an alert, or on Linux in Settings' own dialog, as a
    /// profile's Delete asks.
    fn ask_remove_kept(
        &mut self,
        agents: &Entity<Agents>,
        kept: usize,
        only: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (message, detail, action) = match kept {
            1 => (
                "Remove 1 thread?".to_string(),
                "The conversation is removed from this computer.".to_string(),
                REMOVE,
            ),
            kept => (
                format!("Remove {kept} threads?"),
                "Their conversations are removed from this computer.".to_string(),
                "Remove All",
            ),
        };
        let agents = agents.clone();
        let remove = move |cx: &mut App| {
            log!("browser: kept tasks removed from settings");
            agents.update(cx, |agents, cx| match &only {
                Some(task) => agents.remove_kept_task(task, cx),
                None => agents.remove_kept_tasks(cx),
            })
        };
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            action,
            dialog::Return::Action,
            Box::new(remove),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            cx.spawn(async move |_, cx| {
                if crate::ask(&message, &detail, &[action, "Cancel"], true) == 0 {
                    cx.update(remove);
                }
            })
            .detach();
        }
    }

    /// Where `which`'s menu opens: under the button the canvas fills, at its start.
    fn menu_at(&self, which: Popup) -> gpui::Canvas<()> {
        let texts = self.popup_texts.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                let at = point(bounds.origin.x, bounds.origin.y + bounds.size.height);
                let mut texts = texts.borrow_mut();
                texts.retain(|&(each, _)| each != which);
                texts.push((which, at));
            },
        )
        .absolute()
        .inset_0()
    }

    /// The sites where the user answered the agent for good, each with Revoke, and Revoke All
    /// for several, then the tabs of its Space the user doesn't hold, with Take Back All; None
    /// for none.
    fn grants(&self, agents: &Entity<Agents>, client: Client, cx: &App) -> Option<gpui::Div> {
        let look = &self.look;
        let mut rows = self.answers(agents, client.id(), cx);
        let tabs = agents.read(cx).tabs_to_take(client.id(), cx).len();
        if tabs > 0 {
            let agents = agents.clone();
            let take = self
                .button(("take-back-agent-tabs", client as usize), "Take Back All")
                .on_click(move |_, _, cx| {
                    agents.update(cx, |agents, cx| agents.take_back_all(client.id(), cx))
                });
            let open = match tabs {
                1 => "1 tab it may act in".into(),
                _ => format!("{tabs} tabs it may act in"),
            };
            rows.push(line_row(
                look,
                "Its tabs",
                Some(open.into()),
                vec![take.into_any_element()],
            ));
        }
        (!rows.is_empty()).then(|| group(look, rows))
    }

    /// The sites where the user answered for good, by `key`, the answers' keeper: a connected
    /// app's id, or a ⌘T CLI's (agents::task_answers). Each has Revoke, and several Revoke All.
    fn answers(&self, agents: &Entity<Agents>, key: &str, cx: &App) -> Vec<gpui::Div> {
        let look = &self.look;
        let grants = agents.read(cx).grants(key);
        let revoke = |site: Option<SharedString>, title| {
            let id = format!("revoke-{key}-{}", site.as_deref().unwrap_or(""));
            let at = self.drawn(format!("revoke {}", site.as_deref().unwrap_or("all")));
            let (agents, key) = (agents.clone(), key.to_string());
            self.button(gpui::ElementId::Name(id.into()), title)
                .relative()
                .child(at)
                .on_click(move |_, _, cx| {
                    agents.update(cx, |agents, cx| agents.revoke(&key, site.as_deref(), cx))
                })
                .into_any_element()
        };
        let several = grants.len() > 1;
        let mut rows: Vec<gpui::Div> = (grants.into_iter())
            .map(|(site, may)| {
                let answer = if may { "Always Allow" } else { "Never" };
                line_row(
                    look,
                    site.clone(),
                    Some(answer.into()),
                    vec![revoke(Some(site), "Revoke")],
                )
            })
            .collect();
        if several {
            rows.push(line_row(
                look,
                "All sites",
                None,
                vec![revoke(None, "Revoke All")],
            ));
        }
        rows
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
        let look = &self.look;
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
            client.set_up_line().into()
        } else if client == Client::Other {
            // Says which it looked for, so one missing from the list isn't a mystery.
            let missing: Vec<&str> = (Client::APPS.into_iter())
                .filter(|client| !listed.contains(client))
                .map(Client::name)
                .collect();
            model.other_client_detail(&missing).into()
        } else {
            format!(
                "Not connected. Connect adds the browser to {}",
                client.config()
            )
            .into()
        };
        let index = client as usize;
        let setup = (connected && client == Client::Other)
            .then(|| model.setup(client))
            .flatten();
        let copy = setup.map(|setup| {
            self.button(("copy-agent-setup", index), "Copy Setup")
                .on_click(move |_, _, cx| {
                    log!("browser: agent setup copied");
                    cx.write_to_clipboard(ClipboardItem::new_string(setup.clone()));
                })
                .into_any_element()
        });
        let agents = agents.clone();
        let (action, at) = match connected {
            true => (
                self.button(("disconnect-agent", index), "Disconnect")
                    .on_click(move |_, _, cx| {
                        agents.update(cx, |agents, cx| agents.disconnect(client, cx))
                    }),
                "disconnect",
            ),
            false => (
                self.button(("connect-agent", index), "Connect")
                    .on_click(move |_, _, cx| {
                        // check.sh reads this line.
                        log!("browser: connect agent {}", client.id());
                        agents.update(cx, |agents, cx| agents.connect(client, cx))
                    }),
                "connect",
            ),
        };
        let action = action
            .relative()
            .child(self.drawn(format!("{at} {}", client.id())));
        let glyph = match client {
            Client::ClaudeCode => "icons/terminal.svg",
            Client::Codex => "icons/square-terminal.svg",
            Client::Other => "icons/cable.svg",
            Client::Command => COMMAND_GLYPH,
        };
        let controls = copy
            .into_iter()
            .chain([action.into_any_element()])
            .collect();
        feature_row(look, glyph, client.name(), Some(detail), controls)
    }

    /// The `ilo` command's row under Command line: Add, or where it's linked with Remove, and the
    /// line to paste with Copy Line when the user's shell doesn't look there (agents::cli).
    fn command_line(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Row {
        let look = &self.look;
        let model = agents.read(cx);
        let client = Client::Command;
        let state = model.command_state();
        let failure = model.failure(client);
        let mono = browser::face::mono(&cx.text_system().all_font_names());
        let mut detail = agents::cli::detail(
            state,
            model.command_flatpak_line().as_deref(),
            model.command_windows_folder().as_deref(),
        );
        // After its own line: the skill written for each program, or one a Remove left (SK1, SK2).
        let (added, changed) = (model.skills(), model.skills_changed());
        let skill = match (added.is_empty(), changed.is_empty()) {
            (false, _) => Some(format!("Skill added for {}.", agents::named(&added))),
            (_, false) => Some(format!(
                "Skill for {} was changed, so it wasn't removed.",
                agents::named(&changed)
            )),
            _ => None,
        };
        if let Some(skill) = skill {
            detail.line = format!("{} {skill}", detail.line);
        }
        let reveal = (!changed.is_empty())
            .then(|| model.skill_to_reveal())
            .flatten();
        let action = |title: &'static str, id: &'static str| {
            let agents = agents.clone();
            self.button(id, title)
                .relative()
                .child(self.drawn(format!("ilo-command {}", title.to_lowercase())))
                .on_click(move |_, _, cx| {
                    // Add follows the row, not the agent: on Windows a command whose PATH entry
                    // was taken off is still connected, and Add puts the entry back (#3783).
                    agents.update(cx, |agents, cx| match agents.command_state() {
                        agents::cli::State::Off => {
                            // check.sh reads this line.
                            log!("browser: connect agent {}", client.id());
                            agents.connect(client, cx)
                        }
                        _ => agents.disconnect(client, cx),
                    })
                })
                .into_any_element()
        };
        let mut controls = Vec::new();
        if let Some(export) = detail.export.clone() {
            let copy = self
                .button("copy-command-line", "Copy Line")
                .relative()
                .child(self.drawn("ilo-command copy line".into()))
                .on_click(move |_, _, cx| {
                    log!("browser: ilo command line copied");
                    cx.write_to_clipboard(ClipboardItem::new_string(export.clone()));
                });
            controls.push(copy.into_any_element());
        }
        if let Some(path) = reveal {
            // The app's words for the system's file manager.
            let title = match cfg!(target_os = "macos") {
                true => "Show in Finder",
                false => "Show in Folder",
            };
            let show = self
                .button("reveal-skill", title)
                .relative()
                .child(self.drawn("ilo-command reveal skill".into()))
                .on_click(move |_, _, _| crate::boosts_host::show_file(&path));
            controls.push(show.into_any_element());
        }
        controls.push(match state {
            agents::cli::State::Off => action("Add", "add-command"),
            _ => action("Remove", "remove-command"),
        });
        let export = (failure.is_none())
            .then_some(detail.export.clone())
            .flatten()
            .map(|export| command_export(look, export, mono));
        let text = match &failure {
            Some(why) => text(look, client.name(), Some(why.clone())),
            None => command_text(look, client.name(), &detail, mono),
        };
        let line: SharedString = match failure {
            Some(why) => why,
            None => [Some(detail.line.as_str()), detail.export.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
                .into(),
        };
        Row {
            div: frame()
                .flex_wrap()
                .child(glyph_plate(look, COMMAND_GLYPH, true))
                .child(text)
                .child(end(controls))
                .children(export),
            name: client.name().into(),
            line: Some(line),
            keys: None,
            nested: false,
        }
    }

    /// Google's Widevine module, where the core offers it: where it is, and the ways to get it,
    /// load it or remove it. The user's, as the note's yes is.
    fn widevine_row(&self, cx: &mut Context<Self>) -> Row {
        let look = &self.look;
        let row = self.widevine.read(cx);
        let (title, detail, actions) = (row.title(), row.detail(), row.actions());
        let buttons = actions.iter().enumerate().map(|(index, &(action, title))| {
            let act = cx.listener(move |settings: &mut Self, _, _, cx| {
                log!("browser: settings widevine {action:?}");
                settings.widevine.update(cx, |row, cx| row.act(action, cx));
            });
            let what = format!("widevine {}", format!("{action:?}").to_lowercase());
            self.button(("widevine", index), title)
                .relative()
                .child(self.drawn(what))
                .on_click(act)
                .into_any_element()
        });
        let buttons: Vec<AnyElement> = buttons.collect();
        feature(look, "icons/video.svg", title, Some(detail.into()), buttons)
    }

    /// Gives the renaming field the keyboard once its profile is listed, which a new one is once
    /// Chrome has made it.
    fn focus_field(&mut self, profiles: &[Profile], window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = &mut self.renaming else {
            return;
        };
        if !profiles
            .iter()
            .any(|profile| profile.id == renaming.profile)
            || !std::mem::take(&mut self.focus_renaming)
        {
            return;
        }
        let field = renaming.name.focus_handle(cx);
        window.focus(&field, cx);
        // check.sh waits for this line, which says the field has the keyboard.
        log!("browser: rename profile {}", renaming.profile);
        let name = renaming.name.clone();
        renaming._watches =
            text_input::on_lose_keyboard(&name, window, cx, |settings, window, cx| {
                settings.keep_name(window, cx)
            });
    }

    /// Shortcuts: the search, then the commands it finds by the menus' sections, each with its
    /// keys in a chip. A click on a command's chip records its next keys; escape leaves them and
    /// ⌫ takes them away. ↺ gives a command its default back, and Restore Defaults every
    /// command, with the favorites numbered and ⌘9 the last tab.
    fn shortcuts(&self, accent: Rgba, cx: &mut Context<Self>) -> Page {
        let mut blocks = Vec::new();
        // Little ilo's commands and New Note's have no row while it's off.
        let little = unsafe { bc::bc_little_windows() };
        let note = new_note_on(cx);
        for (section, rows) in SHORTCUTS {
            let rows: Vec<Row> = rows
                .iter()
                .filter(|(_, keys)| match keys {
                    Keys::Of(Command::NewLittleWindow | Command::OpenInMainWindow) => little,
                    Keys::Of(Command::NewNote | Command::NewNoteSplit) => note,
                    // The Mac's menu bar holds ilo's menu, so there's no control to ring.
                    Keys::Of(Command::FocusMenu) => shell::OFF_THE_MAC,
                    _ => true,
                })
                .map(|(name, keys)| {
                    let control = match (keys, keys.fixed()) {
                        (Keys::Of(command), _) => self.shortcut(*command, &shown(name), accent, cx),
                        (Keys::FavoritesNumbered, _) => {
                            let on = shell::user_keys(cx).favorites_numbered();
                            self.toggle(
                                "favorites numbered",
                                "Include Favorites in ⌘1–⌘8",
                                on,
                                set_favorites_numbered,
                                accent,
                                cx,
                            )
                        }
                        (Keys::LastTabNinth, _) => {
                            let on = shell::user_keys(cx).last_tab_ninth();
                            self.toggle(
                                "last tab ninth",
                                "Last Tab Goes to the Ninth Tab",
                                on,
                                set_last_tab_ninth,
                                accent,
                                cx,
                            )
                        }
                        (_, keys) => self
                            .chip(keys.unwrap_or_default().into(), Chip::Fixed)
                            .into_any_element(),
                    };
                    let name: SharedString = shown(name).into();
                    Row {
                        div: short_row(name.clone(), control),
                        name,
                        line: None,
                        keys: keys_label(keys, cx),
                        nested: false,
                    }
                })
                .collect();
            blocks.push(Block::Heading((*section).into()));
            blocks.push(Block::Rows(rows));
        }
        let restore = self.button("restore-shortcuts", "Restore Defaults");
        let restore = restore
            .relative()
            .child(self.drawn("restore shortcuts".into()));
        let restore = restore.on_click(cx.listener(|settings, _, _, cx| {
            shell::change_keys(cx, UserKeys::reset_all);
            // check.sh waits for this line.
            log!("browser: shortcuts restored");
            settings.recording = None;
            settings.refused = None;
            save_shortcuts(cx);
            cx.notify();
        }));
        blocks.push(Block::Under(div().flex().child(restore)));
        Page::new(
            Section::Shortcuts,
            "Click a command's keys, then press new ones. ↺ gives a command its own back.",
            blocks,
        )
    }

    /// Keys as a chip, as the command bar shows a command's (settings_rows::chip).
    fn chip(&self, keys: SharedString, kind: Chip) -> gpui::Div {
        chip(self.base(), keys, kind)
    }

    /// `command`'s chip, which records its keys if they can be changed, after why the last keys
    /// were refused and the way back to its default, if it has others. An assistive app reads
    /// them by its row's `name`.
    fn shortcut(
        &self,
        command: Command,
        name: &str,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let look = &self.look;
        let user = shell::user_keys(cx);
        let shown = user.shown_keys(command);
        // Chrome's Alt keys that follow a key off the Mac, as Alt+Left does ⌘←, which only change
        // with it.
        let twins = |chip: usize| {
            let key = shown.get(chip).and_then(|keys| keys.first());
            (key.map(|key| shell::chrome_twins(key))
                .unwrap_or_default()
                .into_iter())
            .map(|twin| self.chip(shell::pressed_label(twin).into(), Chip::Fixed))
            .map(IntoElement::into_any_element)
        };
        if !shell::rebindable(command) {
            let chips = (0..shown.len()).flat_map(|chip| {
                let keys = shell::keystroke_label(shown[chip][0]).into();
                std::iter::once(self.chip(keys, Chip::Fixed).into_any_element()).chain(twins(chip))
            });
            return div()
                .flex()
                .items_center()
                .gap(px(CHIP_GAP))
                .children(chips.collect::<Vec<_>>())
                .into_any_element();
        }
        let said = |text: SharedString| div().text_color(look.detail).child(text);
        let refused = (self.refused.as_ref())
            .filter(|(refused, _)| *refused == command)
            .map(|(_, why)| said(why.clone()));
        // Who gets the keys first, once the user has chosen ilo, as the menu below says it.
        let preferred = user
            .prefers_ilo(command)
            .then(|| said(MenuEntry::label(PREFER_ILO)));
        let restore = user.changed(command).then(|| {
            let id = SharedString::from(format!("restore-{}", command.key_name()));
            stop_restore(look, id, name, self.a11y)
                .child(self.drawn(format!("restore {}", command.key_name())))
                .on_click(cx.listener(move |settings, _, _, cx| settings.restore(command, cx)))
        });
        // A chip each key, None when there's none, each recording only its own key. The first's
        // names are the command's, as check.sh clicks them.
        let chips = (0..shown.len().max(1)).flat_map(|chip| {
            let recording = self.recording == Some((command, chip));
            let (keys, kind) = match shown.get(chip) {
                _ if recording => ("Type Keys".into(), Chip::Keys),
                Some(keys) => (shell::keystroke_label(keys[0]).into(), Chip::Keys),
                None => ("None".into(), Chip::None),
            };
            let (id, drawn) = match chip {
                0 => (
                    command.key_name().to_string(),
                    command.key_name().to_string(),
                ),
                _ => (
                    format!("{}-{}", command.key_name(), chip + 1),
                    format!("{} {}", command.key_name(), chip + 1),
                ),
            };
            let shown = stop_chip(look, self.base(), id, name, keys, kind, self.a11y)
                .when(recording, |chip| {
                    chip.text_color(accent).border_1().border_color(accent)
                })
                .on_click(cx.listener(move |settings, _, window, cx| {
                    settings.start_recording(command, chip, window, cx)
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |settings, event: &MouseDownEvent, window, cx| {
                        settings.choose_preference(command, event.position, window, cx)
                    }),
                )
                .relative()
                .child(self.drawn(format!("shortcut {drawn}")));
            std::iter::once(shown.into_any_element()).chain(twins(chip))
        });
        div()
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .children(refused)
            .children(preferred)
            .children(restore)
            .children(chips.collect::<Vec<_>>())
            .into_any_element()
    }

    /// Import & Export: the import's view, as it is, and Export Spaces as Markdown.
    fn import_export(&self, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        let export = self
            .button("export-spaces", "Export…")
            .relative()
            .child(self.drawn("export spaces".into()))
            .on_click(|_, _, cx| crate::export_spaces(cx));
        let export = setting(
            look,
            "Spaces as Markdown",
            None,
            vec![export.into_any_element()],
        );
        let import = self.import.as_ref().map(|import| import.read(cx));
        // Inside an import, the heading names the browser it's from, and Export waits for its end.
        let inside = import.and_then(ImportView::crumb).is_some();
        let heading = match import.and_then(ImportView::crumb) {
            Some(inside) => Block::Crumb("Import".into(), inside),
            None => Block::Heading("Import".into()),
        };
        let under_way = import.is_some_and(ImportView::under_way);
        let mut blocks = vec![heading];
        blocks.extend(
            self.import
                .clone()
                .map(|import| Block::View(import.into_any_element())),
        );
        if !under_way {
            blocks.push(Block::Heading("Export".into()));
            blocks.push(Block::Rows(vec![export]));
        }
        // Inside an import the crumb says where it is, so the page's sentence gives way.
        let lede = match inside {
            true => "",
            false => "Bring your things from another browser, or take your Spaces with you.",
        };
        Page::new(Section::SyncImport, lede, blocks)
    }

    /// About: the app's name as the page's heading, its version and its engine's, where its
    /// update stands and, where the build carries a nightly feed, whether it takes nightly builds,
    /// then the licence it's under and the notices of what it's built from, each of which opens in
    /// a tab.
    fn about(&self, accent: Rgba, cx: &mut Context<Self>) -> Page {
        let look = &self.look;
        // check.sh reads the engine's version here.
        let line = format!("browser: settings about {}", version(cx));
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
                let open = self.button(id, "Open").relative().child(at);
                open.on_click(move |_, _, cx| open_notice(&url, cx))
                    .into_any_element()
            })
        };
        let offered = updates::followed(cx).is_some();
        let notices = [
            detailed_row(
                look,
                "Licence",
                concat!(
                    env!("PRODUCT_NAME"),
                    " is free software: you may use, study, share and change it under the GNU \
                     General Public License, version 3."
                ),
                open("licence", carried("LICENSE")),
            ),
            detailed_row(
                look,
                "Notices",
                "The projects it adapts code, icons and words from.",
                open("notices", carried("NOTICE")),
            ),
            detailed_row(
                look,
                "Libraries",
                "The Rust libraries built into the app, and their licences.",
                open("libraries", carried("THIRD-PARTY.txt")),
            ),
            detailed_row(
                look,
                "Chromium",
                "The engine's credits, and the licences of what it's built from.",
                open("chromium", Some("chrome://credits".into())),
            ),
        ];
        let version = [
            Some(div().child(self.version.clone())),
            offered.then(|| div().child(self.nightly.clone())),
        ];
        let whats_new = self.toggle(
            "whats new",
            "Show what's new after an update",
            !off(updates::NO_WHATS_NEW),
            set_whats_new,
            accent,
            cx,
        );
        let whats_new = setting(
            look,
            "Show what's new after an update",
            Some("Its notes open in a tab. What's New opens them any time.".into()),
            vec![whats_new],
        );
        // The Mac's and Windows' own updater's: a Flatpak's updates are the system's, and the
        // Store's are the Store's.
        let checks = (cfg!(not(target_os = "linux")) && updates::own_updater(cx)).then(|| {
            let on = automatic_update_checks(cx);
            let switch = self.toggle(
                "automatic update checks",
                "Check for updates automatically",
                on,
                set_automatic_update_checks,
                accent,
                cx,
            );
            let line = match on {
                true => "Every 8 hours, and it downloads them in the background.".to_string(),
                false => format!(
                    "{} won't look for new versions until you choose Check for Updates.",
                    env!("PRODUCT_NAME")
                ),
            };
            setting(
                look,
                "Check for updates automatically",
                Some(line.into()),
                vec![switch],
            )
        });
        let blocks = vec![
            Block::Group(group(look, version.into_iter().flatten())),
            Block::Rows(checks.into_iter().chain([whats_new]).collect()),
            Block::Group(group(look, notices)),
        ];
        Page::new(Section::About, "a good tool.", blocks)
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
            MenuEntry {
                checked: !ilo,
                ..MenuEntry::new("Prefer Websites", can)
            },
            MenuEntry {
                checked: ilo,
                ..MenuEntry::new(PREFER_ILO, can)
            },
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
        show_menu(
            ns_view(window),
            items,
            at,
            false,
            self.menu_look(),
            menu::picked(picked),
            cx,
        );
    }

    /// Takes the next keys pressed for `command`'s `chip`th chip, or stops if it's taking them
    /// already.
    fn start_recording(
        &mut self,
        command: Command,
        chip: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refused = None;
        self.recording = match self.recording == Some((command, chip)) {
            true => None,
            false => {
                // check.sh waits for this line.
                log!("browser: shortcut recording {}", command.key_name());
                Some((command, chip))
            }
        };
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Gives the command being recorded `keystroke` in place of the chip's key, unless it's
    /// escape, which leaves it, or ⌫, which takes it away. Keys that can't be its stay
    /// recording, saying why.
    fn record(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some((command, chip)) = self.recording else {
            return;
        };
        cx.stop_propagation();
        let plain = !keystroke.modifiers.modified();
        let keys = match keystroke.key.as_str() {
            "escape" if plain => {
                log!("browser: shortcut recording stopped");
                self.recording = None;
                self.refused = None;
                return cx.notify();
            }
            "backspace" if plain => Ok(Vec::new()),
            _ => shell::recorded(keystroke),
        };
        let set =
            keys.and_then(|keys| shell::change_keys(cx, |user| user.set_key(command, chip, keys)));
        match set {
            Ok(_) => {
                self.recording = None;
                self.refused = None;
                save_shortcuts(cx);
            }
            Err(refused) => {
                let why = refusal(refused);
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

fn set_link_cards(on: bool, cx: &mut App) {
    crate::link_cards::set_on(on, cx);
}

fn set_sign_in_close(on: bool, cx: &mut App) {
    crate::sign_in_close::set_sign_in_close(on, cx);
}

fn set_little_windows(on: bool, cx: &mut App) {
    crate::set_little_windows(on, cx);
}

/// Where New Note's switch is kept: there while it's off. The app's, not a profile's, as its keys
/// and the command bar are.
fn new_note_off_path() -> String {
    format!("{}/new-note-off", crate::profile_dir())
}

/// Reads New Note's switch, before the windows open and read it.
pub fn read_new_note(cx: &mut App) {
    let on = !std::path::Path::new(&new_note_off_path()).exists();
    cx.set_global(shell::NewNoteOn(on));
}

fn new_note_on(cx: &App) -> bool {
    cx.try_global::<shell::NewNoteOn>().is_none_or(|on| on.0)
}

/// Every window follows at once; the tabs New Note opened stay.
fn set_new_note(on: bool, cx: &mut App) {
    let path = new_note_off_path();
    _ = match on {
        true => std::fs::remove_file(path),
        false => std::fs::write(path, ""),
    };
    cx.set_global(shell::NewNoteOn(on));
    crate::browser_windows::update_shells_in(cx, |shell, cx| shell.set_new_note(on, cx));
}

/// Where Copy links without tracking is kept: there while it's off (#2351). The app's, not a
/// profile's, as its copy paths are.
fn clean_links_off_path() -> String {
    format!("{}/copy-links-as-is", crate::profile_dir())
}

/// Reads Copy links without tracking, before the windows open and read it.
pub fn read_clean_links(cx: &mut App) {
    let on = !std::path::Path::new(&clean_links_off_path()).exists();
    cx.set_global(shell::CleanLinksOn(on));
}

fn clean_links_on(cx: &App) -> bool {
    cx.try_global::<shell::CleanLinksOn>().is_none_or(|on| on.0)
}

/// Every window follows at once.
fn set_clean_links(on: bool, cx: &mut App) {
    let path = clean_links_off_path();
    _ = match on {
        true => std::fs::remove_file(path),
        false => std::fs::write(path, ""),
    };
    cx.set_global(shell::CleanLinksOn(on));
    crate::browser_windows::update_shells_in(cx, |shell, cx| shell.set_clean_links(on, cx));
}

fn set_unload_idle_tabs(unload: bool, _: &mut App) {
    unsafe { bc::bc_set_unload_idle_tabs(unload) };
}

/// Chrome's Local State pref, so off turns Battery Saver off at once and it never comes on.
fn set_battery_saver(on: bool, _: &mut App) {
    unsafe { bc::bc_set_battery_saver_enabled(on) };
}

/// In every profile; a site's own stays as the user set it in the Site Controls.
fn set_ad_blocking(on: bool, _: &mut App) {
    unsafe { bc::bc_set_ad_blocking(on) };
}

/// Ad Blocking's groups of filter lists, in its order, as the core names them and as the page
/// heads them.
const AD_BLOCK_GROUPS: [(&str, &str); 7] = [
    ("default", "Built-in"),
    ("ads", "Ads"),
    ("privacy", "Privacy"),
    ("malware", "Malware"),
    ("multipurpose", "Multipurpose"),
    ("annoyances", "Annoyances"),
    ("regions", "Regional"),
];

/// One of the ad blocker's filter lists, as Ad Blocking lists it.
struct AdBlockList {
    key: SharedString,
    group: SharedString,
    title: SharedString,
    on: bool,
}

/// The ad blocker's filter lists, by group and title, each on or off.
fn ad_block_lists() -> Vec<AdBlockList> {
    let lists = core_string(bc::bc_ad_block_lists);
    (lists.lines())
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let (key, group, on) = (parts.next()?, parts.next()?, parts.next()?);
            Some(AdBlockList {
                key: key.to_owned().into(),
                group: group.to_owned().into(),
                on: on == "1",
                title: parts.next()?.to_owned().into(),
            })
        })
        .collect()
}

fn set_ad_block_list(key: &str, on: bool) {
    let Ok(key) = CString::new(key) else {
        return;
    };
    unsafe { bc::bc_set_ad_block_list(key.as_ptr(), on) };
}

/// My filters, each line that holds one.
fn ad_block_filters() -> Vec<String> {
    let filters = core_string(bc::bc_ad_block_filters);
    (filters.lines().map(str::trim))
        .filter(|filter| !filter.is_empty())
        .map(str::to_owned)
        .collect()
}

fn set_ad_block_filters(filters: &[String]) {
    let Ok(filters) = CString::new(filters.join("\n")) else {
        return;
    };
    unsafe { bc::bc_set_ad_block_filters(filters.as_ptr()) };
}

/// A string the core's `copy` copies, as bc_crash_reports_collector does.
fn core_string(copy: unsafe extern "C" fn(*mut std::ffi::c_char, usize) -> usize) -> String {
    let length = unsafe { copy(std::ptr::null_mut(), 0) };
    let mut text = vec![0u8; length + 1];
    unsafe { copy(text.as_mut_ptr().cast(), text.len()) };
    text.truncate(length);
    String::from_utf8_lossy(&text).into_owned()
}

/// On, a page's WebRTC goes only through its TURN relays, as it does in a profile with a proxy.
fn set_relay_calls(on: bool, _: &mut App) {
    unsafe { bc::bc_set_relay_calls(on) };
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
    core_string(bc::bc_crash_reports_collector)
}

fn set_link_previews(previews: bool, _: &mut App) {
    unsafe { bc::bc_set_link_previews(previews) };
}

/// In every profile; a site the user allows in its Site Controls still floats.
fn set_mini_player(on: bool, _: &mut App) {
    unsafe { bc::bc_set_auto_picture_in_picture(on) };
}

/// The app's own switches that are on unless the user turned them off, each a file in the
/// profile directory while it's off, as the core keeps no settings of the app's own.
const NO_EDGE_REVEAL: &str = "no-edge-reveal";
/// What's recent in hover cards, off (#3074); and the sites Site Controls turned it off for, a line
/// each.
const NO_RECENT: &str = "no-recent-in-cards";
const RECENT_SITES_OFF: &str = "recent-off-sites";
/// Sidebar position, there while it's Right (#2375): this device's, as the other switches are, and
/// never synced.
const SIDEBAR_RIGHT: &str = "sidebar-right";

fn off(name: &str) -> bool {
    std::path::Path::new(&format!("{}/{name}", crate::profile_dir())).exists()
}

fn set_off(name: &str, is_off: bool) {
    let path = format!("{}/{name}", crate::profile_dir());
    _ = match is_off {
        true => std::fs::write(path, ""),
        false => std::fs::remove_file(path),
    };
}

/// Hands the app's switches to what they change, at launch.
pub fn read_switches(cx: &mut App) {
    cx.set_global(shell::SidebarEdge(!off(NO_EDGE_REVEAL)));
    cx.set_global(shell::SidebarRight(off(SIDEBAR_RIGHT)));
    browser::recent::set_switches(!off(NO_RECENT), recent_sites_off(), cx);
}

fn recent_sites_off() -> HashSet<SharedString> {
    std::fs::read_to_string(format!("{}/{RECENT_SITES_OFF}", crate::profile_dir()))
        .unwrap_or_default()
        .lines()
        .filter(|site| !site.is_empty())
        .map(|site| SharedString::from(site.to_string()))
        .collect()
}

/// Off, every profile's kept rows go with it, and no card asks again until it's back on.
fn set_recent(on: bool, cx: &mut App) {
    set_off(NO_RECENT, !on);
    browser::recent::set_on(on, cx);
}

/// Site Controls' switch for `site` (#3074), kept on this device as General's is.
pub fn set_recent_site(site: &str, on: bool, cx: &mut App) {
    browser::recent::set_site_on(site, on, cx);
    let mut sites: Vec<_> = cx
        .global::<browser::recent::Recents>()
        .sites_off
        .iter()
        .cloned()
        .collect();
    sites.sort();
    let path = format!("{}/{RECENT_SITES_OFF}", crate::profile_dir());
    _ = match sites.is_empty() {
        true => std::fs::remove_file(path),
        false => std::fs::write(path, sites.join("\n") + "\n"),
    };
}

/// Whether Sidebar position is kept as Right, for the launch, before the windows read it.
pub fn sidebar_right_kept() -> bool {
    off(SIDEBAR_RIGHT)
}

/// Moves the sidebar to the window's right, or back to its left: kept on this device, and every
/// window follows at once, as each reads the side as it draws.
pub fn set_sidebar_right(right: bool, cx: &mut App) {
    set_off(SIDEBAR_RIGHT, right);
    cx.set_global(shell::SidebarRight(right));
    cx.refresh_windows();
}

fn set_edge_reveals(reveals: bool, cx: &mut App) {
    set_off(NO_EDGE_REVEAL, !reveals);
    cx.set_global(shell::SidebarEdge(reveals));
}

/// About's "Check for updates automatically", which only the Mac and Windows show. It reads the
/// updates' own state first, so it shows what the update banner's Update Automatically set.
fn automatic_update_checks(cx: &App) -> bool {
    if let Some(on) = updates::automatic(cx) {
        return on;
    }
    #[cfg(not(target_os = "linux"))]
    return crate::updates_host::automatic_checks();
    #[cfg(target_os = "linux")]
    true
}

// Through the updates' own state, as the update banner reads it.
fn set_automatic_update_checks(on: bool, cx: &mut App) {
    updates::set_automatic(on, cx);
}

fn set_whats_new(shown: bool, _: &mut App) {
    set_off(updates::NO_WHATS_NEW, !shown);
}

fn set_ask_where_to_save(ask: bool, _: &mut App) {
    unsafe { bc::bc_set_ask_where_to_save(ask) };
}

fn set_notification_prompts(on: bool, _: &mut App) {
    unsafe { bc::bc_set_notification_prompts(on) };
}

fn set_warn_moving_profiles(warn: bool, _: &mut App) {
    unsafe { bc::bc_set_warn_moving_profiles(warn) };
}

/// The browser window in front's shell, if there's one.
fn front_shell() -> Option<Entity<shell::Shell>> {
    let (_, view) = crate::browser_windows::front();
    crate::browser_windows::shell_with_view(view).upgrade()
}

/// Tabs & Sidebar's Restore…: Settings goes, and the window in front lists the backups where its
/// command bar was, newest first, each asking first, as the command bar's Restore Earlier
/// Sidebar… does.
fn restore_earlier_sidebar(cx: &mut App) {
    let Some(shell) = front_shell() else {
        return;
    };
    log!("browser: settings restore sidebar");
    hide(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::RestoreEarlierSidebar, "settings", cx)
    });
}

fn set_option_reveals(reveals: bool, _: &mut App) {
    unsafe { bc::bc_set_option_reveals_sidebar(reveals) };
}

thread_local! {
    static ABOUT_LOGGED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// The app's version and its engine's: a nightly's says so first, as long as a release's says
/// Version, so About's first row reads the same width.
fn version(cx: &App) -> String {
    let built = match updates::built(cx) {
        Track::Nightly => "Nightly",
        Track::Release => "Version",
    };
    let version = format!("{built} {}", build_info::version());
    match build_info::ENGINE_VERSION.as_deref() {
        Some(engine) => format!("{version}, on Chromium {engine}"),
        None => version,
    }
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
    path.is_file()
        .then(|| shell::file_url(&path.to_string_lossy()))
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
        log!(
            "browser: download folder of profile {profile} {} {done}",
            folder.display()
        );
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

/// Opens Chrome's list of the passkeys this Mac keeps for `profile` in a new tab of one of its
/// Spaces and goes to it, where each can be renamed or deleted.
fn manage_passkeys(profile: u64, cx: &mut App) {
    // check.sh waits for this line.
    log!("browser: manage passkeys {profile}");
    crate::open_in_profile(profile, "chrome://settings/passkeys");
    hide(cx);
}

/// A request's method, the first word of its target ("POST /api/items").
fn method(target: &str) -> &str {
    target.split(' ').next().unwrap_or_default()
}

/// Apps that connect's heading.
const APPS_THAT_CONNECT: &str = "Apps that connect";

/// A line under a group that says more of it, in a row's line's grey, as Fenced sites' has.
fn foot(look: &RowLook, line: impl Into<SharedString>) -> gpui::Div {
    div()
        .px(px(TEXT_X))
        .text_size(px(text::SMALL))
        .line_height(px(DETAIL_LINE))
        .text_color(look.detail)
        .child(line.into())
}

/// A row: its name, the line under it, if any, and its controls at its end.
fn line_row(
    look: &RowLook,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    controls: Vec<AnyElement>,
) -> gpui::Div {
    frame()
        .child(text(look, name, line))
        .when(!controls.is_empty(), |row| row.child(end(controls)))
}

/// A feature's row, or a thing's in a list: its glyph on its quiet plate before its name.
fn feature_row(
    look: &RowLook,
    glyph: &'static str,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    controls: Vec<AnyElement>,
) -> gpui::Div {
    frame()
        .child(glyph_plate(look, glyph, true))
        .child(text(look, name, line))
        .when(!controls.is_empty(), |row| row.child(end(controls)))
}

/// The line under Settings › Agents' row for the threads kept (table KT2), and the buttons: a
/// kept task's own, and the one for them all. An assistant that keeps none has no task in the
/// list, which says it.
const KEPT_AS: &str = "Threads are kept as you saw them. Your assistant keeps the conversation.";
const REMOVE: &str = "Remove";
const REMOVE_ALL: &str = "Remove All…";

/// Asks before the kept conversations go, or `only` that task's (`Settings::ask_remove_kept`),
/// once the click that asked is over, in Settings' window.
fn ask_remove_kept(agents: Entity<Agents>, kept: usize, only: Option<SharedString>, cx: &mut App) {
    cx.defer(move |cx| {
        let Some(window) = cx.try_global::<SettingsWindow>().map(|w| w.0) else {
            return;
        };
        let ask = |settings: &mut Settings, window: &mut Window, cx: &mut Context<Settings>| {
            settings.ask_remove_kept(&agents, kept, only.clone(), window, cx)
        };
        window.update(cx, ask).ok();
    });
}

/// The `ilo` command's glyph, under Command line.
const COMMAND_GLYPH: &str = "icons/square-code.svg";

/// A setting's row: its name, its line and its controls, as the search reads it.
fn setting(
    look: &RowLook,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    controls: Vec<AnyElement>,
) -> Row {
    let name = name.into();
    Row {
        div: line_row(look, name.clone(), line.clone(), controls),
        name,
        line,
        keys: None,
        nested: false,
    }
}

/// A feature's row, its glyph on its quiet plate before its name, as the search reads it.
fn feature(
    look: &RowLook,
    glyph: &'static str,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    controls: Vec<AnyElement>,
) -> Row {
    let name = name.into();
    Row {
        div: feature_row(look, glyph, name.clone(), line.clone(), controls),
        name,
        line,
        keys: None,
        nested: false,
    }
}

/// A row whose control is drawn pictures, as Settings' search reads it.
fn pictured(
    look: &RowLook,
    name: &'static str,
    line: SharedString,
    pictures: impl IntoElement,
) -> Row {
    Row {
        div: browser::settings_rows::pictured(look, name, line.clone(), pictures),
        name: name.into(),
        line: Some(line),
        keys: None,
        nested: false,
    }
}

/// A feature's own setting, under it in its group, its text and the hairline over it starting
/// where the feature's name does.
fn nested(row: Row) -> Row {
    Row {
        nested: true,
        ..row
    }
}

/// A row whose control fills it, such as a shortcut's chip: shorter than one with a line.
fn short_row(name: impl Into<SharedString>, control: AnyElement) -> gpui::Div {
    div()
        .min_h(px(SHORT_ROW))
        .px(px(ROW_X))
        .py(px(SHORT_PADDING))
        .flex()
        .items_center()
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
        .child(end([control]))
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
    named
        .chain(match command {
            Command::SelectTab(_) => Some("Tabs 1 to 8"),
            Command::RecentTabBack => Some("Last Used Tab"),
            Command::Undo => Some("Undo"),
            Command::Redo => Some("Redo"),
            _ => None,
        })
        .next()
        .unwrap_or("another command")
}

/// The keys a Shortcuts row's chips show, a line each, which the search finds it by.
fn keys_label(keys: &Keys, cx: &App) -> Option<String> {
    match keys {
        Keys::Of(command) => {
            let shown = shell::user_keys(cx).shown_keys(*command);
            let labels = shown.iter().flat_map(|chip| {
                let twins = shell::chrome_twins(chip[0]).into_iter();
                std::iter::once(shell::keystroke_label(chip[0]))
                    .chain(twins.map(shell::pressed_label))
            });
            Some(labels.collect::<Vec<_>>().join("\n")).filter(|labels| !labels.is_empty())
        }
        Keys::FavoritesNumbered | Keys::LastTabNinth => None,
        _ => keys.fixed(),
    }
}

/// A row's name as the tab shows it, with ⌘1–⌘8 in this OS's keys, Ctrl+1–Ctrl+8 off the Mac.
fn shown(name: &str) -> String {
    let tabs = format!(
        "{}–{}",
        shell::keystroke_label("cmd-1"),
        shell::keystroke_label("cmd-8")
    );
    name.replace("⌘1–⌘8", &tabs)
}

/// Where the user's keys are kept: in the profile directory, beside Chromium's profiles, as they
/// stay with the computer (docs/ux-spec.md, Sync).
fn shortcuts_path() -> String {
    format!("{}/Shortcuts", crate::profile_dir())
}

/// Reads the keys the user gave the commands.
pub fn read_shortcuts(cx: &mut App) {
    let text = std::fs::read_to_string(shortcuts_path()).unwrap_or_default();
    cx.set_global(UserKeys::parse(&text));
}

/// Where View › Appearance's choice was kept before the core kept it, beside the keys.
fn appearance_path() -> String {
    format!("{}/Appearance", crate::profile_dir())
}

/// Where Show Space Header's choice was kept before the core kept it.
fn lone_header_path() -> String {
    format!("{}/Space Header", crate::profile_dir())
}

/// What an older version kept at `path`, which the core keeps from now on, so it goes.
fn take_old(path: String) -> Option<String> {
    let text = std::fs::read_to_string(&path).ok()?;
    if let Err(error) = std::fs::remove_file(&path) {
        log!("couldn't remove {path}: {error}");
    }
    Some(text)
}

/// View › Appearance's choice as the core keeps it.
fn core_appearance() -> Appearance {
    match unsafe { bc::bc_appearance() } {
        1 => Appearance::Light,
        2 => Appearance::Dark,
        _ => Appearance::Automatic,
    }
}

/// Where Portrait Mode's background is kept, in the profile.
fn portrait_background_path() -> String {
    format!("{}/Portrait Background", crate::profile_dir())
}

/// Reads the background Portrait Mode last captured on, if it's kept.
pub fn read_portrait_background(cx: &mut App) {
    let text = std::fs::read_to_string(portrait_background_path()).unwrap_or_default();
    if let Some(background) = crate::capture::portrait::Background::parse(&text) {
        cx.set_global(shell::PortraitBackground(background));
    }
}

pub fn save_portrait_background(background: crate::capture::portrait::Background) {
    if let Err(error) = std::fs::write(portrait_background_path(), background.name()) {
        log!("couldn't save portrait mode's background: {error}");
    }
}

/// Whether Show Space Header keeps a lone unnamed Space's name row, which it doesn't at first.
/// check.sh's runs keep it with BROWSER_SPACE_HEADER, as they place their clicks under it.
fn lone_header_kept() -> bool {
    let kept = unsafe { bc::bc_space_header() };
    kept || std::env::var("BROWSER_SPACE_HEADER").is_ok_and(|env| env.trim() == "shown")
}

/// Reads View › Appearance's choice and Show Space Header's, which the core keeps, as they follow
/// the account. Those an older version kept in files go to the core first.
pub fn read_look(cx: &mut App) {
    let old = take_old(appearance_path()).and_then(|text| Appearance::named(text.trim()));
    if let Some(appearance) = old {
        unsafe { bc::bc_set_appearance(crate::menu_bar::appearance_code(appearance)) };
    }
    if let Some(text) = take_old(lone_header_path()) {
        unsafe { bc::bc_set_space_header(text.trim() == "shown") };
    }
    cx.set_global(shell::AppAppearance(core_appearance()));
    cx.set_global(shell::LoneSpaceHeader(lone_header_kept()));
}

/// View › Appearance's choice or Show Space Header's may have come from the account
/// (settings_synced): every window and the host take them.
pub fn look_followed(cx: &mut App) {
    let appearance = core_appearance();
    if appearance != shell::app_appearance(cx) {
        // check.sh waits for this line.
        log!("browser: appearance followed {}", appearance.name());
        crate::menu_bar::give_host_appearance(appearance);
    }
    let header = lone_header_kept();
    cx.set_global(shell::AppAppearance(appearance));
    cx.set_global(shell::LoneSpaceHeader(header));
    crate::browser_windows::update_shells_in(cx, |shell, cx| {
        shell.look_followed(appearance, header, cx)
    });
    refresh(cx);
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
        self.drawn_logged.borrow_mut().frame();
        self.a11y = window.is_a11y_active();
        // What's on is neutral, the text colour, never green or a Space's colour (#4092).
        let accent = self.look.on;
        // As Linux's client-side decorations leave it, and Windows', where GPUI hides the system's
        // title bar and its buttons for a transparent one; AppKit's frame is always there.
        let frameless = cfg!(target_os = "windows")
            || matches!(
                window.window_decorations(),
                gpui::Decorations::Client { .. }
            );
        if self
            .switched
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < SLIDE)
        {
            window.request_animation_frame();
        }
        // Only a frame drawn on screen counts: AppKit puts the window there some frames after it's
        // shown, and a click before then finds nothing to land on.
        // The row opened at has drawn: its control takes the keyboard, the stop after the row.
        if self.focus_row.is_some() && self.row_drawn.get() {
            self.focus_row = None;
            window.focus(&self.row_focus, cx);
            window.focus_next(cx);
        }
        if self.shown && !window.is_visible() {
            window.request_animation_frame();
        } else if std::mem::take(&mut self.shown) {
            // check.sh waits for this line, the section each show or switch draws, whichever
            // window was key.
            log!("browser: settings drew {}", self.section.name());
        }
        let size = window.viewport_size();
        let size = (
            f32::from(size.width).round(),
            f32::from(size.height).round(),
        );
        if self.drawn_size.replace(size) != Some(size) {
            // check.sh waits for this line after it resizes the window.
            log!("browser: settings drawn at {}x{}", size.0, size.1);
        }
        if self
            .flash
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < FOUND_FLASH_TIME)
        {
            window.request_animation_frame();
        }
        let typed = settings_search::typed(&self.search, cx);
        let (page, found) = match typed.trim().is_empty() {
            true => {
                self.log_search(&[], cx);
                let page = self.section_page(self.section, accent, window, cx);
                (self.page(page, cx), None)
            }
            false => {
                let (results, found) = self.results(&typed, accent, cx);
                (results, Some(found))
            }
        };
        let base = self.base();
        let view = cx.entity().downgrade();
        let pick: settings_column::Pick = Rc::new(move |section, window, cx| {
            let show = |settings: &mut Settings, cx: &mut Context<Settings>| {
                settings.end_search(window, cx);
                settings.show_section(section, window, cx)
            };
            view.update(cx, show).ok();
        });
        let foot: settings_column::Foot = Rc::new(|_, cx| more_settings(cx));
        let search = self.search.clone();
        let field = settings_search::field(
            &self.search,
            base,
            self.drawn("search".into()),
            move |window, cx| {
                settings_search::end(&search, window, cx);
            },
            cx,
        );
        // Keys typed there are the search's, not a command's being recorded.
        let field = div()
            .capture_any_mouse_down(cx.listener(|settings, _, _, cx| {
                settings.recording = None;
                settings.refused = None;
                cx.notify();
            }))
            .child(field);
        let column = settings_column::column(
            &self.sections,
            self.section,
            self.dark,
            self.drawn_logged.clone(),
            field.into_any_element(),
            found.as_deref(),
            pick,
            foot,
            settings_column::Version::of(cx),
            self.a11y,
        );
        // Once scrolled, the section's name and the heading gone under it hold at the card's top,
        // and where rows go on under the card's foot, it fades into the card's fill.
        let colors = settings_card::Colors {
            line: self.look.line,
            name: base.opacity(NAME),
            heading: self.look.heading,
        };
        let card = self.card.draw(self.section.name(), page, self.dark, colors);
        // The stand-in for the window's material, as the browser window's where it has none.
        let ground = if self.dark {
            rgb(STAND_IN)
        } else {
            rgb(LIGHT_STAND_IN)
        };
        div()
            // While a command's key is recorded, every key is the recording's, ⌘W too.
            .when(self.recording.is_none(), |root| {
                root.key_context("Settings")
            })
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|settings, event: &KeyDownEvent, _, cx| {
                settings.record(&event.keystroke, cx)
            }))
            .on_key_down(cx.listener(|settings, event: &KeyDownEvent, window, cx| {
                settings.space_pages(event, window, cx);
                settings.type_to_search(event, window, cx)
            }))
            .on_action(|_: &CloseSettings, _, cx| hide(cx))
            .on_action(cx.listener(|settings, _: &Search, window, cx| {
                settings.recording = None;
                let focus = settings.search.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
                cx.notify();
            }))
            .on_action(
                cx.listener(|settings, _: &PageDown, _, cx| settings.page_card(Paging::Down, cx)),
            )
            .on_action(
                cx.listener(|settings, _: &PageUp, _, cx| settings.page_card(Paging::Up, cx)),
            )
            .on_action(
                cx.listener(|settings, _: &PageTop, _, cx| settings.page_card(Paging::Top, cx)),
            )
            .on_action(
                cx.listener(|settings, _: &PageBottom, _, cx| {
                    settings.page_card(Paging::Bottom, cx)
                }),
            )
            .on_action(|_: &NextControl, window, cx| window.focus_next(cx))
            .on_action(|_: &PreviousControl, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|settings, _: &EndSearch, window, cx| {
                let search = settings.search.clone();
                if browser::dismissal::Close::escape_from([&search], cx).is_some()
                    && !settings.end_search(window, cx)
                {
                    cx.propagate();
                }
            }))
            .on_action(|_: &ReleaseHeldProfile, _, _| crate::import_host::release_held())
            .on_action(
                cx.listener(|settings, _: &browser::text_input::Undo, _, cx| settings.undo(cx)),
            )
            .size_full()
            .relative()
            .flex()
            .bg(ground)
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(self.look.ink)
            .child(column)
            .child(card)
            .children(frameless.then(|| close_button(base, self.drawn("close".into()), self.a11y)))
            .children(self.cover(cx))
    }
}

/// `first`, with each of `rest`'s blocks after its own, on `first`'s page: today's pages that the
/// owner's drawing puts on another's, until each part has its drawn place. A page that starts with
/// rows of its own goes under its old `name`, so where it starts reads.
fn joined<const N: usize>(first: Page, rest: [(&'static str, Page); N]) -> Page {
    let mut page = first;
    for (name, other) in rest {
        let headed = matches!(
            other.blocks.first(),
            Some(Block::Heading(_) | Block::Crumb(..) | Block::Scoped(..))
        );
        if !headed {
            page.blocks.push(Block::Heading(name.into()));
        }
        page.blocks.extend(other.blocks);
    }
    page
}

impl Settings {
    /// `section`'s page, as it shows on the card.
    fn section_page(
        &mut self,
        section: Section,
        accent: Rgba,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Page {
        match section {
            Section::Overview => self.overview(cx),
            Section::General => {
                let profiles = profiles();
                let general = self.general(&profiles, accent, cx);
                joined(
                    general,
                    [("Features", self.features(&profiles, accent, cx))],
                )
            }
            Section::Window => self.appearance_page(cx),
            Section::Privacy => {
                let privacy = self.privacy(&profiles(), accent, cx);
                let off_sites = off_sites();
                self.log_off_sites(&off_sites);
                let ad_blocking = self.ad_blocking(&off_sites, accent, cx);
                joined(privacy, [("Ad Blocking", ad_blocking)])
            }
            Section::Passwords => self.passwords(&profiles(), accent, cx),
            Section::SyncImport => {
                let sync = Page::new(
                    Section::SyncImport,
                    "Sync Spaces, bookmarks, passwords and open tabs across your devices, \
                     encrypted with a key only they hold.",
                    vec![Block::View(self.account.clone().into_any_element())],
                );
                joined(sync, [("Import", self.import_export(cx))])
            }
            Section::Shortcuts => self.shortcuts(accent, cx),
            Section::Threads | Section::Models | Section::Access | Section::OtherApps => {
                match agents::global(cx) {
                    Some(agents) => {
                        let pages = self.agents(&agents, accent, cx);
                        let at = pages.iter().position(|page| page.section == section);
                        pages
                            .into_iter()
                            .nth(at.unwrap_or(0))
                            .unwrap_or_else(|| Page::new(section, "", Vec::new()))
                    }
                    None => Page::new(section, "", Vec::new()),
                }
            }
            Section::TabsAndSpaces => {
                let profiles = profiles();
                self.focus_field(&profiles, window, cx);
                let tabs = self.tabs_and_sidebar(&profiles, accent, cx);
                joined(tabs, [("Profiles", self.profiles(&profiles, cx))])
            }
            Section::Links => self.links(accent, cx),
            Section::About => self.about(accent, cx),
        }
    }

    /// What `typed` finds, in place of the section's page, and the sections it's in. It reads the
    /// sections of settings; the user's own things, their profiles, logins, rules and account,
    /// are found where they're listed.
    fn results(
        &mut self,
        typed: &str,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> (gpui::Div, Vec<Section>) {
        let (entries, mut divs, under) = self.searched_rows(accent, cx);
        let found = settings_search::find(typed, &entries);
        let found: Vec<(Entry, gpui::Div)> = found
            .into_iter()
            .filter_map(|index| Some((entries[index].clone(), divs[index].take()?)))
            .collect();
        self.show_results(found, under, cx)
    }

    /// What the search reads of the sections of settings, in the column's order: each row's entry
    /// and the row, and the buttons under each section's groups.
    #[allow(clippy::type_complexity)]
    fn searched_rows(
        &mut self,
        accent: Rgba,
        cx: &mut Context<Self>,
    ) -> (
        Vec<Entry>,
        Vec<Option<gpui::Div>>,
        Vec<(Section, gpui::Div)>,
    ) {
        let profiles = profiles();
        let mut pages = vec![
            self.general(&profiles, accent, cx),
            self.appearance_page(cx),
            self.tabs_and_sidebar(&profiles, accent, cx),
            self.features(&profiles, accent, cx),
        ];
        if let Some(agents) = agents::global(cx) {
            pages.extend(self.agents(&agents, accent, cx));
        }
        pages.push(self.shortcuts(accent, cx));
        pages.push(self.privacy(&profiles, accent, cx));
        pages.push(self.ad_blocking(&off_sites(), accent, cx));
        pages.push(self.import_export(cx));
        pages.push(self.about(accent, cx));
        // In the column's order, as the results run.
        pages.sort_by_key(|page| Section::all().position(|each| each == page.section));
        let (mut entries, mut divs, mut under) = (Vec::new(), Vec::new(), Vec::new());
        for page in pages {
            // The heading over the groups that follow it, which the search reads too.
            let mut group = None;
            for block in page.blocks {
                let rows = match block {
                    Block::Rows(rows) => rows,
                    Block::Under(buttons) => {
                        under.push((page.section, buttons));
                        continue;
                    }
                    Block::Heading(heading)
                    | Block::Crumb(heading, _)
                    | Block::Scoped(heading, _) => {
                        group = Some(heading);
                        continue;
                    }
                    _ => continue,
                };
                let head = rows
                    .iter()
                    .find(|row| !row.nested)
                    .map(|row| row.name.clone());
                for row in rows {
                    entries.push(Entry {
                        section: page.section,
                        group: group.clone(),
                        under: row.nested.then(|| head.clone()).flatten(),
                        name: row.name,
                        line: row.line,
                        keys: row.keys,
                    });
                    divs.push(Some(row.div));
                }
            }
        }
        (entries, divs, under)
    }

    /// The rows the search found, in place of the section's page, and the sections they're in.
    fn show_results(
        &mut self,
        found: Vec<(Entry, gpui::Div)>,
        under: Vec<(Section, gpui::Div)>,
        cx: &mut Context<Self>,
    ) -> (gpui::Div, Vec<Section>) {
        let names: Vec<&str> = found.iter().map(|(entry, _)| entry.name.as_ref()).collect();
        self.log_search(&names, cx);
        let sections =
            settings_search::sections(&found.iter().map(|(entry, _)| entry).collect::<Vec<_>>());
        let view = cx.entity().downgrade();
        let show: settings_search::Show = Rc::new(move |section, row, window, cx| {
            let show = |settings: &mut Settings, cx: &mut Context<Settings>| {
                settings.show_found(section, row, window, cx)
            };
            view.update(cx, show).ok();
        });
        let logged = self.drawn_logged.clone();
        let drawn = move |what| browser::settings_rows::drawn(what, logged.clone());
        let results = settings_search::results(&self.look, found, under, show, drawn);
        (results, sections)
    }

    /// Logs what the search found once it changes, for check.sh.
    /// Logs what the search found once it changes, and what an input method composes in its
    /// field, which finds nothing until it commits, for check.sh and check/ime.sh.
    fn log_search(&mut self, found: &[&str], cx: &App) {
        let input = self.search.read(cx);
        let (shown, composing) = (input.text().to_string(), input.composing());
        let typed = settings_search::typed(&self.search, cx);
        let typed = typed.trim();
        let now = match composing {
            true => format!("composing {shown:?}"),
            false => typed.to_string(),
        };
        if now == self.searched {
            return;
        }
        self.searched = now;
        // check.sh and check/ime.sh wait for these lines.
        match (composing, typed) {
            (true, _) => log!("browser: settings search composing {shown:?}"),
            (false, "") => log!("browser: settings search cleared"),
            (false, _) => log!(
                "browser: settings search {typed:?} finds {}",
                found.join(", ")
            ),
        }
    }

    /// The card's page moves as `to` says, as Page Down, Page Up, Home and End ask.
    fn page_card(&mut self, to: Paging, cx: &mut Context<Self>) {
        if self.card.page(to) {
            cx.notify();
        }
    }

    /// Space and ⇧Space page the card while Settings itself has the keyboard.
    fn space_pages(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(to) = Paging::space(&event.keystroke).filter(|_| self.focus.is_focused(window))
        else {
            return;
        };
        cx.stop_propagation();
        self.page_card(to, cx);
    }

    /// Typing a character with Settings itself holding the keyboard, as it opens, starts a search
    /// with it, as a type-to-search list does: the search field takes the keyboard and the text.
    fn type_to_search(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = &event.keystroke;
        let Some(typed) = keys.key_char.as_deref().filter(|typed| {
            !typed.is_empty() && typed.chars().all(|c| !c.is_control()) && *typed != " "
        }) else {
            return;
        };
        if keys.modifiers.platform || keys.modifiers.control || !self.focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        let focus = self.search.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        let typed = typed.to_string();
        self.search.update(cx, |search, cx| {
            gpui::EntityInputHandler::replace_text_in_range(search, None, &typed, window, cx)
        });
    }

    /// Settings, or the import while it shows, takes the keyboard back once nothing in the window
    /// holds it.
    fn keep_the_keyboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.focused(cx).is_some() {
            return;
        }
        let import = (self.section == Section::SyncImport)
            .then(|| {
                self.import
                    .as_ref()
                    .and_then(|import| import.read(cx).focus())
            })
            .flatten();
        log!(
            "browser: settings keyboard back to {}",
            if import.is_some() {
                "import"
            } else {
                "settings"
            }
        );
        window.focus(&import.unwrap_or_else(|| self.focus.clone()), cx);
    }

    /// Esc: the field empties and the section shows again. False when there was nothing to end.
    fn end_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !settings_search::end(&self.search, window, cx) {
            return false;
        }
        window.focus(&self.focus, cx);
        cx.notify();
        true
    }

    /// A click on a found group's section name: the section shows, scrolled to `row`, which
    /// flashes once.
    fn show_found(
        &mut self,
        section: Section,
        row: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_search(window, cx);
        self.show_section(section, window, cx);
        // check.sh waits for this line.
        log!("browser: settings found {} › {row}", section.name());
        self.card.reveal_next();
        self.row_drawn.set(false);
        self.focus_row = Some(row.clone());
        self.flash = Some((row, Instant::now()));
        cx.notify();
    }

    /// `div`, the row named `name`, with the plate that flashes once under it, up and back, after
    /// a click on its section's name, and the card scrolled to it.
    fn flashed(&self, name: &SharedString, div: gpui::Div) -> gpui::Div {
        let Some((_, at)) = self.flash.as_ref().filter(|(flashed, _)| flashed == name) else {
            return div;
        };
        let t = (at.elapsed().as_secs_f32() / FOUND_FLASH_TIME).min(1.);
        let up = 1. - (2. * t - 1.).abs();
        let drawn = self.row_drawn.clone();
        let focused = self.focus_row.as_ref() == Some(name);
        div.relative()
            .rounded(px(radius::ROW))
            .bg(self.base().opacity(FOUND_FLASH * up))
            .child(self.card.reveal())
            .when(focused, |row| {
                row.track_focus(&self.row_focus)
                    .child(canvas(|_, _, _| {}, move |_, _, _, _| drawn.set(true)))
            })
    }

    #[cfg(target_os = "linux")]
    fn cover(&self, cx: &App) -> Option<AnyElement> {
        dialog::cover(&self.dialog, cx)
    }

    #[cfg(not(target_os = "linux"))]
    fn cover(&self, _: &App) -> Option<AnyElement> {
        None
    }
}
