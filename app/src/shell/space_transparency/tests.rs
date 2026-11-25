//! Transparency one per Space, on a host recording what the shell asks of the core.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

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
