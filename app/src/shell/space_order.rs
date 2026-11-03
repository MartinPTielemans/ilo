//! Spaces reordered by dragging a dot along the sidebar's switcher. GPUI holds the drag, as it
//! holds a Library card's tab; the Shell keeps the gap the pointer is over, draws the drop line
//! there, and glides the dots to their new places once the browser reports the new order.

use std::{collections::HashMap, time::Instant};

use gpui::{
    Bounds, Context, DispatchPhase, DragMoveEvent, Entity, MouseButton, MouseUpEvent, Pixels,
    Point, Render, WeakEntity, Window, canvas, div, prelude::*, px, rgba,
};

use super::{
    Curve, DROP, DROP_LINE, Edit, FOOTER_HEIGHT, SPACE_DROP_LINE, SPACE_HEIGHT, SWITCHER_INSET,
    Shell, What, footer::space_cells,
};

/// A Space's dot pressed and dragged along the switcher, as GPUI holds it. Which Space is the
/// Shell's to keep, in its SpaceDrag.
#[derive(Clone, Copy)]
pub(super) struct DraggedSpace;

/// A dot being dragged.
pub(super) struct SpaceDrag {
    space: u64,
    /// The view following the pointer, which lives as long as GPUI's drag: a drag let go anywhere
    /// ends with it.
    ghost: WeakEntity<SpaceGhost>,
    /// How far right of its cell's left the pointer holds the dot, and that cell's width.
    grab: f32,
    cell: f32,
    /// While the pointer is over the footer, the gap among the other dots it's over, 0 before the
    /// first. Off the footer it would go nowhere.
    gap: Option<usize>,
}

/// The dots still gliding to their places: how far right of its place each one started, and when.
pub(super) type DotSlides = HashMap<u64, (f32, Instant)>;

impl Shell {
    /// The dot dragged, while GPUI still holds it.
    pub(super) fn space_drag(&self) -> Option<&SpaceDrag> {
        self.space_drag.as_ref().filter(|drag| drag.ghost.upgrade().is_some())
    }

    /// Whether `space`'s dot is being dragged, so its place dims.
    pub(super) fn space_dragged(&self, space: u64) -> bool {
        self.space_drag().is_some_and(|drag| drag.space == space)
    }

    /// Makes the view that follows the pointer for the dot of `space` in a cell `cell` wide, held
    /// `grab` from its left.
    pub(super) fn start_space_drag(
        shell: &WeakEntity<Shell>,
        space: u64,
        cell: f32,
        grab: Point<Pixels>,
        cx: &mut gpui::App,
    ) -> Entity<SpaceGhost> {
        let ghost = cx.new(|_| SpaceGhost { shell: shell.clone(), space, cell });
        let weak = ghost.downgrade();
        shell
            .update(cx, |shell, cx| {
                // check.sh waits for this line.
                log!("browser: drag space {space}");
                shell.space_drag =
                    Some(SpaceDrag { space, ghost: weak, grab: f32::from(grab.x), cell, gap: None });
                cx.notify();
            })
            .ok();
        ghost
    }

    /// The gap among the other dots that the dot dragged is over with the pointer at `position`,
    /// while that's over the footer: the sidebar's width wide and FOOTER_HEIGHT tall around the
    /// switcher, at `bounds`. None elsewhere.
    fn gap_over(&self, position: Point<Pixels>, bounds: Bounds<Pixels>) -> Option<usize> {
        let drag = self.space_drag()?;
        let from = self.spaces.iter().position(|space| space.id == drag.space)?;
        let (span, x) = (f32::from(bounds.size.width), f32::from(position.x - bounds.left()));
        let middle = f32::from(bounds.top()) + SPACE_HEIGHT / 2.;
        let over = (f32::from(position.y) - middle).abs() <= FOOTER_HEIGHT / 2.
            && (-SWITCHER_INSET..=span + SWITCHER_INSET).contains(&x);
        over.then(|| gap_at(&self.switcher_cells(span), span, from, x))
    }

    /// The pointer holding a dot moved over the window: the line goes to the gap it's over.
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

    /// The pointer let go at `position`, with the switcher at `bounds`: a dot dragged moves into
    /// the gap it's over, if that's not where it was, gliding there from where its copy was let
    /// go. ⌘Z puts it back. The gap is found again here, as the press, the move that starts the
    /// drag and the release can come before any other move.
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
        let x = f32::from(position.x - bounds.left());
        self.dot_landing = Some((space, x - drag.grab + drag.cell / 2.));
        // check.sh waits for this line.
        log!("browser: dropped space {space} after {after}");
        if let Some(back) = self.move_space(space, after, cx) {
            self.keep(What::MOVE_SPACE, Some(Edit::MoveSpace { space, after: back }), cx);
        } else {
            self.dot_landing = None;
        }
    }

    /// Esc while a dot is dragged: its copy goes, and the release moves nothing.
    pub(super) fn cancel_space_drag(&mut self, cx: &mut Context<Self>) -> bool {
        if self.space_drag().is_none() {
            return false;
        }
        self.space_drag = None;
        cx.notify();
        true
    }

    /// The Space a dot dragged from its place goes after for `gap` among the others: none before
    /// the first, and None for the gap it came from.
    fn after_gap(&self, space: u64, gap: usize) -> Option<u64> {
        let others: Vec<u64> =
            self.spaces.iter().map(|each| each.id).filter(|&id| id != space).collect();
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
        let before = if from == 0 { 0 } else { self.spaces[from - 1].id };
        let gone = after != 0 && !self.spaces.iter().any(|each| each.id == after);
        if after == before || after == space || gone {
            return None;
        }
        self.browser.move_space(space, after, cx);
        Some(before)
    }

    /// The switcher's cells for the Spaces as they are, in a span `span` wide.
    pub(super) fn switcher_cells(&self, span: f32) -> Vec<f32> {
        let shown = self.spaces.iter().position(|space| space.id == self.space);
        space_cells(span, self.spaces.len(), shown.unwrap_or(usize::MAX))
    }

    /// Where each dot's middle is drawn in the switcher now, from its left, gliding ones where
    /// they are on their way.
    pub(super) fn dot_places(&self) -> HashMap<u64, f32> {
        let span = self.width() - 2. * SWITCHER_INSET;
        let cells = self.switcher_cells(span);
        let mut left = (span - cells.iter().sum::<f32>()) / 2.;
        let mut places = HashMap::new();
        for (space, cell) in self.spaces.iter().zip(cells) {
            places.insert(space.id, left + cell / 2. + self.dot_slide(space.id));
            left += cell;
        }
        places
    }

    /// The Spaces came in another order: the dots that were in `before` glide from where they
    /// were drawn to their places, and one just let go from where its copy was.
    pub(super) fn glide_dots(&mut self, before: HashMap<u64, f32>, moved: bool) {
        let landing = self.dot_landing.take();
        if !moved && landing.is_none() {
            return;
        }
        let now = Instant::now();
        for (id, at) in self.dot_places() {
            let from = match landing {
                Some((space, x)) if space == id => x,
                _ => match before.get(&id) {
                    Some(&x) => x,
                    None => continue,
                },
            };
            if from != at {
                self.dot_slides.insert(id, (from - at + self.dot_slide(id), now));
            }
        }
    }

    /// How far right of its place a dot is drawn while it glides there.
    pub(super) fn dot_slide(&self, space: u64) -> f32 {
        self.dot_slides.get(&space).map_or(0., |&(from, start)| {
            Curve::Slide.at(start.elapsed().as_secs_f32()).map_or(0., |done| from * (1. - done))
        })
    }

    /// Drops the glides that are done, and keeps frames coming for the rest.
    pub(super) fn step_dot_slides(&mut self, window: &mut Window) {
        self.dot_slides
            .retain(|_, (_, start)| Curve::Slide.at(start.elapsed().as_secs_f32()).is_some());
        if !self.dot_slides.is_empty() {
            window.request_animation_frame();
        }
    }

    /// The drop line in the gap the dragged dot would go into, from the switcher's left, unless
    /// that's where it was.
    pub(super) fn space_drop_line(&self, span: f32) -> Option<impl IntoElement + use<>> {
        let drag = self.space_drag()?;
        let from = self.spaces.iter().position(|space| space.id == drag.space)?;
        let x = line_x(&self.switcher_cells(span), span, from, drag.gap?)?;
        Some(
            div()
                .absolute()
                .left(px(x - DROP_LINE / 2.))
                .top(px((SPACE_HEIGHT - SPACE_DROP_LINE) / 2.))
                .w(px(DROP_LINE))
                .h(px(SPACE_DROP_LINE))
                .bg(rgba(DROP)),
        )
    }

    /// The release that ends a dot's drag anywhere in the window, heard before GPUI lets go of it.
    /// It's there whenever the switcher is, since a drag can start and end before a frame shows
    /// that it started.
    pub(super) fn space_drop_listener(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let position = event.position;
                        shell.update(cx, |shell, cx| shell.drop_space(position, bounds, cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }
}

/// The gap among the cells but the `from`th that `x`, from a switcher `span` wide's left, is over:
/// how many of the others' middles are left of it.
fn gap_at(cells: &[f32], span: f32, from: usize, x: f32) -> usize {
    let mut left = (span - cells.iter().sum::<f32>()) / 2.;
    let mut gap = 0;
    for (index, &cell) in cells.iter().enumerate() {
        if index != from && left + cell / 2. < x {
            gap += 1;
        }
        left += cell;
    }
    gap
}

/// Where the drop line stands for `gap` among the cells but the `from`th, from the switcher's
/// left: on the edge between the cells either side of it. None for the gap it came from.
fn line_x(cells: &[f32], span: f32, from: usize, gap: usize) -> Option<f32> {
    let others = cells.len().checked_sub(1)?;
    if gap == from || gap > others {
        return None;
    }
    // The cell right of the gap, counted with the dragged one's.
    let next = if gap < from { gap } else { gap + 1 };
    let left = (span - cells.iter().sum::<f32>()) / 2.;
    Some(left + cells[..next].iter().sum::<f32>())
}

/// What follows the pointer while a dot is dragged: the dot, drawn from the Shell each frame, and
/// nothing once Esc has put it back.
pub(super) struct SpaceGhost {
    shell: WeakEntity<Shell>,
    space: u64,
    cell: f32,
}

impl Render for SpaceGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = self.shell.upgrade();
        let (space, cell) = (self.space, self.cell);
        let ghost = shell.and_then(|shell| {
            let shell = shell.read(cx);
            shell.space_dragged(space).then(|| shell.dot_ghost(space, cell)).flatten()
        });
        div().children(ghost)
    }
}

#[cfg(test)]
mod tests {
    use super::{gap_at, line_x};

    /// Three cells 34 wide, centred in 216: they start at 57, so their middles are at 74, 108 and
    /// 142, and their edges at 57, 91, 125 and 159.
    const CELLS: [f32; 3] = [34.; 3];
    const SPAN: f32 = 216.;

    /// The gap is how many of the others' middles the pointer is past; its own cell is the gap it
    /// came from.
    #[test]
    fn the_gap_counts_the_other_dots_the_pointer_is_past() {
        assert_eq!(gap_at(&CELLS, SPAN, 1, 0.), 0);
        assert_eq!(gap_at(&CELLS, SPAN, 1, 80.), 1);
        assert_eq!(gap_at(&CELLS, SPAN, 1, 108.), 1);
        assert_eq!(gap_at(&CELLS, SPAN, 1, 150.), 2);
        assert_eq!(gap_at(&CELLS, SPAN, 0, 120.), 1);
        assert_eq!(gap_at(&CELLS, SPAN, 0, 216.), 2);
    }

    /// The line stands on the edge between the dots either side of the gap, and there's none
    /// where the dot came from.
    #[test]
    fn the_line_stands_between_the_dots_either_side() {
        assert_eq!(line_x(&CELLS, SPAN, 1, 0), Some(57.));
        assert_eq!(line_x(&CELLS, SPAN, 1, 1), None);
        assert_eq!(line_x(&CELLS, SPAN, 1, 2), Some(159.));
        assert_eq!(line_x(&CELLS, SPAN, 0, 1), Some(125.));
        assert_eq!(line_x(&CELLS, SPAN, 2, 1), Some(91.));
        assert_eq!(line_x(&CELLS, SPAN, 2, 3), None);
    }
}
