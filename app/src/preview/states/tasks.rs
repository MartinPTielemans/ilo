//! Made-up tasks: their Spaces, cards and conversations.

use crate::*;
use browser::agents::runner::Cli;

/// The first task's Space, apart from the ids the rest of the preview takes.
pub(crate) const FIRST_TASK: u64 = 9001;

/// The tasks PREVIEW_TASKS shows: each one's title, status and dot.
pub(crate) const TASKS: [(&str, &str, Dot); 3] = [
    (
        "Find two tickets for Saturday",
        "Step 3 · Clicking “Tickets”",
        Dot::Working,
    ),
    (
        "Compare the museum passes",
        "Stopped after 6 steps",
        Dot::Ended,
    ),
    (
        "Book a table for four",
        "A table for four at 20:00 is held",
        Dot::Done,
    ),
];

/// What each task was asked, as its view says it.
const ASKED: [&str; 3] = [
    "Find two tickets for Saturday's concert, in the stalls",
    "Compare the museum passes for a weekend in the city",
    "Book a table for four on Friday at 20:00",
];

/// The working task's last three steps, as its trace lines say them.
const WORKING_STEPS: [&str; 3] = [
    "Opened encore.example",
    "Read encore.example",
    "Clicking “Tickets”",
];

/// The working task's four pages, as its conversation lists them: their titles and addresses.
const WORKING_PAGES: [(&str, &str); 4] = [
    ("Seats · Saturday", "https://tickets.example/seats"),
    ("Stalls · Seat map", "https://seatmap.example/stalls"),
    ("Basket · Tickets", "https://tickets.example/basket"),
    ("Directions · Concert hall", "https://maps.example/hall"),
];

/// The pages the stopped task had open.
const STOPPED_PAGES: [(&str, &str); 2] = [
    ("Passes · City Museums", "https://museums.example/passes"),
    ("Weekend pass · Gallery", "https://gallery.example/weekend"),
];

/// The pages the done task left the user, each with its site.
pub(crate) const TASK_PAGES: [(&str, &str); 3] = [
    ("Booking · Taberna", "https://taberna.example/booking"),
    ("Menu · Taberna", "https://taberna.example/menu"),
    ("Directions", "https://maps.example/taberna"),
];

/// A task's pages by its place in `TASKS`, the tabs of its Space, whose icons its card shows:
/// each one's tab, title and address.
pub(crate) fn task_pages(at: usize) -> Vec<(u64, &'static str, &'static str)> {
    let (first, pages): (u64, &[(&str, &str)]) = match at {
        // The bench's task stands on its one page, or has one for each helper (#4102).
        0 if states::var_os("PREVIEW_TASK").is_some() => {
            (200, &HELPER_PAGES[..bench_helpers().unwrap_or(1)])
        }
        0 => (200, &WORKING_PAGES),
        1 => (150, &STOPPED_PAGES),
        _ => (100, &TASK_PAGES),
    };
    let tabs = FIRST_TASK + first..;
    (tabs
        .zip(pages)
        .map(|(tab, &(title, url))| (tab, title, url)))
    .collect()
}

/// The state PREVIEW_TASKS=<state> shows one task's group in, its view over the page area: done,
/// with the pages it left open; needs-you; stopped; interrupted, with its assistant's fix under
/// its header; or two, the done task's group open over the working one's, closed to its header.
/// =open is the working task's, with four pages, the other two closed.
pub(crate) fn group_state() -> Option<String> {
    let state = states::var("PREVIEW_TASKS").ok()?;
    let one = ["done", "needs-you", "stopped", "interrupted", "two"];
    one.contains(&state.as_str()).then_some(state)
}

/// The task whose view PREVIEW_TASKS=<state> opens, by its place in `TASKS`.
pub(crate) fn shown_task(state: &str) -> Option<usize> {
    match state {
        "open" | "needs-you" | "interrupted" => Some(0),
        "stopped" => Some(1),
        "done" | "two" => Some(2),
        _ => None,
    }
}

/// The pages of the helpers PREVIEW_TASK's helper states show (#4102), one each: their titles and
/// addresses. With four, the third is the one its helper handed the user, for the sign-in.
pub(crate) const HELPER_PAGES: [(&str, &str); 4] = [
    ("Seats · Saturday", "https://tickets.example/seats"),
    ("Stalls · Saturday", "https://stalls.example/saturday"),
    ("Sign in · Box Office", "https://boxoffice.example/sign-in"),
    ("Saturday · Encore", "https://encore.example/saturday"),
];

/// What the bench's task says before it starts its four helpers.
const FOUR_SHOPS: &str = "Four shops sell Saturday's stalls. I'll look at all four at once.";

/// What the bench's task named each of its helpers, as an assistant does: its own words.
const HELPER_NAMES: [&str; 4] = [
    "Stalls at Tickets",
    "Stalls at Stalls",
    "Stalls at Box Office",
    "Stalls at Encore",
];

/// Which of `HELPER_PAGES` waits on the user's sign-in.
pub(crate) const HELPER_WAITING: usize = 2;

/// How many helpers the bench's task has, if it shows one with helpers: four, one of them
/// waiting on the user (helpers), stopped at the limit (helpers-limit) or after its turn ended
/// (helpers-ended), or one (helper).
pub(crate) fn bench_helpers() -> Option<usize> {
    match states::var("PREVIEW_TASK").ok()?.as_str() {
        "helpers" | "helpers-limit" | "helpers-ended" => Some(HELPER_PAGES.len()),
        "helper" => Some(1),
        _ => None,
    }
}

/// Whether one of the bench's helpers waits on the user, which its task's header and card say.
pub(crate) fn bench_helper_waits() -> bool {
    states::var("PREVIEW_TASK").as_deref() == Ok("helpers")
}

/// Whether the bench's task handed its one page to the user (PREVIEW_TASK=yours): the page is
/// theirs, so nothing works on it, and the task needs them.
pub(crate) fn bench_yours() -> bool {
    states::var("PREVIEW_TASK").as_deref() == Ok("yours")
}

/// The helpers' lines of the bench's task: each on its page, reading it, and with `waits`, the
/// one that handed its page to the user. None is `running` once the turn is over.
fn helper_lines(count: usize, waits: bool, running: bool) -> Vec<TraceItem> {
    let sites = helper_tabs();
    let helper = |at: usize, tool: &str, did: &str, running| TraceItem::Helper {
        step: TraceStep {
            key: 10 + at,
            glyph: browser::agents::tools::step_glyph(tool),
            title: format!("{did} {}", sites[at].1).into(),
            description: "".into(),
            running,
            failed: false,
            opens: Opens::Nothing,
            under: "".into(),
            page: None,
            says: "".into(),
        },
        name: HELPER_NAMES[at].into(),
        tab: Some(sites[at].0),
    };
    (0..count)
        .map(|at| match (at == HELPER_WAITING && waits, running) {
            (true, _) => helper(at, "hand_off", "Handed you", false),
            (false, true) => helper(at, "read_page", "Reading", true),
            (false, false) => helper(at, "read_page", "Read", false),
        })
        .collect()
}

/// The stretch under way of the bench's task with helpers, whose line its card says too.
fn helper_trace(count: usize, waits: bool) -> Trace {
    Trace {
        key: 10,
        running: true,
        took: None,
        started: None,
        items: helper_lines(count, waits, true),
    }
}

/// The helpers' pages as the task's conversation lists them, each tab with its site: the tiles
/// of Pages, and the icons on the task's card.
pub(crate) fn helper_tabs() -> Vec<(u64, SharedString)> {
    let site = |url: &str| url.split('/').nth(2).unwrap_or("").to_string().into();
    let pages = (FIRST_TASK + 200..).zip(HELPER_PAGES);
    pages.map(|(tab, (_, url))| (tab, site(url))).collect()
}

/// A task's Space, as the core lists one: its agent's name in the agent column, its agent's id
/// a task's (`Fake::space_agent`).
pub(crate) fn task_space(id: u64) -> SpaceEntry {
    (
        id,
        "Task".into(),
        None,
        0,
        "".into(),
        "Claude".into(),
        "".into(),
    )
}

/// The tasks' cards, each asked in `home`.
pub(crate) fn task_cards(home: u64) -> Vec<TaskCard> {
    let card = |(at, (title, status, dot)): (u64, (&str, &str, Dot))| TaskCard {
        agent: format!("task-{at}").into(),
        home,
        space: at,
        name: "Claude".into(),
        replies: true,
        title: title.into(),
        status: status.into(),
        // The working task's last steps. The preview has no pictures, so no card has a plate.
        steps: match dot {
            Dot::Working => WORKING_STEPS.map(SharedString::from).to_vec(),
            _ => Vec::new(),
        },
        last_tab: None,
        // A done task's status is its answer's first line, Latest tasks' one line.
        line: (dot == Dot::Done).then(|| status.into()),
        ended_at: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        pages: (task_pages((at - FIRST_TASK) as usize).iter())
            .map(|&(tab, ..)| tab)
            .collect(),
        kept_picture: None,
        tier: Default::default(),
        question: None,
        open: match dot {
            Dot::Done => {
                let site = |url: &str| url.split('/').nth(2).unwrap_or("").into();
                let pages = task_pages((at - FIRST_TASK) as usize).into_iter();
                pages.map(|(tab, _, url)| (tab, site(url))).collect()
            }
            _ => Vec::new(),
        },
        page_steps: Vec::new(),
        retry: false,
        fix: None,
        visited: Vec::new(),
        ended_time: Default::default(),
        // Its ended task is the stopped one, whose status is how far it got.
        fuller: dot == Dot::Ended,
        // Its working task's status leads with its step's count.
        lead: match dot {
            Dot::Working => status.find(" · ").unwrap_or(0),
            _ => 0,
        },
        finding: Default::default(),
        told: Default::default(),
    };
    let mut cards: Vec<TaskCard> = (FIRST_TASK..).zip(TASKS).map(card).collect();
    // PREVIEW_SESSION=working, done or open shows an outside agent's card alone: at work after it
    // closed its last page, done with nothing open, and done with its page to open (#3652).
    if let Ok(which) = states::var("PREVIEW_SESSION") {
        let (status, dot, open) = match which.as_str() {
            "working" => ("Step 4 · Closed example.com", Dot::Working, Vec::new()),
            "open" => {
                let page = (FIRST_TASK + 100, "example.com".into());
                ("Done · 10:42", Dot::Done, vec![page])
            }
            _ => ("Done · 10:42", Dot::Done, Vec::new()),
        };
        return vec![TaskCard {
            agent: "session-1".into(),
            name: "Claude Code".into(),
            replies: false,
            title: "Claude Code".into(),
            status: status.into(),
            // Done, its status is its receipt.
            fuller: dot == Dot::Done,
            lead: 0,
            dot,
            word: Word::for_dot(dot),
            pages: Vec::new(),
            tier: Default::default(),
            open,
            ..card((FIRST_TASK, TASKS[0]))
        }];
    }
    // PREVIEW_TASK_STEP=long gives the working task a step line too long for a narrow window's
    // strip.
    if states::var("PREVIEW_TASK_STEP").as_deref() == Ok("long") {
        cards[0].status =
            "Step 3 · Comparing the seats in rows 12 to 18 for two on the aisle".into();
    }
    // The bench's task with helpers (#4102) says on its card what its conversation does: that it
    // needs the user while one of them waits, else the live line's own words.
    match (bench_helpers(), bench_helper_waits()) {
        (Some(_), true) => {
            let (_, site) = helper_tabs().swap_remove(HELPER_WAITING);
            cards[0].status =
                format!("{} · Sign in to {site}", browser::agents::task::NEEDS_YOU).into();
            cards[0].lead = browser::agents::task::NEEDS_YOU.len();
            cards[0].dot = Dot::Waiting;
            cards[0].word = Word::NeedsYou;
        }
        (Some(count), false) => {
            cards[0].status = helper_trace(count, false).line();
            cards[0].fuller = true;
            cards[0].lead = 0;
        }
        (None, _) => {}
    }
    if bench_yours() {
        cards[0].status = format!(
            "{} · Sign in to tickets.example",
            browser::agents::task::NEEDS_YOU
        )
        .into();
        cards[0].fuller = false;
        cards[0].lead = browser::agents::task::NEEDS_YOU.len();
        cards[0].dot = Dot::Waiting;
    }
    // Each helper's page has its own step, which the strip under it and its tile say.
    if let Some(count) = bench_helpers().filter(|_| bench_ended().is_none()) {
        let steps = helper_lines(count, bench_helper_waits(), true);
        cards[0].page_steps = (helper_tabs().into_iter().zip(steps))
            .filter_map(|((tab, _), step)| match step {
                TraceItem::Helper { step, .. } => Some((tab, step.title)),
                _ => None,
            })
            .collect();
    }
    // The bench's task that has ended (#4105) is done on its card too, which the strip under its
    // page follows, with the page it left the user.
    if let Some(name) = bench_ended() {
        cards[0].name = name.into();
        cards[0].status = "Two seats in row 12 are in your basket".into();
        cards[0].fuller = false;
        cards[0].lead = 0;
        cards[0].dot = Dot::Done;
        cards[0].open = vec![(FIRST_TASK + 200, "tickets.example".into())];
    }
    // One group's state (`group_state`): the task it's of, alone, or with the one beside it.
    match group_state().as_deref() {
        Some("needs-you") => {
            cards[0].status = format!(
                "{} · Sign in to tickets.example",
                browser::agents::task::NEEDS_YOU
            )
            .into();
            cards[0].fuller = false;
            cards[0].lead = browser::agents::task::NEEDS_YOU.len();
            cards[0].dot = Dot::Waiting;
            cards[0].word = Word::NeedsYou;
            cards.truncate(1);
        }
        Some("interrupted") => {
            cards[0].status = "Claude Code isn't signed in.".into();
            cards[0].fuller = false;
            cards[0].lead = 0;
            cards[0].dot = Dot::Ended;
            cards[0].word = Word::Interrupted;
            cards[0].fix = Some((
                browser::agents::runner::Cli::Claude,
                browser::agents::sign_in::Offer::SignIn,
            ));
            cards.truncate(1);
        }
        Some("stopped") => cards = vec![cards.swap_remove(1)],
        Some("done") => cards = vec![cards.swap_remove(2)],
        Some("two") => drop(cards.remove(1)),
        _ => {}
    }
    // Newest first, as the Agents send them.
    cards.reverse();
    cards
}

/// The assistant of the task PREVIEW_TASK's bench shows as ended (#4105), if it's one of those
/// states: all but picked-up, which a reply set to work again.
pub(crate) fn bench_ended() -> Option<&'static str> {
    match states::var("PREVIEW_TASK").ok()?.as_str() {
        "ended" | "gone" | "not-kept" | "helpers-ended" => Some("Claude"),
        "past" | "past-no-profile" | "past-no-space" => Some("Claude"),
        "codex-ended" => Some("Codex"),
        _ => None,
    }
}

/// The working task's conversation: a step done, one that failed, what the agent said of it, and
/// the step under way, on four pages, so it opens on Pages.
pub(crate) fn task_conversation(agent: SharedString) -> Conversation {
    let (title, _, _) = TASKS[0];
    // Every step is on the tickets page: the first went to it, so it heads the page's line.
    let step = |key, glyph, title: &str, description: &str, running, failed| TraceStep {
        key,
        glyph,
        title: title.into(),
        description: description.into(),
        running,
        failed,
        opens: match failed {
            true => Opens::Text("Another element would get the click: div.cookie-banner".into()),
            false => Opens::Nothing,
        },
        under: "".into(),
        page: Some(OnPage {
            tab: (key != 0).then_some(FIRST_TASK + 200),
            site: "tickets.example".into(),
            went: key == 0,
        }),
        says: "".into(),
    };
    let (click, read) = ("icons/mouse-pointer-2.svg", "icons/text-align-start.svg");
    let items = vec![
        TraceItem::Step(step(
            0,
            "icons/globe.svg",
            "Opened tickets.example",
            &browser::agents::task::duration(2),
            false,
            false,
        )),
        TraceItem::Step(step(1, read, "Read tickets.example", "", false, false)),
        TraceItem::Step(step(
            2,
            click,
            "Couldn't click “Tickets”",
            "A banner covers it",
            false,
            true,
        )),
        TraceItem::Said("The banner is in the way, so I'll close it first.".into()),
        TraceItem::Step(step(3, click, "Clicked “Accept”", "", false, false)),
        TraceItem::Step(step(4, click, "Clicking “Tickets”", "", true, false)),
    ];
    let mut talk = Conversation {
        agent,
        name: "Claude".into(),
        title: title.into(),
        about: "Claude · Working".into(),
        asked: "Find two tickets for Saturday's concert, in the stalls".into(),
        page: None,
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            started: None,
            items: items.clone(),
        })],
        tabs: vec![
            (FIRST_TASK + 200, "tickets.example".into()),
            (FIRST_TASK + 201, "seatmap.example".into()),
            (FIRST_TASK + 202, "tickets.example".into()),
            (FIRST_TASK + 203, "maps.example".into()),
        ],
        on: Some(FIRST_TASK + 200),
        held: Vec::new(),
        stoppable: true,
        live: Default::default(),
        open: None,
        answers: Vec::new(),
        writing: false,
        tier: Default::default(),
        finished: Vec::new(),
        folder: None,
        model: Some(model(Cli::Claude, Some("Opus"))),
    };
    group_talk(&mut talk, &items);
    bench_task(&mut talk, items);
    talk
}

/// A thread's model as its composer's row says it, the app's way: its CLI's mark and name with
/// the model's, or its default's row, opening the model's menu where the CLI takes another.
fn model(cli: Cli, name: Option<&str>) -> (&'static str, SharedString, SharedString, bool) {
    let (said, word) = match name {
        Some(name) => (format!("{} \u{b7} {name}", cli.name()), name.to_string()),
        None => {
            let default = browser::agents::task::model_default(cli);
            (default, browser::command_bar::DEFAULT_ROW.to_string())
        }
    };
    (cli.glyph(), said.into(), word.into(), cli.switches_model())
}

/// The view of a task that isn't at work says what its group's header does: its title, where it
/// is and its pages, after a stretch of work that's over.
fn group_talk(talk: &mut Conversation, items: &[TraceItem]) {
    let cards = task_cards(0);
    let Some(card) = cards.iter().find(|card| card.agent == talk.agent) else {
        return;
    };
    if card.word == Word::Working {
        return;
    }
    let at = (card.space - FIRST_TASK) as usize;
    let site =
        |url: &str| -> SharedString { url.split('/').nth(2).unwrap_or("").to_string().into() };
    let pages = task_pages(at);
    talk.title = card.title.clone();
    talk.asked = ASKED[at].into();
    talk.about = match card.word {
        Word::Done => format!("{} · Done at 14:02", card.name).into(),
        word => format!("{} · {}", card.name, word.after_name()).into(),
    };
    talk.tabs = (pages.iter().map(|&(tab, _, url)| (tab, site(url)))).collect();
    talk.on = None;
    talk.stoppable = card.word == Word::NeedsYou;
    // The first task's steps are the working conversation's; the others read their first page.
    let read = |key, glyph, did: &str| {
        TraceItem::Step(TraceStep {
            key,
            glyph,
            title: format!("{did} {}", site(pages[0].2)).into(),
            description: "".into(),
            running: false,
            failed: false,
            opens: Opens::Nothing,
            under: "".into(),
            page: Some(OnPage {
                tab: (key != 0).then_some(pages[0].0),
                site: site(pages[0].2),
                went: key == 0,
            }),
            says: "".into(),
        })
    };
    let steps = match at {
        0 => items[..items.len() - 1].to_vec(),
        _ => vec![
            read(0, "icons/globe.svg", "Opened"),
            read(1, "icons/text-align-start.svg", "Read"),
        ],
    };
    let worked = Line::Trace(Trace {
        key: 0,
        running: false,
        took: Some(40),
        started: None,
        items: steps,
    });
    talk.lines = match card.word {
        Word::Done => {
            talk.finished = vec![(1, "14:02".into())];
            vec![worked, Line::Said(card.status.clone())]
        }
        Word::NeedsYou => vec![
            worked,
            Line::Ilo(IloLine {
                key: 100,
                kind: IloKind::Yours {
                    tab: pages[0].0,
                    site: None,
                },
                settled: false,
            }),
        ],
        _ => vec![worked],
    };
}

/// PREVIEW_TASK=<state> (#4095's bench) shows the working task's conversation beside its page in
/// one of the states ilo's own lines are made for, from made-up data: working, refused,
/// off-limits, yours, tab-back, signed-in, raised or notes; or of a task that has ended (#4105):
/// ended, picked-up, gone, not-kept or codex-ended; or of one a run of the browser before left,
/// brought back from the Archive: past, past-no-profile or past-no-space; or of one with helpers
/// (#4102): helpers, helper, helpers-limit or helpers-ended; or of one with files whose assistant
/// ilo couldn't limit to them (#4101): files-wall.
pub(crate) fn bench_task(talk: &mut Conversation, items: Vec<TraceItem>) {
    let Ok(state) = states::var("PREVIEW_TASK") else {
        return;
    };
    let tab = FIRST_TASK + 200;
    let done = |key, items: &[TraceItem]| {
        Line::Trace(Trace {
            key,
            running: false,
            took: Some(40),
            started: None,
            items: items.to_vec(),
        })
    };
    let working = |key: usize, title: &str| {
        Line::Trace(Trace {
            key,
            running: true,
            took: None,
            started: None,
            items: vec![TraceItem::Step(TraceStep {
                key,
                glyph: "icons/mouse-pointer-2.svg",
                title: title.to_string().into(),
                description: "".into(),
                running: true,
                failed: false,
                opens: Opens::Nothing,
                under: "".into(),
                page: None,
                says: "".into(),
            })],
        })
    };
    let ilo = |key, kind, settled| Line::Ilo(IloLine { key, kind, settled });
    let line = |key, kind| ilo(key, kind, false);
    let before = &items[..items.len() - 1];
    let stopped = |why, at: &str, count| IloKind::Stopped {
        tab,
        at: at.to_string().into(),
        doing: format!("clicking \u{201c}{at}\u{201d}").into(),
        why,
        tier: Tier::Work,
        count,
    };
    // It stands on its one page, beside it.
    talk.tabs.truncate(1);
    talk.lines = match state.as_str() {
        "refused" => vec![
            done(0, before),
            Line::Said(
                "Two seats in row 12 are in the basket. Placing the order is the last step.".into(),
            ),
            line(100, stopped(Why::Order, "Place order", 2)),
        ],
        "off-limits" => vec![
            done(0, before),
            line(100, stopped(Why::Fenced, "seatmap.example", 1)),
        ],
        "yours" => {
            talk.about = format!("{} · {}", talk.name, browser::agents::task::NEEDS_YOU).into();
            talk.on = None;
            vec![
            done(0, before),
            Line::Said("tickets.example wants you to sign in before it shows the seats. I'll carry on once you hand the tab back.".into()),
            line(100, IloKind::Yours { tab, site: None }),
        ]
        }
        "tab-back" => vec![
            done(0, before),
            Line::Said("I need the seat map back to pick two seats together.".into()),
            line(
                100,
                IloKind::TabBack {
                    tab: FIRST_TASK + 201,
                },
            ),
        ],
        "signed-in" => vec![
            done(0, before),
            line(
                100,
                IloKind::SignedIn {
                    tab,
                    site: "tickets.example".into(),
                    with: SignedInWith::Saved("ada@example.com".into()),
                },
            ),
            working(10, "Reading this page"),
        ],
        "raised" => {
            talk.tier = Tier::Full;
            vec![
                done(0, before),
                ilo(100, stopped(Why::Order, "Place order", 1), true),
                line(
                    101,
                    IloKind::TierChanged {
                        from: Tier::Work,
                        to: Tier::Full,
                    },
                ),
                working(10, "Clicking “Place order”"),
            ]
        }
        // A task with files whose assistant ilo couldn't limit to them (#4101), each line where
        // it comes: started without the folder and the file, set back under the level the user
        // set, and stopped once the folder was taken back, with what its run left running.
        "files-wall" => {
            use browser::agents::task::ilo::{LeftOut, Reach};
            let folder = || gpui::SharedString::from("Lisbon trip");
            let said = |key, kind| ilo(key, kind, true);
            talk.about = format!("{} · Stopped", talk.name).into();
            talk.stoppable = false;
            talk.on = None;
            vec![
                said(
                    100,
                    IloKind::StartedWithout {
                        left: LeftOut::Folder(folder()),
                    },
                ),
                said(
                    101,
                    IloKind::StartedWithout {
                        left: LeftOut::File("boarding passes.pdf".into()),
                    },
                ),
                done(0, before),
                Line::Said("Two seats in row 12 are in the basket.".into()),
                Line::You("Put the tickets in the folder too.".into()),
                said(
                    102,
                    IloKind::TierChanged {
                        from: Tier::Work,
                        to: Tier::Full,
                    },
                ),
                said(
                    103,
                    IloKind::SetBack {
                        at: Tier::Work,
                        refused: Tier::Full,
                        folder: folder(),
                    },
                ),
                said(
                    104,
                    IloKind::NotRestarted {
                        left: LeftOut::Folder(folder()),
                    },
                ),
                said(
                    105,
                    IloKind::LeftRunning {
                        reach: Reach::Folder(folder()),
                    },
                ),
            ]
        }
        // A task that has ended (#4105): with its reply field, the first one's line saying where
        // the conversation lives, a reply that took it up, and the three lines that stand in the
        // field's place.
        "ended" | "picked-up" | "gone" | "not-kept" | "codex-ended" => {
            let ended = state != "picked-up";
            if state == "codex-ended" {
                talk.name = "Codex".into();
                talk.model = Some(model(Cli::Codex, None));
            }
            talk.about = match ended {
                true => format!("{} · Done at 14:02", talk.name).into(),
                false => talk.about.clone(),
            };
            talk.stoppable = !ended;
            if ended {
                talk.on = None;
            }
            // Its answer, the line after the trace, finished when the task did.
            talk.finished = vec![(1, "14:02".into())];
            let mut lines = vec![
                done(0, &items),
                Line::Said(
                    "Two seats in row 12 of the stalls are in your basket on tickets.example."
                        .into(),
                ),
            ];
            match state.as_str() {
                "picked-up" => lines.extend([
                    line(100, IloKind::Resumed { tier: Tier::Work }),
                    Line::You("Are there two together for Sunday too?".into()),
                    working(10, "Reading this page"),
                ]),
                "gone" => lines.push(line(100, IloKind::Gone)),
                "not-kept" => lines.push(line(100, IloKind::NotKept)),
                "codex-ended" => lines.push(line(100, IloKind::KeepsNothing)),
                _ => {}
            }
            lines
        }
        // A task a run of the browser before left (#4105), brought back from the Archive: all
        // there is of it is its title and how it ended, so where its messages would begin it says
        // where they are, over its reply field; or, with its profile deleted or left with no
        // Space, that, in the field's place. It ended on an earlier day, so it says the day.
        "past" | "past-no-profile" | "past-no-space" => {
            talk.title = "Concert tickets".into();
            talk.about = format!("{} · Done · 21 Nov", talk.name).into();
            talk.asked = "".into();
            talk.tabs.clear();
            talk.on = None;
            talk.stoppable = false;
            vec![line(
                100,
                match state.as_str() {
                    "past" => IloKind::KeptHere,
                    "past-no-profile" => IloKind::NoProfile,
                    _ => IloKind::NoSpace,
                },
            )]
        }
        // A task at work with four helpers (#4102), each on a page of its own: the stretch counts
        // them and opens to a line for each, a step's line in ilo's words for the helper's page.
        // The third handed its page to the user for a sign-in, which ilo's line under them says,
        // while the other three go on, so the task needs the user. The conversation opens with
        // the task's first page alone, as the task did; `helper_tabs` are the rest, which the
        // bench gives it once it shows.
        "helpers" => {
            talk.about = format!("{} · {}", talk.name, browser::agents::task::NEEDS_YOU).into();
            // One of several pages: its line names the page's site (O1).
            let (tab, site) = helper_tabs().remove(HELPER_WAITING);
            let yours = IloKind::Yours {
                tab,
                site: Some(site),
            };
            vec![
                done(0, before),
                Line::Said(FOUR_SHOPS.into()),
                Line::Trace(helper_trace(HELPER_PAGES.len(), true)),
                line(100, yours),
            ]
        }
        // One helper, at work: the live line says "1 helper".
        "helper" => vec![
            done(0, before),
            Line::Said("The stalls are on another page. A helper reads them while I go on.".into()),
            Line::Trace(helper_trace(1, false)),
        ],
        // ilo stopped the task after it started a fifth: its line, with Take Over alone.
        "helpers-limit" => vec![
            done(0, before),
            Line::Said(FOUR_SHOPS.into()),
            Line::Trace(helper_trace(HELPER_PAGES.len(), false)),
            line(
                100,
                IloKind::Helpers {
                    tab,
                    limit: browser::agents::task::ilo::HelpersLimit::Count,
                },
            ),
        ],
        // The turn ended with its helpers: the stretch is over, and no helper's line is under way.
        "helpers-ended" => {
            talk.about = format!("{} · Done at 14:02", talk.name).into();
            talk.stoppable = false;
            talk.on = None;
            vec![
                done(0, before),
                Line::Said(FOUR_SHOPS.into()),
                Line::Trace(Trace {
                    key: 10,
                    running: false,
                    took: Some(52),
                    started: None,
                    items: helper_lines(HELPER_PAGES.len(), false, false),
                }),
                Line::Said(
                    "Two seats together in row 12 of the stalls are in your basket at \
                     tickets.example, 38 € each. The other three shops have none left."
                        .into(),
                ),
            ]
        }
        // A step whose answer carried a site's notes (#4122), which the line under it says.
        "notes" => {
            let github = &browser::agents::notes::NOTES[0];
            let opened = TraceStep {
                key: 10,
                glyph: "icons/globe.svg",
                title: "Opened github.com".into(),
                description: browser::agents::task::duration(2).into(),
                running: false,
                failed: false,
                opens: Opens::Nothing,
                under: github.with().into(),
                page: None,
                says: "".into(),
            };
            let reading = TraceStep {
                key: 11,
                glyph: "icons/text-align-start.svg",
                title: "Reading github.com".into(),
                description: "".into(),
                running: true,
                failed: false,
                opens: Opens::Nothing,
                under: "".into(),
                page: None,
                says: "Looking for the pull requests that wait on a review".into(),
            };
            let trace = Trace {
                key: 10,
                running: true,
                took: None,
                started: None,
                items: vec![TraceItem::Step(opened), TraceItem::Step(reading)],
            };
            vec![done(0, before), Line::Trace(trace)]
        }
        _ => vec![done(0, before), working(10, "Reading this page")],
    };
}
