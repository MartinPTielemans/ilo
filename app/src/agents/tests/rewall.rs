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
use crate::agents::task::State;
use crate::agents::task::rewall::{Change, PutUp, Rewalled};
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
fn stands(run: &Run, agent: &str, cx: &mut TestAppContext) -> (Tier, bool, Vec<Rewalled>) {
    (run.agents).read_with(cx, |agents, _| {
        let task = agents.task(agent).unwrap();
        (task.tier, task.files.any(), task.rewalled.clone())
    })
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
    };
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, true, vec![stays]));
    assert_eq!((run_of(&run, &agent, cx), logged(&run, &agent, cx)), before);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(sent(&run, 1, 1).len(), 1);
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
    assert_eq!(stands(&run, &agent, cx), (Tier::Look, false, vec![without]));
    assert_eq!(run.state(&agent, cx), State::Working);
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
    };
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, true, vec![stays]));
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
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, false, vec![without]));
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
    assert_eq!(stands(&run, &agent, cx), (Tier::Work, false, vec![stopped]));
    assert_eq!(run_of(&run, &agent, cx), 0);
    assert!(matches!(run.state(&agent, cx), State::Failed { .. }));
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
