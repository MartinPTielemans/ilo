//! Edit › AutoFill on a host that records what the shell asks of the core: Chrome's dropdown at the
//! field of the page with the keyboard, or Settings › Passwords with no field there.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Command, Folders, Shell, recorder::Recorder};

/// A window showing Space 1, with tab 11 shown if `tab`.
fn window(cx: &mut TestAppContext, tab: bool) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let today = if tab { vec![11] } else { Vec::new() };
        shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
        if tab {
            shell.set_selected(11, cx);
        }
    });
    host.take();
    (shell, host)
}

/// What Edit › AutoFill's three items asked of the host, each after the one before.
fn autofill(shell: &Entity<Shell>, host: &Recorder, cx: &mut TestAppContext) -> Vec<String> {
    let mut asked = Vec::new();
    for command in [Command::AutoFill, Command::AutoFillPasswords, Command::AutoFill] {
        shell.update(cx, |shell, cx| shell.run(command, "menu", cx));
        let calls = host.take();
        let ours = |call: &&String| call.starts_with("autofill ") || call.starts_with("passwords ");
        asked.push(calls.iter().filter(ours).cloned().collect::<Vec<_>>().join(", "));
    }
    asked
}

/// With a field of the page focused, Contact… and Credit Card… open the field's own dropdown, as a
/// click in it does, and Passwords… one with every login, and none opens Settings.
#[gpui::test]
fn autofill_opens_chromes_dropdown_at_the_focused_field(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, true);
    host.field_focused.set(true);
    let asked = autofill(&shell, &host, cx);
    assert_eq!(asked, ["autofill 11 field", "autofill 11 logins", "autofill 11 field"]);
}

/// With no field of the page focused, or no page, each opens Settings › Passwords on the Space's
/// profile, as Chrome's do.
#[gpui::test]
fn autofill_opens_settings_with_no_field_focused(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, true);
    let asked = autofill(&shell, &host, cx);
    let settings = "passwords 1";
    let tried = |logins| format!("autofill 11 {logins}, {settings}");
    assert_eq!(asked, [tried("field"), tried("logins"), tried("field")]);

    let (shell, host) = window(cx, false);
    assert_eq!(autofill(&shell, &host, cx), [settings; 3]);
}
