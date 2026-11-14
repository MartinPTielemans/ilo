//! Where the note panel goes beside an element, and its ↩ Add and esc, on a host that records what
//! it asks of its window.

use std::cell::RefCell;

use gpui::{TestAppContext, VisualTestContext};

use super::*;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

const PAGE: (f32, f32, f32, f32) = (0., 0., 1000., 800.);

fn page() -> Bounds<Pixels> {
    bounds(PAGE.0, PAGE.1, PAGE.2, PAGE.3)
}

#[test]
fn it_goes_left_of_the_element_with_room_there() {
    let at = bounds(600., 300., 200., 100.);
    let placed = place(at, page(), 150.);
    assert_eq!(placed.origin.x, px(600. - GAP - WIDTH));
    // Its top a little over the element's, so the field is level with the mark's label.
    assert_eq!(placed.origin.y, px(300. - ABOVE));
    assert_eq!(placed.size, size(px(WIDTH), px(150.)));
}

#[test]
fn it_goes_right_when_the_left_has_no_room() {
    let at = bounds(100., 300., 200., 100.);
    assert_eq!(place(at, page(), 150.).origin.x, px(300. + GAP));
}

#[test]
fn over_an_elements_left_when_neither_side_has_room() {
    // As wide as most of the page: the panel covers its left, inside the page.
    let at = bounds(100., 300., 800., 100.);
    assert_eq!(place(at, page(), 150.).origin.x, px(100.));
    let wide = bounds(500., 300., 480., 100.);
    assert_eq!(
        place(wide, bounds(400., 0., 600., 800.), 150.).origin.x,
        px(500.)
    );
}

#[test]
fn it_stays_in_the_page() {
    // At the page's top, and at its foot.
    assert_eq!(
        place(bounds(600., 5., 100., 40.), page(), 150.).origin.y,
        px(0.)
    );
    let low = place(bounds(600., 780., 100., 20.), page(), 150.);
    assert_eq!(low.origin.y, px(800. - 150.));
}

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, window: u64, _: Bounds<Pixels>) {
        self.0.borrow_mut().push(format!("show {window}"));
    }

    fn resize(&self, _: f32) {}

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

/// The element's picture is drawable on the panel's first frame, in the BGRA GPUI draws.
#[test]
fn the_picture_is_decoded_as_it_opens() {
    let red = image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255]));
    let mut png = Vec::new();
    red.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let picture = decoded(&png).expect("a PNG decodes");
    assert_eq!(picture.size(0), size(3.into(), 2.into()));
    assert_eq!(&picture.as_bytes(0).unwrap()[..4], &[0, 0, 255, 255]);
    assert!(decoded(b"not a png").is_none());
}

/// The panel's window, and what its note's Add and its closing told the shell.
fn panel_window(
    cx: &mut TestAppContext,
) -> (
    Rc<Recorder>,
    Rc<RefCell<Vec<String>>>,
    WindowHandle<NotePanel>,
) {
    let host = Rc::new(Recorder::default());
    let told = Rc::new(RefCell::new(Vec::new()));
    let window = cx.update(|cx| {
        open_at(host.clone(), bounds(0., 0., WIDTH, 120.), true, cx).expect("the panel's window")
    });
    (host, told, window)
}

fn panel(told: &Rc<RefCell<Vec<String>>>) -> Panel {
    let added = told.clone();
    let dropped = told.clone();
    Panel {
        number: 1,
        agent: "Claude".into(),
        element: Element {
            kind: "section".into(),
            name: "Trending".into(),
            ..Default::default()
        },
        at: bounds(600., 300., 200., 100.),
        page: page(),
        palette: Palette::default(),
        added: Rc::new(move |words, _| added.borrow_mut().push(format!("added {words}"))),
        dropped: Rc::new(move |_| dropped.borrow_mut().push("dropped".into())),
    }
}

#[gpui::test]
fn add_notes_what_the_user_wrote_and_esc_drops_it(cx: &mut TestAppContext) {
    let (host, told, window) = panel_window(cx);
    cx.update(|cx| show(4, panel(&told), cx));
    assert_eq!(host.0.take(), ["show 4"]);

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_input("The title is cut off");
    visual.simulate_keystrokes("enter");
    assert_eq!(host.0.take(), ["hide"]);
    assert_eq!(told.take(), ["added The title is cut off"]);

    // A new note starts empty; esc closes it without a note.
    cx.update(|cx| show(4, panel(&told), cx));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_keystrokes("escape");
    assert_eq!(host.0.take(), ["show 4", "hide"]);
    assert_eq!(told.take(), ["dropped"]);
}

/// Past its most height the opened rows scroll, and fade at their bottom edge while more is below
/// it, as Settings' card does, until they're scrolled to their end.
#[gpui::test]
fn opened_rows_past_its_height_fade_until_their_end(cx: &mut TestAppContext) {
    let (_, told, window) = panel_window(cx);
    let mut long = panel(&told);
    long.element.text = (1..=60)
        .map(|n| format!("Line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|cx| show(4, long, cx));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    // A frame shows what the last layout's scroll does, so two: as the window asks for one more
    // when they differ.
    let more = |visual: &mut VisualTestContext| {
        for _ in 0..2 {
            visual.update(|window, _| window.refresh());
            visual.run_until_parked();
        }
        window.update(visual, |note, _, _| note.more_below).unwrap()
    };
    assert_eq!(more(&mut visual), None, "folded, it has no rows");
    _ = window.update(&mut visual, |note, _, cx| note.toggle(cx));
    assert_eq!(more(&mut visual), Some(true));
    _ = window.update(&mut visual, |note, _, cx| {
        let end = note.rows.max_offset().y;
        note.rows.set_offset(point(px(0.), -end));
        cx.notify();
    });
    assert_eq!(more(&mut visual), Some(false));
}

#[gpui::test]
fn the_same_note_asked_again_only_moves_and_keeps_its_words(cx: &mut TestAppContext) {
    let (host, told, window) = panel_window(cx);
    cx.update(|cx| show(4, panel(&told), cx));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_input("Too small");
    // The page scrolled: the same note, its element higher.
    let mut moved = panel(&told);
    moved.at = bounds(600., 200., 200., 100.);
    cx.update(|cx| show(4, moved, cx));
    assert_eq!(host.0.take(), ["show 4", "show 4"]);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_keystrokes("enter");
    assert_eq!(told.take(), ["added Too small"]);
}

#[gpui::test]
fn a_click_outside_drops_the_note(cx: &mut TestAppContext) {
    let (_, told, _) = panel_window(cx);
    cx.update(|cx| {
        show(4, panel(&told), cx);
        dismissed(cx);
        // Once closed, another click outside drops nothing.
        dismissed(cx);
    });
    assert_eq!(told.take(), ["dropped"]);
}

#[gpui::test]
fn hidden_with_its_page_it_doesnt_drop_the_note(cx: &mut TestAppContext) {
    let (host, told, _) = panel_window(cx);
    cx.update(|cx| {
        show(4, panel(&told), cx);
        // Another window's page going from view leaves it.
        hide(5, cx);
        hide(4, cx);
    });
    assert_eq!(host.0.take(), ["show 4", "hide"]);
    assert_eq!(told.take(), Vec::<String>::new());
}

/// A picture in its row is drawn at the column's width, its height kept in proportion, so the
/// rows end with it (#2462's capture had its full height under it, empty, as the panel's end).
#[test]
fn a_rows_picture_is_as_tall_as_it_is_drawn() {
    // Point's 520 × 320 section with 8 around it, at 1× and 2×.
    let (w, h) = fitted((536., 336.), 1., 250., 160.);
    assert_eq!((w, (h * 10.).round() / 10.), (250., 156.7));
    assert_eq!(fitted((1072., 672.), 2., 250., 160.), (w, h));
    // A tall one is held to the most, and a small one keeps its own size.
    assert_eq!(fitted((100., 400.), 1., 250., 160.), (40., 160.));
    assert_eq!(fitted((120., 60.), 1., 250., 160.), (120., 60.));
}

/// Scrolled to their end, the rows show the picture whole, its foot inside theirs (#2462's capture
/// had it cut by the first row's gap, which the scroll's end leaves out).
#[gpui::test]
fn at_the_rows_end_the_picture_shows_whole(cx: &mut TestAppContext) {
    let (_, told, window) = panel_window(cx);
    let mut long = panel(&told);
    long.element.text = (1..=40)
        .map(|n| format!("Line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let shot = image::RgbaImage::from_pixel(536, 336, image::Rgba([255, 255, 255, 255]));
    let mut png = Vec::new();
    shot.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    long.element.png = Some(png.into());
    cx.update(|cx| show(4, long, cx));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let drawn = |visual: &mut VisualTestContext| {
        for _ in 0..2 {
            visual.update(|window, _| window.refresh());
            visual.run_until_parked();
        }
    };
    _ = window.update(&mut visual, |note, _, cx| note.toggle(cx));
    drawn(&mut visual);
    _ = window.update(&mut visual, |note, _, cx| {
        let end = note.rows.max_offset().y;
        assert!(end > px(0.), "the rows pass the panel's height");
        note.rows.set_offset(point(px(0.), -end));
        cx.notify();
    });
    drawn(&mut visual);
    window
        .update(&mut visual, |note, _, _| {
            let (rows, end) = (note.rows.bounds(), note.rows.max_offset().y);
            let last = note.rows.children_count() - 1;
            // The child's bounds are laid out unscrolled; the picture's row is the last.
            let row = note.rows.bounds_for_item(last).expect("the picture's row");
            let picture = fitted(
                (536., 336.),
                1.,
                WIDTH - INSET * 2. - NAME_WIDTH,
                PICTURE_MOST,
            );
            assert!(
                row.size.height >= px(picture.1),
                "the row holds the picture"
            );
            let foot = row.bottom() - end;
            assert!(
                foot <= rows.bottom() + px(0.5),
                "at their end the picture's foot {foot:?} is under the rows' {:?}",
                rows.bottom()
            );
        })
        .unwrap();
}
