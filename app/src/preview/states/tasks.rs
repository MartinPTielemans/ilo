//! Made-up tasks: their Spaces, cards and conversations.

use crate::*;

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

/// The pages the done task left the user, each with its site.
pub(crate) const TASK_PAGES: [(&str, &str); 3] = [
    ("Booking · Taberna", "https://taberna.example/booking"),
    ("Menu · Taberna", "https://taberna.example/menu"),
    ("Directions", "https://maps.example/taberna"),
];

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

/// The helpers' lines of the bench's task: each on its page, reading it, and with `waits`, the
/// one that handed its page to the user. None is `running` once the turn is over.
fn helper_lines(count: usize, waits: bool, running: bool) -> Vec<TraceItem> {
    let sites = helper_tabs();
    let helper = |at: usize, tool: &str, did: &str, running| {
        TraceItem::Helper(TraceStep {
            key: 10 + at,
            glyph: browser::agents::tools::step_glyph(tool),
            title: format!("{did} {}", sites[at].1).into(),
            description: "".into(),
            running,
            failed: false,
            opens: Opens::Nothing,
            under: "".into(),
        })
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
        items: helper_lines(count, waits, true),
    }
}

/// The helpers' pages as the task's conversation lists them, each tab with its site: the tiles
/// of Pages, and the task's group at the sidebar's foot.
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
        asks: false,
        dot,
        answers: Vec::new(),
        question: None,
        open: match dot {
            Dot::Done => {
                let site = |url: &str| url.split('/').nth(2).unwrap_or("").into();
                let pages = (FIRST_TASK + 100..).zip(TASK_PAGES);
                pages.map(|(tab, (_, url))| (tab, site(url))).collect()
            }
            _ => Vec::new(),
        },
        page_steps: Vec::new(),
        retry: false,
        fix: None,
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
            dot,
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
            cards[0].dot = Dot::Waiting;
        }
        (Some(count), false) => cards[0].status = helper_trace(count, false).line(),
        (None, _) => {}
    }
    // The bench's task that has ended (#4105) is done on its card too, which the strip under its
    // page and the page's ring follow, with the page it left the user.
    if let Some(name) = bench_ended() {
        cards[0].name = name.into();
        cards[0].status = "Two seats in row 12 are in your basket".into();
        cards[0].dot = Dot::Done;
        cards[0].open = vec![(FIRST_TASK + 200, "tickets.example".into())];
    }
    // Newest first, as the Agents send them.
    cards.reverse();
    cards
}

/// The assistant of the task PREVIEW_TASK's bench shows as ended (#4105), if it's one of those
/// states: all but picked-up, which a reply set to work again.
pub(crate) fn bench_ended() -> Option<&'static str> {
    match states::var("PREVIEW_TASK").ok()?.as_str() {
        "ended" | "ended-first" | "gone" | "not-kept" | "helpers-ended" => Some("Claude"),
        "codex-ended" => Some("Codex"),
        _ => None,
    }
}

/// The working task's conversation: a step done, one that failed, what the agent said of it, and
/// the step under way, on four pages, so it opens on Pages.
pub(crate) fn task_conversation(agent: SharedString) -> Conversation {
    let (title, _, _) = TASKS[0];
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
        progress: "step 3 · 4 pages".into(),
        asked: "Find two tickets for Saturday's concert, in the stalls".into(),
        page: None,
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            items: items.clone(),
        })],
        tabs: vec![
            (FIRST_TASK + 200, "tickets.example".into()),
            (FIRST_TASK + 201, "seatmap.example".into()),
            (FIRST_TASK + 202, "tickets.example".into()),
            (FIRST_TASK + 203, "maps.example".into()),
        ],
        on: Some(FIRST_TASK + 200),
        stoppable: true,
        open: None,
        answers: Vec::new(),
        writing: false,
        tier: Default::default(),
    };
    bench_task(&mut talk, items);
    talk
}

/// PREVIEW_TASK=<state> (#4095's bench) shows the working task's conversation beside its page in
/// one of the states ilo's own lines are made for, from made-up data: working, refused,
/// off-limits, yours, tab-back, signed-in, raised or notes; or of a task that has ended (#4105):
/// ended, ended-first, picked-up, gone, not-kept or codex-ended; or of one with helpers (#4102):
/// helpers, helper, helpers-limit or helpers-ended.
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
            items: items.to_vec(),
        })
    };
    let working = |key: usize, title: &str| {
        Line::Trace(Trace {
            key,
            running: true,
            took: None,
            items: vec![TraceItem::Step(TraceStep {
                key,
                glyph: "icons/mouse-pointer-2.svg",
                title: title.to_string().into(),
                description: "".into(),
                running: true,
                failed: false,
                opens: Opens::Nothing,
                under: "".into(),
            })],
        })
    };
    let ilo = |key, kind, settled| Line::Ilo(IloLine { key, kind, settled });
    let line = |key, kind| ilo(key, kind, false);
    let before = &items[..items.len() - 1];
    let stopped = |why, at: &str, count| IloKind::Stopped {
        tab,
        at: at.to_string().into(),
        why,
        tier: Tier::Work,
        count,
    };
    talk.progress = "".into();
    // It stands on its one page, beside it.
    talk.tabs.truncate(1);
    talk.lines = match state.as_str() {
        "refused" => vec![
            done(0, before),
            Line::Said("Two seats in row 12 are in the basket. Placing the order is the last step.".into()),
            line(100, stopped(Why::Order, "Place order", 2)),
        ],
        "off-limits" => vec![
            done(0, before),
            line(100, stopped(Why::Fenced, "seatmap.example", 1)),
        ],
        "yours" => vec![
            done(0, before),
            Line::Said("tickets.example wants you to sign in before it shows the seats. I'll carry on once you hand the tab back.".into()),
            line(100, IloKind::Yours { tab }),
        ],
        "tab-back" => vec![
            done(0, before),
            Line::Said("I need the seat map back to pick two seats together.".into()),
            line(100, IloKind::TabBack { tab: FIRST_TASK + 201 }),
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
        // A task that has ended (#4105): with its reply field, the first one's line saying where
        // the conversation lives, a reply that took it up, and the three lines that stand in the
        // field's place.
        "ended" | "ended-first" | "picked-up" | "gone" | "not-kept" | "codex-ended" => {
            let ended = state != "picked-up";
            if state == "codex-ended" {
                talk.name = "Codex".into();
            }
            talk.about = match ended {
                true => format!("{} · Done at 14:02", talk.name).into(),
                false => talk.about.clone(),
            };
            talk.stoppable = !ended;
            if ended {
                talk.on = None;
            }
            let mut lines = vec![
                done(0, &items),
                Line::Said("Two seats in row 12 of the stalls are in your basket on tickets.example.".into()),
            ];
            match state.as_str() {
                "ended-first" => lines.push(line(100, IloKind::KeptHere)),
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
        // A task at work with four helpers (#4102), each on a page of its own: the stretch counts
        // them and opens to a line for each, a step's line in ilo's words for the helper's page.
        // The third handed its page to the user for a sign-in, which ilo's line under them says,
        // while the other three go on, so the task needs the user. The conversation opens with
        // the task's first page alone, as the task did; `helper_tabs` are the rest, which the
        // bench gives it once it shows.
        "helpers" => {
            talk.about = format!("{} · {}", talk.name, browser::agents::task::NEEDS_YOU).into();
            let yours = IloKind::Yours {
                tab: helper_tabs()[HELPER_WAITING].0,
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
        // ilo stopped the task from starting a fifth: its line, with Take Over alone.
        "helpers-limit" => vec![
            done(0, before),
            Line::Said(FOUR_SHOPS.into()),
            Line::Trace(helper_trace(HELPER_PAGES.len(), false)),
            line(100, IloKind::Helpers { tab, deep: false }),
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
                    items: helper_lines(HELPER_PAGES.len(), false, false),
                }),
                Line::Said("Encore has two together in row 12 of the stalls, at 38 € each. The other three have none left.".into()),
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
            };
            let trace = Trace {
                key: 10,
                running: true,
                took: None,
                items: vec![TraceItem::Step(opened), TraceItem::Step(reading)],
            };
            vec![done(0, before), Line::Trace(trace)]
        }
        _ => vec![done(0, before), working(10, "Reading this page")],
    };
}
