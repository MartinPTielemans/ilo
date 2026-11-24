use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui::{Bounds, Hsla, Rgba, TestAppContext, point, px, size};

use super::{
    DELAY, GRACE, HEIGHT, Host, MARGIN, NEAR, PAD_X, Plate, RADIUS, RELEASE_POLL, Side, Target,
    WINDOW_HEIGHT, WINDOW_MARGIN, WINDOW_WIDTH, address_colors, near, place, plate, window_left,
    window_top,
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

/// What the plate asks of its window, and a mouse button the test holds down or lets go. The
/// outlines its window tells are apart, as each frame that draws differently tells one.
#[derive(Default)]
struct Recorder {
    asked: RefCell<Vec<String>>,
    pressed: Cell<bool>,
    outlines: RefCell<Vec<(Bounds<gpui::Pixels>, f32)>>,
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

    fn card_picture(&self, card: u64, url: &str, page: &str) -> bool {
        self.asked
            .borrow_mut()
            .push(format!("card {card} picture {url} of {page}"));
        !url.contains("blocked")
    }

    fn card_cancel(&self, card: u64) {
        self.asked.borrow_mut().push(format!("card {card} cancel"));
    }

    fn outline(&self, _: &mut gpui::Window, frame: Bounds<gpui::Pixels>, radius: f32) {
        self.outlines.borrow_mut().push((frame, radius));
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
        window_top(frame)
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

/// Whether the plate's window has a timer it waits on.
fn waiting(cx: &mut TestAppContext) -> bool {
    cx.update(|cx| {
        let window = cx.global::<super::LinkTargetWindow>().0;
        window.read(cx).is_ok_and(|target| target.wait.is_some())
    })
}

/// The pointer crossing from one link to the next: the next plate shows at once within GRACE of
/// the last going, and waits its delay again after it.
#[gpui::test]
fn the_next_links_plate_shows_at_once_within_its_grace_and_not_after(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    assert_eq!(after(cx, &host, DELAY), vec![shown_at(frame)]);
    off_link(cx);
    assert_eq!(after(cx, &host, GRACE / 2), vec!["hide".to_string()]);
    on_link(cx, frame);
    assert_eq!(
        host.asked.take(),
        vec![shown_at(frame)],
        "it waited within its grace"
    );
    off_link(cx);
    assert_eq!(after(cx, &host, GRACE), vec!["hide".to_string()]);
    on_link(cx, frame);
    assert_eq!(
        after(cx, &host, DELAY / 2),
        Vec::<String>::new(),
        "it showed at once past its grace"
    );
    assert_eq!(after(cx, &host, DELAY), vec![shown_at(frame)]);
}

/// The pointer leaving a link before the delay: nothing shows, no timer is left to wake the
/// browser, and the next link waits its delay, as none went.
#[gpui::test]
fn leaving_a_link_before_the_delay_shows_nothing_and_leaves_no_timer(cx: &mut TestAppContext) {
    let host = open_plate(cx);
    let frame = place(card(), 120., Side::Left);
    on_link(cx, frame);
    assert!(waiting(cx));
    after(cx, &host, DELAY / 2);
    off_link(cx);
    assert!(!waiting(cx), "a timer was left with nothing hovered");
    assert_eq!(after(cx, &host, DELAY * 2), Vec::<String>::new());
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

/// The link card's state in the plate's window: its id, its state's name, and whether it shows.
fn card_state(cx: &mut TestAppContext) -> Option<(u64, &'static str, bool)> {
    cx.update(|cx| {
        let window = cx.global::<super::LinkTargetWindow>().0;
        let target = window.read(cx).unwrap();
        (target.card.as_ref()).map(|card| (card.id, card.state.name(), card.shown))
    })
}

const PAGE: &str = "https://example.com/docs";

/// The plate shows for example.com/docs, and ⇧ asked for card 5.
fn asked(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = open_plate(cx);
    on_link(cx, place(card(), 120., Side::Left));
    after(cx, &host, DELAY);
    let link = target("example.com/docs", "example.com");
    cx.update(|cx| super::card_asked(5, link, cx));
    cx.run_until_parked();
    host
}

fn fetched(cx: &mut TestAppContext, status: i32, mime: &str, bytes: &[u8]) {
    let bytes = bytes.to_vec();
    let mime = mime.to_string();
    cx.update(|cx| super::card_fetched(5, status, PAGE.into(), mime, bytes, cx));
    cx.run_until_parked();
}

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(40, 21, image::Rgba([200, 40, 40, 255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[gpui::test]
fn a_card_loads_after_the_plates_delay_then_shows_its_page_and_picture(cx: &mut TestAppContext) {
    let host = asked(cx);
    assert_eq!(
        card_state(cx),
        Some((5, "loading", false)),
        "the card showed before the core's own wait was over, so a ⇧-click would flash it"
    );
    after(cx, &host, DELAY);
    assert_eq!(card_state(cx), Some((5, "loading", true)));
    fetched(
        cx,
        200,
        "text/html",
        b"<title>Docs</title><meta property='og:image' content='/i.png'>",
    );
    assert_eq!(
        host.asked.take(),
        vec![format!(
            "card 5 picture https://example.com/i.png of {PAGE}"
        )]
    );
    assert_eq!(card_state(cx), Some((5, "page, picture waiting", true)));
    fetched(cx, 200, "image/png", &png());
    assert_eq!(card_state(cx), Some((5, "page with picture", true)));
}

#[gpui::test]
fn the_window_tells_the_outline_it_draws_the_plate_then_the_card_as_it_grows(
    cx: &mut TestAppContext,
) {
    let host = asked(cx);
    // The plate, at the window's bottom left past its margin, with the plate's corners.
    let bottom = WINDOW_MARGIN + WINDOW_HEIGHT;
    let plate = Bounds::new(
        point(px(WINDOW_MARGIN), px(bottom - HEIGHT)),
        size(px(120.), px(HEIGHT)),
    );
    assert_eq!(host.outlines.take(), vec![(plate, RADIUS)]);
    // A frame that draws the same tells nothing.
    cx.update(|cx| cx.refresh_windows());
    cx.run_until_parked();
    assert_eq!(host.outlines.take(), vec![]);
    // The card the plate grew into, at the same corner, each of its states once, as each is taller.
    let mut heights = Vec::new();
    let mut card_told = || {
        let told = host.outlines.take();
        assert_eq!(told.len(), 1, "one outline a state: {told:?}");
        let (frame, radius) = told[0];
        assert_eq!(radius, crate::tokens::hover_card::RADIUS);
        assert_eq!(
            (frame.left(), frame.bottom()),
            (px(WINDOW_MARGIN), px(bottom))
        );
        assert_eq!(frame.size.width, px(crate::tokens::link_card::WIDTH));
        heights.push(f32::from(frame.size.height));
    };
    after(cx, &host, DELAY);
    card_told();
    fetched(
        cx,
        200,
        "text/html",
        b"<title>Docs</title><meta property='og:image' content='/i.png'>",
    );
    card_told();
    fetched(cx, 200, "image/png", &png());
    card_told();
    assert!(
        heights[0] > HEIGHT && heights[2] > heights[1],
        "the card's outline didn't grow from the plate's: {heights:?}"
    );
    // Esc takes the card, and the window tells the plate's outline again.
    assert!(cx.update(super::close_card));
    cx.run_until_parked();
    assert_eq!(host.outlines.take(), vec![(plate, RADIUS)]);
}

#[gpui::test]
fn the_pages_answer_shows_the_card_at_once(cx: &mut TestAppContext) {
    let host = asked(cx);
    fetched(cx, 200, "text/html", b"<title>Docs</title>");
    assert_eq!(card_state(cx), Some((5, "page", true)));
    // With no picture to ask, the core's card, kept for it, goes.
    assert_eq!(host.asked.take(), vec!["card 5 cancel".to_string()]);
}

#[gpui::test]
fn a_blocked_picture_is_no_picture(cx: &mut TestAppContext) {
    let host = asked(cx);
    fetched(
        cx,
        200,
        "text/html",
        b"<title>Docs</title><meta property='og:image' content='https://ads.example/blocked.png'>",
    );
    assert_eq!(
        host.asked.take(),
        vec![
            format!("card 5 picture https://ads.example/blocked.png of {PAGE}"),
            "card 5 cancel".to_string(),
        ]
    );
    assert_eq!(card_state(cx), Some((5, "page", true)));
}

#[gpui::test]
fn a_broken_picture_leaves_the_card_without_one(cx: &mut TestAppContext) {
    let _host = asked(cx);
    fetched(
        cx,
        200,
        "text/html",
        b"<title>Docs</title><meta property='og:image' content='/i.png'>",
    );
    fetched(cx, 200, "image/png", b"not a picture");
    assert_eq!(card_state(cx), Some((5, "page", true)));
}

#[gpui::test]
fn a_failed_fetch_says_so_and_moving_on_says_nothing(cx: &mut TestAppContext) {
    let _host = asked(cx);
    fetched(cx, 403, "text/html", b"");
    assert_eq!(card_state(cx), Some((5, "failed", true)));

    let host = asked(cx);
    // net::ERR_ABORTED: a key, press or wheel in the page within the core's wait.
    fetched(cx, -3, "", b"");
    assert_eq!(card_state(cx), None);
    assert_eq!(
        after(cx, &host, DELAY),
        Vec::<String>::new(),
        "a card that went quietly showed after its delay"
    );
}

#[gpui::test]
fn the_card_goes_with_its_plate_and_the_core_stops(cx: &mut TestAppContext) {
    let host = asked(cx);
    fetched(cx, 200, "text/html", b"<title>Docs</title>");
    host.asked.take();
    off_link(cx);
    assert_eq!(card_state(cx), None);
    assert_eq!(
        host.asked.take(),
        vec!["card 5 cancel".to_string(), "hide".to_string()]
    );
}

#[gpui::test]
fn esc_takes_a_shown_card_and_leaves_the_plate(cx: &mut TestAppContext) {
    let host = asked(cx);
    assert!(
        !cx.update(super::close_card),
        "Esc was kept from the page for a card that hadn't shown"
    );
    assert_eq!(host.asked.take(), vec!["card 5 cancel".to_string()]);
    let link = target("example.com/docs", "example.com");
    cx.update(|cx| super::card_asked(6, link, cx));
    after(cx, &host, DELAY);
    assert!(cx.update(super::close_card));
    assert_eq!(card_state(cx), None);
    assert_eq!(
        host.asked.take(),
        vec!["card 6 cancel".to_string()],
        "Esc took the plate with the card"
    );
}

#[gpui::test]
fn a_card_is_asked_once_for_its_link(cx: &mut TestAppContext) {
    let _host = asked(cx);
    let (same, other) = (
        target("example.com/docs", "example.com"),
        target("example.org/", "example.org"),
    );
    assert!(cx.update(|cx| super::card_for(&same, cx)));
    assert!(!cx.update(|cx| super::card_for(&other, cx)));
}

#[test]
fn the_tallest_card_fits_the_windows_room() {
    assert!(super::card::tallest() <= super::CARD_ROOM);
}

#[test]
fn the_path_is_dimmer_than_the_site_on_a_dark_window_as_on_a_light_one() {
    use crate::palette::{TEXT, Tone, contrast, over};
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        // The window's colour with no theme, opaque, which the plate and the card are filled with.
        let fill = Hsla::from(palette.underlay);
        let (path, site) = address_colors(fill, &palette);
        assert!(
            path.a <= site.a - 0.3,
            "on a {tone:?} window the path at {} is clearly dimmer than the site at {}",
            path.a,
            site.a
        );
    }
    // However light or dark the fill, the path still reads on it.
    for step in 0..=20 {
        let level = step as f32 / 20.;
        let fill = Rgba {
            r: level,
            g: level,
            b: level,
            a: 1.,
        };
        for tone in [Tone::Dark, Tone::Light] {
            let palette = Palette::of(None, tone);
            let (path, _) = address_colors(fill.into(), &palette);
            let drawn = over(fill, palette.ink, path.a);
            assert!(
                path.a == 1. || contrast(drawn, fill) >= TEXT,
                "the path at {} on grey {level} in a {tone:?} window reads at {}:1",
                path.a,
                contrast(drawn, fill)
            );
        }
    }
}
