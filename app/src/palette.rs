//! What the chrome draws with over a Space's theme, or with none (#320's rules): the window's
//! tint and how strongly the theme lies over it, the text and glyphs' alphas, the Space's accent
//! and the label on it, and the fills of what floats over the page.
//!
//! The window is translucent, Dia's measured composite (check/window.sh), so what's behind it
//! shows through, and a theme is the user's, anything the picker makes. So what's drawn over it
//! is chosen against the window's every point over a black and a white backdrop, the extremes:
//! the ink's alphas are floored where they'd fade, a theme the ink can't read on lies fainter by
//! the least steps that let it read, never darkening the window's glass, and the accent is the
//! main colour moved in OKLab lightness, its hue kept, until it reads everywhere it's drawn. A dark window draws Paper over
//! its material under black, a light one Ink under white. It's worked out once when a
//! Space's theme changes, never while drawing. The tests sweep the picker's output and hold every
//! component to WCAG's minimums.

use gpui::{
    BoxShadow, ColorSpace, Div, Global, HighlightStyle, Hsla, Rgba, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, rgb, rgba,
};

use crate::space_transparency::solid_share;
use crate::theme::{self, DEFAULT_INTENSITY, GRAIN_STRENGTH, Theme};

use crate::tokens::icon_picker::{
    FILL as CARD, HINT as CARD_HINT, LABEL as CARD_LABEL, SEGMENTS_FILL as CARD_SEGMENTS,
};
pub use crate::tokens::palette::SECOND_AT;
use crate::tokens::palette::{
    ACCENT_HOVERED, ACCENT_PRESSED, INTENSITY_FLOOR, INTENSITY_SPAN, RING, SELECTION,
    SWATCH_MAIN_TO,
};
pub use crate::tokens::palette::{
    BUTTON_PLATE, CARD_PLATE, FIELD_PLATE, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};
use crate::tokens::palette::{
    DIALOG_DARK, DIALOG_LIGHT, GRADIENT_ALPHA, LIGHT_GRADIENT_ALPHA, LIGHT_MATERIAL_LIFT,
    LIGHT_MATERIAL_OVER_BLACK, LIGHT_MATERIAL_OVER_GREY, LIGHT_MATERIAL_OVER_WHITE,
    LIGHT_MATERIAL_SOLID, LIGHT_NEUTRAL_GRADIENT_ALPHA, LIGHT_STAND_IN, MATERIAL_LIFT,
    MATERIAL_OVER_BLACK, MATERIAL_OVER_GREY, MATERIAL_OVER_WHITE, MATERIAL_SOLID, NEUTRAL,
    NEUTRAL_GRADIENT_ALPHA, OVERLAY_LIGHT, STAND_IN,
};
use crate::tokens::palette::{FAR_GLOW, LIGHT_SHOWN, LIGHT_SHOWN_HOVERED, SHOWN, SHOWN_HOVERED};
use crate::tokens::palette::{GRAY_TEXT, GRAY_TEXT_LESS, GRAY_TEXT_MORE, PLATE_GRAY};
pub use crate::tokens::palette::{INK, NO_THEME_ACCENT, OVERLAY_DARK, PAPER};
use crate::tokens::palette::{
    LIGHT_SELECTED, LIGHT_SELECTED_HOVERED, LIGHT_SELECTED_SHADOW, LIGHT_SELECTED_SHADOW_BLUR,
    LIGHT_SELECTED_SHADOW_Y,
};
pub use crate::tokens::palette::{
    STATE_HOVERED, STATE_PRESSED, STATE_SELECTED, STATE_SELECTED_HOVERED,
};
use crate::tokens::shell::library::{COLUMN, LIBRARY_PLATE};
use crate::tokens::shell::{CARD as PAGE_CARD, LIGHT_CARD as LIGHT_PAGE_CARD};
use crate::tokens::shell::{DROP, LIGHT_UNPAINTED, UNPAINTED};
use crate::tokens::site_controls::{
    TILE_BOTTOM as SITE_CONTROLS_TILE_BOTTOM, TILE_TOP as SITE_CONTROLS_TILE_TOP,
    TINT as SITE_CONTROLS_TINT,
};
use crate::tokens::space_transparency::SOLID as SPACE_SOLID;
use crate::transparency::Transparency;
/// The ends of the scale each family's text runs toward, which the accent's search needs, and the
/// ground a swatch's colours are judged on.
const BLACK: Rgba = Rgba {
    r: 0.,
    g: 0.,
    b: 0.,
    a: 1.,
};
const WHITE: Rgba = Rgba {
    r: 1.,
    g: 1.,
    b: 1.,
    a: 1.,
};
/// Nothing: the glows of a Space without a colour.
const CLEAR: Rgba = Rgba {
    r: 0.,
    g: 0.,
    b: 0.,
    a: 0.,
};
/// The steps a plate's shade rises by from none, and a theme's alpha falls by.
const SHADE_STEP: f32 = 0.05;
/// WCAG's minimums: text, and glyphs, boundaries and focus rings.
pub const TEXT: f32 = 4.5;
pub const GLYPH: f32 = 3.;
/// A plate under the pointer tells from the window around it at this at least: what a dark
/// window's has at Standard over a white backdrop, where it's plain to see. A shade that would
/// take it under would leave the pointer on a plate nobody can see.
pub const TELLS: f32 = 1.2;
/// A plate aims past TELLS, so rounding to 8 bits can't take it under.
const TELLS_AIM: f32 = 1.22;
/// The steps a shade falls by where its plate has to tell: finer than SHADE_STEP, as each one it
/// loses is taken from the text on it.
const TELLS_STEP: f32 = 0.01;
/// The accent's label aims past TEXT, so rounding to 8 bits can't take it under.
const LABEL: f32 = 4.6;
/// Grain at 1 moves the window toward the text's colour by half its strength on the whole, where
/// the text has least room.
const GRAIN_SHIFT: f32 = GRAIN_STRENGTH * 0.5;
/// The plates under rows, in Paper: the selected row's, one under the pointer, a field's (the
/// palette's `field-plate`, a step under the button's, which is checked in its place), a Library
/// section's and a split target's.
pub const SELECTED_PLATE: f32 = 0.2;
pub const HOVERED_PLATE: f32 = 0.12;
pub const SECTION_PLATE: f32 = 0.09;
pub const TARGET_PLATE: f32 = 0.06;
/// A split target lies the accent at this over its plate.
pub const TARGET_ACCENT: f32 = 0.15;
/// Where on a swatch its colours are checked: across its gradient's line and down it.
const ALONG: [f32; 9] = [0., 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.];
const DOWN: [f32; 9] = ALONG;
/// How strong the foot's glow and the far one are where the window is checked: anywhere from none
/// to their centres, as text may lie anywhere under them. The tests check finer steps.
const FOOT: [f32; 5] = [0., 0.25, 0.5, 0.75, 1.];
const FAR: [f32; 2] = [0., 1.];
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

    /// The colour the window's material lies under, and a plate the ink can't read on is shaded
    /// toward.
    fn tint_color(self) -> Rgba {
        match self {
            Tone::Dark => BLACK,
            Tone::Light => WHITE,
        }
    }

    /// What lies over the material where the Space's glass is `glass`: `tint_color` at the bar's
    /// alpha, and in a dark window the drawing's solid ground in its place toward the solid end.
    fn tint(self, glass: f32) -> Rgba {
        let a = crate::space_transparency::tint(glass, self == Tone::Dark);
        let color = match self {
            Tone::Dark => over(BLACK, rgb(SPACE_SOLID), solid_share(glass)),
            Tone::Light => WHITE,
        };
        Rgba { a, ..color }
    }

    /// The end of the scale the ink runs toward, which an accent has to stand out from the window
    /// toward.
    fn end(self) -> Rgba {
        match self {
            Tone::Dark => WHITE,
            Tone::Light => BLACK,
        }
    }

    /// The material under `tint` of `tint_color` at its darkest and its lightest: over a black
    /// backdrop on a black desktop, and over a white one with the most a desktop picture lifts it.
    fn grounds(self, tint: Rgba) -> [Rgba; 2] {
        let [black, ..] = self.materials(0.);
        let [.., white] = self.materials(1.);
        [black, white].map(|under| over(under, tint, tint.a))
    }

    /// What AppKit draws in the material's place with Reduce Transparency or Increase Contrast.
    fn solid(self) -> Rgba {
        rgb(match self {
            Tone::Dark => MATERIAL_SOLID,
            Tone::Light => LIGHT_MATERIAL_SOLID,
        })
    }

    /// The material alone over a black, a grey 128 and a white backdrop, as measured on a black
    /// desktop, and `lift` of the most a desktop picture lifts each.
    fn materials(self, lift: f32) -> [Rgba; 3] {
        let (measured, most) = match self {
            Tone::Dark => (
                [MATERIAL_OVER_BLACK, MATERIAL_OVER_GREY, MATERIAL_OVER_WHITE],
                MATERIAL_LIFT,
            ),
            Tone::Light => (
                [
                    LIGHT_MATERIAL_OVER_BLACK,
                    LIGHT_MATERIAL_OVER_GREY,
                    LIGHT_MATERIAL_OVER_WHITE,
                ],
                LIGHT_MATERIAL_LIFT,
            ),
        };
        let up = lift * most / 255.;
        measured.map(|material| {
            let material = rgb(material);
            Rgba {
                r: (material.r + up).min(1.),
                g: (material.g + up).min(1.),
                b: (material.b + up).min(1.),
                a: 1.,
            }
        })
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

    /// The tab shown's plate in the Space's colour, at rest and under the pointer.
    fn shown(self) -> (f32, f32) {
        match self {
            Tone::Dark => (SHOWN, SHOWN_HOVERED),
            Tone::Light => (LIGHT_SHOWN, LIGHT_SHOWN_HOVERED),
        }
    }

    /// The card the pages lie in.
    fn page(self) -> Rgba {
        rgba(match self {
            Tone::Dark => PAGE_CARD,
            Tone::Light => LIGHT_PAGE_CARD,
        })
    }

    /// What a page shows until its first frame, while websites are told this is the appearance.
    pub fn unpainted(self) -> Rgba {
        rgba(match self {
            Tone::Dark => UNPAINTED,
            Tone::Light => LIGHT_UNPAINTED,
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

/// What floats' alphas over the material at `level`: the dark fill's, which the level gives, and
/// beside it the light fill's and a light dialog's (#4090).
fn float_alphas(level: Transparency) -> [f32; 3] {
    use crate::tokens::palette as t;
    let light = match level {
        Transparency::Less => [t::LIGHT_OVERLAY_FILL_LESS, t::LIGHT_DIALOG_FILL_LESS],
        Transparency::Standard => [t::LIGHT_OVERLAY_FILL, t::LIGHT_DIALOG_FILL],
        Transparency::More => [t::LIGHT_OVERLAY_FILL_MORE, t::LIGHT_DIALOG_FILL_MORE],
    };
    [level.overlay_fill(), light[0], light[1]]
}

/// Whether this host's windows have a material behind them, which the host says once as the app
/// starts (host.h): what floats over it takes the level, and the level is offered. Until it says,
/// as in the preview, which has no host, it's the Mac's answer on the Mac and no elsewhere.
static WINDOW_MATERIAL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(cfg!(target_os = "macos"));

pub fn set_window_material(has: bool) {
    WINDOW_MATERIAL.store(has, std::sync::atomic::Ordering::Relaxed);
}

pub fn window_material() -> bool {
    WINDOW_MATERIAL.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    /// The user's transparency level, which transparency.rs hands over as it reads or sets it. Only
    /// the thread that draws reads it, and each test's own thread keeps its own.
    static TRANSPARENCY: std::cell::Cell<Transparency> = const {
        std::cell::Cell::new(Transparency::Standard)
    };
}

pub fn set_transparency(level: Transparency) {
    TRANSPARENCY.set(level);
}

/// The level the window's tint and what floats over it are drawn at (#4128).
pub fn transparency() -> Transparency {
    TRANSPARENCY.get()
}

/// What floats over the window and takes its fill by one rule (#4090): its colour at the
/// transparency level's alpha over the host's material where the host has one under it, and the
/// same colour opaque where it hasn't, as a sharp page through it would read as noise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Float {
    Menu,
    SiteControls,
    Toast,
    ZoomHud,
    Tooltip,
    HoverCard,
    LinkPreview,
    TabSwitcher,
    Dialog,
    FindBar,
}

impl Float {
    pub const ALL: [Float; 10] = [
        Float::Menu,
        Float::SiteControls,
        Float::Toast,
        Float::ZoomHud,
        Float::Tooltip,
        Float::HoverCard,
        Float::LinkPreview,
        Float::TabSwitcher,
        Float::Dialog,
        Float::FindBar,
    ];

    /// Whether the host has its windows' material under it (host.h): the Mac's overlays, Site
    /// Controls and menus. The rest have none under them yet.
    pub fn over_material(self) -> bool {
        window_material() && matches!(self, Float::Menu | Float::SiteControls | Float::TabSwitcher)
    }

    /// Its fill in a `tone` window, as drawn on this host: Standard's over the material whatever
    /// the Spaces' glass, as a Space's value changes only the window's.
    pub fn fill(self, tone: Tone) -> Rgba {
        self.fill_on(tone, self.over_material())
    }

    /// Its fill in a `tone` window, over the host's material or with none under it.
    fn fill_on(self, tone: Tone, material: bool) -> Rgba {
        self.fill_at(tone, material.then_some(Transparency::Standard))
    }

    /// Its fill at `level` over the host's material, or opaque with none.
    pub fn fill_at(self, tone: Tone, level: Option<Transparency>) -> Rgba {
        let colour = self.colour(tone);
        let Some(level) = level else {
            return colour;
        };
        let [dark, light, light_dialog] = float_alphas(level);
        let a = match (self, self.tone(tone)) {
            (_, Tone::Dark) => dark,
            (Float::Dialog, Tone::Light) => light_dialog,
            (_, Tone::Light) => light,
        };
        Rgba { a, ..colour }
    }

    /// The look it's drawn in: the find bar and tooltips are dark in both, so their white text
    /// holds on any page.
    fn tone(self, tone: Tone) -> Tone {
        match self {
            Float::FindBar | Float::Tooltip => Tone::Dark,
            _ => tone,
        }
    }

    /// Its colour, opaque.
    fn colour(self, tone: Tone) -> Rgba {
        rgb(match (self, self.tone(tone)) {
            (Float::Dialog, Tone::Dark) => DIALOG_DARK,
            (Float::Dialog, Tone::Light) => DIALOG_LIGHT,
            (_, Tone::Dark) => OVERLAY_DARK,
            (_, Tone::Light) => OVERLAY_LIGHT,
        })
    }

    /// The colour its text and glyphs are drawn in, Paper on the dark fills and Ink on the light.
    pub fn ink(self, tone: Tone) -> Rgba {
        self.tone(tone).ink()
    }
}

/// The colours the chrome draws with for one Space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Whether the window is dark or light.
    pub tone: Tone,
    /// Black or white over the window's material at the tone's tint, with a theme or without.
    pub tint: Rgba,
    /// Where the Space's glass sits on its Transparency bar, which `tint` is worked out from.
    pub glass: f32,
    /// The window's ground where it has to be opaque, under rows it covers or in the reveal's
    /// panel over the page.
    pub underlay: Rgba,
    /// How opaque the Space's colour lies at the centre of the glow from the sidebar's foot; 0
    /// without a theme.
    pub layer_alpha: f32,
    /// The glows' colours, each at its alpha at its centre: the main colour from the sidebar's
    /// foot and the second, faint, from the window's top right. Clear without a theme.
    pub glow: Rgba,
    pub far_glow: Rgba,
    /// The tab shown's colour: the Space's main colour, or the colour nearest it at its hue on
    /// which the ink reads; none without a theme, which takes the selected plate.
    pub shown: Option<Rgba>,
    /// The text and glyphs' colour, Paper on a dark window and Ink on a light one.
    pub ink: Rgba,
    /// The least alpha at which `ink` reads as text, and as a glyph, on the window and its
    /// plates everywhere.
    pub text_floor: f32,
    pub glyph_floor: f32,
    /// The least alpha at which `ink` reads as gray text (placeholders, section labels, a row's
    /// detail, a status) at the user's level, on the window and its plates everywhere, and at
    /// Less's on the solid window Reduce Transparency gives.
    pub gray_floor: f32,
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
    /// Each ground's shade, by `Ground`: the least step of the tint's colour over its plate at
    /// which the ink reads on it in every state, worked out with the palette.
    pub ground_shades: [f32; Ground::ALL.len()],
    /// The shade of a dark window's plate under the pointer: what its text asks, or as much of
    /// it as leaves the plate telling from the window.
    pub pointed_shade: f32,
}

/// A plate primary text sits on over the window. The window stays as see-through as the user's
/// transparency level says, and each of these takes the least shade at which its text reads,
/// its own (#4090).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// The selected row's plate.
    Selected,
    /// The tab shown's plate in the Space's colour.
    Shown,
    /// A sidebar row's plate under the pointer.
    Hovered,
    /// A Library row's or card's plate under the pointer or pressed.
    Library,
    /// The URL field, and a button drawn as one.
    Field,
    /// The offer's buttons.
    Offer,
    /// A toast's or a page's key chip.
    Chip,
}

impl Ground {
    pub const ALL: [Ground; 7] = [
        Ground::Selected,
        Ground::Shown,
        Ground::Hovered,
        Ground::Library,
        Ground::Field,
        Ground::Offer,
        Ground::Chip,
    ];
}

/// A Space's mark: its theme's colours as a small swatch (#320), lying as the window's gradient
/// does, with a hairline ring of Ink or Paper where its edge would fade into what's behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    /// The gradient's ends: the main colour at the top left, held to SWATCH_MAIN_TO along 135°,
    /// and the second at the bottom right, or a colour alone going lighter down. GPUI's 135° runs
    /// a disc's diagonal from 0 at its top left edge to 1 at its bottom right.
    from: Rgba,
    to: Rgba,
    angle: f32,
    from_at: f32,
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
    /// The palette for a Space with `theme`, or none, in a window of `tone`, at the app's level.
    pub fn of(theme: Option<&Theme>, tone: Tone) -> Palette {
        Palette::at(
            theme,
            tone,
            crate::space_transparency::of_level(transparency()),
        )
    }

    /// The palette for a Space with `theme`, or none, in a window of `tone`, with its glass at
    /// `glass` on the Space's Transparency bar.
    pub fn at(theme: Option<&Theme>, tone: Tone, glass: f32) -> Palette {
        let Some(theme) = theme else {
            return Palette::untinted(tone, glass);
        };
        let window = Window::of(theme, tone, glass);
        let layer_alpha = window.fit();
        let main = rgb(theme.main_color());
        let shown = solve_shown(main, tone, &window.samples(layer_alpha));
        let mut palette = Palette::with(layer_alpha, &window, Some(shown));
        palette.glow = Rgba {
            a: layer_alpha,
            ..main
        };
        palette.far_glow = Rgba {
            a: window.far_at(layer_alpha),
            ..window.far
        };
        let accent = palette.solve_accent(main, &window);
        let section_glyph = palette.solve_section_glyph(accent, &window);
        let swatch = Swatch::of(Some(theme), &window);
        Palette {
            accent,
            accent_label: label_on(accent),
            section_glyph,
            swatch,
            ..palette
        }
    }

    /// No theme: the tinted material alone, and ilo green, as light as it has to
    /// be to read on it, as the accent.
    fn untinted(tone: Tone, glass: f32) -> Palette {
        let window = Window::of_none(tone, glass);
        let palette = Palette::with(0., &window, None);
        let accent = palette.solve_accent(rgb(NO_THEME_ACCENT), &window);
        let section_glyph = palette.solve_section_glyph(accent, &window);
        let swatch = Swatch::of(None, &window);
        Palette {
            accent,
            accent_label: label_on(accent),
            section_glyph,
            swatch,
            ..palette
        }
    }

    fn with(layer_alpha: f32, window: &Window, shown: Option<Rgba>) -> Palette {
        let tone = window.tone;
        let ink = tone.ink();
        let (overlay, dialog) = tone.overlays();
        let mut palette = Palette {
            tone,
            tint: tone.tint(window.glass),
            glass: window.glass,
            underlay: tone.stand_in(),
            layer_alpha,
            glow: CLEAR,
            far_glow: CLEAR,
            shown,
            ink,
            text_floor: 0.,
            glyph_floor: 0.,
            gray_floor: 0.,
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
            ground_shades: [0.; Ground::ALL.len()],
            pointed_shade: 0.,
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
        let mut shades = Ground::ALL.map(|ground| palette.solve_ground(ground, window));
        // A row's plate under the pointer lies beside the selected row's and the tab shown's, and
        // takes at least their shade, or the selected row could end the darker of the two.
        let beside = shades[Ground::Selected as usize].max(shades[Ground::Shown as usize]);
        shades[Ground::Hovered as usize] = shades[Ground::Hovered as usize].max(beside);
        palette.ground_shades = shades;
        palette.pointed_shade = palette.solve_pointed(window);
        palette.ground_shades[Ground::Selected as usize] = palette.solve_selected(window);
        let grounds = palette.grounds(window);
        palette.text_floor = least_alpha(ink, &grounds, TEXT + BETWEEN);
        palette.glyph_floor = least_alpha(ink, &grounds, GLYPH + BETWEEN);
        let gray = gray_text_at(window.glass);
        palette.gray_floor = least_alpha(ink, &grounds, gray + BETWEEN);
        // With Reduce Transparency the window is solid at every level, the level's tint over it,
        // and nothing shows through, so gray text there holds what it does at Less.
        let solid = palette.grounds_on(&[over(tone.solid(), palette.tint, palette.tint.a)], 0.);
        let solid_gray = least_alpha(ink, &solid, gray_text(Transparency::Less) + BETWEEN);
        palette.gray_floor = palette.gray_floor.max(solid_gray);
        palette
    }

    /// The window at every point checked over both backdrops, with grain moving it
    /// toward the ink; and under the selected, hovered and field plates.
    fn grounds(&self, window: &Window) -> Vec<Rgba> {
        self.grounds_on(&window.samples(self.layer_alpha), window.grain)
    }

    /// `plain`, with `grain` moving it toward the ink, and under the selected, hovered and field
    /// plates.
    fn grounds_on(&self, plain: &[Rgba], grain: f32) -> Vec<Rgba> {
        let mut grounds: Vec<Rgba> = plain
            .iter()
            .map(|&at| over(at, self.ink, GRAIN_SHIFT * grain))
            .collect();
        // A light window's selected row is white, and a control's states still reach the ink's
        // selected plate.
        let (selected, alpha, _) = self.tone.selected();
        grounds.extend(plain.iter().map(|&at| over(at, selected, alpha)));
        if let Some(shown) = self.shown {
            let (rest, under) = self.tone.shown();
            for alpha in [rest, under] {
                grounds.extend(plain.iter().map(|&at| over(at, shown, alpha)));
            }
        }
        for plate in [SELECTED_PLATE, HOVERED_PLATE, FIELD_PLATE] {
            grounds.extend(plain.iter().map(|&at| over(at, self.ink, plate)));
        }
        grounds
    }

    /// The colour of `ground`'s plate and its alphas in each state it's drawn in.
    fn ground_plates(&self, ground: Ground) -> (Rgba, Vec<f32>) {
        use crate::tokens::shell::{HOVERED, LIGHT_HOVERED, TILE};
        let pressable = |rest: f32| {
            vec![
                rest,
                state_alpha(rest, STATE_HOVERED),
                state_alpha(rest, STATE_PRESSED),
            ]
        };
        let ink = self.ink;
        match ground {
            Ground::Selected => {
                let (colour, rest, under) = self.tone.selected();
                // A dark window's selected plate keeps its strength under the pointer (`shown`).
                match self.tone {
                    Tone::Dark => (colour, vec![rest]),
                    Tone::Light => (colour, vec![rest, under]),
                }
            }
            Ground::Shown => match self.shown {
                Some(colour) => {
                    let (rest, under) = self.tone.shown();
                    (colour, vec![rest, under])
                }
                None => (ink, Vec::new()),
            },
            Ground::Hovered => (
                ink,
                vec![match self.tone {
                    Tone::Dark => HOVERED,
                    Tone::Light => LIGHT_HOVERED,
                }],
            ),
            Ground::Library => (ink, vec![STATE_HOVERED, STATE_PRESSED]),
            Ground::Field => {
                let mut alphas = pressable(FIELD_PLATE);
                // The URL field brightens under the pointer as a tile does.
                alphas.push(state_alpha(TILE, STATE_HOVERED));
                (ink, alphas)
            }
            Ground::Offer => (ink, pressable(crate::tokens::offer::PLATE)),
            Ground::Chip => (ink, pressable(crate::tokens::toast::CHIP_FILL)),
        }
    }

    /// The least shade, in steps, of the tint's colour over `ground`'s plate at which the ink
    /// reads on it in every state over every point of the window: none where it reads already.
    fn solve_ground(&self, ground: Ground, window: &Window) -> f32 {
        let (colour, alphas) = self.ground_plates(ground);
        let plain = window.samples(self.layer_alpha);
        let tint = self.tone.tint_color();
        let reads = |shade: f32| {
            alphas.iter().all(|&alpha| {
                plain.iter().all(|&at| {
                    let ground = over(over(at, colour, alpha), tint, shade);
                    contrast(self.ink, ground) >= TEXT + BETWEEN
                })
            })
        };
        let steps = (1. / SHADE_STEP).round() as i32;
        (0..=steps)
            .map(|step| step as f32 * SHADE_STEP)
            .find(|&shade| reads(shade))
            .unwrap_or(1.)
    }

    /// The shade of a dark window's plate under the pointer: the least at which the ink reads on
    /// it beside the selected row's (`Ground::Hovered`), or as much of that, in finer steps, as
    /// leaves the plate telling from every point of the window. Where the
    /// window is bright, at More or in a Space's colour, the two pull apart, and the plate is
    /// seen first: its title, under the pointer alone, reads what that leaves it. A light
    /// window's takes none.
    fn solve_pointed(&self, window: &Window) -> f32 {
        // A light window's is the ink at its step: its rows' shade is worked out for a row's
        // fainter plate, and isn't this one's.
        if self.tone == Tone::Light {
            return 0.;
        }
        let plain = window.samples(self.layer_alpha);
        let tint = self.tone.tint_color();
        let tells = |shade: f32| {
            plain.iter().all(|&at| {
                let plate = over(
                    over(at, self.ink, crate::tokens::shell::HOVERED),
                    tint,
                    shade,
                );
                contrast(plate, at) >= TELLS_AIM
            })
        };
        let row = self.ground_shades[Ground::Hovered as usize];
        let steps = (row / TELLS_STEP).round() as i32;
        (1..=steps)
            .rev()
            .map(|step| row.min(step as f32 * TELLS_STEP))
            .find(|&shade| tells(shade))
            .unwrap_or(0.)
    }

    /// The selected plate's shade where the one `solve_ground` gave leaves the plate too faint to
    /// tell from the window, which a dark window with no Space's colour does where most comes
    /// through: the least at which the ink still reads on the plate at TEXT, whatever step it
    /// falls on, on the colour a screen shows, 8 bits a channel. It has nothing more to spare
    /// there, and needs none: that window is its material alone, one colour over each backdrop,
    /// with no points between to dip. A plate that tells already keeps its shade, and so does any
    /// with a Space's colour or on a light window, where what tells is the plate's shadow.
    fn solve_selected(&self, window: &Window) -> f32 {
        let coarse = self.ground_shades[Ground::Selected as usize];
        if self.tone == Tone::Light || self.shown.is_some() {
            return coarse;
        }
        let plain = window.samples(self.layer_alpha);
        let (colour, alphas) = self.ground_plates(Ground::Selected);
        let tint = self.tone.tint_color();
        let plates = |shade: f32| {
            let plain = &plain;
            alphas.iter().flat_map(move |&alpha| {
                (plain.iter()).map(move |&at| (over(over(at, colour, alpha), tint, shade), at))
            })
        };
        if plates(coarse).all(|(plate, at)| contrast(plate, at) >= TELLS_AIM) {
            return coarse;
        }
        // The ink reads more the more shade its plate has, so the least shade lies between one it
        // doesn't read at and one it does: halved until the plate's colour no longer changes.
        // Read on the plate as drawn: one solved to TEXT exactly is under it once it's rounded.
        let reads =
            |shade: f32| plates(shade).all(|(plate, _)| contrast(self.ink, drawn(plate)) >= TEXT);
        let (mut short, mut least) = (0., coarse);
        for _ in 0..12 {
            let middle = (short + least) / 2.;
            match reads(middle) {
                true => least = middle,
                false => short = middle,
            }
        }
        least
    }

    /// `plate` drawn on `ground`: with the ground's shade of the tint's colour over it, as one
    /// colour at one alpha, so it's drawn as one fill. A plate not drawn takes no shade.
    pub fn on_ground(&self, ground: Ground, plate: Hsla) -> Hsla {
        self.shaded(plate, self.ground_shades[ground as usize])
    }

    /// `plate` with `shade` of the tint's colour over it, as one colour at one alpha.
    fn shaded(&self, plate: Hsla, shade: f32) -> Hsla {
        if shade == 0. || plate.a == 0. {
            return plate;
        }
        let (p, tint) = (plate.to_rgb(), self.tone.tint_color());
        let a = 1. - (1. - p.a) * (1. - shade);
        let mix = |plate: f32, tint: f32| (plate * p.a * (1. - shade) + tint * shade) / a;
        Rgba {
            r: mix(p.r, tint.r),
            g: mix(p.g, tint.g),
            b: mix(p.b, tint.b),
            a,
        }
        .into()
    }

    /// A plate of the ink at `alpha` drawn on `ground`.
    pub fn ground_plate(&self, ground: Ground, alpha: f32) -> Hsla {
        self.on_ground(ground, self.plate(alpha))
    }

    /// A control's fill on `ground` in `state`, over its resting plate `rest`.
    pub fn ground_state(&self, ground: Ground, rest: f32, state: f32) -> Hsla {
        self.on_ground(ground, self.state(rest, state))
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
        let (near, _) = if t < 0.5 {
            (self, other)
        } else {
            (other, self)
        };
        Palette {
            tone: near.tone,
            underlay: mix(self.underlay, other.underlay),
            tint: mix(self.tint, other.tint),
            glass: self.glass + (other.glass - self.glass) * t,
            layer_alpha: self.layer_alpha + (other.layer_alpha - self.layer_alpha) * t,
            glow: near.glow,
            far_glow: near.far_glow,
            shown: match (self.shown, other.shown) {
                (Some(from), Some(to)) => Some(mix(from, to)),
                _ => near.shown,
            },
            ink: mix(self.ink, other.ink),
            text_floor: self.text_floor.max(other.text_floor),
            glyph_floor: self.glyph_floor.max(other.glyph_floor),
            gray_floor: self.gray_floor.max(other.gray_floor),
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
            ground_shades: std::array::from_fn(|ground| {
                let (from, to) = (self.ground_shades[ground], other.ground_shades[ground]);
                from + (to - from) * t
            }),
            pointed_shade: self.pointed_shade + (other.pointed_shade - self.pointed_shade) * t,
        }
    }

    /// The accent under the pointer, darker, as every filled button goes.
    pub fn accent_hovered(&self) -> Hsla {
        let Rgba { r, g, b, a } = self.accent;
        Rgba {
            r: r * ACCENT_HOVERED,
            g: g * ACCENT_HOVERED,
            b: b * ACCENT_HOVERED,
            a,
        }
        .into()
    }

    /// The accent pressed, a step darker than under the pointer, whatever its label (the owner).
    pub fn accent_pressed(&self) -> Hsla {
        let Rgba { r, g, b, a } = self.accent;
        Rgba {
            r: r * ACCENT_PRESSED,
            g: g * ACCENT_PRESSED,
            b: b * ACCENT_PRESSED,
            a,
        }
        .into()
    }

    /// A site's own colour as the command bar's accent in its site search (#1397), and its label:
    /// the colour nearest `main` at its hue that reads on the bar, as the accent does, with Ink or
    /// Paper on it.
    pub fn site_accent(&self, main: Rgba) -> (Rgba, Rgba) {
        let passes = |c: Rgba| {
            contrast(rgb(INK), c).max(contrast(rgb(PAPER), c)) >= LABEL
                && contrast(c, self.overlay) >= GLYPH
        };
        let fill = nearest(main, passes).unwrap_or(self.accent);
        (fill, label_on(fill))
    }

    /// Behind selected text in a field: the accent, faint.
    pub fn selection(&self) -> Hsla {
        Hsla::from(self.accent).opacity(SELECTION)
    }

    /// Where a dragged row or Space would go: the ink, firm, so it shows on either window.
    pub fn drop_line(&self) -> Hsla {
        Hsla::from(self.ink).opacity(DROP)
    }

    /// Ink or Paper at `alpha`, or as much more as text needs to read on any theme.
    pub fn text(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.text_floor))
    }

    /// Gray text (placeholders, section labels, a row's detail, a status) at `alpha`, or as much
    /// more as the user's level asks of gray text on any theme. Its floor is never over `text`'s,
    /// so where the window shows more through it stays dimmer than the text beside it.
    pub fn gray(&self, alpha: f32) -> Hsla {
        Hsla::from(self.ink).opacity(alpha.max(self.gray_floor))
    }

    /// Gray text at `alpha` on a row's or a card's plate. A dark window's plate is lighter than
    /// the window beside it, so gray reads less on it than off it, and there it is one ink
    /// wherever it lies, PLATE_GRAY, or more where `alpha` or the floor asks more. A light
    /// window's plate is lighter than its gray's ground too, which the ink gains by.
    pub fn gray_on_plate(&self, alpha: f32) -> Hsla {
        match self.tone {
            Tone::Dark => self.gray(alpha.max(PLATE_GRAY)),
            Tone::Light => self.gray(alpha),
        }
    }

    /// Ink or Paper at `alpha` on the opaque `fill`, or as much more as text needs to read on it.
    /// The window's floor answers for its material over any backdrop and the plates on it, so on a
    /// dark window it lifts a dim alpha almost to the ink, where the fill under it needs far less.
    pub fn text_on(&self, alpha: f32, fill: Hsla) -> Hsla {
        let floor = least_alpha(self.ink, &[fill.to_rgb()], TEXT + BETWEEN);
        Hsla::from(self.ink).opacity(alpha.max(floor))
    }

    /// Text at `alpha`, `lit` of the way to the ink at `high`, as a shimmer's highlight passes. It
    /// goes from where the floor puts the text, so the highlight shows however far that raises it.
    pub fn lit(&self, alpha: f32, high: f32, lit: f32) -> Hsla {
        let rest = self.text(alpha);
        Hsla {
            a: rest.a + (high.max(rest.a) - rest.a) * lit,
            ..rest
        }
    }

    /// The highlight that draws text in `lit(alpha, high, 1.)`, its peak, `lit` of the way there.
    /// GPUI keeps the text's own alpha under a highlight's colour, so a brighter one of the same ink
    /// shows nothing: the text is drawn at its peak, and each part fades down from it.
    pub fn lit_highlight(&self, alpha: f32, high: f32, lit: f32) -> HighlightStyle {
        let peak = self.lit(alpha, high, 1.).a;
        let fade = 1. - self.lit(alpha, high, lit).a / peak.max(f32::EPSILON);
        HighlightStyle {
            fade_out: Some(fade),
            ..Default::default()
        }
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
        self.on_ground(
            Ground::Selected,
            Hsla::from(color).opacity(rest + (under - rest) * hover),
        )
    }

    /// The tab shown's plate, `hover` of the way to its fill under the pointer: the Space's colour,
    /// or the selected plate for a Space without one.
    pub fn shown(&self, hover: f32) -> Hsla {
        let Some(color) = self.shown else {
            // On a dark window the ladder's hovered plate is Paper too strong for Paper's text to
            // read on, so without a colour the plate keeps its strength under the pointer.
            return match self.tone {
                Tone::Dark => self.selected(0.),
                Tone::Light => self.selected(hover),
            };
        };
        let (rest, under) = self.tone.shown();
        self.on_ground(
            Ground::Shown,
            Hsla::from(color).opacity(rest + (under - rest) * hover),
        )
    }

    /// What lifts the tab shown's plate off a light window, at `opacity`: the selected plate's,
    /// for a Space without a colour; a plate of the Space's colour lies flat.
    pub fn shown_shadow(&self, opacity: f32) -> Vec<BoxShadow> {
        match self.shown {
            Some(_) => Vec::new(),
            None => self.selected_shadow(opacity),
        }
    }

    /// The window's colour over its ground, `foot` of the way to the foot glow's full strength and
    /// `far` of the way to the far one's, with each one's alpha: what to lay over the ground there.
    pub fn glow_at(&self, foot: f32, far: f32) -> Rgba {
        let (foot, far) = (
            Rgba {
                a: self.glow.a * foot,
                ..self.glow
            },
            Rgba {
                a: self.far_glow.a * far,
                ..self.far_glow
            },
        );
        composite(foot, far)
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

    /// A sidebar row's plate under the pointer, the only one a row has but the tab shown's: on a
    /// dark window the plate anything has under the pointer, so a row's and a card's are one and
    /// both are seen; on a light one the ink at a row's fainter step.
    pub fn hovered_row(&self) -> Hsla {
        match self.tone {
            Tone::Dark => self.hovered_plate(),
            Tone::Light => self.ground_plate(Ground::Hovered, crate::tokens::shell::LIGHT_HOVERED),
        }
    }

    /// A plate under the pointer over the window, a sidebar row's on a dark window, a card's at
    /// the sidebar's foot or a form's row: the ink at the ladder's step in both looks, and on a
    /// dark window with the shade that makes what's written on it read however much of a bright
    /// window comes through, held to what leaves the plate itself to be seen (`solve_pointed`).
    pub fn hovered_plate(&self) -> Hsla {
        self.shaded(
            self.plate(crate::tokens::shell::HOVERED),
            self.pointed_shade,
        )
    }

    /// The window where its foot's glow is `foot` of its full strength and the far one `far` of
    /// its, as it's seen over a black and over a white backdrop.
    pub fn window_at(&self, foot: f32, far: f32) -> [Rgba; 2] {
        let glow = self.glow_at(foot, far);
        let grounds = self.tone.grounds(self.tint);
        grounds.map(|ground| over(ground, glow, glow.a))
    }

    /// The window over a grey backdrop at black, grey 128 and white, the three its material was
    /// measured over, where its glows are `foot` and `far` of their full strength, with `lift` of
    /// the most a desktop picture lifts the material. Everything over the material lies over it
    /// evenly, so over a grey between two of these the window is as far between theirs.
    pub fn window_over_greys(&self, foot: f32, far: f32, lift: f32) -> [Rgba; 3] {
        let glow = self.glow_at(foot, far);
        self.tone.materials(lift).map(|material| {
            let ground = over(material, self.tint, self.tint.a);
            over(ground, glow, glow.a)
        })
    }

    /// The window where its theme's colour is `layer`, translucent, over a black and over a
    /// white backdrop: what a mark on it is judged against.
    pub fn over_backdrops(&self, layer: Hsla) -> [Rgba; 2] {
        (self.tone.grounds(self.tint)).map(|ground| Hsla::from(ground).blend(layer).to_rgb())
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
        Swatch::of_two(from, to).ring_on(behind)
    }

    /// Where a swatch ring is checked against this palette's window: every point checked, over
    /// both backdrops.
    pub fn window_samples(&self, theme: Option<&Theme>) -> Vec<Rgba> {
        let window = theme.map_or(Window::of_none(self.tone, self.glass), |theme| {
            Window::of(theme, self.tone, self.glass)
        });
        window.samples(self.layer_alpha)
    }

    /// The pickers' card as they draw it: translucent where the host blurs what's behind a
    /// picker, the Mac's always and Linux's where its compositor can. Elsewhere what's behind
    /// shows sharp through the card and reads as noise, so there the card is opaque over the dark
    /// overlay.
    pub fn card_drawn(&self) -> Rgba {
        #[cfg(target_os = "linux")]
        let blurred = gpui_linux::overlays_blur();
        #[cfg(not(target_os = "linux"))]
        let blurred = cfg!(target_os = "macos");
        self.card_where(blurred)
    }

    /// The pickers' card where the host does or doesn't blur what's behind a picker.
    fn card_where(&self, blurred: bool) -> Rgba {
        if blurred {
            return self.card;
        }
        over(rgb(OVERLAY_DARK), self.card, self.card.a)
    }

    /// The pickers' card: Arc's measured grey, then a step at a time toward the dark overlay,
    /// opaque, until its hint and its segments' labels read on it over every point of the window.
    fn solve_card(&self, window: &Window) -> Rgba {
        let (measured, opaque) = (rgba(CARD), rgb(OVERLAY_DARK));
        let behind = window.samples(self.layer_alpha);
        let steps = (1. / SHADE_STEP).round() as i32;
        let toward = |t: f32| Rgba {
            r: measured.r + (opaque.r - measured.r) * t,
            g: measured.g + (opaque.g - measured.g) * t,
            b: measured.b + (opaque.b - measured.b) * t,
            a: measured.a + (opaque.a - measured.a) * t,
        };
        (0..=steps)
            .map(|step| toward(step as f32 * SHADE_STEP))
            .find(|&fill| {
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
        let plain = window.samples(self.layer_alpha);
        let section: Vec<Rgba> = plain
            .iter()
            .map(|&at| over(at, self.ink, SECTION_PLATE))
            .collect();
        let target: Vec<Rgba> = plain
            .iter()
            .map(|&at| over(at, self.ink, TARGET_PLATE))
            .collect();
        // A switch's or the shield's glyph, on Site Controls' tile tinted with the accent: on a
        // window dark enough, the accent that reads on the window is too dark for it.
        let tiles = [SITE_CONTROLS_TILE_TOP, SITE_CONTROLS_TILE_BOTTOM]
            .map(|plate| over(self.overlay, self.ink, plate));
        let passes = |c: Rgba| {
            contrast(rgb(INK), c).max(contrast(rgb(PAPER), c)) >= LABEL
                && contrast(c, self.overlay) >= GLYPH
                && contrast(c, self.dialog) >= GLYPH
                && plain
                    .iter()
                    .chain(&section)
                    .all(|&at| contrast(c, at) >= GLYPH + BETWEEN)
                && target
                    .iter()
                    .all(|&at| contrast(c, over(at, c, TARGET_ACCENT)) >= GLYPH + BETWEEN)
                && tiles
                    .iter()
                    .all(|&at| contrast(c, over(at, c, SITE_CONTROLS_TINT)) >= GLYPH)
        };
        nearest(main, passes).unwrap_or(self.ink)
    }

    /// The accent, or the colour nearest it at its hue, that reads at 3:1 on the Library's column
    /// under the selected section's plate, everywhere on the window.
    fn solve_section_glyph(&self, accent: Rgba, window: &Window) -> Rgba {
        let selected: Vec<Rgba> = (window.samples(self.layer_alpha).iter())
            .map(|&at| over(over(at, self.ink, COLUMN), self.ink, LIBRARY_PLATE))
            .collect();
        let passes = |c: Rgba| {
            selected
                .iter()
                .all(|&at| contrast(c, at) >= GLYPH + BETWEEN)
        };
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
        let glass = crate::space_transparency::START;
        match theme {
            Some(theme) => Swatch::of(Some(theme), &Window::of(theme, Tone::Dark, glass)),
            None => Swatch::of(None, &Window::of_none(Tone::Dark, glass)),
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
                from_at: 0.,
                to_at: 1.,
                third: None,
                darkest: lum,
                lightest: lum,
            };
        };
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let (from, to, angle, from_at) = match colors[..] {
            [_] => (
                Layer::of(theme, 0., 0.).color,
                Layer::of(theme, 0., 1.).color,
                180.,
                0.,
            ),
            _ => (colors[0], colors[1], 135., SWATCH_MAIN_TO),
        };
        let lums = window.swatch().into_iter().map(luminance);
        let (darkest, lightest) = lums.fold((f32::MAX, f32::MIN), |(d, l), y| (d.min(y), l.max(y)));
        Swatch {
            from,
            to,
            angle,
            from_at,
            to_at: 1.,
            third: colors.get(2).copied(),
            darkest,
            lightest,
        }
    }

    /// Two colours alone, as a Space's swatch in a menu: `from`, the main one, at the top left,
    /// as a theme's swatch lies.
    pub fn of_two(from: Rgba, to: Rgba) -> Swatch {
        let (a, b) = (luminance(from), luminance(to));
        Swatch {
            from,
            to,
            angle: 135.,
            from_at: SWATCH_MAIN_TO,
            to_at: 1.,
            third: None,
            darkest: a.min(b),
            lightest: a.max(b),
        }
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
        let worst = |ring: Rgba| {
            behind
                .iter()
                .map(|&b| contrast(ring, b))
                .fold(f32::MAX, f32::min)
        };
        Some(if worst(ink) >= worst(paper) {
            ink
        } else {
            paper
        })
    }

    /// The swatch, `size` across, on any of `behind`.
    pub fn draw(&self, size: f32, behind: &[Rgba]) -> Div {
        let gradient = linear_gradient(
            self.angle,
            linear_color_stop(self.from, self.from_at),
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
                swatch.child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded_full()
                        .bg(linear_gradient(
                            180.,
                            linear_color_stop(third.opacity(0.), 0.),
                            linear_color_stop(third, 1.),
                        )),
                )
            })
            .when_some(self.ring_on(behind), |swatch, ring| {
                // The ring is its own layer, over the colours, so it doesn't move them.
                swatch.child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded_full()
                        .border(px(RING))
                        .border_color(ring),
                )
            })
    }
}

/// The theme's window, worked out once: how opaque its glows are and their colours, and the
/// colours its swatch runs through.
struct Window {
    tone: Tone,
    /// The foot's glow at its centre, and the far one's at the theme's own alpha.
    alpha: f32,
    far_alpha: f32,
    main: Rgba,
    far: Rgba,
    grain: f32,
    swatch: Vec<Rgba>,
    /// Where the Space's glass sits on its Transparency bar.
    glass: f32,
}

/// The theme's colour at one point of its swatch's run: the gradient's, and a third colour over it
/// at a share of the layer's alpha.
#[derive(Clone, Copy)]
struct Layer {
    color: Rgba,
    third: Option<(Rgba, f32)>,
}

impl Layer {
    /// `theme`'s colour `along` the gradient's line (0 at the window's top left, 1 at its bottom
    /// right) and `y` down the window: one colour going to the same at HSL lightness + 0.25 down
    /// the window; two from the main one to the second SECOND_AT along; a third coming in toward
    /// the bottom.
    fn of(theme: &Theme, along: f32, y: f32) -> Layer {
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        if colors.len() == 1 {
            let main = Hsla::from(colors[0]);
            return Layer {
                color: Hsla {
                    l: (main.l + 0.25 * y).min(1.),
                    ..main
                }
                .into(),
                third: None,
            };
        }
        let along = (along / SECOND_AT).min(1.);
        let color = theme::mix_oklab(colors[0], colors[1], along);
        Layer {
            color,
            third: colors.get(2).map(|&third| (third, y)),
        }
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
    fn of_none(tone: Tone, glass: f32) -> Window {
        Window {
            tone,
            glass,
            alpha: 0.,
            far_alpha: 0.,
            main: BLACK,
            far: BLACK,
            grain: 0.,
            swatch: vec![BLACK],
        }
    }

    fn of(theme: &Theme, tone: Tone, glass: f32) -> Window {
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let alongs: &[f32] = if colors.len() == 1 { &[0.] } else { &ALONG };
        let swatch = alongs
            .iter()
            .flat_map(|&along| DOWN.iter().map(move |&y| Layer::of(theme, along, y)))
            .map(|layer| layer.over(BLACK, 1.))
            .collect();
        let alpha = layer_alpha(theme, tone);
        let (colored, _) = tone.gradient_alphas();
        Window {
            tone,
            alpha,
            far_alpha: FAR_GLOW * alpha / colored,
            main: colors[0],
            far: colors.get(1).copied().unwrap_or(colors[0]),
            grain: theme.grain,
            swatch,
            glass,
        }
    }

    /// The far glow's alpha where the foot's is `alpha`, as `fit` may fade them together.
    fn far_at(&self, alpha: f32) -> f32 {
        match self.alpha > 0. {
            true => self.far_alpha * alpha / self.alpha,
            false => 0.,
        }
    }

    /// The window at every point checked over both backdrops: its ground, with the foot's glow
    /// at `alpha` at its centre and the far one with it, each anywhere from none to full.
    fn samples(&self, alpha: f32) -> Vec<Rgba> {
        let far_alpha = self.far_at(alpha);
        (self.tone.grounds(self.tone.tint(self.glass)))
            .into_iter()
            .flat_map(|ground| {
                FOOT.iter().flat_map(move |&foot| {
                    FAR.iter().map(move |&far| {
                        let glow = composite(
                            Rgba {
                                a: alpha * foot,
                                ..self.main
                            },
                            Rgba {
                                a: far_alpha * far,
                                ..self.far
                            },
                        );
                        over(ground, glow, glow.a)
                    })
                })
            })
            .collect()
    }

    /// The theme's colours as a swatch shows them, opaque.
    fn swatch(&self) -> Vec<Rgba> {
        self.swatch.clone()
    }

    /// The theme's alpha, its own, or lower by the least steps at which the ink reads as text on
    /// the window over both backdrops, grain and all, and the end of the scale the ink runs toward
    /// stands out on the whole window, so an accent always exists. The colour only adds its glow:
    /// the window keeps the glass it has without a theme, never shaded toward the tint (#4090),
    /// and a plate the ink can't read on takes its own shade (`solve_ground`). With no glow left
    /// it's the window without a theme, which reads.
    fn fit(&self) -> f32 {
        let (ink, end) = (self.tone.ink(), self.tone.end());
        let reads = |alpha: f32| {
            self.samples(alpha).iter().all(|&at| {
                let grained = over(at, ink, GRAIN_SHIFT * self.grain);
                contrast(end, at) >= GLYPH + BETWEEN && contrast(ink, grained) >= TEXT + BETWEEN
            })
        };
        let steps = (1. / SHADE_STEP).round() as i32;
        (0..=steps)
            .rev()
            .map(|step| self.alpha * step as f32 * SHADE_STEP)
            .find(|&alpha| reads(alpha))
            .unwrap_or(0.)
    }
}

/// The tab shown's colour for a Space whose main colour is `main`, on a window of `tone` checked
/// at `samples`: `main`, or the colour nearest it at its hue on which the ink reads at text's
/// minimum, at rest and under the pointer, everywhere on the window.
fn solve_shown(main: Rgba, tone: Tone, samples: &[Rgba]) -> Rgba {
    let ink = tone.ink();
    let (rest, under) = tone.shown();
    let passes = |c: Rgba| {
        samples.iter().all(|&at| {
            [rest, under]
                .iter()
                .all(|&alpha| contrast(ink, over(at, c, alpha)) >= TEXT + BETWEEN)
        })
    };
    nearest(main, passes).unwrap_or(ink)
}

/// How opaque `theme`'s colours lie over a window of `tone`, as a Space's colour alone does at
/// the intensity a theme starts with, less for a neutral one.
fn layer_alpha(theme: &Theme, tone: Tone) -> f32 {
    let saturation = Hsla::from(rgb(theme.main_color())).s;
    let (colored, neutral) = tone.gradient_alphas();
    let alpha = if saturation < NEUTRAL {
        neutral
    } else {
        colored
    };
    let at = |intensity: f32| INTENSITY_FLOOR + INTENSITY_SPAN * intensity;
    alpha * at(theme.intensity) / at(DEFAULT_INTENSITY)
}

/// The least contrast gray text holds where a Space's glass is `glass`: Less's from solid up to
/// where Less sits, then toward Standard's and More's where they sit, as the tint goes.
pub fn gray_text_at(glass: f32) -> f32 {
    use crate::space_transparency::of_level;
    let stops = [
        Transparency::Less,
        Transparency::Standard,
        Transparency::More,
    ]
    .map(|level| (of_level(level), gray_text(level)));
    let glass = glass.clamp(0., 1.);
    let mut pairs = stops.windows(2);
    pairs
        .find(|pair| glass <= pair[1].0)
        .map_or(GRAY_TEXT_MORE, |pair| {
            let [(from, at_from), (to, at_to)] = [pair[0], pair[1]];
            match glass <= from {
                true => at_from,
                false => at_from + (at_to - at_from) * (glass - from) / (to - from),
            }
        })
}

/// The least contrast gray text holds at `level`, from the table beside the window's tints;
/// primary text holds TEXT at every level.
pub fn gray_text(level: Transparency) -> f32 {
    match level {
        Transparency::Less => GRAY_TEXT_LESS,
        Transparency::Standard => GRAY_TEXT,
        Transparency::More => GRAY_TEXT_MORE,
    }
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
    if contrast(ink, color) >= contrast(paper, color) {
        ink
    } else {
        paper
    }
}

/// The least alpha, in 25ths, at which `ink` over each of `grounds` reaches `minimum`; 1 where
/// none does.
fn least_alpha(ink: Rgba, grounds: &[Rgba], minimum: f32) -> f32 {
    (0..=25)
        .map(|step| step as f32 / 25.)
        .find(|&alpha| {
            grounds
                .iter()
                .all(|&at| contrast(over(at, ink, alpha), at) >= minimum)
        })
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
            if inside(middle) {
                low = middle
            } else {
                high = middle
            }
        }
    }
    let [r, g, b] = theme::from_oklab([l, a * low, b * low]);
    quantized(Rgba { r, g, b, a: 1. })
}

/// `color` as it is once it's 8 bits a channel.
pub fn quantized(color: Rgba) -> Rgba {
    rgb(theme::packed(color))
}

/// Translucent `top` over translucent `under`, as one translucent colour.
pub fn composite(under: Rgba, top: Rgba) -> Rgba {
    let a = top.a + under.a * (1. - top.a);
    if a <= 0. {
        return CLEAR;
    }
    let mix = |u: f32, t: f32| (t * top.a + u * under.a * (1. - top.a)) / a;
    Rgba {
        r: mix(under.r, top.r),
        g: mix(under.g, top.g),
        b: mix(under.b, top.b),
        a,
    }
}

/// `color` at `alpha` over opaque `base`.
pub fn over(base: Rgba, color: Rgba, alpha: f32) -> Rgba {
    let mix = |b: f32, c: f32| b + (c - b) * alpha;
    Rgba {
        r: mix(base.r, color.r),
        g: mix(base.g, color.g),
        b: mix(base.b, color.b),
        a: 1.,
    }
}

/// `color` as a screen shows it, each channel one of 8 bits' 256 levels.
pub fn drawn(color: Rgba) -> Rgba {
    rgb(theme::packed(color))
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
