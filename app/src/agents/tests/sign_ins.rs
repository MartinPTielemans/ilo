//! Signing in to a task CLI and out of it from its row (#4100), on stand-ins the tests drive: what
//! the row says, where the CLI's page opens and when it closes, and that no agent can reach it.
//! Every value is made up.

use super::*;
use crate::agents::ready::{Found, Ready};
use crate::agents::runner::Cli;
use crate::agents::sign_in::{Ended, Heard, Row, quiet};

/// Codex's page, with the state a real one carries.
const PAGE: &str = "https://auth.example.test/oauth/authorize?state=made-up-state-0002";

/// What a CLI that's installed and signed out answers when looked for, and with `signed_in`, in.
fn found(ready: Ready) -> Found {
    Found {
        ready,
        program: Some("/made/up/bin/codex".into()),
        ..Default::default()
    }
}

/// The Agents with Codex installed and signed out, as Settings › Agents found it, saving their
/// settings to `saved` if given.
fn signed_out(
    saved: Option<PathBuf>,
    cx: &mut TestAppContext,
) -> (Entity<Agents>, Rc<Made>, Arc<Configs>) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let looked = (Cli::Codex, found(Ready::SignedOut));
    configs.looked.lock().unwrap().push(looked);
    let agents = agents(&browser, &configs, saved, cx);
    agents.update(cx, |agents, cx| agents.look_for_task_agents(cx));
    cx.run_until_parked();
    (agents, browser, configs)
}

/// Says `heard` from the `index`th sign-in started.
fn hear(configs: &Configs, index: usize, heard: Heard, cx: &mut TestAppContext) {
    let sign_ins = configs.sign_ins.lock().unwrap();
    sign_ins[index].1.unbounded_send(heard).unwrap();
    drop(sign_ins);
    cx.run_until_parked();
}

/// The tab the sign-in's page opened in.
fn page(browser: &Made) -> u64 {
    browser.sign_in_pages.borrow()[0].1
}

fn row(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Row {
    agents.read_with(cx, |agents, _| agents.sign_in_row(Cli::Codex))
}

fn sign_in(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| agents.sign_in(Cli::Codex, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn codex_signs_in_in_a_private_window_that_closes_as_it_ends(cx: &mut TestAppContext) {
    let (agents, browser, configs) = signed_out(None, cx);
    sign_in(&agents, cx);
    assert_eq!(row(&agents, cx), Row::SigningIn);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    // The page is in a private window's tab, which no log line names the address of.
    let tab = page(&browser);
    assert_eq!(*browser.sign_in_pages.borrow(), [(PAGE.to_string(), tab)]);
    assert!(quiet(tab));
    // Codex says it's done, and is signed in when looked at again.
    configs.looked.lock().unwrap()[0].1 = found(Ready::SignedIn);
    hear(&configs, 0, Heard::Ended(Ended::Done), cx);
    assert_eq!(*browser.sign_in_closed.borrow(), [(tab, tab - 600)]);
    assert!(!quiet(tab));
    assert_eq!(row(&agents, cx), Row::Idle);
    let ready = agents.read_with(cx, |agents, _| agents.found(Cli::Codex));
    assert_eq!(ready, Ready::SignedIn);
}

#[gpui::test]
fn a_sign_in_under_way_is_the_only_one(cx: &mut TestAppContext) {
    let (agents, _, configs) = signed_out(None, cx);
    sign_in(&agents, cx);
    sign_in(&agents, cx);
    assert_eq!(configs.sign_ins.lock().unwrap().len(), 1);
    // Nor does Sign Out run while it does.
    agents.update(cx, |agents, cx| agents.sign_out(Cli::Codex, cx));
    cx.run_until_parked();
    assert!(configs.signed_out.lock().unwrap().is_empty());
}

#[gpui::test]
fn a_failed_sign_in_says_why_on_its_row_until_the_next(cx: &mut TestAppContext) {
    let (agents, browser, configs) = signed_out(None, cx);
    sign_in(&agents, cx);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    let said = Some("Your workspace doesn't allow Codex".to_string());
    hear(&configs, 0, Heard::Ended(Ended::Failed(said.clone())), cx);
    assert_eq!(row(&agents, cx), Row::Failed(said));
    let tab = page(&browser);
    assert_eq!(*browser.sign_in_closed.borrow(), [(tab, tab - 600)]);
    // Try Again.
    sign_in(&agents, cx);
    assert_eq!(row(&agents, cx), Row::SigningIn);
}

#[gpui::test]
fn cancel_ends_the_sign_in_and_closes_its_page(cx: &mut TestAppContext) {
    let (agents, browser, configs) = signed_out(None, cx);
    sign_in(&agents, cx);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    agents.update(cx, |agents, _| agents.cancel_sign_in(Cli::Codex));
    cx.run_until_parked();
    assert_eq!(*configs.sign_ins_cancelled.lock().unwrap(), [Cli::Codex]);
    let tab = page(&browser);
    assert_eq!(*browser.sign_in_closed.borrow(), [(tab, tab - 600)]);
    assert_eq!(row(&agents, cx), Row::Idle);
}

#[gpui::test]
fn closing_the_sign_ins_page_ends_it(cx: &mut TestAppContext) {
    let (agents, browser, configs) = signed_out(None, cx);
    sign_in(&agents, cx);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    let tab = page(&browser);
    agents.update(cx, |agents, cx| agents.tab_closed(tab, cx));
    cx.run_until_parked();
    assert_eq!(*configs.sign_ins_cancelled.lock().unwrap(), [Cli::Codex]);
    assert!(!quiet(tab));
    // It's closed already.
    assert!(browser.sign_in_closed.borrow().is_empty());
    assert_eq!(row(&agents, cx), Row::Idle);
}

#[gpui::test]
fn a_sign_in_with_nowhere_to_open_its_page_ends(cx: &mut TestAppContext) {
    let (agents, browser, configs) = signed_out(None, cx);
    browser.no_sign_in_page.set(true);
    sign_in(&agents, cx);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    assert_eq!(*configs.sign_ins_cancelled.lock().unwrap(), [Cli::Codex]);
    assert_eq!(row(&agents, cx), Row::Idle);
}

#[gpui::test]
fn a_sign_in_that_cant_start_says_so(cx: &mut TestAppContext) {
    let (agents, _, configs) = signed_out(None, cx);
    *configs.sign_in_fails.lock().unwrap() = true;
    sign_in(&agents, cx);
    assert_eq!(row(&agents, cx), Row::Failed(None));
}

#[gpui::test]
fn sign_out_runs_the_clis_own_and_looks_again(cx: &mut TestAppContext) {
    let (agents, _, configs) = signed_out(None, cx);
    configs.looked.lock().unwrap()[0].1 = found(Ready::SignedIn);
    agents.update(cx, |agents, cx| agents.look_for_task_agents(cx));
    cx.run_until_parked();
    configs.looked.lock().unwrap()[0].1 = found(Ready::SignedOut);
    agents.update(cx, |agents, cx| {
        agents.sign_out(Cli::Codex, cx);
        assert_eq!(agents.sign_in_row(Cli::Codex), Row::SigningOut);
    });
    cx.run_until_parked();
    assert_eq!(*configs.signed_out.lock().unwrap(), [Cli::Codex]);
    assert_eq!(row(&agents, cx), Row::Idle);
    let ready = agents.read_with(cx, |agents, _| agents.found(Cli::Codex));
    assert_eq!(ready, Ready::SignedOut);

    let words = Some("Couldn't remove the saved sign-in".to_string());
    *configs.sign_out_answers.lock().unwrap() = Some(Err(words.clone()));
    agents.update(cx, |agents, cx| agents.sign_out(Cli::Codex, cx));
    cx.run_until_parked();
    assert_eq!(row(&agents, cx), Row::Failed(words));
}

#[gpui::test]
fn nothing_a_sign_in_carries_is_saved(cx: &mut TestAppContext) {
    let folder = std::env::temp_dir().join(format!("ilo-sign-ins-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let saved = folder.join("Agents");
    let (agents, _, configs) = signed_out(Some(saved.clone()), cx);
    sign_in(&agents, cx);
    hear(&configs, 0, Heard::Page(PAGE.into()), cx);
    let said = Some("Workspace made-up-workspace-name refused".to_string());
    hear(&configs, 0, Heard::Ended(Ended::Failed(said)), cx);
    // Something else saves the settings after.
    agents.update(cx, |agents, cx| {
        agents.set_default_task_agent(Cli::Codex, cx)
    });
    cx.run_until_parked();
    let kept = std::fs::read_to_string(&saved).unwrap();
    for carried in ["auth.example.test", "made-up-state", "made-up-workspace"] {
        assert!(
            !kept.contains(carried),
            "the saved settings keep {carried}: {kept}"
        );
    }
    std::fs::remove_dir_all(folder).unwrap();
}

/// No agent can start, read or end a sign-in: the MCP server has no tool for it, and nothing an
/// agent's call runs reaches the Agents' sign-in.
#[test]
fn no_agent_reaches_a_sign_in() {
    for agent in ["claude-code", "codex", "task-1"] {
        for name in crate::agents::tools::names(agent) {
            for word in ["login", "logout", "sign_in", "sign_out", "signin", "auth"] {
                assert!(!name.contains(word), "{agent} has the tool {name}");
            }
        }
    }
    let reached = [
        ("agents.rs", include_str!("../../agents.rs")),
        ("tools.rs", include_str!("../tools.rs")),
        ("server.rs", include_str!("../server.rs")),
        ("request.rs", include_str!("../request.rs")),
        ("command.rs", include_str!("../command.rs")),
        ("cli.rs", include_str!("../cli.rs")),
        ("task.rs", include_str!("../task.rs")),
    ];
    for (file, source) in reached {
        // A card's Sign in to … comes only from the user's press, through the shell.
        if file != "task.rs" {
            for action in ["TaskAction::SignIn", "TaskAction::CancelSignIn"] {
                assert!(!source.contains(action), "{file} sends {action}");
            }
        }
        let calls = [
            "sign_in(Cli::",
            "sign_in(cli,",
            "sign_out(Cli::",
            "sign_out(cli,",
        ];
        for call in
            calls
                .into_iter()
                .chain(["cancel_sign_in(", "sign_in::start(", "start_sign_in("])
        {
            assert!(!source.contains(call), "{file} calls {call}");
        }
    }
}
