//! A reply to a task that has ended (#4105): what its run keeps, how the reply takes it up, what
//! its conversation says where it can't, and how what was kept goes. The assistant is a stand-in
//! the tests write themselves, in a profile folder of their own: never the user's own.

#[cfg(unix)]
mod cards;
#[cfg(unix)]
pub(crate) mod past;

use super::*;
use crate::agents::kept::{self, Id};
use crate::agents::ready::{Found, Ready};
use crate::agents::runner::{self, Cli, Out};
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};
use crate::agents::task::{AskedOn, Conversation, Line, State, TaskAction, Word};
use crate::agents::tier::Tier;

/// A stand-in for `claude`. A run of it writes what it was started with and where beside itself,
/// numbered by run, then keeps what it's sent and never answers. Asked to purge, it writes what
/// it was asked and ends with the status in its `.code`, or well.
#[cfg(unix)]
const STAND_IN: &str = r#"#!/bin/sh
if [ "$1" = purge ]; then
  printf '%s\n' "$@" >> "$0.purge"
  exit "$(cat "$0.code" 2>/dev/null || echo 0)"
fi
run=1
while [ -e "$0.args.$run" ]; do run=$((run + 1)); done
pwd > "$0.dir.$run"
printf '%s\n' "$@" > "$0.args.$run"
exec cat > "$0.in.$run"
"#;

/// A run of the browser with a profile folder of its own, agents on, and Claude signed in at a
/// stand-in in it.
#[cfg(unix)]
pub(super) struct Run {
    pub(super) agents: Entity<Agents>,
    pub(super) browser: Rc<Made>,
    profile: PathBuf,
    pub(super) program: PathBuf,
}

#[cfg(unix)]
impl Run {
    pub(super) fn new(name: &str, cx: &mut TestAppContext) -> Run {
        use std::os::unix::fs::PermissionsExt;
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let profile = std::env::temp_dir().join(format!(
            "ilo-kept-tasks-{}-{name}-{made}",
            std::process::id()
        ));
        _ = std::fs::remove_dir_all(&profile);
        std::fs::create_dir_all(profile.join("bin")).unwrap();
        let profile = profile.canonicalize().unwrap();
        let program = profile.join("bin/claude");
        std::fs::write(&program, STAND_IN).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Another test's thread that forked while the script was open for writing holds it until
        // its child runs, and it won't run meanwhile ("Text file busy"): run it until it does.
        for _ in 0..200 {
            match crate::child::command(&program)
                .stdin(std::process::Stdio::null())
                .status()
            {
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                ran => {
                    ran.expect("the stand-in didn't run");
                    break;
                }
            }
        }
        for left in ["args", "dir", "in"] {
            std::fs::remove_file(beside(&program, &format!("{left}.1"))).unwrap();
        }
        Run::again(profile, program, cx)
    }

    /// The browser, started on a profile and the stand-in a run before left.
    fn again(profile: PathBuf, program: PathBuf, cx: &mut TestAppContext) -> Run {
        // The stand-in's thread says when it ends, whenever that is, from a thread GPUI doesn't
        // run: unless told the test has real I/O, GPUI fails the test for it.
        cx.executor().allow_parking();
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let path = profile.join("agents.json");
        let agents = agents(&browser, &configs, Some(path.clone()), cx);
        let saved = std::fs::read_to_string(&path).unwrap_or_default();
        agents.update(cx, |agents, cx| {
            agents.load(&saved, cx);
            agents.set_threads(true, cx);
            for cli in Cli::ALL {
                let found = Found {
                    ready: Ready::SignedIn,
                    program: (cli == Cli::Claude).then(|| program.clone()),
                    ..Default::default()
                };
                agents.tasks.found.insert(cli, found);
            }
            agents.tasks.default = Some(Cli::Claude);
            agents.save();
        });
        Run {
            agents,
            browser,
            profile,
            program,
        }
    }

    pub(super) fn start(&self, cli: Cli, cx: &mut TestAppContext) -> String {
        let page = Some(AskedOn {
            url: "https://www.example.com/trains".into(),
            title: "Trains to Lisbon".into(),
        });
        (self.agents)
            .update(cx, |agents, cx| {
                agents.start_task(
                    cli,
                    "Find two seats to Lisbon for Saturday",
                    page,
                    1,
                    Tier::default(),
                    cx,
                )
            })
            .expect("started")
    }

    pub(super) fn said(&self, agent: &str, out: Out, cx: &mut TestAppContext) {
        let event = runner::Event {
            agent: agent.into(),
            run: (self.agents).read_with(cx, |agents, _| agents.run_of(agent)),
            out,
        };
        (self.agents).update(cx, |agents, cx| agents.task_said(event, cx));
    }

    /// The task's turn ends well, with its answer.
    pub(super) fn finish(&self, agent: &str, cx: &mut TestAppContext) {
        self.said(agent, Out::Said("Two seats at 9:05, car 4.".into()), cx);
        self.said(agent, Out::Ended(None), cx);
        assert!(matches!(self.state(agent, cx), State::Done { .. }));
    }

    pub(super) fn state(&self, agent: &str, cx: &mut TestAppContext) -> State {
        (self.agents).read_with(cx, |agents, _| agents.task(agent).unwrap().state.clone())
    }

    pub(super) fn talk(&self, agent: &str, cx: &mut TestAppContext) -> Conversation {
        (self.agents)
            .read_with(cx, |agents, cx| agents.conversation(agent, cx))
            .expect("its conversation")
    }

    /// The kinds of ilo's lines in the task's conversation, in its order.
    pub(super) fn ilo_lines(&self, agent: &str, cx: &mut TestAppContext) -> Vec<IloKind> {
        (self.talk(agent, cx).lines.iter())
            .filter_map(|line| match line {
                Line::Ilo(line) => Some(line.kind.clone()),
                _ => None,
            })
            .collect()
    }

    /// The id ilo holds for the task's conversation.
    fn id(&self, agent: &str, cx: &mut TestAppContext) -> Id {
        (self.agents).read_with(cx, |agents, _| {
            (agents.tasks.kept.of(agent).expect("nothing kept"))
                .id
                .clone()
        })
    }

    fn kept(&self, cx: &mut TestAppContext) -> (usize, usize) {
        self.agents.read_with(cx, |agents, _| agents.kept_tasks())
    }

    /// What the stand-in's `run` was started with, once it has the run's first message, and that
    /// message's text.
    pub(super) fn ran(&self, run: usize) -> (Vec<String>, String) {
        let sent = wait_for(&beside(&self.program, &format!("in.{run}")));
        let args = std::fs::read_to_string(beside(&self.program, &format!("args.{run}"))).unwrap();
        let first: Value = serde_json::from_str(sent.lines().next().unwrap()).unwrap();
        let text = first["message"]["content"].as_str().unwrap().to_string();
        (args.lines().map(String::from).collect(), text)
    }

    /// Where the stand-in's `run` ran.
    fn ran_in(&self, run: usize) -> PathBuf {
        let dir = std::fs::read_to_string(beside(&self.program, &format!("dir.{run}"))).unwrap();
        PathBuf::from(dir.trim_end())
    }

    /// What the stand-in was asked to purge, a line per argument, once the removal is over.
    fn purged(&self, cx: &mut TestAppContext) -> Vec<String> {
        cx.run_until_parked();
        let asked = std::fs::read_to_string(beside(&self.program, "purge")).unwrap_or_default();
        asked.lines().map(String::from).collect()
    }

    fn folder(&self, id: &Id) -> PathBuf {
        kept::folder(&self.profile.join(kept::FOLDER), id)
    }

    /// Every file under the profile's folder but the stand-in's own, as text.
    pub(super) fn files(&self) -> Vec<(PathBuf, String)> {
        let (mut folders, mut files) = (vec![self.profile.clone()], Vec::new());
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else if !path.starts_with(self.profile.join("bin")) {
                    let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
                    files.push((path, text));
                }
            }
        }
        files
    }
}

/// A file the stand-in writes beside itself.
#[cfg(unix)]
pub(super) fn beside(program: &std::path::Path, what: &str) -> PathBuf {
    PathBuf::from(format!("{}.{what}", program.display()))
}

/// The file's text, once it has some.
#[cfg(unix)]
fn wait_for(path: &std::path::Path) -> String {
    for _ in 0..500 {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if !text.is_empty() {
            return text;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{} stayed empty", path.display());
}

/// The argument after `flag`.
#[cfg(unix)]
pub(super) fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).map(String::as_str)
}

/// A kept task's run starts with an id of ilo's making, in a folder of its own under the
/// profile's, named by the id, with nothing in it. ilo holds the id and counts it.
#[cfg(unix)]
#[gpui::test]
fn a_kept_task_runs_under_an_id_of_ilos_in_a_folder_of_its_own(cx: &mut TestAppContext) {
    let run = Run::new("kept", cx);
    let agent = run.start(Cli::Claude, cx);
    let (args, first) = run.ran(1);
    let id = run.id(&agent, cx);
    assert_eq!(after(&args, "--session-id"), Some(id.as_str()));
    assert!(!args.iter().any(|arg| arg == "--no-session-persistence"));
    assert!(!args.iter().any(|arg| arg == "--resume"));
    assert!(first.starts_with("Find two seats to Lisbon for Saturday"));
    assert_eq!(run.ran_in(1), run.folder(&id));
    assert_eq!(std::fs::read_dir(run.folder(&id)).unwrap().count(), 0);
    assert_eq!(run.kept(cx), (1, 0));
    let second = run.start(Cli::Claude, cx);
    run.ran(2);
    run.finish(&second, cx);
    assert_eq!(run.kept(cx), (2, 0));
    let saved = std::fs::read_to_string(run.profile.join("agents.json")).unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["tasks"]["keep"], json!(true));
}

/// A reply to a task that ended takes it up: at Work on pages though it ended at Full access,
/// which ilo's line over the reply says, by a run that goes on with the same conversation and is
/// started with everything a new task's run is, since its assistant brings back no tool or
/// permission by itself. The run is told the reply alone, as the assistant has what came before.
#[cfg(unix)]
#[gpui::test]
fn a_reply_takes_an_ended_task_up_at_work_on_pages(cx: &mut TestAppContext) {
    let run = Run::new("resumed", cx);
    let agent = run.start(Cli::Claude, cx);
    let (new, _) = run.ran(1);
    let id = run.id(&agent, cx);
    // The user gives it the highest access while it works, and it ends there.
    (run.agents).update(cx, |agents, cx| agents.set_tier(&agent, Tier::Full, cx));
    run.finish(&agent, cx);
    // Its run went, as at its assistant's own end.
    run.said(&agent, Out::Gone(String::new()), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    let reply = "Are there two together for Sunday too?";
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, reply, cx));

    let (resumed, first) = run.ran(2);
    assert_eq!(after(&resumed, "--resume"), Some(id.as_str()));
    assert_eq!(first, reply);
    assert_eq!(run.ran_in(2), run.folder(&id));
    // Every flag of the access the app decided is passed again: a new task's own, which is Work
    // on pages' until Full access has flags of its own, and none it wasn't given.
    let as_new: Vec<String> = (resumed.iter())
        .map(|arg| match arg.as_str() {
            "--resume" => "--session-id".to_string(),
            _ => arg.clone(),
        })
        .collect();
    assert_eq!(as_new, new);
    let tier = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().tier)
    };
    assert_eq!(tier(cx), Tier::Work);

    // ilo's line is right over the reply, under the first run's work and answer.
    let talk = run.talk(&agent, cx);
    let at = (talk.lines.iter())
        .position(|line| matches!(line, Line::You(said) if said.as_ref() == reply))
        .expect("the reply");
    assert!(matches!(
        &talk.lines[at - 1],
        Line::Ilo(IloLine {
            kind: IloKind::Resumed { tier: Tier::Work },
            ..
        })
    ));
    let answer = (talk.lines[..at - 1].iter())
        .any(|line| matches!(line, Line::Said(said) if said.contains("9:05")));
    assert!(answer, "the first run's answer");
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(run.kept(cx), (1, 0), "a pick-up is the same conversation");
    // The activity log has it once, under the task.
    let noted = log(&run.agents, &agent, cx);
    let resumes = (noted.iter()).filter(|(action, _, _)| action == "resumed");
    assert_eq!(resumes.count(), 1, "{noted:?}");
    // A reply while it works is a reply, with no line of ilo's.
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "By the window", cx));
    let resumed = |kind: &IloKind| matches!(kind, IloKind::Resumed { .. });
    let lines = run.ilo_lines(&agent, cx);
    assert_eq!(lines.iter().filter(|kind| resumed(kind)).count(), 1);
    // Done again at Work on pages, the next reply takes it up there: the reply is the resume, and
    // ilo has no second line.
    run.finish(&agent, cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And Monday?", cx));
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(tier(cx), Tier::Work);
    let lines = run.ilo_lines(&agent, cx);
    assert_eq!(lines.iter().filter(|kind| resumed(kind)).count(), 1);
    // And a task that only looked comes back only looking.
    assert_eq!(kept::comes_back(Tier::Look), Tier::Look);
}

/// On a task that has ended the level's menu sets the level its next reply starts at, Full access
/// too: the chip says it and ilo's line and the activity log have it at the pick, as on a task at
/// work, and nothing starts until the reply, which comes at that level with no line over it. Any
/// site is the same. A refusal's button on a task that has ended does nothing.
#[cfg(unix)]
#[gpui::test]
fn a_level_picked_on_an_ended_task_is_the_one_its_reply_starts_at(cx: &mut TestAppContext) {
    let run = Run::new("picked", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    run.finish(&agent, cx);
    run.said(&agent, Out::Gone(String::new()), cx);
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));
    let act = |action, cx: &mut TestAppContext| {
        (run.agents).update(cx, |agents, cx| _ = agents.task_action(&agent, action, cx));
    };
    let tier = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().tier)
    };
    let started = || beside(&run.program, "args.2").exists();
    let accesses = |cx: &mut TestAppContext| {
        let noted = log(&run.agents, &agent, cx);
        (noted.iter())
            .filter(|(action, _, _)| action == "access")
            .count()
    };

    let before = accesses(cx);
    act(TaskAction::PickTier(Tier::Full), cx);
    assert_eq!(tier(cx), Tier::Full);
    assert_eq!(run.talk(&agent, cx).tier, Tier::Full, "the chip");
    let changed = IloKind::TierChanged {
        from: Tier::Work,
        to: Tier::Full,
    };
    assert!(run.ilo_lines(&agent, cx).contains(&changed));
    assert_eq!(accesses(cx), before + 1, "the pick's activity entry");
    assert!(!started(), "a pick started a run");
    assert!(matches!(run.state(&agent, cx), State::Done { .. }));

    act(TaskAction::SetTier(Tier::Look), cx);
    assert_eq!(
        tier(cx),
        Tier::Full,
        "a line's button acted on an ended task"
    );

    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And for Sunday?", cx));
    let (resumed, _) = run.ran(2);
    assert!(resumed.iter().any(|arg| arg == "--resume"));
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(tier(cx), Tier::Full, "the reply didn't start at the pick");
    let resumed = |kind: &IloKind| matches!(kind, IloKind::Resumed { .. });
    assert!(!run.ilo_lines(&agent, cx).iter().any(resumed));
}

/// A reply to a task done or stopped in this run, at the access it had, has no line of ilo's over
/// it: the reply is the resume. The activity log still has each.
#[cfg(unix)]
#[gpui::test]
fn a_reply_at_the_access_a_task_had_has_no_line_of_ilos(cx: &mut TestAppContext) {
    let run = Run::new("same-access", cx);
    let done = run.start(Cli::Claude, cx);
    run.ran(1);
    run.finish(&done, cx);
    run.said(&done, Out::Gone(String::new()), cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&done, "And for Sunday?", cx));
    let (resumed, _) = run.ran(2);
    assert!(resumed.iter().any(|arg| arg == "--resume"));
    assert_eq!(run.state(&done, cx), State::Working);
    assert_eq!(run.ilo_lines(&done, cx), []);

    // One that only looks, stopped, comes back only looking, with no line either.
    let stopped = run.start(Cli::Claude, cx);
    run.ran(3);
    (run.agents).update(cx, |agents, cx| {
        agents.set_tier(&stopped, Tier::Look, cx);
        _ = agents.task_action(&stopped, TaskAction::Stop, cx);
    });
    assert_eq!(run.state(&stopped, cx), State::Stopped);
    (run.agents).update(cx, |agents, cx| agents.reply(&stopped, "Go on", cx));
    assert_eq!(run.state(&stopped, cx), State::Working);
    let resumed = |kind: &IloKind| matches!(kind, IloKind::Resumed { .. });
    assert!(!run.ilo_lines(&stopped, cx).iter().any(resumed));
    (run.agents).read_with(cx, |agents, _| {
        assert_eq!(agents.task(&stopped).unwrap().tier, Tier::Look)
    });
    for agent in [&done, &stopped] {
        let noted = log(&run.agents, agent, cx);
        let resumes = (noted.iter()).filter(|(action, _, _)| action == "resumed");
        assert_eq!(resumes.count(), 1, "{noted:?}");
    }
}

/// A task whose run still waits when the reply comes is taken up the same, on that run.
#[cfg(unix)]
#[gpui::test]
fn a_reply_to_a_run_that_still_waits_takes_the_task_up_on_it(cx: &mut TestAppContext) {
    let run = Run::new("waits", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    run.finish(&agent, cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And for Sunday?", cx));
    let input = beside(&run.program, "in.1");
    for _ in 0..500 {
        if wait_for(&input).lines().count() == 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let sent = wait_for(&input);
    let second: Value = serde_json::from_str(sent.lines().nth(1).expect("the reply")).unwrap();
    assert_eq!(second["message"]["content"], json!("And for Sunday?"));
    assert!(!beside(&run.program, "args.2").exists(), "a second run");
    assert_eq!(run.state(&agent, cx), State::Working);
    // At the access it had: the reply is the resume, with no line of ilo's.
    assert_eq!(run.ilo_lines(&agent, cx), []);
}

/// Settings › Agents' switch off: a task's run keeps nothing, ilo holds no id and makes no folder,
/// and the task, once done, says its assistant didn't keep the conversation where its reply field
/// was, with Start Again: not that it no longer has one, which is for one that was kept.
#[cfg(unix)]
#[gpui::test]
fn with_the_switch_off_a_run_keeps_nothing(cx: &mut TestAppContext) {
    let run = Run::new("off", cx);
    (run.agents).update(cx, |agents, cx| agents.set_keeps_tasks(false, cx));
    let agent = run.start(Cli::Claude, cx);
    let (args, _) = run.ran(1);
    assert!(args.iter().any(|arg| arg == "--no-session-persistence"));
    assert!(!args.iter().any(|arg| arg == "--session-id"));
    assert_eq!(run.kept(cx), (0, 0));
    assert!(!run.profile.join(kept::FOLDER).exists());
    assert_eq!(run.ran_in(1), run.profile.join("Tasks").join(&agent));
    run.finish(&agent, cx);
    assert_eq!(run.ilo_lines(&agent, cx), [IloKind::NotKept]);
    let talk = run.talk(&agent, cx);
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("ilo's line isn't last");
    };
    assert!(ilo::in_field(&last.kind));
    assert_eq!(ilo::buttons(last), [IloButton::StartAgain]);
    // The switch is kept, and on again the next task's run is.
    let saved = std::fs::read_to_string(run.profile.join("agents.json")).unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["tasks"]["keep"], json!(false));
    (run.agents).update(cx, |agents, cx| agents.set_keeps_tasks(true, cx));
    run.start(Cli::Claude, cx);
    let (args, _) = run.ran(2);
    assert!(args.iter().any(|arg| arg == "--session-id"));
    // The task that ended with the switch off still kept none, whatever the switch says now.
    assert_eq!(run.ilo_lines(&agent, cx), [IloKind::NotKept]);
}

/// Codex keeps nothing, whatever the switch says: its task, stopped, says so where its reply
/// field was, with Start Again, and ilo holds nothing for it.
#[cfg(unix)]
#[gpui::test]
fn an_ended_codex_task_says_codex_keeps_none(cx: &mut TestAppContext) {
    let run = Run::new("codex", cx);
    let agent = run.start(Cli::Codex, cx);
    // Codex isn't installed here, so its task couldn't go on, and keeps its field for the fix.
    assert!(matches!(run.state(&agent, cx), State::Failed { .. }));
    assert_eq!(run.ilo_lines(&agent, cx), []);
    // One that ran and was stopped, or is done.
    (run.agents).update(cx, |agents, _| {
        agents.task_mut(&agent).unwrap().state = State::Stopped
    });
    assert_eq!(run.ilo_lines(&agent, cx), [IloKind::KeepsNothing]);
    let talk = run.talk(&agent, cx);
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("ilo's line isn't last");
    };
    assert!(ilo::in_field(&last.kind));
    assert_eq!(ilo::buttons(last), [IloButton::StartAgain]);
    assert_eq!(run.kept(cx), (0, 0));
    assert!(!run.profile.join(kept::FOLDER).exists());
}

/// A conversation its assistant no longer has: the run that was to go on with it says so and
/// goes, the task is as it was before the reply, which never reached it, and its conversation
/// says so where its reply field was. Nothing more is started for it.
#[cfg(unix)]
#[gpui::test]
fn a_conversation_its_assistant_no_longer_has_says_so(cx: &mut TestAppContext) {
    let run = Run::new("gone", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    run.finish(&agent, cx);
    let done = run.state(&agent, cx);
    run.said(&agent, Out::Gone(String::new()), cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And for Sunday?", cx));
    run.ran(2);
    // As Claude Code says it.
    let said = format!("No conversation found with session ID: {}", id.as_str());
    run.said(&agent, Out::Gone(said), cx);

    assert_eq!(run.state(&agent, cx), done);
    let talk = run.talk(&agent, cx);
    assert!(
        !(talk.lines.iter()).any(|line| matches!(line, Line::You(said) if said.contains("Sunday")))
    );
    assert_eq!(run.ilo_lines(&agent, cx), [IloKind::Gone]);
    assert!(matches!(talk.lines.last(), Some(Line::Ilo(_))));
    // The id stays ilo's to remove, with the task.
    assert_eq!(run.kept(cx), (1, 0));
    (run.agents).update(cx, |agents, cx| agents.drop_task(&agent, cx));
    assert_eq!(run.purged(cx).len(), 3);
    assert_eq!(run.kept(cx), (0, 0));
}

/// A reply that reached a task still under way, whose run had gone, starts a run that finds the
/// conversation gone. Nobody stopped that task: it's Interrupted, why is the line in its reply
/// field's place, and with Start Again there, it takes no reply.
#[cfg(unix)]
#[gpui::test]
fn a_task_under_way_whose_conversation_went_is_interrupted(cx: &mut TestAppContext) {
    let run = Run::new("gone-under-way", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    (run.agents).update(cx, |agents, _| agents.run_went_unheard(&agent));
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And for Sunday?", cx));
    let (args, _) = run.ran(2);
    assert!(args.iter().any(|arg| arg == id.as_str()), "{args:?}");
    let said = format!("No conversation found with session ID: {}", id.as_str());
    run.said(&agent, Out::Gone(said), cx);

    let why = "Claude no longer has this conversation.";
    assert_eq!(
        run.state(&agent, cx),
        State::Failed {
            why: why.into(),
            assistant: false,
        }
    );
    let card = (run.agents).read_with(cx, |agents, cx| {
        let mut cards = agents.shown_cards(cx).into_iter();
        cards.find(|card| card.agent.as_ref() == agent).unwrap()
    });
    assert_eq!(card.word, Word::Interrupted);
    assert_eq!(card.status.as_ref(), why);
    let talk = run.talk(&agent, cx);
    assert_eq!(talk.about.as_ref(), "Claude · Interrupted");
    assert_eq!(run.ilo_lines(&agent, cx), [IloKind::Gone]);
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("ilo's line isn't last");
    };
    assert!(ilo::in_field(&last.kind));
    assert_eq!(ilo::buttons(last), [IloButton::StartAgain]);
}

/// Removing a task removes what its assistant kept of it: the assistant is asked for that task's
/// folder alone, then the folder, the id and the file of ids go.
#[cfg(unix)]
#[gpui::test]
fn removing_a_task_removes_its_conversation(cx: &mut TestAppContext) {
    let run = Run::new("removed", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let other = run.start(Cli::Claude, cx);
    run.ran(2);
    let (id, others) = (run.id(&agent, cx), run.id(&other, cx));
    run.finish(&agent, cx);
    (run.agents).update(cx, |agents, cx| agents.drop_task(&agent, cx));
    let folder = run.folder(&id);
    assert_eq!(run.purged(cx), ["purge", folder.to_str().unwrap(), "--yes"]);
    assert!(!folder.exists());
    assert_eq!(run.kept(cx), (1, 0));
    assert!(
        run.agents
            .read_with(cx, |agents, _| agents.tasks.kept.of(&agent).is_none())
    );
    // The other task's stays, and nothing ilo wrote names the one that went.
    assert!(run.folder(&others).exists());
    for (path, text) in run.files() {
        assert!(!text.contains(id.as_str()), "{} names it", path.display());
    }
    // With the last one, the folder of them all goes.
    (run.agents).update(cx, |agents, cx| agents.drop_task(&other, cx));
    run.purged(cx);
    assert_eq!(run.kept(cx), (0, 0));
    assert!(!run.profile.join(kept::FOLDER).exists());
}

/// A removal the assistant couldn't do removes nothing of ilo's: the folder and the id stay,
/// counted as not removed, and Remove All tries again. Remove All leaves a task still at work its
/// own, and ends a finished one's run before its conversation goes.
#[cfg(unix)]
#[gpui::test]
fn a_removal_that_failed_stays_listed_and_remove_all_tries_again(cx: &mut TestAppContext) {
    let run = Run::new("stuck", cx);
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let id = run.id(&agent, cx);
    run.finish(&agent, cx);
    let working = run.start(Cli::Claude, cx);
    run.ran(2);
    let at_work = run.id(&working, cx);
    let code = beside(&run.program, "code");
    std::fs::write(&code, "2").unwrap();
    (run.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    // The task at work keeps its own, and wasn't asked about.
    assert_eq!(run.purged(cx).len(), 3);
    assert_eq!(run.kept(cx), (2, 1));
    assert!(run.folder(&id).exists());
    assert!(run.folder(&at_work).exists());
    let saved = std::fs::read_to_string(run.profile.join(kept::FOLDER).join("index.json")).unwrap();
    assert!(
        saved.contains(id.as_str()),
        "the id went though nothing was removed"
    );
    // The finished task has no conversation to go on with while its removal is tried, so it
    // says so rather than take a reply that would go nowhere.
    assert_eq!(run.ilo_lines(&agent, cx).last(), Some(&IloKind::Gone));

    std::fs::remove_file(&code).unwrap();
    (run.agents).update(cx, |agents, cx| agents.remove_kept_tasks(cx));
    assert_eq!(run.purged(cx).len(), 6);
    assert_eq!(run.kept(cx), (1, 0));
    assert!(!run.folder(&id).exists());
    assert!(run.folder(&at_work).exists());
    // Its conversation was kept and is gone, which is what the task says: not that it kept none.
    assert_eq!(run.ilo_lines(&agent, cx).last(), Some(&IloKind::Gone));
}

/// A launch after a run that kept nothing looks for the file of ids and does no more: no process
/// is started, nothing is spawned, nothing is made in the profile, and no one is told of a change.
#[cfg(unix)]
#[gpui::test]
fn a_launch_with_no_file_of_ids_starts_and_spawns_nothing(cx: &mut TestAppContext) {
    let run = Run::new("no-file", cx);
    // What starting the agents spawned is over, so anything that runs after is the launch's.
    cx.run_until_parked();
    let before = run.files();
    let told = Rc::new(std::cell::Cell::new(0));
    let _told = cx.update(|cx| {
        let told = told.clone();
        cx.observe(&run.agents, move |_, _| told.set(told.get() + 1))
    });
    (run.agents).update(cx, |agents, cx| agents.take_old_kept(cx));
    assert!(!cx.executor().tick(), "the launch spawned something");
    assert_eq!(told.get(), 0);
    assert!(run.purged(cx).is_empty());
    assert!(!beside(&run.program, "args.1").exists());
    assert_eq!(run.files(), before);
    assert!(!run.profile.join(kept::FOLDER).exists());
    assert_eq!(run.kept(cx), (0, 0));
}

/// The id goes to the assistant's command line and ilo's file of ids, and nowhere else: not in
/// anything an agent or an outside client is told, nor its task's instructions, MCP config or
/// messages, nor the agents' settings, the conversation the user sees, the activity log or
/// Library › Agents' record of the task.
#[cfg(unix)]
#[gpui::test]
fn the_id_is_in_nothing_an_agent_or_a_client_is_told(cx: &mut TestAppContext) {
    let run = Run::new("id", cx);
    let agent = run.start(Cli::Claude, cx);
    let (args, first) = run.ran(1);
    let id = run.id(&agent, cx);
    let names = |text: &str| text.contains(id.as_str());
    // On its command line it's the one argument after its flag, and no other argument has it:
    // not the instructions, the tools or the MCP config.
    assert_eq!(args.iter().filter(|arg| names(arg)).count(), 1);
    assert!(!names(&first));

    // The task's own tools, and an outside client's, each asked what they can say of tabs and
    // tasks; and no tool takes up, lists or removes a kept task.
    (run.agents).update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    for client in [agent.as_str(), "claude-code"] {
        let listed = tools::list(client);
        assert!(!names(&listed.to_string()));
        for tool in listed.as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            for word in ["resume", "kept", "pick", "conversation", "session", "purge"] {
                assert!(!name.contains(word), "{client} has {name}");
            }
        }
        for tool in ["list_tabs", "list_split", "snapshot", "read_page"] {
            if let Ok(answer) = call(&run.agents, client, tool, json!({}), cx).try_recv() {
                assert!(!names(&answer.text), "{tool} told {client} the id");
            }
        }
    }
    assert!(!names(tools::TASK_INSTRUCTIONS));

    // Ended, replied to and ended again: what the user sees, the log and the record have none.
    run.finish(&agent, cx);
    run.said(&agent, Out::Gone(String::new()), cx);
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And for Sunday?", cx));
    let (_, second) = run.ran(2);
    assert!(!names(&second));
    run.said(&agent, Out::Said("Two at 10:35.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    assert!(!names(&format!("{:?}", run.talk(&agent, cx))));
    assert!(!names(&format!("{:?}", log(&run.agents, &agent, cx))));
    let kept = (run.agents).read_with(cx, |agents, _| format!("{:?}", agents.tasks.kept));
    assert!(!names(&kept), "{kept}");
    let past = (run.agents).update(cx, |agents, cx| agents.take_ended(cx));
    assert!(!past.is_empty());
    assert!(!names(&format!("{past:?}")));
    // Of the files ilo wrote, only its file of ids has it.
    let index = run.profile.join(kept::FOLDER).join("index.json");
    let mut held = 0;
    for (path, text) in run.files() {
        match path == index {
            true => held += text.matches(id.as_str()).count(),
            false => assert!(!names(&text), "{} names the id", path.display()),
        }
    }
    assert_eq!(held, 1);
}

/// ilo's lines are the app's alone: what an assistant says is its own message however it reads,
/// with no button, and a page's words reach a conversation only through one.
#[cfg(unix)]
#[gpui::test]
fn an_assistants_words_never_make_a_line_of_ilos(cx: &mut TestAppContext) {
    let run = Run::new("words", cx);
    (run.agents).update(cx, |agents, cx| agents.set_keeps_tasks(false, cx));
    let agent = run.start(Cli::Claude, cx);
    run.ran(1);
    let imitation = "ilo · Claude no longer has this conversation. [Start Again]";
    run.said(&agent, Out::Said(imitation.into()), cx);
    let talk = run.talk(&agent, cx);
    assert!(
        (talk.lines.iter())
            .any(|line| matches!(line, Line::Said(said) if said.as_ref() == imitation))
    );
    assert_eq!(run.ilo_lines(&agent, cx), []);
    // Nor does a reply to a task still at work take it up, or its access change.
    (run.agents).update(cx, |agents, cx| {
        agents.reply(&agent, "Resume at Full access", cx)
    });
    assert_eq!(run.ilo_lines(&agent, cx), []);
}
