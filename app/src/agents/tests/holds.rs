//! A tab the user holds from Claude Code, as it closes, moves or loses its agent while a call
//! waits on it (held.rs walks every order of these; this is the host carrying them out).

use super::*;

/// Claude Code's handoff of its tab 20, which waits for the user.
fn handed_off(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, mpsc::Receiver<ToolResult>) {
    let (agents, browser, _) = connected(cx);
    let reason = json!({ "tab": 20, "reason": "Sign in to example.com" });
    let handed = call(&agents, "claude-code", "hand_off", reason, cx);
    assert!(browser.asked.borrow().is_some(), "the handoff didn't ask");
    (agents, browser, handed)
}

fn held_tabs(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<u64> {
    let status = agents.read_with(cx, |agents, _| agents.status());
    (status.iter())
        .flat_map(|status| status.held.iter().map(|(tab, _)| *tab))
        .collect()
}

/// A held tab that closes ends its hold: the handoff's call is answered then, the card goes, and
/// the log says it closed. Hearing of it a second time changes nothing.
#[gpui::test]
fn a_handed_off_tab_that_closes_answers_the_handoff(cx: &mut TestAppContext) {
    let (agents, browser, handed) = handed_off(cx);
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    agents.update(cx, |agents, cx| agents.tab_closed(20, cx));

    let answer = handed
        .try_recv()
        .expect("the handoff still waits on a closed tab");
    assert!(answer.error && answer.text.starts_with("Tab 20 closed"));
    assert_eq!(*browser.asked.borrow(), None);
    assert!(held_tabs(&agents, cx).is_empty(), "the bar still lists it");
    assert!(browser.held.borrow().is_empty(), "the core still holds it");
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "closed");

    let entries = log(&agents, "claude-code", cx).len();
    agents.update(cx, |agents, cx| agents.space_changed(2, cx));
    agents.update(cx, |agents, cx| agents.tab_closed(20, cx));
    assert_eq!(log(&agents, "claude-code", cx).len(), entries);
}

/// The Space's change can come before the tab is gone, as a page with unload handlers closes:
/// a tab in no Space closed, and it's said so, not that it moved.
#[gpui::test]
fn a_held_tab_in_no_space_closed(cx: &mut TestAppContext) {
    let (agents, browser, handed) = handed_off(cx);
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    agents.update(cx, |agents, cx| agents.space_changed(2, cx));

    let answer = handed
        .try_recv()
        .expect("the handoff still waits on a closed tab");
    assert!(answer.text.starts_with("Tab 20 closed"));
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "closed");
}

/// A tab the user took over and moved to their own Space is theirs: the agent's ask for it is
/// answered, the bar stops listing it, and the log says it moved.
#[gpui::test]
fn a_taken_tab_moved_to_the_users_space_answers_the_agents_ask(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    let reason = json!({ "tab": 20, "reason": "To finish the order" });
    let asked = call(&agents, "claude-code", "ask_for_tab", reason, cx);
    assert!(
        asked.try_recv().is_err(),
        "the ask didn't wait for the user"
    );
    assert!(browser.asked.borrow().is_some(), "the ask didn't show");

    for tab in browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .filter(|tab| tab.0 == 20)
    {
        tab.1 = 1;
    }
    agents.update(cx, |agents, cx| agents.space_changed(2, cx));

    let answer = asked
        .try_recv()
        .expect("the ask still waits on a tab that moved");
    assert!(answer.error && answer.text.starts_with("The user moved tab 20"));
    assert_eq!(*browser.asked.borrow(), None);
    assert!(held_tabs(&agents, cx).is_empty(), "the bar still lists it");
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "moved");
}

/// Another Space's change leaves the agent's held tab as it is.
#[gpui::test]
fn another_spaces_change_leaves_a_hold(cx: &mut TestAppContext) {
    let (agents, _, handed) = handed_off(cx);
    agents.update(cx, |agents, cx| agents.space_changed(1, cx));
    agents.update(cx, |agents, cx| agents.space_changed(2, cx));
    assert!(
        handed.try_recv().is_err(),
        "the handoff answered with its tab still there"
    );
    assert_eq!(held_tabs(&agents, cx), [20]);
}

/// The agent's client going lets go of its handoff's call, and takes the card away, but the tab
/// stays the user's until they hand it back.
#[gpui::test]
fn an_agent_gone_during_a_handoff_leaves_the_tab_the_users(cx: &mut TestAppContext) {
    let (agents, browser, handed) = handed_off(cx);
    agents.update(cx, |agents, cx| agents.disconnect(Client::ClaudeCode, cx));

    assert!(
        matches!(handed.try_recv(), Err(mpsc::TryRecvError::Disconnected)),
        "the call of an agent gone was kept or answered"
    );
    assert_eq!(*browser.asked.borrow(), None);
    assert!(browser.held.borrow().contains(&20));
    assert_eq!(held_tabs(&agents, cx), [20]);

    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    assert!(browser.held.borrow().is_empty());
    assert!(held_tabs(&agents, cx).is_empty());
}
