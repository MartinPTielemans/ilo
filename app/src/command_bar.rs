//! The command bar. GPUI can't draw over the page's native view, so the bar is a window of its
//! own that the host shows over the browser window, as its child. Under its input it lists the
//! core's suggestions for what's typed, and the window grows down to fit them. While the first is
//! selected, the input completes what's typed to it, as Chrome's address bar does. Its size and
//! rows are Arc's (docs/ux-spec.md › Command bar). Tab on a row that hints a search engine, or a
//! space after the engine's keyword, searches only with that engine until ⌫ leaves, as Chrome's
//! keyword mode does. A click on the sidebar's URL field opens a narrower panel over the field,
//! as Arc's does. What it asks of the core and of its window goes through `Host`.

use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc, time::Instant};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, Focusable, FontWeight, Global, HoverListenerMode,
    Hsla, Image, KeyBinding, Pixels, ScrollHandle, SharedString, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, actions, div,
    img, point, prelude::*, px, size, svg,
};
use gpui::{Role, accesskit::Live};

use crate::{
    palette,
    shell::{self, Match},
    text_input::{self, PasteAndGo, PasteAndSearch, TextInput, without_javascript},
};

mod calculator;

actions!(
    command_bar,
    [
        Go, GoBeside, GoBehind, GoDotCom, GoWindow, GoDotComWindow, Search, Dismiss, Next,
        Previous, First, Last, Remove, EnterSite, LeaveSite
    ]
);

pub use crate::tokens::command_bar::WIDTH;
use crate::tokens::command_bar::HEIGHT;
use crate::tokens::command_bar::LINE;
use crate::tokens::command_bar::LINE_INSET;
use crate::tokens::command_bar::LIST_TOP;
use crate::tokens::command_bar::LIST_END;
/// The rows are 6 in from the panel's sides, 50 tall, and the bar shows five before it scrolls.
use crate::tokens::command_bar::LIST_X;
use crate::tokens::command_bar::ROW_HEIGHT;
const ROWS_SHOWN: usize = 5;
use crate::tokens::command_bar::PLATE_INSET;
use crate::tokens::command_bar::PLATE_RADIUS;
use crate::tokens::command_bar::ROW_END;
use crate::tokens::command_bar::ICON_BOX;
use crate::tokens::command_bar::ICON_BOX_X;
use crate::tokens::command_bar::ICON_SIZE;
use crate::tokens::command_bar::TITLE_X;
use crate::tokens::command_bar::INPUT_ICON;
use crate::tokens::command_bar::INPUT_TEXT;
use crate::tokens::command_bar::INPUT_ICON_X;
use crate::tokens::command_bar::INPUT_TEXT_X;
use crate::tokens::command_bar::HINT_END;
use crate::tokens::command_bar::HINT_GAP;
pub use crate::tokens::command_bar::RADIUS;
pub use crate::tokens::command_bar::ABOVE_MIDDLE;
pub use crate::tokens::command_bar::FIELD_WIDTH;
use crate::tokens::command_bar::FIELD_ABOVE;
pub use crate::tokens::command_bar::FIELD_RADIUS;
/// There the input's row is a row tall, with no line under it, and its icon and text sit in
/// the rows' icon and title columns.
const FIELD_INPUT: f32 = ROW_HEIGHT;
/// There it hangs from the field as the Site Controls do. The modal shadow, 24 down at a 64 blur,
/// fell mostly below a panel only 50 tall, a grey bar under it over a light page (#1306).
const FIELD_LEVEL: Level = Level::Popover;
use crate::tokens::command_bar::FIELD_INPUT_TEXT;
use crate::tokens::command_bar::BESIDE_END;
use crate::elevation::Level;
use crate::tokens::command_bar::LINE_COLOR;
use crate::tokens::command_bar::TITLE;
use crate::tokens::command_bar::DIM;
use crate::tokens::command_bar::HOVERED;
use crate::tokens::command_bar::PLACEHOLDER;
use crate::tokens::command_bar::ROW_GLYPH;
use crate::tokens::command_bar::ARROW;
use crate::tokens::command_bar::SELECTED_DIM;
use crate::tokens::command_bar::CHIP;
use crate::tokens::command_bar::ARROW_TILE;
use crate::tokens::command_bar::CHIP_RADIUS;
use crate::tokens::text;
pub(crate) const PLACEHOLDER_TEXT: &str = "Search or Enter URL…";
/// What its input says for Search Tabs (ours, after the command's name).
const TABS_PLACEHOLDER: &str = "Search Tabs…";
/// What its input says, and its chip, while it lists only commands (ours).
const COMMANDS_PLACEHOLDER: &str = "Search Commands…";
const COMMANDS_CHIP: &str = "Commands";

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
    /// Whether ⇧⌦, ⌥⌘⌫ or its ✕ can take it out of the profile's history: a page visited or a
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
    None,
}

/// The glyph a command shares with the others no Lucide icon plainly names.
const COMMAND_GLYPH: &str = "icons/command.svg";

impl Suggestion {
    fn kind(&self) -> Kind {
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
        match self.kind() {
            Kind::Page if self.tab == 0 && self.removable => Icon::Glyph("icons/clock.svg"),
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
        Command::FocusSidebar => "icons/keyboard.svg",
        Command::NewFolder => "icons/folder-plus.svg",
        Command::TogglePin | Command::PinToSpace(_) => "icons/pin.svg",
        Command::OpenFolder(_) => "icons/folder-open.svg",
        Command::SelectSpace(_) => "icons/layers.svg",
        Command::MoveToSpace(_) => "icons/arrow-right.svg",
        Command::ToggleMute => "icons/volume-2.svg",
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
        Command::ZoomIn => "icons/zoom-in.svg",
        Command::ZoomOut => "icons/zoom-out.svg",
        Command::FlipSplit => "icons/arrow-left-right.svg",
        Command::ToggleSidebar => "icons/panel-left.svg",
        Command::CapturePortion | Command::CaptureFullPage => "icons/camera.svg",
        Command::SavePage => "icons/save.svg",
        Command::Print => "icons/printer.svg",
        Command::ViewSource => "icons/code.svg",
        Command::ToggleDeveloperMode => "icons/square-code.svg",
        Command::InspectElements => "icons/square-mouse-pointer.svg",
        Command::JavaScriptConsole => "icons/terminal.svg",
        Command::RecordTrace => "icons/activity.svg",
        Command::ManageExtensions | Command::AddExtension => "icons/puzzle.svg",
        Command::RenameSpace | Command::RenameTab => "icons/pencil.svg",
        Command::EditTheme => "icons/palette.svg",
        Command::ExportSpace => "icons/file.svg",
        Command::NextTab => "icons/arrow-down.svg",
        Command::PreviousTab => "icons/arrow-up.svg",
        Command::ViewLibrary => "icons/library.svg",
        Command::ViewDownloads => "icons/arrow-down-to-line.svg",
        Command::ViewMedia => "icons/image.svg",
        Command::ViewHistory => "icons/rotate-ccw-clock.svg",
        Command::ClearBrowsingData => "icons/brush-cleaning.svg",
        Command::NewNote | Command::NewNoteSplit => "icons/file.svg",
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
        Command::CopyBuildInfo => "icons/copy.svg",
        _ => COMMAND_GLYPH,
    }
}

impl From<shell::BarTab> for Suggestion {
    fn from(tab: shell::BarTab) -> Self {
        Suggestion { url: tab.url.to_string(), text: tab.title, tab: tab.tab, ..Default::default() }
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
    /// Takes the suggestion going to `url` out of the profile's history. The suggestions come
    /// again without it.
    fn remove_suggestion(&self, url: &str);
    /// The page the address bar goes to for `text` typed in it: the one it names, else a search.
    fn typed_url(&self, text: &str) -> Option<String>;
    /// A search for `text` with the search engine whose keyword is `site`, else the default one,
    /// even if it names a page.
    fn search_url(&self, site: Option<&str>, text: &str) -> Option<String>;
    /// Asks for the favicon the profile's history has for the page at `url`, which comes back
    /// through `page_favicon`.
    fn page_favicon(&self, url: &str);
    /// Opens a tab loading `url` at the top of the Space's Today tabs, and shows it unless
    /// `behind`, when it loads only once it's first shown.
    fn open_tab(&self, url: &str, behind: bool);
    /// Opens a browser window after the one the bar shows over, as ⌘N does, and shows a tab
    /// loading `url` in it. The user's: agents get none, as it opens a window (decision 8).
    fn open_window(&self, url: &str, cx: &mut App);
    fn navigate(&self, tab: u64, url: &str);
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
    fn open_split(&self, url: &str, with: u64, cx: &mut App);
    /// Copies `text`, a sum's result, and says so.
    fn copy(&self, text: &str, cx: &mut App);
    /// Shows the bar's window, `width` wide, its corners rounded by `radius` and shadowed as
    /// `level` is, with the keyboard: centred across the browser window with its top `top` above
    /// the window's middle, else at `left` and `top` in it.
    fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32, level: Level);
    fn set_height(&self, height: f32);
    /// Hides the bar's window once the user has chosen something in it.
    fn hide(&self);
    /// Hides the bar's window as the user leaves it without choosing anything.
    fn dismiss(&self);
    /// Gives the bar's window the keyboard back, which it found it hasn't while it shows, unless
    /// the user left it, which the host knows first.
    fn take_keyboard(&self);
    /// The browser's commands that match `text` and do something now, best first.
    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand>;
}

/// A search engine's site search, which the bar's searches stay in until the user leaves it, as in
/// Chrome's keyword mode.
struct Site {
    keyword: String,
    name: SharedString,
    /// Tab entered it and nothing has been typed since, so leaving puts the keyword back without
    /// the space after it, as Chrome does.
    tabbed: bool,
}

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
}

pub struct CommandBar {
    host: Rc<dyn Host>,
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
    /// The row whose ✕ was last drawn, and its page, for check.sh's line as it draws.
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
    /// The rows' favicons by page, asked for as they first show. None until one comes.
    favicons: HashMap<String, Option<Arc<Image>>>,
    /// The Space's tabs it lists before anything is typed.
    tabs: Vec<shell::BarTab>,
    scroll: ScrollHandle,
    /// Whether it shows, until it hides or the host reports a click outside it. The host closes
    /// the window at that click's press, before the click reaches the shell, so this stays set
    /// until the report, which comes after.
    open: Cell<bool>,
}

struct CommandBarWindow(WindowHandle<CommandBar>);

impl Global for CommandBarWindow {}

/// Opens the command bar's window, hidden, for `host` to show over the browser window. Opening it
/// ahead of time keeps window setup off the path from the shortcut to the bar on screen.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<CommandBar>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    open_at(host, bounds, false, cx)
}

/// Opens the command bar's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<CommandBar>> {
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
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // The preview's has none, so it hasn't the window buttons the host's takes off.
        titlebar: (!show).then_some(TitlebarOptions {
            title: None,
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
        let input = cx.new(|cx| {
            TextInput::new(PLACEHOLDER_TEXT, cx)
                .placeholder_color(palette::current(cx).on_overlay(PLACEHOLDER))
                .caret_color(palette::current(cx).accent)
                .selection_color(palette::current(cx).selection())
                .paste_and_go()
        });
        window.focus(&input.focus_handle(cx), cx);
        cx.new(|cx| {
            cx.observe_in(&input, window, |bar: &mut CommandBar, _, window, cx| {
                bar.edited(window, cx)
            })
            .detach();
            cx.observe_window_activation(window, |bar: &mut CommandBar, window, cx| {
                bar.activation_changed(window, cx)
            })
            .detach();
            text_input::on_leave(&input.focus_handle(cx), window, cx, |bar, window, cx| {
                bar.keep_focus(window, cx)
            })
            .detach();
            CommandBar {
                host,
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
                favicons: HashMap::new(),
                tabs: Vec::new(),
                scroll: ScrollHandle::new(),
                open: Cell::new(false),
            }
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(CommandBarWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the command bar: {error}");
            None
        }
    }
}

/// Shows the command bar with the keyboard: empty for a new tab, listing `tabs`, or on `editing`'s
/// URL, selected. It suggests nothing for the URL until the text is edited, as Chrome's address
/// bar, and lists `tabs` again once the text is empty. It shows at once: over `field`, the URL
/// field's box in the window, when a click on the field opened it, else centred, as Arc's is for
/// ⌘T and ⌘L.
pub fn show(
    editing: Option<(u64, SharedString)>,
    tabs: Vec<shell::BarTab>,
    field: Option<Bounds<Pixels>>,
    cx: &mut App,
) {
    let target = editing.as_ref().map_or(Target::NewTab, |&(tab, _)| Target::Tab(tab));
    let text = editing.map(|(_, url)| url).unwrap_or_default();
    // check.sh reads these lines.
    let ids: Vec<String> =
        tabs.iter().take(shell::BAR_TABS_SHOWN).map(|tab| tab.tab.to_string()).collect();
    log!("browser: command bar tabs {}", ids.join(" "));
    let place = if field.is_some() { "over the URL field" } else { "centred" };
    log!("browser: command bar opens {place}");
    let Some(host) = reset(target, text, tabs, field.is_some(), cx) else {
        return;
    };
    match field {
        Some(field) => {
            let top = f32::from(field.top()) - FIELD_ABOVE;
            host.show(false, field.left().into(), top, FIELD_WIDTH, FIELD_RADIUS, FIELD_LEVEL);
        }
        None => host.show(true, 0., ABOVE_MIDDLE, WIDTH, RADIUS, Level::Modal),
    }
}

/// Shows the command bar centred and empty for Search Tabs, listing `tabs` alone, as many as it
/// lists rows, and then those that what's typed names.
pub fn show_tabs(tabs: Vec<shell::BarTab>, cx: &mut App) {
    // check.sh reads these lines.
    let ids: Vec<String> =
        tabs.iter().take(shell::BAR_ROWS).map(|tab| tab.tab.to_string()).collect();
    log!("browser: command bar searches tabs {}", ids.join(" "));
    if let Some(host) = reset(Target::Tabs, SharedString::default(), tabs, false, cx) {
        host.show(true, 0., ABOVE_MIDDLE, WIDTH, RADIUS, Level::Modal);
    }
}

/// Shows the command bar, empty, over `pane`, a split view's new pane in the card at `card`, for
/// a page to show there beside `with`. It's centred over the pane, as far into the card as it
/// needs to be, and as far down the pane as it opens down the window for a new tab.
pub fn show_split(with: u64, pane: Bounds<Pixels>, card: Bounds<Pixels>, cx: &mut App) {
    let text = SharedString::default();
    if let Some(host) = reset(Target::Beside(with), text, Vec::new(), false, cx) {
        let left = pane.center().x - px(WIDTH / 2.);
        let left = left.min(card.right() - px(BESIDE_END + WIDTH)).max(card.left());
        // The card is as far in from the window's bottom as from its top.
        let window = f32::from(card.top() + card.bottom());
        let top = centered_top(window) + f32::from(pane.top() - card.top());
        host.show(false, left.into(), top, WIDTH, RADIUS, Level::Modal);
    }
}

/// Puts the bar to `target` with `text` in its input, and `tabs` to list while it's empty, over
/// the URL field if `over_field`. Returns its host, or none if there's no bar.
fn reset(
    target: Target,
    text: SharedString,
    tabs: Vec<shell::BarTab>,
    over_field: bool,
    cx: &mut App,
) -> Option<Rc<dyn Host>> {
    let window = cx.try_global::<CommandBarWindow>()?.0;
    window
        .update(cx, |bar, window, cx| {
            bar.target = target;
            bar.over_field = over_field;
            // The pointer has been elsewhere since the bar last showed.
            bar.hovered = None;
            // Favicons change over time, and a long session shouldn't keep every one it saw. The
            // tabs' are the ones their rows show.
            bar.favicons.clear();
            bar.keep_favicons(&tabs);
            bar.tabs = tabs;
            bar.open.set(true);
            bar.page = text.to_string();
            if !text.is_empty() {
                bar.favicons.insert(bar.page.clone(), None);
                bar.host.page_favicon(&bar.page);
            }
            bar.site_left(cx);
            bar.commands = None;
            let placeholder = match target {
                Target::Tabs => TABS_PLACEHOLDER,
                _ => PLACEHOLDER_TEXT,
            };
            bar.input.update(cx, |input, cx| input.set_placeholder(placeholder.into(), cx));
            bar.asked = text.to_string();
            // Its answer, for nothing typed, is timed from here, not from the last ask (#1072).
            bar.asked_at = Instant::now();
            bar.complete = false;
            bar.completion.clear();
            bar.before = (text.len(), 0);
            // The tabs show at once, in the frame the bar opens with.
            let rows = if text.is_empty() { bar.tab_rows() } else { Vec::new() };
            bar.set_rows(String::new(), rows, window, cx);
            bar.input.update(cx, |input, cx| input.reset(text, window, cx));
            bar.keep_focus(window, cx);
            // The last query's suggestions are of no use now.
            bar.host.suggest(None, "", false);
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
    let Some(bar_window) = cx.try_global::<CommandBarWindow>().map(|bar| bar.0) else {
        return false;
    };
    bar_window
        .update(cx, |bar, window, cx| {
            if !bar.open.get() {
                return false;
            }
            if let Target::Beside(_) = bar.target {
                // The shell is in an update, which its dismissal would update again.
                let host = bar.host.clone();
                cx.defer(move |_| host.dismiss());
                return false;
            }
            // check.sh reads this line.
            log!("browser: command bar follows the Space");
            let tabs = if matches!(bar.target, Target::Tabs) { open_tabs } else { tabs };
            bar.favicons.clear();
            bar.keep_favicons(&tabs);
            bar.tabs = tabs;
            // The site search's engine is the profile's, which this one may not have.
            bar.leave_site(window, cx);
            let mut text = bar.input.read(cx).text().to_string();
            if let Target::Tab(_) = bar.target {
                bar.target = Target::NewTab;
                if text == bar.page {
                    text.clear();
                }
                bar.page.clear();
            }
            if bar.commands.is_some() {
                // Which commands do something now is the shell's to say, once out of its update.
                cx.defer(move |cx| {
                    bar_window.update(cx, |bar, window, cx| bar.list_commands(text, window, cx)).ok();
                });
            } else if matches!(bar.target, Target::Tabs) {
                bar.name_tabs(text, window, cx);
            } else {
                // A completion from the profile before goes with its rows.
                let typed = bar.typed(cx);
                let text = if bar.completion.is_empty() { text } else { typed };
                let caret = text.len();
                let shown = text.clone().into();
                bar.input.update(cx, |input, cx| input.set_text(shown, caret, window, cx));
                bar.ask_for(text, window, cx);
            }
            cx.notify();
            matches!(bar.target, Target::NewTab)
        })
        .unwrap_or(false)
}

/// The user left the bar with a click outside it, or Esc.
pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<CommandBarWindow>().map(|bar| bar.0) {
        window.update(cx, |bar, _, _| bar.open.set(false)).ok();
    }
}

/// Whether the bar shows, for a click on what opens it to close it rather than open it again: the
/// click outside the bar has closed it by now, but the shell hears so only after the click's press.
pub fn is_open(cx: &App) -> bool {
    cx.try_global::<CommandBarWindow>()
        .and_then(|bar| bar.0.read(cx).ok())
        .is_some_and(|bar| bar.open.get())
}

/// The favicon `Host::page_favicon` asked for, for a row going to `url`.
pub fn page_favicon(url: &str, favicon: Option<Arc<Image>>, cx: &mut App) {
    let Some(window) = cx.try_global::<CommandBarWindow>().map(|bar| bar.0) else {
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

/// Where the centred panel's top is, down a browser window `height` tall.
pub fn centered_top(height: f32) -> f32 {
    height / 2. - ABOVE_MIDDLE
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
    let Some(window) = cx.try_global::<CommandBarWindow>().map(|bar| bar.0) else {
        return;
    };
    window
        .update(cx, |bar, window, cx| {
            // Older text's come after the bar has asked about newer, and Search Tabs asks for none.
            // The commands-only list asks for none either.
            if text != bar.request() || matches!(bar.target, Target::Tabs) || bar.commands.is_some()
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
            // A site search lists only its searches, and nothing typed lists the Space's tabs.
            let mut rows = match bar.site {
                Some(_) => rows,
                None if text.is_empty() => bar.tab_rows(),
                None => {
                    let mut rows = rows;
                    let open = bar.take_open_tab(&text, &mut rows);
                    let mut rows = with_calculation(&text, bar.with_commands(&text, rows, cx));
                    rows.splice(0..0, open);
                    with_new_document(&text, rows)
                }
            };
            rows.truncate(shell::BAR_ROWS);
            // The core doesn't say which are searches, so they're the rows that go where a search
            // for their words does.
            let site = bar.site.as_ref().map(|site| site.keyword.as_str());
            for row in &mut rows {
                row.search = row.command.is_none()
                    && row.result.is_none()
                    && bar.host.search_url(site, &row.text).is_some_and(|url| url == row.url);
            }
            bar.set_rows(bar.asked.clone(), rows, window, cx);
        })
        .ok();
}

/// The clipboard's text as a paste puts it in the input: on one line, without a leading
/// `javascript:`, trimmed. None if it has none.
fn pasted(cx: &App) -> Option<String> {
    let text = cx.read_from_clipboard()?.text()?.replace('\n', " ");
    Some(without_javascript(&text).trim().to_string()).filter(|text| !text.is_empty())
}

/// Whether a word of `tab`'s title, or of its site's name, starts with `typed`, lowercase. A site's
/// name is its host's labels without the last, the top-level domain, and without a leading www.
fn names_tab(tab: &shell::BarTab, typed: &str) -> bool {
    let title = tab.title.to_lowercase();
    let word_start = |&(at, c): &(usize, char)| {
        c.is_alphanumeric() && !title[..at].chars().next_back().is_some_and(char::is_alphanumeric)
    };
    if (title.char_indices().filter(word_start)).any(|(at, _)| title[at..].starts_with(typed)) {
        return true;
    }
    // Only a web page has a site: a data: page's scheme isn't its name.
    let Some((_, rest)) = tab.url.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default().to_lowercase();
    let labels: Vec<&str> = host.strip_prefix("www.").unwrap_or(&host).split('.').collect();
    let named = &labels[..labels.len().saturating_sub(1).max(1)];
    named.iter().flat_map(|label| label.split('-')).any(|word| word.starts_with(typed))
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
        if self.commands.is_some() {
            return self.list_commands(text, window, cx);
        }
        // Chrome completes only typing at the end: not a deletion, a paste, or with the caret
        // elsewhere, and not in a site search.
        let deleted = text.len() < length && selection.start <= start;
        let typing = !deleted && !input.pasted() && selection == (text.len()..text.len());
        let complete = typing && self.site.is_none();
        // "/ " typed lists only the commands, as Arc's bar does.
        let slashed = typing && self.asked == "/" && text.strip_suffix([' ', '\u{3000}']) == Some("/");
        if slashed && self.site.is_none() && self.lists_commands() {
            return self.enter_commands("/ ", window, cx);
        }
        // A space typed after the keyword the first row hints enters its engine's site search, or
        // an ideographic one, as Japanese input types, as in Chrome.
        let hint = self.rows.first().filter(|row| {
            self.site.is_none()
                && self.suggested == self.asked
                && !row.keyword.is_empty()
                && row.keyword == self.asked
        });
        let spaced = typing && text.strip_suffix([' ', '\u{3000}']) == Some(self.asked.as_str());
        if let Some(row) = hint.filter(|_| spaced) {
            let (keyword, name) = (row.keyword.clone(), row.site.clone());
            return self.enter_site(keyword, name, String::new(), false, window, cx);
        }
        if let Some(site) = self.site.as_mut().filter(|_| !text.is_empty()) {
            site.tabbed = false;
        }
        // While what's typed follows the completion, the rest of it stays until the core answers,
        // so it doesn't flicker.
        let completion = shown
            .get(text.len()..)
            .filter(|_| complete)
            .filter(|_| {
                shown.get(..text.len()).is_some_and(|typed| typed.eq_ignore_ascii_case(&text))
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
        let site = self.site.as_ref().map(|site| site.keyword.as_str());
        self.host.suggest(site, &self.asked, complete);
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
        log!("browser: command bar finds tabs \"{typed}\": {}", ids.join(" "));
        self.asked = text.clone();
        self.set_rows(text, rows, window, cx);
    }

    /// Keeps the favicons `tabs` have, for their rows.
    fn keep_favicons(&mut self, tabs: &[shell::BarTab]) {
        for tab in tabs {
            if let Some(favicon) = &tab.favicon {
                self.favicons.insert(tab.url.to_string(), Some(favicon.clone()));
            }
        }
    }

    /// Whether the bar lists commands: not over a new pane or for Search Tabs.
    fn lists_commands(&self) -> bool {
        matches!(self.target, Target::NewTab | Target::Tab(_))
    }

    /// Lists only the commands, entered by Tab in an empty field or by `by`, "/ ", typed, as
    /// Arc's bar does after Tab right after ⌘T.
    fn enter_commands(&mut self, by: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: command bar lists commands by {}", if by.is_empty() { "tab" } else { "slash" });
        self.commands = Some(by);
        self.completion.clear();
        let placeholder = COMMANDS_PLACEHOLDER.into();
        self.input.update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        self.input.update(cx, |input, cx| input.set_text(SharedString::default(), 0, window, cx));
        self.list_commands(String::new(), window, cx);
    }

    /// Lists the commands that match `text`, or with nothing typed every one that does something
    /// now.
    fn list_commands(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let rows: Vec<Suggestion> =
            self.host.commands(&text, cx).into_iter().map(Suggestion::from).collect();
        // check.sh reads this line.
        let names: Vec<&str> = rows.iter().map(|row| row.text.as_ref()).collect();
        log!("browser: command bar commands only \"{text}\": {}", names.join(", "));
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
        self.input.update(cx, |input, cx| input.set_placeholder(PLACEHOLDER_TEXT.into(), cx));
        let text = format!("{by}{}", self.input.read(cx).text());
        self.input.update(cx, |input, cx| input.set_text(text.clone().into(), by.len(), window, cx));
        self.ask_for(text, window, cx);
        true
    }

    /// Lists the rows for `text` in the input from the start: the Space's tabs for none, else the
    /// core's suggestions once they come, and none until then.
    fn ask_for(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.asked = text;
        self.asked_at = Instant::now();
        self.complete = false;
        self.completion.clear();
        let rows = if self.asked.is_empty() { self.tab_rows() } else { Vec::new() };
        self.set_rows(String::new(), rows, window, cx);
        if !self.asked.is_empty() {
            let site = self.site.as_ref().map(|site| site.keyword.as_str());
            self.host.suggest(site, &self.asked, false);
        }
    }

    /// What the core was last asked for, as its suggestions name it: what was typed, after the
    /// site search's keyword and a space.
    fn request(&self) -> String {
        match &self.site {
            Some(site) => format!("{} {}", site.keyword, self.asked),
            None => self.asked.clone(),
        }
    }

    /// Enters the site search of the engine whose keyword is `keyword`, by Tab if `tabbed`, else
    /// by Space, with `query` in the input.
    fn enter_site(
        &mut self,
        keyword: String,
        name: SharedString,
        query: String,
        tabbed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // check.sh reads this line.
        let by = if tabbed { "tab" } else { "space" };
        log!("browser: command bar site search {keyword} by {by}");
        let placeholder = format!("Search {name}").into();
        self.input.update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        self.site = Some(Site { keyword, name, tabbed });
        self.completion.clear();
        let caret = query.len();
        self.input.update(cx, |input, cx| input.set_text(query.into(), caret, window, cx));
        cx.notify();
    }

    /// Leaves the site search, if the bar is in one, putting its keyword back before what was
    /// typed, with the caret after it, as Chrome does. Returns whether it was in one.
    fn leave_site(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(site) = self.site_left(cx) else {
            return false;
        };
        // check.sh reads this line.
        log!("browser: command bar leaves site search");
        let space = if site.tabbed { "" } else { " " };
        let prefix = format!("{}{space}", site.keyword);
        let text = format!("{prefix}{}", self.input.read(cx).text());
        self.input.update(cx, |input, cx| input.set_text(text.into(), prefix.len(), window, cx));
        cx.notify();
        true
    }

    /// Takes the bar out of its site search, if it's in one, and returns it.
    fn site_left(&mut self, cx: &mut Context<Self>) -> Option<Site> {
        let site = self.site.take()?;
        self.input.update(cx, |input, cx| input.set_placeholder(PLACEHOLDER_TEXT.into(), cx));
        Some(site)
    }

    /// Tab: enters the site search the selected row hints, keeping what was typed after its first
    /// word, the keyword, as Chrome does. Else it selects the next row.
    /// In an empty field, it lists only the commands, as Arc's does.
    fn enter_site_or_next(&mut self, _: &EnterSite, window: &mut Window, cx: &mut Context<Self>) {
        let empty = self.input.read(cx).text().is_empty();
        if empty && self.site.is_none() && self.commands.is_none() && self.lists_commands() {
            return self.enter_commands("", window, cx);
        }
        let hint = self.chosen().filter(|row| !row.keyword.is_empty() && self.site.is_none());
        let Some((keyword, name)) = hint.map(|row| (row.keyword.clone(), row.site.clone())) else {
            return self.select_next(true, window, cx);
        };
        let typed = self.typed(cx);
        let query = typed.split_once(char::is_whitespace).map_or("", |(_, rest)| rest.trim_start());
        self.enter_site(keyword, name, query.to_string(), true, window, cx);
    }

    /// ⇧Tab: leaves the site search or the commands-only list, else selects the row before.
    fn leave_site_or_previous(
        &mut self,
        _: &LeaveSite,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.leave_site(window, cx) && !self.leave_commands(window, cx) {
            self.select_next(false, window, cx);
        }
    }

    /// ⌫, ⌘⌫ or ⌥⌫ with the caret at the start of a site search's input, or the commands-only
    /// list's, leaves it.
    fn leave_at_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).selection() == (0..0)
            && (self.leave_site(window, cx) || self.leave_commands(window, cx))
        {
            cx.stop_propagation();
        }
    }

    /// Where ↩ goes for `text` before its suggestions come: the page it names, else a search for
    /// it, or in a site search, a search with that site's engine.
    /// None for Search Tabs, which goes nowhere but its tabs, and the commands-only list, which
    /// runs only its commands.
    fn typed_url(&self, text: &str) -> Option<String> {
        match &self.site {
            _ if matches!(self.target, Target::Tabs) || self.commands.is_some() => None,
            Some(site) => self.host.search_url(Some(&site.keyword), text),
            None => self.host.typed_url(text),
        }
    }

    /// Shows `completion` after what was typed, selected, or no completion.
    fn show_completion(
        &mut self,
        completion: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if completion == self.completion {
            return;
        }
        self.completion = completion;
        // check.sh reads this line.
        log!(
            "browser: command bar completes \"{}\" with \"{}\"",
            self.asked, self.completion
        );
        let text = format!("{}{}", self.asked, self.completion);
        let typed = self.asked.len();
        self.input.update(cx, |input, cx| input.complete(text.into(), typed, window, cx));
    }

    /// Completes what was typed to the first row while it's selected, unless the user has moved
    /// the caret.
    fn update_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input.read(cx);
        let (typed, end) = (self.asked.len(), self.asked.len() + self.completion.len());
        let selection = if self.completion.is_empty() { end..end } else { typed..end };
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
            0 => self.rows.first().map(|row| row.completion.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        self.show_completion(completion, window, cx);
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
        if commands.is_empty() {
            return rows;
        }
        // check.sh reads these lines.
        let names: Vec<&str> = commands.iter().map(|command| command.name.as_ref()).collect();
        log!("browser: command bar commands \"{text}\": {}", names.join(", "));
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
        merged.extend(rows);
        merged.extend(commands.map(Suggestion::from));
        merged
    }

    /// Takes out of `rows` the open tab that goes first, for its row to go above the rest with
    /// Switch to Tab, as Arc's bar is first of all a tab switcher (ours: its ranking isn't
    /// recorded). It's the most recently shown whose title or site has a word that two letters or
    /// more typed start. Chrome's first row stays when the typing reads as a URL or the row hints a
    /// site search, and a first row completing to an open tab's page becomes that tab's row.
    fn take_open_tab(&self, text: &str, rows: &mut Vec<Suggestion>) -> Option<Suggestion> {
        let first = rows.first();
        if first.is_some_and(|row| !row.keyword.is_empty()) {
            return None;
        }
        if let Some(first) = first.filter(|row| !row.completion.is_empty()) {
            let at_page = |tab: &&shell::BarTab| *tab.url == *first.url && !self.is_shown(tab.tab);
            let open = self.tabs.iter().find(at_page)?;
            let id = open.tab;
            // check.sh reads this line.
            log!("browser: command bar puts tab {id} first");
            rows[0].tab = id;
            let mut at = 0;
            rows.retain(|row| {
                at += 1;
                at == 1 || row.tab != id
            });
            return None;
        }
        let typed = text.trim().to_lowercase();
        if typed.chars().count() < 2 || typed.contains(['.', ':', '/']) {
            return None;
        }
        let named = |tab: &&shell::BarTab| !self.is_shown(tab.tab) && names_tab(tab, &typed);
        let tab = self.tabs.iter().find(named)?;
        // check.sh reads this line.
        log!("browser: command bar puts tab {} first", tab.tab);
        Some(match rows.iter().position(|row| row.tab == tab.tab) {
            Some(at) => rows.remove(at),
            None => tab.clone().into(),
        })
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
        // The page selected stays selected as more suggestions come for what was asked, and when
        // it's taken out, the row that takes its place is, as in Chrome.
        let selected = self.rows.get(self.selected).filter(|_| text == self.suggested);
        self.selected = match selected {
            Some(old) => rows
                .iter()
                .position(|row| row.url == old.url && row.command == old.command)
                .unwrap_or(self.selected.min(rows.len().saturating_sub(1))),
            None => 0,
        };
        self.rows = rows;
        self.suggested = text;
        let rows = self.rows.len();
        self.host.set_height(if self.over_field { field_height(rows) } else { height(rows) });
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

    /// The row ↩ takes, once there are rows for what's typed. A completion comes from the first
    /// row, even before the rows for the last key arrive.
    fn chosen(&self) -> Option<&Suggestion> {
        let current = self.suggested == self.asked || !self.completion.is_empty();
        self.rows.get(self.selected).filter(|_| current)
    }

    /// What the user typed, without a completion still selected after it.
    fn typed(&self, cx: &App) -> String {
        let input = self.input.read(cx);
        let completing = !self.completion.is_empty()
            && input.selection() == (self.asked.len()..input.text().len());
        let typed = if completing { self.asked.as_str() } else { input.text() };
        typed.trim().to_string()
    }

    fn go(&mut self, _: &Go, _: &mut Window, cx: &mut Context<Self>) {
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
            self.close();
            return self.host.reload(tab);
        }
        // Before anything is typed, that's one of the Space's tabs.
        if self.chosen().is_some() {
            self.take(self.selected, cx);
            return;
        }
        // Before the suggestions for what's typed come, ↩ goes where the first of them will.
        if let Some(url) = self.typed_url(&text).filter(|_| !text.is_empty()) {
            self.go_to(&url, cx);
        }
    }

    /// Paste and Go, from the input's menu: goes where ↩ would for the clipboard's text, as
    /// Chrome does.
    fn paste_and_go(&mut self, _: &PasteAndGo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(url) = pasted(cx).and_then(|text| self.typed_url(&text)) {
            self.go_to(&url, cx);
        }
    }

    /// Paste and Search: searches for the clipboard's text, even if it names a page, as ⇧⌘↩
    /// does for what's typed.
    fn paste_and_search(&mut self, _: &PasteAndSearch, _: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.target, Target::Tabs) || self.commands.is_some() {
            return;
        }
        let site = self.site.as_ref().map(|site| site.keyword.as_str());
        if let Some(url) = pasted(cx).and_then(|text| self.host.search_url(site, &text)) {
            self.go_to(&url, cx);
        }
    }

    /// Shows `url` where the bar shows the page chosen, and closes the bar.
    fn go_to(&self, url: &str, cx: &mut App) {
        // check.sh reads this line.
        log!("browser: command bar go {url}");
        self.show_page(url, cx);
        self.close();
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
            self.host.open_tab(&url, true);
        }
    }

    /// Whether ↩'s row is a command or a sum, or the bar is over a new pane or for Search Tabs,
    /// where the keys that open a page elsewhere take what ↩ takes.
    fn takes_in_place(&self) -> bool {
        let command =
            self.chosen().is_some_and(|row| row.command.is_some() || row.result.is_some());
        command || matches!(self.target, Target::Beside(_) | Target::Tabs)
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
        self.close();
        self.host.open_window(url, cx);
    }

    /// www.<word>.com for a word typed outside a site search, where ⌃↩ goes, as in Chrome.
    fn dot_com(&self, cx: &App) -> Option<String> {
        let typed = self.typed(cx);
        let word = !typed.is_empty() && typed.chars().all(|c| c.is_alphanumeric() || c == '-');
        let word = word && self.site.is_none() && self.commands.is_none();
        let word = word && !matches!(self.target, Target::Tabs);
        word.then(|| self.host.typed_url(&format!("www.{typed}.com"))).flatten()
    }

    /// ⌃↩: goes to www.<word>.com for a word typed, as Chrome does, and else where ↩ goes, as in a
    /// site search.
    fn go_dot_com(&mut self, _: &GoDotCom, window: &mut Window, cx: &mut Context<Self>) {
        match self.dot_com(cx) {
            Some(url) => self.go_to(&url, cx),
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
        if matches!(self.target, Target::Tabs) || self.commands.is_some() {
            return;
        }
        let typed = self.typed(cx);
        let site = self.site.as_ref().map(|site| site.keyword.as_str());
        if let Some(url) = self.host.search_url(site, &typed).filter(|_| !typed.is_empty()) {
            self.go_to(&url, cx);
        }
    }

    /// ⇧⌦, as Chrome's, or ⌥⌘⌫, as Arc's: takes the selected row out of the profile's history.
    fn remove(&mut self, _: &Remove, _: &mut Window, _: &mut Context<Self>) {
        if self.chosen().is_some() {
            self.remove_row(self.selected);
        }
    }

    /// Takes row `index`'s page or search out of the profile's history, if it came from there. The
    /// core lists the rows again without it, and the row under it takes its place.
    fn remove_row(&self, index: usize) {
        let Some(row) = self.rows.get(index).filter(|row| row.removable) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: command bar removes {}", row.url);
        self.host.remove_suggestion(&row.url);
    }

    /// The ✕ a row from history shows under the pointer, as Arc's does, which takes it out. It's
    /// the arrow's tile, so the two read as one kind.
    fn remove_button(&self, index: usize, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let palette = palette::current(cx);
        let (tile, color) = match selected {
            true => (Hsla::from(palette.accent_label), Hsla::from(palette.accent)),
            false => (palette.plate(ARROW_TILE), palette.on_overlay(ARROW)),
        };
        div()
            .id(("remove", index))
            .when(self.a11y, |button| button.role(Role::Button).aria_label("Remove Suggestion"))
            .size(px(ICON_BOX))
            .flex_none()
            .ml(px(7.))
            .mr(px(3.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .bg(tile)
            .child(svg().path("icons/x.svg").size(px(14.)).text_color(color))
            // The click is the ✕'s, not the row's under it.
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |bar, _, _, cx| {
                cx.stop_propagation();
                bar.remove_row(index);
            }))
            .into_any_element()
    }

    /// ⌥↩: goes where ↩ would, in a new tab in a split view with the tab shown, or the tab the
    /// bar edits, on its right (Arc). For Search Tabs it's ↩.
    fn go_beside(&mut self, _: &GoBeside, window: &mut Window, cx: &mut Context<Self>) {
        let with = match self.target {
            Target::Tab(tab) | Target::Beside(tab) => tab,
            Target::NewTab => shell::shown_tab(cx),
            Target::Tabs => 0,
        };
        if with != 0 {
            self.target = Target::Beside(with);
        }
        self.go(&Go, window, cx);
    }

    /// Shows `url` where the bar shows the page chosen.
    fn show_page(&self, url: &str, cx: &mut App) {
        match self.target {
            Target::NewTab | Target::Tabs => self.host.open_tab(url, false),
            Target::Tab(tab) => self.host.navigate(tab, url),
            Target::Beside(with) => self.host.open_split(url, with, cx),
        }
    }

    /// Goes where row `index` goes: to its tab, else its page, where the bar shows the page
    /// chosen. Beside a tab, its tab moves into the split view. A tab another window shows live
    /// brings that window forward with it, as Chrome's Switch to Tab does, rather than taking its
    /// page (Arc unrecorded). A sum's row copies its result.
    fn take(&mut self, index: usize, cx: &mut App) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        if let Some(result) = &row.result {
            // check.sh reads this line.
            log!("browser: command bar copies {result}");
            self.close();
            self.host.copy(result, cx);
            return;
        }
        // check.sh reads these lines.
        if let Some(command) = row.command {
            log!("browser: command bar runs {}", row.text);
            use shell::Command::{AddSplit, EditUrl, NewTab, SearchTabs};
            if !matches!(command, NewTab | SearchTabs | EditUrl | AddSplit) {
                self.close();
                shell::run_from_bar(command, cx);
                return;
            }
            // One that opens the bar again, such as Add Split View over its pane, runs once the
            // bar is done with the key, as it updates the bar's window, which this is in. The bar
            // moves there without going, as hidden and shown at once it would lose the keyboard
            // to the page on X11, and goes only if it didn't open.
            self.open.set(false);
            let host = self.host.clone();
            cx.defer(move |cx| {
                shell::run_from_bar(command, cx);
                if !is_open(cx) {
                    host.hide();
                }
            });
            return;
        }
        match (row.tab, self.target) {
            (0, Target::Tab(_)) if row.new_document => {
                // check.sh reads this line.
                log!("browser: command bar new tab {}", row.url);
                self.host.open_tab(&row.url, false);
            }
            (0, _) => return self.go_to(&row.url, cx),
            (tab, Target::Beside(with)) => {
                log!("browser: command bar split with tab {tab}");
                self.host.split(tab, with, cx);
            }
            (tab, _) => match self.host.tab_window(tab) {
                Some(window) => {
                    log!("browser: command bar switch to tab {tab} in window {window}");
                    self.close();
                    return self.host.activate(window, tab, cx);
                }
                None => {
                    log!("browser: command bar switch to tab {tab}");
                    self.host.select(tab);
                }
            },
        }
        self.close();
    }

    /// Selects the next row, or the one before, wrapping at the ends as Chrome does.
    fn select_next(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.rows.len();
        if count > 0 {
            self.select((self.selected + if forward { 1 } else { count - 1 }) % count, window, cx);
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
        let named = row.command.is_some() || row.result.is_some();
        let name = if named { row.text.as_ref() } else { row.url.as_str() };
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
        cx.notify();
    }

    /// Hides the bar's window.
    fn close(&self) {
        self.open.set(false);
        self.host.hide();
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
            let from = if window.focused(cx).is_some() { "another view" } else { "nothing" };
            log!("browser: command bar's field takes the focus back from {from}");
            window.focus(&field, cx);
        }
    }

    /// ⌘T or ⌘L, which open the bar, close it again, as Arc's do, whichever opened it. The browser's
    /// other shortcuts go on to the browser.
    fn shortcut(&mut self, run: &shell::Run, _: &mut Window, cx: &mut Context<Self>) {
        if !matches!(run.0, shell::Command::NewTab | shell::Command::EditUrl) {
            return cx.propagate();
        }
        // check.sh reads this line.
        log!("browser: command bar closes again on {}", run.0.name());
        self.host.dismiss();
    }

    /// Esc leaves the site search or the commands-only list first, then the bar.
    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if !self.leave_site(window, cx) && !self.leave_commands(window, cx) {
            self.host.dismiss();
        }
    }

    /// `row`'s icon, `size` square, in `color` where it's a glyph.
    fn glyph(&self, row: &Suggestion, size: f32, color: Hsla) -> Option<AnyElement> {
        Some(match row.icon() {
            Icon::Favicon => self.favicon(&row.url, size, color),
            Icon::Glyph(path) => glyph(path, size, color),
            Icon::None => return None,
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
            _ => shell::BAR_TABS_SHOWN,
        };
        self.tabs.iter().take(shown).cloned().map(Suggestion::from).collect()
    }

    /// The input's icon: the search glyph, or once something is typed, the selected row's, as
    /// Arc's becomes its favicon. On a page's URL it's the page's favicon until it's edited.
    fn input_icon(&self, cx: &App) -> AnyElement {
        let color = palette::current(cx).ink.into();
        let size = if self.over_field { ICON_SIZE } else { INPUT_ICON };
        if !self.page.is_empty() && self.asked == self.page {
            return self.favicon(&self.page, size, color);
        }
        let row = self.chosen().filter(|_| !self.asked.is_empty());
        match row.and_then(|row| self.glyph(row, size, color)) {
            Some(icon) => icon,
            None if self.commands.is_some() => glyph(COMMAND_GLYPH, size, color),
            None => glyph("icons/search.svg", size, color),
        }
    }

    /// What the chip before the input names: the site search's engine, or the commands-only list.
    fn mode_name(&self) -> Option<SharedString> {
        match (&self.site, self.commands) {
            (Some(site), _) => Some(site.name.clone()),
            (None, Some(_)) => Some(COMMANDS_CHIP.into()),
            (None, None) => None,
        }
    }

    /// "Search <site>" and the Tab key, at the input's end, while the row ↩ takes hints a site
    /// search.
    fn site_hint(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let row = self.chosen().filter(|row| self.site.is_none() && !row.keyword.is_empty())?;
        let palette = palette::current(cx);
        let key = div()
            .h(px(ICON_BOX))
            .px(px(4.))
            .flex()
            .items_center()
            .rounded(px(CHIP_RADIUS))
            .bg(palette.plate(CHIP))
            .text_color(palette.on_overlay(TITLE))
            .child("Tab");
        Some(
            div()
                .flex_none()
                .ml(px(HINT_GAP))
                .flex()
                .items_center()
                .gap(px(9.))
                .text_size(px(text::BODY))
                .child(div().text_color(palette.on_overlay(DIM)).child(format!("Search {}", row.site)))
                .child(key),
        )
    }

    /// What VoiceOver reads for a row: what it says, then what ↩ does on it when that isn't to
    /// open it.
    fn spoken(&self, row: &Suggestion) -> String {
        let mut spoken = row.text.to_string();
        if !row.detail.is_empty() && !self.over_field {
            spoken = format!("{spoken}, {}", row.detail);
        }
        if row.tab != 0 {
            let action = match self.target {
                Target::Beside(_) => "Add to Split View",
                _ => "Switch to Tab",
            };
            spoken = format!("{spoken}, {action}");
        }
        spoken
    }

    /// The selected row and where it stands, which VoiceOver says as the keys move the selection
    /// while the field keeps the keyboard, as Chrome's address bar does.
    fn announced(&self) -> Option<String> {
        let row = self.rows.get(self.selected)?;
        Some(format!("{}, {} of {}", self.spoken(row), self.selected + 1, self.rows.len()))
    }

    fn row(&self, index: usize, row: &Suggestion, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = index == self.selected;
        // The selected row keeps its accent under the pointer.
        let hovered = !selected && self.hovered == Some(index);
        let palette = palette::current(cx);
        let accent = Hsla::from(palette.accent);
        let label = Hsla::from(palette.accent_label);
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
        let icon = div()
            .size(px(ICON_BOX))
            .mr(px(TITLE_X - ICON_BOX_X - ICON_BOX))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .when_some(tile, |icon, tile| icon.bg(tile))
            .children(self.glyph(row, ICON_SIZE, glyph));
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
            .on_click(cx.listener(move |bar, _, _, cx| bar.take(index, cx)))
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
            .when(row.tab != 0, |line| {
                let action = match self.target {
                    Target::Beside(_) => "Add to Split View",
                    _ => "Switch to Tab",
                };
                let arrow = div()
                    .size(px(ICON_BOX))
                    .flex_none()
                    .mr(px(3.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(CHIP_RADIUS))
                    .bg(if selected { label } else { palette.plate(ARROW_TILE) })
                    .child(
                        svg()
                            .path("icons/arrow-right.svg")
                            .size(px(14.))
                            .text_color(if selected { accent } else { palette.on_overlay(ARROW) }),
                    );
                let tint = if selected { label } else { dim };
                let label = div().flex_none().mr(px(7.)).text_color(tint);
                line.child(div().flex_1()).child(label.child(action)).child(arrow)
            })
            .when_some(row.shortcut.clone(), |line, shortcut| {
                line.child(div().flex_1()).child(chip(shortcut))
            })
            // Under the pointer, the selected row too, as Arc's.
            .when(row.removable && self.hovered == Some(index), |line| {
                line.when(row.tab == 0, |line| line.child(div().flex_1()))
                    .child(self.remove_button(index, selected, cx))
            })
    }
}

impl CommandBar {
    /// A live region that says the selected row as it changes, a polite one, so typing's echo
    /// comes first.
    fn announcer(&self) -> impl IntoElement + use<> {
        div()
            .id("selected-suggestion")
            .absolute()
            .size_0()
            .role(Role::Status)
            .when_some(self.announced(), |announcer, said| announcer.aria_value(said))
            .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
    }
}

/// The glyph at `path`, `size` square, in `color`.
fn glyph(path: &'static str, size: f32, color: Hsla) -> AnyElement {
    svg().path(path).size(px(size)).flex_none().text_color(color).into_any_element()
}

impl Render for CommandBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // check.sh reads these lines.
        log!("browser: command bar drew \"{}\"", self.input.read(cx).text());
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
        let shown = rows.len().min(ROWS_SHOWN) as f32;
        let (radius, input_height, list_top) = match self.over_field {
            true => (FIELD_RADIUS, FIELD_INPUT, 0.),
            false => (RADIUS, HEIGHT, LIST_TOP),
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
            input.set_colors(palette.on_overlay(PLACEHOLDER), palette.accent.into(), palette.selection())
        });
        // The browser's shortcuts work here too, below the bar's own keys.
        div().key_context(shell::KEY_CONTEXT).size_full().font_family(crate::face::UI).child(
            div()
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
                .capture_action(cx.listener(|bar, _: &text_input::Backspace, window, cx| {
                    bar.leave_at_start(window, cx)
                }))
                .capture_action(cx.listener(|bar, _: &text_input::DeleteToStart, window, cx| {
                    bar.leave_at_start(window, cx)
                }))
                .capture_action(cx.listener(|bar, _: &text_input::DeleteWordBack, window, cx| {
                    bar.leave_at_start(window, cx)
                }))
                .on_action(
                    cx.listener(|bar, _: &Next, window, cx| bar.select_next(true, window, cx)),
                )
                .on_action(
                    cx.listener(|bar, _: &Previous, window, cx| bar.select_next(false, window, cx)),
                )
                .on_action(cx.listener(|bar, _: &First, window, cx| bar.select(0, window, cx)))
                .on_action(cx.listener(|bar, _: &Last, window, cx| {
                    bar.select(bar.rows.len().saturating_sub(1), window, cx)
                }))
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
                        .line_height(px(23.))
                        .child(self.input_icon(cx))
                        .when_some(self.mode_name(), |line, name| {
                            line.child(
                                div()
                                    .flex_none()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(CHIP_RADIUS))
                                    .bg(palette.plate(CHIP))
                                    .text_size(px(text::BODY))
                                    .child(name),
                            )
                        })
                        .child(div().flex_1().min_w_0().child(self.input.clone()))
                        .children(self.site_hint(cx)),
                )
                .when(!rows.is_empty() && !self.over_field, |bar| {
                    bar.child(
                        div().h(px(hairline)).mx(px(LINE_INSET)).flex_none().bg(palette.faint(LINE_COLOR)),
                    )
                })
                .when(!rows.is_empty(), |bar| {
                    bar.child(
                        div()
                            .id("suggestions")
                            .when(self.a11y, |list| {
                                list.role(Role::ListBox).aria_label("Suggestions")
                            })
                            .mt(px(list_top))
                            .mx(px(LIST_X))
                            .h(px(shown * ROW_HEIGHT))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .children(rows),
                    )
                })
                .children(self.a11y.then(|| self.announcer()))
                // The modal level's rim in the palette's ink; its outline and shadow are the
                // host's, round the window.
                .child(edge(0., 1., Level::Modal.rim_in(&palette))),
        )
    }
}

#[cfg(test)]
mod tests;
