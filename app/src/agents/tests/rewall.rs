//! A task with files as its level changes or its files are taken back (#4101), on a stand-in
//! assistant in a profile folder of the test's own and a stand-in for the system's wall, which
//! says what it was asked for and can refuse. No folder here is anywhere.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use super::kept::{Run, after, beside};
use super::*;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::files::Given;
use crate::agents::runner::{self, Cli, Out};
use crate::agents::task::ilo::{IloKind, IloLine, LeftOut};
use crate::agents::task::rewall::{Change, PutUp, Rewalled, STOPPED_LOGGED, WITHOUT_FILES_LOGGED};
use crate::agents::task::{Line, State};
use crate::agents::tier::{self, Tier};
use crate::agents::wall::{Granted, Wall, WallFor, WallRefused};

/// What the task was asked, as `Run::start` asks it.
const ASKED: &str = "Find two seats to Lisbon for Saturday";

/// The stand-in for the system's wall: each wall it was asked for, by its level and whether it
/// had a folder and attached copies, and whether it refuses the next.
#[derive(Default)]
struct Walls {
    asked: Rc<RefCell<Vec<(Tier, bool, bool)>>>,
    refuses: Rc<Cell<bool>>,
}

impl Walls {
    fn maker(&self) -> PutUp {
        let (asked, refuses) = (self.asked.clone(), self.refuses.clone());
        Rc::new(move |wall: &WallFor| {
            let has = (wall.folder.is_some(), wall.attached.is_some());
            asked.borrow_mut().push((wall.level, has.0, has.1));
            match refuses.get() {
                true => Err(refused()),
                false => Ok(Wall::for_test(None)),
            }
        })
    }

    fn asked(&self) -> Vec<(Tier, bool, bool)> {
        self.asked.borrow().clone()
    }
}

fn refused() -> WallRefused {
    WallRefused::Failed("refused in a test".into())
}

fn folder() -> Given {
    Given {
        folder: Some(Granted::for_test()),
        attached: None,
    }
}

fn attached() -> Given {
    Given {
        folder: None,
        attached: Some(PathBuf::from("/nonexistent/attached")),
    }
}

/// A task started with `files`, at Work on pages, where every task starts.
fn started(name: &str, files: Given, cx: &mut TestAppContext) -> (Run, Walls, String) {
    let (run, walls) = (Run::new(name, cx), Walls::default());
    (run.agents).update(cx, |agents, _| {
        agents.tasks.files = files;
        agents.tasks.wall = Some(walls.maker());
    });
    let agent = run.start(Cli::Claude, cx);
    (run, walls, agent)
}

fn set(run: &Run, agent: &str, tier: Tier, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| agents.set_tier(agent, tier, cx));
}

fn run_of(run: &Run, agent: &str, cx: &mut TestAppContext) -> u32 {
    (run.agents).read_with(cx, |agents, _| agents.run_of(agent))
}

/// The task's level, whether it has files, and what came of its walls that weren't put up.
fn stands(run: &Run, agent: &str, cx: &mut TestAppContext) -> (Tier, bool, Vec<(usize, Rewalled)>) {
    (run.agents).read_with(cx, |agents, _| {
        let task = agents.task(agent).unwrap();
        let came = (task.rewalled.iter())
            .map(|(turn, _, came)| (*turn, came.clone()))
            .collect();
        (task.tier, task.files.any(), came)
    })
}

/// What ilo's lines in the task's conversation say, in its order. Each is settled, as none of a
/// wall's has a button.
fn lines(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<IloKind> {
    (run.talk(agent, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Ilo(IloLine { kind, settled, .. }) => {
                assert!(settled, "{kind:?}");
                Some(kind)
            }
            _ => None,
        })
        .collect()
}

/// The name `folder()` is shown by.
const FOLDER: &str = "folder";

/// The changes of level the log has for the task, oldest first.
fn logged(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<String> {
    (run.agents).read_with(cx, |agents, _| {
        let mut changes: Vec<String> = (agents.activity(agent))
            .filter(|entry| entry.action.as_ref() == ACCESS_ACTION)
            .map(|entry| entry.target.to_string())
            .collect();
        changes.reverse();
        changes
    })
}

/// The messages the stand-in's run `to` was sent, once it has `count` of them.
fn sent(run: &Run, to: usize, count: usize) -> Vec<String> {
    let path = beside(&run.program, &format!("in.{to}"));
    for _ in 0..500 {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.lines().count() >= count && text.ends_with('\n') {
            let message = |line: &str| {
                let message: Value = serde_json::from_str(line).unwrap();
                message["message"]["content"].as_str().unwrap().to_string()
            };
            return text.lines().map(message).collect();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {to} wasn't sent {count} messages");
}

/// Whether a run's arguments are a walled start's.
fn walled(args: &[String]) -> bool {
    after(args, "--settings").is_some()
}

/// A task with files starts inside a wall of its level, and its assistant keeps no conversation
/// of it, whatever the switch says.
#[gpui::test]
fn a_task_with_files_starts_inside_its_wall_and_keeps_nothing(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("starts", folder(), cx);
    assert!(run.agents.read_with(cx, |agents, _| agents.keeps_tasks()));
    let (args, first) = run.ran(1);
    assert_eq!(walls.asked(), [(Tier::Work, true, false)]);
    assert!(walled(&args), "{args:?}");
    assert!(first.contains(ASKED), "{first}");
    assert_eq!(after(&args, "--session-id"), None);
    assert_eq!(after(&args, "--resume"), None);
    assert!(
        run.agents
            .read_with(cx, |agents, _| agents.tasks.kept.of(&agent).is_none())
    );
    assert_eq!(
        run.agents.read_with(cx, |agents, _| agents.kept_tasks().0),
        0
    );
}

/// A task with no files is never asked a wall for, at its start or as its level changes, and a
/// change of level is told to the run it has.
#[gpui::test]
fn a_task_with_no_files_never_has_a_wall(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("none", Given::default(), cx);
    let (args, _) = run.ran(1);
    assert!(!walled(&args), "{args:?}");
    let before = run_of(&run, &agent, cx);
    set(&run, &agent, Tier::Full, cx);
    set(&run, &agent, Tier::Look, cx);
    assert!(walls.asked().is_empty());
    assert_eq!(run_of(&run, &agent, cx), before);
    let told = sent(&run, 1, 3);
    assert_eq!(told[1], tier::changed_turn(Tier::Work, Tier::Full));
    assert_eq!(told[2], tier::changed_turn(Tier::Full, Tier::Look));
}

/// A raise and a lower each end the run the task has and start another inside the new level's
/// wall, from the task's own words with ilo's about the change after them. What the run before
/// still says isn't the task's.
#[gpui::test]
fn a_change_of_level_starts_it_again_inside_the_new_levels_wall(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("level", folder(), cx);
    run.ran(1);
    let mut before = run_of(&run, &agent, cx);
    for (at, (from, to)) in [(Tier::Work, Tier::Full), (Tier::Full, Tier::Look)]
        .into_iter()
        .enumerate()
    {
        set(&run, &agent, to, cx);
        let (args, first) = run.ran(at + 2);
        assert_eq!(walls.asked().last(), Some(&(to, true, false)));
        assert_eq!(walls.asked().len(), at + 2);
        assert!(walled(&args), "{args:?}");
        assert_eq!(after(&args, "--session-id"), None);
        assert_eq!(after(&args, "--resume"), None);
        assert!(first.contains(ASKED), "{first}");
        assert!(first.ends_with(&tier::changed_turn(from, to)), "{first}");
        let now = run_of(&run, &agent, cx);
        assert_ne!(now, before);
        // The run that ended says it's gone, which is no end of the task's.
        let event = runner::Event {
            agent: agent.clone(),
            run: before,
            out: Out::Gone(String::new()),
        };
        (run.agents).update(cx, |agents, cx| agents.task_said(event, cx));
        assert_eq!(run_of(&run, &agent, cx), now);
        assert_eq!(run.state(&agent, cx), State::Working);
        assert_eq!(stands(&run, &agent, cx), (to, true, Vec::new()));
        before = now;
    }
    // The run before was sent nothing of the change.
    assert_eq!(sent(&run, 1, 1).len(), 1);
}

/// A change that leaves the wall as it is ends nothing: a task whose files are attached copies
/// alone reads them at every level, so its change of level is told to the run it has.
#[gpui::test]
fn a_change_that_leaves_the_wall_as_it_is_ends_nothing(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("same", attached(), cx);
    let (args, _) = run.ran(1);
    assert!(walled(&args), "{args:?}");
    let before = run_of(&run, &agent, cx);
    set(&run, &agent, Tier::Full, cx);
    set(&run, &agent, Tier::Look, cx);
    assert_eq!(walls.asked(), [(Tier::Work, false, true)]);
    assert_eq!(run_of(&run, &agent, cx), before);
    let told = sent(&run, 1, 3);
    assert_eq!(told[1], tier::changed_turn(Tier::Work, Tier::Full));
    assert_eq!(told[2], tier::changed_turn(Tier::Full, Tier::Look));
    assert_eq!(stands(&run, &agent, cx), (Tier::Look, true, Vec::new()));
}

/// A kept task that is given a folder, which no road does yet and the test does by hand.
fn kept_with_a_folder(name: &str, cx: &mut TestAppContext) -> (Run, Walls, String) {
    let (run, walls) = (Run::new(name, cx), Walls::default());
    (run.agents).update(cx, |agents, _| agents.tasks.wall = Some(walls.maker()));
    let agent = run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    assert!(after(&args, "--session-id").is_some(), "{args:?}");
    (run.agents).update(cx, |agents, _| {
        agents.task_mut(&agent).unwrap().files = folder();
    });
    (run, walls, agent)
}

/// A start inside a wall from the task's own words, with `reply` the last of them: it goes on
/// with no conversation and keeps none, and ilo's words about a change of level aren't in it.
fn starts_from_its_words(args: &[String], first: &str, reply: &str) {
    assert!(walled(args), "{args:?}");
    assert_eq!(after(args, "--resume"), None);
    assert_eq!(after(args, "--session-id"), None);
    assert!(first.contains(ASKED), "{first}");
    assert!(first.contains(&format!("Me: {reply}")), "{first}");
    assert!(!first.contains("changed this task's access"), "{first}");
}

/// A task with files never goes on with a conversation its assistant kept, which it can't reach
/// inside a wall: a reply after its run went starts it from its own words.
#[cfg(unix)]
#[gpui::test]
fn a_task_with_files_never_goes_on_with_a_kept_conversation(cx: &mut TestAppContext) {
    let (run, walls, agent) = kept_with_a_folder("unkept", cx);
    run.finish(&agent, cx);
    run.said(&agent, Out::Gone(String::new()), cx);
    let reply = "Are there two together for Sunday too?";
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, reply, cx));
    let (args, first) = run.ran(2);
    assert_eq!(walls.asked(), [(Tier::Work, true, false)]);
    starts_from_its_words(&args, &first, reply);
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, true, Vec::new()));
}

/// A reply to a task that ended at Full access takes it up lower (kept.rs). For a task with a
/// folder that changes its wall: the run inside Full access's wall is over and is sent nothing
/// of the reply, which starts one inside the lower level's wall from the task's own words. Its
/// assistant is told nothing of the access, as at any reply that takes a task up.
#[cfg(unix)]
#[gpui::test]
fn a_level_lowered_at_a_reply_starts_it_inside_the_lower_levels_wall(cx: &mut TestAppContext) {
    let (run, walls, agent) = kept_with_a_folder("reply", cx);
    // The user gives it the highest access while it works, and it ends there.
    set(&run, &agent, Tier::Full, cx);
    let (args, _) = run.ran(2);
    assert_eq!(after(&args, "--resume"), None);
    assert_eq!(walls.asked(), [(Tier::Full, true, false)]);
    let at_full = run_of(&run, &agent, cx);
    run.finish(&agent, cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));

    let reply = "Are there two together for Sunday too?";
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, reply, cx));
    let (args, first) = run.ran(3);
    assert_eq!(walls.asked().last(), Some(&(Tier::Work, true, false)));
    assert_eq!(walls.asked().len(), 2);
    starts_from_its_words(&args, &first, reply);
    assert_ne!(run_of(&run, &agent, cx), at_full);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, true, Vec::new()));
    // The run inside Full access's wall was sent what started it and no more.
    assert_eq!(sent(&run, 2, 1).len(), 1);
}

/// A raise whose wall isn't put up changes nothing: the task runs on as it was, at its level,
/// told nothing, and the log has no change.
#[gpui::test]
fn a_raise_whose_wall_isnt_put_up_leaves_the_task_as_it_runs(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("raise", folder(), cx);
    run.ran(1);
    let before = (run_of(&run, &agent, cx), logged(&run, &agent, cx));
    walls.refuses.set(true);
    set(&run, &agent, Tier::Full, cx);
    assert_eq!(walls.asked().last(), Some(&(Tier::Full, true, false)));
    let stays = Rewalled::Stays {
        at: Tier::Work,
        refused: Tier::Full,
        files: folder(),
    };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Work, true, vec![(0, stays)])
    );
    assert_eq!((run_of(&run, &agent, cx), logged(&run, &agent, cx)), before);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(sent(&run, 1, 1).len(), 1);
    // ilo's line says the level it stays at and the one it couldn't have, and no line says the
    // user set it.
    let kept = IloKind::KeptAt {
        at: Tier::Work,
        refused: Tier::Full,
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [kept]);
}

/// A start whose wall isn't put up goes on without its files: ilo's line says which folder it
/// started without, and the log has the event with no name in it. Files the user attached have
/// no name a line says yet, so theirs is the log's event alone.
#[gpui::test]
fn a_start_whose_wall_isnt_put_up_says_what_it_started_without(cx: &mut TestAppContext) {
    for (name, files, said) in [
        (
            "start-folder",
            folder(),
            Some(LeftOut::Folder(FOLDER.into())),
        ),
        ("start-attached", attached(), None),
    ] {
        let (run, walls) = (Run::new(name, cx), Walls::default());
        walls.refuses.set(true);
        (run.agents).update(cx, |agents, _| {
            agents.tasks.files = files;
            agents.tasks.wall = Some(walls.maker());
        });
        let agent = run.start(Cli::Claude, cx);
        let (args, _) = run.ran(1);
        assert!(!walled(&args), "{args:?}");
        assert!(!stands(&run, &agent, cx).1);
        let said: Vec<IloKind> = (said.into_iter())
            .map(|left| IloKind::StartedWithout { left })
            .collect();
        assert_eq!(lines(&run, &agent, cx), said, "{name}");
        let logged = logged(&run, &agent, cx);
        assert_eq!(
            logged.last().map(String::as_str),
            Some(WITHOUT_FILES_LOGGED)
        );
        let named = |text: &String| text.contains(FOLDER) || text.contains("nonexistent");
        assert!(!logged.iter().any(named), "{logged:?}");
    }
}

/// What came of a wall is kept with the turn it came in, as an ilo line's is, so its line stands
/// where it happened: one refused after the user's reply is the next turn's, and the first's
/// stays the first's.
#[gpui::test]
fn what_came_of_a_wall_is_kept_with_its_turn(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("turns", folder(), cx);
    run.ran(1);
    walls.refuses.set(true);
    set(&run, &agent, Tier::Full, cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And a hotel", cx));
    set(&run, &agent, Tier::Full, cx);
    let stays = Rewalled::Stays {
        at: Tier::Work,
        refused: Tier::Full,
        files: folder(),
    };
    let kept = stands(&run, &agent, cx).2;
    assert_eq!(kept, vec![(0, stays.clone()), (1, stays)]);
}

/// A lower whose wall isn't put up still lowers the task, at once: it goes on at the new level
/// without its files, which needs no wall.
#[gpui::test]
fn a_lower_whose_wall_isnt_put_up_goes_on_without_its_files(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("lower", folder(), cx);
    run.ran(1);
    let before = run_of(&run, &agent, cx);
    walls.refuses.set(true);
    set(&run, &agent, Tier::Look, cx);
    let (args, first) = run.ran(2);
    assert_eq!(walls.asked().last(), Some(&(Tier::Look, true, false)));
    assert!(!walled(&args), "{args:?}");
    assert!(
        first.ends_with(&tier::changed_turn(Tier::Work, Tier::Look)),
        "{first}"
    );
    assert_ne!(run_of(&run, &agent, cx), before);
    let without = Rewalled::WithoutFiles { files: folder() };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Look, false, vec![(0, without)])
    );
    assert_eq!(run.state(&agent, cx), State::Working);
    // Under the line of the level the user set, ilo's of what the task went on without.
    let lowered = IloKind::TierChanged {
        from: Tier::Work,
        to: Tier::Look,
    };
    let without = IloKind::StartedWithout {
        left: LeftOut::Folder(FOLDER.into()),
    };
    assert_eq!(lines(&run, &agent, cx), [lowered, without]);
    assert_eq!(
        logged(&run, &agent, cx)[1..],
        [
            tier::changed_logged(Tier::Work, Tier::Look),
            WITHOUT_FILES_LOGGED.to_string()
        ]
    );
    // With no files left, a change of level is any task's.
    walls.refuses.set(false);
    let asked = walls.asked().len();
    set(&run, &agent, Tier::Work, cx);
    assert_eq!(walls.asked().len(), asked);
    assert_eq!(sent(&run, 2, 2).len(), 2);
}

/// The system's own no to a raise's wall, which comes once its start was made: the task is back
/// at the level it ran at, inside that level's wall, told nothing of a raise, and the log has
/// that the raise didn't hold.
#[gpui::test]
fn a_raise_the_system_refuses_goes_back_to_its_level(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("late-raise", folder(), cx);
    run.ran(1);
    set(&run, &agent, Tier::Full, cx);
    run.ran(2);
    run.said(&agent, Out::Walled(Err(refused())), cx);
    let (args, first) = run.ran(3);
    assert!(walled(&args), "{args:?}");
    assert!(first.contains(ASKED), "{first}");
    assert!(!first.contains("access"), "{first}");
    let levels: Vec<Tier> = walls.asked().iter().map(|asked| asked.0).collect();
    assert_eq!(levels, [Tier::Work, Tier::Full, Tier::Work]);
    let stays = Rewalled::Stays {
        at: Tier::Work,
        refused: Tier::Full,
        files: folder(),
    };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Work, true, vec![(0, stays)])
    );
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(
        logged(&run, &agent, cx)[1..],
        [
            tier::changed_logged(Tier::Work, Tier::Full),
            tier::changed_logged(Tier::Full, Tier::Work)
        ]
    );
    // A wall that is up is no answer of the task's, and a second no isn't a raise's.
    run.said(&agent, Out::Walled(Ok(())), cx);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(walls.asked().len(), 3);
}

/// The system's own no to a start's wall: the task starts again without its files, with the
/// message it had.
#[gpui::test]
fn a_start_the_system_refuses_goes_on_without_its_files(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("late-start", folder(), cx);
    let (_, asked) = run.ran(1);
    run.said(&agent, Out::Walled(Err(refused())), cx);
    let (args, first) = run.ran(2);
    assert!(!walled(&args), "{args:?}");
    assert_eq!(first, asked);
    assert_eq!(walls.asked().len(), 1);
    let without = Rewalled::WithoutFiles { files: folder() };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Work, false, vec![(0, without)])
    );
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// The user takes back the task's folder, as the menu's row will: the run that had it ends and
/// the task starts again without it, told what its caller says. One whose assistant can't be
/// started again is left stopped, which is kept for its line.
#[gpui::test]
fn files_taken_back_start_it_again_without_them(cx: &mut TestAppContext) {
    const TOLD: &str = "The user took the folder back.";
    let take_back = |run: &Run, agent: &str, cx: &mut TestAppContext| {
        (run.agents).update(cx, |agents, cx| {
            let files = std::mem::take(&mut agents.task_mut(agent).unwrap().files);
            agents.rewall(agent, Change::TookBack(files), TOLD.into(), cx);
        })
    };
    let (run, walls, agent) = started("taken", folder(), cx);
    run.ran(1);
    let before = run_of(&run, &agent, cx);
    take_back(&run, &agent, cx);
    let (args, first) = run.ran(2);
    assert!(!walled(&args), "{args:?}");
    assert!(first.contains(ASKED) && first.ends_with(TOLD), "{first}");
    assert_ne!(run_of(&run, &agent, cx), before);
    assert_eq!(walls.asked().len(), 1);
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, false, Vec::new()));
    assert_eq!(run.state(&agent, cx), State::Working);

    let (run, _, agent) = started("taken-stopped", folder(), cx);
    run.ran(1);
    (run.agents).update(cx, |agents, _| {
        let found = agents.tasks.found.get_mut(&Cli::Claude).unwrap();
        found.program = Some(PathBuf::from("/nonexistent/claude"));
    });
    take_back(&run, &agent, cx);
    let stopped = Rewalled::Stopped { files: folder() };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Work, false, vec![(0, stopped)])
    );
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert!(matches!(run.state(&agent, cx), State::Failed { .. }));
    // ilo's line says why it stopped, and the log has that it did, with no name in it.
    let stopped = IloKind::NotRestarted {
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [stopped]);
    let logged = logged(&run, &agent, cx);
    assert_eq!(logged.last().map(String::as_str), Some(STOPPED_LOGGED));
}

/// Files taken back from a task that isn't at work end its run at once all the same, and its
/// next message starts it without them.
#[gpui::test]
fn files_taken_back_from_a_task_at_rest_end_its_run(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("rest", folder(), cx);
    run.ran(1);
    run.said(&agent, Out::Said("Two seats at 9:05, car 4.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    assert_ne!(run_of(&run, &agent, cx), 0);
    (run.agents).update(cx, |agents, cx| {
        let files = std::mem::take(&mut agents.task_mut(&agent).unwrap().files);
        agents.rewall(&agent, Change::TookBack(files), "Taken back.".into(), cx);
    });
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And on Sunday?", cx));
    let (args, _) = run.ran(2);
    assert!(!walled(&args), "{args:?}");
    assert_eq!(walls.asked().len(), 1);
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// A folder a live task holds, as Settings › Agents › Folders in use lists it: by the name the
/// user saw, beside its task's title.
fn in_use(run: &Run, cx: &mut TestAppContext) -> Vec<(String, String)> {
    (run.agents).read_with(cx, |agents, _| {
        (agents.folders_in_use().into_iter())
            .map(|held| (held.folder.to_string(), held.agent.to_string()))
            .collect()
    })
}

/// Revoke takes a live task's folder at once: Folders in use no longer lists it, its assistant
/// starts again without it, and the conversation alone says so by the folder's name (J2). The log
/// has the act with no name in it.
#[gpui::test]
fn a_revoked_folder_goes_at_once_and_only_the_conversation_names_it(cx: &mut TestAppContext) {
    use crate::agents::task::revoke::REVOKED_LOGGED;
    let (run, walls, agent) = started("revoke", folder(), cx);
    run.ran(1);
    assert_eq!(in_use(&run, cx), [(FOLDER.to_string(), agent.clone())]);
    (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
    assert_eq!(in_use(&run, cx), []);
    assert!(!stands(&run, &agent, cx).1, "the task has no files left");
    let (args, first) = run.ran(2);
    assert!(!walled(&args), "{args:?}");
    let told = crate::agents::task::revoke::revoked_told(true, false);
    assert!(first.ends_with(&told), "{first}");
    assert_eq!(
        walls.asked().len(),
        1,
        "no wall for a task with nothing left"
    );
    let revoked = IloKind::Revoked {
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [revoked]);
    // The log has its own words alone, never the folder's name or where it is.
    let started = tier::started_logged(Tier::default(), Default::default());
    assert_eq!(logged(&run, &agent, cx), [started.as_str(), REVOKED_LOGGED]);
    // Nothing to revoke a second time, and no second line.
    (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
    assert_eq!(lines(&run, &agent, cx).len(), 1);
}

/// A folder that's no longer where it was ends its grant at the task's start, and the task goes on
/// without it, saying so once (J6) and not as a wall ilo couldn't put up.
#[gpui::test]
fn a_folder_gone_at_the_start_ends_its_grant_and_the_task_goes_on(cx: &mut TestAppContext) {
    use crate::agents::wall::Refusal;
    let run = Run::new("gone", cx);
    let asked: Rc<RefCell<Vec<bool>>> = Rc::default();
    let put_up: PutUp = {
        let asked = asked.clone();
        Rc::new(move |wall: &WallFor| {
            asked.borrow_mut().push(wall.folder.is_some());
            match wall.folder {
                Some(_) => Err(WallRefused::Refused(Refusal::Gone)),
                None => Ok(Wall::for_test(None)),
            }
        })
    };
    (run.agents).update(cx, |agents, _| {
        agents.tasks.files = Given {
            folder: Some(Granted::for_test()),
            attached: Some(PathBuf::from("/nonexistent/attached")),
        };
        agents.tasks.wall = Some(put_up);
    });
    let agent = run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    // The attached copies still go inside a wall, without the folder.
    assert_eq!(*asked.borrow(), [true, false]);
    assert!(walled(&args), "{args:?}");
    let gone = IloKind::FolderGone {
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [gone]);
    assert_eq!(in_use(&run, cx), []);
}

/// Attached files ilo couldn't copy don't go silently: the task starts without them, its line
/// names the one file (X6c), and the start's entry counts them and says they were left out.
#[gpui::test]
fn files_ilo_couldnt_copy_are_said_and_the_start_counts_them(cx: &mut TestAppContext) {
    use crate::agents::files::Picked;
    use crate::agents::task::{AskedOn, Start};
    let run = Run::new("not-copied", cx);
    (run.agents).update(cx, |agents, _| agents.tasks.files_available = true);
    let missing = std::env::temp_dir().join("ilo-test-no-such-dir/seats.pdf");
    let start = Start {
        tier: Tier::default(),
        model: None,
        pages: Vec::new(),
        only_added: false,
        files: Picked {
            files: vec![missing],
            folder: None,
        },
    };
    let page = Some(AskedOn {
        url: "https://www.example.com/trains".into(),
        title: "Trains to Lisbon".into(),
    });
    let agent = (run.agents)
        .update(cx, |agents, cx| {
            agents.start_task(Cli::Claude, ASKED, page, 1, start, cx)
        })
        .expect("started");
    cx.run_until_parked();
    run.ran(1);
    let left = LeftOut::File("seats.pdf".into());
    assert_eq!(lines(&run, &agent, cx), [IloKind::NotCopied { left }]);
    let started = tier::started_logged(
        Tier::default(),
        tier::StartedWith {
            files: 1,
            left_out: true,
            ..Default::default()
        },
    );
    assert_eq!(
        started,
        "Started at Work on pages · 1 file · files left out"
    );
    assert_eq!(logged(&run, &agent, cx), [started]);
}

/// What the assistant is told as its folder goes is one of the lead's texts exactly, and none
/// names a folder, holds a path or gives a reason.
#[test]
fn a_revoked_folders_assistant_is_told_the_leads_words_alone() {
    use crate::agents::task::revoke::revoked_told;
    const REST: &str = "You can no longer read or change anything in it. Don't try to reach it \
                        another way: carry on with the rest, and say what you couldn't finish. \
                        Only the user can give a task a folder.";
    const STILL: &str = " The files attached to this task are still there.";
    let user = format!("The user revoked this task's folder. {REST}");
    let ilo = format!("ilo revoked this task's folder. {REST}");
    let forms = [
        (revoked_told(true, false), user.clone()),
        (revoked_told(true, true), format!("{user}{STILL}")),
        (revoked_told(false, false), ilo.clone()),
        (revoked_told(false, true), format!("{ilo}{STILL}")),
    ];
    for (told, text) in &forms {
        assert_eq!(told, text);
        assert!(
            !told.contains('/') && !told.contains('\\'),
            "a path in {told:?}"
        );
        assert!(
            !told.contains('\u{201c}') && !told.contains('"'),
            "a name in {told:?}"
        );
        for why in [
            "moved",
            "renamed",
            "deleted",
            "ejected",
            "because",
            "couldn't find",
        ] {
            assert!(!told.contains(why), "a reason in {told:?}");
        }
    }
}

/// The folder's chip under a live task's reply field names the folder the task holds, and its
/// Revoke is the user's press on Folders in use: the folder goes, and the chip with it.
#[gpui::test]
fn the_reply_fields_revoke_takes_the_folder_and_its_chip(cx: &mut TestAppContext) {
    use crate::agents::task::TaskAction;
    let (run, _, agent) = started("reply-revoke", folder(), cx);
    run.ran(1);
    let shown = |run: &Run, cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, cx| {
            agents.conversation(&agent, cx).and_then(|talk| talk.folder)
        })
    };
    assert_eq!(shown(&run, cx).as_deref(), Some(FOLDER));
    (run.agents).update(cx, |agents, cx| {
        agents.task_action(&agent, TaskAction::RevokeFolder, cx)
    });
    assert_eq!(in_use(&run, cx), []);
    assert_eq!(shown(&run, cx), None, "the chip outlived the folder");
    let revoked = IloKind::Revoked {
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [revoked]);
}
