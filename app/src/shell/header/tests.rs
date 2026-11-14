//! Where the shell asks for the Site Controls, and the tooltips its buttons ask for, on a host
//! that records them.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AppContext, Bounds, Entity, Modifiers, MouseButton, NavigationDirection, TestAppContext,
    VisualTestContext, point, px, size,
};

use crate::{
    passwords::Key,
    shell::{
        Command, FOOTER_HEIGHT, Folders, NAV_PRESS_SCALE, NAV_SIZE, NAV_TOP, PRESS_TIME, Page,
        SIDEBAR_BUTTON_X, Shell, beside::AtRest, header::nav_places, history::HOLD,
        library::LIBRARY_BACK_INSET, recorder::Recorder,
    },
    site_controls::Place,
};

/// Back, forward and reload in the 228 pt sidebar a window starts with (Arc 1.170).
const BACK_X: f32 = 122.;
const FORWARD_X: f32 = 156.;
const RELOAD_X: f32 = 190.;

/// Arc's back, forward and reload keep to the sidebar's right edge, 6 in, 2 apart: at 228 where
/// ours always were, at 347 where Arc 1.170 has them, and the sidebar button where it was.
#[test]
fn back_forward_and_reload_keep_to_the_sidebars_right_edge() {
    let at = |width| nav_places(width, false, false);
    assert_eq!(
        at(228.),
        [Some(77.), Some(BACK_X), Some(FORWARD_X), Some(RELOAD_X)]
    );
    assert_eq!(at(347.), [Some(77.), Some(241.), Some(275.), Some(309.)]);
    let [_, _, forward, reload] = at(500.);
    assert_eq!(
        (forward.unwrap() + 34., reload.unwrap() + 32.),
        (reload.unwrap(), 494.)
    );
}

/// Narrower than 228 reload goes, then forward under 194, the rest still at the right edge, and
/// back never comes nearer the sidebar button than its 13 at 228.
#[test]
fn a_narrow_sidebar_loses_reload_then_forward() {
    let at = |width| nav_places(width, false, false);
    assert_eq!(at(227.), [Some(77.), Some(155.), Some(189.), None]);
    assert_eq!(at(194.), [Some(77.), Some(122.), Some(156.), None]);
    assert_eq!(at(193.), [Some(77.), Some(155.), None, None]);
    assert_eq!(at(164.), [Some(77.), Some(126.), None, None]);
}

/// In full screen, with no traffic lights, the sidebar button's glyph is centred 29.25 in (Arc
/// 1.171), and the others keep their places, with room for all three down to the narrowest.
#[test]
fn in_full_screen_the_sidebar_button_moves_left() {
    let [sidebar, back, ..] = nav_places(228., true, false);
    assert_eq!(
        (sidebar.unwrap() + NAV_SIZE / 2., back),
        (29.25, Some(BACK_X))
    );
    assert_eq!(nav_places(165., true, false).iter().flatten().count(), 4);
}

/// A shell showing one Space with `pinned` and `today` tabs and `folders`.
fn shell_with(
    cx: &mut TestAppContext,
    pinned: &[u64],
    today: &[u64],
    folders: Folders,
) -> (gpui::Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, pinned.to_vec(), today.to_vec(), Vec::new(), folders, cx);
    });
    (shell, host)
}

/// From the command bar as from its button: hanging from the button at x 191 of the 228 pt
/// sidebar, where Arc's is, under the URL field.
#[gpui::test]
fn site_controls_open_under_the_url_field(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2], Folders::default());
    shell.update(cx, |shell, cx| {
        shell.set_selected(2, cx);
        shell.run(Command::SiteControls, "ui", cx);
    });
    let button = Bounds::new(point(px(191.), px(49.)), size(px(30.), px(30.)));
    let field = Bounds::new(point(px(8.), px(46.)), size(px(212.), px(36.)));
    assert_eq!(
        host.site_controls.take(),
        [(2, Place::Button { field, button })]
    );
}

/// With the sidebar hidden they open at the page's top left corner, as an extension's popup does.
#[gpui::test]
fn site_controls_open_at_the_corner_with_the_sidebar_hidden(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
    shell.update(cx, |shell, cx| {
        shell.set_selected(1, cx);
        shell.sidebar_hidden = true;
        shell.run(Command::SiteControls, "ui", cx);
    });
    let corner = point(px(18.), px(8.));
    assert_eq!(host.site_controls.take(), [(1, Place::Corner(corner))]);
}

/// The new tab page has no site.
#[gpui::test]
fn site_controls_need_a_tab(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
    shell.update(cx, |shell, cx| shell.run(Command::SiteControls, "ui", cx));
    assert!(host.site_controls.take().is_empty());
}

/// A drawn shell showing one Space.
fn drawn_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// The tooltips the shell asked for as the pointer moved to (`x`, `y`).
fn move_to(cx: &mut VisualTestContext, host: &Recorder, x: f32, y: f32) -> Vec<String> {
    host.take();
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.run_until_parked();
    host.take()
        .into_iter()
        .filter(|call| call.starts_with("tooltip"))
        .collect()
}

/// Resting on the footer's Library button or the header's sidebar button asks for its name and
/// keys, and leaving it ends the tooltip. Coming straight from a favorite's tile, whose leaving
/// ends its own tooltip later in the same move, the button's still shows, as in the check.
#[gpui::test]
fn sidebar_buttons_name_themselves(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(10, 1)], cx));
    cx.run_until_parked();
    let (tile, height) = shell.read_with(cx, |shell, _| {
        let tile = shell.tip_frames.borrow()[&10].center();
        (tile, f32::from(shell.viewport.height))
    });
    move_to(cx, &host, tile.x.into(), tile.y.into());
    let library = (
        LIBRARY_BACK_INSET + NAV_SIZE / 2.,
        height - FOOTER_HEIGHT / 2.,
    );
    let asked = move_to(cx, &host, library.0, library.1);
    assert_eq!(
        asked.last().map(String::as_str),
        Some("tooltip View Library ⇧⌘L")
    );
    assert_eq!(move_to(cx, &host, 100., 300.), ["tooltip none"]);
    let sidebar = move_to(
        cx,
        &host,
        SIDEBAR_BUTTON_X + NAV_SIZE / 2.,
        NAV_TOP + NAV_SIZE / 2.,
    );
    assert_eq!(sidebar, ["tooltip Hide Sidebar ⌘S"]);
    assert_eq!(move_to(cx, &host, 100., 300.), ["tooltip none"]);
}

/// Resting on Clear says what it does in Arc's words, with no keys, and leaving it, or pressing
/// it, ends the tooltip.
#[gpui::test]
fn clear_says_what_it_does(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    // The pointer coming onto the sidebar fades Clear in.
    move_to(cx, &host, 100., 300.);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let clear = shell.read_with(cx, |shell, _| {
        shell.button_frames.borrow()["clear"].center()
    });
    let (x, y) = (f32::from(clear.x), f32::from(clear.y));
    assert_eq!(
        move_to(cx, &host, x, y),
        ["tooltip Clear unpinned tabs in this space "]
    );
    assert_eq!(move_to(cx, &host, 100., 300.), ["tooltip none"]);
    move_to(cx, &host, x, y);
    host.take();
    cx.simulate_mouse_down(clear, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().iter().any(|call| call == "tooltip none"));
}

/// A press in the sidebar with no move before it, as the check's click on a permission question's
/// Allow, tells the sidebar the pointer is over it, so Clear and the URL field's buttons show once
/// the question closes, and a move to the page hides them.
#[gpui::test]
fn a_press_with_no_move_before_it_puts_the_pointer_over_the_sidebar(cx: &mut TestAppContext) {
    let (shell, cx, _host) = drawn_shell(cx);
    let tools = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| (shell.sidebar_hovered, shell.tools_shown()))
    };
    assert_eq!(tools(&shell, cx), (false, false));
    cx.simulate_click(point(px(100.), px(300.)), Modifiers::none());
    assert_eq!(tools(&shell, cx), (true, true));
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    assert!(!tools(&shell, cx).0);
    // A press on the page leaves the sidebar's hover where it is.
    cx.simulate_click(point(px(600.), px(400.)), Modifiers::none());
    assert!(!tools(&shell, cx).0);
}

/// A shell with Spaces One and Two, One shown, its tab 10 on a page with history both ways.
fn two_spaces(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = drawn_shell(cx);
    let page = Page {
        url: "https://example.org/".into(),
        can_go_back: true,
        can_go_forward: true,
        ..Page::default()
    };
    shell.update(cx, |shell, cx| {
        let space = |id, name: &str| (id, name.into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space(1, "One"), space(2, "Two")], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.set_page(10, page, cx);
    });
    cx.run_until_parked();
    host.take();
    host.shown.take();
    (shell, cx, host)
}

/// A press of the mouse's back or forward button over the sidebar: the Spaces it showed and what
/// else it asked of the core. The page's history is never one of them: over the UI the buttons
/// are the Spaces', and over a page, which takes them itself, they're its history's.
fn navigate(back: bool, cx: &mut VisualTestContext, host: &Recorder) -> (Vec<u64>, Vec<String>) {
    let direction = match back {
        true => NavigationDirection::Back,
        false => NavigationDirection::Forward,
    };
    let button = MouseButton::Navigate(direction);
    cx.simulate_mouse_down(point(px(100.), px(300.)), button, Modifiers::none());
    cx.run_until_parked();
    let calls = host.take();
    assert!(calls.iter().all(|call| !call.ends_with(" 10")), "{calls:?}");
    (host.shown.take(), calls)
}

fn library_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.library_open)
}

/// Between Spaces the mouse's buttons show the one before and after, as Arc's do, and Back on the
/// first Space shows the Library, as a swipe from it does (#1852).
#[gpui::test]
fn mouse_back_on_the_first_space_shows_the_library(cx: &mut TestAppContext) {
    let (shell, cx, host) = two_spaces(cx);
    assert_eq!(navigate(false, cx, &host).0, [2]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert_eq!(navigate(true, cx, &host).0, [1]);
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    assert!(!library_open(&shell, cx));
    assert!(navigate(true, cx, &host).0.is_empty());
    assert!(library_open(&shell, cx));
}

/// Forward on the last Space starts a new one after it, with its form, as the pull past it does;
/// Forward again makes no second, and Back cancels it (#1852).
#[gpui::test]
fn mouse_forward_on_the_last_space_starts_a_new_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = two_spaces(cx);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    let (_, calls) = navigate(false, cx, &host);
    assert!(
        calls.contains(&"create space Space 3 after 2".to_string()),
        "{calls:?}"
    );
    shell.update(cx, |shell, cx| {
        let space = |id, name: &str| (id, name.into(), None, 0, "".into(), "".into(), "".into());
        let spaces = vec![space(1, "One"), space(2, "Two"), space(7, "Space 3")];
        shell.set_spaces(spaces, cx);
        shell.set_space(7, cx);
    });
    cx.run_until_parked();
    let form = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.form().map(|form| form.space))
    };
    assert_eq!(form(&shell, cx), Some(7));
    let (_, calls) = navigate(false, cx, &host);
    assert!(
        calls.iter().all(|call| !call.starts_with("create space")),
        "{calls:?}"
    );
    assert_eq!(form(&shell, cx), Some(7));
    host.take_edits();
    navigate(true, cx, &host);
    assert_eq!(host.take_edits(), ["delete space 7"]);
    assert_eq!(form(&shell, cx), None);
}

/// In the Library, opened from any Space, Forward shows the first Space as a swipe out of it does,
/// and Back does nothing; neither moves the Space behind it (#1852).
#[gpui::test]
fn mouse_buttons_in_the_library_leave_the_space_behind_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = two_spaces(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.run(Command::ViewLibrary, "test", cx);
    });
    cx.run_until_parked();
    assert!(library_open(&shell, cx));
    host.take();
    host.shown.take();
    assert!(navigate(true, cx, &host).0.is_empty());
    assert!(library_open(&shell, cx));
    assert_eq!(navigate(false, cx, &host).0, [1]);
    assert!(!library_open(&shell, cx));
}

/// Whether the pointer is over the sidebar and Clear and the URL field's buttons show, once the
/// sidebar has stopped moving.
fn tools(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, bool) {
    shell.update(cx, |shell, cx| {
        shell.sidebar_motion = None;
        cx.notify();
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| (shell.sidebar_hovered, shell.tools_shown()))
}

/// The pointer leaving while the sidebar is hidden, with nothing drawn to hear it go, leaves ⌘S's
/// sidebar without its tools until the pointer comes over it.
#[gpui::test]
fn the_sidebar_comes_back_with_its_tools_only_under_the_pointer(cx: &mut TestAppContext) {
    let (shell, cx, _host) = drawn_shell(cx);
    cx.simulate_mouse_move(point(px(100.), px(300.)), None, Modifiers::none());
    assert_eq!(tools(&shell, cx), (true, true));
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    assert!(!tools(&shell, cx).0);
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    assert_eq!(tools(&shell, cx), (false, false));
    cx.simulate_mouse_move(point(px(100.), px(300.)), None, Modifiers::none());
    assert_eq!(tools(&shell, cx), (true, true));
}

/// The reveal's panel shows them, and docking it, under the pointer that keeps it out, keeps them
/// with no fade.
#[gpui::test]
fn the_revealed_sidebar_keeps_its_tools_as_it_docks(cx: &mut TestAppContext) {
    let (shell, cx, _host) = drawn_shell(cx);
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    assert_eq!(tools(&shell, cx), (false, true));
    shell.update(cx, |shell, cx| shell.set_revealed(true, cx));
    assert_eq!(tools(&shell, cx), (false, true));
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    assert_eq!(tools(&shell, cx), (true, true));
    assert_eq!(shell.read_with(cx, |shell, _| shell.tools_opacity()), 1.);
}

/// A click on the URL field opens the command bar over the field, as Arc's does, and ⌘L centred.
/// With the sidebar hidden there's no field, and the bar is centred (*interim*).
#[gpui::test]
fn a_click_on_the_url_field_opens_the_command_bar_over_it(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(2, cx);
    });
    cx.run_until_parked();
    cx.simulate_click(point(px(100.), px(64.)), Modifiers::none());
    let field = Bounds::new(point(px(8.), px(46.)), size(px(212.), px(36.)));
    assert_eq!(host.command_bars.take(), [Some(field)]);
    shell.update(cx, |shell, cx| shell.run(Command::EditUrl, "ui", cx));
    assert_eq!(host.command_bars.take(), [None]);
    shell.update(cx, |shell, cx| {
        shell.sidebar_hidden = true;
        shell.run(Command::EditUrl, "sidebar", cx);
    });
    assert_eq!(host.command_bars.take(), [None]);
}

/// A click on the URL field's Copy URL button says so in the toast, as ⇧⌘C does and Arc's button
/// does, and swaps its glyph for the checkmark, without opening the command bar.
#[gpui::test]
fn copy_url_shows_the_toast_and_the_checkmark(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        shell.set_selected(1, cx);
        let page = Page {
            url: "https://example.org/?utm_source=x".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
    });
    // The pointer coming onto the sidebar fades the button in, at check.sh's point.
    move_to(cx, &host, 100., 300.);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.simulate_click(point(px(178.), px(64.)), Modifiers::none());
    assert_eq!(host.toasts.take(), ["Copied Current URL"]);
    assert!(host.command_bars.take().is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.url_copied.is_some()));
}

/// Where there's a share sheet, ⇧⌘C's toast has the capture toast's Share button, which shares the
/// page from where it was pressed in the toast. Copy URL as Markdown's toast has none.
#[gpui::test]
fn copy_url_s_toast_shares_the_page(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    host.share_sheet.set(true);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        shell.set_selected(1, cx);
        shell.set_title(1, "Example".into(), cx);
        let page = Page {
            url: "https://example.org/?utm_source=x".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
        shell.run(Command::CopyUrl, "ui", cx);
    });
    assert_eq!(host.toasts.take(), ["Copied Current URL [share]"]);
    let share = host
        .toast_share
        .take()
        .expect("the toast has no Share button");
    host.take();
    cx.update(|_, cx| share(point(px(240.), px(23.)), cx));
    assert_eq!(
        host.take(),
        ["share from toast https://example.org/ Example at 240 23"]
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::CopyMarkdownLink, "ui", cx)
    });
    assert_eq!(host.toasts.take(), ["Copied URL as Markdown"]);
    assert!(host.toast_share.take().is_none());
    // The URL field's button shows ⇧⌘C's toast, Share and all.
    shell.update(cx, |shell, cx| shell.copy_url_from_button(cx));
    assert_eq!(host.toasts.take(), ["Copied Current URL [share]"]);
}

/// The password manager's key shows after the host of the tab it's for. A press on it brings back
/// the offer that showing another tab closed, or opens the Site Controls, which list the site's
/// logins, and it goes once the page has nothing for the password manager.
#[gpui::test]
fn the_key_brings_the_offer_back_or_opens_the_site_controls(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    let page = Page {
        url: "https://example.com/".into(),
        ..Page::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(1, cx);
        shell.set_page(1, page, cx);
        shell.set_key(2, Some(Key::Offer), cx);
    });
    cx.run_until_parked();
    let key = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.button_frames.borrow().get("key").copied()
        })
    };
    assert_eq!(key(&shell, cx), None, "another tab's key showed");
    let press = |at: Bounds<gpui::Pixels>, cx: &mut VisualTestContext| {
        cx.simulate_mouse_down(at.center(), gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at.center(), gpui::MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    };

    shell.update(cx, |shell, cx| shell.set_key(1, Some(Key::Offer), cx));
    cx.run_until_parked();
    let at = key(&shell, cx).expect("the tab's key didn't show");
    host.take();
    press(at, cx);
    assert_eq!(host.take(), ["show offer 1"]);

    shell.update(cx, |shell, cx| shell.set_key(1, Some(Key::Saved), cx));
    cx.run_until_parked();
    press(at, cx);
    assert_eq!(
        host.site_controls
            .take()
            .iter()
            .map(|(tab, _)| *tab)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(
        !host
            .take()
            .iter()
            .any(|call| call.starts_with("show offer"))
    );

    shell.update(cx, |shell, cx| shell.set_key(1, None, cx));
    cx.run_until_parked();
    press(at, cx);
    assert!(
        host.site_controls.take().is_empty(),
        "a press where the key was opened them"
    );
    assert!(
        !host
            .take()
            .iter()
            .any(|call| call.starts_with("show offer"))
    );
}

/// Back, forward and reload shrink about their middle, 1 pt in at each end, over PRESS_TIME while
/// they're held, and grow back as long once let go; the click still goes on the release. Reload
/// keeps shrinking as it turns into stop. Held for its menu, back grows back as the menu opens, and
/// the sidebar button doesn't shrink. Grown back, the button is let go at the next frame, so a
/// button at rest asks for no frames.
#[gpui::test]
fn back_forward_and_reload_shrink_a_little_while_held(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    let page = Page {
        url: "https://example.org/".into(),
        can_go_back: true,
        ..Page::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.set_page(10, page.clone(), cx);
    });
    cx.run_until_parked();
    host.take();
    let at = |x: f32| point(px(x + NAV_SIZE / 2.), px(NAV_TOP + NAV_SIZE / 2.));
    let scale = |cx: &mut VisualTestContext, x| shell.read_with(cx, |shell, _| shell.nav_scale(x));
    // As if the press were `ago` seconds back, and the release `let_go` seconds back.
    let back = |cx: &mut VisualTestContext, ago: f32, let_go: f32| {
        shell.update(cx, |shell, _| {
            let held = shell.held_nav.as_mut().unwrap();
            let now = Instant::now();
            held.pressed = now - Duration::from_secs_f32(ago);
            held.let_go = held.let_go.map(|_| now - Duration::from_secs_f32(let_go));
        })
    };
    let between = |scale: f32| scale < 1. && scale > NAV_PRESS_SCALE;
    assert_eq!(NAV_SIZE * NAV_PRESS_SCALE, NAV_SIZE - 2.);

    cx.simulate_mouse_down(at(BACK_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    back(cx, PRESS_TIME / 2., 0.);
    assert!(between(scale(cx, BACK_X)), "half way in");
    back(cx, PRESS_TIME * 2., 0.);
    assert_eq!(scale(cx, BACK_X), NAV_PRESS_SCALE);
    assert_eq!(
        (scale(cx, FORWARD_X), scale(cx, RELOAD_X)),
        (1., 1.),
        "only the one held"
    );
    assert!(
        !host.take().contains(&"back 10".to_string()),
        "not before the release"
    );

    cx.simulate_mouse_up(at(BACK_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().contains(&"back 10".to_string()));
    back(cx, PRESS_TIME * 3., PRESS_TIME / 2.);
    assert!(between(scale(cx, BACK_X)), "half way out");
    back(cx, PRESS_TIME * 3., PRESS_TIME);
    assert_eq!(scale(cx, BACK_X), 1.);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.held_nav.is_none()),
        "let go once grown back"
    );

    cx.simulate_mouse_down(at(RELOAD_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        shell.set_page(
            10,
            Page {
                loading: true,
                ..page.clone()
            },
            cx,
        )
    });
    cx.run_until_parked();
    back(cx, PRESS_TIME * 2., 0.);
    assert_eq!(
        scale(cx, RELOAD_X),
        NAV_PRESS_SCALE,
        "reload, turned into stop"
    );
    cx.simulate_mouse_up(at(RELOAD_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    shell.update(cx, |shell, cx| shell.set_page(10, page.clone(), cx));
    cx.simulate_mouse_down(at(BACK_X), MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(HOLD);
    cx.run_until_parked();
    assert!(host.take().contains(&"history 10 back".to_string()));
    let let_go = shell.read_with(cx, |shell, _| {
        shell.held_nav.as_ref().map(|held| held.let_go)
    });
    assert!(
        matches!(let_go, Some(Some(_))),
        "its menu took the press over"
    );
    cx.simulate_mouse_up(at(BACK_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    shell.update(cx, |shell, _| shell.held_nav = None);
    cx.simulate_mouse_down(at(SIDEBAR_BUTTON_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.held_nav.is_none()),
        "the sidebar button"
    );
}

/// The URL field says the page's host, and its whole URL while it's in Developer Mode.
#[gpui::test]
fn the_url_field_shows_the_whole_url_in_developer_mode(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[], &[1], Folders::default());
    let url = "http://localhost:3000/app?tab=2#top";
    let mut words = |developer_mode| {
        shell.update(cx, |shell, cx| {
            let page = Page {
                url: url.into(),
                developer_mode,
                ..Page::default()
            };
            shell.set_page(1, page, cx);
            shell.set_selected(1, cx);
            shell.url_field_words(url, cx).0.to_string()
        })
    };
    assert_eq!(words(false), "localhost");
    assert_eq!(words(true), url);
}

/// In Developer Mode the URL field has DevTools' button before Copy URL, under the pointer as
/// those are, named with ⌥⌘I, and a click on it opens the page's DevTools as ⌥⌘I does. Out of it
/// there's none, and a click there is the field's, which opens the command bar.
#[gpui::test]
fn developer_mode_puts_devtools_in_the_url_field(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    let url = "http://localhost:3000/";
    let set = |developer_mode, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let page = Page {
                url: url.into(),
                developer_mode,
                ..Page::default()
            };
            shell.set_page(1, page, cx);
        });
        cx.run_until_parked();
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        shell.set_selected(1, cx);
    });
    set(true, &shell, cx);
    move_to(cx, &host, 100., 300.);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    host.take();
    cx.simulate_click(point(px(150.), px(64.)), Modifiers::none());
    let calls = host.take();
    assert!(
        calls
            .iter()
            .any(|call| call == "tooltip Developer Tools ⌥⌘I"),
        "{calls:?}"
    );
    let calls: Vec<_> = calls
        .into_iter()
        .filter(|call| !call.starts_with("tooltip"))
        .collect();
    assert_eq!(calls, ["devtools 1 0"]);
    assert!(host.command_bars.take().is_empty());
    set(false, &shell, cx);
    cx.simulate_click(point(px(150.), px(64.)), Modifiers::none());
    assert!(host.take().iter().all(|call| !call.starts_with("devtools")));
    assert_eq!(host.command_bars.take().len(), 1);
}

/// While the page slides with a Space switch, a press on DevTools' place does what it does once
/// it settles, in Developer Mode only.
#[gpui::test]
fn devtools_rests_in_the_url_field_in_developer_mode(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[], &[1], Folders::default());
    let at = |developer_mode, cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| {
            let page = Page {
                url: "http://localhost/".into(),
                developer_mode,
                ..Page::default()
            };
            shell.set_page(1, page, cx);
            shell.set_selected(1, cx);
            shell.sidebar_hovered = true;
            shell.tools_on = true;
            shell.field_part_at(150. - 8.)
        })
    };
    assert_eq!(at(true, cx), AtRest::DevTools);
    assert_eq!(at(false, cx), AtRest::Field);
}

/// The header's glyphs take the disabled alpha, 0.26, as it is while the host says the window isn't
/// the one the user is in, as Arc's do, under the glyph floor that would leave the dim unseen, and
/// 0.60 again once it is. In the user's window a disabled glyph, as back's with no history, keeps
/// the floor. GPUI's window losing the keyboard, as it does to the window's own page on X11 and to
/// its command bar or menu on the Mac, leaves them lit, and so does the command bar open over it.
#[gpui::test]
fn the_header_glyphs_dim_while_the_window_is_not_the_users(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn_shell(cx);
    let round = |alpha: f32| (alpha * 100.).round() / 100.;
    let alphas = |cx: &mut VisualTestContext| {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            let [lit, disabled] = [true, false].map(|enabled| round(shell.nav_glyph(enabled).a));
            (shell.nav_dimmed_drawn.get(), lit, disabled)
        })
    };
    let set_main = |main: bool, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.set_window_main(main, cx));
    };
    let floor = round(shell.read_with(cx, |shell, _| shell.palette.glyph_floor));
    assert!(
        floor > 0.26,
        "the test's Space needs a floor over 0.26, not {floor}"
    );
    let lit = (Some(false), 0.6, floor);
    cx.update(|window, _| window.activate_window());
    assert_eq!(alphas(cx), lit, "not lit in the user's window");
    // The page or an overlay has the keyboard: GPUI's window isn't active, the host's still main.
    cx.deactivate_window();
    assert_eq!(
        alphas(cx),
        lit,
        "dimmed as the page or an overlay took the keys"
    );
    host.bar_open.set(true);
    assert_eq!(alphas(cx), lit, "dimmed under the window's command bar");
    host.bar_open.set(false);
    // Another window came to the front: 0.26 for every glyph, under the floor.
    set_main(false, cx);
    assert_eq!(
        alphas(cx),
        (Some(true), 0.26, 0.26),
        "not 0.26 behind another window"
    );
    // Back, by a click in its page, which doesn't give GPUI's window the keys.
    set_main(true, cx);
    assert_eq!(alphas(cx), lit, "not lit once it's the user's again");
}
