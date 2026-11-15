//! Point's mode in a window, on a host that records what the shell asks of the core: the bar, the
//! keys, the notes, and the mode following its tab.

use std::rc::Rc;

use gpui::{Bounds, Entity, TestAppContext, VisualTestContext, point, px, size};

use crate::agents::PointMode;
use crate::point::Element;
use crate::shell::{Folders, Shell, recorder::Recorder};

/// A window showing Space 1's tabs 10 and 11, 10 in front.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.viewport = size(px(1400.), px(900.));
        let space = (
            1,
            "Space 1".into(),
            None,
            0,
            "".into(),
            "".into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    (shell, host, cx)
}

fn mode(id: u64) -> PointMode {
    PointMode {
        id,
        tab: 10,
        from: "task-1".into(),
        agent: "Claude".into(),
        question: "Which part of the page looks wrong?".into(),
    }
}

fn header() -> Element {
    Element {
        kind: "banner".into(),
        name: "Field notes".into(),
        tag: "header".into(),
        frame: Bounds::new(point(px(40.), px(80.)), size(px(600.), px(120.))),
        ..Default::default()
    }
}

/// The bar as the shell would show it, as (lead, words, hint, each button's label and whether it's
/// enabled), or None while it shows none.
type Shown = (String, String, String, Vec<(String, bool)>);

fn bar(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Shown> {
    shell.update(cx, |shell, cx| {
        let (bar, _) = shell.point_bar(cx)?;
        Some((
            bar.lead
                .map(|lead| lead.text.to_string())
                .unwrap_or_default(),
            bar.words.to_string(),
            bar.hint.map(|hint| hint.to_string()).unwrap_or_default(),
            (bar.buttons.iter())
                .map(|button| (button.label.to_string(), button.enabled))
                .collect(),
        ))
    })
}

fn points(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|call| call.contains("point"))
        .collect()
}

/// The user clicks the header in tab 10 and adds `words` in the panel.
fn note(shell: &Entity<Shell>, words: &str, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.point_picked(10, header(), cx);
        shell.point_add(words.to_owned(), cx);
    });
}

#[gpui::test]
fn the_bar_says_who_asks_and_sends_only_once_theres_a_note(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    assert_eq!(bar(&shell, cx), None);

    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    let not_now = ("Not Now".to_owned(), true);
    assert_eq!(
        bar(&shell, cx),
        Some((
            "Claude asks".into(),
            "Which part of the page looks wrong?".into(),
            "Click what you mean".into(),
            vec![not_now.clone(), ("Send".into(), false)],
        ))
    );

    note(&shell, "  The title is cut off ", cx);
    let (_, _, hint, buttons) = bar(&shell, cx).expect("the bar");
    assert_eq!(hint, "1 note");
    assert_eq!(buttons, vec![not_now, ("Send".into(), true)]);
    note(&shell, "", cx);
    assert_eq!(bar(&shell, cx).expect("the bar").2, "2 notes");
    shell.read_with(cx, |shell, _| {
        let notes = &shell.point.as_ref().expect("pointing").notes;
        // What the user wrote, trimmed; a note with no words is numbered by what it is.
        assert_eq!(notes[0].label(1), "1 · The title is cut off");
        assert_eq!(notes[1].label(2), "2 · Header · Field notes");
    });
}

#[gpui::test]
fn a_note_closed_without_add_isnt_counted(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_point_mode(Some(mode(7)), cx);
        shell.point_picked(10, header(), cx);
        shell.point_note_dropped(cx);
        // Nor does a click in another tab's page open one.
        shell.point_picked(11, header(), cx);
        let pointing = shell.point.as_ref().expect("pointing");
        assert!(pointing.notes.is_empty());
        assert!(pointing.writing.is_none());
    });
}

#[gpui::test]
fn the_same_ask_keeps_its_notes_and_a_new_one_starts_with_none(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    note(&shell, "This one", cx);
    shell.update(cx, |shell, cx| {
        shell.set_point_mode(Some(mode(7)), cx);
        assert_eq!(shell.point.as_ref().expect("pointing").notes.len(), 1);
        shell.set_point_mode(Some(mode(8)), cx);
        assert!(shell.point.as_ref().expect("pointing").notes.is_empty());
        shell.set_point_mode(None, cx);
        assert!(shell.point.is_none());
    });
}

#[gpui::test]
fn the_mode_shows_only_with_its_tab(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    shell.update(cx, |shell, _| assert!(shell.point_drawn().is_some()));
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    assert_eq!(bar(&shell, cx), None);
    shell.update(cx, |shell, cx| {
        assert!(shell.point_drawn().is_none());
        // Its keys aren't taken while another tab shows.
        assert!(!shell.point_key("escape", None, cx));
        shell.set_selected(10, cx);
    });
    assert!(bar(&shell, cx).is_some());
}

/// The frame and marks are drawn in the window over the page, so it shows while Point does, and
/// goes with the mode or its tab.
#[gpui::test]
fn the_window_over_the_page_shows_while_pointing(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    cx.run_until_parked();
    assert!(!host.over_page_shown.get());
    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    cx.run_until_parked();
    assert!(
        host.over_page_shown.get(),
        "pointing shows the window over the page"
    );
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    cx.run_until_parked();
    assert!(!host.over_page_shown.get(), "another tab in front hides it");
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    cx.run_until_parked();
    assert!(
        host.over_page_shown.get(),
        "its tab in front again shows it"
    );
    shell.update(cx, |shell, cx| shell.set_point_mode(None, cx));
    cx.run_until_parked();
    assert!(!host.over_page_shown.get(), "the mode's end hides it");
}

#[gpui::test]
fn esc_answers_not_now_and_cmd_enter_sends_the_notes(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    host.take();

    // Nothing to send yet: ⌘↩ is taken, and sends nothing.
    shell.update(cx, |shell, cx| {
        assert!(shell.point_key("cmd-enter", None, cx))
    });
    assert_eq!(points(&host), Vec::<String>::new());

    note(&shell, "The title is cut off", cx);
    shell.update(cx, |shell, cx| {
        assert!(shell.point_key("cmd-enter", Some(10), cx))
    });
    assert_eq!(points(&host), ["answer point 7: The title is cut off"]);

    shell.update(cx, |shell, cx| assert!(shell.point_key("escape", None, cx)));
    assert_eq!(points(&host), ["answer point 7 not now"]);
}

#[gpui::test]
fn up_and_down_in_the_page_move_the_mark_out_and_in(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_point_mode(Some(mode(7)), cx));
    host.take();
    shell.update(cx, |shell, cx| {
        assert!(shell.point_key("up", Some(10), cx));
        assert!(shell.point_key("down", Some(10), cx));
        // From the UI, or another pane's page, the arrows stay theirs.
        assert!(!shell.point_key("up", None, cx));
        assert!(!shell.point_key("up", Some(11), cx));
        assert!(!shell.point_key("a", Some(10), cx));
    });
    assert_eq!(points(&host), ["point do 10 false", "point do 10 true"]);
}

#[gpui::test]
fn while_a_note_is_written_its_field_has_the_keys(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_point_mode(Some(mode(7)), cx);
        shell.point_picked(10, header(), cx);
    });
    host.take();
    shell.update(cx, |shell, cx| {
        assert!(!shell.point_key("escape", None, cx));
        assert!(!shell.point_key("cmd-enter", None, cx));
    });
    assert_eq!(points(&host), Vec::<String>::new());
}

#[gpui::test]
fn point_starts_only_where_it_can(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    host.take();
    // A private window says why not, and the press does nothing.
    host.point_can.set(2);
    shell.update(cx, |shell, cx| {
        assert_eq!(shell.may_point_here("task-1"), 2);
        shell.press_point(7, "task-1", cx);
    });
    assert_eq!(points(&host), Vec::<String>::new());
    assert_eq!(
        crate::point::why_not(2),
        Some("Point can't point into a private window.")
    );

    host.point_can.set(0);
    shell.update(cx, |shell, cx| shell.press_point(7, "task-1", cx));
    assert_eq!(points(&host), ["start point 7 10"]);
}
