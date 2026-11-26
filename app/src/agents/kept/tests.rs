//! What a run keeps, what Claude Code is started with for it, the access a task comes back at, and
//! how a kept conversation is removed: on a stand-in for the assistant the tests write themselves,
//! never the user's own. Every id and path is made up.

use std::path::{Path, PathBuf};

use super::*;
use crate::agents::runner::{self, Launch, Server};

/// When a task of these tests started, in seconds since 1970.
const STARTED: u64 = 1_794_000_000;

/// A folder of the test's own, as a profile's.
fn profile(name: &str) -> PathBuf {
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ilo-kept-{}-{name}-{made}", std::process::id()));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// A stand-in for the assistant in `dir`, which writes what it's run with beside itself and ends
/// with `status`.
#[cfg(unix)]
fn stand_in(dir: &Path, status: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("claude");
    let script = format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.ran\"\nexit {status}\n");
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Another test's thread that forked while the script was open for writing holds it until its
    // child runs, and it won't run meanwhile ("Text file busy"): run it until it does.
    for _ in 0..200 {
        match crate::child::command(&program).status() {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(Duration::from_millis(10))
            }
            ran => {
                ran.expect("the stand-in didn't run");
                break;
            }
        }
    }
    std::fs::remove_file(ran(&program)).unwrap();
    program
}

/// Where the stand-in writes what it was run with.
fn ran(program: &Path) -> PathBuf {
    PathBuf::from(format!("{}.ran", program.display()))
}

fn launch() -> Launch {
    Launch {
        cli: Cli::Claude,
        program: PathBuf::from("/made/up/bin/claude"),
        path: None,
        dir: PathBuf::from("/made/up/Kept tasks/one"),
        config: PathBuf::from("/made/up/Tasks/task-1.json"),
        server: Server {
            command: "/app/mcp-bridge".into(),
            args: vec!["/profile/agents.sock".into()],
            token: "secret".into(),
        },
        model: None,
        effort: None,
        instructions: "Work in the browser.".into(),
        tools: vec!["open_tab".into(), "finish".into()],
        helpers: false,
        wall: None,
    }
}

#[test]
fn the_table_has_a_row_for_each_assistant() {
    assert_eq!(keeps(Cli::Claude), Keeps::Always);
    assert_eq!(keeps(Cli::Codex), Keeps::Never);
    assert!(matches!(decide(Cli::Claude, false), Keep::New(_)));
    // Codex keeps nothing, and a task with files keeps nothing, whose assistant it is.
    assert_eq!(decide(Cli::Codex, false), Keep::Nothing);
    for cli in [Cli::Claude, Cli::Codex] {
        assert_eq!(decide(cli, true), Keep::Nothing);
    }
}

#[test]
fn a_kept_run_takes_its_id_where_a_run_that_keeps_nothing_says_so() {
    let nothing = runner::claude_args(&launch());
    assert!(nothing.iter().any(|arg| arg == KEEP_NOTHING));
    assert_eq!(claude_args(nothing.clone(), &Keep::Nothing), nothing);
    let id = Id::new();
    for (keep, flag, other) in [
        (Keep::New(id.clone()), "--session-id", "--resume"),
        (Keep::Resume(id.clone()), "--resume", "--session-id"),
    ] {
        let args = claude_args(nothing.clone(), &keep);
        let at = args.iter().position(|arg| arg == flag).expect(flag);
        assert_eq!(args[at + 1], id.as_str());
        assert!(!args.iter().any(|arg| arg == KEEP_NOTHING || arg == other));
        // Every other argument is there again, in its place: a run that goes on with a
        // conversation has its tools, permissions and settings from this run alone.
        let mut rest = args.clone();
        rest.splice(at..=at + 1, [KEEP_NOTHING.to_string()]);
        assert_eq!(rest, nothing);
    }
}

#[test]
fn a_task_comes_back_at_work_on_pages_and_never_above_where_it_was() {
    assert_eq!(comes_back(Tier::Full), Tier::Work);
    assert_eq!(comes_back(Tier::Work), Tier::Work);
    assert_eq!(comes_back(Tier::Look), Tier::Look);
}

#[test]
fn an_id_is_a_uuid_that_nothing_prints() {
    let id = Id::new();
    assert_ne!(id, Id::new());
    assert_eq!(Id::parse(id.as_str()), Some(id.clone()));
    assert_eq!(id.as_str().as_bytes()[14], b'4', "not a version 4 UUID");
    let printed = format!("{id:?} {:?}", Keep::Resume(id.clone()));
    assert!(!printed.contains(id.as_str()), "{printed}");
    // What a run says on its way out may name it, and the log's line doesn't.
    let said = format!("No conversation found with session ID: {}", id.as_str());
    assert!(gone(&said));
    assert!(!without(&said, &id).contains(id.as_str()));
    for other in [
        "",
        "..",
        "../../elsewhere",
        "/made/up",
        "0F3C1E9A-5B7D-4C2E-8A1F-6D4B2C9E7A10",
        "0f3c1e9a-5b7d-4c2e-8a1f-6d4b2c9e7a1",
        "0f3c1e9a/5b7d-4c2e-8a1f-6d4b2c9e7a10",
    ] {
        assert_eq!(Id::parse(other), None, "{other:?} read as an id");
    }
}

#[test]
fn the_file_of_ids_holds_each_id_and_its_assistant_and_nothing_else() {
    let root = profile("file").join(FOLDER);
    let (mut kept, id) = (Kept::default(), Id::new());
    kept.hold("task-1", Cli::Claude, id.clone(), "Default", STARTED);
    // Deleted from the Archive: its conversation is still to be removed.
    kept.of_mut("task-1").unwrap().past = None;
    kept.write(&root);
    let saved: Value = serde_json::from_slice(&std::fs::read(root.join(INDEX)).unwrap()).unwrap();
    assert_eq!(saved, json!([{ "id": id.as_str(), "cli": "claude" }]));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.join(INDEX))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // A run after reads them as ones whose tasks went, and takes no id that isn't one.
    let other = json!([
        { "id": id.as_str(), "cli": "claude" },
        { "id": "../../elsewhere", "cli": "claude" },
        { "id": Id::new().as_str(), "cli": "someone" },
    ]);
    std::fs::write(root.join(INDEX), other.to_string()).unwrap();
    let mut after = Kept::default();
    assert!(after.read(&root));
    assert_eq!(after.count(), 1);
    assert_eq!(after.of("task-1"), None);
    assert_eq!(
        after.removing(|record| record.task.is_none()),
        [(id.clone(), Cli::Claude)]
    );
    // With none left, the file and the folder go.
    after.removed(&id, true);
    after.write(&root);
    assert!(!root.exists());
    // And with no file there's nothing to take.
    assert!(!after.read(&root));
    assert_eq!(after.count(), 0);
}

#[cfg(unix)]
#[test]
fn removing_asks_the_assistant_for_the_tasks_folder_alone_then_removes_it() {
    let dir = profile("remove");
    let (root, id) = (dir.join(FOLDER), Id::new());
    let program = stand_in(&dir, 0);
    std::fs::create_dir_all(folder(&root, &id)).unwrap();
    assert!(remove(&program, None, &root, &id));
    let asked = std::fs::read_to_string(ran(&program)).unwrap();
    let folder = folder(&root, &id);
    assert_eq!(
        asked.lines().collect::<Vec<_>>(),
        ["purge", folder.to_str().unwrap(), "--yes"]
    );
    assert!(!folder.exists());
}

#[cfg(unix)]
#[test]
fn an_assistant_with_nothing_for_the_folder_counts_as_removed() {
    let dir = profile("nothing");
    let (root, id) = (dir.join(FOLDER), Id::new());
    let program = stand_in(&dir, 1);
    // Its folder went missing meanwhile: the assistant is still asked, by the folder's path.
    assert!(remove(&program, None, &root, &id));
    assert!(ran(&program).exists());
    assert!(!folder(&root, &id).exists());
}

#[cfg(unix)]
#[test]
fn a_removal_that_failed_removes_nothing_of_ilos_either() {
    let dir = profile("failed");
    let (root, id) = (dir.join(FOLDER), Id::new());
    std::fs::create_dir_all(folder(&root, &id)).unwrap();
    // The assistant crashed, or couldn't.
    let program = stand_in(&dir, 2);
    assert!(!remove(&program, None, &root, &id));
    assert!(folder(&root, &id).exists());
    // The assistant isn't there.
    assert!(!remove(&dir.join("no-such-claude"), None, &root, &id));
    assert!(folder(&root, &id).exists());
    // The id stays held, marked, and goes once a later try removes it.
    let mut kept = Kept::default();
    kept.hold("task-1", Cli::Claude, id.clone(), "Default", STARTED);
    assert!(!kept.had("task-1"));
    assert_eq!(kept.removing(|_| true), [(id.clone(), Cli::Claude)]);
    assert_eq!(kept.removing(|_| true), [], "removed twice at once");
    // Its task is left with none, having had one, until the task itself goes.
    assert_eq!(kept.of("task-1"), None);
    assert!(kept.had("task-1") && !kept.had("task-2"));
    kept.removed(&id, false);
    assert_eq!((kept.count(), kept.stuck()), (1, 1));
    assert_eq!(kept.removing(|_| true), [(id.clone(), Cli::Claude)]);
    kept.removed(&id, true);
    assert_eq!((kept.count(), kept.stuck()), (0, 0));
    kept.forget("task-1");
    assert!(!kept.had("task-1"));
}

#[cfg(unix)]
#[test]
fn a_folder_that_points_elsewhere_removes_nothing() {
    let dir = profile("elsewhere");
    let (root, id) = (dir.join(FOLDER), Id::new());
    let program = stand_in(&dir, 0);
    // Someone put a link where the task's folder would be, to a folder that isn't ilo's.
    let elsewhere = dir.join("Projects");
    std::fs::create_dir_all(elsewhere.join("mine")).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&elsewhere, folder(&root, &id)).unwrap();
    assert!(!remove(&program, None, &root, &id));
    assert!(!ran(&program).exists(), "the assistant was asked");
    assert!(elsewhere.join("mine").exists());
    // And the folder of all of them, linked elsewhere, is still the only place one is in.
    let linked = dir.join("Linked");
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    assert!(remove(&program, None, &linked, &id));
    let asked = std::fs::read_to_string(ran(&program)).unwrap();
    let named = PathBuf::from(asked.lines().nth(1).unwrap());
    assert_eq!(named.parent(), Some(elsewhere.as_path()));
    assert!(elsewhere.join("mine").exists());
}

/// The assistant's command that removes every project's data is never ilo's to run: no source
/// file of the agents' names its flag.
#[test]
fn no_code_asks_an_assistant_to_remove_everything() {
    let flag = ["--", "all"].concat();
    let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agents")];
    let mut read = 0;
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let named = source
                .match_indices(&flag)
                // `--allowedTools` and its like are other flags.
                .any(|(at, _)| !source[at + flag.len()..].starts_with(char::is_alphanumeric));
            assert!(!named, "{} names {flag}", path.display());
            read += 1;
        }
    }
    assert!(read > 20, "read only {read} files");
}

/// A task has its row from its start, with no word of what was asked: if the browser goes without
/// writing its end, the run after finds a task that failed at the time it started, and the name
/// its agent gave it meanwhile.
#[test]
fn a_task_under_way_has_a_row_that_reads_as_one_that_broke_off() {
    let root = profile("under-way").join(FOLDER);
    let (mut kept, id) = (Kept::default(), Id::new());
    kept.hold("task-1", Cli::Claude, id.clone(), "Profile 2", STARTED);
    kept.write(&root);
    let saved: Value = serde_json::from_slice(&std::fs::read(root.join(INDEX)).unwrap()).unwrap();
    let card = kept.of("task-1").unwrap().card.clone();
    let row = json!({
        "id": id.as_str(), "cli": "claude", "title": "", "state": "failed",
        "ended": STARTED, "profile": "Profile 2", "card": card.as_str(),
    });
    assert_eq!(saved, json!([row]));
    kept.of_mut("task-1").unwrap().past.as_mut().unwrap().title = "Lisbon seats".into();
    kept.write(&root);
    let mut after = Kept::default();
    assert!(after.read(&root));
    let under_way = Past::under_way("Lisbon seats".into(), STARTED);
    assert_eq!(after.left(), [(id.clone(), Cli::Claude, under_way)]);
    // It's a row as any other's, so nothing takes it for one to remove.
    assert_eq!(after.removing(|record| record.past.is_none()), []);
}

/// Of a task that has ended the file says its title, how and when it ended, its profile and its
/// card's id, and no more; a run after takes them back as they were written, and of a file someone changed,
/// only what has their shape: an end it doesn't know leaves an id alone, a profile with a path
/// in it names none, and a title is one line of a title's length.
#[test]
fn the_file_says_of_an_ended_task_its_title_its_end_and_its_profile() {
    let root = profile("past").join(FOLDER);
    let (mut kept, id) = (Kept::default(), Id::new());
    kept.hold("task-1", Cli::Claude, id.clone(), "Profile 2", STARTED);
    kept.of_mut("task-1").unwrap().past = Some(Past {
        title: "Lisbon seats".into(),
        end: End::Limit,
        at: 1_795_000_000,
    });
    kept.write(&root);
    let saved: Value = serde_json::from_slice(&std::fs::read(root.join(INDEX)).unwrap()).unwrap();
    let card = kept.of("task-1").unwrap().card.clone();
    let row = json!({
        "id": id.as_str(), "cli": "claude", "title": "Lisbon seats", "state": "limit",
        "ended": 1_795_000_000u64, "profile": "Profile 2", "card": card.as_str(),
    });
    assert_eq!(saved, json!([row]));
    let mut after = Kept::default();
    assert!(after.read(&root));
    let left = after.left();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].2, kept.of("task-1").unwrap().past.clone().unwrap());
    after.give(&id, "task-9");
    assert_eq!(after.of("task-9").unwrap().profile, "Profile 2");
    assert_eq!(after.of("task-9").unwrap().card, card);
    assert_eq!(after.listed().collect::<Vec<_>>(), ["task-9"]);
    assert!(after.left().is_empty());

    // Every end has its own word in the file and reads back as itself.
    for end in End::ALL {
        assert_eq!(
            End::ALL.into_iter().find(|each| each.id() == end.id()),
            Some(end)
        );
    }
    let (unknown, elsewhere, long) = (Id::new(), Id::new(), Id::new());
    let changed = json!([
        { "id": unknown.as_str(), "cli": "claude", "title": "A", "state": "paused", "ended": 5 },
        { "id": elsewhere.as_str(), "cli": "claude", "title": "B", "state": "done", "ended": 6,
          "profile": "../Default" },
        { "id": long.as_str(), "cli": "claude", "title": format!("C\n{}", "c".repeat(200)),
          "state": "done", "ended": 7, "profile": "Default" },
    ]);
    std::fs::write(root.join(INDEX), changed.to_string()).unwrap();
    let mut after = Kept::default();
    assert!(after.read(&root));
    let left = after.left();
    // The newest to end first, and the one with no end the file knows isn't among them.
    assert_eq!(
        left.iter().map(|(id, ..)| id.clone()).collect::<Vec<_>>(),
        [long.clone(), elsewhere.clone()]
    );
    assert_eq!(left[0].2.title.chars().count(), 80);
    assert!(!left[0].2.title.contains('\n'));
    after.give(&elsewhere, "task-1");
    assert_eq!(after.of("task-1").unwrap().profile, "");
}

/// A task deleted from the Archive, one whose conversation has gone and one being removed have
/// an id alone in the file, as a conversation held only to be removed has.
#[test]
fn a_task_that_takes_no_reply_has_an_id_alone_in_the_file() {
    let root = profile("unlisted").join(FOLDER);
    let past = || {
        Some(Past {
            title: "Lisbon seats".into(),
            end: End::Done,
            at: 1_795_000_000,
        })
    };
    let rows = |kept: &Kept| -> Value {
        kept.write(&root);
        serde_json::from_slice(&std::fs::read(root.join(INDEX)).unwrap()).unwrap()
    };
    let (mut kept, id) = (Kept::default(), Id::new());
    kept.hold("task-1", Cli::Claude, id.clone(), "Default", STARTED);
    kept.of_mut("task-1").unwrap().past = past();
    assert_eq!(rows(&kept)[0]["title"], json!("Lisbon seats"));
    let alone = json!([{ "id": id.as_str(), "cli": "claude" }]);
    // Deleted from the Archive.
    kept.of_mut("task-1").unwrap().past = None;
    assert_eq!(rows(&kept), alone);
    // Its assistant no longer has it.
    kept.of_mut("task-1").unwrap().past = past();
    kept.of_mut("task-1").unwrap().gone = true;
    assert_eq!(rows(&kept), alone);
    assert_eq!(kept.listed().count(), 0);
    // Being removed.
    kept.of_mut("task-1").unwrap().gone = false;
    assert_eq!(kept.removing(|_| true).len(), 1);
    assert_eq!(rows(&kept), alone);
    assert_eq!(kept.listed().count(), 0);
}

/// A task's row carries the names of the places its runs left something running in, and nothing
/// else of them: read again, it holds each, and drops what isn't a place's name. A sweep of one
/// that came clean takes it off the row that carried it, once.
#[test]
fn a_row_carries_the_places_its_runs_left_by_name_alone() {
    const PLACE: &str = "0123456789abcdef";
    let root = profile("left").join(FOLDER);
    let mut kept = Kept::default();
    kept.hold("task-1", Cli::Claude, Id::new(), "Default", 7);
    kept.of_mut("task-1").unwrap().left.push(PLACE.into());
    kept.write(&root);
    let saved = std::fs::read(root.join(INDEX)).unwrap();
    let mut saved: Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(saved[0]["left"], json!([PLACE]));
    // As anything else could have written it.
    let other = ["../../tmp", "0123", "ABCDEF0123456789"];
    saved[0]["left"] = json!([PLACE, other[0], other[1], other[2], 5]);
    std::fs::write(root.join(INDEX), saved.to_string()).unwrap();
    let mut again = Kept::default();
    assert!(again.read(&root));
    for name in other {
        assert_eq!(again.came_clean(name), None, "{name}");
    }
    assert_eq!(again.came_clean("fedcba9876543210"), None);
    assert_eq!(again.came_clean(PLACE), Some(None));
    assert_eq!(again.came_clean(PLACE), None);
}
