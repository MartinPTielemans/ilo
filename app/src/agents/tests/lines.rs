//! ilo's lines of what happened to a ⌘T task (#4086 step 3), each made by the app from the task's
//! own state and read here as its conversation has them: a refusal with its count, a step handed
//! to the user, a tab asked back and a change of access, with their buttons doing what they say.

use super::kept::Run;
use super::*;
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};
use crate::agents::task::{Line, TaskAction};
use crate::agents::tier::{Tier, Why};

/// The task's Space, the first the browser makes, and its pages.
const SPACE: u64 = 100;
const PAGES: [(u64, &str, &str); 2] = [
    (41, "Seats", "https://tickets.example/seats"),
    (43, "Sign in", "https://boxoffice.example/sign-in"),
];

fn started(name: &str, pages: usize, cx: &mut TestAppContext) -> (Run, String) {
    let run = Run::new(name, cx);
    let agent = run.start(crate::agents::runner::Cli::Claude, cx);
    let mut tabs = run.browser.tabs.borrow_mut();
    tabs.retain(|(_, space, ..)| *space != SPACE);
    tabs.extend((PAGES.iter().take(pages)).map(|&(tab, title, url)| (tab, SPACE, title, url)));
    drop(tabs);
    (run, agent)
}

/// ilo's lines in the task's conversation, in its order.
fn lines(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<IloLine> {
    (run.talk(agent, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Ilo(line) => Some(line),
            _ => None,
        })
        .collect()
}

fn act(run: &Run, agent: &str, action: TaskAction, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| _ = agents.task_action(agent, action, cx));
    cx.run_until_parked();
}

/// A refusal is ilo's line with its count, the same refusal again counting on it; a change of
/// access is a line of its own, which settles the refusals before it. Its step up is the one that
/// lets the act through, as only the user's press sets a task's access.
#[gpui::test]
fn a_refusal_counts_on_its_line_and_a_change_of_access_settles_it(cx: &mut TestAppContext) {
    let (run, agent) = started("refusal", 1, cx);
    let tab = PAGES[0].0;
    let answer = call(&run.agents, &agent, "snapshot", json!({ "tab": tab }), cx);
    assert!(!page_done(&run.agents, &run.browser, answer, shop_tree(), cx).error);
    (run.agents).update(cx, |agents, cx| agents.set_tier(&agent, Tier::Look, cx));
    let click = || json!({ "ref": "e1", "tab": tab });
    for _ in 0..2 {
        let refused = call(&run.agents, &agent, "click", click(), cx);
        assert!(refused.try_recv().expect("not refused at once").error);
    }
    let [changed, stopped] = &lines(&run, &agent, cx)[..] else {
        panic!("{:?}", lines(&run, &agent, cx));
    };
    assert_eq!(
        changed.kind,
        IloKind::TierChanged {
            from: Tier::Work,
            to: Tier::Look
        }
    );
    assert!(ilo::buttons(changed).is_empty());
    let IloKind::Stopped {
        tab: at,
        why,
        tier,
        count,
        ..
    } = &stopped.kind
    else {
        panic!("{stopped:?}");
    };
    assert_eq!((*at, *why, *tier, *count), (tab, Why::Act, Tier::Look, 2));
    assert_eq!(
        ilo::buttons(stopped),
        [IloButton::TakeOver, IloButton::Raise(Tier::Work)]
    );

    (run.agents).update(cx, |agents, cx| agents.set_tier(&agent, Tier::Work, cx));
    let after = lines(&run, &agent, cx);
    assert_eq!(after.len(), 3, "{after:?}");
    assert!(after[1].settled, "the refusal still offers its step up");
    assert_eq!(
        after[2].kind,
        IloKind::TierChanged {
            from: Tier::Look,
            to: Tier::Work
        }
    );
}

/// A step handed to the user is its tab's line, Hand Back and Not Now, with the page named only
/// among several (O1). Hand Back gives the tab back and answers the call that waited, and the
/// line keeps its words without its buttons.
#[gpui::test]
fn a_step_handed_over_is_its_tabs_line_until_hand_back(cx: &mut TestAppContext) {
    let (run, agent) = started("yours", 1, cx);
    let tab = PAGES[0].0;
    let reason = json!({ "tab": tab, "reason": "Sign in, please." });
    let handed = call(&run.agents, &agent, "hand_off", reason, cx);
    let [line] = &lines(&run, &agent, cx)[..] else {
        panic!("{:?}", lines(&run, &agent, cx));
    };
    assert_eq!(line.kind, IloKind::Yours { tab, site: None });
    assert_eq!(
        ilo::drawn(line, None),
        [IloButton::HandBack, IloButton::NotNow]
    );
    assert!(handed.try_recv().is_err(), "the hand-off was answered");

    act(&run, &agent, TaskAction::HandBack(tab), cx);
    assert!(!handed.recv().unwrap().error);
    assert!(!run.browser.held.borrow().contains(&tab));
    let [line] = &lines(&run, &agent, cx)[..] else {
        panic!("{:?}", lines(&run, &agent, cx));
    };
    assert!(line.settled, "its buttons stayed once the tab came back");
}

/// O1 and O2 on the real path: with two pages the step's line names its site, and offers Show
/// Page until that page is beside the conversation.
#[gpui::test]
fn a_step_on_one_of_two_pages_names_its_site(cx: &mut TestAppContext) {
    let (run, agent) = started("two-pages", 2, cx);
    let tab = PAGES[1].0;
    let reason = json!({ "tab": tab, "reason": "Sign in, please." });
    let _handed = call(&run.agents, &agent, "hand_off", reason, cx);
    let [line] = &lines(&run, &agent, cx)[..] else {
        panic!("{:?}", lines(&run, &agent, cx));
    };
    let site = Some("boxoffice.example".into());
    assert_eq!(line.kind, IloKind::Yours { tab, site });
    assert_eq!(
        ilo::drawn(line, Some(PAGES[0].0)),
        [IloButton::ShowPage, IloButton::NotNow]
    );
    assert_eq!(
        ilo::drawn(line, Some(tab)),
        [IloButton::HandBack, IloButton::NotNow]
    );
}

/// Not Now leaves the tab the user's and tells the agent's call that waited they won't do the
/// step now; its line settles.
#[gpui::test]
fn not_now_keeps_the_tab_and_answers_the_wait(cx: &mut TestAppContext) {
    let (run, agent) = started("not-now", 1, cx);
    let tab = PAGES[0].0;
    let reason = json!({ "tab": tab, "reason": "Sign in, please." });
    let handed = call(&run.agents, &agent, "hand_off", reason, cx);
    let key = lines(&run, &agent, cx)[0].key;
    act(&run, &agent, TaskAction::NotNow(key), cx);
    let answer = handed.recv().unwrap();
    assert!(
        answer.error && answer.text.contains("won't do the step"),
        "{answer:?}"
    );
    assert!(
        run.browser.held.borrow().contains(&tab),
        "the tab went back"
    );
    assert!(lines(&run, &agent, cx)[0].settled);
}

/// An agent asking for a tab the user took is ilo's line, Hand Back and Not Now; Not Now keeps
/// the tab as Keep did, and Hand Back gives it back.
#[gpui::test]
fn a_tab_asked_back_is_its_own_line(cx: &mut TestAppContext) {
    let (run, agent) = started("tab-back", 1, cx);
    let tab = PAGES[0].0;
    let ask = || json!({ "tab": tab, "reason": "To pick the seats." });
    (run.agents).update(cx, |agents, cx| agents.take_over(&agent, tab, cx));
    let asked = call(&run.agents, &agent, "ask_for_tab", ask(), cx);
    let [line] = &lines(&run, &agent, cx)[..] else {
        panic!("{:?}", lines(&run, &agent, cx));
    };
    assert_eq!(line.kind, IloKind::TabBack { tab });
    assert_eq!(ilo::buttons(line), [IloButton::HandBack, IloButton::NotNow]);
    act(&run, &agent, TaskAction::NotNow(line.key), cx);
    let kept = asked.recv().unwrap();
    assert!(kept.error && kept.text.contains("keeps tab"), "{kept:?}");
    assert!(run.browser.held.borrow().contains(&tab));

    let asked = call(&run.agents, &agent, "ask_for_tab", ask(), cx);
    let shown = lines(&run, &agent, cx);
    assert_eq!(shown.len(), 2, "{shown:?}");
    act(&run, &agent, TaskAction::HandBack(tab), cx);
    assert!(!asked.recv().unwrap().error);
    assert!(lines(&run, &agent, cx).iter().all(|line| line.settled));
}

/// An ended task's lines keep their words and offer nothing: there's nothing left to hand back.
#[gpui::test]
fn an_ended_tasks_lines_have_no_buttons(cx: &mut TestAppContext) {
    let (run, agent) = started("ended", 1, cx);
    let tab = PAGES[0].0;
    let reason = json!({ "tab": tab, "reason": "Sign in, please." });
    let _handed = call(&run.agents, &agent, "hand_off", reason, cx);
    act(&run, &agent, TaskAction::Stop, cx);
    let shown = lines(&run, &agent, cx);
    assert!(
        (shown.iter()).any(|line| matches!(line.kind, IloKind::Yours { .. })),
        "{shown:?}"
    );
    assert!(
        shown.iter().all(|line| ilo::buttons(line).is_empty()),
        "{shown:?}"
    );
}
