//! The link target's plate on the shell: which page it's for, when it goes, and which corner of
//! the card it's in, on a host that records what the shell asks.

use std::{rc::Rc, time::Duration};

use gpui::{Entity, TestAppContext, VisualTestContext, point, px, size};

use crate::{
    link_target::{Target, WHOLE_AFTER},
    shell::{Folders, Shell, recorder::Recorder},
};

/// A window showing tab 10 of a Space with tabs 10 and 20, 1000 × 700.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            )],
            cx,
        );
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.viewport = size(px(1000.), px(700.));
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn target() -> Target {
    Target {
        text: "example.com/docs".into(),
        domain: 0..11,
    }
}

/// An address of `path` repeated, too long for half the card, which the whole card holds.
fn long(path: &str) -> Target {
    Target {
        text: format!("example.com/{}", path.repeat(16)).into(),
        domain: 0..11,
    }
}

/// What the shell asked of the plate since the last time.
fn plates(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call.starts_with("link target"))
        .collect()
}

/// What the shell asked of the plate in the next `time`.
fn after(cx: &mut VisualTestContext, host: &Recorder, time: Duration) -> Vec<String> {
    cx.executor().advance_clock(time);
    cx.run_until_parked();
    plates(host)
}

#[gpui::test]
fn the_plate_shows_for_the_shown_tab_only(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(20, Some(target()), cx)
    });
    assert!(
        plates(&host).is_empty(),
        "a tab behind the shown one got a plate"
    );
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    assert_eq!(plates(&host), ["link target example.com/docs left"]);
}

#[gpui::test]
fn the_plate_hides_when_the_pointer_leaves_the_link(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    plates(&host);
    // The report of a tab that isn't the plate's leaves it be.
    shell.update(cx, |shell, cx| shell.link_target_changed(20, None, cx));
    assert!(plates(&host).is_empty());
    shell.update(cx, |shell, cx| shell.link_target_changed(10, None, cx));
    assert_eq!(plates(&host), ["link target none"]);
    // Nothing shows, so nothing more is asked.
    shell.update(cx, |shell, cx| shell.link_target_changed(10, None, cx));
    assert!(plates(&host).is_empty());
}

#[gpui::test]
fn the_plate_goes_when_the_shown_tab_changes(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    plates(&host);
    shell.update(cx, |shell, cx| shell.set_selected(20, cx));
    assert_eq!(plates(&host), ["link target none"]);
}

#[gpui::test]
fn the_plate_hides_in_a_pages_full_screen(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    plates(&host);
    shell.update_in(cx, |shell, window, cx| {
        shell.set_page_fullscreen(true, window, cx)
    });
    assert_eq!(plates(&host), ["link target none"]);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    assert!(
        plates(&host).is_empty(),
        "a page in full screen got a plate"
    );
}

#[gpui::test]
fn the_plate_moves_aside_near_the_pointer(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    plates(&host);
    let frame = host.link_frame.get().expect("the plate has a place");
    let (inside, far) = (frame.center(), point(px(500.), px(100.)));

    // Away from it, it stays.
    shell.update(cx, |shell, cx| {
        shell.link_pointer(far.x.into(), far.y.into(), cx)
    });
    assert!(plates(&host).is_empty());

    // On it, it goes to the card's other corner.
    shell.update(cx, |shell, cx| {
        shell.link_pointer(inside.x.into(), inside.y.into(), cx)
    });
    assert_eq!(plates(&host), ["link target example.com/docs right"]);
    let moved = host.link_frame.get().expect("the plate has a place");
    assert!(
        moved.left() > frame.left(),
        "the plate stayed in the left corner"
    );

    // The pointer is on the left plate's place now, which is not near the right one, so it stays.
    shell.update(cx, |shell, cx| {
        shell.link_pointer(inside.x.into(), inside.y.into(), cx)
    });
    assert!(plates(&host).is_empty());

    // Near the right one, it goes back.
    let back = moved.center();
    shell.update(cx, |shell, cx| {
        shell.link_pointer(back.x.into(), back.y.into(), cx)
    });
    assert_eq!(plates(&host), ["link target example.com/docs left"]);
}

#[gpui::test]
fn a_new_link_keeps_the_side_and_leaving_it_forgets_it(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    let inside = host
        .link_frame
        .get()
        .expect("the plate has a place")
        .center();
    shell.update(cx, |shell, cx| {
        shell.link_pointer(inside.x.into(), inside.y.into(), cx)
    });
    plates(&host);
    // The pointer goes from one link to the next: the plate is still where it moved to.
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    assert_eq!(plates(&host), ["link target example.com/docs right"]);
    // Off the links, the next one starts on the left again.
    shell.update(cx, |shell, cx| shell.link_target_changed(10, None, cx));
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    let calls = plates(&host);
    assert_eq!(
        calls.last().map(String::as_str),
        Some("link target example.com/docs left")
    );
}

#[gpui::test]
fn resting_on_a_link_shows_its_whole_address(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    let link = long("docs/");
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(link.clone()), cx)
    });
    let cut = plates(&host);
    assert_eq!(cut.len(), 1);
    assert!(
        cut[0].contains('…'),
        "the long address fit half the card: {}",
        cut[0]
    );
    let rest = host.link_frame.get().expect("the plate has a place");

    assert!(after(cx, &host, WHOLE_AFTER - Duration::from_millis(1)).is_empty());
    let grown = after(cx, &host, Duration::from_millis(1));
    assert_eq!(grown, [format!("link target {} left", link.text)]);
    let whole = host.link_frame.get().expect("the plate has a place");
    assert!(whole.size.width > rest.size.width, "the plate didn't grow");
    assert_eq!(whole.left(), rest.left(), "the plate left its corner");
    // Once is all.
    assert!(after(cx, &host, WHOLE_AFTER).is_empty());
}

#[gpui::test]
fn leaving_the_link_ends_the_wait(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(long("docs/")), cx)
    });
    after(cx, &host, WHOLE_AFTER / 2);
    shell.update(cx, |shell, cx| shell.link_target_changed(10, None, cx));
    assert_eq!(after(cx, &host, WHOLE_AFTER), ["link target none"]);
    // The next link waits again from its start.
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(long("docs/")), cx)
    });
    assert!(plates(&host)[0].contains('…'));
    assert!(after(cx, &host, WHOLE_AFTER / 2).is_empty());
    assert_eq!(after(cx, &host, WHOLE_AFTER / 2).len(), 1);
}

#[gpui::test]
fn the_same_link_again_keeps_its_wait_and_another_starts_one(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(long("docs/")), cx)
    });
    after(cx, &host, WHOLE_AFTER / 2);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(long("docs/")), cx)
    });
    plates(&host);
    let whole = after(cx, &host, WHOLE_AFTER / 2);
    assert!(whole.len() == 1 && !whole[0].contains('…'), "{whole:?}");

    // While it shows, the next link has its whole address at once, as Chrome's status bubble does.
    let next = long("api/");
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(next.clone()), cx)
    });
    assert_eq!(plates(&host), [format!("link target {} left", next.text)]);
    assert!(after(cx, &host, WHOLE_AFTER).is_empty());
}

#[gpui::test]
fn a_link_that_fits_waits_for_nothing(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(target()), cx)
    });
    plates(&host);
    assert!(after(cx, &host, WHOLE_AFTER).is_empty());
    // It's no whole plate either: the next long link starts cut.
    shell.update(cx, |shell, cx| {
        shell.link_target_changed(10, Some(long("docs/")), cx)
    });
    assert!(plates(&host)[0].contains('…'));
}
