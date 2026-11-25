//! A task's helpers (#4102), on a stand-in assistant in a profile folder of the test's own: what
//! a task starts with, the tab a call must name, the limit as ilo's act, and what the task's
//! conversation and card say of its helpers. The helpers' calls come as the task's, as they do:
//! ilo knows one from the other only by what the task's output tells.

use super::kept::{Run, after, beside};
use super::*;
use crate::agents::helpers::NAME_YOUR_TAB;
use crate::agents::runner::{Cli, Failure, Helper, Out};
use crate::agents::task::ilo::{self, HelpersLimit, IloButton, IloKind, IloLine};
use crate::agents::task::{
    Card, Counted, Dot, Line, NEEDS_YOU, State, TaskAction, Trace, TraceItem,
};
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

/// The task's output tells `told` of the helper `id`.
fn told(run: &Run, agent: &str, id: &str, told: Helper, cx: &mut TestAppContext) {
    let id = id.to_string();
    run.said(agent, Out::Helper { id, told }, cx);
}

/// The task's output says it asked for the helper `id`, and its CLI that the helper started.
fn begun(run: &Run, agent: &str, id: &str, cx: &mut TestAppContext) {
    told(run, agent, id, Helper::Asked(format!("Shop {id}")), cx);
    let (id, deep) = (Some(id.to_string()), DEEP.into());
    run.said(agent, Out::Started { id, deep }, cx);
}

/// The task's output says it asked for the helper `id`, which started and whose lines name `tab`.
fn helper(run: &Run, agent: &str, id: &str, tab: u64, cx: &mut TestAppContext) {
    begun(run, agent, id, cx);
    told(run, agent, id, Helper::On(tab), cx);
}

/// The task's CLI says `count` helpers started, each `deep` under the task.
fn started(run: &Run, agent: &str, count: u32, deep: u32, cx: &mut TestAppContext) {
    for _ in 0..count {
        let (id, deep) = (None, deep.into());
        run.said(agent, Out::Started { id, deep }, cx);
    }
}

/// The task's output has `line`, read as its CLI's lines are.
fn line(run: &Run, agent: &str, line: Value, cx: &mut TestAppContext) {
    for out in runner::claude_read(&line, &mut runner::Turn::default()) {
        run.said(agent, out, cx);
    }
}

/// The helpers' lines of a stretch: each one's name, its page, and whether it's under way.
fn helpers_of(trace: &Trace) -> Vec<(&str, Option<u64>, bool)> {
    (trace.items.iter())
        .filter_map(|item| match item {
            TraceItem::Helper { step, name, tab } => Some((name.as_ref(), *tab, step.running)),
            _ => None,
        })
        .collect()
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

/// With the switch as it ships, off, a task starts with only its assistant's own search and
/// what it's told is what it was told before helpers, to the byte. Its CLI saying helpers started
/// changes nothing, as it has none.
#[gpui::test]
fn off_a_task_starts_as_it_did_and_counts_no_helpers(cx: &mut TestAppContext) {
    let run = Run::new("off", cx);
    let agent = run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    assert_eq!(after(&args, "--tools"), Some("WebSearch"));
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
    assert_eq!(after(&args, "--tools"), Some("Agent,WebSearch"));
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
        limit: HelpersLimit::Count,
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
    started(&run, &agent, 2, DEEP, cx);
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

/// What ilo's line and the task's state say once ilo has stopped the task over a helper, by
/// `limit`: Stopped, as the card's own word has it, one line with no button, the turn interrupted
/// and nothing told yet. Each call after is answered as stopped (#4209). The user's next message
/// carries what ilo has to tell, once, and the task works again.
fn stopped_over(run: &Run, agent: &str, limit: HelpersLimit, says: &str, cx: &mut TestAppContext) {
    sent(run, "\"interrupt\"", 1);
    assert_eq!(run.state(agent, cx), State::Stopped);
    let line = limit_line(run, agent, cx).expect("no line");
    assert!(matches!(line.kind, IloKind::Helpers { limit: now, .. } if now == limit));
    assert!(
        line.settled,
        "an ended task's line has nothing to take over"
    );
    let unsettled = IloLine {
        settled: false,
        ..line
    };
    assert_eq!(ilo::buttons(&unsettled), [], "its kind has no button");
    let named = call(&run.agents, agent, "title", json!({ "text": "Probe" }), cx);
    let stopped = ToolResult::failed(tools::Kind::Stopped, crate::agents::STOPPED);
    assert_eq!(named.recv().unwrap(), stopped);
    // The interrupted turn's end changes nothing, and tells the task nothing.
    let interrupted = Failure::Other("Interrupted".into());
    run.said(agent, Out::Ended(Some(interrupted)), cx);
    assert_eq!(run.state(agent, cx), State::Stopped);
    (run.agents).update(cx, |agents, cx| {
        agents.reply(agent, "Go on without it.", cx)
    });
    let was = sent(run, "Go on without it.", 1);
    assert_eq!(was.matches("ilo stopped this task.").count(), 1, "{was}");
    assert!(was.contains(says), "{was}");
    assert_eq!(was.matches("\"interrupt\"").count(), 1, "{was}");
    assert_eq!(run.state(agent, cx), State::Working);
}

/// A helper that starts one of its own stops the task, however few the task started: its CLI
/// says how deep each start is, and deeper than a helper may be isn't the count's matter, which
/// stops a turn.
#[gpui::test]
fn a_helper_that_starts_its_own_stops_the_task(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("deep", cx);
    started(&run, &agent, 1, DEEP, cx);
    assert_eq!(limit_line(&run, &agent, cx), None);
    started(&run, &agent, 1, DEEP + 1, cx);
    let says = "A helper started a helper of its own";
    stopped_over(&run, &agent, HelpersLimit::Depth, says, cx);
}

/// A helper ilo doesn't allow stops the task: one of a kind ilo didn't make, one in the
/// background, one that calls with no start told, or one that called a tool that is the task's
/// alone, each as its output tells it.
#[gpui::test]
fn a_helper_ilo_doesnt_allow_stops_the_task(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("kind", cx);
    run.said(&agent, Out::NotAllowed(HelpersLimit::Kind), cx);
    // A second telling of it, as the start follows the call, adds no line and no interrupt.
    run.said(&agent, Out::NotAllowed(HelpersLimit::Kind), cx);
    let limits = |kind: &&IloKind| matches!(kind, IloKind::Helpers { .. });
    assert_eq!(run.ilo_lines(&agent, cx).iter().filter(limits).count(), 1);
    stopped_over(
        &run,
        &agent,
        HelpersLimit::Kind,
        "a helper ilo doesn't allow",
        cx,
    );

    // A task without helpers has none to stop for, whatever its output says.
    let run = Run::new("kind-off", cx);
    let agent = run.start(Cli::Claude, cx);
    run.said(&agent, Out::NotAllowed(HelpersLimit::Kind), cx);
    run.said(&agent, Out::Kinds { ours: false }, cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(limit_line(&run, &agent, cx), None);
    assert!(!beside(&run.program, "args.2").exists());
}

/// The floor waits for the start. A call for a kind of helper that isn't ilo's, which the
/// program refuses, stops nothing: no start follows it, so it's no helper, no step of the task's
/// and no line of ilo's. The lines are made up, in the shapes a recorded run showed.
#[gpui::test]
fn a_refused_call_for_another_kind_is_nothing(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("refused", cx);
    let asks = json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "toolu_x", "name": "Agent",
          "input": { "subagent_type": "general-purpose", "description": "Stalls price",
                     "prompt": "One shop." } },
    ] } });
    line(&run, &agent, asks, cx);
    let refused = json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": "toolu_x", "is_error": true,
          "content": "Agent type 'general-purpose' has been denied by permission rule." },
    ] } });
    line(&run, &agent, refused, cx);

    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(run.ilo_lines(&agent, cx), []);
    let stretch = |line: &Line| matches!(line, Line::Trace(_));
    assert!(!run.talk(&agent, cx).lines.iter().any(stretch));
    assert!(!card(&run, cx).status.contains("helper"));
    run.said(&agent, Out::Said("Two seats at 9:05.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    assert!(!sent(&run, "Find two seats", 1).contains("interrupt"));
}

/// A start of another kind than ilo's stops the task at the start, waiting for nothing else,
/// and one in the background does too.
#[gpui::test]
fn a_start_of_another_kind_stops_the_task(cx: &mut TestAppContext) {
    for (name, fields) in [
        ("start-kind", json!({ "subagent_type": "general-purpose" })),
        (
            "start-background",
            json!({ "subagent_type": "page-helper", "is_backgrounded": true }),
        ),
    ] {
        let (run, agent) = with_helpers(name, cx);
        let mut start = json!({ "type": "system", "subtype": "task_started",
            "tool_use_id": "toolu_x", "spawn_depth": 1, "is_backgrounded": false,
            "task_type": "local_agent" });
        for (key, value) in fields.as_object().unwrap() {
            start[key] = value.clone();
        }
        line(&run, &agent, start, cx);
        let says = "a helper ilo doesn't allow";
        stopped_over(&run, &agent, HelpersLimit::Kind, says, cx);
    }
}

/// A start two levels under the task is a helper's own helper, which stops the task as that,
/// though the helper is of ilo's kind and the task started only one.
#[gpui::test]
fn a_start_two_levels_under_the_task_stops_it(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("start-deep", cx);
    begun(&run, &agent, "toolu_a", cx);
    assert_eq!(limit_line(&run, &agent, cx), None);
    let start = json!({ "type": "system", "subtype": "task_started", "tool_use_id": "toolu_n",
        "subagent_type": "page-helper", "spawn_depth": 2, "is_backgrounded": false,
        "task_type": "local_agent", "parent_tool_use_id": "toolu_a" });
    line(&run, &agent, start, cx);
    let says = "A helper started a helper of its own";
    stopped_over(&run, &agent, HelpersLimit::Depth, says, cx);
}

/// A helper's line that calls a tool when no start was told for that helper stops the task, so
/// a start told some way ilo doesn't read fails closed: a helper the task asked for whose start
/// never came, and one nobody asked for, as a helper's own helper is.
#[gpui::test]
fn a_helpers_call_without_a_start_stops_the_task(cx: &mut TestAppContext) {
    let calls = |parent: &str| {
        json!({ "type": "assistant", "parent_tool_use_id": parent,
            "message": { "content": [
                { "type": "tool_use", "id": "toolu_c", "name": "mcp__ilo__open_tab",
                  "input": { "url": "https://stalls.example/saturday" } },
            ] } })
    };
    let says = "a helper ilo doesn't allow";
    let (run, agent) = with_helpers("no-start", cx);
    told(
        &run,
        &agent,
        "toolu_a",
        Helper::Asked("Stalls price".into()),
        cx,
    );
    // What it says before it calls stops nothing.
    let says_only = json!({ "type": "assistant", "parent_tool_use_id": "toolu_a",
        "message": { "content": [{ "type": "text", "text": "I'll open the shop." }] } });
    line(&run, &agent, says_only, cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    line(&run, &agent, calls("toolu_a"), cx);
    stopped_over(&run, &agent, HelpersLimit::Kind, says, cx);

    let (run, agent) = with_helpers("no-start-nested", cx);
    begun(&run, &agent, "toolu_a", cx);
    // The helper asks for one of its own, which stops nothing by itself, and its own calls go on.
    let nests = json!({ "type": "assistant", "parent_tool_use_id": "toolu_a",
        "message": { "content": [
            { "type": "tool_use", "id": "toolu_n", "name": "Agent",
              "input": { "subagent_type": "page-helper", "prompt": "One shop." } },
        ] } });
    line(&run, &agent, nests, cx);
    line(&run, &agent, calls("toolu_a"), cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(limit_line(&run, &agent, cx), None);
    line(&run, &agent, calls("toolu_n"), cx);
    stopped_over(&run, &agent, HelpersLimit::Kind, says, cx);
}

/// A helper is counted and shown from its start: before it has done anything its line stands,
/// under way, with the name its task gave it, and the live line and the card count it. The page
/// it opens is its own, never a step of the task's nor a group of them, and has its own step on
/// the card from when it's open. Once its CLI says it ended, its page has no step of its own,
/// so its row and its line say it's done, and the count stays the number that started, as the
/// line of the stretch that's over counts it.
#[gpui::test]
fn a_helper_shows_and_counts_from_its_start(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("start", cx);
    pages(&run, 1);
    begun(&run, &agent, "toolu_a", cx);
    let trace = live(&run, &agent, cx);
    assert_eq!(trace.counted(), (1, Counted::Helpers));
    assert_eq!(helpers_of(&trace), [("Shop toolu_a", None, true)]);
    assert_eq!(card(&run, cx).status, trace.line());
    begun(&run, &agent, "toolu_b", cx);
    let key = live(&run, &agent, cx).key;

    // The first opens its page: its line comes, then the call, which names no tab.
    told(&run, &agent, "toolu_a", Helper::Opens, cx);
    let open = json!({ "url": "https://encore.example/saturday" });
    let opened = call(&run.agents, &agent, "open_tab", open.clone(), cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.steps(), trace.helpers(), trace.key), (0, 2, key));
    let named = [
        ("Shop toolu_a", Some(30), true),
        ("Shop toolu_b", None, true),
    ];
    assert_eq!(helpers_of(&trace), named);
    let made = card(&run, cx);
    assert_eq!(made.status, trace.line());
    let own: Vec<u64> = made.page_steps.iter().map(|(tab, _)| *tab).collect();
    assert_eq!(own, [30]);
    let opened = page_done(&run.agents, &run.browser, opened, shop_tree(), cx);
    assert!(!opened.error, "{}", opened.text);

    // The second is given a page of the task's, and works there.
    told(&run, &agent, "toolu_b", Helper::On(PAGES[0].0), cx);
    let _read = call(
        &run.agents,
        &agent,
        "read_page",
        json!({ "tab": PAGES[0].0 }),
        cx,
    );
    let own = |run: &Run, cx: &mut TestAppContext| {
        let mut tabs: Vec<u64> = (card(run, cx).page_steps.iter())
            .map(|(tab, _)| *tab)
            .collect();
        tabs.sort();
        tabs
    };
    assert_eq!(own(&run, cx), [30, PAGES[0].0]);

    // The first ends: its page has no step, its line isn't under way, and it still counts.
    told(&run, &agent, "toolu_a", Helper::Ended, cx);
    assert_eq!(own(&run, cx), [PAGES[0].0]);
    let trace = live(&run, &agent, cx);
    assert_eq!(trace.counted(), (2, Counted::Helpers));
    assert_eq!(card(&run, cx).status, trace.line());
    // A step of the task's own on that page afterwards is the task's.
    let _own = call(&run.agents, &agent, "screenshot", json!({ "tab": 30 }), cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.steps(), trace.helpers()), (1, 2));
    assert_eq!(own(&run, cx), [30, PAGES[0].0]);

    told(&run, &agent, "toolu_b", Helper::Ended, cx);
    run.said(&agent, Out::Said("Two seats at 9:05.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    let trace = live(&run, &agent, cx);
    assert!(trace.took.is_some() && !trace.running);
    assert_eq!(trace.counted(), (2, Counted::Helpers));
    assert!(helpers_of(&trace).iter().all(|(.., running)| !running));
}

/// A page opened while helpers work is the helper's whose line asked for it, in the order the
/// lines and the calls came, whichever came first; the task's own stays the task's step. Where
/// the order gave a page to the wrong helper, the helper's own line that names it puts it right.
#[gpui::test]
fn a_page_a_helper_opens_is_its_own(cx: &mut TestAppContext) {
    let open = || json!({ "url": "https://encore.example/saturday" });
    // The call comes before the helper's line: its step is the task's until the line comes.
    let (run, agent) = with_helpers("opens-late", cx);
    begun(&run, &agent, "toolu_a", cx);
    let _opened = call(&run.agents, &agent, "open_tab", open(), cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.steps(), trace.helpers()), (1, 1));
    told(&run, &agent, "toolu_a", Helper::Opens, cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.steps(), trace.helpers()), (0, 1));
    assert_eq!(helpers_of(&trace), [("Shop toolu_a", Some(30), true)]);

    // With no helper at work, a page the task opens is its own step.
    let (run, agent) = with_helpers("opens-own", cx);
    let _opened = call(&run.agents, &agent, "open_tab", open(), cx);
    begun(&run, &agent, "toolu_a", cx);
    told(&run, &agent, "toolu_a", Helper::Opens, cx);
    let trace = live(&run, &agent, cx);
    assert_eq!((trace.steps(), trace.helpers()), (1, 1));
    assert_eq!(helpers_of(&trace), [("Shop toolu_a", None, true)]);

    // Two lines, then one call: the order gives its page to the first, and the second's own line
    // that names the page takes it.
    let (run, agent) = with_helpers("opens-named", cx);
    begun(&run, &agent, "toolu_a", cx);
    begun(&run, &agent, "toolu_b", cx);
    told(&run, &agent, "toolu_a", Helper::Opens, cx);
    told(&run, &agent, "toolu_b", Helper::Opens, cx);
    let _opened = call(&run.agents, &agent, "open_tab", open(), cx);
    let pages = |run: &Run, cx: &mut TestAppContext| {
        let trace = live(run, &agent, cx);
        let pages: Vec<Option<u64>> = (helpers_of(&trace).iter())
            .map(|(_, tab, _)| *tab)
            .collect();
        (trace.steps(), pages)
    };
    assert_eq!(pages(&run, cx), (0, vec![Some(30), None]));
    told(&run, &agent, "toolu_b", Helper::On(30), cx);
    assert_eq!(pages(&run, cx), (0, vec![None, Some(30)]));
}

/// A run whose first line lists a kind of helper ilo didn't make ends before its first answer,
/// and the task starts again as one without helpers does, with the message it had. Nothing is
/// said and nothing is kept: no line of ilo's, no entry in the activity log, and the
/// conversation the first run began is removed. A first line with ilo's kind alone changes
/// nothing.
#[gpui::test]
fn a_first_line_with_another_kind_starts_the_task_again_without_helpers(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("kinds", cx);
    let (first_args, first) = run.ran(1);
    assert!(first_args.iter().any(|arg| arg == "--agents"));
    let logged = log(&run.agents, &agent, cx);
    let first_run = (run.agents).read_with(cx, |agents, _| agents.run_of(&agent));
    run.said(&agent, Out::Kinds { ours: false }, cx);

    let (args, again) = run.ran(2);
    assert_eq!(again, first);
    assert_eq!(after(&args, "--tools"), Some("WebSearch"));
    for flag in ["--agents", "--disallowedTools"] {
        assert!(!args.iter().any(|arg| arg == flag), "{flag} in {args:?}");
    }
    let told = std::fs::read_to_string(after(&args, "--append-system-prompt-file").unwrap());
    assert_eq!(told.unwrap(), tools::TASK_INSTRUCTIONS);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(run.ilo_lines(&agent, cx), []);
    assert_eq!(log(&run.agents, &agent, cx), logged);
    // The first run's conversation goes, and the second keeps one under an id of its own.
    let (was, now) = (
        after(&first_args, "--session-id"),
        after(&args, "--session-id"),
    );
    assert!(
        was.is_some() && now.is_some() && was != now,
        "{was:?} {now:?}"
    );
    cx.run_until_parked();
    let purged = std::fs::read_to_string(beside(&run.program, "purge")).unwrap_or_default();
    assert!(purged.contains(was.unwrap()), "{purged}");
    assert!(!purged.contains(now.unwrap()), "{purged}");
    // What the first run still had to say, its end too, isn't taken for the task's.
    for out in [Out::Said("Starting.".into()), Out::Gone(String::new())] {
        let event = runner::Event {
            agent: agent.clone(),
            run: first_run,
            out,
        };
        (run.agents).update(cx, |agents, cx| agents.task_said(event, cx));
    }
    assert_eq!(run.state(&agent, cx), State::Working);
    let said = |line: &Line| matches!(line, Line::Said(_));
    assert!(!run.talk(&agent, cx).lines.iter().any(said));
    // From here it's a task without helpers: its output's word of them is nothing to it.
    started(&run, &agent, MOST + 1, DEEP + 1, cx);
    assert_eq!(run.state(&agent, cx), State::Working);

    // With ilo's kind alone the run goes on as it started.
    let (run, agent) = with_helpers("kinds-ours", cx);
    run.ran(1);
    run.said(&agent, Out::Kinds { ours: true }, cx);
    assert!(!beside(&run.program, "args.2").exists());
    assert_eq!(run.state(&agent, cx), State::Working);
    started(&run, &agent, MOST + 1, DEEP, cx);
    assert!(limit_line(&run, &agent, cx).is_some());
}

/// A first line that comes once the run has answered or done something isn't started again, as
/// that would do it twice: the task is stopped, as for any helper ilo doesn't allow.
#[gpui::test]
fn a_first_line_with_another_kind_after_an_answer_stops_the_task(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("kinds-late", cx);
    run.ran(1);
    run.said(&agent, Out::Said("I'll look at both shops.".into()), cx);
    run.said(&agent, Out::Kinds { ours: false }, cx);
    assert!(!beside(&run.program, "args.2").exists());
    stopped_over(
        &run,
        &agent,
        HelpersLimit::Kind,
        "a helper ilo doesn't allow",
        cx,
    );
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
/// is the task's own step. The helper has started, so its line stands, with no page.
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
    assert_eq!((trace.helpers(), trace.steps()), (2, 1));
    let pages: Vec<Option<u64>> = (helpers_of(&trace).iter())
        .map(|(_, tab, _)| *tab)
        .collect();
    assert_eq!(pages, [None, Some(PAGES[0].0)]);
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
            crate::agents::task::TraceItem::Helper { step, .. } => step.title.clone(),
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
    // Each line has the name its task gave the helper and the page it works on, for its favicon
    // and its word. The name is held in memory alone: no file of the profile's has it.
    let named: Vec<_> = (trace.items.iter())
        .filter_map(|item| match item {
            crate::agents::task::TraceItem::Helper { name, tab, .. } => Some((name.as_ref(), *tab)),
            _ => None,
        })
        .collect();
    let wanted = [
        ("Shop toolu_a", Some(PAGES[0].0)),
        ("Shop toolu_b", Some(PAGES[1].0)),
    ];
    assert_eq!(named, wanted);
    run.said(&agent, Out::Ended(None), cx);
    cx.run_until_parked();
    let files = run.files();
    assert!(!files.is_empty(), "nothing was kept to look in");
    for (path, text) in files {
        assert!(!text.contains("Shop toolu_"), "{}", path.display());
    }
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
            crate::agents::task::TraceItem::Helper { step, .. } => Some(step),
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
                |item| matches!(item, crate::agents::task::TraceItem::Helper { step, .. } if step.running),
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

/// A helper's search is a step of its own, with its words, under its line alone and never as the
/// task's; the thread's one activity entry counts it.
#[gpui::test]
fn a_helpers_search_stands_under_its_own_line(cx: &mut TestAppContext) {
    let (run, agent) = with_helpers("search", cx);
    begun(&run, &agent, "a", cx);
    let search = json!({ "type": "assistant", "parent_tool_use_id": "a", "message": { "content": [
        { "type": "tool_use", "id": "s", "name": "WebSearch", "input": { "query": "stalls on Saturday" } },
    ] } });
    line(&run, &agent, search, cx);
    let said = "Searched the web for “stalls on Saturday”";
    let live = live(&run, &agent, cx);
    let under =
        |item: &TraceItem| matches!(item, TraceItem::Helper { step, .. } if step.title == said);
    let own = |item: &TraceItem| matches!(item, TraceItem::Step(step) if step.title == said);
    assert!(live.items.iter().any(under), "{live:?}");
    assert!(!live.items.iter().any(own), "{live:?}");
    let counted: Vec<String> = (run.agents).read_with(cx, |agents, _| {
        (agents.activity(&agent))
            .filter(|entry| entry.action == tools::SEARCHED)
            .map(|entry| entry.target.to_string())
            .collect()
    });
    assert_eq!(counted, ["1"]);
}
