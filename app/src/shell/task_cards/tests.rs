//! The tasks' cards in Today's list, on a host that records what the shell asks of the core.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, Entity, Modifiers, SharedString, TestAppContext, VisualTestContext};

use super::card::{Look, look, second_line, spoken};
use super::glyph::Mark;
use super::{Card, Chip, Cli, Dot, Form, Offer, PageMark, PageWord, chip};
use crate::agents::Step;
use crate::agents::task::Wait;
use crate::agents::task::{FINDING_KEPT, Message, Who, Word, finding};
use crate::agents::tier::{Tier, Why};
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
        wait: Default::default(),
        word: Word::for_dot(dot),
        pages: Vec::new(),
        tier: Default::default(),
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

/// The line under a waiting thread's title is the product's: Needs you and the site, never the
/// assistant's question; a thread in any other state has none. The assistant's own words, its
/// question or its result, are never on a line but as its title, nor said for it (#3690).
#[gpui::test]
fn a_card_says_the_agents_words_only_as_its_title(cx: &mut TestAppContext) {
    let said = "I found three flights and booked the cheapest";
    let mut done = card(1, Dot::Done);
    done.status = said.into();
    let mut asks = card(3, Dot::Waiting);
    asks.asks = true;
    asks.wait = Wait::Asked;
    asks.status = "Which would you rather visit?".into();
    assert_eq!(
        second_line(&asks, Form::Waits, "maat.pt").as_ref(),
        "Asked a question"
    );
    // A wait of no kind the browser knows says N1's fallback.
    let waits = card(5, Dot::Waiting);
    assert_eq!(
        second_line(&waits, Form::Waits, "maat.pt").as_ref(),
        "Needs you · maat.pt"
    );
    assert_eq!(second_line(&waits, Form::Waits, "").as_ref(), "Needs you");
    let mut ended = card(4, Dot::Ended);
    ended.status = "Stopped after 6 steps".into();
    for card in [&done, &asks, &ended] {
        for form in [Form::Working, Form::Waits, Form::Unseen, Form::Rest] {
            let (line, told) = (second_line(card, form, ""), spoken(card, form, ""));
            for words in [said, "Which would", "after 6 steps"] {
                assert!(!line.contains(words) && !told.contains(words), "{form:?}");
            }
        }
    }
    assert!(second_line(&done, Form::Unseen, "maat.pt").is_empty());
    let (shell, _, cx) = shell_with(cx, vec![done]);
    assert_eq!(today(&shell, cx), ["task-1", "10", "11"]);
}

/// A waiting line's second line is picked by its kind of wait, in the product's words, its reason
/// first so a long host cuts and never the reason, in at most 30 characters before the site. The
/// page's button and field words, which the card's status and question carry, never reach it, nor
/// do the agent's, whatever the wait (words table, N1, N1a–N1k).
#[test]
fn a_waiting_lines_words_are_its_kinds_never_the_pages() {
    let (order, field, reason) = ("Place order", "Résumé", "Sign in, please");
    let words = |wait: Wait, site: &str| {
        let card = Card {
            status: format!("Stopped at {order}").into(),
            question: Some(crate::agents::Question {
                why: format!("Choose a file for “{field}”. {reason}").into(),
                ..Default::default()
            }),
            wait,
            ..card(1, Dot::Waiting)
        };
        let line = second_line(&card, Form::Waits, site);
        assert_eq!(spoken(&card, Form::Waits, site), line, "{wait:?}");
        for page in [order, field, reason] {
            assert!(!line.contains(page), "{wait:?}: {line}");
        }
        line.to_string()
    };
    let refused = |why: Why, level: Option<Tier>| Wait::Refused(why, level);
    let cases = [
        (Wait::Asked, "Asked a question"),
        (Wait::Step(Step::SignIn), "Sign in · maat.pt"),
        (
            Wait::Step(Step::Files { many: false }),
            "Choose a file · maat.pt",
        ),
        (
            Wait::Step(Step::Files { many: true }),
            "Choose files · maat.pt",
        ),
        (Wait::Step(Step::HandOff), "Your step · maat.pt"),
        (Wait::Step(Step::Back), "Needs you · maat.pt"),
        (
            refused(Why::Order, Some(Tier::Full)),
            "Paused before an order",
        ),
        (
            refused(Why::Message, Some(Tier::Full)),
            "Paused before sending",
        ),
        (
            refused(Why::Download, Some(Tier::Work)),
            "Paused before downloading",
        ),
        (
            refused(Why::Discard, Some(Tier::Full)),
            "Paused before discarding text",
        ),
        (refused(Why::SignIn, Some(Tier::Work)), "Sign in · maat.pt"),
        (
            refused(Why::SignInBank, Some(Tier::Full)),
            "Sensitive site · maat.pt",
        ),
        (refused(Why::Act, Some(Tier::Work)), "Needs Work on pages"),
        (refused(Why::Bank, Some(Tier::Full)), "Needs Full access"),
        (refused(Why::Request, Some(Tier::Full)), "Needs Full access"),
        (refused(Why::Fenced, None), "Needs you · maat.pt"),
        (refused(Why::Never, None), "Needs you · maat.pt"),
        (Wait::Other, "Needs you · maat.pt"),
    ];
    for (wait, said) in cases {
        assert_eq!(words(wait, "maat.pt"), said);
        let reason = words(wait, "");
        assert!(said.starts_with(&reason), "{wait:?}: the reason leads");
        assert!(reason.chars().count() <= 30, "{reason}");
    }
    // A long host comes after the reason, which the line's end cuts first.
    let long = "tickets.a-very-long-museum-name.example";
    assert!(words(Wait::Step(Step::SignIn), long).starts_with("Sign in · "));
}

/// An outside app's card has its line from the kind of its connection alone, a session with no
/// task's token, and a task's never does, whatever either is named. The name, however long or made
/// of whatever state words, moves nothing and changes no mark or second line.
#[test]
fn an_outside_apps_line_comes_from_its_connection_alone() {
    let long = "Flights ".repeat(25);
    let names = [
        "Flights",
        "Needs you",
        "Done · 13:31",
        "Working",
        "Claude",
        "",
        long.as_str(),
    ];
    let forms = [Form::Working, Form::Waits, Form::Unseen, Form::Rest];
    for name in names {
        for dot in [Dot::Working, Dot::Waiting, Dot::Done, Dot::Ended] {
            let task = Card {
                name: name.into(),
                ..paged(1, dot, &[30])
            };
            let session = Card {
                replies: false,
                ..task.clone()
            };
            let short = Card {
                name: "Flights".into(),
                ..session.clone()
            };
            for form in forms {
                let (theirs, ours) = (look(&task, form, true), look(&session, form, true));
                assert!(!theirs.from && ours.from, "{name:?} {form:?}");
                assert_eq!(
                    Look {
                        from: false,
                        ..ours
                    },
                    theirs,
                    "{name:?} {form:?}"
                );
                let line = |card: &Card| second_line(card, form, "maat.pt");
                assert_eq!(line(&session), line(&short), "{name:?} {form:?}");
                assert_eq!(line(&session), line(&task), "{name:?} {form:?}");
            }
        }
    }
}

/// A card's mark and its second line come from its task's state alone. Whatever the assistant
/// writes, even ilo's own words for another state, the line at work has the ring, the waiting one
/// the pause and the line that says it needs you, page or none, the done one the check, and the
/// ended one today's glyph.
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
            line: Some(status.into()),
            ..paged(1, dot, pages)
        };
        assert_eq!(
            look(&with(Dot::Working, &[30]), Form::Working, false),
            Look {
                mark: Some(Mark::Ring { turns: 0. }),
                needs: false,
                dot: false,
                from: false,
            },
            "{status}"
        );
        assert_eq!(
            look(&with(Dot::Waiting, &[30]), Form::Waits, false),
            Look {
                mark: Some(Mark::Pause),
                needs: true,
                dot: false,
                from: false,
            },
            "{status}"
        );
        assert!(look(&with(Dot::Waiting, &[]), Form::Waits, false).needs);
        let done = with(Dot::Done, &[30]);
        assert_eq!(
            look(&done, Form::Unseen, true),
            Look {
                mark: Some(Mark::Check { unseen: true }),
                needs: false,
                dot: true,
                from: false,
            },
            "{status}"
        );
        assert_eq!(
            look(&done, Form::Rest, false).mark,
            Some(Mark::Check { unseen: false })
        );
        let ended = look(&with(Dot::Ended, &[30]), Form::Unseen, true);
        assert_eq!((ended.mark, ended.needs, ended.dot), (None, false, true));
    }
    // The form follows the state too: one line, a second under it while it waits, the dot while
    // finished and not seen, a chip or none.
    let form = |dot, unseen| Form::of(&card(1, dot), unseen);
    assert_eq!(form(Dot::Working, false), Form::Working);
    assert_eq!(form(Dot::Waiting, false), Form::Waits);
    assert_eq!(form(Dot::Done, true), Form::Unseen);
    assert_eq!(form(Dot::Done, false), Form::Rest);
    assert_eq!(form(Dot::Ended, true), Form::Unseen);
    assert_eq!(form(Dot::Ended, false), Form::Rest);
    let retry = Card {
        retry: true,
        ..card(1, Dot::Ended)
    };
    assert_eq!(Form::of(&retry, true), Form::Unseen);
    assert_eq!(Form::of(&retry, false), Form::Rest);
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
}

/// A thread that finished out of sight is one line with the dot, as one at work is, and a screen
/// reader hears its title and the dot's word, a thread's row, never its answer or when it ended,
/// which the line doesn't draw. Shown, it rests, and says its title alone. An ended one with its
/// chip is a line too: the chip is in its conversation.
#[gpui::test]
fn a_finished_thread_not_seen_is_its_line_with_the_dot(cx: &mut TestAppContext) {
    use crate::tokens::shell::task_card::LINE;
    let answer = "Payment confirmed by the browser. Nothing else is needed.";
    let done = Card {
        status: answer.into(),
        line: Some(answer.into()),
        ended_at: Some("10:42".into()),
        ..paged(1, Dot::Done, &[1010])
    };
    let stopped = Card {
        title: "Book a table".into(),
        word: Word::Stopped,
        ended_at: Some("10:43".into()),
        ..paged(2, Dot::Ended, &[1011])
    };
    let working = |card: &Card| Card {
        dot: Dot::Working,
        word: Word::Working,
        ..card.clone()
    };
    let (shell, _, cx) = with_task_spaces(cx, vec![working(&stopped), working(&done)]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![stopped.clone(), done.clone()], cx)
    });
    cx.simulate_a11y_activation();
    still(cx);
    tick(5.);
    draw(&shell, cx);
    for agent in ["task-1", "task-2"] {
        assert_eq!(form(&shell, agent, cx), Some(Form::Unseen), "{agent}");
        let plate = shell
            .read_with(cx, |shell, _| shell.task_plate_drawn(agent))
            .expect("its line drawn");
        assert_eq!(f32::from(plate.size.height), LINE, "{agent}: a row high");
    }
    let names = |cx: &mut VisualTestContext| -> Vec<String> {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        (tree.nodes.iter())
            .filter_map(|(_, node)| node.label().map(str::to_owned))
            .collect()
    };
    let heard = names(cx);
    for said in ["Find a flight to Lisbon, Unread", "Book a table, Unread"] {
        assert!(heard.iter().any(|name| name == said), "{said}: {heard:?}");
    }
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let row = (tree.nodes.iter())
        .map(|(_, node)| node)
        .find(|node| node.label() == Some("Book a table, Unread"))
        .expect("its row");
    assert_eq!(row.role_description(), Some("Thread"));
    for name in &heard {
        assert!(
            !name.contains("Payment") && !name.contains("10:4"),
            "{name}"
        );
    }
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    draw(&shell, cx);
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Rest));
    assert_eq!(form(&shell, "task-2", cx), Some(Form::Unseen));
    let heard = names(cx);
    let rests = "Find a flight to Lisbon";
    assert!(heard.iter().any(|name| name == rests), "{heard:?}");
    // With its chip, the ended one is a line too, the chip in its conversation.
    let retry = Card {
        retry: true,
        ..stopped.clone()
    };
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![working(&retry), done.clone()], cx);
        shell.set_task_cards(vec![retry.clone(), done.clone()], cx);
    });
    assert_eq!(form(&shell, "task-2", cx), Some(Form::Unseen));
    // At rest a line says nothing after its title, at work its state's word; a waiting card its
    // second line.
    assert_eq!(spoken(&done, Form::Rest, "").as_ref(), "");
    assert_eq!(
        spoken(&card(4, Dot::Working), Form::Working, "").as_ref(),
        "Working"
    );
    let waits = card(3, Dot::Waiting);
    assert_eq!(
        spoken(&waits, Form::Waits, "taberna.example").as_ref(),
        "Needs you · taberna.example"
    );
}

/// Every thread at work is one line, a tab's, however many work: nothing folds, each line is a
/// row high, among the tabs with nothing between, and no fill or outline of its own. A screen
/// reader hears its title whole, then its mark's state, and nothing it doesn't draw: not its step,
/// its pages or a count.
#[gpui::test]
fn every_thread_at_work_is_one_line_however_many_work(cx: &mut TestAppContext) {
    use crate::shell::PLATE_INSET;
    use crate::tokens::shell::task_card::LINE;
    let long = "Find a flight to Lisbon that leaves after nine and lands before noon, window seat";
    let working = |n: u64, title: &str| Card {
        title: title.into(),
        status: "Step 3 · Opening tap.pt".into(),
        ..paged(n, Dot::Working, &[1010, 1011])
    };
    let (shell, _, cx) = shell_with(cx, vec![working(1, long)]);
    let alone = shell.read_with(cx, |shell, _| shell.layout());
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![working(2, "Book a table"), working(1, long)], cx)
    });
    cx.simulate_a11y_activation();
    still(cx);
    tick(5.);
    draw(&shell, cx);
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
    for agent in ["task-1", "task-2"] {
        assert_eq!(form(&shell, agent, cx), Some(Form::Working), "{agent}");
        let plate = shell
            .read_with(cx, |shell, _| shell.task_plate_drawn(agent))
            .expect("its line drawn");
        assert_eq!(f32::from(plate.size.height), LINE, "{agent}: a row high");
        assert_eq!(f32::from(plate.left()), PLATE_INSET, "{agent}");
    }
    // The second line pushes the tabs down by its own height alone.
    let both = shell.read_with(cx, |shell, _| shell.layout());
    let tab = |layout: &std::collections::HashMap<Slot, f32>| layout[&Slot::Tab(10)];
    assert_eq!(tab(&both) - tab(&alone), LINE);
    shell.read_with(cx, |shell, _| {
        for card in &shell.task_cards {
            let framed = shell.card_framed(card);
            assert_eq!(framed, super::card::Framed::Plain, "{}", card.agent);
        }
    });
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let names: Vec<String> = (tree.nodes.iter())
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    let said = format!("{long}, Working");
    assert!(names.iter().any(|name| *name == said), "{names:?}");
    assert!(
        names.iter().any(|name| name == "Book a table, Working"),
        "{names:?}"
    );
    for name in &names {
        assert!(
            !name.contains("Step 3") && !name.contains(" of 2"),
            "{name}"
        );
    }
}

/// A task that finishes while its conversation doesn't show keeps the dot on its line. Shown once,
/// it loses the dot and rests, which it stays.
#[gpui::test]
fn the_dot_goes_on_the_first_show(cx: &mut TestAppContext) {
    let (shell, _, cx) = with_task_spaces(cx, vec![paged(1, Dot::Working, &[1010])]);
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Working));
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![paged(1, Dot::Done, &[1010])], cx)
    });
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Unseen));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Rest));
    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Rest));
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

/// A waiting line shows in a Space of another profile too, and its click goes to the task's own
/// Space, as showing a page of another Space does, and stands on the page it waits on.
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
    let line = shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-1"));
    cx.simulate_click(line.expect("its line").center(), Modifiers::none());
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

/// An ended thread's one chip is the user's own act, which has no page: Limit's Retry, or its
/// assistant's fix. A done thread and one that asks have none. It's in the conversation, never on
/// the sidebar's line, whose one ring stop is the row, as every line's.
#[gpui::test]
fn the_chip_is_the_conversations_never_the_lines(cx: &mut TestAppContext) {
    use crate::tokens::shell::task_card::LINE;
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

    // It ends out of sight: a line with the dot, a row high, and one stop.
    let (shell, _, cx) = shell_with(cx, vec![asks.clone(), done.clone(), card(4, Dot::Working)]);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![asks, done, limit], cx)
    });
    still(cx);
    tick(5.);
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        let space = shell.shown_space().expect("a Space shown");
        let placed = (space.placed.iter())
            .find(|placed| placed.agent.as_ref() == "task-4")
            .expect("its place");
        assert_eq!((placed.form, placed.room), (Form::Unseen, LINE));
        let stops: Vec<_> = (shell.card_stops().into_iter())
            .filter(|(stop, _)| *stop == StripStop::Header(4))
            .collect();
        assert_eq!(stops.len(), 1);
        assert_eq!(shell.ended_chip("task-4"), Some(Chip::Retry));
        assert_eq!(shell.ended_chip("task-5"), None);
        assert_eq!(shell.ended_chip("task-6"), None);
    });
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

/// A waiting line grows a second line that says so in the product's words for its kind of wait;
/// never the page's own button words where the browser paused, and nothing on it to press. A screen reader hears it as drawn, a thread's row. Its
/// click stands on the page it waits on and does nothing else: the task is told nothing and an
/// agent's page isn't given the keyboard.
#[gpui::test]
fn a_waiting_lines_click_shows_its_page_and_nothing_else(cx: &mut TestAppContext) {
    use crate::tokens::shell::task_card::{LINE, NEEDS_LINE};
    let paused = Card {
        status: "Stopped at Place order".into(),
        wait: Wait::Refused(Why::Order, Some(Tier::Full)),
        ..paged(1, Dot::Waiting, &[1010, 1011])
    };
    let (shell, host, cx) = with_task_spaces(cx, vec![paused]);
    cx.simulate_a11y_activation();
    still(cx);
    tick(5.);
    draw(&shell, cx);
    host.take();
    let line = shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-1"));
    let line = line.expect("its line");
    assert_eq!(f32::from(line.size.height), LINE + NEEDS_LINE);
    let said = "Find a flight to Lisbon, Paused before an order".to_string();
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let names: Vec<(String, Option<String>)> = (tree.nodes.iter())
        .filter_map(|(_, node)| {
            let label = node.label()?.to_owned();
            Some((label, node.role_description().map(str::to_owned)))
        })
        .collect();
    assert!(
        names.contains(&(said.clone(), Some("Thread".into()))),
        "{said}: {names:?}"
    );
    for (name, _) in &names {
        assert!(!name.contains("Place order"), "{name}");
        assert!(!name.contains("Show Page"), "{name}");
    }
    shell.read_with(cx, |shell, _| {
        let stops = shell.card_stops();
        assert_eq!(stops.len(), 1, "the line alone: {stops:?}");
    });
    cx.simulate_click(line.center(), Modifiers::none());
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

/// A waiting line whose page is the one the page area shows has no page left to show, so its click
/// shows its conversation, as any line's does; the line keeps its form and its height. Once the
/// user stands elsewhere, its click shows the page again.
#[gpui::test]
fn a_waiting_line_whose_page_shows_shows_its_conversation(cx: &mut TestAppContext) {
    let (shell, _, cx) = with_task_spaces(cx, vec![paged(1, Dot::Waiting, &[1010, 1011])]);
    still(cx);
    tick(5.);
    draw(&shell, cx);
    let click = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let line = shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-1"));
        let line = line.expect("its line");
        cx.simulate_click(line.center(), Modifiers::none());
        cx.run_until_parked();
        draw(shell, cx);
        line.size.height
    };
    let watched = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.watched_task_page())
    };
    let height = click(&shell, cx);
    assert_eq!(watched(&shell, cx), Some((101, 1010)));
    assert_eq!(click(&shell, cx), height);
    assert_eq!(watched(&shell, cx), None);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.talk_agent()),
        Some("task-1".into())
    );
    assert_eq!(form(&shell, "task-1", cx), Some(Form::Waits));
    // Another tab, the user's own: the page isn't the one shown, so the click shows it again.
    shell.update(cx, |shell, cx| shell.run(Command::SelectTab(1), "test", cx));
    cx.run_until_parked();
    assert_eq!(watched(&shell, cx), None);
    click(&shell, cx);
    assert_eq!(watched(&shell, cx), Some((101, 1010)));
}

/// A line that grows or shrinks moves the rows under it, and a press under a pointer that hasn't
/// moved since was meant for what was there: for PROTECT it takes none, read at the press with no
/// timer. A pointer that moved aims afresh, and the press counts at once.
#[gpui::test]
fn a_row_a_line_moved_under_a_resting_pointer_takes_no_press_at_once(cx: &mut TestAppContext) {
    let two = vec![
        paged(2, Dot::Working, &[1020]),
        paged(1, Dot::Done, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two.clone());
    still(cx);
    tick(5.);
    draw(&shell, cx);
    assert_eq!(today(&shell, cx), ["task-2", "task-1", "10", "11"]);
    let talk = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.talk_agent())
    };
    let line = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let line = shell.read_with(cx, |shell, _| shell.task_plate_drawn("task-1"));
        line.expect("its line").center()
    };
    let before = line(&shell, cx);
    // The line above grows as its thread comes to need the user, and task-1 moves down.
    let waits = vec![paged(2, Dot::Waiting, &[1020]), two[1].clone()];
    shell.update(cx, |shell, cx| shell.set_task_cards(waits, cx));
    draw(&shell, cx);
    let at = line(&shell, cx);
    assert!(at.y > before.y, "the row under it moved");
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(talk(&shell, cx), None, "a press meant for what was there");
    tick(0.5);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(talk(&shell, cx), Some("task-1".into()), "PROTECT on");
    shell.update(cx, |shell, cx| shell.close_task(cx));
    // It shrinks back; the pointer that moves to the row counts at once.
    shell.update(cx, |shell, cx| shell.set_task_cards(two.clone(), cx));
    draw(&shell, cx);
    let at = line(&shell, cx);
    assert_eq!(at, before);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        talk(&shell, cx),
        Some("task-1".into()),
        "a pointer that moved"
    );
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
    // A waiting line has no stop of its own but itself: nothing on it answers the wait.
    let up: Vec<_> = (0..3).map(|_| press("up", &shell, cx)).collect();
    assert_eq!(up, [header(1), header(2), (Some(new_tab), None)]);
    let down: Vec<_> = (0..3).map(|_| press("down", &shell, cx)).collect();
    assert_eq!(down, [header(2), header(1), (Some(10), None)]);
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

/// The shown thread's card joins its stage, on the stage's ground rather than the sidebar's: its
/// title still reads there at 4.5:1, Dark and Light, on every theme.
#[test]
fn the_joined_cards_title_reads_on_its_stage() {
    use crate::palette::{Palette, Tone, contrast, over};
    use crate::theme::{Appearance, Theme};
    let mut palettes = vec![Palette::default()];
    for appearance in Appearance::ALL {
        for step in 0..24 {
            let angle = step as f32 * std::f32::consts::TAU / 24.;
            for distance in [0., 0.35, 0.7, 1.] {
                let theme = Theme {
                    appearance,
                    ..Theme::at(distance * angle.cos(), distance * angle.sin())
                };
                palettes
                    .extend([Tone::Dark, Tone::Light].map(|tone| Palette::of(Some(&theme), tone)));
            }
        }
    }
    for palette in &palettes {
        let stage = super::super::conversation::stage_fill(palette);
        let ground = gpui::Rgba { a: 1., ..stage };
        let ink = gpui::Rgba::from(super::super::title_ink(palette, true));
        let ratio = contrast(over(ground, gpui::Rgba { a: 1., ..ink }, ink.a), ground);
        assert!(
            ratio >= 4.5,
            "the joined card's title is {ratio:.2}:1 on {stage:?}"
        );
    }
}

/// The shown thread's card is a shape of its own in its stage's colour (spec-sidebar, "Thread,
/// shown"): it takes the stage's fill, has no outline of its own, is inset at both sides and has
/// four round corners, with a hairline of light round it on a dark stage and none on a light one.
/// A line not shown, a tab's, has no frame, and goes back to none once another thread is shown.
#[gpui::test]
fn the_shown_threads_card_is_a_shape_of_its_own_in_its_stages_colour(cx: &mut TestAppContext) {
    use super::card::Framed;
    use crate::palette::Tone;
    use crate::shell::own_shape::OWN_SHAPE;
    use crate::theme::Appearance;
    use crate::tokens::shell::task_card::RADIUS;
    use crate::tokens::shell::{DIVIDER, PLATE_INSET};
    let two = vec![
        paged(2, Dot::Done, &[1020]),
        paged(1, Dot::Working, &[1010]),
    ];
    let (shell, _, cx) = with_task_spaces(cx, two);
    still(cx);
    tick(5.);
    draw(&shell, cx);
    let framed = |shell: &Entity<Shell>, agent: &str, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let card = (shell.task_cards.iter())
                .find(|card| card.agent.as_ref() == agent)
                .expect("its card");
            shell.card_framed(card)
        })
    };
    assert_eq!(framed(&shell, "task-2", cx), Framed::Plain);
    assert_eq!(framed(&shell, "task-1", cx), Framed::Plain);
    let show = |shell: &Entity<Shell>, agent: &str, cx: &mut VisualTestContext| {
        let at = (shell.read_with(cx, |shell, _| shell.task_plate_drawn(agent)))
            .expect("its card")
            .origin
            + gpui::point(gpui::px(24.), gpui::px(8.));
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        draw(shell, cx);
    };
    show(&shell, "task-2", cx);
    assert_eq!(
        shell
            .read_with(cx, |shell, _| shell.talk_agent())
            .as_deref(),
        Some("task-2")
    );
    assert_eq!(framed(&shell, "task-2", cx), Framed::Own);
    assert_eq!(framed(&shell, "task-1", cx), Framed::Plain);
    for appearance in [Appearance::Dark, Appearance::Light] {
        shell.update(cx, |shell, cx| {
            shell.run(Command::Appearance(appearance), "ui", cx)
        });
        cx.run_until_parked();
        OWN_SHAPE.set(None);
        draw(&shell, cx);
        let own = OWN_SHAPE.take().expect("the card's own shape");
        let (edge, plate, palette) = shell.read_with(cx, |shell, _| {
            (
                shell.width(),
                shell.task_plate_drawn("task-2").expect("its card"),
                shell.palette,
            )
        });
        assert_eq!(f32::from(plate.left()), PLATE_INSET, "{appearance:?}");
        assert_eq!(
            f32::from(plate.right()),
            edge - PLATE_INSET,
            "{appearance:?}: the shown card's right end"
        );
        let stage = crate::shell::conversation::stage_fill(&palette);
        assert_eq!(own.fill, gpui::Hsla::from(stage), "{appearance:?}");
        assert_eq!(own.shape, plate, "{appearance:?}");
        assert_eq!(
            own.corners,
            gpui::Corners::all(gpui::px(RADIUS)),
            "{appearance:?}"
        );
        let on = palette.on_fill(stage);
        let line = (on.tone == Tone::Dark).then(|| on.faint(DIVIDER));
        assert_eq!(own.outline, line, "{appearance:?}");
        assert_eq!(
            own.outline.is_some(),
            appearance == Appearance::Dark,
            "{appearance:?}"
        );
    }
    show(&shell, "task-1", cx);
    assert_eq!(framed(&shell, "task-1", cx), Framed::Own);
    assert_eq!(framed(&shell, "task-2", cx), Framed::Plain);
}
