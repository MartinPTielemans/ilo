use super::*;
use crate::theme::Appearance;

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

/// The window, finer than the palette checks it: 17 points along the gradient's line and 17 down,
/// the sidebar's and the page's alike, each over a black and a white backdrop.
fn fine_window(palette: &Palette, theme: Option<&Theme>) -> Vec<Rgba> {
    let steps = 16;
    (0..=steps)
        .flat_map(|along| {
            (0..=steps).map(move |y| (along as f32 / steps as f32, y as f32 / steps as f32))
        })
        .flat_map(|(along, y)| palette.window_at(theme, along, y))
        .collect()
}

/// What fails: each of the chrome's components drawn as the shell draws it, with its alpha
/// from the token ladder, on every point of a window of `tone`, below its WCAG minimum.
fn failures(theme: Option<&Theme>, tone: Tone) -> Vec<String> {
    let palette = Palette::of(theme, tone);
    let window = fine_window(&palette, theme);
    let grain = theme.map_or(0., |theme| theme.grain);
    let ink = palette.ink;
    let grained: Vec<Rgba> = window
        .iter()
        .map(|&at| over(at, ink, GRAIN_SHIFT * grain))
        .collect();
    let plated =
        |plate: f32| -> Vec<Rgba> { window.iter().map(|&at| over(at, ink, plate)).collect() };
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
    check("unselected title", on(palette.text(0.78), &grained), TEXT);
    check(
        "title on hovered plate",
        on(palette.text(0.78), &plated(HOVERED_PLATE)),
        TEXT,
    );
    check(
        "URL host on field",
        on(palette.text(0.62), &plated(FIELD_PLATE)),
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
    check("focus ring, New Space circle", against(&window), GLYPH);
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
        .flat_map(|y| palette.window_at(theme, 0.5, y as f32 / 16.))
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
    for (name, content, minimum, on_states) in states(&palette, theme) {
        check(
            &format!("{name}, every state"),
            on(content, &on_states),
            minimum,
        );
    }
    // The Space's swatch, ringed where its edge doesn't reach 3:1 on the window.
    let swatch = match theme {
        Some(theme) => Window::of(theme, tone).swatch(),
        None => vec![rgb(NO_THEME_ACCENT)],
    };
    let edge = match Palette::swatch_ring_on(theme, &window) {
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
/// resting fill under each state it takes.
fn states(palette: &Palette, theme: Option<&Theme>) -> Vec<(String, Hsla, f32, Vec<Rgba>)> {
    use crate::tokens::{command_bar, dialog, offer, shell, site_controls, toast};
    const PRESSABLE: [f32; 2] = [STATE_PRESSED, STATE_HOVERED];
    let window = fine_window(palette, theme);
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
            palette.text(shell::library::SECTION_LABEL),
            TEXT,
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
            lay(&window, 0., &PRESSABLE),
        ),
        (
            "Library row detail",
            palette.text(shell::SECONDARY),
            TEXT,
            lay(&window, 0., &PRESSABLE),
        ),
        (
            "Library card name",
            palette.text(shell::UNSELECTED_TITLE),
            TEXT,
            lay(&window, 0., &[STATE_HOVERED]),
        ),
        (
            "Library and side panel buttons",
            palette.glyph(shell::NAV),
            GLYPH,
            lay(&window, 0., &PRESSABLE),
        ),
        (
            "URL field host",
            palette.text(shell::SECONDARY),
            TEXT,
            lay(&window, shell::URL_FIELD, &[STATE_HOVERED]),
        ),
        (
            "URL field placeholder",
            palette.text(shell::header::URL_PLACEHOLDER),
            TEXT,
            lay(&window, shell::URL_FIELD, &[STATE_HOVERED]),
        ),
        (
            "question card button",
            palette.text(shell::TITLE),
            TEXT,
            lay(&window, shell::URL_FIELD, &PRESSABLE),
        ),
        (
            "offer button",
            palette.text(offer::TITLE),
            TEXT,
            lay(&window, offer::PLATE, &PRESSABLE),
        ),
        (
            "offer chevron",
            palette.glyph(offer::DETAIL),
            GLYPH,
            lay(&window, 0., &PRESSABLE),
        ),
        (
            "toast chip key",
            palette.text(toast::KEY_COLOR),
            TEXT,
            lay(&window, toast::CHIP_FILL, &PRESSABLE),
        ),
        (
            "page chip key",
            palette.text(shell::conversation::CHIP_KEY),
            TEXT,
            lay(&window, toast::CHIP_FILL, &PRESSABLE),
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

/// A Space with no theme has the measured window, black 0.25 over the material and nothing more,
/// and ilo green made to read with Ink on it, and everything passes.
#[test]
fn no_theme_is_the_measured_window_and_ilo_green_with_ink() {
    let palette = Palette::of(None, Tone::Dark);
    assert_eq!(
        (palette.tint.a, palette.shade, palette.layer_alpha),
        (TINT, 0., 0.)
    );
    // #289054 reads 2.7:1 on the window over a white backdrop, so ilo green, lighter, at its hue.
    assert_eq!(hex(palette.accent), 0x57b97a);
    assert_eq!(hex(palette.accent_label), INK);
    assert!(contrast(palette.accent_label, palette.accent) >= TEXT);
    // #0B8045 with Paper, the value #319 first had, is under 4.5.
    assert!(contrast(rgb(PAPER), rgb(0x0b8045)) < TEXT);
    assert_eq!(failures(None, Tone::Dark), Vec::<String>::new());
}

/// A light window is its material under white 0.8 with Ink on it, a theme's colours over it at
/// Dia's light layer's alpha, and ilo green darker so it reads there; everything passes.
#[test]
fn a_light_window_is_white_over_the_material_with_ink() {
    let palette = Palette::of(None, Tone::Light);
    assert_eq!(
        (palette.tint, palette.shade, palette.layer_alpha),
        (
            Rgba {
                a: LIGHT_TINT,
                ..WHITE
            },
            0.,
            0.
        )
    );
    assert_eq!(hex(palette.ink), INK);
    assert!(luminance(palette.accent) < luminance(Palette::of(None, Tone::Dark).accent));
    assert_eq!(failures(None, Tone::Light), Vec::<String>::new());
    let violet = Palette::of(Some(&Theme::of_color(0x5a2a9a)), Tone::Light);
    assert_eq!(
        (violet.shade, violet.layer_alpha),
        (0., LIGHT_GRADIENT_ALPHA)
    );
}

/// A theme lies over the window as a Space's colour did before themes, and only a theme too light
/// for Paper over a white backdrop is shaded, by the least step that lets it read.
#[test]
fn the_window_is_shaded_only_where_paper_needs_it() {
    // check/window.sh's black Space, measured against Dia's composite: neutral, unshaded.
    let black = Palette::of(Some(&Theme::at(0., 0.)), Tone::Dark);
    assert_eq!(
        (black.tint.a, black.shade, black.layer_alpha),
        (TINT, 0., NEUTRAL_GRADIENT_ALPHA)
    );
    let violet = Palette::of(Some(&Theme::of_color(0x5a2a9a)), Tone::Dark);
    assert_eq!((violet.shade, violet.layer_alpha), (0., GRADIENT_ALPHA));
    // Blue lightens to #9AC3FF down the window, too light for Paper's plates over white.
    let pale = Theme::of_color(0x1a79ff);
    let palette = Palette::of(Some(&pale), Tone::Dark);
    assert!(palette.shade > 0., "the blue isn't shaded");
    let less = Palette {
        shade: palette.shade - SHADE_STEP,
        ..palette
    };
    let paper = rgb(PAPER);
    let fails = less
        .window_samples(Some(&pale))
        .iter()
        .any(|&at| contrast(paper, over(at, paper, SELECTED_PLATE)) < TEXT + BETWEEN);
    assert!(fails, "a step less shade would have read");
}

/// The pickers' card keeps Arc's measured grey where its text reads, and Capture's menu, ilo's
/// menu's panel, reads in both looks: its glyphs, in the secondary colour, at 3:1, and its field's
/// words as text on the field's tint of the line colour, which the secondary colour doesn't reach
/// in the dark look.
#[test]
fn the_pickers_card_and_captures_menu_read() {
    let black = Palette::of(Some(&Theme::at(0., 0.)), Tone::Dark);
    assert_eq!(
        hex(black.card),
        hex(rgba(CARD)),
        "the measured grey over the black Space"
    );
    assert_eq!(
        Palette::of(None, Tone::Dark).card,
        rgba(CARD),
        "the measured grey with no theme"
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
        assert!(light || contrast(lay(field, menu.secondary), field) < TEXT);
    }
}

/// With no theme over a white backdrop, ilo green reads under 3:1 on the Library's column under
/// the selected section's plate, so that glyph alone takes the nearest colour at its hue that
/// reads, and the accent stays. An accent that reads there is the glyph's colour as it is.
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
    assert_eq!(hex(none.accent), 0x57b97a);
    assert!(
        worst(none.accent) < GLYPH,
        "ilo green reads there: {:.2}",
        worst(none.accent)
    );
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
        let footer = (0..=16).flat_map(|along| {
            (12..=16).flat_map(move |y| {
                palette.window_at(Some(theme), along as f32 / 16., y as f32 / 16.)
            })
        });
        for at in footer.map(|at| over(at, palette.ink, GRAIN_SHIFT * theme.grain)) {
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
        0x469462
    );
}

/// A filled button under the pointer goes 0.9 darker, whatever its label (the owner, 2026-10-26):
/// the dialog's OK on ilo green from #57B97A to #4EA76E, not lighter.
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
    assert_eq!(hex(green.accent), 0x57b97a);
    assert_eq!(hex(green.accent_hovered().to_rgb()), 0x4ea76e);
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
    let shaded = palettes.iter().filter(|p| p.shade > 0.).count();
    let most = palettes.iter().map(|p| p.shade).fold(0., f32::max);
    let card = palettes.iter().filter(|p| p.card != rgba(CARD)).count();
    let faded = (themes.iter().flat_map(|t| [t, t]))
        .zip(&palettes)
        .filter(|(t, p)| p.layer_alpha < layer_alpha(t, p.tone) - 1e-6)
        .count();
    eprintln!(
        "{} themes, {shaded} shaded (at most {most}), {faded} faded, {card} with a darker card, {} failing",
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
