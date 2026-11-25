//! What the log says of a tab's title (#3094). A Board's name is the user's own writing and stays
//! out of every log line, so a Board's tab is written as "(a Board)". The core reports a tab's
//! title and its page apart, in either order, so a title that comes before the tab's page is held
//! until the page says what the tab is: never written first and corrected after. A renamed tab
//! keeps its name when it leaves a Board for a page, so the titles it had as a Board stay out of
//! the log there too. A page's own titles, a renamed one's among them, are written as they are,
//! but for a sign-in's page's, which can be its address while it loads (#4100).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::agents::sign_in::logged;

/// What a line says in place of a Board's title.
pub const A_BOARD: &str = "(a Board)";

#[derive(Default)]
struct Tab {
    /// Whether its page is a Board's, once the core has said where its page is.
    board: Option<bool>,
    /// The title that came before the page did, written once it has.
    held: Option<String>,
    /// The titles it had as a Board, which it may keep as its name on a page.
    as_board: HashSet<String>,
}

impl Tab {
    /// What a line says for `title`, or nothing while it isn't known what the tab is.
    fn said(&mut self, title: &str) -> Option<String> {
        match self.board? {
            true => {
                self.as_board.insert(title.to_string());
                Some(A_BOARD.to_string())
            }
            false if self.as_board.contains(title) => Some(A_BOARD.to_string()),
            false => Some(title.to_string()),
        }
    }
}

thread_local! {
    static TABS: RefCell<HashMap<u64, Tab>> = RefCell::default();
    #[cfg(test)]
    static WRITTEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// check.sh reads these lines.
fn write(line: String) {
    #[cfg(test)]
    WRITTEN.with_borrow_mut(|written| written.push(line.clone()));
    crate::log!("{line}");
}

/// The core says `tab`'s page is at `url`, which tells whether it's a Board's; a title held for it
/// is written now. An empty `url` tells nothing yet.
pub fn page(tab: u64, url: &str) {
    if url.is_empty() {
        return;
    }
    let said = TABS.with_borrow_mut(|tabs| {
        let each = tabs.entry(tab).or_default();
        each.board = Some(crate::boards::id_in(url).is_some());
        let held = each.held.take()?;
        each.said(&held)
    });
    if let Some(title) = said {
        write(format!("browser: title {tab} {}", logged(tab, &title)));
    }
}

/// The core says `tab`'s title is `title`: its line, or held until its page is known.
pub fn title(tab: u64, title: &str) {
    let said = TABS.with_borrow_mut(|tabs| {
        let each = tabs.entry(tab).or_default();
        if each.board.is_none() {
            each.held = Some(title.to_string());
            return None;
        }
        each.said(title)
    });
    if let Some(title) = said {
        write(format!("browser: title {tab} {}", logged(tab, &title)));
    }
}

/// The window's title is now `title`, the shown `tab`'s, or its Space's name with none (0): its
/// line by the same rule, and none while it isn't known what the tab is.
pub fn window(tab: u64, title: &str) {
    let said = match tab {
        0 => Some(title.to_string()),
        tab => TABS.with_borrow_mut(|tabs| tabs.entry(tab).or_default().said(title)),
    };
    if let Some(title) = said {
        write(format!("browser: window title {}", logged(tab, &title)));
    }
}

/// `tab` closed: what was known of it goes.
pub fn closed(tab: u64) {
    TABS.with_borrow_mut(|tabs| tabs.remove(&tab));
}

/// The lines written since the last call, for the tests.
#[cfg(test)]
pub fn take_written() -> Vec<String> {
    WRITTEN.with_borrow_mut(std::mem::take)
}
