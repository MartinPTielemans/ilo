//! A saved read's row in Settings › Agents (#3822): its name over its site and its state, which a
//! press opens in place to show what it reads, with Remove Read at its foot. Settings gives what
//! a press does; the words are agents::reads::rows's.

use std::rc::Rc;

use gpui::{App, ClickEvent, Rgba, Window, div, prelude::*, px, rgba};

use crate::agents::reads::rows::{Ink, REMOVE, Row};
use crate::settings_rows::{
    Drawn, Look, chevron_slot, drawn, frame, glyph_plate, named_button, ring, stop_button,
};
use crate::tokens::menu::{DARK_DESTRUCTIVE, LIGHT_DESTRUCTIVE};
use crate::tokens::settings::{
    BUTTONS_TOP, DETAIL_LINE, DETAIL_TOP, PADDING, ROW_X, SENT_GAP, SENT_LABEL, STATUS_DOT,
    STATUS_DOT_GAP, WARNING_DARK, WARNING_LIGHT,
};
use crate::tokens::text::SMALL;

/// What a press on a row does: the row's own, as Settings has it.
pub type Press = Rc<dyn Fn(&mut Window, &mut App)>;

/// The ink of a state's words and its dot, by `look`'s ground.
pub fn ink(look: &Look, ink: Ink) -> (Rgba, Rgba) {
    let light = look.light();
    let pick = |light_ink: u32, dark_ink: u32| rgba(if light { light_ink } else { dark_ink });
    match ink {
        Ink::Ready => (look.detail, look.accent),
        Ink::Warning => {
            let warning = pick(WARNING_LIGHT, WARNING_DARK);
            (warning, warning)
        }
        Ink::Broken => {
            let red = pick(LIGHT_DESTRUCTIVE, DARK_DESTRUCTIVE);
            (red, red)
        }
        Ink::Paused => (look.detail, look.detail),
    }
}

/// `row`, the `at`th, `opened` or not: a press on its head calls `open`, and Remove Read
/// `remove`. Its name is what an assistive app calls it, and it says whether it's opened. Where
/// each is drawn goes in `logged` (settings_rows::drawn), for check.sh to press it.
#[allow(clippy::too_many_arguments)]
pub fn row(
    look: &Look,
    row: &Row,
    at: usize,
    opened: bool,
    a11y: bool,
    logged: &Drawn,
    open: Press,
    remove: Press,
) -> gpui::Div {
    let (words, dot) = ink(look, row.ink);
    let small = |line: gpui::Div| {
        line.text_size(px(SMALL))
            .line_height(px(DETAIL_LINE))
            .text_color(look.detail)
    };
    // A paused read's dot is a ring: it's stopped by the user's list, not by its site.
    let dot = div()
        .flex_none()
        .size(px(STATUS_DOT))
        .mr(px(STATUS_DOT_GAP))
        .rounded_full()
        .map(|mark| match row.ink {
            Ink::Paused => mark.border_1().border_color(dot),
            _ => mark.bg(dot),
        });
    let state = small(div().mt(px(DETAIL_TOP)))
        .flex()
        .items_center()
        .min_w_0()
        .child(div().flex_none().child(format!("{} · ", row.site)))
        .child(dot)
        .child(div().truncate().text_color(words).child(row.state.clone()));
    let says = (row.says.clone()).map(|says| small(div().mt(px(DETAIL_TOP))).child(says));
    let text = div()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .truncate()
                .text_color(look.ink)
                .child(row.name.clone()),
        )
        .child(state)
        .children(says);
    let head = frame()
        .id(("saved-read", at))
        .relative()
        .child(drawn(format!("saved read {at}"), logged.clone()))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .cursor_pointer()
        .child(glyph_plate(look, "icons/database.svg", true))
        .child(text)
        .child(chevron_slot(Some(opened), look.detail.into()))
        .when(a11y, |head| {
            named_button(head, row.name.clone()).aria_expanded(opened)
        })
        .on_click(move |_: &ClickEvent, window, cx| open(window, cx));
    let part = opened.then(|| {
        let facts = (row.facts.iter()).map(|(label, value)| {
            small(div())
                .flex()
                .gap(px(SENT_GAP))
                .child(div().w(px(SENT_LABEL)).flex_none().child(*label))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(look.ink)
                        .child(value.clone()),
                )
        });
        let red = ink(look, Ink::Broken).0;
        let button = stop_button(look, ("saved-read-remove", at), REMOVE, a11y)
            .relative()
            .child(drawn(format!("saved read {at} remove"), logged.clone()))
            .text_color(red)
            .on_click(move |_: &ClickEvent, window, cx| remove(window, cx));
        div().px(px(ROW_X)).pb(px(PADDING)).children(facts).child(
            div()
                .mt(px(BUTTONS_TOP))
                .flex()
                .items_center()
                .child(small(div()).flex_1().min_w_0().child(row.foot.clone()))
                .child(button),
        )
    });
    div().child(head).children(part)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};

    use super::row;
    use crate::agents::reads::rows::{Ink, Row};
    use crate::settings_rows::{Drawn, DrawnLog, Look, group, spoken::pressable};

    struct Rows {
        rows: Vec<Row>,
        logged: Drawn,
        opened: Rc<RefCell<Option<usize>>>,
        removed: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for Rows {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let look = Look::settings(false);
            let opened = *self.opened.borrow();
            let drawn: Vec<gpui::Div> = (self.rows.iter().enumerate())
                .map(|(at, each)| {
                    let (open, removed) = (self.opened.clone(), self.removed.clone());
                    let view = cx.entity().downgrade();
                    row(
                        &look,
                        each,
                        at,
                        opened == Some(at),
                        true,
                        &self.logged,
                        Rc::new(move |_, cx| {
                            let mut open = open.borrow_mut();
                            *open = (*open != Some(at)).then_some(at);
                            _ = view.update(cx, |_, cx| cx.notify());
                        }),
                        Rc::new(move |_, _| removed.borrow_mut().push(at)),
                    )
                })
                .collect();
            div().w(gpui::px(560.)).child(group(&look, drawn))
        }
    }

    fn read(name: &str, state: &str, ink: Ink, says: Option<&str>) -> Row {
        Row {
            name: name.into(),
            site: "reads.test".into(),
            state: state.into(),
            ink,
            says: says.map(str::to_string),
            facts: vec![
                ("Reads", "reads.test/reads/table".into()),
                ("Columns", "Name, City".into()),
            ],
            foot: "Saved Nov 14 · Asked for by Claude Code".into(),
        }
    }

    /// Lib test 6: each row an assistive app presses is called by the name it shows and says
    /// whether it's opened; a press opens it in place, where Remove Read is called as it reads and
    /// removes that read, and a second press closes it.
    #[gpui::test]
    fn a_row_opens_in_place_and_says_what_it_shows(cx: &mut TestAppContext) {
        use gpui::accesskit::Role;
        let (opened, removed) = (
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(Vec::new())),
        );
        let rows = vec![
            read("city buildings", "Read 4 min ago", Ink::Ready, None),
            read(
                "orders this week",
                "Needs a sign-in",
                Ink::Warning,
                Some("Sign in to reads.test in any tab so agents can read it again."),
            ),
        ];
        let logged = Rc::new(RefCell::new(DrawnLog::default()));
        let (_view, cx) = cx.add_window_view({
            let (opened, removed, logged) = (opened.clone(), removed.clone(), logged.clone());
            move |_, _| Rows {
                rows,
                logged,
                opened,
                removed,
            }
        });
        // A press where the control `what` was drawn, as check.sh presses it.
        let click = |what: &str, cx: &mut gpui::VisualTestContext| {
            let at = format!("browser: settings saved read {what} at ");
            let line = (logged.borrow().lines().iter())
                .find_map(|line| line.strip_prefix(&at).map(str::to_string))
                .unwrap_or_else(|| panic!("{what} isn't drawn: {:?}", logged.borrow().lines()));
            let (x, y) = line.split_once(' ').expect("no place");
            let centre = gpui::point(gpui::px(x.parse().unwrap()), gpui::px(y.parse().unwrap()));
            cx.simulate_click(centre, gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let buttons = |cx: &mut gpui::VisualTestContext| -> Vec<String> {
            (pressable(cx).into_iter())
                .filter(|(role, _, _)| *role == Role::Button)
                .map(|(_, name, _)| name)
                .collect()
        };
        assert_eq!(buttons(cx), ["city buildings", "orders this week"]);
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let expanded: Vec<Option<bool>> = (tree.nodes.iter())
            .filter(|(_, node)| node.role() == Role::Button)
            .map(|(_, node)| node.is_expanded())
            .collect();
        assert_eq!(expanded, [Some(false), Some(false)]);

        click("1", cx);
        assert_eq!(*opened.borrow(), Some(1));
        assert_eq!(
            buttons(cx),
            ["city buildings", "orders this week", "Remove Read"]
        );
        click("1 remove", cx);
        assert_eq!(*removed.borrow(), [1], "Remove Read removed another read");
        click("1", cx);
        assert_eq!(*opened.borrow(), None, "a second press didn't close it");
        assert_eq!(buttons(cx), ["city buildings", "orders this week"]);
    }

    /// Each state's words read at WCAG AA's 4.5:1 for small text on their group, in light and
    /// dark: the warning's amber too.
    #[test]
    fn each_states_words_read_in_light_and_dark() {
        for dark in [true, false] {
            let look = Look::settings(dark);
            for state in [Ink::Ready, Ink::Warning, Ink::Broken, Ink::Paused] {
                let (words, _) = super::ink(&look, state);
                let ratio = crate::palette::contrast(words, look.surface);
                assert!(ratio >= 4.5, "{state:?} reads at {ratio:.2}:1, dark {dark}");
            }
        }
    }
}
