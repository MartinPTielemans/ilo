//! A Boost's edits without code (#695): what the Boost editor sets for a site, as the one block of
//! CSS it keeps in the site's file, rewritten whole on each change. The user's own lines and Zap's
//! stay as they are, above or below it. The block starts with a comment that holds the edits, so
//! the editor reads back what it wrote, not CSS it would have to parse.

use crate::palette::{INK, PAPER};
use crate::theme;

/// The block's first and last lines.
pub const START: &str = "/* ilo's editor: start. Rewritten on each change. */";
pub const END: &str = "/* ilo's editor: end */";

/// The page size's bounds and step, in percent, as Arc's (90 to 150 %).
pub const SMALLEST: u16 = 90;
pub const LARGEST: u16 = 150;
pub const SIZE_STEP: u16 = 10;

/// Advanced's sliders' step, in percent.
pub const ADJUST_STEP: u16 = 5;

/// What Advanced's sliders set, each in percent, 100 changing nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adjust {
    Contrast,
    Brightness,
    Saturation,
}

impl Adjust {
    pub const ALL: [Adjust; 3] = [Adjust::Contrast, Adjust::Brightness, Adjust::Saturation];

    pub fn title(self) -> &'static str {
        match self {
            Adjust::Contrast => "Contrast",
            Adjust::Brightness => "Brightness",
            Adjust::Saturation => "Saturation",
        }
    }

    /// Its name in the block's state, and its CSS filter's.
    fn name(self) -> &'static str {
        match self {
            Adjust::Contrast => "contrast",
            Adjust::Brightness => "brightness",
            Adjust::Saturation => "saturate",
        }
    }

    /// Its least and most, 100 in the middle: past half or double, a page stops reading.
    pub fn range(self) -> (u16, u16) {
        match self {
            Adjust::Saturation => (0, 200),
            _ => (50, 150),
        }
    }

    pub fn of(self, style: &Style) -> u16 {
        style.adjust[self as usize]
    }

    /// `style` with this at `value`, held in its range on its step.
    pub fn set(self, style: &mut Style, value: i32) {
        let (least, most) = self.range();
        let step = i32::from(ADJUST_STEP);
        let value =
            ((value + step / 2).div_euclid(step) * step).clamp(i32::from(least), i32::from(most));
        style.adjust[self as usize] = value as u16;
    }
}

/// A dot's place in the field, as the theme picker's: across and up from the middle, in a disc of
/// radius 1, whose angle is the hue and whose distance the lightness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub x: f32,
    pub y: f32,
}

impl Dot {
    /// The dot's colour, 0xRRGGBB, at the field's own saturation.
    pub fn color(self) -> u32 {
        theme::color_at(self.x, self.y, 1.)
    }
}

/// The faces the editor offers besides the site's own, as CSS's generic families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Sans,
    Serif,
    Rounded,
    Mono,
}

impl Face {
    pub const ALL: [Face; 4] = [Face::Sans, Face::Serif, Face::Rounded, Face::Mono];

    pub fn name(self) -> &'static str {
        match self {
            Face::Sans => "sans",
            Face::Serif => "serif",
            Face::Rounded => "rounded",
            Face::Mono => "mono",
        }
    }

    fn named(name: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|face| face.name() == name)
    }

    /// The tile's label.
    pub fn title(self) -> &'static str {
        match self {
            Face::Sans => "Sans",
            Face::Serif => "Serif",
            Face::Rounded => "Rounded",
            Face::Mono => "Mono",
        }
    }

    /// CSS's family for it.
    pub fn family(self) -> &'static str {
        match self {
            Face::Sans => "system-ui, sans-serif",
            Face::Serif => "ui-serif, serif",
            Face::Rounded => "ui-rounded, system-ui, sans-serif",
            Face::Mono => "ui-monospace, monospace",
        }
    }

    /// Whether this computer has the face, rather than another tile's in its place: only the Mac
    /// has a rounded system face, which Chromium gives `ui-rounded`.
    pub fn resolves(self) -> bool {
        self != Face::Rounded || cfg!(target_os = "macos")
    }
}

/// Text case, besides the site's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Lower,
    Upper,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Case::Lower => "lowercase",
            Case::Upper => "uppercase",
        }
    }

    fn named(name: &str) -> Option<Case> {
        [Case::Lower, Case::Upper]
            .into_iter()
            .find(|case| case.name() == name)
    }
}

/// What the editor sets for a site. The default changes nothing, and writes no block.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// The page's ground, whose text takes black or white by it.
    pub ground: Option<Dot>,
    /// The links' colour.
    pub links: Option<Dot>,
    /// The page's lightness turned round, its hues kept: a dark mode for the site.
    pub invert: bool,
    pub face: Option<Face>,
    /// The page's text size, in percent.
    pub size: u16,
    pub case: Option<Case>,
    /// Advanced's contrast, brightness and saturation, in percent, by `Adjust`.
    pub adjust: [u16; 3],
    /// Advanced is open in the editor for this Boost, as the user left it.
    pub advanced: bool,
    /// The user has dragged a dot of this Boost, so the field's hint has gone for good.
    pub dragged: bool,
    /// What the user named the Boost, which shows in place of its site.
    pub name: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            ground: None,
            links: None,
            invert: false,
            face: None,
            size: 100,
            case: None,
            adjust: [100; 3],
            advanced: false,
            dragged: false,
            name: None,
        }
    }
}

impl Style {
    /// The block for the site's file, or none for the default.
    pub fn block(&self) -> Option<String> {
        if *self == Style::default() {
            return None;
        }
        let mut out = format!("{START}\n/* {} */\n", self.state());
        let mut text = Vec::new();
        if let Some(ground) = self.ground {
            text.push(format!(
                "background-color: {} !important;",
                hex(ground.color())
            ));
            text.push(format!("color: {} !important;", hex(ink(ground.color()))));
        }
        if let Some(face) = self.face {
            text.push(format!("font-family: {} !important;", face.family()));
        }
        if let Some(case) = self.case {
            text.push(format!("text-transform: {} !important;", case.name()));
        }
        if !text.is_empty() {
            out += "html, body, body * {\n";
            text.iter().for_each(|line| out += &format!("  {line}\n"));
            out += "}\n";
        }
        if let Some(links) = self.links {
            out += &format!("a, a * {{ color: {} !important; }}\n", hex(links.color()));
        }
        // Advanced's after the inversion, so they work on the page as it's seen, its images too.
        let filter: Vec<String> = (self.invert)
            .then(|| "invert(1) hue-rotate(180deg)".to_owned())
            .into_iter()
            .chain(Adjust::ALL.into_iter().filter_map(|adjust| {
                let value = adjust.of(self);
                (value != 100).then(|| format!("{}({value}%)", adjust.name()))
            }))
            .collect();
        if !filter.is_empty() {
            out += &format!("html {{ filter: {} !important; }}\n", filter.join(" "));
        }
        // Images and video turned back, so they show as they are on the dark page.
        if self.invert {
            out += "img, video, picture, canvas, svg image { filter: invert(1) hue-rotate(180deg) \
                    !important; }\n";
        }
        if self.size != 100 {
            out += &format!("html {{ font-size: {}% !important; }}\n", self.size);
        }
        Some(out + END)
    }

    /// Shuffle's style, from `seed`: two dots anywhere in the field, the links' kept off its
    /// darkest and lightest so they read, and a face and case of those offered, the site's own
    /// among them. The page size, Advanced's and its being open stay the user's.
    pub fn shuffled(&self, seed: u64) -> Style {
        // SplitMix64's mix first, so near seeds start far apart.
        let mut state = seed.wrapping_add(0x9e3779b97f4a7c15);
        state = (state ^ (state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        state = (state ^ (state >> 27)).wrapping_mul(0x94d049bb133111eb);
        state = (state ^ (state >> 31)) | 1;
        let mut next = move || {
            // xorshift64: enough to vary a look, and the same for the same seed in tests.
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut dot = |least: f32, most: f32| {
            let turn = next() * std::f32::consts::TAU;
            let distance = least + (most - least) * next().sqrt();
            Dot {
                x: distance * turn.cos(),
                y: distance * turn.sin(),
            }
        };
        let ground = dot(0., 1.);
        let links = dot(0.35, 0.75);
        let faces: Vec<Option<Face>> = std::iter::once(None)
            .chain(
                Face::ALL
                    .into_iter()
                    .filter(|face| face.resolves())
                    .map(Some),
            )
            .collect();
        let face = faces[(next() * faces.len() as f32) as usize % faces.len()];
        let cases = [None, None, Some(Case::Lower), Some(Case::Upper)];
        let case = cases[(next() * 4.) as usize % 4];
        Style {
            ground: Some(ground),
            links: Some(links),
            face,
            case,
            ..self.clone()
        }
    }

    /// The edits, as the block's second line keeps them.
    pub fn state(&self) -> String {
        let dot =
            |name: &str, dot: Option<Dot>| dot.map(|Dot { x, y }| format!("{name} {x:.3} {y:.3}"));
        [
            dot("ground", self.ground),
            dot("links", self.links),
            self.invert.then(|| "invert".to_owned()),
            self.face.map(|face| format!("face {}", face.name())),
            (self.size != 100).then(|| format!("size {}", self.size)),
            self.case.map(|case| format!("case {}", case.name())),
        ]
        .into_iter()
        .chain(Adjust::ALL.into_iter().map(|adjust| {
            let value = adjust.of(self);
            (value != 100).then(|| format!("{} {value}", adjust.name()))
        }))
        .chain([
            self.advanced.then(|| "advanced".to_owned()),
            self.dragged.then(|| "dragged".to_owned()),
            // Last, as it may hold spaces.
            self.name.as_deref().map(|name| format!("name {name}")),
        ])
        .flatten()
        .collect::<Vec<_>>()
        .join("; ")
    }

    /// The edits the block in `css` keeps, the default with none, or with one the editor didn't
    /// write.
    pub fn of(css: &str) -> Style {
        let mut style = Style::default();
        let Some((block, _)) = block_in(css) else {
            return style;
        };
        let Some(state) = block.lines().nth(1).and_then(|line| {
            line.strip_prefix("/* ")
                .and_then(|line| line.strip_suffix(" */"))
        }) else {
            return style;
        };
        let (state, name) = match state.split_once("name ") {
            Some((state, name)) if state.is_empty() || state.ends_with("; ") => (state, Some(name)),
            _ => (state, None),
        };
        style.name = name.and_then(|name| named(name));
        for item in state.split("; ") {
            let words: Vec<&str> = item.split(' ').collect();
            let dot = || {
                let number = |at: usize| words.get(at)?.parse::<f32>().ok();
                Some(Dot {
                    x: number(1)?.clamp(-1., 1.),
                    y: number(2)?.clamp(-1., 1.),
                })
            };
            match words[0] {
                "ground" => style.ground = dot(),
                "links" => style.links = dot(),
                "invert" => style.invert = true,
                "face" => style.face = words.get(1).and_then(|name| Face::named(name)),
                "size" => {
                    let size = words.get(1).and_then(|size| size.parse().ok());
                    style.size = size.map_or(100, |size: u16| size.clamp(SMALLEST, LARGEST));
                }
                "case" => style.case = words.get(1).and_then(|name| Case::named(name)),
                "advanced" => style.advanced = true,
                "dragged" => style.dragged = true,
                name => {
                    let adjust = Adjust::ALL.into_iter().find(|each| each.name() == name);
                    let value = words.get(1).and_then(|value| value.parse::<i32>().ok());
                    if let (Some(adjust), Some(value)) = (adjust, value) {
                        adjust.set(&mut style, value);
                    }
                }
            }
        }
        style
    }

    /// `css` with its block this style's, in place of the one it had, else at its end, the rest
    /// of it as it was. The default takes the block out.
    pub fn into_file(&self, css: &str) -> String {
        let block = self.block();
        let Some((_, at)) = block_in(css) else {
            return match block {
                None => css.to_owned(),
                Some(block) if css.trim().is_empty() => block + "\n",
                Some(block) => {
                    let gap = if css.ends_with('\n') { "" } else { "\n" };
                    format!("{css}{gap}{block}\n")
                }
            };
        };
        let (before, after) = (&css[..at.start], &css[at.end..]);
        match block {
            Some(block) => format!("{before}{block}{after}"),
            // The block's own line goes with it.
            None => format!("{before}{}", after.strip_prefix('\n').unwrap_or(after)),
        }
    }
}

/// `name` as a Boost's name: on one line, without what would end the comment that keeps it,
/// trimmed, and none if that leaves nothing.
pub fn named(name: &str) -> Option<String> {
    let name = name.replace("*/", "").replace(['\n', '\r'], " ");
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The block in `css`, from its first line to its last, and where it is.
fn block_in(css: &str) -> Option<(&str, std::ops::Range<usize>)> {
    let start = css.find(START)?;
    let end = start + css[start..].find(END)? + END.len();
    Some((&css[start..end], start..end))
}

/// Black or white, whichever reads better on `ground`, by WCAG's contrast.
pub fn ink(ground: u32) -> u32 {
    let channel = |shift: u32| {
        let c = ((ground >> shift) & 0xff) as f32 / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
    // White's contrast (1.05 / (L + 0.05)) against black's ((L + 0.05) / 0.05) meet at 0.179.
    if luminance > 0.179 { INK } else { PAPER }
}

fn hex(color: u32) -> String {
    format!("#{color:06x}")
}

#[cfg(test)]
mod tests;
