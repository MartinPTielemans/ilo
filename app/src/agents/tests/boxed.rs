//! Each run of Claude Code in its box (#4101), on a stand-in assistant in a profile folder of the
//! test's own and a stand-in for the system's box, which says what it was asked for and can
//! refuse: where it starts, that a change of level or a revoked folder ends nothing, that a box
//! refused fails the run, where there's no box at all, and the sweep after a boxed run's end.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use super::kept::{Run, after, beside};
use super::*;
use crate::agents::access::ACCESS_ACTION;
use crate::agents::files::Given;
use crate::agents::runner::{self, Cli, Out};
use crate::agents::task::boxed::{LEFT_RUNNING_LOGGED, PutUp, ended_left_logged};
use crate::agents::task::ilo::{IloKind, IloLine, LeftOut, NotGiven};
use crate::agents::task::{Line, State};
use crate::agents::tier::{self, Tier};
use crate::agents::wall::leftovers::{SweepFailed, Swept};
use crate::agents::wall::{Granted, Unavailable, Wall, WallFor, WallRefused};

/// What the task was asked, as `Run::start` asks it.
const ASKED: &str = "Find two seats to Lisbon for Saturday";

/// The stand-in for the system's box: each box it was asked for, by its run's folder and config
/// as names and whether the run keeps its conversation, and whether it refuses the next.
#[derive(Default)]
struct Boxes {
    asked: Rc<RefCell<Vec<(String, String, bool)>>>,
    refuses: Rc<Cell<bool>>,
}

impl Boxes {
    fn maker(&self) -> PutUp {
        let (asked, refuses) = (self.asked.clone(), self.refuses.clone());
        Rc::new(move |wall: &WallFor| {
            // The system's box resolves every path its rules name, so the run's folder and its
            // config's are there before it's asked for, in a profile no run has used yet too.
            assert!(
                wall.task_dir.is_dir(),
                "the run's folder is made before its box"
            );
            assert!(
                wall.config.parent().is_some_and(Path::is_dir),
                "and its config's"
            );
            let name = |path: &Path| path.file_name().unwrap().to_string_lossy().into_owned();
            let each = (name(wall.task_dir), name(wall.config), wall.keeps);
            asked.borrow_mut().push(each);
            match refuses.get() {
                true => Err(WallRefused::Failed("refused in a test".into())),
                false => Ok(Wall::for_test(None)),
            }
        })
    }

    fn asked(&self) -> Vec<(String, String, bool)> {
        self.asked.borrow().clone()
    }
}

fn folder() -> Given {
    Given {
        folder: Some(Granted::for_test()),
        attached: None,
    }
}

/// The name `folder()` is shown by.
const FOLDER: &str = "folder";

/// A run whose system boxes it in, with `boxes` for the system's box and the task started with
/// `files`, at Work on pages, where every task starts.
fn started(name: &str, files: Given, cx: &mut TestAppContext) -> (Run, Boxes, String) {
    let (run, boxes) = (Run::new(name, cx), Boxes::default());
    (run.agents).update(cx, |agents, _| {
        agents.tasks.files = files;
        agents.tasks.box_available = Some(Ok(()));
        agents.tasks.wall = Some(boxes.maker());
    });
    let agent = run.start(Cli::Claude, cx);
    (run, boxes, agent)
}

fn set(run: &Run, agent: &str, tier: Tier, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| agents.set_tier(agent, tier, cx));
}

fn run_of(run: &Run, agent: &str, cx: &mut TestAppContext) -> u32 {
    (run.agents).read_with(cx, |agents, _| agents.run_of(agent))
}

/// What ilo's lines in the task's conversation say, in its order, each settled.
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

/// The access entries the log has for the task, oldest first.
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
pub(super) fn sent(run: &Run, to: usize, count: usize) -> Vec<String> {
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

/// What the task's card says of its failure.
fn why_failed(run: &Run, agent: &str, cx: &mut TestAppContext) -> String {
    match run.state(agent, cx) {
        State::Failed { why, .. } => why.to_string(),
        state => panic!("{state:?}"),
    }
}

/// Whether a run's arguments are a boxed start's.
fn boxed(args: &[String]) -> bool {
    after(args, "--settings").is_some()
}

/// Whether the stand-in was started for its run `at`.
fn ran(run: &Run, at: usize) -> bool {
    beside(&run.program, &format!("args.{at}")).exists()
}

/// Every Claude Code run starts in a box of its own, for the folder it runs in, which is its kept
/// conversation's, and keeps that conversation, a task with files too.
#[gpui::test]
fn every_claude_code_run_starts_in_its_box_and_keeps_its_conversation(cx: &mut TestAppContext) {
    for (name, files) in [("plain", Given::default()), ("files", folder())] {
        let (run, boxes, agent) = started(name, files, cx);
        let (args, first) = run.ran(1);
        assert!(boxed(&args), "{args:?}");
        assert!(first.contains(ASKED), "{first}");
        let id = (run.agents).read_with(cx, |agents, _| {
            agents.tasks.kept.of(&agent).expect("kept").id.clone()
        });
        assert_eq!(after(&args, "--session-id"), Some(id.as_str()));
        let config = format!("{agent}.json");
        assert_eq!(boxes.asked(), [(id.as_str().into(), config, true)]);
    }
}

/// A run's box that rose says so in the dev log, which the checks wait for, and is no answer.
#[gpui::test]
fn a_box_that_rose_says_so_in_the_dev_log(cx: &mut TestAppContext) {
    let (run, _, agent) = started("risen", Given::default(), cx);
    run.ran(1);
    crate::heard::listen();
    run.said(&agent, Out::Walled(Ok(())), cx);
    let heard = crate::heard::take();
    assert_eq!(heard, [format!("browser: task {agent} is boxed in")]);
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// A change of level is told to the run the task has, in a box that's the same at every level:
/// no box is asked for again, and no run ends or starts.
#[gpui::test]
fn a_change_of_level_ends_nothing(cx: &mut TestAppContext) {
    let (run, boxes, agent) = started("level", folder(), cx);
    run.ran(1);
    let before = run_of(&run, &agent, cx);
    set(&run, &agent, Tier::Full, cx);
    set(&run, &agent, Tier::Look, cx);
    assert_eq!(boxes.asked().len(), 1);
    assert_eq!(run_of(&run, &agent, cx), before);
    assert!(!ran(&run, 2), "a second run started");
    let told = sent(&run, 1, 3);
    assert_eq!(told[1], tier::changed_turn(Tier::Work, Tier::Full));
    assert_eq!(told[2], tier::changed_turn(Tier::Full, Tier::Look));
    assert_eq!(run.state(&agent, cx), State::Working);
}

/// A box that isn't put up fails the run as any start that can't, with the words that say so, and
/// nothing starts outside one.
#[gpui::test]
fn a_box_that_isnt_put_up_fails_the_run_and_starts_nothing(cx: &mut TestAppContext) {
    let (run, boxes) = (Run::new("refused", cx), Boxes::default());
    boxes.refuses.set(true);
    (run.agents).update(cx, |agents, _| {
        agents.tasks.box_available = Some(Ok(()));
        agents.tasks.wall = Some(boxes.maker());
    });
    let agent = run.start(Cli::Claude, cx);
    assert_eq!(boxes.asked().len(), 1);
    assert!(!ran(&run, 1), "it started outside its box");
    assert_eq!(why_failed(&run, &agent, cx), "Claude couldn't start.");
}

/// The system's own no, once the launcher had started, fails the run the same way, and ilo starts
/// nothing again.
#[gpui::test]
fn a_box_the_system_refuses_fails_the_run(cx: &mut TestAppContext) {
    let (run, boxes, agent) = started("system-no", Given::default(), cx);
    run.ran(1);
    let refused = WallRefused::Failed("refused in a test".into());
    run.said(&agent, Out::Walled(Err(refused)), cx);
    assert_eq!(why_failed(&run, &agent, cx), "Claude couldn't start.");
    assert_eq!(boxes.asked().len(), 1);
    assert!(!ran(&run, 2), "it started again");
}

/// Where there's no box, Claude Code never starts: the start fails with the words any start that
/// can't has, and nothing is asked of a box. Codex, which runs outside one, starts.
#[gpui::test]
fn where_there_is_no_box_claude_code_never_starts(cx: &mut TestAppContext) {
    let (run, boxes) = (Run::new("no-box", cx), Boxes::default());
    (run.agents).update(cx, |agents, _| {
        agents.tasks.box_available = Some(Err(Unavailable::NotBuilt));
        agents.tasks.wall = Some(boxes.maker());
    });
    let agent = run.start(Cli::Claude, cx);
    assert!(!ran(&run, 1), "Claude Code started with no box");
    assert!(boxes.asked().is_empty());
    assert_eq!(why_failed(&run, &agent, cx), "Claude couldn't start.");
    let codex = crate::agents::wall::runs(Cli::Codex, Err(Unavailable::NotBuilt));
    assert_eq!(codex, crate::agents::wall::Runs::Unboxed);
}

/// Where the system has no sandbox ilo uses, Claude Code starts outside a box, as a test's does
/// unless it says otherwise.
#[gpui::test]
fn where_there_is_no_sandbox_claude_code_starts_unboxed(cx: &mut TestAppContext) {
    let (run, boxes) = (Run::new("no-sandbox", cx), Boxes::default());
    (run.agents).update(cx, |agents, _| {
        agents.tasks.box_available = Some(Err(Unavailable::NoSandbox));
        agents.tasks.wall = Some(boxes.maker());
    });
    run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    assert!(!boxed(&args), "{args:?}");
    assert!(boxes.asked().is_empty());
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

/// Revoke takes a live task's folder at once: Folders in use no longer lists it, its assistant is
/// told in the run it has, and the conversation alone says so by the folder's name (J2). The log
/// has the act with no name in it.
#[gpui::test]
fn a_revoked_folder_goes_at_once_and_only_the_conversation_names_it(cx: &mut TestAppContext) {
    use crate::agents::task::revoke::REVOKED_LOGGED;
    let (run, boxes, agent) = started("revoke", folder(), cx);
    run.ran(1);
    let before = run_of(&run, &agent, cx);
    assert_eq!(in_use(&run, cx), [(FOLDER.to_string(), agent.clone())]);
    (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
    assert_eq!(in_use(&run, cx), []);
    let told = crate::agents::task::revoke::revoked_told(true, false);
    assert_eq!(sent(&run, 1, 2)[1], told);
    assert_eq!(run_of(&run, &agent, cx), before);
    assert_eq!(boxes.asked().len(), 1);
    let revoked = IloKind::Revoked {
        folder: FOLDER.into(),
    };
    assert_eq!(lines(&run, &agent, cx), [revoked]);
    let started = tier::started_logged(Tier::default(), Default::default());
    assert_eq!(logged(&run, &agent, cx), [started.as_str(), REVOKED_LOGGED]);
    // Nothing to revoke a second time, and no second line.
    (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
    assert_eq!(lines(&run, &agent, cx).len(), 1);
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
}

/// A folder's name is the user's, and one place says it: ilo's line in the task's conversation.
/// With each line made on a boxed task whose folder has a name no other string holds, and with a
/// box refused, which says no line, nothing else holds it: the dev log, what the browser was
/// asked, the activity log, the task's card and its bar, a posted notification, each tool's
/// answer to the task and to a client from outside, Library's record and every file ilo wrote.
#[gpui::test]
fn a_folders_name_is_in_ilos_line_and_nowhere_else(cx: &mut TestAppContext) {
    use crate::agents::files::Picked;
    use crate::agents::task::Start;
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
    let named = || Some(Granted::named_for_test(NAME));
    let names = |text: &str| text.contains(NAME) || text.contains("nonexistent");
    let posted = Rc::new(Posted::default());

    // Each starts its task, as its test above does, and says the line that names the folder.
    type Makes = fn(&Run, &Boxes, &mut TestAppContext) -> (String, Option<IloKind>);
    let cases: [(&str, Makes); 3] = [
        // A folder ilo couldn't give (X6): the task starts in its box without it.
        ("named-not-given", |run, _, cx| {
            (run.agents).update(cx, |agents, _| agents.tasks.files_available = true);
            let start = Start {
                tier: Tier::default(),
                model: None,
                effort: None,
                pages: Vec::new(),
                files: Picked {
                    files: Vec::new(),
                    folder: Some(Granted::named_for_test(NAME)),
                },
            };
            let agent = (run.agents)
                .update(cx, |agents, cx| {
                    agents.start_task(Cli::Claude, ASKED, None, 1, start, cx)
                })
                .expect("started");
            run.ran(1);
            let line = IloKind::StartedWithout {
                folder: NAME.into(),
                why: NotGiven::Gone,
            };
            (agent, Some(line))
        }),
        ("named-revoked", |run, _, cx| {
            let agent = run.start(Cli::Claude, cx);
            run.ran(1);
            (run.agents).update(cx, |agents, cx| agents.revoke_folder(&agent, cx));
            let line = IloKind::Revoked {
                folder: NAME.into(),
            };
            (agent, Some(line))
        }),
        ("named-refused", |run, boxes, cx| {
            boxes.refuses.set(true);
            (run.start(Cli::Claude, cx), None)
        }),
    ];
    let mut notified = 0;
    for (name, makes) in cases {
        // Each task's notification is its own, whatever the one before was told.
        cx.update(|cx| needs_you::open(posted.clone(), cx));
        crate::heard::listen();
        let (run, boxes) = (Run::new(name, cx), Boxes::default());
        (run.agents).update(cx, |agents, _| {
            agents.tasks.files.folder = named();
            agents.tasks.box_available = Some(Ok(()));
            agents.tasks.wall = Some(boxes.maker());
        });
        let (agent, line) = makes(&run, &boxes, cx);
        assert_eq!(boxes.asked().len(), 1, "{name}");

        // The conversation says it once, in ilo's line, and no other line of it does.
        let said = run.talk(&agent, cx).lines;
        let held: Vec<&Line> = (said.iter())
            .filter(|line| names(&format!("{line:?}")))
            .collect();
        match &line {
            Some(line) => assert!(
                matches!(held[..], [Line::Ilo(IloLine { kind, .. })] if kind == line),
                "{name}: {held:?}"
            ),
            None => assert!(held.is_empty(), "{name}: {held:?}"),
        }

        let mut words: Vec<String> = Vec::new();
        // Its card and its notification, as it is and once it ends, where it still works.
        let mut follow = |cx: &mut TestAppContext| {
            let cards = (run.agents).read_with(cx, |agents, cx| agents.shown_cards(cx));
            cx.update(|cx| needs_you::follow(&cards, Notify::Away, cx));
            format!("{cards:?}")
        };
        words.push(follow(cx));
        let works = run.state(&agent, cx) == State::Working;
        assert_eq!(works, line.is_some(), "{name}");
        if works {
            run.finish(&agent, cx);
            words.push(follow(cx));
        }
        (run.agents).update(cx, |agents, cx| {
            words.push(format!("{:?}", agents.status()));
            words.push(format!("{:?}", agents.question()));
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
        let heard = crate::heard::take();
        assert!(!heard.is_empty(), "{name}: the dev log was never heard");
        words.extend(heard);
        let posted = posted.0.borrow();
        // A task that ended posted its notification, so one was read.
        assert_eq!(posted.len() > notified, works, "{name}: {posted:?}");
        words.extend(posted[notified..].iter().cloned());
        notified = posted.len();
        for word in &words {
            assert!(!names(word), "{name}: {word}");
        }
    }
    assert_eq!(notified, 2);
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
        for why in ["moved", "renamed", "deleted", "ejected", "because"] {
            assert!(!told.contains(why), "a reason in {told:?}");
        }
    }
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
    let with = tier::StartedWith {
        files: 1,
        left_out: true,
        ..Default::default()
    };
    let started = tier::started_logged(Tier::default(), with);
    assert_eq!(
        started,
        "Started at Work on pages · 1 file · files left out"
    );
    assert_eq!(logged(&run, &agent, cx), [started]);
}

/// A folder the user picked that ilo doesn't give isn't given, and the task starts in its box
/// without it, its line saying why by the folder's name (X6): one that no longer resolves to
/// itself, moved, renamed or gone, and one ilo never gives, its own tasks' folder among them.
#[gpui::test]
fn a_folder_ilo_doesnt_give_starts_the_task_without_it_and_says_why(cx: &mut TestAppContext) {
    use crate::agents::files::Picked;
    use crate::agents::task::Start;
    type Folder = fn(&Run, &mut TestAppContext) -> Granted;
    let cases: [(&str, Folder, NotGiven); 2] = [
        (
            "gone",
            |_, _| Granted::named_for_test("Kept elsewhere"),
            NotGiven::Gone,
        ),
        (
            "never",
            |run, cx| {
                let tasks = (run.agents).read_with(cx, |agents, _| agents.tasks_dir());
                let folder = tasks.join("Kept elsewhere");
                std::fs::create_dir_all(&folder).unwrap();
                Granted::at_for_test(folder.canonicalize().unwrap())
            },
            NotGiven::Never,
        ),
    ];
    for (name, folder, why) in cases {
        let (run, boxes) = (Run::new(name, cx), Boxes::default());
        (run.agents).update(cx, |agents, _| {
            agents.tasks.files_available = true;
            agents.tasks.box_available = Some(Ok(()));
            agents.tasks.wall = Some(boxes.maker());
        });
        let start = Start {
            tier: Tier::default(),
            model: None,
            effort: None,
            pages: Vec::new(),
            files: Picked {
                files: Vec::new(),
                folder: Some(folder(&run, cx)),
            },
        };
        let agent = (run.agents)
            .update(cx, |agents, cx| {
                agents.start_task(Cli::Claude, ASKED, None, 1, start, cx)
            })
            .expect("started");
        let (args, _) = run.ran(1);
        assert!(boxed(&args), "{name}: {args:?}");
        let given = (run.agents).read_with(cx, |agents, _| {
            agents.task(&agent).map(|task| task.files.folder.is_some())
        });
        assert_eq!(given, Some(false), "{name}");
        let line = IloKind::StartedWithout {
            folder: "Kept elsewhere".into(),
            why,
        };
        assert_eq!(lines(&run, &agent, cx), [line], "{name}");
    }
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

/// A sweep that couldn't end everything a boxed run started is in the log, once for that run,
/// and the task's run goes on; a clean one, or one of a run that wasn't boxed, says nothing.
#[gpui::test]
fn a_sweep_that_couldnt_end_everything_is_in_the_log(cx: &mut TestAppContext) {
    let (run, _, agent) = started("swept", Given::default(), cx);
    run.ran(1);
    let now = run_of(&run, &agent, cx);
    let before = logged(&run, &agent, cx);
    // As a boxed start keeps them, with a run before that ended.
    (run.agents).update(cx, |agents, _| {
        agents.task_mut(&agent).unwrap().ran_boxed = vec![(now - 1, None), (now, None)];
    });
    swept(&run, &agent, now - 1, Err(SweepFailed::Cap), cx);
    swept(&run, &agent, now - 1, Err(SweepFailed::Cap), cx);
    swept(&run, &agent, now, Ok(Swept::default()), cx);
    swept(&run, &agent, now + 1, Err(SweepFailed::Unlisted), cx);
    let mut after = before;
    after.push(LEFT_RUNNING_LOGGED.into());
    assert_eq!(logged(&run, &agent, cx), after);
    let left = (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().ran_boxed.len());
    assert_eq!(left, 0);
    assert_eq!(run_of(&run, &agent, cx), now);
    assert_eq!(run.state(&agent, cx), State::Working);
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
        agents.task_mut(&agent).unwrap().ran_boxed = vec![(at, Some(PLACE.into()))];
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
        let record = agents.tasks.kept.of_mut(&past).unwrap();
        record.left.push(PLACE.into());
        agents.came_clean(vec![(PLACE.to_string(), 0), (PLACE.to_string(), 4)], cx);
    });
    assert_eq!(logged(&next, &past, cx), [ended_left_logged(2)]);
    let left = (next.agents).read_with(cx, |agents, _| {
        agents.tasks.kept.of(&past).unwrap().left.clone()
    });
    assert!(left.is_empty(), "{left:?}");
}
