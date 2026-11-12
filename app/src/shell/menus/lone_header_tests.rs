//! A lone unnamed Space hides its name row, as Arc 1.171's does, and its menu brings it back, in a
//! window on a host that records what the shell asks of the core (#1400).

use std::rc::Rc;

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use crate::shell::{
    Folders, NAME_HEIGHT, ROW_PITCH, Shell, Slot, SpaceEntry, recorder::Recorder, rows::given_name,
};
use crate::theme::Appearance;
use crate::theme_picker;

/// A Space `id` named `name`, with `icon`.
fn space(id: u64, name: &str, icon: &str) -> SpaceEntry {
    (
        id,
        name.to_string().into(),
        None,
        1,
        icon.to_string().into(),
        "".into(),
        "".into(),
    )
}

/// A window on `spaces`, showing the first, which has pinned tab 10 and Today tab 11.
fn window(
    cx: &mut TestAppContext,
    spaces: Vec<SpaceEntry>,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let first = spaces[0].0;
        shell.set_spaces(spaces, cx);
        shell.set_space(first, cx);
        shell.set_tabs(
            first,
            vec![10],
            vec![11],
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

/// Where the first pinned row is under the list's top.
fn first_row(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.read_with(cx, |shell, _| shell.layout()[&Slot::Tab(10)])
}

/// The titles of the menu a right-click under the Space's rows shows.
fn empty_space_menu(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
    host: &Recorder,
) -> Vec<String> {
    let y = shell.read_with(cx, |shell, _| {
        let last = shell.layout().into_values().fold(0., f32::max);
        shell.drawn_list_top() + last + ROW_PITCH + 40.
    });
    host.menus.borrow_mut().clear();
    cx.simulate_mouse_down(
        point(px(100.), px(y)),
        MouseButton::Right,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(y)),
        MouseButton::Right,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let menus = host.menus.borrow();
    let Some(menu) = menus.last() else {
        return Vec::new();
    };
    menu.iter().map(|(title, _)| title.to_string()).collect()
}

/// With the browser's only Space named "Space 1" and with no icon, the name row hides and the rows
/// rise as Arc's do, 39 under the URL field. Show Space Header, after Edit Theme Color… in the menu
/// of the sidebar's empty space, brings it back in every window and keeps it for the next launch,
/// and the item then says Hide Space Header, which hides it again.
#[gpui::test]
fn a_lone_unnamed_space_hides_its_name_row_until_show_space_header(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![space(1, "Space 1", "")]);
    assert!(
        shell.read_with(cx, |shell, _| shell.name_row_hidden()),
        "the name row shows"
    );
    assert_eq!(
        first_row(&shell, cx),
        NAME_HEIGHT - 39.,
        "the rows didn't rise as Arc's do"
    );

    let menu = empty_space_menu(&shell, cx, &host);
    let theme = menu
        .iter()
        .position(|title| title == "Edit Theme Color…")
        .expect("no Edit Theme");
    assert_eq!(menu[theme + 1], "Show Space Header", "{menu:?}");
    cx.update(|_, cx| host.choose("Show Space Header", cx));
    cx.run_until_parked();
    assert_eq!(host.lone_header.get(), Some(true), "the choice isn't kept");
    assert_eq!(
        first_row(&shell, cx),
        NAME_HEIGHT,
        "the name row didn't come back"
    );

    let menu = empty_space_menu(&shell, cx, &host);
    assert!(
        menu.iter().any(|title| title == "Hide Space Header"),
        "{menu:?}"
    );
    cx.update(|_, cx| host.choose("Hide Space Header", cx));
    cx.run_until_parked();
    assert_eq!(host.lone_header.get(), Some(false));
    assert_eq!(
        first_row(&shell, cx),
        NAME_HEIGHT - 39.,
        "the name row didn't hide again"
    );
}

/// Show Space Header and View › Appearance as they came from the account (settings_synced) redraw
/// the window: the name row comes back, the window takes the look, the menu offers Hide Space
/// Header, and neither choice is kept again, as the core has them.
#[gpui::test]
fn the_accounts_space_header_and_appearance_redraw_the_window(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![space(1, "Space 1", "")]);
    shell.update(cx, |shell, cx| {
        shell.look_followed(Appearance::Light, true, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        first_row(&shell, cx),
        NAME_HEIGHT,
        "the name row didn't come back"
    );
    assert_eq!(
        host.window_light.get(),
        Some(true),
        "the window isn't light"
    );
    shell.update(cx, |shell, cx| {
        shell.look_followed(Appearance::Dark, true, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        host.window_light.get(),
        Some(false),
        "the window isn't dark"
    );
    let menu = empty_space_menu(&shell, cx, &host);
    assert!(
        menu.iter().any(|title| title == "Hide Space Header"),
        "{menu:?}"
    );
    assert_eq!(
        host.lone_header.get(),
        None,
        "the header's choice was kept again"
    );
    assert_eq!(host.appearance.get(), None, "the appearance was kept again");

    shell.update(cx, |shell, cx| {
        shell.look_followed(Appearance::Dark, false, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        first_row(&shell, cx),
        NAME_HEIGHT - 39.,
        "the name row didn't hide again"
    );
}

/// Under the favorites the rows rise 36, as Arc's do.
#[gpui::test]
fn under_the_favorites_the_rows_rise_less(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx, vec![space(1, "Space 1", "")]);
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(30, 1)], cx));
    cx.run_until_parked();
    assert_eq!(first_row(&shell, cx), NAME_HEIGHT - 36.);
}

/// A Space the user named or gave an icon, or one of two, keeps its name row, and its menu has no
/// Show Space Header.
#[gpui::test]
fn a_named_space_or_one_of_two_keeps_its_name_row(cx: &mut TestAppContext) {
    let cases = [
        vec![space(1, "Work", "")],
        vec![space(1, "Space 1", "🌿")],
        vec![space(1, "Space 1", ""), space(2, "Space 2", "")],
    ];
    for spaces in cases {
        let what = format!("{spaces:?}");
        let (shell, cx, host) = window(cx, spaces);
        assert_eq!(
            first_row(&shell, cx),
            NAME_HEIGHT,
            "{what} hid its name row"
        );
        let menu = empty_space_menu(&shell, cx, &host);
        assert!(
            !menu.iter().any(|title| title.ends_with("Space Header")),
            "{what}: {menu:?}"
        );
    }
}

/// Rename Space… shows the name row for its field, and cancelled, the row hides again.
#[gpui::test]
fn renaming_shows_the_name_row_for_its_field(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx, vec![space(1, "Space 1", "")]);
    shell.update(cx, |shell, cx| shell.rename_space(1, cx));
    cx.run_until_parked();
    assert_eq!(first_row(&shell, cx), NAME_HEIGHT, "the field has no row");
    shell.update_in(cx, |shell, window, cx| shell.cancel_name(window, cx));
    cx.run_until_parked();
    assert_eq!(first_row(&shell, cx), NAME_HEIGHT - 39.);
}

struct Themes;

impl theme_picker::Host for Themes {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}

    fn hide(&self) {}

    fn detent(&self) {}
}

/// Edit Theme Color… opens the theme picker against the sidebar's edge with the name row still
/// hidden, as what's left of the row, which the picker waits for to settle, is drawn.
#[gpui::test]
fn edit_theme_color_opens_with_the_name_row_hidden(cx: &mut TestAppContext) {
    cx.update(|cx| theme_picker::open(Rc::new(Themes), cx));
    let (shell, cx, _) = window(cx, vec![space(1, "Space 1", "")]);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.edit_space_theme(1, cx));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    assert_eq!(
        cx.update(|_, cx| theme_picker::open_for(cx)),
        Some(1),
        "the picker didn't open"
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.name_row_hidden()),
        "the name row showed for it"
    );
}

#[test]
fn only_the_names_ilo_gives_count_as_none() {
    assert!(given_name("Space 1") && given_name("Space 12"));
    for name in ["Space", "Space ", "Space 1a", "Work", "", "space 1"] {
        assert!(!given_name(name), "{name:?}");
    }
}
