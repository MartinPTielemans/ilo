//! Capture mode's keys and clicks, on a host that records what it asks of its window, the core
//! and the system.

use std::{
    cell::{Cell, RefCell},
    io::Cursor,
};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;
use crate::tokens::menu::OVERLAP;

const TAB: u64 = 7;

/// What it was asked, the last picture copied, and whether the core refuses a capture put on a
/// Board at once, as when the Board's folder is gone.
#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, RefCell<Option<Vec<u8>>>, Cell<bool>);

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
        self.1.replace(Some(png.to_vec()));
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

    fn desktop_picture(&self) -> Option<Vec<u8>> {
        None
    }

    fn read_boards(&self, tab: u64) {
        self.record(format!("read boards {tab}"));
    }

    fn new_board(&self, tab: u64, id: &str, png: &[u8]) {
        self.record(format!("new board {tab} {id} {}", png.len()));
    }

    fn add_to_board(&self, tab: u64, board: &Board, png: &[u8]) -> bool {
        self.record(format!("add to board {tab} {} {}", board.id, png.len()));
        !self.2.get()
    }

    fn open_board(&self, id: &str) {
        self.record(format!("open board {id}"));
    }
}

/// The toast's window, which shows nothing here.
struct Toasts;

impl toast::Host for Toasts {
    fn show(&self) {}
    fn hide(&self) {}
    fn take_clicks(&self, _: bool) {}
    fn plate(&self, _: Bounds<Pixels>) {}
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
        cx.update(|cx| {
            let look = |_: &App| Default::default();
            toast::open(Rc::new(Toasts), look, cx)
        })
        .expect("no window for the toast");
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
        self.captured_as(request, png());
    }

    fn captured_as(&mut self, request: u64, png: Vec<u8>) {
        self.cx
            .cx
            .update(|cx| captured(TAB, request, Some(png), cx));
        self.cx.run_until_parked();
    }

    /// The pointer to `at`, and a click there, as a mouse moves before it clicks.
    fn click_at(&mut self, (x, y): (f32, f32)) {
        let at = point(px(x), px(y));
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        self.click(x, y);
    }

    /// A drag with the button down, from `from` to `to`, as one move.
    fn drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        let (none, left) = (Modifiers::none(), MouseButton::Left);
        let (from, to) = (point(px(from.0), px(from.1)), point(px(to.0), px(to.1)));
        self.cx.simulate_mouse_move(from, None, none);
        self.cx.simulate_mouse_down(from, left, none);
        self.cx.simulate_mouse_move(to, left, none);
        self.cx.simulate_mouse_up(to, left, none);
        self.cx.run_until_parked();
    }

    /// The marks on what's chosen, and whether Edit's bar shows.
    fn marks(&mut self) -> (usize, bool) {
        let window = self.window;
        let marks = |capture: &mut Capture| {
            let session = capture.session.as_ref().unwrap();
            (session.marks.len(), session.editing.is_some())
        };
        self.cx
            .cx
            .update(|cx| window.update(cx, |capture, _, _| marks(capture)))
            .unwrap()
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
        // A window with a Library asks for its profile's Boards, for Add to Board.
        let asked = [
            format!("capture {TAB} 1 view"),
            format!("read boards {TAB}"),
        ];
        let shown = "show 0 0 800 600".to_string();
        let calls = match library {
            true => vec![asked[0].clone(), asked[1].clone(), shown],
            false => vec![asked[0].clone(), shown],
        };
        assert_eq!(mode.calls(), calls);
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
    let started = [
        format!("capture {TAB} 2 view"),
        format!("read boards {TAB}"),
    ];
    assert_eq!(mode.calls(), started);
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
        [
            &[
                format!("capture {TAB} 2 view"),
                format!("read boards {TAB}")
            ],
            &left[..]
        ]
        .concat()
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
    let shown = [
        format!("capture {TAB} 1 view"),
        format!("read boards {TAB}"),
        "show 0 0 800 600".into(),
    ];
    assert_eq!(mode.calls(), shown);
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

/// The row under the pointer is lit, its title with it (#3137): one row at a time, the field too,
/// and none once the pointer leaves the menu.
#[gpui::test]
fn the_row_under_the_pointer_is_lit_and_none_off_the_menu(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.hover(100., 100., rect(50., 60., 200., 80.));
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    let window = mode.window;
    let mut lit_at = |y: f32| {
        let at = point(px(menu.x + MENU_WIDTH / 2.), px(y));
        mode.cx.simulate_mouse_move(at, None, Modifiers::none());
        mode.cx.run_until_parked();
        let lit = |capture: &mut Capture| capture.session.as_ref().and_then(|s| s.lit);
        mode.cx
            .cx
            .update(|cx| window.update(cx, |capture, _, _| lit(capture)))
            .unwrap()
    };
    assert_eq!(
        lit_at(menu.y + row(Action::SaveToLibrary)),
        Some(Action::SaveToLibrary)
    );
    assert_eq!(lit_at(menu.y + row(Action::Copy)), Some(Action::Copy));
    assert_eq!(lit_at(menu.y + PADDING + ROW / 2.), Some(Action::SendTo));
    assert_eq!(lit_at(menu.y + row(Action::Retake)), Some(Action::Retake));
    assert_eq!(lit_at(590.), None);
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

/// Portrait Mode asks for the page as it shows, without capture mode, and copies the picture it
/// makes of it, larger than the page by its margin, and lets the core go of the page.
#[gpui::test]
fn portrait_mode_copies_the_page_on_its_background(cx: &mut TestAppContext) {
    let mut mode = Mode::new(cx);
    // The page is 8 pixels wide at 2 to a point.
    let frame = rect(0., 0., 4., 3.);
    let background = portrait::Background::White;
    mode.cx
        .cx
        .update(|cx| portrait(TAB, frame, 1., background, cx));
    mode.cx.run_until_parked();
    assert_eq!(mode.calls(), [format!("capture {TAB} 1 view")]);
    mode.captured(1);
    let calls = mode.calls();
    assert_eq!(calls[0], format!("end {TAB}"));
    let copied: usize = (calls[1].strip_prefix("copy "))
        .and_then(|size| size.parse().ok())
        .expect("Portrait Mode copied nothing");
    assert!(
        copied > png().len(),
        "the copy isn't the portrait: {calls:?}"
    );
}

/// The middle of Edit's bar's `part`, for what's chosen at `chosen` in an 800 by 600 page: under
/// it, at its ring's left.
fn part(chosen: Bounds<Pixels>, part: Part) -> (f32, f32) {
    let index = Part::all().position(|each| each == part).unwrap() as f32;
    let x = f32::from(chosen.left()) - RING + PADDING + index * (BUTTON + BAR_GAP) + BUTTON / 2.;
    let y = f32::from(chosen.bottom()) + RING + MARGIN + PADDING + BUTTON / 2.;
    (x, y)
}

/// A white picture of what's chosen, 200 by 80, as the core sends it at 1×.
fn white() -> Vec<u8> {
    let mut png = Vec::new();
    RgbaImage::from_pixel(200, 80, image::Rgba([255; 4]))
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    png
}

/// Edit's marks are drawn where they were made, in the picture the menu's Copy then copies: a red
/// pen's line, first chosen, from the box's left across it.
#[gpui::test]
fn edit_draws_on_the_picture_copy_takes(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let chosen = rect(50., 60., 200., 80.);
    mode.hover(100., 100., chosen);
    mode.click(100., 100.);
    assert_eq!(mode.calls(), ["camera false"]);
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Edit));
    // No picture is asked for yet.
    assert!(mode.calls().is_empty());
    assert_eq!(mode.marks(), (0, true));
    assert_eq!(mode.drawn(), Some(("markup", false)));
    mode.drag((80., 100.), (180., 100.));
    assert_eq!(mode.marks(), (1, true));
    // Return keeps them and brings the menu back, whose Copy takes the picture with them.
    mode.keys("enter");
    assert_eq!(mode.marks(), (1, false));
    assert_eq!(mode.drawn(), Some(("menu", false)));
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Copy));
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured_as(2, white());
    let calls = mode.calls();
    assert_eq!(calls[..2], ["hide".to_string(), format!("end {TAB}")]);
    assert!(calls[2].starts_with("copy "), "{calls:?}");
    let copied = mode.host.1.take().unwrap();
    let copied = image::load_from_memory(&copied).unwrap().into_rgba8();
    assert_eq!(copied.dimensions(), (200, 80));
    let [r, g, b] = [16, 8, 0].map(|shift| (MARK_COLOURS[0] >> shift) as u8);
    assert_eq!(
        copied.get_pixel(80, 40).0,
        [r, g, b, 255],
        "no line across the box"
    );
    assert_eq!(
        copied.get_pixel(80, 10).0,
        [255; 4],
        "the line is elsewhere"
    );
}

/// Edit's bar picks a tool, ⌘Z takes the last mark back, a press away from what's chosen
/// doesn't leave, and Escape takes back that Edit's marks and then leaves.
#[gpui::test]
fn edit_undoes_and_escape_goes_back(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let chosen = rect(50., 60., 200., 80.);
    mode.hover(100., 100., chosen);
    mode.click(100., 100.);
    mode.calls();
    let menu = point(50. - RING + MENU_LEFT, 60. - RING);
    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Edit));
    mode.drag((60., 70.), (240., 130.));
    let (x, y) = part(chosen, Part::Tool(Tool::Box));
    mode.click(x, y);
    // A click isn't a box.
    mode.click_at((150., 100.));
    assert_eq!(mode.marks(), (1, true));
    mode.drag((60., 70.), (240., 130.));
    assert_eq!(mode.marks(), (2, true));
    mode.keys("cmd-z");
    assert_eq!(mode.marks(), (1, true));
    let (x, y) = part(chosen, Part::Done);
    mode.click(x, y);
    assert_eq!(mode.marks(), (1, false));

    mode.click(menu.x + MENU_WIDTH / 2., menu.y + row(Action::Edit));
    mode.drag((60., 70.), (240., 130.));
    mode.click_at((700., 500.));
    assert_eq!(mode.marks(), (2, true));
    assert!(
        mode.calls().is_empty(),
        "a press away from what's chosen did something"
    );
    mode.keys("escape");
    assert_eq!(mode.marks(), (1, false));
    mode.keys("escape");
    assert_eq!(mode.calls(), ["hide".to_string(), format!("end {TAB}")]);
}

/// The profile's Boards in the order Library › Boards lists them, the last changed first.
const BOARDS: [&str; 3] = ["Kyoto trip", "Kitchen ideas", "Reading list"];

impl Mode {
    /// The menu over the element at 50, 60, its corner returned, with the profile's Boards
    /// `names`, each id `b` and where it is in the list.
    fn menu_with(&mut self, names: &[&str]) -> Point<f32> {
        self.hover(100., 100., rect(50., 60., 200., 80.));
        self.click(100., 100.);
        assert_eq!(self.calls(), ["camera false"]);
        self.boards(names);
        point(50. - RING + MENU_LEFT, 60. - RING)
    }

    /// The core's list of the profile's Boards, named `names`.
    fn boards(&mut self, names: &[&str]) {
        let boards = (names.iter().enumerate())
            .map(|(at, name)| Board {
                id: format!("b{at}"),
                name: name.to_string().into(),
            })
            .collect();
        self.cx.cx.update(|cx| boards_listed(TAB, boards, cx));
        self.cx.run_until_parked();
    }

    /// The pointer to `x`, `y`.
    fn point_to(&mut self, x: f32, y: f32) {
        let at = point(px(x), px(y));
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// The row lit, whether Add to Board's submenu shows, and the rows drawn lit.
    fn menu_state(&mut self) -> (Option<Action>, bool, [Option<Action>; 2]) {
        let window = self.window;
        let state = |capture: &mut Capture| {
            let session = capture.session.as_ref().unwrap();
            (session.lit, session.submenu, session.lit_rows())
        };
        self.cx
            .cx
            .update(|cx| window.update(cx, |capture, _, _| state(capture)))
            .unwrap()
    }

    /// Where Add to Board's submenu is, while it shows.
    fn submenu_at(&mut self) -> Option<Bounds<Pixels>> {
        let window = self.window;
        let at = |capture: &mut Capture| {
            let session = capture.session.as_ref().unwrap();
            session
                .submenu_bounds(session.chosen?)
                .filter(|_| session.submenu)
        };
        self.cx
            .cx
            .update(|cx| window.update(cx, |capture, _, _| at(capture)))
            .unwrap()
    }

    /// The core's word on the capture put on Board `id`.
    fn answer(&mut self, id: &str, arrived: bool) {
        self.cx
            .cx
            .update(|cx| board_answered(BoardAnswer { id, arrived }, cx));
        self.cx.run_until_parked();
    }

    fn toast_shows(&mut self, text: &str) -> bool {
        self.cx.cx.update(|cx| toast::showing(text, cx))
    }

    /// A click on the toast's button `name`.
    fn press_toast(&mut self, name: &str) {
        let pressed = self.cx.cx.update(|cx| toast::press(name, cx));
        assert!(pressed, "the toast showing has no {name} button");
        self.cx.run_until_parked();
    }
}

/// The toast a Board with no room for a capture shows, naming it and saying what to do.
fn no_room(board: &str) -> String {
    let most = boards::MOST_WAITING;
    format!("{board} has {most} new captures you haven't seen. Open it, then add this again.")
}

/// The pointer on Add to Board opens its submenu beside the menu, its first row level with Add to
/// Board's; a Board under the pointer lights with Add to Board lit too, as menu.rs keeps a
/// submenu's parent lit, and a click adds the capture to it and leaves; the toast names it.
#[gpui::test]
fn the_pointer_opens_add_to_board_and_a_click_adds_to_the_board(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let menu = mode.menu_with(&BOARDS);
    let add = menu.y + row(Action::AddToBoard);
    mode.point_to(menu.x + MENU_WIDTH / 2., add);
    let parent = Some(Action::AddToBoard);
    assert_eq!(mode.menu_state(), (parent, true, [parent, parent]));
    let submenu = mode
        .submenu_at()
        .expect("Add to Board's submenu didn't show");
    // OVERLAP over the menu's right edge, its first row level with Add to Board's.
    assert_eq!(f32::from(submenu.left()), menu.x + MENU_WIDTH - OVERLAP);
    assert_eq!(f32::from(submenu.top()) + PADDING, add - ROW / 2.);
    let x = f32::from(submenu.left()) + 40.;
    mode.point_to(x, add);
    let first = Some(Action::Board(0));
    assert_eq!(mode.menu_state(), (first, true, [first, parent]));
    let second = Some(Action::Board(1));
    mode.point_to(x, add + ROW);
    assert_eq!(mode.menu_state(), (second, true, [second, parent]));
    mode.click(x, add + ROW);
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured(2);
    let len = png().len();
    let added = [
        "hide".to_string(),
        format!("end {TAB}"),
        format!("add to board {TAB} b1 {len}"),
    ];
    assert_eq!(mode.calls(), added);
    // The toast waits for the core's word, which says the capture is there.
    assert!(!mode.toast_shows("Added to Kitchen ideas"));
    // A word on another Board's capture isn't this one's.
    mode.answer("b0", true);
    assert!(!mode.toast_shows("Added to Kitchen ideas"));
    mode.answer("b1", true);
    assert!(mode.toast_shows("Added to Kitchen ideas"));
}

/// Another of the menu's rows under the pointer closes the submenu, but on the pointer's way to
/// it the submenu stays, until the pointer rests, as menu.rs's safe triangle keeps one.
#[gpui::test]
fn the_submenu_stays_on_the_pointers_way_to_it(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let menu = mode.menu_with(&BOARDS);
    let (add, new) = (
        menu.y + row(Action::AddToBoard),
        menu.y + row(Action::NewBoard),
    );
    let parent = Some(Action::AddToBoard);
    mode.point_to(menu.x + 20., add);
    assert_eq!(mode.menu_state(), (parent, true, [parent, parent]));
    // Across New Board, toward the submenu.
    mode.point_to(menu.x + MENU_WIDTH - 12., new);
    assert_eq!(mode.menu_state(), (parent, true, [parent, parent]));
    mode.cx.executor().advance_clock(Duration::from_millis(300));
    mode.cx.run_until_parked();
    let lit = Some(Action::NewBoard);
    assert_eq!(mode.menu_state(), (lit, false, [lit, None]));
    // Away from it, the submenu closes at once.
    mode.point_to(menu.x + 20., add);
    assert_eq!(mode.menu_state(), (parent, true, [parent, parent]));
    mode.point_to(menu.x + 10., menu.y + row(Action::Retake));
    let lit = Some(Action::Retake);
    assert_eq!(mode.menu_state(), (lit, false, [lit, None]));
}

/// The keys as menu.rs has them: → or ↩ on Add to Board opens its submenu with its first Board
/// lit, ↑ and ↓ stay at the ends, ← or esc closes only the submenu, and ↩ on a Board adds to it.
#[gpui::test]
fn the_keys_open_add_to_board_and_return_adds_to_the_board(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.menu_with(&BOARDS);
    let parent = Some(Action::AddToBoard);
    // From none, ↑ lights the last row, New Board, and ↓ stays there.
    mode.keys("up");
    assert_eq!(mode.menu_state().0, Some(Action::NewBoard));
    mode.keys("down");
    assert_eq!(mode.menu_state().0, Some(Action::NewBoard));
    mode.keys("up");
    assert_eq!(mode.menu_state(), (parent, false, [parent, None]));
    mode.keys("right");
    let first = Some(Action::Board(0));
    assert_eq!(mode.menu_state(), (first, true, [first, parent]));
    mode.keys("up");
    assert_eq!(mode.menu_state().0, first);
    mode.keys("down down down down");
    assert_eq!(mode.menu_state().0, Some(Action::Board(2)));
    mode.keys("left");
    assert_eq!(mode.menu_state(), (parent, false, [parent, None]));
    mode.keys("enter");
    assert_eq!(mode.menu_state(), (first, true, [first, parent]));
    // esc closes the submenu, and capture mode stays.
    mode.keys("escape");
    assert_eq!(mode.menu_state(), (parent, false, [parent, None]));
    assert!(mode.calls().is_empty());
    // ← with no submenu does nothing.
    mode.keys("left");
    assert_eq!(mode.menu_state(), (parent, false, [parent, None]));
    mode.keys("right down enter");
    assert_eq!(mode.calls(), [format!("capture {TAB} 2 50 60 200 80")]);
    mode.captured(2);
    let len = png().len();
    assert_eq!(mode.calls()[2], format!("add to board {TAB} b1 {len}"));
}

/// Add to Board shows only where there's a Board to add to: not before the profile's Boards are
/// read, not when it has none, and never in a private window, which asks for none, nor New Board.
/// It lists ten, the last changed first: an eleventh is reached by opening it.
#[gpui::test]
fn add_to_board_needs_a_board_and_a_library(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    mode.menu_with(&[]);
    mode.keys("up");
    assert_eq!(mode.menu_state().0, Some(Action::NewBoard));
    mode.keys("up");
    assert_eq!(mode.menu_state().0, Some(Action::Retake));
    // Another tab's Boards aren't this page's.
    mode.cx.cx.update(|cx| {
        boards_listed(
            TAB + 1,
            vec![Board {
                id: "b0".into(),
                name: "Kyoto trip".into(),
            }],
            cx,
        )
    });
    mode.cx.run_until_parked();
    mode.keys("down");
    assert_eq!(mode.menu_state().0, Some(Action::NewBoard));
    let twelve: Vec<String> = (0..12).map(|at| format!("Board {at}")).collect();
    mode.boards(&twelve.iter().map(String::as_str).collect::<Vec<_>>());
    mode.keys("up right down down down down down down down down down down down");
    assert_eq!(mode.menu_state().0, Some(Action::Board(boards::LISTED - 1)));

    let mut private = Mode::shown_with(false, cx);
    private.menu_with(&BOARDS);
    private.keys("up");
    assert_eq!(private.menu_state().0, Some(Action::Retake));
    private.keys("right");
    assert_eq!(
        private.menu_state(),
        (Some(Action::Retake), false, [Some(Action::Retake), None])
    );
}

/// The core refuses a capture when the Board can take no more arrivals: nothing is stored and
/// capture mode has left, so the toast names the Board and what to do, with Open to go there and
/// Copy to keep the capture another way.
#[gpui::test]
fn a_board_with_no_room_says_so_and_offers_open_and_copy(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let menu = mode.menu_with(&BOARDS);
    let add = menu.y + row(Action::AddToBoard);
    mode.point_to(menu.x + MENU_WIDTH / 2., add);
    let x = f32::from(mode.submenu_at().unwrap().left()) + 40.;
    mode.click_at((x, add));
    mode.captured(2);
    let len = png().len();
    assert_eq!(mode.calls()[3], format!("add to board {TAB} b0 {len}"));
    mode.answer("b0", false);
    assert!(mode.toast_shows(&no_room("Kyoto trip")));
    mode.press_toast("open");
    assert_eq!(mode.calls(), ["open board b0"]);

    // Refused at once, as when the Board's folder is gone: the same toast, and Copy copies.
    mode.host.2.set(true);
    mode.start();
    mode.captured(3);
    mode.calls();
    let menu = mode.menu_with(&BOARDS);
    mode.point_to(menu.x + MENU_WIDTH / 2., add);
    mode.click_at((x, add + 2. * ROW));
    mode.captured(4);
    assert_eq!(mode.calls()[3], format!("add to board {TAB} b2 {len}"));
    assert!(mode.toast_shows(&no_room("Reading list")));
    mode.press_toast("copy");
    assert_eq!(mode.calls(), [format!("copy {len}")]);
    assert!(mode.toast_shows(COPIED));
}

/// New Board makes a Board of the capture, which opens with it, so only a refusal says anything.
#[gpui::test]
fn new_board_makes_a_board_of_the_capture(cx: &mut TestAppContext) {
    let mut mode = Mode::shown(cx);
    let menu = mode.menu_with(&BOARDS);
    mode.click_at((menu.x + 20., menu.y + row(Action::NewBoard)));
    mode.captured(2);
    let calls = mode.calls();
    let len = png().len();
    let id = (calls[3].strip_prefix(&format!("new board {TAB} ")))
        .and_then(|rest| rest.strip_suffix(&format!(" {len}")))
        .unwrap_or_else(|| panic!("New Board made no Board: {calls:?}"))
        .to_string();
    mode.answer(&id, true);
    assert!(!mode.toast_shows("Added to Board"));
    assert!(!mode.toast_shows(&no_room("Board")));
}
