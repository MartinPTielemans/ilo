//! A Space's theme, as Arc's theme picker makes it: one to three colours placed in a square field,
//! how strongly the window takes them, its grain and the texture it lies in, and its appearance
//! (docs/ux-spec.md › Themes). A theme from Arc can also have another look for a dark window, and
//! its third colour exactly. The core keeps it with the Space as text, and the main colour as the
//! Space's colour.
//!
//! The field is a disc of HSL colours about its centre (Arc 1.170 rec at 2×, twelve handles): the
//! hue is the angle clockwise from the right, saturation is 1, and lightness is the distance from
//! the centre in half the field's width, black at the centre and white at the edge. A second colour
//! is the main one's complement; with a third the two sit 150° and 210° round from it, until one
//! is dragged to a place of its own. Dragging the main one moves them all with it. A swatch places
//! the main colour at its hue and lightness and keeps its saturation, the others' too, until a
//! handle is dragged.

use std::{cell::OnceCell, sync::Arc};

use gpui::{Hsla, RenderImage, Rgba, hsla, rgb};
use image::{Frame, RgbaImage};

pub const MAX_COLORS: usize = 3;
pub use crate::tokens::theme::DEFAULT_INTENSITY;

/// The window's look with the theme: light, dark, or picked from the colours (Arc's sparkles).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Appearance {
    #[default]
    Automatic,
    Light,
    Dark,
}

/// The pattern grain lies over the window in, one of ours for each of Arc's four.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Texture {
    /// Fine noise, every pixel its own: Arc's grain.
    #[default]
    Noise,
    /// Scattered specks: Arc's sand.
    Dust,
    /// Diagonal threads: Arc's denim.
    Weave,
    /// Short fibres over a fainter noise: Arc's tweed.
    Fleck,
}

impl Texture {
    pub const ALL: [Texture; 4] = [
        Texture::Noise,
        Texture::Dust,
        Texture::Weave,
        Texture::Fleck,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Texture::Noise => "noise",
            Texture::Dust => "dust",
            Texture::Weave => "weave",
            Texture::Fleck => "fleck",
        }
    }

    /// The texture `name` names, as `name` writes it.
    pub fn named(name: &str) -> Option<Texture> {
        Texture::ALL.into_iter().find(|each| each.name() == name)
    }

    /// How strongly grain at 1 lies over the window in this texture, so that each moves the
    /// window's colours as much as the others at the same grain.
    pub fn strength(self) -> f32 {
        use crate::tokens::theme::*;
        GRAIN_STRENGTH
            * match self {
                Texture::Noise => 1.,
                Texture::Dust => DUST_STRENGTH,
                Texture::Weave => WEAVE_STRENGTH,
                Texture::Fleck => FLECK_STRENGTH,
            }
    }
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::Automatic, Appearance::Light, Appearance::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Appearance::Automatic => "automatic",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    /// The appearance `name` names, as `name` writes it.
    pub fn named(name: &str) -> Option<Appearance> {
        Appearance::ALL.into_iter().find(|each| each.name() == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The main colour's place in the field, from its centre, in half the field's width: x to the
    /// right, y down. Never further out than 1, where the colour is white.
    pub x: f32,
    pub y: f32,
    /// The colours' HSL saturation: the field's 1, or a swatch's own.
    pub saturation: f32,
    /// How many colours, 1 to MAX_COLORS.
    pub count: usize,
    /// 0 to 1: how strongly the window takes the colours.
    pub intensity: f32,
    /// 0 to 1: how much grain lies over them, and in what.
    pub grain: f32,
    pub texture: Texture,
    /// The third colour exactly, 0xRRGGBB, as Arc's is, standing in for the field's colour at its
    /// handle until the picker places or counts the colours again.
    pub third: Option<u32>,
    pub appearance: Appearance,
    /// Where the second and third colours are from the main one, once either has a place of its
    /// own, as Arc's do; none while they sit at the set angles round it.
    pub places: Option<[Place; 2]>,
    /// The look a dark window draws instead, where the theme has two, as Arc's can; the picker
    /// makes themes of one.
    pub dark: Option<Look>,
}

/// Where a colour other than the main one is, from the main one: `turn` degrees further round the
/// field's centre clockwise, and `lift` further out from it, in half the field's width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub turn: f32,
    pub lift: f32,
}

/// A theme's colours, intensity, grain and texture, without its appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub x: f32,
    pub y: f32,
    pub saturation: f32,
    pub count: usize,
    pub intensity: f32,
    pub grain: f32,
    pub texture: Texture,
    pub third: Option<u32>,
    pub places: Option<[Place; 2]>,
}

impl Theme {
    /// One colour, at (x, y) in the field, with the picker's other settings as they start.
    pub fn at(x: f32, y: f32) -> Theme {
        let (x, y) = in_disc(x, y);
        Theme {
            x,
            y,
            saturation: 1.,
            count: 1,
            intensity: DEFAULT_INTENSITY,
            grain: 0.,
            texture: Texture::Noise,
            third: None,
            appearance: Appearance::Automatic,
            places: None,
            dark: None,
        }
    }

    /// The theme's own look, the one a light window draws.
    pub fn look(&self) -> Look {
        let Theme {
            x,
            y,
            saturation,
            count,
            intensity,
            grain,
            texture,
            third,
            places,
            ..
        } = *self;
        Look {
            x,
            y,
            saturation,
            count,
            intensity,
            grain,
            texture,
            third,
            places,
        }
    }

    /// The theme as a window draws it, `dark` or light: its dark look on a dark window where it
    /// has one, as a theme of one look.
    pub fn shown(self, dark: bool) -> Theme {
        let look = self.dark.filter(|_| dark).unwrap_or(self.look());
        let Look {
            x,
            y,
            saturation,
            count,
            intensity,
            grain,
            texture,
            third,
            places,
        } = look;
        Theme {
            x,
            y,
            saturation,
            count,
            intensity,
            grain,
            texture,
            third,
            places,
            dark: None,
            ..self
        }
    }

    /// A theme of `color`, 0xRRGGBB, alone: in the field at its hue and HSL lightness, and at its
    /// saturation, so it is the colour itself, such as for a Space whose colour came before themes
    /// did, or a swatch.
    pub fn of_color(color: u32) -> Theme {
        let hsla = Hsla::from(rgb(color));
        let angle = hsla.h * std::f32::consts::TAU;
        let theme = Theme::at(hsla.l * angle.cos(), hsla.l * angle.sin());
        Theme {
            saturation: hsla.s,
            ..theme
        }
    }

    /// How far round from the main colour each colour is, in degrees.
    pub fn offsets(count: usize) -> &'static [f32] {
        match count {
            0 | 1 => &[0.],
            2 => &[0., 180.],
            _ => &[0., 150., 210.],
        }
    }

    /// Where colour `index` other than the main one is from it: its own place, else its set angle
    /// at the main one's distance.
    pub fn place(&self, index: usize) -> Place {
        match self.places {
            Some(places) if (1..MAX_COLORS).contains(&index) => places[index - 1],
            _ => Place {
                turn: Self::offsets(self.count).get(index).copied().unwrap_or(0.),
                lift: 0.,
            },
        }
    }

    /// Where colour `index` is in the field, as `x` and `y` are.
    pub fn handle(&self, index: usize) -> (f32, f32) {
        if index == 0 {
            return (self.x, self.y);
        }
        placed(self.x, self.y, self.place(index))
    }

    /// The colours, the main one first, 0xRRGGBB.
    pub fn colors(&self) -> Vec<u32> {
        (0..self.count)
            .map(|index| match self.third {
                Some(third) if index == 2 => third,
                _ => {
                    let (x, y) = self.handle(index);
                    color_at(x, y, self.saturation)
                }
            })
            .collect()
    }

    /// How opaque the texture lies over the window.
    pub fn grain_opacity(&self) -> f32 {
        self.grain * self.texture.strength()
    }

    /// The main colour, which the core keeps as the Space's colour.
    pub fn main_color(&self) -> u32 {
        color_at(self.x, self.y, self.saturation)
    }

    /// The theme with colour `index` dragged to (x, y): the main one takes the others with it,
    /// keeping their places round it, and another goes there alone. All take the field's colours
    /// there, a third from Arc too.
    pub fn moved(self, index: usize, x: f32, y: f32) -> Theme {
        let (x, y) = in_disc(x, y);
        if (x, y) == self.handle(index) {
            return self;
        }
        if index == 0 {
            return Theme {
                x,
                y,
                saturation: 1.,
                third: None,
                ..self
            };
        }
        let mut places = [self.place(1), self.place(2)];
        places[index.min(MAX_COLORS - 1) - 1] = place_of(self.x, self.y, x, y);
        Theme {
            places: Some(places),
            saturation: 1.,
            third: None,
            ..self
        }
    }

    /// The theme with `count` colours, from 1 to MAX_COLORS, back at the set angles round the main
    /// one, each in its place in the field.
    pub fn with_count(self, count: usize) -> Theme {
        Theme {
            count: count.clamp(1, MAX_COLORS),
            third: None,
            places: None,
            ..self
        }
    }

    /// The text the core keeps; the saturation only once a swatch has set it, so a theme from the
    /// field reads as it did before swatches kept theirs; the texture, third colour and places
    /// only where a theme has them; and a dark look as its place, count, intensity, grain and
    /// saturation, then its texture, third colour and places where it has them.
    pub fn to_text(&self) -> String {
        let text = format!(
            "x={:.4} y={:.4} colors={} intensity={:.3} grain={:.3} appearance={}",
            self.x,
            self.y,
            self.count,
            self.intensity,
            self.grain,
            self.appearance.name()
        );
        let mut text = text;
        if self.saturation < 1. {
            text += &format!(" saturation={:.4}", self.saturation);
        }
        if self.texture != Texture::Noise {
            text += &format!(" texture={}", self.texture.name());
        }
        if let Some(third) = self.third {
            text += &format!(" third={third:06x}");
        }
        if let Some(places) = self.places {
            text += &format!(" places={}", places_text(places, ","));
        }
        if let Some(Look {
            x,
            y,
            saturation,
            count,
            intensity,
            grain,
            texture,
            third,
            places,
        }) = self.dark
        {
            text +=
                &format!(" dark={x:.4},{y:.4},{count},{intensity:.3},{grain:.3},{saturation:.4}");
            if texture != Texture::Noise || third.is_some() {
                text += &format!(",{}", texture.name());
            }
            if let Some(third) = third {
                text += &format!(",{third:06x}");
            }
            if let Some(places) = places {
                text += &format!(",places:{}", places_text(places, ":"));
            }
        }
        text
    }

    /// The theme `text` describes, as `to_text` writes it; None for empty text, or text without the
    /// main colour's place. What it doesn't say is as a new theme has it.
    pub fn parse(text: &str) -> Option<Theme> {
        let (mut x, mut y) = (None, None);
        let mut theme = Theme::at(0., 0.);
        for pair in text.split_whitespace() {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let number = value.parse::<f32>().ok().filter(|value| value.is_finite());
            match key {
                "x" => x = number,
                "y" => y = number,
                "colors" => {
                    if let Ok(count) = value.parse::<usize>() {
                        theme.count = count.clamp(1, MAX_COLORS);
                    }
                }
                "intensity" => theme.intensity = number.unwrap_or(theme.intensity).clamp(0., 1.),
                "grain" => theme.grain = number.unwrap_or(theme.grain).clamp(0., 1.),
                "saturation" => theme.saturation = number.unwrap_or(theme.saturation).clamp(0., 1.),
                "appearance" => {
                    if let Some(appearance) = Appearance::named(value) {
                        theme.appearance = appearance;
                    }
                }
                "texture" => theme.texture = Texture::named(value).unwrap_or(theme.texture),
                "third" => theme.third = hex(value),
                "places" => theme.places = parse_places(value, ','),
                "dark" => theme.dark = parse_look(value),
                _ => {}
            }
        }
        let (x, y) = in_disc(x?, y?);
        Some(Theme { x, y, ..theme })
    }
}

/// A dark look as `to_text` writes it: x, y, count, intensity, grain and saturation, then its
/// texture, third colour and places where it has them, each known by its form.
fn parse_look(text: &str) -> Option<Look> {
    let mut items = text.split(',');
    let numbers: Vec<f32> = (items.by_ref().take(6))
        .map(|n| n.parse::<f32>().ok().filter(|n| n.is_finite()))
        .collect::<Option<_>>()?;
    let [x, y, count, intensity, grain, saturation] = numbers[..] else {
        return None;
    };
    let (x, y) = in_disc(x, y);
    let count = (count as usize).clamp(1, MAX_COLORS);
    let [intensity, grain, saturation] = [intensity, grain, saturation].map(|n| n.clamp(0., 1.));
    let (mut texture, mut third, mut places) = (Texture::Noise, None, None);
    for item in items {
        if let Some(named) = Texture::named(item) {
            texture = named;
        } else if let Some(color) = hex(item) {
            third = Some(color);
        } else if let Some(text) = item.strip_prefix("places:") {
            places = parse_places(text, ':');
        }
    }
    Some(Look {
        x,
        y,
        saturation,
        count,
        intensity,
        grain,
        texture,
        third,
        places,
    })
}

/// The second colour's turn and lift, then the third's, between `between`.
fn places_text([second, third]: [Place; 2], between: &str) -> String {
    format!(
        "{:.2}{between}{:.4}{between}{:.2}{between}{:.4}",
        second.turn, second.lift, third.turn, third.lift
    )
}

/// Places as `places_text` writes them.
fn parse_places(text: &str, between: char) -> Option<[Place; 2]> {
    let numbers: Vec<f32> = (text.split(between))
        .map(|n| n.parse::<f32>().ok().filter(|n| n.is_finite()))
        .collect::<Option<_>>()?;
    let [turn, lift, third_turn, third_lift] = numbers[..] else {
        return None;
    };
    let place = |turn: f32, lift: f32| Place {
        turn: turn.rem_euclid(360.),
        lift: lift.clamp(-1., 1.),
    };
    Some([place(turn, lift), place(third_turn, third_lift)])
}

/// 0xRRGGBB of six hex digits.
fn hex(text: &str) -> Option<u32> {
    (text.len() == 6)
        .then(|| u32::from_str_radix(text, 16).ok())
        .flatten()
}

/// Where `place` is from a main colour at (x, y): turned round the field's centre, and moved out
/// or in, staying in the field.
pub fn placed(x: f32, y: f32, place: Place) -> (f32, f32) {
    let distance = (x.hypot(y) + place.lift).clamp(0., 1.);
    let angle = y.atan2(x) + place.turn.to_radians();
    (distance * angle.cos(), distance * angle.sin())
}

/// The place of (x, y) from a main colour at (main_x, main_y).
pub fn place_of(main_x: f32, main_y: f32, x: f32, y: f32) -> Place {
    let turn = (y.atan2(x) - main_y.atan2(main_x))
        .to_degrees()
        .rem_euclid(360.);
    Place {
        turn,
        lift: x.hypot(y) - main_x.hypot(main_y),
    }
}

/// (x, y), brought in to the field's disc.
fn in_disc(x: f32, y: f32) -> (f32, f32) {
    let distance = x.hypot(y);
    if distance > 1. {
        (x / distance, y / distance)
    } else {
        (x, y)
    }
}

/// The colour at (x, y) in the field, at HSL `saturation` (the field's own is 1), 0xRRGGBB.
pub fn color_at(x: f32, y: f32, saturation: f32) -> u32 {
    let hue = y.atan2(x).to_degrees().rem_euclid(360.) / 360.;
    packed(hsla(hue, saturation, x.hypot(y).min(1.), 1.).into())
}

/// `color` as 0xRRGGBB.
pub fn packed(color: Rgba) -> u32 {
    let channel = |c: f32| (c.clamp(0., 1.) * 255.).round() as u32;
    channel(color.r) << 16 | channel(color.g) << 8 | channel(color.b)
}

/// `from` mixed toward `to` by `t`, in OKLab, as the window's gradient between two colours is.
pub fn mix_oklab(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let (a, b) = (oklab(from), oklab(to));
    let mixed = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
    let [r, g, bl] = from_oklab(mixed);
    Rgba {
        r,
        g,
        b: bl,
        a: from.a + (to.a - from.a) * t,
    }
}

pub(crate) fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> f32 {
    let c = c.clamp(0., 1.);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    }
}

pub(crate) fn oklab(color: Rgba) -> [f32; 3] {
    let (r, g, b) = (to_linear(color.r), to_linear(color.g), to_linear(color.b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

pub(crate) fn from_oklab(lab: [f32; 3]) -> [f32; 3] {
    oklab_to_linear(lab).map(from_linear)
}

/// The linear sRGB channels of OKLab `[l, a, b]`, unclamped: outside 0 to 1 where it's out of
/// gamut.
pub(crate) fn oklab_to_linear([l, a, b]: [f32; 3]) -> [f32; 3] {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l_ - 3.3077118613 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076127010 * s_,
    ]
}

/// How far along the window's diagonal, from its top left (0) to its bottom right (1), the point
/// `x` and `y` of the way across and down is, in a window `width` by `height`: a 135° CSS
/// gradient's line, whose ends' perpendiculars pass through those corners.
pub fn diagonal(x: f32, y: f32, width: f32, height: f32) -> f32 {
    ((x * width + y * height) / (width + height).max(1.)).clamp(0., 1.)
}

pub use crate::tokens::theme::{GRAIN_STRENGTH, INACTIVE_TEXTURE};

/// A texture's side, in device pixels: a tile the window repeats.
pub const GRAIN_TILE: usize = 256;

thread_local! {
    static TILES: [OnceCell<Arc<RenderImage>>; 4] = const { [const { OnceCell::new() }; 4] };
}

impl Texture {
    /// The tile to lay over the window's colours, each pixel white or black, as many of each, so
    /// it neither lightens nor darkens them on the whole. Made once, the same each time.
    pub fn image(self) -> Arc<RenderImage> {
        TILES.with(|tiles| {
            tiles[self as usize]
                .get_or_init(|| {
                    let data = self.pixels().into_iter().flat_map(|(white, alpha)| {
                        let shade = if white { 255 } else { 0 };
                        [
                            shade,
                            shade,
                            shade,
                            (alpha.clamp(0., 1.) * 255.).round() as u8,
                        ]
                    });
                    let image =
                        RgbaImage::from_raw(GRAIN_TILE as u32, GRAIN_TILE as u32, data.collect())
                            .expect("sized to fit");
                    Arc::new(RenderImage::new(vec![Frame::new(image)]))
                })
                .clone()
        })
    }

    /// Each pixel of the tile, row by row: whether it's white, and how opaque. Every pattern
    /// repeats within the tile, so tiles meet without a seam.
    pub(crate) fn pixels(self) -> Vec<(bool, f32)> {
        const SIDE: usize = GRAIN_TILE;
        let mut random = Random(0x9e37_79b9);
        let noise = |random: &mut Random| {
            let value = random.next() as f32 / u32::MAX as f32 * 2. - 1.;
            (value >= 0., value.abs())
        };
        match self {
            Texture::Noise => (0..SIDE * SIDE).map(|_| noise(&mut random)).collect(),
            Texture::Dust => (0..SIDE * SIDE)
                .map(|_| {
                    let (white, alpha) = noise(&mut random);
                    // One pixel in about twelve is a speck, the rest clear.
                    if alpha < 1. / 12. {
                        (white, 0.6 + 0.4 * random.unit())
                    } else {
                        (white, 0.)
                    }
                })
                .collect(),
            Texture::Weave => {
                // Each thread along the diagonal is a shade of its own, so the twill isn't flat.
                let threads: Vec<f32> = (0..SIDE).map(|_| 0.45 + 0.55 * random.unit()).collect();
                (0..SIDE * SIDE)
                    .map(|at| {
                        let (x, y) = (at % SIDE, at / SIDE);
                        let white = (x + y) % 4 < 2;
                        let (_, fine) = noise(&mut random);
                        (white, threads[(x + SIDE - y) % SIDE] * (0.7 + 0.3 * fine))
                    })
                    .collect()
            }
            Texture::Fleck => {
                let mut pixels: Vec<(bool, f32)> = (0..SIDE * SIDE)
                    .map(|_| {
                        let (white, alpha) = noise(&mut random);
                        (white, alpha * 0.4)
                    })
                    .collect();
                // Fibres 4 to 9 long, across or down, wrapping round the tile's edges.
                for _ in 0..SIDE * SIDE / 40 {
                    let (x, y) = (random.below(SIDE), random.below(SIDE));
                    let (white, length) = (random.next() & 1 == 0, 4 + random.below(6));
                    let across = random.next() & 1 == 0;
                    let alpha = 0.7 + 0.3 * random.unit();
                    for step in 0..length {
                        let (x, y) = if across {
                            ((x + step) % SIDE, y)
                        } else {
                            (x, (y + step) % SIDE)
                        };
                        pixels[y * SIDE + x] = (white, alpha);
                    }
                }
                pixels
            }
        }
    }
}

/// xorshift32, so every tile is the same each time.
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }

    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }
}

#[cfg(test)]
mod tests;
