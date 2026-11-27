//! What the switcher draws and tells the shell, on a host that records what it asks of its window:
//! its keys move the selection and never reach the page, and a long list scrolls the least.

use std::cell::RefCell;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, window: u64, height: f32) {
        self.0
            .borrow_mut()
            .push(format!("show {height} over {window}"));
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

fn item(tab: u64) -> Item {
    Item {
        tab,
        title: format!("Tab {tab}").into(),
        host: "example.com".into(),
        icon: Icon::Favicon(None),
        glyph: None,
        sleeping: false,
        speaker: None,
    }
}

struct Switcher {
    host: Rc<Recorder>,
    heard: Rc<RefCell<Vec<Heard>>>,
    window: WindowHandle<TabSwitcher>,
}

fn open_switcher(cx: &mut TestAppContext) -> Switcher {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no switcher window");
    Switcher {
        host,
        heard: Rc::default(),
        window,
    }
}

impl Switcher {
    /// Shows tabs 1 to `count` over browser window 7, with row `selected` selected.
    fn show(&self, count: u64, selected: usize, cx: &mut TestAppContext) {
        let heard = self.heard.clone();
        let show = Show {
            window: 7,
            items: (1..=count).map(item).collect(),
            selected,
            heard: Rc::new(move |each, _| heard.borrow_mut().push(each)),
        };
        self.ask(Ask::Show(show), cx);
    }

    fn ask(&self, what: Ask, cx: &mut TestAppContext) {
        cx.update(|cx| ask(what, cx));
        cx.run_until_parked();
    }

    fn keys(&self, keys: &str, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(self.window.into(), cx);
        visual.simulate_keystrokes(keys);
        visual.run_until_parked();
    }

    /// Presses `button` on row `at` of three, with `modifiers` held, as the platform hands it to
    /// GPUI's window.
    fn press(&self, at: usize, button: MouseButton, modifiers: Modifiers, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(self.window.into(), cx);
        // As tall as its rows, as the host makes it, with the room on Windows for its own shadow.
        let room = Level::Popover.panel_margin();
        visual.simulate_resize(room.around(size(px(WIDTH), px(height(3)))));
        let y = room.above + LIST_X + (at as f32 + 0.5) * ROW_HEIGHT;
        visual.simulate_mouse_down(point(px(room.sides + WIDTH / 2.), px(y)), button, modifiers);
        visual.run_until_parked();
    }

    fn heard(&self) -> Vec<Heard> {
        self.heard.take()
    }

    fn asked(&self) -> Vec<String> {
        self.host.0.take()
    }

    fn showing(&self, cx: &mut TestAppContext) -> Option<(Vec<u64>, u64)> {
        self.window
            .read_with(cx, |switcher, _| switcher.showing())
            .unwrap()
    }

    fn scrolled(&self, cx: &mut TestAppContext) -> f32 {
        self.window
            .read_with(cx, |switcher, _| -f32::from(switcher.scroll.offset().y))
            .unwrap()
    }
}

#[gpui::test]
fn it_shows_over_the_browser_window_as_tall_as_its_rows(cx: &mut TestAppContext) {
    let switcher = open_switcher(cx);
    switcher.show(3, 1, cx);
    assert_eq!(switcher.asked(), [format!("show {} over 7", height(3))]);
    assert_eq!(switcher.showing(cx), Some((vec![1, 2, 3], 2)));
    switcher.ask(Ask::Close, cx);
    assert_eq!(switcher.asked(), ["hide"]);
    assert_eq!(switcher.showing(cx), None);
    // Closed, it asks nothing more of its window.
    switcher.ask(Ask::Close, cx);
    assert!(switcher.asked().is_empty());
}

#[test]
fn past_its_rows_it_is_no_taller() {
    let rows = ROWS as usize;
    assert!(height(1) < height(2));
    assert_eq!(height(rows), height(rows + 5));
    assert_eq!(height(0), height(1));
}

#[gpui::test]
fn tab_and_the_arrows_go_on_or_back_and_escape_stays(cx: &mut TestAppContext) {
    let switcher = open_switcher(cx);
    switcher.show(3, 1, cx);
    // ⌃ is held while it shows, so its keys come with ⌃ or without, where a system takes them.
    switcher.keys("ctrl-tab tab right ctrl-right", cx);
    assert_eq!(switcher.heard(), [Heard::Step(true); 4]);
    switcher.keys("ctrl-shift-tab shift-tab left ctrl-left", cx);
    assert_eq!(switcher.heard(), [Heard::Step(false); 4]);
    switcher.keys("escape", cx);
    assert_eq!(switcher.heard(), [Heard::Stay]);
    // It only tells: the shell moves the selection.
    assert_eq!(switcher.showing(cx), Some((vec![1, 2, 3], 2)));
}

#[gpui::test]
fn the_shell_moves_the_selection(cx: &mut TestAppContext) {
    let switcher = open_switcher(cx);
    switcher.show(3, 1, cx);
    switcher.asked();
    switcher.ask(Ask::Select(2), cx);
    assert_eq!(switcher.showing(cx), Some((vec![1, 2, 3], 3)));
    // Moving it asks nothing of the window.
    assert!(switcher.asked().is_empty());
}

#[gpui::test]
fn a_click_outside_stays_and_leaving_the_app_lets_go(cx: &mut TestAppContext) {
    let switcher = open_switcher(cx);
    switcher.show(3, 1, cx);
    cx.update(|cx| dismissed(cx));
    cx.run_until_parked();
    // The test has no active window, as when the user left the app.
    assert_eq!(switcher.heard(), [Heard::LetGo]);
    assert_eq!(switcher.showing(cx), None);
    // Dismissed again, it was already gone, and tells nothing.
    cx.update(|cx| dismissed(cx));
    assert!(switcher.heard().is_empty());
}

#[gpui::test]
fn a_press_on_a_row_with_ctrl_held_picks_it(cx: &mut TestAppContext) {
    let switcher = open_switcher(cx);
    // ⌃ is held while it shows. On Windows and Linux a press comes as it is.
    switcher.show(3, 0, cx);
    switcher.press(2, MouseButton::Left, Modifiers::control(), cx);
    assert_eq!(switcher.heard(), [Heard::Chose(2)]);
    // On the Mac, GPUI hands a ⌃-left press on as a right press with ⌃ cleared (#3360).
    switcher.show(3, 0, cx);
    switcher.press(1, MouseButton::Right, Modifiers::none(), cx);
    assert_eq!(switcher.heard(), [Heard::Chose(1)]);
    // A right press the user made, ⌃ still held, picks nothing.
    switcher.show(3, 0, cx);
    switcher.press(1, MouseButton::Right, Modifiers::control(), cx);
    assert!(switcher.heard().is_empty());
}

#[gpui::test]
fn the_list_scrolls_the_least_that_shows_the_selected_row_whole(cx: &mut TestAppContext) {
    let rows = ROWS as usize;
    // In view, it stays.
    assert_eq!(scrolled_to(3, 0., rows), 0.);
    // Below, the row comes up to the bottom edge; above, down to the top.
    let below = scrolled_to(rows, 0., rows);
    assert_eq!(below, ROW_HEIGHT);
    assert_eq!(scrolled_to(0, below, rows), 0.);

    let switcher = open_switcher(cx);
    switcher.show(rows as u64 + 4, rows + 1, cx);
    assert_eq!(switcher.scrolled(cx), 2. * ROW_HEIGHT);
    switcher.ask(Ask::Select(rows + 3), cx);
    assert_eq!(switcher.scrolled(cx), 4. * ROW_HEIGHT);
    switcher.ask(Ask::Select(5), cx);
    assert_eq!(switcher.scrolled(cx), 4. * ROW_HEIGHT);
    switcher.ask(Ask::Select(0), cx);
    assert_eq!(switcher.scrolled(cx), 0.);
    // Shown again, it starts from the top.
    switcher.show(rows as u64 + 4, 2, cx);
    assert_eq!(switcher.scrolled(cx), 0.);
}
