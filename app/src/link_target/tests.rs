use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui::{Bounds, TestAppContext, point, px, size};

use super::{
    DELAY, GRACE, HEIGHT, Host, MARGIN, NEAR, PAD_X, Plate, RELEASE_POLL, Side, Target,
    WINDOW_WIDTH, near, place, plate, window_left,
};
use crate::palette::Palette;

/// Every character 7 points wide, so a test counts characters.
fn measure(text: &str) -> f32 {
    7. * text.chars().count() as f32
}

/// A link whose site is the part of `text` that reads `site`.
fn target(text: &str, site: &str) -> Target {
    let start = text.find(site).unwrap();
    Target {
        text: text.to_string().into(),
        domain: start..start + site.len(),
    }
}

#[test]
fn an_address_that_fits_is_whole_with_its_site_apart() {
    let plate = plate(
        &target("www.example.com/docs", "example.com"),
        480.,
        measure,
    );
    assert_eq!(
        (
            plate.before.as_ref(),
            plate.site.as_ref(),
            plate.after.as_ref()
        ),
        ("www.", "example.com", "/docs")
    );
    assert_eq!(plate.width, measure("www.example.com/docs") + 2. * PAD_X);
}

#[test]
fn a_long_path_loses_its_middle_and_the_site_stays() {
    let path = "/a/very/long/path/that/goes/on/and/on/and/ends/in/report-final.pdf";
    let text = format!("login.example.co.uk{path}");
    let plate = plate(&target(&text, "example.co.uk"), 240., measure);
    // 240 less 2 × 10 of padding takes 31 characters.
    assert!(plate.width <= 240.);
    assert_eq!(plate.before.as_ref(), "login.");
    assert_eq!(plate.site.as_ref(), "example.co.uk");
    assert!(
        plate.after.starts_with("/a/"),
        "the path's start stays: {}",
        plate.after
    );
    assert!(plate.after.contains('…'), "{}", plate.after);
    assert!(
        plate.after.ends_with(".pdf"),
        "the file's end stays: {}",
        plate.after
    );
    assert!(plate.text().chars().count() <= 31);
}

#[test]
fn subdomains_go_from_the_left_before_the_site_does() {
    let text = "accounts.security.verification.example.com/";
    let plate = plate(&target(text, "example.com"), 200., measure);
    assert_eq!(plate.site.as_ref(), "example.com");
    assert!(plate.before.starts_with('…'), "{}", plate.before);
    assert!(plate.before.ends_with("verification."), "{}", plate.before);
    assert_eq!(plate.after.as_ref(), "");
    assert!(plate.width <= 200.);
}

#[test]
fn a_site_wider_than_the_plate_is_cut_at_its_end() {
    let host = "an-extremely-long-name.example.com";
    let plate = plate(&target(host, host), 120., measure);
    assert_eq!(plate.before.as_ref(), "");
    assert!(plate.site.ends_with('…'), "{}", plate.site);
    assert!(plate.width <= 120.);
}

#[test]
fn an_address_with_no_host_has_no_site() {
    let plate = plate(
        &Target {
            text: "mailto:someone@example.com".into(),
            domain: 0..0,
        },
        480.,
        measure,
    );
    assert_eq!(plate.site.as_ref(), "");
    assert_eq!(plate.text(), "mailto:someone@example.com");
    // A range that isn't in the text, or splits a character, is no site either.
    let wrong = Target {
        text: "日本語.jp/".into(),
        domain: 1..4,
    };
    assert_eq!(super::plate(&wrong, 480., measure).text(), "日本語.jp/");
    let past = Target {
        text: "a.com".into(),
        domain: 2..40,
    };
    assert_eq!(super::plate(&past, 480., measure).text(), "a.com");
}

#[test]
fn a_path_in_other_scripts_is_cut_on_characters() {
    let text = "例.jp/日本語の長いパス/日本語の長いパス/日本語の長いパス/ファイル.html";
    let plate = plate(&target(text, "例.jp"), 180., measure);
    assert!(plate.after.contains('…'));
    assert!(plate.after.ends_with(".html"));
    assert!(plate.width <= 180.);
}

#[test]
fn a_huge_address_is_cut_before_it_is_measured() {
    let text = format!("example.com/{}", "x".repeat(200_000));
    let measured = std::cell::Cell::new(0);
    let plate = plate(&target(&text, "example.com"), 480., |text| {
        measured.set(measured.get().max(text.chars().count()));
        measure(text)
    });
    assert!(
        measured.get() < 1000,
        "measured a string of {}",
        measured.get()
    );
    assert!(plate.after.contains('…'));
}

fn card() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(228.), px(10.)), size(px(762.), px(600.)))
}

#[test]
fn the_plate_sits_inside_the_cards_bottom_corners() {
    let left = place(card(), 200., Side::Left);
    assert_eq!(f32::from(left.left()), 228. + MARGIN);
    assert_eq!(f32::from(left.bottom()), 610. - MARGIN);
    assert_eq!(f32::from(left.size.height), HEIGHT);
    let right = place(card(), 200., Side::Right);
    assert_eq!(f32::from(right.right()), 990. - MARGIN);
    assert_eq!(f32::from(right.bottom()), f32::from(left.bottom()));
}

#[test]
fn the_pointer_near_the_plate_is_near_and_far_from_it_is_not() {
    let frame = place(card(), 200., Side::Left);
    let (x, y) = (f32::from(frame.left()), f32::from(frame.top()));
    assert!(near(frame, x + 50., y + 5.));
    assert!(near(frame, x - NEAR + 1., y - NEAR + 1.));
    assert!(!near(frame, x - NEAR - 1., y + 5.));
    assert!(!near(frame, x + 50., y - NEAR - 1.));
    assert!(!near(frame, x + 500., y + 5.));
}

#[test]
fn the_plates_window_stays_inside_a_card_wider_than_it_from_either_corner() {
    let card = Bounds::new(
        point(px(240.), px(10.)),
        size(px(WINDOW_WIDTH + 100.), px(600.)),
    );
    for side in [Side::Left, Side::Right] {
        let frame = place(card, 200., side);
        let left = window_left(frame, side);
        assert!(
            left >= f32::from(card.left()),
            "{side:?}: the window starts left of the card"
        );
        assert!(
            left + WINDOW_WIDTH <= f32::from(card.right()),
            "{side:?}: it goes past the card"
        );
        assert!(left <= f32::from(frame.left()) && left + WINDOW_WIDTH >= f32::from(frame.right()));
    }
}

/// What the plate asks of its window, and a mouse button the test holds down or lets go.
#[derive(Default)]
struct Recorder {
    asked: RefCell<Vec<String>>,
    pressed: Cell<bool>,
}

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32) {
        self.asked
            .borrow_mut()
            .push(format!("show {left} {top} over {window}"));
    }

    fn hide(&self) {
        self.asked.borrow_mut().push("hide".into());
    }

    fn pressed(&self) -> bool {
        self.pressed.get()
    }
}

fn open_plate(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| super::open(host.clone(), |_| (gpui::black(), Palette::default()), cx))
        .expect("no link target window");
    host
}

/// The pointer comes onto a link in browser window 1, whose plate goes at `frame`.
fn on_link(cx: &mut TestAppContext, frame: Bounds<gpui::Pixels>) {
    let plate = Plate {
        before: "".into(),
        site: "example.com".into(),
        after: "/docs".into(),
        width: 120.,
    };
    cx.update(|cx| super::show(1, plate, frame, Side::Left, cx));
    cx.run_until_parked();
}

fn off_link(cx: &mut TestAppContext) {
    cx.update(super::hide);
    cx.run_until_parked();
}

fn after(cx: &mut TestAppContext, host: &Recorder, time: Duration) -> Vec<String> {
    cx.executor().advance_clock(time);
    cx.run_until_parked();
    host.asked.take()
}

fn shown_at(frame: Bounds<gpui::Pixels>) -> String {
    format!(
        "show {} {} over 1",
        f32::from(frame.left()),
        f32::from(frame.top())
    )
}

#[gpui::test]
fn the_plate_shows_after_its_delay_with_no_button_down(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    assert_eq!(after(cx, &host, DELAY / 2), Vec::<String>::new());
    assert_eq!(after(cx, &host, DELAY), vec![shown_at(frame)]);
}

#[gpui::test]
fn a_plate_due_while_a_button_is_down_shows_at_its_release(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    host.pressed.set(true);
    assert_eq!(
        after(cx, &host, DELAY * 3),
        Vec::<String>::new(),
        "the plate's window showed under a press, which on Windows is a move at the pointer that \
         takes an Alt-click on a link away"
    );
    host.pressed.set(false);
    assert_eq!(after(cx, &host, RELEASE_POLL), vec![shown_at(frame)]);
}

#[gpui::test]
fn a_plate_whose_link_the_pointer_left_during_the_press_never_shows(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    on_link(cx, place(card(), 120., Side::Left));
    host.pressed.set(true);
    assert_eq!(after(cx, &host, DELAY * 2), Vec::<String>::new());
    off_link(cx);
    host.pressed.set(false);
    assert_eq!(after(cx, &host, RELEASE_POLL * 4), Vec::<String>::new());
}

#[gpui::test]
fn a_shown_plate_stays_through_a_press_and_goes_at_its_release(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    assert_eq!(after(cx, &host, DELAY), vec![shown_at(frame)]);
    host.pressed.set(true);
    off_link(cx);
    assert_eq!(after(cx, &host, GRACE), Vec::<String>::new());
    host.pressed.set(false);
    assert_eq!(after(cx, &host, RELEASE_POLL), vec!["hide".to_string()]);
}

#[gpui::test]
fn a_shown_plate_moves_only_at_the_release(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    assert_eq!(after(cx, &host, DELAY), vec![shown_at(frame)]);
    host.pressed.set(true);
    let moved = place(card(), 120., Side::Right);
    on_link(cx, moved);
    assert_eq!(after(cx, &host, DELAY), Vec::<String>::new());
    host.pressed.set(false);
    assert_eq!(after(cx, &host, RELEASE_POLL), vec![shown_at(moved)]);
}
