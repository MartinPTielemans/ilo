//! The assistant's own web search, which its provider runs: each search is a step of the task's
//! trace with its words, held in memory alone, and counts among the stretch's steps. The card
//! never says one. The thread's one activity entry counts them and never holds their words.

use super::kept::Run;
use super::*;
use crate::agents::runner::{Cli, Out};
use crate::agents::task::{Counted, Line, Trace, TraceItem};
use crate::agents::tools::{self, SEARCHED};

/// The task's stretches, as its conversation has them.
fn stretches(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<Trace> {
    (run.talk(agent, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Trace(trace) => Some(trace),
            _ => None,
        })
        .collect()
}

/// The titles of the task's steps, oldest first.
pub(super) fn titles(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<String> {
    (stretches(run, agent, cx).into_iter())
        .flat_map(|trace| trace.items)
        .filter_map(|item| match item {
            TraceItem::Step(step) => Some(step.title.to_string()),
            _ => None,
        })
        .collect()
}

/// The thread's search entries in the activity log, each with every word it holds and its line
/// as Settings › Agents and the past task's row say it.
fn entries(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<(String, String)> {
    (run.agents).read_with(cx, |agents, _| {
        (agents.activity(agent))
            .filter(|entry| entry.action == SEARCHED)
            .map(|entry| {
                let held = [
                    &entry.action,
                    &entry.site,
                    &entry.target,
                    &entry.refused,
                    &entry.by,
                    &entry.url,
                    &entry.said,
                    &entry.tier,
                    &entry.notes,
                ];
                let did = tools::Did {
                    tool: entry.action.to_string(),
                    site: entry.site.to_string(),
                    target: entry.target.to_string(),
                    ..Default::default()
                };
                let held: Vec<&str> = held.iter().map(|each| each.as_ref()).collect();
                (tools::step_title(&did, tools::Tense::Done), held.join(" "))
            })
            .collect()
    })
}

#[gpui::test]
fn each_search_is_a_step_with_its_words_and_one_entry_counts_them(cx: &mut TestAppContext) {
    let run = Run::new("searches", cx);
    let agent = run.start(Cli::Claude, cx);
    // Its words, one line, whatever breaks its CLI sent in them.
    run.said(
        &agent,
        Out::Searched("sleeper\n carriages   Madrid".into()),
        cx,
    );
    let first = "Searched the web for “sleeper carriages Madrid”";
    assert_eq!(titles(&run, &agent, cx), [first]);
    let made = entries(&run, &agent, cx);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!(made[0].0, "Searched the web");

    run.said(&agent, Out::Searched("ferry to Tangier".into()), cx);
    let second = "Searched the web for “ferry to Tangier”";
    assert_eq!(titles(&run, &agent, cx), [first, second]);
    // Still one entry for the thread, with the app's own count.
    let made = entries(&run, &agent, cx);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!(made[0].0, "Searched the web 2 times");
    for word in ["sleeper", "carriages", "Madrid", "ferry", "Tangier"] {
        assert!(!made[0].1.contains(word), "{word} in the entry: {made:?}");
    }
    // Each is a step its stretch counts.
    let stretch = stretches(&run, &agent, cx).pop().expect("no stretch");
    assert_eq!(stretch.counted(), (2, Counted::Steps));

    // The card says none of it: no step and no word. (Its title is the user's ask, of Lisbon.)
    let card = (run.agents).read_with(cx, |agents, cx| agents.cards(1, cx).remove(0));
    assert!(card.steps.is_empty(), "{:?}", card.steps);
    for said in [
        &card.status,
        &card.title,
        &card.line.clone().unwrap_or_default(),
    ] {
        for word in ["Searched", "Madrid", "Tangier"] {
            assert!(!said.contains(word), "{word} on the card: {said}");
        }
    }
}

/// A search's words are cut as a thought's are.
#[gpui::test]
fn a_searchs_words_are_cut_as_a_thoughts(cx: &mut TestAppContext) {
    let run = Run::new("search-cut", cx);
    let agent = run.start(Cli::Claude, cx);
    run.said(&agent, Out::Searched("a".repeat(5000)), cx);
    let titles = titles(&run, &agent, cx);
    assert_eq!(titles.len(), 1);
    let words = titles[0]
        .trim_start_matches("Searched the web for “")
        .trim_end_matches("”");
    assert_eq!(words.chars().count(), 4000);
    assert!(words.ends_with('…'), "{words}");
}

/// A task's own call by the search's name, or by a name no tool has, is answered as no tool and
/// makes no step and no entry: a search is a step only as its assistant's provider ran it.
#[gpui::test]
fn a_tasks_call_by_the_searchs_name_is_no_tool(cx: &mut TestAppContext) {
    let run = Run::new("search-called", cx);
    let agent = run.start(Cli::Claude, cx);
    for (id, tool) in [(1, SEARCHED), (2, "search_the_web_twice")] {
        let (reply, answer) = mpsc::channel();
        let call = Call {
            id,
            agent: agent.clone(),
            connection: 1,
            tool: tool.into(),
            arguments: json!({ "query": "sleeper carriages Madrid" }),
            reply,
            at: std::time::Instant::now(),
        };
        (run.agents).update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        assert_eq!(
            answer.try_recv().expect("the call wasn't answered at once"),
            ToolResult::error(format!("There's no tool {tool}."))
        );
    }
    assert!(titles(&run, &agent, cx).is_empty());
    assert!(entries(&run, &agent, cx).is_empty());
}

/// The thread's past row, as Library › Agents keeps it on disk, holds its searches' count and
/// none of their words.
#[gpui::test]
fn the_past_row_holds_the_searches_count_alone(cx: &mut TestAppContext) {
    let run = Run::new("search-past", cx);
    let agent = run.start(Cli::Claude, cx);
    run.said(&agent, Out::Searched("sleeper carriages Madrid".into()), cx);
    run.said(&agent, Out::Searched("ferry to Tangier".into()), cx);
    run.finish(&agent, cx);
    let ended = (run.agents).update(cx, |agents, cx| agents.take_ended(cx));
    let past: Vec<_> = ended.into_iter().map(|(_, past)| past).collect();
    assert_eq!(past.len(), 1, "{past:?}");
    let file = crate::agent_history::write(&past);
    let searched: Vec<_> = (crate::agent_history::read(&file).into_iter())
        .flat_map(|past| past.steps)
        .filter(|step| step.action == SEARCHED)
        .collect();
    assert_eq!(searched.len(), 1, "{file}");
    assert_eq!(searched[0].target, "2", "{file}");
    for word in ["sleeper", "carriages", "Madrid", "ferry", "Tangier"] {
        assert!(!file.contains(word), "{word} in the past file: {file}");
    }
}
