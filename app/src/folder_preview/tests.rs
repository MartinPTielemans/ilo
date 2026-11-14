//! When a closed folder's preview shows and goes, what its search finds and which tab it shows, on
//! a host that records what the preview asks of its window.

use std::{cell::RefCell, time::Duration};

use gpui::{Modifiers, Pixels, Point, Size, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32, height: f32, from: f32) {
        self.0.borrow_mut().push(format!(
            "show {left} {top} {height} from {from} over {window}"
        ));
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

fn window() -> Size<Pixels> {
    size(px(1000.), px(700.))
}

/// The row of folder `folder`, the sidebar's rows being 36 apart from 100 down.
fn row(folder: u64) -> Anchor {
    let top = 100. + 36. * folder as f32;
    Anchor {
        row: Bounds::new(point(px(8.), px(top)), size(px(224.), px(34.))),
        window: window(),
        left: false,
    }
}

fn item(tab: u64, title: &str, url: &str) -> Item {
    let (title, url) = (title.to_string().into(), url.into());
    Item {
        tab,
        title,
        url,
        icon: Icon::Favicon(None),
        age: "1 hour ago".into(),
    }
}

/// Folder 1, Reading, with three tabs, and folder 2, Work, with one.
fn rest(folder: u64, chose: &Rc<RefCell<Vec<u64>>>) -> Rest {
    let items = match folder {
        1 => vec![
            item(11, "Rust Book", "https://doc.rust-lang.org/book/"),
            item(12, "Hacker News", "https://news.ycombinator.com/"),
            item(13, "The Rust Blog", "https://blog.rust-lang.org/"),
        ],
        _ => vec![item(21, "Mail", "https://mail.example.com/")],
    };
    let chose = chose.clone();
    Rest {
        window: 1,
        folder,
        name: "Reading".into(),
        items,
        anchor: row(folder),
        chosen: Rc::new(move |tab, _| chose.borrow_mut().push(tab)),
    }
}

struct Preview {
    host: Rc<Recorder>,
    chose: Rc<RefCell<Vec<u64>>>,
    window: WindowHandle<FolderPreview>,
}

fn open_preview(cx: &mut TestAppContext) -> Preview {
    let host = Rc::new(Recorder::default());
    cx.update(crate::text_input::bind_keys);
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no preview window");
    let preview = Preview {
        host,
        chose: Rc::default(),
        window,
    };
    // The pointer is on the folder's row in the sidebar, not on the preview beside it.
    preview.pointer_at(OFF, cx);
    preview
}

const OFF: Point<Pixels> = point(px(-100.), px(-100.));

impl Preview {
    fn rest_on(&self, folder: u64, cx: &mut TestAppContext) {
        self.rest_in(1, folder, cx);
    }

    /// The pointer rests on `folder`'s row in the sidebar of browser window `window`.
    fn rest_in(&self, window: u64, folder: u64, cx: &mut TestAppContext) {
        let rest = Rest {
            window,
            ..rest(folder, &self.chose)
        };
        cx.update(|cx| ask(Ask::Rest(rest), cx));
        cx.run_until_parked();
    }

    fn ask(&self, what: Ask, cx: &mut TestAppContext) {
        cx.update(|cx| ask(what, cx));
        cx.run_until_parked();
    }

    fn after(&self, time: Duration, cx: &mut TestAppContext) -> Vec<String> {
        cx.executor().advance_clock(time);
        cx.run_until_parked();
        self.host.0.take()
    }

    /// Rests on `folder` until its preview shows, and forgets what the host was asked.
    fn shown(&self, folder: u64, cx: &mut TestAppContext) {
        self.rest_on(folder, cx);
        self.after(DWELL, cx);
    }

    fn showing(&self, cx: &mut TestAppContext) -> Option<(u64, Vec<u64>, Option<u64>)> {
        self.window
            .read_with(cx, |preview, _| preview.showing())
            .unwrap()
    }

    fn keys(&self, keys: &str, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(self.window.into(), cx);
        visual.simulate_keystrokes(keys);
        visual.run_until_parked();
    }

    fn pointer_at(&self, at: Point<Pixels>, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(self.window.into(), cx);
        visual.simulate_mouse_move(at, None, Modifiers::none());
        visual.run_until_parked();
    }

    fn type_in(&self, text: &str, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(self.window.into(), cx);
        visual.simulate_input(text);
        visual.run_until_parked();
    }
}

/// Where the host is told the preview of `folder`, with `count` tabs, goes, in browser window 1.
fn shown_at(folder: u64, count: usize) -> String {
    shown_in(1, folder, count)
}

fn shown_in(window: u64, folder: u64, count: usize) -> String {
    let height = height(count);
    let (left, top) = place(row(folder), height);
    format!("show {left} {top} {height} from {FROM} over {window}")
}

#[gpui::test]
fn a_closed_folder_s_preview_shows_once_the_pointer_has_rested_on_its_row(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.rest_on(1, cx);
    assert!(
        preview
            .after(DWELL - Duration::from_millis(10), cx)
            .is_empty()
    );
    assert_eq!(
        preview.after(Duration::from_millis(10), cx),
        [shown_at(1, 3)]
    );
    assert_eq!(
        preview.showing(cx),
        Some((1, vec![11, 12, 13], Some(11))),
        "the first is selected"
    );
}

#[gpui::test]
fn under_reduce_motion_it_fades_in_without_growing(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    cx.update(|cx| cx.set_reduce_motion(true));
    preview.rest_on(2, cx);
    let shown = preview.after(DWELL, cx);
    assert_eq!(
        shown,
        [shown_at(2, 1).replace(&format!("from {FROM}"), "from 1")]
    );
}

#[gpui::test]
fn the_pointer_passing_over_a_folder_opens_nothing(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.rest_on(1, cx);
    preview.ask(Ask::Left(1), cx);
    assert!(preview.after(DWELL * 2, cx).is_empty());
    assert_eq!(preview.showing(cx), None);
}

#[gpui::test]
fn the_preview_goes_once_the_pointer_has_left_its_row_and_it(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.ask(Ask::Left(1), cx);
    assert!(
        preview.after(LEAVE / 2, cx).is_empty(),
        "the pointer may be crossing onto it"
    );
    assert_eq!(preview.after(LEAVE, cx), ["hide"]);
}

#[gpui::test]
fn the_pointer_on_the_preview_keeps_it(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.ask(Ask::Left(1), cx);
    preview.pointer_at(point(px(20.), px(20.)), cx);
    assert!(
        preview.after(LEAVE * 4, cx).is_empty(),
        "it stays while the pointer is on it"
    );
    preview.pointer_at(OFF, cx);
    assert_eq!(preview.after(LEAVE, cx), ["hide"]);
}

// As the preview's window comes up it can hear the pointer come and go at a place it last had, the
// pointer still on the folder's row; so it goes only once the pointer has left both.
#[gpui::test]
fn the_pointer_off_the_preview_but_on_its_row_keeps_it(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.pointer_at(point(px(20.), px(20.)), cx);
    preview.pointer_at(OFF, cx);
    assert!(
        preview.after(LEAVE * 4, cx).is_empty(),
        "it stays while the pointer is on its row"
    );
    preview.ask(Ask::Left(1), cx);
    assert_eq!(preview.after(LEAVE, cx), ["hide"]);
}

#[gpui::test]
fn the_next_folder_s_preview_takes_its_place_at_once(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.ask(Ask::Left(1), cx);
    preview.rest_on(2, cx);
    assert_eq!(preview.after(Duration::ZERO, cx), [shown_at(2, 1)]);
    assert!(
        preview.after(LEAVE * 2, cx).is_empty(),
        "leaving the first doesn't end the second"
    );
}

#[gpui::test]
fn it_shows_over_the_window_whose_row_the_pointer_rests_on(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    // A window behind another still hears the pointer, so its folder's preview shows over it
    // (#1432), and the next folder's, in the window in front, over that one.
    preview.rest_in(2, 2, cx);
    assert_eq!(preview.after(DWELL, cx), [shown_in(2, 2, 1)]);
    preview.ask(Ask::Left(2), cx);
    preview.rest_in(1, 1, cx);
    assert_eq!(preview.after(Duration::ZERO, cx), [shown_in(1, 1, 3)]);
}

#[gpui::test]
fn typing_narrows_the_list_and_selects_the_first_match(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.type_in("rust", cx);
    assert_eq!(preview.showing(cx), Some((1, vec![11, 13], Some(11))));
    // By the address too, ignoring case.
    preview.keys("backspace backspace backspace backspace", cx);
    preview.type_in("YCOMB", cx);
    assert_eq!(preview.showing(cx), Some((1, vec![12], Some(12))));
    preview.type_in("zzz", cx);
    assert_eq!(preview.showing(cx), Some((1, Vec::new(), None)));
    // Cleared, it lists them all again with the first selected.
    preview.keys(&"backspace ".repeat(8), cx);
    assert_eq!(preview.showing(cx), Some((1, vec![11, 12, 13], Some(11))));
}

#[gpui::test]
fn return_shows_the_tab_selected_and_the_preview_goes(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.type_in("rust", cx);
    preview.keys("down enter", cx);
    assert_eq!(*preview.chose.borrow(), [13]);
    assert_eq!(preview.host.0.take(), ["hide"]);
    assert_eq!(preview.showing(cx), None);
}

#[gpui::test]
fn the_arrows_select_without_typing_and_stop_at_the_ends(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.keys("up", cx);
    assert_eq!(
        preview.showing(cx).unwrap().2,
        Some(11),
        "↑ on the first stays there"
    );
    preview.keys("down down down", cx);
    assert_eq!(preview.showing(cx).unwrap().2, Some(13));
    preview.keys("up", cx);
    assert_eq!(preview.showing(cx).unwrap().2, Some(12));
}

#[gpui::test]
fn return_at_once_shows_the_first_tab(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.keys("enter", cx);
    assert_eq!(*preview.chose.borrow(), [11]);
}

#[gpui::test]
fn once_something_is_typed_the_pointer_leaving_doesn_t_end_it(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    preview.type_in("r", cx);
    preview.ask(Ask::Left(1), cx);
    assert!(preview.after(LEAVE * 2, cx).is_empty());
    preview.keys("escape", cx);
    assert_eq!(preview.host.0.take(), ["hide"]);
    assert!(preview.chose.borrow().is_empty(), "escape shows nothing");
}

#[gpui::test]
fn a_click_outside_leaves_it_without_showing_a_tab(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    preview.shown(1, cx);
    cx.update(dismissed);
    assert_eq!(preview.showing(cx), None);
    assert!(preview.host.0.take().is_empty(), "the host hid it itself");
    assert!(preview.chose.borrow().is_empty());
}

#[gpui::test]
fn an_empty_folder_has_no_preview(cx: &mut TestAppContext) {
    let preview = open_preview(cx);
    let empty = Rest {
        items: Vec::new(),
        ..rest(1, &preview.chose)
    };
    preview.ask(Ask::Rest(empty), cx);
    assert!(preview.after(DWELL * 2, cx).is_empty());
}

#[test]
fn it_is_arcs_height_and_scrolls_past_320() {
    // Arc's 53 + 45 a row, and 3 more under the last row's fill, as far as the sides.
    assert_eq!(height(1), 101.);
    assert_eq!(height(5), 281.);
    assert_eq!(
        height(6),
        320.,
        "the sixth row is cut, so the list reads as one that scrolls"
    );
    assert_eq!(height(30), 320.);
    assert_eq!(
        height(0),
        height(1),
        "an empty search keeps a row for its words"
    );
}

#[test]
fn it_is_beside_the_sidebar_centred_on_its_row() {
    // The row's plate ends 8 inside the sidebar, which ends at 240.
    let (left, top) = place(row(1), height(3));
    assert_eq!(left, 244.);
    // Its middle on the row's, 153, to the whole point: 191 tall, so 57.5 rounds to 58.
    assert_eq!((height(3), top), (191., 58.));
}

#[test]
fn it_stays_18_inside_the_window_beside_a_row_high_or_low_in_it() {
    let at = |top: f32| Anchor {
        row: Bounds::new(point(px(8.), px(top)), size(px(224.), px(34.))),
        ..row(1)
    };
    assert_eq!(place(at(4.), height(6)).1, 18.);
    assert_eq!(place(at(650.), height(6)).1 + height(6), 700. - 18.);
    // In a window too narrow for it beside the sidebar, it keeps inside the window's right side.
    let narrow = Anchor {
        window: size(px(400.), px(700.)),
        ..row(1)
    };
    assert_eq!(place(narrow, height(3)).0, 400. - WIDTH - 18.);
}

/// With the sidebar on the right, the preview opens on the folder's left, toward the page, as far
/// from the sidebar's edge as on the left (#2375).
#[test]
fn a_preview_opens_toward_the_page_from_a_sidebar_on_the_right() {
    let left = place(row(1), height(3)).0;
    let mirrored = Anchor {
        row: Bounds::new(
            point(px(1000. - 8. - 224.), px(136.)),
            size(px(224.), px(34.)),
        ),
        left: true,
        ..row(1)
    };
    assert_eq!(place(mirrored, height(3)).0, 1000. - left - WIDTH);
}
