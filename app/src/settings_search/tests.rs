//! The search as the user meets it: what a few words find among Settings' rows, the found rows
//! under their sections' names, a click there, and Esc.

use std::cell::RefCell;

use gpui::{Context, Focusable, Modifiers, TestAppContext, VisualTestContext, size};

use super::*;
use crate::settings_rows::{frame, text as row_text};

/// Settings' rows that the searches below meet, as its sections name them: their names, lines
/// and keys, the Little ilo's own settings under it.
fn rows() -> Vec<Entry> {
    let row = |section, under: Option<&str>, name: &str, line: Option<&str>| Entry {
        section,
        under: under.map(|under| SharedString::from(under.to_string())),
        name: name.to_string().into(),
        line: line.map(|line| SharedString::from(line.to_string())),
        keys: None,
    };
    let command = |name: &str, keys: &str| Entry {
        keys: Some(shell::keystroke_label(keys)),
        ..row(Section::Shortcuts, None, name, None)
    };
    let little = concat!("Little ", env!("APP_NAME"));
    vec![
        row(
            Section::General,
            None,
            "Reopen your windows and tabs",
            Some("Off: ilo opens one empty window, and Today tabs go to the archive."),
        ),
        row(
            Section::TabsAndSidebar,
            None,
            "Archive Today tabs after",
            Some("Tabs you haven't looked at go to the archive. ⇧⌘T brings one back."),
        ),
        row(
            Section::TabsAndSidebar,
            None,
            "Unload tabs you haven't looked at",
            Some("After 2 hours out of sight a tab frees its memory."),
        ),
        row(
            Section::TabsAndSidebar,
            None,
            "Sidebar position",
            Some("The side of the window the sidebar and the window's buttons are on."),
        ),
        row(
            Section::Features,
            None,
            little,
            Some("A small window for a quick look."),
        ),
        row(
            Section::Features,
            Some(little),
            "Archive windows left unused after",
            Some("Counted from when you leave the window."),
        ),
        row(
            Section::Features,
            None,
            "Peek",
            Some("Links open in a preview."),
        ),
        row(
            Section::Features,
            None,
            "Mini Player",
            Some("A playing video or call floats over your work when you leave its tab."),
        ),
        command("New Tab", "cmd-t"),
        command("Restore Last Closed Tab", "shift-cmd-t"),
        command("Archive Tab", "cmd-w"),
        command("View Archive", ""),
        command("Clear Archive", ""),
    ]
}

fn names(typed: &str) -> Vec<String> {
    let rows = rows();
    find(typed, &rows)
        .into_iter()
        .map(|index| rows[index].name.to_string())
        .collect()
}

#[test]
fn pip_finds_the_mini_player_by_another_name() {
    assert_eq!(names("pip"), ["Mini Player"]);
    assert_eq!(names("picture"), ["Mini Player"]);
    assert_eq!(names("link prev"), ["Peek"]);
}

/// The drawings have no words to find, so the row's other names hold its sides: either side's
/// word finds it, as its name does (#2407).
#[test]
fn either_side_finds_sidebar_position() {
    assert_eq!(names("sidebar position"), ["Sidebar position"]);
    assert_eq!(names("right"), ["Sidebar position"]);
    assert_eq!(
        names("left"),
        ["Sidebar position", "Archive windows left unused after"]
    );
}

#[test]
fn cmd_t_finds_the_commands_on_t() {
    assert_eq!(names("cmd t"), ["New Tab", "Restore Last Closed Tab"]);
    let keys = shell::keystroke_label("shift-cmd-t");
    assert_eq!(names(&keys), ["Restore Last Closed Tab"]);
}

/// "archive" names five rows in three sections. The line under Reopen your windows and tabs says
/// the archive too, but a setting named by the word comes before every line that mentions it.
#[test]
fn archive_finds_five_rows_in_three_sections() {
    let rows = rows();
    let found: Vec<&Entry> = find("archive", &rows)
        .into_iter()
        .map(|index| &rows[index])
        .collect();
    let names: Vec<&str> = found.iter().map(|entry| entry.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "Archive Today tabs after",
            "Archive windows left unused after",
            "Archive Tab",
            "View Archive",
            "Clear Archive"
        ]
    );
    assert_eq!(
        sections(&found),
        [
            Section::TabsAndSidebar,
            Section::Features,
            Section::Shortcuts
        ]
    );
}

/// Words no row is named by find the rows whose lines have them, in any order.
#[test]
fn a_line_finds_its_row_when_no_name_does() {
    assert_eq!(names("memory"), ["Unload tabs you haven't looked at"]);
    assert_eq!(names("work video"), ["Mini Player"]);
    assert!(names("zzz").is_empty());
    assert!(names("  ").is_empty());
}

/// Settings in small: the column's field, and under it either the section's page or what the
/// field finds, as Settings draws them.
struct Searched {
    focus: gpui::FocusHandle,
    input: Entity<TextInput>,
    shown: Section,
    /// Each section name clicked, with the row it was to show.
    clicked: Rc<RefCell<Vec<(Section, String)>>>,
    /// Where each found group's section name was last drawn, by what check.sh calls it.
    names: Rc<RefCell<Vec<(String, gpui::Point<gpui::Pixels>)>>>,
}

impl Render for Searched {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = Look::settings(true);
        let rows = rows();
        let typed = typed(&self.input, cx);
        let clicked = self.clicked.clone();
        let show: Show = Rc::new(move |section, row, _, _| {
            clicked.borrow_mut().push((section, row.to_string()))
        });
        self.names.borrow_mut().clear();
        let names = self.names.clone();
        let drawn = move |what: String| {
            let names = names.clone();
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, _, _| names.borrow_mut().push((what.clone(), bounds.center())),
            )
            .absolute()
            .inset_0()
        };
        let card = match typed.is_empty() {
            true => div()
                .relative()
                .child(self.shown.name())
                .child(drawn("page".into())),
            false => {
                let found = find(&typed, &rows).into_iter().map(|index| {
                    let entry = rows[index].clone();
                    let row = frame().child(row_text(&look, entry.name.clone(), None));
                    (entry, row)
                });
                results(&look, found.collect(), Vec::new(), show, drawn)
            }
        };
        let input = self.input.clone();
        let field = field(
            &self.input,
            gpui::rgb(0xffffff),
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            move |window, cx| {
                end(&input, window, cx);
            },
            cx,
        );
        div()
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(|view, _: &Search, window, cx| {
                window.focus(&view.input.read(cx).focus_handle(cx), cx)
            }))
            .on_action(cx.listener(|view, _: &EndSearch, window, cx| {
                end(&view.input, window, cx);
                cx.notify();
            }))
            .size_full()
            .flex()
            .child(div().w(px(208.)).pt(px(48.)).child(field))
            .child(div().flex_1().p(px(36.)).child(card))
    }
}

fn searched(cx: &mut TestAppContext) -> (Entity<Searched>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| {
        let input = cx.new(|cx| TextInput::new("Search", cx));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        Searched {
            focus: cx.focus_handle(),
            input,
            shown: Section::General,
            clicked: Rc::default(),
            names: Rc::default(),
        }
    });
    cx.simulate_resize(size(px(820.), px(660.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// ⌘F, Ctrl+F off the Mac, puts the keyboard in the field, and what's typed there searches.
fn search(typed: &str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(if shell::OFF_THE_MAC {
        "ctrl-f"
    } else {
        "cmd-f"
    });
    cx.simulate_input(typed);
    cx.run_until_parked();
}

/// Whether the last frame drew `what`.
fn drew(view: &Entity<Searched>, what: &str, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |view, _| {
        view.names.borrow().iter().any(|(each, _)| each == what)
    })
}

fn field_text(view: &Entity<Searched>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| view.input.read(cx).text().to_string())
}

/// What's typed replaces the page with the rows found, and Esc empties the field, so the section
/// shows again.
#[gpui::test]
fn esc_empties_the_field_and_shows_the_section_again(cx: &mut TestAppContext) {
    let (view, cx) = searched(cx);
    search("pip", cx);
    assert_eq!(field_text(&view, cx), "pip");
    assert!(!drew(&view, "page", cx), "the page showed over the results");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(field_text(&view, cx), "");
    assert!(drew(&view, "page", cx), "the section didn't show again");
}

/// A search that finds nothing says so, in place of the page.
#[gpui::test]
fn a_search_that_finds_nothing_says_so(cx: &mut TestAppContext) {
    let (view, cx) = searched(cx);
    search("zzz", cx);
    assert!(
        drew(&view, "nothing found", cx),
        "nothing said nothing was found"
    );
    assert!(!drew(&view, "page", cx));
}

/// "archive" shows its rows under Tabs & Sidebar, Features › Little ilo and Shortcuts, and a click
/// on Features shows that section at its row.
#[gpui::test]
fn a_click_on_a_section_name_shows_the_section_at_the_row(cx: &mut TestAppContext) {
    let (view, cx) = searched(cx);
    search("archive", cx);
    let names = view.read_with(cx, |view, _| view.names.borrow().clone());
    let labels: Vec<&str> = names.iter().map(|(what, _)| what.as_str()).collect();
    assert_eq!(
        labels,
        [
            "found Tabs & Sidebar Archive Today tabs after",
            "found Features Archive windows left unused after",
            "found Shortcuts Archive Tab"
        ]
    );
    cx.simulate_click(names[1].1, Modifiers::none());
    cx.run_until_parked();
    let clicked = view.read_with(cx, |view, _| view.clicked.borrow().clone());
    assert_eq!(
        clicked,
        [(
            Section::Features,
            "Archive windows left unused after".to_string()
        )]
    );
}

/// What an input method composes shows in the field and finds nothing until it commits, then the
/// word committed finds its rows; Esc while it composes is the composition's, so the search stays.
#[gpui::test]
fn a_composition_finds_nothing_until_it_commits(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler as _;
    let (view, cx) = searched(cx);
    search("", cx);
    let input = view.read_with(cx, |view, _| view.input.clone());
    let compose = |text: &str, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, text, None, window, cx)
            })
        });
        cx.run_until_parked();
    };
    compose("ぴっぷ", cx);
    assert_eq!(field_text(&view, cx), "ぴっぷ");
    assert!(drew(&view, "page", cx), "the composition searched");
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, "pip", window, cx)
        })
    });
    cx.run_until_parked();
    assert!(!drew(&view, "page", cx), "the committed word didn't search");
    let names = view.read_with(cx, |view, _| view.names.borrow().clone());
    assert!(
        names
            .iter()
            .any(|(what, _)| what == "found Features Mini Player")
    );
    compose("あ", cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        field_text(&view, cx),
        "pipあ",
        "Esc ended the search while composing"
    );
    assert!(!drew(&view, "page", cx));
}
