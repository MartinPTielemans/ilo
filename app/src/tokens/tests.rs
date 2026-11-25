//! A colour, a corner radius or a type size written into `app/src` instead of `design/tokens.toml`
//! fails the tests, and so does a weight off the scale, so the design file can't miss a value the
//! UI draws with.

use std::{fs, path::Path};

/// The literals that stay in the code: the file, a piece of the line, and why.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "agents/wall/links.rs",
        "0x2FC1_2FC1",
        "ZFS's filesystem number, not a colour",
    ),
    (
        "agents/wall/links.rs",
        "0xCA45_1A4E",
        "bcachefs's filesystem number, not a colour",
    ),
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
        "preview.rs",
        "const MORE_COLORS",
        "the preview's made-up Space colours",
    ),
    (
        "shell/undo_tests.rs",
        "Some(0xff0000)",
        "a test's made-up Space colour",
    ),
    (
        "shell/tab_background_tests.rs",
        "const PAGES",
        "a test's made-up pages' background colours",
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
    for (file, text) in sources(&src, false) {
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
    // First run's window, as its drawing has it (values-first-run.md): the indicator's track, the
    // foot's buttons and their key, the keys step's caps, and the picture's panel.
    "shell.onboarding.window.track-radius",
    "shell.onboarding.window.button-radius",
    "shell.onboarding.window.key-radius",
    "shell.onboarding.window.key-cap-radius",
    "shell.onboarding.window.panel-radius",
    "icon-picker.arrow-tip-radius",
    "icon-picker.arrow-foot-radius",
    "theme-picker.arrow-tip-radius",
    "theme-picker.arrow-foot-radius",
    "site-controls.beak-tip-radius",
    "site-controls.beak-foot-radius",
    "shell.split-targets.half-glyph-radius",
    "shell.extensions.badge-radius",
    "shell.recent-downloads.preview-radius",
    // The page's own corner, as the redesign draws it: between a control's and a row's.
    "shell.card-radius",
    // The two curves that take the open tab's line into its page, which round no corner.
    "shell.join-radius",
    // The menu's glass, concentric round its rows as the rework's mocks draw it (#4090).
    "menu.radius",
    "menu.row-radius",
    "menu.quiet-radius",
    "menu.quiet-row-radius",
    // A drawing of the window, its corners and its page card's at the drawing's scale (#2407).
    "settings.side-radius",
    "settings.side-page-radius",
    // Settings in the owner's drawing (spec-settings.md): its pages' rows at 8, the page's plane
    // at 10 inside the window's 12, and the search field's ⌘F chip at 5.
    "settings.form.row-radius",
    "settings.form.panel-radius",
    "settings.form.chip-radius",
    // Share favorites' Spaces and their tiles at the drawing's scale (#2430).
    "settings.share-space-radius",
    "settings.share-tile-radius",
    "mini-bar.radius",
    // The Transparency picker's small ilo, at the drawing's scale (#4129).
    "transparency.picture-radius",
    "transparency.bright-radius",
    "transparency.window-radius",
    "transparency.row-radius",
    "transparency.page-radius",
    "transparency.menu-row-radius",
    // A card's recent rows, inside its picture's box (#3074).
    "hover-card.recent-row-radius",
    // First run's sheet (#4093): its slot inset in its corner, the sheet's less the inset.
    "shell.onboarding.sheet.slot-radius",
    // First run's picture, a small browser at the drawing's scale (values-first-run.md).
    "shell.onboarding.picture.window-radius",
    "shell.onboarding.picture.button-radius",
    "shell.onboarding.picture.address-radius",
    "shell.onboarding.picture.favorite-radius",
    "shell.onboarding.picture.favorite-mark-radius",
    "shell.onboarding.picture.mark-radius",
    "shell.onboarding.picture.open-tab-radius",
    "shell.onboarding.picture.page-radius",
    "shell.onboarding.picture.chip-radius",
    "shell.onboarding.picture.thread-radius",
    "shell.onboarding.picture.notice-radius",
    "shell.onboarding.picture.notice-tile-radius",
    "shell.onboarding.picture.bar-radius",
    "shell.onboarding.picture.bar-row-radius",
    "shell.onboarding.picture.bar-mark-radius",
    "shell.onboarding.picture.phone-radius",
    "shell.onboarding.picture.island-radius",
    "shell.onboarding.picture.phone-space-radius",
    // The composer's switch round its parts, and a card's page inside the card (#4140).
    "shell.composer.switch-radius",
    "shell.composer.page-radius",
    // The theme panel's Auto · Light · Dark: a segment concentric inside its frame, as drawn.
    "theme-panel.segment-frame-radius",
    "theme-panel.segment-radius",
    // A thread card's button, at the drawing's 8 (spec-thread-card).
    "shell.task-card.button-radius",
    // Settings' segmented track, concentric round its segments at the control radius.
    "settings.segment-radius",
    // The foot's plates in the owner's drawing (spec-library-foot-theme.md): 28 square at 8.
    "shell.foot-plate-radius",
    // The downloads plate in the same drawing: a row at 9 and its tile at 7.
    "shell.recent-downloads.row-radius",
    "shell.recent-downloads.tile-radius",
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
        command_bar, foot_bar, hover_card, menu, mini_bar, radius, shell, shell::extensions,
        site_controls, toast,
    };
    // outer = inner + inset, where the inset is a token (or two).
    assert_eq!(foot_bar::RADIUS, foot_bar::BUTTON_RADIUS + foot_bar::RIGHT);
    assert_eq!(
        foot_bar::TALL_RADIUS,
        foot_bar::BUTTON_RADIUS + foot_bar::TALL_RIGHT
    );
    assert_eq!(
        hover_card::RADIUS,
        hover_card::PICTURE_RADIUS + hover_card::PADDING
    );
    assert_eq!(
        hover_card::PICTURE_RADIUS,
        hover_card::RECENT_ROW_RADIUS + hover_card::PADDING
    );
    assert_eq!(mini_bar::RADIUS, mini_bar::BUTTON_RADIUS + mini_bar::INSET);
    assert_eq!(
        shell::composer::SWITCH_RADIUS,
        shell::composer::SWITCH_PART_RADIUS + shell::composer::SWITCH_INSET
    );
    assert_eq!(
        shell::composer::CARD_RADIUS,
        shell::composer::PAGE_RADIUS + shell::composer::PAGE_INSET
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
    // The toast's chips sit as far in from its top as its corner needs, to half a point.
    let inset = (toast::HEIGHT - toast::CHIP_HEIGHT) / 2.;
    assert!(
        (toast::RADIUS - toast::CHIP_RADIUS - inset).abs() < 0.5,
        "{inset}"
    );
    // ilo's menu: its panel is its rows' corner and its padding, quiet rows or not.
    assert_eq!(menu::RADIUS, menu::ROW_RADIUS + menu::PADDING);
    assert_eq!(menu::QUIET_RADIUS, menu::QUIET_ROW_RADIUS + menu::PADDING);
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
        ".line_height(px(18.))",
        "offset: point(px(0.), px(1.)),",
        "offset: point(px(-2.5), px(0.)),",
        "blur_radius: px(3.),",
        "spread_radius: px(0.5),",
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
        ".line_height(px(TITLE_LINE))",
        "offset: point(px(0.), px(0.)),",
        "offset: point(px(0.), px(y)),",
        "blur_radius: px(sigma(blur)),",
        "spread_radius: px(0.),",
        "spread_radius: px(KEYED_RING),",
    ] {
        assert_eq!(look_literal(code), None, "{code}");
    }
}

/// A test that asserts a constant is a literal, as WIDTH is 765, fails on every design
/// change and catches no behaviour (#2196). A relation between constants, or what a function
/// answers, is a test.
#[test]
fn no_test_restates_a_constant() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for (file, text) in sources(&src, true) {
        // Not the needle below, which is in quotes.
        for (at, _) in text
            .match_indices("assert_eq!(")
            .filter(|(at, _)| !text[..*at].ends_with('"'))
        {
            if let [a, b, ..] = &macro_args(&text[at + "assert_eq!(".len()..])[..]
                && restates(a, b)
            {
                let line = text[..at].matches('\n').count() + 1;
                found.push(format!("{file}:{line}: {a} == {b}"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these tests restate a constant; test what it does instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn finds_a_constant_against_a_literal_but_not_a_relation() {
    for (a, b) in [
        ("WIDTH", "765."),
        ("tooltip::DELAY", "Duration::from_millis(1600)"),
        ("FADE_IN", "Duration::ZERO"),
        ("CHIP + CHIP_GAP", "22."),
        ("\"Docs\"", "NAME"),
    ] {
        assert!(restates(a, b), "{a} == {b}");
    }
    for (a, b) in [
        ("menu::RADIUS", "radius::CONTROL + menu::PADDING"),
        ("height(5)", "328.5"),
        ("label(&tip)", "\"Docs\""),
        ("DELAY / 2", "half"),
        ("4.", "2. * 2."),
    ] {
        assert!(!restates(a, b), "{a} == {b}");
    }
}

/// Whether one side is a literal and the other constants and literals alone.
fn restates(a: &str, b: &str) -> bool {
    matches!((constant(a), constant(b)), (Some(x), Some(y)) if x != y)
}

/// Whether `side` names a constant, if it's made of constants, literals and arithmetic alone.
fn constant(side: &str) -> Option<bool> {
    let screaming = |w: &str| {
        w.len() > 1
            && w.starts_with(|c: char| c.is_ascii_uppercase())
            && w.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    };
    let (mut rest, mut names) = (side, false);
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || "()+-*/".contains(c));
        if rest.is_empty() {
            return Some(names);
        }
        if let Some(after) = rest.strip_prefix('"') {
            rest = &after[after.find('"')? + 1..];
            continue;
        }
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || "_:.".contains(c)))
            .unwrap_or(rest.len());
        let (word, after) = rest.split_at(end);
        rest = after;
        let mut path: Vec<&str> = word.split("::").collect();
        let name = path.pop()?;
        let literal = word.starts_with(|c: char| c.is_ascii_digit())
            || ["true", "false", "Duration::ZERO"].contains(&word)
            // Its argument comes next, as a word of its own.
            || (["px", "Duration::from_millis", "Duration::from_secs", "Duration::from_secs_f32"]
                .contains(&word)
                && rest.starts_with('('));
        if literal {
            continue;
        }
        if !screaming(name)
            || path
                .iter()
                .any(|m| m.is_empty() || m.contains(char::is_uppercase))
        {
            return None;
        }
        names = true;
    }
}

/// The arguments of the macro call whose text starts after its `(`, split at its top-level commas.
fn macro_args(text: &str) -> Vec<String> {
    let (mut args, mut arg, mut depth, mut chars) = (Vec::new(), String::new(), 0, text.chars());
    while let Some(c) = chars.next() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => break,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(std::mem::take(&mut arg).trim().to_string());
                continue;
            }
            '"' => {
                arg.push(c);
                while let Some(c) = chars.next() {
                    arg.push(c);
                    match c {
                        '\\' => arg.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
                continue;
            }
            _ => {}
        }
        arg.push(c);
    }
    args.push(arg.trim().to_string());
    args.retain(|a| !a.is_empty());
    args
}

/// Every Rust file under `dir`, by its path from there, with the test files or without.
fn sources(dir: &Path, tests: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(at) = dirs.pop() {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && (tests || !path.ends_with("tests.rs"))
            {
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

/// The first colour, radius, type size, line height, shadow or weight written as a number in
/// `code`: a hex colour (six or eight digits), `rgb` or `rgba` of a number, `hsla` of four numbers,
/// a corner radius, text size or line height of a number, a shadow's offset, blur or spread of a
/// number but nought, or a weight other than the scale's three.
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
        "line_height",
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
    // A shadow's offset, blur or spread of a number but nought, which is no look.
    let some = |arg: &str| {
        let arg = arg.trim().trim_start_matches("px(").trim_end_matches(')');
        number(arg) && arg.contains(|c: char| ('1'..='9').contains(&c))
    };
    for field in ["offset: point(", "blur_radius: px(", "spread_radius: px("] {
        for (at, _) in code.match_indices(field).filter(|(at, _)| starts(*at)) {
            let args = &code[at + field.len()..];
            let close = at
                + field.len()
                + match field {
                    "offset: point(" => args.find("))").map_or(args.len(), |end| end + 1),
                    _ => args.find(')').unwrap_or(args.len()),
                };
            if code[at + field.len()..close].split(',').any(some) {
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
            "shell::recent_downloads::DETAIL_SIZE",
            super::shell::recent_downloads::DETAIL_SIZE,
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
