//! A double-click on a folder's row opens or closes it at its first press and gives its name a
//! field at its second, wherever the keyboard was before (#3329).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

use crate::shell::{Folders, Naming, ROW_HEIGHT, Shell, Slot, fields_opened, recorder::Recorder};

const FOLDER: u64 = 20;

fn folders(open: bool) -> Folders {
    Folders {
        folders: [(FOLDER, ("Videos".into(), open))].into(),
        parents: [(12, FOLDER)].into(),
        ..Default::default()
    }
}

/// An active window with a closed folder pinned, its tab 12 in it, and the Today tab 10 selected.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            vec![FOLDER, 12],
            vec![10],
            Vec::new(),
            folders(false),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    cx.update(|window, _| window.activate_window());
    (shell, cx, host)
}

fn row(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let top = shell.drawn_list_top() + shell.layout()[&Slot::Tab(tab)];
        point(px(shell.width() / 2.), px(top + ROW_HEIGHT / 2.))
    })
}

fn click(cx: &mut VisualTestContext, position: Point<Pixels>, clicks: usize) {
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
}

fn naming(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Naming> {
    shell.read_with(cx, |shell, _| {
        shell.renaming.as_ref().map(|renaming| renaming.naming)
    })
}

/// The core reports the folder opened, as it does after the first press.
fn opened(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![FOLDER, 12], vec![10], Vec::new(), folders(true), cx)
    });
}

#[gpui::test]
fn a_double_click_on_a_folder_opens_it_then_names_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let at = row(&shell, cx, FOLDER);
    click(cx, at, 1);
    cx.run_until_parked();
    assert!(
        host.take().contains(&format!("folder open {FOLDER} true")),
        "the first press didn't open the folder"
    );
    opened(&shell, cx);
    cx.run_until_parked();
    click(cx, at, 2);
    cx.run_until_parked();
    assert!(
        naming(&shell, cx) == Some(Naming::Folder(FOLDER)),
        "the second press gave no field"
    );
}

/// The page had the keyboard, from a press in it the host tells of only after the double-click.
#[gpui::test]
fn a_double_click_on_a_folder_after_a_press_in_the_page_names_it(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let pressed = fields_opened();
    let at = row(&shell, cx, FOLDER);
    click(cx, at, 1);
    click(cx, at, 2);
    shell.update_in(cx, |shell, window, cx| {
        shell.keyboard_left(pressed, window, cx)
    });
    cx.run_until_parked();
    opened(&shell, cx);
    cx.run_until_parked();
    assert!(
        naming(&shell, cx) == Some(Naming::Folder(FOLDER)),
        "the field closed or never opened"
    );
}

/// Both presses come before a frame is drawn, and the core's word that the folder opened between
/// them.
#[gpui::test]
fn a_double_click_on_a_folder_before_a_frame_names_it(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let at = row(&shell, cx, FOLDER);
    click(cx, at, 1);
    opened(&shell, cx);
    click(cx, at, 2);
    cx.run_until_parked();
    assert!(
        naming(&shell, cx) == Some(Naming::Folder(FOLDER)),
        "the second press gave no field"
    );
}
