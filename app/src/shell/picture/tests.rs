//! A window whose page another window shows draws a picture of it, on a host that records what the
//! shell asks of the core. The user's way in is the core's report, a click on the picture and the
//! window coming to the front.

use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    Bounds, Entity, Modifiers, Pixels, RenderImage, TestAppContext, VisualTestContext, point, px,
};

use super::{PagePicture, REST};
use crate::shell::{Folders, Shell, recorder::Recorder};

/// A window on one Space of tabs 10 and 20, showing 10, drawn.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// A picture of a page 400 by 200 pixels, twice as wide as it is tall.
fn page() -> PagePicture {
    PagePicture::from_bgra(vec![0; 400 * 200 * 4], 400, 200).unwrap()
}

/// Answers the request the shell has out with a picture.
fn answer(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let request = shell.read_with(cx, |shell, _| shell.picture.as_ref().unwrap().request);
    shell.update(cx, |shell, cx| {
        shell.page_pictured(request, Some(page()), cx)
    });
    cx.run_until_parked();
}

/// The core says another window shows the tab, and the picture comes.
fn picture_arrives(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.page_elsewhere(tab, cx));
    answer(shell, cx);
}

fn card(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| shell.picture_card())
}

/// Where the picture is `later` from the start of its motion, or from now if it has none, so a test
/// on a busy machine reads the motion where it means to.
fn picture_at(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
    later: Duration,
) -> Option<Bounds<Pixels>> {
    shell.read_with(cx, |shell, _| {
        let motion = shell.picture.as_ref().and_then(|picture| picture.motion);
        shell.picture_bounds(motion.map_or_else(Instant::now, |motion| motion.start) + later)
    })
}

/// What the shell asked of the core about pictures since the last time.
fn asked(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call.starts_with("picture"))
        .collect()
}

/// The picture the window shows, if it has one.
fn image(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Arc<RenderImage>> {
    shell.read_with(cx, |shell, _| {
        Some(shell.picture.as_ref()?.image.as_ref()?.image.clone())
    })
}

fn pictured(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.picture.is_some())
}

/// The core says another window shows the selected tab's page, and the shell asks for one picture
/// of it, however often it's told. It asks nothing for a tab the window has since left.
#[gpui::test]
fn a_page_elsewhere_is_asked_for_once(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert_eq!(asked(&host), ["picture 10"]);
    shell.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert!(asked(&host).is_empty());
    shell.update(cx, |shell, cx| shell.page_elsewhere(20, cx));
    assert!(asked(&host).is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.pictured(10) && !shell.pictured(20)));
}

/// A core that answers inside the call that asked, while the shell is being updated, gives its
/// picture once that update is over, as the core's report of a page that went and the window coming
/// to the front both ask; nothing panics (#364).
#[gpui::test]
fn a_picture_answered_inside_the_ask_comes_after_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    host.pictures.replace(Some((shell.downgrade(), page())));
    shell.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert_eq!(asked(&host), ["picture 10"]);
    cx.run_until_parked();
    assert!(asked(&host).is_empty());
    assert!(picture_at(&shell, cx, Duration::from_millis(300)).is_some());

    shell.update(cx, |shell, cx| shell.became_key(cx));
    assert_eq!(asked(&host), ["picture 10"]);
    cx.run_until_parked();
    assert!(asked(&host).is_empty());
    assert!(picture_at(&shell, cx, Duration::from_millis(300)).is_some());
}

/// Nothing shows until the picture comes. Then it's fitted to 0.8 of the card, centred, once it has
/// stepped back from the card's size.
#[gpui::test]
fn the_picture_rests_at_four_fifths_of_the_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert!(picture_at(&shell, cx, Duration::ZERO).is_none());
    answer(&shell, cx);
    assert_eq!(asked(&host), ["picture 10"]);

    let card = card(&shell, cx);
    let rest = picture_at(&shell, cx, Duration::from_millis(300)).unwrap();
    let (card_width, card_height) = (f32::from(card.size.width), f32::from(card.size.height));
    let (width, height) = (f32::from(rest.size.width), f32::from(rest.size.height));
    assert!(width <= REST * card_width + 0.01 && height <= REST * card_height + 0.01);
    assert!(
        (width - REST * card_width).abs() < 0.01 || (height - REST * card_height).abs() < 0.01,
        "{width}x{height} touches neither 0.8 side of {card_width}x{card_height}"
    );
    assert!(
        (width / height - 2.).abs() < 0.01,
        "it keeps the page's shape: {width}x{height}"
    );
    let (centre, card_centre) = (rest.center(), card.center());
    assert!((f32::from(centre.x) - f32::from(card_centre.x)).abs() < 0.01);
    assert!((f32::from(centre.y) - f32::from(card_centre.y)).abs() < 0.01);

    // It arrives at the card's size and steps back.
    let first = picture_at(&shell, cx, Duration::ZERO).unwrap();
    let middle = picture_at(&shell, cx, Duration::from_millis(60)).unwrap();
    assert!(first.size.width > middle.size.width && middle.size.width > rest.size.width);
}

/// A press on the picture takes the page back at once, by the call a press on the tab's row makes;
/// a press beside it does nothing, and another press while the page comes asks for nothing more.
/// The row's own click works too, which does nothing for the tab selected while its page is live
/// here.
#[gpui::test]
fn a_click_on_the_picture_takes_the_page_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| shell.select(10, cx));
    assert!(host.take().is_empty());
    picture_arrives(&shell, cx, 10);
    host.take();

    let card = card(&shell, cx);
    let corner = card.origin + point(px(3.), px(3.));
    cx.simulate_click(corner, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(
        host.take().is_empty(),
        "a click beside the picture took the page"
    );

    shell.update(cx, |shell, cx| shell.select(10, cx));
    assert_eq!(host.take(), ["select 10"]);

    cx.simulate_click(card.center(), Modifiers::none());
    assert_eq!(host.take(), ["select 10"]);
    cx.simulate_click(card.center(), Modifiers::none());
    // Nor does the window coming to the front ask for a picture of a page on its way here.
    shell.update(cx, |shell, cx| shell.became_key(cx));
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(host.take().is_empty());
}

/// The window a page leaves shows at once the picture the window taking it showed, at the card's
/// size, stepping back, and asks for a fresh one, which replaces it where it is. A picture is
/// shared only while some window shows it.
#[gpui::test]
fn the_window_a_page_leaves_shows_the_taker_s_picture_at_once(cx: &mut TestAppContext) {
    let (taker, cx, _) = window(cx);
    picture_arrives(&taker, cx, 10);
    let shown = image(&taker, cx).unwrap();
    let (giver, cx, host) = window(cx);
    giver.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert_eq!(asked(&host), ["picture 10"]);
    assert!(Arc::ptr_eq(&image(&giver, cx).unwrap(), &shown));
    let card = card(&giver, cx);
    let first = picture_at(&giver, cx, Duration::ZERO).unwrap();
    assert!(
        (first.size.width - card.size.width).abs() < px(2.)
            || (first.size.height - card.size.height).abs() < px(2.),
        "it starts at the card's size: {:?} in {:?}",
        first.size,
        card.size
    );
    let rest = picture_at(&giver, cx, Duration::from_millis(300)).unwrap();
    assert!(first.size.width > rest.size.width);

    // The fresh one replaces it where it is, without stepping back again.
    answer(&giver, cx);
    assert!(!Arc::ptr_eq(&image(&giver, cx).unwrap(), &shown));
    assert_eq!(
        picture_at(&giver, cx, Duration::from_millis(300)),
        Some(rest)
    );

    // Once no window shows a picture of the tab, a window it leaves shows none until one comes.
    drop(shown);
    giver.update(cx, |shell, cx| shell.page_elsewhere(0, cx));
    taker.update(cx, |shell, cx| shell.page_elsewhere(0, cx));
    cx.run_until_parked();
    giver.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    assert!(image(&giver, cx).is_none());
}

/// The picture is taken again whenever the window comes to the front, one at a time, replacing the
/// one it has; the old one is let go of.
#[gpui::test]
fn the_window_coming_to_the_front_takes_the_picture_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    picture_arrives(&shell, cx, 10);
    host.take();

    shell.update(cx, |shell, cx| shell.became_key(cx));
    assert_eq!(asked(&host), ["picture 10"]);
    // Still waiting for that one, it doesn't ask again.
    shell.update(cx, |shell, cx| shell.became_key(cx));
    assert!(asked(&host).is_empty());
    answer(&shell, cx);
    assert!(asked(&host).is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.stale_pictures.is_empty()));
    assert!(picture_at(&shell, cx, Duration::from_millis(300)).is_some());
}

/// The picture goes when the page comes back, the window selects another tab or none, or the tab
/// closes (the core then selects another or none), and an answer that comes after is ignored.
#[gpui::test]
fn the_picture_goes_with_the_page_it_stands_for(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    // The page comes back to this window.
    picture_arrives(&shell, cx, 10);
    shell.update(cx, |shell, cx| shell.page_elsewhere(0, cx));
    assert!(!pictured(&shell, cx));

    // The window selects another tab.
    picture_arrives(&shell, cx, 10);
    shell.update(cx, |shell, cx| shell.set_selected(20, cx));
    assert!(!pictured(&shell, cx));

    // The tab closes, and the window has none to show.
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    picture_arrives(&shell, cx, 10);
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    assert!(!pictured(&shell, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.stale_pictures.is_empty()));

    // The tab selected again keeps its picture, which the core's report of it doesn't touch.
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    picture_arrives(&shell, cx, 10);
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    assert!(pictured(&shell, cx));

    // Left before the picture came: the answer is ignored.
    shell.update(cx, |shell, cx| shell.page_elsewhere(0, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.page_elsewhere(10, cx));
    let request = shell.read_with(cx, |shell, _| shell.picture.as_ref().unwrap().request);
    shell.update(cx, |shell, cx| shell.set_selected(20, cx));
    assert_eq!(asked(&host), ["picture 10"]);
    shell.update(cx, |shell, cx| {
        shell.page_pictured(request, Some(page()), cx)
    });
    cx.run_until_parked();
    assert!(!pictured(&shell, cx));
}

/// A page that is live here has no picture, and nothing is captured for it.
#[gpui::test]
fn a_page_that_is_live_here_is_never_captured(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.became_key(cx);
        shell.set_selected(20, cx);
        shell.set_selected(10, cx);
        shell.became_key(cx);
    });
    cx.run_until_parked();
    assert!(asked(&host).is_empty());
    assert!(!pictured(&shell, cx));
    assert!(picture_at(&shell, cx, Duration::from_millis(300)).is_none());
}
