//! Capture's Edit (#673): marks on what's chosen before it's copied, saved or sent, as Arc's capture
//! menu's Edit opens the picture for markup. Arc's markup wasn't recorded, so its tools are ours:
//! a pen, an arrow and a box, in a few colours, with Undo. The marks are kept in the page's
//! points, drawn by GPUI while they're made and into the picture's pixels once it comes.

use gpui::{Bounds, PathBuilder, PathStyle, Pixels, Point, StrokeOptions, Window, point, px, rgb};
use image::RgbaImage;
use lyon::tessellation::{LineCap, LineJoin};

use crate::tokens::capture::markup::{HEAD, WIDTH};

/// What a press on what's chosen draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// A line where the pointer went.
    Pen,
    /// A straight line from the press with a head where it's let go.
    Arrow,
    /// A rectangle from the press to where it's let go.
    Box,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Pen, Tool::Arrow, Tool::Box];

    /// Lucide's.
    pub fn glyph(self) -> &'static str {
        match self {
            Tool::Pen => "icons/pencil.svg",
            Tool::Arrow => "icons/arrow-up-right.svg",
            Tool::Box => "icons/square.svg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Pen => "Pen",
            Tool::Arrow => "Arrow",
            Tool::Box => "Box",
        }
    }

    /// For the logs.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Pen => "pen",
            Tool::Arrow => "arrow",
            Tool::Box => "box",
        }
    }
}

/// A mark, in the page's points from its view's top left.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub tool: Tool,
    /// 0xrrggbb.
    pub colour: u32,
    /// Where the pointer went for the pen; the press and where it is now for the others.
    pub points: Vec<(f32, f32)>,
}

impl Mark {
    pub fn new(tool: Tool, colour: u32, at: (f32, f32)) -> Mark {
        Mark {
            tool,
            colour,
            points: vec![at],
        }
    }

    /// The pointer moved to `at` with the button down.
    pub fn to(&mut self, at: (f32, f32)) {
        if self.tool != Tool::Pen {
            self.points.truncate(1);
        }
        self.points.push(at);
    }

    /// Whether it's worth keeping once it's let go: a pen's dot is, but an arrow or a box needs
    /// some length, as a click isn't one.
    pub fn kept(&self) -> bool {
        match (self.tool, self.points.first(), self.points.last()) {
            (Tool::Pen, Some(_), _) => true,
            (_, Some(&(x0, y0)), Some(&(x1, y1))) => (x1 - x0).hypot(y1 - y0) >= WIDTH,
            _ => false,
        }
    }

    /// The runs of points its stroke follows.
    pub fn lines(&self) -> Vec<Vec<(f32, f32)>> {
        let (Some(&from), Some(&to)) = (self.points.first(), self.points.last()) else {
            return Vec::new();
        };
        match self.tool {
            Tool::Pen => vec![self.points.clone()],
            Tool::Arrow => {
                let angle = (to.1 - from.1).atan2(to.0 - from.0);
                let side = |turn: f32| {
                    let angle = angle + std::f32::consts::PI - turn;
                    (to.0 + HEAD * angle.cos(), to.1 + HEAD * angle.sin())
                };
                let spread = std::f32::consts::FRAC_PI_6;
                vec![vec![from, to], vec![side(spread), to, side(-spread)]]
            }
            Tool::Box => {
                let (left, right) = (from.0.min(to.0), from.0.max(to.0));
                let (top, bottom) = (from.1.min(to.1), from.1.max(to.1));
                let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
                vec![[&corners[..], &corners[..1]].concat()]
            }
        }
    }
}

/// Round ends and corners, so a pen's line reads as drawn.
fn stroke() -> StrokeOptions {
    StrokeOptions::default()
        .with_line_width(WIDTH)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
}

/// Draws `marks` on the window, with the page's view's top left at `origin`.
pub fn paint(marks: &[Mark], origin: Point<Pixels>, window: &mut Window) {
    for mark in marks {
        for line in mark.lines() {
            let at = |(x, y): (f32, f32)| origin + point(px(x), px(y));
            let mut path = PathBuilder::stroke(px(WIDTH)).with_style(PathStyle::Stroke(stroke()));
            path.move_to(at(line[0]));
            // A dot is a line as long as nothing, which the round ends draw.
            for &each in line
                .iter()
                .skip(1)
                .chain((line.len() == 1).then_some(&line[0]))
            {
                path.line_to(at(each));
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(mark.colour));
            }
        }
    }
}

/// `png`, the picture of `chosen` in the page's view, with `marks` drawn on it, as a PNG.
pub fn draw(png: &[u8], marks: &[Mark], chosen: Bounds<Pixels>) -> Option<Vec<u8>> {
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let picture = picture.into_rgba8();
    let (width, height) = picture.dimensions();
    let scale = width as f32 / f32::from(chosen.size.width).max(1.);
    let mut data = picture.into_raw();
    // tiny-skia's pixels carry their alpha in their colours.
    for pixel in data.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = (u16::from(*channel) * alpha / 255) as u8;
        }
    }
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    let mut pixmap = tiny_skia::Pixmap::from_vec(data, size)?;
    // From the page's points to the picture's pixels.
    let (left, top) = (f32::from(chosen.origin.x), f32::from(chosen.origin.y));
    let transform =
        tiny_skia::Transform::from_row(scale, 0., 0., scale, -left * scale, -top * scale);
    let stroke = tiny_skia::Stroke {
        width: WIDTH,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    for mark in marks {
        let mut paint = tiny_skia::Paint::default();
        let [r, g, b] = [16, 8, 0].map(|shift| (mark.colour >> shift) as u8);
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        for line in mark.lines() {
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(line[0].0, line[0].1);
            for &(x, y) in line.iter().skip(1) {
                path.line_to(x, y);
            }
            if line.len() == 1 {
                // A dot, which the round ends draw: a line a hair long, as tiny-skia drops one
                // that's none.
                path.line_to(line[0].0 + 0.01, line[0].1);
            }
            if let Some(path) = path.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, transform, None);
            }
        }
    }
    let pixels = pixmap.pixels().iter().flat_map(|pixel| {
        let pixel = pixel.demultiply();
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
    });
    let marked = RgbaImage::from_raw(width, height, pixels.collect())?;
    let mut out = std::io::Cursor::new(Vec::new());
    marked.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
mod tests {
    use gpui::size;
    use image::Rgba;

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let picture = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// A mark is drawn where it was made in the page, at the picture's scale: a box at 2× drawn
    /// round what's chosen's middle, and nothing outside its line.
    #[test]
    fn marks_are_drawn_where_they_were_made() {
        let chosen = Bounds::new(point(px(100.), px(50.)), size(px(200.), px(100.)));
        let mut mark = Mark::new(Tool::Box, 0xff0000, (150., 75.));
        mark.to((250., 125.));
        let marked = draw(&png(400, 200), &[mark], chosen).unwrap();
        let marked = image::load_from_memory(&marked).unwrap().into_rgba8();
        assert_eq!(marked.dimensions(), (400, 200));
        // The box's left side, at page x 150, is picture x 100 at 2×.
        assert_eq!(
            marked.get_pixel(100, 100).0,
            [255, 0, 0, 255],
            "the box's side"
        );
        assert_eq!(marked.get_pixel(200, 100).0, [255; 4], "inside the box");
        assert_eq!(marked.get_pixel(20, 20).0, [255; 4], "outside the box");
    }

    /// An arrow has a head where it's let go, and a click isn't an arrow; a pen's click is a dot.
    #[test]
    fn an_arrow_has_its_head_and_a_click_draws_only_a_dot() {
        let mut arrow = Mark::new(Tool::Arrow, 0, (0., 0.));
        arrow.to((50., 0.));
        arrow.to((100., 0.));
        assert_eq!(
            arrow.points,
            [(0., 0.), (100., 0.)],
            "an arrow keeps its ends alone"
        );
        let lines = arrow.lines();
        assert_eq!(lines.len(), 2);
        let head = &lines[1];
        assert_eq!(head[1], (100., 0.));
        assert!(head[0].0 < 100. && head[2].0 < 100., "{head:?}");
        assert!(
            head[0].1 * head[2].1 < 0.,
            "the head's sides are one side: {head:?}"
        );
        assert!(arrow.kept());
        assert!(!Mark::new(Tool::Arrow, 0, (5., 5.)).kept());
        assert!(!Mark::new(Tool::Box, 0, (5., 5.)).kept());

        let dot = Mark::new(Tool::Pen, 0x0000ff, (60., 30.));
        assert!(dot.kept());
        let chosen = Bounds::new(point(px(0.), px(0.)), size(px(120.), px(60.)));
        let marked = draw(&png(120, 60), &[dot], chosen).unwrap();
        let marked = image::load_from_memory(&marked).unwrap().into_rgba8();
        assert_eq!(marked.get_pixel(60, 30).0, [0, 0, 255, 255], "no dot");
    }
}
