//! The favorites an import offers, as tiles (#1673), one part for the first launch's Favorites
//! step and Settings' end of an import: the site's letter on a plate over its name, ticked in the
//! accent at the cards' badge place, which a click unticks. They're four to a row across the view,
//! and land as the cards do, in order of most visits.

use web_time::Instant;

use gpui::{AnyElement, Div, FontWeight, div, prelude::*, px, relative, rgb, svg};

use super::arrival::pose;
use crate::settings_rows::Look;
use crate::tokens::radius;
use crate::tokens::settings::{CHECK_TICK, ROW_X};
use crate::tokens::shell::import::{
    BADGE, BADGE_INSET, CARD_GAP, TILE_HEIGHT, TILE_LETTER, TILE_NAME, TILE_NAME_GAP,
    TILE_NAME_LINE, TILE_PLATE, TILE_UNTICKED, TILES_PER_ROW,
};
use crate::tokens::shell::task::motion::{CHIPS, CHIPS_STAGGER};

/// A badge's change, as ux-spec's Glyph change: the old one fades out and shrinks to 0.6 over
/// `LEAVE`, the new fades in over `COME` and grows from 0.8 over `SWAP`, all from one moment.
const SWAP: f32 = 0.08;
const LEAVE: f32 = 0.07;
const COME: f32 = 0.04;

/// A favorite offered, as its tile shows it.
pub struct Tile {
    /// The site's name, as the favorite's row would be named: "news.example.org".
    pub name: String,
    pub on: bool,
    /// Its place among the tiles, which lands this many staggers after the first.
    pub index: usize,
    /// When the tiles first showed, and when this one was last ticked or unticked.
    pub shown: Instant,
    pub toggled: Option<Instant>,
}

impl Tile {
    /// When it starts landing.
    fn lands(&self) -> Instant {
        self.shown + std::time::Duration::from_secs_f32(CHIPS_STAGGER * self.index as f32)
    }

    /// Whether it still moves at `now`: landing, or its badge changing.
    pub fn moving(&self, now: Instant) -> bool {
        let since = |at: Instant| now.saturating_duration_since(at).as_secs_f32();
        now < self.lands()
            || since(self.lands()) < CHIPS
            || self.toggled.is_some_and(|at| since(at) < SWAP)
    }
}

/// One tile, `settings`' on its group's fill, else on the first launch's ground; unticked, an
/// outline with its letter and name quiet.
pub fn tile(tile: &Tile, look: &Look, settings: bool, still: bool, now: Instant) -> Div {
    let (rise, came) = pose(tile.lands(), now, still);
    // The plate a step further from the ground than the tile, as a control's hover is from it.
    let (fill, plate_fill) = match settings {
        true => (look.control, look.hovered),
        false => (look.surface, look.control),
    };
    let quiet = if tile.on { 1. } else { TILE_UNTICKED };
    let letter = (tile.name.chars().next())
        .map(|first| first.to_uppercase().collect::<String>())
        .unwrap_or_default();
    let plate = div()
        .size(px(TILE_PLATE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .bg(plate_fill)
        .text_size(px(TILE_LETTER))
        .font_weight(FontWeight::SEMIBOLD)
        .child(letter);
    let name = div()
        .max_w_full()
        .px(px(BADGE_INSET))
        .truncate()
        .text_size(px(TILE_NAME))
        .line_height(px(TILE_NAME_LINE))
        .child(tile.name.clone());
    let badge = badge(tile, look, still, now);
    div()
        .relative()
        .top(px(rise))
        .opacity(came)
        .w_full()
        .h(px(TILE_HEIGHT))
        .rounded(px(radius::ROW))
        .border_1()
        .when(tile.on, |tile| tile.bg(fill).border_color(fill))
        .when(!tile.on, |tile| tile.border_color(look.line))
        .text_color(look.ink)
        .child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(TILE_NAME_GAP))
                .opacity(quiet)
                .child(plate)
                .child(name),
        )
        .child(
            div()
                .absolute()
                .top(px(BADGE_INSET))
                .right(px(BADGE_INSET))
                .size(px(BADGE))
                .child(badge),
        )
}

/// The badge: the accent with a tick, or an empty ring; for a moment after a click, the one
/// before leaving as the new comes, each about the badge's middle.
fn badge(tile: &Tile, look: &Look, still: bool, now: Instant) -> Div {
    let elapsed = tile
        .toggled
        .map_or(SWAP, |at| now.saturating_duration_since(at).as_secs_f32());
    let mark = |on: bool, scale: f32, opacity: f32| {
        let size = BADGE * if still { 1. } else { scale };
        let mark = div().size(px(size)).rounded_full().opacity(opacity);
        let mark = match on {
            true => mark
                .bg(look.accent)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path("icons/check.svg")
                        .size(px(size * 0.625))
                        .text_color(rgb(CHECK_TICK)),
                ),
            false => mark.border_1().border_color(look.detail),
        };
        div()
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(mark)
    };
    let left = (elapsed / LEAVE).clamp(0., 1.);
    let come = (elapsed / COME).clamp(0., 1.);
    let grown = (elapsed / SWAP).clamp(0., 1.);
    div()
        .relative()
        .size_full()
        .when(left < 1., |badge| {
            badge.child(mark(!tile.on, 1. - 0.4 * left, 1. - left))
        })
        .child(mark(tile.on, 0.8 + 0.2 * grown, come))
}

/// The tiles in rows of `tiles-per-row`, each a row's share wide less the gap. A single row is
/// centred, under the view's centred heading and button; more rows keep their left edge, the
/// last one's places left empty.
pub fn grid(tiles: Vec<AnyElement>, look: &Look, settings: bool) -> Div {
    let per = TILES_PER_ROW as usize;
    let one_row = tiles.len() <= per;
    let mut rows = Vec::new();
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        // Each tile in a place of its own, half the gap in from each side, so a row's places are
        // its share wide whether it's full or not.
        let places = tiles.by_ref().take(per).map(|tile| {
            div()
                .flex()
                .w(relative(1. / per as f32))
                .px(px(CARD_GAP / 2.))
                .child(tile)
        });
        rows.push(
            div()
                .flex()
                .mx(px(-CARD_GAP / 2.))
                .when(one_row, |row| row.justify_center())
                .children(places),
        );
    }
    let grid = div().flex().flex_col().gap(px(CARD_GAP)).children(rows);
    match settings {
        true => div()
            .p(px(ROW_X))
            .rounded(px(radius::ROW))
            .bg(look.surface)
            .when_some(look.edge, |box_, edge| box_.border_1().border_color(edge))
            .child(grid),
        false => grid,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// The tiles land one stagger after another, and a tile ticked or unticked moves only while
    /// its badge changes, so the offer asks for no frames once it's still.
    #[test]
    fn tiles_move_only_while_they_land_or_their_badge_changes() {
        let shown = Instant::now();
        let at = |seconds: f32| shown + Duration::from_secs_f32(seconds);
        let mut tile = Tile {
            name: "news.example.org".into(),
            on: true,
            index: 3,
            shown,
            toggled: None,
        };
        let lands = CHIPS_STAGGER * 3.;
        assert!(tile.moving(at(lands - 0.01)), "it hadn't landed yet");
        assert!(tile.moving(at(lands + CHIPS / 2.)));
        assert!(
            !tile.moving(at(lands + CHIPS + 0.01)),
            "it moved once it had landed"
        );
        tile.toggled = Some(at(5.));
        assert!(tile.moving(at(5. + SWAP / 2.)));
        assert!(
            !tile.moving(at(5. + SWAP + 0.01)),
            "its badge kept changing"
        );
    }
}
