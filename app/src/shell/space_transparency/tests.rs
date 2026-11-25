//! Transparency one per Space, on a host recording what the shell asks of the core.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::palette::Tone;
use crate::shell::recorder::Recorder;
use crate::shell::undo::{Prop, Value};
use crate::shell::{Shell, SpaceEntry};
use crate::theme::Theme;
use crate::transparency::{self, Transparency};

/// A folder of the test's own for the app's, keeping `level` as today's app does.
fn folder(level: Transparency, cx: &mut TestAppContext) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ilo-space-transparency-{}-{}",
        std::process::id(),
        cx.test_function_name().unwrap_or("test")
    ));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if level != Transparency::Standard {
        std::fs::write(dir.join("transparency"), level.key()).unwrap();
    }
    dir
}

/// Space `id` as the core reports it, with theme text `text`.
fn entry(id: u64, text: &str) -> SpaceEntry {
    let color = Theme::parse(text).map(|theme| theme.main_color());
    let name = format!("Space {id}");
    (id, name.into(), color, 1, "".into(), "".into(), text.into())
}

/// A window given `spaces`, on a host of its own.
fn window(
    spaces: Vec<SpaceEntry>,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(spaces, cx);
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// What the shell asked the core to keep as Spaces' theme texts.
fn themes(host: &Recorder) -> Vec<String> {
    (host.take_edits().into_iter())
        .filter(|edit| edit.starts_with("theme "))
        .collect()
}

/// Today's level reaches every Space once, at the first Spaces a window is given: the Spaces given
/// again, a second window and the next launch, with the Spaces as the core kept them, carry
/// nothing, and the level's own file is as it was.
#[gpui::test]
fn todays_level_is_carried_into_every_space_once(cx: &mut TestAppContext) {
    let dir = folder(Transparency::More, cx);
    let kept = std::fs::read(dir.join("transparency")).unwrap();
    let colored = Theme::at(0.3, -0.5).to_text();
    let spaces = || vec![entry(1, &colored), entry(2, "")];
    let path = dir.to_string_lossy().into_owned();
    cx.update(|cx| transparency::kept_in(path.clone(), cx));
    let (shell, cx, host) = window(spaces(), cx);
    let carried = [
        format!("{colored} transparency=1.000"),
        "transparency=1.000".to_string(),
    ];
    assert_eq!(
        themes(&host),
        [
            format!("theme 1 {}", carried[0]),
            format!("theme 2 {}", carried[1]),
        ]
    );
    shell.update(cx, |shell, cx| shell.set_spaces(spaces(), cx));
    assert!(themes(&host).is_empty(), "the Spaces given again");
    let (_, _, second) = window(spaces(), cx);
    assert!(themes(&second).is_empty(), "a second window");
    cx.update(|_, cx| transparency::kept_in(path.clone(), cx));
    let kept_by_core = vec![entry(1, &carried[0]), entry(2, &carried[1])];
    shell.update(cx, |shell, cx| shell.set_spaces(kept_by_core, cx));
    assert!(themes(&host).is_empty(), "the next launch");
    assert_eq!(std::fs::read(dir.join("transparency")).unwrap(), kept);
    _ = std::fs::remove_dir_all(&dir);
}

/// A window before the launch has read the level, as in a test that keeps none, carries nothing.
#[gpui::test]
fn nothing_is_carried_before_the_level_is_read(cx: &mut TestAppContext) {
    let (_, _, host) = window(vec![entry(1, "")], cx);
    assert!(themes(&host).is_empty());
}

/// Settings' row and the onboarding's step set every Space at once, and a Space already at the
/// level isn't asked again.
#[gpui::test]
fn a_level_picked_in_settings_reaches_every_space(cx: &mut TestAppContext) {
    let dir = folder(Transparency::Standard, cx);
    let path = dir.to_string_lossy().into_owned();
    cx.update(|cx| transparency::kept_in(path, cx));
    let colored = Theme::at(0.3, -0.5).to_text();
    let spaces = vec![entry(1, &colored), entry(2, "transparency=0.300")];
    let (_, cx, host) = window(spaces, cx);
    assert!(
        themes(&host).is_empty(),
        "a Space keeps a value: carried before"
    );
    cx.update(|_, cx| transparency::set(Transparency::Less, cx));
    cx.run_until_parked();
    assert_eq!(
        themes(&host),
        [format!("theme 1 {colored} transparency=0.300")]
    );
    _ = std::fs::remove_dir_all(&dir);
}

/// A colour picked, or taken away, keeps the Space's value beside it.
#[gpui::test]
fn a_colour_change_keeps_the_spaces_value(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(vec![entry(1, "transparency=0.600")], cx);
    host.take_edits();
    let theme = Theme::at(0.3, -0.5);
    let color = Value::Color {
        color: Some(theme.main_color()),
        theme: theme.to_text(),
    };
    shell.update(cx, |shell, cx| shell.change(Prop::SpaceColor(1), color, cx));
    assert_eq!(
        themes(&host),
        [format!("theme 1 {} transparency=0.600", theme.to_text())]
    );
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![entry(1, &format!("{} transparency=0.600", theme.to_text()))],
            cx,
        );
        shell.remove_space_theme(1, cx);
    });
    assert_eq!(themes(&host), ["theme 1 transparency=0.600"]);
}

/// The tint a window draws at, and where its glass sits.
fn glass(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (f32, f32, bool) {
    shell.update(cx, |shell, _| {
        let palette = shell.palette;
        (palette.tint.a, palette.glass, palette.tone == Tone::Dark)
    })
}

/// Ends a Space switch's slide, and draws the window again.
fn settle(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// Two windows showing two Spaces each draw their own Space's glass, and a window going to the
/// other Space takes that one's.
#[gpui::test]
fn each_window_draws_its_shown_spaces_glass(cx: &mut TestAppContext) {
    let spaces = || {
        vec![
            entry(1, "transparency=0.300"),
            entry(2, "transparency=1.000"),
        ]
    };
    let (first, cx, _) = window(spaces(), cx);
    let (second, cx, _) = window(spaces(), cx);
    second.update(cx, |shell, cx| shell.set_space(2, cx));
    settle(&second, cx);
    settle(&first, cx);
    let (tint, at, dark) = glass(&first, cx);
    let expected = crate::space_transparency::tint(0.3, dark);
    assert_eq!((tint, at), (expected, 0.3), "the first window, on Space 1");
    let (tint, at, dark) = glass(&second, cx);
    let expected = crate::space_transparency::tint(1., dark);
    assert_eq!((tint, at), (expected, 1.), "the second, on Space 2");
    first.update(cx, |shell, cx| shell.set_space(2, cx));
    settle(&first, cx);
    assert_eq!(
        glass(&first, cx),
        glass(&second, cx),
        "the first, gone to 2"
    );
}

/// A window works out a palette once for each Space and value it shows, not at each frame or each
/// switch back, and once more when a Space's value changes.
#[gpui::test]
fn a_palette_is_worked_out_once_for_each_spaces_value(cx: &mut TestAppContext) {
    let spaces = |first: &str| vec![entry(1, first), entry(2, "transparency=1.000")];
    let (shell, cx, _) = window(spaces("transparency=0.300"), cx);
    let worked_out = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        settle(shell, cx);
        shell.update(cx, |shell, _| shell.palettes.borrow().len())
    };
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    let both = worked_out(&shell, cx);
    for space in [1, 2, 1, 2, 1] {
        shell.update(cx, |shell, cx| shell.set_space(space, cx));
        assert_eq!(worked_out(&shell, cx), both, "Space {space} shown again");
    }
    let spaces = spaces("transparency=0.600");
    shell.update(cx, |shell, cx| shell.set_spaces(spaces, cx));
    assert_eq!(worked_out(&shell, cx), both + 1, "Space 1 at its new value");
    assert_eq!(glass(&shell, cx).1, 0.6);
}

/// A Space keeping no value, as one sync brings from a device that hasn't carried, draws at the
/// app's level from its own file and not at the start, so nothing changes under the user.
#[gpui::test]
fn a_space_keeping_no_value_draws_at_the_apps_level(cx: &mut TestAppContext) {
    let dir = folder(Transparency::More, cx);
    let path = dir.to_string_lossy().into_owned();
    cx.update(|cx| transparency::kept_in(path, cx));
    let spaces = vec![entry(1, ""), entry(2, "transparency=0.300")];
    let (shell, cx, _) = window(spaces, cx);
    let (tint, at, dark) = glass(&shell, cx);
    assert_eq!(at, crate::space_transparency::of_level(Transparency::More));
    assert_eq!(tint, Transparency::More.tint(dark));
    _ = std::fs::remove_dir_all(&dir);
}
