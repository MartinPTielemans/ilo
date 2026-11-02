//! The command bar's keys and completion, on a host that records what the bar asks of it, with the
//! core's suggestions made up.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, ClipboardItem, EntityInputHandler, Modifiers, MouseButton, TestAppContext,
    VisualTestContext, WindowHandle,
};

use super::*;
use crate::text_input::{self, Chosen, FieldMenu};

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
    elsewhere: RefCell<HashMap<u64, u64>>,
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

    fn remove_suggestion(&self, url: &str) {
        self.record(format!("remove {url}"));
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        Some(if text.contains('.') { format!("https://{text}/") } else { search(text) })
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
        self.shown.borrow_mut().push(format!("{place}, {width} wide, radius {radius}"));
        self.levels.borrow_mut().push((level, self.heights.borrow().last().copied()));
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
        shell::find_commands(text, &shell::UserKeys::default(), |name, _| Some(name))
    }
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
        Bar::open(cx, Vec::new(), Recorder { commands: true, ..Recorder::default() })
    }

    /// The bar, opened for a new tab in a Space with `tabs`.
    fn with_tabs(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>) -> Self {
        Bar::open(cx, tabs, Recorder::default())
    }

    fn open(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>, host: Recorder) -> Self {
        Bar::opened(cx, host, |cx| show(None, tabs, None, cx))
    }

    /// The bar, opened for Search Tabs on `tabs`.
    fn search_tabs(cx: &mut TestAppContext, tabs: Vec<shell::BarTab>) -> Self {
        Bar::opened(cx, Recorder::default(), |cx| show_tabs(tabs, cx))
    }

    fn opened(cx: &mut TestAppContext, host: Recorder, show: impl FnOnce(&mut App)) -> Self {
        let host = Rc::new(host);
        cx.update(text_input::bind_keys);
        let window = cx.update(|cx| open(host.clone(), cx)).expect("no window for the bar");
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
        self.window.read_with(&self.cx.cx, |bar, cx| bar.input.read(cx).composing()).unwrap()
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

    /// The rows listed: each one's page, and its tab if it has one.
    fn rows(&mut self) -> Vec<(String, u64)> {
        self.window
            .read_with(&self.cx.cx, |bar, _| {
                bar.rows.iter().map(|row| (row.url.clone(), row.tab)).collect()
            })
            .unwrap()
    }

    /// The bar's window becoming the key one, as the host makes it when it shows the bar.
    fn activate(&mut self) {
        (self.window.update(&mut self.cx.cx, |_, window, _| window.activate_window())).unwrap();
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

    /// Where the host was asked to show the bar since the last time.
    fn shown(&self) -> Vec<String> {
        self.host.shown.take()
    }

    /// Opens the bar again on tab 1's `url`, over `field` if it's the URL field's box.
    fn edit(&mut self, url: &str, field: Option<Bounds<Pixels>>) {
        let editing = Some((1, SharedString::from(url.to_string())));
        self.cx.cx.update(|cx| show(editing, Vec::new(), field, cx));
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

    fn over_field(&mut self) -> bool {
        self.window.read_with(&self.cx.cx, |bar, _| bar.over_field).unwrap()
    }

    /// The height the bar last asked its window to be.
    fn asked_height(&self) -> f32 {
        *self.host.heights.borrow().last().expect("the bar asked for no height")
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
        *self.host.levels.borrow().last().expect("the bar didn't show")
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
        self.cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        self.cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        self.cx.run_until_parked();
        shown.take().expect("a right-click in the input showed no menu")
    }

    /// Chooses `title` in the menu a right-click showed.
    fn choose(&mut self, (items, chosen): (Vec<String>, Chosen), title: &str) {
        let index = items.iter().position(|item| item == title).expect("no such item");
        self.cx.update(|window, cx| chosen(index, window, cx));
        self.cx.run_until_parked();
    }

    fn copy_text(&mut self, text: &str) {
        self.cx.cx.write_to_clipboard(ClipboardItem::new_string(text.into()));
    }

    /// Moves the pointer onto row `index`, with the window at its height for five rows, as the
    /// host makes it.
    fn point_at(&mut self, index: usize) {
        self.cx.simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN))));
        let y = height(0) + LINE + LIST_TOP + (index as f32 + 0.5) * ROW_HEIGHT;
        self.cx.simulate_mouse_move(point(px(200.), px(y)), None, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// A click where row `index` shows its ✕ under the pointer, at its end.
    fn click_remove(&mut self, index: usize) {
        self.point_at(index);
        let x = WIDTH - LIST_X - PLATE_INSET - ROW_END - 3. - ICON_BOX / 2.;
        let y = height(0) + LINE + LIST_TOP + (index as f32 + 0.5) * ROW_HEIGHT;
        self.cx.simulate_click(point(px(x), px(y)), Modifiers::none());
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
    Suggestion { completion: completion.into(), ..page(url) }
}

/// The search row for `text`.
fn searching(text: &str) -> Suggestion {
    Suggestion { removable: false, ..page(&search(text)) }
}

#[gpui::test]
fn return_goes_where_the_completion_goes(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("example.c");
    let rows = vec![completing("https://example.com/", "om"), searching("example.c")];
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
    let shift = Modifiers { shift: true, ..Modifiers::none() };
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
        bar.cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    }
    bar.cx.run_until_parked();
    assert_eq!(bar.input().0, "Go !");
    assert_eq!(shell::web_keystroke("a", Modifiers::control()).key_char, None);
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
    bar.suggest("127.0.0.1:58351", vec![page("http://127.0.0.1:58351/"), page(back)]);
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
    assert_eq!(bar.calls(), [format!("open {}", search("exa")), "hide".into()]);
}

#[gpui::test]
fn arrows_wrap_at_the_ends(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let rows = || vec![searching("ex"), page("https://a.test/"), page("https://b.test/")];
    bar.suggest("ex", rows());
    bar.keys("up enter");
    assert_eq!(bar.calls(), ["open https://b.test/", "hide"]);

    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", rows());
    bar.keys("down down ctrl-n enter");
    assert_eq!(bar.calls(), [format!("open {}", search("ex")), "hide".into()]);
}

#[gpui::test]
fn return_on_a_tab_switches_to_it(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), Suggestion { tab: 7, ..page("https://a.test/") }]);
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
    assert_eq!(bar.calls(), [format!("open {}", search("exa")), "hide".into()]);
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
    assert_eq!(bar.calls(), [format!("open {}", search("two words")), "hide".into()]);
}

#[gpui::test]
fn shift_return_opens_where_return_goes_in_a_new_window(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("exa");
    let rows = vec![completing("https://example.com/", "mple.com"), searching("exa")];
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
    bar.suggest("ex", vec![searching("ex"), Suggestion { tab: 7, ..page("https://a.test/") }]);
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
    assert_eq!(bar.calls(), ["hide".to_string(), format!("window {}", search("two words"))]);
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
    bar.suggest("ex", vec![searching("ex"), Suggestion { tab: 7, ..page("https://a.test/") }]);
    bar.keys("down enter");
    assert_eq!(bar.calls(), ["hide", "activate 3 with 7"]);
}

#[gpui::test]
fn shift_delete_takes_a_page_out_of_history(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/"), page("https://b.test/")]);
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

/// A page from history, or a search typed before, shows a ✕ under the pointer that takes it out,
/// as Arc's does, and ⌥⌘⌫ takes out the selected one, as ⇧⌦ does. The search for what's typed
/// and the well-known sites have none: there a click at the row's end takes the row.
#[gpui::test]
fn the_pointer_shows_a_cross_that_takes_a_suggestion_out(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let typed_before = Suggestion { removable: true, ..searching("example") };
    let site = Suggestion { removable: false, ..page("https://example.com/") };
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
    assert_eq!(bar.calls(), [format!("open {}", search("ex")), "hide".into()]);
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
    Suggestion { keyword: keyword.into(), site: "Duck".into(), ..page(&url) }
}

#[gpui::test]
fn tab_on_a_hinted_row_searches_its_site_until_backspace_leaves(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test"), page("https://a.test/")]);
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
    bar.suggest("ex", vec![searching("ex"), hinting("duck.test"), page("https://a.test/")]);
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

#[gpui::test]
fn a_space_after_the_hinted_keyword_searches_its_site(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.type_text(" ");
    assert_eq!(bar.input(), (String::new(), 0..0));
    // The site's searches only, from the core, which names them by the keyword and the words.
    bar.type_text("cats");
    bar.suggest("duck.test cats", vec![page("https://duck.test/?q=cats&ia=web")]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://duck.test/?q=cats&ia=web", "hide"]);

    // So does Japanese input's space.
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.type_text("\u{3000}");
    assert_eq!(bar.input(), (String::new(), 0..0));

    // Esc leaves, keeping the space, and then closes the bar.
    let mut bar = Bar::new(cx);
    bar.type_text("duck.test");
    bar.suggest("duck.test", vec![hinting("duck.test")]);
    bar.type_text(" cats");
    bar.keys("escape");
    assert_eq!(bar.input(), ("duck.test cats".into(), 10..10));
    bar.keys("escape");
    assert_eq!(bar.calls(), ["dismiss"]);
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
fn a_space_after_other_text_stays_in_it(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("duck");
    bar.suggest("duck", vec![Suggestion { completion: ".test".into(), ..hinting("duck.test") }]);
    bar.type_text(" ");
    assert_eq!(bar.input(), ("duck ".into(), 5..5));
    bar.keys("enter");
    assert_eq!(bar.calls(), [format!("open {}", search("duck")), "hide".into()]);
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
    assert_eq!(bar.calls(), [format!("open {}", search("example.org")), "hide".into()]);

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
    assert_eq!(bar.calls(), [format!("open {}", search("alert(1)")), "hide".into()]);
}

#[test]
fn only_a_leading_javascript_scheme_goes() {
    assert_eq!(without_javascript("\u{1}javascript:\njavascript:x"), "x");
    assert_eq!(without_javascript(" example.org/javascript:x"), " example.org/javascript:x");
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
    assert_eq!(WIDTH, 765.);
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
            bar.rows.iter().map(|row| matches!(row.kind(), Kind::Search)).collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(kinds, [false, true]);
}

/// One of the Space's open tabs.
fn tab(tab: u64, url: &str) -> shell::BarTab {
    shell::BarTab { tab, title: url.to_string().into(), url: url.to_string().into(), favicon: None }
}

#[gpui::test]
fn before_anything_is_typed_it_lists_the_spaces_tabs(cx: &mut TestAppContext) {
    let tabs = vec![tab(4, "https://a.test/"), tab(2, "https://b.test/")];
    let mut bar = Bar::with_tabs(cx, tabs);
    let listed = vec![("https://a.test/".to_string(), 4), ("https://b.test/".to_string(), 2)];
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
    shell::BarTab { title: title.to_string().into(), ..tab(id, url) }
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
    assert_eq!(bar.shown(), ["centred 169 above the middle, 765 wide, radius 20"]);
    let ids = |bar: &mut Bar| bar.rows().into_iter().map(|(_, tab)| tab).collect::<Vec<_>>();
    assert_eq!(ids(&mut bar), [4, 2, 9]);
    bar.type_text("conv");
    assert_eq!(ids(&mut bar), [4, 9]);
    bar.type_text("ex d");
    assert_eq!(ids(&mut bar), [9]);
    bar.keys(&["backspace"; 8].join(" "));
    assert_eq!(ids(&mut bar), [4, 2, 9], "with nothing typed it lists them all again");
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
    let mut tabs: Vec<_> = (1..=20).map(|id| tab(id, &format!("https://site{id}.test/"))).collect();
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
    let open = Suggestion { tab: 4, ..page(CONVEX) };
    bar.suggest("conv", vec![searching("conv"), page("https://conv.test/"), open]);
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
    assert_eq!(bar.calls(), [format!("open {}", search("c")), "hide".into()]);
}

#[gpui::test]
fn typing_that_reads_as_a_url_keeps_chromes_first_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Convex", CONVEX)]);
    bar.type_text("convex.dev");
    bar.suggest("convex.dev", vec![page("https://convex.dev/"), searching("convex.dev")]);
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

#[gpui::test]
fn a_hinted_site_search_or_a_completion_to_another_page_keeps_chromes_first_row(
    cx: &mut TestAppContext,
) {
    let tabs = vec![titled(4, "Convex", CONVEX)];
    let mut bar = Bar::with_tabs(cx, tabs.clone());
    bar.type_text("conv");
    let hint = Suggestion { keyword: "conv.test".into(), ..searching("conv") };
    bar.suggest("conv", vec![hint, page("https://conv.test/")]);
    assert_eq!(bar.rows()[0], (search("conv"), 0));

    let mut bar = Bar::with_tabs(cx, tabs);
    bar.type_text("conv");
    bar.suggest("conv", vec![completing("https://convoy.test/", "oy.test"), searching("conv")]);
    assert_eq!(bar.rows()[0], ("https://convoy.test/".to_string(), 0));
    assert_eq!(bar.input(), ("convoy.test".into(), 4..11));
}

#[gpui::test]
fn a_completion_to_an_open_tabs_page_is_that_tabs_row(cx: &mut TestAppContext) {
    let mut bar = Bar::with_tabs(cx, vec![titled(4, "Home", "https://convex.dev/")]);
    bar.type_text("conv");
    let rows = vec![completing("https://convex.dev/", "ex.dev"), searching("conv")];
    bar.suggest("conv", rows);
    assert_eq!(bar.rows(), [("https://convex.dev/".to_string(), 4), (search("conv"), 0)]);
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
    // The core lists the other as its own row, which stays under the first.
    let other = Suggestion { tab: 4, ..page(CONVEX) };
    bar.suggest("conv", vec![searching("conv"), other]);
    assert_eq!(bar.rows(), [(docs.to_string(), 2), (search("conv"), 0), (CONVEX.to_string(), 4)]);
    bar.keys("enter");
    assert_eq!(bar.calls(), ["select 2", "hide"]);
    let mut bar = Bar::with_tabs(cx, vec![titled(2, "Convex docs", docs)]);
    bar.type_text("conv");
    let other = Suggestion { tab: 4, ..page(CONVEX) };
    bar.suggest("conv", vec![searching("conv"), other]);
    bar.keys("down down enter");
    assert_eq!(bar.calls(), ["select 4", "hide"]);
}

#[gpui::test]
fn the_tab_shown_is_never_offered(cx: &mut TestAppContext) {
    // Editing tab 4's URL, which is the tab shown, and tab 2 is another.
    let mut bar = Bar::with_tabs(cx, Vec::new());
    let docs = titled(2, "Convex docs", "https://docs.convex.dev/");
    let tabs = vec![titled(4, "Convex", CONVEX), docs];
    let editing = Some((4, SharedString::from(CONVEX)));
    bar.cx.cx.update(|cx| show(editing, tabs, None, cx));
    bar.cx.run_until_parked();
    bar.type_text("conv");
    bar.suggest("conv", vec![searching("conv")]);
    assert_eq!(bar.rows()[0], ("https://docs.convex.dev/".to_string(), 2));

    let editing = Some((4, SharedString::from(CONVEX)));
    bar.cx.cx.update(|cx| show(editing, vec![titled(4, "Convex", CONVEX)], None, cx));
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
    assert!(names_tab(&titled(3, "Home", "http://localhost:3000/"), "local"));
}

#[test]
fn a_page_only_in_history_has_a_clock_and_an_open_tab_or_a_site_its_favicon() {
    assert_eq!(page("https://a.test/").icon(), Icon::Glyph("icons/clock.svg"));
    let open = Suggestion { tab: 3, ..page("https://a.test/") };
    assert_eq!(open.icon(), Icon::Favicon);
    // A well-known site isn't in the profile's history.
    let site = Suggestion { removable: false, ..page("https://wikipedia.org/") };
    assert_eq!(site.icon(), Icon::Favicon);
    let typed = Suggestion { search: true, ..searching("a") };
    assert_eq!(typed.icon(), Icon::Glyph("icons/search.svg"));
    let sum = Suggestion { result: Some("2".into()), ..Suggestion::default() };
    assert_eq!(sum.icon(), Icon::None);
}

#[test]
fn a_command_has_its_own_glyph_where_lucide_names_it_and_each_is_built_in() {
    use gpui::AssetSource;
    use shell::Command;
    let glyph = |command| {
        Suggestion { command: Some(command), ..Suggestion::default() }.icon()
    };
    assert_eq!(glyph(Command::Clear), Icon::Glyph("icons/brush-cleaning.svg"));
    assert_eq!(glyph(Command::ClearArchive), Icon::Glyph("icons/archive.svg"));
    assert_eq!(glyph(Command::CollapseFolders), Icon::Glyph("icons/fold-vertical.svg"));
    assert_eq!(glyph(Command::SeparateSplit), Icon::Glyph(COMMAND_GLYPH));
    let commands = [
        Command::Clear, Command::ClearCacheAndRefresh, Command::ClearCookiesAndRefresh,
        Command::ClearArchive, Command::CollapseFolders, Command::NewFolder, Command::TogglePin,
        Command::ToggleMute, Command::CloseTab, Command::ReopenClosedTab, Command::CopyUrl,
        Command::Find, Command::Back, Command::Forward, Command::Reload, Command::Stop,
        Command::ZoomIn, Command::ZoomOut, Command::FlipSplit, Command::ToggleSidebar,
        Command::SavePage, Command::Print, Command::ViewSource, Command::InspectElements,
        Command::JavaScriptConsole, Command::ManageExtensions, Command::RenameSpace,
        Command::NextTab, Command::PreviousTab, Command::ViewLibrary, Command::ViewDownloads,
        Command::ViewHistory, Command::Settings, Command::KeyboardShortcuts, Command::About,
        Command::Import, Command::NewSpace, Command::SiteSearchSettings, Command::ClearBrowsingData,
        Command::HelpCenter, Command::WhatsNew, Command::ReportProblem, Command::CopyBuildInfo,
        Command::PrivacyPolicy,
    ];
    for command in commands {
        let Icon::Glyph(path) = glyph(command) else {
            panic!("{command:?} has no glyph");
        };
        assert!(shell::Assets.load(path).unwrap().is_some(), "{path} isn't built in");
    }
}

/// Arc lights the row under the pointer with its own plate while the selection keeps the accent,
/// and the keys move only the selection.
#[gpui::test]
fn the_row_under_the_pointer_has_a_plate_as_the_keys_move_the_selection(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    let rows = vec![searching("ex"), page("https://a.test/"), page("https://b.test/")];
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
    assert_eq!(bar.calls(), [format!("open {}", search("ex")), "hide".into()]);
}

#[gpui::test]
fn the_pointer_moving_takes_the_plate_with_it(cx: &mut TestAppContext) {
    let mut bar = Bar::new(cx);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/"), page("https://b.test/")]);
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
    assert_eq!(bar.shown(), ["centred 169 above the middle, 765 wide, radius 20"]);
    let field = Bounds::new(point(px(8.), px(46.)), size(px(244.), px(36.)));
    bar.edit("https://example.com/", Some(field));
    assert_eq!(bar.shown(), ["at 8 44, 350 wide, radius 20"]);
    assert!(bar.over_field());
    assert_eq!(bar.input(), ("https://example.com/".into(), 0..20));
    bar.edit("https://example.com/", None);
    assert_eq!(bar.shown(), ["centred 169 above the middle, 765 wide, radius 20"]);
    assert!(!bar.over_field());
}

/// Over the URL field the window is as tall as what the bar draws, asked for before it shows: with
/// no rows, as rows come, as they go, and on opening again after a taller panel or the centred one
/// (#1306).
#[gpui::test]
fn over_the_field_the_window_is_as_tall_as_what_the_bar_draws(cx: &mut TestAppContext) {
    let tabs = vec![titled(4, "Projects", CONVEX), titled(2, "Inbox", "https://mail.test/")];
    let mut bar = Bar::with_tabs(cx, tabs);
    assert_eq!(bar.asked_height(), height(2), "centred, with the Space's tabs");
    let field = Bounds::new(point(px(8.), px(46.)), size(px(244.), px(36.)));
    let url = "https://example.com/";
    bar.edit(url, Some(field));
    assert_eq!(bar.last_show(), (FIELD_LEVEL, Some(FIELD_INPUT)), "the input row, before it shows");
    assert_eq!(bar.drawn_over_field(), FIELD_INPUT);
    bar.type_text("ex");
    bar.suggest("ex", vec![searching("ex"), page("https://a.test/")]);
    assert_eq!(bar.drawn_over_field(), bar.asked_height());
    assert_eq!(bar.asked_height(), field_height(2));
    let many: Vec<_> = (0..7).map(|at| page(&format!("https://{at}.test/"))).collect();
    bar.type_text("x");
    bar.suggest("exx", many);
    assert_eq!(bar.drawn_over_field(), bar.asked_height());
    assert_eq!(bar.asked_height(), field_height(ROWS_SHOWN));
    bar.type_text("q");
    bar.suggest("exxq", Vec::new());
    assert_eq!(bar.asked_height(), FIELD_INPUT, "no rows left");
    assert_eq!(bar.drawn_over_field(), FIELD_INPUT);
    bar.type_text("z");
    bar.suggest("exxqz", (0..7).map(|at| page(&format!("https://{at}.test/"))).collect());
    bar.keys("escape");
    bar.edit(url, Some(field));
    assert_eq!(bar.last_show(), (FIELD_LEVEL, Some(FIELD_INPUT)), "after the taller panel");
    bar.edit(url, None);
    assert_eq!(bar.last_show(), (Level::Modal, Some(height(0))));
    bar.edit(url, Some(field));
    assert_eq!(bar.last_show(), (FIELD_LEVEL, Some(FIELD_INPUT)), "after the centred one");
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
    assert!(level.reach().below < FIELD_INPUT, "{level:?} reaches {:?}", level.reach());
    bar.edit("https://example.com/", None);
    assert_eq!(bar.last_show().0, Level::Modal, "the centred bar keeps its modal shadow");
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
    assert_eq!(bar.listed(), [search("q"), "https://quora.com/".into(), quit.into()]);
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
        "New Note Split".into(),
    ];
    assert_eq!(bar.listed(), expected);
    bar.keys("cmd-a");
    type_and_suggest(&mut bar, "library", suggestions("library", &[]));
    assert_eq!(bar.listed(), ["Open Library… ⇧⌘L".to_string(), search("library")]);
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
        assert_eq!(bar.listed()[..2], [search("sp"), "https://spotify.com/".into()], "{leave}");
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
    bar.suggest("exa", vec![completing("https://example.com/", "mple.com"), searching("exa")]);
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
    assert_eq!(bar.rows(), [("https://b.test/".to_string(), 9), ("https://c.test/".into(), 8)]);

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
    let expected =
        ["Duplicate Current Tab".to_string(), search("duplicate"), "New Window ⌘N".into()];
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

/// Arc's "tab", where a site completes what's typed: the site first, then the core's rows, then
/// the commands, to Arc's 15 rows at most.
#[gpui::test]
fn a_site_completing_puts_every_command_after_the_suggestions(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.type_text("tab");
    let hosts = ["tabs.test", "tablet.test", "table.test", "tableau.test", "tabel.test"];
    let mut rows = vec![completing("https://tabelog.com/", "elog.com"), searching("tab")];
    rows.extend(hosts.iter().chain(&["a.test", "b.test", "c.test"]).map(|host| {
        page(&format!("https://{host}/"))
    }));
    bar.suggest("tab", rows);
    let listed = bar.listed();
    assert_eq!(listed.len(), shell::BAR_ROWS);
    assert_eq!(listed[0], "https://tabelog.com/");
    let commands =
        ["Pin Tab ⌘D", "Next Tab ⌥⌘↓", "Search Tabs… ⇧⌘A", "Archive Tab ⌘W", "Clear Today ⇧⌘K"];
    assert_eq!(listed[10..], commands);
}

/// Fifteen rows show five at a time, and the list scrolls to keep the selection in view.
#[gpui::test]
fn the_list_scrolls_to_keep_the_selection_in_view(cx: &mut TestAppContext) {
    let mut bar = Bar::with_commands(cx);
    bar.cx.simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN))));
    type_and_suggest(&mut bar, "c", suggestions("c", &["c1.test", "c2.test", "c3.test"]));
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
    bar.cx.simulate_resize(size(px(WIDTH), px(height(ROWS_SHOWN) + 1.)));
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
    let docs = Suggestion { text: "Docs".into(), detail: "docs.test".into(), tab: 7, ..docs };
    bar.suggest("do", vec![Suggestion { text: "do".into(), ..searching("do") }, docs]);
    let tree = bar.cx.a11y_tree().expect("GPUI built a tree");
    let nodes = |role: Role| -> Vec<_> {
        tree.nodes.iter().filter(|(_, node)| node.role() == role).cloned().collect()
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
        [("do".into(), Some(true)), ("Docs, docs.test, Switch to Tab".into(), Some(false))]
    );
    let status = &nodes(Role::Status)[0].1;
    assert_eq!((status.live(), status.value()), (Some(Live::Polite), Some("do, 1 of 2")));

    bar.keys("down");
    let tree = bar.cx.a11y_tree().expect("GPUI built a tree");
    let status = tree.nodes.iter().find(|(_, node)| node.role() == Role::Status).unwrap();
    assert_eq!(status.1.value(), Some("Docs, docs.test, Switch to Tab, 2 of 2"));
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
    cx.update(|cx| menu::open(Rc::new(MenuWindow), cx)).expect("no window for the menu");
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
        bar.cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        bar.cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
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
    let window = cx.update(|cx| menu::open(Rc::new(MenuWindow), cx)).expect("no window for the menu");
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
    let focused = tree.nodes.iter().find(|(id, _)| *id == tree.focus).map(|(_, node)| node);
    assert_eq!(focused.and_then(|node| node.label()), Some("Paste"), "Cut and Copy are dimmed");
    in_menu.simulate_keystrokes("enter");
    bar.cx.run_until_parked();
    let text = bar.window.update(&mut bar.cx.cx, |bar, _, cx| bar.input.read(cx).text().to_owned());
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
    assert_eq!(bar.listed(), ["https://meet.new/", "https://meet./", &search("meet.")]);
    let named = bar.window.read_with(&bar.cx.cx, |bar, _| bar.rows[0].text.clone()).unwrap();
    assert_eq!(named, "New Google Meet");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["open https://meet.new/", "hide"]);

    bar.edit("https://example.com/", None);
    bar.type_text("docs.new");
    bar.suggest("docs.new", vec![page("https://docs.new/"), searching("docs.new")]);
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
    assert_eq!(bar.host.asked.take().last().map(|(text, _)| text.as_str()), Some("cats and "));
    bar.keys("left left left left cmd-backspace");
    assert_eq!(bar.input(), ("and ".into(), 0..0));
    assert_eq!(bar.host.asked.take().last().map(|(text, _)| text.as_str()), Some("and "));

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
        (bar.window.update(&mut bar.cx.cx, |_, window, cx| window.blur(cx))).unwrap();
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
