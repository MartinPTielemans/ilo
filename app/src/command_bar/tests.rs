//! The command bar's keys and completion, on a host that records what the bar asks of it, with the
//! core's suggestions made up.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, ClipboardItem, EntityInputHandler, Modifiers, MouseButton, TestAppContext,
    VisualTestContext, WindowHandle,
};

use super::*;
use crate::text_input::{self, Chosen, FieldMenu, korean};

/// A host that records what the bar asks of it, but its suggestions and its window's height, and
/// apart from the rest, where it shows the window. A text with a dot names a page there; any other
/// is searched for. It has commands only if asked to. The tabs in `elsewhere` are live in the other
/// browser window given.
#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    /// What the core was asked to suggest for, the text and whether it may complete it.
    asked: RefCell<Vec<(String, bool)>>,
    shown: RefCell<Vec<String>>,
    /// Each show's level and the height last asked for as it showed, and every height asked for.
    levels: RefCell<Vec<(Level, Option<f32>)>>,
    heights: RefCell<Vec<f32>>,
    commands: bool,
    /// Commands that do nothing now, so the bar isn't offered them, such as a split's outside one.
    idle: Vec<shell::Command>,
    elsewhere: RefCell<HashMap<u64, u64>>,
    /// The agents ⇥ asks, None while tasks are off, and the page the user is on.
    agents: RefCell<Option<TaskAgents>>,
    page: Option<(SharedString, SharedString)>,
    /// The bar shows over a private window.
    private: bool,
    /// The tooltips asked for and ended, apart from the rest, and where each hangs from.
    tips: RefCell<Vec<String>>,
    anchors: RefCell<Vec<Anchor>>,
    /// What Settings' search reads of its rows, once the browser has built it, and how many
    /// times the bar read it.
    settings: RefCell<Option<Rc<Vec<settings_search::Entry>>>>,
    settings_read: std::cell::Cell<usize>,
    /// The tabs open on the account's other computers.
    others: Vec<OtherTab>,
    /// The archive, newest first: each tab's id, page and when it closed, in milliseconds.
    archive: RefCell<Vec<(u64, String, f64)>>,
    /// Now, in milliseconds since 1970.
    now: f64,
}

impl Recorder {
    fn record(&self, call: String) {
        self.calls.borrow_mut().push(call);
    }
}

fn search(text: &str) -> String {
    format!("https://search.test/?q={text}")
}

impl Host for Recorder {
    fn suggest(&self, _: Option<&str>, text: &str, complete: bool) {
        if !text.is_empty() {
            self.asked.borrow_mut().push((text.into(), complete));
        }
    }

    fn remove_suggestion(&self, url: &str, _: &mut App) {
        self.record(format!("remove {url}"));
    }

    fn step_removal(&self, redo: bool, _: &mut App) -> bool {
        self.record(format!("{} removal", if redo { "redo" } else { "undo" }));
        true
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        Some(if text.contains('.') {
            format!("https://{text}/")
        } else {
            search(text)
        })
    }

    fn search_url(&self, site: Option<&str>, text: &str) -> Option<String> {
        Some(match site {
            Some(site) => format!("https://{site}/?q={text}"),
            None => search(text),
        })
    }

    fn page_favicon(&self, _: &str) {}

    fn open_tab(&self, url: &str, behind: bool) {
        self.record(format!("open {url}{}", if behind { " behind" } else { "" }));
    }

    fn open_window(&self, url: &str, _: &mut App) {
        self.record(format!("window {url}"));
    }

    fn navigate(&self, tab: u64, url: &str) {
        self.record(format!("navigate {tab} {url}"));
    }

    fn reload(&self, tab: u64) {
        self.record(format!("reload {tab}"));
    }

    fn select(&self, tab: u64) {
        self.record(format!("select {tab}"));
    }

    fn tab_window(&self, tab: u64) -> Option<u64> {
        self.elsewhere.borrow().get(&tab).copied()
    }

    fn activate(&self, window: u64, tab: u64, _: &mut App) {
        self.record(format!("activate {window} with {tab}"));
    }

    fn split(&self, tab: u64, with: u64, _: &mut App) {
        self.record(format!("split {tab} with {with}"));
    }

    fn open_split(&self, url: &str, with: u64, _: &mut App) {
        self.record(format!("open {url} beside {with}"));
    }

    fn copy(&self, text: &str, _: &mut App) {
        self.record(format!("copy {text}"));
    }

    fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32, level: Level) {
        let place = match centered {
            true => format!("centred {top} above the middle"),
            false => format!("at {left} {top}"),
        };
        self.shown
            .borrow_mut()
            .push(format!("{place}, {width} wide, radius {radius}"));
        self.levels
            .borrow_mut()
            .push((level, self.heights.borrow().last().copied()));
    }

    fn set_height(&self, height: f32) {
        self.heights.borrow_mut().push(height);
    }

    fn hide(&self) {
        self.record("hide".into());
    }

    fn dismiss(&self) {
        self.record("dismiss".into());
    }

    fn take_keyboard(&self) {
        self.record("take keyboard".into());
    }

    /// Every command, as if each did something now, with its default key.
    fn commands(&self, text: &str, _: &App) -> Vec<shell::BarCommand> {
        if !self.commands {
            return Vec::new();
        }
        let keys = shell::UserKeys::default();
        shell::find_commands(text, &keys, |name, command| {
            (!self.idle.contains(&command)).then_some(name)
        })
    }

    fn task_agents(&self, _: &App) -> Option<TaskAgents> {
        self.agents.borrow().clone()
    }

    fn over_private(&self) -> bool {
        self.private
    }

    fn other_tabs(&self, _: &App) -> Rc<Vec<OtherTab>> {
        Rc::new(self.others.clone())
    }

    fn archived(&self, url: &str, _: &App) -> Option<(u64, f64)> {
        (self.archive.borrow().iter())
            .find(|(_, page, _)| page == url)
            .map(|&(id, _, closed)| (id, closed))
    }

    fn restore_archived(&self, archived: u64, _: &mut App) -> bool {
        let kept = (self.archive.borrow().iter()).any(|&(id, _, _)| id == archived);
        if kept {
            self.record(format!("restore archived {archived}"));
        }
        kept
    }

    fn now(&self) -> f64 {
        self.now
    }

    fn settings_index(&self, _: &App) -> Option<Rc<Vec<settings_search::Entry>>> {
        self.settings_read.set(self.settings_read.get() + 1);
        self.settings.borrow().clone()
    }

    fn show_setting(&self, section: Section, name: SharedString, _: &mut App) {
        self.record(format!("show setting {} › {name}", section.name()));
    }

    fn asking(&self, shown: bool, _: &mut App) {
        self.record(format!("asking {shown}"));
    }

    fn task_page(&self, _: &App) -> Option<(SharedString, SharedString)> {
        self.page.clone()
    }

    fn run_task(
        &self,
        agent: &str,
        asked: &str,
        page: Option<(SharedString, SharedString)>,
        _: &mut App,
    ) {
        let page = page.map_or(String::new(), |(url, _)| format!(" on {url}"));
        self.record(format!("task {agent} {asked}{page}"));
    }

    fn set_up_agents(&self, _: &mut App) {
        self.record("set up agents".into());
    }

    fn tooltip(&self, tip: Option<(Tip, Anchor)>, _: &mut App) {
        let Some((tip, anchor)) = tip else {
            return self.tips.borrow_mut().push("no tooltip".into());
        };
        self.tips
            .borrow_mut()
            .push(format!("tooltip {} {}", tip.text, tip.keys.join(" ")));
        self.anchors.borrow_mut().push(anchor);
    }
}

/// The agents the host offers: each ready, at its limit if it has a reset time. Claude runs on
/// Haiku, and Codex on its own default.
fn agents(listed: &[(&str, Option<&str>)]) -> Option<TaskAgents> {
    let agents = (listed.iter())
        .map(|&(id, resets)| TaskAgent {
            id: id.to_string().into(),
            name: id.to_string().into(),
            glyph: if id == "claude" {
                "icons/terminal.svg"
            } else {
                "icons/square-terminal.svg"
            },
            model: (id == "claude").then(|| "Haiku".into()),
            resets: resets.map(|at| at.to_string().into()),
        })
        .collect();
    Some(TaskAgents {
        agents,
        known: true,
    })
}

/// The bar, opened for a new tab.
struct Bar {
    cx: VisualTestContext,
    window: WindowHandle<CommandBar>,
    host: Rc<Recorder>,
}

impl Bar {
    fn new(cx: &mut TestAppContext) -> Self {
        Bar::with_tabs(cx, Vec::new())
    }

    /// The bar, opened for a new tab, finding the browser's commands.
    fn with_commands(cx: &mut TestAppContext) -> Self {
        Bar::open(
            cx,
            Vec::new(),
            Recorder {
                commands: true,
                ..Recorder::default()
            },
        )
    }

    /// The bar, opened for a new tab, finding the browser's commands but `idle`.
    fn with_commands_but(cx: &mut TestAppContext, idle: &[shell::Command]) -> Self {
        let host = Recorder {
            commands: true,
            idle: idle.to_vec(),
            ..Recorder::default()
        };
        Bar::open(cx, Vec::new(), host)
    }

    /// The bar, opened for a new tab in a Space with `tabs`.
    fn with_tabs(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>) -> Self {
        Bar::open(cx, tabs, Recorder::default())
    }

    fn open(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>, host: Recorder) -> Self {
        Bar::opened(cx, host, |cx| show(None, Vec::new(), tabs, None, cx))
    }

    /// The bar, opened for Search Tabs on `tabs`.
    fn search_tabs(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>) -> Self {
        Bar::opened(cx, Recorder::default(), |cx| show_tabs(tabs, cx))
    }

    fn opened(cx: &mut TestAppContext, host: Recorder, show: impl FnOnce(&mut App)) -> Self {
        let host = Rc::new(host);
        cx.update(text_input::bind_keys);
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for the bar");
        cx.update(show);
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        Bar { cx, window, host }
    }

    fn type_text(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    /// Does to the input what the platform's input method does through GPUI's input handler.
    fn input_method(
        &mut self,
        change: impl FnOnce(&mut TextInput, &mut Window, &mut Context<TextInput>),
    ) {
        (self.window)
            .update(&mut self.cx.cx, |bar, window, cx| {
                bar.input.update(cx, |input, cx| change(input, window, cx))
            })
            .unwrap();
        self.cx.run_until_parked();
    }

    /// The input method composing `text` in place of what it composed before.
    fn compose(&mut self, text: &str) {
        self.input_method(|input, window, cx| {
            input.replace_and_mark_text_in_range(None, text, None, window, cx)
        });
    }

    fn composing(&mut self) -> bool {
        self.window
            .read_with(&self.cx.cx, |bar, cx| bar.input.read(cx).composing())
            .unwrap()
    }

    /// The core's suggestions for `text`.
    fn suggest(&mut self, text: &str, rows: Vec<Suggestion>) {
        self.cx.cx.update(|cx| suggested(text.into(), rows, cx));
        self.cx.run_until_parked();
    }

    /// The rows listed: a command's name and its key, else the row's page.
    fn listed(&mut self) -> Vec<String> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                (bar.rows.iter())
                    .map(|row| match (&row.command, &row.shortcut) {
                        (Some(_), Some(key)) => format!("{} {key}", row.text),
                        (Some(_), None) => row.text.to_string(),
                        (None, _) if row.setting.is_some() => row.text.to_string(),
                        (None, _) => row.url.clone(),
                    })
                    .collect()
            })
            .unwrap()
    }

    /// What the input shows, and what's selected in it.
    fn input(&mut self) -> (String, Range<usize>) {
        self.window
            .read_with(&self.cx.cx, |bar, cx| {
                let input = bar.input.read(cx);
                (input.text().to_string(), input.selection())
            })
            .unwrap()
    }

    /// The selected row's fill and its label.
    fn accent(&mut self) -> (Hsla, Hsla) {
        self.window
            .read_with(&self.cx.cx, |bar, cx| bar.accent(&palette::current(cx)))
            .unwrap()
    }

    /// What each row says at its end about where its result is, and the glyph on its tile.
    fn ends(&mut self) -> Vec<(String, Option<&'static str>)> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                (bar.rows.iter())
                    .filter_map(|row| bar.end(row))
                    .map(|(said, glyph)| (said.to_string(), glyph))
                    .collect()
            })
            .unwrap()
    }

    /// Each row's icon.
    fn icons(&mut self) -> Vec<Icon> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                bar.rows.iter().map(Suggestion::icon).collect()
            })
            .unwrap()
    }

    /// The rows listed: each one's page, and its tab if it has one.
    fn rows(&mut self) -> Vec<(String, u64)> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                bar.rows
                    .iter()
                    .map(|row| (row.url.clone(), row.tab))
                    .collect()
            })
            .unwrap()
    }

    /// The bar's window becoming the key one, as the host makes it when it shows the bar.
    fn activate(&mut self) {
        (self
            .window
            .update(&mut self.cx.cx, |_, window, _| window.activate_window()))
        .unwrap();
        self.cx.run_until_parked();
    }

    fn field_focused(&mut self) -> bool {
        (self.window)
            .update(&mut self.cx.cx, |bar, window, cx| {
                bar.input.read(cx).focus_handle(cx).is_focused(window)
            })
            .unwrap()
    }

    /// What the bar asked of the host since the last time.
    fn calls(&self) -> Vec<String> {
        self.host.calls.take()
    }

    /// The tooltips asked for and ended since the last time.
    fn tips(&self) -> Vec<String> {
        self.host.tips.take()
    }

    /// Where the host was asked to show the bar since the last time.
    fn shown(&self) -> Vec<String> {
        self.host.shown.take()
    }

    /// Opens the bar again on tab 1's `url`, over `field` if it's the URL field's box.
    fn edit(&mut self, url: &str, field: Option<Bounds<Pixels>>) {
        let editing = Some((1, SharedString::from(url.to_string())));
        self.cx
            .cx
            .update(|cx| show(editing, Vec::new(), Vec::new(), field, cx));
        self.cx.run_until_parked();
    }

    /// The window shows another Space, with `tabs`, which Search Tabs lists too. Returns whether
    /// the bar's pick opens a new tab now.
    fn follow(&mut self, tabs: Vec<shell::BarTab>) -> bool {
        let opens = self.cx.cx.update(|cx| space_shown(tabs.clone(), tabs, cx));
        self.cx.run_until_parked();
        opens
    }

    /// The browser's shortcut for `command`, pressed while the bar has the keyboard.
    fn run(&mut self, command: shell::Command) {
        self.cx.dispatch_action(shell::Run(command));
        self.cx.run_until_parked();
    }

    /// Whether the bar is open, as the shell asks.
    fn is_open(&mut self) -> bool {
        self.cx.cx.update(|cx| is_open(cx))
    }

    fn over_field(&mut self) -> bool {
        self.window
            .read_with(&self.cx.cx, |bar, _| bar.over_field)
            .unwrap()
    }

    /// The height the bar last asked its window to be.
    fn asked_height(&self) -> f32 {
        *self
            .host
            .heights
            .borrow()
            .last()
            .expect("the bar asked for no height")
    }

    /// How tall the bar's panel over the field reads in a window the height it last asked for, as
    /// the host makes it: the input row, and with rows, to the list's foot and the space under it.
    fn drawn_over_field(&mut self) -> f32 {
        let asked = self.asked_height();
        self.cx.simulate_resize(size(px(FIELD_WIDTH), px(asked)));
        self.cx.run_until_parked();
        (self.window)
            .read_with(&self.cx.cx, |bar, _| match bar.rows.is_empty() {
                true => FIELD_INPUT,
                false => f32::from(bar.scroll.bounds().bottom()) + LIST_END,
            })
            .unwrap()
    }

    /// The level the bar last showed at, and the height it had asked for by then.
    fn last_show(&self) -> (Level, Option<f32>) {
        *self
            .host
            .levels
            .borrow()
            .last()
            .expect("the bar didn't show")
    }

    /// A right-click in the input: the items of the menu it shows, " (disabled)" after those
    /// that are and "" for a separator, and what takes the one chosen.
    fn right_click(&mut self) -> (Vec<String>, Chosen) {
        let shown: Rc<RefCell<Option<(Vec<String>, Chosen)>>> = Rc::default();
        let menu = shown.clone();
        self.cx.cx.update(|cx| {
            cx.set_global(FieldMenu(Rc::new(move |items, _, _, _, chosen, _| {
                let items = (items.into_iter())
                    .map(|(title, enabled)| match enabled || title.is_empty() {
                        true => title.to_string(),
                        false => format!("{title} (disabled)"),
                    })
                    .collect();
                *menu.borrow_mut() = Some((items, chosen));
            })))
        });
        let at = point(px(200.), px(HEIGHT / 2.));
        self.cx
            .simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        self.cx
            .simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        self.cx.run_until_parked();
        shown
            .take()
            .expect("a right-click in the input showed no menu")
    }

    /// Chooses `title` in the menu a right-click showed.
    fn choose(&mut self, (items, chosen): (Vec<String>, Chosen), title: &str) {
        let index = items
            .iter()
            .position(|item| item == title)
            .expect("no such item");
        self.cx
            .update(|window, cx| chosen(crate::menu::Closed::Chose(index), window, cx));
        self.cx.run_until_parked();
    }

    fn copy_text(&mut self, text: &str) {
        self.cx
            .cx
            .write_to_clipboard(ClipboardItem::new_string(text.into()));
    }

    /// Moves the pointer onto row `index`, with the window at its height for five rows, as the
    /// host makes it.
    fn point_at(&mut self, index: usize) {
        self.cx
            .simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN))));
        let y = height(0) + LINE + LIST_TOP + (index as f32 + 0.5) * ROW_HEIGHT;
        self.cx
            .simulate_mouse_move(point(px(200.), px(y)), None, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// A click where row `index` shows its trash under the pointer, at its end.
    fn click_remove(&mut self, index: usize) {
        self.point_at(index);
        self.cx
            .simulate_click(remove_button(index), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// The pointer comes onto row `index`, then onto the remove button it shows at its end.
    fn point_at_remove(&mut self, index: usize) {
        self.point_at(index);
        self.cx
            .simulate_mouse_move(remove_button(index), None, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// The row with the selection's plate, and the row with the pointer's, if it has one.
    fn plates(&mut self) -> (usize, Option<usize>) {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                let hovered = bar.hovered.filter(|&index| index != bar.selected);
                (bar.selected, hovered)
            })
            .unwrap()
    }
}

/// The middle of the remove button at the end of row `index`, in the bar's window.
fn remove_button(index: usize) -> gpui::Point<gpui::Pixels> {
    let x = WIDTH - LIST_X - PLATE_INSET - ROW_END - 3. - ICON_BOX / 2.;
    let y = height(0) + LINE + LIST_TOP + (index as f32 + 0.5) * ROW_HEIGHT;
    point(px(x), px(y))
}

/// A row for a page from the profile's history, which ⇧⌦ can take out.
fn page(url: &str) -> Suggestion {
    Suggestion {
        url: url.into(),
        text: url.to_string().into(),
        detail: SharedString::default(),
        tab: 0,
        completion: String::new(),
        removable: true,
        ..Suggestion::default()
    }
}

/// The first row for what's typed, as the core makes it: the page `completion` completes the text
/// to.
fn completing(url: &str, completion: &str) -> Suggestion {
    Suggestion {
        completion: completion.into(),
        ..page(url)
    }
}

/// The search row for `text`.
fn searching(text: &str) -> Suggestion {
    Suggestion {
        removable: false,
        ..page(&search(text))
    }
}

#[gpui::test]
fn return_goes_where_the_completion_goes(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example.c");
    let rows = vec![
        completing("https://example.com/", "om"),
        searching("example.c"),
    ];
    bar.suggest("example.c", rows);
    assert_eq!(bar.input(), ("example.com".into(), 9..11));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.com/", "hide"]);
}

/// The keys typed in the page after ⌃L, which the core held from it (bc_hold_keys), go in the
/// field as typed once it opens: characters with their text and ⌫ as the key it is. A shortcut,
/// such as ⌃A, types nothing.
#[gpui::test]
fn keys_held_from_the_page_go_in_as_typed(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    let held = [
        ("G", shift),
        ("o", Modifiers::none()),
        ("x", Modifiers::none()),
        ("Backspace", Modifiers::none()),
        (" ", Modifiers::none()),
        ("!", shift),
    ];
    for (key, modifiers) in held {
        let keystroke = shell::web_keystroke(key, modifiers);
        bar.cx
            .update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    }
    bar.cx.run_until_parked();
    assert_eq!(bar.input().0, "Go !");
    assert_eq!(
        shell::web_keystroke("a", Modifiers::control()).key_char,
        None
    );
}

#[gpui::test]
fn backspace_takes_the_completion_back_until_the_next_key(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example.c");
    bar.suggest("example.c", vec![completing("https://example.com/", "om")]);
    bar.keys("backspace");
    assert_eq!(bar.input(), ("example.c".into(), 9..9));
    // The core's rows for the same text, even ones that would complete it, as a late answer to
    // the ask before ⌫ would.
    bar.suggest("example.c", vec![completing("https://example.com/", "om")]);
    assert_eq!(bar.input(), ("example.c".into(), 9..9));
    // The next key typed at the end completes again.
    bar.type_text("o");
    bar.suggest("example.co", vec![completing("https://example.com/", "m")]);
    assert_eq!(bar.input(), ("example.com".into(), 10..11));
}

#[gpui::test]
fn right_keeps_the_completion_as_typed(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example.c");
    bar.suggest("example.c", vec![completing("https://example.com/", "om")]);
    bar.keys("right");
    assert_eq!(bar.input(), ("example.com".into(), 11..11));
    // Before the rows for it come, ↩ goes where the text now names.
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.com/", "hide"]);
}

/// A host and port typed on to the end of a completion to a page under it keeps that completion
/// only until the core answers. Then the bar fills in what the core's first row does, as Chrome's
/// omnibox does: nothing when the host's root comes first, so ↩ goes to the root, not to the page
/// below it in the rows (#1148).
#[gpui::test]
fn a_typed_host_and_port_completes_only_as_the_core_says(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("127.0.0.1:5835");
    let back = "http://127.0.0.1:58351/back.html";
    bar.suggest("127.0.0.1:5835", vec![completing(back, "1/back.html")]);
    assert_eq!(bar.input(), ("127.0.0.1:58351/back.html".into(), 14..25));
    bar.type_text("1");
    assert_eq!(bar.input(), ("127.0.0.1:58351/back.html".into(), 15..25));
    bar.suggest(
        "127.0.0.1:58351",
        vec![page("http://127.0.0.1:58351/"), page(back)],
    );
    assert_eq!(bar.input(), ("127.0.0.1:58351".into(), 15..15));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open http://127.0.0.1:58351/", "hide"]);
}

#[gpui::test]
fn return_before_the_suggestions_goes_where_the_text_names(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example.org");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.org/", "hide"]);
}

#[gpui::test]
fn older_suggestions_are_dropped(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.type_text("a");
    bar.suggest("ex", vec![completing("https://example.com/", "ample.com")]);
    assert_eq!(bar.input(), ("exa".into(), 3..3));
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("exa")), "hide".into()]
    );
}

#[gpui::test]
fn arrows_wrap_at_the_ends(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let rows = || {
        vec![
            searching("ex"),
            page("https://a.test/"),
            page("https://b.test/"),
        ]
    };
    bar.suggest("ex", rows());
    bar.keys("up enter");
    assert_eq!(bar.calls(), ["open https://b.test/", "hide"]);

    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", rows());
    bar.keys("down down ctrl-n enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("ex")), "hide".into()]
    );
}

#[gpui::test]
fn return_on_a_tab_switches_to_it(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            Suggestion {
                tab: 7,
                ..page("https://a.test/")
            },
        ],
    );
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["select 7", "hide"]);
}

#[gpui::test]
fn cmd_return_opens_behind_and_keeps_the_bar_open(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    let rows = vec![
        completing("https://example.com/", "mple.com"),
        searching("exa"),
        page("https://example.org/"),
    ];
    bar.suggest("exa", rows);
    bar.keys("down down cmd-enter");
    assert_eq!(bar.calls(), ["open https://example.org/ behind"]);
    // Another, from the same rows.
    bar.keys("up up cmd-enter");
    assert_eq!(bar.calls(), ["open https://example.com/ behind"]);
}

#[gpui::test]
fn shift_cmd_return_searches_for_what_was_typed(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    bar.suggest("exa", vec![completing("https://example.com/", "mple.com")]);
    bar.keys("shift-cmd-enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("exa")), "hide".into()]
    );
}

#[gpui::test]
fn ctrl_return_puts_www_and_com_around_a_word(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    bar.suggest("exa", vec![completing("https://example.com/", "mple.com")]);
    bar.keys("ctrl-enter");
    assert_eq!(bar.calls(), ["open https://www.exa.com/", "hide"]);

    // More than a word goes where ↩ goes.
    let mut bar = Bar::new(cx);
    bar.type_text("two words");
    bar.keys("ctrl-enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("two words")), "hide".into()]
    );
}

#[gpui::test]
fn shift_return_opens_where_return_goes_in_a_new_window(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    let rows = vec![
        completing("https://example.com/", "mple.com"),
        searching("exa"),
    ];
    bar.suggest("exa", rows);
    bar.keys("shift-enter");
    assert_eq!(bar.calls(), ["hide", "window https://example.com/"]);

    // Before the suggestions come, where what's typed names.
    let mut bar = Bar::new(cx);
    bar.type_text("example.org");
    bar.keys("shift-enter");
    assert_eq!(bar.calls(), ["hide", "window https://example.org/"]);

    // A tab's row opens its page there, as ⌘↩ opens it behind.
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            Suggestion {
                tab: 7,
                ..page("https://a.test/")
            },
        ],
    );
    bar.keys("down shift-enter");
    assert_eq!(bar.calls(), ["hide", "window https://a.test/"]);
}

#[gpui::test]
fn ctrl_shift_return_opens_www_and_com_around_a_word_in_a_new_window(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    bar.suggest("exa", vec![completing("https://example.com/", "mple.com")]);
    bar.keys("ctrl-shift-enter");
    assert_eq!(bar.calls(), ["hide", "window https://www.exa.com/"]);

    // More than a word goes where ⇧↩ goes.
    let mut bar = Bar::new(cx);
    bar.type_text("two words");
    bar.keys("ctrl-shift-enter");
    assert_eq!(
        bar.calls(),
        [
            "hide".to_string(),
            format!("window {}", search("two words"))
        ]
    );
}

#[gpui::test]
fn a_tab_another_window_shows_brings_that_window_forward(cx: &mut TestAppContext) {
    let tabs = vec![tab(4, "https://a.test/"), tab(2, "https://b.test/")];
    let mut bar = Bar::with_tabs(cx, tabs);
    bar.host.elsewhere.borrow_mut().insert(2, 9);
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["hide", "activate 9 with 2"]);

    // A tab no other window shows comes to this one.
    let mut bar = Bar::with_tabs(cx, vec![tab(4, "https://a.test/")]);
    bar.host.elsewhere.borrow_mut().insert(2, 9);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);

    // A suggestion for what's typed does the same.
    let mut bar = Bar::new(cx);
    bar.host.elsewhere.borrow_mut().insert(7, 3);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            Suggestion {
                tab: 7,
                ..page("https://a.test/")
            },
        ],
    );
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["hide", "activate 3 with 7"]);
}

#[gpui::test]
fn shift_delete_takes_a_page_out_of_history(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            page("https://a.test/"),
            page("https://b.test/"),
        ],
    );
    // The search isn't from history.
    bar.keys("shift-delete");
    assert_eq!(bar.calls(), Vec::<String>::new());
    bar.keys("down shift-delete");
    assert_eq!(bar.calls(), ["remove https://a.test/"]);
    // The core lists the rows again without it, and the row under it takes its place.
    bar.suggest("ex", vec![searching("ex"), page("https://b.test/")]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://b.test/", "hide"]);
}

/// A page from history, or a search typed before, shows a trash under the pointer that takes it
/// out, where Arc's has a ✕, and ⌥⌘⌫ takes out the selected one, as ⇧⌦ does. The search for
/// what's typed and the well-known sites have none: there a click at the row's end takes the row.
#[gpui::test]
fn the_pointer_shows_a_trash_that_takes_a_suggestion_out(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let typed_before = Suggestion {
        removable: true,
        ..searching("example")
    };
    let site = Suggestion {
        removable: false,
        ..page("https://example.com/")
    };
    let rows = vec![searching("ex"), page("https://a.test/"), typed_before, site];
    bar.suggest("ex", rows);
    bar.click_remove(1);
    assert_eq!(bar.calls(), ["remove https://a.test/"]);
    bar.click_remove(2);
    assert_eq!(bar.calls(), [format!("remove {}", search("example"))]);
    bar.keys("down alt-cmd-backspace");
    assert_eq!(bar.calls(), ["remove https://a.test/"]);
    bar.click_remove(3);
    assert_eq!(bar.calls(), ["open https://example.com/", "hide"]);

    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/")]);
    bar.click_remove(0);
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("ex")), "hide".into()]
    );
}

/// ⌘Z and ⇧⌘Z in the field, which has no undo of its own, go to the window's history, which
/// undoes or redoes a row taken out of history while that's its next step (shell's undo tests).
#[gpui::test]
fn cmd_z_in_the_field_goes_to_the_windows_removal_from_history(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/")]);
    bar.click_remove(1);
    assert_eq!(bar.calls(), ["remove https://a.test/"]);
    bar.keys("cmd-z");
    assert_eq!(bar.calls(), ["undo removal"]);
    bar.keys("shift-cmd-z");
    assert_eq!(bar.calls(), ["redo removal"]);
}

/// A history row's remove button names itself and its key in a tooltip, Arc's ⌥⌘⌫ on the Mac and
/// ⇧⌦ off it, which hangs from the button in the bar's window and stays inside it. The tooltip goes
/// as the pointer leaves the button, as another row takes the row's place, and as it's clicked.
#[gpui::test]
fn the_remove_button_names_itself_and_its_key(cx: &mut TestAppContext) {
    let keys = if shell::OFF_THE_MAC {
        "Shift Del"
    } else {
        "⌥ ⌘ ⌫"
    };
    let named = format!("tooltip Remove from History {keys}");
    let rows = || {
        vec![
            searching("ex"),
            page("https://a.test/"),
            page("https://b.test/"),
        ]
    };
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", rows());
    bar.point_at_remove(1);
    assert_eq!(bar.tips(), [named.clone()]);
    let anchor = bar.host.anchors.take().pop().expect("no tooltip asked for");
    let button = remove_button(1);
    let (middle, row) = (f32::from(button.x), f32::from(button.y));
    assert!(
        (anchor.x - middle).abs() < 1.,
        "it hangs from {}, not the button's middle",
        anchor.x
    );
    let (top, bottom) = (row - ICON_BOX / 2., row + ICON_BOX / 2.);
    assert!((anchor.top - top).abs() < 1. && (anchor.bottom - bottom).abs() < 1.);
    assert_eq!(
        anchor.window,
        (WIDTH, height(ROWS_SHOWN)),
        "it isn't kept inside the bar"
    );

    // Off the button onto its row, the tooltip goes.
    bar.point_at(1);
    assert_eq!(bar.tips(), ["no tooltip"]);

    // More suggestions for what's typed keep it, and another page in the row's place ends it.
    bar.point_at_remove(1);
    bar.suggest("ex", rows());
    assert_eq!(bar.tips(), [named.clone()]);
    bar.suggest("ex", vec![searching("ex"), page("https://c.test/")]);
    assert_eq!(bar.tips(), ["no tooltip"]);

    // A click on the button ends it before the page goes.
    bar.suggest("ex", rows());
    bar.point_at_remove(2);
    assert_eq!(bar.tips(), [named]);
    bar.cx.simulate_click(remove_button(2), Modifiers::none());
    bar.cx.run_until_parked();
    assert_eq!(bar.tips(), ["no tooltip"]);
    assert_eq!(bar.calls(), ["remove https://b.test/"]);
}

#[gpui::test]
fn a_sum_comes_after_the_first_row_and_return_on_it_copies_its_result(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("12*7");
    bar.suggest("12*7", vec![searching("12*7"), page("https://a.test/12*7")]);
    // The search stays first, as the core put it, and the sum's result is the row after it.
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["hide", "copy 84"]);
}

/// The first row for an engine's keyword typed, which hints its site search.
fn hinting(keyword: &str) -> Suggestion {
    let url = format!("https://{keyword}/");
    Suggestion {
        keyword: keyword.into(),
        site: "Duck".into(),
        ..page(&url)
    }
}

#[gpui::test]
fn tab_on_a_hinted_row_searches_its_site_until_backspace_leaves(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest(
        "duck.test",
        vec![hinting("duck.test"), page("https://a.test/")],
    );
    bar.keys("tab");
    assert_eq!(bar.input(), (String::new(), 0..0));
    // ⌫ in the middle of the words stays in the site search.
    bar.type_text("cats");
    bar.keys("backspace");
    assert_eq!(bar.input(), ("cat".into(), 3..3));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://duck.test/?q=cat", "hide"]);

    // Left before anything is typed, the keyword comes back as it was, without a space.
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.keys("tab backspace");
    assert_eq!(bar.input(), ("duck.test".into(), 9..9));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://duck.test/", "hide"]);
}

#[gpui::test]
fn tab_on_a_row_without_a_hint_selects_the_next(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            hinting("duck.test"),
            page("https://a.test/"),
        ],
    );
    bar.keys("tab");
    // Now on the hinted row, Tab enters its site search.
    bar.keys("tab");
    assert_eq!(bar.input(), (String::new(), 0..0));

    // It keeps the words after the first, the keyword.
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test cats");
    bar.suggest("duck.test cats", vec![hinting("duck.test")]);
    bar.keys("tab");
    assert_eq!(bar.input(), ("cats".into(), 4..4));
}

/// Only Tab enters a site search, as in Arc: a space after the keyword is only a space, Japanese
/// input's too, so the words after it are a plain search (#1397).
#[gpui::test]
fn a_space_after_the_hinted_keyword_is_only_a_space(cx: &mut TestAppContext) {
    for space in [" ", "\u{3000}"] {
        let mut bar = Bar::new(cx);
        bar.type_text("duck.test");
        bar.suggest("duck.test", vec![hinting("duck.test")]);
        bar.type_text(space);
        let typed = format!("duck.test{space}");
        assert_eq!(
            bar.input(),
            (typed.clone(), typed.len()..typed.len()),
            "{space:?}"
        );
    }

    let mut bar = Bar::new(cx);
    bar.type_text("duck");
    bar.suggest(
        "duck",
        vec![Suggestion {
            completion: ".test".into(),
            ..hinting("duck.test")
        }],
    );
    bar.type_text(" cats");
    assert_eq!(bar.input(), ("duck cats".into(), 9..9));
    bar.suggest("duck cats", vec![searching("duck cats")]);
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("duck cats")), "hide".into()]
    );
}

/// A site search lists only the search for what's typed, as Arc's does, though the core lists the
/// site's searches from history and its pages too. Its field says "Search…", as the chip names the
/// site, and VoiceOver hears it as "Search <site>" (#1397).
#[gpui::test]
fn a_site_search_lists_only_the_search_for_whats_typed(cx: &mut TestAppContext) {
    use gpui::accesskit::Role;
    let mut bar = Bar::new(cx);
    bar.cx.simulate_a11y_activation();
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.keys("tab");
    bar.type_text("cats");
    let searched = |text: &str| Suggestion {
        text: text.to_string().into(),
        removable: text != "cats",
        ..page(&format!("https://duck.test/?q={text}"))
    };
    bar.suggest(
        "duck.test cats",
        vec![
            searched("cats"),
            searched("cats videos"),
            page("https://duck.test/cats"),
        ],
    );
    assert_eq!(bar.rows(), [("https://duck.test/?q=cats".to_string(), 0)]);
    let label = |bar: &mut Bar| {
        let tree = bar.cx.a11y_tree().expect("GPUI built a tree");
        let field = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::TextInput);
        field.and_then(|(_, node)| node.label().map(str::to_owned))
    };
    assert_eq!(label(&mut bar).as_deref(), Some("Search Duck"));
    let placeholder = bar
        .window
        .read_with(&bar.cx.cx, |bar, cx| {
            bar.input.read(cx).placeholder().clone()
        })
        .unwrap();
    assert_eq!(placeholder, "Search…");
    bar.keys("escape");
    assert_eq!(label(&mut bar).as_deref(), Some("Search or Enter URL"));
}

/// Leaving a site search puts back what was typed before Tab, as Arc's ⌫ and ⇧Tab do, not the
/// keyword it completed to: "duck" with a hint for duck.test comes back "duck", before what's typed
/// since. Esc puts the bar back as it opened, the Space's tabs or the URL ⌘L edits, selected, and
/// it stays open (#1397).
#[gpui::test]
fn leaving_a_site_search_puts_back_what_was_there_before(cx: &mut TestAppContext) {
    let duck = || Suggestion {
        completion: ".test".into(),
        ..hinting("duck.test")
    };
    for leave in ["backspace", "shift-tab"] {
        let mut bar = Bar::new(cx);
        bar.type_text("duck");
        bar.suggest("duck", vec![duck()]);
        bar.keys("tab");
        bar.keys(leave);
        assert_eq!(bar.input(), ("duck".into(), 4..4), "{leave}");

        bar.suggest("duck", vec![duck()]);
        bar.keys("tab");
        bar.type_text("cats");
        bar.keys(&format!("home {leave}"));
        assert_eq!(bar.input(), ("duck cats".into(), 5..5), "{leave}");
    }

    let tabs = vec![tab(4, "https://a.test/")];
    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("duck");
    bar.suggest("duck", vec![duck()]);
    bar.keys("tab");
    bar.type_text("cats");
    bar.keys("escape");
    assert_eq!(bar.input(), (String::new(), 0..0));
    assert_eq!(bar.rows(), [("https://a.test/".to_string(), 4)]);
    assert!(
        bar.is_open(),
        "Esc closed the bar rather than leaving the site search"
    );
    // Out of it, Esc closes the bar.
    bar.keys("escape");
    assert_eq!(bar.calls(), ["dismiss"]);

    let mut bar = Bar::new(cx);
    bar.edit("https://a.test/page", None);
    bar.type_text("duck");
    bar.suggest("duck", vec![duck()]);
    bar.keys("tab escape");
    assert_eq!(bar.input(), ("https://a.test/page".into(), 0..19));
    // ↩ then does what it does on the bar ⌘L opened: loads the page again.
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide", "reload 1"]);
}

/// A site search a new profile has is in its site's colour, which reads on the bar with its label
/// in both tones; one whose site has none, as GitHub's, or one the user added, keeps the Space's
/// accent (#1397).
#[gpui::test]
fn a_site_search_is_in_its_sites_colour(cx: &mut TestAppContext) {
    for tone in [palette::Tone::Dark, palette::Tone::Light] {
        let palette = palette::Palette::of(None, tone);
        for (site, color) in SITE_COLORS {
            let (fill, label) = palette.site_accent(gpui::rgb(color));
            let (on_bar, on_fill) = (
                palette::contrast(fill, palette.overlay),
                palette::contrast(label, fill),
            );
            assert!(
                on_bar >= palette::GLYPH && on_fill >= palette::TEXT,
                "{site} {tone:?}"
            );
        }
    }

    let mut bar = Bar::new(cx);
    let space = bar.accent();
    let youtube = Suggestion {
        site: "YouTube".into(),
        ..hinting("youtube.com")
    };
    bar.type_text("yout");
    bar.suggest("yout", vec![youtube]);
    assert_eq!(bar.accent(), space);
    bar.keys("tab");
    let palette = bar
        .window
        .read_with(&bar.cx.cx, |_, cx| palette::current(cx))
        .unwrap();
    let (fill, label) = palette.site_accent(gpui::rgb(site_colors::YOUTUBE));
    assert_eq!(bar.accent(), (fill.into(), label.into()));
    assert_ne!(bar.accent(), space);

    let mut bar = Bar::new(cx);
    bar.type_text("gith");
    bar.suggest(
        "gith",
        vec![Suggestion {
            site: "GitHub".into(),
            ..hinting("github.com")
        }],
    );
    bar.keys("tab");
    assert_eq!(bar.accent(), space);
}

/// Japanese input, as Windows' input method and AppKit's give it to the field: what it composes is
/// marked, and the core's rows don't complete over it until it's committed.
#[gpui::test]
fn the_input_method_composes_and_commits_in_the_bar(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.compose("にほん");
    assert!(bar.composing());
    bar.suggest("にほん", vec![completing("https://にほんご.test/", "ご")]);
    assert_eq!(bar.input(), ("にほん".into(), 9..9));
    bar.input_method(|input, window, cx| input.replace_text_in_range(None, "日本", window, cx));
    assert!(!bar.composing());
    assert_eq!(bar.input(), ("日本".into(), 6..6));
    assert_eq!(bar.calls(), Vec::<String>::new());

    // The keyboard leaving the field keeps what was composed, as committed.
    let mut bar = Bar::new(cx);
    bar.compose("ねこ");
    bar.input_method(|input, window, cx| input.unmark_text(window, cx));
    assert!(!bar.composing());
    assert_eq!(bar.input(), ("ねこ".into(), 6..6));
}

/// `key` pressed and released as Wayland's platform hands it to the window: down, then its
/// character typed into the field if the window didn't take the key, then up. A macro, since
/// GPUI's test window can't be named here.
macro_rules! press {
    ($platform:expr, $key:expr) => {{
        use gpui::PlatformWindow as _;
        let keystroke = gpui::Keystroke::parse($key).unwrap().with_simulated_ime();
        let down = gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        if !$platform.simulate_input(down) {
            let mut field = $platform.take_input_handler().expect("no input handler");
            field.replace_text_in_range(None, keystroke.key_char.as_deref().unwrap());
            $platform.set_input_handler(field);
        }
        $platform.simulate_input(gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }));
    }};
}

/// A character typed while the App is in an update, as a nested event pump delivers it, lands in
/// the bar once the update is over, and ahead of the one typed after it (#2672).
#[gpui::test]
fn a_character_typed_mid_update_lands_in_the_bar(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    let mut platform = bar.cx.cx.test_window(bar.window.into());
    (bar.window)
        .update(&mut bar.cx.cx, |_, _, _| press!(platform, "a"))
        .unwrap();
    press!(platform, "b");
    bar.cx.run_until_parked();
    assert_eq!(bar.input(), ("ab".into(), 2..2));
}

/// Japanese the input method composes and commits while the App is in an update, with no key of
/// its own, as a candidate picked with the pointer is: once the update is over the bar has the
/// committed text and nothing marked (#2672).
#[gpui::test]
fn a_japanese_commit_mid_update_lands_in_the_bar(cx: &mut TestAppContext) {
    use gpui::PlatformWindow as _;
    let mut bar = Bar::new(cx);
    bar.compose("にほん");
    let mut platform = bar.cx.cx.test_window(bar.window.into());
    (bar.window)
        .update(&mut bar.cx.cx, |_, _, _| {
            let mut ime = platform.take_input_handler().expect("no input handler");
            ime.replace_and_mark_text_in_range(None, "にほんご", None);
            ime.replace_text_in_range(None, "日本語");
            platform.set_input_handler(ime);
        })
        .unwrap();
    bar.cx.run_until_parked();
    assert!(!bar.composing());
    assert_eq!(bar.input(), ("日本語".into(), 9..9));
    assert_eq!(bar.calls(), Vec::<String>::new());
}

/// Korean from a 2-set keyboard, where one key can commit a syllable and start the next (#2457):
/// in a new tab's bar and over the URL the bar opened on. The core is asked only for committed
/// text, and ↩, which AppKit's, IBus's and Windows' Korean input methods pass on once they've
/// committed the last syllable, goes with the whole word.
#[gpui::test]
fn korean_composes_syllable_by_syllable_in_the_bar(cx: &mut TestAppContext) {
    let search = |word| format!("https://search.test/?q={word}");
    for (word, calls, url, went) in [
        (
            "한글",
            korean::HANGEUL,
            None,
            format!("open {}", search("한글")),
        ),
        (
            "안녕하세요",
            korean::ANNYEONG,
            None,
            format!("open {}", search("안녕하세요")),
        ),
        (
            "한글",
            korean::HANGEUL,
            Some("https://example.com/"),
            format!("navigate 1 {}", search("한글")),
        ),
    ] {
        let mut bar = Bar::new(cx);
        if let Some(url) = url {
            bar.edit(url, None);
        }
        bar.host.asked.take();
        for call in calls {
            bar.input_method(|input, window, cx| call.apply(input, window, cx));
        }
        assert!(bar.composing());
        let last = word.chars().last().unwrap().to_string();
        bar.input_method(|input, window, cx| input.replace_text_in_range(None, &last, window, cx));
        assert_eq!(bar.input(), (word.to_string(), word.len()..word.len()));
        let asked: Vec<String> = (bar.host.asked.take().into_iter())
            .map(|(text, _)| text)
            .collect();
        let mut syllables = String::new();
        let committed: Vec<String> = (word.chars())
            .map(|syllable| {
                syllables.push(syllable);
                syllables.clone()
            })
            .collect();
        assert_eq!(asked, committed, "{word} over {url:?}");
        bar.calls();
        bar.keys("enter");
        assert_eq!(bar.calls(), [went, "hide".into()], "{word} over {url:?}");
    }
}

/// Esc goes to the input method while it composes, which cancels, putting back the text from
/// before, with the bar open; Esc then closes it. An input method cancels by replacing what it
/// composed with nothing, or by composing nothing.
#[gpui::test]
fn esc_cancels_the_composition_before_it_closes_the_bar(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("cats ");
    bar.compose("ねこ");
    bar.input_method(|input, window, cx| input.replace_text_in_range(None, "", window, cx));
    assert!(!bar.composing());
    assert_eq!(bar.input(), ("cats ".into(), 5..5));
    assert_eq!(bar.calls(), Vec::<String>::new());
    bar.keys("escape");
    assert_eq!(bar.calls(), ["dismiss"]);

    let mut bar = Bar::new(cx);
    bar.type_text("cats ");
    bar.compose("ねこ");
    bar.compose("");
    assert!(!bar.composing());
    assert_eq!(bar.input(), ("cats ".into(), 5..5));
}

#[gpui::test]
fn the_inputs_menu_pastes_and_goes_where_return_would(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("something else");
    let menu = bar.right_click();
    let items = [
        "Cut (disabled)",
        "Copy (disabled)",
        "Paste",
        "Paste and Go",
        "Paste and Search",
        "",
        "Select All",
    ];
    assert_eq!(menu.0, items);
    // What was typed doesn't count, and a copied line break is a space.
    bar.copy_text("  example.org\n");
    bar.choose(menu, "Paste and Go");
    assert_eq!(bar.calls(), ["open https://example.org/", "hide"]);

    let mut bar = Bar::new(cx);
    bar.copy_text("example.org");
    let menu = bar.right_click();
    bar.choose(menu, "Paste and Search");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("example.org")), "hide".into()]
    );

    // Nothing to paste, nowhere to go.
    let mut bar = Bar::new(cx);
    bar.copy_text(" \n");
    let menu = bar.right_click();
    bar.choose(menu, "Paste and Go");
    assert_eq!(bar.calls(), Vec::<String>::new());
}

/// A page can talk the user into pasting script into the bar, so a paste loses its leading
/// `javascript:`, as Chrome's address bar's does, whatever its case and the space before it, and
/// Paste and Go goes where the rest leads. Typed, it stays, as in Chrome.
#[gpui::test]
fn a_pasted_javascript_url_loses_its_scheme(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.copy_text(" \tJavaScript:javascript: alert(1)");
    bar.keys("cmd-v");
    assert_eq!(bar.input(), ("alert(1)".into(), 8..8));
    bar.keys("cmd-a");
    bar.type_text("javascript:alert(1)");
    assert_eq!(bar.input(), ("javascript:alert(1)".into(), 19..19));

    let mut bar = Bar::new(cx);
    bar.copy_text("javascript:alert(1)\n");
    let menu = bar.right_click();
    bar.choose(menu, "Paste and Go");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("alert(1)")), "hide".into()]
    );
}

#[test]
fn only_a_leading_javascript_scheme_goes() {
    assert_eq!(without_javascript("\u{1}javascript:\njavascript:x"), "x");
    assert_eq!(
        without_javascript(" example.org/javascript:x"),
        " example.org/javascript:x"
    );
    assert_eq!(without_javascript("javascript"), "javascript");
    assert_eq!(without_javascript("javascript:"), "");
}

#[gpui::test]
fn a_right_click_in_the_selection_keeps_it_for_copy(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example");
    bar.keys("cmd-a");
    let menu = bar.right_click();
    assert_eq!(menu.0[..2], ["Cut", "Copy"]);
    bar.choose(menu, "Copy");
    let copied = bar.cx.cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("example"));
    assert_eq!(bar.input(), ("example".into(), 0..7));
}

#[test]
fn the_window_fits_up_to_five_rows_of_arcs_size() {
    assert_eq!(height(0), 64.);
    assert_eq!(height(1), 128.5);
    // Arc's panel with five rows is 328.5 tall, and more rows scroll inside it.
    assert_eq!(height(5), 328.5);
    assert_eq!(height(11), 328.5);
    // Over the URL field the input is a row tall with no line, and five rows make it 310.
    assert_eq!(field_height(0), 50.);
    assert_eq!(field_height(5), 310.);
    assert_eq!(field_height(11), 310.);
}

/// Arc's centred panel is 169 above the window's middle at both heights it was measured at.
#[test]
fn the_centred_bar_hangs_a_fixed_distance_above_the_middle() {
    assert_eq!(centered_top(1130.), 396.);
    assert_eq!(centered_top(949.), 305.5);
}

#[gpui::test]
fn the_rows_that_go_to_a_search_are_marked_for_their_icon(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example");
    let typed = Suggestion {
        url: search("example"),
        text: "example".into(),
        ..Suggestion::default()
    };
    bar.suggest("example", vec![page("https://example.com/"), typed]);
    let kinds = bar
        .window
        .read_with(&bar.cx.cx, |bar, _| {
            bar.rows
                .iter()
                .map(|row| matches!(row.kind(), Kind::Search))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(kinds, [false, true]);
}

/// One of the Space's open tabs.
fn tab(tab: u64, url: &str) -> shell::BarTab {
    shell::BarTab {
        tab,
        title: url.to_string().into(),
        url: url.to_string().into(),
        favicon: None,
        renamed: false,
        switched: 0.0,
    }
}

#[gpui::test]
fn before_anything_is_typed_it_lists_the_spaces_tabs(cx: &mut TestAppContext) {
    let tabs = vec![tab(4, "https://a.test/"), tab(2, "https://b.test/")];
    let mut bar = Bar::with_tabs(cx, tabs);
    let listed = vec![
        ("https://a.test/".to_string(), 4),
        ("https://b.test/".to_string(), 2),
    ];
    assert_eq!(bar.rows(), listed);
    // The core's answer for the empty text keeps them.
    bar.suggest("", Vec::new());
    assert_eq!(bar.rows(), listed);
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["select 2", "hide"]);
}

#[gpui::test]
fn typing_lists_suggestions_and_emptying_the_input_lists_the_tabs_again(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![tab(4, "https://a.test/")]);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex")]);
    assert_eq!(bar.rows(), [(search("ex"), 0)]);
    bar.keys("backspace backspace");
    bar.suggest("", Vec::new());
    assert_eq!(bar.rows(), [("https://a.test/".to_string(), 4)]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);
}

#[gpui::test]
fn return_with_nothing_typed_and_no_tabs_does_nothing(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.keys("enter");
    assert!(bar.calls().is_empty());
}

/// An open tab with a title of its own.
fn titled(id: u64, title: &str, url: &str) -> shell::BarTab {
    shell::BarTab {
        title: title.to_string().into(),
        ..tab(id, url)
    }
}

const CONVEX: &str = "https://dashboard.convex.dev/";

#[gpui::test]
fn the_start_of_an_open_tabs_name_puts_it_first_and_return_switches_to_it(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Projects | Convex", CONVEX)]);
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv")]);
    assert_eq!(bar.rows(), [(CONVEX.to_string(), 4), (search("conv"), 0)]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);

    // The site's name does as well as the title's words, and case doesn't matter.
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Projects", CONVEX)]);
    bar.type_text("CONV");
    bar.suggest("CONV", vec![searching("CONV")]);
    assert_eq!(bar.rows()[0], (CONVEX.to_string(), 4));

    // A word's start, not its middle or the top-level domain.
    for typed in ["vex", "dev"] {
        let mut bar = Bar::with_tabs(cx, vec![titled(4, "Projects", CONVEX)]);
        bar.type_text(typed);
        bar.suggest(typed, vec![searching(typed)]);
        assert_eq!(bar.rows()[0], (search(typed), 0), "{typed}");
    }
}

/// Search Tabs lists the tabs it's given, centred as for ⌘T, and what's typed keeps those it
/// names, by a word of their title or site, in their order. Return switches to the one selected.
#[gpui::test]
fn search_tabs_lists_the_tabs_named_and_return_switches(cx: &mut TestAppContext) {
    let tabs = vec![
        titled(4, "Projects | Convex", CONVEX),
        titled(2, "Inbox", "https://mail.test/"),
        titled(9, "Convex docs", "https://docs.convex.dev/"),
    ];
    let mut bar = Bar::search_tabs(cx, tabs);
    assert_eq!(
        bar.shown(),
        ["centred 169 above the middle, 765 wide, radius 20"]
    );
    let ids = |bar: &mut Bar| {
        bar.rows()
            .into_iter()
            .map(|(_, tab)| tab)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&mut bar), [4, 2, 9]);
    bar.type_text("conv");
    assert_eq!(ids(&mut bar), [4, 9]);
    bar.type_text("ex d");
    assert_eq!(ids(&mut bar), [9]);
    bar.keys(&["backspace"; 8].join(" "));
    assert_eq!(
        ids(&mut bar),
        [4, 2, 9],
        "with nothing typed it lists them all again"
    );
    bar.type_text("mail");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 2", "hide"]);
}

/// Search Tabs switches across windows as the bar does, and goes nowhere but its tabs: what's
/// typed is never a page or a search, by any key.
#[gpui::test]
fn search_tabs_goes_only_to_its_tabs(cx: &mut TestAppContext) {
    let mut bar = Bar::search_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.host.elsewhere.borrow_mut().insert(4, 9);
    bar.keys("cmd-enter");
    assert_eq!(bar.calls(), ["hide", "activate 9 with 4"]);
    let mut bar = Bar::search_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("example.com");
    assert!(bar.rows().is_empty());
    bar.keys("enter shift-enter cmd-enter alt-enter shift-cmd-enter ctrl-enter");
    assert!(bar.calls().is_empty(), "{:?}", bar.calls());
}

/// It lists as many tabs as the bar has rows, and what's typed finds those after them.
#[gpui::test]
fn search_tabs_finds_tabs_past_the_rows_listed(cx: &mut TestAppContext) {
    let mut tabs: Vec<_> = (1..=20)
        .map(|id| tab(id, &format!("https://site{id}.test/")))
        .collect();
    tabs.push(titled(30, "Last of all", "https://last.test/"));
    let mut bar = Bar::search_tabs(cx, tabs);
    assert_eq!(bar.rows().len(), shell::BAR_ROWS);
    bar.type_text("last");
    assert_eq!(bar.rows(), [("https://last.test/".to_string(), 30)]);
}

#[gpui::test]
fn a_tab_the_core_lists_moves_up_rather_than_twice(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("conv");
    let open = Suggestion {
        tab: 4,
        ..page(CONVEX)
    };
    bar.suggest(
        "conv",
        vec![searching("conv"), page("https://conv.test/"), open],
    );
    let rows = bar.rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], (CONVEX.to_string(), 4));
}

#[gpui::test]
fn a_single_letter_does_not_jump_to_a_tab(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("c");
    bar.suggest("c", vec![searching("c")]);
    assert_eq!(bar.rows(), [(search("c"), 0)]);
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("c")), "hide".into()]
    );
}

#[gpui::test]
fn typing_that_reads_as_a_url_keeps_chromes_first_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("convex.dev");
    bar.suggest(
        "convex.dev",
        vec![page("https://convex.dev/"), searching("convex.dev")],
    );
    assert_eq!(bar.rows()[0], ("https://convex.dev/".to_string(), 0));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://convex.dev/", "hide"]);

    for typed in ["localhost:3000", "https://conv", "conv/docs"] {
        let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
        bar.type_text(typed);
        bar.suggest(typed, vec![searching(typed)]);
        assert_eq!(bar.rows()[0], (search(typed), 0), "{typed}");
    }
}

/// The open tab goes first though the first row hints a site search, and takes the hint, which
/// follows what's typed rather than the row (#1397): ↩ switches to it, and Tab enters the search.
#[gpui::test]
fn an_open_tab_first_takes_the_site_search_the_first_row_hints(cx: &mut TestAppContext) {
    let tabs = vec![titled(4, "Convex", CONVEX)];
    let mut bar = Bar::with_tabs(cx, tabs.clone());
    bar.type_text("conv");
    let hint = || Suggestion {
        keyword: "conv.test".into(),
        site: "Conv".into(),
        ..searching("conv")
    };
    bar.suggest("conv", vec![hint(), page("https://conv.test/")]);
    assert_eq!(bar.rows()[0], (CONVEX.to_string(), 4));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);

    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("conv");
    bar.suggest("conv", vec![hint(), page("https://conv.test/")]);
    bar.keys("tab");
    assert_eq!(bar.input(), (String::new(), 0..0));
    bar.type_text("cats");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://conv.test/?q=cats", "hide"]);
}

#[gpui::test]
fn a_completion_to_another_page_keeps_chromes_first_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("conv");
    bar.suggest(
        "conv",
        vec![
            completing("https://convoy.test/", "oy.test"),
            searching("conv"),
        ],
    );
    assert_eq!(bar.rows()[0], ("https://convoy.test/".to_string(), 0));
    assert_eq!(bar.input(), ("convoy.test".into(), 4..11));
}

#[gpui::test]
fn a_completion_to_an_open_tabs_page_is_that_tabs_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Home", "https://convex.dev/")]);
    bar.type_text("conv");
    let rows = vec![
        completing("https://convex.dev/", "ex.dev"),
        searching("conv"),
    ];
    bar.suggest("conv", rows);
    assert_eq!(
        bar.rows(),
        [("https://convex.dev/".to_string(), 4), (search("conv"), 0)]
    );
    assert_eq!(bar.input(), ("convex.dev".into(), 4..10));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);
}

#[gpui::test]
fn of_two_matching_tabs_the_one_shown_last_is_first(cx: &mut TestAppContext) {
    // The Space's tabs come most recently shown first.
    let docs = "https://docs.convex.dev/";
    let tabs = vec![titled(2, "Convex docs", docs), titled(4, "Convex", CONVEX)];
    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("conv");
    // The other rises too, right under the search row, in place of the core's own row for it.
    let other = Suggestion {
        tab: 4,
        ..page(CONVEX)
    };
    bar.suggest("conv", vec![searching("conv"), other]);
    assert_eq!(
        bar.rows(),
        [
            (docs.to_string(), 2),
            (search("conv"), 0),
            (CONVEX.to_string(), 4)
        ]
    );
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 2", "hide"]);
    let mut bar = Bar::with_tabs(cx, vec![titled(2, "Convex docs", docs)]);
    bar.type_text("conv");
    let other = Suggestion {
        tab: 4,
        ..page(CONVEX)
    };
    bar.suggest("conv", vec![searching("conv"), other]);
    bar.keys("down down enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);
}

/// An open tab the user renamed, as Arc's custom title.
fn renamed(id: u64, name: &str, url: &str) -> shell::BarTab {
    shell::BarTab {
        renamed: true,
        ..titled(id, name, url)
    }
}

/// Every tab the typing names rises, up to three: the first above the search row, the others right
/// after it, before the profile's pages. A renamed tab whose own name the typing starts goes
/// first, then the most recently shown, and a renamed one named only by a later word doesn't.
#[gpui::test]
fn the_tabs_named_rise_a_renamed_one_starting_with_the_typing_first(cx: &mut TestAppContext) {
    let tabs = vec![
        titled(2, "Convex docs", "https://docs.convex.dev/"),
        renamed(3, "My convoy", "https://convoy.test/"),
        titled(4, "Projects | Convex", CONVEX),
        renamed(5, "Conv board", "https://board.test/"),
        titled(6, "Convert units", "https://units.test/"),
    ];
    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv"), page("https://conv.test/")]);
    let rows = bar.rows();
    let ids: Vec<u64> = rows.iter().map(|(_, tab)| *tab).collect();
    assert_eq!(ids, [5, 0, 2, 3, 0], "{rows:?}");
    assert_eq!(rows[4].0, "https://conv.test/");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 5", "hide"]);
}

/// Among the tabs the typing names, those the user switched to more rise first, after a renamed
/// one its own name starts, and the most recently shown leads among equals (#1396).
#[gpui::test]
fn the_tabs_named_rise_by_how_often_they_were_switched_to(cx: &mut TestAppContext) {
    let switched = |tab: shell::BarTab, switched| shell::BarTab { switched, ..tab };
    let tabs = vec![
        titled(2, "Convex docs", "https://docs.convex.dev/"),
        switched(titled(4, "Projects | Convex", CONVEX), 3.0),
        renamed(5, "Conv board", "https://board.test/"),
        switched(titled(6, "Convert units", "https://units.test/"), 0.5),
    ];
    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv")]);
    let ids: Vec<u64> = bar.rows().iter().map(|(_, tab)| *tab).collect();
    assert_eq!(ids, [5, 0, 4, 6]);
}

/// An address typed whose page an open tab shows keeps its row first, where ↩ goes, and the
/// tab's row comes second, as Arc's bar lists them.
#[gpui::test]
fn a_typed_address_an_open_tab_shows_keeps_its_row_with_the_tab_second(cx: &mut TestAppContext) {
    let home = "https://convex.dev/";
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", home)]);
    bar.type_text("convex.dev");
    let listed = Suggestion {
        tab: 4,
        ..page(home)
    };
    bar.suggest(
        "convex.dev",
        vec![page(home), searching("convex.dev"), listed],
    );
    let search = search("convex.dev");
    assert_eq!(
        bar.rows(),
        [(home.to_string(), 0), (home.to_string(), 4), (search, 0)]
    );
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);
}

/// A page from history with an earlier row's title, or an open tab's, on a web app that gives
/// many pages one title, is left out, as Arc's bar keeps one; elsewhere both stay.
#[gpui::test]
fn rows_alike_on_a_one_title_web_app_list_once(cx: &mut TestAppContext) {
    let titled_page = |url: &str, title: &str| Suggestion {
        text: title.to_string().into(),
        ..page(url)
    };
    let figma = "https://www.figma.com/design/c";
    let mut bar = Bar::with_tabs(cx, vec![titled(7, "Untitled", figma)]);
    bar.type_text("pull");
    bar.suggest(
        "pull",
        vec![
            searching("pull"),
            Suggestion {
                tab: 7,
                ..titled_page(figma, "Untitled")
            },
            titled_page("https://github.com/a/b/pulls", "Pull requests"),
            titled_page("https://github.com/c/d/pulls", "Pull requests"),
            titled_page("https://gist.github.com/e", "Pull requests"),
            titled_page("https://gitlab.test/f/pulls", "Pull requests"),
            titled_page("https://gitlab.test/g/pulls", "Pull requests"),
            titled_page("https://www.figma.com/design/h", "Untitled"),
        ],
    );
    let urls: Vec<String> = bar.rows().into_iter().map(|(url, _)| url).collect();
    assert_eq!(
        urls,
        [
            search("pull"),
            figma.to_string(),
            "https://github.com/a/b/pulls".to_string(),
            "https://gitlab.test/f/pulls".to_string(),
            "https://gitlab.test/g/pulls".to_string(),
        ]
    );
}

/// Rows for the pages shown go last, as Arc's bar deranks the page it's on, but for the first,
/// where ↩ goes, and another tab's, which switches to it.
#[gpui::test]
fn rows_for_the_page_shown_go_last(cx: &mut TestAppContext) {
    let shown = "https://notes.test/today";
    let open = |cx: &mut TestAppContext| {
        Bar::opened(cx, Recorder::default(), |cx| {
            show(None, vec![shown.into()], Vec::new(), None, cx)
        })
    };
    let mut bar = open(cx);
    bar.type_text("notes");
    let other_tab = Suggestion {
        tab: 8,
        ..page(shown)
    };
    let rows = vec![
        searching("notes"),
        page(shown),
        other_tab,
        page("https://notes.test/"),
    ];
    bar.suggest("notes", rows);
    let notes = "https://notes.test/".to_string();
    let expected = [
        (search("notes"), 0),
        (shown.to_string(), 8),
        (notes, 0),
        (shown.into(), 0),
    ];
    assert_eq!(bar.rows(), expected);

    let mut bar = open(cx);
    bar.type_text("notes");
    bar.suggest(
        "notes",
        vec![completing(shown, ".test/today"), searching("notes")],
    );
    assert_eq!(bar.rows()[0], (shown.to_string(), 0));
}

#[gpui::test]
fn the_tab_shown_is_never_offered(cx: &mut TestAppContext) {
    // Editing tab 4's URL, which is the tab shown, and tab 2 is another.
    let mut bar = Bar::with_tabs(cx, Vec::new());
    let docs = titled(2, "Convex docs", "https://docs.convex.dev/");
    let tabs = vec![titled(4, "Convex", CONVEX), docs];
    let editing = Some((4, SharedString::from(CONVEX)));
    bar.cx
        .cx
        .update(|cx| show(editing, Vec::new(), tabs, None, cx));
    bar.cx.run_until_parked();
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv")]);
    assert_eq!(bar.rows()[0], ("https://docs.convex.dev/".to_string(), 2));

    let editing = Some((4, SharedString::from(CONVEX)));
    bar.cx.cx.update(|cx| {
        show(
            editing,
            Vec::new(),
            vec![titled(4, "Convex", CONVEX)],
            None,
            cx,
        )
    });
    bar.cx.run_until_parked();
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv")]);
    assert_eq!(bar.rows(), [(search("conv"), 0)]);
}

#[test]
fn a_tab_is_named_by_the_starts_of_its_titles_words_and_its_sites_labels() {
    let url = "https://www.my-team.dashboard.convex.dev:8080/x";
    let tab = titled(1, "Projects | Convex-Dashboard", url);
    for yes in ["pro", "convex", "dash", "my", "team", "dashboard", "conv"] {
        assert!(names_tab(&tab, yes), "{yes}");
    }
    for no in ["rojects", "ex", "dev", "www", "8080", "dashboard.c"] {
        assert!(!names_tab(&tab, no), "{no}");
    }
    // A page that isn't on the web has no site to name.
    assert!(!names_tab(&titled(2, "Home", "data:text/html,hi"), "dat"));
    // A host with one label is named whole.
    assert!(names_tab(
        &titled(3, "Home", "http://localhost:3000/"),
        "local"
    ));
}

#[test]
fn a_page_only_in_history_has_a_clock_and_an_open_tab_or_a_site_its_favicon() {
    assert_eq!(
        page("https://a.test/").icon(),
        Icon::Glyph("icons/clock.svg")
    );
    let open = Suggestion {
        tab: 3,
        ..page("https://a.test/")
    };
    assert_eq!(open.icon(), Icon::Favicon);
    // A well-known site isn't in the profile's history.
    let site = Suggestion {
        removable: false,
        ..page("https://wikipedia.org/")
    };
    assert_eq!(site.icon(), Icon::Favicon);
    let typed = Suggestion {
        search: true,
        ..searching("a")
    };
    assert_eq!(typed.icon(), Icon::Glyph("icons/search.svg"));
    let sum = Suggestion {
        result: Some("2".into()),
        ..Suggestion::default()
    };
    assert_eq!(sum.icon(), Icon::None);
}

#[test]
fn a_command_has_its_own_glyph_where_lucide_names_it_and_each_is_built_in() {
    use gpui::AssetSource;
    use shell::Command;
    let glyph = |command| {
        Suggestion {
            command: Some(command),
            ..Suggestion::default()
        }
        .icon()
    };
    assert_eq!(
        glyph(Command::Clear),
        Icon::Glyph("icons/brush-cleaning.svg")
    );
    assert_eq!(
        glyph(Command::ClearArchive),
        Icon::Glyph("icons/archive.svg")
    );
    assert_eq!(
        glyph(Command::CollapseFolders),
        Icon::Glyph("icons/fold-vertical.svg")
    );
    assert_eq!(glyph(Command::SeparateSplit), Icon::Glyph(COMMAND_GLYPH));
    let commands = [
        Command::Clear,
        Command::ClearCacheAndRefresh,
        Command::ClearCookiesAndRefresh,
        Command::ClearArchive,
        Command::CollapseFolders,
        Command::NewFolder,
        Command::TogglePin,
        Command::ToggleMute,
        Command::CloseTab,
        Command::ReopenClosedTab,
        Command::CopyUrl,
        Command::Find,
        Command::Back,
        Command::Forward,
        Command::Reload,
        Command::Stop,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::FlipSplit,
        Command::ToggleSidebar,
        Command::SavePage,
        Command::Print,
        Command::ViewSource,
        Command::InspectElements,
        Command::JavaScriptConsole,
        Command::ManageExtensions,
        Command::RenameSpace,
        Command::NextTab,
        Command::PreviousTab,
        Command::ViewLibrary,
        Command::ViewDownloads,
        Command::ViewHistory,
        Command::Settings,
        Command::KeyboardShortcuts,
        Command::About,
        Command::Import,
        Command::NewSpace,
        Command::SiteSearchSettings,
        Command::ClearBrowsingData,
        Command::HelpCenter,
        Command::WhatsNew,
        Command::ReportProblem,
        Command::CopyBuildInfo,
        Command::PrivacyPolicy,
        Command::RestoreEarlierSidebar,
    ];
    for command in commands {
        let Icon::Glyph(path) = glyph(command) else {
            panic!("{command:?} has no glyph");
        };
        assert!(
            shell::Assets.load(path).unwrap().is_some(),
            "{path} isn't built in"
        );
    }
}

/// Arc lights the row under the pointer with its own plate while the selection keeps the accent,
/// and the keys move only the selection.
#[gpui::test]
fn the_row_under_the_pointer_has_a_plate_as_the_keys_move_the_selection(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let rows = vec![
        searching("ex"),
        page("https://a.test/"),
        page("https://b.test/"),
    ];
    bar.suggest("ex", rows);
    bar.point_at(2);
    assert_eq!(bar.plates(), (0, Some(2)));
    bar.keys("down");
    assert_eq!(bar.plates(), (1, Some(2)));
    bar.keys("up");
    assert_eq!(bar.plates(), (0, Some(2)));
    // On the selected row it's the selection's plate alone, and the hover comes back as it goes.
    bar.keys("down down");
    assert_eq!(bar.plates(), (2, None));
    bar.keys("down");
    assert_eq!(bar.plates(), (0, Some(2)));
    // ↩ takes the selection, not the row under the pointer.
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        [format!("open {}", search("ex")), "hide".into()]
    );
}

#[gpui::test]
fn the_pointer_moving_takes_the_plate_with_it(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest(
        "ex",
        vec![
            searching("ex"),
            page("https://a.test/"),
            page("https://b.test/"),
        ],
    );
    bar.point_at(1);
    assert_eq!(bar.plates(), (0, Some(1)));
    bar.point_at(2);
    assert_eq!(bar.plates(), (0, Some(2)));
}

/// A click on the URL field opens Arc's narrower panel over the field, its left at the field's
/// and its top 2 above; ⌘T and ⌘L open the centred one.
#[gpui::test]
fn a_click_on_the_url_field_opens_the_bar_over_it_and_cmd_l_centred(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    assert_eq!(
        bar.shown(),
        ["centred 169 above the middle, 765 wide, radius 20"]
    );
    let field = Bounds::new(point(px(8.), px(46.)), size(px(244.), px(36.)));
    bar.edit("https://example.com/", Some(field));
    assert_eq!(bar.shown(), ["at 8 44, 350 wide, radius 20"]);
    assert!(bar.over_field());
    assert_eq!(bar.input(), ("https://example.com/".into(), 0..20));
    bar.edit("https://example.com/", None);
    assert_eq!(
        bar.shown(),
        ["centred 169 above the middle, 765 wide, radius 20"]
    );
    assert!(!bar.over_field());
}

/// Over the URL field the window is as tall as what the bar draws, asked for before it shows: with
/// no rows, as rows come, as they go, and on opening again after a taller panel or the centred one
/// (#1306).
#[gpui::test]
fn over_the_field_the_window_is_as_tall_as_what_the_bar_draws(cx: &mut TestAppContext) {
    let tabs = vec![
        titled(4, "Projects", CONVEX),
        titled(2, "Inbox", "https://mail.test/"),
    ];
    let mut bar = Bar::with_tabs(cx, tabs);
    assert_eq!(
        bar.asked_height(),
        height(2),
        "centred, with the Space's tabs"
    );
    let field = Bounds::new(point(px(8.), px(46.)), size(px(244.), px(36.)));
    let url = "https://example.com/";
    bar.edit(url, Some(field));
    assert_eq!(
        bar.last_show(),
        (FIELD_LEVEL, Some(FIELD_INPUT)),
        "the input row, before it shows"
    );
    assert_eq!(bar.drawn_over_field(), FIELD_INPUT);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/")]);
    assert_eq!(bar.drawn_over_field(), bar.asked_height());
    assert_eq!(bar.asked_height(), field_height(2));
    let many: Vec<_> = (0..7)
        .map(|at| page(&format!("https://{at}.test/")))
        .collect();
    bar.type_text("x");
    bar.suggest("exx", many);
    assert_eq!(bar.drawn_over_field(), bar.asked_height());
    assert_eq!(bar.asked_height(), field_height(ROWS_SHOWN));
    bar.type_text("q");
    bar.suggest("exxq", Vec::new());
    assert_eq!(bar.asked_height(), FIELD_INPUT, "no rows left");
    assert_eq!(bar.drawn_over_field(), FIELD_INPUT);
    bar.type_text("z");
    bar.suggest(
        "exxqz",
        (0..7)
            .map(|at| page(&format!("https://{at}.test/")))
            .collect(),
    );
    bar.keys("escape");
    bar.edit(url, Some(field));
    assert_eq!(
        bar.last_show(),
        (FIELD_LEVEL, Some(FIELD_INPUT)),
        "after the taller panel"
    );
    bar.edit(url, None);
    assert_eq!(bar.last_show(), (Level::Modal, Some(height(0))));
    bar.edit(url, Some(field));
    assert_eq!(
        bar.last_show(),
        (FIELD_LEVEL, Some(FIELD_INPUT)),
        "after the centred one"
    );
    assert_eq!(bar.drawn_over_field(), FIELD_INPUT);
}

/// Over the URL field the panel is a row tall, and its shadow can't be seen a row below it. The
/// centred bar's modal shadow, 24 down at a 64 blur, reached twice that, and over a light page
/// read as a blank grey bar under the panel (#1306).
#[gpui::test]
fn over_the_field_the_shadow_ends_within_a_row_below_the_panel(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    let field = Bounds::new(point(px(8.), px(46.)), size(px(244.), px(36.)));
    bar.edit("https://example.com/", Some(field));
    let (level, _) = bar.last_show();
    assert!(
        level.reach().below < FIELD_INPUT,
        "{level:?} reaches {:?}",
        level.reach()
    );
    bar.edit("https://example.com/", None);
    assert_eq!(
        bar.last_show().0,
        Level::Modal,
        "the centred bar keeps its modal shadow"
    );
}

/// ⌘T or ⌘L closes the bar, whichever opened it, as Arc's do, and the browser's other shortcuts go
/// on to the browser.
#[gpui::test]
fn cmd_t_or_cmd_l_closes_the_open_bar(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    let browser: Rc<RefCell<Vec<shell::Command>>> = Rc::default();
    let ran = browser.clone();
    bar.cx.cx.update(|cx| {
        cx.on_action(move |run: &shell::Run, _| ran.borrow_mut().push(run.0));
    });
    bar.type_text("ex");
    bar.run(shell::Command::NewTab);
    assert_eq!(bar.calls(), ["dismiss"]);
    bar.edit("https://example.com/", None);
    bar.run(shell::Command::EditUrl);
    bar.run(shell::Command::NewTab);
    assert_eq!(bar.calls(), ["dismiss", "dismiss"]);
    bar.run(shell::Command::CopyUrl);
    assert_eq!(bar.calls(), Vec::<String>::new());
    assert_eq!(*browser.borrow(), [shell::Command::CopyUrl]);
}

/// The bar is closed once it asks the host to close it, on Esc or ⌘T again, not only once the
/// host says it went, which on Windows comes a message later or, before #1346, not at all: a Space
/// shown since found it open and put the new tab's plate back in the sidebar, with no bar.
#[gpui::test]
fn the_bar_is_closed_as_it_asks_to_go(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.keys("escape");
    assert_eq!(bar.calls(), ["dismiss"]);
    assert!(!bar.is_open());
    assert!(
        !bar.follow(Vec::new()),
        "a Space shown since opens no new tab"
    );

    let mut bar = Bar::new(cx);
    assert!(
        bar.follow(Vec::new()),
        "the open bar opens a new tab in the Space shown"
    );
    bar.calls();
    bar.run(shell::Command::NewTab);
    assert_eq!(bar.calls(), ["dismiss"]);
    assert!(!bar.is_open());
    assert!(
        !bar.follow(Vec::new()),
        "a Space shown since opens no new tab"
    );
}

/// ↩ on the URL ⌘L opened the bar on loads the page again, as Arc's does; typed over, it goes
/// where the text names.
#[gpui::test]
fn return_on_the_url_it_opened_on_reloads(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.edit("https://example.com/", None);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide", "reload 1"]);
    bar.edit("https://example.com/", None);
    bar.type_text("example.org");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["navigate 1 https://example.org/", "hide"]);
    // A new tab's bar has no page to load again.
    let mut bar = Bar::new(cx);
    bar.type_text("example.com");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.com/", "hide"]);
}

/// The URL the bar opens on is the page's whole address, so a copy of it keeps its https://, as
/// Arc's does.
#[gpui::test]
fn a_copied_address_keeps_its_scheme(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.edit("https://example.com/a?b=1", None);
    bar.cx.dispatch_action(text_input::Copy);
    let copied = bar.cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("https://example.com/a?b=1"));
}

/// The core's rows for `text`: its search row, then a page for each of `hosts`.
fn suggestions(text: &str, hosts: &[&str]) -> Vec<Suggestion> {
    let pages = hosts.iter().map(|host| page(&format!("https://{host}/")));
    std::iter::once(searching(text)).chain(pages).collect()
}

/// Types `text` and has the core suggest `rows` for it.
fn type_and_suggest(bar: &mut Bar, text: &str, rows: Vec<Suggestion>) {
    bar.type_text(text);
    bar.suggest(text, rows);
}

/// One letter is enough to find a command, as "q" lists Quit in Arc's bar, with its key at the
/// row's end. Under three letters, commands come after the suggestions.
#[gpui::test]
fn one_letter_lists_quit_after_the_suggestions(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "q", suggestions("q", &["quora.com"]));
    let quit = concat!("Quit ", env!("APP_NAME"), " ⌘Q");
    assert_eq!(
        bar.listed(),
        [search("q"), "https://quora.com/".into(), quit.into()]
    );
}

/// Arc's "copy": the best command above the search row, and the others a name or a hidden word
/// starts with before the core's other rows, as "copy" finds New Window and Duplicate Current Tab.
/// Copy Build Info is ours, for Arc's Copy Arc Info in its Help menu.
#[gpui::test]
fn copy_puts_copy_url_above_the_search_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "copy", suggestions("copy", &["copy.com"]));
    let expected = [
        "Copy URL ⇧⌘C".to_string(),
        search("copy"),
        "Copy URL as Markdown ⌥⇧⌘C".into(),
        "Copy Build Info".into(),
        "New Window ⌘N".into(),
        "Duplicate Current Tab".into(),
        "https://copy.com/".into(),
    ];
    assert_eq!(bar.listed(), expected);
    // ↩ on the first row runs it.
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide"]);
}

/// Arc's "clear": Clear Today first, then the others whose names start with it, and one with only
/// its letters in order after the suggestions.
#[gpui::test]
fn clear_lists_clear_today_first_and_collapse_last(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "clear", suggestions("clear", &["clearhaus.com"]));
    let expected = [
        "Clear Today ⇧⌘K".to_string(),
        search("clear"),
        "Clear Cache and Refresh".into(),
        "Clear Cookies and Refresh".into(),
        "Clear Archive".into(),
        "Clear Browsing Data… ⇧⌘⌫".into(),
        "https://clearhaus.com/".into(),
        "Collapse All Folders".into(),
    ];
    assert_eq!(bar.listed(), expected);
}

/// Arc's "split": the command a word of whose name starts with it comes first, and the others
/// such come after the suggestions. Arc's "theme" puts Edit Theme… first the same way, and
/// "library" Open Library….
#[gpui::test]
fn a_word_of_the_name_puts_the_best_first_and_the_rest_after(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "split", suggestions("split", &["splitwise.com"]));
    let expected = [
        "Add Split View ⌃+".to_string(),
        search("split"),
        "https://splitwise.com/".into(),
        "Close this Split Pane ⌃⇧-".into(),
        "Separate Page from Split View".into(),
        "Expand Current Split".into(),
        "Flip Split View".into(),
        "Convert to Vertical Split View".into(),
        "Convert to Horizontal Split View".into(),
        "New Note Split".into(),
    ];
    assert_eq!(bar.listed(), expected);
    bar.keys("cmd-a");
    type_and_suggest(&mut bar, "library", suggestions("library", &[]));
    assert_eq!(
        bar.listed(),
        ["Open Library… ⇧⌘L".to_string(), search("library")]
    );
}

/// Tab in an empty field lists only the commands, as Arc's bar does: all that do something now,
/// and then those what's typed names, none of the core's rows among them. ↩ runs one. Esc, ⇧Tab
/// or ⌫ at the start leaves, and the bar lists what's typed as ever.
#[gpui::test]
fn tab_in_an_empty_field_lists_only_the_commands(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.keys("tab");
    let all = bar.listed();
    assert!(all.len() > shell::BAR_ROWS, "{all:?}");
    assert_eq!(all[0], "Pin Tab ⌘D");
    type_and_suggest(&mut bar, "split", suggestions("split", &["splitwise.com"]));
    let only = [
        "Add Split View ⌃+",
        "Close this Split Pane ⌃⇧-",
        "Separate Page from Split View",
        "Expand Current Split",
        "Flip Split View",
        "Convert to Vertical Split View",
        "Convert to Horizontal Split View",
        "New Note Split",
    ];
    assert_eq!(bar.listed(), only);
    // What's typed doesn't go to a page or a search.
    bar.keys("shift-cmd-enter");
    assert!(bar.calls().is_empty());
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide"]);

    for leave in ["escape", "shift-tab", "home backspace"] {
        let mut bar = Bar::with_commands(cx);
        bar.keys("tab");
        bar.type_text("sp");
        bar.keys(leave);
        assert_eq!(bar.input().0, "sp", "{leave}");
        assert!(bar.calls().is_empty(), "{leave}");
        bar.suggest("sp", suggestions("sp", &["spotify.com"]));
        assert_eq!(
            bar.listed()[..2],
            [search("sp"), "https://spotify.com/".into()],
            "{leave}"
        );
    }
}

/// "/ " lists only the commands too, as in Arc's bar, and leaving puts it back. A slash elsewhere
/// is typed as ever.
#[gpui::test]
fn slash_space_lists_only_the_commands(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.type_text("/");
    bar.type_text(" ");
    assert_eq!(bar.input(), (String::new(), 0..0));
    bar.type_text("flip");
    assert_eq!(bar.listed(), ["Flip Split View"]);
    bar.keys("home backspace");
    assert_eq!(bar.input(), ("/ flip".into(), 2..2));
    bar.suggest("/ flip", vec![searching("/ flip")]);
    assert_eq!(bar.listed()[0], search("/ flip"));

    let mut bar = Bar::with_commands(cx);
    bar.type_text("a/");
    bar.type_text(" ");
    assert_eq!(bar.input(), ("a/ ".into(), 3..3));
}

/// Search Tabs and the bar over a new pane list no commands, so there Tab moves the selection.
#[gpui::test]
fn search_tabs_keeps_tab_for_the_next_row(cx: &mut TestAppContext) {
    let tabs = vec![tab(4, "https://a.test/"), tab(5, "https://b.test/")];
    let mut bar = Bar::search_tabs(cx, tabs);
    bar.keys("tab enter");
    assert_eq!(bar.calls(), ["select 5", "hide"]);
}

/// Another Space shown while the bar is open: it lists that Space's tabs, and asks the core again
/// for what's typed, which it answers from that Space's profile. The rows from before go at once,
/// and a completion from them, so a pick never goes to the other profile's page.
#[gpui::test]
fn another_space_shown_lists_its_tabs_and_asks_again(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![tab(4, "https://a.test/")]);
    assert!(bar.follow(vec![tab(9, "https://b.test/")]));
    assert_eq!(bar.rows(), [("https://b.test/".to_string(), 9)]);
    bar.type_text("exa");
    bar.suggest(
        "exa",
        vec![
            completing("https://example.com/", "mple.com"),
            searching("exa"),
        ],
    );
    bar.host.asked.take();
    assert!(bar.follow(Vec::new()));
    assert_eq!(bar.input(), ("exa".into(), 3..3));
    assert!(bar.rows().is_empty());
    assert_eq!(bar.host.asked.take(), [("exa".to_string(), false)]);
    bar.suggest("exa", vec![searching("exa"), page("https://exa.test/")]);
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["open https://exa.test/", "hide"]);
    // Closed, it has nothing to follow.
    assert!(!bar.follow(vec![tab(4, "https://a.test/")]));
    assert!(bar.host.asked.take().is_empty());
}

/// ⌘L's bar edited a tab of the Space before, so after the switch its pick opens a new tab, and
/// the URL it opened on goes with that Space's rows; typed over, what's typed stays.
#[gpui::test]
fn the_url_bar_of_the_space_before_opens_a_new_tab(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.edit("https://example.com/", None);
    assert!(bar.follow(vec![tab(9, "https://b.test/")]));
    assert_eq!(bar.input(), (String::new(), 0..0));
    assert_eq!(bar.rows(), [("https://b.test/".to_string(), 9)]);
    bar.type_text("example.org");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.org/", "hide"]);

    let mut bar = Bar::new(cx);
    bar.edit("https://example.com/", None);
    bar.type_text("example.org");
    bar.follow(Vec::new());
    assert_eq!(bar.input().0, "example.org");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://example.org/", "hide"]);
}

/// A site search leaves for the new profile, whose engines may differ; Search Tabs lists the open
/// tabs from the Space shown; the bar over a new pane goes, as the split view stays behind.
#[gpui::test]
fn the_other_bars_follow_another_space_as_they_can(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.keys("tab");
    bar.type_text("cats");
    bar.follow(Vec::new());
    assert_eq!(bar.input().0, "duck.test cats");

    let mut bar = Bar::search_tabs(cx, vec![tab(4, "https://a.test/")]);
    assert!(!bar.follow(vec![tab(9, "https://b.test/"), tab(8, "https://c.test/")]));
    assert_eq!(
        bar.rows(),
        [
            ("https://b.test/".to_string(), 9),
            ("https://c.test/".into(), 8)
        ]
    );

    let mut bar = Bar::new(cx);
    let pane = Bounds::new(point(px(400.), px(10.)), size(px(400.), px(600.)));
    let card = Bounds::new(point(px(10.), px(10.)), size(px(800.), px(600.)));
    bar.cx.cx.update(|cx| show_split(3, pane, card, cx));
    assert!(!bar.follow(Vec::new()));
    assert_eq!(bar.calls(), ["dismiss"]);
}

/// The commands-only list lists the commands again, for the Space shown.
#[gpui::test]
fn the_commands_only_list_follows_another_space(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.keys("tab");
    bar.type_text("flip");
    bar.follow(Vec::new());
    assert_eq!(bar.input().0, "flip");
    assert_eq!(bar.listed(), ["Flip Split View"]);
}

/// Arc's "space": New Space… first, as a word of its name starts with it, and the other Space
/// commands after the suggestions.
#[gpui::test]
fn space_puts_new_space_first(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "space", suggestions("space", &["spacex.com"]));
    let expected = [
        "New Space…".to_string(),
        search("space"),
        "https://spacex.com/".into(),
        "Rename Space".into(),
        "Export Space as Markdown…".into(),
        "Next Space ⌥⌘→".into(),
        "Previous Space ⌥⌘←".into(),
        "Stop Recording Trace".into(),
    ];
    assert_eq!(bar.listed(), expected);
}

/// The hidden words: "duplicate" finds New Window after Duplicate Current Tab, as Arc's does with
/// a page, and "hide" Collapse Pinned Tabs and Toggle Sidebar after Hide Sidebar.
#[gpui::test]
fn a_hidden_word_finds_its_command(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    type_and_suggest(&mut bar, "duplicate", suggestions("duplicate", &[]));
    let expected = [
        "Duplicate Current Tab".to_string(),
        search("duplicate"),
        "New Window ⌘N".into(),
    ];
    assert_eq!(bar.listed(), expected);
    bar.keys("cmd-a");
    type_and_suggest(&mut bar, "hide", suggestions("hide", &[]));
    let expected = [
        "Hide Sidebar ⌘S".to_string(),
        search("hide"),
        "Collapse Pinned Tabs".into(),
        "Toggle Sidebar ⌘S".into(),
    ];
    assert_eq!(bar.listed(), expected);
}

/// "close" archives the tab, as Arc lists Archive Tab above Close Window for it, and so does
/// "clos": the window's close comes first only once what's typed goes past its first word.
#[gpui::test]
fn close_archives_the_tab_until_the_window_is_named(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands_but(cx, &[shell::Command::CloseSplitPane]);
    for typed in ["close", "clos"] {
        type_and_suggest(&mut bar, typed, suggestions(typed, &[]));
        let listed = bar.listed();
        assert_eq!(
            listed[..2],
            ["Archive Tab ⌘W".to_string(), search(typed)],
            "{typed}"
        );
        assert!(
            listed[2..].contains(&"Close Window ⇧⌘W".to_string()),
            "{typed}: {listed:?}"
        );
        bar.keys("cmd-a");
    }
    type_and_suggest(&mut bar, "close", suggestions("close", &[]));
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide"]);

    let mut bar = Bar::with_commands_but(cx, &[shell::Command::CloseSplitPane]);
    type_and_suggest(&mut bar, "close w", suggestions("close w", &[]));
    assert_eq!(
        bar.listed(),
        ["Close Window ⇧⌘W".to_string(), search("close w")]
    );
}

/// Arc's "tab", where a site completes what's typed: the site first, then the core's rows, then
/// the commands, to Arc's 15 rows at most.
#[gpui::test]
fn a_site_completing_puts_every_command_after_the_suggestions(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.type_text("tab");
    let hosts = [
        "tabs.test",
        "tablet.test",
        "table.test",
        "tableau.test",
        "tabel.test",
    ];
    let mut rows = vec![
        completing("https://tabelog.com/", "elog.com"),
        searching("tab"),
    ];
    rows.extend(
        hosts
            .iter()
            .chain(&["a.test", "b.test", "c.test"])
            .map(|host| page(&format!("https://{host}/"))),
    );
    bar.suggest("tab", rows);
    let listed = bar.listed();
    assert_eq!(listed.len(), shell::BAR_ROWS);
    assert_eq!(listed[0], "https://tabelog.com/");
    let commands = [
        "Pin Tab ⌘D",
        "Next Tab ⌥⌘↓",
        "Search Tabs… ⇧⌘A",
        "Archive Tab ⌘W",
        "Clear Today ⇧⌘K",
    ];
    assert_eq!(listed[10..], commands);
}

/// Fifteen rows show five at a time, and the list scrolls to keep the selection in view.
#[gpui::test]
fn the_list_scrolls_to_keep_the_selection_in_view(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.cx
        .simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN))));
    type_and_suggest(
        &mut bar,
        "c",
        suggestions("c", &["c1.test", "c2.test", "c3.test"]),
    );
    assert_eq!(bar.listed().len(), shell::BAR_ROWS);
    // The rows' bounds as laid out, before the list's scroll moves them.
    let in_view = |bar: &mut Bar, index: usize| {
        bar.window
            .read_with(&bar.cx.cx, |bar, _| {
                let (list, offset) = (bar.scroll.bounds(), bar.scroll.offset().y);
                let row = bar.scroll.bounds_for_item(index).expect("no such row");
                list.top() <= row.top() + offset && row.bottom() + offset <= list.bottom()
            })
            .unwrap()
    };
    assert!(!in_view(&mut bar, 8));
    for _ in 0..8 {
        bar.keys("down");
    }
    assert_eq!(bar.plates().0, 8);
    // A frame, for the list to scroll.
    bar.cx
        .simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN) + 1.)));
    bar.cx.run_until_parked();
    assert!(in_view(&mut bar, 8));
    assert!(!in_view(&mut bar, 0));
}

/// VoiceOver finds the keyboard in a text field called what its placeholder says, and the rows in
/// a list of suggestions with the selected one marked. As the keys move the selection while the
/// field keeps the keyboard, a polite live region says the row and where it stands.
#[gpui::test]
fn voiceover_hears_the_selected_row_as_the_keys_move_it(cx: &mut TestAppContext) {
    use gpui::accesskit::{Live, Role};
    let mut bar = Bar::new(cx);
    bar.cx.simulate_a11y_activation();
    bar.type_text("do");
    let docs = page("https://docs.test/");
    let docs = Suggestion {
        text: "Docs".into(),
        detail: "docs.test".into(),
        tab: 7,
        ..docs
    };
    bar.suggest(
        "do",
        vec![
            Suggestion {
                text: "do".into(),
                ..searching("do")
            },
            docs,
        ],
    );
    let tree = bar.cx.a11y_tree().expect("GPUI built a tree");
    let nodes = |role: Role| -> Vec<_> {
        tree.nodes
            .iter()
            .filter(|(_, node)| node.role() == role)
            .cloned()
            .collect()
    };

    let field = &nodes(Role::TextInput)[0];
    assert_eq!(field.1.label(), Some("Search or Enter URL"));
    assert_eq!(field.1.value(), Some("do"));
    assert_eq!(tree.focus, field.0);
    let list = &nodes(Role::ListBox)[0];
    assert_eq!(list.1.label(), Some("Suggestions"));
    let options: Vec<_> = (nodes(Role::ListBoxOption).iter())
        .map(|(_, node)| (node.label().unwrap().to_owned(), node.is_selected()))
        .collect();
    assert_eq!(
        options,
        [
            ("do".into(), Some(true)),
            ("Docs, docs.test, Switch to Tab".into(), Some(false))
        ]
    );
    let status = &nodes(Role::Status)[0].1;
    assert_eq!(
        (status.live(), status.value()),
        (Some(Live::Polite), Some("do, 1 of 2"))
    );

    bar.keys("down");
    let tree = bar.cx.a11y_tree().expect("GPUI built a tree");
    let status = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Status)
        .unwrap();
    assert_eq!(
        status.1.value(),
        Some("Docs, docs.test, Switch to Tab, 2 of 2")
    );
}

/// ilo's menu's window, which shows nothing in a test.
struct MenuWindow;

impl crate::menu::Host for MenuWindow {
    fn show(&self, _: gpui::Bounds<gpui::Pixels>, _: &[gpui::Bounds<gpui::Pixels>]) {}
    fn hide(&self) {}
}

/// The field's menu as the app shows it, in ilo's menu: Cut and Copy are dimmed with nothing
/// selected, the field keeps the keyboard, and Paste and Go goes where return would.
#[gpui::test]
fn the_inputs_menu_is_ilos_and_pastes_and_goes(cx: &mut TestAppContext) {
    use crate::menu;
    cx.update(|cx| menu::open(Rc::new(MenuWindow), cx))
        .expect("no window for the menu");
    cx.update(|cx| {
        cx.set_global(menu::field_menu(Rc::new(|_, request, cx| {
            let area = gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(2000.), px(2000.)));
            menu::show(menu::Request { area, ..request }, cx);
        })))
    });
    let mut bar = Bar::new(cx);
    bar.type_text("something else");
    let right_click = |bar: &mut Bar| {
        let at = point(px(200.), px(HEIGHT / 2.));
        bar.cx
            .simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        bar.cx
            .simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        bar.cx.run_until_parked();
    };
    let choose = |bar: &mut Bar, title: &str| {
        let chosen = bar.cx.cx.update(|cx| menu::choose(title, cx));
        bar.cx.run_until_parked();
        chosen
    };
    right_click(&mut bar);
    assert!(!choose(&mut bar, "Cut"), "Cut with nothing selected");
    assert!(!choose(&mut bar, "Copy"), "Copy with nothing selected");
    assert!(choose(&mut bar, "Select All"));
    let focused = bar.window.update(&mut bar.cx.cx, |bar, window, cx| {
        bar.input.read(cx).focus_handle(cx).is_focused(window)
    });
    assert!(focused.unwrap(), "the field lost the keyboard to its menu");
    right_click(&mut bar);
    // What was typed doesn't count, and a copied line break is a space.
    bar.copy_text("  example.org\n");
    assert!(choose(&mut bar, "Paste and Go"));
    assert_eq!(bar.calls(), ["open https://example.org/", "hide"]);
}

/// ⇧F10 in the field opens its menu under the caret with the first item it can choose
/// highlighted, which VoiceOver reads as it opens, and return picks it.
#[gpui::test]
fn shift_f10_opens_the_fields_menu_on_its_first_item(cx: &mut TestAppContext) {
    use crate::menu;
    let window = cx
        .update(|cx| menu::open(Rc::new(MenuWindow), cx))
        .expect("no window for the menu");
    let mut in_menu = VisualTestContext::from_window(window.into(), cx);
    in_menu.simulate_a11y_activation();
    cx.update(|cx| {
        cx.set_global(menu::field_menu(Rc::new(|_, request, cx| {
            let area = gpui::Bounds::new(point(px(0.), px(0.)), gpui::size(px(2000.), px(2000.)));
            menu::show(menu::Request { area, ..request }, cx);
        })))
    });
    let mut bar = Bar::new(cx);
    bar.type_text("docs");
    bar.copy_text(".test");
    bar.keys("shift-f10");
    in_menu.run_until_parked();
    let tree = in_menu.a11y_tree().expect("GPUI built a tree");
    let focused = tree
        .nodes
        .iter()
        .find(|(id, _)| *id == tree.focus)
        .map(|(_, node)| node);
    assert_eq!(
        focused.and_then(|node| node.label()),
        Some("Paste"),
        "Cut and Copy are dimmed"
    );
    in_menu.simulate_keystrokes("enter");
    bar.cx.run_until_parked();
    let text = bar.window.update(&mut bar.cx.cx, |bar, _, cx| {
        bar.input.read(cx).text().to_owned()
    });
    assert_eq!(text.unwrap(), "docs.test");
}

/// A .new address typed to its dot gets its row first, named by what it makes, in place of the
/// core's row for the same page (#698). ↩ opens it in a new tab, even from a tab's URL, which it
/// leaves. The word alone stays a search.
#[gpui::test]
fn a_typed_new_address_opens_its_new_document_in_a_new_tab(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("meet");
    bar.suggest("meet", vec![searching("meet")]);
    assert_eq!(bar.listed(), [search("meet")]);
    bar.type_text(".");
    bar.suggest("meet.", vec![page("https://meet./"), searching("meet.")]);
    assert_eq!(
        bar.listed(),
        ["https://meet.new/", "https://meet./", &search("meet.")]
    );
    let named = bar
        .window
        .read_with(&bar.cx.cx, |bar, _| bar.rows[0].text.clone())
        .unwrap();
    assert_eq!(named, "New Google Meet");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://meet.new/", "hide"]);

    bar.edit("https://example.com/", None);
    bar.type_text("docs.new");
    bar.suggest(
        "docs.new",
        vec![page("https://docs.new/"), searching("docs.new")],
    );
    assert_eq!(bar.listed(), ["https://docs.new/", &search("docs.new")]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://docs.new/", "hide"]);
}

/// ⌘⌫ and ⌥⌫ edit the bar's field as in any other, and the bar asks again for what's left. At
/// the start of a site search they leave it, as ⌫ does (#1076).
#[gpui::test]
fn cmd_and_option_backspace_ask_again_and_leave_a_site_search(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("cats and dogs");
    bar.host.asked.take();
    bar.keys("alt-backspace");
    assert_eq!(bar.input(), ("cats and ".into(), 9..9));
    assert_eq!(
        bar.host.asked.take().last().map(|(text, _)| text.as_str()),
        Some("cats and ")
    );
    bar.keys("left left left left cmd-backspace");
    assert_eq!(bar.input(), ("and ".into(), 0..0));
    assert_eq!(
        bar.host.asked.take().last().map(|(text, _)| text.as_str()),
        Some("and ")
    );

    for key in ["cmd-backspace", "alt-backspace"] {
        let mut bar = Bar::new(cx);
        bar.type_text("duck.test");
        bar.suggest("duck.test", vec![hinting("duck.test")]);
        bar.keys("tab");
        bar.keys(key);
        assert_eq!(bar.input(), ("duck.test".into(), 9..9), "{key}");
    }
}

/// Chromium makes the browser window key by itself, such as when a page commits behind the bar,
/// and the bar shows on, deaf, unless it gets the keyboard back (#1314). It asks for it as its
/// window stops being key, the host saying no if the user left it, and the keys typed once it's
/// back go to its field.
#[gpui::test]
fn the_bar_takes_the_keyboard_back_while_it_shows(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.activate();
    assert_eq!(bar.calls(), Vec::<String>::new());
    bar.cx.deactivate_window();
    assert_eq!(bar.calls(), ["take keyboard"]);
    bar.activate();
    assert!(bar.field_focused());
    bar.type_text("cats");
    assert_eq!(bar.input(), ("cats".into(), 4..4));
    assert_eq!(bar.calls(), Vec::<String>::new());

    // Once the user has left it, the keyboard stays where they took it.
    bar.keys("escape");
    assert_eq!(bar.calls(), ["dismiss"]);
    bar.cx.cx.update(dismissed);
    bar.cx.deactivate_window();
    assert_eq!(bar.calls(), Vec::<String>::new());
}

/// The field has the focus whenever the bar shows: GPUI gives the input method's text only to the
/// focused field, so a bar without it shows deaf. Something taking the focus while the bar shows,
/// as an assistive app can, gives it back, and a bar that lost it while closed has it again as it
/// opens.
#[gpui::test]
fn the_bar_keeps_the_focus_in_its_field(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.activate();
    let blur = |bar: &mut Bar| {
        (bar.window
            .update(&mut bar.cx.cx, |_, window, cx| window.blur(cx)))
        .unwrap();
        bar.cx.run_until_parked();
    };
    blur(&mut bar);
    assert!(bar.field_focused());
    bar.type_text("cats");
    assert_eq!(bar.input(), ("cats".into(), 4..4));

    bar.keys("escape");
    bar.cx.cx.update(dismissed);
    blur(&mut bar);
    assert!(!bar.field_focused(), "a closed bar leaves the focus be");
    bar.edit("https://cats.test/", None);
    assert!(bar.field_focused());
    bar.type_text("dogs");
    assert_eq!(bar.input(), ("dogs".into(), 4..4));
}

impl Bar {
    /// The bar, opened for a new tab, with tasks on and `listed` agents ready, on `page`.
    fn with_agents(
        cx: &mut TestAppContext,
        listed: &[(&str, Option<&str>)],
        page: Option<(&str, &str)>,
    ) -> Self {
        let page = page.map(|(url, title)| (url.to_string().into(), title.to_string().into()));
        let host = Recorder {
            agents: RefCell::new(agents(listed)),
            page,
            ..Recorder::default()
        };
        Bar::open(cx, Vec::new(), host)
    }

    /// The rows' names, each with what its end says: its model, or when its limit resets.
    fn agent_rows(&mut self) -> Vec<String> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                (bar.rows.iter())
                    .map(|row| match row.task.as_ref().and_then(TaskRow::end) {
                        Some(end) => format!("{} · {end}", row.text),
                        None => row.text.to_string(),
                    })
                    .collect()
            })
            .unwrap()
    }

    /// The chip before the input, and the hint at its end.
    fn chip_and_hint(&mut self) -> (Option<String>, bool, bool) {
        self.window
            .update(&mut self.cx.cx, |bar, _, cx| {
                let chip = bar.mode_name().map(String::from);
                (chip, bar.ask.is_some(), bar.offers_ask(cx))
            })
            .unwrap()
    }
}

/// ⇥ after typing makes it a task: the page the user is on goes with it as a chip, the agents
/// ready are the rows, the default first, and ↩ closes the bar, then runs the task on the one
/// selected. Nothing about tasks runs as the bar opens or takes keys: the host is asked only
/// what it last learned.
#[gpui::test]
fn tab_makes_what_is_typed_a_task(cx: &mut TestAppContext) {
    let page = Some(("https://lisbon.example/guide", "Lisbon guide"));
    let mut bar = Bar::with_agents(cx, &[("claude", None), ("codex", None)], page);
    bar.type_text("flight here next friday");
    assert_eq!(bar.chip_and_hint(), (None, false, true), "⇥ isn't offered");
    let asked = bar.host.asked.borrow().len();
    bar.keys("tab");
    assert_eq!(
        bar.chip_and_hint(),
        (Some("Lisbon guide".into()), true, false)
    );
    assert_eq!(bar.agent_rows(), ["claude · Haiku", "codex"]);
    let icons = bar.window.read_with(&bar.cx.cx, |bar, _| {
        bar.rows.iter().map(Suggestion::icon).collect::<Vec<_>>()
    });
    assert_eq!(
        icons.unwrap(),
        [
            Icon::Glyph("icons/terminal.svg"),
            Icon::Glyph("icons/square-terminal.svg")
        ],
        "each agent has its CLI's glyph"
    );
    assert_eq!(bar.calls(), ["asking true"]);
    // What's typed now is the task's, which the core isn't asked to suggest for.
    bar.type_text(" under 150");
    assert_eq!(bar.host.asked.borrow().len(), asked);
    assert_eq!(bar.input().0, "flight here next friday under 150");
    bar.keys("down enter");
    assert_eq!(
        bar.calls(),
        ["task codex flight here next friday under 150 on https://lisbon.example/guide"]
    );
    assert!(!bar.is_open());
}

/// Esc and ⇧⇥ leave the ask with what was typed, closing Today's slot, and ⌫ at the start takes
/// the page's chip off before it leaves.
#[gpui::test]
fn the_ask_is_left_as_site_search_is(cx: &mut TestAppContext) {
    let page = Some(("https://lisbon.example/guide", ""));
    let mut bar = Bar::with_agents(cx, &[("claude", None)], page);
    bar.type_text("book it");
    bar.keys("tab");
    // A page without a title is named by its site.
    assert_eq!(bar.chip_and_hint().0.as_deref(), Some("lisbon.example"));
    bar.calls();
    bar.keys("escape");
    assert_eq!(bar.chip_and_hint(), (None, false, true));
    assert_eq!(bar.calls(), ["asking false"]);
    assert_eq!(bar.input().0, "book it");
    assert!(
        bar.is_open(),
        "Esc closed the bar rather than leaving the ask"
    );

    bar.keys("tab");
    bar.keys("shift-tab");
    assert_eq!(bar.chip_and_hint().1, false);
    assert!(bar.calls().contains(&"asking false".to_string()));

    bar.keys("tab");
    bar.keys("home backspace");
    assert_eq!(
        bar.chip_and_hint(),
        (None, true, false),
        "⌫ didn't take the chip off first"
    );
    bar.keys("backspace");
    assert_eq!(bar.chip_and_hint().1, false);
    assert_eq!(bar.input().0, "book it");
    // Run without the page: nothing goes with the task.
    bar.keys("tab");
    bar.keys("home backspace enter");
    assert!(bar.calls().contains(&"task claude book it".to_string()));
}

/// An agent at its plan's limit is dimmed with its reset time, and not selected; with none ready
/// once the browser knows, ⇥ offers to set one up, and with tasks off ⇥ selects the next row as
/// it did.
#[gpui::test]
fn agents_at_their_limit_or_none_ready(cx: &mut TestAppContext) {
    let mut bar = Bar::with_agents(cx, &[("claude", Some("18:00")), ("codex", None)], None);
    bar.type_text("summarise this");
    bar.keys("tab");
    assert_eq!(bar.agent_rows(), ["claude · Limit resets 18:00", "codex"]);
    bar.keys("up enter");
    // ↩ on the agent at its limit runs nothing.
    assert!(!bar.calls().iter().any(|call| call.starts_with("task")));
    assert!(bar.is_open());

    let mut bar = Bar::with_agents(cx, &[], None);
    bar.type_text("summarise this");
    // ⇥ doesn't say Ask, as no agent is ready, but leads to setting one up.
    assert_eq!(bar.chip_and_hint(), (None, false, false));
    bar.keys("tab");
    assert_eq!(bar.agent_rows(), [SET_UP]);
    // ↩ on it ends the ask as the bar hides, so Today's slot closes with it, and the next ⌘T,
    // which the Shell opens from inside its own update, has no ask to end (#2459).
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        ["asking true", "asking false", "hide", "set up agents"]
    );
    bar.cx
        .update(|_, cx| show(None, Vec::new(), Vec::new(), None, cx));
    bar.cx.run_until_parked();
    assert!(
        !bar.calls().iter().any(|call| call.starts_with("asking")),
        "the bar opened again ended an ask"
    );

    let mut bar = Bar::new(cx);
    bar.type_text("summarise this");
    bar.suggest(
        "summarise this",
        vec![searching("summarise this"), page("https://a.test/")],
    );
    bar.keys("tab");
    assert_eq!(bar.chip_and_hint(), (None, false, false));
    assert_eq!(bar.plates().0, 1);
}

/// A private window offers no task, as agents get no path into private windows: with agents
/// ready, a question typed there says no Ask ⇥, ⇥ selects the next row as with tasks off, and ↩
/// opens it. Nothing of tasks shows, even as the browser learns the agents again.
#[gpui::test]
fn a_private_window_offers_no_task(cx: &mut TestAppContext) {
    let host = Recorder {
        agents: RefCell::new(agents(&[("claude", None), ("codex", None)])),
        page: Some(("https://rijksmuseum.nl/".into(), "Rijksmuseum".into())),
        private: true,
        ..Recorder::default()
    };
    let mut bar = Bar::open(cx, Vec::new(), host);
    let asked = "when does the rijksmuseum open";
    bar.type_text(asked);
    bar.suggest(
        asked,
        vec![
            searching(asked),
            page("https://www.rijksmuseum.nl/en/visit"),
        ],
    );
    assert_eq!(
        bar.chip_and_hint(),
        (None, false, false),
        "Ask ⇥ is offered"
    );
    bar.keys("tab");
    bar.cx.cx.update(task_agents_changed);
    bar.cx.run_until_parked();
    assert_eq!(
        bar.chip_and_hint(),
        (None, false, false),
        "⇥ asked an agent"
    );
    let tasks = bar.window.read_with(&bar.cx.cx, |bar, _| {
        bar.rows.iter().any(|row| row.task.is_some())
    });
    assert!(!tasks.unwrap(), "a row is a task's");
    assert_eq!(bar.plates().0, 1);
    bar.keys("enter");
    assert_eq!(
        bar.calls(),
        ["open https://www.rijksmuseum.nl/en/visit", "hide"]
    );
}

/// While the browser hasn't learned which agents are ready, the ask lists none, then lists them
/// as it learns.
#[gpui::test]
fn the_ask_lists_the_agents_as_the_browser_learns(cx: &mut TestAppContext) {
    let mut bar = Bar::with_agents(cx, &[], None);
    *bar.host.agents.borrow_mut() = Some(TaskAgents::default());
    bar.edit("", None);
    bar.type_text("book a table");
    assert_eq!(
        bar.chip_and_hint(),
        (None, false, true),
        "Ask ⇥ isn't offered while unknown"
    );
    bar.keys("tab");
    assert!(bar.agent_rows().is_empty());
    *bar.host.agents.borrow_mut() = agents(&[("codex", None)]);
    bar.cx.cx.update(task_agents_changed);
    bar.cx.run_until_parked();
    assert_eq!(bar.agent_rows(), ["codex"]);
    bar.keys("enter");
    assert!(bar.calls().contains(&"task codex book a table".to_string()));
}

/// A Little ilo window's Spaces menu is the compact bar where it was asked for, listing its Spaces
/// alone, each with its mark. What's typed filters them by a word's start and asks the core for
/// nothing, no key goes to a page or a search, and ↩ runs Move to Space for the one selected.
#[gpui::test]
fn the_spaces_menu_lists_its_spaces_and_filters_them(cx: &mut TestAppContext) {
    use shell::Command::MoveToSpace;

    let swatch = crate::menu::Swatch {
        from: 0x3f9d5b,
        to: 0x7fcf9b,
    };
    let space = |space, name: &str| shell::BarSpace {
        space,
        name: name.into(),
        swatch,
    };
    let spaces = vec![space(1, "Home"), space(2, "Side Project"), space(4, "Work")];
    let at = Bounds::new(point(px(516.), px(43.5)), gpui::size(px(240.), px(50.)));
    let mut bar = Bar::opened(cx, Recorder::default(), |cx| show_spaces(spaces, at, cx));
    assert_eq!(bar.shown(), ["at 516 43.5, 240 wide, radius 20"]);
    let listed = |bar: &mut Bar| {
        (bar.window)
            .read_with(&bar.cx.cx, |bar, _| {
                let rows = bar
                    .rows
                    .iter()
                    .map(|row| (row.text.to_string(), row.command, row.icon()));
                rows.collect::<Vec<_>>()
            })
            .unwrap()
    };
    let row = |name: &str, space| {
        (
            name.to_string(),
            Some(MoveToSpace(space)),
            Icon::Swatch(swatch),
        )
    };
    assert_eq!(
        listed(&mut bar),
        [row("Home", 1), row("Side Project", 2), row("Work", 4)]
    );
    bar.type_text("pro");
    assert_eq!(listed(&mut bar), [row("Side Project", 2)]);
    bar.keys("backspace backspace backspace");
    bar.type_text("o");
    assert!(
        listed(&mut bar).is_empty(),
        "no Space has a word starting with o"
    );
    bar.keys("enter shift-enter cmd-enter alt-enter shift-cmd-enter ctrl-enter");
    assert!(bar.calls().is_empty(), "{:?}", bar.calls());
    assert!(
        bar.host.asked.borrow().is_empty(),
        "it asked the core for {:?}",
        bar.host.asked
    );
    bar.keys("backspace");
    bar.type_text("w");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["hide"]);
}

/// Settings' rows as its search reads them: Peek and the Mini Player in Features, and Shortcuts'
/// row for a command, which the bar lists itself.
fn settings_rows() -> Vec<settings_search::Entry> {
    let row = |section, name: &str| settings_search::Entry {
        section,
        under: None,
        name: name.to_string().into(),
        line: Some("Links open in a preview.".into()),
        keys: None,
    };
    vec![
        row(Section::Features, "Peek"),
        row(Section::Features, "Mini Player"),
        row(Section::Shortcuts, "Peek Link"),
    ]
}

/// "peek" lists Peek — Settings under the first row, found as Settings' search finds it, and ↩
/// on it opens Settings at its row without anything of the switch's changing; Shortcuts' row
/// stays out, and fewer than three letters list none. Its other name finds it too.
#[gpui::test]
fn a_setting_named_opens_settings_at_its_row(cx: &mut TestAppContext) {
    let host = Recorder {
        settings: RefCell::new(Some(Rc::new(settings_rows()))),
        ..Recorder::default()
    };
    let mut bar = Bar::open(cx, Vec::new(), host);
    bar.type_text("pe");
    bar.suggest("pe", vec![searching("pe")]);
    assert_eq!(bar.listed(), [search("pe")]);
    bar.type_text("ek");
    bar.suggest("peek", vec![searching("peek"), page("https://peek.test/")]);
    assert_eq!(
        bar.listed(),
        [
            search("peek"),
            "Peek — Settings".into(),
            "https://peek.test/".into()
        ]
    );
    bar.keys("down enter");
    let calls = bar.calls();
    assert_eq!(
        bar.host.settings_read.get(),
        1,
        "the index was read more than once a time the bar opened"
    );
    assert!(
        calls.contains(&"show setting Features › Peek".to_string()),
        "{calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|call| call.starts_with("open") || call.starts_with("navigate"))
    );
    assert!(!bar.is_open());

    let host = Recorder {
        settings: RefCell::new(Some(Rc::new(settings_rows()))),
        ..Recorder::default()
    };
    let mut bar = Bar::open(cx, Vec::new(), host);
    bar.type_text("picture in");
    bar.suggest("picture in", vec![searching("picture in")]);
    assert_eq!(
        bar.listed(),
        [search("picture in"), "Mini Player — Settings".into()]
    );
}

/// Until the browser has built Settings' index the bar lists no settings, without waiting for it,
/// and asks again as the next suggestions come; once it's ready they show.
#[gpui::test]
fn settings_rows_wait_for_nothing(cx: &mut TestAppContext) {
    let mut bar = Bar::open(cx, Vec::new(), Recorder::default());
    bar.type_text("peek");
    bar.suggest("peek", vec![searching("peek")]);
    assert_eq!(bar.listed(), [search("peek")]);
    bar.host.settings.replace(Some(Rc::new(settings_rows())));
    bar.type_text(" ");
    bar.suggest("peek ", vec![searching("peek ")]);
    assert_eq!(bar.listed(), [search("peek "), "Peek — Settings".into()]);
    assert_eq!(
        bar.host.settings_read.get(),
        2,
        "the bar didn't ask again, or asked once it had it"
    );
}

/// The hint names who ⇥ asks (#1080): the agent the ask lists first, which ↩ runs on, so "Ask"
/// beside its key isn't read as two words, or any agent while the browser hasn't learned them.
#[test]
fn the_ask_hint_names_the_agent_it_asks() {
    use super::ask_words;
    let named = |listed: &[(&str, Option<&str>)]| {
        let mut agents = agents(listed).unwrap();
        for agent in &mut agents.agents {
            agent.name = match agent.id.as_ref() {
                "claude" => "Claude".into(),
                _ => "Codex".into(),
            };
        }
        ask_words(Some(&agents))
    };
    assert_eq!(named(&[("claude", None), ("codex", None)]), "Ask Claude");
    assert_eq!(named(&[("codex", None), ("claude", None)]), "Ask Codex");
    assert_eq!(named(&[]), "Ask an agent");
    assert_eq!(ask_words(None), "Ask an agent");
}

/// A tab open on another of the account's computers.
fn other(url: &str, title: &str, computer: &str) -> OtherTab {
    OtherTab {
        url: url.to_string().into(),
        title: title.to_string().into(),
        computer: computer.to_string().into(),
    }
}

const RISOTTO: &str = "https://www.seriouseats.com/risotto";
const LINUX: &str = "ilo on Linux (ThinkPad X1)";

impl Bar {
    /// The bar, opened for a new tab in a Space with `tabs`, the account's other computers having
    /// `others` open.
    fn with_others(
        cx: &mut TestAppContext,
        tabs: Vec<shell::BarTab>,
        others: Vec<OtherTab>,
    ) -> Self {
        let host = Recorder {
            others,
            ..Recorder::default()
        };
        Bar::open(cx, tabs, host)
    }

    /// The computer each row names at its end, empty for none.
    fn computers(&mut self) -> Vec<String> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                bar.rows
                    .iter()
                    .map(|row| row.computer.to_string())
                    .collect()
            })
            .unwrap()
    }
}

/// A word of the title or the site's name that what's typed starts lists the page open on another
/// computer after the search row, naming the computer, and ↩ opens it here (#1234).
#[gpui::test]
fn a_tab_open_on_another_computer_is_listed_by_name_and_opens_here(cx: &mut TestAppContext) {
    let others = vec![other(RISOTTO, "Mushroom Risotto", LINUX)];
    let mut bar = Bar::with_others(cx, Vec::new(), others.clone());
    bar.type_text("risotto");
    bar.suggest(
        "risotto",
        vec![searching("risotto"), page("https://risotto.test/")],
    );
    assert_eq!(
        bar.rows(),
        [
            (search("risotto"), 0),
            (RISOTTO.to_string(), 0),
            ("https://risotto.test/".to_string(), 0),
        ]
    );
    assert_eq!(bar.computers(), ["", LINUX, ""]);
    bar.keys("down enter");
    assert_eq!(bar.calls(), [format!("open {RISOTTO}"), "hide".into()]);

    // By its site's name too.
    let mut bar = Bar::with_others(cx, Vec::new(), others);
    bar.type_text("serious");
    bar.suggest("serious", vec![searching("serious")]);
    assert_eq!(bar.computers(), ["", LINUX]);
}

/// Another computer's tab takes the place of the core's row from history for its page, and never
/// the first row, where ↩ goes. A page a tab here has open lists only that tab's row, and so does
/// one open on two computers, the one seen last. At most two are listed.
#[gpui::test]
fn another_computers_tabs_list_each_page_once_and_never_first(cx: &mut TestAppContext) {
    let others = vec![
        other(RISOTTO, "Mushroom Risotto", LINUX),
        other("https://a.test/risotto", "Risotto A", "ilo on Mac"),
        other(RISOTTO, "Mushroom Risotto", "ilo on Windows"),
        other("https://b.test/risotto", "Risotto B", "ilo on Mac"),
        other("https://c.test/risotto", "Risotto C", "ilo on Mac"),
    ];
    let mut bar = Bar::with_others(cx, Vec::new(), others.clone());
    bar.type_text("risotto");
    bar.suggest("risotto", vec![searching("risotto"), page(RISOTTO)]);
    assert_eq!(
        bar.rows(),
        [
            (search("risotto"), 0),
            (RISOTTO.to_string(), 0),
            ("https://a.test/risotto".to_string(), 0),
        ]
    );
    assert_eq!(bar.computers(), ["", LINUX, "ilo on Mac"]);

    // The first row's page stays the core's.
    let mut bar = Bar::with_others(cx, Vec::new(), others.clone());
    bar.type_text("risotto");
    bar.suggest("risotto", vec![page(RISOTTO), searching("risotto")]);
    assert_eq!(bar.rows()[0], (RISOTTO.to_string(), 0));
    assert_eq!(bar.computers(), ["", "ilo on Mac", "ilo on Mac", ""]);

    // An open tab here at the page rises; the other computer's row for it goes.
    let tabs = vec![titled(4, "Mushroom Risotto", RISOTTO)];
    let mut bar = Bar::with_others(cx, tabs, others);
    bar.type_text("risotto");
    bar.suggest("risotto", vec![searching("risotto")]);
    assert_eq!(bar.rows()[0], (RISOTTO.to_string(), 4));
    assert!(!bar.rows().iter().skip(1).any(|(url, _)| url == RISOTTO));
}

/// One letter, or what reads as an address, lists none, as it raises no open tab.
#[gpui::test]
fn another_computers_tabs_wait_for_two_letters_of_a_name(cx: &mut TestAppContext) {
    for typed in ["r", "risotto.", "seriouseats.com/"] {
        let others = vec![other(RISOTTO, "Mushroom Risotto", LINUX)];
        let mut bar = Bar::with_others(cx, Vec::new(), others);
        bar.type_text(typed);
        bar.suggest(typed, vec![searching(typed)]);
        assert_eq!(bar.rows(), [(search(typed), 0)], "{typed}");
    }
}

/// Midnight at the start of `day` November 2026 in the user's time zone, in milliseconds since
/// 1970: the 10th is a Tuesday.
fn november(day: u32) -> f64 {
    use chrono::{Local, TimeZone};
    let midnight = Local.with_ymd_and_hms(2026, 11, day, 0, 0, 0).earliest();
    midnight
        .expect("a time zone with that midnight")
        .timestamp_millis() as f64
}

const HOUR: f64 = 3_600_000.;

/// A page from the profile's history, last visited at `at`, in milliseconds since 1970.
fn visited(url: &str, at: f64) -> Suggestion {
    Suggestion {
        last_visit: (at * 1000.) as i64,
        ..page(url)
    }
}

/// The bar, opened for a new tab at noon on Tuesday 10 November, over a Space with `tabs`, with
/// `archive` in the archive: each tab's id, page and when it closed.
fn with_archive(
    cx: &mut TestAppContext,
    tabs: Vec<shell::BarTab>,
    archive: &[(u64, &str, f64)],
) -> Bar {
    let archive = (archive.iter())
        .map(|&(id, url, closed)| (id, url.to_string(), closed))
        .collect();
    let host = Recorder {
        archive: RefCell::new(archive),
        now: november(10) + 12. * HOUR,
        ..Recorder::default()
    };
    Bar::open(cx, tabs, host)
}

/// A page's row says where its result is (#2337): an open tab's Switch to Tab on the arrow's
/// tile, a closed tab's day and Reopen on ↺'s, with its favicon for the clock, and a visited
/// page's day in words alone, keeping the clock. A page with no visit known says nothing.
#[gpui::test]
fn each_row_says_where_its_result_is(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", now - HOUR)]);
    bar.type_text("pa");
    let rows = vec![
        searching("pa"),
        Suggestion {
            tab: 7,
            ..page("https://open.test/")
        },
        visited("https://closed.test/", now - 2. * HOUR),
        visited("https://visited.test/", november(9) + 8. * HOUR),
        page("https://unknown.test/"),
    ];
    bar.suggest("pa", rows);
    assert_eq!(
        bar.ends(),
        [
            ("Switch to Tab".into(), Some("icons/arrow-right.svg")),
            ("Closed today · Reopen".into(), Some("icons/rotate-ccw.svg")),
            ("Visited yesterday".into(), None),
        ]
    );
    assert_eq!(
        bar.icons()[1..],
        [
            Icon::Favicon,
            Icon::Favicon,
            Icon::Glyph("icons/clock.svg"),
            Icon::Glyph("icons/clock.svg"),
        ]
    );
}

/// ↩ on a closed tab's row brings the tab back as it was, with its history, rather than loading
/// its page afresh.
#[gpui::test]
fn return_on_a_closed_row_reopens_its_tab(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", now - HOUR)]);
    bar.type_text("pa");
    bar.suggest("pa", vec![searching("pa"), page("https://closed.test/")]);
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["restore archived 5", "hide"]);
}

/// A closed tab's row whose tab has left the archive since it was listed loads its page, as any
/// page's row does.
#[gpui::test]
fn a_closed_row_gone_from_the_archive_loads_its_page(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", now - HOUR)]);
    bar.type_text("pa");
    bar.suggest("pa", vec![searching("pa"), page("https://closed.test/")]);
    bar.host.archive.borrow_mut().clear();
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["open https://closed.test/", "hide"]);
}

/// A row says the day in words: today from midnight, yesterday from the midnight before, then
/// the weekday for the five days before that, so it's never last week's, then the date. A visit
/// after now, from a clock set back, is today's.
#[gpui::test]
fn a_row_says_its_day_in_words_at_each_boundary(cx: &mut TestAppContext) {
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", november(8))]);
    let days = [
        (november(10) + 13. * HOUR, "today"),
        (november(10), "today"),
        (november(10) - 1., "yesterday"),
        (november(9), "yesterday"),
        (november(9) - 1., "Sunday"),
        (november(4), "Wednesday"),
        (november(4) - 1., "3 Nov"),
        (november(1), "1 Nov"),
    ];
    bar.type_text("pa");
    for (at, day) in days {
        bar.suggest(
            "pa",
            vec![searching("pa"), visited("https://visited.test/", at)],
        );
        assert_eq!(bar.ends(), [(format!("Visited {day}"), None)], "{day}");
    }
    bar.suggest("pa", vec![searching("pa"), page("https://closed.test/")]);
    assert_eq!(
        bar.ends(),
        [(
            "Closed Sunday · Reopen".into(),
            Some("icons/rotate-ccw.svg")
        )]
    );
}

/// A page that's both open and in the archive shows as open: a tab elsewhere's row switches to
/// it, and one open in the Space or shown is never offered back from the archive.
#[gpui::test]
fn a_page_both_open_and_closed_shows_as_open(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let archive = [
        (5, "https://elsewhere.test/", now - HOUR),
        (6, "https://here.test/", now - HOUR),
    ];
    let mut bar = with_archive(cx, vec![tab(3, "https://here.test/")], &archive);
    bar.type_text("pa");
    let rows = vec![
        searching("pa"),
        Suggestion {
            tab: 7,
            ..page("https://elsewhere.test/")
        },
        visited("https://here.test/", now - HOUR),
    ];
    bar.suggest("pa", rows);
    assert_eq!(
        bar.ends(),
        [
            ("Switch to Tab".into(), Some("icons/arrow-right.svg")),
            ("Visited today".into(), None),
        ]
    );
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["select 7", "hide"]);
}

/// An address typed loads where ↩ goes, as it does when it's an open tab's page, though a tab at
/// it closed.
#[gpui::test]
fn an_address_typed_loads_though_its_tab_closed(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", now - HOUR)]);
    bar.type_text("closed.test");
    bar.suggest("closed.test", vec![page("https://closed.test/")]);
    assert_eq!(bar.ends(), []);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://closed.test/", "hide"]);
}

/// A page completed from what's typed reopens its closed tab, as a completion to an open tab's
/// page switches to it.
#[gpui::test]
fn a_completion_to_a_closed_tab_reopens_it(cx: &mut TestAppContext) {
    let now = november(10) + 12. * HOUR;
    let mut bar = with_archive(cx, Vec::new(), &[(5, "https://closed.test/", now - HOUR)]);
    bar.type_text("closed.t");
    bar.suggest("closed.t", vec![completing("https://closed.test/", "est")]);
    assert_eq!(
        bar.ends(),
        [("Closed today · Reopen".into(), Some("icons/rotate-ccw.svg"))]
    );
    bar.keys("enter");
    assert_eq!(bar.calls(), ["restore archived 5", "hide"]);
}
