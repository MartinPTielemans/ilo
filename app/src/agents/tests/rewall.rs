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
use crate::agents::task::ilo::{IloKind, IloLine, LeftOut, Reach};
use crate::agents::task::rewall::{
    Change, LEFT_RUNNING_LOGGED, PutUp, Rewalled, STOPPED_FILES_LOGGED, STOPPED_FOLDER_LOGGED,
    ended_left_logged,
};
use crate::agents::task::{Line, State, Who};
use crate::agents::tier::{self, Tier};
use crate::agents::wall::leftovers::{SweepFailed, Swept};
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
        // The turn each came in, as the user's messages before it count it.
        let turn = |after: usize| {
            let before = task.messages.iter().take(after);
            before.filter(|message| message.who == Who::User).count()
        };
        let came = (task.rewalled.iter())
            .map(|came| (turn(came.after), came.what.clone()))
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

/// A start's row in the log where it left the task's files out.
fn started_without(tier: Tier, pages: usize) -> String {
    let with = tier::StartedWith {
        pages,
        left_out: true,
        ..Default::default()
    };
    tier::started_logged(tier, with)
}

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
/// of it, which its conversation says once it's stopped.
#[gpui::test]
fn a_task_with_files_starts_inside_its_wall_and_keeps_nothing(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("starts", folder(), cx);
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
    // Stopped, it says so where its reply field was, as nothing of it was kept to go on with.
    (run.agents).update(cx, |agents, cx| agents.stop_task(&agent, cx));
    assert_eq!(run.ilo_lines(&agent, cx).last(), Some(&IloKind::NotKept));
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

/// The same after the user's Stop, which leaves no run to send a reply to once that run has
/// gone: the reply starts the task from its own words inside a wall put up anew, at the level it
/// was stopped at, which nothing lowers as nothing of it was kept.
#[gpui::test]
fn a_reply_to_a_stopped_task_with_files_starts_it_inside_a_new_wall(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("stopped", folder(), cx);
    run.ran(1);
    set(&run, &agent, Tier::Full, cx);
    run.ran(2);
    let stopped = run_of(&run, &agent, cx);
    (run.agents).update(cx, |agents, cx| agents.stop_task(&agent, cx));
    assert_eq!(run.state(&agent, cx), State::Stopped);
    run.said(&agent, Out::Gone(String::new()), cx);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Stopped);
    assert_eq!(walls.asked().len(), 2);

    let reply = "Are there two together for Sunday too?";
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, reply, cx));
    let (args, first) = run.ran(3);
    assert_eq!(walls.asked().len(), 3);
    assert_eq!(walls.asked().last(), Some(&(Tier::Full, true, false)));
    starts_from_its_words(&args, &first, reply);
    let now = run_of(&run, &agent, cx);
    assert!(now != 0 && now != stopped);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(stands(&run, &agent, cx), (Tier::Full, true, Vec::new()));
    // The run that was stopped was sent its stop and nothing of the reply.
    let stopped = std::fs::read_to_string(beside(&run.program, "in.2")).unwrap();
    assert!(!stopped.contains(reply), "{stopped}");
}

/// A reply to a task that ended at Full access takes it up lower (kept.rs). For a task with a
/// folder that changes its wall: the run inside Full access's wall is over and is sent nothing
/// of the reply, which starts one inside the lower level's wall from the task's own words. Its
/// assistant is told nothing of the access, as at any reply that takes a task up.
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
/// started without, and the start's one row in the log ends with that its files were left out,
/// with no name in it. Files the user attached have no name a line says yet, so theirs is the
/// log's row alone.
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
        assert_eq!(logged, [started_without(Tier::Work, 0)], "{name}");
        let named = |text: &String| text.contains(FOLDER) || text.contains("nonexistent");
        assert!(!logged.iter().any(named), "{logged:?}");
    }
}

/// What came of a wall is kept with where it came among the task's messages and steps, as a
/// refusal's line is, so its line stands where it happened: one refused after the user's reply is
/// the next turn's and stands after the reply, and the first's stays before it.
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
    let stood: Vec<&str> = (run.talk(&agent, cx).lines.iter())
        .filter_map(|line| match line {
            Line::You(_) => Some("you"),
            Line::Ilo(_) => Some("ilo"),
            _ => None,
        })
        .collect();
    assert_eq!(stood, ["ilo", "you", "ilo"]);
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
    // The start at the lower level has a row of its own, as no row is written over.
    assert_eq!(
        logged(&run, &agent, cx)[1..],
        [
            tier::changed_logged(Tier::Work, Tier::Look),
            started_without(Tier::Look, 0)
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
    let back = Rewalled::SetBack {
        at: Tier::Work,
        refused: Tier::Full,
        files: folder(),
    };
    assert_eq!(
        stands(&run, &agent, cx),
        (Tier::Work, true, vec![(0, back)])
    );
    assert_eq!(run.state(&agent, cx), State::Working);
    // It was at the new level for a moment, so under the line of the level the user set, ilo's
    // says it set the task back, and none says it kept it.
    let raised = IloKind::TierChanged {
        from: Tier::Work,
        to: Tier::Full,
    };
    let back = IloKind::SetBack {
        at: Tier::Work,
        refused: Tier::Full,
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [raised, back]);
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
    let without = IloKind::StartedWithout {
        left: LeftOut::Folder(FOLDER.into()),
    };
    assert_eq!(lines(&run, &agent, cx), [without]);
    // The start it had made has its row, and the one without its files its own.
    assert_eq!(
        logged(&run, &agent, cx),
        [
            tier::started_logged(Tier::Work, Default::default()),
            started_without(Tier::Work, 0)
        ]
    );
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
        left: LeftOut::Folder(FOLDER.into()),
    };
    assert_eq!(lines(&run, &agent, cx), [stopped]);
    let rows = logged(&run, &agent, cx);
    assert_eq!(rows.last().map(String::as_str), Some(STOPPED_FOLDER_LOGGED));

    // Attached copies taken back say the files in the log, and have no name a line says yet.
    let (run, _, agent) = started("taken-copies", attached(), cx);
    run.ran(1);
    (run.agents).update(cx, |agents, _| {
        let found = agents.tasks.found.get_mut(&Cli::Claude).unwrap();
        found.program = Some(PathBuf::from("/nonexistent/claude"));
    });
    take_back(&run, &agent, cx);
    assert!(matches!(run.state(&agent, cx), State::Failed { .. }));
    let rows = logged(&run, &agent, cx);
    assert_eq!(rows.last().map(String::as_str), Some(STOPPED_FILES_LOGGED));
}

/// What the wait on a raise's restart, which nothing makes yet, and the sweep at a run's end hand
/// a task: each is a line of ilo's in the task's conversation, in the order it came,
/// and what's left running is said by what the task had, the folder by its name and the files
/// the user attached as the task's.
#[gpui::test]
fn a_restart_that_cant_start_and_a_run_left_running_are_lines_of_ilos(cx: &mut TestAppContext) {
    let both = Given {
        attached: attached().attached,
        ..folder()
    };
    let (run, _, agent) = started("left", folder(), cx);
    run.ran(1);
    (run.agents).update(cx, |agents, _| {
        let task = agents.task_mut(&agent).unwrap();
        task.not_raised(Tier::Full);
        task.left_running(folder());
        task.left_running(attached());
        task.left_running(both);
        // A run that had no files left nothing that reaches any.
        task.left_running(Given::default());
        task.ended_left();
    });
    let left = |reach| IloKind::LeftRunning { reach };
    assert_eq!(
        lines(&run, &agent, cx),
        [
            IloKind::NotRaised {
                refused: Tier::Full
            },
            left(Reach::Folder(FOLDER.into())),
            left(Reach::Files),
            left(Reach::Both(FOLDER.into())),
            IloKind::EndedLeft,
        ]
    );
    // The lines are the task's own record: the log has nothing of them from here.
    assert_eq!(
        logged(&run, &agent, cx),
        [tier::started_logged(Tier::Work, Default::default())]
    );
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
        effort: None,
        pages: Vec::new(),
        files: Picked {
            files: vec![missing],
            folder: None,
        },
        tabs: Default::default(),
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
    let ilo = format!(
        "{} revoked this task's folder. {REST}",
        env!("PRODUCT_NAME")
    );
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

/// A folder's name is the user's, and one place says it: ilo's line in the task's conversation.
/// With each of a wall's lines made on a task whose folder has a name no other string holds,
/// nothing else holds it: what the browser was asked, the activity log, the task's card and its
/// bar, a posted notification, each tool's answer to the task and to a client from outside, a
/// start outside a wall, Library's record and every file ilo wrote.
#[gpui::test]
fn a_folders_name_is_in_ilos_line_and_nowhere_else(cx: &mut TestAppContext) {
    use crate::needs_you::{self, Host, Notify};
    #[derive(Default)]
    struct Posted(RefCell<Vec<String>>);
    impl Host for Posted {
        fn badge(&self, _: usize) {}
        fn active(&self) -> bool {
            false
        }
        fn notify(&self, _: u64, title: &str, body: &str) {
            self.0.borrow_mut().push(format!("{title}\n{body}"));
        }
    }
    const NAME: &str = "Saffron ledger";
    let named = || Given {
        folder: Some(Granted::named_for_test(NAME)),
        attached: None,
    };
    let names = |text: &str| text.contains(NAME) || text.contains("nonexistent");
    let posted = Rc::new(Posted::default());

    // Each line, made as its test above makes it, with the runs of its task that started
    // outside a wall.
    type Makes = fn(&Run, &Walls, &str, &mut TestAppContext) -> (IloKind, Vec<usize>);
    let cases: [(&str, bool, Makes); 5] = [
        ("named-start", true, |_, _, _, _| {
            let left = LeftOut::Folder(NAME.into());
            (IloKind::StartedWithout { left }, vec![1])
        }),
        ("named-kept", false, |run, walls, agent, cx| {
            run.ran(1);
            walls.refuses.set(true);
            set(run, agent, Tier::Full, cx);
            let kept = IloKind::KeptAt {
                at: Tier::Work,
                refused: Tier::Full,
                folder: NAME.into(),
            };
            (kept, Vec::new())
        }),
        ("named-back", false, |run, _, agent, cx| {
            run.ran(1);
            set(run, agent, Tier::Full, cx);
            run.ran(2);
            run.said(agent, Out::Walled(Err(refused())), cx);
            run.ran(3);
            let back = IloKind::SetBack {
                at: Tier::Work,
                refused: Tier::Full,
                folder: NAME.into(),
            };
            (back, Vec::new())
        }),
        ("named-stopped", false, |run, _, agent, cx| {
            run.ran(1);
            (run.agents).update(cx, |agents, cx| {
                let found = agents.tasks.found.get_mut(&Cli::Claude).unwrap();
                found.program = Some(PathBuf::from("/nonexistent/claude"));
                let files = std::mem::take(&mut agents.task_mut(agent).unwrap().files);
                agents.rewall(agent, Change::TookBack(files), "Taken back.".into(), cx);
            });
            let left = LeftOut::Folder(NAME.into());
            (IloKind::NotRestarted { left }, Vec::new())
        }),
        // The user's Revoke (J2): its assistant, started again with nothing left to wall, is told
        // without the name.
        ("named-revoked", false, |run, _, agent, cx| {
            run.ran(1);
            (run.agents).update(cx, |agents, cx| agents.revoke_folder(agent, cx));
            let revoked = IloKind::Revoked {
                folder: NAME.into(),
            };
            (revoked, vec![2])
        }),
    ];
    let mut notified = 0;
    for (name, refuses, makes) in cases {
        // Each task's notification is its own, whatever the one before was told.
        cx.update(|cx| needs_you::open(posted.clone(), cx));
        let (run, walls) = (Run::new(name, cx), Walls::default());
        walls.refuses.set(refuses);
        (run.agents).update(cx, |agents, _| {
            agents.tasks.files = named();
            agents.tasks.wall = Some(walls.maker());
        });
        let agent = run.start(Cli::Claude, cx);
        let (line, unwalled) = makes(&run, &walls, &agent, cx);

        // The conversation says it once, in ilo's line, and no other line of it does.
        let said = run.talk(&agent, cx).lines;
        let ilos = |line: &&Line| matches!(line, Line::Ilo(_));
        let held: Vec<&Line> = (said.iter())
            .filter(|line| names(&format!("{line:?}")))
            .collect();
        assert_eq!(held.len(), 1, "{name}: {held:?}");
        assert!(
            matches!(held[0], Line::Ilo(IloLine { kind, .. }) if *kind == line),
            "{name}: {held:?}"
        );
        assert!(said.iter().filter(ilos).count() >= 1, "{name}");

        let mut words: Vec<String> = Vec::new();
        for ran in unwalled {
            let (args, first) = run.ran(ran);
            assert!(!walled(&args), "{name}: {args:?}");
            words.extend(args);
            words.push(first);
        }
        // Its card and its notification, as it is and once it ends, where it still works.
        let mut follow = |cx: &mut TestAppContext| {
            let cards = (run.agents).read_with(cx, |agents, cx| agents.shown_cards(cx));
            cx.update(|cx| needs_you::follow(&cards, Notify::Away, cx));
            format!("{cards:?}")
        };
        words.push(follow(cx));
        let works = run.state(&agent, cx) == State::Working;
        if works {
            run.finish(&agent, cx);
            words.push(follow(cx));
        }
        (run.agents).update(cx, |agents, cx| {
            words.push(format!("{:?}", agents.status()));
            words.push(format!("{:?}", agents.question()));
            words.extend(agents.activity(&agent).map(|entry| format!("{entry:?}")));
            agents.connect(Client::ClaudeCode, cx);
        });
        cx.run_until_parked();
        // Each tool, asked with nothing: what it answers at once, a refusal included.
        let before = words.len();
        for client in [agent.as_str(), "claude-code"] {
            for tool in tools::names(client) {
                if let Ok(answer) = call(&run.agents, client, &tool, json!({}), cx).try_recv() {
                    words.push(answer.text);
                }
            }
        }
        assert!(words.len() > before + 10, "{name}: few tools answered");
        cx.run_until_parked();
        let browser = &run.browser;
        words.push(format!("{:?}", browser.made.borrow()));
        words.push(format!("{:?}", browser.navigated.borrow()));
        words.push(format!("{:?}", browser.checked.borrow()));
        words.push(format!("{:?}", browser.sign_in_pages.borrow()));
        (run.agents).update(cx, |agents, cx| {
            words.extend(agents.activity(&agent).map(|entry| format!("{entry:?}")));
            words.push(format!("{:?}", agents.take_ended(cx)));
            agents.save();
        });
        words.extend(run.files().into_iter().map(|(_, text)| text));
        let posted = posted.0.borrow();
        // A task that ended posted its notification, so one was read.
        assert_eq!(posted.len() > notified, works, "{name}: {posted:?}");
        words.extend(posted[notified..].iter().cloned());
        notified = posted.len();
        for word in &words {
            assert!(!names(word), "{name}: {word}");
        }
    }
    assert_eq!(notified, 4);
}

/// A task whose files ilo copies, from the user's picks on the `+`, on the stand-in assistant.
#[cfg(unix)]
mod copying {
    use super::*;

    /// A folder the test makes, as the panel would give it: beside the build, as ilo may give that on
    /// every system the tests run on, where the system's temporary folder is one ilo never gives on
    /// the Mac.
    fn real_folder(name: &str) -> PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/ilo-test-folders")
            .join(format!("{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    /// A task started from ⌘T with `folder` and one attached file, as the user picks them, its copy
    /// not made yet: nothing has run until the test lets the copy end.
    fn picked(name: &str, folder: PathBuf, cx: &mut TestAppContext) -> (Run, Walls, String) {
        picked_as(name, Granted::at_for_test(folder), cx)
    }

    /// The same with a folder granted as the test made it, which may no longer be itself.
    fn picked_as(name: &str, folder: Granted, cx: &mut TestAppContext) -> (Run, Walls, String) {
        use crate::agents::files::Picked;
        use crate::agents::task::Start;
        let (run, walls) = (Run::new(name, cx), Walls::default());
        let file = beside(&run.program, "seats.pdf");
        std::fs::write(&file, "two seats").unwrap();
        (run.agents).update(cx, |agents, _| {
            agents.tasks.files_available = true;
            agents.tasks.wall = Some(walls.maker());
        });
        let start = Start {
            tier: Tier::default(),
            model: None,
            effort: None,
            pages: Vec::new(),
            files: Picked {
                files: vec![file],
                folder: Some(folder),
            },
            tabs: Default::default(),
        };
        let agent = (run.agents)
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, ASKED, None, 1, start, cx)
            })
            .expect("started");
        (run, walls, agent)
    }

    /// How many times the task's assistant was started.
    fn starts(run: &Run) -> usize {
        (1..)
            .take_while(|n| beside(&run.program, &format!("in.{n}")).exists())
            .count()
    }

    fn copies(run: &Run, agent: &str, cx: &mut TestAppContext) -> PathBuf {
        (run.agents).read_with(cx, |agents, _| {
            agents.tasks_dir().join(format!("{agent} files"))
        })
    }

    /// While ilo copies the files attached to a task, nothing starts its assistant: a Revoke then
    /// takes the folder, says so, and the task starts once, as the copies end, with its files and
    /// without the folder.
    #[gpui::test]
    fn a_revoke_while_the_files_are_copied_gives_one_start_without_the_folder(
        cx: &mut TestAppContext,
    ) {
        let folder = real_folder("revoked-copying");
        let (run, walls, agent) = picked("revoke-copying", folder.clone(), cx);
        (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
        assert_eq!(walls.asked(), [], "a start before the copies ended");
        cx.run_until_parked();
        run.ran(1);
        cx.run_until_parked();
        assert_eq!(starts(&run), 1, "the task started twice");
        assert_eq!(walls.asked(), [(Tier::Work, false, true)]);
        let name = crate::agents::files::shown_name(&folder);
        assert_eq!(
            lines(&run, &agent, cx),
            [IloKind::Revoked {
                folder: name.into()
            }]
        );
        let started = tier::started_logged(
            Tier::Work,
            tier::StartedWith {
                files: 1,
                ..Default::default()
            },
        );
        assert!(
            logged(&run, &agent, cx).contains(&started),
            "{:?}",
            logged(&run, &agent, cx)
        );
        _ = std::fs::remove_dir_all(folder);
    }

    /// A change of level while the files are copied is the task's from then on, and its one start
    /// is at it.
    #[gpui::test]
    fn a_level_change_while_the_files_are_copied_gives_one_start_at_it(cx: &mut TestAppContext) {
        let folder = real_folder("level-copying");
        let (run, walls, agent) = picked("level-copying", folder.clone(), cx);
        set(&run, &agent, Tier::Full, cx);
        assert_eq!(walls.asked(), [], "a wall before the copies ended");
        cx.run_until_parked();
        run.ran(1);
        cx.run_until_parked();
        assert_eq!(starts(&run), 1, "the task started twice");
        assert_eq!(walls.asked(), [(Tier::Full, true, true)]);
        _ = std::fs::remove_dir_all(folder);
    }

    /// Stop while the files are copied starts nothing, and the copies go.
    #[gpui::test]
    fn stop_while_the_files_are_copied_starts_nothing(cx: &mut TestAppContext) {
        use crate::agents::task::TaskAction;
        let folder = real_folder("stop-copying");
        let (run, walls, agent) = picked("stop-copying", folder.clone(), cx);
        (run.agents).update(cx, |agents, cx| {
            _ = agents.task_action(&agent, TaskAction::Stop, cx)
        });
        cx.run_until_parked();
        assert_eq!((starts(&run), walls.asked()), (0, vec![]));
        assert!(!copies(&run, &agent, cx).exists(), "the copies stayed");
        _ = std::fs::remove_dir_all(folder);
    }

    /// At the start the folder is checked again by its real path: one moved, or reached through a
    /// link, since the panel gave it isn't given, and the task starts without it, its line saying so
    /// by the name the user saw (X6). Its files still go.
    #[gpui::test]
    fn a_folder_that_isnt_itself_at_the_start_is_left_out_and_said(cx: &mut TestAppContext) {
        let real = real_folder("linked-real");
        let link = real.with_file_name(format!("linked-{}", std::process::id()));
        _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&real, &link).unwrap();
        // Each was granted while it was the folder, then became a link to one or went.
        let linked = Granted::was_for_test(link.clone(), &real);
        let moved = real_folder("moved");
        let gone = Granted::at_for_test(moved.clone());
        std::fs::remove_dir(&moved).unwrap();
        for (case, folder, granted) in [("link", link.clone(), linked), ("moved", moved, gone)] {
            let (run, walls, agent) = picked_as(&format!("not-itself-{case}"), granted, cx);
            cx.run_until_parked();
            run.ran(1);
            assert_eq!(walls.asked(), [(Tier::Work, false, true)], "{case}");
            let name = crate::agents::files::shown_name(&folder);
            let left = LeftOut::Folder(name.into());
            assert_eq!(
                lines(&run, &agent, cx),
                [IloKind::StartedWithout { left }],
                "{case}"
            );
            let started = tier::started_logged(
                Tier::Work,
                tier::StartedWith {
                    files: 1,
                    left_out: true,
                    ..Default::default()
                },
            );
            assert_eq!(logged(&run, &agent, cx), [started], "{case}");
            assert_eq!(in_use(&run, cx), [], "{case}");
        }
        _ = std::fs::remove_file(link);
        _ = std::fs::remove_dir_all(real);
    }

    /// ilo's copies of a finished task's files go with its conversation, on Remove and on Remove All.
    #[gpui::test]
    fn a_finished_tasks_copies_go_on_remove_and_remove_all(cx: &mut TestAppContext) {
        for all in [false, true] {
            let folder = real_folder(&format!("removed-{all}"));
            let (run, _, agent) = picked(&format!("copies-removed-{all}"), folder.clone(), cx);
            cx.run_until_parked();
            run.ran(1);
            let copies = copies(&run, &agent, cx);
            let made = std::fs::read_dir(&copies).map_or(0, |copied| copied.count());
            assert_eq!(made, 1, "no copy made");
            run.finish(&agent, cx);
            (run.agents).update(cx, |agents, cx| match all {
                true => agents.remove_kept_tasks(cx),
                false => agents.remove_kept_task(&agent, cx),
            });
            cx.run_until_parked();
            assert!(!copies.exists(), "Remove All {all} left the copies");
            _ = std::fs::remove_dir_all(folder);
        }
    }
}

/// What the sweep after a walled run's end found, which comes once that run has ended, so most
/// often with the next under way: a sweep that couldn't end everything is a line naming what
/// that run was given, not what the task has now, and an entry in the log. One that could, a
/// second answer for the same run, and one for a run never walled say nothing.
#[gpui::test]
fn a_sweep_that_couldnt_end_everything_is_a_line_of_what_that_run_had(cx: &mut TestAppContext) {
    let (run, _, agent) = started("swept", folder(), cx);
    run.ran(1);
    let first = run_of(&run, &agent, cx);
    // Taken back, so the run under way has nothing, while the one that ended had the folder.
    (run.agents).update(cx, |agents, cx| {
        let files = std::mem::take(&mut agents.task_mut(&agent).unwrap().files);
        agents.rewall(&agent, Change::TookBack(files), "Taken back.".into(), cx);
    });
    run.ran(2);
    let now = run_of(&run, &agent, cx);
    assert_ne!(now, first);
    let before = logged(&run, &agent, cx);
    // As a walled start keeps them, which the stand-in's start isn't.
    (run.agents).update(cx, |agents, _| {
        let task = agents.task_mut(&agent).unwrap();
        task.ran_walled = vec![(first, folder(), None), (now, Given::default(), None)];
    });
    let swept = |at: u32, answer: Result<Swept, SweepFailed>, cx: &mut TestAppContext| {
        let event = runner::Event {
            agent: agent.clone(),
            run: at,
            out: Out::Swept(answer),
        };
        (run.agents).update(cx, |agents, cx| agents.task_said(event, cx));
    };
    swept(first, Err(SweepFailed::Cap), cx);
    swept(first, Err(SweepFailed::Cap), cx);
    swept(now, Ok(Swept::default()), cx);
    swept(now + 1, Err(SweepFailed::Unlisted), cx);
    let left: Vec<IloKind> = (lines(&run, &agent, cx).into_iter())
        .filter(|line| matches!(line, IloKind::LeftRunning { .. }))
        .collect();
    assert_eq!(
        left,
        [IloKind::LeftRunning {
            reach: Reach::Folder(FOLDER.into())
        }]
    );
    let mut after = before;
    after.push(LEFT_RUNNING_LOGGED.into());
    assert_eq!(logged(&run, &agent, cx), after);
    // Each run's answer is taken once, and the task's run goes on.
    let kept = (run.agents).read_with(cx, |agents, _| {
        agents.task(&agent).unwrap().ran_walled.len()
    });
    assert_eq!(kept, 0);
    assert_eq!(run_of(&run, &agent, cx), now);
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// The run under way as a walled start keeps it, which the stand-in's start isn't, with what it
/// was given.
fn walled_run(run: &Run, agent: &str, had: Given, cx: &mut TestAppContext) -> u32 {
    let at = run_of(run, agent, cx);
    (run.agents).update(cx, |agents, _| {
        agents.task_mut(agent).unwrap().ran_walled = vec![(at, had, None)];
    });
    at
}

/// The sweep after `at`, a run of `agent`'s, answers.
fn swept(
    run: &Run,
    agent: &str,
    at: u32,
    answer: Result<Swept, SweepFailed>,
    cx: &mut TestAppContext,
) {
    let event = runner::Event {
        agent: agent.to_string(),
        run: at,
        out: Out::Swept(answer),
    };
    (run.agents).update(cx, |agents, cx| agents.task_said(event, cx));
}

/// A change that ends a walled run starts the next only once the sweep after it answers clean,
/// so nothing that run left is at work beside the new one. Meanwhile the task is at work with no
/// run: no call of its is taken, nothing its last run still says is its, and the user's message
/// goes with the start.
#[gpui::test]
fn a_raise_after_a_walled_run_starts_once_its_sweep_is_clean(cx: &mut TestAppContext) {
    let (run, walls, agent) = started("waits", folder(), cx);
    run.ran(1);
    let first = walled_run(&run, &agent, folder(), cx);
    set(&run, &agent, Tier::Full, cx);
    assert_eq!(walls.asked().len(), 2);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Working);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And on Sunday?", cx));
    let gone = runner::Event {
        agent: agent.clone(),
        run: first,
        out: Out::Gone(String::new()),
    };
    (run.agents).update(cx, |agents, cx| agents.task_said(gone, cx));
    let may = (run.agents).read_with(cx, |agents, _| agents.may_call(&agent));
    assert!(!may);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Working);
    swept(&run, &agent, first, Ok(Swept::default()), cx);
    let (args, given) = run.ran(2);
    assert!(walled(&args), "{args:?}");
    assert!(given.contains(ASKED), "{given}");
    assert!(given.contains("And on Sunday?"), "{given}");
    assert!(
        given.ends_with(&tier::changed_turn(Tier::Work, Tier::Full)),
        "{given}"
    );
    assert_ne!(run_of(&run, &agent, cx), 0);
    let may = (run.agents).read_with(cx, |agents, _| agents.may_call(&agent));
    assert!(may);
    assert_eq!(stands(&run, &agent, cx), (Tier::Full, true, Vec::new()));
}

/// A kept task whose run left something running carries that run's place's name in its row,
/// nothing more of it. A later launch whose cleanup sweeps the place clean says on the task, in
/// the Archive, that ilo ended what its assistant had left, with an entry of how many, and the row
/// no longer carries the name; the cleanup may answer before the kept tasks are read. A sweep
/// that ended nothing, or a place no row carries, says nothing.
#[gpui::test]
fn a_later_launch_that_ends_what_a_kept_task_left_says_so_on_it(cx: &mut TestAppContext) {
    const PLACE: &str = "0123456789abcdef";
    let run = Run::new("ended-left", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let at = run_of(&run, &agent, cx);
    (run.agents).update(cx, |agents, _| {
        let task = agents.task_mut(&agent).unwrap();
        task.ran_walled = vec![(at, Given::default(), Some(PLACE.into()))];
    });
    swept(&run, &agent, at, Err(SweepFailed::Cap), cx);
    let left = (run.agents).read_with(cx, |agents, _| {
        agents.tasks.kept.of(&agent).unwrap().left.clone()
    });
    assert_eq!(left, [PLACE]);
    run.finish(&agent, cx);
    (run.agents).update(cx, |agents, cx| agents.end_tasks(false, cx));

    let next = Run::again(run.profile.clone(), run.program.clone(), cx);
    let past = (next.agents).read_with(cx, |agents, _| {
        agents.tasks.kept.listed().next().unwrap().to_string()
    });
    assert!(!next.ilo_lines(&past, cx).contains(&IloKind::EndedLeft));
    // The cleanup answers before the kept tasks are read.
    (next.agents).update(cx, |agents, cx| {
        agents.tasks.kept_taken = false;
        let clean = vec![(PLACE.to_string(), 2), ("fedcba9876543210".to_string(), 3)];
        agents.came_clean(clean, cx);
    });
    assert!(!next.ilo_lines(&past, cx).contains(&IloKind::EndedLeft));
    (next.agents).update(cx, |agents, cx| agents.take_old_kept(cx));
    let ended: Vec<_> = (next.ilo_lines(&past, cx).into_iter())
        .filter(|line| *line == IloKind::EndedLeft)
        .collect();
    assert_eq!(ended, [IloKind::EndedLeft]);
    assert_eq!(logged(&next, &past, cx), [ended_left_logged(2)]);
    let left = (next.agents).read_with(cx, |agents, _| {
        agents.tasks.kept.of(&past).unwrap().left.clone()
    });
    assert!(left.is_empty(), "{left:?}");

    // Once more, and with a sweep that ended nothing: nothing more is said.
    (next.agents).update(cx, |agents, cx| {
        agents
            .tasks
            .kept
            .of_mut(&past)
            .unwrap()
            .left
            .push(PLACE.into());
        agents.came_clean(vec![(PLACE.to_string(), 0), (PLACE.to_string(), 4)], cx);
    });
    assert_eq!(logged(&next, &past, cx), [ended_left_logged(2)]);
    let left = (next.agents).read_with(cx, |agents, _| {
        agents.tasks.kept.of(&past).unwrap().left.clone()
    });
    assert!(left.is_empty(), "{left:?}");
}

/// ilo's lines in a task ilo stopped, but for the one any stopped task's conversation that
/// isn't kept has, which this test's task has too.
fn stopped_lines(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<IloKind> {
    let all = run.ilo_lines(agent, cx);
    assert!(all.contains(&IloKind::NotKept), "{all:?}");
    (all.into_iter())
        .filter(|kind| *kind != IloKind::NotKept)
        .collect()
}

/// A sweep that couldn't end everything starts nothing: ilo stops the task, with the line of what
/// didn't start, the level of a raise (under the raise's own) or what a take-back would have gone
/// without, and under it what was left running.
#[gpui::test]
fn a_restart_whose_sweep_isnt_clean_starts_nothing(cx: &mut TestAppContext) {
    let (run, _, agent) = started("raised", folder(), cx);
    run.ran(1);
    let first = walled_run(&run, &agent, folder(), cx);
    set(&run, &agent, Tier::Full, cx);
    swept(&run, &agent, first, Err(SweepFailed::Cap), cx);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Stopped);
    let left = IloKind::LeftRunning {
        reach: Reach::Folder(FOLDER.into()),
    };
    assert_eq!(
        stopped_lines(&run, &agent, cx),
        [
            IloKind::TierChanged {
                from: Tier::Work,
                to: Tier::Full
            },
            IloKind::NotRaised {
                refused: Tier::Full
            },
            left.clone()
        ]
    );
    assert_eq!(
        logged(&run, &agent, cx).last().unwrap(),
        LEFT_RUNNING_LOGGED
    );

    let (run, _, agent) = started("taken", folder(), cx);
    run.ran(1);
    let first = walled_run(&run, &agent, folder(), cx);
    (run.agents).update(cx, |agents, cx| {
        let files = std::mem::take(&mut agents.task_mut(&agent).unwrap().files);
        agents.rewall(&agent, Change::TookBack(files), "Taken back.".into(), cx);
    });
    assert_eq!(run_of(&run, &agent, cx), 0);
    swept(&run, &agent, first, Err(SweepFailed::Missing), cx);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Stopped);
    let stopped = IloKind::NotRestarted {
        left: LeftOut::Folder(FOLDER.into()),
    };
    assert_eq!(stopped_lines(&run, &agent, cx), [stopped, left]);
    let logged = logged(&run, &agent, cx);
    assert_eq!(
        logged[logged.len() - 2..],
        [STOPPED_FOLDER_LOGGED, LEFT_RUNNING_LOGGED]
    );
}

/// Stop while a start waits on a sweep ends the task there: the start never happens, whatever
/// the sweep then says.
#[gpui::test]
fn stop_while_a_restart_waits_drops_it(cx: &mut TestAppContext) {
    let (run, _, agent) = started("stop", folder(), cx);
    run.ran(1);
    let first = walled_run(&run, &agent, folder(), cx);
    set(&run, &agent, Tier::Full, cx);
    (run.agents).update(cx, |agents, cx| agents.stop_task(&agent, cx));
    assert_eq!(run.state(&agent, cx), State::Stopped);
    swept(&run, &agent, first, Ok(Swept::default()), cx);
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert_eq!(run.state(&agent, cx), State::Stopped);
}
