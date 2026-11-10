//! A Little ilo window's shell on a host that records what it asks of the core: what closes it,
//! what it leaves out, and where ⌥⌘N and ⌘O send their page.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};

/// A regular window's shell with the user's two Spaces, the first shown.
fn main_window(cx: &mut TestAppContext, host: &Rc<Recorder>) -> Entity<Shell> {
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_space(1, cx);
    });
    shell
}

/// A Little ilo window for the Space One, its own Space 9 holding the page 10 at `url`.
fn little(cx: &mut TestAppContext, url: &str) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    let shell = cx.new(|cx| Shell::little_window(host.clone(), 9, 1, 1, Some(&main), cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.set_page(
            10,
            Page {
                url: url.to_string().into(),
                ..Page::default()
            },
            cx,
        );
    });
    host.take();
    (shell, host)
}

fn run(shell: &Entity<Shell>, cx: &mut TestAppContext, command: Command, from: &str) {
    shell.update(cx, |shell, cx| shell.run(command, from, cx));
}

#[gpui::test]
fn it_takes_its_spaces_look_and_has_no_sidebar(cx: &mut TestAppContext) {
    let (shell, _) = little(cx, "https://example.com/");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.little(), Some(1));
        assert!(shell.sidebar_hidden);
        let own: Vec<_> = (shell.spaces.iter())
            .map(|space| (space.id, space.name.to_string(), space.color))
            .collect();
        assert_eq!(own, [(9, "One".to_string(), Some(7))]);
    });
    // The Space renamed and recoloured, the window follows.
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Uno".into(), Some(3), 1, "".into(), "".into(), "".into()),
                (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
            ],
            cx,
        )
    });
    shell.read_with(cx, |shell, _| {
        let own = &shell.spaces[0];
        let look = (shell.spaces.len(), own.id, own.name.as_ref(), own.color);
        assert_eq!(look, (1, 9, "Uno", Some(3)));
    });
}

#[gpui::test]
fn command_w_closes_the_window_not_the_tab(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    run(&shell, cx, Command::CloseTab, "ui");
    assert_eq!(host.take(), ["close window"]);
}

#[gpui::test]
fn an_unused_escape_closes_it_unless_the_page_is_loading(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://example.com/".into(),
            loading: true,
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    // Escape stops the load first.
    assert!(!shell.update(cx, |shell, cx| shell.escape_unused(cx)));
    shell.update(cx, |shell, cx| {
        shell.set_page(
            10,
            Page {
                url: "https://example.com/".into(),
                ..Page::default()
            },
            cx,
        )
    });
    run(&shell, cx, Command::Stop, "page");
    assert_eq!(host.take(), ["close window"]);
}

#[gpui::test]
fn an_escape_in_another_window_is_the_pages(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    host.take();
    assert!(!main.update(cx, |shell, cx| shell.escape_unused(cx)));
    assert!(host.take().is_empty());
}

#[gpui::test]
fn the_sidebar_spaces_and_split_view_are_left_out(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    for command in [
        Command::ToggleSidebar,
        Command::NewSpace,
        Command::NextSpace,
        Command::AddSplit,
        Command::ViewLibrary,
        Command::NewTab,
        Command::ReopenClosedTab,
    ] {
        shell.read_with(cx, |shell, _| {
            assert!(!shell.offers(command), "{command:?}")
        });
        run(&shell, cx, command, "ui");
    }
    assert!(host.take().is_empty());
    shell.read_with(cx, |shell, _| assert!(shell.sidebar_hidden));
}

#[gpui::test]
fn command_o_moves_the_page_to_its_space_in_the_main_window(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    shell.read_with(cx, |shell, _| {
        assert!(shell.offers(Command::OpenInMainWindow))
    });
    run(&shell, cx, Command::OpenInMainWindow, "ui");
    assert_eq!(host.take(), ["open in main window 10 1"]);
}

#[gpui::test]
fn it_turns_its_pages_developer_mode_off_and_on(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    shell.read_with(cx, |shell, _| {
        assert!(shell.offers(Command::ToggleDeveloperMode))
    });
    run(&shell, cx, Command::ToggleDeveloperMode, "command bar");
    assert_eq!(host.take(), ["developer mode 10 true"]);
}

#[gpui::test]
fn open_in_main_window_is_little_ilos_alone(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    host.take();
    main.read_with(cx, |shell, _| {
        assert!(!shell.offers(Command::OpenInMainWindow))
    });
    run(&main, cx, Command::OpenInMainWindow, "ui");
    assert!(host.take().is_empty());
}

#[gpui::test]
fn a_new_little_window_is_for_the_space_it_came_from(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| shell.set_space(2, cx));
    host.take();
    run(&main, cx, Command::NewLittleWindow, "ui");
    assert_eq!(host.take(), ["new little window 2"]);
    let (shell, host) = little(cx, "https://example.com/");
    run(&shell, cx, Command::NewLittleWindow, "ui");
    assert_eq!(host.take(), ["new little window 1"]);
}

/// ⌥⌘N reaches the browser before the page, as ⌘N does, and ⌘O only if the page doesn't keep it.
#[test]
fn its_keys_are_option_command_n_and_command_o() {
    let keys = crate::shell::UserKeys::default();
    assert_eq!(
        keys.command_for("alt-cmd-n", true),
        Some(Command::NewLittleWindow)
    );
    assert_eq!(keys.command_for("cmd-o", true), None);
    assert_eq!(
        keys.command_for("cmd-o", false),
        Some(Command::OpenInMainWindow)
    );
}

/// A left press on a sidebar row of the main window, with `modifiers`, as the row's handler gets it.
fn press_row(
    shell: &Entity<Shell>,
    tab: u64,
    modifiers: gpui::Modifiers,
    cx: &mut TestAppContext,
) -> bool {
    let event = gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: gpui::point(gpui::px(40.), gpui::px(200.)),
        modifiers,
        click_count: 1,
        first_mouse: false,
    };
    shell.update(cx, |shell, cx| shell.little_press_row(tab, &event, cx))
}

/// ⌥ and ⌘, which is Control off the Mac, as the host's platform has them.
fn option_command(cx: &mut TestAppContext) -> gpui::Modifiers {
    let command = cx.read(gpui::Modifiers::secondary_key);
    gpui::Modifiers {
        alt: true,
        ..command
    }
}

/// ⌥⌘-click on a Today tab's row opens that tab in a Little ilo window (Arc), and on a pinned
/// tab, with ⇧ too, or with ⌥ alone it doesn't, leaving ⌥-click's split view to the row.
#[gpui::test]
fn option_command_click_on_a_today_tab_opens_it_in_little_ilo(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![4], vec![5, 6], Vec::new(), Folders::default(), cx);
        shell.set_selected(5, cx);
    });
    host.take();
    let alt_cmd = option_command(cx);
    assert!(
        !press_row(&main, 4, alt_cmd, cx),
        "a pinned tab keeps its place"
    );
    let alt = gpui::Modifiers {
        alt: true,
        ..Default::default()
    };
    assert!(!press_row(&main, 6, alt, cx));
    let alt_shift_cmd = gpui::Modifiers {
        shift: true,
        ..alt_cmd
    };
    assert!(!press_row(&main, 6, alt_shift_cmd, cx));
    assert!(host.take().is_empty());
    assert!(press_row(&main, 6, alt_cmd, cx));
    // After the hover cards it closes.
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("open tab in little 6")
    );
}

/// Off the Mac ⌘ is Control, so ⌃⌥-click opens the tab in Little ilo there, and ⌥ with Super,
/// the platform key there, is left to ⌥-click's split.
#[gpui::test]
fn off_the_mac_option_control_click_opens_little_ilo(cx: &mut TestAppContext) {
    let info = cx.read(|cx| cx.platform_info());
    cx.set_platform_info(gpui::PlatformInfo {
        os: gpui::OperatingSystem::Linux,
        ..info
    });
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![4], vec![5, 6], Vec::new(), Folders::default(), cx);
        shell.set_selected(5, cx);
    });
    host.take();
    let alt_super = gpui::Modifiers {
        alt: true,
        platform: true,
        ..Default::default()
    };
    assert!(!press_row(&main, 6, alt_super, cx));
    let alt_control = gpui::Modifiers {
        alt: true,
        control: true,
        ..Default::default()
    };
    assert!(press_row(&main, 6, alt_control, cx));
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("open tab in little 6")
    );
}

/// A Little ilo window has no sidebar rows of its own to send, and an agent's Space keeps its tabs.
#[gpui::test]
fn option_command_click_is_the_users_spaces_alone(cx: &mut TestAppContext) {
    let alt_cmd = option_command(cx);
    let (shell, host) = little(cx, "https://example.com/");
    assert!(!press_row(&shell, 10, alt_cmd, cx));
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), None, 1, "".into(), "".into(), "".into()),
                (
                    3,
                    "Agent".into(),
                    None,
                    1,
                    "".into(),
                    "claude".into(),
                    "".into(),
                ),
            ],
            cx,
        );
        shell.set_space(3, cx);
        shell.set_tabs(3, Vec::new(), vec![7], Vec::new(), Folders::default(), cx);
    });
    host.take();
    assert!(!press_row(&main, 7, alt_cmd, cx));
    assert!(host.take().is_empty());
}

/// With more than one profile, the bar has a profile picker before Copy Link, whose menu lists
/// them, the page's checked, and one picked goes to the app to open the page in. With one profile
/// there's nothing to pick, and no button.
#[gpui::test]
fn its_bar_picks_the_pages_profile_once_there_are_two(cx: &mut TestAppContext) {
    use gpui::{Modifiers, point, px};

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::little_window(browser, 9, 1, 1, None, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
        shell.set_profiles(vec![(1, "Default".into())], cx);
    });
    cx.run_until_parked();
    // Before Copy Link.
    let picker = shell.read_with(cx, |shell, cx| shell.little_end(cx).profile);
    let click = |cx: &mut gpui::VisualTestContext| {
        cx.simulate_click(picker, Modifiers::none());
        cx.run_until_parked();
    };
    click(cx);
    assert!(
        host.menus.borrow().is_empty(),
        "a profile menu with one profile"
    );

    shell.update(cx, |shell, cx| {
        shell.set_profiles(vec![(1, "Default".into()), (5, "Work".into())], cx)
    });
    cx.run_until_parked();
    host.take();
    click(cx);
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    assert_eq!(menu, [("Default".into(), true), ("Work".into(), true)]);
    // It hangs from its plate's left edge, 4 under the header's 32 pt plate.
    assert_eq!(
        host.menu_at.get(),
        Some(point(picker.x - px(16.), px(7.5 + 32. + 4.)))
    );
    let checked: Vec<bool> = host
        .entries
        .borrow()
        .iter()
        .map(|entry| entry.checked)
        .collect();
    assert_eq!(checked, [true, false]);
    cx.update(|_, cx| host.choose("Work", cx));
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("little profile 5 https://example.com/")
    );
}

/// A Little ilo window knows the profiles the window it opened from does, so its bar has the
/// picker from its first frame, not only once the core next reports them.
#[gpui::test]
fn its_bar_has_the_picker_from_the_start(cx: &mut TestAppContext) {
    use gpui::Modifiers;

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        let from = cx.new(|cx| {
            let mut from = Shell::new(browser.clone(), cx);
            from.profiles = vec![(1, "Default".into()), (5, "Work".into())];
            from
        });
        Shell::little_window(browser, 9, 1, 1, Some(&from), cx)
    });
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    let picker = shell.read_with(cx, |shell, cx| shell.little_end(cx).profile);
    cx.simulate_click(picker, Modifiers::none());
    cx.run_until_parked();
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    assert_eq!(menu, [("Default".into(), true), ("Work".into(), true)]);
}

/// The bar is the header's row. Open in <Space> ⌘O | ⌄ ends it: its chevron's plate ends 4 in from
/// the window's right edge, where a header button's would whose glyph mirrors the close button's,
/// and its words name the window's Space and ⌘O. Copy Link's plate is 2 before it and names itself
/// and its key as the header's buttons do. The title, from where the header's first button is,
/// opens the command bar over itself.
#[gpui::test]
fn its_bar_is_the_headers_row(cx: &mut TestAppContext) {
    use gpui::{Modifiers, VisualTestContext, point, px};

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        let main = cx.new(|cx| {
            let mut main = Shell::new(browser.clone(), cx);
            main.set_spaces(
                vec![
                    (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                    (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
                ],
                cx,
            );
            main
        });
        Shell::little_window(browser, 9, 1, 1, Some(&main), cx)
    });
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    let width = cx.update(|window, _| f32::from(window.viewport_size().width));
    let (end, words) = shell.read_with(cx, |shell, cx| {
        (shell.little_end(cx), shell.little_open_words(cx))
    });
    assert_eq!(words, ("Open in One".into(), Some("⌘O".into())));
    let x = |at: gpui::Pixels| f32::from(at);
    assert_eq!(
        (x(end.chevron.left()), x(end.chevron.right())),
        (width - 32., width - 4.)
    );
    assert_eq!(
        (x(end.line), x(end.words.right())),
        (width - 33., width - 33.)
    );
    assert_eq!(x(end.copy.x), x(end.words.left()) - 2. - 16.);
    // The header's row: its plates 7.5 from the top, 32 tall.
    let row = 7.5 + 16.;
    let rest = |cx: &mut VisualTestContext, x: f32| {
        host.take();
        cx.simulate_mouse_move(point(px(x), px(row)), None, Modifiers::none());
        cx.run_until_parked();
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("tooltip"))
            .collect::<Vec<_>>()
    };
    let click = |cx: &mut VisualTestContext, x: f32| {
        host.take();
        cx.simulate_click(point(px(x), px(row)), Modifiers::none());
        cx.run_until_parked();
        host.take()
    };
    let words_middle = x(end.words.center().x);
    assert_eq!(rest(cx, x(end.copy.x)), ["tooltip Copy URL ⇧⌘C"]);
    assert_eq!(rest(cx, words_middle), ["tooltip none"]);
    assert!(click(cx, x(end.copy.x)).contains(&"copy https://example.com/".to_string()));
    assert!(click(cx, words_middle).contains(&"open in main window 10 1".to_string()));
    click(cx, 77. + 16.);
    let field = host
        .command_bars
        .borrow()
        .last()
        .copied()
        .flatten()
        .expect("no field");
    assert_eq!(
        (f32::from(field.origin.x), f32::from(field.origin.y)),
        (77., 7.5)
    );
    assert!(f32::from(field.right()) < x(end.copy.x) - 16.);
}

/// The chevron opens the Spaces menu: the compact command bar 240 wide, its right edge the
/// button's and its top 4 under it, listing the user's Spaces in the sidebar's order, an agent's
/// left out. A Space picked takes the page there; one in another profile asks first, the first
/// time, as Move to Space does.
#[gpui::test]
fn its_chevron_lists_the_users_spaces_to_send_the_page_to(cx: &mut TestAppContext) {
    use gpui::{Modifiers, point, px};

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        let main = cx.new(|cx| {
            let mut main = Shell::new(browser.clone(), cx);
            main.set_spaces(
                vec![
                    (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                    (
                        3,
                        "Agent".into(),
                        None,
                        1,
                        "".into(),
                        "claude".into(),
                        "".into(),
                    ),
                    (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
                    (4, "Work".into(), None, 5, "".into(), "".into(), "".into()),
                ],
                cx,
            );
            main.profiles = vec![(1, "Default".into()), (5, "Job".into())];
            main
        });
        Shell::little_window(browser, 9, 1, 1, Some(&main), cx)
    });
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    let end = shell.read_with(cx, |shell, cx| shell.little_end(cx));
    host.take();
    cx.simulate_click(end.chevron.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        host.take()
            .contains(&"spaces menu 1 One, 2 Two, 4 Work".to_string())
    );
    let menu = host.spaces_menus.borrow().last().copied().expect("no menu");
    // Its right edge 8 in from the window's, as every overlay's is, which is 4 inside the chevron's.
    let at = (menu.origin, menu.size.width, end.chevron.right());
    let width = shell.read_with(cx, |shell, _| shell.viewport.width);
    let right = width - px(8.);
    assert_eq!(
        at,
        (
            point(right - px(240.), px(7.5 + 32. + 4.)),
            px(240.),
            width - px(4.)
        )
    );

    // A Space renamed is named so the next time.
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                (2, "Deux".into(), None, 1, "".into(), "".into(), "".into()),
            ],
            cx,
        )
    });
    cx.simulate_click(end.chevron.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        host.take()
            .contains(&"spaces menu 1 One, 2 Deux".to_string())
    );

    // The bar's row runs Move to Space, which here sends the page there.
    run(&shell, cx, Command::MoveToSpace(2), "command bar");
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("open in main window 10 2")
    );
    // Another profile's Space, once there's one again, asks first.
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                (4, "Work".into(), None, 5, "".into(), "".into(), "".into()),
            ],
            cx,
        )
    });
    host.warn_profiles.set(true);
    run(&shell, cx, Command::MoveToSpace(4), "command bar");
    assert!(
        !host
            .take()
            .iter()
            .any(|call| call.starts_with("open in main window"))
    );
    let (message, _, confirmed) = host.confirming.borrow_mut().take().expect("no warning");
    assert_eq!(message, "Move this tab to the “Job” profile?");
    cx.update(|_, cx| confirmed(cx));
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("open in main window 10 4")
    );
}

/// ⌘L and ⌘T open the command bar inside the window, on its page, 500 wide and centred across it,
/// its top a third of the way down, as Arc's does; only a click on the title opens it over the
/// title.
#[gpui::test]
fn its_keys_open_the_command_bar_inside_it(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::little_window(browser, 9, 1, 1, None, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    let size = cx.update(|window, _| window.viewport_size());
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    for command in [Command::EditUrl, Command::NewTab] {
        host.take();
        shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
        let bar = host
            .bars_in
            .borrow_mut()
            .pop()
            .expect("no bar inside the window");
        let bar_width = 500f32.min(width);
        let at = (
            f32::from(bar.origin.x),
            f32::from(bar.origin.y),
            f32::from(bar.size.width),
        );
        assert_eq!(
            at,
            ((width - bar_width) / 2., height * 0.333_333, bar_width)
        );
        assert_eq!(host.bar_edits.borrow_mut().pop(), Some(Some(10)));
        assert!(
            host.command_bars.borrow().is_empty(),
            "{command:?} opened over the title"
        );
    }
    shell.update(cx, |shell, cx| {
        shell.run(Command::EditUrl, "little bar", cx)
    });
    assert!(host.bars_in.borrow().is_empty());
    assert!(
        host.command_bars
            .borrow()
            .last()
            .copied()
            .flatten()
            .is_some()
    );
}

#[gpui::test]
fn it_grows_in_from_its_first_frame_on_screen_once(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        let main = cx.new(|cx| Shell::new(browser.clone(), cx));
        Shell::little_window(browser, 9, 1, 1, Some(&main), cx)
    });
    cx.run_until_parked();
    let drawn = |host: &Recorder| {
        host.take()
            .iter()
            .filter(|call| *call == "little drawn")
            .count()
    };
    assert_eq!(
        drawn(&host),
        1,
        "its first frame on screen didn't start its opening"
    );
    // Drawn again, it has grown already.
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(drawn(&host), 0, "a later frame started its opening again");

    // A regular window has no opening of its own.
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (_, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.run_until_parked();
    assert_eq!(drawn(&host), 0, "a regular window asked to grow in");
}

/// A Little ilo window drawn for the Space One, its own Space 9 holding the page 10.
fn drawn_little(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut gpui::VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::little_window(browser, 9, 1, 1, None, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://signin.example/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// A page's question has no URL field to hang from in a Little ilo window, and nothing can be drawn
/// over the page, so it hangs from the bar and the page makes room under it, until it's answered
/// (#1914).
#[gpui::test]
fn a_question_hangs_under_the_bar_and_the_page_makes_room(cx: &mut TestAppContext) {
    use crate::shell::Question;
    use crate::tokens::shell::LITTLE_BAR;
    use gpui::px;

    let (shell, cx, _) = drawn_little(cx);
    let page_top = |cx: &mut gpui::VisualTestContext| {
        shell.update(cx, |shell, _| {
            let viewport = shell.viewport;
            shell.card_frames(viewport).0.top()
        })
    };
    assert_eq!(page_top(cx), px(LITTLE_BAR));
    shell.update(cx, |shell, cx| {
        let permissions = vec!["Access other apps and services on this device".into()];
        let question = Question {
            site: "signin.example".into(),
            permissions,
            quiet: false,
        };
        shell.set_question(10, Some(question), cx);
    });
    cx.run_until_parked();
    let card = shell.read_with(cx, |shell, _| shell.question_drawn.get());
    let card = card.expect("the question wasn't drawn");
    // Its drawn bounds are inside its 1-point edge, which starts at the bar's foot.
    assert_eq!(card.top(), px(LITTLE_BAR + 1.), "it hangs from the bar");
    assert_eq!(
        page_top(cx),
        card.bottom() + px(crate::shell::CARD_INSET),
        "the page goes under it"
    );
    // The core lays the page out there by the next frame, with nothing else to draw the window
    // again: the page covered the question until something else did (#1930).
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    let laid = shell.read_with(cx, |shell, _| shell.page_laid.map(|(frame, _)| frame.top()));
    assert_eq!(
        laid,
        Some(card.bottom() + px(crate::shell::CARD_INSET)),
        "the core wasn't told to lay the page out under the question"
    );
    shell.update(cx, |shell, cx| shell.set_question(10, None, cx));
    cx.run_until_parked();
    assert_eq!(page_top(cx), px(LITTLE_BAR), "the page takes its room back");
}

/// A tab the window's page opened goes behind, where no window shows it, so when its page asks to
/// open another app, as a sign-in's pop-up hands back to its app, the Little ilo window asks
/// instead, over its page (#1914). Its own page's dialog comes first, and the other's other dialogs
/// wait for their tab.
#[gpui::test]
fn a_tab_sent_behind_asks_to_open_an_app_here(cx: &mut TestAppContext) {
    use crate::shell::{DialogKind, PageDialog};

    let (shell, cx, host) = drawn_little(cx);
    let dialog = |id: u64, kind: DialogKind| PageDialog {
        id,
        kind,
        title: "Open Handback?".into(),
        message: "".into(),
        text: "".into(),
        accept: "Open Handback".into(),
        cancel: "Cancel".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    };
    let shown = |cx: &mut gpui::VisualTestContext| {
        cx.run_until_parked();
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("page dialog"))
            .collect::<Vec<_>>()
    };
    shell.update(cx, |shell, cx| {
        shell.little_sent(11, cx);
        shell.set_page_dialog(11, Some(dialog(1, DialogKind::Alert)), cx);
    });
    assert_eq!(
        shown(cx),
        Vec::<String>::new(),
        "a sent tab's alert waits for it"
    );
    shell.update(cx, |shell, cx| {
        shell.set_page_dialog(11, Some(dialog(2, DialogKind::OpenApp)), cx)
    });
    assert_eq!(shown(cx), ["page dialog 11 2"]);
    shell.update(cx, |shell, cx| {
        shell.set_page_dialog(10, Some(dialog(3, DialogKind::Alert)), cx)
    });
    assert_eq!(
        shown(cx),
        ["page dialog 10 3"],
        "its own page's comes first"
    );
    shell.update(cx, |shell, cx| {
        shell.answer_page_dialog(10, 3, accept(), cx);
    });
    let after: Vec<String> = shown(cx);
    assert!(
        after.iter().any(|call| call == "page dialog 11 2"),
        "the sent tab's asks again once its page's is answered: {after:?}"
    );
    shell.update(cx, |shell, cx| {
        shell.answer_page_dialog(11, 2, accept(), cx)
    });
    let answered = host.take();
    assert!(
        answered
            .iter()
            .any(|call| call == "answer dialog 11 2 accept"),
        "Open answered the sent tab's page: {answered:?}"
    );
}

fn accept() -> crate::shell::PageReply {
    crate::shell::PageReply {
        accept: true,
        text: String::new(),
        password: String::new(),
        checked: false,
    }
}

/// A Little ilo window's pinned extensions are buttons at its bar's title's end, as at the URL
/// field's in the sidebar, on its pitch: a click runs one, its popup hanging from its button. More
/// than fit end in the overflow button (#693).
#[gpui::test]
fn its_pinned_extensions_end_its_bars_title(cx: &mut TestAppContext) {
    use crate::shell::ExtensionAction;
    use crate::tokens::shell::URL_BUTTON_SIZE;

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::little_window(browser, 9, 1, 1, None, cx));
    // Little ilo's first size.
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(640.)));
    let action = |id: &str| ExtensionAction {
        id: id.to_string().into(),
        title: id.to_string().into(),
        icon: None,
        badge: "".into(),
        badge_color: 0,
        enabled: true,
        pinned: true,
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let url = "https://example.com/".into();
        shell.set_page(
            10,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
        shell.set_extension_actions(vec![action("guard"), action("notes")], cx);
    });
    cx.run_until_parked();
    let title = shell
        .read_with(cx, |shell, _| {
            shell.little.as_ref().and_then(|l| l.title.get())
        })
        .expect("the bar's title wasn't drawn");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.shown_actions()),
        (2, false)
    );
    host.take();
    shell.update(cx, |shell, cx| shell.run_extension("notes", cx));
    let size = URL_BUTTON_SIZE;
    let top = f32::from(title.top()) + (f32::from(title.size.height) - size) / 2.;
    let left = f32::from(title.right()) - size;
    assert_eq!(
        host.take(),
        [format!(
            "run extension notes from {left} {top} {size} {size}"
        )],
        "the last one's popup hangs from the title's end"
    );

    // More than its room: the last place is the overflow button.
    let many: Vec<_> = (0..40).map(|n| action(&format!("e{n}"))).collect();
    shell.update(cx, |shell, cx| shell.set_extension_actions(many, cx));
    cx.run_until_parked();
    let (shown, overflow) = shell.read_with(cx, |shell, _| shell.shown_actions());
    assert!(overflow && shown < 40, "{shown} shown, overflow {overflow}");
}

/// With Little ilo off in Settings › Features: ⌥⌘N and the command bar's New Little ilo Window
/// open nothing and aren't offered, File has none, and ⌥⌘-click on a Today tab's row is a click.
/// On again, all of it comes back.
#[gpui::test]
fn off_nothing_offers_or_opens_a_little_window(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![5, 6],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(5, cx);
        shell.set_little_windows(false, cx);
    });
    host.take();
    main.read_with(cx, |shell, cx| {
        assert!(!shell.offers(Command::NewLittleWindow));
        assert!(
            shell.menu_bar(cx).little_off,
            "File still has New Little ilo Window"
        );
        assert_eq!(
            shell.bar_name("New Little ilo Window", Command::NewLittleWindow, cx),
            None
        );
    });
    run(&main, cx, Command::NewLittleWindow, "key");
    let alt_cmd = option_command(cx);
    assert!(
        !press_row(&main, 6, alt_cmd, cx),
        "⌥⌘-click opened Little ilo while it's off"
    );
    assert!(
        !host.take().iter().any(|call| call.contains("little")),
        "something asked for a Little ilo window while it's off"
    );
    main.update(cx, |shell, cx| shell.set_little_windows(true, cx));
    run(&main, cx, Command::NewLittleWindow, "key");
    assert_eq!(
        host.take().last().map(String::as_str),
        Some("new little window 1")
    );
    assert!(main.read_with(cx, |shell, cx| !shell.menu_bar(cx).little_off));
}

#[gpui::test]
fn its_bar_says_a_sign_in_page_is_closing_and_how_to_bring_it_back(cx: &mut TestAppContext) {
    let url = "http://127.0.0.1:8976/callback?code=x";
    let (shell, host) = little(cx, url);
    let title = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, cx| {
            shell.little_bar_title(10, "127.0.0.1", url, cx).to_string()
        })
    };
    let before = title(&shell, cx);
    shell.update(cx, |shell, cx| shell.set_little_closing(true, cx));
    let closing = title(&shell, cx);
    assert!(closing.starts_with("Signed in, closing · "), "{closing}");
    assert!(closing.ends_with(" reopens"), "{closing}");
    // Turned off, or the page moved on, before it closed: the bar is the page's again, and nothing
    // was asked of the core.
    shell.update(cx, |shell, cx| shell.set_little_closing(false, cx));
    assert_eq!(title(&shell, cx), before);
    assert!(host.take().is_empty());
}
