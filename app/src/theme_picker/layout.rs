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

/// The panel's width inside its padding.
pub const INNER: f32 = WIDTH - 2. * PADDING;
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

/// Appearance segment `index`'s left and top, the three at the head's right.
pub fn segment(index: usize) -> (f32, f32) {
    let frame = 3. * SEGMENT_WIDTH + 2. * SEGMENT_INSET;
    let left = WIDTH - PADDING - frame + SEGMENT_INSET + index as f32 * SEGMENT_WIDTH;
    let top = PADDING + (HEAD - SEGMENT_HEIGHT) / 2.;
    (left, top)
}

/// The top of the palettes' label row.
pub const fn palettes_top() -> f32 {
    PADDING + HEAD + GAP
}

/// The middle of palette disc `index`'s slot, eight across the panel's inside, spaced between.
pub fn disc(index: usize) -> (f32, f32) {
    let count = palette_count() as f32;
    let x = PADDING + SLOT / 2. + index as f32 * (INNER - SLOT) / (count - 1.);
    (x, palettes_top() + LABEL + PALETTES_GAP + SLOT / 2.)
}

/// The pad's top left.
pub const fn pad() -> (f32, f32) {
    (PADDING, palettes_top() + LABEL + PALETTES_GAP + SLOT + GAP)
}

/// The top of bar `bar`'s label row.
pub const fn bar_top(bar: Bar) -> f32 {
    let first = pad().1 + PAD_HEIGHT + GAP;
    first + bar.index() as f32 * (LABEL + BAR_LABEL_GAP + THUMB + BAR_GAP)
}

/// The middle of bar `bar`'s track, down the panel.
pub const fn track_y(bar: Bar) -> f32 {
    bar_top(bar) + LABEL + BAR_LABEL_GAP + THUMB / 2.
}

/// The track's left end and its length, in from the panel's inside by the label's 2.
pub fn track() -> (f32, f32) {
    (PADDING + LABEL_INSET, INNER - 2. * LABEL_INSET)
}

/// The middle of a bar's thumb across the panel at `value`, 0 to 1: its edge at the track's ends.
pub fn thumb_x(value: f32) -> f32 {
    let (left, length) = track();
    left + THUMB / 2. + value.clamp(0., 1.) * (length - THUMB)
}

/// The value for a thumb whose middle is at `x`.
pub fn value_at(x: f32) -> f32 {
    let (left, length) = track();
    ((x - left - THUMB / 2.) / (length - THUMB)).clamp(0., 1.)
}

/// The top of the note row, under the last bar.
pub const fn note_top() -> f32 {
    track_y(BARS[BARS.len() - 1]) + THUMB / 2. + GAP
}

/// How tall the panel is.
pub const fn height() -> f32 {
    note_top() + LABEL + PADDING
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
pub fn on_panel(across: f32, down: f32) -> (f32, f32) {
    let (left, top) = pad();
    let width = DOT_PITCH * (DOTS_ACROSS - 1.);
    let height = DOT_PITCH * (DOTS_DOWN - 1.);
    (
        left + DOT_LEFT + across.rem_euclid(1.) * width,
        top + DOT_TOP + down * height,
    )
}

/// The pad's point under (x, y) on the panel, across wrapping round and down held to the pad.
pub fn on_pad(x: f32, y: f32) -> (f32, f32) {
    let (left, top) = pad();
    let width = DOT_PITCH * (DOTS_ACROSS - 1.);
    let height = DOT_PITCH * (DOTS_DOWN - 1.);
    (
        ((x - left - DOT_LEFT) / width).clamp(0., 1.),
        ((y - top - DOT_TOP) / height).clamp(0., 1.),
    )
}

/// Where colour `index`'s handle is on the panel for `theme`.
pub fn handle(theme: &Theme, index: usize) -> (f32, f32) {
    let (x, y) = theme.handle(index);
    let (across, down) = pad_point(x, y);
    on_panel(across, down)
}

/// The Points chip: its left, its top and its width, with a disc for each colour and the plus
/// while there's room for one more.
pub fn chip(count: usize) -> (f32, f32, f32) {
    let (left, top) = pad();
    let plus = if count < MAX_COLORS {
        CHIP_GAP + PLUS
    } else {
        0.
    };
    let discs = count as f32 * CHIP_DOT + count.saturating_sub(1) as f32 * CHIP_GAP;
    let width = CHIP_LEFT + discs + plus + CHIP_RIGHT;
    (
        left + CHIP_INSET,
        top + PAD_HEIGHT - CHIP_INSET - CHIP_HEIGHT,
        width,
    )
}

/// The middle of the chip's disc for colour `index`, or with `index` the count, the plus's.
pub fn chip_item(count: usize, index: usize) -> (f32, f32) {
    let (left, top, _) = chip(count);
    let y = top + CHIP_HEIGHT / 2.;
    let before = left + CHIP_LEFT + index as f32 * (CHIP_DOT + CHIP_GAP);
    if index < count {
        (before + CHIP_DOT / 2., y)
    } else {
        (before + PLUS / 2., y)
    }
}

/// The value field's left and top, at the pad's foot on the right.
pub fn value_field() -> (f32, f32) {
    let (left, top) = pad();
    (
        left + PAD_WIDTH - CHIP_INSET - FIELD_WIDTH,
        top + PAD_HEIGHT - CHIP_INSET - CHIP_HEIGHT,
    )
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
