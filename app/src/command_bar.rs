//! The command bar. GPUI can't draw over the page's native view, so the bar is a window of its
//! own that the host shows over the browser window, as its child. Under its input it lists the
//! core's suggestions for what's typed, and the window grows down to fit them. While the first is
//! selected, the input completes what's typed to it, as Chrome's address bar does. Its size and
//! rows are Arc's (docs/ux-spec.md › Command bar). Tab on a row that hints a search engine
//! searches only with that engine until ⌫ leaves, as Chrome's keyword mode does, and only Tab, as
//! Arc's. A click on the sidebar's URL field opens a narrower panel over the field,
//! as Arc's does. What it asks of the core and of its window goes through `Host`.

use std::time::Instant;
use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, Bounds, Context, Entity, Focusable, FontWeight,
    Global, HoverListenerMode, Hsla, Image, KeyBinding, Pixels, Rgba, ScrollHandle, SharedString,
    TitlebarOptions, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, actions, div, img, point, prelude::*, px, size, svg,
};
use gpui::{Role, Size, accesskit::Live, canvas};

use crate::{
    dismissal::{self, By, Close, Why},
    palette,
    shell::{self, Match, MenuEntry},
    text_input::{self, PasteAndGo, PasteAndSearch, TextInput, without_javascript},
    tooltip::{Anchor, Tip},
};

mod calculator;

actions!(
    command_bar,
    [
        Go,
        GoBeside,
        GoBehind,
        GoDotCom,
        GoWindow,
        GoDotComWindow,
        Search,
        Dismiss,
        Next,
        Previous,
        First,
        Last,
        Remove,
        EnterSite,
        LeaveSite
    ]
);

use crate::settings_column::Section;
use crate::settings_search;
use crate::tokens::command_bar::HEIGHT;
use crate::tokens::command_bar::LINE;
use crate::tokens::command_bar::LINE_INSET;
use crate::tokens::command_bar::LIST_END;
use crate::tokens::command_bar::LIST_TOP;
/// The rows are 6 in from the panel's sides, 50 tall, and the bar shows five before it scrolls.
use crate::tokens::command_bar::LIST_X;
use crate::tokens::command_bar::ROW_HEIGHT;
pub use crate::tokens::command_bar::WIDTH;
const ROWS_SHOWN: usize = 5;
use crate::agents::tier::Tier;
use crate::product;
pub use crate::tokens::command_bar::ABOVE_MIDDLE;
use crate::tokens::command_bar::FIELD_ABOVE;
pub use crate::tokens::command_bar::FIELD_RADIUS;
pub use crate::tokens::command_bar::FIELD_WIDTH;
use crate::tokens::command_bar::HINT_END;
use crate::tokens::command_bar::ICON_BOX;
use crate::tokens::command_bar::ICON_BOX_X;
use crate::tokens::command_bar::ICON_SIZE;
use crate::tokens::command_bar::INPUT_ICON;
use crate::tokens::command_bar::INPUT_ICON_X;
use crate::tokens::command_bar::INPUT_LINE;
use crate::tokens::command_bar::INPUT_TEXT;
use crate::tokens::command_bar::INPUT_TEXT_X;
use crate::tokens::command_bar::PLATE_INSET;
use crate::tokens::command_bar::PLATE_RADIUS;
pub use crate::tokens::command_bar::RADIUS;
use crate::tokens::command_bar::ROW_END;
use crate::tokens::command_bar::TITLE_X;
use crate::tokens::command_bar::{
    FIELD_FADE, HINT_GAP, HINT_KEY_GAP, HINT_KEY_RIM, HINT_KEY_X, MARK,
};
/// There the input's row is a row tall, with no line under it, and its icon and text sit in
/// the rows' icon and title columns.
const FIELD_INPUT: f32 = ROW_HEIGHT;
/// There it hangs from the field as the Site Controls do. The modal shadow, 24 down at a 64 blur,
/// fell mostly below a panel only 50 tall, a grey bar under it over a light page (#1306).
const FIELD_LEVEL: Level = Level::Popover;
/// The level whose shadow the bar's window keeps room for round the bar where it draws its own
/// shadow (#2988): the modal's, which reaches past the popover's, so the bar keeps one margin as it
/// moves between the centre and the field.
pub const ROOM: Level = Level::Modal;
use crate::elevation::Level;
use crate::tokens::command_bar::ARROW;
use crate::tokens::command_bar::ARROW_TILE;
use crate::tokens::command_bar::BESIDE_END;
use crate::tokens::command_bar::CHIP;
use crate::tokens::command_bar::CHIP_RADIUS;
use crate::tokens::command_bar::DIM;
use crate::tokens::command_bar::FIELD_INPUT_TEXT;
use crate::tokens::command_bar::HOVERED;
use crate::tokens::command_bar::LINE_COLOR;
use crate::tokens::command_bar::PLACEHOLDER;
use crate::tokens::command_bar::ROW_GLYPH;
use crate::tokens::command_bar::SELECTED_DIM;
use crate::tokens::command_bar::TITLE;
use crate::tokens::command_bar::site_colors;
use crate::tokens::command_bar::{
    CHOICES_GAP, CHOICES_HEIGHT, CHOICES_LEFT, CHOICES_MENU_DROP, CHOICES_RIGHT, CHOICES_UNDER,
    FOOT_GAP, FOOT_HEIGHT, FOOT_LEFT, FOOT_LINE, FOOT_RIGHT, FOOT_TEXT, MODEL_GAP, MODEL_LEFT,
    MODEL_MARK, START_ARROW, START_SIZE, START_SPACE, TRIGGER_CHEVRON, TRIGGER_GAP, TRIGGER_HEIGHT,
    TRIGGER_LINE, TRIGGER_TEXT, TRIGGER_X,
};
use crate::tokens::text;
/// What the field says with nothing typed: in the bar, the sidebar's field and the composer.
pub(crate) const PLACEHOLDER_TEXT: &str = "Search or enter address";
/// What the bar's field says with nothing typed where the bar can start a thread (Z2).
const THREAD_PLACEHOLDER: &str = "Search, or start a thread";
/// What its input says for Search Tabs (ours, after the command's name).
const TABS_PLACEHOLDER: &str = "Search Tabs…";
const SPACES_PLACEHOLDER: &str = "Search Spaces…";
/// What its input says, and its chip, while it lists only commands (ours).
const COMMANDS_PLACEHOLDER: &str = "Search Commands…";
const COMMANDS_CHIP: &str = "Commands";
/// What its input says in a site search, whose chip names the site, as Arc's pill does (#1397).
const SITE_PLACEHOLDER: &str = "Search…";

/// What a history row's remove button is called, to an assistive app and in its tooltip: it takes
/// the page or search out of the profile's history, not only out of the list.
const REMOVE_NAME: &str = "Remove from History";

/// The keys the remove button's tooltip names: Arc's ⌥⌘⌫ on the Mac, where most keyboards have no
/// ⌦ for Chrome's ⇧⌦, and ⇧⌦ off it, its only key there.
fn remove_keys() -> Vec<SharedString> {
    let keystroke = if shell::OFF_THE_MAC {
        "shift-delete"
    } else {
        "alt-cmd-backspace"
    };
    shell::keystroke_keys(keystroke)
        .into_iter()
        .map(Into::into)
        .collect()
}

/// The key on a hint's chip at the input's end: ↩ and ⇥ on the Mac, and off it Enter and Tab, the
/// words the menus use, as the UI's font there has no keyboard symbols (#772).
fn hint_key(key: &str, off_the_mac: bool) -> String {
    shell::key_name(key, off_the_mac)
}

/// The row that grows the bar into the composer with what's typed, second once something is
/// typed, and the kind at its end (the words table's Z2b).
const NEW_THREAD: &str = "New Thread";
const THREAD_KIND: &str = "Thread";
/// The composer's foot: what ⇥ does at its left, and what ↩ does at its right (Z8).
const BACK_TO_SEARCH: &str = "Back to Search";
const OPENS_AS: &str = "Opens as a thread in its own tab";

/// One of the core's suggestions for what's typed.
#[derive(Default)]
pub struct Suggestion {
    pub url: String,
    pub text: SharedString,
    /// Shown after the text, dimmer, unless it's empty.
    pub detail: SharedString,
    /// The tab already at the page, which the row switches to, or 0.
    pub tab: u64,
    /// What completes the text typed to this row, for the first row only.
    pub completion: String,
    /// Whether ⇧⌦, ⌥⌘⌫ or its trash can take it out of the profile's history: a page visited or a
    /// search typed before, never one of the well-known sites or the search for what's typed.
    pub removable: bool,
    /// The browser command the row runs instead, and its shortcut.
    pub command: Option<shell::Command>,
    pub shortcut: Option<SharedString>,
    /// What a sum typed comes to, which the row copies instead.
    pub result: Option<SharedString>,
    /// Whether it's a search for what was typed, which the bar finds out as the rows come.
    pub search: bool,
    /// The keyword of the search engine whose site search Tab enters on this row, and its name.
    pub keyword: String,
    pub site: SharedString,
    /// A .new address typed, whose new document ↩ opens in a new tab even from a tab's URL, as
    /// it's a document of its own rather than the page edited.
    pub new_document: bool,
    /// While the bar asks an agent (Agent task): the agent the row runs the task on, or the row
    /// that sets agents up.
    pub task: Option<TaskRow>,
    /// The mark of the Space a Spaces menu's row sends the page to.
    pub swatch: Option<crate::menu::Swatch>,
    /// A setting's row, which opens Settings at it: its section and its name, as Settings' search
    /// finds it.
    pub setting: Option<(Section, SharedString)>,
    /// The account's other computer the page is open on, which the row names at its end, or
    /// empty (#1234).
    pub computer: SharedString,
    /// When the profile's history last saw a visit to the page, in microseconds since 1970, as
    /// the core gives it, or 0 (#2337).
    pub last_visit: i64,
    /// The archived tab at the page, which ↩ reopens as the tab it was rather than loading the
    /// page afresh, or 0, and what its row says at its end, such as "Closed today · Reopen"
    /// (#2337).
    pub archived: u64,
    pub closed: SharedString,
    /// What the row of a page the profile's history has says at its end, such as "Visited
    /// yesterday", or empty (#2337).
    pub visited: SharedString,
}

/// A tab open on one of the account's other computers, as sync last brought it (#1234).
#[derive(Clone, Debug, PartialEq)]
pub struct OtherTab {
    pub url: SharedString,
    pub title: SharedString,
    /// The computer, as Settings › Sync names it.
    pub computer: SharedString,
}

/// The most of them the bar lists for what's typed (ours).
const OTHER_TABS_SHOWN: usize = 2;

/// A row that starts a thread rather than going somewhere.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskRow {
    /// New Thread: grows the bar into the composer with what's typed.
    Thread,
}

/// An agent ⌘T's ask lists (Agent task).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskAgent {
    pub id: SharedString,
    pub name: SharedString,
    /// Its glyph, Settings › Agents' own.
    pub glyph: &'static str,
    /// The model it runs tasks on, by name, or None for its own default.
    pub model: Option<SharedString>,
    /// That model by id, empty for its own default: Settings' Model row.
    pub model_id: String,
    /// Its Model menu's first row, `{assistant}'s default`, and which model that is where it said.
    pub default_row: SharedString,
    pub default_model: Option<SharedString>,
    /// The models it offers after its default, as id and name: the one list Settings' Model row
    /// is built from.
    pub models: Vec<(String, String)>,
    /// When its plan's limit resets, if it's at it.
    pub resets: Option<SharedString>,
}

/// What ⌘T's ask offers, as the browser last learned it: the agents signed in, the default first,
/// and whether it has learned which are yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskAgents {
    pub agents: Vec<TaskAgent>,
    pub known: bool,
    /// The assistants installed but not signed in, by id and name, which the composer's model
    /// menu offers to sign in.
    pub signed_out: Vec<(SharedString, SharedString)>,
}

/// The model the next task on `agent` starts on, by id, empty for its own default: the pick for
/// it, or Settings'. A pick it no longer offers is its default, as in Settings.
pub fn next_model(picked: Option<&(SharedString, String)>, agent: &TaskAgent) -> String {
    let picked = picked.filter(|(on, _)| *on == agent.id).map(|(_, id)| id);
    let id = picked.unwrap_or(&agent.model_id);
    match agent.models.iter().any(|(each, _)| each == id) {
        true => id.clone(),
        false => String::new(),
    }
}

/// What the assistant's chip says: its name and the model the next task starts on, or its Model
/// menu's first row for its own default.
pub fn model_words(agent: &TaskAgent, id: &str) -> String {
    match agent.models.iter().find(|(each, _)| each == id) {
        Some((_, name)) => format!("{} · {name}", agent.name),
        None => agent.default_row.to_string(),
    }
}

/// Whether the composer's model pick is for the assistant `id`.
fn ask_model_on(ask: Option<&Ask>, id: &SharedString) -> bool {
    ask.and_then(|ask| ask.model.as_ref())
        .is_some_and(|(on, _)| on == id)
}

/// A heading in a menu named as written, such as an assistant's name over its models.
fn heading(title: SharedString) -> MenuEntry {
    MenuEntry {
        header: true,
        ..MenuEntry::named(title, false)
    }
}

/// Records where the element it's put in is drawn, for a menu to hang from it.
fn drawn_at(at: &Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    let at = at.clone();
    canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
        .absolute()
        .size_full()
}

/// A pick on the composer's model menu.
#[derive(Clone, Debug)]
enum ModelPick {
    /// A header or a line, which is never chosen.
    Nothing,
    /// A model of an assistant, by the assistant's id and the model's, empty for its default.
    Model(SharedString, String),
    /// Sign In under an assistant not signed in, by its id.
    SignIn(SharedString),
    Settings,
}

/// Words on the composer's choices, from the words table (Z9, A5).
const SIGN_IN: &str = "Sign In";
const SETTINGS: &str = "Settings";
/// The model menu's search field (Z9).
const SEARCH_MODELS: &str = "Search models";
const NEVER_GETS: &str = "A thread never gets passwords, cookies or your other tabs.";
/// The composer's triggers, by their elements' ids.
const LEVEL: &str = "composer-level";
const MODEL: &str = "composer-model";

/// The most settings the bar lists for what's typed, after the commands that start with it.
const SETTINGS_SHOWN: usize = 3;

/// The widest the chip before the input grows, as a page's title can be long, and its favicon's
/// size, as the chip's text is the rows' body size.
const CHIP_MAX: f32 = 180.;
const CHIP_FAVICON: f32 = 12.;
/// The tile after a row's end, as Switch to Tab's: its margin from the row's end, and its glyph's
/// box.
const END_TILE_RIGHT: f32 = 3.;
const END_GLYPH: f32 = 14.;

/// How far from the row's end its end ends with no tile after it, as a visited page's day: at the
/// tile's glyph's right edge, so it lines up with a closed tab's ↺ over or under it, the tile's
/// plate hanging past (the orchestrator, 11-14).
fn bare_end() -> f32 {
    END_TILE_RIGHT + (ICON_BOX - END_GLYPH) / 2.
}

/// A task ⇥ is making of what's typed: the page that goes with it, as its address and title, which
/// ⌫ at the field's start takes off.
struct Ask {
    page: Option<(SharedString, SharedString)>,
    /// The level the thread starts at: what the composer last started one with in the profile,
    /// or the user's pick on its level menu. Only these set it.
    tier: Tier,
    /// The assistant and model picked on the model menu, by the assistant's id and the model's,
    /// empty for its own default; None for the first ready assistant on Settings' model.
    model: Option<(SharedString, String)>,
}

/// What a row's icon stands for.
enum Kind {
    Page,
    Search,
    Command,
    Sum,
}

/// What a row shows for its icon.
#[derive(Debug, PartialEq)]
enum Icon {
    /// Its page's favicon, with a globe until it comes or if it has none.
    Favicon,
    Glyph(&'static str),
    /// A Space's mark, as Move to Space's menu shows it.
    Swatch(crate::menu::Swatch),
    /// The product's mark, in the text's colour.
    Mark,
    None,
}

/// The glyph a command shares with the others no Lucide icon plainly names.
const COMMAND_GLYPH: &str = "icons/command.svg";

impl Suggestion {
    /// Whether it's New Thread, which grows the bar into the composer rather than going.
    fn starts_thread(&self) -> bool {
        self.task == Some(TaskRow::Thread)
    }

    /// A command's, a sum's or a setting's row, rather than a page's or a search's.
    fn named(&self) -> bool {
        self.command.is_some() || self.result.is_some() || self.setting.is_some()
    }

    fn kind(&self) -> Kind {
        if self.setting.is_some() {
            return Kind::Command;
        }
        match (&self.command, &self.result, self.search) {
            (Some(_), _, _) => Kind::Command,
            (_, Some(_), _) => Kind::Sum,
            (_, _, true) => Kind::Search,
            _ => Kind::Page,
        }
    }

    /// Arc's: a page only in the profile's history has a clock, and an open tab or a well-known
    /// site its favicon. Each command has its own glyph.
    fn icon(&self) -> Icon {
        if let Some(swatch) = self.swatch {
            return Icon::Swatch(swatch);
        }
        match &self.task {
            Some(TaskRow::Thread) => return Icon::Mark,
            None => {}
        }
        if let Some((section, _)) = &self.setting {
            return Icon::Glyph(section.glyph());
        }
        match self.kind() {
            // An archived tab comes back as itself, so it has its favicon as an open tab does.
            Kind::Page if self.tab == 0 && self.removable && self.archived == 0 => {
                Icon::Glyph("icons/clock.svg")
            }
            Kind::Page => Icon::Favicon,
            Kind::Search => Icon::Glyph("icons/search.svg"),
            Kind::Command => Icon::Glyph(self.command.map_or(COMMAND_GLYPH, command_glyph)),
            Kind::Sum => Icon::None,
        }
    }
}

/// A command's glyph: the Lucide icon that plainly names what it does, else the shared one.
fn command_glyph(command: shell::Command) -> &'static str {
    use shell::Command;
    match command {
        Command::Clear => "icons/brush-cleaning.svg",
        Command::ClearCacheAndRefresh => "icons/trash.svg",
        Command::ClearCookiesAndRefresh => "icons/cookie.svg",
        Command::ClearArchive | Command::CloseTab | Command::ViewArchive => "icons/archive.svg",
        Command::CollapseFolders | Command::CollapsePinned => "icons/fold-vertical.svg",
        Command::ExpandFolders => "icons/folder-open.svg",
        Command::ExpandPinned => "icons/chevrons-up-down.svg",
        Command::FocusSidebar | Command::FocusAgentQuestion => "icons/keyboard.svg",
        Command::FocusMenu => "icons/nav-menu.svg",
        Command::NewFolder => "icons/folder-plus.svg",
        Command::TogglePin | Command::PinToSpace(_) => "icons/pin.svg",
        Command::OpenFolder(_) => "icons/folder-open.svg",
        Command::SelectSpace(_) => "icons/layers.svg",
        Command::MoveToSpace(_) | Command::MoveToFolder(_) => "icons/arrow-right.svg",
        Command::TurnOffBatterySaver => "icons/leaf.svg",
        Command::TranslatePage | Command::UseOriginal => "icons/languages.svg",
        Command::ToggleMute | Command::MuteAll(false) => "icons/volume-2.svg",
        Command::MuteAll(true) => "icons/volume-x.svg",
        Command::ToggleFullScreen => "icons/maximize.svg",
        Command::Exit => "icons/x.svg",
        Command::ChangeProfile(_) => "icons/user.svg",
        Command::ReopenClosedTab => "icons/undo-2.svg",
        Command::CopyUrl | Command::CopyMarkdownLink => "icons/link.svg",
        Command::PasteUrlAsTab => "icons/clipboard.svg",
        Command::Share => "icons/share.svg",
        Command::Find => "icons/text-search.svg",
        Command::SearchTabs => "icons/search.svg",
        Command::SiteControls => "icons/sliders-horizontal.svg",
        Command::Cast => "icons/cast.svg",
        Command::Back => "icons/arrow-left.svg",
        Command::Forward => "icons/arrow-right.svg",
        Command::Reload | Command::ForceReload => "icons/rotate-cw.svg",
        Command::Stop => "icons/x.svg",
        Command::StayOnTop => "icons/pin.svg",
        Command::ZoomIn => "icons/zoom-in.svg",
        Command::ZoomOut => "icons/zoom-out.svg",
        Command::FlipSplit => "icons/arrow-left-right.svg",
        Command::ToggleSidebar => "icons/panel-left.svg",
        Command::MoveSidebar => "icons/panel-right.svg",
        Command::CapturePortion | Command::CaptureFullPage => "icons/camera.svg",
        Command::CapturePortrait | Command::CapturePortraitNow => "icons/image.svg",
        Command::SavePage => "icons/save.svg",
        Command::Print => "icons/printer.svg",
        Command::ViewSource => "icons/code.svg",
        Command::ToggleDeveloperMode => "icons/square-code.svg",
        Command::InspectElements => "icons/square-mouse-pointer.svg",
        Command::JavaScriptConsole => "icons/terminal.svg",
        Command::RecordTrace => "icons/activity.svg",
        Command::ManageExtensions | Command::AddExtension | Command::RunExtension(_) => {
            "icons/puzzle.svg"
        }
        Command::RenameSpace | Command::RenameTab => "icons/pencil.svg",
        Command::EditTheme => "icons/palette.svg",
        Command::ExportSpace => "icons/file.svg",
        Command::NextTab => "icons/arrow-down.svg",
        Command::PreviousTab => "icons/arrow-up.svg",
        Command::ViewLibrary => "icons/library.svg",
        Command::ViewDownloads => "icons/arrow-down-to-line.svg",
        Command::ViewMedia => "icons/image.svg",
        Command::ViewAgents => "icons/square-mouse-pointer.svg",
        Command::NewBoost | Command::EditBoost | Command::ViewBoosts => "icons/paintbrush.svg",
        Command::ViewHistory => "icons/rotate-ccw-clock.svg",
        Command::ClearBrowsingData => "icons/brush-cleaning.svg",
        Command::NewNote | Command::NewNoteSplit => "icons/file.svg",
        Command::NewBoard | Command::DuplicateBoard | Command::ViewBoards => {
            "icons/presentation.svg"
        }
        Command::Settings | Command::SiteSearchSettings => "icons/settings.svg",
        Command::KeyboardShortcuts => "icons/keyboard.svg",
        Command::About => "icons/info.svg",
        Command::CheckForUpdates => "icons/rotate-cw.svg",
        Command::Import => "icons/arrow-down-to-line.svg",
        Command::Passwords => "icons/key-round.svg",
        Command::HelpCenter => "icons/book-open.svg",
        Command::WhatsNew => "icons/megaphone.svg",
        Command::PrivacyPolicy => "icons/shield.svg",
        Command::ReportProblem => "icons/flag.svg",
        Command::RestoreEarlierSidebar => "icons/rotate-ccw-clock.svg",
        Command::CopyBuildInfo => "icons/copy.svg",
        _ => COMMAND_GLYPH,
    }
}

impl From<shell::BarTab> for Suggestion {
    fn from(tab: shell::BarTab) -> Self {
        Suggestion {
            url: tab.url.to_string(),
            text: tab.title,
            tab: tab.tab,
            ..Default::default()
        }
    }
}

impl From<shell::BarCommand> for Suggestion {
    fn from(command: shell::BarCommand) -> Self {
        Suggestion {
            text: command.name,
            command: Some(command.command),
            shortcut: command.shortcut,
            ..Suggestion::default()
        }
    }
}

/// What the bar asks of the browser: of the core, for suggestions and pages, and of the host, for
/// the bar's window. The browser's are calls to the core's C API; the tests' are recorded.
pub trait Host {
    /// Asks for the suggestions for `text`, the first completing it if `complete`, or the searches
    /// for it in the site search of the engine whose keyword is `site`. They come back through
    /// `suggested`.
    fn suggest(&self, site: Option<&str>, text: &str, complete: bool);
    /// Takes the suggestion going to `url` out of the profile's history, a step on the window's ⌘Z
    /// history. The suggestions come again without it.
    fn remove_suggestion(&self, url: &str, cx: &mut App);
    /// ⌘Z, or ⇧⌘Z for `redo`: undoes or redoes the window's next step if it's a removal from
    /// history, so a key in the bar's field does nothing out of sight. Whether it did.
    fn step_removal(&self, redo: bool, cx: &mut App) -> bool;
    /// The page the address bar goes to for `text` typed in it: the one it names, else a search.
    fn typed_url(&self, text: &str) -> Option<String>;
    /// A search for `text` with the search engine whose keyword is `site`, else the default one,
    /// even if it names a page.
    fn search_url(&self, site: Option<&str>, text: &str) -> Option<String>;
    /// Asks for the favicon the profile's history has for the page at `url`, which comes back
    /// through `page_favicon`.
    fn page_favicon(&self, url: &str);
    /// Opens a tab loading `url` at the top of the Space's Today tabs, and shows it unless
    /// `behind`, when it loads only once it's first shown. `typed` is what the user typed for it,
    /// which keeps an http address written out as written (`bc_tab_open_typed`), as for each load
    /// below.
    fn open_tab(&self, url: &str, typed: &str, behind: bool);
    /// Opens a browser window after the one the bar shows over, as ⌘N does, and shows a tab
    /// loading `url` in it. The user's: agents get none, as it opens a window (decision 8).
    fn open_window(&self, url: &str, typed: &str, cx: &mut App);
    fn navigate(&self, tab: u64, url: &str, typed: &str);
    /// Loads `tab`'s page again, as ↩ does on the URL the bar opened on (Arc).
    fn reload(&self, tab: u64);
    fn select(&self, tab: u64);
    /// The browser window that shows `tab`'s page live (`bc_tab_window`), if it's another than
    /// the one the bar shows over.
    fn tab_window(&self, tab: u64) -> Option<u64>;
    /// Makes browser window `window` key and puts it in front, showing `tab` there. The user's:
    /// agents get none, as it takes the user's focus (decision 8).
    fn activate(&self, window: u64, tab: u64, cx: &mut App);
    /// Puts `tab` in a split view with `with`, on its right, and shows it there.
    fn split(&self, tab: u64, with: u64, cx: &mut App);
    /// Opens a tab loading `url` in a split view with `with`, on its right, and shows it there.
    fn open_split(&self, url: &str, typed: &str, with: u64, cx: &mut App);
    /// Copies `text`, a sum's result, and says so.
    fn copy(&self, text: &str, cx: &mut App);
    /// Shows the bar's window, `width` wide, its corners rounded by `radius` and shadowed as
    /// `level` is, with the keyboard: centred across the browser window with its top `top` above
    /// the window's middle, else at `left` and `top` in it.
    fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32, level: Level);
    fn set_height(&self, height: f32);
    /// Hides the bar's window once the user has chosen something in it.
    fn hide(&self);
    /// Gives the page the browser window shows the keyboard, once the bar has hidden over the page
    /// it went to.
    fn focus_page(&self);
    /// Whether the page the bar goes to would be an agent's: a task's page the window shows in
    /// place, or a tab in an agent's Space, neither of which takes the user's keyboard (#3164).
    fn agents_page(&self, cx: &App) -> bool;
    /// Hides the bar's window as the user leaves it without choosing anything.
    fn dismiss(&self);
    /// Gives the bar's window the keyboard back, which it found it hasn't while it shows, unless
    /// the user left it, which the host knows first.
    fn take_keyboard(&self);
    /// The browser's commands that match `text` and do something now, best first.
    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand>;
    /// The agents ⇥ asks a task of, as the browser last learned, or None while tasks are off,
    /// which leaves ⇥ as it was (Agent task). It only reads: nothing about tasks runs while the
    /// bar opens or takes keys.
    fn task_agents(&self, cx: &App) -> Option<TaskAgents>;
    /// Whether the bar shows over a private window, which offers no tasks, as agents get no path
    /// into private windows.
    fn over_private(&self) -> bool;
    /// What Settings' search reads of its rows, which the bar finds settings by, as last built,
    /// which the browser does off the bar's frames: None until it's ready, when the bar lists no
    /// settings rather than wait. It only reads, as the bar opens and is typed in.
    fn settings_index(&self, _: &App) -> Option<Rc<Vec<settings_search::Entry>>> {
        None
    }
    /// Opens Settings at the row named `name` in `section`, scrolled to it and flashed once, as a
    /// click on a found section's name does. Agents have no path to it: it's the bar's alone.
    fn show_setting(&self, _: Section, _: SharedString, _: &mut App) {}
    /// The ask shows, or no longer does: Today opens an empty slot at its top for the card, or
    /// closes it. As it first shows in a run, the browser learns again which agents are ready, in
    /// the background, which `task_agents_changed` reports.
    fn asking(&self, shown: bool, cx: &mut App);
    /// The page's switch went to Ask: as ⌘T's ask does as it first shows in a run, the browser
    /// learns again which agents are ready, in the background, which `agents_changed` then gives
    /// the page's bar.
    fn look_for_agents(&self, cx: &mut App);
    /// The page the user is on, if it's a web page: its address and title, all of it that goes
    /// with an ask, as the agent never gets the user's tab.
    fn task_page(&self, cx: &App) -> Option<(SharedString, SharedString)>;
    /// Starts `asked` as a task on the agent `agent`, with `page`, from the Space the bar shows
    /// over, and hides the bar by folding it into the ask's slot, which its card takes, or where
    /// the window shows no slot, as `hide` does.
    fn run_task(
        &self,
        agent: &str,
        asked: &str,
        page: Option<(SharedString, SharedString)>,
        tier: Tier,
        model: Option<String>,
        cx: &mut App,
    );
    /// As `run_task`, from a resting bar's composer, in the Space it rests in, which shows no
    /// page: there's no window to fold, and the thread's view takes the page's place.
    fn run_task_at_rest(
        &self,
        agent: &str,
        asked: &str,
        tier: Tier,
        model: Option<String>,
        cx: &mut App,
    );
    /// The level and model the composer last started a thread with in the profile of the Space
    /// the bar shows over, Full access never: what its choices open at. Only the composer's own
    /// starts set it; no agent reads or sets it.
    fn composer_last(&self, _: &App) -> (Tier, Option<(SharedString, String)>) {
        (Tier::default(), None)
    }
    /// Starts signing in to the assistant `agent`, as Settings' Sign In does.
    fn sign_in(&self, _agent: &str, _: &mut App) {}
    /// Opens Settings › Agents, where agents are set up.
    fn set_up_agents(&self, cx: &mut App);
    /// Shows `tip` hanging from a button at its anchor in the bar's window, over the bar, after the
    /// tooltip's delay, or ends the tooltip.
    fn tooltip(&self, tip: Option<(Tip, Anchor)>, cx: &mut App);
    /// The tabs open on the account's other computers, most recently seen first, if the bar shows
    /// over a window of the profile that syncs, and none over a private one. It only reads.
    fn other_tabs(&self, _: &App) -> Rc<Vec<OtherTab>> {
        Rc::default()
    }
    /// The newest tab in the archive at `url`, as the Library lists it: its id and when it
    /// closed, in milliseconds since 1970. None over a private window, whose pages the archive
    /// keeps apart. It only reads.
    fn archived(&self, url: &str, cx: &App) -> Option<(u64, f64)>;
    /// Reopens the archived tab `archived` at the top of the Today tabs of the Space shown, with
    /// its history, name and icon, and shows it, as the Library's Reopen does. Whether it did:
    /// not once it has left the archive. The user's: agents get no path to the bar.
    fn restore_archived(&self, archived: u64, cx: &mut App) -> bool;
    /// Now, in milliseconds since 1970, which a row counts the days since its page's visit from.
    fn now(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0., |since| since.as_secs_f64() * 1000.)
    }
}

/// The day `at`, in milliseconds since 1970, as a row's end says it from `now` in the user's time
/// zone: today, yesterday, the weekday for the five days before, then the date, such as 3 Nov, so
/// a weekday is never last week's (#2337). A time after now, from a clock set back, is today.
fn day_said(at: f64, now: f64) -> String {
    use chrono::{Local, TimeZone};
    let local = |ms: f64| Local.timestamp_millis_opt(ms as i64).single();
    let (Some(at), Some(now)) = (local(at), local(now)) else {
        return String::new();
    };
    match (now.date_naive() - at.date_naive()).num_days() {
        ..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=6 => at.format("%A").to_string(),
        _ => at.format("%-d %b").to_string(),
    }
}

/// A search engine's site search, which the bar's searches stay in until the user leaves it, as in
/// Chrome's keyword mode.
struct Site {
    keyword: String,
    name: SharedString,
    /// The word typed before Tab, which leaving puts back in the keyword's place, as Arc's
    /// `queryBeforeEnteringScopedSearch` (#1397): "yout" comes back as "yout", not youtube.com.
    word: String,
    /// The site's own colour, which its chip and the selected row are in, if it has one.
    color: Option<u32>,
}

/// The colours of the site searches a new profile has (core/site_searches.cc), by keyword, for those
/// whose brand has one.
pub(crate) const SITE_COLORS: [(&str, u32); 15] = [
    ("drive.google.com", site_colors::GOOGLE_DRIVE),
    ("mail.google.com", site_colors::GMAIL),
    ("maps.google.com", site_colors::GOOGLE_MAPS),
    ("calendar.google.com", site_colors::GOOGLE_CALENDAR),
    ("docs.google.com", site_colors::GOOGLE_DOCS),
    ("dropbox.com", site_colors::DROPBOX),
    ("amazon.com", site_colors::AMAZON),
    ("figma.com", site_colors::FIGMA),
    ("youtube.com", site_colors::YOUTUBE),
    ("reddit.com", site_colors::REDDIT),
    ("soundcloud.com", site_colors::SOUNDCLOUD),
    ("asana.com", site_colors::ASANA),
    ("linkedin.com", site_colors::LINKEDIN),
    ("music.apple.com", site_colors::APPLE_MUSIC),
    ("spotify.com", site_colors::SPOTIFY),
];

/// Where the bar shows the page chosen.
#[derive(Clone, Copy)]
enum Target {
    NewTab,
    /// The tab whose URL it edits.
    Tab(u64),
    /// A new tab, in a split view with this one, on its right.
    Beside(u64),
    /// None: Search Tabs only switches to one of its tabs.
    Tabs,
    /// None: a Little ilo window's Spaces menu only sends its page to one of its Spaces.
    Spaces,
}

/// Where the bar draws: in a window of its own, which the host shows over the browser window, or
/// resting in a browser window's empty Space, whose page draws it where ⌘T's opens (#4140). Only
/// the window's bar keeps its field's focus, follows its window's activation, and asks the host to
/// show, size, hide or dismiss it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Window,
    InPlace,
}

pub struct CommandBar {
    host: Rc<dyn Host>,
    surface: Surface,
    input: Entity<TextInput>,
    target: Target,
    /// What was typed, which the core was last asked to suggest for, when, and whether it was
    /// allowed to complete it.
    asked: String,
    asked_at: Instant,
    complete: bool,
    /// What the input shows after what was typed, selected, to complete it.
    completion: String,
    /// The input's length and selection start before its last change, which tell a deletion.
    before: (usize, usize),
    /// The core's suggestions, and the text they're for, until the bar asks again.
    rows: Vec<Suggestion>,
    suggested: String,
    /// The row ↩ takes.
    selected: usize,
    /// The row under the pointer, which keeps its plate as the keys move the selection (Arc).
    hovered: Option<usize>,
    /// The row whose trash was last drawn, and its page, for check.sh's line as it draws.
    remove_drawn: Option<(usize, String)>,
    /// Whether an assistive app reads the bar's window, so its rows say what they are.
    a11y: bool,
    /// Whether it shows over the URL field, from a click on it, rather than centred.
    over_field: bool,
    /// The URL it opened on, whose favicon the input shows until it's edited, as Arc's does.
    page: String,
    /// The site search the bar is in, if any.
    site: Option<Site>,
    /// While it lists only commands, as after Tab in an empty field or "/ " typed (Arc), what
    /// leaving puts back before what's typed: nothing after Tab, "/ " after "/ ".
    commands: Option<&'static str>,
    /// What Settings' search reads of its rows, read once since the bar opened, once it's ready.
    settings_index: Option<Rc<Vec<settings_search::Entry>>>,
    /// When the bar was asked to open, until its first frame on screen logs how long it took.
    opened_at: Option<Instant>,
    /// The bar drew since it was asked to open while its window was off screen, as the host has it
    /// draw before putting it on: that frame is its first on screen once the window comes on.
    opened_drawn: bool,
    /// The agents ⇥ asks, as last learned, read as the bar opens and as they change.
    task_agents: Option<TaskAgents>,
    /// While ⇥ makes what's typed a task (Agent task).
    ask: Option<Ask>,
    /// Where the composer's level and model triggers were last drawn, which their menus hang from.
    level_at: Rc<Cell<Option<Bounds<Pixels>>>>,
    model_at: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The trigger whose menu is open, by its id, which wears a plate until the menu closes.
    menu_open: Option<&'static str>,
    /// The rows' favicons by page, asked for as they first show. None until one comes.
    favicons: HashMap<String, Option<Arc<Image>>>,
    /// The Space's tabs it lists before anything is typed.
    tabs: Vec<shell::BarTab>,
    /// The Spaces a Spaces menu lists.
    spaces: Vec<shell::BarSpace>,
    /// The pages shown as it opened, whose rows go last, as Arc's bar deranks the current page.
    shown: Vec<SharedString>,
    scroll: ScrollHandle,
    /// The row whose remove button the bar asked a tooltip for, until the pointer leaves it, and
    /// where that button last drew in the bar's window, which the tooltip hangs from.
    tipped: Option<usize>,
    remove_frame: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Whether it shows: until it hides or asks the host to go, which on Windows reports it gone
    /// only a message later, or the host reports the user left it. The host closes the window at a
    /// click outside it, before the click reaches the shell, so this stays set until that report.
    open: Cell<bool>,
}

/// The bar, and the window of its own it draws in.
#[derive(Clone)]
struct CommandBarWindow {
    window: AnyWindowHandle,
    bar: Entity<CommandBar>,
}

impl Global for CommandBarWindow {}

/// The resting bar that last asked the core, in the window it rests in, which the core's answers
/// go to while the window's bar isn't open. Weak, so a bar its Space has dropped gets none.
struct CommandBarInPlace {
    window: AnyWindowHandle,
    bar: WeakEntity<CommandBar>,
}

impl Global for CommandBarInPlace {}

/// The keys were bound for a resting bar, where no window's bar bound them.
struct InPlaceKeys;

impl Global for InPlaceKeys {}

/// The bar the core's answers go to: the window's while it's open, else the resting bar that last
/// asked, else the window's, as when no bar rests anywhere.
fn answering(cx: &App) -> Option<CommandBarWindow> {
    let window = cx.try_global::<CommandBarWindow>().cloned();
    if window
        .as_ref()
        .is_some_and(|window| window.bar.read(cx).open.get())
    {
        return window;
    }
    let resting = cx.try_global::<CommandBarInPlace>().and_then(|resting| {
        Some(CommandBarWindow {
            window: resting.window,
            bar: resting.bar.upgrade()?,
        })
    });
    resting.or(window)
}

impl CommandBarWindow {
    /// Runs `update` on the bar, with the window it draws in, as a `WindowHandle`'s does.
    fn update<R>(
        &self,
        cx: &mut App,
        update: impl FnOnce(&mut CommandBar, &mut Window, &mut Context<CommandBar>) -> R,
    ) -> Result<R, ()> {
        let bar = self.bar.clone();
        let update = |_: AnyView, window: &mut Window, cx: &mut App| {
            bar.update(cx, |bar, cx| update(bar, window, cx))
        };
        self.window.update(cx, update).map_err(|_| ())
    }
}

/// What the bar's window is called to an assistive app.
pub const WINDOW_NAME: &str = "Command Bar";

/// Opens the command bar's window, hidden, for `host` to show over the browser window. Opening it
/// ahead of time keeps window setup off the path from the shortcut to the bar on screen.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<CommandBar>> {
    let window = ROOM.panel_margin().around(size(px(WIDTH), px(HEIGHT)));
    let bounds = Bounds::new(point(px(0.), px(0.)), window);
    open_at(host, bounds, false, cx)
}

/// Opens the command bar's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<CommandBar>> {
    bind_keys(cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // The preview's has none, so it hasn't the window buttons the host's takes off.
        titlebar: (!show).then_some(TitlebarOptions {
            // A screen reader says the window as the keyboard comes into it and leaves: "Command
            // Bar", as the menu names it, rather than a frame with no name (#3497). The Mac hides
            // a clear title bar's title, and the other hosts draw none.
            title: Some(WINDOW_NAME.into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: show,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The panel's rounded shape is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let bar = new_bar(host, Surface::Window, window, cx);
        window.focus(&bar.read(cx).input.focus_handle(cx), cx);
        bar
    });
    match window {
        Ok(window) => {
            let bar = window.entity(cx).ok()?;
            cx.set_global(CommandBarWindow {
                window: window.into(),
                bar,
            });
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the command bar: {error}");
            None
        }
    }
}

fn bind_keys(cx: &mut App) {
    // Off the Mac ⌃ and ⌥ are both Alt, and Alt+↩ is ⌥↩'s, near Chrome's Alt+Enter, which opens a
    // tab, so ⌃↩ has no key there. Arc's ⌥⌘⌫ would be Ctrl+Alt+Backspace, which X11 can take to end
    // the session, so there ⇧⌦ alone removes a suggestion, as in Chrome.
    if !crate::shell::OFF_THE_MAC {
        cx.bind_keys([
            KeyBinding::new("ctrl-enter", GoDotCom, Some("CommandBar")),
            KeyBinding::new("alt-cmd-backspace", Remove, Some("CommandBar")),
        ]);
    }
    let key = |key, action| crate::shell::view_binding(key, action, "CommandBar", cx);
    let mac_keys = [
        key("alt-enter", Box::new(GoBeside)),
        key("cmd-enter", Box::new(GoBehind)),
        key("ctrl-shift-enter", Box::new(GoDotComWindow)),
        key("shift-cmd-enter", Box::new(Search)),
        key("ctrl-n", Box::new(Next)),
        key("ctrl-p", Box::new(Previous)),
    ];
    cx.bind_keys(mac_keys);
    cx.bind_keys([
        KeyBinding::new("enter", Go, Some("CommandBar")),
        KeyBinding::new("shift-enter", GoWindow, Some("CommandBar")),
        KeyBinding::new("escape", Dismiss, Some("CommandBar")),
        KeyBinding::new("down", Next, Some("CommandBar")),
        KeyBinding::new("up", Previous, Some("CommandBar")),
        KeyBinding::new("tab", EnterSite, Some("CommandBar")),
        KeyBinding::new("shift-tab", LeaveSite, Some("CommandBar")),
        KeyBinding::new("pageup", First, Some("CommandBar")),
        KeyBinding::new("pagedown", Last, Some("CommandBar")),
        KeyBinding::new("shift-delete", Remove, Some("CommandBar")),
    ]);
}

/// A bar to rest in `window`'s empty Space, which its page draws where ⌘T's opens: the same bar,
/// whose ⇥ grows it into the composer in place. It asks nothing of the core or the host until it's
/// typed in, so the Space's first frame reads nothing for it.
pub fn in_place(host: Rc<dyn Host>, window: &mut Window, cx: &mut App) -> Entity<CommandBar> {
    if !cx.has_global::<CommandBarWindow>() && !cx.has_global::<InPlaceKeys>() {
        bind_keys(cx);
        cx.set_global(InPlaceKeys);
    }
    new_bar(host, Surface::InPlace, window, cx)
}

/// The bar, in its own window or at rest in `window`.
fn new_bar(
    host: Rc<dyn Host>,
    surface: Surface,
    window: &mut Window,
    cx: &mut App,
) -> Entity<CommandBar> {
    let input = cx.new(|cx| {
        // Its colours are the bar's to give each time it draws. At rest the field takes the
        // keyboard by itself as the last page leaves, so ⌘Z there is for what the user did.
        TextInput::new(PLACEHOLDER_TEXT, cx)
            .paste_and_go()
            .passes_undo(surface == Surface::InPlace)
    });
    cx.new(|cx| {
        cx.observe_in(&input, window, |bar: &mut CommandBar, _, window, cx| {
            bar.edited(window, cx)
        })
        .detach();
        // In a window of its own, nothing else there takes the field's focus. At rest, the
        // sidebar, the URL field and the overlays over the window do, and keep it.
        if surface == Surface::Window {
            cx.observe_window_activation(window, |bar: &mut CommandBar, window, cx| {
                bar.activation_changed(window, cx)
            })
            .detach();
            text_input::on_leave(&input.focus_handle(cx), window, cx, |bar, window, cx| {
                bar.keep_focus(window, cx)
            })
            .detach();
            // The frame drawn off screen shows as the window comes on, and nothing draws the bar
            // again until something changes in it, such as the first letter typed.
            cx.observe_window_visibility(window, |bar: &mut CommandBar, visible, _, _| {
                if visible.is_visible() && std::mem::take(&mut bar.opened_drawn) {
                    bar.log_first_frame();
                }
            })
            .detach();
        }
        CommandBar {
            host,
            surface,
            input,
            target: Target::NewTab,
            asked: String::new(),
            asked_at: Instant::now(),
            complete: false,
            completion: String::new(),
            before: (0, 0),
            rows: Vec::new(),
            suggested: String::new(),
            selected: 0,
            hovered: None,
            remove_drawn: None,
            a11y: false,
            over_field: false,
            page: String::new(),
            site: None,
            commands: None,
            settings_index: None,
            opened_at: None,
            opened_drawn: false,
            task_agents: None,
            ask: None,
            level_at: Rc::default(),
            model_at: Rc::default(),
            menu_open: None,
            favicons: HashMap::new(),
            tabs: Vec::new(),
            spaces: Vec::new(),
            shown: Vec::new(),
            scroll: ScrollHandle::new(),
            tipped: None,
            remove_frame: Rc::default(),
            // A resting bar shows from the start, until a pick takes the Space out of empty.
            open: Cell::new(surface == Surface::InPlace),
        }
    })
}

/// Shows the command bar with the keyboard: empty for a new tab, listing `tabs`, or on `editing`'s
/// URL, selected. It suggests nothing for the URL until the text is edited, as Chrome's address
/// bar, and lists `tabs` again once the text is empty. It shows at once: over `field`, the URL
/// field's box in the window, when a click on the field opened it, else centred, as Arc's is for
/// ⌘T and ⌘L. Rows for the pages `shown` go last.
pub fn show(
    editing: Option<(u64, SharedString)>,
    shown: Vec<SharedString>,
    tabs: Vec<shell::BarTab>,
    field: Option<Bounds<Pixels>>,
    cx: &mut App,
) {
    let target = editing
        .as_ref()
        .map_or(Target::NewTab, |&(tab, _)| Target::Tab(tab));
    let text = editing.map(|(_, url)| url).unwrap_or_default();
    // check.sh reads these lines.
    let ids: Vec<String> = tabs
        .iter()
        .take(shell::BAR_TABS_SHOWN)
        .map(|tab| tab.tab.to_string())
        .collect();
    log!("browser: command bar tabs {}", ids.join(" "));
    let place = if field.is_some() {
        "over the URL field"
    } else {
        "centred"
    };
    log!("browser: command bar opens {place}");
    let Some(host) = reset(target, text, shown, tabs, field.is_some(), cx) else {
        return;
    };
    match field {
        Some(field) => {
            let top = f32::from(field.top()) - FIELD_ABOVE;
            host.show(
                false,
                field.left().into(),
                top,
                FIELD_WIDTH,
                FIELD_RADIUS,
                FIELD_LEVEL,
            );
        }
        None => host.show(true, 0., ABOVE_MIDDLE, WIDTH, RADIUS, Level::Modal),
    }
}

/// The bar, open for a new tab, takes `asked` as an ask, as if the user had typed it and pressed
/// ⇥: an ended task's Start Again (#4105). It's theirs to send or change; nothing is sent here.
/// With tasks off or no assistant to ask, it stays what's typed.
pub fn ask_again(asked: SharedString, cx: &mut App) {
    let Some(window) = cx.try_global::<CommandBarWindow>().cloned() else {
        return;
    };
    _ = window.update(cx, |bar, window, cx| {
        if !bar.open.get() || !matches!(bar.target, Target::NewTab) {
            return;
        }
        let caret = asked.len();
        bar.input.update(cx, |input, cx| {
            input.set_text(asked.clone(), caret, window, cx)
        });
        if bar.offers_ask(cx) {
            bar.enter_ask(window, cx);
        }
    });
}

/// Shows the command bar at `bar` in the window, with the keyboard, on `editing`'s URL, selected, or
/// empty, listing `tabs`: a Little ilo window's for ⌘L and ⌘T, inside it (Arc).
pub fn show_in(
    editing: Option<(u64, SharedString)>,
    shown: Vec<SharedString>,
    tabs: Vec<shell::BarTab>,
    bar: Bounds<Pixels>,
    cx: &mut App,
) {
    let target = editing
        .as_ref()
        .map_or(Target::NewTab, |&(tab, _)| Target::Tab(tab));
    let text = editing.map(|(_, url)| url).unwrap_or_default();
    // check.sh reads this line.
    log!("browser: command bar opens inside the window");
    if let Some(host) = reset(target, text, shown, tabs, false, cx) {
        let (left, top, width) = (bar.left().into(), bar.top().into(), bar.size.width.into());
        host.show(false, left, top, width, RADIUS, Level::Modal);
    }
}

/// Shows the compact command bar at `bar` in a Little ilo window, its Spaces menu: it lists only
/// `spaces`, filters them by what's typed and sends the window's page to the one ↩ takes (Arc).
pub fn show_spaces(spaces: Vec<shell::BarSpace>, bar: Bounds<Pixels>, cx: &mut App) {
    // check.sh reads this line.
    let ids: Vec<String> = spaces.iter().map(|space| space.space.to_string()).collect();
    log!("browser: command bar lists spaces {}", ids.join(" "));
    let Some(window) = cx.try_global::<CommandBarWindow>().cloned() else {
        return;
    };
    // Before the reset, which lists them as the bar opens.
    window.update(cx, |bar, _, _| bar.spaces = spaces).ok();
    if let Some(host) = reset(
        Target::Spaces,
        SharedString::default(),
        Vec::new(),
        Vec::new(),
        true,
        cx,
    ) {
        let (left, top, width) = (bar.left().into(), bar.top().into(), bar.size.width.into());
        host.show(false, left, top, width, FIELD_RADIUS, FIELD_LEVEL);
    }
}

/// Shows the command bar centred and empty for Search Tabs, listing `tabs` alone, as many as it
/// lists rows, and then those that what's typed names.
pub fn show_tabs(tabs: Vec<shell::BarTab>, cx: &mut App) {
    // check.sh reads these lines.
    let ids: Vec<String> = tabs
        .iter()
        .take(shell::BAR_ROWS)
        .map(|tab| tab.tab.to_string())
        .collect();
    log!("browser: command bar searches tabs {}", ids.join(" "));
    if let Some(host) = reset(
        Target::Tabs,
        SharedString::default(),
        Vec::new(),
        tabs,
        false,
        cx,
    ) {
        host.show(true, 0., ABOVE_MIDDLE, WIDTH, RADIUS, Level::Modal);
    }
}

/// Shows the command bar, empty, over `pane`, a split view's new pane in the card at `card`, for
/// a page to show there beside `with`. It's centred over the pane, as far into the card as it
/// needs to be, and as far down the pane as it opens down the window for a new tab.
pub fn show_split(with: u64, pane: Bounds<Pixels>, card: Bounds<Pixels>, cx: &mut App) {
    let text = SharedString::default();
    if let Some(host) = reset(
        Target::Beside(with),
        text,
        Vec::new(),
        Vec::new(),
        false,
        cx,
    ) {
        let left = pane.center().x - px(WIDTH / 2.);
        let left = left
            .min(card.right() - px(BESIDE_END + WIDTH))
            .max(card.left());
        // The card is as far in from the window's bottom as from its top.
        let window = f32::from(card.top() + card.bottom());
        let top = centered_top(window) + f32::from(pane.top() - card.top());
        host.show(false, left.into(), top, WIDTH, RADIUS, Level::Modal);
    }
}

/// Puts the bar to `target` with `text` in its input, the pages `shown`, and `tabs` to list while
/// it's empty, over the URL field if `over_field`. Returns its host, or none if there's no bar.
fn reset(
    target: Target,
    text: SharedString,
    shown: Vec<SharedString>,
    tabs: Vec<shell::BarTab>,
    over_field: bool,
    cx: &mut App,
) -> Option<Rc<dyn Host>> {
    let window = cx.try_global::<CommandBarWindow>()?.clone();
    window
        .update(cx, |bar, window, cx| {
            bar.target = target;
            bar.over_field = over_field;
            // The pointer has been elsewhere since the bar last showed, and the host ended the
            // tooltip over it as it hid.
            bar.hovered = None;
            bar.tipped = None;
            // Favicons change over time, and a long session shouldn't keep every one it saw. The
            // tabs' are the ones their rows show.
            bar.favicons.clear();
            bar.keep_favicons(&tabs);
            bar.tabs = tabs;
            bar.shown = shown;
            bar.open.set(true);
            bar.page = text.to_string();
            if !text.is_empty() {
                bar.favicons.insert(bar.page.clone(), None);
                bar.host.page_favicon(&bar.page);
            }
            bar.site_left(cx);
            bar.commands = None;
            bar.settings_index = None;
            bar.opened_at = Some(Instant::now());
            bar.opened_drawn = false;
            bar.end_ask(cx);
            // What the browser last learned, read in memory: nothing about tasks runs as it opens.
            bar.task_agents = bar.learned_agents(cx);
            let placeholder = match target {
                Target::Tabs => TABS_PLACEHOLDER,
                Target::Spaces => SPACES_PLACEHOLDER,
                _ => bar.search_placeholder(),
            };
            bar.input.update(cx, |input, cx| {
                input.set_placeholder(placeholder.into(), cx)
            });
            bar.as_opened(window, cx);
            bar.keep_focus(window, cx);
            bar.host.clone()
        })
        .ok()
}

/// The window the bar shows over shows another Space: the bar lists `tabs` from it, or for Search
/// Tabs `open_tabs`, and asks again for what's typed, which the core answers from that Space's
/// profile (decision 5), as Arc's bar does. So a pick never goes to the Space before or its
/// profile: the rows from there go at once, the URL ⌘L opened it on with them, and the tab it
/// edited isn't shown now, so ↩ opens a new tab. A new pane's bar goes, as its split view stays
/// behind. Returns whether its pick opens a new tab.
pub fn space_shown(tabs: Vec<shell::BarTab>, open_tabs: Vec<shell::BarTab>, cx: &mut App) -> bool {
    let Some(bar_window) = cx.try_global::<CommandBarWindow>().cloned() else {
        return false;
    };
    // The handle the commands' list goes through, once out of the shell's update.
    let later = bar_window.clone();
    bar_window
        .update(cx, |bar, window, cx| {
            if !bar.open.get() {
                return false;
            }
            // A Little ilo window shows its own Space alone.
            if bar.lists_spaces() {
                return false;
            }
            if let Target::Beside(_) = bar.target {
                // Its split view isn't shown.
                bar.close_later(Close::new(Why::FocusLeft, By::Code), cx);
                return false;
            }
            // check.sh reads this line.
            log!("browser: command bar follows the Space");
            let tabs = if matches!(bar.target, Target::Tabs) {
                open_tabs
            } else {
                tabs
            };
            bar.favicons.clear();
            bar.keep_favicons(&tabs);
            bar.tabs = tabs;
            // The pages shown were the Space before's.
            bar.shown.clear();
            // The site search's engine is the profile's, which this one may not have.
            bar.leave_site(window, cx);
            // An ask goes on in the Space shown, whose Today opens its slot.
            if bar.ask.is_some() {
                let host = bar.host.clone();
                cx.defer(move |cx| host.asking(true, cx));
                return matches!(bar.target, Target::NewTab);
            }
            bar.retarget(later, window, cx);
            cx.notify();
            matches!(bar.target, Target::NewTab)
        })
        .unwrap_or(false)
}

/// The window the bar shows over shows `tab`, another tab of its Space, as a browser shortcut can
/// while the bar has the keyboard. ⌘L's bar edited the tab before, which isn't shown now, so its
/// pick opens a new tab, as after another Space, and never loads where the user can't see it
/// (#3428). A new pane's bar goes, as its split view isn't shown. Returns whether a bar that
/// edited a tab opens a new tab now.
pub fn tab_shown(tab: u64, cx: &mut App) -> bool {
    let Some(bar_window) = cx.try_global::<CommandBarWindow>().cloned() else {
        return false;
    };
    let later = bar_window.clone();
    bar_window
        .update(cx, |bar, window, cx| {
            if !bar.open.get() {
                return false;
            }
            match bar.target {
                Target::Tab(edited) if edited != tab => {}
                Target::Beside(with) if with != tab => {
                    // Its split view isn't shown.
                    bar.close_later(Close::new(Why::FocusLeft, By::Code), cx);
                    return false;
                }
                _ => return false,
            }
            log!("browser: command bar follows the tab shown");
            if bar.ask.is_some() {
                // An ask goes on, and its task opens where asks do.
                bar.target = Target::NewTab;
                bar.page.clear();
            } else {
                bar.retarget(later, window, cx);
            }
            cx.notify();
            true
        })
        .unwrap_or(false)
}

/// The user left the bar with a click outside it, or Esc.
pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<CommandBarWindow>().cloned() {
        let close = Close::left(cx.active_window().is_none());
        window
            .update(cx, |bar, _, cx| {
                bar.closed(close);
                bar.end_ask(cx);
            })
            .ok();
    }
}

/// What the browser knows of the agents ⇥ asks has changed, as after it learned which are ready:
/// an ask that shows lists them again.
pub fn task_agents_changed(cx: &mut App) {
    let Some(window) = answering(cx) else {
        return;
    };
    window
        .update(cx, |bar, _, cx| {
            if !bar.open.get() {
                return;
            }
            let agents = bar.learned_agents(cx);
            if agents == bar.task_agents {
                return;
            }
            bar.task_agents = agents;
            if bar.ask.is_some() {
                bar.log_agents();
            } else if bar.site.is_none() && bar.commands.is_none() && bar.lists_commands() {
                let placeholder = bar.search_placeholder();
                bar.input.update(cx, |input, cx| {
                    input.set_placeholder(placeholder.into(), cx)
                });
            }
            cx.notify();
        })
        .ok();
}

/// Whether the bar shows, for a click on what opens it to close it rather than open it again: the
/// click outside the bar has closed it by now, but the shell hears so only after the click's press.
pub fn is_open(cx: &App) -> bool {
    cx.try_global::<CommandBarWindow>()
        .is_some_and(|bar| bar.bar.read(cx).open.get())
}

/// The favicon `Host::page_favicon` asked for, for a row going to `url`.
pub fn page_favicon(url: &str, favicon: Option<Arc<Image>>, cx: &mut App) {
    let Some(window) = answering(cx) else {
        return;
    };
    window
        .update(cx, |bar, _, cx| {
            if let Some(entry) = bar.favicons.get_mut(url) {
                *entry = favicon;
                cx.notify();
            }
        })
        .ok();
}

/// A chip before the input: a mode's or a page's, with the page's favicon, in `fill`.
fn mode_chip(
    fill: impl Into<gpui::Fill>,
    label: Hsla,
    favicon: Option<AnyElement>,
    name: SharedString,
) -> gpui::Div {
    div()
        .flex_none()
        .max_w(px(CHIP_MAX))
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(6.))
        .py(px(2.))
        .rounded(px(CHIP_RADIUS))
        .bg(fill)
        .text_color(label)
        .text_size(px(text::BODY))
        .children(favicon)
        .child(div().min_w_0().truncate().child(name))
}

/// Where the centred panel's top is, down a browser window `height` tall.
pub fn centered_top(height: f32) -> f32 {
    height / 2. - ABOVE_MIDDLE
}

/// Where the middle of the centred bar's input row is, down a browser window `height` tall: the
/// height the composer page's field rests at, so ⌘T's bar opens on it.
pub fn centered_row_middle(height: f32) -> f32 {
    centered_top(height) + HEIGHT / 2.
}

/// How tall the bar's window is with `rows` suggestions listed: the input, and under it, with its
/// line, up to five rows.
pub fn height(rows: usize) -> f32 {
    match rows {
        0 => HEIGHT,
        rows => HEIGHT + LINE + LIST_TOP + rows.min(ROWS_SHOWN) as f32 * ROW_HEIGHT + LIST_END,
    }
}

/// How tall it is over the URL field: the input's row, and under it up to five rows.
pub fn field_height(rows: usize) -> f32 {
    match rows {
        0 => FIELD_INPUT,
        rows => FIELD_INPUT + rows.min(ROWS_SHOWN) as f32 * ROW_HEIGHT + LIST_END,
    }
}

/// The core's suggestions for `text`.
pub fn suggested(text: String, rows: Vec<Suggestion>, cx: &mut App) {
    let Some(window) = answering(cx) else {
        return;
    };
    window
        .update(cx, |bar, window, cx| {
            let started = Instant::now();
            // Older text's come after the bar has asked about newer, and Search Tabs asks for none.
            // The commands-only list asks for none either, nor does an ask.
            if text != bar.request()
                || bar.lists_own()
                || bar.commands.is_some()
                || bar.ask.is_some()
            {
                return;
            }
            // check.sh reads these lines.
            let urls: Vec<String> = rows
                .iter()
                .map(|row| {
                    let tab = match row.tab {
                        0 => String::new(),
                        tab => format!(" tab {tab}"),
                    };
                    let site = match row.keyword.as_str() {
                        "" => String::new(),
                        keyword => format!(" site {keyword}"),
                    };
                    let removable = if row.removable { " removable" } else { "" };
                    format!("{}{tab}{site}{removable}", row.url)
                })
                .collect();
            log!(
                "browser: command bar suggestions \"{text}\" in {:.1} ms: {}",
                bar.asked_at.elapsed().as_secs_f64() * 1000.,
                urls.join(", ")
            );
            // An address typed, which ↩ loads where it goes even if it was a tab that closed, as
            // it does if it's an open tab's page.
            let address = (rows.first())
                .filter(|row| row.completion.is_empty() && text.contains(['.', ':', '/']))
                .map(|row| row.url.clone());
            // A site search lists only the search for what's typed, as Arc's does, and nothing
            // typed lists the Space's tabs. The core's first search is it, by its relevance.
            let places = bar.site.is_none() && !text.is_empty();
            let mut rows = match &bar.site {
                Some(site) => rows
                    .into_iter()
                    .find(|row| {
                        bar.host.search_url(Some(&site.keyword), &row.text).as_ref()
                            == Some(&row.url)
                    })
                    .into_iter()
                    .collect(),
                None if text.is_empty() => bar.tab_rows(),
                None => {
                    let mut rows = rows;
                    if bar.settings_index.is_none() {
                        bar.settings_index = bar.host.settings_index(cx);
                    }
                    let (open, mut risen) = bar.take_open_tabs(&text, &mut rows);
                    let others = bar.host.other_tabs(cx);
                    risen.extend(bar.take_other_tabs(&text, &others, &mut rows));
                    let mut rows = with_calculation(&text, bar.with_commands(&text, rows, cx));
                    // The other tabs go right after the core's first row, before its others.
                    let after = rows
                        .iter()
                        .position(|row| row.command.is_none() && row.setting.is_none());
                    let after = after.map_or(rows.len(), |at| at + 1);
                    rows.splice(after..after, risen);
                    rows.splice(0..0, open);
                    let rows = with_new_document(&text, without_repeated_titles(rows));
                    with_shown_last(rows, &bar.shown)
                }
            };
            rows.truncate(shell::BAR_ROWS);
            // The core doesn't say which are searches, so they're the rows that go where a search
            // for their words does.
            let site = bar.site.as_ref().map(|site| site.keyword.as_str());
            for row in &mut rows {
                row.search = !row.named()
                    && bar
                        .host
                        .search_url(site, &row.text)
                        .is_some_and(|url| url == row.url);
            }
            if places {
                bar.place(&mut rows, address.as_deref(), cx);
            }
            bar.set_rows(bar.asked.clone(), rows, window, cx);
            // What the rows cost the frame they come in.
            log!(
                "browser: command bar rows for \"{text}\" in {:.1} ms",
                started.elapsed().as_secs_f64() * 1000.
            );
            // check.sh reads this line: the rows as listed, a page's address with its tab if it's
            // one, a command's name. After the timing, which it doesn't count in.
            let listed: Vec<String> = bar
                .rows
                .iter()
                .map(|row| match (row.named(), row.tab) {
                    (true, _) => row.text.to_string(),
                    (false, 0) => row.url.to_string(),
                    (false, tab) => format!("{} tab {tab}", row.url),
                })
                .collect();
            log!(
                "browser: command bar lists \"{text}\": {}",
                listed.join(", ")
            );
            // ranking.sh reads this line: what a closed tab's or a visited page's row says at its end.
            let said: Vec<String> = (bar.rows.iter())
                .filter(|row| row.tab == 0 && !row.named())
                .filter_map(|row| match row.archived {
                    0 => (!row.visited.is_empty()).then(|| format!("{} {}", row.url, row.visited)),
                    _ => Some(format!("{} {}", row.url, row.closed)),
                })
                .collect();
            if !said.is_empty() {
                log!("browser: command bar says \"{text}\": {}", said.join("; "));
            }
        })
        .ok();
}

/// Hands the bar the clipboard's text as a paste puts it in the input: on one line, without a
/// leading `javascript:`, trimmed. Not if it has none.
fn pasted(
    cx: &mut Context<CommandBar>,
    then: impl FnOnce(&mut CommandBar, &str, &mut Context<CommandBar>) + 'static,
) {
    let bar = cx.entity().downgrade();
    crate::clipboard::read_text(cx, move |text, cx| {
        let Some(text) = text.map(|text| text.replace('\n', " ")) else {
            return;
        };
        let text = without_javascript(&text).trim();
        if !text.is_empty() {
            bar.update(cx, |bar, cx| then(bar, text, cx)).ok();
        }
    });
}

/// Whether a word of `tab`'s title, or of its site's name, starts with `typed`, lowercase. A site's
/// name is its host's labels without the last, the top-level domain, and without a leading www.
/// Whether a word of the Space `name` starts with `typed`, lowercase, as a tab's title is named.
fn names_space(name: &str, typed: &str) -> bool {
    let name = name.to_lowercase();
    let word_start = |&(at, c): &(usize, char)| {
        c.is_alphanumeric()
            && !name[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
    };
    (name.char_indices().filter(word_start)).any(|(at, _)| name[at..].starts_with(typed))
}

fn names_tab(tab: &shell::BarTab, typed: &str) -> bool {
    names_page(&tab.title, &tab.url, typed)
}

/// Whether a word of `title`, or of the name of `url`'s site, starts with `typed`, lowercase.
fn names_page(title: &str, url: &str, typed: &str) -> bool {
    let title = title.to_lowercase();
    let word_start = |&(at, c): &(usize, char)| {
        c.is_alphanumeric()
            && !title[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
    };
    if (title.char_indices().filter(word_start)).any(|(at, _)| title[at..].starts_with(typed)) {
        return true;
    }
    let Some(host) = site_host(url) else {
        return false;
    };
    let labels: Vec<&str> = host.split('.').collect();
    let named = &labels[..labels.len().saturating_sub(1).max(1)];
    named
        .iter()
        .flat_map(|label| label.split('-'))
        .any(|word| word.starts_with(typed))
}

/// The host of `url`, lowercase and without a leading www. None but for a web page: a data:
/// page's scheme isn't its site.
fn site_host(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default().to_lowercase();
    Some(
        host.strip_prefix("www.")
            .map(str::to_string)
            .unwrap_or(host),
    )
}

/// The most open tabs that go above the core's rows for what's typed, the first among them, so
/// some of the profile's pages still show among the five rows (ours).
const TABS_RISEN: usize = 3;

/// Web apps that give many pages one title, so their history would fill the list with rows that
/// read alike. Arc's bar keeps one row per title on its list for them, which names these
/// (`dedupByTitleAndHostList`); the rest of its list isn't known, and notion.com is ours.
const ONE_TITLE_SITES: [&str; 5] = [
    "figma.com",
    "notion.so",
    "notion.com",
    "github.com",
    "zoom.us",
];

/// `rows` without the page rows, but the first, where ↩ goes, whose title an open tab's row or
/// an earlier row has on the same site of ONE_TITLE_SITES. An open tab's row always stays.
fn without_repeated_titles(rows: Vec<Suggestion>) -> Vec<Suggestion> {
    let one_title = |row: &Suggestion| {
        let host = site_host(&row.url)?;
        let site = ONE_TITLE_SITES.iter().find(|site| {
            host == **site
                || host
                    .strip_suffix(**site)
                    .is_some_and(|sub| sub.ends_with('.'))
        })?;
        Some((*site, row.text.clone()))
    };
    let page = |row: &Suggestion| !row.named();
    let mut seen: Vec<_> = rows
        .iter()
        .filter(|row| row.tab != 0 && page(row))
        .filter_map(one_title)
        .collect();
    let mut kept = Vec::with_capacity(rows.len());
    for (at, row) in rows.into_iter().enumerate() {
        match one_title(&row).filter(|_| row.tab == 0 && page(&row)) {
            Some(title) if at > 0 && seen.contains(&title) => {}
            Some(title) => {
                seen.push(title);
                kept.push(row);
            }
            None => kept.push(row),
        }
    }
    kept
}

/// `rows` with those for the pages `shown`, but the first, where ↩ goes, last, as Arc's bar
/// deranks the current page: going there would load it again. Another tab's row keeps its place,
/// since it switches rather than loads.
fn with_shown_last(rows: Vec<Suggestion>, shown: &[SharedString]) -> Vec<Suggestion> {
    let mut at = 0;
    let (mut rows, last): (Vec<_>, Vec<_>) = rows.into_iter().partition(|row| {
        at += 1;
        let page = !row.named() && row.tab == 0;
        at == 1 || !page || !shown.iter().any(|shown| **shown == row.url)
    });
    rows.extend(last);
    rows
}

/// Puts the row for the .new address `text` names, if it does, in place of the rows going there:
/// first, unless the first completes what's typed, which keeps its place.
fn with_new_document(text: &str, mut rows: Vec<Suggestion>) -> Vec<Suggestion> {
    let Some((address, name)) = crate::notes::typed_new(text) else {
        return rows;
    };
    let url = format!("https://{address}/");
    // check.sh reads this line.
    log!("browser: command bar new document {address}: {name}");
    rows.retain(|row| row.url != url || row.tab != 0);
    let at = usize::from(rows.first().is_some_and(|row| !row.completion.is_empty()));
    let row = Suggestion {
        url,
        text: name.into(),
        detail: address.into(),
        new_document: true,
        ..Suggestion::default()
    };
    rows.insert(at, row);
    rows
}

/// Puts what `text` comes to, if it's a sum, after the first of `rows`, as Dia's bar does.
fn with_calculation(text: &str, mut rows: Vec<Suggestion>) -> Vec<Suggestion> {
    if let Some(result) = calculator::calculate(text) {
        // check.sh reads this line.
        log!("browser: command bar calculates \"{text}\": {result}");
        let row = Suggestion {
            text: format!("= {result}").into(),
            result: Some(result.into()),
            ..Suggestion::default()
        };
        rows.insert(rows.len().min(1), row);
    }
    rows
}

impl CommandBar {
    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input.read(cx);
        let (text, selection) = (input.text().to_string(), input.selection());
        let (length, start) = std::mem::replace(&mut self.before, (text.len(), selection.start));
        if input.composing() {
            // The input method's text took the completion's place, if it was selected.
            self.completion.clear();
            return;
        }
        // A resting bar reads nothing of tasks for the page's first frame, only once typed in.
        if self.surface == Surface::InPlace && self.task_agents.is_none() {
            self.task_agents = self.learned_agents(cx);
        }
        let shown = format!("{}{}", self.asked, self.completion);
        // Moving the selection off the completion takes it as typed, as in Chrome.
        if text == shown
            && (self.completion.is_empty() || selection == (self.asked.len()..text.len()))
        {
            return;
        }
        if matches!(self.target, Target::Tabs) {
            return self.name_tabs(text, window, cx);
        }
        if self.lists_spaces() {
            return self.name_spaces(text, window, cx);
        }
        if self.commands.is_some() {
            return self.list_commands(text, window, cx);
        }
        // What an ask types is the task, which nothing suggests for.
        if self.ask.is_some() {
            self.asked = text;
            return cx.notify();
        }
        // Chrome completes only typing at the end: not a deletion, a paste, or with the caret
        // elsewhere, and not in a site search.
        let deleted = text.len() < length && selection.start <= start;
        let typing = !deleted && !input.pasted() && selection == (text.len()..text.len());
        let complete = typing && self.site.is_none();
        // "/ " typed lists only the commands, as Arc's bar does.
        let slashed =
            typing && self.asked == "/" && text.strip_suffix([' ', '\u{3000}']) == Some("/");
        if slashed && self.site.is_none() && self.lists_commands() {
            return self.enter_commands("/ ", window, cx);
        }
        // While what's typed follows the completion, the rest of it stays until the core answers,
        // so it doesn't flicker.
        let completion = shown
            .get(text.len()..)
            .filter(|_| complete)
            .filter(|_| {
                shown
                    .get(..text.len())
                    .is_some_and(|typed| typed.eq_ignore_ascii_case(&text))
            })
            .unwrap_or_default()
            .to_string();
        self.asked = text;
        self.asked_at = Instant::now();
        self.complete = complete;
        // Rows for the same text asked before are old too, as after ⌫ takes a completion back.
        self.suggested.clear();
        self.completion.clear();
        self.show_completion(completion, window, cx);
        self.answers_here(window, cx);
        let site = self.site.as_ref().map(|site| site.keyword.as_str());
        self.host.suggest(site, &self.asked, complete);
    }

    /// A resting bar is about to ask the core: its answers come to it from now on, while the
    /// window's bar isn't open.
    fn answers_here(&self, window: &Window, cx: &mut Context<Self>) {
        if self.surface == Surface::InPlace {
            cx.set_global(CommandBarInPlace {
                window: window.window_handle(),
                bar: cx.entity().downgrade(),
            });
        }
    }

    /// Lists the tabs a word of whose title or site starts with `text`, in their order, or with
    /// nothing typed all of them, as many as the bar lists rows.
    fn name_tabs(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let typed = text.trim().to_lowercase();
        let rows: Vec<Suggestion> = (self.tabs.iter())
            .filter(|tab| typed.is_empty() || names_tab(tab, &typed))
            .take(shell::BAR_ROWS)
            .cloned()
            .map(Suggestion::from)
            .collect();
        // check.sh reads this line.
        let ids: Vec<String> = rows.iter().map(|row| row.tab.to_string()).collect();
        log!(
            "browser: command bar finds tabs \"{typed}\": {}",
            ids.join(" ")
        );
        self.asked = text.clone();
        self.set_rows(text, rows, window, cx);
    }

    /// Keeps the favicons `tabs` have, for their rows.
    fn keep_favicons(&mut self, tabs: &[shell::BarTab]) {
        for tab in tabs {
            if let Some(favicon) = &tab.favicon {
                self.favicons
                    .insert(tab.url.to_string(), Some(favicon.clone()));
            }
        }
    }

    /// Whether the bar lists commands: not over a new pane, for Search Tabs or a Spaces menu.
    fn lists_commands(&self) -> bool {
        matches!(self.target, Target::NewTab | Target::Tab(_))
    }

    /// Whether the bar lists only rows of its own, which nothing suggests for and ↩ alone takes:
    /// Search Tabs' tabs or a Spaces menu's Spaces.
    fn lists_own(&self) -> bool {
        matches!(self.target, Target::Tabs | Target::Spaces)
    }

    /// The tab it edited isn't shown now, so its pick opens a new tab, with what's typed kept and
    /// the URL it opened on gone, and it asks again for the text.
    fn retarget(&mut self, later: CommandBarWindow, window: &mut Window, cx: &mut Context<Self>) {
        let mut text = self.input.read(cx).text().to_string();
        if let Target::Tab(_) = self.target {
            self.target = Target::NewTab;
            if text == self.page {
                text.clear();
            }
            self.page.clear();
        }
        if self.commands.is_some() {
            // Which commands do something now is the shell's to say, once out of its update.
            cx.defer(move |cx| {
                later
                    .update(cx, |bar, window, cx| bar.list_commands(text, window, cx))
                    .ok();
            });
        } else if matches!(self.target, Target::Tabs) {
            self.name_tabs(text, window, cx);
        } else {
            // A completion from the profile before goes with its rows.
            let typed = self.typed(cx);
            let text = if self.completion.is_empty() {
                text
            } else {
                typed
            };
            let caret = text.len();
            let shown = text.clone().into();
            self.input
                .update(cx, |input, cx| input.set_text(shown, caret, window, cx));
            self.ask_for(text, window, cx);
        }
    }

    fn lists_spaces(&self) -> bool {
        matches!(self.target, Target::Spaces)
    }

    /// The Spaces menu's rows: its Spaces whose names have a word starting with `typed`, in the
    /// sidebar's order, each sending the page to its Space.
    fn space_rows(&self, typed: &str) -> Vec<Suggestion> {
        (self.spaces.iter())
            .filter(|space| typed.is_empty() || names_space(&space.name, typed))
            .map(|space| Suggestion {
                text: space.name.clone(),
                command: Some(shell::Command::MoveToSpace(space.space)),
                swatch: Some(space.swatch),
                ..Suggestion::default()
            })
            .collect()
    }

    /// Lists the Spaces what's typed names.
    fn name_spaces(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let typed = text.trim().to_lowercase();
        let rows = self.space_rows(&typed);
        let names: Vec<&str> = rows.iter().map(|row| row.text.as_ref()).collect();
        log!(
            "browser: command bar finds spaces \"{typed}\": {}",
            names.join(", ")
        );
        self.asked = text.clone();
        self.set_rows(text, rows, window, cx);
    }

    /// Lists only the commands, entered by Tab in an empty field or by `by`, "/ ", typed, as
    /// Arc's bar does after Tab right after ⌘T.
    fn enter_commands(&mut self, by: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!(
            "browser: command bar lists commands by {}",
            if by.is_empty() { "tab" } else { "slash" }
        );
        self.commands = Some(by);
        self.completion.clear();
        let placeholder = COMMANDS_PLACEHOLDER.into();
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        self.input.update(cx, |input, cx| {
            input.set_text(SharedString::default(), 0, window, cx)
        });
        self.list_commands(String::new(), window, cx);
    }

    /// Lists the commands that match `text`, or with nothing typed every one that does something
    /// now.
    fn list_commands(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let rows: Vec<Suggestion> = self
            .host
            .commands(&text, cx)
            .into_iter()
            .map(Suggestion::from)
            .collect();
        // check.sh reads this line.
        let names: Vec<&str> = rows.iter().map(|row| row.text.as_ref()).collect();
        log!(
            "browser: command bar commands only \"{text}\": {}",
            names.join(", ")
        );
        self.asked = text.clone();
        self.set_rows(text, rows, window, cx);
    }

    /// Leaves the commands-only list, if it shows, putting back what entered it before what was
    /// typed, with the caret after it, as the site search puts its keyword back. Returns whether
    /// it showed.
    fn leave_commands(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(by) = self.commands.take() else {
            return false;
        };
        // check.sh reads this line.
        log!("browser: command bar leaves commands");
        let placeholder = self.search_placeholder();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder.into(), cx)
        });
        let text = format!("{by}{}", self.input.read(cx).text());
        self.input.update(cx, |input, cx| {
            input.set_text(text.clone().into(), by.len(), window, cx)
        });
        self.ask_for(text, window, cx);
        true
    }

    /// Lists the rows for `text` in the input from the start: the Space's tabs for none, else the
    /// core's suggestions once they come, and none until then.
    fn ask_for(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_asked(text, window, cx);
        if !self.asked.is_empty() {
            self.answers_here(window, cx);
            let site = self.site.as_ref().map(|site| site.keyword.as_str());
            self.host.suggest(site, &self.asked, false);
        }
    }

    /// Takes `text` as what's typed, the rows the tabs with nothing typed and none otherwise,
    /// asking the core nothing for it.
    fn keep_asked(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.asked = text;
        self.asked_at = Instant::now();
        self.complete = false;
        self.completion.clear();
        let rows = if self.asked.is_empty() {
            self.tab_rows()
        } else {
            Vec::new()
        };
        self.set_rows(String::new(), rows, window, cx);
    }

    /// What the core was last asked for, as its suggestions name it: what was typed, after the
    /// site search's keyword and a space.
    fn request(&self) -> String {
        match &self.site {
            Some(site) => format!("{} {}", site.keyword, self.asked),
            None => self.asked.clone(),
        }
    }

    /// Enters the site search of the engine whose keyword is `keyword`, by Tab, with `query` in
    /// the input. A space after the keyword is only a space, as in Arc, where it once entered too,
    /// as in Chrome, and took a search like "github issues" from a plain one (#1397).
    fn enter_site(
        &mut self,
        keyword: String,
        name: SharedString,
        word: String,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // check.sh reads this line.
        log!("browser: command bar site search {keyword} by tab");
        // VoiceOver hears the site in the field's name, as the chip shows it.
        let spoken = format!("Search {name}").into();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(SITE_PLACEHOLDER.into(), cx);
            input.set_spoken(Some(spoken), cx);
        });
        let color = SITE_COLORS
            .iter()
            .find(|(site, _)| *site == keyword)
            .map(|&(_, color)| color);
        self.site = Some(Site {
            keyword,
            name,
            word,
            color,
        });
        self.completion.clear();
        let caret = query.len();
        self.input.update(cx, |input, cx| {
            input.set_text(query.into(), caret, window, cx)
        });
        cx.notify();
    }

    /// Leaves the site search, if the bar is in one, putting the word typed before Tab back before
    /// what's typed now, with the caret after it, as Arc does: left untouched, the field reads as
    /// it did before Tab. Returns whether it was in one.
    fn leave_site(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(site) = self.site_left(cx) else {
            return false;
        };
        // check.sh reads this line.
        log!("browser: command bar leaves site search");
        let typed = self.input.read(cx).text().to_string();
        let space = if typed.is_empty() { "" } else { " " };
        let prefix = format!("{}{space}", site.word);
        let text = format!("{prefix}{typed}");
        self.input.update(cx, |input, cx| {
            input.set_text(text.into(), prefix.len(), window, cx)
        });
        cx.notify();
        true
    }

    /// Puts the field and rows back as the bar opened: the URL it edits, selected, or nothing typed
    /// and the Space's tabs.
    fn as_opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.page.clone();
        self.asked = text.clone();
        // Its answer, for nothing typed, is timed from here, not from the last ask (#1072).
        self.asked_at = Instant::now();
        self.complete = false;
        self.completion.clear();
        self.before = (text.len(), 0);
        // The tabs show at once, in the frame the bar opens with.
        let rows = if text.is_empty() {
            self.tab_rows()
        } else {
            Vec::new()
        };
        self.set_rows(String::new(), rows, window, cx);
        self.input
            .update(cx, |input, cx| input.reset(text.into(), window, cx));
        // The last query's suggestions are of no use now.
        self.host.suggest(None, "", false);
    }

    /// Takes the bar out of its site search, if it's in one, and returns it.
    fn site_left(&mut self, cx: &mut Context<Self>) -> Option<Site> {
        let site = self.site.take()?;
        let placeholder = self.search_placeholder();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder.into(), cx);
            input.set_spoken(None, cx);
        });
        Some(site)
    }

    /// Tab: enters the site search the selected row hints, keeping what was typed after its first
    /// word, the keyword, as Chrome does. Else, with tasks on, it makes what's typed a task for an
    /// agent (Agent task), and else it selects the next row.
    /// In an empty field, it lists only the commands, as Arc's does.
    fn enter_site_or_next(&mut self, _: &EnterSite, window: &mut Window, cx: &mut Context<Self>) {
        // In the composer ⇥ goes back to the search, with what's typed.
        if self.leave_ask(window, cx) {
            return;
        }
        let empty = self.input.read(cx).text().is_empty();
        if empty && self.site.is_none() && self.commands.is_none() && self.lists_commands() {
            return self.enter_commands("", window, cx);
        }
        let hint = self
            .chosen()
            .filter(|row| !row.keyword.is_empty() && self.site.is_none());
        if let Some((keyword, name)) = hint.map(|row| (row.keyword.clone(), row.site.clone())) {
            let typed = self.typed(cx);
            let (word, query) = typed
                .split_once(char::is_whitespace)
                .map_or((typed.as_str(), ""), |(word, rest)| {
                    (word, rest.trim_start())
                });
            let (word, query) = (word.to_string(), query.to_string());
            return self.enter_site(keyword, name, word, query, window, cx);
        }
        if self.may_ask(cx) {
            return self.enter_ask(window, cx);
        }
        self.select_next(true, window, cx);
    }

    /// ⇧Tab: leaves the site search, the commands-only list or the ask, else selects the row
    /// before.
    fn leave_site_or_previous(
        &mut self,
        _: &LeaveSite,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.leave_site(window, cx)
            && !self.leave_commands(window, cx)
            && !self.leave_ask(window, cx)
        {
            self.select_next(false, window, cx);
        }
    }

    /// ⌫, ⌘⌫ or ⌥⌫ with the caret at the start of a site search's input, or the commands-only
    /// list's, leaves it. In an ask it takes the page's chip off first, then leaves.
    fn leave_at_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).selection() != (0..0) {
            return;
        }
        if let Some(ask) = self.ask.as_mut().filter(|ask| ask.page.is_some()) {
            log!("browser: command bar ask without its page");
            ask.page = None;
            cx.notify();
            return cx.stop_propagation();
        }
        if self.leave_site(window, cx)
            || self.leave_commands(window, cx)
            || self.leave_ask(window, cx)
        {
            cx.stop_propagation();
        }
    }

    /// The agents ⇥ asks, as the browser last learned, or None, as while tasks are off, over a
    /// private window, so nothing of tasks shows there.
    fn learned_agents(&self, cx: &App) -> Option<TaskAgents> {
        (!self.host.over_private())
            .then(|| self.host.task_agents(cx))
            .flatten()
    }

    /// Whether ⇥ makes what's typed a task: tasks are on, and the bar opens a page, outside a site
    /// search and the commands-only list.
    fn may_ask(&self, cx: &App) -> bool {
        self.task_agents.is_some()
            && self.lists_commands()
            && self.site.is_none()
            && self.commands.is_none()
            && self.ask.is_none()
            && !self.typed(cx).is_empty()
    }

    /// Whether the input's end says Ask ⇥: ⇥ makes a task, and an agent is ready, or the browser
    /// hasn't learned yet. With none ready, ⇥ offers to set one up, and nothing else of tasks shows.
    fn offers_ask(&self, cx: &App) -> bool {
        self.agents_ready() && self.may_ask(cx)
    }

    /// Whether an agent is ready for a thread, or the browser hasn't learned yet.
    fn agents_ready(&self) -> bool {
        (self.task_agents.as_ref()).is_some_and(|agents| !agents.known || !agents.agents.is_empty())
    }

    /// What the field says with nothing typed, outside a site search and the commands-only list:
    /// Z2 where the bar can start a thread, as New Thread is offered; else E1, as with tasks off or
    /// no assistant set up.
    fn search_placeholder(&self) -> &'static str {
        match self.surface == Surface::Window && self.lists_commands() && self.agents_ready() {
            true => THREAD_PLACEHOLDER,
            false => PLACEHOLDER_TEXT,
        }
    }

    /// ⇥: what's typed becomes a thread's, with the page the user is on as a chip before it, and
    /// the rows give way to the composer's choices, which open as it last started a thread in the
    /// profile.
    fn enter_ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: command bar asks an agent");
        let typed = self.typed(cx);
        // At rest the Space shows no page to ask over.
        let page = match self.surface {
            Surface::Window => self.host.task_page(cx),
            Surface::InPlace => None,
        };
        if let Some((url, _)) = &page
            && !self.favicons.contains_key(url.as_ref())
        {
            self.favicons.insert(url.to_string(), None);
            self.host.page_favicon(url);
        }
        let (tier, model) = self.host.composer_last(cx);
        self.ask = Some(Ask { page, tier, model });
        self.completion.clear();
        self.asked = typed.clone();
        let caret = typed.len();
        self.input.update(cx, |input, cx| {
            input.set_text(typed.into(), caret, window, cx)
        });
        self.ask_shown(true, cx);
        self.log_agents();
        self.set_rows(String::new(), Vec::new(), window, cx);
        self.suggested = self.asked.clone();
    }

    /// Says which assistants the composer may start a thread on, ready or not.
    fn log_agents(&self) {
        let agents = self.task_agents.clone().unwrap_or_default();
        let names: Vec<&str> = agents
            .agents
            .iter()
            .map(|agent| agent.name.as_ref())
            .collect();
        // check.sh reads this line.
        log!("browser: command bar agents: {}", names.join(", "));
    }

    /// The assistant the composer starts its thread on: the one picked on its model menu while
    /// it's signed in and not at its limit, else the first that is.
    fn composer_assistant(&self) -> Option<&TaskAgent> {
        let agents = self.task_agents.as_ref()?;
        let picked = (self.ask.as_ref()).and_then(|ask| ask.model.as_ref().map(|(id, _)| id));
        let mut ready = agents.agents.iter().filter(|agent| agent.resets.is_none());
        let first = ready.clone().next();
        ready.find(|agent| Some(&agent.id) == picked).or(first)
    }

    /// The model the composer's thread starts on, by id, empty for its assistant's own default:
    /// the pick on the model menu, or Settings'.
    fn composer_model(&self) -> Option<String> {
        let agent = self.composer_assistant()?;
        let picked = self.ask.as_ref().and_then(|ask| ask.model.as_ref());
        Some(next_model(picked, agent))
    }

    /// Leaves the ask, if the bar is in one: what was typed stays, and nothing is asked for it
    /// until the user's next edit, so words written for an assistant don't go to the search
    /// provider for going back. Returns whether it was in one.
    fn leave_ask(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.end_ask(cx) {
            return false;
        }
        log!("browser: command bar leaves the ask");
        let text = self.input.read(cx).text().to_string();
        self.keep_asked(text, window, cx);
        true
    }

    /// Ends the ask, if there is one, and closes Today's slot. Returns whether there was one.
    fn end_ask(&mut self, cx: &mut App) -> bool {
        if self.ask.take().is_none() {
            return false;
        }
        self.ask_shown(false, cx);
        true
    }

    /// The composer shows, or no longer does. Over a page, Today opens its slot for the card the
    /// bar folds into; at rest the thread's view takes the page's place instead, so only the
    /// agents are learned again.
    fn ask_shown(&self, shown: bool, cx: &mut App) {
        match self.surface {
            Surface::Window => self.host.asking(shown, cx),
            Surface::InPlace if shown => self.host.look_for_agents(cx),
            Surface::InPlace => {}
        }
    }

    /// ↩ or Start in the composer: closes the bar, then starts the thread on its assistant, at
    /// the level and on the model its menus hold. Nothing with no words or no assistant ready.
    fn run_ask(&mut self, by: By, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.ask.as_ref() else { return };
        let (page, tier) = (ask.page.clone(), ask.tier);
        let asked = self.input.read(cx).text().trim().to_string();
        let (Some(agent), Some(model)) = (self.composer_assistant(), self.composer_model()) else {
            return;
        };
        if asked.is_empty() {
            return;
        }
        let id = agent.id.clone();
        // Settings' model unless the user picked one for this assistant, here or last time.
        let picked = ask_model_on(self.ask.as_ref(), &id);
        let model = picked.then_some(model);
        // check.sh reads this line.
        log!("browser: command bar runs a task on {id}");
        // The host folds the bar into the slot its card fills, so the slot doesn't close first,
        // and the bar doesn't hide on its own.
        self.ask = None;
        self.closed(Close::new(Why::Chosen, by));
        if self.surface == Surface::Window {
            return self.host.run_task(&id, &asked, page, tier, model, cx);
        }
        // At rest the bar stays, empty for the next, as the thread's view takes the page's place.
        self.asked.clear();
        self.input
            .update(cx, |input, cx| input.set_text("".into(), 0, window, cx));
        self.set_rows(String::new(), Vec::new(), window, cx);
        self.host.run_task_at_rest(&id, &asked, tier, model, cx);
    }

    /// The level trigger's menu, under it: the three levels, the one the thread starts at
    /// checked, each with its line, and the line every level holds to. A pick holds for this
    /// thread and is offered next time; Full access is never offered again by itself.
    fn level_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = &self.ask else { return };
        let current = ask.tier;
        let mut items: Vec<MenuEntry> = (Tier::ALL.into_iter())
            .map(|tier| MenuEntry {
                checked: tier == current,
                ..MenuEntry::named(tier.label(), true)
            })
            .collect();
        items.push(MenuEntry::line());
        items.push(MenuEntry::named(NEVER_GETS, false));
        // The pick goes to the bar whose trigger showed the menu.
        let bar = cx.entity().downgrade();
        let chosen = Box::new(
            move |closed: crate::menu::Closed, _: &mut Window, cx: &mut App| {
                let tier = closed
                    .chosen()
                    .and_then(|item| Tier::ALL.get(item).copied());
                let closed = |bar: &mut Self, cx: &mut Context<Self>| {
                    bar.menu_closed(cx);
                    tier.map(|tier| bar.pick_tier(tier, cx));
                };
                bar.update(cx, closed).ok();
            },
        );
        self.show_choices(LEVEL, items, chosen, window, cx);
    }

    /// The user's pick on the composer's level menu.
    fn pick_tier(&mut self, tier: Tier, cx: &mut Context<Self>) {
        let Some(ask) = &mut self.ask else { return };
        ask.tier = tier;
        log!("browser: composer level {}", tier.label());
        cx.notify();
    }

    /// The model trigger's menu, under it: each assistant by its own name, then the models it
    /// offers, its default first and the one the thread starts on checked; one at its limit says
    /// when it resets, and one not signed in has Sign In, which starts its own sign-in as
    /// Settings' does; then Settings › Agents. Over them all a field narrows them as the user
    /// types, leaving Sign In and Settings whatever is typed; a pick is still one of these rows.
    fn model_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agents = self.task_agents.clone().unwrap_or_default();
        let (on, chosen) = match self.composer_assistant() {
            Some(agent) => (Some(agent.id.clone()), self.composer_model()),
            None => (None, None),
        };
        let mut items = vec![MenuEntry::search(SEARCH_MODELS)];
        let mut picks: Vec<ModelPick> = vec![ModelPick::Nothing];
        for agent in &agents.agents {
            items.push(heading(agent.name.clone()));
            picks.push(ModelPick::Nothing);
            let at_limit = agent
                .resets
                .clone()
                .map(|resets| format!("Limit resets {resets}"));
            let checked = chosen.as_deref().filter(|_| on.as_ref() == Some(&agent.id));
            let default = agent.default_model.as_deref();
            let (ids, entries) = crate::agents::task::model_menu(
                checked.unwrap_or("\0"),
                &agent.default_row,
                default,
                agent.models.clone(),
            );
            for ((id, _), entry) in ids.into_iter().zip(entries) {
                let entry = MenuEntry {
                    enabled: at_limit.is_none(),
                    detail: at_limit.clone().map(Into::into).or(entry.detail.clone()),
                    over: false,
                    ..entry
                };
                items.push(entry);
                picks.push(ModelPick::Model(agent.id.clone(), id));
            }
        }
        for (id, name) in &agents.signed_out {
            items.push(heading(name.clone()));
            picks.push(ModelPick::Nothing);
            items.push(MenuEntry::named(SIGN_IN, true).kept());
            picks.push(ModelPick::SignIn(id.clone()));
        }
        items.push(MenuEntry::line());
        picks.push(ModelPick::Nothing);
        items.push(MenuEntry::named(SETTINGS, true).kept());
        picks.push(ModelPick::Settings);
        let bar = cx.entity().downgrade();
        let chosen = Box::new(
            move |closed: crate::menu::Closed, _: &mut Window, cx: &mut App| {
                let pick = closed.chosen().and_then(|item| picks.get(item).cloned());
                let closed = |bar: &mut Self, cx: &mut Context<Self>| {
                    bar.menu_closed(cx);
                    pick.map(|pick| bar.pick_model(pick, cx));
                };
                bar.update(cx, closed).ok();
            },
        );
        self.show_choices(MODEL, items, chosen, window, cx);
    }

    /// The user's pick on the composer's model menu.
    fn pick_model(&mut self, pick: ModelPick, cx: &mut Context<Self>) {
        match pick {
            ModelPick::Model(agent, id) => {
                let Some(ask) = &mut self.ask else { return };
                log!("browser: composer model picked");
                ask.model = Some((agent, id));
                cx.notify();
            }
            ModelPick::SignIn(agent) => self.host.sign_in(&agent, cx),
            ModelPick::Settings => {
                self.end_ask(cx);
                self.close(Close::new(Why::Chosen, By::Pointer));
                self.host.set_up_agents(cx);
            }
            ModelPick::Nothing => {}
        }
    }

    /// Shows a menu of the composer's choices hanging from the trigger `id`, 6 under it, its
    /// left edge at the trigger's; the trigger wears its plate while it's open.
    fn show_choices(
        &mut self,
        id: &'static str,
        items: Vec<MenuEntry>,
        chosen: text_input::Chosen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let trigger = match id {
            LEVEL => self.level_at.get(),
            _ => self.model_at.get(),
        };
        let (Some(trigger), Some(menu)) = (trigger, cx.try_global::<crate::menu::EntriesMenu>())
        else {
            return;
        };
        let menu = menu.0.clone();
        self.menu_open = Some(id);
        cx.notify();
        let at = point(trigger.left(), trigger.bottom() + px(CHOICES_MENU_DROP));
        menu(items, at, false, window, chosen, cx);
    }

    /// A composer's menu closing, however it closed: its trigger's plate goes.
    fn menu_closed(&mut self, cx: &mut Context<Self>) {
        self.menu_open = None;
        cx.notify();
    }

    /// Where ↩ goes for `text` before its suggestions come: the page it names, else a search for
    /// it, or in a site search, a search with that site's engine.
    /// None for Search Tabs, which goes nowhere but its tabs, the commands-only list, which runs
    /// only its commands, and an ask, which goes only to an agent.
    fn typed_url(&self, text: &str) -> Option<String> {
        match &self.site {
            _ if self.lists_own() || self.commands.is_some() => None,
            _ if self.ask.is_some() => None,
            Some(site) => self.host.search_url(Some(&site.keyword), text),
            None => self.host.typed_url(text),
        }
    }

    /// Shows `completion` after what was typed, selected, or no completion.
    fn show_completion(&mut self, completion: String, window: &mut Window, cx: &mut Context<Self>) {
        if completion == self.completion {
            return;
        }
        self.completion = completion;
        // check.sh reads this line.
        log!(
            "browser: command bar completes \"{}\" with \"{}\"",
            self.asked,
            self.completion
        );
        let text = format!("{}{}", self.asked, self.completion);
        let typed = self.asked.len();
        self.input.update(cx, |input, cx| {
            input.complete(text.into(), typed, window, cx)
        });
    }

    /// Completes what was typed to the first row while it's selected, unless the user has moved
    /// the caret.
    fn update_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input.read(cx);
        let (typed, end) = (self.asked.len(), self.asked.len() + self.completion.len());
        let selection = if self.completion.is_empty() {
            end..end
        } else {
            typed..end
        };
        if input.composing()
            || input.text() != format!("{}{}", self.asked, self.completion)
            || input.selection() != selection
        {
            return;
        }
        let completion = match self.selected {
            // The core's suggestions for the same text asked without completing it may come.
            _ if !self.complete => String::new(),
            // Rows for older text keep the completion shown, which came from their first.
            0 if self.suggested != self.asked => self.completion.clone(),
            0 => self
                .rows
                .first()
                .map(|row| row.completion.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.show_completion(completion, window, cx);
    }

    /// The settings `text` names, as "Peek — Settings", from three letters, as Settings' search
    /// finds them by their names, other names and keys, at most SETTINGS_SHOWN. Shortcuts' rows
    /// stay out, as the bar lists each command itself with its keys.
    fn setting_rows(&self, text: &str) -> Vec<Suggestion> {
        let Some(index) = &self.settings_index else {
            return Vec::new();
        };
        if text.trim().chars().count() < 3 {
            return Vec::new();
        }
        let rows: Vec<Suggestion> = settings_search::named(text, index)
            .into_iter()
            .map(|at| &index[at])
            .filter(|entry| entry.section != Section::Shortcuts)
            .take(SETTINGS_SHOWN)
            .map(|entry| Suggestion {
                text: format!("{} — Settings", entry.name).into(),
                setting: Some((entry.section, entry.name.clone())),
                ..Suggestion::default()
            })
            .collect();
        if !rows.is_empty() {
            // check.sh reads this line.
            let names: Vec<&str> = rows.iter().map(|row| row.text.as_ref()).collect();
            log!(
                "browser: command bar settings \"{text}\": {}",
                names.join(", ")
            );
        }
        rows
    }

    /// Puts the commands found for `text` among the core's `rows`, as Arc's bar does: from three
    /// letters, the best first, above the search row, if its name or a word of it starts with
    /// what's typed, then those whose name or keyword does before the core's other rows; the rest
    /// after them. A site completing what's typed puts them all after. None over a new pane.
    fn with_commands(&self, text: &str, rows: Vec<Suggestion>, cx: &App) -> Vec<Suggestion> {
        // Nothing but spaces typed would list every command, as the commands-only list does.
        if matches!(self.target, Target::Beside(_)) || text.trim().is_empty() {
            return rows;
        }
        let commands = self.host.commands(text, cx);
        let settings = self.setting_rows(text);
        if commands.is_empty() && settings.is_empty() {
            return rows;
        }
        // check.sh reads these lines.
        let names: Vec<&str> = commands
            .iter()
            .map(|command| command.name.as_ref())
            .collect();
        log!(
            "browser: command bar commands \"{text}\": {}",
            names.join(", ")
        );
        let completes = rows.first().is_some_and(|row| !row.completion.is_empty());
        let strong = !completes && text.trim().chars().count() >= 3;
        let mut commands = commands.into_iter().peekable();
        let mut rows = rows.into_iter();
        let mut merged = Vec::with_capacity(shell::BAR_ROWS + 1);
        let first = |command: &shell::BarCommand| strong && command.matched >= Match::Word;
        let early = |command: &shell::BarCommand| strong && command.matched >= Match::Keyword;
        merged.extend(commands.next_if(first).map(Suggestion::from));
        merged.extend(rows.next());
        while let Some(command) = commands.next_if(early) {
            merged.push(command.into());
        }
        merged.extend(settings);
        merged.extend(rows);
        merged.extend(commands.map(Suggestion::from));
        merged
    }

    /// Says on each page's row where its result is, besides an open tab's (#2337): a tab in the
    /// archive, which ↩ reopens as the tab it was, unless the page is open here, as an open tab is
    /// the better row, or it's the `address` typed, which loads where ↩ goes; else the day the
    /// profile's history last saw a visit to it. Once for each list the core gives, over the rows
    /// shown alone, never as the bar draws.
    fn place(&self, rows: &mut [Suggestion], address: Option<&str>, cx: &App) {
        let now = self.host.now();
        // Over a split's pane, ↩ puts the page beside it, which a reopened tab wouldn't be.
        let reopens = !matches!(self.target, Target::Beside(_));
        for row in rows {
            if row.named() || row.search || row.task.is_some() || row.tab != 0 {
                continue;
            }
            let here = |url: &SharedString| **url == *row.url;
            let open = self.tabs.iter().any(|tab| here(&tab.url)) || self.shown.iter().any(here);
            let reopenable = reopens
                && !open
                && !row.new_document
                && row.computer.is_empty()
                && address != Some(row.url.as_str());
            let archived = reopenable
                .then(|| self.host.archived(&row.url, cx))
                .flatten();
            if let Some((id, closed)) = archived {
                row.archived = id;
                row.closed = format!("Closed {} · Reopen", day_said(closed, now)).into();
            } else if row.last_visit > 0 {
                let visited = day_said(row.last_visit as f64 / 1000., now);
                row.visited = format!("Visited {visited}").into();
            }
        }
    }

    /// What a page's row says at its end about where its result is, and the glyph on the tile
    /// after it, if it has one: Switch to Tab and its arrow for an open tab, the day it closed
    /// and Reopen with ↺ for a closed one, and the day in words alone for a page only in the
    /// history, as ↩ only loads it (#2337).
    fn end(&self, row: &Suggestion) -> Option<(SharedString, Option<&'static str>)> {
        if row.tab != 0 {
            let action = match self.target {
                Target::Beside(_) => "Add to Split View",
                _ => "Switch to Tab",
            };
            return Some((action.into(), Some("icons/arrow-right.svg")));
        }
        if row.archived != 0 {
            return Some((row.closed.clone(), Some("icons/rotate-ccw.svg")));
        }
        (!row.visited.is_empty()).then(|| (row.visited.clone(), None))
    }

    /// Takes out of `rows` the open tabs that go above the rest with Switch to Tab, as Arc's bar is
    /// first of all a tab switcher: the one for the top, and those for right after the core's
    /// first row. They're those whose title or site has a word that two letters or more typed
    /// start, at most TABS_RISEN: a renamed tab whose own name what's typed starts first, as Arc
    /// boosts a custom title's start, then the most recently shown (ours: Arc's weights aren't
    /// recoverable). The one on top takes the site search the first row hints, as that follows
    /// what's typed rather than the row (#1397). Chrome's first row stays when the typing reads as
    /// a URL, and a first row completing to an open tab's page becomes that tab's row. An address
    /// typed that's an open tab's page keeps its row first, where ↩ goes, and the tab's comes
    /// second, as Arc's does.
    fn take_open_tabs(
        &self,
        text: &str,
        rows: &mut Vec<Suggestion>,
    ) -> (Option<Suggestion>, Vec<Suggestion>) {
        let first = rows.first();
        let typed = text.trim().to_lowercase();
        let address = typed.contains(['.', ':', '/']);
        if let Some(first) = first.filter(|row| !row.completion.is_empty() || address) {
            let at_page = |tab: &&shell::BarTab| *tab.url == *first.url && !self.is_shown(tab.tab);
            let Some(open) = self.tabs.iter().find(at_page) else {
                return (None, Vec::new());
            };
            let id = open.tab;
            if first.completion.is_empty() {
                log!("browser: command bar puts tab {id} second");
                rows.retain(|row| row.tab != id);
                return (None, vec![open.clone().into()]);
            }
            // check.sh reads this line.
            log!("browser: command bar puts tab {id} first");
            rows[0].tab = id;
            let mut at = 0;
            rows.retain(|row| {
                at += 1;
                at == 1 || row.tab != id
            });
            return (None, Vec::new());
        }
        if address || typed.chars().count() < 2 {
            return (None, Vec::new());
        }
        let named = |tab: &&shell::BarTab| !self.is_shown(tab.tab) && names_tab(tab, &typed);
        let mut named: Vec<&shell::BarTab> = self.tabs.iter().filter(named).collect();
        // A renamed tab whose name what's typed starts, then the tabs switched to more, as Arc's
        // rank (#1396). Stable, so the most recently shown stays first among equals. A score is
        // never negative, so its bits order as it does.
        named.sort_by_cached_key(|tab| {
            let prefix = tab.renamed && tab.title.to_lowercase().starts_with(&typed);
            (!prefix, std::cmp::Reverse(tab.switched.to_bits()))
        });
        named.truncate(TABS_RISEN);
        // Each in place of the core's row for it, where the core listed one.
        let mut risen: Vec<Suggestion> = named
            .into_iter()
            .map(|tab| match rows.iter().position(|row| row.tab == tab.tab) {
                Some(at) => rows.remove(at),
                None => tab.clone().into(),
            })
            .collect();
        if risen.is_empty() {
            return (None, risen);
        }
        let mut open = risen.remove(0);
        if let Some(first) = rows.first_mut().filter(|row| !row.keyword.is_empty()) {
            open.keyword = std::mem::take(&mut first.keyword);
            open.site = std::mem::take(&mut first.site);
        }
        // check.sh reads this line.
        log!("browser: command bar puts tab {} first", open.tab);
        (Some(open), risen)
    }

    /// Takes out of `rows` the pages the core listed that are open on the account's other
    /// computers and named by what's typed, and returns those computers' rows for them, to go
    /// after the open tabs that rise (#1234). The same rule as an open tab's names them: a word of
    /// the title or the site's name that two letters or more start, not while the typing reads as
    /// an address. At most OTHER_TABS_SHOWN, most recently seen first, one per page, never the
    /// first row, where ↩ goes, and never a page a tab here or the window shows, whose row is
    /// better. A pick opens the page here, as any page's row does.
    fn take_other_tabs(
        &self,
        text: &str,
        others: &[OtherTab],
        rows: &mut Vec<Suggestion>,
    ) -> Vec<Suggestion> {
        let typed = text.trim().to_lowercase();
        if typed.contains(['.', ':', '/']) || typed.chars().count() < 2 {
            return Vec::new();
        }
        let first = rows.first().map(|row| row.url.clone());
        let mut taken: Vec<Suggestion> = Vec::new();
        for other in others {
            if taken.len() == OTHER_TABS_SHOWN {
                break;
            }
            let here = |url: &SharedString| **url == *other.url;
            if first.as_deref() == Some(&*other.url)
                || self.tabs.iter().any(|tab| here(&tab.url))
                || self.shown.iter().any(here)
                || taken.iter().any(|row| row.url == *other.url)
                || !names_page(&other.title, &other.url, &typed)
            {
                continue;
            }
            // In place of the core's own row for the page, a page's from history.
            rows.retain(|row| row.named() || row.tab != 0 || row.url != *other.url);
            taken.push(Suggestion {
                url: other.url.to_string(),
                text: other.title.clone(),
                detail: site_host(&other.url).unwrap_or_default().into(),
                computer: other.computer.clone(),
                ..Suggestion::default()
            });
        }
        if !taken.is_empty() {
            // check.sh reads this line.
            let listed: Vec<String> = (taken.iter())
                .map(|row| format!("{} on {}", row.url, row.computer))
                .collect();
            log!(
                "browser: command bar lists other computers' tabs: {}",
                listed.join(", ")
            );
        }
        taken
    }

    /// The selected row's fill and its label, and the chip's in a site search: the site's own
    /// colour, made to read on the bar, where it has one, else the Space's accent.
    fn accent(&self, palette: &palette::Palette) -> (Hsla, Hsla) {
        let (fill, label) = match self.site.as_ref().and_then(|site| site.color) {
            Some(color) => palette.site_accent(gpui::rgb(color)),
            None => (palette.accent, palette.accent_label),
        };
        (fill.into(), label.into())
    }

    /// Whether `tab` is the one the bar edits or opens beside, which is shown, so never offered.
    fn is_shown(&self, tab: u64) -> bool {
        matches!(self.target, Target::Tab(shown) | Target::Beside(shown) if shown == tab)
    }

    fn set_rows(
        &mut self,
        text: String,
        rows: Vec<Suggestion>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The tooltip names the remove button of a page or search, which goes if another row takes
        // its place; the same page's comes again with more suggestions.
        let tipped = |rows: &[Suggestion]| {
            let row = rows.get(self.tipped?).filter(|row| row.removable)?;
            Some(row.url.clone())
        };
        if self.tipped.is_some() && tipped(&self.rows) != tipped(&rows) {
            self.end_tip(cx);
        }
        let rows = self.with_thread_row(rows, cx);
        // The page selected stays selected as more suggestions come for what was asked, and when
        // it's taken out, the row that takes its place is, as in Chrome.
        let selected = self
            .rows
            .get(self.selected)
            .filter(|_| text == self.suggested);
        self.selected = match selected {
            Some(old) => rows
                .iter()
                .position(|row| {
                    row.url == old.url
                        && row.command == old.command
                        && row.setting == old.setting
                        && row.task == old.task
                })
                .unwrap_or(self.selected.min(rows.len().saturating_sub(1))),
            None => 0,
        };
        self.rows = rows;
        self.suggested = text;
        if self.surface == Surface::Window {
            self.host.set_height(self.drawn_height());
        }
        self.scroll.scroll_to_item(self.selected);
        for row in self.rows.iter().filter(|row| row.icon() == Icon::Favicon) {
            if !self.favicons.contains_key(&row.url) {
                self.favicons.insert(row.url.clone(), None);
                self.host.page_favicon(&row.url);
            }
        }
        self.update_completion(window, cx);
        cx.notify();
    }

    /// How tall the bar draws: its input, its rows, and the composer's choices and foot.
    fn drawn_height(&self) -> f32 {
        let rows = self.rows.len();
        let foot = match self.ask.is_some() {
            true => CHOICES_HEIGHT + LINE + FOOT_HEIGHT,
            false => 0.,
        };
        foot + match self.over_field {
            true => field_height(rows),
            false => height(rows),
        }
    }

    /// `rows` with New Thread second, where ⇥ would grow the bar into the composer with what's
    /// typed. The first row stays the page or search for it, which ↩ takes.
    fn with_thread_row(&self, mut rows: Vec<Suggestion>, cx: &App) -> Vec<Suggestion> {
        rows.retain(|row| !row.starts_thread());
        if !rows.is_empty() && self.offers_ask(cx) {
            let thread = Suggestion {
                text: NEW_THREAD.into(),
                task: Some(TaskRow::Thread),
                ..Suggestion::default()
            };
            rows.insert(1, thread);
        }
        rows
    }

    /// The row ↩ takes, once there are rows for what's typed. A completion comes from the first
    /// row, even before the rows for the last key arrive.
    fn chosen(&self) -> Option<&Suggestion> {
        // An ask's rows are the agents, whatever is typed.
        let current =
            self.ask.is_some() || self.suggested == self.asked || !self.completion.is_empty();
        self.rows.get(self.selected).filter(|_| current)
    }

    /// What the user typed, without a completion still selected after it.
    fn typed(&self, cx: &App) -> String {
        let input = self.input.read(cx);
        let completing = !self.completion.is_empty()
            && input.selection() == (self.asked.len()..input.text().len());
        let typed = if completing {
            self.asked.as_str()
        } else {
            input.text()
        };
        typed.trim().to_string()
    }

    fn go(&mut self, _: &Go, window: &mut Window, cx: &mut Context<Self>) {
        // ↩ while an input method composes is its own, to commit with, as AppKit's and Windows'
        // take it before the bar: one that passes it on doesn't make the bar go with half a word.
        if !dismissal::submits([&self.input], cx) {
            return;
        }
        // A task isn't logged: it's the user's words.
        if self.ask.is_some() {
            return self.run_ask(By::Keyboard, window, cx);
        }
        let text = self.input.read(cx).text().trim().to_string();
        if !text.is_empty() {
            // check.sh reads these lines.
            log!("browser: command bar typed {text}");
        }
        // On the URL it opened on, as Arc's does, ↩ loads the page again rather than going to it.
        if let Target::Tab(tab) = self.target
            && !self.page.is_empty()
            && text == self.page
            && self.chosen().is_none_or(|row| row.url == self.page)
        {
            // check.sh reads this line.
            log!("browser: command bar reloads tab {tab}");
            let users = !self.host.agents_page(cx);
            self.close(Close::new(Why::Chosen, By::Keyboard));
            self.host.reload(tab);
            if users {
                self.host.focus_page();
            }
            return;
        }
        // New Thread grows the bar into the composer: ↩ on the first row never starts one.
        if self.chosen().is_some_and(Suggestion::starts_thread) {
            return self.enter_ask(window, cx);
        }
        // Before anything is typed, that's one of the Space's tabs.
        if self.chosen().is_some() {
            self.take(self.selected, By::Keyboard, cx);
            return;
        }
        // Before the suggestions for what's typed come, ↩ goes where the first of them will.
        if let Some(url) = self.typed_url(&text).filter(|_| !text.is_empty()) {
            self.go_to(&url, By::Keyboard, cx);
        }
    }

    /// Paste and Go, from the input's menu: goes where ↩ would for the clipboard's text, as
    /// Chrome does.
    fn paste_and_go(&mut self, _: &PasteAndGo, _: &mut Window, cx: &mut Context<Self>) {
        pasted(cx, |bar, text, cx| {
            if let Some(url) = bar.typed_url(text) {
                bar.go_to_typed(&url, text, By::Keyboard, cx);
            }
        });
    }

    /// Paste and Search: searches for the clipboard's text, even if it names a page, as ⇧⌘↩
    /// does for what's typed.
    fn paste_and_search(&mut self, _: &PasteAndSearch, _: &mut Window, cx: &mut Context<Self>) {
        if self.lists_own() || self.commands.is_some() || self.ask.is_some() {
            return;
        }
        let site = self.site.as_ref().map(|site| site.keyword.clone());
        pasted(cx, move |bar, text, cx| {
            if let Some(url) = bar.host.search_url(site.as_deref(), text) {
                bar.go_to(&url, By::Keyboard, cx);
            }
        });
    }

    /// Shows `url` where the bar shows the page chosen, and closes the bar.
    fn go_to(&self, url: &str, by: By, cx: &mut App) {
        self.go_to_typed(url, &self.typed(cx), by, cx);
    }

    /// As `go_to`, for `url` given as `typed`, which an http address written out keeps.
    fn go_to_typed(&self, url: &str, typed: &str, by: By, cx: &mut App) {
        // check.sh reads this line.
        log!("browser: command bar go {url}");
        // Asked before the page shows, which can take the window out of a task's area.
        let users = !self.host.agents_page(cx);
        self.show_page(url, typed, cx);
        self.close(Close::new(Why::Chosen, by));
        // The page takes the keyboard, as Chrome's address bar gives it, wherever it was before the
        // bar opened, such as in the UI over a Board. Chrome gives it only while its window is
        // active, which on the Mac and Windows the bar's own window was until it hid (#3453).
        if users {
            self.host.focus_page();
        }
    }

    /// ⌘↩: opens where ↩ would go in a tab behind the one shown, at the top of Today, and the bar
    /// stays open (Arc). A command or a sum's result is taken as ↩ takes it, and over a new pane
    /// ⌘↩ is ↩.
    fn go_behind(&mut self, _: &GoBehind, window: &mut Window, cx: &mut Context<Self>) {
        if self.takes_in_place() {
            return self.go(&Go, window, cx);
        }
        if let Some(url) = self.return_url(cx) {
            // check.sh reads this line.
            log!("browser: command bar behind {url}");
            self.host.open_tab(&url, &self.typed(cx), true);
        }
    }

    /// Whether ↩'s row is a command or a sum, or the bar is over a new pane, for Search Tabs or in
    /// an ask, where the keys that open a page elsewhere take what ↩ takes.
    fn takes_in_place(&self) -> bool {
        let command = self.chosen().is_some_and(Suggestion::named);
        command
            || self.ask.is_some()
            || matches!(self.target, Target::Beside(_))
            || self.lists_own()
    }

    /// The page ↩ goes to: its row's, a tab's row's too, else the one what's typed names.
    fn return_url(&self, cx: &App) -> Option<String> {
        let text = self.input.read(cx).text().trim().to_string();
        match self.chosen() {
            Some(row) => Some(row.url.clone()),
            None if text.is_empty() => None,
            None => self.typed_url(&text),
        }
    }

    /// ⇧↩: opens where ↩ would go in a new browser window, as Chrome does (Arc unrecorded). A
    /// command or a sum's result is taken as ↩ takes it, and over a new pane ⇧↩ is ↩.
    fn go_window(&mut self, _: &GoWindow, window: &mut Window, cx: &mut Context<Self>) {
        if self.takes_in_place() {
            return self.go(&Go, window, cx);
        }
        if let Some(url) = self.return_url(cx) {
            self.go_to_window(&url, cx);
        }
    }

    /// Opens `url` in a new browser window, and closes the bar.
    fn go_to_window(&self, url: &str, cx: &mut App) {
        // check.sh reads this line.
        log!("browser: command bar window {url}");
        // The bar leaves its window first, which the new one then takes the keyboard from.
        self.close(Close::new(Why::Chosen, By::Keyboard));
        self.host.open_window(url, &self.typed(cx), cx);
    }

    /// www.<word>.com for a word typed outside a site search, where ⌃↩ goes, as in Chrome.
    fn dot_com(&self, cx: &App) -> Option<String> {
        let typed = self.typed(cx);
        let word = !typed.is_empty() && typed.chars().all(|c| c.is_alphanumeric() || c == '-');
        let word = word && self.site.is_none() && self.commands.is_none() && self.ask.is_none();
        let word = word && !self.lists_own();
        word.then(|| self.host.typed_url(&format!("www.{typed}.com")))
            .flatten()
    }

    /// ⌃↩: goes to www.<word>.com for a word typed, as Chrome does, and else where ↩ goes, as in a
    /// site search.
    fn go_dot_com(&mut self, _: &GoDotCom, window: &mut Window, cx: &mut Context<Self>) {
        match self.dot_com(cx) {
            Some(url) => self.go_to(&url, By::Keyboard, cx),
            None => self.go(&Go, window, cx),
        }
    }

    /// ⌃⇧↩: ⌃↩'s page in a new browser window, as ⇧↩ opens ↩'s, as in Chrome.
    fn go_dot_com_window(
        &mut self,
        _: &GoDotComWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.dot_com(cx) {
            Some(url) => self.go_to_window(&url, cx),
            None => self.go_window(&GoWindow, window, cx),
        }
    }

    /// ⇧⌘↩: searches for what was typed, even if it names a page, where ↩ goes (Dia). Search Tabs
    /// searches only its tabs.
    fn search(&mut self, _: &Search, _: &mut Window, cx: &mut Context<Self>) {
        if self.lists_own() || self.commands.is_some() || self.ask.is_some() {
            return;
        }
        let typed = self.typed(cx);
        let site = self.site.as_ref().map(|site| site.keyword.as_str());
        if let Some(url) = self
            .host
            .search_url(site, &typed)
            .filter(|_| !typed.is_empty())
        {
            self.go_to(&url, By::Keyboard, cx);
        }
    }

    /// ⇧⌦, as Chrome's, or ⌥⌘⌫, as Arc's: takes the selected row out of the profile's history.
    fn remove(&mut self, _: &Remove, _: &mut Window, cx: &mut Context<Self>) {
        if self.chosen().is_some() {
            self.end_tip(cx);
            self.remove_row(self.selected, cx);
        }
    }

    /// Takes row `index`'s page or search out of the profile's history, if it came from there. The
    /// core lists the rows again without it, and the row under it takes its place.
    fn remove_row(&self, index: usize, cx: &mut App) {
        let Some(row) = self.rows.get(index).filter(|row| row.removable) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: command bar removes {}", row.url);
        self.host.remove_suggestion(&row.url, cx);
    }

    /// ⌘Z or ⇧⌘Z: a field's are its own, and the bar's has no undo, but a row it took out of
    /// history is the window's step, which they undo and redo here while it's the next.
    fn step_removal(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.host.step_removal(redo, cx) {
            cx.stop_propagation();
        }
    }

    /// The trash a row from history shows under the pointer, where Arc's has a ✕, which takes it
    /// out: ✕ closes and trash removes (ux-principles, the owner, #1394). It's the arrow's tile, so
    /// the two read as one kind.
    fn remove_button(&self, index: usize, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let palette = palette::current(cx);
        let (accent, label) = self.accent(&palette);
        let (tile, color) = match selected {
            true => (label, accent),
            false => (palette.plate(ARROW_TILE), palette.on_overlay(ARROW)),
        };
        div()
            .id(("remove", index))
            .when(self.a11y, |button| {
                button.role(Role::Button).aria_label(REMOVE_NAME)
            })
            .size(px(ICON_BOX))
            .flex_none()
            .ml(px(7.))
            .mr(px(3.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .bg(tile)
            .child(
                svg()
                    .path("icons/trash.svg")
                    .size(px(14.))
                    .text_color(color),
            )
            // The click is the trash's, not the row's under it.
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |bar, _, _, cx| {
                cx.stop_propagation();
                bar.end_tip(cx);
                bar.remove_row(index, cx);
            }))
            // Where it drew, which its tooltip hangs from.
            .child({
                let frame = self.remove_frame.clone();
                canvas(|_, _, _| {}, move |bounds, _, _, _| frame.set(Some(bounds)))
                    .absolute()
                    .size_full()
            })
            // Its tooltip waits for the pointer to rest, as a tile's does.
            .on_mouse_move(|_, _, cx| crate::tooltip::moved(cx))
            .on_hover(cx.listener(move |bar, hovered, window, cx| {
                bar.hover_remove(index, *hovered, window.viewport_size(), cx);
            }))
            .into_any_element()
    }

    /// The pointer has come onto or left row `index`'s remove button, whose tooltip names it and its
    /// key. It hangs under the button, inside the bar, or above it when the bar has no room below.
    fn hover_remove(&mut self, index: usize, hovered: bool, window: Size<Pixels>, cx: &mut App) {
        // The host hangs it over the bar's own window, which a resting bar hasn't got.
        if self.surface == Surface::InPlace {
            return;
        }
        match self.remove_frame.get().filter(|_| hovered) {
            Some(frame) => {
                self.tipped = Some(index);
                let tip = Tip {
                    text: REMOVE_NAME.into(),
                    keys: remove_keys(),
                    detail: SharedString::default(),
                };
                self.host
                    .tooltip(Some((tip, Anchor::under(frame, window))), cx);
            }
            None if self.tipped == Some(index) => self.end_tip(cx),
            None => {}
        }
    }

    /// Ends the tooltip of the remove button it asked for one for, if any.
    fn end_tip(&mut self, cx: &mut App) {
        if self.tipped.take().is_some() {
            self.host.tooltip(None, cx);
        }
    }

    /// ⌥↩: goes where ↩ would, in a new tab in a split view with the tab shown, or the tab the
    /// bar edits, on its right (Arc). For Search Tabs and an ask it's ↩.
    fn go_beside(&mut self, _: &GoBeside, window: &mut Window, cx: &mut Context<Self>) {
        if self.ask.is_some() {
            return self.go(&Go, window, cx);
        }
        let with = match self.target {
            // A task's page stays in its Space, so it never goes into a split of the user's.
            Target::Tab(tab) if shell::task_page_shown(tab, cx) => 0,
            Target::Tab(tab) | Target::Beside(tab) => tab,
            Target::NewTab => shell::shown_tab(cx),
            Target::Tabs | Target::Spaces => 0,
        };
        if with != 0 {
            self.target = Target::Beside(with);
        }
        self.go(&Go, window, cx);
    }

    /// Shows `url`, given as `typed`, where the bar shows the page chosen.
    fn show_page(&self, url: &str, typed: &str, cx: &mut App) {
        match self.target {
            Target::NewTab | Target::Tabs | Target::Spaces => self.host.open_tab(url, typed, false),
            Target::Tab(tab) => self.host.navigate(tab, url, typed),
            Target::Beside(with) => self.host.open_split(url, typed, with, cx),
        }
    }

    /// Goes where row `index` goes: to its tab, else its page, where the bar shows the page
    /// chosen. Beside a tab, its tab moves into the split view. A tab another window shows live
    /// brings that window forward with it, as Chrome's Switch to Tab does, rather than taking its
    /// page (Arc unrecorded). A sum's row copies its result.
    fn take(&mut self, index: usize, by: By, cx: &mut App) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        if row.starts_thread() {
            return;
        }
        if let Some(result) = &row.result {
            // check.sh reads this line.
            log!("browser: command bar copies {result}");
            self.close(Close::new(Why::Chosen, by));
            self.host.copy(result, cx);
            return;
        }
        if let Some((section, name)) = row.setting.clone() {
            // check.sh reads this line.
            log!(
                "browser: command bar shows setting {} › {name}",
                section.name()
            );
            self.close(Close::new(Why::Chosen, by));
            self.host.show_setting(section, name, cx);
            return;
        }
        // check.sh reads these lines.
        if let Some(command) = row.command {
            log!("browser: command bar runs {}", row.text);
            shell::picked_in_bar(&row.text, command, cx);
            use shell::Command::{AddSplit, EditUrl, NewTab, SearchTabs};
            if !matches!(command, NewTab | SearchTabs | EditUrl | AddSplit) {
                self.close(Close::new(Why::Chosen, by));
                shell::run_from_bar(command, cx);
                return;
            }
            // One that opens the bar again, such as Add Split View over its pane, runs once the
            // bar is done with the key, as it updates the bar's window, which this is in. The bar
            // moves there without going, as hidden and shown at once it would lose the keyboard
            // to the page on X11, and goes only if it didn't open.
            self.closed(Close::new(Why::Chosen, by));
            let host = (self.surface == Surface::Window).then(|| self.host.clone());
            cx.defer(move |cx| {
                shell::run_from_bar(command, cx);
                if let Some(host) = host.filter(|_| !is_open(cx)) {
                    host.hide();
                }
            });
            return;
        }
        // A closed tab comes back as it was, with its history, as the Library's Reopen brings it,
        // unless it has left the archive since, when its page loads as any page's.
        if row.archived != 0 && !matches!(self.target, Target::Beside(_)) {
            let (archived, url) = (row.archived, row.url.clone());
            if !self.host.restore_archived(archived, cx) {
                return self.go_to(&url, by, cx);
            }
            self.close(Close::new(Why::Chosen, by));
            return;
        }
        match (row.tab, self.target) {
            (0, Target::Tab(_)) if row.new_document => {
                // check.sh reads this line.
                log!("browser: command bar new tab {}", row.url);
                let users = !self.host.agents_page(cx);
                self.host.open_tab(&row.url, "", false);
                self.close(Close::new(Why::Chosen, by));
                // A new document is a page the user went to, as go_to_typed's are.
                if users {
                    self.host.focus_page();
                }
                return;
            }
            (0, _) => return self.go_to(&row.url, by, cx),
            (tab, Target::Beside(with)) => {
                log!("browser: command bar split with tab {tab}");
                self.host.split(tab, with, cx);
            }
            (tab, _) => match self.host.tab_window(tab) {
                Some(window) => {
                    log!("browser: command bar switch to tab {tab} in window {window}");
                    self.close(Close::new(Why::Chosen, by));
                    return self.host.activate(window, tab, cx);
                }
                None => {
                    log!("browser: command bar switch to tab {tab}");
                    self.host.select(tab);
                }
            },
        }
        self.close(Close::new(Why::Chosen, by));
    }

    /// Selects the next row, or the one before, wrapping at the ends as Chrome does.
    fn select_next(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.rows.len();
        if count > 0 {
            self.select(
                (self.selected + if forward { 1 } else { count - 1 }) % count,
                window,
                cx,
            );
        }
    }

    /// Selects row `index`, if there is one.
    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.rows.len() {
            return;
        }
        self.selected = index;
        self.scroll.scroll_to_item(index);
        // check.sh reads this line.
        let row = &self.rows[self.selected];
        let named = row.named();
        let name = if named {
            row.text.as_ref()
        } else {
            row.url.as_str()
        };
        log!("browser: command bar selected {name}");
        self.update_completion(window, cx);
        cx.notify();
    }

    /// The pointer came onto row `index`, or left it.
    fn hover(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered = Some(index);
        } else if self.hovered == Some(index) {
            self.hovered = None;
        }
        // The remove button goes with the pointer's row, and its tooltip with it.
        if self
            .tipped
            .is_some_and(|tipped| self.hovered != Some(tipped))
        {
            self.end_tip(cx);
        }
        cx.notify();
    }

    /// Closes the bar for `close` (#4108). The window's hides: for a choice as the host's hide,
    /// which gives the keyboard back where it was unless the choice then gives it to a page; as
    /// the user leaving it for Escape, its own key or close, or the keyboard or pointer gone
    /// elsewhere; and not again when the host hid it as the user left it. A resting bar has no window: its field keeps the
    /// keyboard, and its page goes on to what was picked.
    fn close(&self, close: Close) {
        self.closed(close);
        if self.surface == Surface::InPlace {
            return;
        }
        match close.why {
            Why::Chosen => self.host.hide(),
            Why::Escape | Why::Toggled | Why::CloseCommand | Why::FocusLeft | Why::PointerLeft => {
                self.host.dismiss()
            }
            Why::PressOutside | Why::WindowLeft => {}
        }
    }

    /// The bar is closed for `close`, its window hidden by the host already or by what it's asked
    /// next, such as folding into a task's slot. A resting bar stays open, as its field stays in
    /// its page for the next thing typed.
    fn closed(&self, close: Close) {
        if self.surface == Surface::Window && self.open.replace(false) {
            log!(
                "browser: command bar closed: {:?} by {:?}",
                close.why,
                close.by
            );
        }
    }

    /// As `close`, once the update the bar is in is over: the Shell's, which the host's dismissal
    /// would update again.
    fn close_later(&self, close: Close, cx: &mut Context<Self>) {
        self.closed(close);
        let bar = cx.entity().downgrade();
        cx.defer(move |cx| {
            bar.read_with(cx, |bar, _| bar.close(close)).ok();
        });
    }

    /// While it shows, the keys are the bar's until the user leaves it (#1314). Chromium makes its
    /// window key by itself, such as when a page commits, which the host undoes as it's told; if
    /// the bar finds its window isn't key anyway, it asks for the keyboard back.
    fn activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open.get() {
            return;
        }
        if window.is_window_active() {
            self.keep_focus(window, cx);
        } else {
            self.host.take_keyboard();
        }
    }

    /// Gives the field the focus back while the bar shows: without it, GPUI hands the field no
    /// input method's text, and the bar shows deaf. Nothing else in its window takes the focus,
    /// but an assistive app can.
    fn keep_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.input.focus_handle(cx);
        if self.open.get() && !field.is_focused(window) {
            let from = if window.focused(cx).is_some() {
                "another view"
            } else {
                "nothing"
            };
            log!("browser: command bar's field takes the focus back from {from}");
            window.focus(&field, cx);
        }
    }

    /// ⌘T or ⌘L, which open the bar, close it again, as Arc's do, whichever opened it. The browser's
    /// other shortcuts go on to the browser.
    fn shortcut(&mut self, run: &shell::Run, _: &mut Window, cx: &mut Context<Self>) {
        // At rest they're the window's, which gives the field the keyboard.
        if self.surface == Surface::InPlace
            || !matches!(run.0, shell::Command::NewTab | shell::Command::EditUrl)
        {
            return cx.propagate();
        }
        // check.sh reads this line.
        log!("browser: command bar closes again on {}", run.0.name());
        self.end_ask(cx);
        self.close(Close::new(Why::Toggled, By::Keyboard));
    }

    /// Esc leaves the site search, the commands-only list or the ask first, then the bar. Out of a
    /// site search the bar is as it opened, as Arc's `postScopedSearchRestoration` (#1397).
    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        // Esc while an input method composes is its own, to cancel with: it neither closes the bar
        // nor leaves what's in it, nor, at rest, takes the page out of Ask.
        let Some(close) = Close::escape_from([&self.input], cx) else {
            return;
        };
        if self.site_left(cx).is_some() {
            // check.sh reads this line.
            log!("browser: command bar leaves site search as it opened");
            return self.as_opened(window, cx);
        }
        if self.leave_commands(window, cx) {
            return;
        }
        // At rest the bar stays: Esc leaves the composer with its words, as ⇧⇥ does.
        if self.surface == Surface::InPlace {
            self.leave_ask(window, cx);
            return;
        }
        // The composer closes as the bar does, its words with it (P3).
        self.end_ask(cx);
        self.close(close);
    }

    /// `row`'s icon, `size` square, in `color` where it's a glyph, on `behind`.
    fn glyph(&self, row: &Suggestion, size: f32, color: Hsla, behind: Rgba) -> Option<AnyElement> {
        Some(match row.icon() {
            Icon::Favicon => self.favicon(&row.url, size, color),
            Icon::Glyph(path) => glyph(path, size, color),
            Icon::Swatch(swatch) => swatch.dot(behind).into_any_element(),
            // Drawn in the text's colour where a row has it (`row`).
            Icon::Mark | Icon::None => return None,
        })
    }

    /// The favicon of the page at `url`, `size` square, with a globe in `color` until it comes or
    /// if it has none.
    fn favicon(&self, url: &str, size: f32, color: Hsla) -> AnyElement {
        match self.favicons.get(url).cloned().flatten() {
            // GPUI decodes the favicon off the main thread, and draws nothing until it has.
            Some(favicon) => img(favicon)
                .size(px(size))
                .flex_none()
                .with_loading(move || glyph("icons/globe.svg", size, color))
                .into_any_element(),
            None => glyph("icons/globe.svg", size, color),
        }
    }

    /// The Space's tabs, as rows, which the bar lists before anything is typed, and for Search
    /// Tabs as many as it lists rows.
    fn tab_rows(&self) -> Vec<Suggestion> {
        let shown = match self.target {
            Target::Tabs => shell::BAR_ROWS,
            Target::Spaces => return self.space_rows(""),
            _ => shell::BAR_TABS_SHOWN,
        };
        self.tabs
            .iter()
            .take(shown)
            .cloned()
            .map(Suggestion::from)
            .collect()
    }

    /// The input's icon: the search glyph, or once something is typed, the selected row's, as
    /// Arc's becomes its favicon. On a page's URL it's the page's favicon until it's edited.
    fn input_icon(&self, cx: &App) -> AnyElement {
        let color = palette::current(cx).ink.into();
        let size = if self.over_field {
            ICON_SIZE
        } else {
            INPUT_ICON
        };
        if !self.page.is_empty() && self.asked == self.page {
            return self.favicon(&self.page, size, color);
        }
        // The composer's slot is the product's mark.
        if self.ask.is_some() {
            return product::mark(size, &palette::current(cx)).into_any_element();
        }
        // A Spaces menu searches, whichever Space is selected.
        let row = self
            .chosen()
            .filter(|_| !self.asked.is_empty() && !self.lists_spaces());
        let overlay = palette::current(cx).overlay;
        match row.and_then(|row| self.glyph(row, size, color, overlay)) {
            Some(icon) => icon,
            None if self.commands.is_some() => glyph(COMMAND_GLYPH, size, color),
            None => glyph("icons/search.svg", size, color),
        }
    }

    /// What the chip before the input names: the site search's engine, the commands-only list, or
    /// in an ask, the page that goes with it.
    fn mode_name(&self) -> Option<SharedString> {
        if let Some(ask) = &self.ask {
            return ask
                .page
                .as_ref()
                .map(|(url, title)| match title.is_empty() {
                    true => SharedString::from(crate::agents::site(url)),
                    false => title.clone(),
                });
        }
        match (&self.site, self.commands) {
            (Some(site), _) => Some(site.name.clone()),
            (None, Some(_)) => Some(COMMANDS_CHIP.into()),
            (None, None) => None,
        }
    }

    /// "Search <site>" and the Tab key, at the input's end, while the row ↩ takes hints a site
    /// search; "Ask Claude" and ⇥ where ⇥ would make what's typed a task; "Run" and ↩ in the ask.
    fn site_hint(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let site = self
            .chosen()
            .filter(|row| self.site.is_none() && !row.keyword.is_empty());
        let (said, key) = match site {
            Some(row) => (format!("Search {}", row.site), "Tab".to_string()),
            None => return None,
        };
        let palette = palette::current(cx);
        let key = key_chip(key, &palette);
        Some(
            div()
                .flex_none()
                .ml(px(HINT_GAP))
                .flex()
                .items_center()
                .gap(px(HINT_KEY_GAP))
                .text_size(px(text::BODY))
                .child(div().text_color(palette.on_overlay(DIM)).child(said))
                .child(key),
        )
    }

    /// The composer's row of choices: the level and the model, each opening its menu under it,
    /// then Start, which starts the thread as ↩ does and does nothing with no words.
    fn choices(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = palette::current(cx);
        let (text, dim) = (palette.on_overlay(TITLE), palette.on_overlay(DIM));
        let tier = self.ask.as_ref().map(|ask| ask.tier).unwrap_or_default();
        let words = |said: SharedString, color: Hsla| {
            div()
                .flex_none()
                .text_size(px(TRIGGER_TEXT))
                .line_height(px(TRIGGER_LINE))
                .text_color(color)
                .child(said)
        };
        let open = self.menu_open;
        let trigger = |id: &'static str, at: &Rc<Cell<Option<Bounds<Pixels>>>>| {
            div()
                .id(id)
                .relative()
                .h(px(TRIGGER_HEIGHT))
                .flex_none()
                .flex()
                .items_center()
                .rounded(px(CHIP_RADIUS))
                // A trigger has no fill at rest, only while its menu is open.
                .when(open == Some(id), |shown| shown.bg(palette.plate(CHIP)))
                .child(drawn_at(at))
        };
        let level = trigger(LEVEL, &self.level_at)
            .when(self.a11y, |level| {
                level.role(Role::Button).aria_label(tier.label())
            })
            .px(px(TRIGGER_X))
            .gap(px(TRIGGER_GAP))
            .on_click(cx.listener(|bar, _, window, cx| bar.level_menu(window, cx)))
            .child(words(tier.label().into(), text))
            .child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, text));
        let (mark, said) = match self.composer_assistant() {
            Some(agent) => {
                let model = self.composer_model().unwrap_or_default();
                (Some(agent.glyph), model_words(agent, &model))
            }
            None => (None, SIGN_IN.to_string()),
        };
        let model = trigger(MODEL, &self.model_at)
            .when(self.a11y, |model| {
                model.role(Role::Button).aria_label(said.clone())
            })
            .pl(px(MODEL_LEFT))
            .pr(px(TRIGGER_X))
            .gap(px(MODEL_GAP))
            .on_click(cx.listener(|bar, _, window, cx| bar.model_menu(window, cx)))
            .children(mark.map(|mark| glyph(mark, MODEL_MARK, text)))
            .child(words(said.into(), text))
            .child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, dim));
        let ready = !self.input.read(cx).text().trim().is_empty();
        let (fill, arrow) = match ready {
            true => (palette.accent.into(), palette.accent_label.into()),
            false => (palette.plate(CHIP), dim),
        };
        let start = div()
            .id("composer-start")
            .when(self.a11y, |start| {
                start.role(Role::Button).aria_label(NEW_THREAD)
            })
            .size(px(START_SIZE))
            .ml(px(START_SPACE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(fill)
            .on_click(cx.listener(|bar, _, window, cx| bar.run_ask(By::Pointer, window, cx)))
            .child(glyph("icons/arrow-up.svg", START_ARROW, arrow));
        div()
            .h(px(CHOICES_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .pb(px(CHOICES_UNDER))
            .pl(px(CHOICES_LEFT))
            .pr(px(CHOICES_RIGHT))
            .gap(px(CHOICES_GAP))
            .child(level)
            .child(div().flex_1())
            .child(model)
            .child(start)
    }

    /// The composer's foot: ⇥ and Back to Search, which a click does too, then what ↩ does.
    fn foot(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = palette::current(cx);
        let words = |said: &'static str| {
            div()
                .flex_none()
                .text_size(px(FOOT_TEXT))
                .line_height(px(FOOT_LINE))
                .text_color(palette.on_overlay(DIM))
                .child(said)
        };
        let back = div()
            .id("back-to-search")
            .when(self.a11y, |back| {
                back.role(Role::Button).aria_label(BACK_TO_SEARCH)
            })
            .flex()
            .items_center()
            .gap(px(FOOT_GAP))
            .on_click(cx.listener(|bar, _, window, cx| {
                bar.leave_ask(window, cx);
            }))
            .child(key_chip(hint_key("tab", shell::OFF_THE_MAC), &palette))
            .child(words(BACK_TO_SEARCH));
        div()
            .h(px(FOOT_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(FOOT_LEFT))
            .pr(px(FOOT_RIGHT))
            .gap(px(FOOT_GAP))
            .child(back)
            .child(div().flex_1())
            .child(words(OPENS_AS))
            .child(key_chip(hint_key("enter", shell::OFF_THE_MAC), &palette))
    }

    /// What VoiceOver reads for a row: what it says, then what ↩ does on it when that isn't to
    /// open it.
    fn spoken(&self, row: &Suggestion) -> String {
        let mut spoken = row.text.to_string();
        if !row.detail.is_empty() && !self.over_field {
            spoken = format!("{spoken}, {}", row.detail);
        }
        if let Some((said, _)) = self.end(row) {
            spoken = format!("{spoken}, {}", said.replace(" · ", ", "));
        }
        if !row.computer.is_empty() {
            spoken = format!("{spoken}, open on {}", row.computer);
        }
        spoken
    }

    /// The selected row and where it stands, which VoiceOver says as the keys move the selection
    /// while the field keeps the keyboard, as Chrome's address bar does.
    fn announced(&self) -> Option<String> {
        let row = self.rows.get(self.selected)?;
        Some(format!(
            "{}, {} of {}",
            self.spoken(row),
            self.selected + 1,
            self.rows.len()
        ))
    }

    fn row(&self, index: usize, row: &Suggestion, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = index == self.selected;
        let thread = row.starts_thread();
        let end = self.end(row);
        // The selected row keeps its accent under the pointer.
        let hovered = !selected && self.hovered == Some(index);
        let palette = palette::current(cx);
        let (accent, label) = self.accent(&palette);
        let (title, dim) = match selected {
            true => (label, label.opacity(SELECTED_DIM)),
            false => (palette.on_overlay(TITLE), palette.on_overlay(DIM)),
        };
        // The icon sits on a tile of the accent's label in the selected row, and a command's on a
        // faint one in the rest.
        let glyph = match selected {
            true => accent,
            false => palette.on_overlay(ROW_GLYPH),
        };
        let tile = match (selected, row.kind()) {
            (true, _) => Some(label),
            (false, Kind::Command) => Some(palette.plate(CHIP)),
            _ => None,
        };
        // What a Space's mark is ringed against: the selected row's tile, else the panel, which a
        // command's faint tile hardly changes.
        let tile_behind = if selected {
            label
        } else {
            Hsla::from(palette.overlay)
        };
        let icon = div()
            .size(px(ICON_BOX))
            .mr(px(TITLE_X - ICON_BOX_X - ICON_BOX))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .when_some(tile, |icon, tile| icon.bg(tile))
            .children(match row.icon() {
                // The product's mark is the text's colour, selected or not, a little over an icon.
                Icon::Mark => Some(product::mark(MARK, &palette).into_any_element()),
                _ => self.glyph(row, ICON_SIZE, glyph, Rgba::from(tile_behind)),
            });
        // A key's chip: the accent's label with the accent on it on the selected row.
        let chip = |text: SharedString| {
            let (fill, text_color) = match selected {
                true => (label, accent),
                false => (palette.plate(CHIP), dim),
            };
            div()
                .h(px(ICON_BOX))
                .flex_none()
                .px(px(7.))
                .flex()
                .items_center()
                .rounded(px(CHIP_RADIUS))
                .bg(fill)
                .text_color(text_color)
                .text_size(px(text::BODY))
                .child(text)
        };
        div()
            .id(("suggestion", index))
            .when(self.a11y, |line| {
                line.role(Role::ListBoxOption)
                    .aria_label(self.spoken(row))
                    .aria_selected(selected)
                    .aria_position_in_set(index + 1)
                    .aria_size_of_set(self.rows.len())
            })
            .h(px(ROW_HEIGHT - 2. * PLATE_INSET))
            .mx(px(PLATE_INSET))
            .my(px(PLATE_INSET))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(ICON_BOX_X - PLATE_INSET))
            .pr(px(ROW_END))
            .rounded(px(PLATE_RADIUS))
            .text_color(title)
            .when(selected, |line| line.bg(accent))
            .when(hovered, |line| line.bg(palette.plate(HOVERED)))
            // Arc keeps the plate under the pointer as the keys move the selection.
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(move |bar, hovered: &bool, _, cx| bar.hover(index, *hovered, cx)))
            .on_click(cx.listener(move |bar, _, window, cx| match thread {
                true => bar.enter_ask(window, cx),
                false => bar.take(index, By::Pointer, cx),
            }))
            .child(icon)
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .gap(px(3.))
                    .child(div().min_w_0().truncate().child(row.text.clone()))
                    // Over the URL field Arc's titles have no host after them.
                    .when(!row.detail.is_empty() && !self.over_field, |line| {
                        let detail = format!("— {}", row.detail);
                        line.child(div().min_w_0().truncate().text_color(dim).child(detail))
                    }),
            )
            // Where its result is: an open tab's Switch to Tab on a tile with its arrow, a closed
            // tab's day and Reopen on one with ↺, as ↩ brings it back as it was, and a visited
            // page's day in words alone, as ↩ only loads it (#2337).
            .when_some(end.clone(), |line, (said, glyph)| {
                let tile = glyph.map(|glyph| {
                    div()
                        .size(px(ICON_BOX))
                        .flex_none()
                        .mr(px(END_TILE_RIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(CHIP_RADIUS))
                        .bg(if selected {
                            label
                        } else {
                            palette.plate(ARROW_TILE)
                        })
                        .child(
                            svg()
                                .path(glyph)
                                .size(px(END_GLYPH))
                                .text_color(if selected {
                                    accent
                                } else {
                                    palette.on_overlay(ARROW)
                                }),
                        )
                });
                let tint = if selected { label } else { dim };
                let end = if tile.is_some() { 7. } else { bare_end() };
                let label = div().flex_none().mr(px(end)).text_color(tint);
                line.child(div().flex_1())
                    .child(label.child(said))
                    .children(tile)
            })
            .when_some(row.shortcut.clone(), |line, shortcut| {
                line.child(div().flex_1()).child(chip(shortcut))
            })
            // Another computer's tab ends with that computer's name, as an agent's row with its
            // model.
            .when(!row.computer.is_empty(), |line| {
                let tint = if selected { label } else { dim };
                line.child(div().flex_1()).child(
                    div()
                        .flex_none()
                        .ml(px(HINT_GAP))
                        .text_color(tint)
                        .child(row.computer.clone()),
                )
            })
            // New Thread ends with its kind and ⇥, which grows the bar as a click does.
            .when(thread, |line| {
                let tint = if selected { label } else { dim };
                line.child(div().flex_1())
                    .child(
                        div()
                            .flex_none()
                            .mr(px(HINT_KEY_GAP))
                            .text_color(tint)
                            .child(THREAD_KIND),
                    )
                    .child(chip(hint_key("tab", shell::OFF_THE_MAC).into()))
            })
            // Under the pointer, the selected row too, as Arc's.
            .when(row.removable && self.hovered == Some(index), |line| {
                line.when(end.is_none(), |line| line.child(div().flex_1()))
                    .child(self.remove_button(index, selected, cx))
            })
    }
}

impl CommandBar {
    /// A live region that says the selected row as it changes, a polite one, so typing's echo
    /// comes first. The Mac's AccessKit announces a region's value, and Windows' and Linux's its
    /// name, so it has both (#3236).
    fn announcer(&self) -> impl IntoElement + use<> {
        div()
            .id("selected-suggestion")
            .absolute()
            .size_0()
            .role(Role::Status)
            .when_some(self.announced(), |announcer, said| {
                announcer.aria_label(said.clone()).aria_value(said)
            })
            .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
    }
}

/// A key's chip, its rim saying it's a key and not a word: "Tab" by "Switch to Tab" reads as a tab
/// otherwise.
fn key_chip(key: String, palette: &palette::Palette) -> gpui::Div {
    div()
        .h(px(ICON_BOX))
        .flex_none()
        .px(px(HINT_KEY_X))
        .flex()
        .items_center()
        .rounded(px(CHIP_RADIUS))
        .bg(palette.plate(CHIP))
        .border_1()
        .border_color(palette.plate(HINT_KEY_RIM))
        .text_color(palette.on_overlay(TITLE))
        .child(key)
}

/// The glyph at `path`, `size` square, in `color`.
fn glyph(path: &'static str, size: f32, color: Hsla) -> AnyElement {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}

impl CommandBar {
    /// The bar's keys, on what holds its field: the panel in its window, the field at rest.
    fn with_keys(&self, holder: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        holder
            .key_context("CommandBar")
            .on_action(cx.listener(Self::go))
            .on_action(cx.listener(Self::go_beside))
            .on_action(cx.listener(Self::go_behind))
            .on_action(cx.listener(Self::go_dot_com))
            .on_action(cx.listener(Self::go_window))
            .on_action(cx.listener(Self::go_dot_com_window))
            .on_action(cx.listener(Self::search))
            .on_action(cx.listener(Self::paste_and_go))
            .on_action(cx.listener(Self::paste_and_search))
            .on_action(cx.listener(Self::remove))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::shortcut))
            .on_action(cx.listener(Self::enter_site_or_next))
            .on_action(cx.listener(Self::leave_site_or_previous))
            .capture_action(
                cx.listener(|bar, _: &text_input::Undo, _, cx| bar.step_removal(false, cx)),
            )
            .capture_action(
                cx.listener(|bar, _: &text_input::Redo, _, cx| bar.step_removal(true, cx)),
            )
            .capture_action(cx.listener(|bar, _: &text_input::Backspace, window, cx| {
                bar.leave_at_start(window, cx)
            }))
            .capture_action(
                cx.listener(|bar, _: &text_input::DeleteToStart, window, cx| {
                    bar.leave_at_start(window, cx)
                }),
            )
            .capture_action(
                cx.listener(|bar, _: &text_input::DeleteWordBack, window, cx| {
                    bar.leave_at_start(window, cx)
                }),
            )
            .on_action(cx.listener(|bar, _: &Next, window, cx| bar.select_next(true, window, cx)))
            .on_action(
                cx.listener(|bar, _: &Previous, window, cx| bar.select_next(false, window, cx)),
            )
            .on_action(cx.listener(|bar, _: &First, window, cx| bar.select(0, window, cx)))
            .on_action(cx.listener(|bar, _: &Last, window, cx| {
                bar.select(bar.rows.len().saturating_sub(1), window, cx)
            }))
    }

    /// The chip before the input that names the mode it's in, if it's in one (`mode_name`).
    fn mode_chip(&self, palette: &palette::Palette) -> Option<gpui::Div> {
        let name = self.mode_name()?;
        let page = self.ask.as_ref().and_then(|ask| ask.page.as_ref());
        let favicon = page.map(|(url, _)| self.favicon(url, CHIP_FAVICON, palette.ink.into()));
        // A site search with a colour of its own is in it.
        let colored = self.site.as_ref().is_some_and(|site| site.color.is_some());
        let (fill, label) = match colored {
            true => self.accent(&palette),
            false => (palette.plate(CHIP), palette.ink.into()),
        };
        Some(mode_chip(fill, label, favicon, name))
    }

    /// The rows, `top` under the input's row, as many as show before the list scrolls.
    fn list(&self, rows: Vec<AnyElement>, top: f32) -> gpui::Stateful<gpui::Div> {
        let shown = rows.len().min(ROWS_SHOWN) as f32;
        div()
            .id("suggestions")
            .when(self.a11y, |list| {
                list.role(Role::ListBox).aria_label("Suggestions")
            })
            .mt(px(top))
            .mx(px(LIST_X))
            .h(px(shown * ROW_HEIGHT))
            .flex_none()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(rows)
    }
}

/// A resting bar's parts and state, for the page it rests in (#4140).
impl CommandBar {
    pub fn surface(&self) -> Surface {
        self.surface
    }

    /// What its field takes the keyboard by.
    pub fn field_focus(&self, cx: &App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }

    /// What the user typed, without a completion still selected after it.
    pub fn typed_text(&self, cx: &App) -> String {
        self.typed(cx)
    }

    /// Whether ⇥ grew it into the composer.
    #[cfg(test)]
    pub fn composing_thread(&self) -> bool {
        self.ask.is_some()
    }

    /// The assistant the composer would start its thread on, by id.
    #[cfg(test)]
    pub fn composer_agent(&self) -> Option<SharedString> {
        self.composer_assistant().map(|agent| agent.id.clone())
    }

    /// Its field.
    #[cfg(test)]
    pub fn field(&self) -> Entity<TextInput> {
        self.input.clone()
    }

    /// What its field says with nothing typed.
    #[cfg(test)]
    pub fn placeholder(&self, cx: &App) -> SharedString {
        self.input.read(cx).placeholder().clone()
    }

    /// The resting bar's page is another empty Space's now. A composer open in the Space before
    /// doesn't carry over, since that Space may be another profile's, whose remembered level is
    /// its own: the bar is search again with the words kept, as ⌘T's keeps them on a change of
    /// Space, and the level and model picked go with the composer.
    pub fn space_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_ask(window, cx);
    }

    /// What the browser knows of the agents changed, as after it learned which are ready: a
    /// resting bar that has read them reads them again, so its New Thread row and composer show
    /// the ones found. Whether they changed.
    pub fn agents_changed(&mut self, cx: &mut Context<Self>) -> bool {
        if self.task_agents.is_none() {
            return false;
        }
        let agents = self.learned_agents(cx);
        if agents == self.task_agents {
            return false;
        }
        self.task_agents = agents;
        if self.ask.is_some() {
            self.log_agents();
        }
        cx.notify();
        true
    }
}

impl CommandBar {
    /// Logs the time from ⌘T to the bar's first frame on screen, once an open.
    fn log_first_frame(&mut self) {
        if let Some(opened) = self.opened_at.take() {
            // The ⌘T gate reads this number.
            log!(
                "browser: command bar first frame in {:.1} ms",
                opened.elapsed().as_secs_f64() * 1000.
            );
        }
    }
}

impl Render for CommandBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let in_place = self.surface == Surface::InPlace;
        // check.sh reads these lines. A resting bar draws with every frame of its page.
        if !in_place {
            log!(
                "browser: command bar drew \"{}\"",
                self.input.read(cx).text()
            );
        }
        if window.is_visible() {
            self.log_first_frame();
        } else {
            self.opened_drawn = self.opened_at.is_some();
        }
        let remove = self.hovered.and_then(|index| {
            let row = self.rows.get(index).filter(|row| row.removable)?;
            Some((index, row.url.clone()))
        });
        if remove != self.remove_drawn {
            if let Some((index, url)) = &remove {
                log!("browser: command bar drew remove on row {index} {url}");
            }
            self.remove_drawn = remove;
        }
        self.a11y = window.is_a11y_active();
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| self.row(index, row, cx).into_any_element())
            .collect();
        let (radius, input_height, list_top, level) = match self.over_field {
            true => (FIELD_RADIUS, FIELD_INPUT, 0., FIELD_LEVEL),
            false => (RADIUS, HEIGHT, LIST_TOP, Level::Modal),
        };
        // Over the field the input's icon and text are in the rows' columns.
        let (icon_x, icon, text_x, text_size) = match self.over_field {
            true => {
                let icon_x = LIST_X + ICON_BOX_X + (ICON_BOX - ICON_SIZE) / 2.;
                (icon_x, ICON_SIZE, LIST_X + TITLE_X, FIELD_INPUT_TEXT)
            }
            false => (INPUT_ICON_X, INPUT_ICON, INPUT_TEXT_X, INPUT_TEXT),
        };
        // One device pixel, which a 0.5 pt line isn't on a 1× display, where it doesn't draw.
        let hairline = 1. / window.scale_factor();
        let edge = |inset: f32, width: f32, color: Hsla| {
            div()
                .absolute()
                .top(px(inset))
                .left(px(inset))
                .right(px(inset))
                .bottom(px(inset))
                .rounded(px(radius - inset))
                .border(px(width))
                .border_color(color)
        };
        let palette = palette::current(cx);
        // The placeholder and the accent follow the Space's, which may have changed since it opened.
        self.input.update(cx, |input, _| {
            input.set_colors(
                palette.on_overlay(PLACEHOLDER),
                palette.accent.into(),
                palette.selection(),
            );
            input.set_fade(px(FIELD_FADE), palette.overlay.into());
        });
        let panel = self
            .with_keys(div(), cx)
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(radius))
            .bg(palette.overlay)
            .text_color(palette.ink)
            // Arc's text is medium, by its strokes at 2× (Arc 1.170.0 cap).
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(text::BODY))
            .child(
                div()
                    .h(px(input_height))
                    .flex_none()
                    .flex()
                    .items_center()
                    .pl(px(icon_x))
                    .pr(px(HINT_END))
                    .gap(px(text_x - icon_x - icon))
                    .text_size(px(text_size))
                    .line_height(px(INPUT_LINE))
                    .child(self.input_icon(cx))
                    .children(self.mode_chip(&palette))
                    .child(div().flex_1().min_w_0().child(self.input.clone()))
                    .children(self.site_hint(cx)),
            )
            .when(!rows.is_empty() && !self.over_field, |bar| {
                bar.child(
                    div()
                        .h(px(hairline))
                        .mx(px(LINE_INSET))
                        .flex_none()
                        .bg(palette.faint(LINE_COLOR)),
                )
            })
            .when(!rows.is_empty(), |bar| bar.child(self.list(rows, list_top)))
            .when(self.ask.is_some(), |bar| {
                bar.child(self.choices(cx))
                    .child(
                        div()
                            .h(px(hairline))
                            .mx(px(LINE_INSET))
                            .flex_none()
                            .bg(palette.faint(LINE_COLOR)),
                    )
                    .child(self.foot(cx))
            })
            .children(self.a11y.then(|| self.announcer()))
            // The modal level's rim in the palette's ink; its outline and shadow are the host's,
            // round the window, or on Windows round it in the window.
            .child(edge(0., 1., Level::Modal.rim_in(&palette)));
        // At rest no window is round it: it casts the level's shadow on its page itself, and is
        // as tall as what it shows, over what the page has under it.
        if in_place {
            return div()
                .w_full()
                .h(px(self.drawn_height()))
                .font_family(crate::face::UI)
                .child(panel.shadow(Level::Modal.shadow()))
                .into_any_element();
        }
        // The browser's shortcuts work here too, below the bar's own keys.
        div()
            .key_context(shell::KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI)
            .child(level.in_room_of(ROOM, panel))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
