//! Settings › Overview (spec-settings.md › S1): the default browser's notice while the product
//! isn't the default, which stays until it is; Loose ends, what setup left that the app already
//! knows is undone, each put away for good by its ✕; and a tile for six pages, each saying what is
//! true of that page's settings and opening it. Overview reads what the app holds: nothing of an
//! assistant's program runs, and nothing is asked of the OS, because it draws.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, FontWeight, SharedString, Window, div, prelude::*, px, svg};

use crate::agents::Agents;
use crate::agents::ready::Ready;
use crate::agents::runner::Cli;
use crate::product::PRODUCT;
use crate::settings_column::Section;
use crate::settings_rows::{Look, SIGN_IN, assistant_name, button, frame, icon_button, text};
use crate::tokens::settings::overview as form;

/// What setup left, as Overview's heading says it (batch Z19).
pub const LOOSE_ENDS: &str = "Loose ends";
/// What Loose ends are, at the heading's end (the drawing's).
pub const LEFT_FROM_SETUP: &str = "Left from setup. Skip what you don't use.";
/// The notice's second line while the product isn't the default (the drawing's).
pub const ELSEWHERE: &str = "Links from other apps open somewhere else.";
/// The notice's one button, which asks the OS, as General's does.
pub const MAKE_DEFAULT: &str = "Make Default";
/// The file in the profile's folder that keeps which loose ends the user put away: on this
/// computer alone, never synced.
pub const PUT_AWAY_FILE: &str = "Loose ends put away";

/// The notice's name while the product isn't the default (the drawing's).
pub fn not_default() -> String {
    format!("{PRODUCT} isn't your default browser")
}

/// The heading over the tiles (the drawing's).
pub fn set_up() -> String {
    format!("How {PRODUCT} is set up")
}

/// The line under the tiles (the drawing's), with the keys that put the cursor in the search.
pub fn search_note(find: &str) -> String {
    format!("Looking for one setting? Type its name in Search, or press {find}.")
}

/// Whether the default browser's notice shows: from what the app last read of the OS, which
/// drawing never asks again. It stays, with no ✕, until the product is the default.
pub fn notice_shows(default: &crate::default_browser::DefaultBrowser) -> bool {
    !default.is_default()
}

/// A thing setup left that the app already knows is undone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LooseEnd {
    /// An assistant the app last found signed out or missing, while another is signed in, so
    /// threads run without it.
    SignIn(Cli),
}

impl LooseEnd {
    /// What its ✕ keeps, so it's never listed again on this computer.
    pub fn key(self) -> String {
        match self {
            LooseEnd::SignIn(cli) => format!("sign-in {}", cli.id()),
        }
    }

    /// Its name (the drawing's), with the assistant's own.
    pub fn title(self) -> String {
        match self {
            LooseEnd::SignIn(cli) => format!("Sign in to {}", assistant_name(cli)),
        }
    }

    /// Its second line (the drawing's): true only while another assistant runs threads.
    pub fn line(self) -> &'static str {
        match self {
            LooseEnd::SignIn(_) => "Only if you use it. Your threads run fine without it.",
        }
    }

    /// Its one button, as the assistant's row on Models says it.
    pub fn action(self) -> &'static str {
        match self {
            LooseEnd::SignIn(_) => SIGN_IN,
        }
    }
}

/// The loose ends to list: each one the app already knows is undone and the user hasn't put
/// away. It reads what Agents last found and asks nothing: an assistant whose state isn't known
/// is no loose end, and with the inside switch off there are none, as Models then shows names
/// alone.
pub fn loose_ends(agents: &Agents, put_away: &PutAway) -> Vec<LooseEnd> {
    if !agents.threads() {
        return Vec::new();
    }
    let found: Vec<(Cli, Ready)> = Cli::ALL
        .into_iter()
        .map(|cli| (cli, agents.found(cli)))
        .collect();
    let runs = found.iter().any(|&(_, ready)| ready == Ready::SignedIn);
    (found.into_iter())
        .filter(|&(_, ready)| runs && matches!(ready, Ready::SignedOut | Ready::NotFound))
        .map(|(cli, _)| LooseEnd::SignIn(cli))
        .filter(|end| !put_away.has(&end.key()))
        .collect()
}

/// The loose ends the user put away, kept as one key a line in `PUT_AWAY_FILE` in the profile's
/// folder, which nothing else reads and nothing syncs.
#[derive(Debug, Default)]
pub struct PutAway {
    path: Option<PathBuf>,
    keys: BTreeSet<String>,
}

impl PutAway {
    /// What `folder`'s file keeps; none if it has none.
    pub fn load(folder: impl Into<PathBuf>) -> PutAway {
        let path = folder.into().join(PUT_AWAY_FILE);
        let keys = (std::fs::read_to_string(&path).unwrap_or_default().lines())
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(String::from)
            .collect();
        PutAway {
            path: Some(path),
            keys,
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    /// Keeps `key` for good: the user's ✕ alone calls it.
    pub fn put_away(&mut self, key: &str) {
        if !self.keys.insert(key.into()) {
            return;
        }
        if let Some(path) = &self.path {
            let text: String = self.keys.iter().map(|key| format!("{key}\n")).collect();
            if let Err(error) = std::fs::write(path, text) {
                crate::log!("browser: loose ends not kept: {error}");
            }
        }
    }
}

/// What Overview's tiles say, each read from a setting the app already holds. A fact left `None`
/// or false is a line the tile leaves out: a tile says only what's true, in the drawing's words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// Whether the hidden sidebar shows at the window's edge.
    pub edge: bool,
    /// The zoom pages open at, 1 for 100%.
    pub zoom: Option<f64>,
    /// How long a Today tab waits before the archive, in words, while it's archived at all.
    pub archive: Option<String>,
    pub profiles: usize,
    pub spaces: usize,
    /// The level a new thread starts at in the front Space, as Access names it.
    pub level: Option<SharedString>,
    pub ads_blocked: bool,
    /// Whether the browser sends nothing of its own: crash reports and the daily count off.
    pub nothing_sent: bool,
    /// Whether passwords are offered to be saved, and filled, in the passwords' profile.
    pub saved_and_filled: bool,
    /// The logins the passwords' profile holds, once Settings has read them, and its name.
    pub logins: Option<(usize, SharedString)>,
    pub signed_in: bool,
    /// The account's devices, once Settings has read them.
    pub devices: Option<usize>,
}

/// One of Overview's tiles: the page it opens, named as the drawing names it, and its lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub section: Section,
    pub title: &'static str,
    pub glyph: &'static str,
    pub lines: Vec<String>,
}

/// `n` of a thing, one or many.
fn counted(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// The six tiles, in the drawing's two rows of three. The Agents tile leaves the plan's usage out
/// until Models shows it.
pub fn tiles(facts: &Facts) -> [Tile; 6] {
    let line = |on: bool, words: &str| on.then(|| words.to_string());
    let tile = |section, title, glyph, lines: [Option<String>; 2]| Tile {
        section,
        title,
        glyph,
        lines: lines.into_iter().flatten().collect(),
    };
    let places = format!(
        "{}, {}",
        counted(facts.profiles, "profile", "profiles"),
        counted(facts.spaces, "Space", "Spaces")
    );
    [
        tile(
            Section::Window,
            "Window",
            Section::Window.glyph(),
            [
                line(facts.edge, "Sidebar shows at the edge"),
                (facts.zoom).map(|zoom| format!("Pages at {:.0}%", zoom * 100.)),
            ],
        ),
        tile(
            Section::TabsAndSpaces,
            "Tabs & Spaces",
            Section::TabsAndSpaces.glyph(),
            [
                (facts.archive.as_ref()).map(|after| format!("Today tabs archive after {after}")),
                (facts.profiles > 0).then_some(places),
            ],
        ),
        tile(
            Section::Threads,
            "Agents",
            "icons/sparkles.svg",
            [
                None,
                (facts.level.as_ref()).map(|level| format!("New threads start at {level}")),
            ],
        ),
        tile(
            Section::Privacy,
            "Privacy",
            Section::Privacy.glyph(),
            [
                line(facts.ads_blocked, "Ads and trackers blocked"),
                line(facts.nothing_sent, "Nothing is sent to us"),
            ],
        ),
        tile(
            Section::Passwords,
            "Passwords",
            Section::Passwords.glyph(),
            [
                line(facts.saved_and_filled, "Saved and filled"),
                (facts.logins.as_ref()).map(|(count, profile)| {
                    format!("{} in {profile}", counted(*count, "login", "logins"))
                }),
            ],
        ),
        tile(
            Section::SyncImport,
            "Sync",
            Section::SyncImport.glyph(),
            [
                line(facts.signed_in, "Signed in"),
                (facts.devices.filter(|_| facts.signed_in)).map(|devices| {
                    format!(
                        "{}, end-to-end encrypted",
                        counted(devices, "device", "devices")
                    )
                }),
            ],
        ),
    ]
}

/// A loose end's row: the empty ring, its name and line, its one button and its ✕. A press on
/// the button calls `act`, on the ✕ `put_away`.
pub fn loose_end_row(
    look: &Look,
    index: usize,
    end: LooseEnd,
    a11y: bool,
    act: Rc<dyn Fn(&mut Window, &mut App)>,
    put_away: Rc<dyn Fn(&mut Window, &mut App)>,
) -> gpui::Div {
    let ring = svg()
        .flex_none()
        .path("icons/circle.svg")
        .size(px(form::LOOSE_RING))
        .mr(px(form::LOOSE_GAP))
        .text_color(look.detail);
    let action = button(look, ("loose-end", index), end.action())
        .when(a11y, |action| {
            crate::settings_rows::named_button(action, end.title())
        })
        .on_click(move |_, window, cx| act(window, cx));
    let close = icon_button(look, ("loose-end-away", index), "icons/x.svg")
        .size(px(form::CLOSE_BOX))
        .when(a11y, |close| {
            crate::settings_rows::named_button(close, format!("Put away {}", end.title()))
        })
        .on_click(move |_, window, cx| put_away(window, cx));
    frame()
        .min_h(px(form::LOOSE))
        .pl(px(form::LOOSE_LEFT))
        .pr(px(form::LOOSE_RIGHT))
        .gap(px(form::LOOSE_GAP))
        .child(ring)
        .child(text(look, end.title(), Some(end.line().into())))
        .child(action)
        .child(close)
}

/// A tile: its glyph and page's name, then its lines. A click anywhere on it calls `open`;
/// nothing is changed from it.
pub fn tile_card(
    look: &Look,
    tile: &Tile,
    a11y: bool,
    open: Rc<dyn Fn(&mut Window, &mut App)>,
) -> gpui::Stateful<gpui::Div> {
    let head = div()
        .flex()
        .items_center()
        .gap(px(form::TILE_HEAD_GAP))
        .child(
            svg()
                .flex_none()
                .path(tile.glyph)
                .size(px(form::TILE_GLYPH))
                .text_color(look.glyph),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(form::TILE_TITLE))
                .line_height(px(form::TILE_TITLE_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(look.ink)
                .child(tile.title),
        );
    let lines = (tile.lines.iter()).map(|line| div().truncate().child(line.clone()));
    let title = tile.title;
    div()
        .id(("overview-tile", tile.section as usize))
        .when(a11y, |card| crate::settings_rows::named_button(card, title))
        .flex_1()
        .min_w_0()
        .h(px(form::TILE))
        .py(px(form::TILE_Y))
        .px(px(form::TILE_X))
        .rounded(px(form::TILE_RADIUS))
        .bg(look.surface)
        .when_some(look.edge, |card, edge| card.border_1().border_color(edge))
        .flex()
        .flex_col()
        .gap(px(form::TILE_GAP))
        .cursor_pointer()
        .child(head)
        .child(
            div()
                .text_size(px(form::TILE_BODY))
                .line_height(px(form::TILE_BODY_LINE))
                .text_color(look.detail)
                .children(lines),
        )
        .on_click(move |_, window, cx| open(window, cx))
}

/// Two rows of three tiles, `TILE_GAP` apart.
pub fn tiles_grid(cards: Vec<gpui::Stateful<gpui::Div>>) -> gpui::Div {
    let mut cards = cards.into_iter();
    let row = |cards: Vec<gpui::Stateful<gpui::Div>>| {
        div().flex().gap(px(form::TILE_GAP)).children(cards)
    };
    let first: Vec<_> = cards.by_ref().take(3).collect();
    let second: Vec<_> = cards.collect();
    div()
        .flex()
        .flex_col()
        .gap(px(form::TILE_GAP))
        .child(row(first))
        .child(row(second))
}

/// The line under the tiles.
pub fn note(look: &Look, find: &str) -> gpui::Div {
    div()
        .pt(px(form::NOTE_TOP))
        .px(px(form::NOTE_X))
        .text_size(px(form::TILE_BODY))
        .line_height(px(form::TILE_BODY_LINE))
        .text_color(look.detail)
        .child(search_note(find))
}

#[cfg(test)]
mod tests;
