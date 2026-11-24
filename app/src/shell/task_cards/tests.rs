//! The tasks at the sidebar's foot, on a host that records what the shell asks of the core.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, Entity, Modifiers, SharedString, TestAppContext, VisualTestContext};

use super::{
    Card, Chip, Cli, Dot, Holds, Offer, PLATE_TOP, PageWord, chip, group_height, pages_room,
    state_line,
};
use crate::agents::task::Word;
use crate::agents::tier::Tier;
use crate::shell::page_ends::{STOPS_DRAWN, StripStop};
use crate::shell::{
    Command, FOOTER_HEIGHT, Folders, PLATE_HEIGHT, PLATE_INSET, ROW_HEIGHT, Shell, Slot,
    recorder::Recorder,
};

fn card(n: u64, dot: Dot) -> Card {
    Card {
        agent: format!("task-{n}").into(),
        home: 1,
        space: 100 + n,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight to Lisbon".into(),
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_site: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        pages: Vec::new(),
        tier: Default::default(),
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
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
    hold_clock();
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str| {
            (
                id,
                name.to_string().into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            )
        };
        shell.set_spaces(vec![space(1, "Space 1"), space(2, "Space 2")], cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_task_cards(first, cx);
        shell
            .foot
            .groups
            .iter_mut()
            .for_each(|group| group.settle(later()));
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

/// The foot's groups as it keeps them, top first, as the log says them.
fn plates(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> String {
    shell.read_with(cx, |shell, _| shell.foot.logged.clone())
}

/// Long after now, when anything that moved has stopped.
fn later() -> Instant {
    super::clock() + Duration::from_secs(5)
}

/// Holds the foot's clock still at the real now. Each step's motion then starts and is read at the
/// same instant, however long the runner takes between them, until `tick` moves it on (#3462).
fn hold_clock() {
    super::set_clock(Instant::now());
}

/// Moves the foot's clock on by `seconds`.
fn tick(seconds: f32) {
    super::set_clock(super::clock() + Duration::from_secs_f32(seconds));
}

/// The line under a header's title is ilo's: who works on the task, or Full access where it has
/// that, and the task's one word, in the fuller form its card has for the same state. The agent's
/// own words, its step, its question or its result, are never in it: they are the title's alone
/// (#3690).
#[gpui::test]
fn a_header_says_the_agents_words_only_as_its_title(cx: &mut TestAppContext) {
    let said = "I found three flights and booked the cheapest";
    let mut done = card(1, Dot::Done);
    done.status = said.into();
    assert_eq!(state_line(&done).as_ref(), "Claude · Done");
    let mut working = card(2, Dot::Working);
    working.status = "Step 3 · Opening tap.pt".into();
    assert_eq!(state_line(&working).as_ref(), "Claude · Working");
    working.status = "Working · 3 helpers".into();
    assert_eq!(
        state_line(&working).as_ref(),
        "Claude · Working · 3 helpers"
    );
    working.tier = Tier::Full;
    assert_eq!(
        state_line(&working).as_ref(),
        "Full access · Working · 3 helpers"
    );
    assert_eq!(
        super::state_lead(&state_line(&working)),
        "Full access".len()
    );
    assert_eq!(super::state_lead(&state_line(&done)), 0);
    let mut asks = card(3, Dot::Waiting);
    asks.asks = true;
    asks.status = "Which would you rather visit?".into();
    assert_eq!(state_line(&asks).as_ref(), "Claude · Needs you");
    // An ended task says which of the three it is, from its card's word.
    let mut ended = card(4, Dot::Ended);
    ended.status = "Stopped after 6 steps".into();
    assert_eq!(
        state_line(&ended).as_ref(),
        "Claude · Stopped after 6 steps"
    );
    ended.word = Word::Interrupted;
    ended.status = "Claude Code quit".into();
    assert_eq!(state_line(&ended).as_ref(), "Claude · Interrupted");
    ended.word = Word::AtLimit;
    assert_eq!(state_line(&ended).as_ref(), "Claude · At limit");
    // A session's receipt is ilo's line, with its time.
    let mut session = card(5, Dot::Done);
    session.replies = false;
    session.name = "Flights".into();
    session.status = "Done · 10:42".into();
    assert_eq!(state_line(&session).as_ref(), "Flights · Done · 10:42");

    // As the header draws it: the title is the agent's, the line under it is not.
    let (shell, _, cx) = shell_with(cx, vec![done.clone()]);
    shell.read_with(cx, |shell, _| {
        let face = shell.foot.groups[0].face.as_ref().unwrap();
        assert_eq!(face.title.text, done.title);
        assert_eq!(face.status.text.as_ref(), "Claude · Done");
        assert!(!face.status.text.contains(said));
    });
}

/// A page's row ends in one word: Reading where the task works in the page now, Done where it has
/// moved on or finished, and Paused where the user holds the page or the task waits or stopped.
#[gpui::test]
fn a_pages_row_says_one_word(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.read_with(cx, |shell, _| {
        let mut working = card(1, Dot::Working);
        working.pages = vec![30, 31];
        working.page_steps = vec![(31, "Opening maat.pt".into())];
        assert_eq!(shell.page_word(&working, 30), PageWord::Done);
        assert_eq!(shell.page_word(&working, 31), PageWord::Reading);
        // With one page, that's the one it works in.
        working.pages = vec![30];
        working.page_steps.clear();
        assert_eq!(shell.page_word(&working, 30), PageWord::Reading);
        let mut waits = card(2, Dot::Waiting);
        waits.pages = vec![40, 41, 42];
        assert_eq!(shell.page_word(&waits, 40), PageWord::Paused);
        let mut done = card(3, Dot::Done);
        done.pages = vec![50];
        assert_eq!(shell.page_word(&done, 50), PageWord::Done);
        let mut stopped = card(4, Dot::Ended);
        stopped.pages = vec![60];
        assert_eq!(shell.page_word(&stopped, 60), PageWord::Paused);
    });
    // The page the task handed the user says it needs them there, in the task's own word for it;
    // one they took over, and its others, say Paused. Once it works again, the page still theirs
    // is Paused too.
    shell.update(cx, |shell, cx| {
        let status = crate::agents::Status {
            agent: "task-2".into(),
            name: "Claude".into(),
            space: 102,
            doing: SharedString::default(),
            held: vec![(40, "Sign in".into()), (41, SharedString::default())],
        };
        shell.set_agent_status(vec![status], cx);
    });
    shell.read_with(cx, |shell, _| {
        let mut waits = card(2, Dot::Waiting);
        waits.pages = vec![40, 41, 42];
        let words = |card: &Card| [40, 41, 42].map(|tab| shell.page_word(card, tab));
        assert_eq!(
            words(&waits),
            [PageWord::NeedsYou, PageWord::Paused, PageWord::Paused]
        );
        let mut working = card(2, Dot::Working);
        working.pages = vec![40, 41, 42];
        working.page_steps = vec![(42, "Opening maat.pt".into())];
        assert_eq!(
            words(&working),
            [PageWord::Paused, PageWord::Paused, PageWord::Reading]
        );
    });
    assert_eq!(
        [PageWord::Reading, PageWord::Done, PageWord::Paused].map(PageWord::label),
        ["Reading", "Done", "Paused"]
    );
    assert_eq!(PageWord::NeedsYou.label(), Word::NeedsYou.after_name());
}

/// The foot is the window's: a task asked from either Space shows in both, in the same place, and
/// the list's rows above stay where they were as tasks come, the list ending over the foot.
/// A card that turns to wait for the user brings the hidden sidebar out, as an agent's question
/// does, so it reaches them while ilo is in front (#1080). Only the turn does: a card that stays
/// waiting, or one that works, leaves the sidebar where the user put it.
#[gpui::test]
fn a_card_that_turns_to_wait_brings_the_hidden_sidebar_out(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shell_with(cx, cards(&[(1, Dot::Working)]));
    still(cx);
    let hide = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.set_revealed(false, cx);
        });
        cx.run_until_parked();
    };
    let revealed = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    hide(&shell, cx);
    let working = cards(&[(2, Dot::Working), (1, Dot::Working)]);
    shell.update(cx, |shell, cx| shell.set_task_cards(working, cx));
    assert!(
        !revealed(&shell, cx),
        "a working card brought the sidebar out"
    );

    let waiting = cards(&[(2, Dot::Working), (1, Dot::Waiting)]);
    shell.update(cx, |shell, cx| shell.set_task_cards(waiting.clone(), cx));
    cx.run_until_parked();
    assert!(
        revealed(&shell, cx),
        "a card waiting for the user left the sidebar hidden"
    );

    hide(&shell, cx);
    let mut still_waiting = waiting;
    still_waiting[1].status = "Asks which flight".into();
    shell.update(cx, |shell, cx| shell.set_task_cards(still_waiting, cx));
    assert!(
        !revealed(&shell, cx),
        "a card still waiting brought the sidebar out again"
    );
}

#[gpui::test]
fn the_foot_stays_put_through_a_space_switch(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    still(cx);
    let (row, bottom) = shell.read_with(cx, |shell, _| {
        (shell.layout()[&Slot::Tab(10)], shell.list_bottom())
    });
    let mut elsewhere = card(2, Dot::Working);
    elsewhere.home = 2;
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![elsewhere, card(1, Dot::Working)], cx)
    });
    draw(&shell, cx);
    assert_eq!(plates(&shell, cx), "task-2 task-1");
    let (moved, grown) = shell.read_with(cx, |shell, _| {
        (shell.layout()[&Slot::Tab(10)], shell.list_bottom())
    });
    assert_eq!(moved, row, "the rows moved");
    assert_eq!(
        grown,
        bottom + 2. * ROW_HEIGHT,
        "the list doesn't end over the foot"
    );
    let drawn = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            (
                shell.task_plate_drawn("task-1"),
                shell.task_plate_drawn("task-2"),
            )
        })
    };
    let before = drawn(&shell, cx);
    assert!(before.0.is_some() && before.1.is_some());
    // The lowest header's row sits on the footer, its plate a row's: a plate's inset in from each
    // side, centred in the row.
    let window = shell.read_with(cx, |shell, _| (shell.viewport.height, shell.width()));
    let lowest = before.0.unwrap();
    assert_eq!(
        f32::from(lowest.bottom()) + PLATE_TOP,
        f32::from(window.0) - FOOTER_HEIGHT
    );
    assert_eq!(f32::from(lowest.size.height), PLATE_HEIGHT);
    assert_eq!(f32::from(lowest.left()), PLATE_INSET);
    assert_eq!(f32::from(lowest.size.width), window.1 - 2. * PLATE_INSET);

    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    draw(&shell, cx);
    assert_eq!(
        plates(&shell, cx),
        "task-2 task-1",
        "switching Spaces hid a task"
    );
    assert_eq!(drawn(&shell, cx), before, "the foot moved with the Space");
}

/// Three show, waiting first, then working, then done, newest first in each, in their age's order;
/// a row over them says how many more, and opens the foot to all of them, as "Show less" folds it
/// back. The ask's slot takes one of the three places.
#[gpui::test]
fn the_foot_shows_three_and_counts_the_rest(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    let five = cards(&[
        (1, Dot::Done),
        (2, Dot::Working),
        (3, Dot::Waiting),
        (4, Dot::Done),
        (5, Dot::Working),
    ]);
    shell.update(cx, |shell, cx| shell.set_task_cards(five.clone(), cx));
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-2");
    shell.update(cx, |shell, cx| shell.set_ask_slot(true, cx));
    assert_eq!(plates(&shell, cx), "more 3 slot task-5 task-3");
    shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));

    // The count's click opens the foot to every task, and again folds it.
    shell.update(cx, |shell, _| {
        shell
            .foot
            .groups
            .iter_mut()
            .for_each(|group| group.settle(later()))
    });
    draw(&shell, cx);
    let more = shell.read_with(cx, |shell, _| {
        shell
            .foot
            .plates_drawn
            .borrow()
            .get(&Holds::More.key())
            .copied()
    });
    cx.simulate_click(more.expect("the count's row").center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        plates(&shell, cx),
        "more 2 task-5 task-4 task-3 task-2 task-1"
    );
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
    let (target, _, _) = shell
        .read_with(cx, |shell, _| shell.fold_target())
        .expect("a slot to fold into");
    assert_eq!(
        f32::from(target.bottom()) + PLATE_TOP,
        f32::from(window.0) - FOOTER_HEIGHT
    );
    assert_eq!(f32::from(target.size.height), PLATE_HEIGHT);
    assert_eq!(f32::from(target.size.width), window.1 - 2. * PLATE_INSET);

    // An agent's task, not ⌘T's, takes its own plate under the slot.
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    assert_eq!(plates(&shell, cx), "slot task-1");

    shell.update(cx, |shell, cx| {
        shell.fold_started(true, cx);
        shell.set_task_cards(cards(&[(1, Dot::Working), (2, Dot::Working)]), cx);
    });
    assert_eq!(
        plates(&shell, cx),
        "task-2 task-1",
        "the card didn't take the slot"
    );
    let taken = shell.read_with(cx, |shell, _| {
        (shell.foot.slot, shell.foot.groups[0].height.end())
    });
    assert_eq!(taken, (false, ROW_HEIGHT));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.layout()[&Slot::Tab(10)]),
        row,
        "the rows moved"
    );
    // The bar's going changes nothing now.
    shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));
    assert_eq!(plates(&shell, cx), "task-2 task-1");

    // Left without a task, the slot closes.
    shell.update(cx, |shell, cx| {
        shell.set_ask_slot(true, cx);
        shell.set_ask_slot(false, cx);
    });
    assert_eq!(plates(&shell, cx), "task-2 task-1");
    assert!(
        shell
            .read_with(cx, |shell, _| shell.fold_target())
            .is_none()
    );
}

/// ✕ dismisses a task, its plate folding away, as ⌘Z brings it back and ⌘⇧Z dismisses it again,
/// and ⇧⌘T brings back the task dismissed last.
#[gpui::test]
fn a_dismissed_task_comes_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    host.take();
    shell.update(cx, |shell, cx| {
        shell.dismiss_task("task-1".into(), cx);
        assert!(shell.undo(cx));
        assert!(shell.redo(cx));
    });
    let done = host.take();
    let tasks: Vec<_> = done
        .iter()
        .filter(|line| line.starts_with("task "))
        .collect();
    assert_eq!(
        tasks,
        [
            "task task-1 Dismiss",
            "task task-1 Restore",
            "task task-1 Dismiss"
        ]
    );
    // The Agents take the card away; its plate folds, then goes.
    shell.update(cx, |shell, cx| shell.set_task_cards(Vec::new(), cx));
    assert_eq!(plates(&shell, cx), "");
    let leaving = shell.read_with(cx, |shell, _| {
        shell
            .foot
            .groups
            .iter()
            .filter(|group| group.leaving)
            .count()
    });
    assert_eq!(leaving, 1, "it went at once");
    // Once its fold has had its time, the next frame drops it.
    tick(super::GROW.max(super::FADE));
    draw(&shell, cx);
    let left = shell.read_with(cx, |shell, _| shell.foot.groups.len());
    assert_eq!(left, 0, "its plate stayed once it had folded");

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
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(5, Dot::Done), (6, Dot::Working)]), cx)
    });
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Clear, "test", cx));
    let cleared = host.take();
    assert!(cleared.contains(&"clear".to_string()), "{cleared:?}");
    assert!(
        !cleared.iter().any(|line| line.starts_with("task ")),
        "{cleared:?}"
    );
}

/// The one chip under a header is the user's own act, which has no page: Limit's Retry, or its
/// assistant's fix. Its words sit on the title's column and its click does the act. Done's pages
/// are rows now and an ask's answers are the task's view's, so neither makes a chip.
#[gpui::test]
fn the_chip_acts(cx: &mut TestAppContext) {
    let mut limit = card(4, Dot::Ended);
    limit.retry = true;
    let mut done = card(5, Dot::Done);
    done.open = vec![(30, "maat.pt".into()), (31, "gulbenkian.pt".into())];
    let mut asks = card(6, Dot::Waiting);
    asks.asks = true;
    asks.answers = vec!["Gulbenkian".into(), "MAAT".into()];
    assert_eq!((chip(&done), chip(&asks)), (None, None));
    assert_eq!(chip(&limit), Some(Chip::Retry));
    let mut failed = card(7, Dot::Ended);
    failed.fix = Some((Cli::Claude, Offer::SignIn));
    assert_eq!(
        chip(&failed).unwrap().label().as_ref(),
        "Sign In to Claude Code"
    );
    failed.fix = Some((Cli::Codex, Offer::Get));
    assert_eq!(chip(&failed).unwrap().label().as_ref(), "Get Codex");
    assert_eq!(
        group_height(&limit, false) - group_height(&done, false),
        super::CHIP_ROOM,
        "the chip has no room under its header"
    );

    STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
    let (shell, host, cx) = shell_with(cx, vec![asks, done, limit]);
    draw(&shell, cx);
    let drawn = |stop: StripStop| STOPS_DRAWN.with(|drawn| drawn.borrow().get(&stop).copied());
    assert!(drawn(StripStop::Chip(5)).is_none() && drawn(StripStop::Chip(6)).is_none());
    let retry = drawn(StripStop::Chip(4)).expect("Retry");
    let column = shell
        .read_with(cx, |shell, _| shell.task_plate_drawn("task-4"))
        .unwrap()
        .left()
        + gpui::px(super::COLUMN);
    // A stop's bounds are recorded inside its padding, where its words start.
    assert_eq!(
        retry.left(),
        column,
        "the chip's words aren't on the title's column"
    );
    cx.simulate_click(retry.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["task task-4 Retry"]);
}

/// With motion reduced, a group comes at its height at once and fades in, its dot holds still on
/// the ring, nothing draws on the loading dots' clock, and a new state crossfades in place.
#[gpui::test]
fn with_motion_reduced_the_foot_crossfades(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    still(cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    draw(&shell, cx);
    let now = super::clock();
    shell.read_with(cx, |shell, _| {
        let group = &shell.foot.groups[0];
        assert_eq!(group.height.at(now), ROW_HEIGHT, "it grew");
        assert!(group.opacity.at(now) < 1., "it didn't fade in");
        assert!(!shell.foot_live(), "its dot circles");
    });
    let waits = card(1, Dot::Waiting);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![waits], cx));
    shell.read_with(cx, |shell, _| {
        let face = shell.foot.groups[0].face.as_ref().unwrap();
        assert!(
            face.status
                .going(
                    super::clock(),
                    crate::tokens::shell::task::motion::CROSSFADE
                )
                .is_some()
        );
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
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(
            cards(&[(1, Dot::Done), (2, Dot::Ended), (3, Dot::Working)]),
            cx,
        )
    });
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(shell.foot_live(), "the working dot doesn't circle");
        assert!(shell.breath.pending());
    });
    // Done, it stops: the tick draws in on the display's frames, then nothing.
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(
            cards(&[(1, Dot::Done), (2, Dot::Ended), (3, Dot::Done)]),
            cx,
        )
    });
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(!shell.foot_live());
        assert!(shell.foot_moving(super::clock()), "the tick didn't draw in");
        assert!(!shell.foot_moving(later()));
    });
}

/// A window with Space 1, showing tab 10 of its 10 and 11, and a task Space for each of `first`,
/// holding its pages, whose cards are at rest in its foot.
fn with_task_spaces(
    cx: &mut TestAppContext,
    first: Vec<Card>,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    hold_clock();
    let host = Rc::new(Recorder::default());
    for card in &first {
        host.space_agents
            .borrow_mut()
            .insert(card.space, card.agent.clone());
    }
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let mut spaces = vec![(
            1,
            "Space 1".into(),
            None,
            0,
            "".into(),
            "".into(),
            "".into(),
        )];
        let tasks = first.iter().map(|card| {
            (
                card.space,
                card.title.clone(),
                None,
                0,
                "".into(),
                "Claude".into(),
                "".into(),
            )
        });
        spaces.extend(tasks);
        shell.set_spaces(spaces, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        for card in &first {
            let tabs = match card.pages.is_empty() {
                true => vec![card.space * 10],
                false => card.pages.clone(),
            };
            shell.set_tabs(
                card.space,
                Vec::new(),
                tabs,
                Vec::new(),
                Folders::default(),
                cx,
            );
        }
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_task_cards(first, cx);
        shell.slides.clear();
        shell
            .foot
            .groups
            .iter_mut()
            .for_each(|group| group.settle(later()));
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

/// Brings the Archive folds due, as their time does, and draws the frame that folds them.
fn fold_due(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, _| {
        shell
            .foot
            .folds
            .iter_mut()
            .for_each(|(_, at)| *at = super::clock())
    });
    draw(shell, cx);
}

/// The foot's folds waiting, by task.
fn folds(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .foot
            .folds
            .iter()
            .map(|(agent, _)| agent.to_string())
            .collect()
    })
}

fn dismissed(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|line| line.starts_with("task ") && line.ends_with("Dismiss"))
        .collect()
}

/// The foot keeps what's live or unread. A finished task whose conversation the user opened folds
/// into the Archive once they leave it, after a beat: its plate goes at once and a copy flies into
/// the Archive button, which bumps as it lands. One finished while its conversation showed has been
/// seen too. One never opened stays, as does one still working when it was left.
#[gpui::test]
fn a_finished_task_folds_into_the_archive_once_seen(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(
        cx,
        cards(&[(1, Dot::Done), (2, Dot::Done), (3, Dot::Working)]),
    );
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    draw(&shell, cx);
    assert!(
        folds(&shell, cx).is_empty(),
        "it folds while its conversation shows"
    );

    // Back to the Space's tab: it folds a beat later.
    let wait = shell.update(cx, |shell, cx| {
        shell.close_task(cx);
        shell
            .foot
            .folds
            .first()
            .map(|(_, at)| at.saturating_duration_since(super::clock()))
    });
    let wait = wait.expect("it didn't fold").as_secs_f32();
    assert!(
        wait > super::ARCHIVE_LEAVE - 0.05,
        "it folded {wait} s after the user left"
    );
    assert!(dismissed(&host).is_empty(), "it folded at once");
    fold_due(&shell, cx);
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
    let now = super::clock();
    shell.read_with(cx, |shell, _| {
        let group = shell
            .foot
            .groups
            .iter()
            .find(|group| group.holds == Holds::Task("task-1".into()))
            .unwrap();
        assert!(
            group.leaving && group.archived.is_some(),
            "its copy doesn't fly"
        );
        assert_eq!(group.opacity.at(now), 0., "its header stays");
        assert_eq!(shell.archive_flights().len(), 1);
        let bump = shell.foot.bump.expect("the Archive doesn't bump");
        assert_eq!(
            shell.archive_bump(now),
            0.,
            "it bumps before the copy lands"
        );
        let peak = (1..100)
            .map(|t| shell.archive_bump(bump + super::seconds(t as f32 / 100. * super::BUMP)));
        assert!(peak.fold(0., f32::max) > 0.95);
        assert_eq!(
            shell.archive_glyph(bump + super::seconds(super::BUMP)),
            (1., super::NAV)
        );
    });

    // Task-3 is left working, so it stays; it finishes unseen, so it stays still.
    shell.update(cx, |shell, cx| shell.show_task("task-3".into(), cx));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(2, Dot::Done), (3, Dot::Done)]), cx)
    });
    fold_due(&shell, cx);
    assert!(
        folds(&shell, cx).is_empty() && dismissed(&host).is_empty(),
        "an unseen result went"
    );
    assert_eq!(plates(&shell, cx), "task-3 task-2");

    // Finished while its conversation shows, it has been seen: showing task-2 instead leaves it.
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(2, Dot::Done), (3, Dot::Working)]), cx)
    });
    shell.update(cx, |shell, cx| shell.show_task("task-3".into(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(2, Dot::Done), (3, Dot::Done)]), cx)
    });
    shell.update(cx, |shell, cx| shell.show_task("task-2".into(), cx));
    assert_eq!(folds(&shell, cx), ["task-3"]);
    fold_due(&shell, cx);
    assert_eq!(dismissed(&host), ["task task-3 Dismiss"]);
}

/// The strip's Open moving a finished task's last page into Today leaves nothing in its group,
/// which folds into the Archive once the page has landed; with a page left, it stays.
#[gpui::test]
fn opening_a_finished_tasks_last_page_folds_it_after_the_page_lands(cx: &mut TestAppContext) {
    let mut done = card(1, Dot::Done);
    done.open = vec![(30, "maat.pt".into()), (31, "gulbenkian.pt".into())];
    let (shell, host, cx) = with_task_spaces(cx, vec![done.clone()]);
    shell.update(cx, |shell, cx| {
        shell.open_task_pages("task-1".into(), Some(30), None, cx)
    });
    assert!(folds(&shell, cx).is_empty(), "it folded with a page left");
    done.open.remove(0);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![done], cx));

    let from = gpui::Bounds::new(
        gpui::point(gpui::px(400.), gpui::px(500.)),
        gpui::size(gpui::px(80.), gpui::px(22.)),
    );
    host.take();
    shell.update(cx, |shell, cx| {
        shell.open_task_pages("task-1".into(), Some(31), Some(from), cx)
    });
    assert!(host.take().contains(&"task task-1 OpenTab(31)".to_string()));
    assert_eq!(folds(&shell, cx), ["task-1"]);
    assert!(
        shell.read_with(cx, |shell, _| shell.landing(31)),
        "the row shows before its page lands"
    );
    // It waits for the list to have the page's row, then for it to land.
    fold_due(&shell, cx);
    assert!(
        dismissed(&host).is_empty(),
        "it folded before its page flew"
    );
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![30, 31, 10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(shell.foot.opening.is_none() && shell.foot.ghosts.len() == 1);
        assert!(shell.foot_moving(super::clock()));
        let lands = shell
            .foot
            .ghosts
            .iter()
            .map(|ghost| ghost.start + super::seconds(super::GHOST))
            .max()
            .unwrap();
        assert_eq!(
            shell.foot.folds[0].1,
            lands + super::seconds(super::ARCHIVE_OPEN)
        );
    });
    assert!(dismissed(&host).is_empty(), "it folded as its page flew");
}

/// ⌘Z and ⇧⌘T bring a folded task back, as they do one ✕ took, and its group comes back where it
/// was if it's still on its way. Once it has landed nothing is left moving or waiting.
#[gpui::test]
fn a_folded_task_comes_back_and_leaves_nothing_scheduled(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, cards(&[(1, Dot::Done), (2, Dot::Ended)]));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    fold_due(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert!(host.take().contains(&"task task-1 Restore".to_string()));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Done), (2, Dot::Ended)]), cx)
    });
    shell.read_with(cx, |shell, _| {
        let group = shell
            .foot
            .groups
            .iter()
            .find(|group| group.holds == Holds::Task("task-1".into()))
            .unwrap();
        assert!(
            !group.leaving && group.archived.is_none(),
            "its group didn't come back"
        );
    });

    // ⌘W in its conversation folds it too, and ⇧⌘T brings it back.
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
    assert!(
        dismissed(&host).is_empty(),
        "⌘W dismissed it before it folded"
    );
    fold_due(&shell, cx);
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(2, Dot::Ended)]), cx)
    });
    shell.update(cx, |shell, cx| shell.reopen_closed(cx));
    assert!(host.take().contains(&"task task-1 Restore".to_string()));

    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(shell.foot.folds.is_empty());
        assert!(
            !shell.foot_moving(later()),
            "the foot moves once the fold is over"
        );
        assert!(!shell.breath.pending(), "a timer is out");
    });
}

/// With motion reduced, a seen task fades out where it is and the stack closes, with no flight and
/// no bump.
#[gpui::test]
fn with_motion_reduced_a_folded_task_fades(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, cards(&[(1, Dot::Done), (2, Dot::Done)]));
    still(cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    fold_due(&shell, cx);
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(2, Dot::Done)]), cx)
    });
    let now = super::clock();
    shell.read_with(cx, |shell, _| {
        let group = shell
            .foot
            .groups
            .iter()
            .find(|group| group.holds == Holds::Task("task-1".into()))
            .unwrap();
        assert!(group.leaving && group.archived.is_none(), "it flew");
        assert!(
            group.opacity.at(now) > 0. && group.height.at(now) > 0.,
            "it didn't fade where it was"
        );
        assert!(shell.foot.bump.is_none() && shell.archive_flights().is_empty());
    });
}

/// A regular window's foot shows every task, as tasks are the window's: one asked from another
/// window's Space too.
#[gpui::test]
fn a_regular_windows_foot_shows_every_task(cx: &mut TestAppContext) {
    let elsewhere = Card {
        home: 9,
        ..card(3, Dot::Working)
    };
    let (shell, _, cx) = shell_with(cx, vec![elsewhere, card(2, Dot::Working)]);
    assert_eq!(plates(&shell, cx), "task-3 task-2");
}

/// A private window offers no tasks and shows none: its foot stays empty while a regular window
/// shows a task.
#[gpui::test]
fn a_private_windows_foot_is_empty_while_a_regular_window_has_a_task(cx: &mut TestAppContext) {
    let (regular, _, cx) = shell_with(cx, vec![card(2, Dot::Working)]);
    assert_eq!(plates(&regular, cx), "task-2");
    let host = Rc::new(Recorder::default());
    let private = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    private.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(2, Dot::Working)], cx);
        shell
            .foot
            .groups
            .iter_mut()
            .for_each(|group| group.settle(later()));
    });
    cx.run_until_parked();
    assert!(private.read_with(cx, |shell, _| shell.task_cards.is_empty()
        && shell.foot.groups.is_empty()));
    assert_eq!(plates(&regular, cx), "task-2");
}

/// An outside agent's session is numbered with the tasks, so its group, its motion and its close
/// are its own, and two sessions never share one.
#[test]
fn a_sessions_card_has_a_number_of_its_own() {
    use super::number;
    assert_eq!(
        [number("task-2"), number("session-3"), number("session-4")],
        [2, 3, 4]
    );
}

/// A card with `pages` in its Space.
fn paged(n: u64, dot: Dot, pages: &[u64]) -> Card {
    Card {
        pages: pages.to_vec(),
        ..card(n, dot)
    }
}

/// Each live group's height at rest, top first, by its task.
fn heights(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(String, f32)> {
    shell.read_with(cx, |shell, _| {
        (shell.foot.groups.iter())
            .filter(|group| !group.leaving)
            .filter_map(|group| match &group.holds {
                Holds::Task(agent) => Some((agent.to_string(), group.height.end())),
                _ => None,
            })
            .collect()
    })
}

/// Where the foot drew `tab`'s row last.
fn row_drawn(
    shell: &Entity<Shell>,
    tab: u64,
    cx: &mut VisualTestContext,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    shell.read_with(cx, |shell, _| {
        shell.foot.rows_drawn.borrow().get(&tab).copied()
    })
}

/// Several tasks are a group each, the one the page area shows open to its pages and the others
/// closed to their header. A header's click shows its task's view and opens its group, and the
/// group shown before closes; the view put away, every group is closed again.
#[gpui::test]
fn the_group_shown_is_open_and_the_others_are_closed(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Working, &[1020, 1021, 1022, 1023]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two);
    still(cx);
    draw(&shell, cx);
    let closed = [
        ("task-2".to_string(), ROW_HEIGHT),
        ("task-1".to_string(), ROW_HEIGHT),
    ];
    assert_eq!(heights(&shell, cx), closed, "a group not shown is open");
    assert!(row_drawn(&shell, 1020, cx).is_none());

    let header = |agent: &str, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell
            .read_with(cx, |shell, _| shell.task_plate_drawn(agent))
            .expect("its header")
    };
    let at = header("task-2", &shell, cx).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        heights(&shell, cx),
        [
            ("task-2".to_string(), ROW_HEIGHT + pages_room(4)),
            ("task-1".to_string(), ROW_HEIGHT),
        ]
    );
    shell.update(cx, |shell, _| {
        shell
            .foot
            .groups
            .iter_mut()
            .for_each(|group| group.settle(later()))
    });
    draw(&shell, cx);
    // Its rows are a step in from the header's plate, under one another.
    let plate = header("task-2", &shell, cx);
    let rows: Vec<_> = [1020, 1021, 1022, 1023]
        .map(|tab| row_drawn(&shell, tab, cx).expect("its page's row"))
        .to_vec();
    assert!(rows[0].left() > plate.left() && rows[0].top() >= plate.bottom());
    assert!(
        rows.windows(2)
            .all(|pair| pair[1].top() == pair[0].bottom())
    );
    assert_eq!(rows[0].right(), plate.right());

    // The other header's click swaps which is open.
    let at = header("task-1", &shell, cx).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        heights(&shell, cx),
        [
            ("task-2".to_string(), ROW_HEIGHT),
            ("task-1".to_string(), ROW_HEIGHT + pages_room(1)),
        ]
    );
    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert_eq!(heights(&shell, cx), closed);
}

/// The sidebar shows one thing at a time. On the user's own tab its row has the shown plate and
/// nothing at the foot does; in a task's view its header has it; on one of its pages that row has
/// it and the header does not.
#[gpui::test]
fn one_thing_in_the_sidebar_is_shown_at_a_time(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Working, &[1020, 1021]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two);
    still(cx);
    draw(&shell, cx);
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let at_foot = shell.shown_at_foot();
            // Every stop the foot draws that the page area shows: never more than the one.
            let plated = (shell.foot_stops().into_iter())
                .filter(|stop| Some(*stop) == at_foot)
                .count();
            (shell.shown_has_plate(), at_foot, plated)
        })
    };
    assert_eq!(shown(&shell, cx), (true, None, 0));

    shell.update(cx, |shell, cx| shell.show_task("task-2".into(), cx));
    draw(&shell, cx);
    assert_eq!(shown(&shell, cx), (false, Some(StripStop::Header(2)), 1));

    let row = row_drawn(&shell, 1021, cx).expect("its page's row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shown(&shell, cx), (false, Some(StripStop::Row(1021)), 1));

    // Its header's click goes back to the task's view, which has the plate again.
    let header = shell
        .read_with(cx, |shell, _| shell.task_plate_drawn("task-2"))
        .unwrap();
    cx.simulate_click(header.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shown(&shell, cx), (false, Some(StripStop::Header(2)), 1));

    // And on the header shown, it puts the view away, for the user's tab.
    cx.simulate_click(header.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shown(&shell, cx), (true, None, 0));
}

/// A page's row's click stands on that page, as its picture's click in the task's view does, and
/// does nothing else: the core is asked to show the page, the task is told nothing, and an agent's
/// page isn't given the keyboard. Nothing of it is kept either: the foot stores nothing, and the
/// Agents' log, which records what a task did, hears of no click here.
#[gpui::test]
fn a_page_rows_click_watches_its_page_and_nothing_else(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, vec![paged(1, Dot::Working, &[1010, 1011])]);
    still(cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    draw(&shell, cx);
    host.take();
    let row = row_drawn(&shell, 1011, cx).expect("its page's row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page()),
        Some((101, 1011))
    );
    let asked = host.take();
    assert!(asked.contains(&"select 1011".to_string()), "{asked:?}");
    let told = |line: &&String| {
        line.starts_with("task ") || line.starts_with("show task") || line.starts_with("focus page")
    };
    assert!(!asked.iter().any(|line| told(&line)), "{asked:?}");

    // What the same call from the task's view asks of the core is all the row's click asked.
    shell.update(cx, |shell, cx| shell.task_page_back(cx));
    draw(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| {
        assert!(shell.watch_task_page(1011, row, None, cx));
    });
    cx.run_until_parked();
    assert_eq!(asked, host.take());

    // The row of the page watched does nothing more.
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().is_empty());
}

/// Every header and every page's row is reached by the keys. Focus Sidebar rings what the page
/// area shows, at the foot the task's header, and the arrows go over the foot's stops, top first,
/// on from the list's last row and back to it. The mark is the sidebar's focus ring, never the
/// plate, so what the page area shows doesn't change as the ring moves. Return on a row stands on
/// its page, which never gets the keyboard while its agent has it; Return on a header shows its
/// task's view.
#[gpui::test]
fn the_keys_reach_every_header_and_page_row(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Working, &[1020, 1021]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, host, cx) = with_task_spaces(cx, two);
    still(cx);
    cx.update(|window, cx| {
        crate::shell::commands::bind_keys(cx);
        window.focus(&shell.read(cx).focus_handle.clone(), cx);
    });
    let ring = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| (shell.keyed, shell.keyed_strip))
    };
    let press = |key: &str, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        ring(shell, cx)
    };
    let focus_sidebar = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        draw(shell, cx);
        ring(shell, cx)
    };

    // With the user's tab shown every group is closed: the arrows go from the list's last row
    // over the headers.
    assert_eq!(focus_sidebar(&shell, cx), (Some(10), None));
    let closed: Vec<_> = (0..4).map(|_| press("down", &shell, cx)).collect();
    assert_eq!(
        closed,
        [
            (Some(11), None),
            (None, Some(StripStop::Header(2))),
            (None, Some(StripStop::Header(1))),
            (None, Some(StripStop::Header(1))),
        ]
    );
    // Return on a header shows its task's view, which opens its group.
    press("up", &shell, cx);
    press("enter", &shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.talk_agent()),
        Some("task-2".into())
    );
    draw(&shell, cx);
    let foot = [
        StripStop::Header(2),
        StripStop::Row(1020),
        StripStop::Row(1021),
        StripStop::Header(1),
    ];
    assert_eq!(shell.read_with(cx, |shell, _| shell.foot_stops()), foot);

    // Focus Sidebar keeps the view and rings its header, and the arrows go over its pages' rows.
    let stops = foot.map(|stop| (None, Some(stop)));
    assert_eq!(focus_sidebar(&shell, cx), stops[0]);
    let down: Vec<_> = (0..4).map(|_| press("down", &shell, cx)).collect();
    assert_eq!(down, [stops[1], stops[2], stops[3], stops[3]]);
    // The ring moved over all of them, and what's shown stayed the task's view.
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.shown_at_foot()),
        Some(StripStop::Header(2))
    );
    STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
    draw(&shell, cx);
    STOPS_DRAWN.with(|drawn| {
        let drawn = drawn.borrow();
        assert!(foot.iter().all(|stop| drawn.contains_key(stop)));
    });
    let up: Vec<_> = (0..4).map(|_| press("up", &shell, cx)).collect();
    assert_eq!(
        up,
        [stops[2], stops[1], stops[0], (Some(11), None)],
        "Up didn't go back over the foot to the list"
    );

    // Return on a page's row stands on that page. Its agent has it, so the keys stay with the
    // ring, on the row.
    for _ in 0..3 {
        press("down", &shell, cx);
    }
    host.take();
    assert_eq!(press("enter", &shell, cx), stops[2]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page()),
        Some((102, 1021))
    );
    let asked = host.take();
    assert!(asked.contains(&"select 1021".to_string()), "{asked:?}");
    assert!(
        !asked.iter().any(|line| line.starts_with("focus page")),
        "the agent's page got the keyboard: {asked:?}"
    );
    // Focus Sidebar there rings the row of the page shown.
    press("up", &shell, cx);
    assert_eq!(focus_sidebar(&shell, cx), stops[2]);
    // With the page watched the list's rows aren't stops: Up ends at the foot's first.
    let up: Vec<_> = (0..3).map(|_| press("up", &shell, cx)).collect();
    assert_eq!(up, [stops[1], stops[0], stops[0]]);
    // Return on its header goes back to the task's view.
    press("enter", &shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((102, None))
    );
}

/// The limit of three counts headers, not rows: a group open to four pages is still one of the
/// three, and the row over them says how many more tasks there are. The task the page area shows
/// is always one of the three, so what's shown is in the sidebar.
#[gpui::test]
fn the_count_is_of_headers(cx: &mut TestAppContext) {
    let five = vec![
        paged(5, Dot::Working, &[1050]),
        paged(4, Dot::Done, &[1040]),
        paged(3, Dot::Waiting, &[1030]),
        paged(2, Dot::Working, &[1020, 1021, 1022, 1023]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, five);
    still(cx);
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-2");
    shell.update(cx, |shell, cx| shell.show_task("task-2".into(), cx));
    draw(&shell, cx);
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-2");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.more_words().as_ref(), "2 more tasks");
        let stops = shell.foot_stops();
        let headers = |stop: &&StripStop| matches!(stop, StripStop::Header(_));
        let rows = |stop: &&StripStop| matches!(stop, StripStop::Row(_));
        assert_eq!(stops.iter().filter(headers).count(), 3);
        assert_eq!(stops.iter().filter(rows).count(), 4);
        assert_eq!(stops[0], StripStop::More);
    });
    // A done task ranks last, and shown it still has its place.
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    assert_eq!(plates(&shell, cx), "more 2 task-5 task-3 task-1");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.shown_at_foot()),
        Some(StripStop::Header(1))
    );
}

/// An ended task keeps its group while it has pages open, seen or not, and the group goes into the
/// Archive as the last of them closes.
#[gpui::test]
fn an_ended_task_keeps_its_group_while_it_has_pages_open(cx: &mut TestAppContext) {
    let done = paged(1, Dot::Done, &[1010, 1011]);
    let (shell, host, cx) = with_task_spaces(cx, vec![done.clone()]);
    still(cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert!(
        folds(&shell, cx).is_empty(),
        "seen, it folded with its pages open"
    );
    let left = |pages: &[u64]| vec![paged(1, Dot::Done, pages)];
    shell.update(cx, |shell, cx| shell.set_task_cards(left(&[1011]), cx));
    assert!(folds(&shell, cx).is_empty(), "it folded with a page left");
    assert_eq!(plates(&shell, cx), "task-1");
    shell.update(cx, |shell, cx| shell.set_task_cards(left(&[]), cx));
    fold_due(&shell, cx);
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
    // A working task whose pages close stays: it isn't over.
    let working = |pages: &[u64]| vec![paged(2, Dot::Working, pages)];
    shell.update(cx, |shell, cx| shell.set_task_cards(working(&[1020]), cx));
    shell.update(cx, |shell, cx| shell.set_task_cards(working(&[]), cx));
    assert!(folds(&shell, cx).is_empty());
}
