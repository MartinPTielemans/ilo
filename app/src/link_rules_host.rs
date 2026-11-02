//! The link rules the browser goes by (browser::link_rules), kept in the profile directory as
//! `Link rules`, and the Links tab's calls. The user's alone, so no agent path reads or sets them.

use std::cell::{Cell, RefCell};

use browser::link_rules::{Host, LITTLE, Rules};
use browser::log;
use browser::menu::Look;
use browser::shell::MenuEntry;
use gpui::{App, Pixels, Point, SharedString, Window};

use crate::{bc, ids, name, ns_view};

thread_local! {
    static RULES: RefCell<Rules> = RefCell::new(Rules::default());
    /// The file as it was last read or written, so it's written only when it changes.
    static WRITTEN: RefCell<String> = const { RefCell::new(String::new()) };
    /// Whether the file has been read, once the Spaces it names by place have come back: before
    /// that, writing would lose it.
    static READ: Cell<bool> = const { Cell::new(false) };
}

fn path() -> String {
    format!("{}/Link rules", crate::profile_dir())
}

/// The user's Spaces, in order: the places the file names them by, and the only ones a rule
/// goes to.
fn users_spaces() -> Vec<u64> {
    let users = |space: &u64| name(bc::bc_space_agent, *space).is_empty();
    ids(bc::bc_spaces).into_iter().filter(users).collect()
}

/// Reads the rules, once the first window has brought the Spaces back.
pub fn read() {
    if READ.replace(true) {
        return;
    }
    let text = std::fs::read_to_string(path()).unwrap_or_default();
    let rules = Rules::parse(&text, &users_spaces());
    // check.sh reads this line.
    log!("browser: link rules read {}", rules.rules.len());
    RULES.set(rules);
    WRITTEN.set(text);
}

/// Writes the rules, if they read differently now, as a Space moved or went.
pub fn save() {
    if !READ.get() {
        return;
    }
    let text = RULES.with_borrow(|rules| rules.to_text(&users_spaces()));
    if WRITTEN.with_borrow(|written| *written == text) {
        return;
    }
    if let Err(error) = std::fs::write(path(), &text) {
        log!("couldn't save the link rules: {error}");
    }
    WRITTEN.set(text);
}

/// The user's Space a link to `url` from another app opens in, by the first rule it meets, else
/// everything else's, 0 being the Space in front and `LITTLE` Little ilo.
pub fn route(url: &str) -> u64 {
    let (space, rule) = RULES.with_borrow(|rules| rules.route(url, &users_spaces()));
    if let Some(rule) = rule {
        let place = match space {
            LITTLE => "little ilo".to_string(),
            space => format!("space {space}"),
        };
        // check.sh reads this line.
        log!("browser: link rule {rule} sends {url} to {place}");
    }
    space
}

/// The user's Space a link that would open in a link preview goes to instead, by the first rule it
/// meets, if that's another than `shown`, else 0: everything else's Space is for links from other
/// apps, and a preview already shows its link in the Space shown.
pub fn preview_route(url: &str, shown: u64) -> u64 {
    let routed = RULES.with_borrow(|rules| rules.preview_route(url, &users_spaces(), shown));
    if let Some((space, rule)) = routed {
        // check.sh reads this line.
        log!("browser: link rule {rule} sends {url} to space {space}");
    }
    routed.map_or(0, |(space, _)| space)
}

/// The Links tab's calls.
pub struct CoreLinkRules;

impl Host for CoreLinkRules {
    fn spaces(&self) -> Vec<(u64, SharedString)> {
        (users_spaces().into_iter()).map(|space| (space, name(bc::bc_space_name, space))).collect()
    }

    fn rules(&self) -> Rules {
        RULES.with_borrow(Rules::clone)
    }

    fn set_rules(&self, rules: &Rules) {
        RULES.set(rules.clone());
        save();
    }

    fn show_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        window: &Window,
        picked: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        crate::show_menu(ns_view(window), items, at, false, Look::dark(), picked, cx);
    }
}
