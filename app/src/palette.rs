//! What the chrome draws with over a Space's theme, or with none (#320's rules): the window's
//! tint and how strongly the theme lies over it, the text and glyphs' alphas, the Space's accent
//! and the label on it, and the opaque fills of what floats over the page.
//!
//! The window is translucent, Dia's measured composite (check/window.sh), so what's behind it
//! shows through, and a theme is the user's, anything the picker makes. So what's drawn over it
//! is chosen against the window's every point over a black and a white backdrop, the extremes:
//! the ink's alphas are floored where they'd fade, a theme the ink can't read on takes the least
//! shade of the tint's colour that lets it read, and the accent is the main colour moved in OKLab
//! lightness, its hue kept, until it reads everywhere it's drawn. A dark window draws Paper over
//! its material under black, a light one Ink under white. It's worked out once when a
//! Space's theme changes, never while drawing. The tests sweep the picker's output and hold every
//! component to WCAG's minimums.

use gpui::{
    BoxShadow, ColorSpace, Div, Global, Hsla, Rgba, div, linear_color_stop, linear_gradient, point,
    prelude::*, px, rgb, rgba,
};

use crate::theme::{self, DEFAULT_INTENSITY, GRAIN_STRENGTH, Theme};

pub use crate::tokens::palette::{INK, NO_THEME_ACCENT, OVERLAY_DARK, PAPER};
use crate::tokens::icon_picker::{
    FILL as CARD, HINT as CARD_HINT, LABEL as CARD_LABEL, SEGMENTS_FILL as CARD_SEGMENTS,
};
use crate::tokens::palette::{
    DIALOG_DARK, DIALOG_LIGHT, GRADIENT_ALPHA, GROUND_OVER_BLACK, GROUND_OVER_WHITE, LIGHT_GRADIENT_ALPHA,
    LIGHT_GROUND_OVER_BLACK, LIGHT_GROUND_OVER_WHITE, LIGHT_NEUTRAL_GRADIENT_ALPHA, LIGHT_STAND_IN,
    LIGHT_TINT, NEUTRAL, NEUTRAL_GRADIENT_ALPHA, OVERLAY_LIGHT, STAND_IN, TINT,
};
pub use crate::tokens::palette::SECOND_AT;
pub use crate::tokens::palette::{STATE_HOVERED, STATE_PRESSED, STATE_SELECTED, STATE_SELECTED_HOVERED};
use crate::tokens::shell::library::{COLUMN, LIBRARY_PLATE};
use crate::tokens::shell::{CARD as PAGE_CARD, LIGHT_CARD as LIGHT_PAGE_CARD};
use crate::tokens::palette::{ACCENT_HOVERED, ACCENT_PRESSED, INTENSITY_FLOOR, INTENSITY_SPAN, RING, SELECTION};
use crate::tokens::palette::{
    LIGHT_SELECTED, LIGHT_SELECTED_HOVERED, LIGHT_SELECTED_SHADOW, LIGHT_SELECTED_SHADOW_BLUR,
    LIGHT_SELECTED_SHADOW_Y,
};
/// The ends of the scale each family's text runs toward, which the accent's search needs, and the
/// ground a swatch's colours are judged on.
const BLACK: Rgba = Rgba { r: 0., g: 0., b: 0., a: 1. };
const WHITE: Rgba = Rgba { r: 1., g: 1., b: 1., a: 1. };
/// The shade's steps, from none up.
const SHADE_STEP: f32 = 0.05;
/// WCAG's minimums: text, and glyphs, boundaries and focus rings.
pub const TEXT: f32 = 4.5;
pub const GLYPH: f32 = 3.;
/// The accent's label aims past TEXT, so rounding to 8 bits can't take it under.
const LABEL: f32 = 4.6;
/// Grain at 1 moves the window toward the text's colour by half its strength on the whole, where
/// the text has least room.
const GRAIN_SHIFT: f32 = GRAIN_STRENGTH * 0.5;
/// The plates under rows, in Paper: the selected row's, one under the pointer, the URL field's,
/// a Library section's and a split target's.
pub const SELECTED_PLATE: f32 = 0.2;
pub const HOVERED_PLATE: f32 = 0.12;
pub const FIELD_PLATE: f32 = 0.1;
pub const SECTION_PLATE: f32 = 0.09;
pub const TARGET_PLATE: f32 = 0.06;
/// A split target lies the accent at this over its plate.
pub const TARGET_ACCENT: f32 = 0.15;
/// Where on the window the colours are checked: across the gradient's line and down it. The
/// tests check a finer grid.
const ALONG: [f32; 9] = [0., 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.];
const DOWN: [f32; 9] = ALONG;
/// The window between those points can dip a little under them, so what's checked at them has
/// this much to spare.
const BETWEEN: f32 = 0.05;

/// Which way a window looks: dark, Paper over its material under black, or light, Ink over its
/// material under white (docs/ux-spec.md › Window material).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Dark,
    Light,
}

impl Tone {
    /// The text and glyphs' colour.
    pub fn ink(self) -> Rgba {
        match self {
            Tone::Dark => rgb(PAPER),
            Tone::Light => rgb(INK),
        }
    }

    /// The colour the window's material lies under, and a theme the ink can't read on is shaded
    /// toward.
    fn tint_color(self) -> Rgba {
        match self {
            Tone::Dark => BLACK,
            Tone::Light => WHITE,
        }
    }

    /// How much of `tint_color` lies over the material.
    fn tint(self) -> f32 {
        match self {
            Tone::Dark => TINT,
            Tone::Light => LIGHT_TINT,
        }
    }

    /// The end of the scale the ink runs toward, which an accent has to stand out from the window
    /// toward.
    fn end(self) -> Rgba {
        match self {
            Tone::Dark => WHITE,
            Tone::Light => BLACK,
        }
    }

    /// The tinted material over a black and over a white backdrop.
    fn grounds(self) -> [u32; 2] {
        match self {
            Tone::Dark => [GROUND_OVER_BLACK, GROUND_OVER_WHITE],
            Tone::Light => [LIGHT_GROUND_OVER_BLACK, LIGHT_GROUND_OVER_WHITE],
        }
    }

    fn stand_in(self) -> Rgba {
        rgb(match self {
            Tone::Dark => STAND_IN,
            Tone::Light => LIGHT_STAND_IN,
        })
    }

    /// How opaque a theme's colours lie over the window at the intensity a theme starts with, and
    /// a neutral one's.
    fn gradient_alphas(self) -> (f32, f32) {
        match self {
            Tone::Dark => (GRADIENT_ALPHA, NEUTRAL_GRADIENT_ALPHA),
            Tone::Light => (LIGHT_GRADIENT_ALPHA, LIGHT_NEUTRAL_GRADIENT_ALPHA),
        }
    }

    /// The selected row's plate, at rest and under the pointer: the ink on a dark window, as
    /// Arc's dark rows are, and white on a light one, as its light rows are.
    fn selected(self) -> (Rgba, f32, f32) {
        match self {
            Tone::Dark => (rgb(PAPER), STATE_SELECTED, STATE_SELECTED_HOVERED),
            Tone::Light => (WHITE, LIGHT_SELECTED, LIGHT_SELECTED_HOVERED),
        }
    }

    /// The card the pages lie in, what shows before a page first paints.
    fn page(self) -> Rgba {
        rgba(match self {
            Tone::Dark => PAGE_CARD,
            Tone::Light => LIGHT_PAGE_CARD,
        })
    }

    /// The fills of what floats over the window, and of a dialog.
    fn overlays(self) -> (Rgba, Rgba) {
        match self {
            Tone::Dark => (rgb(OVERLAY_DARK), rgb(DIALOG_DARK)),
            Tone::Light => (rgb(OVERLAY_LIGHT), rgb(DIALOG_LIGHT)),
        }
    }
}

/// The colours the chrome draws with for one Space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Whether the window is dark or light.
    pub tone: Tone,
    /// Black or white over the window's material: the tone's tint, and the theme's shade over
    /// that.
    pub tint: Rgba,
    /// The window's ground where it has to be opaque, under rows it covers or in the reveal's
    /// panel over the page.
    pub underlay: Rgba,
    /// How opaque the theme's colours lie over the ground; 0 without a theme.
    pub layer_alpha: f32,
    /// The tint's colour over the tint for a theme the ink can't read on over a backdrop; 0 for
    /// most, and always without a theme.
    pub shade: f32,
    /// The text and glyphs' colour, Paper on a dark window and Ink on a light one.
    pub ink: Rgba,
    /// The least alpha at which `ink` reads as text, and as a glyph, on the window and its
    /// plates everywhere.
    pub text_floor: f32,
    pub glyph_floor: f32,
    /// The Space's accent, for controls: filled rows and buttons, focus rings, toggles.
    pub accent: Rgba,
    /// Ink or Paper, whichever reads more on the accent.
    pub accent_label: Rgba,
    /// The selected Library section's glyph: the accent where it reads on the column under the
    /// section's plate, and otherwise the colour nearest it at its hue that does, so the accent
    /// itself doesn't move for one glyph.
    pub section_glyph: Rgba,
    /// The opaque fill of what floats over the page: the command bar, menus, the theme picker's
    /// panel, toasts, the zoom HUD.
    pub overlay: Rgba,
    /// A dialog's fill.
    pub dialog: Rgba,
    /// The least alpha at which `ink` reads as text, and as a glyph, on the overlay and the
    /// dialog.
    pub overlay_floor: f32,
    pub overlay_glyph_floor: f32,
    /// The card the pages lie in, over the window.
    pub page: Rgba,
    /// The icon and theme pickers' card, over what's behind it: Arc's grey where the card's text
    /// reads on it over the window, and otherwise the least step toward the dark overlay, opaque,
    /// at which it does.
    pub card: Rgba,
    /// The Space's mark.
    pub swatch: Swatch,
}

/// A Space's mark: its theme's colours as a small swatch (#320), drawn in the same gradient the
/// window is, with a hairline ring of Ink or Paper where its edge would fade into what's behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    /// The gradient's ends: the main colour and the second along 135°, reaching it SECOND_AT of
    /// the way, or a colour alone going lighter down.
    from: Rgba,
    to: Rgba,
    angle: f32,
    to_at: f32,
    /// A third colour coming in toward the bottom.
    third: Option<Rgba>,
    /// The relative luminance of its darkest and its lightest points.
    darkest: f32,
    lightest: f32,
}

impl Global for Palette {}

impl Default for Palette {
    fn default() -> Self {
        Palette::of(None, Tone::Dark)
    }
}

impl Palette {
    /// The palette for a Space with `theme`, or none, in a window of `tone`.
    pub fn of(theme: Option<&Theme>, tone: Tone) -> Palette {
        let Some(theme) = theme else {
            return Palette::untinted(tone);
        };
        let window = Window::of(theme, tone);
        let (shade, layer_alpha) = window.fit();
        let palette = Palette::with(shade, layer_alpha, &window);
        let accent = palette.solve_accent(rgb(theme.main_color()), &window);
        let section_glyph = palette.solve_section_glyph(accent, &window);
        let swatch = Swatch::of(Some(theme), &window);
        Palette { accent, accent_label: label_on(accent), section_glyph, swatch, ..palette }
    }

    /// No theme: the tinted material alone, never shaded, and ilo green, as light as it has to
    /// be to read on it, as the accent.
    fn untinted(tone: Tone) -> Palette {
        let window = Window::of_none(tone);
        let palette = Palette::with(0., 0., &window);
        let accent = palette.solve_accent(rgb(NO_THEME_ACCENT), &window);
        let section_glyph = palette.solve_section_glyph(accent, &window);
        let swatch = Swatch::of(None, &window);
        Palette { accent, accent_label: label_on(accent), section_glyph, swatch, ..palette }
    }

    fn with(shade: f32, layer_alpha: f32, window: &Window) -> Palette {
        let tone = window.tone;
        let ink = tone.ink();
        let (overlay, dialog) = tone.overlays();
        let mut palette = Palette {
            tone,
            tint: Rgba { a: 1. - (1. - tone.tint()) * (1. - shade), ..tone.tint_color() },
            underlay: over(tone.stand_in(), tone.tint_color(), shade),
            layer_alpha,
            shade,
            ink,
            text_floor: 0.,
            glyph_floor: 0.,
            accent: ink,
            accent_label: ink,
            section_glyph: ink,
            overlay,
            dialog,
            overlay_floor: 0.,
            overlay_glyph_floor: 0.,
            page: tone.page(),
            card: rgba(CARD),
            swatch: Swatch::of(None, window),
        };
        // Text and glyphs on an overlay read on it and on a control's plate under the pointer.
        let under = state_alpha(OVERLAY_REST, STATE_HOVERED);
        let overlays: Vec<Rgba> = [palette.overlay, palette.dialog]
            .into_iter()
            .flat_map(|fill| [fill, over(fill, ink, under)])
            .collect();
        palette.overlay_floor = least_alpha(ink, &overlays, TEXT + BETWEEN);
        palette.overlay_glyph_floor = least_alpha(ink, &overlays, GLYPH + BETWEEN);
        palette.card = palette.solve_card(window);
        let grounds = palette.grounds(window);
        palette.text_floor = least_alpha(ink, &grounds, TEXT + BETWEEN);
        palette.glyph_floor = least_alpha(ink, &grounds, GLYPH + BETWEEN);
        palette
    }

    /// The window at every point checked over both backdrops, shaded, with grain moving it
    /// toward the ink; and under the selected, hovered and field plates.
    fn grounds(&self, window: &Window) -> Vec<Rgba> {
        let plain = window.samples(self.shade, self.layer_alpha);
        let mut grounds: Vec<Rgba> =
            plain.iter().map(|&at| over(at, self.ink, GRAIN_SHIFT * window.grain)).collect();
        // A light window's selected row is white, and a control's states still reach the ink's
        // selected plate.
        let (selected, alpha, _) = self.tone.selected();
        grounds.extend(plain.iter().map(|&at| over(at, selected, alpha)));
        for plate in [SELECTED_PLATE, HOVERED_PLATE, FIELD_PLATE] {
            grounds.extend(plain.iter().map(|&at| over(at, self.ink, plate)));
        }
        grounds
    }

    /// This palette `t` of the way to `other`'s, as the window's colour goes from one Space's to
    /// the next while the sidebar pages between them; the label and the swatch change halfway.
    pub fn toward(&self, other: &Palette, t: f32) -> Palette {
        if t <= 0. {
            return *self;
        }
        if t >= 1. {
            return *other;
        }
        let mix = |a: Rgba, b: Rgba| Rgba {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        };
        let (near, _) = if t < 0.5 { (self, other) } else { (other, self) };
        Palette {
            tone: near.tone,
            underlay: mix(self.underlay, other.underlay),
            tint: mix(self.tint, other.tint),
            layer_alpha: self.layer_alpha + (other.layer_alpha - self.layer_alpha) * t,
            shade: self.shade + (other.shade - self.shade) * t,
            ink: mix(self.ink, other.ink),
            text_floor: self.text_floor.max(other.text_floor),
            glyph_floor: self.glyph_floor.max(other.glyph_floor),
            accent: mix(self.accent, other.accent),
            accent_label: near.accent_label,
            section_glyph: mix(self.section_glyph, other.section_glyph),
            overlay: near.overlay,
            dialog: near.dialog,
            overlay_floor: self.overlay_floor.max(other.overlay_floor),
            overlay_glyph_floor: self.overlay_glyph_floor.max(other.overlay_glyph_floor),
            page: mix(self.page, other.page),
            card: near.card,
            swatch: near.swatch,
        }
    }

    /// The accent under the pointer, darker, as every filled button goes.
    pub fn accent_hovered(&self) -> Hsla {
        let Rgba { r, g, b, a } = self.accent;
        Rgba { r: r * ACCENT_HOVERED, g: g * ACCENT_HOVERED, b: b * ACCENT_HOVERED, a }.into()
    }

    /// The accent pressed, a step darker than under the pointer, whatever its label (the owner).
    pub fn accent_pressed(&self) -> Hsla {
        let Rgba { r, g, b, a } = self.accent;
        Rgba { r: r * ACCENT_PRESSED, g: g * ACCENT_PRESSED, b: b * ACCENT_PRESSED, a }.into()
    }

    /// Behind selected text in a field: the accent, faint.
    pub fn selection(&self) -> Hsla {
        Hsla::from(self.accent).opacity(SELECTION)
    }

    /// Ink or Paper at `alpha`, or as much more as text needs to read on any theme.
    pub fn text(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.text_floor))
    }

    /// Ink or Paper at `alpha`, or as much more as a glyph or boundary needs.
    pub fn glyph(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.glyph_floor))
    }

    /// Ink or Paper at `alpha` over the overlay or the dialog, or as much more as text needs.
    pub fn on_overlay(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.overlay_floor))
    }

    /// Ink or Paper at `alpha` over the overlay or the dialog, or as much more as a glyph needs.
    pub fn on_overlay_glyph(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.overlay_glyph_floor))
    }

    /// Ink or Paper at `alpha`, as it is: for what needn't read, such as a disabled glyph or a
    /// shadow.
    pub fn faint(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha)
    }

    /// A control's fill in `state`, a step of the ladder, laid over its resting plate `rest`, as
    /// one plate of the ink.
    pub fn state(&self, rest: f32, state: f32) -> Hsla {
        self.plate(state_alpha(rest, state))
    }

    /// A control whose resting fill `rest` is opaque and not of the ink, in `state`.
    pub fn state_over(&self, rest: Rgba, state: f32) -> Hsla {
        over(rest, self.ink, state).into()
    }

    /// A step of the ladder on a surface that's dark on any theme, with Paper's text on it, such as
    /// the find bar, the icon picker and Settings.
    pub fn on_dark(state: f32) -> Hsla {
        Hsla::from(rgb(PAPER)).opacity(state)
    }

    /// The selected row's plate, `hover` of the way to its fill under the pointer.
    pub fn selected(&self, hover: f32) -> Hsla {
        let (color, rest, under) = self.tone.selected();
        Hsla::from(color).opacity(rest + (under - rest) * hover)
    }

    /// What lifts the selected row's plate off a light window, at `opacity`; nothing on a dark
    /// one.
    pub fn selected_shadow(&self, opacity: f32) -> Vec<BoxShadow> {
        if self.tone == Tone::Dark {
            return Vec::new();
        }
        vec![BoxShadow {
            color: Hsla::from(BLACK).opacity(LIGHT_SELECTED_SHADOW * opacity),
            offset: point(px(0.), px(LIGHT_SELECTED_SHADOW_Y)),
            blur_radius: px(LIGHT_SELECTED_SHADOW_BLUR),
            spread_radius: px(0.),
            inset: false,
        }]
    }

    /// A plate under a row or a field, the ink at `alpha`.
    pub fn plate(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha)
    }

    /// The window `along` the gradient's line and `y` down it, with `theme` (this palette's own),
    /// as it's seen over a black and over a white backdrop.
    pub fn window_at(&self, theme: Option<&Theme>, along: f32, y: f32) -> [Rgba; 2] {
        let layer = theme.map_or(Layer::none(), |theme| Layer::of(theme, along, y));
        grounds(self.tone, self.shade).map(|ground| layer.over(ground, self.layer_alpha))
    }

    /// The window where its theme's colour is `layer`, translucent, over a black and over a
    /// white backdrop: what a mark on it is judged against.
    pub fn over_backdrops(&self, layer: Hsla) -> [Rgba; 2] {
        grounds(self.tone, self.shade).map(|ground| Hsla::from(ground).blend(layer).to_rgb())
    }

    /// The hairline ring a swatch of `theme`'s colours needs on `behind`, opaque: Ink or Paper,
    /// whichever stands out, where any of the swatch is under 3:1 against it; none where all of
    /// it reads.
    pub fn swatch_ring(theme: Option<&Theme>, behind: Rgba) -> Option<Rgba> {
        Palette::swatch_ring_on(theme, &[behind])
    }

    /// The ring a swatch of `theme`'s colours needs on any of `behind`.
    pub fn swatch_ring_on(theme: Option<&Theme>, behind: &[Rgba]) -> Option<Rgba> {
        Swatch::of_theme(theme).ring_on(behind)
    }

    /// The ring a swatch of two colours alone needs on any of `behind`, by the same rule as a
    /// theme's: a Space's swatch in a menu, `from` at its top left to `to` at its bottom right.
    pub fn swatch_ring_of(from: Rgba, to: Rgba, behind: &[Rgba]) -> Option<Rgba> {
        let (a, b) = (luminance(from), luminance(to));
        let (darkest, lightest) = (a.min(b), a.max(b));
        let swatch = Swatch { from, to, angle: 135., to_at: 1., third: None, darkest, lightest };
        swatch.ring_on(behind)
    }

    /// Where a swatch ring is checked against this palette's window: every point checked, over
    /// both backdrops.
    pub fn window_samples(&self, theme: Option<&Theme>) -> Vec<Rgba> {
        let window = theme.map_or(Window::of_none(self.tone), |theme| Window::of(theme, self.tone));
        window.samples(self.shade, self.layer_alpha)
    }

    /// The pickers' card as they draw it. Only the Mac's host blurs what's behind a picker;
    /// elsewhere what's behind shows sharp through the card and reads as noise, so there the card
    /// is opaque over the dark overlay.
    pub fn card_drawn(&self) -> Rgba {
        if cfg!(target_os = "macos") {
            return self.card;
        }
        over(rgb(OVERLAY_DARK), self.card, self.card.a)
    }

    /// The pickers' card: Arc's measured grey, then a step at a time toward the dark overlay,
    /// opaque, until its hint and its segments' labels read on it over every point of the window.
    fn solve_card(&self, window: &Window) -> Rgba {
        let (measured, opaque) = (rgba(CARD), rgb(OVERLAY_DARK));
        let behind = window.samples(self.shade, self.layer_alpha);
        let steps = (1. / SHADE_STEP).round() as i32;
        let toward = |t: f32| Rgba {
            r: measured.r + (opaque.r - measured.r) * t,
            g: measured.g + (opaque.g - measured.g) * t,
            b: measured.b + (opaque.b - measured.b) * t,
            a: measured.a + (opaque.a - measured.a) * t,
        };
        (0..=steps).map(|step| toward(step as f32 * SHADE_STEP)).find(|&fill| {
            behind.iter().all(|&at| {
                let card = over(at, fill, fill.a);
                let segments = over(card, WHITE, rgba(CARD_SEGMENTS).a);
                contrast(over(card, WHITE, rgba(CARD_HINT).a), card) >= TEXT + BETWEEN
                    && contrast(rgb(CARD_LABEL), segments) >= TEXT + BETWEEN
            })
        })
        .unwrap_or(opaque)
    }

    /// The main colour, its hue kept, at the OKLab lightness nearest its own at which it reads
    /// against the window, the overlays, a Library section's plate and a split target, and its
    /// label reads on it; with as much of its chroma as the gamut there allows. Grey, at worst.
    fn solve_accent(&self, main: Rgba, window: &Window) -> Rgba {
        let plain = window.samples(self.shade, self.layer_alpha);
        let section: Vec<Rgba> = plain.iter().map(|&at| over(at, self.ink, SECTION_PLATE)).collect();
        let target: Vec<Rgba> = plain.iter().map(|&at| over(at, self.ink, TARGET_PLATE)).collect();
        let passes = |c: Rgba| {
            contrast(rgb(INK), c).max(contrast(rgb(PAPER), c)) >= LABEL
                && contrast(c, self.overlay) >= GLYPH
                && contrast(c, self.dialog) >= GLYPH
                && plain.iter().chain(&section).all(|&at| contrast(c, at) >= GLYPH + BETWEEN)
                && target.iter().all(|&at| contrast(c, over(at, c, TARGET_ACCENT)) >= GLYPH + BETWEEN)
        };
        nearest(main, passes).unwrap_or(self.ink)
    }

    /// The accent, or the colour nearest it at its hue, that reads at 3:1 on the Library's column
    /// under the selected section's plate, everywhere on the window.
    fn solve_section_glyph(&self, accent: Rgba, window: &Window) -> Rgba {
        let selected: Vec<Rgba> = (window.samples(self.shade, self.layer_alpha).iter())
            .map(|&at| over(over(at, self.ink, COLUMN), self.ink, LIBRARY_PLATE))
            .collect();
        let passes = |c: Rgba| selected.iter().all(|&at| contrast(c, at) >= GLYPH + BETWEEN);
        nearest(accent, passes).unwrap_or(self.ink)
    }
}

/// The colour at `main`'s hue that `passes`, at the OKLab lightness nearest its own, with as much
/// of its chroma as the gamut there allows.
fn nearest(main: Rgba, passes: impl Fn(Rgba) -> bool) -> Option<Rgba> {
    let [l, a, b] = theme::oklab(main);
    // A hundredth at a time, either way from its own lightness, the nearest first: with all its
    // chroma the gamut allows, then half, then none.
    for chroma in [1., 0.5, 0.] {
        for step in 0..=100 {
            for sign in [1., -1.] {
                let at = l + sign * step as f32 * 0.01;
                if !(0. ..=1.).contains(&at) || (step == 0 && sign < 0.) {
                    continue;
                }
                let color = in_gamut(at, a * chroma, b * chroma);
                if passes(color) {
                    return Some(color);
                }
            }
        }
    }
    None
}

impl Swatch {
    /// `theme`'s swatch, or ilo green's for none, without the rest of its palette.
    pub fn of_theme(theme: Option<&Theme>) -> Swatch {
        match theme {
            Some(theme) => Swatch::of(Some(theme), &Window::of(theme, Tone::Dark)),
            None => Swatch::of(None, &Window::of_none(Tone::Dark)),
        }
    }

    fn of(theme: Option<&Theme>, window: &Window) -> Swatch {
        let Some(theme) = theme else {
            let accent = rgb(NO_THEME_ACCENT);
            let lum = luminance(accent);
            return Swatch {
                from: accent,
                to: accent,
                angle: 180.,
                to_at: 1.,
                third: None,
                darkest: lum,
                lightest: lum,
            };
        };
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let (from, to, angle, to_at) = match colors[..] {
            [_] => (Layer::of(theme, 0., 0.).color, Layer::of(theme, 0., 1.).color, 180., 1.),
            _ => (colors[0], colors[1], 135., SECOND_AT),
        };
        let lums = window.swatch().into_iter().map(luminance);
        let (darkest, lightest) = lums.fold((f32::MAX, f32::MIN), |(d, l), y| (d.min(y), l.max(y)));
        Swatch { from, to, angle, to_at, third: colors.get(2).copied(), darkest, lightest }
    }

    /// Ink or Paper for a hairline ring where any of the swatch is under 3:1 against any of
    /// `behind`; none where it all reads. Its colours run between its darkest and lightest, so a
    /// ground whose luminance lies between them always takes a ring.
    pub fn ring_on(&self, behind: &[Rgba]) -> Option<Rgba> {
        let reads = |y: f32, at: Rgba| {
            let b = luminance(at);
            (y.max(b) + 0.05) / (y.min(b) + 0.05) >= GLYPH
        };
        let all_read = behind.iter().all(|&at| {
            let b = luminance(at);
            !(self.darkest..=self.lightest).contains(&b)
                && reads(self.darkest, at)
                && reads(self.lightest, at)
        });
        if all_read {
            return None;
        }
        let (ink, paper) = (rgb(INK), rgb(PAPER));
        let worst = |ring: Rgba| behind.iter().map(|&b| contrast(ring, b)).fold(f32::MAX, f32::min);
        Some(if worst(ink) >= worst(paper) { ink } else { paper })
    }

    /// The swatch, `size` across, on any of `behind`.
    pub fn draw(&self, size: f32, behind: &[Rgba]) -> Div {
        let gradient = linear_gradient(
            self.angle,
            linear_color_stop(self.from, 0.),
            linear_color_stop(self.to, self.to_at),
        )
        .color_space(ColorSpace::Oklab);
        div()
            .relative()
            .flex_none()
            .size(px(size))
            .rounded_full()
            .bg(gradient)
            .when_some(self.third, |swatch, third| {
                let third = Hsla::from(third);
                // Rounded itself: GPUI clips a child to its parent's box, not its rounded corners,
                // so a square layer's corners would show under the circle (#1354).
                swatch.child(div().absolute().size_full().rounded_full().bg(linear_gradient(
                    180.,
                    linear_color_stop(third.opacity(0.), 0.),
                    linear_color_stop(third, 1.),
                )))
            })
            .when_some(self.ring_on(behind), |swatch, ring| {
                // The ring is its own layer, over the colours, so it doesn't move them.
                swatch.child(
                    div().absolute().size_full().rounded_full().border(px(RING)).border_color(ring),
                )
            })
    }
}

/// The theme's window, worked out once: how opaque its layer is, and the layer's colours at every
/// point checked.
struct Window {
    tone: Tone,
    alpha: f32,
    layers: Vec<Layer>,
    grain: f32,
}

/// The theme's colour at one point of the window: the gradient's, and a third colour over it at
/// a share of the layer's alpha.
#[derive(Clone, Copy)]
struct Layer {
    color: Rgba,
    third: Option<(Rgba, f32)>,
}

impl Layer {
    fn none() -> Layer {
        Layer { color: BLACK, third: None }
    }

    /// `theme`'s colour `along` the gradient's line (0 at the window's top left, 1 at its bottom
    /// right) and `y` down the window: one colour going to the same at HSL lightness + 0.25 down
    /// the window; two from the main one to the second SECOND_AT along; a third coming in toward
    /// the bottom.
    fn of(theme: &Theme, along: f32, y: f32) -> Layer {
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        if colors.len() == 1 {
            let main = Hsla::from(colors[0]);
            return Layer { color: Hsla { l: (main.l + 0.25 * y).min(1.), ..main }.into(), third: None };
        }
        let along = (along / SECOND_AT).min(1.);
        let color = theme::mix_oklab(colors[0], colors[1], along);
        Layer { color, third: colors.get(2).map(|&third| (third, y)) }
    }

    fn over(&self, base: Rgba, alpha: f32) -> Rgba {
        let color = over(base, self.color, alpha);
        match self.third {
            Some((third, share)) => over(color, third, alpha * share),
            None => color,
        }
    }
}

impl Window {
    /// No theme: the ground alone.
    fn of_none(tone: Tone) -> Window {
        Window { tone, alpha: 0., layers: vec![Layer::none()], grain: 0. }
    }

    fn of(theme: &Theme, tone: Tone) -> Window {
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let alongs: &[f32] = if colors.len() == 1 { &[0.] } else { &ALONG };
        let layers = alongs
            .iter()
            .flat_map(|&along| DOWN.iter().map(move |&y| Layer::of(theme, along, y)))
            .collect();
        Window { tone, alpha: layer_alpha(theme, tone), layers, grain: theme.grain }
    }

    /// The window at every point checked over both backdrops, its ground under the tint's colour
    /// at `shade` and the theme's layer at `alpha`.
    fn samples(&self, shade: f32, alpha: f32) -> Vec<Rgba> {
        grounds(self.tone, shade)
            .into_iter()
            .flat_map(|ground| self.layers.iter().map(move |layer| layer.over(ground, alpha)))
            .collect()
    }

    /// The theme's colours as a swatch shows them, opaque.
    fn swatch(&self) -> Vec<Rgba> {
        self.layers.iter().map(|layer| layer.over(BLACK, 1.)).collect()
    }

    /// The least shade, in steps, at which the ink reads on the window and the selected and
    /// hovered plates over both backdrops, and the end of the scale the ink runs toward stands out
    /// on the whole window, so an accent always exists; and the theme's alpha, its own. Where even
    /// a full shade doesn't do, as a pale third colour lying over the other two can't, the theme's
    /// alpha drops by the least steps that do. A ground of the tint's colour alone always reads.
    fn fit(&self) -> (f32, f32) {
        let (ink, end) = (self.tone.ink(), self.tone.end());
        let (selected, selected_alpha, _) = self.tone.selected();
        let reads = |shade: f32, alpha: f32| {
            self.samples(shade, alpha).iter().all(|&at| {
                let grained = over(at, ink, GRAIN_SHIFT * self.grain);
                contrast(end, at) >= GLYPH + BETWEEN
                    && contrast(ink, grained) >= TEXT + BETWEEN
                    && contrast(ink, over(at, selected, selected_alpha)) >= TEXT + BETWEEN
                    && [SELECTED_PLATE, HOVERED_PLATE]
                        .iter()
                        .all(|&plate| contrast(ink, over(at, ink, plate)) >= TEXT + BETWEEN)
            })
        };
        let steps = (1. / SHADE_STEP).round() as i32;
        let shaded = (0..=steps).map(|step| (step as f32 * SHADE_STEP, self.alpha));
        let faded = (0..steps).rev().map(|step| (1., self.alpha * step as f32 * SHADE_STEP));
        shaded.chain(faded).find(|&(shade, alpha)| reads(shade, alpha)).unwrap_or((1., 0.))
    }
}

/// How opaque `theme`'s colours lie over a window of `tone`, as a Space's colour alone does at
/// the intensity a theme starts with, less for a neutral one.
fn layer_alpha(theme: &Theme, tone: Tone) -> f32 {
    let saturation = Hsla::from(rgb(theme.main_color())).s;
    let (colored, neutral) = tone.gradient_alphas();
    let alpha = if saturation < NEUTRAL { neutral } else { colored };
    let at = |intensity: f32| INTENSITY_FLOOR + INTENSITY_SPAN * intensity;
    alpha * at(theme.intensity) / at(DEFAULT_INTENSITY)
}

/// The window's tinted material over a black and over a white backdrop, under the tint's colour
/// at `shade`.
fn grounds(tone: Tone, shade: f32) -> [Rgba; 2] {
    tone.grounds().map(|ground| over(rgb(ground), tone.tint_color(), shade))
}

/// The most a control on an overlay rests at under its text: a chip's or a button's plate.
const OVERLAY_REST: f32 = 0.1;

/// The alpha of a control resting at `rest` in `state`: the ladder's plate laid over its own, up
/// to the selected row's, the most the window's colour is worked out for text on.
pub const fn state_alpha(rest: f32, state: f32) -> f32 {
    (rest + state - rest * state).min(SELECTED_PLATE)
}

/// Ink or Paper, whichever reads more on `color`.
fn label_on(color: Rgba) -> Rgba {
    let (ink, paper) = (rgb(INK), rgb(PAPER));
    if contrast(ink, color) >= contrast(paper, color) { ink } else { paper }
}

/// The least alpha, in 25ths, at which `ink` over each of `grounds` reaches `minimum`; 1 where
/// none does.
fn least_alpha(ink: Rgba, grounds: &[Rgba], minimum: f32) -> f32 {
    (0..=25)
        .map(|step| step as f32 / 25.)
        .find(|&alpha| grounds.iter().all(|&at| contrast(over(at, ink, alpha), at) >= minimum))
        .unwrap_or(1.)
}

/// OKLab `[l, a, b]` with its chroma cut to the gamut's edge, as 8-bit sRGB.
fn in_gamut(l: f32, a: f32, b: f32) -> Rgba {
    let inside = |share: f32| {
        theme::oklab_to_linear([l, a * share, b * share])
            .iter()
            .all(|&channel| (-1e-5..=1. + 1e-5).contains(&channel))
    };
    let (mut low, mut high) = (0., 1.);
    if inside(1.) {
        low = 1.;
    } else {
        for _ in 0..14 {
            let middle = (low + high) / 2.;
            if inside(middle) { low = middle } else { high = middle }
        }
    }
    let [r, g, b] = theme::from_oklab([l, a * low, b * low]);
    quantized(Rgba { r, g, b, a: 1. })
}

/// `color` as it is once it's 8 bits a channel.
pub fn quantized(color: Rgba) -> Rgba {
    rgb(theme::packed(color))
}

/// `color` at `alpha` over opaque `base`.
pub fn over(base: Rgba, color: Rgba, alpha: f32) -> Rgba {
    let mix = |b: f32, c: f32| b + (c - b) * alpha;
    Rgba { r: mix(base.r, color.r), g: mix(base.g, color.g), b: mix(base.b, color.b), a: 1. }
}

/// WCAG's relative luminance.
pub fn luminance(color: Rgba) -> f32 {
    let [r, g, b] = [color.r, color.g, color.b].map(theme::to_linear);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG's contrast ratio between two opaque colours.
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The palette of the Space shown, for what floats over the page: the command bar, menus, a
/// dialog, the theme picker.
pub fn current(cx: &gpui::App) -> Palette {
    cx.try_global::<Palette>().copied().unwrap_or_default()
}

/// The Space shown changed its palette.
pub fn set_current(palette: Palette, cx: &mut gpui::App) {
    if cx.try_global::<Palette>() != Some(&palette) {
        cx.set_global(palette);
    }
}

#[cfg(test)]
mod tests;
