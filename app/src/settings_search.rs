//! Settings' search (docs/ux-spec.md › Settings › Search): the field at the column's top, what it
//! finds among the rows, and the found rows, live, under their sections' names on the card.

use std::rc::Rc;

use gpui::{
    App, Entity, Focusable, FontWeight, MouseButton, Rgba, SharedString, Window, actions, div,
    prelude::*, px, svg,
};

use crate::settings_column::Section;
use crate::settings_rows::{Look, group};
use crate::shell;
use crate::text_input::TextInput;
use crate::tokens::settings::{
    HEADING_BOTTOM, HEADING_TOP, PAGE_WIDTH, SEARCH_CLEAR, SEARCH_CLEAR_FILL, SEARCH_CLEAR_GLYPH,
    SEARCH_HINT, TEXT_X,
};
use crate::tokens::shell::PLATE_INSET;
use crate::tokens::shell::library::{
    LIBRARY_PLATE, SEARCH_GLYPH, SEARCH_GLYPH_X, SEARCH_HEIGHT, SEARCH_PLACEHOLDER, SEARCH_RADIUS,
    SEARCH_TEXT_X,
};
use crate::tokens::text;

#[cfg(test)]
mod tests;

actions!(
    settings_search,
    [
        /// ⌘F, Ctrl+F off the Mac: the keyboard goes to the field.
        Search,
        /// Esc: the field empties and the section shows again.
        EndSearch
    ]
);

/// ⌘F and Esc, while Settings has the keyboard. A field or view inside it that takes Esc itself
/// keeps it, as the import does.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        shell::view_binding("cmd-f", Box::new(Search), "Settings", cx),
        gpui::KeyBinding::new("escape", EndSearch, Some("Settings")),
    ]);
}

/// A row the search reads: where it is, its name, its line, and the keys its chip shows.
#[derive(Clone, Debug)]
pub struct Entry {
    pub section: Section,
    /// The feature whose own setting it is, which the results name after the section.
    pub under: Option<SharedString>,
    pub name: SharedString,
    pub line: Option<SharedString>,
    pub keys: Option<String>,
}

/// The names a row is also known by, in the words someone coming from another browser uses.
pub fn also(name: &str) -> &'static [&'static str] {
    match name {
        "Peek" => &["link preview"],
        "Mini Player" => &["picture in picture", "pip"],
        concat!("Little ", env!("APP_NAME")) => &["little arc", "small window"],
        "Block ads and trackers" => &["ad blocker", "content blocker"],
        "Reopen your windows and tabs" => &["restore session"],
        _ => &[],
    }
}

/// The entries `typed` finds, in their order: those whose name, other names or keys have its words
/// at the starts of words, in any order. Only when none does are the rows' lines read, so a word
/// a setting is named by finds that setting and not every line that mentions it.
pub fn find(typed: &str, entries: &[Entry]) -> Vec<usize> {
    if typed.trim().is_empty() {
        return Vec::new();
    }
    let named = |entry: &Entry| {
        shell::shortcut_found(typed, &entry.name, entry.keys.as_deref())
            || also(&entry.name)
                .iter()
                .any(|other| shell::shortcut_found(typed, other, None))
    };
    let found: Vec<usize> = (0..entries.len())
        .filter(|&index| named(&entries[index]))
        .collect();
    if !found.is_empty() {
        return found;
    }
    let lined = |entry: &Entry| {
        (entry.line.as_deref()).is_some_and(|line| shell::shortcut_found(typed, line, None))
    };
    (0..entries.len())
        .filter(|&index| lined(&entries[index]))
        .collect()
}

/// The sections that hold any of `found`, which the column keeps lit.
pub fn sections(found: &[&Entry]) -> Vec<Section> {
    let mut sections: Vec<Section> = found.iter().map(|entry| entry.section).collect();
    sections.dedup();
    sections
}

/// What a click on a found group's section name does: shows the section at that group's first row.
pub type Show = Rc<dyn Fn(Section, SharedString, &mut Window, &mut App)>;

/// The found rows in place of the section's page: each run of them from one section, or one
/// feature in it, under the section's name, which shows the section at the row. "Nothing found"
/// when there are none. `drawn` puts in each name where check.sh can click it.
pub fn results(
    look: &Look,
    found: Vec<(Entry, gpui::Div)>,
    show: Show,
    drawn: impl Fn(String) -> gpui::Canvas<()>,
) -> gpui::Div {
    let page = div().w_full().max_w(px(PAGE_WIDTH)).flex().flex_col();
    if found.is_empty() {
        let none = div()
            .relative()
            .px(px(TEXT_X))
            .text_color(look.detail)
            .child("Nothing found")
            .child(drawn("nothing found".into()));
        return page.child(none);
    }
    let mut runs: Vec<(Entry, Vec<gpui::Div>)> = Vec::new();
    for (entry, row) in found {
        match runs.last_mut() {
            Some((first, rows)) if first.section == entry.section && first.under == entry.under => {
                rows.push(row)
            }
            _ => runs.push((entry, vec![row])),
        }
    }
    let (heading, detail, ink) = (look.heading, look.detail, look.ink);
    let runs = runs.into_iter().enumerate().map(|(index, (first, rows))| {
        let (section, row) = (first.section, first.name.clone());
        let show = show.clone();
        let under = first.under.clone().map(|under| {
            div()
                .text_color(detail)
                .font_weight(FontWeight::NORMAL)
                .child(format!(" › {under}"))
        });
        let name = div()
            .id(SharedString::from(format!("found-{index}")))
            .relative()
            .flex()
            .px(px(TEXT_X))
            .mb(px(HEADING_BOTTOM))
            .font_weight(FontWeight::MEDIUM)
            .text_color(heading)
            .cursor_pointer()
            .hover(move |name| name.text_color(ink))
            .child(div().relative().child(section.name()).child(drawn(format!(
                "found {} {}",
                section.name(),
                first.name
            ))))
            .children(under)
            .on_click(move |_, window, cx| show(section, row.clone(), window, cx));
        div()
            .when(index > 0, |run| run.mt(px(HEADING_TOP)))
            .child(name)
            .child(group(look, rows))
    });
    page.children(runs)
}

/// The field at the column's top, as the Library's: 36 tall at the row radius, 8 in, its glyph
/// then the text, and at its end ⌘F until typing starts, then ✕, which empties it.
pub fn field(
    input: &Entity<TextInput>,
    base: Rgba,
    drawn: gpui::Canvas<()>,
    clear: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui::Div {
    let typed = !input.read(cx).text().is_empty();
    let focus = input.read(cx).focus_handle(cx);
    let end = match typed {
        false => div()
            .flex_none()
            .ml(px(8.))
            .text_size(px(text::SMALL))
            .text_color(base.opacity(SEARCH_HINT))
            .child(shell::keystroke_label("cmd-f"))
            .into_any_element(),
        true => div()
            .id("clear-search")
            .flex_none()
            .ml(px(8.))
            .size(px(SEARCH_CLEAR))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(base.opacity(SEARCH_CLEAR_FILL))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| clear(window, cx))
            .child(
                svg()
                    .path("icons/x.svg")
                    .size(px(SEARCH_CLEAR_GLYPH))
                    .text_color(base.opacity(0.9)),
            )
            .into_any_element(),
    };
    div()
        .relative()
        .flex_none()
        .h(px(SEARCH_HEIGHT))
        .mx(px(PLATE_INSET))
        .pl(px(SEARCH_TEXT_X))
        .pr(px((SEARCH_HEIGHT - SEARCH_CLEAR) / 2.))
        .flex()
        .items_center()
        .rounded(px(SEARCH_RADIUS))
        .bg(base.opacity(LIBRARY_PLATE))
        .text_size(px(text::BODY))
        // A press here is the field's, not the window's move.
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            window.focus(&focus, cx);
        })
        .child(
            svg()
                .path("icons/search.svg")
                .absolute()
                .left(px(SEARCH_GLYPH_X))
                .top(px((SEARCH_HEIGHT - SEARCH_GLYPH) / 2.))
                .size(px(SEARCH_GLYPH))
                .text_color(base.opacity(SEARCH_PLACEHOLDER)),
        )
        .child(div().flex_1().min_w_0().child(input.clone()))
        .child(end)
        .child(drawn)
}

/// Esc's part: empties the field, so the section shows again. False when it was empty, for Esc to
/// go on to whatever else takes it.
pub fn end(input: &Entity<TextInput>, window: &mut Window, cx: &mut App) -> bool {
    if input.read(cx).text().is_empty() {
        return false;
    }
    input.update(cx, |input, cx| input.reset("".into(), window, cx));
    true
}
