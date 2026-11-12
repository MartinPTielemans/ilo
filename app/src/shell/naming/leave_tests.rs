//! A name field ends once it loses the keyboard, however it goes (owner, 2026-11-02, #1414): a
//! press elsewhere in the window, the page taking it, or an overlay, another window or another app.
//! What's typed is kept, as ↩ keeps it, an empty field changes nothing, and Esc still cancels. A
//! pinned URL's field is undone instead.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext, Empty, Entity, EntityInputHandler, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, Point, TestAppContext, VisualTestContext, WindowOptions, point, px,
};

use crate::{
    menu::Closed,
    shell::{
        Folders, NAME_HEIGHT, Naming, ROW_HEIGHT, Shell, Slot, commands::bind_keys, fields_opened,
        recorder::Recorder,
    },
    text_input::{self, Chosen, FieldMenu, korean},
};

/// How the field loses the keyboard.
#[derive(Clone, Copy, Debug)]
enum Way {
    /// A press on another row.
    AnotherRow,
    /// The page takes it, which the host says: a press in it on the Mac and on Wayland, its window
    /// taking the focus on Windows.
    Page,
    /// The window loses it, to an overlay such as the command bar, another window or another app.
    Window,
}

const WAYS: [Way; 3] = [Way::AnotherRow, Way::Page, Way::Window];

/// An active window showing the pinned tab 2 and the tabs 3 and 4, 3 selected.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        bind_keys(cx);
        text_input::bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, vec![2], vec![3, 4], Vec::new(), Folders::default(), cx);
        shell.set_selected(3, cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// The middle of `tab`'s row.
fn row(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let top = shell.drawn_list_top() + shell.layout()[&Slot::Tab(tab)];
        point(px(shell.width() / 2.), px(top + ROW_HEIGHT / 2.))
    })
}

fn press(cx: &mut VisualTestContext, position: Point<Pixels>, button: MouseButton, clicks: usize) {
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position,
        button,
        modifiers,
        click_count: clicks,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button,
        modifiers,
        click_count: clicks,
    });
    cx.run_until_parked();
}

fn naming(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Naming> {
    shell.read_with(cx, |shell, _| {
        shell.renaming.as_ref().map(|renaming| renaming.naming)
    })
}

/// Double-clicks `tab`'s row, as the user renames a tab, its title all selected in the field.
fn rename(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder, tab: u64) {
    cx.update(|window, _| window.activate_window());
    let at = row(shell, cx, tab);
    press(cx, at, MouseButton::Left, 1);
    press(cx, at, MouseButton::Left, 2);
    assert!(
        naming(shell, cx) == Some(Naming::Tab(tab)),
        "the double-click didn't rename"
    );
    host.take();
}

fn leave(shell: &Entity<Shell>, cx: &mut VisualTestContext, way: Way) {
    match way {
        Way::AnotherRow => {
            let at = row(shell, cx, 4);
            press(cx, at, MouseButton::Left, 1);
        }
        Way::Page => shell.update_in(cx, |shell, window, cx| {
            shell.keyboard_left(fields_opened(), window, cx)
        }),
        Way::Window => cx.deactivate_window(),
    }
    cx.run_until_parked();
}

/// The host says the page took the keyboard as the press in it comes, and the field hears it after
/// what's queued behind the press: a double-click's second press may have opened a field since. That
/// field stays, and a press in the page after it opened still ends it (#2260).
#[gpui::test]
fn a_field_opened_after_a_press_in_the_page_stays(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let pressed = fields_opened();
    rename(&shell, cx, &host, 3);
    shell.update_in(cx, |shell, window, cx| {
        shell.keyboard_left(pressed, window, cx)
    });
    cx.run_until_parked();
    assert!(
        naming(&shell, cx) == Some(Naming::Tab(3)),
        "a press in the page before the field opened ended it"
    );
    cx.simulate_input("Work");
    leave(&shell, cx, Way::Page);
    assert!(
        naming(&shell, cx).is_none(),
        "a press in the page left the field open"
    );
    assert_eq!(host.take_edits(), ["rename tab 3 Work"]);
}

#[gpui::test]
fn a_tab_name_is_kept_however_the_field_loses_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    for way in WAYS {
        let typed = format!("Work {way:?}");
        rename(&shell, cx, &host, 3);
        cx.simulate_input(&typed);
        leave(&shell, cx, way);
        assert!(naming(&shell, cx).is_none(), "{way:?} left the field open");
        assert_eq!(
            host.take_edits(),
            [format!("rename tab 3 {typed}")],
            "{way:?}"
        );
        // The page that took the keyboard keeps it.
        if let Way::Page = way {
            assert!(
                !host.take().iter().any(|call| call == "focus ui"),
                "the UI took it back"
            );
        }
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));

        rename(&shell, cx, &host, 3);
        cx.simulate_keystrokes("backspace");
        leave(&shell, cx, way);
        assert!(
            naming(&shell, cx).is_none(),
            "{way:?} left the empty field open"
        );
        assert!(host.take_edits().is_empty(), "{way:?} kept an empty name");
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));

        rename(&shell, cx, &host, 3);
        cx.simulate_input("Gone");
        cx.simulate_keystrokes("escape");
        assert!(naming(&shell, cx).is_none(), "escape left the field open");
        leave(&shell, cx, way);
        assert!(
            host.take_edits().is_empty(),
            "{way:?} kept a name escape cancelled"
        );
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));
    }
}

/// A name being composed with an input method is kept whole, its marked text as typed, with nothing
/// left composing.
#[gpui::test]
fn a_name_being_composed_is_kept_whole(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    for (way, typed) in WAYS.into_iter().zip(["か", "き", "く"]) {
        rename(&shell, cx, &host, 3);
        let field = shell.read_with(cx, |shell, _| {
            shell.renaming.as_ref().map(|r| r.name.clone())
        });
        field.expect("no field").update_in(cx, |input, window, cx| {
            input.replace_and_mark_text_in_range(None, typed, None, window, cx)
        });
        leave(&shell, cx, way);
        assert!(naming(&shell, cx).is_none(), "{way:?} left the field open");
        assert_eq!(
            host.take_edits(),
            [format!("rename tab 3 {typed}")],
            "{way:?}"
        );
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));
    }
}

/// A name typed in Korean from a 2-set keyboard, where one key can commit a syllable and start the
/// next, is kept whole (#2457): at ↩, which the input method passes on once it's committed the last
/// syllable, and when the field loses the keyboard with that syllable still composing. Each way
/// types a name of its own, as a name the tab already has changes nothing.
#[gpui::test]
fn a_name_typed_in_korean_is_kept_whole(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let both = [
        korean::HANGEUL,
        &[korean::Ime::Commit("글")],
        korean::ANNYEONG,
    ]
    .concat();
    for (leaving, calls, name) in [
        (None, korean::ANNYEONG, "안녕하세요"),
        (Some(Way::Page), korean::HANGEUL, "한글"),
        (Some(Way::AnotherRow), &both[..], "한글안녕하세요"),
    ] {
        rename(&shell, cx, &host, 3);
        let field = shell.read_with(cx, |shell, _| {
            shell.renaming.as_ref().map(|r| r.name.clone())
        });
        let field = field.expect("no field");
        for call in calls {
            field.update_in(cx, |input, window, cx| call.apply(input, window, cx));
            cx.run_until_parked();
        }
        match leaving {
            None => {
                field.update_in(cx, |input, window, cx| {
                    input.replace_text_in_range(None, "요", window, cx)
                });
                cx.simulate_keystrokes("enter");
                cx.run_until_parked();
            }
            Some(way) => leave(&shell, cx, way),
        }
        assert!(
            naming(&shell, cx).is_none(),
            "{leaving:?} left the field open"
        );
        assert_eq!(
            host.take_edits(),
            [format!("rename tab 3 {name}")],
            "{leaving:?}"
        );
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));
    }
}

/// The Space's name, in its own row atop the list, ends at a press on a row as a tab's does.
#[gpui::test]
fn a_space_name_is_kept_at_a_press_on_a_row(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.rename_space(1, cx));
    cx.run_until_parked();
    cx.simulate_input("Work");
    let at = row(&shell, cx, 4);
    press(cx, at, MouseButton::Left, 1);
    assert!(naming(&shell, cx).is_none(), "the field stayed");
    assert_eq!(host.take_edits(), ["rename space 1 Work"]);
}

/// Rename Space, chosen in the Space's menu, opens the name's field, which stays open as the menu's
/// window gives the browser window the keyboard back. GPUI hears of each key window change later,
/// in order, so the change from the menu taking the keyboard can arrive once the field is open,
/// with the one giving it back right behind it, as when the choice comes before GPUI has heard of
/// the menu showing (#1589). A window still inactive once both have arrived ends the field.
#[gpui::test]
fn a_name_field_opened_from_a_menu_outlasts_the_menu_giving_back_the_keyboard(
    cx: &mut TestAppContext,
) {
    let (shell, cx, host) = window(cx);
    let menu = cx.update(|_, cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
    });
    let name = shell.read_with(cx, |shell, _| {
        point(px(60.), px(shell.drawn_list_top() + NAME_HEIGHT / 2.))
    });
    for user_left in [false, true] {
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        press(cx, name, MouseButton::Right, 1);
        // The menu's window takes the keyboard, and the user chooses before GPUI hears of it.
        menu.update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.update(|_, cx| host.choose("Rename Space…", cx));
        assert!(
            naming(&shell, cx) == Some(Naming::Space(1)),
            "Rename Space opened no field"
        );
        // Closed, the menu gives the browser window the keyboard back, unless the user went on to
        // another window.
        if !user_left {
            cx.update(|window, _| window.activate_window());
        }
        cx.run_until_parked();
        if user_left {
            assert!(
                naming(&shell, cx).is_none(),
                "going to another window left the field open"
            );
        } else {
            assert!(
                naming(&shell, cx) == Some(Naming::Space(1)),
                "the menu closing ended the field"
            );
            cx.simulate_input("Work");
            cx.simulate_keystrokes("enter");
            assert_eq!(host.take_edits(), ["rename space 1 Work"]);
        }
    }
}

/// The pointer over the Space's name, where the menu's window leaves it as it closes, lights the
/// row but doesn't take the keyboard from the name's field open in it: Esc still cancels (#1664).
/// The hover comes as the browser window gets the keyboard back, before GPUI has heard of it, and
/// after.
#[gpui::test]
fn a_hover_over_the_space_name_leaves_its_field_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let menu = cx.update(|_, cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
    });
    let row = shell
        .read_with(cx, |shell, _| shell.name_row_drawn.get())
        .expect("a name row");
    let name = point(row.left() + px(60.), row.center().y);
    let away = point(px(60.), px(400.));
    for hover_first in [false, true] {
        cx.update(|window, _| window.activate_window());
        cx.simulate_mouse_move(away, None, Modifiers::none());
        cx.run_until_parked();
        press(cx, name, MouseButton::Right, 1);
        menu.update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        cx.update(|_, cx| host.choose("Rename Space…", cx));
        cx.run_until_parked();
        assert!(
            naming(&shell, cx) == Some(Naming::Space(1)),
            "Rename Space opened no field"
        );
        cx.update(|window, _| window.activate_window());
        // AppKit's moves as the menu's window goes, a point on from the press.
        if hover_first {
            cx.simulate_mouse_move(name + point(px(5.), px(0.)), None, Modifiers::none());
        }
        cx.run_until_parked();
        cx.simulate_mouse_move(name + point(px(10.), px(0.)), None, Modifiers::none());
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.name_hovered),
            "the pointer didn't reach the Space's name ({hover_first})"
        );
        assert!(
            naming(&shell, cx) == Some(Naming::Space(1)),
            "a hover over the Space's name ended its field (before the window was heard back: \
             {hover_first})"
        );
        cx.simulate_input("gone");
        cx.simulate_keystrokes("escape");
        assert!(naming(&shell, cx).is_none(), "Esc didn't close the field");
        assert_eq!(
            host.take_edits(),
            Vec::<String>::new(),
            "Esc renamed the Space"
        );
    }
}

/// A pinned tab's Edit… field takes a URL only on purpose: losing the keyboard undoes it.
#[gpui::test]
fn a_pinned_url_typed_is_undone_however_the_field_loses_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    host.pinned_urls
        .borrow_mut()
        .insert(2, "https://old.test/".into());
    for way in WAYS {
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| shell.edit_pinned_url(2, cx));
        cx.run_until_parked();
        assert!(naming(&shell, cx) == Some(Naming::PinnedUrl(2)));
        cx.simulate_input("new.test");
        leave(&shell, cx, way);
        assert!(naming(&shell, cx).is_none(), "{way:?} left the field open");
        assert!(host.take_edits().is_empty(), "{way:?} kept the URL");
    }
}

/// Shows the field's menus as ilo's menu does, handing over each one's answer for the test to give.
fn field_menus(cx: &mut VisualTestContext) -> Rc<RefCell<Option<Chosen>>> {
    let shown: Rc<RefCell<Option<Chosen>>> = Rc::default();
    let menu = shown.clone();
    cx.update(|_, cx| {
        cx.set_global(FieldMenu(Rc::new(move |_, _, _, _, chosen, _| {
            *menu.borrow_mut() = Some(chosen);
        })))
    });
    shown
}

/// The field's own menu takes the window's keyboard while it shows, and gives it back to the
/// field, which is still there for Esc to cancel.
#[gpui::test]
fn the_fields_own_menu_leaves_it_open(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let shown = field_menus(cx);
    rename(&shell, cx, &host, 3);
    cx.simulate_input("Gone");
    let at = row(&shell, cx, 3);
    press(cx, at, MouseButton::Right, 1);
    let chosen = shown
        .take()
        .expect("a right-click in the field showed no menu");
    for way in [Way::Page, Way::Window] {
        leave(&shell, cx, way);
        assert!(
            naming(&shell, cx) == Some(Naming::Tab(3)),
            "the menu's {way:?} closed the field"
        );
    }
    cx.update(|window, cx| chosen(Closed::Otherwise, window, cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        naming(&shell, cx) == Some(Naming::Tab(3)),
        "the menu closing closed the field"
    );
    cx.simulate_keystrokes("escape");
    assert!(naming(&shell, cx).is_none(), "escape didn't cancel");
    assert!(host.take_edits().is_empty(), "the tab was renamed");
    // Closed, the menu no longer holds the field open.
    rename(&shell, cx, &host, 3);
    cx.simulate_input("Work");
    leave(&shell, cx, Way::Page);
    assert_eq!(host.take_edits(), ["rename tab 3 Work"]);
}

/// The press that closes the field's menu goes no further, as a click outside any menu doesn't, but
/// one away from the field ends it as any other click away does, a click in the page as on
/// Windows, keeping what's typed. One on the field leaves it open, and so does Esc in the menu.
#[gpui::test]
fn a_press_away_that_closes_the_fields_menu_ends_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let shown = field_menus(cx);
    let page = shell.read_with(cx, |shell, _| point(px(shell.width() + 200.), px(300.)));
    rename(&shell, cx, &host, 3);
    cx.simulate_input("Work");
    let field = row(&shell, cx, 3);
    for closed in [Closed::PressedAt(field), Closed::Otherwise] {
        press(cx, field, MouseButton::Right, 1);
        let chosen = shown
            .take()
            .expect("a right-click in the field showed no menu");
        cx.update(|window, cx| chosen(closed, window, cx));
        cx.run_until_parked();
        assert!(
            naming(&shell, cx) == Some(Naming::Tab(3)),
            "{closed:?} closed the field"
        );
    }
    press(cx, field, MouseButton::Right, 1);
    let chosen = shown
        .take()
        .expect("a right-click in the field showed no menu");
    cx.update(|window, cx| chosen(Closed::PressedAt(page), window, cx));
    cx.run_until_parked();
    assert!(
        naming(&shell, cx).is_none(),
        "a press in the page left the field open"
    );
    assert_eq!(host.take_edits(), ["rename tab 3 Work"]);
    shell.update(cx, |shell, cx| shell.set_selected(3, cx));

    // The user going to another window or app, with the menu up, ends it too.
    rename(&shell, cx, &host, 3);
    cx.simulate_input("Home");
    press(cx, field, MouseButton::Right, 1);
    let chosen = shown
        .take()
        .expect("a right-click in the field showed no menu");
    cx.deactivate_window();
    assert!(
        naming(&shell, cx) == Some(Naming::Tab(3)),
        "the menu taking the keyboard closed it"
    );
    cx.update(|window, cx| chosen(Closed::Left, window, cx));
    cx.run_until_parked();
    assert!(
        naming(&shell, cx).is_none(),
        "leaving with the menu up left the field open"
    );
    assert_eq!(host.take_edits(), ["rename tab 3 Home"]);
}
