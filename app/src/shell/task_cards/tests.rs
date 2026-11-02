//! The tasks at the sidebar's foot, on a host that records what the shell asks of the core.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{Entity, FontWeight, Modifiers, SharedString, TestAppContext, VisualTestContext};

use super::{Card, Dot, Holds, Shape, chips, shape};
use crate::agents::runner::Cli;
use crate::shell::{Command, FOOTER_HEIGHT, Folders, PLATE_INSET, Shell, Slot, recorder::Recorder};
use crate::tokens::shell::task::{GAP, HEIGHT};

fn card(n: u64, dot: Dot) -> Card {
    Card {
        agent: format!("task-{n}").into(),
        home: 1,
        space: 100 + n,
        cli: Cli::Claude,
        title: "Find a flight to Lisbon".into(),
        status: "Searching flights".into(),
        asks: false,
        dot,
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        retry: false,
        command: None,
    }
}

/// Newest first, as the Agents send them.
fn cards(list: &[(u64, Dot)]) -> Vec<Card> {
    let mut cards: Vec<Card> = list.iter().map(|&(n, dot)| card(n, dot)).collect();
    cards.sort_by_key(|card| std::cmp::Reverse(super::number(&card.agent)));
    cards
}

/// A window with Spaces 1 and 2, showing 1 with its tabs 10 and 11, and `first` its first tasks,
/// at rest in its foot.
fn shell_with(
    cx: &mut TestAppContext,
    first: Vec<Card>,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str| (id, name.to_string().into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space(1, "Space 1"), space(2, "Space 2")], cx);
        shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_task_cards(first, cx);
        shell.foot.plates.iter_mut().for_each(|plate| plate.settle(later()));
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    shell_with(cx, Vec::new())
}

fn still(cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.set_reduce_motion(true));
}

/// Draws the shell again, as the app's next frame does.
fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// The foot's plates as it keeps them, top first, as the log says them.
fn plates(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> String {
    shell.read_with(cx, |shell, _| shell.foot.logged.clone())
}

/// Long after now, when anything that moved has stopped.
fn later() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

/// The agent's question takes a second line when it needs one, and nothing else does. Its chips
/// wrap to two rows; past that, or with an answer wider than the room, none show, and the plate's
/// click opens the conversation, where they do. Open, a chip of its own, always shows, cut to the
/// room.
#[test]
fn a_card_shapes_to_its_question_and_its_chips() {
    // Every character 6 pt wide, in a sidebar 240 wide: 180 for the status, 191 for chips.
    let measure = |text: &SharedString, _: f32, _: FontWeight| text.chars().count() as f32 * 6.;
    let answers = |words: &[&str]| words.iter().map(|word| SharedString::from(word.to_string())).collect();
    let mut asks = card(1, Dot::Waiting);
    asks.asks = true;
    asks.status = "Both are open Saturday. Which would you rather visit?".into();
    asks.answers = answers(&["Gulbenkian", "MAAT"]);
    assert_eq!(shape(&asks, 240., &measure), Shape { lines: 2, rows: vec![vec![0, 1]] });
    let mut working = card(2, Dot::Working);
    working.status = asks.status.clone();
    assert_eq!(shape(&working, 240., &measure), Shape { lines: 1, rows: Vec::new() });

    asks.answers = answers(&["Museum 1", "Museum 2", "Museum 3", "Museum 4"]);
    assert_eq!(shape(&asks, 240., &measure).rows, [vec![0, 1], vec![2, 3]]);
    asks.answers = answers(&["Museum 1", "Museum 2", "Museum 3", "Museum 4", "Museum 5"]);
    assert!(shape(&asks, 240., &measure).rows.is_empty(), "a third row");
    asks.answers = answers(&["The Calouste Gulbenkian Museum's modern wing"]);
    assert!(shape(&asks, 240., &measure).rows.is_empty(), "an answer wider than the room");

    let mut done = card(3, Dot::Done);
    done.open = vec![(30, "tickets.gulbenkian.pt.example.org".into())];
    assert_eq!(shape(&done, 240., &measure).rows, [vec![0]]);
    assert_eq!(chips(&done)[0].label().as_ref(), "Open tickets.gulbenkian.pt.example.org");
    done.open.extend([(31, "maat.pt".into()), (32, "gulbenkian.pt".into())]);
    assert_eq!(chips(&done)[0].label().as_ref(), "Open 3 pages");
    let mut failed = card(4, Dot::Ended);
    failed.command = Some("claude auth login");
    assert_eq!(chips(&failed)[0].label().as_ref(), "Copy");
}

/// The foot is the window's: a task asked from either Space shows in both, in the same place, and
/// the list's rows above stay where they were as tasks come, the list ending over the foot.
#[gpui::test]
fn the_foot_stays_put_through_a_space_switch(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    still(cx);
    let (row, bottom) = shell.read_with(cx, |shell, _| (shell.layout()[&Slot::Tab(10)], shell.list_bottom()));
    let mut elsewhere = card(2, Dot::Working);
    elsewhere.home = 2;
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![elsewhere, card(1, Dot::Working)], cx));
    draw(&shell, cx);
    assert_eq!(plates(&shell, cx), "task-2 task-1");
    let (moved, grown) = shell.read_with(cx, |shell, _| (shell.layout()[&Slot::Tab(10)], shell.list_bottom()));
    assert_eq!(moved, row, "the rows moved");
    assert_eq!(grown, bottom + 2. * (HEIGHT + GAP), "the list doesn't end over the foot");
    let drawn = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| (shell.task_plate_drawn("task-1"), shell.task_plate_drawn("task-2")))
    };
    let before = drawn(&shell, cx);
    assert!(before.0.is_some() && before.1.is_some());
    // The lowest plate sits on the footer, a plate's inset in from each side.
    let window = shell.read_with(cx, |shell, _| (shell.viewport.height, shell.width()));
    let lowest = before.0.unwrap();
    assert_eq!(f32::from(lowest.bottom()), f32::from(window.0) - FOOTER_HEIGHT);
    assert_eq!(f32::from(lowest.left()), PLATE_INSET);
    assert_eq!(f32::from(lowest.size.width), window.1 - 2. * PLATE_INSET);

    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    draw(&shell, cx);
    assert_eq!(plates(&shell, cx), "task-2 task-1", "switching Spaces hid a task");
    assert_eq!(drawn(&shell, cx), before, "the foot moved with the Space");
}

/// Three show, waiting first, then working, then done, newest first in each, in their age's order;
/// a fourth plate says how many more, and opens the foot to all of them, as "Show less" folds it
/// back. The ask's slot takes one of the three places.
#[gpui::test]
fn the_foot_shows_three_and_counts_the_rest(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    let five = cards(&[(1, Dot::Done), (2, Dot::Working), (3, Dot::Waiting), (4, Dot::Done), (5, Dot::Working)]);
    shell.update(cx, |shell, cx| shell.set_task_cards(five.clone(), cx));
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-2");
    shell.update(cx, |shell, cx| shell.set_ask_slot(true, cx));
    assert_eq!(plates(&shell, cx), "more 3 slot task-5 task-3");
    shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));

    // The count's click opens the foot to every task, and again folds it.
    shell.update(cx, |shell, _| shell.foot.plates.iter_mut().for_each(|plate| plate.settle(later())));
    draw(&shell, cx);
    let more = shell.read_with(cx, |shell, _| shell.foot.plates_drawn.borrow().get(&Holds::More.key()).copied());
    cx.simulate_click(more.expect("the count's plate").center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-4 task-3 task-2 task-1");
    assert!(shell.read_with(cx, |shell, _| shell.foot.all));
    // Fewer than four left, nothing's counted, and the next fourth folds them again.
    shell.update(cx, |shell, cx| shell.set_task_cards(five[..3].to_vec(), cx));
    assert_eq!(plates(&shell, cx), "task-5 task-4 task-3");
    shell.update(cx, |shell, cx| shell.set_task_cards(five.clone(), cx));
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-2");
}

/// ⌘T's ask opens an empty slot at the foot, which the bar folds into once the task runs, and the
/// card fills it, so nothing moves. A task that comes from elsewhere while the ask shows takes a
/// plate of its own, and the slot closes with the ask.
#[gpui::test]
fn the_card_takes_the_slot_the_bar_folds_into(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    let row = shell.read_with(cx, |shell, _| shell.layout()[&Slot::Tab(10)]);
    shell.update(cx, |shell, cx| shell.set_ask_slot(true, cx));
    assert_eq!(plates(&shell, cx), "slot");
    let window = shell.read_with(cx, |shell, _| (shell.viewport.height, shell.width()));
    let (target, _, _) = shell.read_with(cx, |shell, _| shell.fold_target()).expect("a slot to fold into");
    assert_eq!(f32::from(target.bottom()), f32::from(window.0) - FOOTER_HEIGHT);
    assert_eq!(f32::from(target.size.height), HEIGHT);
    assert_eq!(f32::from(target.size.width), window.1 - 2. * PLATE_INSET);

    // An agent's task, not ⌘T's, takes its own plate under the slot.
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, Dot::Working)], cx));
    assert_eq!(plates(&shell, cx), "slot task-1");

    shell.update(cx, |shell, cx| {
        shell.fold_started(true, cx);
        shell.set_task_cards(cards(&[(1, Dot::Working), (2, Dot::Working)]), cx);
    });
    assert_eq!(plates(&shell, cx), "task-2 task-1", "the card didn't take the slot");
    let taken = shell.read_with(cx, |shell, _| (shell.foot.slot, shell.foot.plates[0].height.end()));
    assert_eq!(taken, (false, HEIGHT));
    assert_eq!(shell.read_with(cx, |shell, _| shell.layout()[&Slot::Tab(10)]), row, "the rows moved");
    // The bar's going changes nothing now.
    shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));
    assert_eq!(plates(&shell, cx), "task-2 task-1");

    // Left without a task, the slot closes.
    shell.update(cx, |shell, cx| {
        shell.set_ask_slot(true, cx);
        shell.set_ask_slot(false, cx);
    });
    assert_eq!(plates(&shell, cx), "task-2 task-1");
    assert!(shell.read_with(cx, |shell, _| shell.fold_target()).is_none());
}

/// ✕ dismisses a task, its plate folding away, as ⌘Z brings it back and ⌘⇧Z dismisses it again,
/// and ⇧⌘T brings back the task dismissed last.
#[gpui::test]
fn a_dismissed_task_comes_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, Dot::Working)], cx));
    host.take();
    shell.update(cx, |shell, cx| {
        shell.dismiss_task("task-1".into(), cx);
        assert!(shell.undo(cx));
        assert!(shell.redo(cx));
    });
    let done = host.take();
    let tasks: Vec<_> = done.iter().filter(|line| line.starts_with("task ")).collect();
    assert_eq!(tasks, ["task task-1 Dismiss", "task task-1 Restore", "task task-1 Dismiss"]);
    // The Agents take the card away; its plate folds, then goes.
    shell.update(cx, |shell, cx| shell.set_task_cards(Vec::new(), cx));
    assert_eq!(plates(&shell, cx), "");
    let leaving = shell.read_with(cx, |shell, _| shell.foot.plates.iter().filter(|plate| plate.leaving).count());
    assert_eq!(leaving, 1, "it went at once");

    shell.update(cx, |shell, cx| shell.reopen_closed(cx));
    assert!(host.take().contains(&"task task-1 Restore".to_string()));
    // It's back, so ⌘Z has no dismissal of it left to undo.
    shell.update(cx, |shell, cx| shell.undo(cx));
    assert!(!host.take().iter().any(|line| line.starts_with("task ")));

    // A page closed after the dismissal, a task's own too, is what ⇧⌘T reopens.
    shell.update(cx, |shell, cx| {
        shell.dismiss_task("task-1".into(), cx);
        let later = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        let archived = later.unwrap().as_secs_f64() * 1000. + 1.;
        let tab = crate::shell::library::Archived {
            id: 5,
            url: "https://example.com".into(),
            title: "Example".into(),
            archived,
            little: false,
        };
        shell.set_archive(vec![tab], cx);
        host.take();
        shell.reopen_closed(cx);
    });
    assert_eq!(host.take(), ["reopen closed tab"]);
}

/// Clear clears the Space's Today and leaves the tasks, which are the window's.
#[gpui::test]
fn clear_leaves_the_tasks(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_task_cards(cards(&[(5, Dot::Done), (6, Dot::Working)]), cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Clear, "test", cx));
    let cleared = host.take();
    assert!(cleared.contains(&"clear".to_string()), "{cleared:?}");
    assert!(!cleared.iter().any(|line| line.starts_with("task ")), "{cleared:?}");
}

/// A plate's chips sit in its title's column under its status, and each does its action; Open
/// sends its pages flying to their rows, which show as each lands.
#[gpui::test]
fn the_chips_act(cx: &mut TestAppContext) {
    let mut limit = card(4, Dot::Ended);
    limit.retry = true;
    let mut done = card(5, Dot::Done);
    done.open = vec![(30, "maat.pt".into()), (31, "gulbenkian.pt".into())];
    let (shell, host, cx) = shell_with(cx, vec![done, limit]);
    let chip = |n: u64, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.foot.chips_drawn.borrow().get(&(n, 0)).copied()).expect("a chip")
    };
    let retry = chip(4, cx);
    let column = shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-4")).unwrap().left() + gpui::px(super::COLUMN);
    assert_eq!(retry.left() + gpui::px(super::CHIP_X), column, "the chip's words aren't on the title's column");
    cx.simulate_click(retry.center(), Modifiers::none());
    cx.run_until_parked();
    let done = host.take();
    assert!(done.contains(&"task task-4 Retry".to_string()));

    let open = chip(5, cx);
    cx.simulate_click(open.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().contains(&"task task-5 Open".to_string()));
    assert!(shell.read_with(cx, |shell, _| shell.landing(30) && shell.landing(31)), "the rows show before their pages land");
    // The core moves the pages to the top of Today, and they fly there.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![30, 31, 10, 11], Vec::new(), Folders::default(), cx)
    });
    draw(&shell, cx);
    let flying = shell.read_with(cx, |shell, _| (shell.foot.opening.is_none(), shell.foot.ghosts.len()));
    assert_eq!(flying, (true, 2));
    assert!(shell.read_with(cx, |shell, _| shell.foot_moving(Instant::now())));
}

/// With motion reduced, a plate comes at its height at once and fades in, its dot holds still on
/// the ring, nothing draws on the loading dots' clock, and a new status crossfades in place.
#[gpui::test]
fn with_motion_reduced_the_foot_crossfades(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    still(cx);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, Dot::Working)], cx));
    draw(&shell, cx);
    let now = Instant::now();
    shell.read_with(cx, |shell, _| {
        let plate = &shell.foot.plates[0];
        assert_eq!(plate.height.at(now), HEIGHT, "it grew");
        assert!(plate.opacity.at(now) < 1., "it didn't fade in");
        assert!(!shell.foot_live(), "its dot circles");
    });
    let mut stepped = card(1, Dot::Working);
    stepped.status = "Opening maat.pt".into();
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![stepped], cx));
    shell.read_with(cx, |shell, _| {
        let face = shell.foot.plates[0].face.as_ref().unwrap();
        assert!(face.status.going(Instant::now(), crate::tokens::shell::task::motion::CROSSFADE).is_some());
    });
}

/// A foot at rest asks for no frame: done and stopped tasks keep no clock going, and only a
/// working one draws on the loading dots' clock, which a hidden window never draws.
#[gpui::test]
fn an_idle_foot_draws_nothing(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with(cx, cards(&[(1, Dot::Done), (2, Dot::Ended)]));
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(!shell.foot_moving(later()), "the foot moves at rest");
        assert!(!shell.foot_live());
        assert!(!shell.breath.pending(), "a timer is out");
    });
    shell.update(cx, |shell, cx| shell.set_task_cards(cards(&[(1, Dot::Done), (2, Dot::Ended), (3, Dot::Working)]), cx));
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(shell.foot_live(), "the working dot doesn't circle");
        assert!(shell.breath.pending());
    });
    // Done, it stops: the tick draws in on the display's frames, then nothing.
    shell.update(cx, |shell, cx| shell.set_task_cards(cards(&[(1, Dot::Done), (2, Dot::Ended), (3, Dot::Done)]), cx));
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(!shell.foot_live());
        assert!(shell.foot_moving(Instant::now()), "the tick didn't draw in");
        assert!(!shell.foot_moving(later()));
    });
}
