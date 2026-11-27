//! Where the panel's parts lie, in points from its top left, all from the `theme-panel` tokens, and
//! what its pad and palettes mean for a Space's theme.
//!
//! The pad is the theme's field drawn square (crate::theme): the hue goes once round from the left
//! edge to the right, and the lightness from the top down, so a handle's place on the pad is its
//! colour's place in the field, and a colour placed round the main one keeps its place there as
//! the main one moves.

use std::f32::consts::TAU;

use crate::theme::{self, MAX_COLORS, Theme};
use crate::tokens::theme_panel::*;

/// The bars, top to bottom.
pub const BARS: [Bar; 3] = [Bar::Intensity, Bar::Transparency, Bar::Grain];

/// One of the panel's bars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bar {
    Intensity,
    Transparency,
    Grain,
}

impl Bar {
    pub fn name(self) -> &'static str {
        match self {
            Bar::Intensity => "Intensity",
            Bar::Transparency => "Transparency",
            Bar::Grain => "Grain",
        }
    }

    const fn index(self) -> usize {
        match self {
            Bar::Intensity => 0,
            Bar::Transparency => 1,
            Bar::Grain => 2,
        }
    }
}

/// How the panel's parts are set out: in the panel (`PANEL`), or in first run's Space step
/// (first-run-v9 §6), which has no padding and no note, a head as tall as its name field, and a
/// shorter pad as wide as the parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Form {
    pub padding: f32,
    pub head: f32,
    /// Above the pad, and above the first bar.
    pub pad_gap: f32,
    pub pad_height: f32,
    pub bars_gap: f32,
    /// Between one bar's track and the next one's label.
    pub bar_gap: f32,
    /// Where the pad's dots are: the first one's middle from the pad's top left, and how far the
    /// last is across and down from it.
    pub dots: [f32; 4],
    /// The note row under the bars, with Reset.
    pub note: bool,
}

/// The panel's own form.
pub const PANEL: Form = Form {
    padding: PADDING,
    head: HEAD,
    pad_gap: GAP,
    pad_height: PAD_HEIGHT,
    bars_gap: GAP,
    bar_gap: BAR_GAP,
    dots: [
        DOT_LEFT,
        DOT_TOP,
        DOT_PITCH * (DOTS_ACROSS - 1.),
        DOT_PITCH * (DOTS_DOWN - 1.),
    ],
    note: true,
};

impl Form {
    /// The width inside its padding, which the pad and the bars span.
    pub const fn inner(&self) -> f32 {
        WIDTH - 2. * self.padding
    }

    /// Appearance segment `index`'s left and top, the three at the head's right.
    pub fn segment(&self, index: usize) -> (f32, f32) {
        let frame = 3. * SEGMENT_WIDTH + 2. * SEGMENT_INSET;
        let left = WIDTH - self.padding - frame + SEGMENT_INSET + index as f32 * SEGMENT_WIDTH;
        let top = self.padding + (self.head - SEGMENT_HEIGHT) / 2.;
        (left, top)
    }

    /// The top of the palettes' label row.
    pub const fn palettes_top(&self) -> f32 {
        self.padding + self.head + GAP
    }

    /// The middle of palette disc `index`'s slot, eight across the inside, spaced between.
    pub fn disc(&self, index: usize) -> (f32, f32) {
        let count = palette_count() as f32;
        let x = self.padding + SLOT / 2. + index as f32 * (self.inner() - SLOT) / (count - 1.);
        (x, self.palettes_top() + LABEL + PALETTES_GAP + SLOT / 2.)
    }

    /// The pad's top left.
    pub const fn pad(&self) -> (f32, f32) {
        (
            self.padding,
            self.palettes_top() + LABEL + PALETTES_GAP + SLOT + self.pad_gap,
        )
    }

    /// The top of bar `bar`'s label row.
    pub const fn bar_top(&self, bar: Bar) -> f32 {
        let first = self.pad().1 + self.pad_height + self.bars_gap;
        first + bar.index() as f32 * (LABEL + BAR_LABEL_GAP + THUMB + self.bar_gap)
    }

    /// The middle of bar `bar`'s track, down the form.
    pub const fn track_y(&self, bar: Bar) -> f32 {
        self.bar_top(bar) + LABEL + BAR_LABEL_GAP + THUMB / 2.
    }

    /// The track's left end and its length, in from the inside by the label's 2.
    pub fn track(&self) -> (f32, f32) {
        (self.padding + LABEL_INSET, self.inner() - 2. * LABEL_INSET)
    }

    /// The middle of a bar's thumb across the form at `value`, 0 to 1: its edge at the track's
    /// ends.
    pub fn thumb_x(&self, value: f32) -> f32 {
        let (left, length) = self.track();
        left + THUMB / 2. + value.clamp(0., 1.) * (length - THUMB)
    }

    /// The value for a thumb whose middle is at `x`.
    pub fn value_at(&self, x: f32) -> f32 {
        let (left, length) = self.track();
        ((x - left - THUMB / 2.) / (length - THUMB)).clamp(0., 1.)
    }

    /// The top of the note row, under the last bar.
    pub const fn note_top(&self) -> f32 {
        self.track_y(BARS[BARS.len() - 1]) + THUMB / 2. + GAP
    }

    /// How tall the form is.
    pub const fn height(&self) -> f32 {
        match self.note {
            true => self.note_top() + LABEL + self.padding,
            false => self.track_y(BARS[BARS.len() - 1]) + THUMB / 2. + self.padding,
        }
    }

    /// Where on the form the pad's point (across, down) is.
    pub fn on_panel(&self, across: f32, down: f32) -> (f32, f32) {
        let (left, top) = self.pad();
        let [dot_left, dot_top, width, height] = self.dots;
        (
            left + dot_left + across.rem_euclid(1.) * width,
            top + dot_top + down * height,
        )
    }

    /// The pad's point under (x, y) on the form, across wrapping round and down held to the pad.
    pub fn on_pad(&self, x: f32, y: f32) -> (f32, f32) {
        let (left, top) = self.pad();
        let [dot_left, dot_top, width, height] = self.dots;
        (
            ((x - left - dot_left) / width).clamp(0., 1.),
            ((y - top - dot_top) / height).clamp(0., 1.),
        )
    }

    /// Where colour `index`'s handle is on the form for `theme`.
    pub fn handle(&self, theme: &Theme, index: usize) -> (f32, f32) {
        let (x, y) = theme.handle(index);
        let (across, down) = pad_point(x, y);
        self.on_panel(across, down)
    }

    /// The Points chip: its left, its top and its width, with a disc for each colour and the plus
    /// while there's room for one more.
    pub fn chip(&self, count: usize) -> (f32, f32, f32) {
        let (left, top) = self.pad();
        let plus = if count < MAX_COLORS {
            CHIP_GAP + PLUS
        } else {
            0.
        };
        let discs = count as f32 * CHIP_DOT + count.saturating_sub(1) as f32 * CHIP_GAP;
        let width = CHIP_LEFT + discs + plus + CHIP_RIGHT;
        (
            left + CHIP_INSET,
            top + self.pad_height - CHIP_INSET - CHIP_HEIGHT,
            width,
        )
    }

    /// The middle of the chip's disc for colour `index`, or with `index` the count, the plus's.
    pub fn chip_item(&self, count: usize, index: usize) -> (f32, f32) {
        let (left, top, _) = self.chip(count);
        let y = top + CHIP_HEIGHT / 2.;
        let before = left + CHIP_LEFT + index as f32 * (CHIP_DOT + CHIP_GAP);
        if index < count {
            (before + CHIP_DOT / 2., y)
        } else {
            (before + PLUS / 2., y)
        }
    }

    /// The value field's left and top, at the pad's foot on the right.
    pub fn value_field(&self) -> (f32, f32) {
        let (left, top) = self.pad();
        (
            left + self.inner() - CHIP_INSET - FIELD_WIDTH,
            top + self.pad_height - CHIP_INSET - CHIP_HEIGHT,
        )
    }
}

/// Appearance segment `index`'s left and top in the panel.
#[cfg(test)]
pub fn segment(index: usize) -> (f32, f32) {
    PANEL.segment(index)
}

/// The middle of palette disc `index`'s slot in the panel.
#[cfg(test)]
pub fn disc(index: usize) -> (f32, f32) {
    PANEL.disc(index)
}

/// The panel's pad's top left.
#[cfg(test)]
pub const fn pad() -> (f32, f32) {
    PANEL.pad()
}

/// The top of bar `bar`'s label row in the panel.
#[cfg(test)]
pub const fn bar_top(bar: Bar) -> f32 {
    PANEL.bar_top(bar)
}

/// The middle of bar `bar`'s track, down the panel.
#[cfg(test)]
pub const fn track_y(bar: Bar) -> f32 {
    PANEL.track_y(bar)
}

/// The panel's track's left end and its length.
#[cfg(test)]
pub fn track() -> (f32, f32) {
    PANEL.track()
}

/// The middle of a bar's thumb across the panel at `value`.
#[cfg(test)]
pub fn thumb_x(value: f32) -> f32 {
    PANEL.thumb_x(value)
}

/// The top of the panel's note row, under the last bar.
#[cfg(test)]
pub const fn note_top() -> f32 {
    PANEL.note_top()
}

/// How tall the panel is.
pub const fn height() -> f32 {
    PANEL.height()
}

/// How many discs the palettes row has: no colour, then each palette.
pub const fn palette_count() -> usize {
    1 + PALETTE_COLORS.len()
}

/// Palette `index`'s colours, the main first; none for the first disc, no colour.
pub fn palette_colors(index: usize) -> Vec<u32> {
    if index == 0 || index >= palette_count() {
        return Vec::new();
    }
    let start: usize = PALETTE_COLORS[..index - 1]
        .iter()
        .map(|&n| n as usize)
        .sum();
    PALETTES[start..start + PALETTE_COLORS[index - 1] as usize].to_vec()
}

/// The theme palette `index` gives a Space whose theme is `theme`: its colours, the rest of the
/// theme as it was; none for the first disc.
pub fn palette_theme(index: usize, theme: Option<&Theme>) -> Option<Theme> {
    let colors = palette_colors(index);
    let main = Theme::of_color(*colors.first()?);
    let mut made = match theme {
        Some(theme) => Theme {
            x: main.x,
            y: main.y,
            saturation: main.saturation,
            third: None,
            places: None,
            ..*theme
        },
        None => main,
    };
    made.count = colors.len().min(MAX_COLORS);
    if colors.len() > 1 {
        let mut places = [made.place(1), made.place(2)];
        for (place, &color) in places.iter_mut().zip(&colors[1..]) {
            let other = Theme::of_color(color);
            *place = theme::place_of(made.x, made.y, other.x, other.y);
        }
        made.places = Some(places);
    }
    Some(made)
}

/// Which palette `theme` is, if it is one: the first disc for none, and none once a colour has
/// been changed by hand.
pub fn palette_of(theme: Option<&Theme>) -> Option<usize> {
    let Some(theme) = theme else {
        return Some(0);
    };
    (1..palette_count()).find(|&index| {
        palette_theme(index, Some(theme)).is_some_and(|made| same_colors(&made, theme))
    })
}

/// Whether two themes show the same colours, a step of a channel apart at most, as the text the
/// core keeps rounds a theme's places.
fn same_colors(a: &Theme, b: &Theme) -> bool {
    let near = |a: u32, b: u32| {
        [16, 8, 0]
            .iter()
            .all(|shift| ((a >> shift) & 0xff).abs_diff((b >> shift) & 0xff) <= 2)
    };
    let (a, b) = (a.colors(), b.colors());
    a.len() == b.len() && a.iter().zip(&b).all(|(&a, &b)| near(a, b))
}

/// Palette `index`'s name, which its disc reads as.
pub fn palette_name(index: usize) -> Option<&'static str> {
    PALETTE_NAMES.get(index).copied()
}

// Each disc has its name.
const _: () = assert!(PALETTE_NAMES.len() == palette_count());

/// What the palettes' label shows at its right for a Space themed `theme`: the chosen palette's
/// name, and none for a look changed by hand, which is no palette's.
pub fn chosen_name(theme: Option<&Theme>) -> Option<&'static str> {
    palette_of(theme).and_then(palette_name)
}

/// The pad's point under the field's point (x, y): across from the hue, down from the lightness,
/// each from 0 to 1 over where the dots are.
pub fn pad_point(x: f32, y: f32) -> (f32, f32) {
    let hue = y.atan2(x).rem_euclid(TAU) / TAU;
    let across = (hue - HUE_START).rem_euclid(1.);
    let down = (x.hypot(y) - LIGHT_TOP) / (LIGHT_BOTTOM - LIGHT_TOP);
    (across, down.clamp(0., 1.))
}

/// The field's point under the pad's point (across, down).
pub fn field_point(across: f32, down: f32) -> (f32, f32) {
    let angle = (HUE_START + across) * TAU;
    let light = LIGHT_TOP + (LIGHT_BOTTOM - LIGHT_TOP) * down.clamp(0., 1.);
    (light * angle.cos(), light * angle.sin())
}

/// Where on the panel the pad's point (across, down) is.
#[cfg(test)]
pub fn on_panel(across: f32, down: f32) -> (f32, f32) {
    PANEL.on_panel(across, down)
}

/// Where colour `index`'s handle is on the panel for `theme`.
#[cfg(test)]
pub fn handle(theme: &Theme, index: usize) -> (f32, f32) {
    PANEL.handle(theme, index)
}

/// The middle of the panel's chip's disc for colour `index`, or the plus's.
#[cfg(test)]
pub fn chip_item(count: usize, index: usize) -> (f32, f32) {
    PANEL.chip_item(count, index)
}

/// The panel's value field's left and top.
#[cfg(test)]
pub fn value_field() -> (f32, f32) {
    PANEL.value_field()
}

/// The colour a typed value names: six hex digits, with or without the sign; none for anything
/// else.
pub fn typed_color(text: &str) -> Option<u32> {
    let digits = text.trim().strip_prefix('#').unwrap_or(text.trim());
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}

/// How a colour reads in the value field: its sign and six digits, in capitals, as drawn.
pub fn color_text(color: u32) -> String {
    format!("#{color:06X}")
}
