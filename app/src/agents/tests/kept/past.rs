//! Finished tasks across a quit (#4105): what the file says of one, how the next launch lists it,
//! how a reply takes it up in its own profile, and the ways its row leaves the file. On the
//! stand-in assistant and a profile folder of the tests' own, as the tests beside these are.

use super::*;
use crate::agents::runner::Failure;
use crate::agents::task::TaskAction;

/// Quit: the tasks end, as the app ends them.
fn quit(run: &Run, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx));
}

/// The next launch, on the folder and the stand-in `run` left.
fn next(run: &Run, cx: &mut TestAppContext) -> Run {
    Run::again(run.profile.clone(), run.program.clone(), cx)
}

fn index(run: &Run) -> PathBuf {
    run.profile.join(kept::FOLDER).join("index.json")
}

/// The file's rows, or none with no file.
fn rows(run: &Run) -> Vec<Value> {
    let saved = std::fs::read(index(run)).unwrap_or_default();
    let saved: Value = serde_json::from_slice(&saved).unwrap_or_default();
    saved.as_array().cloned().unwrap_or_default()
}

/// The file's row for the conversation under `id`.
fn row(run: &Run, id: &Id) -> Value {
    let found = (rows(run).into_iter()).find(|row| row["id"] == json!(id.as_str()));
    found.unwrap_or_default()
}

/// The Archive's tasks, the last to leave first: each one's task, title and the line under it,
/// with its end said as Settings › Agents' list says it, which has no days over its rows.
fn archive(run: &Run, cx: &mut TestAppContext) -> Vec<(String, String, String)> {
    (run.agents).read_with(cx, |agents, cx| {
        (agents.archived_tasks(cx).into_iter())
            .map(|task| {
                let when = crate::agents::time_or_day((task.ended / 1000.) as u64);
                (
                    task.agent.to_string(),
                    task.title.to_string(),
                    task.about_at(&when),
                )
            })
            .collect()
    })
}

/// Settings › Agents' rows of what's kept, in the same shape.
fn listed(run: &Run, cx: &mut TestAppContext) -> Vec<(String, String, String)> {
    (run.agents).read_with(cx, |agents, cx| {
        (agents.kept_list(cx).into_iter())
            .map(|(agent, title, about)| (agent.to_string(), title.to_string(), about.to_string()))
            .collect()
    })
}

/// The task's agent names it, as its `title` tool does.
fn name_it(run: &Run, agent: &str, name: &str, cx: &mut TestAppContext) {
    _ = call(&run.agents, agent, "title", json!({ "text": name }), cx);
    cx.run_until_parked();
    let title = (run.agents).read_with(cx, |agents, _| agents.task(agent).unwrap().title.clone());
    assert_eq!(title, name);
}

/// A task asked from the user's Space `home`.
fn start_in(run: &Run, home: u64, cx: &mut TestAppContext) -> String {
    (run.agents)
        .update(cx, |agents, cx| {
            let asked = "Find two seats to Lisbon for Saturday";
            agents.start_task(Cli::Claude, asked, None, home, Tier::default(), cx)
        })
        .expect("started")
}

fn act(run: &Run, agent: &str, action: TaskAction, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| _ = agents.task_action(agent, action, cx));
}

fn restore(run: &Run, agent: &str, cx: &mut TestAppContext) {
    let back = (run.agents).update(cx, |agents, cx| agents.restore_task(Some(agent), cx));
    assert_eq!(back.as_deref(), Some(agent));
}

fn reply(run: &Run, agent: &str, text: &str, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| agents.reply(agent, text, cx));
}

/// Whether the stand-in's `run` was ever started.
fn started(run: &Run, at: usize) -> bool {
    beside(&run.program, &format!("args.{at}")).exists()
}

/// That its assistant still has a task's conversation is said only where the messages of a task
/// a run before left would begin. One at work, or done or stopped in this run, has its messages
/// there and no such line; after a quit all there is of it is that line.
#[gpui::test]
fn only_a_task_brought_back_says_its_assistant_still_has_it(cx: &mut TestAppContext) {
    let run = Run::new("still-has", cx);
    let done = run.start(Cli::Claude, cx);
    run.ran(1);
    assert_eq!(run.ilo_lines(&done, cx), []);
    name_it(&run, &done, "Lisbon seats", cx);
    run.finish(&done, cx);
    assert_eq!(run.ilo_lines(&done, cx), []);
    assert!(!run.talk(&done, cx).lines.is_empty());
    let stopped = run.start(Cli::Claude, cx);
    run.ran(2);
    name_it(&run, &stopped, "Porto seats", cx);
    act(&run, &stopped, TaskAction::Stop, cx);
    // Its one line is that the user stopped it, which this run alone holds.
    assert_eq!(
        run.ilo_lines(&stopped, cx),
        [IloKind::YouStopped { pages: 0 }]
    );

    quit(&run, cx);
    let next = next(&run, cx);
    let past = archive(&next, cx);
    assert_eq!(past.len(), 2);
    for (past, ..) in &past {
        assert_eq!(next.talk(past, cx).lines.len(), 1);
        assert_eq!(next.ilo_lines(past, cx), [IloKind::KeptHere]);
    }
}

/// The tasks a run before left are Latest tasks' to list until one is opened, from what the file
/// said of each and no more: the last to end first, each with the user's Space of the profile it
/// was asked in for its home and no card at the foot. One brought back is a card at the foot
/// instead, and Remove All takes the rest out at once, before any assistant has answered.
#[gpui::test]
fn the_tasks_a_run_before_left_are_listed_by_their_profile(cx: &mut TestAppContext) {
    let run = Run::new("latest", cx);
    for (at, name) in ["Lisbon seats", "Porto seats", "Faro seats"]
        .into_iter()
        .enumerate()
    {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        run.finish(&agent, cx);
    }
    // None of a run's own tasks is one: each has its card, or its row in the Archive.
    let kept = |run: &Run, cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, cx| {
            (agents.kept_cards(cx).into_iter())
                .map(|card| (card.title.to_string(), card.home))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(kept(&run, cx), []);
    quit(&run, cx);
    // As the file would say had Porto's ended an hour before the others, and Faro's been asked
    // in another profile.
    let mut saved = rows(&run);
    for row in &mut saved {
        match row["title"].as_str() {
            Some("Porto seats") => row["ended"] = json!(row["ended"].as_u64().unwrap() - 3600),
            Some("Faro seats") => row["profile"] = json!("Profile 2"),
            _ => row["ended"] = json!(row["ended"].as_u64().unwrap() + 60),
        }
    }
    std::fs::write(index(&run), serde_json::to_vec(&saved).unwrap()).unwrap();

    let next = next(&run, cx);
    // Faro's profile has no Space of the user's yet, so it's in the Archive alone.
    assert_eq!(archive(&next, cx).len(), 3);
    let both = [
        ("Lisbon seats".to_string(), 1),
        ("Porto seats".to_string(), 1),
    ];
    assert_eq!(kept(&next, cx), both);
    next.browser.profiles.borrow_mut().push((2, "Profile 2"));
    let all = kept(&next, cx);
    assert_eq!(all.len(), 3);
    assert!(all.contains(&("Faro seats".to_string(), 2)), "{all:?}");
    let cards = (next.agents).read_with(cx, |agents, cx| agents.kept_cards(cx));
    for card in &cards {
        assert!(card.replies && card.steps.is_empty() && card.pages.is_empty());
        assert_eq!(card.space, 0);
        // Its head says when it ended, and no line follows: no run keeps its answer or steps.
        assert!(card.ended_at.is_some(), "{} says no time", card.title);
        assert_eq!(
            (card.line.as_ref(), card.last_tab),
            (None, None),
            "{}",
            card.title
        );
    }
    assert!((next.agents).read_with(cx, |agents, cx| agents.shown_cards(cx).is_empty()));

    let lisbon = cards[0].agent.to_string();
    restore(&next, &lisbon, cx);
    assert!(!kept(&next, cx).iter().any(|card| card.0 == "Lisbon seats"));
    let shown = (next.agents).read_with(cx, |agents, cx| agents.shown_cards(cx));
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].agent.as_ref(), lisbon);
    (next.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    assert_eq!(kept(&next, cx), []);
}

/// However many tasks a run before left, a profile's cards are no more than the composer shows,
/// the last to end, so a change of a task never makes a card nobody sees. Another profile has its
/// own, older than those left out or not.
#[gpui::test]
fn a_profile_has_no_more_kept_cards_than_the_composer_shows(cx: &mut TestAppContext) {
    let most = crate::tokens::shell::composer::CARDS as usize;
    let run = Run::new("cap", cx);
    for at in 0..most + 3 {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, &format!("Task {at}"), cx);
        run.finish(&agent, cx);
    }
    quit(&run, cx);
    // As the file would say had each ended a minute after the one before, and the first been
    // asked in another profile.
    let mut saved = rows(&run);
    assert_eq!(saved.len(), most + 3);
    for row in &mut saved {
        let title = row["title"].as_str().unwrap().to_string();
        let at: u64 = title.trim_start_matches("Task ").parse().unwrap();
        row["ended"] = json!(1_700_000_000 + at * 60);
        if at == 0 {
            row["profile"] = json!("Profile 2");
        }
    }
    std::fs::write(index(&run), serde_json::to_vec(&saved).unwrap()).unwrap();

    let next = next(&run, cx);
    next.browser.profiles.borrow_mut().push((2, "Profile 2"));
    let kept: Vec<(String, u64)> = (next.agents).read_with(cx, |agents, cx| {
        (agents.kept_cards(cx).into_iter())
            .map(|card| (card.title.to_string(), card.home))
            .collect()
    });
    let mut newest: Vec<(String, u64)> = (0..most)
        .map(|back| (format!("Task {}", most + 2 - back), 1))
        .collect();
    newest.push(("Task 0".to_string(), 2));
    assert_eq!(kept, newest);
    // Those left out are in the Archive, where each is still brought back.
    assert_eq!(archive(&next, cx).len(), most + 3);
}

/// For the shell's test of the same road (shell/library/history_tests.rs): a task named `title`
/// ended Done with a page open, and the browser quit with no window left, which leaves the task's
/// Space as a crash does. The next launch finds the Space and its page, and gives none of it to
/// the user. That launch's agents, and the task there.
pub(crate) fn after_a_quit(title: &str, cx: &mut TestAppContext) -> (Entity<Agents>, String) {
    let run = Run::new("road", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    name_it(&run, &agent, title, cx);
    opens_a_page(&run.agents, &agent, cx);
    run.finish(&agent, cx);
    let space = (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().space);
    let in_space = |browser: &Made, space: u64| -> Vec<_> {
        let tabs = browser.tabs.borrow();
        tabs.iter().filter(|tab| tab.1 == space).copied().collect()
    };
    let pages = in_space(&run.browser, space);
    assert_eq!(pages.len(), 1);
    (run.agents).update(cx, |agents, cx| agents.end_tasks(false, cx));
    assert!(run.browser.deleted.borrow().is_empty());

    let next = next(&run, cx);
    let users = in_space(&next.browser, 1);
    next.browser.task_spaces.borrow_mut().push(space);
    next.browser.tabs.borrow_mut().extend(pages);
    // An outside agent's Space is another matter (#1080), and none was left.
    next.browser.agent_spaces.borrow_mut().clear();
    // Copies of a task's attached files a crash left, after the copy and before the task's end,
    // go with the rest of the tasks' folder, as their task is no more.
    let copies = (next.agents).read_with(cx, |agents, _| agents.tasks_dir().join("task-9 files"));
    std::fs::create_dir_all(&copies).unwrap();
    std::fs::write(copies.join("seats.pdf"), "two seats").unwrap();
    (next.agents).update(cx, |agents, cx| agents.end_old_tasks(cx));
    assert!(!copies.exists(), "a crash's copies stayed");
    assert_eq!(*next.browser.deleted.borrow(), [space]);
    assert_eq!(in_space(&next.browser, 1), users);
    assert!(in_space(&next.browser, space).is_empty());
    let past = archive(&next, cx)[0].0.clone();
    (next.agents, past)
}

/// The task's agent opens a page, as its `open_tab` tool does. The task's Space after.
pub(crate) fn opens_a_page(agents: &Entity<Agents>, agent: &str, cx: &mut TestAppContext) -> u64 {
    let url = json!({ "url": "https://www.example.com/timetable" });
    _ = call(agents, agent, "open_tab", url, cx);
    cx.run_until_parked();
    agents.read_with(cx, |agents, _| agents.task(agent).unwrap().space)
}

/// The file says of a task that has ended its conversation's id, its assistant, the name its
/// agent gave it, how and when it ended and its profile: never what was asked, the page it was
/// asked on, a step, or what its agent said. After a quit the next launch lists it in the Archive
/// and in Settings by that, makes and starts nothing for it, and a reply takes it up by the
/// assistant's own resume, told the reply alone.
#[gpui::test]
fn a_finished_task_is_there_after_a_quit_and_takes_a_reply(cx: &mut TestAppContext) {
    let run = Run::new("past", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    name_it(&run, &agent, "Lisbon seats", cx);
    let url = json!({ "url": "https://www.example.com/timetable" });
    _ = call(&run.agents, &agent, "open_tab", url, cx);
    cx.run_until_parked();
    run.finish(&agent, cx);

    let saved = rows(&run);
    assert_eq!(saved.len(), 1);
    let mut keys: Vec<&str> = (saved[0].as_object().unwrap().keys())
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["card", "cli", "ended", "id", "profile", "state", "title"]
    );
    assert_eq!(saved[0]["id"], json!(id.as_str()));
    // The card's id is its own, never the conversation's.
    let card = saved[0]["card"].as_str().unwrap();
    assert!(kept::cards::CardId::parse(card).is_some() && !card.contains(id.as_str()));
    assert_eq!(saved[0]["cli"], json!("claude"));
    assert_eq!(saved[0]["title"], json!("Lisbon seats"));
    assert_eq!(saved[0]["state"], json!("done"));
    assert_eq!(saved[0]["profile"], json!("Default"));
    assert!(
        saved[0]["ended"]
            .as_u64()
            .is_some_and(|at| at > 1_700_000_000)
    );
    let text = std::fs::read_to_string(index(&run)).unwrap();
    for never in [
        "Find two seats",
        "Saturday",
        "example.com",
        "Trains to Lisbon",
        "timetable",
        "car 4",
    ] {
        assert!(!text.contains(never), "the file has {never:?}");
    }
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(index(&run)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    quit(&run, cx);
    assert_eq!(rows(&run), saved, "a quit changed a finished task's row");
    let next = next(&run, cx);
    assert!(next.purged(cx).is_empty());
    assert!(next.folder(&id).exists());
    assert_eq!(next.kept(cx), (1, 0));
    let archived = archive(&next, cx);
    assert_eq!(archived.len(), 1);
    let (past, title, about) = archived[0].clone();
    assert_eq!(title, "Lisbon seats");
    assert!(about.starts_with("Claude · Done "), "{about}");
    assert_eq!(listed(&next, cx), archived);
    // All there is of it: no ask, page, message or step, no Space, no run, and one line, where
    // its messages would begin, that says where they are.
    (next.agents).read_with(cx, |agents, _| {
        let task = agents.task(&past).unwrap();
        assert!(task.asked.is_empty() && task.page.is_none());
        assert!(task.messages.is_empty() && task.trace.is_empty());
        assert_eq!((task.home, task.space), (0, 0));
    });
    assert!(next.browser.made.borrow().is_empty());
    assert!(!started(&next, 2));
    let talk = next.talk(&past, cx);
    assert_eq!(talk.lines.len(), 1);
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::KeptHere]);
    // Library › Agents has its receipt from the run it ended in, and gets no second.
    assert!((next.agents).update(cx, |agents, cx| agents.take_ended(cx).is_empty()));

    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    let (args, first) = next.ran(2);
    assert_eq!(after(&args, "--resume"), Some(id.as_str()));
    assert!(!args.iter().any(|arg| arg == "--session-id"));
    assert_eq!(first, "And for Sunday?");
    assert_eq!(next.ran_in(2), next.folder(&id));
    assert_eq!(
        next.ilo_lines(&past, cx),
        [IloKind::Resumed {
            tier: Tier::default()
        }]
    );
    // Its Space is made in the profile it was asked in, once it opens a page.
    let url = json!({ "url": "https://www.example.com/sunday" });
    _ = call(&next.agents, &past, "open_tab", url, cx);
    cx.run_until_parked();
    assert_eq!(
        next.browser.made.borrow().last(),
        Some(&(past.clone(), "Claude like 1".to_string()))
    );
    // Ended again, its row says so under the name it had.
    next.said(&past, Out::Said("Two at 10:35.".into()), cx);
    next.said(&past, Out::Ended(None), cx);
    let again = row(&next, &id);
    assert_eq!(again["title"], json!("Lisbon seats"));
    assert_eq!(again["state"], json!("done"));
    assert!(again["ended"].as_u64() >= saved[0]["ended"].as_u64());
    assert!(
        !std::fs::read_to_string(index(&next))
            .unwrap()
            .contains("Sunday")
    );
    // The next reply, in the run that brought it back, is the resume: ilo's line stays the one.
    reply(&next, &past, "And Monday?", cx);
    let lines = next.ilo_lines(&past, cx);
    let resumed = |kind: &&IloKind| matches!(kind, IloKind::Resumed { .. });
    assert_eq!(lines.iter().filter(resumed).count(), 1, "{lines:?}");
}

/// A kept task's card comes back after a quit as it was: its title at once, then its line and its
/// last page's picture once they're read, on its card in Latest tasks and on the task brought
/// back. The line is in its card's folder alone: the kept list's row, the Archive's and the file
/// of kept tasks say none of it. A reply takes the card's line and picture away, as its own turn
/// has its own.
#[gpui::test]
fn a_kept_cards_line_and_picture_come_back_after_a_quit(cx: &mut TestAppContext) {
    use super::cards::{last_page, picture, pictured};
    let run = Run::new("finding", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let tab = last_page(&run, &agent, cx);
    pictured(&run, &agent, tab, Some(picture()), cx);
    name_it(&run, &agent, "Lisbon seats", cx);
    run.finish(&agent, cx);
    let finding = (run.agents).read_with(cx, |agents, cx| {
        agents.card(agents.task(&agent).unwrap(), cx).finding
    });
    assert_eq!(finding, "Two seats at 9:05, car 4.");

    quit(&run, cx);
    cx.run_until_parked();
    let next = next(&run, cx);
    let past = archive(&next, cx)[0].0.clone();
    let kept = |next: &Run, cx: &mut TestAppContext| {
        (next.agents).read_with(cx, |agents, cx| {
            let card = (agents.kept_cards(cx).into_iter()).find(|card| card.agent == past.as_str());
            card.expect("no kept card")
        })
    };
    // Its title first: nothing is read on the launch's own path.
    let first = kept(&next, cx);
    assert_eq!(first.title, "Lisbon seats");
    assert!(
        first.finding.is_empty() && first.kept_picture.is_none(),
        "{first:?}"
    );

    cx.run_until_parked();
    let card = kept(&next, cx);
    assert_eq!(card.finding, "Two seats at 9:05, car 4.");
    assert_eq!(card.line.as_deref(), Some("Two seats at 9:05, car 4."));
    assert!(!card.told.is_empty());
    assert_eq!(
        card.kept_picture.as_ref().map(|plate| plate.size),
        Some((2, 2))
    );
    let back = (next.agents).read_with(cx, |agents, cx| {
        agents.card(agents.task(&past).unwrap(), cx)
    });
    assert_eq!(back.finding, card.finding);
    for (_, title, about) in listed(&next, cx).into_iter().chain(archive(&next, cx)) {
        assert!(
            !title.contains("car 4") && !about.contains("car 4"),
            "{title} {about}"
        );
    }
    let text = std::fs::read_to_string(index(&next)).unwrap();
    assert!(!text.contains("car 4"), "the file has the finding");

    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    next.ran(2);
    let replied = (next.agents).read_with(cx, |agents, cx| {
        agents.card(agents.task(&past).unwrap(), cx)
    });
    assert!(replied.finding.is_empty(), "{:?}", replied.finding);
    assert!(replied.kept_picture.is_none());
}

/// A kept thread's log comes back only when the thread is shown, never with the launch: until
/// then its conversation holds no line of it. Shown, its stored lines draw under what was asked,
/// settled, with the answer's time, and ilo's lines as words alone. Remove All takes them with
/// the card.
#[gpui::test]
fn a_kept_threads_log_comes_back_only_when_it_is_shown(cx: &mut TestAppContext) {
    use crate::agents::task::Line;
    use crate::agents::task::ilo::IloKind;
    let run = Run::new("log-back", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    name_it(&run, &agent, "Lisbon seats", cx);
    let result = json!({ "result": "Two seats at 9:05, car 4." });
    _ = call(&run.agents, &agent, "finish", result, cx);
    cx.run_until_parked();
    run.finish(&agent, cx);
    quit(&run, cx);
    cx.run_until_parked();
    let next = next(&run, cx);
    let past = archive(&next, cx)[0].0.clone();
    cx.run_until_parked();
    let shown = |next: &Run, cx: &mut TestAppContext| {
        (next.agents).read_with(cx, |agents, cx| {
            let talk = agents.conversation(&past, cx).expect("no conversation");
            let answer = (talk.lines.iter())
                .position(|line| matches!(line, Line::Said(text) if text.contains("car 4")));
            let at = |at: usize| talk.finished.iter().any(|(line, _)| *line == at);
            let pressed = (talk.lines.iter()).any(|line| match line {
                Line::Ilo(line) => !line.settled && matches!(line.kind, IloKind::Kept { .. }),
                Line::Trace(trace) => trace.running,
                _ => false,
            });
            let asked_twice = (talk.lines.iter())
                .any(|line| matches!(line, Line::You(text) if *text == talk.asked));
            (answer.map(at), pressed, asked_twice)
        })
    };
    // The launch read its card's line and picture, never its log.
    assert_eq!(shown(&next, cx), (None, false, false));

    (next.agents).update(cx, |agents, cx| agents.task_shown(&past, cx));
    cx.run_until_parked();
    assert_eq!(shown(&next, cx), (Some(true), false, false));

    (next.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    cx.run_until_parked();
    assert_eq!(shown(&next, cx).0, None);
}

/// Settings › Agents' Remove on a kept task brought back takes its card's line and picture off
/// the card shown, as its card's folder goes; Remove All takes every one's.
#[gpui::test]
fn removing_a_kept_task_takes_its_cards_line_and_picture(cx: &mut TestAppContext) {
    let run = Run::new("finding-removed", cx);
    for at in 1..=2 {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at);
        run.finish(&agent, cx);
    }
    quit(&run, cx);
    cx.run_until_parked();
    let next = next(&run, cx);
    cx.run_until_parked();
    let findings = |next: &Run, cx: &mut TestAppContext| {
        (next.agents).read_with(cx, |agents, cx| {
            (agents.kept_cards(cx).into_iter())
                .map(|card| (card.agent.to_string(), card.finding.to_string()))
                .collect::<Vec<_>>()
        })
    };
    let before = findings(&next, cx);
    assert_eq!(before.len(), 2, "{before:?}");
    assert!(
        before.iter().all(|(_, finding)| !finding.is_empty()),
        "{before:?}"
    );

    let first = before[0].0.clone();
    (next.agents).update(cx, |agents, cx| agents.remove_kept_task(&first, cx));
    cx.run_until_parked();
    let after = (next.agents).read_with(cx, |agents, cx| {
        agents.card(agents.task(&first).unwrap(), cx).finding
    });
    assert!(after.is_empty(), "{after:?}");
    let other = findings(&next, cx);
    assert!(
        other
            .iter()
            .any(|(agent, finding)| *agent != first && !finding.is_empty())
    );

    (next.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    cx.run_until_parked();
    let all = (next.agents).read_with(cx, |agents, cx| {
        (agents.tasks.tasks.iter())
            .map(|task| agents.card(task, cx).finding.to_string())
            .collect::<Vec<_>>()
    });
    assert!(all.iter().all(String::is_empty), "{all:?}");
}

/// Full access picked on a task that has ended isn't kept across a quit: brought back, it takes a
/// reply at Work on pages at most, as any does.
#[gpui::test]
fn full_access_picked_on_an_ended_task_isnt_kept_across_a_quit(cx: &mut TestAppContext) {
    let run = Run::new("picked-quit", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    run.finish(&agent, cx);
    act(&run, &agent, TaskAction::PickTier(Tier::Full), cx);
    let tier = |run: &Run, agent: &str, cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| agents.task(agent).unwrap().tier)
    };
    assert_eq!(tier(&run, &agent, cx), Tier::Full);
    quit(&run, cx);
    let next = next(&run, cx);
    let past = archive(&next, cx)[0].0.clone();
    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    next.ran(2);
    assert_eq!(tier(&next, &past, cx), Tier::Work);
}

/// A past task whose assistant no longer has its conversation, found by the reply: the reply never
/// reached it and it wasn't under way, so it stays ended as it was, under its own word and not
/// Interrupted, with the line that says so in the reply field's place and Start Again alone. Its
/// row leaves the file, and it takes no second reply.
#[gpui::test]
fn a_past_task_whose_conversation_went_stays_as_it_ended(cx: &mut TestAppContext) {
    let run = Run::new("past-gone", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    name_it(&run, &agent, "Lisbon seats", cx);
    run.finish(&agent, cx);
    quit(&run, cx);
    let next = next(&run, cx);
    let past = archive(&next, cx)[0].0.clone();
    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    let (args, _) = next.ran(2);
    assert_eq!(after(&args, "--resume"), Some(id.as_str()));
    let said = format!("No conversation found with session ID: {}", id.as_str());
    next.said(&past, Out::Gone(said), cx);

    assert!(matches!(next.state(&past, cx), State::Done { .. }));
    let talk = next.talk(&past, cx);
    assert!(talk.about.starts_with("Claude · Done"), "{}", talk.about);
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::Gone]);
    assert_eq!(
        talk.lines.len(),
        1,
        "the reply that never reached it stayed"
    );
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("ilo's line isn't last");
    };
    assert!(ilo::in_field(&last.kind));
    assert_eq!(ilo::buttons(last), [IloButton::StartAgain]);
    assert_eq!(row(&next, &id)["title"], Value::Null);
    assert!(listed(&next, cx).is_empty());
    reply(&next, &past, "Hello?", cx);
    cx.run_until_parked();
    assert!(!started(&next, 3));
}

/// What the user asked names a task until its agent names it, and is never kept: the file has no
/// title for one its agent never named, which the next launch lists as untitled.
#[gpui::test]
fn a_task_its_agent_never_named_keeps_no_title(cx: &mut TestAppContext) {
    let run = Run::new("untitled", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    run.finish(&agent, cx);
    let title = (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().title.clone());
    assert_eq!(title, "Find two seats to Lisbon for Saturday");
    assert_eq!(rows(&run)[0]["title"], json!(""));
    assert!(
        !std::fs::read_to_string(index(&run))
            .unwrap()
            .contains("Lisbon")
    );
    quit(&run, cx);
    let next = next(&run, cx);
    let archived = archive(&next, cx);
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].1, "Untitled thread");
    assert!(archived[0].2.starts_with("Claude · Done "));
    // Out of the Archive and back, it has no more of a title than it had.
    act(&next, &archived[0].0, TaskAction::Delete, cx);
    act(&next, &archived[0].0, TaskAction::Undelete, cx);
    assert_eq!(rows(&next)[0]["title"], json!(""));
    assert_eq!(rows(&next)[0]["state"], json!("done"));
}

/// Each end is kept as itself and reads its own word after the launch: done, stopped, failed and
/// at its assistant's limit, whose reset isn't kept. A task still at work has the row of one that
/// broke off, which is what it is if the browser goes with no quit, and when the user quits it's
/// there as stopped. One at its limit takes Retry, which goes on with its conversation.
#[gpui::test]
fn each_end_comes_back_as_its_own_word(cx: &mut TestAppContext) {
    let run = Run::new("ends", cx);
    // All five start before one fails: after a failure the browser looks for its assistants
    // again, and this run's stand-in is one no look finds.
    let names = ["Failed", "Limit", "Stopped", "Working", "Done"];
    let mut made = Vec::new();
    for (at, name) in names.into_iter().enumerate() {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        made.push((agent.clone(), run.id(&agent, cx)));
    }
    for (name, (agent, _)) in names.into_iter().zip(&made) {
        match name {
            "Failed" => {
                let failure = Failure::Other("It broke.".into());
                run.said(agent, Out::Ended(Some(failure)), cx)
            }
            "Limit" => {
                let resets = Failure::Limit(Some(1_900_000_000));
                run.said(agent, Out::Ended(Some(resets)), cx)
            }
            "Stopped" => act(&run, agent, TaskAction::Stop, cx),
            "Done" => run.finish(agent, cx),
            _ => {}
        }
    }
    let states = |run: &Run| -> Vec<Value> {
        (made.iter())
            .map(|(_, id)| row(run, id)["state"].clone())
            .collect()
    };
    assert_eq!(
        states(&run),
        [
            json!("failed"),
            json!("limit"),
            json!("stopped"),
            json!("failed"),
            json!("done")
        ]
    );
    let text = std::fs::read_to_string(index(&run)).unwrap();
    assert!(!text.contains("1900000000") && !text.contains("It broke"));
    quit(&run, cx);
    assert_eq!(states(&run)[3], json!("stopped"));

    let next = next(&run, cx);
    assert!(next.purged(cx).is_empty());
    let archived = archive(&next, cx);
    let about = |title: &str| {
        let found = archived.iter().find(|task| task.1 == title);
        found.unwrap_or_else(|| panic!("no {title}")).2.clone()
    };
    for (title, word) in [
        ("Failed", "Interrupted"),
        ("Limit", "At limit"),
        ("Stopped", "Stopped"),
        ("Working", "Stopped"),
        ("Done", "Done"),
    ] {
        let about = about(title);
        assert!(about.starts_with(&format!("Claude · {word} ")), "{about}");
    }
    let limit = (archived.iter().find(|task| task.1 == "Limit").unwrap())
        .0
        .clone();
    assert_eq!(next.state(&limit, cx), State::Limit { resets: None });
    // However it ended, its conversation starts with where its messages are.
    assert_eq!(next.ilo_lines(&limit, cx), [IloKind::KeptHere]);
    restore(&next, &limit, cx);
    act(&next, &limit, TaskAction::Retry, cx);
    let (args, _) = next.ran(6);
    assert_eq!(after(&args, "--resume"), Some(made[1].1.as_str()));
}

/// Library › Threads keeps how each thread's last turn ended, which its row says: one that broke
/// as Interrupted, one the user stopped as Stopped, one done as Done. One at its limit or still at
/// work has no entry, and gets none.
#[gpui::test]
fn a_threads_entry_keeps_how_its_last_turn_ended(cx: &mut TestAppContext) {
    use crate::agents::kept::End;
    let run = Run::new("entry-ends", cx);
    let names = ["Failed", "Limit", "Stopped", "Working", "Done"];
    let mut made = Vec::new();
    for (at, name) in names.into_iter().enumerate() {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        made.push(agent);
    }
    for (name, agent) in names.into_iter().zip(&made) {
        match name {
            "Failed" => {
                let failure = Failure::Other("It broke.".into());
                run.said(agent, Out::Ended(Some(failure)), cx)
            }
            "Limit" => {
                let resets = Failure::Limit(Some(1_900_000_000));
                run.said(agent, Out::Ended(Some(resets)), cx)
            }
            "Stopped" => act(&run, agent, TaskAction::Stop, cx),
            "Done" => run.finish(agent, cx),
            _ => {}
        }
    }
    let kept = (run.agents).update(cx, |agents, cx| agents.take_ended(cx));
    let mut ends: Vec<(String, Option<End>)> = (kept.into_iter())
        .map(|(_, past)| (past.title, past.end))
        .collect();
    ends.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        ends,
        [
            ("Done".to_string(), Some(End::Done)),
            ("Failed".to_string(), Some(End::Failed)),
            ("Stopped".to_string(), Some(End::Stopped)),
        ],
        "the entries kept, by title, with how each ended"
    );
}

/// A task at work when the browser goes with no quit, as when it's killed, is there at the next
/// launch as one that broke off: Interrupted, by the name its agent gave it, in its profile, and
/// with its conversation, which nothing removed, so a reply takes it up. Its row is written at
/// its start and when its agent names it, never at a step, and has what any row has: nothing
/// asked, no address, no step. While it works it's in no list of past tasks.
#[gpui::test]
fn a_task_at_work_when_the_browser_went_comes_back_interrupted(cx: &mut TestAppContext) {
    let run = Run::new("cut-off", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    // From its start, before its agent has said a word.
    let at_start = row(&run, &id);
    assert_eq!(at_start["title"], json!(""));
    assert_eq!(at_start["state"], json!("failed"));
    name_it(&run, &agent, "Lisbon seats", cx);
    let saved = rows(&run);
    assert_eq!(saved.len(), 1);
    let mut keys: Vec<&str> = (saved[0].as_object().unwrap().keys())
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["card", "cli", "ended", "id", "profile", "state", "title"]
    );
    assert_eq!(saved[0]["title"], json!("Lisbon seats"));
    assert_eq!(saved[0]["state"], json!("failed"));
    assert_eq!(saved[0]["profile"], json!("Default"));
    // Its time is its start's.
    assert_eq!(saved[0]["ended"], at_start["ended"]);
    assert!(
        saved[0]["ended"]
            .as_u64()
            .is_some_and(|at| at > 1_700_000_000)
    );

    // Nothing a task does at work writes the file: with the file out of the way, none is back.
    let file = std::fs::read(index(&run)).unwrap();
    std::fs::remove_file(index(&run)).unwrap();
    let url = json!({ "url": "https://www.example.com/timetable" });
    _ = call(&run.agents, &agent, "open_tab", url, cx);
    cx.run_until_parked();
    run.said(&agent, Out::Said("The 9:05 has two.".into()), cx);
    name_it(&run, &agent, "Lisbon seats", cx);
    assert!(!index(&run).exists(), "a step wrote the file");
    std::fs::write(index(&run), &file).unwrap();
    let text = String::from_utf8(file).unwrap();
    for never in ["Find two seats", "Saturday", "example.com", "timetable"] {
        assert!(!text.contains(never), "the file has {never:?}");
    }
    // At work it's a task, and no past one.
    assert!(listed(&run, cx).is_empty());
    assert!(archive(&run, cx).is_empty());

    // No quit: the browser went while it worked.
    let next = next(&run, cx);
    assert!(next.purged(cx).is_empty());
    assert!(next.folder(&id).exists());
    assert_eq!(next.kept(cx), (1, 0));
    let archived = archive(&next, cx);
    assert_eq!(archived.len(), 1);
    let (past, title, about) = archived[0].clone();
    assert_eq!(title, "Lisbon seats");
    assert!(about.starts_with("Claude · Interrupted "), "{about}");
    assert_eq!(listed(&next, cx), archived);
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::KeptHere]);
    assert!(next.browser.made.borrow().is_empty());
    assert!(!started(&next, 2));

    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    let (args, first) = next.ran(2);
    assert_eq!(after(&args, "--resume"), Some(id.as_str()));
    assert_eq!(first, "And for Sunday?");
    // Under way again, it's once more a task and no past one.
    assert!(listed(&next, cx).is_empty());
    assert!(archive(&next, cx).is_empty());
    assert_eq!(rows(&next).len(), 1);
    // A quit is the user's, and says stopped.
    quit(&next, cx);
    assert_eq!(row(&next, &id)["state"], json!("stopped"));
    assert_eq!(row(&next, &id)["title"], json!("Lisbon seats"));
}

/// A task that had ended and is at work again, by a reply or by Retry, has the row of one that
/// broke off from then, under the name it had: if the browser goes with no quit it comes back
/// Interrupted, not as it ended before.
#[gpui::test]
fn a_task_taken_up_and_cut_off_comes_back_interrupted(cx: &mut TestAppContext) {
    let run = Run::new("cut-off-again", cx);
    let mut made = Vec::new();
    for (at, name) in ["Lisbon seats", "Porto seats"].into_iter().enumerate() {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        made.push((agent.clone(), run.id(&agent, cx)));
    }
    run.finish(&made[0].0, cx);
    let resets = Failure::Limit(Some(1_900_000_000));
    run.said(&made[1].0, Out::Ended(Some(resets)), cx);
    quit(&run, cx);
    let before: Vec<Value> = made.iter().map(|(_, id)| row(&run, id)).collect();
    assert_eq!(before[0]["state"], json!("done"));
    assert_eq!(before[1]["state"], json!("limit"));

    let next = next(&run, cx);
    let archived = archive(&next, cx);
    let past = |title: &str| {
        let found = archived.iter().find(|task| task.1 == title);
        found.unwrap_or_else(|| panic!("no {title}")).0.clone()
    };
    let (done, limit) = (past("Lisbon seats"), past("Porto seats"));
    restore(&next, &done, cx);
    reply(&next, &done, "And for Sunday?", cx);
    next.ran(3);
    restore(&next, &limit, cx);
    act(&next, &limit, TaskAction::Retry, cx);
    next.ran(4);
    for ((_, id), before) in made.iter().zip(&before) {
        let now = row(&next, id);
        assert_eq!(now["state"], json!("failed"));
        assert_eq!(now["title"], before["title"]);
        assert_eq!(now["profile"], before["profile"]);
        assert!(now["ended"].as_u64() >= before["ended"].as_u64());
    }

    // No quit: the browser went while they worked.
    let last = Run::again(next.profile.clone(), next.program.clone(), cx);
    assert!(last.purged(cx).is_empty());
    let archived = archive(&last, cx);
    assert_eq!(archived.len(), 2);
    for (_, title, about) in &archived {
        assert!(
            about.starts_with("Claude · Interrupted "),
            "{title}: {about}"
        );
    }
}

/// Settings' Remove and Remove All take a task that broke off as any other: its row leaves the
/// file at once and its conversation is asked of its assistant.
#[gpui::test]
fn remove_takes_a_task_that_broke_off_and_its_conversation(cx: &mut TestAppContext) {
    let run = Run::new("cut-off-out", cx);
    let mut made = Vec::new();
    for (at, name) in ["Lisbon seats", "Porto seats"].into_iter().enumerate() {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        made.push(run.id(&agent, cx));
    }
    // At work they keep their own, as their assistant is writing them.
    (run.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    assert!(run.purged(cx).is_empty());
    assert_eq!(rows(&run).len(), 2);

    // No quit: the browser went while they worked.
    let next = next(&run, cx);
    let archived = archive(&next, cx);
    assert_eq!(listed(&next, cx).len(), 2);
    let lisbon = (archived.iter().find(|task| task.1 == "Lisbon seats"))
        .unwrap()
        .0
        .clone();
    (next.agents).update(cx, |agents, cx| agents.remove_kept_task(&lisbon, cx));
    let alone = |id: &Id| json!({ "id": id.as_str(), "cli": "claude" });
    assert_eq!(row(&next, &made[0]), alone(&made[0]));
    assert_eq!(listed(&next, cx).len(), 1);
    let folder = next.folder(&made[0]);
    assert_eq!(
        next.purged(cx),
        ["purge", folder.to_str().unwrap(), "--yes"]
    );
    assert!(!folder.exists());
    assert_eq!(row(&next, &made[0]), Value::Null);
    assert_eq!(next.ilo_lines(&lisbon, cx).last(), Some(&IloKind::Gone));

    (next.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    assert!(listed(&next, cx).is_empty());
    next.purged(cx);
    assert!(!index(&next).exists());
    assert!(!next.folder(&made[1]).exists());
    assert_eq!(next.kept(cx), (0, 0));
}

/// The conversation of a task deleted from the Archive goes at the next launch, with no quit
/// between as with one: its row is its id alone, which is asked of its assistant.
#[gpui::test]
fn the_next_launch_removes_what_a_task_deleted_from_the_archive_kept(cx: &mut TestAppContext) {
    let run = Run::new("launch", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    run.finish(&agent, cx);
    act(&run, &agent, TaskAction::Dismiss, cx);
    act(&run, &agent, TaskAction::Delete, cx);
    assert!(run.folder(&id).exists());
    assert!(run.purged(cx).is_empty());

    // No quit: the browser went.
    let next = next(&run, cx);
    assert!(archive(&next, cx).is_empty());
    let folder = run.folder(&id);
    assert_eq!(
        next.purged(cx),
        ["purge", folder.to_str().unwrap(), "--yes"]
    );
    assert!(!folder.exists());
    assert_eq!(next.kept(cx), (0, 0));
    assert!(!next.profile.join(kept::FOLDER).exists());
}

/// A private window starts no task, whichever way it was asked: every way ends in the one start,
/// which is given the Space the window shows. Nothing is made for it, no assistant runs and
/// nothing of it is kept, where the same words from a Space of the user's start one.
#[gpui::test]
fn a_private_window_starts_no_task_and_keeps_nothing(cx: &mut TestAppContext) {
    let run = Run::new("private", cx);
    run.browser.private.borrow_mut().push(9);
    let asked = "Find two seats to Lisbon for Saturday";
    let from_private = (run.agents).update(cx, |agents, cx| {
        agents.start_task(Cli::Claude, asked, None, 9, Tier::default(), cx)
    });
    cx.run_until_parked();
    assert_eq!(from_private, None);
    assert!((run.agents).read_with(cx, |agents, _| agents.tasks.tasks.is_empty()));
    assert!(run.browser.made.borrow().is_empty());
    assert!(!started(&run, 1));
    assert!(!index(&run).exists());
    assert_eq!(run.kept(cx), (0, 0));
    // No card, so no line, picture or log of it either.
    assert!(
        !run.profile
            .join(crate::agents::kept::cards::FOLDER)
            .exists()
    );

    let agent = start_in(&run, 1, cx);
    run.ran(1);
    assert_eq!(run.browser.made.borrow().len(), 1);
    assert_eq!(rows(&run).len(), 1);
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// The Archive holds ten, so the ten that ended last come back and what the run before kept of
/// the others goes, each asked of its assistant.
#[gpui::test]
fn the_ten_that_ended_last_come_back_and_the_others_go(cx: &mut TestAppContext) {
    let run = Run::new("ten", cx);
    let mut ids = Vec::new();
    for at in 1..=12 {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at);
        name_it(&run, &agent, &format!("Task {at}"), cx);
        ids.push(run.id(&agent, cx));
        run.finish(&agent, cx);
    }
    quit(&run, cx);
    let next = next(&run, cx);
    let purged = next.purged(cx);
    assert_eq!(purged.len(), 6, "{purged:?}");
    for id in &ids[..2] {
        let folder = next.folder(id);
        assert!(purged.iter().any(|arg| arg == folder.to_str().unwrap()));
        assert!(!folder.exists());
    }
    let titles: Vec<String> = (archive(&next, cx).into_iter())
        .map(|task| task.1)
        .collect();
    let want: Vec<String> = (3..=12).rev().map(|at| format!("Task {at}")).collect();
    assert_eq!(titles, want);
    assert_eq!(rows(&next).len(), 10);
    assert_eq!(next.kept(cx), (10, 0));
}

/// Delete from Archive takes a task's row out of the file at once, leaving its id alone to be
/// removed, and ⌘Z puts it back. Settings' Remove on a row takes that one out at once and asks
/// its assistant to remove the conversation; Remove All takes them all. A task that went that
/// way says its conversation is gone, and isn't there at the next launch.
#[gpui::test]
fn delete_remove_and_remove_all_take_a_row_out_at_once(cx: &mut TestAppContext) {
    let run = Run::new("out", cx);
    let mut made = Vec::new();
    for (at, name) in ["Lisbon seats", "Porto seats", "Faro seats"]
        .into_iter()
        .enumerate()
    {
        let agent = run.start(Cli::Claude, cx);
        run.ran(at + 1);
        name_it(&run, &agent, name, cx);
        made.push((agent.clone(), run.id(&agent, cx)));
        run.finish(&agent, cx);
    }
    let alone = |id: &Id| json!({ "id": id.as_str(), "cli": "claude" });
    let (lisbon, porto, faro) = (&made[0], &made[1], &made[2]);
    assert_eq!(listed(&run, cx).len(), 3);

    act(&run, &lisbon.0, TaskAction::Dismiss, cx);
    act(&run, &lisbon.0, TaskAction::Delete, cx);
    assert_eq!(row(&run, &lisbon.1), alone(&lisbon.1));
    assert!(!listed(&run, cx).iter().any(|task| task.0 == lisbon.0));
    act(&run, &lisbon.0, TaskAction::Undelete, cx);
    assert_eq!(row(&run, &lisbon.1)["title"], json!("Lisbon seats"));
    assert_eq!(listed(&run, cx).len(), 3);
    act(&run, &lisbon.0, TaskAction::Delete, cx);

    (run.agents).update(cx, |agents, cx| agents.remove_kept_task(&porto.0, cx));
    // At once, before its assistant has answered.
    assert_eq!(row(&run, &porto.1), alone(&porto.1));
    assert!(!listed(&run, cx).iter().any(|task| task.0 == porto.0));
    let folder = run.folder(&porto.1);
    assert_eq!(run.purged(cx), ["purge", folder.to_str().unwrap(), "--yes"]);
    assert_eq!(row(&run, &porto.1), Value::Null);
    assert_eq!(run.ilo_lines(&porto.0, cx).last(), Some(&IloKind::Gone));
    assert_eq!(rows(&run).len(), 2);

    quit(&run, cx);
    let next = next(&run, cx);
    // The one deleted from the Archive went with the quit.
    let folder = next.folder(&lisbon.1);
    assert!(
        next.purged(cx)
            .contains(&folder.to_str().unwrap().to_string())
    );
    let archived = archive(&next, cx);
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].1, "Faro seats");
    assert_eq!(rows(&next), [row(&next, &faro.1)]);

    (next.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    assert_eq!(rows(&next), [alone(&faro.1)]);
    assert!(listed(&next, cx).is_empty());
    next.purged(cx);
    assert!(!index(&next).exists());
    assert!(!next.folder(&faro.1).exists());
    assert_eq!(next.kept(cx), (0, 0));
    // Nothing of it is here to go on with, so it takes no reply, and none starts a run.
    let past = archived[0].0.clone();
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::Gone]);
    restore(&next, &past, cx);
    reply(&next, &past, "And for Sunday?", cx);
    cx.run_until_parked();
    assert!(!started(&next, 4));
    assert!(matches!(next.state(&past, cx), State::Done { .. }));

    quit(&next, cx);
    let last = Run::again(next.profile.clone(), next.program.clone(), cx);
    assert!(archive(&last, cx).is_empty());
}

/// A past task comes back in the profile it was asked in, whichever Space the user is in: its
/// card goes to a Space of theirs in that profile, and its own Space is made like that one.
#[gpui::test]
fn a_past_task_comes_back_in_its_own_profile(cx: &mut TestAppContext) {
    let run = Run::new("profile", cx);
    run.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let agent = start_in(&run, 7, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    run.finish(&agent, cx);
    assert_eq!(row(&run, &id)["profile"], json!("Profile 2"));
    quit(&run, cx);

    let next = next(&run, cx);
    next.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let past = archive(&next, cx)[0].0.clone();
    // The user is in Space 1, of another profile.
    restore(&next, &past, cx);
    (next.agents).read_with(cx, |agents, cx| {
        assert_eq!(agents.task(&past).unwrap().home, 7);
        assert_eq!(agents.cards(7, cx).len(), 1);
        assert!(agents.cards(1, cx).is_empty());
    });
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::KeptHere]);
    reply(&next, &past, "And for Sunday?", cx);
    let (args, _) = next.ran(2);
    assert_eq!(after(&args, "--resume"), Some(id.as_str()));
    let url = json!({ "url": "https://www.example.com/sunday" });
    _ = call(&next.agents, &past, "open_tab", url, cx);
    cx.run_until_parked();
    assert_eq!(
        next.browser.made.borrow().last(),
        Some(&(past.clone(), "Claude like 7".to_string()))
    );
}

/// A past task whose profile is still there with no Space of the user's left says that, not that
/// the profile was deleted, in the reply field's place with Start Again alone, and takes no
/// reply: its pages would have no Space of its profile to go to. Once the profile has a Space
/// again, a reply picks the task up in it.
#[gpui::test]
fn a_past_task_whose_profile_has_no_space_left_says_so(cx: &mut TestAppContext) {
    let run = Run::new("no-space", cx);
    run.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let agent = start_in(&run, 7, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    name_it(&run, &agent, "Lisbon seats", cx);
    run.finish(&agent, cx);
    quit(&run, cx);

    // "Profile 2" is there, and its last Space was closed.
    let next = next(&run, cx);
    next.browser.profiles.borrow_mut().push((0, "Profile 2"));
    let past = archive(&next, cx)[0].0.clone();
    restore(&next, &past, cx);
    let lines = next.talk(&past, cx).lines;
    let [Line::Ilo(line)] = &lines[..] else {
        panic!("{lines:?}");
    };
    assert_eq!(line.kind, IloKind::NoSpace);
    assert!(ilo::in_field(&line.kind));
    assert_eq!(ilo::buttons(line), [IloButton::StartAgain]);
    reply(&next, &past, "And for Sunday?", cx);
    cx.run_until_parked();
    assert!(!started(&next, 2));

    // A Space in it again, and the task is there to reply to, in that Space's profile.
    *next.browser.profiles.borrow_mut() = vec![(9, "Profile 2")];
    (next.agents).update(cx, |agents, cx| {
        agents.profiles_are(vec!["Default".into(), "Profile 2".into()], cx)
    });
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::KeptHere]);
    reply(&next, &past, "And for Sunday?", cx);
    let (args, _) = next.ran(2);
    assert_eq!(after(&args, "--resume"), Some(id.as_str()));
    (next.agents).read_with(cx, |agents, _| {
        assert_eq!(agents.task(&past).unwrap().home, 9);
    });
}

/// A past task whose profile is gone at launch has nowhere to work: brought back, it says so in
/// the reply field's place and offers Start Again alone. No reply and no Retry starts a run for
/// it in another profile's sessions. Settings still lists it, and Remove takes it out.
#[gpui::test]
fn a_past_task_whose_profile_is_gone_offers_start_again_alone(cx: &mut TestAppContext) {
    let run = Run::new("no-profile", cx);
    run.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let agent = start_in(&run, 7, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    run.said(&agent, Out::Ended(Some(Failure::Limit(None))), cx);
    let other = run.start(Cli::Claude, cx);
    run.ran(2);
    let others = run.id(&other, cx);
    name_it(&run, &other, "Porto seats", cx);
    run.finish(&other, cx);
    quit(&run, cx);

    // No Space is in "Profile 2" any more.
    let next = next(&run, cx);
    let archived = archive(&next, cx);
    assert_eq!(archived.len(), 2);
    let past = (archived.iter().find(|task| task.1 == "Untitled thread"))
        .unwrap()
        .0
        .clone();
    restore(&next, &past, cx);
    let lines = next.talk(&past, cx).lines;
    let [Line::Ilo(line)] = &lines[..] else {
        panic!("{lines:?}");
    };
    assert_eq!(line.kind, IloKind::NoProfile);
    assert!(ilo::in_field(&line.kind));
    assert_eq!(ilo::buttons(line), [IloButton::StartAgain]);
    reply(&next, &past, "And for Sunday?", cx);
    act(&next, &past, TaskAction::Retry, cx);
    cx.run_until_parked();
    assert!(!started(&next, 3));
    assert_eq!(next.state(&past, cx), State::Limit { resets: None });
    (next.agents).read_with(cx, |agents, _| {
        let task = agents.task(&past).unwrap();
        assert!(task.messages.is_empty() && task.space == 0);
    });
    assert!(next.browser.made.borrow().is_empty());
    // Its card is where the user is, to start it again or remove it from.
    assert_eq!(
        (next.agents).read_with(cx, |agents, cx| agents.cards(1, cx).len()),
        1
    );

    assert!(listed(&next, cx).iter().any(|task| task.0 == past));
    (next.agents).update(cx, |agents, cx| agents.remove_kept_task(&past, cx));
    let folder = next.folder(&id);
    assert_eq!(
        next.purged(cx),
        ["purge", folder.to_str().unwrap(), "--yes"]
    );
    assert_eq!(rows(&next), [row(&next, &others)]);
    assert_eq!(next.ilo_lines(&past, cx), [IloKind::Gone]);
    // The other profile's task takes its reply as ever.
    let porto = (archived.iter().find(|task| task.1 == "Porto seats"))
        .unwrap()
        .0
        .clone();
    reply(&next, &porto, "And for Sunday?", cx);
    let (args, _) = next.ran(3);
    assert_eq!(after(&args, "--resume"), Some(others.as_str()));
}

/// A profile's deletion takes the tasks kept for it along, by the way Remove takes one, so no row
/// waits for a profile that later gets the same folder: a past task's of a run before and one of
/// this run's. Another profile's stay, and a core with no profiles to say removes nothing.
#[gpui::test]
fn a_profiles_deletion_removes_its_kept_tasks(cx: &mut TestAppContext) {
    let run = Run::new("deleted", cx);
    run.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let before = start_in(&run, 7, cx);
    run.ran(1);
    let old = run.id(&before, cx);
    run.finish(&before, cx);
    let stays = run.start(Cli::Claude, cx);
    run.ran(2);
    let staying = run.id(&stays, cx);
    run.finish(&stays, cx);
    quit(&run, cx);

    let next = next(&run, cx);
    next.browser.profiles.borrow_mut().push((7, "Profile 2"));
    let profiles = |names: &[&str]| {
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
    };
    let both = profiles(&["Default", "Profile 2"]);
    (next.agents).update(cx, |agents, cx| agents.profiles_are(both.clone(), cx));
    let new = start_in(&next, 7, cx);
    next.ran(3);
    let newer = next.id(&new, cx);
    next.finish(&new, cx);
    assert_eq!(rows(&next).len(), 3);
    assert!(next.purged(cx).is_empty());
    // A core with nothing to say yet, then the same profiles again: nothing goes.
    (next.agents).update(cx, |agents, cx| agents.profiles_are(Vec::new(), cx));
    (next.agents).update(cx, |agents, cx| agents.profiles_are(both, cx));
    assert!(next.purged(cx).is_empty());

    let one = profiles(&["Default"]);
    (next.agents).update(cx, |agents, cx| agents.profiles_are(one, cx));
    // At once, before the assistant has answered.
    assert_eq!(row(&next, &old).get("title"), None);
    assert_eq!(row(&next, &newer).get("title"), None);
    let purged = next.purged(cx);
    assert_eq!(purged.len(), 6, "{purged:?}");
    for id in [&old, &newer] {
        let folder = next.folder(id);
        assert!(purged.contains(&folder.to_str().unwrap().to_string()));
        assert!(!folder.exists());
    }
    assert_eq!(rows(&next), [row(&next, &staying)]);
    assert_eq!(listed(&next, cx).len(), 1);
    assert_eq!(next.ilo_lines(&new, cx).last(), Some(&IloKind::Gone));
}

/// With the inside switch off at launch, the tasks a run before left are listed as ever and have
/// no token, so nothing is let in as one of them; on again, each has its own.
#[gpui::test]
fn a_past_task_has_no_token_while_threads_are_off(cx: &mut TestAppContext) {
    let run = Run::new("threads-off", cx);
    let done = run.start(Cli::Claude, cx);
    run.ran(1);
    name_it(&run, &done, "Lisbon seats", cx);
    run.finish(&done, cx);
    quit(&run, cx);
    let path = run.profile.join("agents.json");
    let mut saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    saved["threads"] = json!(false);
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path), cx);
    agents.update(cx, |agents, cx| agents.load(&saved.to_string(), cx));
    let listed = agents.read_with(cx, |agents, cx| agents.archived_tasks(cx).len());
    assert_eq!(listed, 1, "the past task isn't listed");
    let tokens = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            let tokens = agents.tokens.lock().unwrap();
            tokens.values().filter(|agent| is_task_id(agent)).count()
        })
    };
    assert_eq!(tokens(cx), 0, "a past task has a token with threads off");
    agents.update(cx, |agents, cx| agents.set_threads(true, cx));
    assert_eq!(tokens(cx), 1, "on again, the past task has none");
}
