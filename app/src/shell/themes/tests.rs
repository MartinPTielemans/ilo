//! A Space's theme in the window: what the window's colours are with one, two or three colours,
//! and the picker's theme showing as it's made until the core has it.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext, point, px, size};

use super::*;
use crate::shell::recorder::Recorder;
use crate::theme::DEFAULT_INTENSITY;

const BLUE: u32 = 0x3d81f9;

/// The shell with one Space, with `color` and `theme`, in a window as big as Arc's recording.
fn shell(
    color: Option<u32>,
    theme: Option<Theme>,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext) {
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(Rc::new(Recorder::default()), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(color, theme)], cx);
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    shell.update(cx, |shell, _| shell.viewport = size(px(1800.), px(1130.)));
    (shell, cx)
}

/// Space 1 as the core reports it, with `color` and `theme`.
fn entry(color: Option<u32>, theme: Option<Theme>) -> crate::shell::SpaceEntry {
    let text = theme.map_or(String::new(), |theme| theme.to_text());
    (
        1,
        "Space".into(),
        color,
        1,
        "".into(),
        "".into(),
        text.into(),
    )
}

fn rgb_of(color: Rgba) -> u32 {
    theme::packed(color)
}

#[gpui::test]
fn a_theme_of_one_colour_at_its_start_looks_as_the_colour_alone(cx: &mut TestAppContext) {
    let theme = Theme::at(0.3, -0.5);
    let color = theme.main_color();
    let (alone, cx) = shell(Some(color), None, cx);
    let before: Vec<_> = alone.update(cx, |shell, _| {
        [0., 0.5, 1.].map(|y| shell.window_fill_at(y)).to_vec()
    });
    alone.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(Some(color), Some(theme))], cx)
    });
    let after: Vec<_> = alone.update(cx, |shell, _| {
        [0., 0.5, 1.].map(|y| shell.window_fill_at(y)).to_vec()
    });
    assert_eq!(before, after);
}

#[gpui::test]
fn two_colours_go_from_the_top_left_to_the_second_along_the_diagonal(cx: &mut TestAppContext) {
    let theme = Theme::at(0.6, 0.).with_count(2);
    let [main, second] = theme.colors()[..] else {
        panic!("two colours")
    };
    let (shell, cx) = shell(Some(main), Some(theme), cx);
    shell.update(cx, |shell, _| {
        assert_eq!(rgb_of(shell.theme_color_at(&theme, 1., 0., 0.)), main);
        assert_eq!(rgb_of(shell.theme_color_at(&theme, 1., 1., 1.)), second);
        // The right edge is the second colour from three quarters of the way down the diagonal.
        let (width, height) = (1800., 1130.);
        let y = (SECOND_AT * (width + height) - width) / height;
        assert_eq!(
            rgb_of(shell.theme_color_at(&theme, 1., 1., y + 0.01)),
            second
        );
        assert_ne!(
            rgb_of(shell.theme_color_at(&theme, 1., 1., y - 0.2)),
            second
        );
        // A third comes in down the window.
        let three = theme.with_count(3);
        let third = three.colors()[2];
        let bottom_left = shell.theme_color_at(&three, 1., 0., 1.);
        assert_eq!(
            rgb_of(bottom_left),
            rgb_of(theme::mix_oklab(rgb(main), rgb(third), 1.))
        );
    });
}

#[gpui::test]
fn intensity_washes_the_colours_out(cx: &mut TestAppContext) {
    // A deep red the palette never has to hold back (a paler one stops at its legibility cap).
    let theme = Theme::at(0.3, 0.);
    let (shell, cx) = shell(Some(theme.main_color()), Some(theme), cx);
    shell.update(cx, |shell, _| {
        let alpha = |intensity| {
            let theme = Theme { intensity, ..theme };
            shell
                .theme_color_at(
                    &theme,
                    Palette::of(Some(&theme), Tone::Dark).layer_alpha,
                    0.5,
                    0.5,
                )
                .a
        };
        assert!(alpha(0.) < alpha(0.24) && alpha(0.24) < alpha(DEFAULT_INTENSITY));
        assert!(alpha(DEFAULT_INTENSITY) < alpha(1.));
    });
}

#[gpui::test]
fn the_picker_s_theme_shows_at_once_and_gives_way_to_the_kept_one(cx: &mut TestAppContext) {
    let (shell, cx) = shell(Some(BLUE), None, cx);
    let made = Theme::at(-0.4, 0.2).with_count(2);
    shell.update(cx, |shell, cx| {
        shell.theme_changed(1, made, false, cx);
        assert_eq!(shell.theme_of(1), Some(made));
        // The core's report of another theme, such as the colour kept ahead of it, leaves it.
        shell.set_spaces(vec![entry(Some(made.main_color()), None)], cx);
        assert_eq!(shell.theme_of(1), Some(made));
        shell.set_spaces(vec![entry(Some(made.main_color()), Some(made))], cx);
        assert_eq!(shell.theme_preview, None);
        assert_eq!(
            shell.theme_of(1).map(|theme| theme.to_text()),
            Some(made.to_text())
        );
    });
}

/// Remove Theme Color in the Space's menu takes its theme and colour away, and the menu offers it
/// no more once the core says the Space has none.
#[gpui::test]
fn remove_theme_color_takes_the_space_back_to_the_accent(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    let theme = Theme::at(0.5, 0.).with_count(2);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(Some(theme.main_color()), Some(theme))], cx);
        shell.set_space(1, cx);
        shell.space_menu(1, point(px(100.), px(112.)), cx);
    });
    host.take();
    cx.update(|_, cx| host.choose("Remove Theme Color", cx));
    assert_eq!(host.take(), ["space theme 1 none", "space color 1 none"]);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(None, None)], cx);
        assert_eq!(shell.theme_of(1), None);
        shell.space_menu(1, point(px(100.), px(112.)), cx);
    });
    let titles = host.menus.borrow().last().cloned().unwrap_or_default();
    assert!(titles.iter().any(|(title, _)| title == "Edit Theme Color…"));
    assert!(
        !titles
            .iter()
            .any(|(title, _)| title == "Remove Theme Color")
    );
}

/// ilo's menu takes the window's look: light where the Space's theme makes it light, dark where
/// it makes it dark, and the system's where it's automatic; its highlight the Space's accent in
/// that look. The Space's menu keeps Delete Space apart, below a line.
#[gpui::test]
fn the_menu_takes_the_windows_look(cx: &mut TestAppContext) {
    use crate::menu::Highlight;
    use crate::theme::Appearance;
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    let themed = |appearance| Theme {
        appearance,
        ..Theme::at(0.5, 0.)
    };
    let shown = [
        (Appearance::Light, None, Tone::Light),
        (Appearance::Dark, None, Tone::Dark),
        (Appearance::Automatic, Some(Tone::Dark), Tone::Dark),
        (Appearance::Automatic, Some(Tone::Light), Tone::Light),
    ];
    for (appearance, system, _) in shown {
        let theme = themed(appearance);
        shell.update(cx, |shell, cx| {
            shell.force_tone(system);
            shell.set_spaces(vec![entry(Some(theme.main_color()), Some(theme))], cx);
            shell.set_space(1, cx);
        });
        // The system's appearance is read as the window draws.
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            shell.space_menu(1, point(px(100.), px(112.)), cx)
        });
    }
    let looks = host.menu_looks.borrow();
    let tones = shown.map(|(_, _, tone)| tone);
    assert_eq!(
        looks.iter().map(|look| look.light).collect::<Vec<_>>(),
        tones.map(|tone| tone == Tone::Light)
    );
    let accents = shown
        .map(|(appearance, _, tone)| Highlight::of(&Palette::of(Some(&themed(appearance)), tone)));
    assert_eq!(
        looks.iter().map(|look| look.highlight).collect::<Vec<_>>(),
        accents
    );
    assert_ne!(
        accents[0].fill, accents[1].fill,
        "the light look's accent is the dark one's"
    );
    assert_ne!(
        accents[1].fill,
        Highlight::of(&Palette::default()).fill,
        "the theme's accent is ilo green"
    );
    let entries = host.entries.borrow();
    let [.., line, delete] = &entries[..] else {
        panic!("no menu")
    };
    assert_eq!(
        (line.title.as_ref(), delete.title.as_ref()),
        ("", "Delete Space")
    );
    assert!(delete.destructive && line.parent.is_none());
}

/// Capture mode, started in the window's look, takes it again as the system's appearance or the
/// Space's theme changes it (#861).
#[gpui::test]
fn capture_mode_follows_the_windows_look(cx: &mut TestAppContext) {
    use crate::theme::Appearance;
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    let light = || host.capture_look.get().map(|(_, light)| light);
    for (system, theme, shown) in [
        (Tone::Dark, None, false),
        (Tone::Light, None, true),
        (
            Tone::Light,
            Some(Theme {
                appearance: Appearance::Dark,
                ..Theme::at(0.5, 0.)
            }),
            false,
        ),
        (Tone::Light, None, true),
    ] {
        shell.update(cx, |shell, cx| {
            shell.force_tone(Some(system));
            shell.set_spaces(
                vec![entry(theme.map(|theme| theme.main_color()), theme)],
                cx,
            );
            shell.set_space(1, cx);
        });
        // The system's appearance is read as the window draws.
        cx.run_until_parked();
        assert_eq!(
            light(),
            Some(shown),
            "with the system {system:?} and the theme {theme:?}"
        );
    }
}

/// View › Appearance stands in for the system's: light or dark turns a Space with no theme, or an
/// automatic one, that way, a theme that forces its own keeps it, and automatic gives back the
/// system's. The host keeps the choice, the menu bar checks it, and the command bar offers the
/// other two.
#[gpui::test]
fn the_apps_appearance_stands_in_for_the_systems(cx: &mut TestAppContext) {
    use crate::shell::commands::{Command, ShellHandle, bar_commands};
    use crate::theme::Appearance;
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(None, None)], cx);
        shell.set_space(1, cx);
    });
    cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
    let tone = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| shell.palette.tone)
    };
    let system = tone(cx);
    for (appearance, shown) in [
        (Appearance::Dark, Tone::Dark),
        (Appearance::Light, Tone::Light),
        (Appearance::Automatic, system),
    ] {
        shell.update(cx, |shell, cx| {
            shell.run(Command::Appearance(appearance), "ui", cx)
        });
        assert_eq!(
            tone(cx),
            shown,
            "{appearance:?} drew the window {:?}",
            tone(cx)
        );
        assert_eq!(host.appearance.get(), Some(appearance));
        assert_eq!(host.menu_bar.borrow().appearance, appearance);
        let offered = cx.update(|_, cx| bar_commands("appearance", cx));
        let offered: Vec<Command> = offered.iter().map(|row| row.command).collect();
        let others: Vec<Command> = (Appearance::ALL.into_iter())
            .filter(|&each| each != appearance)
            .map(Command::Appearance)
            .collect();
        assert_eq!(offered, others);
    }
    let light = Theme {
        appearance: Appearance::Light,
        ..Theme::at(0.5, 0.)
    };
    shell.update(cx, |shell, cx| {
        shell.run(Command::Appearance(Appearance::Dark), "ui", cx);
        shell.set_spaces(vec![entry(Some(light.main_color()), Some(light))], cx);
    });
    assert_eq!(
        tone(cx),
        Tone::Light,
        "a theme's own appearance gave way to the app's"
    );
}

/// The shell with Space 1 as `entry` has it, on a host that records what it asks of the core.
fn recorded(
    color: Option<u32>,
    theme: Option<Theme>,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(color, theme)], cx);
        shell.set_space(1, cx);
    });
    host.take_edits();
    (shell, host, cx)
}

/// What the shell asks of the core to give Space 1 `theme`, or take its theme away.
fn gives(theme: Option<Theme>) -> [String; 2] {
    match theme {
        Some(theme) => [
            format!("theme 1 {}", theme.to_text()),
            format!("color 1 Some({})", theme.main_color()),
        ],
        None => ["theme 1 none".into(), "color 1 None".into()],
    }
}

/// ⌘Z takes a picked theme back to none, the system's accent, and ⇧⌘Z gives it again.
#[gpui::test]
fn a_picked_theme_undoes_to_the_accent_and_redoes(cx: &mut TestAppContext) {
    let (shell, host, cx) = recorded(None, None, cx);
    let made = Theme::at(-0.4, 0.2).with_count(2);
    shell.update(cx, |shell, cx| shell.theme_changed(1, made, true, cx));
    assert_eq!(host.take_edits(), gives(Some(made)));
    shell.update(cx, |shell, cx| {
        assert_eq!(shell.history.titles(), (Some("Change Theme Color"), None));
        shell.set_spaces(vec![entry(Some(made.main_color()), Some(made))], cx);
        assert!(shell.undo(cx));
    });
    assert_eq!(host.take_edits(), gives(None));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(None, None)], cx);
        assert_eq!(shell.theme_of(1), None);
        assert!(shell.redo(cx));
    });
    assert_eq!(host.take_edits(), gives(Some(made)));
}

/// The picker hands the theme on each frame of a drag and keeps it on letting go, which is one
/// step: ⌘Z goes back to the theme before the drag, not to a frame of it.
#[gpui::test]
fn a_drag_in_the_picker_undoes_as_one_step(cx: &mut TestAppContext) {
    let before = Theme::at(0.5, 0.).with_count(2);
    let (shell, host, cx) = recorded(Some(before.main_color()), Some(before), cx);
    let frames = [0.2, 0.3, 0.4].map(|intensity| Theme {
        intensity,
        ..before
    });
    shell.update(cx, |shell, cx| {
        for frame in frames {
            shell.theme_changed(1, frame, false, cx);
        }
        assert_eq!(shell.theme_of(1), Some(frames[2]));
        shell.theme_changed(1, frames[2], true, cx);
    });
    assert_eq!(host.take_edits(), gives(Some(frames[2])));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(Some(before.main_color()), Some(frames[2]))], cx);
        assert!(shell.undo(cx));
        assert!(!shell.undo(cx));
    });
    assert_eq!(host.take_edits(), gives(Some(before)));
}

/// ⌘Z after Remove Theme Color gives the Space its theme and colour back.
#[gpui::test]
fn remove_theme_color_undoes(cx: &mut TestAppContext) {
    let theme = Theme::at(0.5, 0.).with_count(3);
    let (shell, host, cx) = recorded(Some(theme.main_color()), Some(theme), cx);
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(100.), px(112.)), cx)
    });
    cx.update(|_, cx| host.choose("Remove Theme Color", cx));
    assert_eq!(host.take_edits(), gives(None));
    shell.update(cx, |shell, cx| {
        assert_eq!(shell.history.titles(), (Some("Remove Theme Color"), None));
        shell.set_spaces(vec![entry(None, None)], cx);
        assert!(shell.undo(cx));
    });
    assert_eq!(host.take_edits(), gives(Some(theme)));
}

/// GPUI's shader, as gpui_apple's fill_color has it for a linear gradient on a square, `side`
/// across from the origin, with stops at `start` and `end`: how far from the first colour to the
/// second `point` is.
fn shader_t(angle: f32, side: f32, start: f32, end: f32, point: (f32, f32)) -> f32 {
    let radians = (angle - 90.).to_radians();
    let direction = (radians.cos(), radians.sin());
    let half = side / 2.;
    let to_point = (point.0 - half, point.1 - half);
    let dot = to_point.0 * direction.0 + to_point.1 * direction.1;
    let along = dot / direction.0.hypot(direction.1);
    let t = (along + half) / side;
    ((t - start) / (end - start)).clamp(0., 1.)
}

#[test]
fn the_diagonal_is_drawn_where_it_is_taken_for_what_covers_it() {
    for (width, height) in [(1800., 1130.), (800., 1200.), (1000., 1000.)] {
        let (side, start, end) = diagonal_stops(width, height);
        for (x, y) in [
            (0., 0.),
            (0.2, 0.7),
            (0.5, 0.5),
            (0.9, 0.1),
            (1., 1.),
            (0.6, 0.3),
        ] {
            let drawn = shader_t(135., side, start, end, (x * width, y * height));
            let taken = (theme::diagonal(x, y, width, height) / SECOND_AT).min(1.);
            assert!(
                (drawn - taken).abs() < 1e-4,
                "{width}×{height} at {x}, {y}: {drawn} {taken}"
            );
        }
    }
}

/// The colours the shell gave the core for its Boosts since the last time.
fn boost_colors_given(host: &Recorder) -> Vec<String> {
    std::mem::take(&mut host.boost_colors.borrow_mut())
}

/// A Space's colours go to its tabs' Boosts once at launch, and again only as its theme or the
/// appearance changes them.
#[gpui::test]
fn boosts_take_a_space_s_colours_as_they_change(cx: &mut TestAppContext) {
    use crate::shell::commands::{Command, ShellHandle};
    use crate::theme::Appearance;
    let (shell, host, cx) = recorded(None, None, cx);
    cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
    cx.run_until_parked();
    let system = shell.read_with(cx, |shell, _| shell.palette.tone);
    let launched = boost_colors(&Palette::of(None, system), None);
    assert_eq!(boost_colors_given(&host), [format!("1 {launched}")]);
    let flip = match system {
        Tone::Dark => Appearance::Light,
        Tone::Light => Appearance::Dark,
    };
    shell.update(cx, |shell, cx| {
        shell.run(Command::Appearance(flip), "ui", cx)
    });
    cx.run_until_parked();
    let tone = shell.read_with(cx, |shell, _| shell.palette.tone);
    let none = boost_colors(&Palette::of(None, tone), None);
    assert_ne!(none, launched);
    assert_eq!(boost_colors_given(&host), [format!("1 {none}")]);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        boost_colors_given(&host),
        Vec::<String>::new(),
        "a frame sent them again"
    );
    let theme = Theme::at(0.5, 0.);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![entry(Some(theme.main_color()), Some(theme))], cx)
    });
    cx.run_until_parked();
    let themed = boost_colors(&Palette::of(Some(&theme), tone), Some(&theme));
    assert_ne!(themed, none);
    assert_eq!(boost_colors_given(&host), [format!("1 {themed}")]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::Appearance(Appearance::Automatic), "ui", cx)
    });
    cx.run_until_parked();
    let back = boost_colors(&Palette::of(Some(&theme), system), Some(&theme));
    assert_eq!(boost_colors_given(&host), [format!("1 {back}")]);
}
