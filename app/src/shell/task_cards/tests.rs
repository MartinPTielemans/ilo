//! The tasks' cards in Today's list, on a host that records what the shell asks of the core.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, Entity, Modifiers, SharedString, TestAppContext, VisualTestContext};

use super::card::{Look, count, look, second_lead, second_line, spoken};
use super::glyph::Mark;
use super::{Card, Chip, Cli, Dot, Form, Offer, PageMark, PageWord, chip, state_line};
use crate::agents::task::{FINDING_KEPT, Message, TOLD_KEPT, Who, Word, finding, told, told_line};
use crate::agents::tier::Tier;
use crate::shell::page_ends::{STOPS_DRAWN, StripStop};
use crate::shell::{Command, Folders, Shell, Slot, recorder::Recorder};

pub(in crate::shell) fn card(n: u64, dot: Dot) -> Card {
    Card {
        kept_picture: None,
        agent: format!("task-{n}").into(),
        home: 1,
        space: 100 + n,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight to Lisbon".into(),
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_tab: None,
        line: None,
        ended_at: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        pages: Vec::new(),
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
        visited: Vec::new(),
        ended_time: Default::default(),
        fuller: false,
        finding: Default::default(),
        told: Default::default(),
    }
}

/// Newest first, as the Agents send them.
fn cards(list: &[(u64, Dot)]) -> Vec<Card> {
    let mut cards: Vec<Card> = list.iter().map(|&(n, dot)| card(n, dot)).collect();
    cards.sort_by_key(|card| std::cmp::Reverse(super::number(&card.agent)));
    cards
}

/// A card with `pages` in its Space.
fn paged(n: u64, dot: Dot, pages: &[u64]) -> Card {
    Card {
        pages: pages.to_vec(),
        ..card(n, dot)
    }
}

/// A window with Spaces 1 and 2, showing 1 with its tabs 10 and 11, and `first` its first tasks.
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
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    shell_with(cx, Vec::new())
}

/// A window with Space 1, showing tab 10 of its 10 and 11, and a task Space for each of `first`,
/// holding its pages.
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
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn still(cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.set_reduce_motion(true));
}

/// Draws the shell again, as the app's next frame does.
fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// What Today lists in the Space shown, top first, a card by its task and a tab by its id, as
/// the list lays them out.
fn today(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let tabs = shell.rows().1;
        let mut rows: Vec<(f32, String)> = (shell.layout().into_iter())
            .filter_map(|(slot, top)| match slot {
                Slot::Task(super::SLOT) => Some((top, "slot".to_string())),
                Slot::Task(n) => Some((top, format!("task-{n}"))),
                Slot::Tab(tab) if tabs.contains(&tab) => Some((top, tab.to_string())),
                _ => None,
            })
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        rows.into_iter().map(|(_, row)| row).collect()
    })
}

/// The form of the card of `agent` in the Space shown.
fn form(shell: &Entity<Shell>, agent: &str, cx: &mut VisualTestContext) -> Option<Form> {
    shell.read_with(cx, |shell, _| {
        let space = shell.shown_space()?;
        let placed = space
            .placed
            .iter()
            .find(|card| card.agent.as_ref() == agent)?;
        Some(placed.form)
    })
}

/// Long after now, when anything that moved has stopped.
fn later() -> Instant {
    super::clock() + Duration::from_secs(5)
}

/// Holds the cards' clock still at the real now. Each step's motion then starts and is read at the
/// same instant, however long the runner takes between them, until `tick` moves it on (#3462).
fn hold_clock() {
    super::set_clock(Instant::now());
}

/// Moves the cards' clock on by `seconds`.
fn tick(seconds: f32) {
    super::set_clock(super::clock() + Duration::from_secs_f32(seconds));
}

fn dismissed(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|line| line.starts_with("task ") && line.ends_with("Dismiss"))
        .collect()
}

/// The line under a card's title is ilo's: who works on the task, or Full access where it has
/// that, and the task's one word; waiting the page's site and Needs you; done when it ended. The
/// agent's own words, its question or its result, are never in it: they are the title's alone
/// (#3690).
#[gpui::test]
fn a_card_says_the_agents_words_only_as_its_title(cx: &mut TestAppContext) {
    let said = "I found three flights and booked the cheapest";
    let mut done = card(1, Dot::Done);
    done.status = said.into();
    assert_eq!(state_line(&done).as_ref(), "Claude · Done");
    let mut working = card(2, Dot::Working);
    working.status = "Step 3 · Opening tap.pt".into();
    assert_eq!(state_line(&working).as_ref(), "Claude · Working");
    working.status = "Working · 3 helpers".into();
    working.fuller = true;
    assert_eq!(
        state_line(&working).as_ref(),
        "Claude · Working · 3 helpers"
    );
    working.tier = Tier::Full;
    assert_eq!(
        state_line(&working).as_ref(),
        "Full access · Working · 3 helpers"
    );
    assert_eq!(super::state(&working).1, Tier::Full.label().len());
    assert_eq!(super::state(&done).1, 0);
    let mut asks = card(3, Dot::Waiting);
    asks.asks = true;
    asks.status = "Which would you rather visit?".into();
    assert_eq!(state_line(&asks).as_ref(), "Claude · Needs you");
    // An ended task says which of the three it is, from its card's word.
    let mut ended = card(4, Dot::Ended);
    ended.status = "Stopped after 6 steps".into();
    ended.fuller = true;
    assert_eq!(
        state_line(&ended).as_ref(),
        "Claude · Stopped after 6 steps"
    );
    ended.word = Word::Interrupted;
    ended.status = "Claude Code quit".into();
    ended.fuller = false;
    assert_eq!(state_line(&ended).as_ref(), "Claude · Interrupted");
    ended.word = Word::AtLimit;
    assert_eq!(state_line(&ended).as_ref(), "Claude · At limit");
    // A session's receipt is ilo's line, with its time.
    let mut session = card(5, Dot::Done);
    session.replies = false;
    session.name = "Flights".into();
    session.status = "Done · 10:42".into();
    session.fuller = true;
    assert_eq!(state_line(&session).as_ref(), "Flights · Done · 10:42");

    // As a whole card says them: waiting, the site and Needs you, never the question; done, when
    // it ended, never the result.
    assert_eq!(
        second_line(&asks, Form::Waits, "maat.pt").as_ref(),
        "maat.pt · Needs you"
    );
    assert_eq!(second_line(&asks, Form::Waits, "").as_ref(), "Needs you");
    done.ended_time = "13:31".into();
    assert_eq!(
        second_line(&done, Form::DoneUnseen, "").as_ref(),
        "Done · 13:31"
    );
    for form in [Form::Waits, Form::DoneUnseen, Form::Ended] {
        assert!(!second_line(&done, form, "").contains(said), "{form:?}");
    }
    let (shell, _, cx) = shell_with(cx, vec![done]);
    assert_eq!(today(&shell, cx), ["task-1", "10", "11"]);
}

/// How much of a card's line is in the title's ink is its level's: a card at Full access leads its
/// state's line with the level's label, and a card at any other level has no lead, though its
/// client is named as the label and its line so begins as that one does. A line that isn't its
/// state's, its step at work, has none at either level.
#[test]
fn a_cards_lead_is_its_level_never_its_lines_opening_words() {
    let label = Tier::Full.label();
    let mut posing = card(1, Dot::Ended);
    posing.name = label.into();
    let mut full = card(2, Dot::Ended);
    full.tier = Tier::Full;
    assert_eq!(state_line(&posing), state_line(&full));
    for form in [Form::Ended, Form::EndedLine, Form::DoneUnseen] {
        assert_eq!(second_lead(&posing, form), 0, "{form:?}");
        assert_eq!(second_lead(&full, form), label.len(), "{form:?}");
    }
    // At work the line is the step, which for a session begins with its client's name.
    for card in [&mut posing, &mut full] {
        card.dot = Dot::Working;
        card.status = format!("{label} · Opening tap.pt").into();
        assert_eq!(second_lead(card, Form::Working), 0);
    }
}

/// Whether a card's line says its status in its word's place is a fact its maker set, never read
/// off the status: words that begin as ilo's own fuller forms do, a session's client named as a
/// state among them, leave the line at the word.
#[test]
fn a_cards_line_says_its_status_only_where_its_maker_said_so() {
    let posing = [
        (Dot::Working, "Working · Opening tap.pt", "Claude · Working"),
        (Dot::Ended, "Stopped by a page", "Claude · Stopped"),
        (Dot::Done, "Done · 3 flights found", "Claude · Done"),
    ];
    for (dot, status, line) in posing {
        for replies in [true, false] {
            let mut card = card(1, dot);
            card.status = status.into();
            card.replies = replies;
            assert_eq!(state_line(&card).as_ref(), line, "{status:?}");
        }
    }
    // And where it did, the line is the status whatever it begins with.
    let mut session = card(2, Dot::Done);
    session.replies = false;
    session.status = "1 order placed".into();
    session.fuller = true;
    assert_eq!(state_line(&session).as_ref(), "Claude · 1 order placed");
}

/// A card's mark, its rim and its button come from its task's state alone. Whatever the assistant
/// writes, even ilo's own words for another state, the card at work has the ring, the waiting one
/// the pause, its rim and Show Page, the done one the check, and the ended one today's glyph.
#[test]
fn a_cards_look_is_its_tasks_state_alone() {
    let posing = [
        "Needs you",
        "Done · 13:31",
        "Working",
        "Stopped",
        "maat.pt · Needs you",
    ];
    for status in posing {
        let with = |dot: Dot, pages: &[u64]| Card {
            status: status.into(),
            title: status.into(),
            finding: status.into(),
            ..paged(1, dot, pages)
        };
        assert_eq!(
            look(&with(Dot::Working, &[30]), Form::Working, false),
            Look {
                mark: Some(Mark::Ring { turns: 0. }),
                rim: false,
                button: false,
                dot: false,
            },
            "{status}"
        );
        assert_eq!(
            look(&with(Dot::Waiting, &[30]), Form::Waits, false),
            Look {
                mark: Some(Mark::Pause),
                rim: true,
                button: true,
                dot: false,
            },
            "{status}"
        );
        // Waiting with no page, it has nothing to show.
        assert!(!look(&with(Dot::Waiting, &[]), Form::Waits, false).button);
        let done = with(Dot::Done, &[30]);
        assert_eq!(
            look(&done, Form::DoneUnseen, true),
            Look {
                mark: Some(Mark::Check { unseen: true }),
                rim: false,
                button: false,
                dot: true,
            },
            "{status}"
        );
        assert_eq!(
            look(&done, Form::DoneLine, false).mark,
            Some(Mark::Check { unseen: false })
        );
        let ended = look(&with(Dot::Ended, &[30]), Form::Ended, true);
        assert_eq!((ended.mark, ended.rim, ended.button), (None, false, false));
    }
    // The form follows the state too: at work beside another at work, a line; done or ended and
    // seen, a line.
    let form = |dot, unseen, several| Form::of(&card(1, dot), unseen, several);
    assert_eq!(form(Dot::Working, false, false), Form::Working);
    assert_eq!(form(Dot::Working, false, true), Form::WorkingLine);
    assert_eq!(form(Dot::Waiting, false, true), Form::Waits);
    assert_eq!(form(Dot::Done, true, false), Form::DoneUnseen);
    assert_eq!(form(Dot::Done, false, false), Form::DoneLine);
    assert_eq!(form(Dot::Ended, true, false), Form::Ended);
    assert_eq!(form(Dot::Ended, false, false), Form::EndedLine);
}

/// A done task's finding is the first paragraph of its last answer as plain text: a link is its
/// words, formatting and line breaks go, a right-to-left override is written out as the title's
/// is (#3690), and it's kept to FINDING_KEPT characters.
#[test]
fn the_finding_is_plain_and_capped() {
    let said = |who: Who, text: &str| Message {
        who,
        text: text.to_string().into(),
        at: Instant::now(),
    };
    let answer = "## Booked\n\n**Taberna** for [two at 20:00](https://taberna.example/booking), \
                  by the `window`.\nSee <https://taberna.example/menu>.\u{202E}fdp.exe\n\n\
                  The second paragraph isn't shown.";
    let found = finding(&[said(Who::Agent, "Looking"), said(Who::Result, answer)]);
    assert_eq!(found.as_ref(), "Booked");
    let answer = "**Taberna** for [two at 20:00](https://taberna.example/booking), by the \
                  `window`.\n- See <https://taberna.example/menu>.\u{202E}fdp.exe\n\n\
                  The second paragraph isn't shown.";
    let found = finding(&[said(Who::Agent, "Looking"), said(Who::Result, answer)]);
    assert_eq!(
        found.as_ref(),
        "Taberna for two at 20:00, by the window. See https://taberna.example/menu.\\u{202E}fdp.exe"
    );
    assert!(!found.contains('\u{202E}') && !found.contains('\n'));
    // The last answer, not a question or the user's reply after it.
    let messages = [
        said(Who::Result, "The first."),
        said(Who::User, "And the second?"),
        said(Who::Agent, "The second."),
        said(Who::Asked, "Which one?"),
    ];
    assert_eq!(finding(&messages).as_ref(), "The second.");
    // None, with no answer.
    assert!(finding(&[said(Who::User, "Book it")]).is_empty());
    let long = "word ".repeat(100);
    let found = finding(&[said(Who::Result, &long)]);
    assert_eq!(found.chars().count(), FINDING_KEPT + 1);
    assert!(found.ends_with('…'));
    // Told, it's the same answer's first line as the conversation tells it: whole words up to
    // TOLD_KEPT, with no mark of the cut.
    let answer = [said(Who::Result, &long)];
    assert_eq!(told(&answer), told_line(&long).unwrap());
    assert!(told(&answer).chars().count() <= TOLD_KEPT && !told(&answer).ends_with('…'));
    assert!(long.starts_with(told(&answer).as_ref()));
    assert!(told(&[said(Who::User, "Book it")]).is_empty());
}

/// A done card not seen yet says its finding, its first sentence apart, wrapped to at most three
/// lines the list makes room for; with none, as after a quit, when it ended.
#[gpui::test]
fn a_done_card_says_its_finding(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with(cx, vec![card(1, Dot::Working), card(2, Dot::Working)]);
    let short = "Booked Taberna.";
    let long = "Booked Taberna for two at eight, by the window, and asked them for the quiet \
                corner, as you said the last time was loud; they will call if anything changes";
    let done = |n: u64, finding: &str| Card {
        finding: finding.into(),
        ended_time: "10:42".into(),
        fuller: false,
        ..card(n, Dot::Done)
    };
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![done(2, long), done(1, short)], cx)
    });
    let height = |n: u64, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let space = shell.spaces.iter().find(|space| space.id == 1).unwrap();
            let placed = space.placed.iter().find(|placed| placed.n == n).unwrap();
            (placed.form, placed.room)
        })
    };
    let (short_form, short_room) = height(1, &shell, cx);
    let (long_form, long_room) = height(2, &shell, cx);
    assert_eq!(
        (short_form, long_form),
        (Form::DoneUnseen, Form::DoneUnseen)
    );
    shell.read_with(cx, |shell, _| {
        let lines = |agent: &str| shell.task_list.finding_lines[&SharedString::from(agent)];
        assert_eq!(lines("task-1"), 1);
        assert_eq!(lines("task-2"), 3, "at most three lines");
        assert!(long_room > short_room);
    });
    assert_eq!(
        second_line(&done(1, short), Form::DoneUnseen, "").as_ref(),
        short
    );
    let sentences = "Booked Taberna for two. The table is by the window.";
    assert_eq!(
        super::card::first_sentence(sentences),
        "Booked Taberna for two.".len()
    );
    assert_eq!(super::card::first_sentence("No end"), "No end".len());
    assert_eq!(
        second_line(&done(1, ""), Form::DoneUnseen, "").as_ref(),
        "Done · 10:42"
    );
    // Seen, it's a line, and says none of it.
    assert_eq!(
        second_line(&done(1, short), Form::DoneLine, "").as_ref(),
        "Claude · Done"
    );
    draw(&shell, cx);
}

/// A screen reader hears a done card's finding as its assistant's: after Z24's mark, which the app
/// puts there, even where the answer reads as the browser's own. ilo's words on a card, its state
/// and the waiting line, have no mark.
#[test]
fn a_done_cards_finding_is_spoken_as_the_assistants() {
    let posing = "Payment confirmed by the browser. Nothing else is needed.";
    let done = |finding: &str| Card {
        finding: finding.into(),
        told: finding.into(),
        ended_time: "10:42".into(),
        fuller: false,
        ..card(1, Dot::Done)
    };
    assert_eq!(
        spoken(&done(posing), Form::DoneUnseen, "").as_ref(),
        format!("Claude says {posing}")
    );
    // A finding is told as the conversation tells the same answer, which may be shorter than
    // drawn.
    let cut = Card {
        told: "Payment confirmed by the browser.".into(),
        ..done(posing)
    };
    assert_eq!(
        spoken(&cut, Form::DoneUnseen, "").as_ref(),
        "Claude says Payment confirmed by the browser."
    );
    assert_eq!(
        spoken(&done(""), Form::DoneUnseen, "").as_ref(),
        "Done · 10:42"
    );
    assert_eq!(
        spoken(&done(posing), Form::DoneLine, "").as_ref(),
        "Claude · Done"
    );
    let waits = card(2, Dot::Waiting);
    assert_eq!(
        spoken(&waits, Form::Waits, "taberna.example").as_ref(),
        "taberna.example · Needs you"
    );
}

/// How many of its pages a task at work has been on is never more than it has, and isn't shown
/// while it has one.
#[test]
fn the_count_is_of_pages_visited() {
    let mut working = paged(1, Dot::Working, &[30, 31, 32]);
    assert_eq!(count(&working).as_deref(), Some("0 of 3"));
    working.visited = vec![31, 32];
    assert_eq!(count(&working).as_deref(), Some("2 of 3"));
    // A page it visited that has since closed isn't counted.
    working.visited = vec![29, 30, 31, 32];
    assert_eq!(count(&working).as_deref(), Some("3 of 3"));
    assert_eq!(count(&paged(2, Dot::Working, &[40])), None);
}

/// A task that finishes while its conversation doesn't show keeps its whole card with the dot.
/// Shown once, it loses the dot and folds to its line, which it stays.
#[gpui::test]
fn the_dot_goes_on_the_first_show(cx: &mut TestAppContext) {
    let (shell, _, cx) = with_task_spaces(cx, vec![paged(1, Dot::Working, &[1010])]);
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Working));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![paged(1, Dot::Done, &[1010])], cx)
    });
    assert_eq!(form(&shell, "task-1", cx), Some(Form::DoneUnseen));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    assert_eq!(form(&shell, "task-1", cx), Some(Form::DoneLine));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert_eq!(form(&shell, "task-1", cx), Some(Form::DoneLine));
}

/// A new card stands above the Today tab on top when it came, so the tabs opened after it go above
/// it; when the tab it stood above closes, it stands above the next one down.
#[gpui::test]
fn a_card_stands_where_it_came_among_the_today_tabs(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    assert_eq!(today(&shell, cx), ["task-1", "10", "11"]);
    let tabs = |today: Vec<u64>, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx)
        });
    };
    tabs(vec![12, 10, 11], &shell, cx);
    assert_eq!(today(&shell, cx), ["12", "task-1", "10", "11"]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Working), (2, Dot::Waiting)]), cx)
    });
    assert_eq!(today(&shell, cx), ["task-2", "12", "task-1", "10", "11"]);
    tabs(vec![12, 11], &shell, cx);
    assert_eq!(today(&shell, cx), ["task-2", "12", "task-1", "11"]);
    tabs(vec![12], &shell, cx);
    assert_eq!(today(&shell, cx), ["task-2", "12", "task-1"]);
    // Its place is the Space's, kept through a switch away and back.
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    assert_eq!(today(&shell, cx), ["task-2", "12", "task-1"]);
}

/// A drag moves the tab and leaves the card where it stands among the rows: dragging the tab it
/// stood above away, it stands above the next row down; dragging another tab, it stays put.
#[gpui::test]
fn a_dragged_tab_leaves_the_card_where_it_stands(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    let tabs = |today: Vec<u64>, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx)
        });
    };
    tabs(vec![10, 11, 12], &shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    assert_eq!(today(&shell, cx), ["task-1", "10", "11", "12"]);
    // The tab under it, dragged to the end.
    tabs(vec![11, 12, 10], &shell, cx);
    assert_eq!(today(&shell, cx), ["task-1", "11", "12", "10"]);
    // Another tab, dragged to the end.
    tabs(vec![11, 10, 12], &shell, cx);
    assert_eq!(today(&shell, cx), ["task-1", "11", "10", "12"]);
}

/// The walkers go over the tabs and pass the cards by: ⌘1–9, the next and previous tab, recent
/// tabs and Reveal Tab land on tabs alone.
#[gpui::test]
fn the_walkers_pass_the_cards_by(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with(cx, cards(&[(1, Dot::Waiting), (2, Dot::Done)]));
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
    let run = |command: Command, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run(command, "test", cx));
        cx.run_until_parked();
        let selected: Vec<String> = (host.take().into_iter())
            .filter(|line| line.starts_with("select "))
            .collect();
        if let Some(tab) = selected.last() {
            let tab: u64 = tab.trim_start_matches("select ").parse().unwrap();
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
        }
        selected
    };
    // Tab 10 shows, so ⌘1 has nothing to do, as on any tab shown.
    assert_eq!(run(Command::SelectTab(1), &shell, cx), ["select 11"]);
    assert_eq!(run(Command::SelectTab(0), &shell, cx), ["select 10"]);
    assert!(run(Command::SelectTab(2), &shell, cx).is_empty());
    assert_eq!(run(Command::SelectLastTab, &shell, cx), ["select 11"]);
    assert_eq!(run(Command::NextTab, &shell, cx), ["select 10"]);
    assert_eq!(run(Command::PreviousTab, &shell, cx), ["select 11"]);
    let recent = run(Command::RecentTab, &shell, cx);
    assert!(
        recent
            .iter()
            .all(|line| line == "select 10" || line == "select 11"),
        "{recent:?}"
    );
    // Reveal Tab goes to the tab shown's row, and asks nothing of a task.
    shell.update(cx, |shell, cx| shell.run(Command::RevealTab, "test", cx));
    cx.run_until_parked();
    assert!(
        !host.take().iter().any(|line| line.starts_with("task ")),
        "Reveal Tab touched a task"
    );
}

/// A card is never a tab: placing one leaves the Space's tabs as they were and tells the core
/// nothing, so the saved session, which is the core's tabs, holds no card.
#[gpui::test]
fn the_saved_session_holds_no_card(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Working), (2, Dot::Done)]), cx)
    });
    cx.run_until_parked();
    assert!(host.take().is_empty(), "placing cards asked the core");
    shell.read_with(cx, |shell, _| {
        let space = shell.shown_space().unwrap();
        assert_eq!(space.today, [10, 11]);
        assert!(space.pinned.is_empty());
        assert_eq!(space.placed.len(), 2);
    });
}

/// At work, done or ended, a card lists in its task's home Space alone. One that waits for the
/// user lists in every Space of the window too, first in Today, and its click there goes to its
/// home Space and shows its conversation.
#[gpui::test]
fn a_waiting_card_lists_in_every_space(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, cards(&[(1, Dot::Working), (2, Dot::Done)]));
    // A second Space of the user's, beside Space 1 and the tasks' own.
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str, agent: &str| {
            (
                id,
                name.to_string().into(),
                None,
                0,
                "".into(),
                agent.to_string().into(),
                "".into(),
            )
        };
        shell.set_spaces(
            vec![
                space(1, "Space 1", ""),
                space(2, "Space 2", ""),
                space(101, "Find a flight to Lisbon", "Claude"),
                space(102, "Find a flight to Lisbon", "Claude"),
            ],
            cx,
        );
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
    });
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert_eq!(today(&shell, cx), ["20"]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Waiting), (2, Dot::Done)]), cx)
    });
    assert_eq!(today(&shell, cx), ["task-1", "20"]);
    host.shown.borrow_mut().clear();
    shell.update(cx, |shell, cx| shell.show_group("task-1".into(), cx));
    assert_eq!(*host.shown.borrow(), [1]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.talk_agent()), None);
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.talk_agent()),
        Some("task-1".into())
    );
    // In its home Space it stands where it stood.
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
}

/// Spaces 1 and 2 of the user's and task 1's own, with Space 2 of `profile` showing its tab 20.
fn away_in_space_2(
    cx: &mut TestAppContext,
    first: Vec<Card>,
    profile: u64,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let (shell, host, cx) = with_task_spaces(cx, first);
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str, profile: u64, agent: &str| {
            (
                id,
                name.to_string().into(),
                None,
                profile,
                "".into(),
                agent.to_string().into(),
                "".into(),
            )
        };
        shell.set_spaces(
            vec![
                space(1, "Space 1", 0, ""),
                space(2, "Space 2", profile, ""),
                space(101, "Find a flight to Lisbon", 0, "Claude"),
            ],
            cx,
        );
        let tabs = vec![20, 21];
        shell.set_tabs(2, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        shell.set_space(2, cx);
        shell.set_selected(20, cx);
        // Space 2 in place, as its slide in from the side has ended.
        shell.settle = None;
        shell.slides.clear();
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

/// A waiting task's card lists at the top of the Space looked at, and goes from there the moment
/// the task stops waiting, answered, stopped or done; its card in its own Space stays. At work,
/// stopped or done, a card never lists outside its own Space.
#[gpui::test]
fn a_waiting_card_leaves_the_space_looked_at_when_it_stops_waiting(cx: &mut TestAppContext) {
    let (shell, _, cx) = away_in_space_2(cx, cards(&[(1, Dot::Working)]), 0);
    assert_eq!(today(&shell, cx), ["20", "21"]);
    for (stops, dot) in [
        ("answered", Dot::Working),
        ("stopped", Dot::Ended),
        ("done", Dot::Done),
    ] {
        shell.update(cx, |shell, cx| {
            shell.set_task_cards(cards(&[(1, Dot::Waiting)]), cx)
        });
        assert_eq!(today(&shell, cx), ["task-1", "20", "21"], "waiting");
        assert_eq!(form(&shell, "task-1", cx), Some(Form::Waits));
        shell.update(cx, |shell, cx| shell.set_task_cards(cards(&[(1, dot)]), cx));
        assert_eq!(today(&shell, cx), ["20", "21"], "{stops}");
        let home = shell.read_with(cx, |shell, _| {
            let space = shell.spaces.iter().find(|space| space.id == 1).unwrap();
            space
                .placed
                .iter()
                .map(|card| card.agent.clone())
                .collect::<Vec<_>>()
        });
        assert_eq!(home, ["task-1"], "{stops}: its own Space");
    }
}

/// The walkers pass a waiting card by in the Space looked at as in its own: ⌘1–9 and the next and
/// previous tab land on that Space's tabs alone.
#[gpui::test]
fn the_walkers_pass_a_waiting_card_by_away_from_its_space(cx: &mut TestAppContext) {
    let (shell, host, cx) = away_in_space_2(cx, cards(&[(1, Dot::Waiting)]), 0);
    assert_eq!(today(&shell, cx), ["task-1", "20", "21"]);
    let run = |command: Command, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run(command, "test", cx));
        cx.run_until_parked();
        let asked = host.take();
        let selected: Vec<String> = (asked.iter())
            .filter(|line| line.starts_with("select "))
            .cloned()
            .collect();
        assert!(
            !asked.iter().any(|line| line.starts_with("task ")),
            "{command:?} touched the task: {asked:?}"
        );
        if let Some(tab) = selected.last() {
            let tab: u64 = tab.trim_start_matches("select ").parse().unwrap();
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
        }
        selected
    };
    assert_eq!(run(Command::SelectTab(1), &shell, cx), ["select 21"]);
    assert_eq!(run(Command::SelectTab(0), &shell, cx), ["select 20"]);
    assert!(run(Command::SelectTab(2), &shell, cx).is_empty());
    assert_eq!(run(Command::SelectLastTab, &shell, cx), ["select 21"]);
    assert_eq!(run(Command::NextTab, &shell, cx), ["select 20"]);
    assert_eq!(run(Command::PreviousTab, &shell, cx), ["select 21"]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.space), 2);
}

/// A waiting card shows in a Space of another profile too, and its Show Page goes to the task's
/// own Space, as showing a page of another Space does, and stands on the page it waits on.
#[gpui::test]
fn show_page_away_goes_to_the_tasks_space_across_profiles(cx: &mut TestAppContext) {
    let first = vec![paged(1, Dot::Waiting, &[1010, 1011])];
    let (shell, host, cx) = away_in_space_2(cx, first, 7);
    assert_eq!(today(&shell, cx), ["task-1", "20", "21"]);
    still(cx);
    tick(5.);
    STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
    draw(&shell, cx);
    host.take();
    host.shown.borrow_mut().clear();
    let button = STOPS_DRAWN
        .with(|drawn| drawn.borrow().get(&StripStop::Chip(1)).copied())
        .expect("Show Page");
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(*host.shown.borrow(), [1]);
    // The core shows Space 1, as it does for any Space shown.
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page()),
        Some((101, 1010))
    );
    let asked = host.take();
    assert!(asked.contains(&"select 1010".to_string()), "{asked:?}");
    let told = |line: &&String| line.starts_with("task ") || line.starts_with("focus page");
    assert!(!asked.iter().any(|line| told(&line)), "{asked:?}");
}

/// A card whose home Space is gone lists in no Space, waiting or not, a session's as a task's: the
/// list knows a card by its home alone, and the Library still lists its task.
#[gpui::test]
fn a_card_whose_home_is_gone_lists_nowhere(cx: &mut TestAppContext) {
    let gone = |n: u64, dot: Dot| Card {
        home: 3,
        ..card(n, dot)
    };
    let (shell, _, cx) = shell_with(cx, vec![gone(2, Dot::Waiting), gone(1, Dot::Working)]);
    assert_eq!(today(&shell, cx), ["10", "11"]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert_eq!(today(&shell, cx), ["20"]);
}

/// A private window offers no tasks and lists none, while a regular window lists them.
#[gpui::test]
fn a_private_window_lists_no_card(cx: &mut TestAppContext) {
    let (regular, _, cx) = shell_with(cx, vec![card(2, Dot::Waiting)]);
    assert_eq!(today(&regular, cx), ["task-2", "10", "11"]);
    let host = Rc::new(Recorder::default());
    let private = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    private.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(2, Dot::Waiting)], cx);
    });
    cx.run_until_parked();
    private.read_with(cx, |shell, _| {
        assert!(shell.task_cards.is_empty());
        assert!(shell.spaces.iter().all(|space| space.placed.is_empty()));
    });
}

/// A done card the user has seen stays until the profile's ageing takes it, as it takes a Today
/// tab: that long after it was last seen it goes into the Archive.
#[gpui::test]
fn a_seen_card_ages_as_a_today_tab_does(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, vec![paged(1, Dot::Working, &[1010])]);
    host.archive_after.set(Some(3600.));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![paged(1, Dot::Done, &[1010])], cx)
    });
    shell.update(cx, |shell, cx| shell.close_task(cx));
    host.take();
    cx.executor().advance_clock(Duration::from_secs(3500));
    cx.run_until_parked();
    assert!(dismissed(&host).is_empty(), "it aged early");
    cx.executor().advance_clock(Duration::from_secs(200));
    cx.run_until_parked();
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
}

/// Ageing never takes a task at work, waiting or done and not yet seen, nor one whose
/// conversation shows, nor any with ageing off.
#[gpui::test]
fn ageing_takes_only_the_seen(cx: &mut TestAppContext) {
    let live = vec![
        paged(4, Dot::Working, &[1040]),
        paged(3, Dot::Waiting, &[1030]),
        paged(2, Dot::Working, &[1020]),
        paged(1, Dot::Working, &[1010]),
    ];
    let (shell, host, cx) = with_task_spaces(cx, live);
    host.archive_after.set(Some(60.));
    // Task 2 finishes out of sight, so it isn't seen; task 1 finishes with its conversation shown.
    shell.update(cx, |shell, cx| {
        shell.show_task("task-1".into(), cx);
        shell.set_task_cards(
            vec![
                paged(4, Dot::Working, &[1040]),
                paged(3, Dot::Waiting, &[1030]),
                paged(2, Dot::Done, &[1020]),
                paged(1, Dot::Done, &[1010]),
            ],
            cx,
        );
    });
    host.take();
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    assert!(dismissed(&host).is_empty());
    // Left, task 1 ages from then; task 2 still hasn't been seen.
    shell.update(cx, |shell, cx| shell.close_task(cx));
    cx.executor().advance_clock(Duration::from_secs(61));
    cx.run_until_parked();
    assert_eq!(dismissed(&host), ["task task-1 Dismiss"]);
}

/// With the profile's ageing set to Never, a seen card stays.
#[gpui::test]
fn ageing_off_leaves_every_card(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, vec![paged(5, Dot::Working, &[1050])]);
    // Never, as the core says it: below nothing.
    host.archive_after.set(Some(-1.));
    shell.update(cx, |shell, cx| shell.show_task("task-5".into(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![paged(5, Dot::Done, &[1050])], cx)
    });
    shell.update(cx, |shell, cx| shell.close_task(cx));
    host.take();
    cx.executor()
        .advance_clock(Duration::from_secs(365 * 24 * 3600));
    cx.run_until_parked();
    assert!(dismissed(&host).is_empty());
}

/// Clear clears the Space's Today tabs and leaves the cards, which aren't tabs.
#[gpui::test]
fn clear_leaves_the_cards(cx: &mut TestAppContext) {
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
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.shown_space().unwrap().placed.len(), 2)
    });
}

/// ⌘W in a task's conversation dismisses it, a finished one's included, as ⌘Z brings it back and
/// ⌘⇧Z dismisses it again, and ⇧⌘T brings back the task dismissed last.
#[gpui::test]
fn a_dismissed_task_comes_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, vec![paged(1, Dot::Done, &[1010])]);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    host.take();
    shell.update(cx, |shell, cx| {
        assert!(shell.dismiss_talk(cx));
        assert!(shell.undo(cx));
        assert!(shell.redo(cx));
    });
    let tasks: Vec<_> = (host.take().into_iter())
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
    // The Agents take the card away, and the list with it.
    shell.update(cx, |shell, cx| shell.set_task_cards(Vec::new(), cx));
    assert_eq!(today(&shell, cx), ["10", "11"]);

    shell.update(cx, |shell, cx| shell.reopen_closed(cx));
    assert!(host.take().contains(&"task task-1 Restore".to_string()));
    // It's back, so ⌘Z has no dismissal of it left to undo.
    shell.update(cx, |shell, cx| shell.undo(cx));
    assert!(!host.take().iter().any(|line| line.starts_with("task ")));
}

/// ⌘T's ask holds an empty slot at the top of Today, which the bar folds into and its task's card
/// takes, so nothing in the list moves; left without a task, it closes.
#[gpui::test]
fn the_card_takes_the_slot_the_bar_folds_into(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_ask_slot(true, cx));
    assert_eq!(today(&shell, cx), ["slot", "10", "11"]);
    let row = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.layout()[&Slot::Tab(10)])
    };
    let under_slot = row(&shell, cx);
    assert!(
        shell
            .read_with(cx, |shell, _| shell.fold_target())
            .is_some(),
        "no slot to fold into"
    );
    // An agent's task, not ⌘T's, stands under the slot.
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    assert_eq!(today(&shell, cx), ["slot", "task-1", "10", "11"]);

    shell.update(cx, |shell, cx| {
        shell.set_ask_slot(false, cx);
        shell.set_task_cards(vec![card(1, Dot::Working)], cx);
    });
    assert_eq!(today(&shell, cx), ["task-1", "10", "11"]);
    // The bar lands in the slot, and its task's card takes the slot's room: the rows stay put.
    shell.update(cx, |shell, cx| {
        shell.set_ask_slot(true, cx);
        shell.fold_started(true, cx);
    });
    let before = row(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Working), (2, Dot::Working)]), cx);
    });
    assert_eq!(
        today(&shell, cx),
        ["task-2", "task-1", "10", "11"],
        "the card didn't take the slot"
    );
    assert!(under_slot <= before);
    // The bar's going changes nothing now.
    shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);

    // Left without a task, the slot closes.
    shell.update(cx, |shell, cx| {
        shell.set_ask_slot(true, cx);
        shell.set_ask_slot(false, cx);
    });
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
    assert!(
        shell
            .read_with(cx, |shell, _| shell.fold_target())
            .is_none()
    );
}

/// An ended task's one button is the user's own act, which has no page: Limit's Retry, or its
/// assistant's fix, and its click does the act. A done task and one that asks have no chip.
#[gpui::test]
fn the_chip_acts(cx: &mut TestAppContext) {
    let mut limit = card(4, Dot::Ended);
    limit.retry = true;
    let mut done = card(5, Dot::Done);
    done.open = vec![(30, "maat.pt".into()), (31, "gulbenkian.pt".into())];
    let mut asks = card(6, Dot::Waiting);
    asks.asks = true;
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

    // It ends out of sight, so its card is whole until it's seen.
    STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
    let (shell, host, cx) = shell_with(cx, vec![asks.clone(), done.clone(), card(4, Dot::Working)]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![asks, done, limit], cx)
    });
    host.take();
    still(cx);
    tick(5.);
    draw(&shell, cx);
    let drawn = |stop: StripStop| STOPS_DRAWN.with(|drawn| drawn.borrow().get(&stop).copied());
    assert!(drawn(StripStop::Chip(5)).is_none() && drawn(StripStop::Chip(6)).is_none());
    let retry = drawn(StripStop::Chip(4)).expect("Retry");
    cx.simulate_click(retry.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["task task-4 Retry"]);
}

/// A list at rest asks for no frame: done and ended cards keep no clock going, and only a ring at
/// work draws, on the loading dots' clock, which holds still with motion reduced.
#[gpui::test]
fn an_idle_list_draws_nothing(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with(cx, cards(&[(1, Dot::Done), (2, Dot::Ended)]));
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        assert!(!shell.cards_moving(later()), "the list moves at rest");
        assert!(!shell.cards_live());
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
        assert!(shell.cards_live(), "the ring doesn't turn");
        assert!(shell.breath.pending());
        assert!(!shell.cards_moving(later()));
    });
    still(cx);
    draw(&shell, cx);
    assert!(
        !shell.read_with(cx, |shell, _| shell.cards_live()),
        "the ring turns with motion reduced"
    );
}

/// An outside agent's session is numbered with the tasks, so its card and its motion are its own,
/// and two sessions never share one.
#[test]
fn a_sessions_card_has_a_number_of_its_own() {
    use super::number;
    assert_eq!(
        [number("task-2"), number("session-3"), number("session-4")],
        [2, 3, 4]
    );
}

/// The sidebar shows one thing at a time: on the user's own tab its row; in a task's
/// conversation its card. A card's click shows its conversation, and on the card shown puts it
/// away.
#[gpui::test]
fn one_thing_in_the_sidebar_is_shown_at_a_time(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Working, &[1020, 1021]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two);
    still(cx);
    tick(5.);
    draw(&shell, cx);
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| (shell.shown_has_plate(), shell.shown_card()))
    };
    assert_eq!(shown(&shell, cx), (true, None));
    let plate = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        (shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-2"))).expect("its card")
    };
    let at = plate(&shell, cx).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shown(&shell, cx), (false, Some(StripStop::Header(2))));
    draw(&shell, cx);
    let at = plate(&shell, cx).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shown(&shell, cx), (true, None));
}

/// Show Page stands on the page the task waits on, as its picture's click in the task's view
/// does, and does nothing else: the task is told nothing and an agent's page isn't given the
/// keyboard.
#[gpui::test]
fn show_page_watches_its_page_and_nothing_else(cx: &mut TestAppContext) {
    let (shell, host, cx) = with_task_spaces(cx, vec![paged(1, Dot::Waiting, &[1010, 1011])]);
    still(cx);
    tick(5.);
    STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
    draw(&shell, cx);
    host.take();
    let button = STOPS_DRAWN
        .with(|drawn| drawn.borrow().get(&StripStop::Chip(1)).copied())
        .expect("Show Page");
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page()),
        Some((101, 1010))
    );
    let asked = host.take();
    assert!(asked.contains(&"select 1010".to_string()), "{asked:?}");
    let told = |line: &&String| line.starts_with("task ") || line.starts_with("focus page");
    assert!(!asked.iter().any(|line| told(&line)), "{asked:?}");
}

/// Every card is reached by the keys, among Today's rows where it stands: the arrows go over the
/// cards and their buttons with the rows, Return on a card shows its conversation, and Focus
/// Sidebar then rings that card.
#[gpui::test]
fn the_keys_reach_every_card(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Waiting, &[1020]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two);
    still(cx);
    tick(5.);
    cx.update(|window, cx| {
        crate::shell::commands::bind_keys(cx);
        window.focus(&shell.read(cx).focus_handle.clone(), cx);
    });
    draw(&shell, cx);
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
    let ring = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| (shell.keyed, shell.keyed_strip))
    };
    let press = |key: &str, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        ring(shell, cx)
    };
    shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
    draw(&shell, cx);
    assert_eq!(ring(&shell, cx), (Some(10), None));
    let header = |n| (None, Some(StripStop::Header(n)));
    let new_tab = shell.read_with(cx, |shell, _| shell.key_rows()[0]);
    let up: Vec<_> = (0..4).map(|_| press("up", &shell, cx)).collect();
    assert_eq!(
        up,
        [
            header(1),
            (None, Some(StripStop::Chip(2))),
            header(2),
            (Some(new_tab), None),
        ]
    );
    let down: Vec<_> = (0..4).map(|_| press("down", &shell, cx)).collect();
    assert_eq!(
        down,
        [
            header(2),
            (None, Some(StripStop::Chip(2))),
            header(1),
            (Some(10), None),
        ]
    );
    press("up", &shell, cx);
    press("enter", &shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.talk_agent()),
        Some("task-1".into())
    );
    draw(&shell, cx);
    shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
    draw(&shell, cx);
    assert_eq!(ring(&shell, cx), header(1));
}

/// A close a page refused has the list back as it stood at once: the conversation's card where it
/// was, in the form it had (#4202).
#[gpui::test]
fn a_refused_close_has_the_list_back_at_once(cx: &mut TestAppContext) {
    use crate::shell::{DialogKind, PageDialog, PageReply};
    let (shell, _, cx) = with_task_spaces(cx, vec![paged(1, Dot::Working, &[1010, 1011])]);
    draw(&shell, cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    tick(5.);
    draw(&shell, cx);
    let placed = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.shown_space().unwrap().placed.clone())
    };
    let stood = (placed(&shell, cx), today(&shell, cx));
    shell.update(cx, |shell, cx| shell.run(Command::CloseWindow, "ui", cx));
    let question = PageDialog {
        id: 7,
        kind: DialogKind::Leave,
        title: "".into(),
        message: "".into(),
        text: "".into(),
        accept: "Leave".into(),
        cancel: "Cancel".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    };
    shell.update(cx, |shell, cx| {
        shell.set_page_dialog(10, Some(question), cx)
    });
    let stay = PageReply {
        accept: false,
        text: String::new(),
        password: String::new(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 7, stay, cx));
    assert_eq!((placed(&shell, cx), today(&shell, cx)), stood);
}

/// A task asked from ⌘T over a page leaves the page: its card comes in at the top of Today, and no
/// conversation takes the page's place.
#[gpui::test]
fn a_task_asked_over_a_page_leaves_the_page(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with(cx, Vec::new());
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(cards(&[(1, Dot::Working)]), cx)
    });
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent(), None, "a view took the page's place");
        assert_eq!(shell.selected, 10, "the page went");
        assert_eq!(shell.task_cards.len(), 1, "no card came in");
    });
    assert_eq!(today(&shell, cx), ["task-1", "10", "11"]);
}

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

/// A page's word and its outline are one answer (`page_mark`), which the rail's tile and the card
/// read. While its task works every page says Working and the outline alone marks the page it's
/// in; Done only once the task is done; Paused where it waits or broke off; Stopped on every page
/// of a task the user stopped; at a handoff the outline is on the page that needs the user; a page
/// they took over says they drive it; a stopped or done task has no outline.
#[gpui::test]
fn a_pages_word_and_outline_are_one_answer(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell(cx);
    let mark = |word, outline| PageMark { word, outline };
    let (working_in, working, paused, done) = (
        mark(PageWord::Working, true),
        mark(PageWord::Working, false),
        mark(PageWord::Paused, false),
        mark(PageWord::Done, false),
    );
    shell.read_with(cx, |shell, _| {
        let marks = |card: &Card| {
            card.pages
                .iter()
                .map(|tab| shell.page_mark(card, *tab))
                .collect::<Vec<_>>()
        };
        // Helpers on two of three pages: each has the outline, and all say Working.
        let mut helpers = card(1, Dot::Working);
        helpers.pages = vec![30, 31, 32];
        helpers.page_steps = vec![(31, "Opening maat.pt".into()), (32, "Reading".into())];
        assert_eq!(marks(&helpers), [working, working_in, working_in]);
        // With one page, that is the one it is in.
        let mut opened = card(1, Dot::Working);
        opened.pages = vec![30];
        assert_eq!(marks(&opened), [working_in]);
        let mut waits = card(2, Dot::Waiting);
        waits.pages = vec![40, 41, 42];
        assert_eq!(marks(&waits), [paused; 3]);
        let mut finished = card(3, Dot::Done);
        finished.pages = vec![50, 51];
        assert_eq!(marks(&finished), [done; 2]);
        let mut stopped = card(4, Dot::Ended);
        stopped.pages = vec![60];
        assert_eq!(marks(&stopped), [mark(PageWord::Stopped, false)]);
    });
    // The page the task handed the user needs them there and has the outline; one they took over
    // says they drive it; its others are Paused. Once it works again, the page it handed them is
    // Paused, and the one they took is still theirs.
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
        let marks = |card: &Card| [40, 41, 42].map(|tab| shell.page_mark(card, tab));
        let driving = mark(PageWord::Driving, false);
        let mut waits = card(2, Dot::Waiting);
        waits.pages = vec![40, 41, 42];
        assert_eq!(
            marks(&waits),
            [mark(PageWord::NeedsYou, true), driving, paused]
        );
        let mut working_again = card(2, Dot::Working);
        working_again.pages = vec![40, 41, 42];
        working_again.page_steps = vec![(42, "Opening maat.pt".into())];
        assert_eq!(marks(&working_again), [paused, driving, working_in]);
        for dot in [Dot::Done, Dot::Ended] {
            let mut over = card(2, dot);
            over.pages = vec![40, 41, 42];
            assert_eq!(shell.page_mark(&over, 41), driving);
            assert!(
                !marks(&over).iter().any(|mark| mark.outline),
                "{dot:?} has an outline"
            );
        }
    });
    assert_eq!(
        [
            PageWord::Working,
            PageWord::Done,
            PageWord::Paused,
            PageWord::Stopped,
            PageWord::Driving
        ]
        .map(PageWord::label),
        ["Working", "Done", "Paused", "Stopped", "You're driving"]
    );
    assert_eq!(PageWord::NeedsYou.label(), Word::NeedsYou.after_name());
}
