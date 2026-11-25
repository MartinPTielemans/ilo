use super::*;
use crate::space_transparency::{START, of_level};
use crate::theme::Appearance;
use crate::tokens::palette::{LIGHT_TINT, TINT};
use crate::tokens::space_transparency::{HOVERED_TITLE, HOVERED_TITLE_PAST, LESS};

/// A theme as the design sweep (#320) names one: `count` colours, the main one at `hue` degrees,
/// HSL `saturation` and `lightness` (its distance from the field's centre).
fn theme(
    count: usize,
    grain: f32,
    appearance: &str,
    hue: f32,
    saturation: f32,
    lightness: f32,
    intensity: f32,
) -> Theme {
    let angle = hue.to_radians();
    let at = Theme::at(lightness * angle.cos(), lightness * angle.sin());
    let appearance = *Appearance::ALL
        .iter()
        .find(|a| a.name() == appearance)
        .unwrap();
    Theme {
        saturation,
        count,
        intensity,
        grain,
        appearance,
        ..at
    }
}

fn hex(color: Rgba) -> u32 {
    theme::packed(color)
}

/// The window, finer than the palette checks it: the foot's glow at 17 strengths and the far one's
/// at 5, each with each wherever the window's size makes them meet, over a black and a white
/// backdrop.
fn fine_window(palette: &Palette) -> Vec<Rgba> {
    let steps = 16;
    (0..=steps)
        .flat_map(|foot| (0..=4).map(move |far| (foot as f32 / steps as f32, far as f32 / 4.)))
        .flat_map(|(foot, far)| palette.window_at(foot, far))
        .collect()
}

/// What fails: each of the chrome's components drawn as the shell draws it, with its alpha
/// from the token ladder, on every point of a window of `tone`, below its WCAG minimum, gray
/// text below the user's level's.
fn failures(theme: Option<&Theme>, tone: Tone) -> Vec<String> {
    let palette = Palette::of(theme, tone);
    let window = fine_window(&palette);
    failures_over(theme, &palette, &window, gray_text(transparency()))
}

/// What fails of `palette`'s chrome drawn on each point of `window`, gray text held to `gray`.
fn failures_over(
    theme: Option<&Theme>,
    palette: &Palette,
    window: &[Rgba],
    gray: f32,
) -> Vec<String> {
    let tone = palette.tone;
    let grain = theme.map_or(0., |theme| theme.grain);
    let ink = palette.ink;
    let grained: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, ink, GRAIN_SHIFT * grain))
        .collect();
    let plated =
        |plate: f32| -> Vec<Rgba> { window.iter().map(|&at| over(at, ink, plate)).collect() };
    // A plate as the palette draws it, shaded on its ground.
    let laid = |plate: Hsla| -> Vec<Rgba> {
        let plate = Rgba::from(plate);
        window.iter().map(|&at| over(at, plate, plate.a)).collect()
    };
    let mut failed = Vec::new();
    let mut check = |name: &str, worst: f32, minimum: f32| {
        if worst < minimum - 1e-4 {
            failed.push(format!("{name} {worst:.2} < {minimum}"));
        }
    };
    let on = |color: Hsla, grounds: &[Rgba]| {
        let color = Rgba::from(color);
        grounds
            .iter()
            .map(|&at| contrast(over(at, color, color.a), at))
            .fold(f32::MAX, f32::min)
    };
    // The sidebar's text and glyphs (#320 rule 5's ladder).
    let selected = Rgba::from(palette.selected(0.));
    let selected: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, selected, selected.a))
        .collect();
    check("selected title", on(palette.text(1.), &selected), TEXT);
    // The tab shown's row, in the Space's colour, at rest and under the pointer (#4091).
    for (name, hover) in [("shown title", 0.), ("hovered shown title", 1.)] {
        let shown = Rgba::from(palette.shown(hover));
        let shown: Vec<Rgba> = window.iter().map(|&at| over(at, shown, shown.a)).collect();
        check(name, on(palette.text(1.), &shown), TEXT);
    }
    check("unselected title", on(palette.text(0.78), &grained), TEXT);
    // A dark window's plate under the pointer tells from the window first, and its title reads
    // what that leaves: 4.5, past the bar's last stretch, where today's More sits, 4.2, and on a
    // Space's colour, fitted to 4.5 on the bare window, 3.6
    // (`a_plate_under_the_pointer_tells_and_its_title_reads_at_every_level`).
    let under_the_pointer = match (tone, theme) {
        (Tone::Light, _) => TEXT,
        (Tone::Dark, Some(_)) => 3.6,
        (Tone::Dark, None) if palette.glass > HOVERED_TITLE_PAST => HOVERED_TITLE,
        (Tone::Dark, None) => TEXT,
    };
    check(
        "title on hovered plate",
        on(palette.text(0.78), &laid(palette.hovered_row())),
        under_the_pointer,
    );
    check(
        "URL host on field",
        on(
            palette.text(0.62),
            &laid(palette.ground_plate(Ground::Field, FIELD_PLATE)),
        ),
        TEXT,
    );
    for name in ["Space name", "New Tab", "Clear", "secondary"] {
        check(name, on(palette.text(0.62), &grained), TEXT);
    }
    check("nav glyph", on(palette.glyph(0.6), &grained), GLYPH);
    check("New Tab +", on(palette.glyph(0.45), &grained), GLYPH);
    check("Space dot", on(palette.glyph(0.6), &grained), GLYPH);
    check("other Space dot", on(palette.glyph(0.35), &grained), GLYPH);
    // The accent, wherever it's drawn, and its label.
    let accent = palette.accent;
    let against = |grounds: &[Rgba]| {
        grounds
            .iter()
            .map(|&at| contrast(accent, at))
            .fold(f32::MAX, f32::min)
    };
    check("focus ring, New Space circle", against(window), GLYPH);
    check(
        "label on accent",
        contrast(palette.accent_label, accent),
        TEXT,
    );
    check(
        "selected row vs command bar",
        contrast(accent, palette.overlay),
        GLYPH,
    );
    check(
        "dialog action vs dialog",
        contrast(accent, palette.dialog),
        GLYPH,
    );
    check(
        "Library section glyph",
        against(&plated(SECTION_PLATE)),
        GLYPH,
    );
    // The selected section's glyph, on the Library's column under Arc's selected plate (#371).
    use crate::tokens::shell::library::{COLUMN, LIBRARY_PLATE};
    let selected: Vec<Rgba> = window
        .iter()
        .map(|&at| over(over(at, ink, COLUMN), ink, LIBRARY_PLATE))
        .collect();
    let glyph = palette.section_glyph;
    check(
        "selected Library section glyph",
        selected
            .iter()
            .map(|&at| contrast(glyph, at))
            .fold(f32::MAX, f32::min),
        GLYPH,
    );
    let targets: Vec<Rgba> = plated(TARGET_PLATE)
        .iter()
        .map(|&at| over(at, accent, TARGET_ACCENT))
        .collect();
    check("split target dashes", against(&targets), GLYPH);
    // What floats over the page.
    check("overlay text", contrast(ink, palette.overlay), TEXT);
    check("dialog text", contrast(ink, palette.dialog), TEXT);
    // The pickers' card over the window, and the text on it.
    let cards: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, palette.card, palette.card.a))
        .collect();
    let white = Hsla::from(WHITE);
    check(
        "picker hint",
        on(white.opacity(rgba(CARD_HINT).a), &cards),
        TEXT,
    );
    let segments: Vec<Rgba> = cards
        .iter()
        .map(|&at| over(at, WHITE, rgba(CARD_SEGMENTS).a))
        .collect();
    let label = segments
        .iter()
        .map(|&at| contrast(rgb(CARD_LABEL), at))
        .fold(f32::MAX, f32::min);
    check("picker segment label", label, TEXT);
    // A Library card is its Space's window, drawn in its palette, split segments too.
    let card: Vec<Rgba> = (0..=16)
        .flat_map(|foot| palette.window_at(foot as f32 / 16., 0.))
        .collect();
    check(
        "card split segment title",
        on(palette.text(0.78), &card),
        TEXT,
    );
    // The theme picker's panel is the pickers' card over the window, with plates of the dark
    // palette's ink on it, as the picker is dark on either window; and the site controls the
    // overlay, with plates of the ink.
    let overlay = palette.overlay;
    let on_overlay = |plates: &[f32]| -> Vec<Rgba> {
        plates
            .iter()
            .map(|&plate| over(overlay, ink, plate))
            .collect()
    };
    let picker = Palette::of(theme, Tone::Dark);
    let on_card = |plates: &[f32]| -> Vec<Rgba> {
        cards
            .iter()
            .flat_map(|&at| plates.iter().map(move |&plate| over(at, picker.ink, plate)))
            .collect()
    };
    {
        use crate::tokens::theme_picker::*;
        let chosen = on_card(&[APPEARANCE_CHOSEN]);
        check(
            "picker appearance",
            on(picker.on_overlay_glyph(APPEARANCE_COLOR), &chosen),
            GLYPH,
        );
        // − and + keep Arc's white 0.35 beside the count they change, under 3:1 on the card.
        let field = on_card(&[FIELD_FILL]);
        check(
            "picker handle ring",
            on(Hsla::from(picker.ink), &field),
            GLYPH,
        );
        let track = on_card(&[TRACK_FILL]);
        check("picker wave", on(picker.plate(WAVE_ON), &track), GLYPH);
        check("picker knob", on(Hsla::from(picker.ink), &track), GLYPH);
        check(
            "picker swept dot",
            on(picker.plate(DIAL_SWEPT), &cards),
            GLYPH,
        );
    }
    {
        use crate::tokens::site_controls::*;
        for (name, alpha) in [("text", TEXT), ("state", SECONDARY), ("link", LINK)] {
            check(
                &format!("site controls {name}"),
                on(palette.on_overlay(alpha), &[overlay]),
                super::TEXT,
            );
        }
        check(
            "site controls off glyph",
            on(palette.on_overlay_glyph(ITEM_GLYPH_OFF), &[overlay]),
            GLYPH,
        );
        check("site controls on line glyph", against(&[overlay]), GLYPH);
        // A switch or the shield while on: the accent's glyph on the tile tinted with it, at rest,
        // as the action tiles' glyphs are checked on their resting plate.
        let tinted: Vec<Rgba> = on_overlay(&[TILE_TOP, TILE_BOTTOM])
            .iter()
            .map(|&at| over(at, accent, crate::tokens::site_controls::TINT))
            .collect();
        check("site controls on switch glyph", against(&tinted), GLYPH);
    }
    // Every surface's controls in each of the ladder's states over their resting fills (#318):
    // the text on them at 4.5:1 and the glyphs at 3:1.
    for (name, content, minimum, on_states) in states(palette, window, gray) {
        check(
            &format!("{name}, every state"),
            on(content, &on_states),
            minimum,
        );
    }
    // The Space's swatch, ringed where its edge doesn't reach 3:1 on the window.
    let swatch = match theme {
        Some(theme) => Window::of(theme, tone, palette.glass).swatch(),
        None => vec![rgb(NO_THEME_ACCENT)],
    };
    let edge = match Palette::swatch_ring_on(theme, window) {
        Some(ring) => window
            .iter()
            .map(|&at| contrast(ring, at))
            .fold(f32::MAX, f32::min),
        None => {
            let each = swatch
                .iter()
                .flat_map(|&c| window.iter().map(move |&at| contrast(c, at)));
            each.fold(f32::MAX, f32::min)
        }
    };
    check("Space swatch edge", edge, GLYPH);
    failed
}

/// Each control drawn on a state of the ladder, off the sidebar, whose measured states the ladder
/// is: its name, what's drawn on it, the contrast that needs, and the grounds it's drawn on, its
/// resting fill under each state it takes, over each point of `window`. Gray text needs `gray`.
fn states(palette: &Palette, window: &[Rgba], gray: f32) -> Vec<(String, Hsla, f32, Vec<Rgba>)> {
    use crate::tokens::{command_bar, dialog, offer, shell, site_controls, toast};
    const PRESSABLE: [f32; 2] = [STATE_PRESSED, STATE_HOVERED];
    let ink = palette.ink;
    // A control resting at `rest`, a plate of the ink, on `grounds`, in each of `states`.
    let lay = |grounds: &[Rgba], rest: f32, states: &[f32]| -> Vec<Rgba> {
        let alphas: Vec<f32> = states
            .iter()
            .map(|&state| state_alpha(rest, state))
            .collect();
        grounds
            .iter()
            .flat_map(|&at| alphas.iter().map(move |&alpha| over(at, ink, alpha)))
            .collect()
    };
    // The same on `ground`, shaded as the palette draws it there.
    let lay_on = |ground: Ground, grounds: &[Rgba], rest: f32, states: &[f32]| -> Vec<Rgba> {
        let plates: Vec<Rgba> = states
            .iter()
            .map(|&state| Rgba::from(palette.ground_state(ground, rest, state)))
            .collect();
        grounds
            .iter()
            .flat_map(|&at| plates.iter().map(move |&plate| over(at, plate, plate.a)))
            .collect()
    };
    // The same on a surface that's dark on any theme, in Paper.
    let paper = rgb(PAPER);
    let lay_dark = |fill: Rgba, rest: f32, states: &[f32]| -> Vec<Rgba> {
        states
            .iter()
            .map(|&state| over(fill, paper, state_alpha(rest, state)))
            .collect()
    };
    let white = |alpha: f32| Hsla::from(WHITE).opacity(alpha);
    let column: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, ink, shell::library::COLUMN))
        .collect();
    let (overlay, dialog) = (vec![palette.overlay], vec![palette.dialog]);
    let sections = [
        shell::library::SECTION_HOVERED,
        shell::library::LIBRARY_PLATE,
    ];
    let mut states = vec![
        // On the window.
        // A section's plates are Arc's selected one and ours under it.
        (
            "Library section label",
            palette.gray(shell::library::SECTION_LABEL),
            gray,
            lay(&column, 0., &sections),
        ),
        (
            "Library section glyph",
            palette.glyph(shell::library::SECTION_GLYPH_UNSELECTED),
            GLYPH,
            lay(&column, 0., &sections),
        ),
        (
            "Library row title",
            palette.text(shell::UNSELECTED_TITLE),
            TEXT,
            lay_on(Ground::Library, window, 0., &PRESSABLE),
        ),
        (
            "Library row detail",
            palette.gray(shell::SECONDARY),
            gray,
            lay_on(Ground::Library, window, 0., &PRESSABLE),
        ),
        (
            "Library card name",
            palette.text(shell::UNSELECTED_TITLE),
            TEXT,
            lay_on(Ground::Library, window, 0., &[STATE_HOVERED]),
        ),
        (
            "Library and side panel buttons",
            palette.glyph(shell::NAV),
            GLYPH,
            lay(window, 0., &PRESSABLE),
        ),
        (
            "URL field host",
            palette.text(shell::SECONDARY),
            TEXT,
            lay_on(Ground::Field, window, FIELD_PLATE, &[STATE_HOVERED]),
        ),
        (
            "URL field placeholder",
            palette.gray(shell::header::URL_PLACEHOLDER),
            gray,
            lay_on(Ground::Field, window, FIELD_PLATE, &[STATE_HOVERED]),
        ),
        (
            "question card button",
            palette.text(shell::TITLE),
            TEXT,
            lay_on(Ground::Field, window, FIELD_PLATE, &PRESSABLE),
        ),
        (
            "offer button",
            palette.text(offer::TITLE),
            TEXT,
            lay_on(Ground::Offer, window, offer::PLATE, &PRESSABLE),
        ),
        (
            "offer chevron",
            palette.glyph(offer::DETAIL),
            GLYPH,
            lay(window, 0., &PRESSABLE),
        ),
        (
            "toast chip key",
            palette.text(toast::KEY_COLOR),
            TEXT,
            lay_on(Ground::Chip, window, toast::CHIP_FILL, &PRESSABLE),
        ),
        (
            "page chip key",
            palette.text(shell::conversation::CHIP_KEY),
            TEXT,
            lay_on(Ground::Chip, window, toast::CHIP_FILL, &PRESSABLE),
        ),
        // On the overlays.
        (
            "command bar row",
            palette.on_overlay(command_bar::DIM),
            TEXT,
            lay(&overlay, 0., &[command_bar::HOVERED]),
        ),
        (
            "command bar row glyph",
            palette.on_overlay_glyph(command_bar::ROW_GLYPH),
            GLYPH,
            lay(&overlay, 0., &[command_bar::HOVERED]),
        ),
        (
            "dialog Cancel",
            palette.on_overlay(dialog::CANCEL_TEXT),
            TEXT,
            lay(
                &dialog,
                0.,
                &[
                    dialog::CANCEL_HOVERED,
                    state_alpha(dialog::CANCEL_FILL, STATE_PRESSED),
                ],
            ),
        ),
        (
            "site controls tile",
            palette.on_overlay_glyph(site_controls::TEXT),
            GLYPH,
            lay(&overlay, site_controls::TILE_TOP, &PRESSABLE),
        ),
        (
            "site controls lock",
            palette.on_overlay(site_controls::TEXT),
            TEXT,
            lay(&overlay, site_controls::PLATE, &PRESSABLE),
        ),
        (
            "site controls …",
            palette.on_overlay_glyph(site_controls::TEXT),
            GLYPH,
            lay(&overlay, site_controls::MORE_FILL, &PRESSABLE),
        ),
        (
            "theme picker appearance",
            palette.on_overlay_glyph(crate::tokens::theme_picker::APPEARANCE_COLOR),
            GLYPH,
            lay(
                &overlay,
                0.,
                &[crate::tokens::theme_picker::APPEARANCE_CHOSEN],
            ),
        ),
    ];
    // Dark on any theme: the find bar, and the icon picker, on the pickers' card over the window,
    // its segments on their track.
    use crate::tokens::{find_bar, icon_picker};
    let cards: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, palette.card, palette.card.a))
        .collect();
    let track: Vec<Rgba> = cards
        .iter()
        .map(|&at| over(at, WHITE, rgba(CARD_SEGMENTS).a))
        .collect();
    let label = Hsla::from(rgb(icon_picker::LABEL));
    let chosen = Rgba::from(gpui::rgba(icon_picker::CHOSEN)).a;
    states.extend([
        (
            "find bar button",
            white(Rgba::from(gpui::rgba(find_bar::GLYPH_COLOR)).a),
            GLYPH,
            lay_dark(rgb(find_bar::FILL), 0., &PRESSABLE),
        ),
        (
            "icon picker segment",
            label,
            TEXT,
            track
                .iter()
                .flat_map(|&at| PRESSABLE.map(|state| over(at, rgb(INK), state)))
                .collect(),
        ),
        (
            "icon picker glyph",
            label,
            GLYPH,
            cards
                .iter()
                .flat_map(|&at| lay_dark(at, 0., &[STATE_PRESSED, STATE_HOVERED, chosen]))
                .collect(),
        ),
    ]);
    states
        .into_iter()
        .map(|(name, content, minimum, grounds)| (name.to_string(), content, minimum, grounds))
        .collect()
}

/// The themes of a grid over the picker's output: colours, field, intensity, grain, appearance.
fn grid(hues: &[f32], saturations: &[f32], lightnesses: &[f32], intensities: &[f32]) -> Vec<Theme> {
    let mut themes = Vec::new();
    for count in 1..=3 {
        for &grain in &[0., 1.] {
            for appearance in Appearance::ALL {
                for &hue in hues {
                    for &saturation in saturations {
                        for &lightness in lightnesses {
                            for &intensity in intensities {
                                let theme = theme(
                                    count,
                                    grain,
                                    appearance.name(),
                                    hue,
                                    saturation,
                                    lightness,
                                    intensity,
                                );
                                themes.push(theme);
                            }
                        }
                    }
                }
            }
        }
    }
    themes
}

/// What fails for each of `themes` on a dark and on a light window.
fn sweep(themes: &[Theme]) -> Vec<String> {
    themes
        .iter()
        .flat_map(|theme| [Tone::Dark, Tone::Light].map(|tone| (theme, tone)))
        .filter_map(|(theme, tone)| {
            let failed = failures(Some(theme), tone);
            (!failed.is_empty())
                .then(|| format!("{} {tone:?}: {}", theme.to_text(), failed.join(", ")))
        })
        .collect()
}

/// A Space with no theme has the measured window, black at `tint` over the material and nothing
/// more, and ilo green made to read with Ink on it, and everything passes, at every transparency
/// level: primary text at 4.5:1 and gray text at the level's (#4090).
#[test]
fn no_theme_is_the_measured_window_and_ilo_green_with_ink() {
    let palette = Palette::of(None, Tone::Dark);
    assert_eq!((palette.tint.a, palette.layer_alpha), (TINT, 0.));
    // #289054 reads 2.7:1 on the window over a white backdrop, so ilo green, lighter, at its hue.
    assert_eq!(hex(palette.accent), 0x78d999);
    assert_eq!(hex(palette.accent_label), INK);
    assert!(contrast(palette.accent_label, palette.accent) >= TEXT);
    // #0B8045 with Paper, the value #319 first had, is under 4.5.
    assert!(contrast(rgb(PAPER), rgb(0x0b8045)) < TEXT);
    for level in Transparency::ALL {
        set_transparency(level);
        let palette = Palette::of(None, Tone::Dark);
        assert_eq!(
            (palette.tint.a, palette.layer_alpha),
            (level.tint(true), 0.),
            "{level:?}: the window is never shaded"
        );
        assert_eq!(
            failures(None, Tone::Dark),
            Vec::<String>::new(),
            "{level:?}"
        );
    }
}

/// A light window is its material under white 0.8 with Ink on it, a theme's colours over it at
/// Dia's light layer's alpha, and ilo green darker so it reads there; everything passes.
#[test]
fn a_light_window_is_white_over_the_material_with_ink() {
    let palette = Palette::of(None, Tone::Light);
    assert_eq!(
        (palette.tint, palette.layer_alpha),
        (
            Rgba {
                a: LIGHT_TINT,
                ..WHITE
            },
            0.
        )
    );
    assert_eq!(hex(palette.ink), INK);
    assert!(luminance(palette.accent) < luminance(Palette::of(None, Tone::Dark).accent));
    for level in Transparency::ALL {
        set_transparency(level);
        assert_eq!(
            failures(None, Tone::Light),
            Vec::<String>::new(),
            "{level:?}"
        );
    }
    set_transparency(Transparency::Standard);
    let violet = Palette::of(Some(&Theme::of_color(0x5a2a9a)), Tone::Light);
    assert_eq!(
        (violet.tint, violet.layer_alpha),
        (palette.tint, LIGHT_GRADIENT_ALPHA)
    );
}

/// A theme lies over the window as a Space's colour did before themes, on the glass the window
/// has without one, never shaded (#4090), and only a theme the ink can't read on lies fainter, by
/// the least step that lets it read.
#[test]
fn a_theme_only_fades_where_the_ink_needs_it() {
    // check/window.sh's black Space: neutral, its own alpha, on the window without a theme.
    let none = Palette::of(None, Tone::Dark);
    let black = Palette::of(Some(&Theme::at(0., 0.)), Tone::Dark);
    assert_eq!(
        (black.tint, black.underlay, black.layer_alpha),
        (none.tint, none.underlay, NEUTRAL_GRADIENT_ALPHA)
    );
    let violet = Palette::of(Some(&Theme::of_color(0x5a2a9a)), Tone::Dark);
    assert_eq!(
        (violet.tint, violet.layer_alpha),
        (none.tint, GRADIENT_ALPHA)
    );
    // The picker's palest swatch, too light for Paper at its own alpha over white.
    let pale = Theme::of_color(0xf0eae5);
    let palette = Palette::of(Some(&pale), Tone::Dark);
    let window = Window::of(&pale, Tone::Dark, START);
    assert_eq!(palette.tint, none.tint, "the pale theme shades the window");
    assert!(
        palette.layer_alpha < window.alpha,
        "the pale theme isn't faded"
    );
    let paper = rgb(PAPER);
    let fails = window
        .samples(palette.layer_alpha + window.alpha * SHADE_STEP)
        .iter()
        .any(|&at| {
            contrast(paper, over(at, paper, GRAIN_SHIFT * pale.grain)) < TEXT + BETWEEN
                || contrast(Tone::Dark.end(), at) < GLYPH + BETWEEN
        });
    assert!(fails, "a step less fade would have read");
}

/// A Space's colour changes only its glows: away from them, its window is the window without a
/// theme, the same tint over the same material, in both looks and at every transparency level, so
/// it's as see-through as the user's level says (#4090).
#[test]
fn a_themed_window_has_the_glass_of_one_without() {
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0., 0.4, 1.],
        &[0., 0.2, 0.45, 0.6, 0.8, 1.],
        &[0., crate::theme::DEFAULT_INTENSITY, 1.],
    );
    for level in Transparency::ALL {
        set_transparency(level);
        for tone in [Tone::Dark, Tone::Light] {
            let none = Palette::of(None, tone);
            for theme in &themes {
                let palette = Palette::of(Some(theme), tone);
                assert_eq!(
                    (palette.tint, palette.underlay),
                    (none.tint, none.underlay),
                    "{level:?} {tone:?} {}",
                    theme.to_text()
                );
                assert_eq!(
                    palette.window_at(0., 0.),
                    none.window_at(0., 0.),
                    "{level:?} {tone:?} {}",
                    theme.to_text()
                );
            }
        }
    }
    set_transparency(Transparency::Standard);
}

/// The pickers' card takes the least step from Arc's measured grey toward the dark overlay at
/// which its text reads, four on the HUD window's material, over the black Space as with no theme,
/// as a theme leaves the window's glass as it is (#4090), and Capture's menu, ilo's menu's panel, reads in both looks: its glyphs, in the
/// secondary colour, at 3:1, and its field's words, in the label's, as text on the field's tint of
/// the line colour.
#[test]
fn the_pickers_card_and_captures_menu_read() {
    let black = Palette::of(Some(&Theme::at(0., 0.)), Tone::Dark);
    assert_eq!(
        (hex(black.card), black.card.a),
        (0x403f3e, 0.65176475),
        "four steps from the measured grey over the black Space"
    );
    let none = Palette::of(None, Tone::Dark).card;
    assert_eq!(
        (hex(none), none.a),
        (0x403f3e, 0.65176475),
        "four steps from the measured grey with no theme"
    );
    let lay = |at: Rgba, colour: u32| over(at, rgb(colour >> 8), (colour & 0xff) as f32 / 255.);
    for light in [false, true] {
        let menu = crate::menu::Look {
            light,
            ..crate::menu::Look::dark()
        }
        .palette();
        let panel = rgb(menu.fill >> 8);
        let field = lay(panel, menu.line);
        assert!(
            contrast(lay(panel, menu.secondary), panel) >= GLYPH,
            "glyphs, light {light}"
        );
        assert!(
            contrast(lay(field, menu.label), field) >= TEXT,
            "Send To…, light {light}"
        );
    }
}

/// The selected Library section's glyph is the accent where the accent reads on the Library's
/// column under the section's plate, and otherwise, alone, the nearest colour at its hue that
/// reads, the accent staying. With no theme on the HUD window's material, ilo green, made to
/// read with Ink, reads there as it is.
#[test]
fn the_selected_library_section_glyph_reads_without_moving_the_accent() {
    use crate::tokens::shell::library::{COLUMN, LIBRARY_PLATE};
    let none = Palette::of(None, Tone::Dark);
    let selected: Vec<Rgba> = (none.window_samples(None).iter())
        .map(|&at| over(over(at, none.ink, COLUMN), none.ink, LIBRARY_PLATE))
        .collect();
    let worst = |c: Rgba| {
        selected
            .iter()
            .map(|&at| contrast(c, at))
            .fold(f32::MAX, f32::min)
    };
    assert_eq!(hex(none.accent), 0x78d999);
    assert!(
        worst(none.accent) >= GLYPH + BETWEEN,
        "ilo green doesn't read there: {:.2}",
        worst(none.accent)
    );
    assert_eq!(none.section_glyph, none.accent);
    assert!(worst(none.section_glyph) >= GLYPH + BETWEEN);
    let (hue, glyph_hue) = (Hsla::from(none.accent).h, Hsla::from(none.section_glyph).h);
    assert!(
        (hue - glyph_hue).abs() < 0.02,
        "the glyph keeps ilo green's hue"
    );
    let violet = Palette::of(Some(&Theme::of_color(0x5a2a9a)), Tone::Dark);
    assert_eq!(violet.section_glyph, violet.accent);
}

/// The switcher's icon for the Space shown is in its accent, which reads at 3:1 or more on the
/// footer for every theme, light and dark, over a black and a white backdrop, its grain moving it
/// toward the ink, wherever the window's height puts the footer on its gradient (#469): the
/// accent's own lift at its hue covers it. Without a colour it's the ink.
#[test]
fn the_shown_spaces_icon_reads_on_the_footer_light_and_dark() {
    let themes = grid(
        &[0., 45., 90., 135., 180., 225., 270., 315.],
        &[0., 0.4, 1.],
        &[0., 0.3, 0.6, 1.],
        &[0., 1.],
    );
    let mut worst = (f32::MAX, String::new());
    for theme in themes
        .iter()
        .filter(|theme| theme.appearance != Appearance::Automatic)
    {
        let tone = if theme.appearance == Appearance::Light {
            Tone::Light
        } else {
            Tone::Dark
        };
        let palette = Palette::of(Some(theme), tone);
        let footer = fine_window(&palette);
        for at in footer
            .into_iter()
            .map(|at| over(at, palette.ink, GRAIN_SHIFT * theme.grain))
        {
            let ratio = contrast(palette.accent, at);
            if ratio < worst.0 {
                worst = (ratio, theme.to_text());
            }
        }
    }
    assert!(worst.0 >= GLYPH, "{:.2} on {}", worst.0, worst.1);
    // A Space with no colour draws it in the ink.
    let none = Palette::of(None, Tone::Dark);
    assert!(
        none.window_samples(None)
            .iter()
            .all(|&at| contrast(none.ink, at) >= GLYPH)
    );
}

/// The themes that came out worst for each component in the design sweep, today's and v3's.
#[test]
fn the_design_sweeps_worst_themes_pass() {
    let worst = [
        (3, 0., "dark", 340., 1., 0.5, 0.4),
        (3, 0., "dark", 340., 0.75, 0.9, 0.7),
        (1, 0., "automatic", 0., 0.05, 0.65, 0.9),
        (1, 0., "light", 350., 0.35, 0.4, 0.8),
        (3, 0., "automatic", 110., 0.05, 0.55, 1.),
        (3, 0., "automatic", 330., 1., 0.55, 0.4),
        (2, 1., "automatic", 60., 0.05, 0.35, 0.8),
        (2, 1., "automatic", 110., 0.1, 0.75, 0.5),
        (3, 1., "dark", 200., 0.2, 0.75, 0.2),
        (1, 1., "light", 320., 0.2, 0.5, 0.4),
        (1, 0., "light", 80., 0.35, 0.35, 0.9),
        (1, 0., "automatic", 90., 0.1, 0.15, 0.),
        (1, 0., "automatic", 280., 1., 0.95, 1.),
        (2, 0., "automatic", 270., 0.1, 0.15, 0.9),
        (2, 0., "dark", 300., 1., 0.45, 0.),
        (3, 1., "automatic", 180., 1., 0.95, 1.),
        (3, 1., "automatic", 300., 1., 0.95, 1.),
        (3, 1., "automatic", 60., 1., 0.95, 1.),
        (3, 0., "automatic", 270., 1., 0.15, 0.2),
        (1, 0., "automatic", 60., 1., 0.55, 0.),
        (1, 0., "automatic", 60., 1., 0.5, 0.),
        (1, 0., "automatic", 180., 0.05, 0.1, 0.),
        (1, 0., "automatic", 30., 0.5, 0.1, 0.),
        (2, 0., "automatic", 300., 0.35, 0.15, 0.3),
        (2, 0., "automatic", 90., 0.2, 0.15, 0.7),
    ];
    let themes: Vec<Theme> = worst
        .iter()
        .map(|&(n, g, app, hue, s, l, i)| theme(n, g, app, hue, s, l, i))
        .collect();
    let mut themes = themes;
    // The picker's own swatches, and the field's extremes.
    for color in [
        0xf0eae5, 0xe59eba, 0x9e749b, 0xe1676f, 0xef8c63, 0xf2d46c, 0x73e49b, 0x7eb6d5, 0x656786,
    ] {
        themes.push(Theme::of_color(color));
    }
    for color in [0x000000, 0xffffff, 0x808080, 0x00ff6a, 0xffff00, 0x0000ff] {
        themes.push(Theme::of_color(color));
    }
    assert_eq!(sweep(&themes), Vec::<String>::new());
}

/// Every theme of a coarse grid over the picker's output keeps every component readable.
#[test]
fn every_theme_of_a_coarse_sweep_reads() {
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0., 0.4, 1.],
        &[0., 0.2, 0.45, 0.6, 0.8, 1.],
        &[0., crate::theme::DEFAULT_INTENSITY, 1.],
    );
    assert_eq!(sweep(&themes), Vec::<String>::new());
}

/// A filled button pressed goes a step darker than under the pointer, whatever its label (the
/// owner, #707): ilo green from #4EA76E to #469462, not lighter.
#[test]
fn a_pressed_accent_goes_darker_than_hovered() {
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[0.5],
    );
    let tones = [Tone::Dark, Tone::Light];
    for (theme, tone) in themes
        .iter()
        .map(Some)
        .chain([None])
        .flat_map(|theme| tones.map(|tone| (theme, tone)))
    {
        let palette = Palette::of(theme, tone);
        let [hovered, _, _] = crate::theme::oklab(palette.accent_hovered().to_rgb());
        let [pressed, _, _] = crate::theme::oklab(palette.accent_pressed().to_rgb());
        assert!(pressed < hovered, "{theme:?} {tone:?}");
    }
    assert_eq!(
        hex(Palette::of(None, Tone::Dark).accent_pressed().to_rgb()),
        0x60ae7a
    );
}

/// A filled button under the pointer goes 0.9 darker, whatever its label (the owner, 2026-10-26):
/// the dialog's OK on ilo green from #78D999 to #6CC38A, not lighter.
#[test]
fn a_hovered_accent_goes_darker() {
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[0.5],
    );
    let tones = [Tone::Dark, Tone::Light];
    for (theme, tone) in themes
        .iter()
        .map(Some)
        .chain([None])
        .flat_map(|theme| tones.map(|tone| (theme, tone)))
    {
        let palette = Palette::of(theme, tone);
        let hovered = palette.accent_hovered().to_rgb();
        for (rest, hovered) in [
            (palette.accent.r, hovered.r),
            (palette.accent.g, hovered.g),
            (palette.accent.b, hovered.b),
        ] {
            assert!((hovered - rest * 0.9).abs() < 0.005, "{theme:?} {tone:?}");
        }
    }
    let green = Palette::of(None, Tone::Dark);
    assert_eq!(hex(green.accent), 0x78d999);
    assert_eq!(hex(green.accent_hovered().to_rgb()), 0x6cc38a);
}

/// The find bar's caret is ilo green on its dark fill, whatever the Space's theme.
#[test]
fn ilo_green_reads_on_the_dark_overlay() {
    assert!(contrast(rgb(NO_THEME_ACCENT), rgb(OVERLAY_DARK)) >= GLYPH);
}

/// The design sweep's grid, 1,197,504 themes: `cargo test --release --lib -- --ignored
/// full_sweep`, or a part of it at a time with SWEEP_PART=1/8 to 8/8.
#[test]
#[ignore]
fn full_sweep() {
    let hues: Vec<f32> = (0..36).map(|i| i as f32 * 10.).collect();
    let saturations = [0., 0.05, 0.1, 0.2, 0.35, 0.5, 0.75, 1.];
    let lightnesses: Vec<f32> = (0..=20).map(|i| i as f32 / 20.).collect();
    let intensities: Vec<f32> = (0..=10).map(|i| i as f32 / 10.).collect();
    let mut themes = grid(&hues, &saturations, &lightnesses, &intensities);
    if let Some((part, parts)) = std::env::var("SWEEP_PART").ok().and_then(|part| {
        let (part, parts) = part.split_once('/')?;
        Some((part.parse::<usize>().ok()?, parts.parse::<usize>().ok()?))
    }) {
        let size = themes.len().div_ceil(parts);
        themes = themes
            .into_iter()
            .skip((part - 1) * size)
            .take(size)
            .collect();
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let failed: Vec<String> = std::thread::scope(|scope| {
        let chunks: Vec<_> = themes
            .chunks(themes.len().div_ceil(threads))
            .map(|chunk| scope.spawn(move || sweep(chunk)))
            .collect();
        chunks
            .into_iter()
            .flat_map(|chunk| chunk.join().unwrap())
            .collect()
    });
    let palettes: Vec<Palette> = (themes.iter())
        .flat_map(|t| [Tone::Dark, Tone::Light].map(|tone| Palette::of(Some(t), tone)))
        .collect();
    let card = palettes.iter().filter(|p| p.card != rgba(CARD)).count();
    let faded = (themes.iter().flat_map(|t| [t, t]))
        .zip(&palettes)
        .filter(|(t, p)| p.layer_alpha < layer_alpha(t, p.tone) - 1e-6)
        .count();
    eprintln!(
        "{} themes, {faded} faded, {card} with a darker card, {} failing",
        themes.len(),
        failed.len()
    );
    assert_eq!(
        failed.iter().take(20).collect::<Vec<_>>(),
        Vec::<&String>::new()
    );
}

/// A swatch that reads on its ground has no ring; one that doesn't takes Ink or Paper, whichever
/// stands out there.
#[test]
fn a_swatch_is_ringed_only_where_it_fades_into_its_ground() {
    let garden = Theme::of_color(0x00ff6a);
    assert_eq!(Palette::swatch_ring(Some(&garden), rgb(0x0b0b0b)), None);
    assert_eq!(
        Palette::swatch_ring(Some(&garden), rgb(0xfafaf9)).map(hex),
        Some(INK)
    );
    let night = Theme::of_color(0x110066);
    assert_eq!(
        Palette::swatch_ring(Some(&night), rgb(0x0b0b0b)).map(hex),
        Some(PAPER)
    );
}

/// The palette is worked out once a theme changes, not while drawing, so it has to be quick. The
/// fastest of 20 is timed: a busy machine only ever adds time to a run, so one of them still runs
/// with the processor to itself, where a mean of them failed under load (#2470).
#[test]
fn a_palette_takes_well_under_a_frame() {
    let theme = theme(3, 1., "automatic", 300., 1., 0.95, 1.);
    let fastest = (0..20)
        .map(|_| {
            let start = std::time::Instant::now();
            std::hint::black_box(Palette::of(Some(&theme), Tone::Dark));
            start.elapsed()
        })
        .min()
        .unwrap();
    assert!(fastest < std::time::Duration::from_millis(4), "{fastest:?}");
}

/// A shimmer's highlight goes from where the floor puts the text to the ink, so it shows on a
/// window that raises the text near the ink, as the dark default does: there 0.5 to 0.95 drew
/// 0.88 to 0.95, a highlight hardly seen.
#[test]
fn a_shimmer_shows_however_far_the_floor_raises_the_text() {
    let palette = Palette::of(None, Tone::Dark);
    let rest = palette.text(0.5).a;
    eprintln!("the dark default's text floor puts 0.5 at {rest:.3}");
    assert!(
        rest > 0.8,
        "the dark default's floor no longer raises the text: {rest}"
    );
    assert_eq!(palette.lit(0.5, 1., 0.).a, rest);
    assert_eq!(palette.lit(0.5, 1., 1.).a, 1.);
    let half = palette.lit(0.5, 1., 0.5).a;
    assert!(
        (half - (rest + 1.) / 2.).abs() < 1e-4,
        "half lit is {half}, not half way from {rest}"
    );
}

/// A shimmer's highlight shows in the colour GPUI draws text in, which keeps the text's own alpha
/// under a highlight's colour: lit as a colour of the same ink, the card's status never moved.
#[test]
fn a_shimmer_draws_as_lit() {
    let palette = Palette::of(None, Tone::Dark);
    let peak = gpui::TextStyle {
        color: palette.lit(0.5, 1., 1.),
        ..Default::default()
    };
    for lit in [0., 0.25, 0.5, 1.] {
        let drawn = peak
            .clone()
            .highlight(palette.lit_highlight(0.5, 1., lit))
            .color
            .a;
        let meant = palette.lit(0.5, 1., lit).a;
        assert!(
            (drawn - meant).abs() < 1e-4,
            "{lit} lit drew {drawn}, not {meant}"
        );
    }
}

/// A Space's swatch of two colours reads as its main colour, the one its accents take, with the
/// second the smaller part, lying as the window's gradient does (#1849). GPUI's 135° runs a disc
/// from its top left edge at 0 to its bottom right at 1, so the middle, and most of the disc, lies
/// nearer the main colour there, and the second still shows whole at the far edge. The window's
/// own 75 % left the middle two thirds of the way to the second.
#[test]
fn a_two_colour_swatch_reads_as_its_main_colour() {
    let orange_blue = theme(2, 0., "automatic", 30., 1., 0.55, DEFAULT_INTENSITY);
    let [main, second] = [0, 1].map(|index| rgb(orange_blue.colors()[index]));
    let swatches = [
        ("a theme's", Swatch::of_theme(Some(&orange_blue))),
        ("a menu's", Swatch::of_two(main, second)),
    ];
    let distance = |a: Rgba, b: Rgba| (a.r - b.r).hypot(a.g - b.g).hypot(a.b - b.b);
    for (which, swatch) in swatches {
        assert_eq!((swatch.from, swatch.to, swatch.angle), (main, second, 135.));
        let toward_second =
            |along: f32| ((along - swatch.from_at) / (swatch.to_at - swatch.from_at)).clamp(0., 1.);
        let color = |along: f32| theme::mix_oklab(main, second, toward_second(along));
        let middle = color(0.5);
        assert!(
            distance(middle, main) < distance(middle, second),
            "{which} middle is nearer its second colour: {:06x}",
            hex(middle)
        );
        assert_eq!(hex(color(0.)), hex(main), "{which} top left");
        assert_eq!(hex(color(1.)), hex(second), "{which} bottom right");
        // The disc's share nearer the main colour, at points 1/40 of it apart.
        let points: Vec<f32> = (0..40)
            .flat_map(|i| {
                (0..40).map(move |j| ((i as f32 + 0.5) / 40. - 0.5, (j as f32 + 0.5) / 40. - 0.5))
            })
            .filter(|(x, y)| x.hypot(*y) <= 0.5)
            .map(|(x, y)| 0.5 + (x + y) * std::f32::consts::FRAC_1_SQRT_2)
            .collect();
        let main_share = points
            .iter()
            .filter(|&&along| distance(color(along), main) < distance(color(along), second))
            .count() as f32
            / points.len() as f32;
        assert!(
            main_share > 0.6,
            "{which} disc is {main_share:.2} its main colour"
        );
    }
}

/// The pickers' card shows what's behind it only where the host blurs it: elsewhere, Linux's
/// Wayland and every compositor that blurs no overlay, and Windows, it's opaque (#3676).
#[test]
fn the_pickers_card_is_opaque_where_nothing_blurs_what_is_behind_it() {
    let bright = theme(3, 1., "automatic", 300., 1., 0.95, 1.);
    for tone in [Tone::Dark, Tone::Light] {
        for chosen in [None, Some(&bright)] {
            let palette = Palette::of(chosen, tone);
            assert!(
                palette.card.a < 1.,
                "{tone:?} {chosen:?}: the card isn't translucent"
            );
            assert_eq!(
                palette.card_where(true),
                palette.card,
                "{tone:?} {chosen:?}: blurred"
            );
            let opaque = palette.card_where(false);
            assert_eq!(
                opaque.a, 1.,
                "{tone:?} {chosen:?}: the card shows what's behind it"
            );
            assert_eq!(
                opaque,
                over(rgb(OVERLAY_DARK), palette.card, palette.card.a)
            );
        }
    }
}

/// The tab shown's row keeps its Space's colour, and its title reads on it at 4.5:1 at rest and
/// under the pointer, wherever the window's glow puts it, on every Space colour the picker makes,
/// on a dark window and a light one, over a black and a white backdrop (#4091).
#[test]
fn the_tab_shown_reads_on_every_space_colour_dark_and_light() {
    let mut themes = grid(
        &[
            0., 30., 60., 90., 120., 150., 180., 210., 240., 270., 300., 330.,
        ],
        &[0., 0.5, 1.],
        &[0., 0.25, 0.5, 0.75, 1.],
        &[0., crate::theme::DEFAULT_INTENSITY, 1.],
    );
    for color in [
        0xf0eae5, 0xe59eba, 0x9e749b, 0xe1676f, 0xef8c63, 0xf2d46c, 0x73e49b, 0x7eb6d5, 0x656786,
        0x7a70f4, 0xe8743b, 0x12a594, 0x000000, 0xffffff, 0xffff00, 0x0000ff,
    ] {
        themes.push(Theme::of_color(color));
    }
    let mut worst = (f32::MAX, String::new());
    for theme in &themes {
        for tone in [Tone::Dark, Tone::Light] {
            let palette = Palette::of(Some(theme), tone);
            let title = Rgba::from(palette.text(1.));
            for hover in [0., 1.] {
                let plate = Rgba::from(palette.shown(hover));
                for at in fine_window(&palette) {
                    let under = over(at, plate, plate.a);
                    let ratio = contrast(over(under, title, title.a), under);
                    if ratio < worst.0 {
                        worst = (ratio, format!("{} {tone:?} hover {hover}", theme.to_text()));
                    }
                }
            }
            // It is the Space's colour: its hue kept wherever it has one.
            let main = rgb(theme.main_color());
            let shown = palette
                .shown
                .expect("a Space with a colour has a shown colour");
            let [_, a, b] = crate::theme::oklab(main);
            let [_, sa, sb] = crate::theme::oklab(shown);
            if a.hypot(b) > 0.02 {
                let turned = (a * sb - b * sa).atan2(a * sa + b * sb).abs();
                assert!(turned < 0.2, "{} {tone:?} turned {turned}", theme.to_text());
            }
        }
    }
    assert!(worst.0 >= TEXT, "{:.2} on {}", worst.0, worst.1);
}

/// The neutrals hold their order, and text at the secondary strength reads at 4.5:1 on a window
/// of either tone over either end of its backdrop, the ground tokens' (#4089): the floor lifts it
/// where 0.6 alone falls short.
#[test]
fn secondary_text_reads_at_both_ends_of_each_window() {
    assert!(TEXT_TERTIARY < TEXT_SECONDARY && TEXT_SECONDARY < TEXT_PRIMARY);
    assert!(CARD_PLATE < FIELD_PLATE && FIELD_PLATE < BUTTON_PLATE);
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        let text = Rgba::from(palette.text(TEXT_SECONDARY));
        for at in palette.window_samples(None) {
            let ratio = contrast(over(at, text, text.a), at);
            assert!(ratio >= TEXT - 1e-4, "{tone:?} {ratio:.2} on {at:?}");
        }
    }
}

/// What 0.6 of the ink alone reads at, at each end of each window, for the PR to report.
#[test]
#[ignore]
fn secondary_text_unlifted() {
    for tone in [Tone::Dark, Tone::Light] {
        for ground in tone.grounds(tone.tint(START)) {
            let ink = tone.ink();
            let ratio = contrast(over(ground, ink, TEXT_SECONDARY), ground);
            println!("{tone:?} over {:06x}: {ratio:.2}", theme::packed(ground));
        }
    }
}

/// What the HUD window's material, which what floats lies on, makes of a white page, a mid grey
/// one, a black one and a photo, untinted, in each look: run 6 in the VM (#4087), as captured.
const MATERIAL_DARK: [u32; 4] = [0xadacac, 0x646464, 0x1f1f1f, 0x733e50];
const MATERIAL_LIGHT: [u32; 4] = [0xf5f5f4, 0xbcbcbb, 0x7c7c7c, 0xcf98ab];

/// What AppKit draws in the material's place with Reduce Transparency or Increase Contrast, an
/// opaque ground, dark and light.
const OPAQUE_MATERIAL_DARK: u32 = crate::tokens::palette::MATERIAL_SOLID;
const OPAQUE_MATERIAL_LIGHT: u32 = crate::tokens::palette::LIGHT_MATERIAL_SOLID;

fn material(tone: Tone) -> [u32; 4] {
    match tone {
        Tone::Dark => MATERIAL_DARK,
        Tone::Light => MATERIAL_LIGHT,
    }
}

/// The lowest contrast of `surface`'s ink at `strength`, at `level` in `tone`, over `grounds`.
fn least_on(
    surface: Float,
    tone: Tone,
    level: Transparency,
    grounds: &[u32],
    strength: f32,
) -> f32 {
    let fill = surface.fill_at(tone, Some(level));
    let text = surface.ink(tone);
    grounds
        .iter()
        .map(|&behind| {
            let ground = over(rgb(behind), fill, fill.a);
            contrast(over(ground, text, strength), ground)
        })
        .fold(f32::MAX, f32::min)
}

/// Each transparency level holds its rule over what the material makes of any page (#4090). Less
/// holds the floor before levels, text at 0.6 of the ink at 4.5:1 over a white, grey and black
/// page. Standard and More hold text at full strength at 4.5:1 and at 0.6 at 3:1 over all of them
/// and a photo.
#[test]
fn each_transparency_level_holds_its_rule_over_any_page() {
    let mut failures = Vec::new();
    for surface in Float::ALL {
        for tone in [Tone::Dark, Tone::Light] {
            // One drawn dark in both looks lies on the dark material.
            let grounds = material(surface.tone(tone));
            for level in Transparency::ALL {
                let rules: &[(&[u32], f32, f32)] = match level {
                    Transparency::Less => &[(&grounds[..3], 0.6, TEXT)],
                    _ => &[(&grounds, 1., TEXT), (&grounds, 0.6, 3.)],
                };
                for &(on, strength, floor) in rules {
                    let ratio = least_on(surface, tone, level, on, strength);
                    if ratio < floor {
                        failures.push(format!(
                            "{surface:?} {tone:?} {level:?} at {strength}: {ratio:.2}"
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Each level shows more of what's behind than the one before, and the dark and light fills step
/// together, so the user's choice reads the same in both looks.
#[test]
fn each_transparency_level_shows_more_than_the_one_before() {
    for surface in Float::ALL {
        for tone in [Tone::Dark, Tone::Light] {
            let alphas = Transparency::ALL.map(|level| surface.fill_at(tone, Some(level)).a);
            assert!(
                alphas.windows(2).all(|pair| pair[0] > pair[1]),
                "{surface:?} {tone:?}: {alphas:?}"
            );
        }
    }
}

/// Reduce Transparency and Increase Contrast win at every level: AppKit draws an opaque ground
/// in the material's place, and text at 0.6 of the ink holds 4.5:1 on any level's fill over it.
#[test]
fn reduce_transparency_wins_at_every_level() {
    for surface in Float::ALL {
        for tone in [Tone::Dark, Tone::Light] {
            let ground = match surface.tone(tone) {
                Tone::Dark => OPAQUE_MATERIAL_DARK,
                Tone::Light => OPAQUE_MATERIAL_LIGHT,
            };
            for level in Transparency::ALL {
                let ratio = least_on(surface, tone, level, &[ground], 0.6);
                assert!(ratio >= TEXT, "{surface:?} {tone:?} {level:?}: {ratio:.2}");
            }
        }
    }
}

/// With no material under it, what floats is opaque, its colour at every level, so text at 0.6
/// of its ink holds 4.5:1 on any page.
#[test]
fn what_floats_is_opaque_with_no_material_under_it() {
    for surface in Float::ALL {
        for tone in [Tone::Dark, Tone::Light] {
            let opaque = surface.fill_at(tone, None);
            assert_eq!(opaque.a, 1., "{surface:?} {tone:?}");
            for level in Transparency::ALL {
                let frosted = surface.fill_at(tone, Some(level));
                assert_eq!(
                    (frosted.r, frosted.g, frosted.b),
                    (opaque.r, opaque.g, opaque.b),
                    "{surface:?} {tone:?} {level:?}: the same colour"
                );
            }
            let ratio = contrast(over(opaque, surface.ink(tone), 0.6), opaque);
            assert!(ratio >= TEXT, "{surface:?} {tone:?}: {ratio:.2}");
        }
    }
}

#[test]
fn the_window_takes_the_users_level_and_shows_more_of_what_is_behind_it_at_each_step() {
    for tone in [Tone::Dark, Tone::Light] {
        let tints = Transparency::ALL.map(|level| {
            set_transparency(level);
            Palette::of(None, tone).tint.a
        });
        set_transparency(Transparency::Standard);
        assert!(
            tints.windows(2).all(|step| step[0] > step[1]),
            "{tone:?}: {tints:?}, from Less to More"
        );
        assert_eq!(
            tints[1],
            Palette::of(None, tone).tint.a,
            "{tone:?}: Standard is the window as it was"
        );
    }
}

/// At Less the window is as opaque as before levels, so no plate is shaded and no text of
/// either ink, primary or gray, is under 4.5:1 (#4090).
#[test]
fn at_less_nothing_is_shaded_and_no_text_is_under_4_5() {
    set_transparency(Transparency::Less);
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        assert_eq!(palette.ground_shades, [0.; Ground::ALL.len()], "{tone:?}");
        let window = fine_window(&palette);
        assert_eq!(
            failures_over(None, &palette, &window, TEXT),
            Vec::<String>::new(),
            "{tone:?}"
        );
    }
}

/// With Reduce Transparency or Increase Contrast, AppKit draws an opaque ground in the window's
/// material's place and the level's tint lies over it: no text of either ink is under 4.5:1 on it,
/// shaded plates and all, at any level (#4090).
#[test]
fn reduce_transparency_holds_the_windows_text_at_every_level() {
    for tone in [Tone::Dark, Tone::Light] {
        let opaque = rgb(match tone {
            Tone::Dark => OPAQUE_MATERIAL_DARK,
            Tone::Light => OPAQUE_MATERIAL_LIGHT,
        });
        for level in Transparency::ALL {
            set_transparency(level);
            let palette = Palette::of(None, tone);
            let window = [over(opaque, palette.tint, palette.tint.a)];
            assert_eq!(
                failures_over(None, &palette, &window, TEXT),
                Vec::<String>::new(),
                "{tone:?} {level:?}"
            );
        }
    }
}

/// The selected row's plate reads apart from a hovered row's at every level, in either look,
/// shaded grounds and all: the lighter of the two by 1.15:1 at every point of the window. With no
/// shade they're 1.18:1 apart at worst, at More over white, where a hovered plate shaded less than
/// the selected one ended the lighter (#4090).
///
/// A dark window at More is the one place it doesn't hold: there a plate under the pointer has
/// to tell from a bright window at 1.2:1 and lets its title give, the selected one tells as much
/// as its title's 4.5:1 leaves, and over a white backdrop the pointer's is the lighter, by under
/// 1.02:1.
#[test]
fn the_selected_plate_reads_apart_from_the_hovered_one_at_every_level() {
    const APART: f32 = 1.15;
    for tone in [Tone::Dark, Tone::Light] {
        for level in Transparency::ALL {
            set_transparency(level);
            let palette = Palette::of(None, tone);
            let selected = Rgba::from(palette.selected(0.));
            let hovered = Rgba::from(palette.hovered_row());
            let bright = (tone, level) == (Tone::Dark, Transparency::More);
            for at in fine_window(&palette) {
                let (selected, hovered) =
                    (over(at, selected, selected.a), over(at, hovered, hovered.a));
                let apart = luminance(selected) > luminance(hovered)
                    && contrast(selected, hovered) >= APART;
                assert!(
                    apart || (bright && contrast(selected, hovered) < 1.02),
                    "{tone:?} {level:?}: {:06x} beside {:06x}",
                    hex(selected),
                    hex(hovered)
                );
            }
        }
    }
}

/// What the palette reckons the window over black, grey 128 and white, which check/window.sh reads
/// the sidebar against, is the window it draws at its darkest and lightest, glows and all: over
/// black on a black desktop, and over white with the most a desktop picture lifts it.
#[test]
fn the_window_reckoned_over_greys_is_the_one_drawn() {
    for (theme, tone) in [
        (None, Tone::Dark),
        (Some(Theme::at(0., 0.)), Tone::Dark),
        (Some(Theme::of_color(0x5a2a9a)), Tone::Light),
    ] {
        let palette = Palette::of(theme.as_ref(), tone);
        for (foot, far) in [(0., 0.), (0.4, 0.), (1., 1.)] {
            let [black, ..] = palette.window_over_greys(foot, far, 0.).map(hex);
            let [.., white] = palette.window_over_greys(foot, far, 1.).map(hex);
            assert_eq!(
                [black, white],
                palette.window_at(foot, far).map(hex),
                "{tone:?} {foot} {far}"
            );
        }
    }
}

/// A dragged row's or Space's drop line shows on every point of the window, dark and light, with a
/// Space's colour or none, at every level: a glyph's 3:1 against the window under it.
#[test]
fn the_drop_line_shows_on_either_window() {
    for level in [
        Transparency::Less,
        Transparency::Standard,
        Transparency::More,
    ] {
        set_transparency(level);
        for tone in [Tone::Dark, Tone::Light] {
            for theme in [
                None,
                Some(Theme::at(0., 0.)),
                Some(Theme::of_color(0x5a2a9a)),
            ] {
                let palette = Palette::of(theme.as_ref(), tone);
                let line = palette.drop_line();
                let line = Rgba::from(line);
                for at in fine_window(&palette) {
                    let drawn = over(at, line, line.a);
                    assert!(
                        contrast(drawn, at) >= GLYPH,
                        "{level:?} {tone:?} {:06x} on {:06x}",
                        hex(drawn),
                        hex(at)
                    );
                }
            }
        }
    }
}

/// Gray text is gray: at every level, in either look, a secondary line meets the level's gray
/// contrast on every point of the window, the lightest included, and is drawn dimmer than a
/// primary line beside it there by 1.15:1, the step this file already holds two plates apart by
/// (above), so a row's detail reads as less than its title at a glance (#4128).
#[test]
fn secondary_text_reads_as_gray_beside_primary_at_every_level() {
    use crate::tokens::palette::{TEXT_PRIMARY, TEXT_SECONDARY};
    const APART: f32 = 1.15;
    for level in Transparency::ALL {
        set_transparency(level);
        for tone in [Tone::Dark, Tone::Light] {
            let palette = Palette::of(None, tone);
            let primary = Rgba::from(palette.text(TEXT_PRIMARY));
            let secondary = Rgba::from(palette.gray(TEXT_SECONDARY));
            for at in fine_window(&palette) {
                let (primary, secondary) = (
                    over(at, primary, primary.a),
                    over(at, secondary, secondary.a),
                );
                assert!(
                    contrast(secondary, at) >= gray_text(level) - 1e-4,
                    "{level:?} {tone:?} on {:06x}: {:.2}",
                    hex(at),
                    contrast(secondary, at)
                );
                assert!(
                    contrast(primary, at) > contrast(secondary, at)
                        && contrast(primary, secondary) >= APART,
                    "{level:?} {tone:?} on {:06x}: {:.3} apart",
                    hex(at),
                    contrast(primary, secondary)
                );
            }
        }
    }
}

/// The plate under the foot's shown card in a dark window with no Space colour, as the real app
/// composed it, read from its own pixels: over the check VM's desktop at Standard and at More
/// (f98ded880), and at Standard with a white window behind (469483355), its lightest measured.
/// And a shown page row's at More, from another capture, over a lighter spot of what was behind.
const CARD_PLATE_DARK: [u32; 3] = [0x6b6a65, 0x696762, 0x6d6d6b];
const PAGE_PLATE_DARK_MORE: u32 = 0x706e69;

/// Gray text on a plate in a dark window is one ink at every level, whatever alpha its place gives
/// it off a plate, and holds text's 4.5:1 with the palette's margin on the shown card's plate as
/// measured, where the state line's own gray read 3.39 to 4.28. A light window's is the gray it
/// has off a plate.
///
/// A test has no window material, so this knows the token's ink over the grounds measured on one
/// desktop and one window behind it, and no others: what another backdrop makes of the plate is
/// the real app's to be measured on. On the page row's measured plate Paper itself is under 4.5:1,
/// so no ink holds it there and it is the plate that would have to give; the gray is held to
/// within 1.05:1 of the title's ink beside it, the most a plate that light lets it read.
#[test]
fn gray_on_a_dark_plate_is_one_ink_that_reads_on_the_measured_card() {
    use crate::tokens::palette::PLATE_GRAY;
    use crate::tokens::shell::task::{PAGE_WORD, PAGE_WORD_WORKING, STATUS};
    let places = [
        STATUS,
        PAGE_WORD,
        PAGE_WORD_WORKING,
        crate::tokens::shell::SECONDARY,
    ];
    for level in Transparency::ALL {
        set_transparency(level);
        let dark = Palette::of(None, Tone::Dark);
        let inks = places.map(|alpha| dark.gray_on_plate(alpha));
        assert!(
            inks.iter().all(|&ink| ink == inks[0]),
            "{level:?}: {inks:?}"
        );
        let ink = Rgba::from(inks[0]);
        assert!(ink.a >= PLATE_GRAY && ink.a >= dark.gray_floor, "{level:?}");
        for plate in CARD_PLATE_DARK.map(rgb) {
            let ratio = contrast(over(plate, ink, ink.a), plate);
            assert!(
                ratio >= TEXT + BETWEEN,
                "{level:?} on {:06x}: {ratio:.2}",
                hex(plate)
            );
        }
        let plate = rgb(PAGE_PLATE_DARK_MORE);
        let (title, gray) = (
            contrast(dark.ink, plate),
            contrast(over(plate, ink, ink.a), plate),
        );
        assert!(
            title < TEXT,
            "the page row's plate holds text now: {title:.2}"
        );
        assert!(
            title / gray <= 1.05,
            "{level:?}: {gray:.2} under {title:.2}"
        );
        let light = Palette::of(None, Tone::Light);
        for alpha in places {
            assert_eq!(light.gray_on_plate(alpha), light.gray(alpha), "{level:?}");
        }
    }
}

/// Gray text on a dark window's plate holds text's 4.5:1 by the plate's own ground, however much
/// of a bright window comes through: on the foot's shown card's and shown page row's plate, at
/// rest and under the pointer, over every point of the window over a black and a white backdrop,
/// with a Space's colour and with none, at every level. The plates take the palette's shade as
/// the window's tint asks, so this follows the tint's tokens wherever they end. The plate a card,
/// the count or a page row has under the pointer is the next test's.
///
/// What it knows is the palette's own account of the window: the material's measured greys under
/// the tint, lifted as a desktop picture lifts it. At Standard with no colour that account's
/// lightest shown plate is #6d6d6b, the real app's own pixel with a white window behind (at
/// 469483355). A material that reads lighter than its account is the real app's to be measured on.
#[test]
fn gray_on_a_dark_plate_reads_over_a_bright_window_at_every_level() {
    use crate::tokens::shell::task::STATUS;
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[crate::theme::DEFAULT_INTENSITY],
    );
    for level in Transparency::ALL {
        set_transparency(level);
        for theme in themes.iter().map(Some).chain([None]) {
            let palette = Palette::of(theme, Tone::Dark);
            let ink = Rgba::from(palette.gray_on_plate(STATUS));
            let plates = [
                ("shown", palette.shown(0.)),
                ("shown, under the pointer", palette.shown(1.)),
            ];
            for at in fine_window(&palette) {
                for (name, plate) in plates {
                    let plate = Rgba::from(plate);
                    let ground = over(at, plate, plate.a);
                    let ratio = contrast(over(ground, ink, ink.a), ground);
                    assert!(
                        ratio >= TEXT,
                        "{level:?} {theme:?} {name} on {:06x}: {ratio:.2}",
                        hex(ground)
                    );
                }
            }
        }
    }
}

/// A plate under the pointer over the window that isn't a sidebar row's (a card's at the sidebar's
/// foot, a row of the Space form) holds two things at once, over every point of the window over a
/// black and a white backdrop, at every level: it tells from the window, and the title on it
/// reads, as a dark window's gray does, which is the title's ink there. With no Space colour both
/// hold in both looks.
///
/// A Space's colour can leave a dark window too bright for both: the ink reads on the bare window
/// at little over 4.5, and a plate that tells takes a fifth of that. There the plate still tells,
/// on every colour, and its title, under the pointer alone, reads what that leaves. A light
/// window's plate is the ink at the ladder's step, as it was, and with a Space's colour holds
/// both over a white backdrop.
///
/// As the test above, it knows the palette's own account of the window and no real material.
#[test]
fn a_plate_under_the_pointer_tells_and_its_title_reads_at_every_level() {
    use crate::tokens::shell::{HOVERED, task::STATUS};
    // 1.2 is what a dark window's plate has at Standard over a white backdrop, where it's plain to
    // see; 4.2 is what a plate that tells leaves the title at More, where most comes through.
    let (tells, at_more) = (1.2, 4.2);
    // A Space's colour is fitted so the ink reads on the bare window at 4.5, and a plate that
    // tells is at most a step past its aim over that, 1.25: 4.5 over 1.25 is the title's least.
    let on_a_colour = 3.6;
    // The least a plate tells and the least its title, and a dark window's gray, read over `window`.
    let least = |palette: &Palette, window: &[Rgba]| {
        let plate = Rgba::from(palette.hovered_plate());
        let gray = Rgba::from(palette.gray_on_plate(STATUS));
        window
            .iter()
            .fold((f32::MAX, f32::MAX), |(told, read), &at| {
                let ground = over(at, plate, plate.a);
                let title = contrast(palette.ink, ground);
                let word = match palette.tone {
                    Tone::Dark => contrast(over(ground, gray, gray.a), ground),
                    Tone::Light => title,
                };
                (told.min(contrast(ground, at)), read.min(title).min(word))
            })
    };
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[crate::theme::DEFAULT_INTENSITY],
    );
    for level in Transparency::ALL {
        set_transparency(level);
        let reads = match level {
            Transparency::More => at_more,
            _ => TEXT,
        };
        for tone in [Tone::Dark, Tone::Light] {
            let palette = Palette::of(None, tone);
            let (told, read) = least(&palette, &fine_window(&palette));
            assert!(
                told >= tells,
                "{level:?} {tone:?}: the plate tells at {told:.2}"
            );
            assert!(
                read >= reads,
                "{level:?} {tone:?}: its title reads {read:.2}"
            );
        }
        for theme in &themes {
            let dark = Palette::of(Some(theme), Tone::Dark);
            let (told, read) = least(&dark, &fine_window(&dark));
            assert!(
                told >= tells,
                "{level:?} {theme:?}: the plate tells at {told:.2}"
            );
            assert!(
                read >= on_a_colour,
                "{level:?} {theme:?}: its title reads {read:.2}"
            );
            let light = Palette::of(Some(theme), Tone::Light);
            assert_eq!(
                light.hovered_plate(),
                light.plate(HOVERED),
                "{level:?} {theme:?}"
            );
            // The window over a white backdrop is the second of each point's two.
            let white: Vec<Rgba> = fine_window(&light).into_iter().skip(1).step_by(2).collect();
            let (told, read) = least(&light, &white);
            assert!(
                told >= tells,
                "{level:?} {theme:?} light: the plate tells at {told:.2}"
            );
            assert!(
                read >= TEXT,
                "{level:?} {theme:?} light: its title reads {read:.2}"
            );
        }
    }
}

/// A sidebar row's plate under the pointer on a dark window is the plate anything has under the
/// pointer there, with a Space's colour and with none, at every level, so a row's and a card's at
/// the sidebar's foot are one form and what the test above holds of one it holds of the other. A
/// light window's row keeps the ink at its own fainter step.
#[test]
fn a_rows_plate_under_the_pointer_is_the_one_a_card_has_on_a_dark_window() {
    use crate::tokens::shell::LIGHT_HOVERED;
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[crate::theme::DEFAULT_INTENSITY],
    );
    for level in Transparency::ALL {
        set_transparency(level);
        for theme in themes.iter().map(Some).chain([None]) {
            let dark = Palette::of(theme, Tone::Dark);
            assert_eq!(
                dark.hovered_row(),
                dark.hovered_plate(),
                "{level:?} {theme:?}"
            );
            let light = Palette::of(theme, Tone::Light);
            assert_eq!(
                light.hovered_row(),
                light.ground_plate(Ground::Hovered, LIGHT_HOVERED),
                "{level:?} {theme:?}"
            );
        }
    }
}

/// The plate of what's shown is never the fainter of the two beside a plate under the pointer. On
/// a dark window with no Space colour, at Less and at Standard, it tells from the window at TELLS
/// at least and no less than a row's plate under the pointer does, at rest and under the pointer
/// itself, and its title keeps its bar, on every point of the window over both backdrops.
///
/// At More the ink reads 5.45 on the window's brightest point, which a plate that tells at TELLS
/// and a title at its bar take between them whole. There the shown plate still tells at TELLS
/// with its title at its bar on the colour a screen shows, and the pointer's, which tells first
/// and lets its title give, is ahead of it by 0.025 at most.
///
/// With a Space's colour the window leaves less than both take, and this holds neither to it.
#[test]
fn the_shown_plate_is_never_the_fainter_where_its_title_keeps_its_bar() {
    // What the pointer's plate may tell over the shown one at More, where the shown one's title
    // keeps 4.5 and the pointer's doesn't.
    let ahead_at_more = 0.025;
    for level in Transparency::ALL {
        set_transparency(level);
        let palette = Palette::of(None, Tone::Dark);
        let window = fine_window(&palette);
        // The least `plate` tells from the window, and the least the ink reads on it.
        let least = |plate: Hsla| {
            let plate = Rgba::from(plate);
            window
                .iter()
                .fold((f32::MAX, f32::MAX), |(told, read), &at| {
                    let ground = over(at, plate, plate.a);
                    (
                        told.min(contrast(ground, at)),
                        read.min(contrast(palette.ink, ground)),
                    )
                })
        };
        let (pointed, _) = least(palette.hovered_row());
        let tells = match level {
            Transparency::More => TELLS.max(pointed - ahead_at_more),
            _ => TELLS.max(pointed),
        };
        for hover in [0., 1.] {
            let (told, read) = least(palette.shown(hover));
            assert!(
                told >= tells,
                "{level:?} hover {hover}: the shown plate tells at {told:.3}, one under the pointer at {pointed:.3}"
            );
            assert!(
                read >= TEXT,
                "{level:?} hover {hover}: its title reads {read:.4}"
            );
        }
    }
}

/// The Library's search field is a plate of the ink on the window, and its placeholder gray text
/// on it: there it holds the level's bar for gray text in both looks, with a Space's colour and
/// with none, over every point of the window over a black and a white backdrop.
///
/// But for one case, which is the window's and not the placeholder's: at Less, whose bar for gray
/// text is text's own, a Space's colour can leave a dark window on which Paper itself is under
/// that bar on a plate. There the placeholder is the ink whole, the most there is, and reads what
/// a title on a plate under the pointer holds there (`failures_over`).
///
/// Only the look's own gray reads. With no Space colour the other look's, on the same field, is
/// under every level's bar in both looks, almost the field's own colour, which is why the field
/// takes its colours from the palette it's drawn with and keeps none.
#[test]
fn the_library_search_placeholder_reads_on_its_field_at_every_level() {
    use crate::tokens::shell::library::{LIBRARY_PLATE, SEARCH_PLACEHOLDER};
    const WHOLE_INK: f32 = 3.6;
    let themes = grid(
        &[0., 60., 120., 180., 240., 300.],
        &[0.4, 1.],
        &[0.2, 0.45, 0.8],
        &[crate::theme::DEFAULT_INTENSITY],
    );
    // The least gray text of `ink` reads on `palette`'s field anywhere on its window.
    let least = |palette: &Palette, ink: Hsla| {
        let ink = Rgba::from(ink);
        fine_window(palette)
            .into_iter()
            .map(|at| {
                let field = over(at, palette.ink, LIBRARY_PLATE);
                contrast(over(field, ink, ink.a), field)
            })
            .fold(f32::MAX, f32::min)
    };
    for level in Transparency::ALL {
        set_transparency(level);
        for theme in themes.iter().map(Some).chain([None]) {
            for tone in [Tone::Dark, Tone::Light] {
                let palette = Palette::of(theme, tone);
                let gray = palette.gray(SEARCH_PLACEHOLDER);
                let ratio = least(&palette, gray);
                let whole = (level, tone) == (Transparency::Less, Tone::Dark)
                    && theme.is_some()
                    && gray.a >= 1.
                    && ratio >= WHOLE_INK;
                assert!(
                    ratio >= gray_text(level) - 1e-4 || whole,
                    "{level:?} {tone:?} {theme:?}: {ratio:.2} at {:.2} of the ink",
                    gray.a
                );
            }
        }
        let (dark, light) = (
            Palette::of(None, Tone::Dark),
            Palette::of(None, Tone::Light),
        );
        for (palette, other) in [(&light, &dark), (&dark, &light)] {
            let ratio = least(palette, other.gray(SEARCH_PLACEHOLDER));
            assert!(
                ratio < gray_text(level),
                "{level:?} {:?}: the other look's gray reads {ratio:.2}",
                palette.tone
            );
        }
    }
}

/// The shown row's title reads at text's minimum on its plate as a screen shows it, 8 bits a
/// channel, at every level, on a dark window with no Space's colour, where the plate's shade is
/// solved to the least that reads (`solve_selected`): over the window's darkest ground and over
/// its lightest, a white backdrop with the most a desktop picture lifts the material. Solved on
/// the unrounded colour, the plate read 4.49 at More once drawn.
#[test]
fn the_shown_rows_title_reads_on_the_plate_as_drawn_at_every_level() {
    for level in Transparency::ALL {
        set_transparency(level);
        let palette = Palette::of(None, Tone::Dark);
        let lifted = palette.window_over_greys(0., 0., 1.);
        let grounds = fine_window(&palette).into_iter().chain(lifted);
        for at in grounds {
            for hover in [0., 1.] {
                let plate = Rgba::from(palette.shown(hover));
                let shown = drawn(over(at, plate, plate.a));
                let reads = contrast(palette.ink, shown);
                assert!(
                    reads >= TEXT,
                    "{level:?} hover {hover}: the title reads {reads:.3} on #{:06x} over #{:06x}",
                    hex(shown),
                    hex(at)
                );
            }
        }
    }
    set_transparency(Transparency::Standard);
}

/// A Space's glass on its bar, from solid to clearest, in hundredths.
fn bar() -> impl Iterator<Item = f32> {
    (0..=100).map(|step| step as f32 / 100.)
}

/// The least gray text reads over `palette`'s window, both backdrops and every glow, less what it
/// has to hold there.
fn gray_margin(palette: &Palette) -> f32 {
    let gray = Rgba::from(palette.gray(0.));
    let window = fine_window(palette);
    let worst = (window.iter())
        .map(|&at| contrast(over(at, gray, gray.a), at))
        .fold(f32::MAX, f32::min);
    worst - gray_text_at(palette.glass)
}

/// The sweep's one excused miss, by the lead's word (2026-11-25): the Library's section label on a
/// dark window in a colour, where Less sits, reads just under gray's 4.5 there, in today's build
/// too, which no coarser test saw. No colour moves at Less for it; P6 redraws the Library and
/// takes this out. It fails the day the label reads 4.5 or more, or less than this.
const LABEL_AT_LESS: f32 = 4.49;
const LIBRARY_LABEL: &str = "Library section label, every state";

/// The least the Library's section label reads on `palette`'s `window` in each state it takes.
fn library_label_reads(palette: &Palette, window: &[Rgba], gray: f32) -> f32 {
    let (_, label, _, grounds) = states(palette, window, gray)
        .into_iter()
        .find(|(name, ..)| format!("{name}, every state") == LIBRARY_LABEL)
        .expect("the Library's section label is one of the states");
    let label = Rgba::from(label);
    (grounds.iter())
        .map(|&at| contrast(over(at, label, label.a), at))
        .fold(f32::MAX, f32::min)
}

/// A Space's glass anywhere on its bar keeps every floor of the window and what lies on it, gray
/// text at the one where its glass sits, over a black and a white backdrop: a sweep from solid to
/// clear in hundredths, in Dark and Light, with no colour and with one. The title on a plate under
/// the pointer may read less past the bar's last stretch, as at today's More, and the Library's
/// section label is excused at one point (`LABEL_AT_LESS`).
#[test]
fn a_spaces_glass_holds_every_floor_across_its_bar() {
    let colored = Theme::at(0.3, -0.5);
    let mut failed = Vec::new();
    for theme in [None, Some(&colored)] {
        for tone in [Tone::Dark, Tone::Light] {
            for glass in bar() {
                let palette = Palette::at(theme, tone, glass);
                let window = fine_window(&palette);
                let gray = gray_text_at(glass);
                let mut failures = failures_over(theme, &palette, &window, gray);
                if tone == Tone::Dark && theme.is_some() && glass == LESS {
                    let reads = library_label_reads(&palette, &window, gray);
                    assert!(
                        (LABEL_AT_LESS..gray).contains(&reads),
                        "the Library's section label, Dark, coloured, where Less sits, reads \
                         {reads:.4}: the miss it was excused for ({LABEL_AT_LESS} to under \
                         {gray}) has gone or grown; take the exception out or stop"
                    );
                    let before = failures.len();
                    failures.retain(|each| !each.starts_with(LIBRARY_LABEL));
                    assert_eq!(before - failures.len(), 1, "{failures:#?}");
                }
                let colour = if theme.is_some() { "coloured" } else { "none" };
                failed.extend(
                    failures
                        .into_iter()
                        .map(|each| format!("{tone:?} {colour} {glass}: {each}")),
                );
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The least gray text reads on a Space's window at `glass`, over a black and a white backdrop and
/// every glow.
fn gray_reads(theme: Option<&Theme>, tone: Tone, glass: f32) -> f32 {
    gray_margin(&Palette::at(theme, tone, glass)) + gray_text_at(glass)
}

/// Gray text holds 4.5:1 from solid up to where Less sits, and then what today's levels hold where
/// they sit, never less than More's.
#[test]
fn gray_text_holds_what_todays_level_did_where_it_sits() {
    for level in Transparency::ALL {
        assert_eq!(gray_text_at(of_level(level)), gray_text(level), "{level:?}");
    }
    assert_eq!(gray_text_at(0.), GRAY_TEXT_LESS);
    assert!(bar().all(|glass| gray_text_at(glass) >= GRAY_TEXT_MORE));
    assert!(GRAY_TEXT_LESS >= TEXT, "Less's gray holds 4.5");
}

/// A Space carried over at today's level is the window it was: its palette at that place on the
/// bar is the app's at the level.
#[test]
fn a_space_at_a_levels_place_is_that_levels_window() {
    let colored = Theme::at(0.3, -0.5);
    for level in Transparency::ALL {
        set_transparency(level);
        for theme in [None, Some(&colored)] {
            for tone in [Tone::Dark, Tone::Light] {
                let app = Palette::of(theme, tone);
                let space = Palette::at(theme, tone, of_level(level));
                assert_eq!(app.tint, space.tint, "{level:?} {tone:?}");
                assert_eq!(app.gray_floor, space.gray_floor, "{level:?} {tone:?}");
            }
        }
    }
    set_transparency(Transparency::Standard);
}

/// The window shows more of what's behind it the further along the bar its Space's glass is, from
/// solid at 0.
#[test]
fn a_spaces_glass_shows_more_further_along_its_bar() {
    for tone in [Tone::Dark, Tone::Light] {
        let tints: Vec<f32> = bar()
            .map(|glass| Palette::at(None, tone, glass).tint.a)
            .collect();
        assert_eq!(tints[0], 1., "{tone:?}: solid at 0");
        assert!(
            tints.windows(2).all(|step| step[0] > step[1]),
            "{tone:?}: {tints:?}"
        );
    }
}

/// A window's overlay and dialog fills are the same whatever its Space's glass: a Space's value
/// changes only the window's glass.
#[test]
fn overlay_and_dialog_fills_dont_follow_a_spaces_glass() {
    for level in Transparency::ALL {
        set_transparency(level);
        for glass in [0., START, 1.] {
            for tone in [Tone::Dark, Tone::Light] {
                let (overlay, dialog) = tone.overlays();
                let palette = Palette::at(None, tone, glass);
                assert_eq!((palette.overlay, palette.dialog), (overlay, dialog));
            }
        }
    }
    set_transparency(Transparency::Standard);
}

/// What gray text reads at the bar's points under 4.5:1 until the contrast floor, for the DEV.
#[test]
#[ignore]
fn gray_text_under_4_5_on_the_bar() {
    for tone in [Tone::Dark, Tone::Light] {
        for glass in [0., of_level(Transparency::Less), START, 1.] {
            let reads = gray_reads(None, tone, glass);
            println!(
                "{tone:?} at {glass}: gray reads {reads:.2}, target {}",
                gray_text_at(glass)
            );
        }
    }
}

/// Every fifth level of each channel, every grey, and the greys' neighbours where Ink and Paper
/// both fall short of text: the colours a page may report as its own.
fn page_colours() -> impl Iterator<Item = Rgba> {
    let levels = || (0..=255u32).step_by(5);
    let stepped = levels()
        .flat_map(move |r| levels().flat_map(move |g| levels().map(move |b| r << 16 | g << 8 | b)));
    let greys = (0..=255u32).map(|grey| grey << 16 | grey << 8 | grey);
    let near_middle = (100..=140u32).flat_map(|grey| {
        [(1, 0, 0), (0, 1, 0), (0, 0, 1), (3, 0, 2)]
            .map(|(r, g, b)| (grey + r) << 16 | (grey + g) << 8 | (grey + b))
    });
    stepped.chain(greys).chain(near_middle).map(rgb)
}

#[test]
fn text_on_a_pages_own_colour_reads_whatever_the_colour() {
    for window in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, window);
        for fill in page_colours() {
            let on = palette.on_fill(fill);
            // The dimmest a title, a host's line or a glyph is asked for: the floor carries it.
            for (ink, bar) in [
                (on.text(0.), TEXT),
                (on.gray(0.), gray_text(transparency())),
                (on.glyph(0.), GLYPH),
            ] {
                let (color, alpha) = (Hsla { a: 1., ..ink }.to_rgb(), ink.a);
                let reads = contrast(drawn(over(fill, color, alpha)), drawn(fill));
                assert!(
                    reads >= bar,
                    "{:06x} on {:06x} at {alpha} reads {reads}, under {bar}",
                    hex(color),
                    hex(fill),
                );
            }
        }
    }
}

#[test]
fn the_ink_on_a_pages_colour_is_the_colours_alone_not_the_windows() {
    let (dark, light) = (
        Palette::of(None, Tone::Dark),
        Palette::of(None, Tone::Light),
    );
    for fill in page_colours() {
        let (on_dark, on_light) = (dark.on_fill(fill), light.on_fill(fill));
        assert_eq!(on_dark.ink, on_light.ink, "on {:06x}", hex(fill));
        assert_eq!(on_dark.tone, on_light.tone, "on {:06x}", hex(fill));
        // A light ink is a dark page's: the tone its plates and quiet steps follow.
        let lit = luminance(on_dark.ink) > luminance(fill);
        assert_eq!(on_dark.tone == Tone::Dark, lit, "on {:06x}", hex(fill));
    }
    // Paper on a dark page and Ink on a light one, as the window's own text is.
    assert_eq!(dark.on_fill(rgb(0x101014)).ink, rgb(PAPER));
    assert_eq!(dark.on_fill(rgb(0xffffff)).ink, rgb(INK));
}
