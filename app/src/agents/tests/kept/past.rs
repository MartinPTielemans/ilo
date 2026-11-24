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

/// The Archive's tasks, the last to leave first: each one's task, title and the line under it.
fn archive(run: &Run, cx: &mut TestAppContext) -> Vec<(String, String, String)> {
    (run.agents).read_with(cx, |agents, cx| {
        (agents.archived_tasks(cx).into_iter())
            .map(|task| {
                (
                    task.agent.to_string(),
                    task.title.to_string(),
                    task.about.to_string(),
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
    assert_eq!(keys, ["cli", "ended", "id", "profile", "state", "title"]);
    assert_eq!(saved[0]["id"], json!(id.as_str()));
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
        "9:05",
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
    assert_eq!(archived[0].1, "Untitled task");
    assert!(archived[0].2.starts_with("Claude · Done "));
    // Out of the Archive and back, it has no more of a title than it had.
    act(&next, &archived[0].0, TaskAction::Delete, cx);
    act(&next, &archived[0].0, TaskAction::Undelete, cx);
    assert_eq!(rows(&next)[0]["title"], json!(""));
    assert_eq!(rows(&next)[0]["state"], json!("done"));
}

/// Each end is kept as itself and reads its own word after the launch: done, stopped, failed and
/// at its assistant's limit, whose reset isn't kept. A task still at work when the user quits is
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
            Value::Null,
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
    let past = (archived.iter().find(|task| task.1 == "Untitled task"))
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
