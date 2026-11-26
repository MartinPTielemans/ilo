//! A thread's search, with the browser's `search` tool in place of its program's own: it opens
//! the results of the user's search engine for the thread's profile in a tab of the thread's, or
//! goes there in the tab it names. Its step holds its words whole, in memory alone, and the card
//! never says one. Its activity row says its words, clipped, and never the engine (AP25).

use super::kept::Run;
use super::*;
use crate::agents::runner::Cli;
use crate::agents::task::{Line, Trace, TraceItem};
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
fn titles(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<String> {
    (stretches(run, agent, cx).into_iter())
        .flat_map(|trace| trace.items)
        .filter_map(|item| match item {
            TraceItem::Step(step) => Some(step.title.to_string()),
            _ => None,
        })
        .collect()
}

/// The thread's activity entries, each as Settings › Agents and the past task's row say it, with
/// every word it holds.
fn entries(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<(String, String)> {
    (run.agents).read_with(cx, |agents, _| {
        (agents.activity(agent))
            .map(|entry| {
                let held = [
                    &entry.action,
                    &entry.site,
                    &entry.target,
                    &entry.refused,
                    &entry.by,
                    &entry.url,
                    &entry.said,
                    &entry.notes,
                ];
                let did = tools::Did {
                    tool: entry.action.to_string(),
                    site: entry.site.to_string(),
                    target: entry.target.to_string(),
                    ..Default::default()
                };
                let tense = match entry.refused.is_empty() {
                    true => tools::Tense::Done,
                    false => tools::Tense::Failed,
                };
                let held: Vec<&str> = held.iter().map(|each| each.as_ref()).collect();
                (tools::step_title(&did, tense), held.join(" "))
            })
            .collect()
    })
}

/// The thread searches for `words`, and the page answers.
fn search(run: &Run, agent: &str, arguments: Value, cx: &mut TestAppContext) -> ToolResult {
    let answer = call(&run.agents, agent, SEARCHED, arguments, cx);
    let result = page_done(&run.agents, &run.browser, answer, shop_tree(), cx);
    answered(run, agent, &result, cx);
    result
}

/// The server sent the thread `result`, which ends the call's step, as it does once it answers.
fn answered(run: &Run, agent: &str, result: &ToolResult, cx: &mut TestAppContext) {
    let event = Event::Answered {
        agent: agent.into(),
        id: 0,
        at: std::time::Instant::now(),
        error: result.error.then(|| result.text.clone()),
        kind: result.kind,
        image: result.image.clone(),
        notes: result.notes.map(|note| note.name),
    };
    (run.agents).update(cx, |agents, cx| agents.handle(event, cx));
}

/// A search under way says it's searching, with its words, and done says it searched (AP28).
#[gpui::test]
fn a_search_under_way_reads_as_searching(cx: &mut TestAppContext) {
    let run = Run::new("search-under-way", cx);
    let agent = run.start(Cli::Claude, cx);
    let answer = call(
        &run.agents,
        &agent,
        SEARCHED,
        json!({ "words": "night trains" }),
        cx,
    );
    assert_eq!(
        titles(&run, &agent, cx),
        ["Searching the web for “night trains”"]
    );
    let result = page_done(&run.agents, &run.browser, answer, shop_tree(), cx);
    answered(&run, &agent, &result, cx);
    assert_eq!(
        titles(&run, &agent, cx),
        ["Searched the web for “night trains”"]
    );
}

#[gpui::test]
fn a_search_opens_the_engines_results_in_a_tab_of_the_threads(cx: &mut TestAppContext) {
    let run = Run::new("search-opens", cx);
    let agent = run.start(Cli::Claude, cx);
    let words = json!({ "words": "  sleeper carriages Madrid " });
    let answer = search(&run, &agent, words, cx);
    assert!(answer.text.starts_with("Opened tab 30."), "{}", answer.text);
    let space = (run.browser).agent_space(&agent);
    let opened = (run.browser.tabs.borrow().iter())
        .find(|tab| tab.0 == 30)
        .map(|tab| (tab.1, tab.3));
    let results = "https://search.example/default?q=sleeper carriages Madrid";
    assert_eq!(opened, Some((space, results)));
    // Its step has its words whole; its row has them quoted, and neither names the engine.
    let titles = titles(&run, &agent, cx);
    assert_eq!(titles, ["Searched the web for “sleeper carriages Madrid”"]);
    let made = entries(&run, &agent, cx);
    let searched: Vec<_> = made
        .iter()
        .filter(|(_, held)| held.contains(SEARCHED))
        .collect();
    assert_eq!(searched.len(), 1, "{made:?}");
    assert_eq!(searched[0].0, "Searched “sleeper carriages Madrid”");
    for (title, held) in &made {
        assert!(
            !held.contains("search.example"),
            "the engine in {title}: {held}"
        );
        assert!(!title.contains("search.example"), "the engine in {title}");
    }
    // The card says none of it: no step and no word.
    let card = (run.agents).read_with(cx, |agents, cx| agents.cards(1, cx).remove(0));
    assert!(card.steps.is_empty(), "{:?}", card.steps);
    for said in [&card.status, &card.line.clone().unwrap_or_default()] {
        for word in ["Searched", "Madrid"] {
            assert!(!said.contains(word), "{word} on the card: {said}");
        }
    }
}

/// With a tab, the search goes there, as navigate does, and opens none.
#[gpui::test]
fn a_search_with_a_tab_goes_there(cx: &mut TestAppContext) {
    let run = Run::new("search-in-tab", cx);
    let agent = run.start(Cli::Claude, cx);
    search(&run, &agent, json!({ "words": "trains" }), cx);
    let answer = search(&run, &agent, json!({ "words": "buses", "tab": 30 }), cx);
    assert!(!answer.error, "{}", answer.text);
    let went = run.browser.navigated.borrow().last().cloned();
    assert_eq!(
        went,
        Some((30, "https://search.example/default?q=buses".into()))
    );
    let opened = (run.browser.tabs.borrow().iter())
        .filter(|tab| tab.0 == 30)
        .count();
    assert_eq!(opened, 1, "the search opened a second tab");
    let rows: Vec<String> = (entries(&run, &agent, cx).into_iter())
        .filter(|(_, held)| held.contains(SEARCHED))
        .map(|(title, _)| title)
        .collect();
    assert_eq!(rows, ["Searched “buses”", "Searched “trains”"]);
}

/// A thread in a second profile searches with that profile's engine, not the first one's.
#[gpui::test]
fn a_search_uses_the_threads_profiles_engine(cx: &mut TestAppContext) {
    let run = Run::new("search-profile", cx);
    // A thread's Space is 100, its first, in the profile the test names for it.
    run.browser.profiles.borrow_mut().push((100, "Work"));
    let agent = run.start(Cli::Claude, cx);
    assert_eq!(run.browser.agent_space(&agent), 100);
    search(&run, &agent, json!({ "words": "ferry to Tangier" }), cx);
    let opened = (run.browser.tabs.borrow().iter())
        .find(|tab| tab.0 == 30)
        .map(|tab| tab.3);
    assert_eq!(
        opened,
        Some("https://search.example/work?q=ferry to Tangier")
    );
}

/// Its row clips the words to 32 characters, the "…" among them and the quotes not (TS6); its
/// step keeps them whole up to a thought's cut.
#[gpui::test]
fn a_searchs_row_clips_its_words_and_its_step_cuts_them_as_a_thoughts(cx: &mut TestAppContext) {
    let run = Run::new("search-cut", cx);
    let agent = run.start(Cli::Claude, cx);
    let long = "night trains from Lisbon to Madrid on Saturday";
    search(&run, &agent, json!({ "words": long }), cx);
    let made = entries(&run, &agent, cx);
    let row = made
        .iter()
        .find(|(_, held)| held.contains(SEARCHED))
        .unwrap();
    let quoted = row
        .0
        .strip_prefix("Searched “")
        .unwrap()
        .strip_suffix("”")
        .unwrap();
    let kept: String = long.chars().take(tools::SEARCH_SHOWN - 1).collect();
    assert_eq!(quoted, format!("{kept}…"));
    assert_eq!(quoted.chars().count(), 32);
    assert_eq!(
        titles(&run, &agent, cx),
        [format!("Searched the web for “{long}”")]
    );

    search(&run, &agent, json!({ "words": "a".repeat(5000) }), cx);
    let titles = titles(&run, &agent, cx);
    let words = titles[1]
        .trim_start_matches("Searched the web for “")
        .trim_end_matches("”");
    assert_eq!(words.chars().count(), 4000);
    assert!(words.ends_with('…'), "{words}");
}

/// A search with no words is answered so, and opens nothing.
#[gpui::test]
fn a_search_with_no_words_opens_nothing(cx: &mut TestAppContext) {
    let run = Run::new("search-empty", cx);
    let agent = run.start(Cli::Claude, cx);
    for arguments in [json!({ "words": "  " }), json!({ "query": "trains" })] {
        let answer = call(&run.agents, &agent, SEARCHED, arguments, cx);
        let answer = answer.try_recv().expect("the call wasn't answered at once");
        assert!(answer.error, "{}", answer.text);
    }
    assert!(run.browser.pages.borrow().is_empty());
    assert!(!run.browser.tabs.borrow().iter().any(|tab| tab.0 == 30));
}

/// An outside app's agent has no search of the browser's: its own client searches.
#[gpui::test]
fn an_outside_agent_has_no_search(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(
        &agents,
        "claude-code",
        SEARCHED,
        json!({ "words": "trains" }),
        cx,
    );
    assert_eq!(
        answer.try_recv().expect("the call wasn't answered at once"),
        ToolResult::error(format!("There's no tool {SEARCHED}."))
    );
    assert!(browser.pages.borrow().is_empty());
    assert!(
        !tools::names("claude-code")
            .iter()
            .any(|name| name == SEARCHED)
    );
    assert!(tools::names("task-1").iter().any(|name| name == SEARCHED));
}

/// The thread's past row, as Library › Agents keeps it on disk, holds each search's clipped words,
/// as its row says them, and never the engine.
#[gpui::test]
fn the_past_row_holds_each_searchs_clipped_words(cx: &mut TestAppContext) {
    let run = Run::new("search-past", cx);
    let agent = run.start(Cli::Claude, cx);
    search(
        &run,
        &agent,
        json!({ "words": "sleeper carriages Madrid" }),
        cx,
    );
    search(&run, &agent, json!({ "words": "ferry to Tangier" }), cx);
    run.finish(&agent, cx);
    let ended = (run.agents).update(cx, |agents, cx| agents.take_ended(cx));
    let past: Vec<_> = ended.into_iter().map(|(_, past)| past).collect();
    assert_eq!(past.len(), 1, "{past:?}");
    let file = crate::agent_history::write(&past);
    let searched: Vec<_> = (crate::agent_history::read(&file).into_iter())
        .flat_map(|past| past.steps)
        .filter(|step| step.action == SEARCHED)
        .map(|step| (step.target, step.site))
        .collect();
    let row = |words: &str| (format!("“{words}”"), String::new());
    assert_eq!(
        searched,
        [row("sleeper carriages Madrid"), row("ferry to Tangier")],
        "{file}"
    );
}

/// Words that fit in the row's 32 characters show whole; one more, and the row shows 31 and the
/// "…" (TS6). The quotes count for neither.
#[test]
fn a_searchs_words_fit_in_32_characters_with_the_cut() {
    let fits = "a".repeat(tools::SEARCH_SHOWN);
    assert_eq!(
        page::curly_within(&fits, tools::SEARCH_SHOWN),
        format!("“{fits}”")
    );
    let over = "b".repeat(tools::SEARCH_SHOWN + 1);
    let cut = format!("“{}…”", "b".repeat(tools::SEARCH_SHOWN - 1));
    assert_eq!(page::curly_within(&over, tools::SEARCH_SHOWN), cut);
    assert_eq!(page::curly_within("ferry", tools::SEARCH_SHOWN), "“ferry”");
}

/// The search's one entry: its words, no site, and why it was refused.
fn searched_entry(run: &Run, agent: &str, cx: &mut TestAppContext) -> (String, String, String) {
    (run.agents).read_with(cx, |agents, _| {
        let entries: Vec<_> = agents.activity(agent).collect();
        let searched: Vec<_> = (entries.iter())
            .filter(|entry| matches!(entry.action.as_ref(), "open_tab" | "navigate" | SEARCHED))
            .collect();
        assert_eq!(searched.len(), 1, "{entries:?}");
        let entry = searched[0];
        assert_eq!(entry.action.as_ref(), SEARCHED, "{entry:?}");
        (
            entry.site.to_string(),
            entry.target.to_string(),
            entry.refused.to_string(),
        )
    })
}

/// A search the user's Off limits list keeps from the engine's site is refused as an open there
/// is, and its entry is still the search's, with its words and no site (AP25).
#[gpui::test]
fn a_search_kept_from_its_engine_is_the_searchs_entry(cx: &mut TestAppContext) {
    let run = Run::new("search-fenced", cx);
    let agent = run.start(Cli::Claude, cx);
    (run.agents).update(cx, |agents, cx| {
        assert!(agents.fence("https://search.example/", cx).is_some());
    });
    let answer = call(
        &run.agents,
        &agent,
        SEARCHED,
        json!({ "words": "trains" }),
        cx,
    );
    assert!(answer.try_recv().expect("refused at once").error);
    let (site, target, refused) = searched_entry(&run, &agent, cx);
    assert_eq!((site.as_str(), target.as_str()), ("", "“trains”"));
    assert!(!refused.is_empty());
    assert!(
        run.browser.tabs.borrow().iter().all(|tab| tab.0 != 30),
        "it opened"
    );
}

/// A search its thread's access refuses, an engine on a bank's site at Work, is the search's
/// entry too.
#[gpui::test]
fn a_search_its_access_refuses_is_the_searchs_entry(cx: &mut TestAppContext) {
    let run = Run::new("search-tier", cx);
    run.browser.engine.set(Some("mybank.example"));
    let agent = run.start(Cli::Claude, cx);
    let answer = call(
        &run.agents,
        &agent,
        SEARCHED,
        json!({ "words": "trains" }),
        cx,
    );
    assert!(answer.try_recv().expect("refused at once").error);
    let (site, target, refused) = searched_entry(&run, &agent, cx);
    assert_eq!((site.as_str(), target.as_str()), ("", "“trains”"));
    assert!(!refused.is_empty());
    let titles = entries(&run, &agent, cx);
    assert!(
        titles.iter().all(|(title, _)| !title.contains("mybank")),
        "{titles:?}"
    );
}

/// A search in a tab that waits for its text, refused later as the user takes the tab, logs
/// that refusal as the search's.
#[gpui::test]
fn a_search_refused_while_it_waits_is_the_searchs_entry(cx: &mut TestAppContext) {
    let run = Run::new("search-waits", cx);
    let agent = run.start(Cli::Claude, cx);
    search(&run, &agent, json!({ "words": "trains" }), cx);
    let waits = json!({ "words": "buses", "tab": 30, "wait_for": "Thanks" });
    let answer = call(&run.agents, &agent, SEARCHED, waits, cx);
    let request = run.browser.pages.borrow().last().unwrap().1;
    (run.agents).update(cx, |agents, cx| {
        agents.page_done(request, shop_tree(), None, cx)
    });
    cx.run_until_parked();
    (run.agents).update(cx, |agents, cx| agents.take_over(&agent, 30, cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(answer.try_recv().expect("the wait didn't answer").error);
    let last = (run.agents).read_with(cx, |agents, _| agents.recent().next().unwrap().clone());
    assert_eq!(last.action.as_ref(), SEARCHED, "{last:?}");
    assert_eq!((last.site.as_ref(), last.target.as_ref()), ("", "“buses”"));
    assert!(!last.refused.is_empty());
}

/// A tab that isn't one is the search's error, as navigate's is, and nothing opens.
#[gpui::test]
fn a_search_with_a_tab_that_isnt_one_opens_nothing(cx: &mut TestAppContext) {
    let run = Run::new("search-bad-tab", cx);
    let agent = run.start(Cli::Claude, cx);
    let bad = json!({ "words": "trains", "tab": "the first" });
    let answer = call(&run.agents, &agent, SEARCHED, bad.clone(), cx);
    let refused = answer.try_recv().expect("answered at once");
    assert!(refused.error);
    assert_eq!(Err(refused.text), tools::tab_argument(&bad).map(|_| ()));
    assert!(
        run.browser.tabs.borrow().iter().all(|tab| tab.0 != 30),
        "it opened"
    );
    assert!(run.browser.navigated.borrow().is_empty(), "it went");
}

/// An outside app's agent has neither the browser's search nor strands: its own app has its own.
#[test]
fn an_outside_agent_has_neither_search_nor_strand() {
    let names = tools::names("claude-code");
    assert!(
        !names
            .iter()
            .any(|name| name == SEARCHED || name == "strand"),
        "{names:?}"
    );
}
