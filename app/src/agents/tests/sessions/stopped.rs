//! A task the user stopped: the last line of its log says so, with the pages it held then.

use super::*;
use crate::agents::task::ilo::{self, IloKind, IloLine};

/// ilo's lines of the task `key`'s conversation that say the user stopped it.
pub(super) fn you_stopped(
    agents: &Entity<Agents>,
    key: &str,
    cx: &mut TestAppContext,
) -> Vec<IloLine> {
    (conversation(agents, key, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Ilo(line) if matches!(line.kind, IloKind::YouStopped { .. }) => Some(line),
            _ => None,
        })
        .collect()
}

/// The user's Stop on Claude Code's task `key`.
pub(super) fn stop(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| {
        agents.task_action(key, TaskAction::Stop, cx)
    });
    assert!(card(agents, key, cx).status.starts_with("Stopped"));
}

/// The user's Stop is the last line of an outside agent's session's log: that they stopped it,
/// with how many pages it held then, said once, and a line with nothing to press.
#[gpui::test]
fn the_users_stop_is_the_last_line_of_a_sessions_log(cx: &mut TestAppContext) {
    let (agents, _, first) = lowered(Tier::Work, cx);
    assert!(you_stopped(&agents, &first, cx).is_empty());
    stop(&agents, &first, cx);

    let talk = conversation(&agents, &first, cx);
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("its log ends with {:?}", talk.lines.last());
    };
    assert_eq!(last.kind, IloKind::YouStopped { pages: 1 });
    assert!(ilo::buttons(last).is_empty(), "{:?}", ilo::buttons(last));
    assert_eq!(you_stopped(&agents, &first, cx).len(), 1);
}

/// A session that held no page when the user stopped it has the line with no count.
#[gpui::test]
fn a_stopped_session_with_no_page_counts_none(cx: &mut TestAppContext) {
    let (agents, browser) = at_work(cx);
    let first = cards(&agents, cx)[0].agent.to_string();
    // Its page left its Space, as one the user moved to their own does.
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    stop(&agents, &first, cx);
    let lines = you_stopped(&agents, &first, cx);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].kind, IloKind::YouStopped { pages: 0 });
}
