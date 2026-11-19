//! The find bar's count of what the page found, and what it says to a screen reader. The bar
//! itself is the app's, a window over the page.

use gpui::{
    Div, FontWeight, Stateful,
    accesskit::{Live, Role},
    div,
    prelude::*,
    px, rgb, rgba,
};

use crate::tokens::find_bar::{COUNT, COUNT_GAP, NOT_FOUND};
use crate::tokens::text;

/// A tab's find in page, as the core reports it.
#[derive(Clone, Default)]
pub struct Find {
    pub open: bool,
    pub text: String,
    /// -1 until the page has looked.
    pub matches: i32,
    /// From 1, or 0 for none.
    pub selected: i32,
}

impl Find {
    /// What the page found for the text, once it has looked: which match of how many, or none,
    /// while the find is open.
    pub fn found(&self) -> Option<(i32, i32)> {
        let looked = self.open && !self.text.is_empty() && self.matches >= 0;
        looked.then_some((self.selected, self.matches))
    }
}

/// Arc's count for `found`, "1/12", with the bar's previous and next, or "NOT FOUND" in their
/// place.
pub fn count(found: Option<(i32, i32)>) -> String {
    match found {
        Some((_, 0)) => "NOT FOUND".to_string(),
        Some((selected, matches)) => format!("{selected}/{matches}"),
        None => String::new(),
    }
}

/// The count for `found` as the bar draws it, and, while `a11y`, as a status a screen reader
/// reads.
pub fn status(found: Option<(i32, i32)>, a11y: bool) -> Stateful<Div> {
    // What it says, said as it changes after what's typed, wherever VoiceOver's cursor is:
    // "1 of 12" rather than its slash, and "not found" rather than its capitals.
    let said = match found {
        Some((_, 0)) => "Not found".to_string(),
        Some((selected, matches)) => format!("{selected} of {matches}"),
        None => String::new(),
    };
    // The Mac's AccessKit announces a live region's value, and Windows' and Linux's its name,
    // so the name has the count too (#3236).
    let name = match said.is_empty() {
        true => "Matches".to_string(),
        false => format!("Matches, {said}"),
    };
    // Live only while there's a count to say: a live region is said as it's added and as its
    // name changes, and the bar is drawn hidden at launch and for a tab whose find is closed,
    // where Orca said "Matches" with no find bar open (#3499).
    let live = !said.is_empty();
    let not_found = matches!(found, Some((_, 0)));
    div()
        .id("matches")
        .when(a11y, |status| {
            status
                .role(Role::Status)
                .aria_label(name)
                .aria_value(said)
                .when(live, |status| {
                    status.a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(Live::Polite)
                    })
                })
        })
        .flex_none()
        .mr(px(COUNT_GAP))
        .text_size(px(text::SMALL))
        .when(not_found, |status| {
            status
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(NOT_FOUND))
        })
        .when(!not_found, |status| status.text_color(rgba(COUNT)))
        .child(count(found))
}

#[cfg(test)]
mod tests;
