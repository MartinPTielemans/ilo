//! A colour, a corner radius or a type size written into `app/src` instead of `design/tokens.toml`
//! fails the tests, and so does a weight off the scale, so the design file can't miss a value the
//! UI draws with.

use std::{fs, path::Path};

/// The literals that stay in the code: the file, a piece of the line, and why.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "theme.rs",
        "0x9e37_79b9",
        "the grain's random seed, not a colour",
    ),
    (
        "shell/new_space.rs",
        "hsla(0., 0., 1., 0.)",
        "the glow's white fading out, not a colour",
    ),
    (
        "preview.rs",
        "Some(0xbf556a)",
        "the preview's made-up Space colour",
    ),
    (
        "shell/dock_dot.rs",
        "0x8655BF",
        "a test's made-up Space colour",
    ),
    (
        "shell/dock_dot.rs",
        "0xE0475B",
        "a test's made-up Space colour",
    ),
    (
        "shell/undo_tests.rs",
        "Some(0xff0000)",
        "a test's made-up Space colour",
    ),
    (
        "shell/extensions.rs",
        "FontWeight::BOLD",
        "the badge's count, a pictogram inside its pill",
    ),
];

#[test]
fn look_values_come_from_the_token_file() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for (file, text) in sources(&src) {
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            // A file's tests come last, and their made-up colours aren't the look.
            let next = lines.get(n + 1);
            if line.starts_with("#[cfg(test)]")
                && next.is_some_and(|l| l.starts_with("mod tests {"))
            {
                break;
            }
            let code = match line.trim_start().starts_with("//") {
                true => "",
                false => line.split(" //").next().unwrap(),
            };
            let Some(literal) = look_literal(code) else {
                continue;
            };
            match ALLOWED
                .iter()
                .position(|(f, piece, _)| *f == file && line.contains(piece))
            {
                Some(i) => used[i] = true,
                None => found.push(format!("{file}:{}: {literal}", n + 1)),
            }
        }
    }
    let found = found.join("\n");
    assert!(
        found.is_empty(),
        "these belong in design/tokens.toml (or in ALLOWED, with why):\n{found}"
    );
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(a, _)| a)
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED lists literals that are gone: {stale:?}"
    );
}

/// The `radius` keys that aren't one of the four: the shapes of a beak and a glyph, which round a
/// point, not a corner, and what the rule derives, which the concentric test below checks.
const NOT_ONE_OF_FOUR: &[&str] = &[
    "icon-picker.arrow-tip-radius",
    "icon-picker.arrow-foot-radius",
    "theme-picker.arrow-tip-radius",
    "theme-picker.arrow-foot-radius",
    "site-controls.beak-tip-radius",
    "site-controls.beak-foot-radius",
    "shell.split-targets.half-glyph-radius",
    "shell.extensions.badge-radius",
    "shell.recent-downloads.preview-radius",
    "menu.radius",
];

#[test]
fn every_corner_is_one_of_the_four_radii() {
    use super::radius::{CONTROL, PANEL, ROW, SHEET};
    let toml = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("design/tokens.toml"))
        .unwrap();
    let (mut table, mut seen, mut wrong) = (String::new(), 0, Vec::new());
    for line in toml.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            table = name.to_string();
        } else if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if table == "radius" || !key.ends_with("radius") || line.starts_with('#') {
                continue;
            }
            let at = format!("{table}.{key}");
            seen += 1;
            let value: f32 = value.split('#').next().unwrap().trim().parse().unwrap();
            if ![CONTROL, ROW, PANEL, SHEET].contains(&value)
                && !NOT_ONE_OF_FOUR.contains(&at.as_str())
            {
                wrong.push(format!("{at} = {value}"));
            }
        }
    }
    assert!(seen > 30, "found only {seen} radius tokens");
    assert!(
        wrong.is_empty(),
        "not one of the four radii (or listed in NOT_ONE_OF_FOUR):\n{}",
        wrong.join("\n")
    );
    assert!(CONTROL < ROW && ROW < PANEL && PANEL < SHEET);
}

#[test]
fn a_corner_inside_another_is_concentric() {
    use super::{
        command_bar, hover_card, menu, radius, shell,
        shell::{extensions, recent_downloads},
        site_controls, toast,
    };
    // outer = inner + inset, where the inset is a token (or two).
    assert_eq!(
        hover_card::RADIUS,
        hover_card::PICTURE_RADIUS + hover_card::PADDING
    );
    assert_eq!(
        hover_card::RADIUS,
        hover_card::BUTTON_RADIUS + hover_card::BUTTON_INSET
    );
    assert_eq!(
        command_bar::RADIUS,
        command_bar::PLATE_RADIUS + command_bar::LIST_X + command_bar::PLATE_INSET
    );
    assert_eq!(command_bar::FIELD_RADIUS, command_bar::RADIUS);
    assert_eq!(
        site_controls::RADIUS,
        site_controls::TILE_RADIUS + site_controls::PADDING
    );
    assert_eq!(
        shell::REVEAL_RADIUS,
        shell::PLATE_RADIUS + shell::PLATE_INSET
    );
    assert_eq!(
        recent_downloads::PREVIEW_RADIUS,
        recent_downloads::PLATE_RADIUS
            - (recent_downloads::COLUMN_X - recent_downloads::PLATE_INSET)
    );
    // The toast's chips sit as far in from its top as its corner needs, to half a point.
    let inset = (toast::HEIGHT - toast::CHIP_HEIGHT) / 2.;
    assert!(
        (toast::RADIUS - toast::CHIP_RADIUS - inset).abs() < 0.5,
        "{inset}"
    );
    // ilo's menu: its rows are controls, and its panel is theirs and its padding.
    assert_eq!(menu::ROW_RADIUS, radius::CONTROL);
    assert_eq!(menu::RADIUS, radius::CONTROL + menu::PADDING);
    // A pill's corners are half its height.
    assert_eq!(extensions::BADGE_RADIUS, extensions::BADGE_HEIGHT / 2.);
}

#[test]
fn finds_colours_and_radii_but_not_tokens() {
    for code in [
        ".bg(rgba(0xffff_ff33))",
        "const FILL: u32 = 0x161b2f;",
        "color: hsla(0., 0., 0., 0.56),",
        ".text_color(rgba(0))",
        ".rounded(px(6.))",
        "corner_radii: Corners::all(px(3.5)),",
        ".text_size(px(14.))",
        ".font_weight(FontWeight::BOLD)",
        "FontWeight(700.)",
    ] {
        assert!(look_literal(code).is_some(), "{code}");
    }
    for code in [
        ".bg(rgba(SELECTED))",
        "hsla(0., 0., 1., TILE_HOVERED)",
        ".rounded(px(PLATE_RADIUS))",
        ".rounded_full()",
        ".text_size(px(text::BODY))",
        ".font_weight(FontWeight::SEMIBOLD)",
        "let flags = 0x0100;",
        "let seed = 0x9e37;",
    ] {
        assert_eq!(look_literal(code), None, "{code}");
    }
}

/// Every Rust file under `dir` but the test files, by its path from there.
fn sources(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(at) = dirs.pop() {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("tests.rs") {
                let file = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((file, fs::read_to_string(&path).unwrap()));
            }
        }
    }
    out
}

/// The first colour, radius, type size or weight written as a number in `code`: a hex colour (six
/// or eight digits), `rgb` or `rgba` of a number, `hsla` of four numbers, a corner radius or text
/// size of a number, or a weight other than the scale's three.
fn look_literal(code: &str) -> Option<&str> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let starts = |at: usize| !code[..at].ends_with(word);
    for (at, _) in code
        .match_indices("FontWeight")
        .filter(|(at, _)| starts(*at))
    {
        let end = at
            + code[at..]
                .find(|c: char| !word(c) && c != ':')
                .unwrap_or(code.len() - at);
        let named = [
            "FontWeight::NORMAL",
            "FontWeight::MEDIUM",
            "FontWeight::SEMIBOLD",
        ];
        // The type itself, as in `weight: FontWeight`, names no weight.
        let bare = code[at..end] == *"FontWeight" && !code[end..].starts_with('(');
        if !bare && !named.contains(&&code[at..end]) {
            return Some(&code[at..(end + 1).min(code.len())]);
        }
    }
    for (at, _) in code.match_indices("0x").filter(|(at, _)| starts(*at)) {
        let end = at
            + 2
            + code[at + 2..]
                .find(|c: char| !word(c))
                .unwrap_or(code.len() - at - 2);
        let count = code[at + 2..end]
            .chars()
            .filter(char::is_ascii_hexdigit)
            .count();
        if count == 6 || count == 8 {
            return Some(&code[at..end]);
        }
    }
    let number = |arg: &str| {
        let arg = arg.trim().trim_start_matches('-');
        arg.starts_with(|c: char| c.is_ascii_digit())
            && arg
                .chars()
                .all(|c| c.is_ascii_hexdigit() || "._x".contains(c))
    };
    for call in [
        "rgb",
        "rgba",
        "hsla",
        "rounded",
        "corner_radii",
        "Corners::all",
        "text_size",
    ] {
        for (at, _) in code.match_indices(call).filter(|(at, _)| starts(*at)) {
            // `rounded` names a family: `rounded_tl` and the rest.
            let name = at + call.len();
            let open = match call {
                "rounded" => name + code[name..].find(|c: char| !word(c)).unwrap_or(0),
                _ => name,
            };
            if !code[open..].starts_with('(') {
                continue;
            }
            let close = open + code[open..].find(')').unwrap_or(code.len() - open);
            let args = code[open + 1..close].trim_start_matches("px(");
            let literal = match call {
                "hsla" => args.split(',').count() == 4 && args.split(',').all(number),
                _ => args.split(',').next().is_some_and(number),
            };
            if literal {
                return Some(&code[at..(close + 1).min(code.len())]);
            }
        }
    }
    None
}

/// The weights the code names are the scale's, and every surface's own type size is one of its four
/// sizes, the badge's aside.
#[test]
fn type_is_on_the_scale() {
    use super::text;
    use gpui::FontWeight;
    let weights = [FontWeight::NORMAL, FontWeight::MEDIUM, FontWeight::SEMIBOLD].map(|w| w.0);
    assert_eq!(weights, [text::REGULAR, text::MEDIUM, text::SEMIBOLD]);
    let scale = [text::SMALL, text::BODY, text::TITLE, text::HEADING];
    for (name, size) in [
        ("capture::PILL_TEXT_SIZE", super::capture::PILL_TEXT_SIZE),
        ("command_bar::INPUT_TEXT", super::command_bar::INPUT_TEXT),
        (
            "command_bar::FIELD_INPUT_TEXT",
            super::command_bar::FIELD_INPUT_TEXT,
        ),
        (
            "icon_picker::emoji::HEADER_TEXT",
            super::icon_picker::emoji::HEADER_TEXT,
        ),
        ("link_target::TEXT_SIZE", super::link_target::TEXT_SIZE),
        ("shell::empty::WORDS_SIZE", super::shell::empty::WORDS_SIZE),
        ("shell::empty::KEYS_SIZE", super::shell::empty::KEYS_SIZE),
        (
            "shell::recent_downloads::NAME_SIZE",
            super::shell::recent_downloads::NAME_SIZE,
        ),
        (
            "shell::recent_downloads::AGE_SIZE",
            super::shell::recent_downloads::AGE_SIZE,
        ),
        (
            "shell::side_panel::HEADER_TEXT",
            super::shell::side_panel::HEADER_TEXT,
        ),
        (
            "shell::trouble::WORDS_SIZE",
            super::shell::trouble::WORDS_SIZE,
        ),
        (
            "site_controls::HEADING_SIZE",
            super::site_controls::HEADING_SIZE,
        ),
        ("site_controls::LINK_SIZE", super::site_controls::LINK_SIZE),
        ("site_controls::NAME_SIZE", super::site_controls::NAME_SIZE),
        (
            "site_controls::STATE_SIZE",
            super::site_controls::STATE_SIZE,
        ),
        ("toast::TEXT_SIZE", super::toast::TEXT_SIZE),
        ("tooltip::TEXT_SIZE", super::tooltip::TEXT_SIZE),
        ("tooltip::CHIP_TEXT_SIZE", super::tooltip::CHIP_TEXT_SIZE),
        ("zoom_hud::TEXT_SIZE", super::zoom_hud::TEXT_SIZE),
    ] {
        assert!(
            scale.contains(&size),
            "{name} is {size}, off the scale {scale:?}"
        );
    }
}
