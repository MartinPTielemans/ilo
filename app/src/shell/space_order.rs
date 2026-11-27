//! Spaces reordered by dragging a card by its handle along the Library's Spaces, as a sortable
//! row (#4452): the card itself follows the pointer along x, the cards it passes spring one cell
//! aside, so the open slot is where it lands, and the release settles it there and tells the
//! browser once. GPUI holds the drag, as it holds a Library card's tab, but draws nothing for it.

use std::collections::HashMap;
use std::time::Instant;

use gpui::{
    App, Bounds, Context, DispatchPhase, DragMoveEvent, Entity, MouseButton, MouseUpEvent, Pixels,
    Point, Render, WeakEntity, Window, canvas, div, prelude::*,
};

use super::{Curve, Edit, Shell, What};
use crate::tokens::shell::library::{CARD_GAP, CARD_WIDTH, SPACES_LEFT};

/// A Space's card handle pressed and dragged, as GPUI holds it. Which Space is the Shell's to
/// keep, in its SpaceDrag.
#[derive(Clone, Copy)]
pub(super) struct DraggedSpace;

/// A card being dragged.
pub(super) struct SpaceDrag {
    space: u64,
    /// The view GPUI holds for the drag, which lives as long as it: a drag let go anywhere ends
    /// with it.
    ghost: WeakEntity<SpaceGhost>,
    /// How far right of its left the pointer holds the card, and how wide that is.
    grab: f32,
    width: f32,
    /// Where the pointer was last, along the row from its left.
    pointer: Option<f32>,
    /// The gap among the others the card's middle is over, None before it has moved.
    slot: Option<usize>,
}

/// The cards still gliding to their places: how far right of its place each one started, and when.
pub(super) type SpaceSlides = HashMap<u64, (f32, Instant)>;

impl Shell {
    /// The card dragged, while GPUI still holds it.
    pub(super) fn space_drag(&self) -> Option<&SpaceDrag> {
        self.space_drag
            .as_ref()
            .filter(|drag| drag.ghost.upgrade().is_some())
    }

    /// Whether `space`'s card is being dragged.
    #[cfg(test)]
    pub(super) fn space_dragged(&self, space: u64) -> bool {
        self.space_drag().is_some_and(|drag| drag.space == space)
    }

    /// Whether `space`'s card is drawn over the others: while it's dragged, and while it settles.
    pub(super) fn space_lifted(&self, space: u64) -> bool {
        self.space_lifted == Some(space)
    }

    /// Starts the drag of `space`'s card, `width` wide and held `grab` from its left.
    pub(super) fn start_space_drag(
        shell: &WeakEntity<Shell>,
        space: u64,
        width: f32,
        grab: f32,
        cx: &mut gpui::App,
    ) -> Entity<SpaceGhost> {
        let ghost = cx.new(|_| SpaceGhost);
        let weak = ghost.downgrade();
        shell
            .update(cx, |shell, cx| {
                // check.sh waits for this line.
                log!("browser: drag space {space}");
                shell.space_drag = Some(SpaceDrag {
                    space,
                    ghost: weak,
                    grab,
                    width,
                    pointer: None,
                    slot: None,
                });
                shell.space_lifted = Some(space);
                cx.notify();
            })
            .ok();
        ghost
    }

    /// Where the pointer at `position` is along the row of cards, from the row's left, with the
    /// Spaces section at `bounds`, and whether it's over the section. The row is scrolled across
    /// the section.
    fn along_at(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> (f32, bool) {
        let x = f32::from(position.x - bounds.left());
        (x + self.cards_at(), bounds.contains(&position))
    }

    /// The cells of the row of cards, one a Space in order, and where the first starts from the
    /// row's left: a card and a gap each, from half a gap before the first, so an edge between two
    /// is the middle of the gap.
    fn cells(&self) -> (Vec<f32>, f32) {
        (
            vec![CARD_WIDTH + CARD_GAP; self.spaces.len()],
            SPACES_LEFT - CARD_GAP / 2.,
        )
    }

    /// The card dragged's middle along the row with the pointer at `x`.
    fn held_middle(drag: &SpaceDrag, x: f32) -> f32 {
        x - drag.grab + drag.width / 2.
    }

    /// How far right of its place the card dragged is drawn with its middle at `middle`.
    fn held_offset(&self, space: u64, middle: f32) -> f32 {
        let (cells, left) = self.cells();
        let Some(index) = self.spaces.iter().position(|each| each.id == space) else {
            return 0.;
        };
        middle - (left + cells[..index].iter().sum::<f32>() + cells[index] / 2.)
    }

    /// How far each of the others goes aside for the card dragged from the `from`th cell to be in
    /// the open slot `gap` among them: one cell for each it has passed.
    fn shifts_for(&self, from: usize, gap: usize) -> HashMap<u64, f32> {
        let (cells, _) = self.cells();
        let pitch = cells.first().copied().unwrap_or(0.);
        let mut shifts = HashMap::new();
        let others = self.spaces.iter().enumerate().filter(|&(at, _)| at != from);
        for (other, (at, space)) in others.enumerate() {
            let now = if other < gap { other } else { other + 1 };
            if at != now {
                shifts.insert(space.id, (now as f32 - at as f32) * pitch);
            }
        }
        shifts
    }

    /// Sends every card to the shift in `shifts`, none for the ones left out, from where it's drawn:
    /// on a spring that goes on from there if it was going, and `held` is where the card just let
    /// go is drawn, which the pointer was holding.
    fn shift_spaces(&mut self, shifts: HashMap<u64, f32>, held: Option<(u64, f32)>, cx: &App) {
        let now = crate::motion::start(cx);
        for space in self.spaces.iter().map(|space| space.id).collect::<Vec<_>>() {
            let to = shifts.get(&space).copied().unwrap_or(0.);
            let drawn = match held {
                Some((held, offset)) if held == space => offset,
                _ => self.space_shift(space),
            };
            if drawn != to {
                self.space_slides.insert(space, (drawn - to, now));
            }
        }
        self.space_shifts = shifts;
    }

    /// How far right of its place `space`'s card is drawn: with the pointer if it's the one
    /// dragged, else where it's been sent, and the way it still has to go.
    pub(super) fn space_shift(&self, space: u64) -> f32 {
        if let Some(drag) = self.space_drag().filter(|drag| drag.space == space)
            && let Some(x) = drag.pointer
        {
            return self.held_offset(space, Self::held_middle(drag, x));
        }
        self.space_shifts.get(&space).copied().unwrap_or(0.) + self.space_slide(space)
    }

    /// The pointer holding a card moved over the window: the card follows it along x, and the
    /// others make room where its middle has crossed theirs, along the section at `bounds`.
    pub(super) fn drag_space_over(
        &mut self,
        event: &DragMoveEvent<DraggedSpace>,
        cx: &mut Context<Self>,
    ) {
        let (x, over) = self.along_at(event.event.position, event.bounds);
        let Some(drag) = self.space_drag() else {
            return;
        };
        let space = drag.space;
        let middle = Self::held_middle(drag, x);
        let slot = drag.slot;
        let from = self.spaces.iter().position(|each| each.id == space);
        if let Some(drag) = self.space_drag.as_mut() {
            drag.pointer = Some(x);
        }
        if let (true, Some(from)) = (over, from) {
            let (cells, left) = self.cells();
            let gap = gap_at(&cells, left, from, middle);
            if slot.unwrap_or(from) != gap {
                let shifts = self.shifts_for(from, gap);
                self.shift_spaces(shifts, None, cx);
            }
            if let Some(drag) = self.space_drag.as_mut() {
                drag.slot = Some(gap);
            }
        }
        cx.notify();
    }

    /// The pointer let go at `position`, with the section at `bounds`: the card settles into the
    /// open slot and the browser is told once, if that's not where it was. Let go off the section,
    /// or back in its place, everything goes back and nothing is told. ⌘Z puts it back.
    pub(super) fn drop_space(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.space_drag() else {
            return;
        };
        let (x, over) = self.along_at(position, bounds);
        let held = (
            drag.space,
            self.held_offset(drag.space, Self::held_middle(drag, x)),
        );
        let middle = Self::held_middle(drag, x);
        let Some(drag) = self.space_drag.take() else {
            return;
        };
        let space = drag.space;
        cx.notify();
        let from = self.spaces.iter().position(|each| each.id == space);
        let (cells, left) = self.cells();
        let gap = from
            .zip(over.then_some(()))
            .map(|(from, _)| gap_at(&cells, left, from, middle));
        let after = gap.and_then(|gap| self.after_gap(space, gap));
        let (Some(from), Some(gap), Some(after)) = (from, gap, after) else {
            return self.shift_spaces(HashMap::new(), Some(held), cx);
        };
        let mut shifts = self.shifts_for(from, gap);
        shifts.insert(space, (gap as f32 - from as f32) * cells[0]);
        self.shift_spaces(shifts, Some(held), cx);
        // check.sh waits for this line.
        log!("browser: dropped space {space} after {after}");
        if let Some(back) = self.move_space(space, after, cx) {
            self.keep(
                What::MOVE_SPACE,
                Some(Edit::MoveSpace { space, after: back }),
                cx,
            );
        } else {
            self.shift_spaces(HashMap::new(), None, cx);
        }
    }

    /// Esc while a card is dragged: it springs home, the others spring back, and the release moves
    /// nothing.
    pub(super) fn cancel_space_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.space_drag() else {
            return false;
        };
        let held = drag.pointer.map(|x| {
            (
                drag.space,
                self.held_offset(drag.space, Self::held_middle(drag, x)),
            )
        });
        self.space_drag = None;
        self.shift_spaces(HashMap::new(), held, cx);
        cx.notify();
        true
    }

    /// The Space a card dragged from its place goes after for `gap` among the others: none before
    /// the first, and None for the gap it came from.
    fn after_gap(&self, space: u64, gap: usize) -> Option<u64> {
        let others: Vec<u64> = self
            .spaces
            .iter()
            .map(|each| each.id)
            .filter(|&id| id != space)
            .collect();
        let from = self.spaces.iter().position(|each| each.id == space)?;
        if gap == from || gap > others.len() {
            return None;
        }
        Some(if gap == 0 { 0 } else { others[gap - 1] })
    }

    /// Moves `space` to right after `after`, or first for 0, and returns the Space it was after,
    /// for ⌘Z to move it back. None if it's there already, or either is gone.
    pub(super) fn move_space(
        &mut self,
        space: u64,
        after: u64,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let from = self.spaces.iter().position(|each| each.id == space)?;
        let before = if from == 0 {
            0
        } else {
            self.spaces[from - 1].id
        };
        let gone = after != 0 && !self.spaces.iter().any(|each| each.id == after);
        if after == before || after == space || gone {
            return None;
        }
        self.browser.move_space(space, after, cx);
        Some(before)
    }

    /// Where each card's middle is drawn now, from its row's left, gliding ones where they are on
    /// their way.
    pub(super) fn space_places(&self) -> HashMap<u64, f32> {
        let mut places = HashMap::new();
        let (cells, mut left) = self.cells();
        for (space, cell) in self.spaces.iter().zip(cells) {
            places.insert(space.id, left + cell / 2. + self.space_shift(space.id));
            left += cell;
        }
        places
    }

    /// The Spaces came in another order: the cards that were in `before` glide from where they
    /// were drawn to their places, which for the ones already sent there is nowhere. Under Reduce
    /// Motion they're in their places from the first frame.
    pub(super) fn glide_spaces(&mut self, before: HashMap<u64, f32>, moved: bool, cx: &App) {
        let shifts = std::mem::take(&mut self.space_shifts);
        let now = crate::motion::start(cx);
        if !moved {
            // Nothing came of the move: the cards stay drawn where they were, and go home.
            for (space, shift) in shifts {
                self.space_slides
                    .insert(space, (shift + self.space_slide(space), now));
            }
            return;
        }
        if self.space_drag.take().is_some() {
            self.space_lifted = None;
        }
        for (key, at) in self.space_places() {
            let Some(&from) = before.get(&key) else {
                continue;
            };
            if from != at {
                self.space_slides
                    .insert(key, (from - at + self.space_slide(key), now));
            }
        }
    }

    /// How far right of its place `space`'s card is drawn while it glides there.
    pub(super) fn space_slide(&self, space: u64) -> f32 {
        self.space_slides.get(&space).map_or(0., |&(from, start)| {
            Curve::Slide
                .at(start.elapsed().as_secs_f32())
                .map_or(0., |done| from * (1. - done))
        })
    }

    /// Drops the glides that are done, and keeps frames coming for the rest.
    pub(super) fn step_space_slides(&mut self, window: &mut Window) {
        let gliding = !self.space_slides.is_empty();
        self.space_slides
            .retain(|_, (_, start)| Curve::Slide.at(start.elapsed().as_secs_f32()).is_some());
        if !self.space_slides.is_empty() {
            window.request_animation_frame();
        } else if gliding {
            // check.sh waits for this line before it presses a card that moved: a press goes to
            // the card drawn there, which can still be the one that was there (#2026).
            log!("browser: spaces settled");
        }
        if self.space_slides.is_empty() && self.space_drag().is_none() {
            self.space_lifted = None;
        }
    }

    /// The release that ends a card's drag anywhere in the window, heard before GPUI lets go of
    /// it. It's there whenever the row is, since a drag can start and end before a frame
    /// shows that it started.
    pub(super) fn space_drop_listener(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let position = event.position;
                        shell
                            .update(cx, |shell, cx| shell.drop_space(position, bounds, cx))
                            .ok();
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }
}

/// The gap among the cells but the `from`th that `x`, from the row's left, is over, with the
/// first cell starting at `left`: how many of the others' middles are left of it.
fn gap_at(cells: &[f32], mut left: f32, from: usize, x: f32) -> usize {
    let mut gap = 0;
    for (index, &cell) in cells.iter().enumerate() {
        if index != from && left + cell / 2. < x {
            gap += 1;
        }
        left += cell;
    }
    gap
}

/// What GPUI holds while a Space is dragged: nothing to draw, as the card itself follows the pointer.
pub(super) struct SpaceGhost;

impl Render for SpaceGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(test)]
mod tests {
    use super::gap_at;

    /// Three cells 34 wide, centred in 216: they start at 57, so their middles are at 74, 108 and
    /// 142.
    const CELLS: [f32; 3] = [34.; 3];
    const LEFT: f32 = 57.;

    /// The gap is how many of the others' middles the card's middle is past; its own cell is the
    /// gap it came from.
    #[test]
    fn the_gap_counts_the_other_cards_the_middle_is_past() {
        assert_eq!(gap_at(&CELLS, LEFT, 1, 0.), 0);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 80.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 108.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 150.), 2);
        assert_eq!(gap_at(&CELLS, LEFT, 0, 120.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 0, 216.), 2);
    }
}
