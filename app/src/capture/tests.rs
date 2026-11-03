//! Capture mode's keys and clicks, on a host that records what it asks of its window, the core
//! and the system.

use std::{cell::RefCell, io::Cursor};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

const TAB: u64 = 7;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Recorder {
    fn record(&self, call: String) {
        self.0.borrow_mut().push(call);
    }
}

impl Host for Recorder {
    fn show(&self, frame: Bounds<Pixels>) {
        self.record(format!("show {}", describe(frame)));
    }

    fn hide(&self) {
        self.record("hide".into());
    }

    fn camera(&self, camera: bool) {
        self.record(format!("camera {camera}"));
    }

    fn element_at(&self, tab: u64, at: Point<Pixels>) {
        self.record(format!(
            "element at {tab} {} {}",
            f32::from(at.x),
            f32::from(at.y)
        ));
    }

    fn capture(&self, tab: u64, request: u64, full_page: bool, rect: Option<Bounds<Pixels>>) {
        let what = match (full_page, rect) {
            (true, _) => "full page".to_string(),
            (false, Some(rect)) => describe(rect),
            (false, None) => "view".to_string(),
        };
        self.record(format!("capture {tab} {request} {what}"));
    }

    fn end(&self, tab: u64) {
        self.record(format!("end {tab}"));
    }

    fn copy(&self, png: &[u8]) {
        self.record(format!("copy {}", png.len()));
    }

    fn save(&self, png: &[u8]) {
        self.record(format!("save {}", png.len()));
    }

    fn save_to_library(&self, png: &[u8]) {
        self.record(format!("save to library {}", png.len()));
    }

    fn share(&self, png: &[u8], share: Share) {
        self.record(format!("share {} {share:?}", png.len()));
    }
}

/// A picture as the core sends one.
fn png() -> Vec<u8> {
    let mut png = Vec::new();
    RgbaImage::new(8, 6)
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    png
}

/// Capture mode's window over a page that fills it.
struct Mode {
    cx: VisualTestContext,
    window: WindowHandle<Capture>,
    host: Rc<Recorder>,
    /// Whether the window has a Library, as a private one hasn't.
    library: bool,
}

impl Mode {
    fn new(cx: &mut TestAppContext) -> Self {
        let host = Rc::new(Recorder::default());
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for Capture");
        let cx = VisualTestContext::from_window(window.into(), cx);
        Mode {
            cx,
            window,
            host,
            library: true,
        }
    }

    /// What ⇧⌘2 does: the shell's `start`.
    fn start(&mut self) {
        self.start_from(None);
    }

    /// The shell's `start`, for ⇧⌘2 or, with `press`, a ⇧⌘-drag on the page from there.
    fn start_from(&mut self, press: Option<Point<Pixels>>) {
        let frame = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
        let library = self.library;
        self.cx
            .cx
            .update(|cx| start(TAB, frame, 8., library, Look::dark(), press, cx));
        self.cx.run_until_parked();
    }

    fn captured(&mut self, request: u64) {
        self.cx
            .cx
            .update(|cx| captured(TAB, request, Some(png()), cx));
        self.cx.run_until_parked();
    }

    /// ⇧⌘2, and the picture of the page the window shows with.
    fn shown(cx: &mut TestAppContext) -> Self {
        Mode::shown_with(true, cx)
    }

    /// ⇧⌘2 in a window with a Library, or in a private one without.
    fn shown_with(library: bool, cx: &mut TestAppContext) -> Self {
        let mut mode = Mode::new(cx);
        mode.library = library;
        mode.start();
        mode.captured(1);
        assert_eq!(
            mode.calls(),
            [format!("capture {TAB} 1 view"), "show 0 0 800 600".into()]
        );
        mode
    }

    /// The pointer over an element, and the box the core answers with.
    fn hover(&mut self, x: f32, y: f32, element: Bounds<Pixels>) {
        let at = point(px(x), px(y));
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        self.cx.run_until_parked();
        assert_eq!(self.calls(), [format!("element at {TAB} {x} {y}")]);
        self.cx
            .cx
            .update(|cx| element_box(TAB, at, Some(element), cx));
        self.cx.run_until_parked();
    }

    fn click(&mut self, x: f32, y: f32) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn calls(&self) -> Vec<String> {
        self.host.0.take()
    }

    /// The browser window over `tab` draws its menus light, or dark, now.
    fn look(&mut self, tab: u64, light: bool) {
        let look = Look {
            light,
            ..Look::dark()
        };
        self.cx.cx.update(|cx| set_look(tab, look, cx));
        self.cx.run_until_parked();
    }

    /// What capture mode last drew, the pill or the menu, and whether in the light look.
    fn drawn(&mut self) -> Option<(&'static str, bool)> {
        let window = self.window;
        let drawn =
            |capture: &mut Capture| capture.session.as_ref().and_then(|session| session.drawn);
        self.cx
            .cx
            .update(|cx| window.update(cx, |capture, _, _| drawn(capture)))
            .unwrap()
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The menu's rows start under the field and its line, inside the panel's padding.
const ROWS_TOP: f32 = PADDING + ROW + LINE;

/// A row's middle in the menu, from the menu's corner: the field, then 24 a row and 9 a line, as
/// ilo's menu has them. As a window with a Library has them.
fn row(action: Action) -> f32 {
    let mut top = ROWS_TOP;
    for row in ROWS {
        match row {
            Some(row) if *row == action => return top + ROW / 2.,
            Some(_) => top += ROW,
            None => top += LINE,
        }
    }
    panic!("no row for {action:?}");
}

#[gpui::test]
fn escape_leaves_and_shift_cmd_2_comes_back(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.keys("escape");
    assert_eq!(mode.calls(), ["hide".to_string(), format!("end {TAB}")]);
    // Esc again has nothing to leave.
    mode.keys("escape");
    assert!(mode.calls().is_empty());
    mode.start();
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 view")]);
}

#[gpui::test]
fn the_second_shift_cmd_2_leaves(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.start();
    assert_eq!(mode.calls(), ["hide".to_string(), format!("end {TAB}")]);
    // Also before the picture of the page has come, which then has nowhere to show.
    mode.start();
    mode.start();
    mode.captured(2);
    let left = ["hide".to_string(), format!("end {TAB}")];
    assert_eq!(
        mode.calls(),
        [&[format!("capture {TAB} 2 view")], &left[..]].concat()
    );
}

#[gpui::test]
fn a_click_takes_the_element_under_the_pointer(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    // The menu is 10 in from the ring's left, level with its top; Copy copies the element's box.
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Copy));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured(2);
    let len = png().len();
    assert_eq!(
        mode.calls(),
        [
            "hide".to_string(),
            format!("end {TAB}"),
            format!("copy {len}")
        ]
    );
}

#[gpui::test]
fn a_drag_takes_the_rectangle_dragged(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    let (left, button) = (Modifiers::none(), MouseButton::Left);
    mode.cx
        .simulate_mouse_down(point(px(100.), px(100.)), button, left);
    // Under 4 pt it's still a click; past it, a drag, which asks the core nothing as it goes.
    mode.cx
        .simulate_mouse_move(point(px(102.), px(101.)), button, left);
    mode.cx
        .simulate_mouse_move(point(px(300.), px(250.)), button, left);
    mode.cx
        .simulate_mouse_up(point(px(300.), px(250.)), button, left);
    mode.cx.run_until_parked();
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(100. - RING + MENU_LEFT, 100. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Save));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 100 100 200 150")]);
    mode.captured(2);
    let len = png().len();
    assert_eq!(
        mode.calls(),
        [
            "hide".to_string(),
            format!("end {TAB}"),
            format!("save {len}")
        ]
    );
}

#[gpui::test]
fn a_shift_cmd_drag_on_the_page_takes_the_rectangle_dragged(cx: &mut TestAppContext) {
    let mut mode = Mode::new(cx);
    // The page's window keeps the pointer, so its moves come from the core, the first before the
    // picture of the page has.
    mode.start_from(Some(point(px(100.), px(100.))));
    mode.cx
        .cx
        .update(|cx| page_dragged(TAB, point(px(200.), px(150.)), cx));
    mode.captured(1);
    assert_eq!(
        mode.calls(),
        [format!("capture {TAB} 1 view"), "show 0 0 800 600".into()]
    );
    // Another ⇧⌘-drag's start, past its window, doesn't leave, as ⇧⌘2's second press does.
    mode.start_from(Some(point(px(10.), px(10.))));
    mode.cx
        .cx
        .update(|cx| page_dragged(TAB, point(px(300.), px(250.)), cx));
    mode.cx.cx.update(|cx| page_released(TAB, cx));
    mode.cx.run_until_parked();
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(100. - RING + MENU_LEFT, 100. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Copy));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 100 100 200 150")]);
}

#[gpui::test]
fn a_press_that_barely_moves_is_a_click(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    let (left, button) = (Modifiers::none(), MouseButton::Left);
    mode.cx
        .simulate_mouse_down(point(px(100.), px(100.)), button, left);
    mode.cx
        .simulate_mouse_move(point(px(102.), px(101.)), button, left);
    mode.cx
        .simulate_mouse_up(point(px(102.), px(101.)), button, left);
    mode.cx.run_until_parked();
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Copy));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
}

#[gpui::test]
fn retake_chooses_again(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Retake));
    // No picture is taken: the pointer is the camera again, and asks about what it's over.
    assert_eq!(mode.calls(), ["camera true"]);
    mode.hover(400., 300., rect(380., 280., 100., 50.));
    mode.click(400., 300.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(380. - RING + MENU_LEFT, 280. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Copy));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 380 280 100 50")]);
}

#[gpui::test]
fn a_click_away_from_the_menu_leaves(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    mode.click(700., 500.);
    assert_eq!(mode.calls(), ["hide".to_string(), format!("end {TAB}")]);
}

#[gpui::test]
fn a_click_over_nothing_chooses_nothing(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.click(100., 100.);
    assert!(mode.calls().is_empty());
    mode.keys("escape");
    assert_eq!(mode.calls(), ["hide".to_string(), format!("end {TAB}")]);
}

/// Send To shows the system's share menu from its field. Linux has none, so there it copies, as
/// Copy does, and its toast says so.
#[gpui::test]
fn send_to_copies_where_the_system_has_no_share_menu(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + PADDING + ROW / 2.);
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured(2);
    let calls = mode.calls();
    assert_eq!(calls[..2], ["hide".to_string(), format!("end {TAB}")]);
    let len = png().len();
    match SHARES {
        true => assert!(calls[2].starts_with(&format!("share {len} Menu("))),
        false => assert_eq!(calls[2], format!("copy {len}")),
    }
}

/// Save to Library is Arc's first row. It saves the picture with no question and says so; a
/// private window has no Library, so its menu starts at Copy.
#[gpui::test]
fn save_to_library_is_the_first_row_but_in_a_private_window(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    let first = ROWS_TOP + ROW / 2.;
    assert_eq!(row(Action::SaveToLibrary), first);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + first);
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured(2);
    let len = png().len();
    let saved = [
        "hide".to_string(),
        format!("end {TAB}"),
        format!("save to library {len}"),
    ];
    assert_eq!(mode.calls(), saved);

    let mut private = Mode::shown_with(false, cx);
    private.hover(100., 100., rect(50., 60., 200., 80.));
    private.click(100., 100.);
    assert_eq!(private.calls(), ["camera false"]);
    private.click(menu.x + MENU_WIDTH / 2., menu.y + first);
    private.captured(2);
    let copied = [
        "hide".to_string(),
        format!("end {TAB}"),
        format!("copy {len}"),
    ];
    assert_eq!(private.calls()[1..], copied);
}

#[gpui::test]
fn it_takes_the_windows_look_as_it_changes(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    assert_eq!(mode.drawn(), Some(("pill", false)));
    // Another window's look isn't this one's.
    mode.look(TAB + 1, true);
    assert_eq!(mode.drawn(), Some(("pill", false)));
    mode.look(TAB, true);
    assert_eq!(mode.drawn(), Some(("pill", true)));
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.drawn(), Some(("menu", true)));
    mode.look(TAB, false);
    assert_eq!(mode.drawn(), Some(("menu", false)));
}
