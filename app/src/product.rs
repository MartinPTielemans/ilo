//! The product's name and mark, from this one place, so the next change of either is one commit.
//! No other string spells the name out: one a user reads takes `PRODUCT`, as `{PRODUCT}` in a
//! format string or `env!("PRODUCT_NAME")` in a `concat!`, and a test below fails on one that
//! doesn't. The name itself is in branding/product. Names in code, ids, files, the
//! data folder and what an assistant is told aren't the product's name as a user reads it, and
//! keep theirs.

use gpui::{Styled, Svg, px, svg};

use crate::palette::Palette;
use crate::tokens::mark::{INK, LARGE_FROM, SMALL_BELOW};

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

/// The mark, `size` points across, in the text's colour. It never turns or moves: a thread at work
/// shows the ring.
pub fn mark(size: f32, palette: &Palette) -> Svg {
    svg()
        .path(mark_path(size))
        .size(px(size))
        .flex_none()
        .text_color(palette.text(INK))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use gpui::{AssetSource, SvgRenderer};

    use super::{LARGE, MARKS, MIDDLE, PRODUCT, SMALL, mark_path};
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
