//! Settings' profile name field ends as the sidebar's do once it loses the keyboard (owner,
//! 2026-11-02, #1414), keeping what's typed, and Esc still cancels. Settings is the app's, not the
//! library's, so this is its Profiles tab as the field sees it: the root takes the keyboard at a
//! press on nothing in particular, the toolbar's tabs stop their presses, and the name is kept
//! through `on_lose_keyboard`, as Settings keeps it.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    KeyBinding, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
    VisualTestContext,
};

use super::*;
use crate::shell::{CancelName, KeepName};

const TOOLBAR_TAB: Point<Pixels> = point(px(100.), px(20.));
const FIELD: Point<Pixels> = point(px(100.), px(55.));
const NOTHING: Point<Pixels> = point(px(100.), px(300.));

/// The Profiles tab, with the names the user kept.
struct Profiles {
    focus: FocusHandle,
    renaming: Option<(Entity<TextInput>, Vec<Subscription>)>,
    kept: Vec<String>,
}

impl Profiles {
    /// As a double-click on a profile's name does, its name all selected in the field.
    fn rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| TextInput::new("Profile name", cx).with_text("Personal".into()));
        window.focus(&field.focus_handle(cx), cx);
        let watches = on_lose_keyboard(&field, window, cx, Self::keep);
        self.renaming = Some((field, watches));
        cx.notify();
    }

    /// Names the profile what the field holds, unless it's empty, as Settings' `keep_name`.
    fn keep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((field, _)) = self.renaming.take() else {
            return;
        };
        let name = field.read(cx).text().trim().to_string();
        if !name.is_empty() {
            self.kept.push(name);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }
}

impl Render for Profiles {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field = self.renaming.as_ref().map(|(field, _)| field.clone());
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .w_full()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
            )
            .child(
                div()
                    .h(px(30.))
                    .w(px(200.))
                    .key_context("Naming")
                    .on_action(
                        cx.listener(|profiles, _: &KeepName, window, cx| profiles.keep(window, cx)),
                    )
                    .on_action(cx.listener(|profiles, _: &CancelName, window, cx| {
                        profiles.cancel(window, cx)
                    }))
                    .children(field),
            )
    }
}

fn settings(cx: &mut TestAppContext) -> (Entity<Profiles>, &mut VisualTestContext) {
    cx.update(|cx| {
        bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new("enter", KeepName, Some("Naming")),
            KeyBinding::new("escape", CancelName, Some("Naming")),
        ]);
    });
    let (profiles, cx) = cx.add_window_view(|_, cx| Profiles {
        focus: cx.focus_handle(),
        renaming: None,
        kept: Vec::new(),
    });
    cx.run_until_parked();
    (profiles, cx)
}

fn press(cx: &mut VisualTestContext, position: Point<Pixels>, button: MouseButton) {
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position,
        button,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button,
        modifiers,
        click_count: 1,
    });
    cx.run_until_parked();
}

fn rename(profiles: &Entity<Profiles>, cx: &mut VisualTestContext) {
    cx.update(|window, _| window.activate_window());
    profiles.update_in(cx, |profiles, window, cx| profiles.rename(window, cx));
    cx.run_until_parked();
}

fn renaming(profiles: &Entity<Profiles>, cx: &mut VisualTestContext) -> bool {
    profiles.read_with(cx, |profiles, _| profiles.renaming.is_some())
}

fn kept(profiles: &Entity<Profiles>, cx: &mut VisualTestContext) -> Vec<String> {
    profiles.update(cx, |profiles, _| std::mem::take(&mut profiles.kept))
}

#[derive(Clone, Copy, Debug)]
enum Way {
    /// A press on a toolbar tab, which stops it.
    ToolbarTab,
    /// A press on nothing, which the root takes the keyboard at.
    Nothing,
    /// To another window or app.
    Window,
}

#[gpui::test]
fn a_profile_name_is_kept_however_the_field_loses_the_keyboard(cx: &mut TestAppContext) {
    let (profiles, cx) = settings(cx);
    for way in [Way::ToolbarTab, Way::Nothing, Way::Window] {
        let leave = |cx: &mut VisualTestContext| match way {
            Way::ToolbarTab => press(cx, TOOLBAR_TAB, MouseButton::Left),
            Way::Nothing => press(cx, NOTHING, MouseButton::Left),
            Way::Window => cx.deactivate_window(),
        };
        let typed = format!("Work {way:?}");
        rename(&profiles, cx);
        cx.simulate_input(&typed);
        leave(cx);
        assert!(!renaming(&profiles, cx), "{way:?} left the field open");
        assert_eq!(kept(&profiles, cx), [typed], "{way:?}");

        rename(&profiles, cx);
        cx.simulate_keystrokes("backspace");
        leave(cx);
        assert!(
            !renaming(&profiles, cx),
            "{way:?} left the empty field open"
        );
        assert!(kept(&profiles, cx).is_empty(), "{way:?} kept an empty name");

        rename(&profiles, cx);
        cx.simulate_input("Gone");
        cx.simulate_keystrokes("escape");
        assert!(!renaming(&profiles, cx), "escape left the field open");
        leave(cx);
        assert!(
            kept(&profiles, cx).is_empty(),
            "{way:?} kept a name escape cancelled"
        );
    }
}

/// The field's own menu has the window's keyboard while it shows. Closed by a press away from the
/// field, which it takes, or by the user going to another window or app, it ends the field, keeping
/// the name; closed by a press on the field or by its keys, it leaves it open.
#[gpui::test]
fn a_profile_name_is_kept_as_its_menu_closes_away_from_it(cx: &mut TestAppContext) {
    let (profiles, cx) = settings(cx);
    let shown: Rc<RefCell<Option<Chosen>>> = Rc::default();
    let menu = shown.clone();
    cx.update(|_, cx| {
        cx.set_global(FieldMenu(Rc::new(move |_, _, _, _, chosen, _| {
            *menu.borrow_mut() = Some(chosen);
        })))
    });
    let show_menu = |cx: &mut VisualTestContext| {
        press(cx, FIELD, MouseButton::Right);
        shown
            .take()
            .expect("a right-click in the field showed no menu")
    };
    for (closed, ends) in [
        (Closed::PressedAt(FIELD), false),
        (Closed::Otherwise, false),
        (Closed::PressedAt(TOOLBAR_TAB), true),
        (Closed::PressedAt(NOTHING), true),
        (Closed::Left, true),
    ] {
        rename(&profiles, cx);
        cx.simulate_input("Work");
        let chosen = show_menu(cx);
        cx.deactivate_window();
        assert!(
            renaming(&profiles, cx),
            "the menu taking the keyboard closed the field"
        );
        cx.update(|window, cx| chosen(closed, window, cx));
        cx.run_until_parked();
        assert_eq!(!renaming(&profiles, cx), ends, "{closed:?}");
        if ends {
            assert_eq!(kept(&profiles, cx), ["Work"], "{closed:?}");
        } else {
            cx.simulate_keystrokes("escape");
            assert!(kept(&profiles, cx).is_empty(), "{closed:?}");
        }
    }
}

/// GPUI can hear the window lost the keyboard once the window has it back, as when a menu's
/// window hands it back as its choice opens the field. The field asks the platform, and stays open
/// while the window has the keyboard (#2019); once it's gone, the name is kept.
#[gpui::test]
fn a_profile_name_stays_open_while_its_window_has_the_keyboard(cx: &mut TestAppContext) {
    let (profiles, cx) = settings(cx);
    set_window_has_keyboard(|_| true);
    rename(&profiles, cx);
    cx.simulate_input("Work");
    cx.deactivate_window();
    assert!(
        renaming(&profiles, cx),
        "GPUI's late word ended the field while its window had the keyboard"
    );
    set_window_has_keyboard(|_| false);
    cx.update(|window, _| window.activate_window());
    cx.deactivate_window();
    assert!(
        !renaming(&profiles, cx),
        "leaving the window left the field open"
    );
    assert_eq!(kept(&profiles, cx), ["Work"]);
}
