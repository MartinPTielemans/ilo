//! Spaces dragged to a place of their own: a dot or icon along the switcher, and a card along the
//! Library's Spaces by its move handle (#1580). Arc's motion isn't recorded, so it's a tab row's
//! (docs/ux-spec.md › Tab rows › Reorder): once the pointer has taken it, a ghost of it follows the
//! pointer within a frame, over what it passes, while it stays in its place and nothing else moves;
//! a line marks the gap it would go into, unless that's where it is; and let go there, the core
//! moves it (bc_space_move), the others gliding to their places on Row motion. ⌘Z puts it back.

use std::time::Instant;

use gpui::{AnyElement, Context, Render, WeakEntity, Window, div, prelude::*, px, rgba};

use super::{
    Shell, row_spring,
    undo::{Edit, What},
};
use crate::tokens::shell::{DROP, DROP_LINE, DROP_RING};

/// A Space dragged by its dot in the switcher, or by its card's handle in the Library.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DraggedSpace {
    pub space: u64,
    pub card: bool,
}

/// Where the Space dragged would go: its index among the Spaces once it's out of its place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpaceDrop {
    pub space: u64,
    pub card: bool,
    pub to: usize,
}

/// The Spaces' order before it last changed, and when, as each glides from its old place.
pub(super) struct SpaceGlide {
    from: Vec<u64>,
    start: Instant,
}

/// The index the item `from` of `count` would take with the pointer at `x`: past each other item
/// whose middle, `middle(i)` along, is before the pointer.
pub(super) fn drop_index(
    count: usize,
    from: usize,
    x: f32,
    middle: impl Fn(usize) -> f32,
) -> usize {
    (0..count).filter(|&i| i != from && middle(i) < x).count()
}

/// The drop line stood on its end, for a gap between dots or cards: Arc's ring at its top and its
/// line down from it, `height` tall, centred on the gap by its caller.
pub(super) fn standing_drop_line(height: f32) -> gpui::Div {
    let ring = div()
        .flex_none()
        .size(px(DROP_RING))
        .rounded_full()
        .border(px(DROP_LINE))
        .border_color(rgba(DROP));
    div()
        .w(px(DROP_RING))
        .h(px(height))
        .flex()
        .flex_col()
        .items_center()
        .child(ring)
        .child(div().flex_1().w(px(DROP_LINE)).bg(rgba(DROP)))
}

impl Shell {
    /// Moves `space` to `to` among the user's Spaces, as the core keeps them, and says whether it
    /// did. Each window hears the new order, which its dots and cards glide into.
    pub(super) fn reorder_space(&mut self, space: u64, to: usize, cx: &mut Context<Self>) -> bool {
        self.browser.move_space(space, to, cx)
    }

    /// The drag's ghost: it starts the drag over, with no gap marked yet.
    pub(super) fn space_ghost(
        &mut self,
        dragged: DraggedSpace,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<SpaceGhost> {
        self.space_drop = None;
        let shell = cx.weak_entity();
        let ghost = cx.new(|_| SpaceGhost { shell, dragged });
        self.space_ghost = Some(ghost.downgrade());
        ghost
    }

    /// Whether a Space is being dragged now: its ghost lives as long as the drag does.
    pub(super) fn dragging_space(&self) -> bool {
        (self.space_ghost.as_ref()).is_some_and(|ghost| ghost.upgrade().is_some())
    }

    /// The dragged Space would go where the pointer is, `x` along what holds the dots or cards, the
    /// `i`th of them with its middle `middle(i)` along.
    pub(super) fn space_dragged_to(
        &mut self,
        dragged: DraggedSpace,
        x: f32,
        middle: impl Fn(usize) -> f32,
        cx: &mut Context<Self>,
    ) {
        let Some(from) = self.space_index(dragged.space) else {
            return;
        };
        let to = drop_index(self.spaces.len(), from, x, middle);
        let drop = Some(SpaceDrop {
            space: dragged.space,
            card: dragged.card,
            to,
        });
        if self.space_drop != drop {
            self.space_drop = drop;
            cx.notify();
        }
    }

    /// The gap a dot's drag (or with `card`, a card's) would drop its Space into, while it's
    /// dragged: the index, in the order shown, of the Space it would go before, the count for
    /// after the last, or None where it already is.
    pub(super) fn space_gap(&self, card: bool) -> Option<usize> {
        let drop = self
            .space_drop
            .filter(|drop| drop.card == card && self.dragging_space())?;
        let from = self.space_index(drop.space)?;
        (drop.to != from).then_some(if drop.to < from { drop.to } else { drop.to + 1 })
    }

    /// The Space dragged was let go: it moves where its gap was, for ⌘Z to put back.
    pub(super) fn drop_space(&mut self, dragged: DraggedSpace, cx: &mut Context<Self>) {
        let drop = self
            .space_drop
            .take()
            .filter(|drop| drop.space == dragged.space && drop.card == dragged.card);
        cx.notify();
        let Some(drop) = drop else {
            return;
        };
        let Some(from) = self.space_index(drop.space).filter(|&from| from != drop.to) else {
            return;
        };
        // check.sh reads this line.
        log!(
            "browser: command move-space via {}",
            if drop.card { "library" } else { "sidebar" }
        );
        if self.reorder_space(drop.space, drop.to, cx) {
            let undo = Edit::MoveSpace {
                space: drop.space,
                to: from,
            };
            self.keep(What::MOVE_SPACE, Some(undo), cx);
        }
    }

    fn space_index(&self, space: u64) -> Option<usize> {
        self.spaces.iter().position(|each| each.id == space)
    }

    /// The Spaces came in another order, from a drop here or in another window, ⌘Z, sync or an
    /// agent: each glides from where it was, as rows do on Row motion.
    pub(super) fn spaces_reordered(&mut self, before: Vec<u64>, cx: &mut Context<Self>) {
        let after: Vec<u64> = self.spaces.iter().map(|space| space.id).collect();
        let same = before.len() == after.len() && after.iter().all(|id| before.contains(id));
        if same && before != after {
            self.space_glide = Some(SpaceGlide {
                from: before,
                start: crate::motion::start(cx),
            });
        }
    }

    /// How far `space`'s dot or card is from its place as the Spaces glide into their new order:
    /// from its old place, the `i`th at `place(i)`, closing on Row motion. Asks for the next frame
    /// while it moves.
    pub(super) fn space_glide(
        &self,
        space: u64,
        place: impl Fn(usize) -> f32,
        cx: &mut Context<Self>,
    ) -> f32 {
        let Some(glide) = &self.space_glide else {
            return 0.;
        };
        let Some(done) = row_spring(glide.start.elapsed().as_secs_f32()) else {
            return 0.;
        };
        let (Some(was), Some(is)) = (
            glide.from.iter().position(|id| *id == space),
            self.space_index(space),
        ) else {
            return 0.;
        };
        cx.notify();
        (place(was) - place(is)) * (1. - done)
    }
}

/// The drag's view, which draws the dot or the card from the Shell as it is each frame.
pub(super) struct SpaceGhost {
    shell: WeakEntity<Shell>,
    dragged: DraggedSpace,
}

impl SpaceGhost {
    /// A ghost of nothing, for a drag that outlived its window.
    pub(super) fn gone(dragged: DraggedSpace) -> Self {
        SpaceGhost {
            shell: WeakEntity::new_invalid(),
            dragged,
        }
    }
}

impl Render for SpaceGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dragged = self.dragged;
        let ghost: Option<AnyElement> = self.shell.upgrade().and_then(|shell| {
            let shell = shell.read(cx);
            match dragged.card {
                true => shell.card_ghost_face(dragged.space),
                false => shell.dot_ghost(dragged.space),
            }
        });
        div().children(ghost)
    }
}

#[cfg(test)]
mod tests;
