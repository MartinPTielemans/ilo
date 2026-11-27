//! Spaces reordered by dragging a card by its handle along the Library's Spaces. GPUI holds the
//! drag, as it holds a Library card's tab; the Shell keeps the gap the pointer is over, draws the
//! drop line there, and glides the cards to their new places once the browser reports the new
//! order.

use std::collections::HashMap;
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, DragMoveEvent, Entity, MouseButton,
    MouseUpEvent, Pixels, Point, Render, WeakEntity, Window, canvas, div, prelude::*, px,
};

use super::{Curve, DROP_LINE, DROP_LINE_END, DROP_RING, DROP_RING_X, Edit, Shell, What};
use crate::tokens::shell::library::{CARD_GAP, CARD_WIDTH, SPACES_INSET, SPACES_LEFT};

/// A Space's card handle pressed and dragged, as GPUI holds it. Which Space is the Shell's to
/// keep, in its SpaceDrag.
#[derive(Clone, Copy)]
pub(super) struct DraggedSpace;

/// A card being dragged.
pub(super) struct SpaceDrag {
    space: u64,
    /// The view following the pointer, which lives as long as GPUI's drag: a drag let go anywhere
    /// ends with it.
    ghost: WeakEntity<SpaceGhost>,
    /// How far right of its left the pointer holds the card, and how wide that is.
    grab: f32,
    width: f32,
    /// While the pointer is over the Spaces section, the gap among the others it's over, 0 before
    /// the first. Off it it would go nowhere.
    gap: Option<usize>,
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

    /// Whether `space`'s card is being dragged, so its place dims.
    pub(super) fn space_dragged(&self, space: u64) -> bool {
        self.space_drag().is_some_and(|drag| drag.space == space)
    }

    /// Makes the view that follows the pointer for `space`'s card dragged, `width` wide and held
    /// `grab` from its left.
    pub(super) fn start_space_drag(
        shell: &WeakEntity<Shell>,
        space: u64,
        width: f32,
        grab: f32,
        cx: &mut gpui::App,
    ) -> Entity<SpaceGhost> {
        let ghost = cx.new(|_| SpaceGhost {
            shell: shell.clone(),
            space,
        });
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
                    gap: None,
                });
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

    /// The gap among the others that the Space dragged is over with the pointer at `position`,
    /// while that's over the section at `bounds`. None elsewhere.
    fn gap_over(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> Option<usize> {
        let drag = self.space_drag()?;
        let from = self
            .spaces
            .iter()
            .position(|space| space.id == drag.space)?;
        let (x, over) = self.along_at(position, bounds);
        let (cells, left) = self.cells();
        over.then(|| gap_at(&cells, left, from, x))
    }

    /// The pointer holding a card moved over the window: the line goes to the gap it's over along
    /// the section at `bounds`.
    pub(super) fn drag_space_over(
        &mut self,
        event: &DragMoveEvent<DraggedSpace>,
        cx: &mut Context<Self>,
    ) {
        let gap = self.gap_over(event.event.position, event.bounds);
        if let Some(drag) = self.space_drag.as_mut()
            && drag.gap != gap
        {
            drag.gap = gap;
            cx.notify();
        }
    }

    /// The pointer let go at `position`, with the section at `bounds`: a Space dragged along it
    /// moves into the gap it's over, if that's not where it was, its card gliding there from where
    /// its copy was let go. ⌘Z puts it back. The gap is found again here, as the press,
    /// the move that starts the drag and the release can come before any other move.
    pub(super) fn drop_space(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.space_drag().is_none() {
            return;
        }
        let gap = self.gap_over(position, bounds);
        let Some(drag) = self.space_drag.take() else {
            return;
        };
        cx.notify();
        let Some(after) = gap.and_then(|gap| self.after_gap(drag.space, gap)) else {
            return;
        };
        let space = drag.space;
        let (x, _) = self.along_at(position, bounds);
        self.space_landing = Some((space, x - drag.grab + drag.width / 2.));
        // check.sh waits for this line.
        log!("browser: dropped space {space} after {after}");
        if let Some(back) = self.move_space(space, after, cx) {
            self.keep(
                What::MOVE_SPACE,
                Some(Edit::MoveSpace { space, after: back }),
                cx,
            );
        } else {
            self.space_landing = None;
        }
    }

    /// Esc while a card is dragged: its copy goes, and the release moves nothing.
    pub(super) fn cancel_space_drag(&mut self, cx: &mut Context<Self>) -> bool {
        if self.space_drag().is_none() {
            return false;
        }
        self.space_drag = None;
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
            places.insert(space.id, left + cell / 2. + self.space_slide(space.id));
            left += cell;
        }
        places
    }

    /// The Spaces came in another order: the cards that were in `before` glide from
    /// where they were drawn to their places, and the one just let go from where its copy was.
    /// Under Reduce Motion they're in their places from the first frame.
    pub(super) fn glide_spaces(&mut self, before: HashMap<u64, f32>, moved: bool, cx: &App) {
        let landing = self.space_landing.take();
        if !moved && landing.is_none() {
            return;
        }
        let now = crate::motion::start(cx);
        for (key, at) in self.space_places() {
            let from = match landing {
                Some((space, x)) if space == key => x,
                _ => match before.get(&key) {
                    Some(&x) => x,
                    None => continue,
                },
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
    }

    /// The drop line in the gap the Space dragged would go into, unless that's where it was: the
    /// rows' line stood up, its ring as far below the cards' tops as the rows' is in from a
    /// plate's left and its end as far above their foot, from the section's left.
    pub(super) fn space_drop_line(&self) -> Option<AnyElement> {
        let drag = self.space_drag()?;
        let colour = self.palette.drop_line();
        let from = self
            .spaces
            .iter()
            .position(|space| space.id == drag.space)?;
        let (cells, left) = self.cells();
        let x = line_x(&cells, left, from, drag.gap?)?;
        let ring = div()
            .flex_none()
            .size(px(DROP_RING))
            .rounded_full()
            .border(px(DROP_LINE))
            .border_color(colour);
        Some(
            div()
                .absolute()
                .left(px(x - self.cards_at() - DROP_RING / 2.))
                .top(px(SPACES_INSET + DROP_RING_X))
                .bottom(px(SPACES_INSET + DROP_LINE_END))
                .w(px(DROP_RING))
                .flex()
                .flex_col()
                .items_center()
                .child(ring)
                .child(div().flex_1().w(px(DROP_LINE)).bg(colour))
                .into_any_element(),
        )
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

/// Where the drop line stands for `gap` among the cells but the `from`th, from the row's left,
/// with the first cell starting at `left`: on the edge between the cells either side of it. None
/// for the gap it came from.
fn line_x(cells: &[f32], left: f32, from: usize, gap: usize) -> Option<f32> {
    let others = cells.len().checked_sub(1)?;
    if gap == from || gap > others {
        return None;
    }
    // The cell right of the gap, counted with the dragged one's.
    let next = if gap < from { gap } else { gap + 1 };
    Some(left + cells[..next].iter().sum::<f32>())
}

/// What follows the pointer while a Space is dragged: its whole card, drawn from the Shell each
/// frame, and nothing once Esc has put it back.
pub(super) struct SpaceGhost {
    shell: WeakEntity<Shell>,
    space: u64,
}

impl Render for SpaceGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let space = self.space;
        let shell = self
            .shell
            .upgrade()
            .filter(|shell| shell.read(cx).space_dragged(space));
        let ghost =
            shell.and_then(|shell| shell.update(cx, |shell, cx| shell.space_card_ghost(space, cx)));
        div().children(ghost)
    }
}

#[cfg(test)]
mod tests {
    use super::{gap_at, line_x};

    /// Three cells 34 wide, centred in 216: they start at 57, so their middles are at 74, 108 and
    /// 142, and their edges at 57, 91, 125 and 159.
    const CELLS: [f32; 3] = [34.; 3];
    const LEFT: f32 = 57.;

    /// The gap is how many of the others' middles the pointer is past; its own cell is the gap it
    /// came from.
    #[test]
    fn the_gap_counts_the_other_cards_the_pointer_is_past() {
        assert_eq!(gap_at(&CELLS, LEFT, 1, 0.), 0);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 80.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 108.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 1, 150.), 2);
        assert_eq!(gap_at(&CELLS, LEFT, 0, 120.), 1);
        assert_eq!(gap_at(&CELLS, LEFT, 0, 216.), 2);
    }

    /// The line stands on the edge between the cards either side of the gap, and there's none
    /// where the card came from.
    #[test]
    fn the_line_stands_between_the_cards_either_side() {
        assert_eq!(line_x(&CELLS, LEFT, 1, 0), Some(57.));
        assert_eq!(line_x(&CELLS, LEFT, 1, 1), None);
        assert_eq!(line_x(&CELLS, LEFT, 1, 2), Some(159.));
        assert_eq!(line_x(&CELLS, LEFT, 0, 1), Some(125.));
        assert_eq!(line_x(&CELLS, LEFT, 2, 1), Some(91.));
        assert_eq!(line_x(&CELLS, LEFT, 2, 3), None);
    }
}
