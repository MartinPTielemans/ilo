//! The product's name and mark, from this one place, so the next change of either is one commit.
//! No other string spells the name out: one a user reads takes `PRODUCT`, as `{PRODUCT}` in a
//! format string or `env!("PRODUCT_NAME")` in a `concat!`, and a test below fails on one that
//! doesn't. The name itself is in branding/product. What an agent is told, the command an agent
//! types and the name its clients list the server by take it too. Names in code, ids, the data
//! folder, the bundle and its files keep branding/name.

use gpui::{Styled, Svg, px, svg};

use crate::palette::Palette;
use crate::tokens::mark::{GHOST, INK, LARGE_FROM, REVEAL, SMALL_BELOW};

/// The product's name as a user reads it: lowercase always, also where a sentence starts, and
/// never followed by a word for what it is. From branding/product, so a fork changes one file; a
/// fixed string takes it as `concat!(…, env!("PRODUCT_NAME"), …)`.
pub const PRODUCT: &str = env!("PRODUCT_NAME");

const SMALL: &str = "brand/knot-16.svg";
const MIDDLE: &str = "brand/knot-32.svg";
const LARGE: &str = "brand/knot.svg";

/// The mark's drawings, black ink on nothing, each cut close to the cord so a slot of a size draws
/// a mark of that size; the app tints them. The shell's assets serve them.
pub(crate) const MARKS: [(&str, &[u8]); 3] = [
    (SMALL, include_bytes!("../assets/brand/knot-16.svg")),
    (MIDDLE, include_bytes!("../assets/brand/knot-32.svg")),
    (LARGE, include_bytes!("../assets/brand/knot.svg")),
];

/// The drawing for a mark `size` points across.
pub fn mark_path(size: f32) -> &'static str {
    if size < SMALL_BELOW {
        SMALL
    } else if size < LARGE_FROM {
        MIDDLE
    } else {
        LARGE
    }
}

/// The mark, `size` points across, in the text's colour. It never turns; the one that moves is the
/// sign over the composer, a gap travelling along its cord (`gapped_mark_path`).
pub fn mark(size: f32, palette: &Palette) -> Svg {
    svg()
        .path(mark_path(size))
        .size(px(size))
        .flex_none()
        .text_color(palette.text(INK))
}

/// Where the assets serve the mark drawn part of the way along its cord (drawn_mark).
const DRAWN: &str = "brand/drawn/";

/// The drawing of the mark `size` points across drawn `fraction` of the way along its cord, as
/// first run's progress and its entry draw it (first-run-v9 §1), in thousandths, so a motion asks
/// for at most a thousand drawings of a size. Never the small drawing, which has no under-passes
/// to close as the cord arrives.
pub fn drawn_mark_path(size: f32, fraction: f32) -> String {
    let drawing = mark_path(size.max(SMALL_BELOW));
    let name = drawing
        .trim_start_matches("brand/")
        .trim_end_matches(".svg");
    let thousandths = (fraction.clamp(0., 1.) * 1000.).round() as u32;
    format!("{DRAWN}{name}/{thousandths}.svg")
}

/// The drawing `drawn_mark_path` names: the mark's own, with its whole cord at `GHOST` under a copy
/// cut by a mask that reveals the cord from where it starts, as a dash along it, each over-pass
/// kept out until the cord has come a third further. The mark isn't redrawn: both copies are its
/// own paths and its own under-pass mask.
pub(crate) fn drawn_mark(path: &str) -> Option<String> {
    let (name, thousandths) = path
        .strip_prefix(DRAWN)?
        .strip_suffix(".svg")?
        .split_once('/')?;
    let fraction = thousandths.parse::<f32>().ok()? / 1000.;
    let (_, bytes) = MARKS.iter().find(|(drawing, _)| {
        drawing
            .trim_start_matches("brand/")
            .trim_end_matches(".svg")
            == name
    })?;
    drawn(std::str::from_utf8(bytes).ok()?, fraction)
}

fn drawn(svg: &str, fraction: f32) -> Option<String> {
    let open = &svg[..svg.find('>')? + 1];
    let (defs, rest) = svg.split_once("</defs>")?;
    let defs = defs.strip_prefix(open)?.strip_prefix("<defs>")?;
    let body = rest.strip_suffix("</svg>")?;
    // The cord inside the under-passes' mask, then the over-passes, each where the cord comes to
    // it in turn.
    let (cord, overs) = body.split_once("</g>")?;
    let d = |path: &str| -> Option<String> {
        let from = path.find(" d=\"")? + 4;
        Some(path[from..from + path[from..].find('"')?].to_owned())
    };
    let width: f32 = {
        let from = cord.find("stroke-width=\"")? + 14;
        cord[from..from + cord[from..].find('"')?].parse().ok()?
    };
    let cord_d = d(cord)?;
    let length = cord_length(&cord_d)?;
    let cut = width + REVEAL;
    let kept_out: String = (overs.split("/>").filter_map(d).enumerate())
        .filter(|(at, _)| fraction <= *at as f32 / 3. + 0.001)
        .map(|(_, over)| {
            format!(
                "<path d=\"{over}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"{cut}\" \
                 stroke-linecap=\"round\"/>"
            )
        })
        .collect();
    let shown = length * fraction;
    Some(format!(
        "{open}<defs>{defs}<mask id=\"drawn\" maskUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" \
         width=\"120\" height=\"120\"><path d=\"{cord_d}\" fill=\"none\" stroke=\"#FFFFFF\" \
         stroke-width=\"{cut}\" stroke-dasharray=\"{shown:.3} {length:.3}\"/>{kept_out}</mask>\
         </defs><g opacity=\"{GHOST}\">{body}</g><g mask=\"url(#drawn)\">{body}</g></svg>"
    ))
}

/// Where the assets serve the small mark with a gap in its cord (gapped_mark).
const GAPPED: &str = "brand/gap/";

/// The small mark with a gap of `gap` of its cord's length, starting `at` of the way along it, as
/// the sign over the composer moves (spec-thread-stage.md, "A wait on the user"). Each in
/// thousandths, so a motion asks for at most a thousand drawings of a gap.
pub fn gapped_mark_path(at: f32, gap: f32) -> String {
    let at = (at.rem_euclid(1.) * 1000.).round() as u32 % 1000;
    let gap = (gap.clamp(0., 1.) * 1000.).round() as u32;
    format!("{GAPPED}{at}-{gap}.svg")
}

/// The drawing `gapped_mark_path` names: the small mark's own, its cord dashed to leave out the
/// gap alone. The mark isn't redrawn.
pub(crate) fn gapped_mark(path: &str) -> Option<String> {
    let (at, gap) = path
        .strip_prefix(GAPPED)?
        .strip_suffix(".svg")?
        .split_once('-')?;
    let (at, gap) = (
        at.parse::<f32>().ok()? / 1000.,
        gap.parse::<f32>().ok()? / 1000.,
    );
    gapped(small()?, at, gap)
}

fn gapped(svg: &str, at: f32, gap: f32) -> Option<String> {
    if gap <= 0. {
        return Some(svg.to_owned());
    }
    let length = cord_length(cord_of(svg)?)?;
    let gap = gap * length;
    // The dash starts where the cord does, so it is set back to end where the gap starts.
    let offset = (length - gap - at * length).rem_euclid(length);
    let end = svg.find("/>")?;
    Some(format!(
        "{} stroke-dasharray=\"{:.3} {gap:.3}\" stroke-dashoffset=\"{offset:.3}\"{}",
        &svg[..end],
        length - gap,
        &svg[end..]
    ))
}

/// How far along the small mark's cord, as a fraction of it, the top of its upper right loop is:
/// the cord's highest point right of its middle, where the waiting sign opens its gap.
pub fn crown() -> f32 {
    static CROWN: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *CROWN.get_or_init(|| {
        let points = small().and_then(cord_of).and_then(cord).unwrap_or_default();
        let length = points.last().map_or(1., |(_, along)| *along);
        let (left, right) = (points.iter())
            .fold((f32::MAX, f32::MIN), |(left, right), ((x, _), _)| {
                (left.min(*x), right.max(*x))
            });
        let middle = (left + right) / 2.;
        (points.iter())
            .filter(|((x, _), _)| *x > middle)
            .min_by(|((_, a), _), ((_, b), _)| a.total_cmp(b))
            .map_or(0., |(_, along)| along / length)
    })
}

/// The small drawing, which is one path: its cord.
fn small() -> Option<&'static str> {
    let (_, bytes) = MARKS.iter().find(|(drawing, _)| *drawing == SMALL)?;
    std::str::from_utf8(bytes).ok()
}

/// The `d` of a drawing's first path.
fn cord_of(svg: &str) -> Option<&str> {
    let from = svg.find(" d=\"")? + 4;
    Some(&svg[from..from + svg[from..].find('"')?])
}

/// The length of a path of absolute moves, cubic curves (C and S) and lines, as the drawings'
/// cords are, measured along each curve in short chords.
fn cord_length(d: &str) -> Option<f32> {
    cord(d)?.last().map(|(_, along)| *along)
}

/// The points of such a path, each curve in short chords, with how far along the path each is.
fn cord(d: &str) -> Option<Vec<((f32, f32), f32)>> {
    let mut words = Vec::new();
    let mut number = String::new();
    for c in d.chars() {
        let starts =
            c.is_ascii_alphabetic() || c == ' ' || c == ',' || (c == '-' && !number.is_empty());
        if starts && !number.is_empty() {
            words.push(Word::Number(number.parse().ok()?));
            number.clear();
        }
        match c {
            c if c.is_ascii_alphabetic() => words.push(Word::Command(c)),
            ' ' | ',' => {}
            c => number.push(c),
        }
    }
    if !number.is_empty() {
        words.push(Word::Number(number.parse().ok()?));
    }
    let (mut at, mut start, mut control) = ((0f32, 0f32), (0f32, 0f32), None);
    let (mut points, mut command, mut index) = (Vec::new(), 'M', 0);
    let numbers = |index: &mut usize, n: usize| -> Option<Vec<f32>> {
        let mut taken = Vec::with_capacity(n);
        while taken.len() < n {
            match words.get(*index)? {
                Word::Number(value) => taken.push(*value),
                Word::Command(_) => return None,
            }
            *index += 1;
        }
        Some(taken)
    };
    let curve = |from: (f32, f32), a: (f32, f32), b: (f32, f32), to: (f32, f32)| {
        (1..=16).map(move |step| {
            let t = step as f32 / 16.;
            let u = 1. - t;
            let mix = |p: f32, q: f32, r: f32, s: f32| {
                u * u * u * p + 3. * u * u * t * q + 3. * u * t * t * r + t * t * t * s
            };
            (mix(from.0, a.0, b.0, to.0), mix(from.1, a.1, b.1, to.1))
        })
    };
    while index < words.len() {
        if let Word::Command(c) = words[index] {
            command = c;
            index += 1;
            if c == 'Z' || c == 'z' {
                walk(&mut points, start);
                at = start;
                control = None;
                continue;
            }
        }
        match command {
            'M' => {
                let p = numbers(&mut index, 2)?;
                at = (p[0], p[1]);
                start = at;
                control = None;
                // A move goes nowhere along the cord.
                let along = points.last().map_or(0., |(_, along)| *along);
                points.push((at, along));
                // Pairs after a move are lines.
                command = 'L';
            }
            'L' => {
                let p = numbers(&mut index, 2)?;
                at = (p[0], p[1]);
                walk(&mut points, at);
                control = None;
            }
            'C' => {
                let p = numbers(&mut index, 6)?;
                let (a, b, to) = ((p[0], p[1]), (p[2], p[3]), (p[4], p[5]));
                curve(at, a, b, to).for_each(|point| walk(&mut points, point));
                (at, control) = (to, Some(b));
            }
            'S' => {
                let p = numbers(&mut index, 4)?;
                // The first control is the last one's mirror, or the point itself after no curve.
                let a = control.map_or(at, |(x, y)| (2. * at.0 - x, 2. * at.1 - y));
                let (b, to) = ((p[0], p[1]), (p[2], p[3]));
                curve(at, a, b, to).for_each(|point| walk(&mut points, point));
                (at, control) = (to, Some(b));
            }
            _ => return None,
        }
    }
    Some(points)
}

/// Goes on along the cord to `to`.
fn walk(points: &mut Vec<((f32, f32), f32)>, to: (f32, f32)) {
    let along = points.last().map_or(0., |&((x, y), along)| {
        along + ((to.0 - x).powi(2) + (to.1 - y).powi(2)).sqrt()
    });
    points.push((to, along));
}

enum Word {
    Command(char),
    Number(f32),
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use gpui::{AssetSource, SvgRenderer};

    use super::{
        LARGE, MARKS, MIDDLE, PRODUCT, SMALL, drawn_mark_path, gapped_mark_path, mark_path,
    };
    use crate::shell::Assets;

    #[test]
    fn the_name_is_lowercase_and_one_word() {
        assert!(!PRODUCT.is_empty());
        assert_eq!(PRODUCT, PRODUCT.to_lowercase(), "the name is lowercase");
        assert!(
            !PRODUCT.contains(char::is_whitespace),
            "the name has no space: {PRODUCT:?}"
        );
    }

    #[test]
    fn the_mark_is_the_small_drawing_under_24_and_the_large_from_40() {
        for (size, drawing) in [
            (14., SMALL),
            (16., SMALL),
            (23.9, SMALL),
            (24., MIDDLE),
            (32., MIDDLE),
            (39.9, MIDDLE),
            (40., LARGE),
            (48., LARGE),
            (120., LARGE),
        ] {
            assert_eq!(mark_path(size), drawing, "the mark at {size}");
        }
        for (path, bytes) in MARKS {
            let served = Assets.load(path).unwrap();
            assert_eq!(served.as_deref(), Some(bytes), "the assets serve {path}");
        }
    }

    /// How much of `bytes`, drawn as GPUI draws a glyph, has any ink.
    fn inked(bytes: &[u8]) -> usize {
        let renderer = SvgRenderer::new(Arc::new(Assets));
        let image = renderer.render_single_frame(bytes, 1.).unwrap();
        let pixels = image.as_bytes(0).unwrap();
        pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0).count()
    }

    /// The ink a drawing lays, each pixel weighed by its alpha.
    fn ink(bytes: &[u8]) -> u64 {
        let renderer = SvgRenderer::new(Arc::new(Assets));
        let image = renderer.render_single_frame(bytes, 1.).unwrap();
        let pixels = image.as_bytes(0).unwrap();
        pixels.chunks_exact(4).map(|pixel| pixel[3] as u64).sum()
    }

    /// First run's progress draws the mark along its cord: the whole cord faint before it starts,
    /// more ink at each step along, and the mark itself at the end.
    #[test]
    fn the_drawn_mark_gains_ink_along_its_cord_and_ends_as_the_mark() {
        for size in [32., 72.] {
            let drawn = |fraction: f32| {
                let path = drawn_mark_path(size, fraction);
                let bytes = Assets
                    .load(&path)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{path}"));
                ink(&bytes)
            };
            let (_, whole) = MARKS
                .iter()
                .find(|(each, _)| *each == mark_path(size))
                .unwrap();
            let whole = ink(whole);
            let steps: Vec<u64> = [0., 0.25, 0.5, 0.75, 1.].map(drawn).to_vec();
            assert!(
                steps[0] > 0 && steps[0] * 3 < whole,
                "{size}: {steps:?} of {whole}"
            );
            assert!(
                steps.windows(2).all(|pair| pair[0] < pair[1]),
                "{size}: {steps:?}"
            );
            assert!(steps[4] * 100 > whole * 98, "{size}: {steps:?} of {whole}");
        }
    }

    /// Each pixel's alpha in `bytes` drawn as GPUI draws a glyph.
    fn alphas(bytes: &[u8]) -> Vec<u8> {
        let renderer = SvgRenderer::new(Arc::new(Assets));
        let image = renderer.render_single_frame(bytes, 1.).unwrap();
        let pixels = image.as_bytes(0).unwrap();
        pixels.chunks_exact(4).map(|pixel| pixel[3]).collect()
    }

    fn gapped(at: f32, gap: f32) -> Vec<u8> {
        let path = gapped_mark_path(at, gap);
        Assets
            .load(&path)
            .unwrap()
            .unwrap_or_else(|| panic!("{path}"))
            .into_owned()
    }

    /// The sign's gap leaves out about its share of the small mark's cord wherever along the cord
    /// it is, across the cord's start too, in a different place at each; with no gap it is the
    /// mark itself.
    #[test]
    fn a_gap_leaves_out_its_share_of_the_cord_where_it_is() {
        let (_, small) = MARKS.iter().find(|(each, _)| *each == SMALL).unwrap();
        let whole = ink(small);
        let mut drawn = Vec::new();
        for at in [0., 0.3, 0.75, 0.95] {
            let bytes = gapped(at, 0.16);
            let out = 1. - ink(&bytes) as f32 / whole as f32;
            assert!(
                (0.06..0.26).contains(&out),
                "a gap of 0.16 at {at} left out {out} of the mark's ink"
            );
            drawn.push(alphas(&bytes));
        }
        assert!(
            drawn.windows(2).all(|pair| pair[0] != pair[1]),
            "the gap stood in one place"
        );
        assert_eq!(gapped(0.4, 0.), small.to_vec(), "no gap isn't the mark");
    }

    /// The small drawing has no under-passes to close, so the drawn mark never uses it.
    #[test]
    fn the_drawn_mark_is_never_the_small_drawing() {
        assert!(!drawn_mark_path(12., 0.5).contains("knot-16"));
    }

    /// The large drawings' under-passes are cut by a mask, which GPUI's drawing keeps: without it
    /// the cord would run unbroken where it passes under.
    #[test]
    fn the_under_passes_are_cut_where_gpui_draws_the_mark() {
        for path in [MIDDLE, LARGE] {
            let (_, bytes) = MARKS.iter().find(|(each, _)| *each == path).unwrap();
            let text = std::str::from_utf8(bytes).unwrap();
            let unmasked = text.replace(" mask=\"url(#under)\"", "");
            assert_ne!(unmasked, text, "{path} has a mask");
            let (cut, whole) = (inked(bytes), inked(unmasked.as_bytes()));
            assert!(
                cut > 0 && cut * 100 < whole * 97,
                "{path}: the mask should cut the cord where it passes under, but drawn with it \
                 {cut} pixels have ink and without it {whole}"
            );
        }
    }

    /// Where `text`, Rust source, has a string literal that spells the product's name out, as its
    /// line. A string a user reads names the product through `PRODUCT`. What tells it from the
    /// weaving words, which share the name's letters: the name never runs straight into "in", "at"
    /// or a placeholder ("Woven in {duration}", "Woven at {time}", "Woven {time}", "You're woven
    /// in."). Comments, names in code and characters are never strings.
    fn spelled_out(text: &str) -> Vec<usize> {
        let chars: Vec<char> = text.chars().collect();
        let (mut at, mut line, mut found) = (0, 1, Vec::new());
        let get = |at: usize| chars.get(at).copied();
        let lines = |from: usize, to: usize| chars[from..to].iter().filter(|c| **c == '\n').count();
        while at < chars.len() {
            match chars[at] {
                '/' if get(at + 1) == Some('/') => {
                    while at < chars.len() && chars[at] != '\n' {
                        at += 1;
                    }
                }
                '/' if get(at + 1) == Some('*') => {
                    let (from, mut depth) = (at, 0);
                    while at < chars.len() {
                        if chars[at] == '/' && get(at + 1) == Some('*') {
                            depth += 1;
                            at += 2;
                        } else if chars[at] == '*' && get(at + 1) == Some('/') {
                            depth -= 1;
                            at += 2;
                            if depth == 0 {
                                break;
                            }
                        } else {
                            at += 1;
                        }
                    }
                    line += lines(from, at);
                }
                '\'' => {
                    // A character, which may be a quote, or a lifetime, which has no end.
                    if get(at + 1) == Some('\\') {
                        at += 3;
                        while at < chars.len() && chars[at] != '\'' {
                            at += 1;
                        }
                        at += 1;
                    } else if get(at + 2) == Some('\'') {
                        at += 3;
                    } else {
                        at += 1;
                    }
                }
                'r' if starts_word(&chars[..at]) && matches!(get(at + 1), Some('#' | '"')) => {
                    let mut hashes = 0;
                    while get(at + 1 + hashes) == Some('#') {
                        hashes += 1;
                    }
                    if get(at + 1 + hashes) != Some('"') {
                        at += 1;
                        continue;
                    }
                    let from = at + 2 + hashes;
                    let mut end = from;
                    while end < chars.len()
                        && !(chars[end] == '"'
                            && (1..=hashes).all(|n| chars.get(end + n) == Some(&'#')))
                    {
                        end += 1;
                    }
                    if names(&chars[from..end.min(chars.len())]) {
                        found.push(line);
                    }
                    line += lines(at, end.min(chars.len()));
                    at = end + 1 + hashes;
                }
                '"' => {
                    let from = at + 1;
                    let mut end = from;
                    while end < chars.len() && chars[end] != '"' {
                        end += if chars[end] == '\\' { 2 } else { 1 };
                    }
                    let end = end.min(chars.len());
                    if names(&chars[from..end]) {
                        found.push(line);
                    }
                    line += lines(at, end);
                    at = end + 1;
                }
                '\n' => {
                    line += 1;
                    at += 1;
                }
                _ => at += 1,
            }
        }
        found
    }

    /// Whether what comes after `before` starts a word of its own, or a byte string's `b` does.
    fn starts_word(before: &[char]) -> bool {
        let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric() || *c == '_');
        match before {
            [.., first, 'b'] => !word(Some(first)),
            ['b'] => true,
            _ => !word(before.last()),
        }
    }

    /// Whether a string's `text` spells the name out, by the rule `spelled_out` gives.
    fn names(text: &[char]) -> bool {
        let lower: Vec<char> = text.iter().flat_map(|c| c.to_lowercase()).collect();
        let name: Vec<char> = PRODUCT.chars().collect();
        let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric() || *c == '_');
        (0..lower.len()).any(|at| {
            if !lower[at..].starts_with(&name) || word(at.checked_sub(1).and_then(|b| lower.get(b)))
            {
                return false;
            }
            let rest = &lower[at + name.len()..];
            let then = |next: &str| {
                let next: Vec<char> = next.chars().collect();
                rest.starts_with(&next) && !word(rest.get(next.len()))
            };
            let weaving = then(" in") || then(" at") || rest.starts_with(&[' ', '{']);
            !word(rest.first()) && !weaving
        })
    }

    #[test]
    fn the_spelling_out_rule_tells_strings_from_the_rest() {
        let n = PRODUCT;
        let cap = format!("{}{}", n[..1].to_uppercase(), &n[1..]);
        let red = [
            format!("let _ = \"{n} stopped the thread\";"),
            format!("let _ = \"{n}'s settings\";"),
            format!("let _ = \"Ask {n}\";"),
            format!("let _ = \"{cap} stopped here\";"),
            format!("let _ = r#\"{n} is reading\"#;"),
            format!("let _ = '\"'; let _ = \"{n}\";"),
            format!("let _ = \"a \\\" {n}\";"),
            format!("let _ = '\\''; let _ = \"{n}\";"),
            format!("let _ = br\"\\\"; let _ = \"{n}\";"),
        ];
        for source in &red {
            assert_eq!(spelled_out(source), [1], "should be found: {source}");
        }
        let green = [
            format!("// {n} stopped the thread"),
            format!("/* {n} */ let _ = 1;"),
            format!("let {n}_thread = 1;"),
            format!("let _ = \"{cap} in {{duration}}\";"),
            format!("let _ = \"{cap} at {{time}}\";"),
            format!("let _ = \"{cap} {{time}} · {{duration}}\";"),
            format!("let _ = \"You're {n} in.\";"),
            format!("let _ = \"{n}browser.com\";"),
            format!("fn f<'a>(x: &'a str) {{}} // \"{n}\""),
            "let _ = \"{PRODUCT} stopped the thread\";".to_owned(),
        ];
        for source in &green {
            assert_eq!(spelled_out(source), [0; 0], "shouldn't be found: {source}");
        }
        assert_eq!(spelled_out(&format!("\n\n/*\n*/\nlet _ = \"{n}\";")), [5]);
    }

    /// Every string in the app's source a user could read takes the name from `PRODUCT`; this file
    /// is the one place that spells it.
    #[test]
    fn no_string_in_the_app_spells_the_name_out() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let here = src.join("product.rs");
        let mut files = vec![src];
        let mut found = Vec::new();
        while let Some(path) = files.pop() {
            if path.is_dir() {
                files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            } else if path.extension().is_some_and(|e| e == "rs") && path != here {
                let text = std::fs::read_to_string(&path).unwrap();
                for line in spelled_out(&text) {
                    found.push(format!("{}:{line}", path.display()));
                }
            }
        }
        found.sort();
        assert!(
            found.is_empty(),
            "these strings spell the product's name out; take it from product::PRODUCT instead:\n{}",
            found.join("\n")
        );
    }
}
