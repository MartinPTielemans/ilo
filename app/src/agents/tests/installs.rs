//! Install and Sign In from Claude Code's row (#4100), on stand-ins the tests drive: no installer,
//! no Claude Code and no home of the computer's is touched. What the row says through each state,
//! that a failed or cancelled install leaves it as it was, and the way out once ilo installed it
//! (AS6–AS7). Every value is made up.

use super::*;
use crate::agents::install::OFFLINE;
use crate::agents::ready::{Found, Ready};
use crate::agents::runner::Cli;
use crate::agents::sign_in::{Ended, Heard, Offer, Shown};

/// Claude Code as a look finds it once it's there.
fn found(ready: Ready) -> Found {
    Found {
        ready,
        program: Some("/made/up/home/.local/bin/claude".into()),
        ..Default::default()
    }
}

/// The Agents with Claude Code missing, as Settings › Models found it, saving to `saved` if given.
fn missing(
    saved: Option<PathBuf>,
    cx: &mut TestAppContext,
) -> (Entity<Agents>, Rc<Made>, Arc<Configs>) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, saved, cx);
    look(&agents, cx);
    cx.run_until_parked();
    (agents, browser, configs)
}

/// What the next look finds Claude Code as: missing where `ready` is None.
fn will_find(configs: &Configs, ready: Option<Ready>) {
    let mut looked = configs.looked.lock().unwrap();
    looked.retain(|(cli, _)| *cli != Cli::Claude);
    looked.extend(ready.map(|ready| (Cli::Claude, found(ready))));
}

/// Looks for the assistants, as Settings › Models does with the inside switch on.
fn look(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        agents.look_for_task_agents(cx);
    });
}

fn press(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| agents.sign_in(Cli::Claude, cx));
    cx.run_until_parked();
}

/// Ends the `index`th install started as `end`.
fn end(configs: &Configs, index: usize, end: Ended, cx: &mut TestAppContext) {
    let installs = configs.installs.lock().unwrap();
    installs[index].1.unbounded_send(end).unwrap();
    drop(installs);
    cx.run_until_parked();
}

fn shown(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Shown {
    agents.read_with(cx, |agents, _| agents.assistant_row(Cli::Claude))
}

fn installed_here(agents: &Entity<Agents>, cx: &mut TestAppContext) -> bool {
    agents.read_with(cx, |agents, _| agents.installed_here(Cli::Claude))
}

fn says(line: &str, offer: Offer) -> Shown {
    Shown {
        line: line.into(),
        offer,
    }
}

/// Its program missing, the row offers Claude Code's install and sign-in in one, and Codex's,
/// with no installer road yet, its maker's page as before.
#[gpui::test]
fn a_missing_claude_code_offers_install_and_sign_in(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    let first = says("Installs Anthropic's Claude Code", Offer::Install);
    assert_eq!(shown(&agents, cx), first);
    let codex = agents.read_with(cx, |agents, _| agents.assistant_row(Cli::Codex));
    assert_eq!(codex.offer, Offer::Get);
    // Nothing installs until the press.
    assert!(configs.installs.lock().unwrap().is_empty());
    assert!(!installed_here(&agents, cx));
}

/// The press installs, then signs in with Claude Code's own sign-in once a look finds it.
#[gpui::test]
fn the_install_then_its_sign_in(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    assert_eq!(configs.installs.lock().unwrap().len(), 1);
    let installing = says("Installing Claude Code", Offer::Cancel);
    assert_eq!(shown(&agents, cx), installing);
    // Another press while it installs starts nothing more.
    press(&agents, cx);
    assert_eq!(configs.installs.lock().unwrap().len(), 1);
    assert!(configs.sign_ins.lock().unwrap().is_empty());

    will_find(&configs, Some(Ready::SignedOut));
    end(&configs, 0, Ended::Done, cx);
    let sign_ins = configs.sign_ins.lock().unwrap().len();
    assert_eq!(sign_ins, 1, "no sign-in started after the install");
    let waiting = says(crate::agents::sign_in::CLAUDE_WAITING, Offer::Cancel);
    assert_eq!(shown(&agents, cx), waiting);
    assert!(installed_here(&agents, cx));

    // The sign-in is the program's own and ends as it does.
    will_find(&configs, Some(Ready::SignedIn));
    let heard = configs.sign_ins.lock().unwrap()[0].1.clone();
    heard.unbounded_send(Heard::Ended(Ended::Done)).unwrap();
    cx.run_until_parked();
    assert_eq!(shown(&agents, cx).offer, Offer::SignedIn);
    assert_eq!(configs.installs.lock().unwrap().len(), 1);
}

/// Where the browser is the computer's default, Claude Code's sign-in page opens as a tab here, and
/// the row says so (FR19); where it isn't, the page is in the computer's browser.
#[gpui::test]
fn the_sign_in_waits_in_a_tab_where_the_browser_is_the_default(cx: &mut TestAppContext) {
    use crate::agents::sign_in::{CLAUDE_WAITING, CLAUDE_WAITING_IN_A_TAB};
    let (agents, browser, configs) = missing(None, cx);
    browser.default_browser.set(true);
    press(&agents, cx);
    will_find(&configs, Some(Ready::SignedOut));
    end(&configs, 0, Ended::Done, cx);
    assert_eq!(
        shown(&agents, cx),
        says(CLAUDE_WAITING_IN_A_TAB, Offer::Cancel)
    );

    // Failed, and no longer the default when Try Again signs in again.
    let heard = configs.sign_ins.lock().unwrap()[0].1.clone();
    heard
        .unbounded_send(Heard::Ended(Ended::Failed(None)))
        .unwrap();
    cx.run_until_parked();
    browser.default_browser.set(false);
    press(&agents, cx);
    assert_eq!(shown(&agents, cx), says(CLAUDE_WAITING, Offer::Cancel));
}

/// A sign-in that fails after the install says so as any does (AS5), and its Try Again signs in
/// again, never installs again.
#[gpui::test]
fn try_again_after_the_install_only_signs_in(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    will_find(&configs, Some(Ready::SignedOut));
    end(&configs, 0, Ended::Done, cx);
    let heard = configs.sign_ins.lock().unwrap()[0].1.clone();
    heard
        .unbounded_send(Heard::Ended(Ended::Failed(None)))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(shown(&agents, cx).offer, Offer::TryAgain);
    press(&agents, cx);
    assert_eq!(configs.sign_ins.lock().unwrap().len(), 2);
    assert_eq!(configs.installs.lock().unwrap().len(), 1);
}

/// Cancel ends the install, and the row is as it was before the press.
#[gpui::test]
fn a_cancelled_install_leaves_the_row_as_it_was(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    let before = shown(&agents, cx);
    press(&agents, cx);
    agents.update(cx, |agents, _| agents.cancel_sign_in(Cli::Claude));
    cx.run_until_parked();
    assert_eq!(*configs.installs_cancelled.lock().unwrap(), [Cli::Claude]);
    assert_eq!(shown(&agents, cx), before);
    assert!(configs.sign_ins.lock().unwrap().is_empty());
    assert!(!installed_here(&agents, cx));
}

/// A failed install says why where it knows, from the words (AS4), starts no sign-in, and its
/// Try Again installs again.
#[gpui::test]
fn a_failed_install_says_why_and_tries_again(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    end(&configs, 0, Ended::Failed(Some(OFFLINE.into())), cx);
    let offline = says("You're offline.", Offer::TryInstall);
    assert_eq!(shown(&agents, cx), offline);
    press(&agents, cx);
    assert_eq!(configs.installs.lock().unwrap().len(), 2);
    end(&configs, 1, Ended::Failed(None), cx);
    let failed = says("Couldn't install Claude Code.", Offer::TryInstall);
    assert_eq!(shown(&agents, cx), failed);
    assert!(configs.sign_ins.lock().unwrap().is_empty());
    assert!(!installed_here(&agents, cx));
}

#[gpui::test]
fn an_install_that_cant_start_says_so(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    *configs.install_fails.lock().unwrap() = true;
    press(&agents, cx);
    let failed = says("Couldn't install Claude Code.", Offer::TryInstall);
    assert_eq!(shown(&agents, cx), failed);
}

/// An installer that said it was done but left nothing a look finds has failed, with no sign-in.
#[gpui::test]
fn an_install_that_leaves_no_program_has_failed(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    end(&configs, 0, Ended::Done, cx);
    let failed = says("Couldn't install Claude Code.", Offer::TryInstall);
    assert_eq!(shown(&agents, cx), failed);
    assert!(configs.sign_ins.lock().unwrap().is_empty());
    assert!(!installed_here(&agents, cx));
}

/// One already signed in once it's installed, as its sign-in kept from before, needs none.
#[gpui::test]
fn an_install_found_signed_in_needs_no_sign_in(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    will_find(&configs, Some(Ready::SignedIn));
    end(&configs, 0, Ended::Done, cx);
    assert_eq!(shown(&agents, cx).offer, Offer::SignedIn);
    assert!(configs.sign_ins.lock().unwrap().is_empty());
    assert!(installed_here(&agents, cx));
}

/// AS6 is only for what ilo installed: a Claude Code the user installed has no line, signed in or
/// out.
#[gpui::test]
fn the_installed_line_is_only_for_what_ilo_installed(cx: &mut TestAppContext) {
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    will_find(&configs, Some(Ready::SignedOut));
    let agents = agents(&browser, &configs, None, cx);
    look(&agents, cx);
    cx.run_until_parked();
    assert_eq!(shown(&agents, cx).offer, Offer::SignIn);
    assert!(!installed_here(&agents, cx));
    press(&agents, cx);
    assert!(configs.installs.lock().unwrap().is_empty());
    will_find(&configs, Some(Ready::SignedIn));
    let heard = configs.sign_ins.lock().unwrap()[0].1.clone();
    heard.unbounded_send(Heard::Ended(Ended::Done)).unwrap();
    cx.run_until_parked();
    assert!(!installed_here(&agents, cx));
}

/// AS7: signing out leaves the line, and runs no uninstall; once the program is gone, so is the
/// line, and the row offers the install again.
#[gpui::test]
fn signing_out_keeps_the_installed_line(cx: &mut TestAppContext) {
    let (agents, _, configs) = missing(None, cx);
    press(&agents, cx);
    will_find(&configs, Some(Ready::SignedIn));
    end(&configs, 0, Ended::Done, cx);
    will_find(&configs, Some(Ready::SignedOut));
    agents.update(cx, |agents, cx| agents.sign_out(Cli::Claude, cx));
    cx.run_until_parked();
    assert_eq!(shown(&agents, cx).offer, Offer::SignIn);
    assert!(installed_here(&agents, cx));
    assert_eq!(configs.installs.lock().unwrap().len(), 1);

    will_find(&configs, None);
    look(&agents, cx);
    cx.run_until_parked();
    assert!(!installed_here(&agents, cx));
    assert_eq!(shown(&agents, cx).offer, Offer::Install);
}

/// That ilo installed it is kept with the settings, so the line is there after a relaunch.
#[gpui::test]
fn the_install_is_remembered_across_a_relaunch(cx: &mut TestAppContext) {
    let folder = std::env::temp_dir().join(format!("ilo-installs-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let saved = folder.join("Agents");
    let (agents, browser, configs) = missing(Some(saved.clone()), cx);
    press(&agents, cx);
    will_find(&configs, Some(Ready::SignedIn));
    end(&configs, 0, Ended::Done, cx);
    drop(agents);

    let kept = std::fs::read_to_string(&saved).unwrap();
    let again = super::agents(&browser, &configs, Some(saved), cx);
    again.update(cx, |agents, cx| agents.load(&kept, cx));
    look(&again, cx);
    cx.run_until_parked();
    assert!(installed_here(&again, cx));
    std::fs::remove_dir_all(folder).unwrap();
}

/// The way out (AS6): the line says who installed what, and its button opens the maker's own
/// uninstall page, for Claude Code only.
#[test]
fn the_way_out_is_the_makers_own_page() {
    let line = Cli::Claude.installed_line();
    assert!(line.starts_with(crate::product::PRODUCT), "{line}");
    assert!(line.contains(Cli::Claude.assistant()), "{line}");
    let page = Cli::Claude.uninstall_page().unwrap();
    assert!(page.starts_with("https://code.claude.com/"), "{page}");
    assert_eq!(Cli::Codex.uninstall_page(), None);
}
