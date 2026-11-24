//! A task's helpers (#4102), on a stand-in assistant in a profile folder of the test's own: what
//! a task starts with, the tab a call must name, the limit as ilo's act, and what the task's
//! conversation and card say of its helpers. The helpers' calls come as the task's, as they do:
//! ilo knows one from the other only by what the task's output tells.

use super::kept::{Run, after, beside};
use super::*;
use crate::agents::helpers::NAME_YOUR_TAB;
use crate::agents::runner::{Cli, Failure, Helper, Out};
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};
use crate::agents::task::{Card, Counted, Dot, Line, NEEDS_YOU, State, TaskAction, Trace};
use crate::agents::tier::{self, Tier};
use crate::tokens::agents::helpers::{DEEP, MOST};

/// The task's Space, the first the browser makes, and its helpers' pages.
const SPACE: u64 = 100;
const PAGES: [(u64, &str, &str); 4] = [
    (41, "Seats", "https://tickets.example/seats"),
    (42, "Stalls", "https://stalls.example/saturday"),
    (43, "Sign in", "https://boxoffice.example/sign-in"),
    (44, "Saturday", "https://encore.example/saturday"),
];

/// A task started with helpers on, as a test turns them on for its own.
fn with_helpers(name: &str, cx: &mut TestAppContext) -> (Run, String) {
    let run = Run::new(name, cx);
    (run.agents).update(cx, |agents, _| agents.tasks.helpers = true);
    let agent = run.start(Cli::Claude, cx);
    (run, agent)
}

/// The task has the first `count` of the pages.
fn pages(run: &Run, count: usize) {
    let mut tabs = run.browser.tabs.borrow_mut();
    tabs.retain(|(_, space, ..)| *space != SPACE);
    tabs.extend((PAGES.iter().take(count)).map(|&(tab, title, url)| (tab, SPACE, title, url)));
}

/// The task has `tab`'s outline, whose refs its calls act by.
fn outline(run: &Run, agent: &str, tab: u64, cx: &mut TestAppContext) {
    let answer = call(&run.agents, agent, "snapshot", json!({ "tab": tab }), cx);
    let outline = page_done(&run.agents, &run.browser, answer, shop_tree(), cx);
    assert!(!outline.error, "{}", outline.text);
}

/// The tabs the task's clicks went to, in turn.
fn clicked(run: &Run) -> Vec<u64> {
    let pages = run.browser.pages.borrow();
    let clicks = pages.iter().filter(|page| page.2 == PageAction::Click);
    clicks.map(|page| page.0).collect()
}

/// The task's output says it asked for the helper `id`, whose lines name `tab`.
fn helper(run: &Run, agent: &str, id: &str, tab: u64, cx: &mut TestAppContext) {
    for told in [Helper::Asked, Helper::On(tab)] {
        let id = id.to_string();
        run.said(agent, Out::Helper { id, told }, cx);
    }
}

/// The task's CLI says `count` helpers started, each `deep` under the task.
fn started(run: &Run, agent: &str, count: u32, deep: u32, cx: &mut TestAppContext) {
    for _ in 0..count {
        run.said(agent, Out::Started { deep: deep.into() }, cx);
    }
}

/// What the stand-in was sent, once `needle` is in it `times` times.
fn sent(run: &Run, needle: &str, times: usize) -> String {
    let path = beside(&run.program, "in.1");
    for _ in 0..500 {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.matches(needle).count() >= times {
            return text;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the stand-in wasn't sent {needle} {times} times");
}

/// The stretch under way, as the conversation's live line has it.
fn live(run: &Run, agent: &str, cx: &mut TestAppContext) -> Trace {
    (run.talk(agent, cx).lines.into_iter().rev())
        .find_map(|line| match line {
            Line::Trace(trace) => Some(trace),
            _ => None,
        })
        .expect("no stretch")
}

fn card(run: &Run, cx: &mut TestAppContext) -> Card {
    (run.agents).read_with(cx, |agents, cx| agents.cards(1, cx).remove(0))
}

/// The limit's line in the task's conversation, if ilo stopped it there.
fn limit_line(run: &Run, agent: &str, cx: &mut TestAppContext) -> Option<IloLine> {
    run.talk(agent, cx)
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Ilo(line) if matches!(line.kind, IloKind::Helpers { .. }) => Some(line),
            _ => None,
        })
}

/// With the switch as it ships, off, a task starts with none of its assistant's own tools and
/// what it's told is what it was told before helpers, to the byte. Its CLI saying helpers started
/// changes nothing, as it has none.
#[gpui::test]
fn off_a_task_starts_as_it_did_and_counts_no_helpers(cx: &mut TestAppContext) {
    let run = Run::new("off", cx);
    let agent = run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    assert_eq!(after(&args, "--tools"), Some(""));
    assert!(!args.iter().any(|arg| arg == "--agents"), "{args:?}");
    let told = std::fs::read_to_string(after(&args, "--append-system-prompt-file").unwrap());
    assert_eq!(told.unwrap(), tools::TASK_INSTRUCTIONS);
    started(&run, &agent, MOST + 1, DEEP + 1, cx);
    assert_eq!(limit_line(&run, &agent, cx), None);
    assert_eq!(run.state(&agent, cx), State::Working);
    run.said(&agent, Out::Said("Two seats at 9:05.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    assert!(!sent(&run, "Find two seats", 1).contains("interrupt"));
}

/// With helpers, a task gets the one tool that starts one and is told the two numbers at its
/// start, after everything any task is told.
#[gpui::test]
fn with_helpers_a_task_is_told_the_two_numbers_at_its_start(cx: &mut TestAppContext) {
    let (run, _) = with_helpers("told", cx);
    let (args, _) = run.ran(1);
    assert_eq!(
        after(&args, "--tools"),
        Some(crate::agents::helpers::TOOLS[0])
    );
    assert!(args.iter().any(|arg| arg == "--agents"), "{args:?}");
    let told = std::fs::read_to_string(after(&args, "--append-system-prompt-file").unwrap());
    let told = told.unwrap();
    let rest = told
        .strip_prefix(tools::TASK_INSTRUCTIONS)
        .expect("the task's own instructions");
    assert!(
        rest.contains(&format!("at most {MOST} for each message")),
        "{rest}"
    );
    assert!(rest.contains("a helper may start none"), "{rest}");
    assert!(rest.contains("give every browser call its tab"), "{rest}");
}

/// Name your tab. With helpers, several callers share the task's connection, so "the tab used
/// last" may be another's page: a call without a tab acts only while the task has one tab. With
/// two, a click without one clicks nothing and is told nothing about either page, though one of
/// them is the tab the task used last; with its tab named it goes.
#[gpui::test]
fn with_helpers_a_call_without_a_tab_acts_only_while_the_task_has_one(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("tab", cx);
    pages(&run, 1);
    outline(&run, &agent, PAGES[0].0, cx);
    let click = || json!({ "ref": "e1" });
    let _one = call(&run.agents, &agent, "click", click(), cx);
    assert_eq!(clicked(&run), [PAGES[0].0], "its one tab wasn't clicked");

    pages(&run, 2);
    outline(&run, &agent, PAGES[1].0, cx);
    let refused = call(&run.agents, &agent, "click", click(), cx);
    let refused = refused.try_recv().expect("it wasn't refused at once");
    assert!(refused.error);
    assert_eq!(refused.text, NAME_YOUR_TAB);
    assert_eq!(clicked(&run), [PAGES[0].0], "a click without a tab clicked");
    for (tab, title, url) in &PAGES[..2] {
        let site = url.split('/').nth(2).unwrap();
        for of_a_page in [&tab.to_string()[..], title, url, site] {
            assert!(!refused.text.contains(of_a_page), "it was told {of_a_page}");
        }
    }

    let named = json!({ "ref": "e1", "tab": PAGES[1].0 });
    let _named = call(&run.agents, &agent, "click", named, cx);
    assert_eq!(clicked(&run), [PAGES[0].0, PAGES[1].0]);
}

/// The task's access is read at each act, a helper's too, as its calls are the task's: lowered
/// while helpers work, a helper's next click is refused at once as the task's own would be.
#[gpui::test]
fn a_level_lowered_mid_turn_refuses_a_helpers_next_act(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("level", cx);
    pages(&run, 2);
    helper(&run, &agent, "toolu_a", PAGES[0].0, cx);
    helper(&run, &agent, "toolu_b", PAGES[1].0, cx);
    outline(&run, &agent, PAGES[0].0, cx);
    outline(&run, &agent, PAGES[1].0, cx);
    let click = |tab: u64| json!({ "ref": "e1", "tab": tab });
    let _went = call(&run.agents, &agent, "click", click(PAGES[0].0), cx);
    assert_eq!(clicked(&run), [PAGES[0].0], "a helper's click didn't go");

    (run.agents).update(cx, |agents, cx| agents.set_tier(&agent, Tier::Look, cx));
    let refused = call(&run.agents, &agent, "click", click(PAGES[1].0), cx);
    let refused = refused.try_recv().expect("it wasn't refused at once");
    assert!(refused.error, "{}", refused.text);
    assert!(
        refused.text.contains("ilo stopped this"),
        "{}",
        refused.text
    );
    assert_eq!(clicked(&run), [PAGES[0].0], "it clicked at Look");
    let stops = (run.agents).read_with(cx, |agents, _| agents.stops(&agent));
    assert_eq!(
        stops
            .iter()
            .map(|stop| (stop.tab, stop.tier))
            .collect::<Vec<_>>(),
        [(PAGES[1].0, Tier::Look)]
    );
}

/// The limit is ilo's act. A task may start the most a task starts for each message of the
/// user's; one more and ilo stops the turn with Stop's own request and says so in one line. The
/// task isn't Stopped: once the interrupted turn ends, however its CLI says so, ilo tells it why
/// and it goes on in a new turn.
#[gpui::test]
fn past_the_most_ilo_stops_the_turn_and_the_task_goes_on(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("most", cx);
    // The user's next message starts the count again.
    started(&run, &agent, MOST, DEEP, cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And Sunday?", cx));
    started(&run, &agent, MOST, DEEP, cx);
    assert_eq!(limit_line(&run, &agent, cx), None);

    started(&run, &agent, 1, DEEP, cx);
    let was = sent(&run, "\"interrupt\"", 1);
    assert!(
        !was.contains("ilo stopped that turn"),
        "it was told before its turn ended"
    );
    let line = limit_line(&run, &agent, cx).expect("no line");
    let kind = IloKind::Helpers {
        tab: 0,
        deep: false,
    };
    assert_eq!((&line.kind, line.settled), (&kind, false));
    // It works on: not Stopped, which only the user's stop is.
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(card(&run, cx).dot, Dot::Working);

    // The interrupted turn's end isn't the task's, and what ilo tells it starts the next.
    let interrupted = Failure::Other("Interrupted".into());
    run.said(&agent, Out::Ended(Some(interrupted)), cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    let told = sent(&run, "ilo stopped that turn", 1);
    assert!(
        told.contains(&format!("more than {MOST} helpers")),
        "{told}"
    );
    run.said(&agent, Out::Said("Two seats at 9:05.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    // Ended, there's nothing left to take over.
    assert!(limit_line(&run, &agent, cx).unwrap().settled);
}

/// The limit ends. Past it a second time since the user wrote, ilo stops that turn too and tells
/// the task nothing: the turn ends there as any turn does, however its CLI says the interrupted
/// one did, and the task waits for the user with its one line standing. So a task that keeps
/// starting helpers gets two stops for a message of the user's and no turn after the second. The
/// user's reply starts the count again.
#[gpui::test]
fn past_the_limit_again_the_turn_ends_and_the_task_waits_for_the_user(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("again", cx);
    started(&run, &agent, MOST + 1, DEEP, cx);
    sent(&run, "\"interrupt\"", 1);
    run.said(&agent, Out::Ended(None), cx);
    sent(&run, "ilo stopped that turn", 1);

    started(&run, &agent, 1, DEEP, cx);
    sent(&run, "\"interrupt\"", 2);
    // More that start while that interrupt lands ask for no third.
    started(&run, &agent, 2, DEEP + 1, cx);
    let interrupted = Failure::Other("Interrupted".into());
    run.said(&agent, Out::Ended(Some(interrupted)), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    let lines = run.ilo_lines(&agent, cx);
    let limits = |kind: &&IloKind| matches!(kind, IloKind::Helpers { .. });
    assert_eq!(lines.iter().filter(limits).count(), 1);

    // The user writes: what the task was sent since has its one telling and its two stops.
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "Go on.", cx));
    let was = sent(&run, "Go on.", 1);
    assert_eq!(was.matches("ilo stopped that turn").count(), 1, "{was}");
    assert_eq!(was.matches("\"interrupt\"").count(), 2, "{was}");
    started(&run, &agent, MOST, DEEP, cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    started(&run, &agent, 1, DEEP, cx);
    sent(&run, "\"interrupt\"", 3);
    assert_eq!(run.ilo_lines(&agent, cx).iter().filter(limits).count(), 2);
}

/// A helper that starts one of its own is past the limit however few the task started: its CLI
/// says how deep each start is, and deeper than a helper may be, ilo stops the turn the same way.
#[gpui::test]
fn a_helper_that_starts_its_own_is_stopped_too(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("deep", cx);
    started(&run, &agent, 1, DEEP, cx);
    assert_eq!(limit_line(&run, &agent, cx), None);
    started(&run, &agent, 1, DEEP + 1, cx);
    sent(&run, "\"interrupt\"", 1);
    let line = limit_line(&run, &agent, cx).expect("no line");
    assert!(matches!(line.kind, IloKind::Helpers { deep: true, .. }));
    run.said(&agent, Out::Ended(None), cx);
    let told = sent(&run, "ilo stopped that turn", 1);
    assert!(
        told.contains("A helper started a helper of its own"),
        "{told}"
    );
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// No level lifts the limit, so its line has no step up: Take Over of the page the task stands on,
/// which settles it, and with no page nothing. The line's arrival moves what's under the pointer,
/// so a press that widens the task's access is taken only once the line has stood.
#[gpui::test]
fn the_limits_line_offers_take_over_alone(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("buttons", cx);
    started(&run, &agent, MOST + 1, DEEP, cx);
    let none = limit_line(&run, &agent, cx).expect("no line");
    assert_eq!(ilo::buttons(&none), []);

    let raise = |run: &Run, cx: &mut TestAppContext| {
        (run.agents).update(cx, |agents, cx| {
            agents.task_action(&agent, TaskAction::SetTier(Tier::Full), cx);
            agents.task(&agent).unwrap().tier
        })
    };
    assert_eq!(raise(&run, cx), Tier::Work, "a press under way raised it");
    (run.agents).update(cx, |agents, _| {
        let over = &mut agents.task_mut(&agent).unwrap().helpers.over;
        over[0].appeared -= tier::PROTECT;
    });
    assert_eq!(raise(&run, cx), Tier::Full);
    // A change of access isn't an answer to the limit.
    assert!(!limit_line(&run, &agent, cx).unwrap().settled);

    // A task on a page: its line takes that page over.
    let (run, agent) = with_helpers("buttons-page", cx);
    pages(&run, 1);
    outline(&run, &agent, PAGES[0].0, cx);
    started(&run, &agent, MOST + 1, DEEP, cx);
    let line = limit_line(&run, &agent, cx).expect("no line");
    assert_eq!(ilo::tab(&line.kind), Some(PAGES[0].0));
    assert_eq!(ilo::buttons(&line), [IloButton::TakeOver]);
    (run.agents).update(cx, |agents, cx| {
        agents.task_action(&agent, TaskAction::TakeOver(PAGES[0].0), cx);
    });
    assert!(limit_line(&run, &agent, cx).unwrap().settled);
}

/// A helper's label is only for a tab of the task's own: a number its lines name that isn't one
/// then labels nothing, so naming numbers grows nothing ilo holds, and a step on that tab later
/// is the task's own step.
#[gpui::test]
fn a_helper_is_labelled_only_by_the_tasks_own_tabs(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("label", cx);
    pages(&run, 1);
    // The second page isn't the task's yet, and 20 never is.
    helper(&run, &agent, "toolu_a", PAGES[1].0, cx);
    helper(&run, &agent, "toolu_a", 20, cx);
    helper(&run, &agent, "toolu_b", PAGES[0].0, cx);
    pages(&run, 2);
    let on = |tab: u64| json!({ "tab": tab });
    let _a = call(&run.agents, &agent, "read_page", on(PAGES[1].0), cx);
    let _b = call(&run.agents, &agent, "read_page", on(PAGES[0].0), cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.helpers(), trace.steps()), (1, 1));
}

/// A task's helpers show as a line each, the step each is on, in the place of its first: ilo's own
/// words for the page its calls name. The live line counts the helpers, the card says the live
/// line's own words, and each page has its own step for the strip under it, never another's.
#[gpui::test]
fn a_tasks_helpers_are_a_line_each_and_its_card_says_the_live_line(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("lines", cx);
    pages(&run, 2);
    helper(&run, &agent, "toolu_a", PAGES[0].0, cx);
    helper(&run, &agent, "toolu_b", PAGES[1].0, cx);
    let on = |tab: u64| json!({ "tab": tab });
    let _a = call(&run.agents, &agent, "read_page", on(PAGES[0].0), cx);
    let _b = call(&run.agents, &agent, "scroll", on(PAGES[1].0), cx);
    let _a = call(&run.agents, &agent, "screenshot", on(PAGES[0].0), cx);

    let trace = live(&run, &agent, cx);
    assert_eq!(trace.counted(), (2, Counted::Helpers));
    assert_eq!((trace.steps(), trace.took), (0, None));
    let card = card(&run, cx);
    assert_eq!(
        (card.status.clone(), card.dot),
        (trace.line(), Dot::Working)
    );
    // Each helper's line is its last step, which is its page's own on the card.
    let lines: Vec<_> = (trace.items.iter())
        .map(|item| match item {
            crate::agents::task::TraceItem::Helper(step) => step.title.clone(),
            other => panic!("not a helper's line: {other:?}"),
        })
        .collect();
    let own = |tab: u64| {
        let step = card.page_steps.iter().find(|(each, _)| *each == tab);
        step.map(|(_, step)| step.clone())
            .expect("no step of its own")
    };
    assert_eq!(lines, [own(PAGES[0].0), own(PAGES[1].0)]);
    assert_ne!(lines[0], lines[1]);
    assert!(lines[1].contains("stalls.example"), "{lines:?}");
}

/// A card is made again whenever any task changes, so one of a task with helpers at work costs no
/// fold of the task's conversation, and still says what the conversation's live line says: after a
/// question the task asked and the user answered too, where the stretch under way starts again
/// and counts only the helpers that worked since.
#[gpui::test]
fn a_card_counts_its_tasks_helpers_without_folding_the_conversation(cx: &mut TestAppContext) {
    use crate::agents::task::FOLDS;
    let (run, agent) = with_helpers("card", cx);
    pages(&run, 2);
    helper(&run, &agent, "toolu_a", PAGES[0].0, cx);
    helper(&run, &agent, "toolu_b", PAGES[1].0, cx);
    let on = |tab: u64| json!({ "tab": tab });
    let _a = call(&run.agents, &agent, "read_page", on(PAGES[0].0), cx);
    let _b = call(&run.agents, &agent, "scroll", on(PAGES[1].0), cx);

    let folds = FOLDS.with(|folds| folds.get());
    let made = [card(&run, cx), card(&run, cx)];
    assert_eq!(FOLDS.with(|folds| folds.get()), folds, "a card folded");
    let trace = live(&run, &agent, cx);
    assert_eq!(trace.counted(), (2, Counted::Helpers));
    assert_eq!(made.map(|card| card.status), [trace.line(), trace.line()]);

    let ask = json!({ "question": "Row 12 or row 14?", "answers": ["Row 12", "Row 14"] });
    let _asked = call(&run.agents, &agent, "ask", ask, cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "Row 12", cx));
    let _a = call(&run.agents, &agent, "screenshot", on(PAGES[0].0), cx);
    let trace = live(&run, &agent, cx);
    assert_eq!(
        (trace.running, trace.counted()),
        (true, (1, Counted::Helpers))
    );
    let folds = FOLDS.with(|folds| folds.get());
    assert_eq!(card(&run, cx).status, trace.line());
    assert_eq!(FOLDS.with(|folds| folds.get()), folds, "a card folded");
}

/// When any helper waits on the user, the task's header and its card say it needs them, whatever
/// the others do: one handed its page over for a sign-in while three work on.
#[gpui::test]
fn a_task_with_a_helper_waiting_says_it_needs_you(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("waits", cx);
    pages(&run, 4);
    let ids = ["toolu_a", "toolu_b", "toolu_c", "toolu_d"];
    let mut calls = Vec::new();
    for (id, (tab, ..)) in ids.iter().zip(PAGES) {
        helper(&run, &agent, id, tab, cx);
        let read = json!({ "tab": tab });
        calls.push(call(&run.agents, &agent, "read_page", read, cx));
    }
    assert!(run.talk(&agent, cx).about.ends_with("· Working"));
    assert_eq!(card(&run, cx).dot, Dot::Working);

    let waiting = PAGES[2].0;
    let handoff = json!({ "reason": "Sign in, please.", "tab": waiting });
    calls.push(call(&run.agents, &agent, "hand_off", handoff, cx));
    let talk = run.talk(&agent, cx);
    assert_eq!(talk.about, format!("{} · {NEEDS_YOU}", talk.name));
    let card = card(&run, cx);
    assert_eq!(card.dot, Dot::Waiting);
    assert!(card.status.starts_with(NEEDS_YOU), "{}", card.status);
    // Its line says the page is handed over, with nothing under way there; the others' go on.
    let trace = live(&run, &agent, cx);
    let helpers: Vec<_> = (trace.items.iter())
        .filter_map(|item| match item {
            crate::agents::task::TraceItem::Helper(step) => Some(step),
            _ => None,
        })
        .collect();
    assert_eq!(helpers.len(), ids.len());
    assert_eq!(
        helpers.iter().map(|step| step.running).collect::<Vec<_>>(),
        [true, true, false, true]
    );
    assert_eq!(helpers[2].title, "Handed you boxoffice.example");
}

/// A turn that ends with helpers' steps unanswered leaves no helper's line under way: the stretch
/// is over, still counts its helpers, and each line stays as the step it was on.
#[gpui::test]
fn a_turn_that_ends_leaves_no_helpers_line_under_way(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("ends", cx);
    pages(&run, 2);
    helper(&run, &agent, "toolu_a", PAGES[0].0, cx);
    helper(&run, &agent, "toolu_b", PAGES[1].0, cx);
    let on = |tab: u64| json!({ "tab": tab });
    let _a = call(&run.agents, &agent, "read_page", on(PAGES[0].0), cx);
    let _b = call(&run.agents, &agent, "read_page", on(PAGES[1].0), cx);
    let under_way = |trace: &Trace| {
        (trace.items.iter())
            .filter(
                |item| matches!(item, crate::agents::task::TraceItem::Helper(step) if step.running),
            )
            .count()
    };
    assert_eq!(under_way(&live(&run, &agent, cx)), 2);

    run.said(&agent, Out::Said("Two seats at 9:05.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    let trace = live(&run, &agent, cx);
    assert!(trace.took.is_some() && !trace.running);
    assert_eq!(trace.counted(), (2, Counted::Helpers));
    assert_eq!(under_way(&trace), 0);
    assert_eq!(card(&run, cx).page_steps, []);
}
