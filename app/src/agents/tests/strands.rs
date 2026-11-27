//! A thread's strands (helpers.rs): a run of the thread's assistant of its own, with the thread's
//! tools but its own, counted toward the most a thread starts, whose last message answers the
//! call that started it, and which ends with its thread.

use super::kept::{Run, after};
use super::*;
use crate::agents::runner::{Cli, Failure, Out};
use crate::agents::task::ilo::{HelpersLimit, IloKind};
use crate::agents::task::{Line, State, TraceItem};
use crate::tokens::agents::helpers::MOST;

/// The thread starts a strand for `goal`: its call, which waits on the strand.
fn strand(
    run: &Run,
    thread: &str,
    goal: &str,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    call(
        &run.agents,
        thread,
        tools::STRAND,
        json!({ "goal": goal }),
        cx,
    )
}

/// The thread's answer when a strand's run ended badly for `reason` (AP29).
fn ended(reason: &str) -> String {
    format!("The helper ended: {reason} Check what's already done and finish its part yourself.")
}

/// The thread's name, as its strands' answers say it.
fn name_of(run: &Run, thread: &str, cx: &mut TestAppContext) -> String {
    (run.agents).read_with(cx, |agents, _| {
        agents.task(thread).unwrap().name.to_string()
    })
}

/// The strand `strand`'s run says `out`.
fn said(run: &Run, strand: &str, out: Out, cx: &mut TestAppContext) {
    let event = runner::Event {
        agent: strand.into(),
        run: (run.agents).read_with(cx, |agents, _| agents.strand_run(strand)),
        out,
    };
    (run.agents).update(cx, |agents, cx| agents.task_said(event, cx));
}

fn strands(run: &Run, thread: &str, cx: &mut TestAppContext) -> Vec<String> {
    (run.agents).read_with(cx, |agents, _| agents.strands_of(thread))
}

/// The limits of ilo's lines about the thread's helpers, in its conversation's order.
fn stops(run: &Run, thread: &str, cx: &mut TestAppContext) -> Vec<HelpersLimit> {
    (run.ilo_lines(thread, cx).into_iter())
        .filter_map(|kind| match kind {
            IloKind::Helpers { limit, .. } => Some(limit),
            _ => None,
        })
        .collect()
}

#[test]
fn a_strands_id_is_its_threads_with_a_number() {
    assert_eq!(strand_parent("task-3-1"), Some("task-3"));
    assert_eq!(strand_parent("task-12-40"), Some("task-12"));
    for not in [
        "task-3",
        "task-3-",
        "task--1",
        "task-x-1",
        "task-3-1-2",
        "claude-code-1-1",
    ] {
        assert_eq!(strand_parent(not), None, "{not}");
    }
    assert_eq!(server::side("task-3-1"), Some(server::Side::Inside));
    assert!(is_task("task-3-1"));
}

/// A strand has every tool of its thread's but the thread's own, and the thread has `strand`.
#[test]
fn a_strands_tools_leave_out_the_threads_own() {
    let (thread, strand) = (tools::names("task-3"), tools::names("task-3-1"));
    for own in tools::THE_THREADS_OWN {
        assert!(thread.contains(&own.to_string()), "{own}");
        assert!(!strand.contains(&own.to_string()), "{own}");
        assert!(!tools::has("task-3-1", own), "{own}");
    }
    let shared: Vec<&String> = (thread.iter())
        .filter(|tool| !tools::THE_THREADS_OWN.contains(&tool.as_str()))
        .collect();
    assert_eq!(strand.iter().collect::<Vec<_>>(), shared);
    assert!(strand.contains(&tools::SEARCHED.to_string()));
}

#[gpui::test]
fn a_strand_starts_as_its_threads_assistant_with_its_tools_alone(cx: &mut TestAppContext) {
    let run = Run::new("strand-starts", cx);
    let thread = run.start(Cli::Claude, cx);
    let answer = strand(&run, &thread, "Find the fare on the first result", cx);
    let id = format!("{thread}-1");
    assert_eq!(strands(&run, &thread, cx), [id.clone()]);
    // Its run is the thread's assistant's, with nothing of Claude's own, and its goal its message.
    // It starts while the thread's own run may still be starting, so either may be numbered first.
    let goal = "Find the fare on the first result";
    let (args, first) = [run.ran(1), run.ran(2)]
        .into_iter()
        .find(|(_, first)| first == goal)
        .expect("the strand's run");
    assert_eq!(after(&args, "--tools"), Some(""));
    let allowed = after(&args, "--allowedTools").expect("its tools");
    assert!(allowed.contains("__open_tab"), "{allowed}");
    for own in tools::THE_THREADS_OWN {
        assert!(!allowed.contains(&format!("__{own}")), "{allowed}");
    }
    assert_eq!(first, goal);
    // It may call while it runs, and its call waits on it.
    assert!(answer.try_recv().is_err());
    let none = call(
        &run.agents,
        &format!("{thread}-9"),
        "list_tabs",
        json!({}),
        cx,
    );
    assert_eq!(none.recv().unwrap().text, "This agent isn't connected.");
}

#[gpui::test]
fn a_strands_last_message_answers_its_call(cx: &mut TestAppContext) {
    let run = Run::new("strand-answers", cx);
    let thread = run.start(Cli::Claude, cx);
    let answer = strand(&run, &thread, "Find the fare on the first result", cx);
    let id = format!("{thread}-1");
    said(&run, &id, Out::Said("Looking at the page.".into()), cx);
    said(&run, &id, Out::Said("The fare is 42 euros.".into()), cx);
    assert!(answer.try_recv().is_err());
    said(&run, &id, Out::Ended(None), cx);
    let answered = answer.try_recv().expect("answered");
    assert_eq!(
        (answered.text.as_str(), answered.error),
        ("The fare is 42 euros.", false)
    );
    assert!(strands(&run, &thread, cx).is_empty());
    // Ended, it calls no more, and nothing it said is the thread's.
    let gone = call(&run.agents, &id, "list_tabs", json!({}), cx);
    assert_eq!(gone.recv().unwrap().text, "This agent isn't connected.");
    assert_eq!(run.state(&thread, cx), State::Working);
}

#[gpui::test]
fn a_strand_that_fails_answers_with_what_went_wrong(cx: &mut TestAppContext) {
    let run = Run::new("strand-fails", cx);
    let thread = run.start(Cli::Claude, cx);
    let answer = strand(&run, &thread, "Find the fare", cx);
    said(
        &run,
        &format!("{thread}-1"),
        Out::Ended(Some(Failure::SignedOut)),
        cx,
    );
    let answered = answer.try_recv().expect("answered");
    assert!(answered.error);
    assert_eq!(
        answered.text,
        ended(&format!("{} isn't signed in.", Cli::Claude.name()))
    );
    assert_eq!(run.state(&thread, cx), State::Working);
}

/// A strand's run that started with tools of its assistant's own ends, and its thread is told
/// so in the AP17 sentence, with what to do next (AP29), and goes on.
#[gpui::test]
fn a_strand_with_tools_of_its_own_ends_and_its_thread_is_told(cx: &mut TestAppContext) {
    let run = Run::new("strand-not-our-tools", cx);
    let thread = run.start(Cli::Claude, cx);
    let answer = strand(&run, &thread, "Find the fare", cx);
    said(
        &run,
        &format!("{thread}-1"),
        Out::Refused("WebSearch".into()),
        cx,
    );
    let answered = answer.try_recv().expect("answered");
    let name = name_of(&run, &thread, cx);
    let ap17 = format!("{PRODUCT} stopped {name}: it started with tools {PRODUCT} doesn't allow.");
    assert_eq!((answered.text, answered.error), (ended(&ap17), true));
    assert!(strands(&run, &thread, cx).is_empty());
    assert_eq!(run.state(&thread, cx), State::Working);
}

/// The call past the most a thread starts for a message of the user's is refused, starts
/// nothing, and stops that turn with one line of ilo's (S1).
#[gpui::test]
fn the_strand_past_most_is_refused_and_starts_nothing(cx: &mut TestAppContext) {
    let run = Run::new("strand-most", cx);
    let thread = run.start(Cli::Claude, cx);
    let started: Vec<_> = (0..MOST)
        .map(|_| strand(&run, &thread, "Find the fare", cx))
        .collect();
    assert_eq!(strands(&run, &thread, cx).len(), MOST as usize);
    let past = strand(&run, &thread, "Find the fare", cx);
    let refused = past.try_recv().expect("refused at once");
    assert!(refused.error);
    assert!(
        refused
            .text
            .starts_with(&format!("{PRODUCT} stopped that turn.")),
        "{}",
        refused.text
    );
    assert_eq!(strands(&run, &thread, cx).len(), MOST as usize);
    assert_eq!(stops(&run, &thread, cx), [HelpersLimit::Count]);
    assert!(started.iter().all(|answer| answer.try_recv().is_err()));
}

/// Its thread's turn ends, and each strand of it with it, its call answered as stopped.
#[gpui::test]
fn a_strand_ends_with_its_thread(cx: &mut TestAppContext) {
    let run = Run::new("strand-ends", cx);
    let thread = run.start(Cli::Claude, cx);
    let answer = strand(&run, &thread, "Find the fare", cx);
    let id = format!("{thread}-1");
    (run.agents).update(cx, |agents, cx| agents.stop_task(&thread, cx));
    assert_eq!(run.state(&thread, cx), State::Stopped);
    let answered = answer.try_recv().expect("answered");
    assert_eq!(
        (answered.kind, answered.text.as_str()),
        (Some(tools::Kind::Stopped), STOPPED)
    );
    assert!(strands(&run, &thread, cx).is_empty());
    // What its run says after is no one's.
    said(&run, &id, Out::Said("The fare is 42 euros.".into()), cx);
    assert_eq!(run.state(&thread, cx), State::Stopped);
}

/// A strand that calls a strand of its own (S2), or one of its thread's own tools (S3), stops
/// the thread, with one line of ilo's that says which.
#[gpui::test]
fn a_strand_calling_what_it_hasnt_stops_its_thread(cx: &mut TestAppContext) {
    for (tool, limit) in [
        (tools::STRAND, HelpersLimit::Depth),
        ("finish", HelpersLimit::Kind),
    ] {
        let run = Run::new("strand-not-allowed", cx);
        let thread = run.start(Cli::Claude, cx);
        let answer = strand(&run, &thread, "Find the fare", cx);
        let id = format!("{thread}-1");
        let refused = call(&run.agents, &id, tool, json!({ "goal": "Find more" }), cx);
        assert_eq!(
            refused.recv().unwrap().text,
            format!("There's no tool {tool}.")
        );
        assert_eq!(run.state(&thread, cx), State::Stopped, "{tool}");
        assert_eq!(stops(&run, &thread, cx), [limit], "{tool}");
        assert_eq!(
            answer.try_recv().expect("answered").kind,
            Some(tools::Kind::Stopped)
        );
        assert!(strands(&run, &thread, cx).is_empty());
    }
}

/// A strand's search that names no tab opens a page that's the strand's own: the thread's trace
/// has one line for the strand, under the name its thread gave it, on that page.
#[gpui::test]
fn a_strands_search_opens_a_page_of_its_own(cx: &mut TestAppContext) {
    let run = Run::new("strand-searches", cx);
    let thread = run.start(Cli::Claude, cx);
    let _waits = strand(&run, &thread, "Find the fare", cx);
    let id = format!("{thread}-1");
    let searched = call(
        &run.agents,
        &id,
        tools::SEARCHED,
        json!({ "words": "fares" }),
        cx,
    );
    let answer = page_done(&run.agents, &run.browser, searched, shop_tree(), cx);
    assert!(answer.text.starts_with("Opened tab 30."), "{}", answer.text);
    let space = run.browser.agent_space(&thread);
    assert!(
        run.browser
            .tabs
            .borrow()
            .iter()
            .any(|tab| (tab.0, tab.1) == (30, space))
    );
    let strands: Vec<_> = (run.talk(&thread, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Trace(trace) => Some(trace.items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            TraceItem::Helper { name, tab, .. } => Some((name.to_string(), tab)),
            _ => None,
        })
        .collect();
    assert_eq!(strands, [("Find the fare".to_string(), Some(30))]);
}

/// A strand's start leaves its thread an entry in the activity log that names the strand.
#[gpui::test]
fn a_strands_start_is_its_threads_entry(cx: &mut TestAppContext) {
    let run = Run::new("strand-start-entry", cx);
    let thread = run.start(Cli::Claude, cx);
    let long = "Find the cheapest fare for two adults on the night train to Madrid";
    let _first = strand(&run, &thread, "Find the fare", cx);
    let _second = strand(&run, &thread, &format!("{long}\nThen note the seats."), cx);
    let started: Vec<(String, String, String)> = (run.agents).read_with(cx, |agents, _| {
        (agents.log.iter())
            .filter(|entry| entry.action == tools::STRAND)
            .map(|entry| {
                let did = tools::Did {
                    tool: entry.action.to_string(),
                    target: entry.target.to_string(),
                    ..Default::default()
                };
                let row = tools::step_title(&did, tools::Tense::Done);
                (entry.agent.to_string(), row, entry.strand.to_string())
            })
            .collect()
    });
    // A row for each strand, never a count, and none keeps its goal.
    let row = |strand: u32| {
        let strand = format!("{thread}-{strand}");
        (thread.clone(), "Started a strand".to_string(), strand)
    };
    assert_eq!(started, [row(1), row(2)]);
    let kept = (run.agents).read_with(cx, |agents, _| {
        (agents.log.iter()).any(|entry| entry.target.contains("fare"))
    });
    assert!(!kept, "an entry kept a strand's goal");
}

/// Each entry a strand's call leaves is its thread's, at its thread's access, and names the
/// strand; its thread's own name none.
#[gpui::test]
fn a_strands_entries_name_it_beside_its_thread(cx: &mut TestAppContext) {
    let run = Run::new("strand-entries", cx);
    let thread = run.start(Cli::Claude, cx);
    let _waits = strand(&run, &thread, "Find the fare", cx);
    let id = format!("{thread}-1");
    let by_strand = call(
        &run.agents,
        &id,
        tools::SEARCHED,
        json!({ "words": "fares" }),
        cx,
    );
    page_done(&run.agents, &run.browser, by_strand, shop_tree(), cx);
    let by_thread = call(
        &run.agents,
        &thread,
        tools::SEARCHED,
        json!({ "words": "trains" }),
        cx,
    );
    page_done(&run.agents, &run.browser, by_thread, shop_tree(), cx);
    let searches: Vec<(String, String, String)> = (run.agents).read_with(cx, |agents, _| {
        (agents.log.iter())
            .filter(|entry| entry.action == tools::SEARCHED)
            .map(|entry| {
                let (agent, target) = (entry.agent.to_string(), entry.target.to_string());
                (agent, target, entry.strand.to_string())
            })
            .collect()
    });
    assert_eq!(
        searches,
        [
            (thread.clone(), "“fares”".to_string(), id),
            (thread.clone(), "“trains”".to_string(), String::new()),
        ]
    );
}

/// A strand has its thread's NO_ANSWER to call or say something, on its thread's own timer: one
/// that doesn't ends, its call answered as its thread's turn would be, and the thread goes on.
#[gpui::test]
fn a_strand_that_never_answers_ends_as_its_thread_would(cx: &mut TestAppContext) {
    use crate::agents::task::NO_ANSWER;
    use std::time::Duration;
    let run = Run::new("strand-no-answer", cx);
    let thread = run.start(Cli::Claude, cx);
    let silent = strand(&run, &thread, "Find the fare", cx);
    let heard = strand(&run, &thread, "Find the seats", cx);
    let second = format!("{thread}-2");
    said(&run, &second, Out::Said("Looking at the page.".into()), cx);
    cx.executor()
        .advance_clock(NO_ANSWER - Duration::from_secs(1));
    cx.run_until_parked();
    assert!(silent.try_recv().is_err());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let answered = silent.try_recv().expect("answered");
    let name = name_of(&run, &thread, cx);
    assert_eq!(
        (answered.text, answered.error),
        (ended(&format!("{name} didn't answer.")), true)
    );
    assert_eq!(strands(&run, &thread, cx), [second]);
    assert!(heard.try_recv().is_err());
    assert_eq!(run.state(&thread, cx), State::Working);
}
