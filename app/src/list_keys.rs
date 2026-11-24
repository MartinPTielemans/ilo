//! The keys of a list, one rule for every list in the app (#4108): the arrows on its axis step,
//! Home and End (Page Up and Page Down, and down a column ⌥↑ and ⌥↓) go to its ends, return or
//! space chooses, and, where the list asks, a character selects by type-ahead. Lifted from the
//! menu's keys (menu.rs `key_of`, `Tracking::key` and `type_select`), the copy that already had
//! all of them.
//!
//! It decides and the surface draws: it's told where the list is and answers where it goes, so it
//! never takes the keyboard or requests focus itself. Focus stays the surface's act.
//!
//! How to compose it: keep a `ListKeys` for the list, and in the list's `on_key_down` pass the
//! event, the index that has the keyboard (or the chosen one, for a radio group) and the count to
//! `ListKeys::step`. On `Some`, stop propagation and act on the `Step`; on `None` let the key go
//! on, as a key the list doesn't take must reach the shortcuts beyond it.
//!
//! When the row with the keyboard is removed, by the user, the app or an agent, the keyboard goes
//! to the row that took its place: the next one, else the one before, as a closed tab's row gives
//! it to the row that moves up into its place. A list composes that by passing its rows as they
//! were and as they are to `took_place` as they change.

use std::time::{Duration, Instant};

use gpui::{App, KeyDownEvent, Window};

/// How long a pause starts type-ahead afresh, as the menu's does.
const TYPE_RESET: Duration = Duration::from_millis(1000);

/// The arrows a list steps by: ↑ and ↓ down a column, ← and → along a row. The other pair is left
/// to the surface, as a menu's → opens a submenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// What a step past an end does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ends {
    /// Stays on the end, as NSMenu's rows and a radio group's choices do.
    Stop,
    /// Comes round to the other end, as the command bar's rows do.
    Wrap,
}

/// What a key does to the list. The surface draws it and, for a choice, acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The keyboard moves to this item.
    To(usize),
    /// This item is chosen: return or space on it, or in a radio group a move to it.
    Choose(usize),
    /// A character to select by, for `TypeAhead::find`.
    Type(char),
}

/// A list's key rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListKeys {
    pub axis: Axis,
    pub ends: Ends,
    /// The choice follows the keyboard, as a radio group's does: every move is a `Choose`.
    pub follows: bool,
    /// A character is a `Step::Type`. Only lists that already select by typing ask for it.
    pub types: bool,
}

impl ListKeys {
    /// A list whose items the keyboard moves over and return or space chooses.
    pub const fn list(axis: Axis, ends: Ends) -> Self {
        Self {
            axis,
            ends,
            follows: false,
            types: false,
        }
    }

    /// A radio group: a move chooses where it lands.
    pub const fn radio(axis: Axis, ends: Ends) -> Self {
        Self {
            follows: true,
            ..Self::list(axis, ends)
        }
    }

    /// The same list, selecting by typing.
    pub const fn typing(self) -> Self {
        Self {
            types: true,
            ..self
        }
    }

    /// What `event` does to a list of `count` items whose keyboard is on `at` (none yet: a move
    /// starts at the end it heads from). `None` is a key the list doesn't take: one with ⌘, ⌃ or
    /// ⇧ (or ⌥ but on a column's ↑ and ↓), an arrow off its axis, or an empty list's.
    pub fn step(&self, event: &KeyDownEvent, at: Option<usize>, count: usize) -> Option<Step> {
        let keys = &event.keystroke;
        let modifiers = &keys.modifiers;
        if count == 0 || modifiers.control || modifiers.platform || modifiers.shift {
            return None;
        }
        let (back, on) = match self.axis {
            Axis::Vertical => ("up", "down"),
            Axis::Horizontal => ("left", "right"),
        };
        let last = count - 1;
        let key = keys.key.as_str();
        // ⌥↑ and ⌥↓ are a column's ends, as the Mac's lists and menus have them; ⌥← and ⌥→ are a
        // word's moves there, nothing a row takes, so with ⌥ a row's keys are the window's.
        let ends = modifiers.alt && self.axis == Axis::Vertical;
        let to = match key {
            _ if key == back && ends => 0,
            _ if key == on && ends => last,
            _ if modifiers.alt => return None,
            _ if key == back => match (at, self.ends) {
                (None, _) => last,
                (Some(0), Ends::Stop) => 0,
                (Some(0), Ends::Wrap) => last,
                (Some(at), _) => at.min(count) - 1,
            },
            _ if key == on => match (at, self.ends) {
                (None, _) => 0,
                (Some(at), Ends::Stop) => (at + 1).min(last),
                (Some(at), Ends::Wrap) if at >= last => 0,
                (Some(at), _) => at + 1,
            },
            "home" | "pageup" => 0,
            "end" | "pagedown" => last,
            "enter" | "space" => return at.filter(|&at| at < count).map(Step::Choose),
            _ => return self.typed(event),
        };
        Some(if self.follows {
            Step::Choose(to)
        } else {
            Step::To(to)
        })
    }

    /// A character for type-ahead, where the list selects by typing. A list has no text to
    /// compose, so a character an input method gives a key, such as kana, selects as itself.
    fn typed(&self, event: &KeyDownEvent) -> Option<Step> {
        if !self.types
            || matches!(
                event.keystroke.key.as_str(),
                "left" | "right" | "up" | "down"
            )
        {
            return None;
        }
        let keys = &event.keystroke;
        let typed = keys.key_char.as_deref().unwrap_or(&keys.key);
        let mut chars = typed.chars();
        let c = chars
            .next()
            .filter(|c| !c.is_control() && chars.next().is_none())?;
        Some(Step::Type(c))
    }
}

/// What's been typed into a list in the last second, to select by.
#[derive(Clone, Debug, Default)]
pub struct TypeAhead {
    typed: String,
    at: Option<Instant>,
}

impl TypeAhead {
    /// The item `c` selects among `titles`, the keyboard on `at`, typed at `now`: the first whose
    /// title starts with what's been typed in the last second, ignoring case and accents, or with
    /// the same letter again, the next one after `at` that starts with it.
    pub fn find(
        &mut self,
        c: char,
        now: Instant,
        at: Option<usize>,
        titles: &[impl AsRef<str>],
    ) -> Option<usize> {
        if self.at.is_none_or(|at| now.duration_since(at) > TYPE_RESET) {
            self.typed.clear();
        }
        self.typed.push(c);
        self.at = Some(now);
        let typed = folded(&self.typed);
        let mut letters = typed.chars();
        let first = letters.next();
        let repeated = typed.chars().count() > 1 && letters.all(|each| Some(each) == first);
        let starts = |item: usize, with: &str| folded(titles[item].as_ref()).starts_with(with);
        if repeated {
            let with = first.map(String::from).unwrap_or_default();
            let start = at.map_or(0, |at| (at + 1).min(titles.len()));
            (start..titles.len())
                .chain(0..start)
                .find(|&item| starts(item, &with))
        } else {
            (0..titles.len()).find(|&item| starts(item, &typed))
        }
    }
}

/// Where the keyboard goes as a list's rows change from `before` to `now`, from the row `keyed`:
/// that row while it's still there, else the row that took its place, the first after it in
/// `before` still there, else the last before it. None once no row it had is left, as when the
/// whole list is new.
pub fn took_place<T: PartialEq + Clone>(keyed: &T, before: &[T], now: &[T]) -> Option<T> {
    if now.contains(keyed) {
        return Some(keyed.clone());
    }
    let at = before.iter().position(|row| row == keyed)?;
    let kept = |row: &&T| now.contains(row);
    let after = before[at + 1..].iter().find(kept);
    after
        .or_else(|| before[..at].iter().rev().find(kept))
        .cloned()
}

/// How a list's controls are named, which decides where the keyboard goes with a removed row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Named {
    /// By the row's line, as a filter's Remove `("ad-block-filter-remove", index)` is: the row
    /// after moves up into the removed one's line, and its control into that control's name and
    /// the keyboard with it.
    ByLine,
    /// By the row itself, as a device's Remove is by the device's id.
    ByRow,
}

/// A row's own control, such as its Remove, is removing `row` from `rows`, pressed from the keys
/// (`from_keys`): hands the keyboard on to the same control of the row that takes its place, as
/// `took_place` has it, before the row goes. A press with the pointer leaves the keyboard where it
/// was, as such a control takes none. With no row left, the control that follows takes it.
/// It moves by ⇥'s stops, so each row's control must be its row's only stop, as each of
/// Settings' and the devices' Removes is; a row given a second stop needs its control found by
/// the row instead.
pub fn removing<T: PartialEq + Clone>(
    row: &T,
    rows: &[T],
    named: Named,
    from_keys: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if !from_keys {
        return;
    }
    let now: Vec<T> = rows.iter().filter(|each| *each != row).cloned().collect();
    let at = |row: &T| rows.iter().position(|each| each == row);
    match (took_place(row, rows, &now), named) {
        (Some(took), _) if at(&took) < at(row) => window.focus_prev(cx),
        (Some(_), Named::ByLine) => {}
        (Some(_), Named::ByRow) | (None, _) => window.focus_next(cx),
    }
}

/// `text` lowercased without its accents, for type-ahead.
pub fn folded(text: &str) -> String {
    const ACCENTED: [(&str, char); 8] = [
        ("àáâãäåā", 'a'),
        ("çćč", 'c'),
        ("èéêëēė", 'e'),
        ("ìíîïī", 'i'),
        ("ñń", 'n'),
        ("òóôõöøō", 'o'),
        ("ùúûüū", 'u'),
        ("ýÿ", 'y'),
    ];
    (text.chars().flat_map(char::to_lowercase))
        .map(|c| {
            ACCENTED
                .iter()
                .find(|(set, _)| set.contains(c))
                .map_or(c, |&(_, base)| base)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Keystroke;

    fn press(keys: &'static str) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke::parse(keys).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }
    }

    /// Where the keyboard ends after pressing `keys` in turn from `at`, in a list of `count`.
    fn walk(
        list: ListKeys,
        at: Option<usize>,
        count: usize,
        keys: &[&'static str],
    ) -> Option<usize> {
        keys.iter()
            .fold(at, |at, keys| match list.step(&press(keys), at, count) {
                Some(Step::To(to) | Step::Choose(to)) => Some(to),
                _ => at,
            })
    }

    const COLUMN: ListKeys = ListKeys::list(Axis::Vertical, Ends::Stop);
    const RING: ListKeys = ListKeys::list(Axis::Vertical, Ends::Wrap);
    const ROW: ListKeys = ListKeys::radio(Axis::Horizontal, Ends::Stop);

    #[test]
    fn the_arrows_on_its_axis_step_and_the_others_pass() {
        assert_eq!(walk(COLUMN, Some(1), 4, &["down", "down"]), Some(3));
        assert_eq!(walk(COLUMN, Some(3), 4, &["up", "up", "up"]), Some(0));
        assert_eq!(COLUMN.step(&press("left"), Some(1), 4), None);
        assert_eq!(COLUMN.step(&press("right"), Some(1), 4), None);
        assert_eq!(walk(ROW, Some(0), 3, &["right", "right"]), Some(2));
        assert_eq!(ROW.step(&press("down"), Some(0), 3), None);
    }

    #[test]
    fn a_first_move_starts_at_the_end_it_heads_from() {
        assert_eq!(walk(COLUMN, None, 4, &["down"]), Some(0));
        assert_eq!(walk(COLUMN, None, 4, &["up"]), Some(3));
    }

    #[test]
    fn a_stopping_list_stays_on_its_ends_and_takes_the_key() {
        assert_eq!(COLUMN.step(&press("down"), Some(3), 4), Some(Step::To(3)));
        assert_eq!(COLUMN.step(&press("up"), Some(0), 4), Some(Step::To(0)));
    }

    #[test]
    fn a_wrapping_list_comes_round() {
        assert_eq!(walk(RING, Some(3), 4, &["down"]), Some(0));
        assert_eq!(walk(RING, Some(0), 4, &["up"]), Some(3));
    }

    #[test]
    fn home_end_and_option_arrows_go_to_the_ends() {
        for (keys, to) in [
            ("home", 0),
            ("pageup", 0),
            ("alt-up", 0),
            ("end", 4),
            ("pagedown", 4),
            ("alt-down", 4),
        ] {
            assert_eq!(walk(COLUMN, Some(2), 5, &[keys]), Some(to), "{keys}");
        }
        assert_eq!(walk(ROW, Some(1), 3, &["end", "home"]), Some(0));
        for keys in ["alt-left", "alt-right"] {
            assert_eq!(ROW.step(&press(keys), Some(1), 3), None, "{keys}");
            assert_eq!(COLUMN.step(&press(keys), Some(2), 5), None, "{keys}");
        }
    }

    #[test]
    fn return_and_space_choose_what_has_the_keyboard() {
        for keys in ["enter", "space"] {
            assert_eq!(COLUMN.step(&press(keys), Some(2), 4), Some(Step::Choose(2)));
            assert_eq!(COLUMN.step(&press(keys), None, 4), None);
        }
    }

    #[test]
    fn in_a_radio_group_the_choice_follows_the_keys() {
        assert_eq!(ROW.step(&press("right"), Some(0), 3), Some(Step::Choose(1)));
        assert_eq!(ROW.step(&press("left"), Some(0), 3), Some(Step::Choose(0)));
        assert_eq!(ROW.step(&press("end"), Some(0), 3), Some(Step::Choose(2)));
    }

    #[test]
    fn shortcuts_and_empty_lists_pass_the_key_on() {
        for keys in ["cmd-down", "ctrl-down", "shift-down", "cmd-enter"] {
            assert_eq!(COLUMN.step(&press(keys), Some(1), 4), None, "{keys}");
        }
        assert_eq!(COLUMN.step(&press("down"), None, 0), None);
        assert_eq!(COLUMN.step(&press("escape"), Some(1), 4), None);
    }

    #[test]
    fn characters_select_only_where_the_list_types() {
        assert_eq!(COLUMN.step(&press("g"), Some(1), 4), None);
        let typing = COLUMN.typing();
        assert_eq!(typing.step(&press("g"), Some(1), 4), Some(Step::Type('g')));
        assert_eq!(typing.step(&press("tab"), Some(1), 4), None);
    }

    #[test]
    fn type_ahead_finds_by_prefix_ignoring_case_and_accents() {
        let titles = ["Copy", "Élan", "Edit", "Export", "Exit"];
        let now = Instant::now();
        let mut typing = TypeAhead::default();
        assert_eq!(typing.find('e', now, None, &titles), Some(1));
        assert_eq!(typing.find('x', now, Some(1), &titles), Some(3));
        assert_eq!(typing.find('i', now, Some(3), &titles), Some(4));
        let later = now + TYPE_RESET + Duration::from_millis(1);
        assert_eq!(typing.find('c', later, Some(4), &titles), Some(0));
    }

    #[test]
    fn the_same_letter_again_steps_through_its_items() {
        let titles = ["Edit", "Copy", "Export", "Exit"];
        let now = Instant::now();
        let mut typing = TypeAhead::default();
        let mut at = typing.find('e', now, None, &titles);
        assert_eq!(at, Some(0));
        for expected in [2, 3, 0] {
            at = typing.find('e', now, at, &titles);
            assert_eq!(at, Some(expected));
        }
    }

    /// The rule a list's keyboard follows as its rows go: the row that took the removed one's
    /// place, the next, else the one before; nothing when no row it had is left.
    #[test]
    fn the_keyboard_goes_to_the_row_that_took_the_removed_ones_place() {
        let rows = [1, 2, 3, 4];
        assert_eq!(
            took_place(&2, &rows, &[1, 3, 4]),
            Some(3),
            "the next row took its place"
        );
        assert_eq!(
            took_place(&4, &rows, &[1, 2, 3]),
            Some(3),
            "the last row's went before it"
        );
        assert_eq!(
            took_place(&2, &rows, &[1, 4]),
            Some(4),
            "two going together, the first after both"
        );
        assert_eq!(
            took_place(&3, &rows, &[0, 1, 2, 3, 4]),
            Some(3),
            "a row added keeps it"
        );
        assert_eq!(
            took_place(&2, &rows, &[9]),
            None,
            "a new list has nothing in its place"
        );
    }

    /// A list of rows with a control each, named by line or by row as `named` says, as Settings'
    /// Removes are.
    struct Controls {
        rows: Vec<u64>,
        named: Named,
        by_line: Vec<gpui::FocusHandle>,
        by_row: std::collections::HashMap<u64, gpui::FocusHandle>,
    }

    impl gpui::Render for Controls {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{InteractiveElement, ParentElement, Styled};
            let controls = self.rows.iter().enumerate().map(|(line, row)| {
                let focus = match self.named {
                    Named::ByLine => &self.by_line[line],
                    Named::ByRow => &self.by_row[row],
                };
                gpui::div().h(gpui::px(20.)).track_focus(focus)
            });
            gpui::div().size_full().children(controls)
        }
    }

    /// A control ⇥ reaches, as each of Settings' is.
    fn stop(cx: &mut gpui::App) -> gpui::FocusHandle {
        cx.focus_handle().tab_index(0).tab_stop(true)
    }

    /// Where the keyboard is after `row`'s control removes it from the keys.
    fn after_removing(named: Named, row: u64, cx: &mut gpui::TestAppContext) -> Option<u64> {
        let (view, cx) = cx.add_window_view(|_, cx| Controls {
            rows: vec![1, 2, 3],
            named,
            by_line: (0..3).map(|_| stop(cx)).collect(),
            by_row: (1..=3).map(|row| (row, stop(cx))).collect(),
        });
        cx.update(|window, cx| {
            let view = view.read(cx);
            let at = view.rows.iter().position(|&each| each == row).unwrap();
            let focus = match named {
                Named::ByLine => view.by_line[at].clone(),
                Named::ByRow => view.by_row[&row].clone(),
            };
            window.focus(&focus, cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let rows = view.read(cx).rows.clone();
            removing(&row, &rows, named, true, window, cx);
            view.update(cx, |view, cx| {
                view.rows.retain(|&each| each != row);
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let view = view.read(cx);
            let mut lines = view.rows.iter().enumerate();
            lines
                .find(|&(line, each)| match named {
                    Named::ByLine => view.by_line[line].is_focused(window),
                    Named::ByRow => view.by_row[each].is_focused(window),
                })
                .map(|(_, &each)| each)
        })
    }

    /// A row's control removing it from the keys leaves the keyboard on the same control of the
    /// row that took its place, however the list names its controls.
    #[gpui::test]
    fn a_removed_rows_control_hands_the_keyboard_to_the_row_in_its_place(
        cx: &mut gpui::TestAppContext,
    ) {
        for named in [Named::ByLine, Named::ByRow] {
            assert_eq!(
                after_removing(named, 2, cx),
                Some(3),
                "{named:?}: the next row"
            );
            assert_eq!(
                after_removing(named, 3, cx),
                Some(2),
                "{named:?}: the last row's"
            );
        }
    }
}
