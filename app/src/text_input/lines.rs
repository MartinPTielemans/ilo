//! A field of several lines, as it was last laid out: each of its lines, wrapped to the field's
//! width, after the one before it. Offsets are in what the field shows, and points are from the
//! top left of its text, before it scrolls.

use std::ops::Range;

use gpui::{Bounds, Pixels, Point, SharedString, WrappedLine, point, px};

pub(super) struct Lines {
    /// What was laid out, which a later edit may have changed since.
    pub text: SharedString,
    /// Each line between two of the text's newlines, by where it starts.
    pub lines: Vec<(usize, WrappedLine)>,
    pub line_height: Pixels,
}

impl Lines {
    pub fn new(text: SharedString, wrapped: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let mut start = 0;
        let lines = (wrapped.into_iter())
            .map(|line| {
                let at = start;
                // Its newline is the next line's start.
                start += line.len() + 1;
                (at, line)
            })
            .collect();
        Lines {
            text,
            lines,
            line_height,
        }
    }

    /// How many rows it takes, its wrapped ones too.
    pub fn rows(&self) -> usize {
        (self.lines.iter())
            .map(|(_, line)| line.wrap_boundaries().len() + 1)
            .sum::<usize>()
            .max(1)
    }

    pub fn height(&self) -> Pixels {
        self.line_height * self.rows()
    }

    /// Where the caret at `index` stands: the top of its row.
    pub fn position(&self, index: usize) -> Point<Pixels> {
        let mut y = px(0.);
        for (start, line) in &self.lines {
            if index <= start + line.len() {
                let at = line.position_for_index(index - start, self.line_height);
                let at = at.unwrap_or_default();
                return point(at.x, y + at.y);
            }
            y += line.size(self.line_height).height;
        }
        point(px(0.), (y - self.line_height).max(px(0.)))
    }

    /// The place between two characters closest to `position`; above the text its start, below
    /// it its end.
    pub fn index(&self, position: Point<Pixels>) -> usize {
        let mut y = px(0.);
        let last = self.lines.len().saturating_sub(1);
        for (row, (start, line)) in self.lines.iter().enumerate() {
            let height = line.size(self.line_height).height;
            if position.y < y + height || row == last {
                let within = (position.y - y).clamp(px(0.), height - px(1.));
                let at =
                    line.closest_index_for_position(point(position.x, within), self.line_height);
                return start + at.unwrap_or_else(|end| end);
            }
            y += height;
        }
        0
    }

    /// The rectangles a selection of `range` covers, one a row it spans, in a field `width` wide.
    pub fn rects(&self, range: Range<usize>, width: Pixels) -> Vec<Bounds<Pixels>> {
        let (from, to) = (self.position(range.start), self.position(range.end));
        let tall = self.line_height;
        if from.y == to.y {
            return vec![Bounds::from_corners(from, point(to.x, to.y + tall))];
        }
        let mut rects = vec![Bounds::from_corners(from, point(width, from.y + tall))];
        if to.y > from.y + tall {
            rects.push(Bounds::from_corners(
                point(px(0.), from.y + tall),
                point(width, to.y),
            ));
        }
        rects.push(Bounds::from_corners(
            point(px(0.), to.y),
            point(to.x, to.y + tall),
        ));
        rects
    }
}
