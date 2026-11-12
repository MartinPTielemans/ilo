//! A split view's own name and icon, as Arc's split menu gives one (#1398): Rename… and Change
//! Icon… on any of its pages name the split, its row then shows one plate, and what moves or
//! copies the split takes them along.

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

use crate::{
    icon_picker::Subject,
    shell::{
        Command, Folders, Naming, Pending, ROW_HEIGHT, Shell, Slot, Split,
        commands::bind_keys,
        fields_opened,
        recorder::Recorder,
        undo::{Prop, Value},
    },
};

/// An active window showing the Today tabs 3, 4 and 5, 3 and 4 a split named `name`, 3 shown.
fn window<'a>(
    cx: &'a mut TestAppContext,
    name: &str,
) -> (Entity<Shell>, &'a mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_selected(3, cx);
    });
    report(&shell, cx, name, "");
    cx.update(|window, _| window.activate_window());
    host.take();
    (shell, cx, host)
}

/// The core reports the split with `name` and `icon`.
fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, name: &str, icon: &str) {
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![3, 4],
            name: name.to_string().into(),
            icon: icon.to_string().into(),
            ..Split::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![3, 4, 5],
            vec![split],
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
}

/// The middle of the second page's segment in the split's row, folded at its end while the first
/// shows.
fn second_segment(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let top = shell.drawn_list_top() + shell.layout()[&Slot::Tab(3)];
        point(px(shell.segment_middle(4)), px(top + ROW_HEIGHT / 2.))
    })
}

fn press(cx: &mut VisualTestContext, position: Point<Pixels>, clicks: usize) {
    let (button, modifiers) = (MouseButton::Left, Modifiers::none());
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

/// The text in the field naming `tab`'s split, if one is.
fn field(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) -> Option<String> {
    shell.read_with(cx, |shell, cx| {
        let renaming = shell
            .renaming
            .as_ref()
            .filter(|r| r.naming == Naming::Split(tab))?;
        Some(renaming.name.read(cx).text().to_string())
    })
}

fn naming(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.renaming.is_some())
}

/// A double-click on either page of a split renames the split, by its first tab, in a field that
/// starts empty. Named, its row is one plate: a click anywhere on it shows the page shown last,
/// where the second page's segment was. ⌘Z takes the name off, and the pages' segments come back.
#[gpui::test]
fn a_double_click_on_a_split_names_the_split(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, "");
    // The second page's segment unfolds at once, so it's where the next press finds it.
    cx.update(|_, cx| cx.set_reduce_motion(true));
    let second = second_segment(&shell, cx);
    press(cx, second, 1);
    assert!(
        host.take().contains(&"select 4".to_string()),
        "the folded segment is the second page"
    );
    shell.update(cx, |shell, cx| shell.set_selected(4, cx));
    cx.run_until_parked();
    let second = second_segment(&shell, cx);
    press(cx, second, 2);
    assert_eq!(field(&shell, cx, 3).as_deref(), Some(""));
    cx.simulate_input("Planning");
    cx.simulate_keystrokes("enter");
    assert!(!naming(&shell, cx), "the field stayed");
    assert_eq!(host.take_edits(), ["split name 3 Planning"]);

    report(&shell, cx, "Planning", "");
    shell.update(cx, |shell, cx| shell.set_selected(5, cx));
    host.take();
    press(cx, second, 1);
    let calls = host.take();
    assert!(
        calls.contains(&"select 4".to_string()),
        "the page shown last: {calls:?}"
    );
    shell.update(cx, |shell, cx| shell.set_selected(3, cx));
    shell.update(cx, |shell, cx| shell.set_selected(5, cx));
    host.take();
    press(cx, second, 1);
    assert!(
        host.take().contains(&"select 3".to_string()),
        "the row's one plate"
    );
    press(cx, second, 2);
    assert_eq!(field(&shell, cx, 3).as_deref(), Some("Planning"));
    cx.simulate_keystrokes("escape");
    assert!(host.take_edits().is_empty(), "escape named it");

    shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
    assert_eq!(host.take_edits(), ["split name 3 "]);
    shell.update(cx, |shell, cx| shell.run(Command::Redo, "ui", cx));
    assert_eq!(host.take_edits(), ["split name 3 Planning"]);
}

/// ↩ on an emptied field takes the name off, back to the pages' titles, but a field that loses
/// the keyboard empty changes nothing, as a tab's doesn't.
#[gpui::test]
fn an_empty_name_is_taken_off_only_by_return(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, "Planning");
    shell.update(cx, |shell, cx| shell.rename_tab(4, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("backspace");
    shell.update_in(cx, |shell, window, cx| {
        shell.keyboard_left(fields_opened(), window, cx)
    });
    cx.run_until_parked();
    assert!(!naming(&shell, cx), "the field stayed");
    assert!(
        host.take_edits().is_empty(),
        "leaving it empty took the name off"
    );

    shell.update(cx, |shell, cx| shell.rename_tab(4, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("backspace enter");
    assert_eq!(host.take_edits(), ["split name 3 "]);
}

/// Change Icon… on a split's second page hangs the picker at the split's row, from its first tab,
/// and what's picked is the split's icon, which ⌘Z takes off.
#[gpui::test]
fn change_icon_on_a_split_gives_the_split_its_icon(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, "Planning");
    shell.update(cx, |shell, cx| shell.change_icon(4, cx));
    cx.run_until_parked();
    let (pending, row) = shell.read_with(cx, |shell, _| {
        (
            shell.picker_pending.map(|(picker, _)| picker),
            shell.tab_row_drawn.get(),
        )
    });
    assert_eq!(pending, Some(Pending::Icon(Subject::Tab(3))));
    assert!(row.is_some(), "the split's row didn't say where it is");

    // What the picker gives back for what's picked.
    shell.update(cx, |shell, cx| {
        shell.change(Prop::SplitIcon(3), Value::Text("📅".into()), cx)
    });
    assert_eq!(host.take_edits(), ["split icon 3 📅"]);
    report(&shell, cx, "Planning", "📅");
    shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
    assert_eq!(host.take_edits(), ["split icon 3 "]);
}

/// Duplicate copies each page and joins the copies with the split's name and icon; separating a
/// split of two takes them off with it, and ⌘Z joins it back with both.
#[gpui::test]
fn a_splits_look_goes_with_its_copy_and_comes_back_with_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, "Planning");
    report(&shell, cx, "Planning", "📅");
    shell.update(cx, |shell, cx| shell.duplicate_row(4, cx));
    let (copy, other) = (1001, 1002);
    assert_eq!(
        host.take_edits(),
        [
            format!("split {other} {copy} false"),
            format!("split name {copy} Planning"),
            format!("split icon {copy} 📅"),
        ]
    );

    shell.update(cx, |shell, cx| shell.separate_tab(4, cx));
    assert_eq!(host.take_edits(), ["separate 4"]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![3, 4, 5],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
    assert_eq!(
        host.take_edits(),
        [
            "split 4 3 false",
            "split name 3 Planning",
            "split icon 3 📅"
        ]
    );
}
