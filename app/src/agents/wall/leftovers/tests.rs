//! The sweep and the cleanup over a made-up system: processes are numbers in a table, and nothing
//! is ever signalled.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CString;

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Pids,
    Holds(u32),
    Kill(u32),
    Group(u32),
}

/// The processes there are, each inside the wall or not, and what each starts as it's killed.
#[derive(Default)]
struct Fake {
    alive: RefCell<BTreeMap<u32, bool>>,
    /// Killing the key starts the value, inside the wall.
    starts: BTreeMap<u32, u32>,
    /// Every kill starts one more, inside: a chain that never ends.
    endless: bool,
    missing: bool,
    unlisted: bool,
    calls: RefCell<Vec<Call>>,
    asked: RefCell<Vec<(CString, CString)>>,
}

impl Fake {
    fn with(alive: &[(u32, bool)]) -> Fake {
        Fake {
            alive: RefCell::new(alive.iter().copied().collect()),
            ..Fake::default()
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    fn killed(&self) -> Vec<u32> {
        let calls = self.calls.borrow();
        let kills = calls.iter().filter_map(|call| match call {
            Call::Kill(pid) => Some(*pid),
            _ => None,
        });
        kills.collect()
    }
}

impl System for Fake {
    fn pids(&self) -> io::Result<Vec<u32>> {
        self.calls.borrow_mut().push(Call::Pids);
        if self.unlisted {
            return Err(io::Error::other("no list"));
        }
        Ok(self.alive.borrow().keys().copied().collect())
    }

    fn holds(&self, pid: u32, mark: &CStr, control: &CStr) -> Option<bool> {
        self.calls.borrow_mut().push(Call::Holds(pid));
        self.asked.borrow_mut().push((mark.into(), control.into()));
        if self.missing {
            return None;
        }
        Some(self.alive.borrow().get(&pid).copied().unwrap_or(false))
    }

    fn kill(&self, pid: u32) {
        self.calls.borrow_mut().push(Call::Kill(pid));
        let mut alive = self.alive.borrow_mut();
        alive.remove(&pid);
        if let Some(started) = self.starts.get(&pid) {
            alive.insert(*started, true);
        }
        if self.endless {
            alive.insert(pid + 1, true);
        }
    }

    fn end_group(&self, leader: u32) {
        self.calls.borrow_mut().push(Call::Group(leader));
    }
}

#[test]
fn a_sweep_ends_each_one_inside_as_it_is_found_and_walks_until_none() {
    let mut fake = Fake::with(&[(5, true), (6, true), (7, false)]);
    // 6 starts 8 as it's killed, which the next walk finds.
    fake.starts.insert(6, 8);
    let swept = sweep_by(&fake, &Mark::new());
    assert_eq!(swept, Ok(Swept { ended: 3, walks: 3 }));
    assert_eq!(fake.killed(), [5, 6, 8]);
    // Each kill follows its own question at once.
    let calls = fake.calls();
    for (at, call) in calls.iter().enumerate() {
        if let Call::Kill(pid) = call {
            assert_eq!(calls[at - 1], Call::Holds(*pid), "{calls:?}");
        }
    }
    // Each walk lists the processes anew.
    assert_eq!(calls.iter().filter(|call| **call == Call::Pids).count(), 3);
    assert!(fake.alive.borrow().contains_key(&7));
}

#[test]
fn a_sweep_asks_with_its_mark_and_the_control() {
    let fake = Fake::with(&[(5, false)]);
    let mark = Mark::new();
    sweep_by(&fake, &mark).unwrap();
    assert_eq!(
        *fake.asked.borrow(),
        [(mark.as_cstr().into(), CONTROL.into())]
    );
}

#[test]
fn a_sweep_that_still_finds_one_at_the_cap_says_so() {
    let mut fake = Fake::with(&[(5, true)]);
    fake.endless = true;
    assert_eq!(sweep_by(&fake, &Mark::new()), Err(SweepFailed::Cap));
    let walks = fake
        .calls()
        .iter()
        .filter(|call| **call == Call::Pids)
        .count();
    assert_eq!(walks, CAP);
}

#[test]
fn a_sweep_never_asks_about_or_ends_ilo_0_or_1() {
    let own = std::process::id();
    let fake = Fake::with(&[(0, true), (1, true), (own, true)]);
    assert_eq!(
        sweep_by(&fake, &Mark::new()),
        Ok(Swept { ended: 0, walks: 1 })
    );
    assert_eq!(fake.calls(), [Call::Pids]);
}

#[test]
fn a_system_that_cant_be_asked_fails_the_sweep_at_the_first_process() {
    let mut fake = Fake::with(&[(5, true), (6, true)]);
    fake.missing = true;
    assert_eq!(sweep_by(&fake, &Mark::new()), Err(SweepFailed::Missing));
    assert_eq!(fake.calls(), [Call::Pids, Call::Holds(5)]);
    let mut fake = Fake::with(&[(5, true)]);
    fake.unlisted = true;
    assert_eq!(sweep_by(&fake, &Mark::new()), Err(SweepFailed::Unlisted));
    assert!(fake.killed().is_empty());
}

#[test]
fn a_start_the_system_doesnt_place_inside_its_wall_ends_its_group_and_starts_nothing() {
    let place = Place {
        tmp: PathBuf::from("/nonexistent/Tasks/.tmp/0123456789abcdef"),
        mark: Mark::new(),
    };
    let inside = Fake::with(&[(40, true)]);
    assert_eq!(control_by(&inside, Some(&place), 40), Ok(()));
    assert_eq!(inside.calls(), [Call::Holds(40)]);
    let outside = Fake::with(&[(40, false)]);
    let refused = WallRefused::Failed(OUTSIDE.into());
    assert_eq!(control_by(&outside, Some(&place), 40), Err(refused.clone()));
    assert_eq!(outside.calls(), [Call::Holds(40), Call::Group(40)]);
    let mut missing = Fake::with(&[(40, true)]);
    missing.missing = true;
    assert_eq!(control_by(&missing, Some(&place), 40), Err(refused.clone()));
    // A wall with no mark can't be told from none.
    let unmarked = Fake::with(&[(40, true)]);
    assert_eq!(control_by(&unmarked, None, 40), Err(refused));
    assert_eq!(unmarked.calls(), [Call::Group(40)]);
}

#[test]
fn a_mark_has_its_shape_never_the_controls_and_is_never_printed() {
    let mark = Mark::new();
    let text = mark.as_cstr().to_str().unwrap();
    let digits = text.strip_prefix("ilo.mark.").unwrap();
    assert_eq!(digits.len(), 32);
    assert!(
        digits
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_ne!(mark.as_cstr(), CONTROL);
    assert!(Mark::parse(CONTROL.to_str().unwrap()).is_none());
    assert_ne!(Mark::new().as_cstr(), mark.as_cstr());
    let printed = format!("{mark:?}");
    assert!(!printed.contains(digits), "{printed}");
    for wrong in [
        "ilo.mark.",
        &format!("{text}0"),
        &text.to_uppercase(),
        &text.replace("ilo.mark.", "ilo.mark.x")[..text.len()],
    ] {
        assert!(Mark::parse(wrong).is_none(), "{wrong}");
    }
}

/// A folder of the test's own, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ilo-leftovers-{}-{name}", std::process::id()));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_place_is_its_mark_and_its_folder_only_its_user_reads() {
    let tasks = scratch("place");
    let root = tasks.join(ROOT);
    let place = Place::make(&root).unwrap();
    let mark_file = place.tmp().with_extension("mark");
    assert_eq!(
        std::fs::read(&mark_file).unwrap(),
        place.mark().as_cstr().to_bytes()
    );
    assert!(place.tmp().is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&mark_file), 0o600);
        assert_eq!(mode(place.tmp()), 0o700);
        assert_eq!(mode(&root), 0o700);
    }
    let name = place
        .tmp()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let expected = Left {
        name,
        mark: true,
        folder: true,
    };
    assert_eq!(place.name(), Some(expected.name.as_str()));
    assert!(is_name(&expected.name) && !is_name("0123") && !is_name(&"A".repeat(16)));
    assert_eq!(left_in(&root), [expected]);
    assert!(!format!("{place:?}").contains("ilo-leftovers"));
    std::fs::remove_dir_all(&tasks).unwrap();
}

#[test]
fn the_launchs_cleanup_sweeps_each_mark_and_removes_what_started_nothing() {
    let tasks = scratch("cleanup");
    let root = tasks.join(ROOT);
    let swept = Place::make(&root).unwrap();
    let unreadable = Place::make(&root).unwrap();
    std::fs::write(unreadable.tmp().with_extension("mark"), "not a mark").unwrap();
    let no_mark = Place::make(&root).unwrap();
    std::fs::remove_file(no_mark.tmp().with_extension("mark")).unwrap();
    let no_folder = Place::make(&root).unwrap();
    std::fs::remove_dir(no_folder.tmp()).unwrap();
    std::fs::write(swept.tmp().join("left.txt"), "a file the task wrote").unwrap();
    // Not a place's name, so not the cleanup's.
    std::fs::create_dir(root.join("0123")).unwrap();
    let left = left_in(&root);
    assert_eq!(left.len(), 4);
    // A place made after the list, as a task's own would be.
    let later = Place::make(&root).unwrap();
    let fake = Fake::with(&[(5, true), (6, false)]);
    let mut cleaned = clean_up_by(&fake, &root, &left);
    // Each place whose mark was swept is named clean, with what its sweep ended.
    cleaned.clean.sort();
    let name = |place: &Place| place.name().unwrap().to_string();
    let mut clean = vec![name(&swept), name(&no_folder)];
    clean.sort();
    assert_eq!(
        (cleaned.clean.iter().map(|(name, _)| name.clone())).collect::<Vec<_>>(),
        clean
    );
    assert_eq!(
        cleaned.clean.iter().map(|(_, ended)| ended).sum::<usize>(),
        1
    );
    cleaned.clean.clear();
    assert_eq!(
        cleaned,
        Cleaned {
            ended: 1,
            removed: 4,
            kept: 0,
            clean: Vec::new(),
        }
    );
    // Both marks that could be read were swept, and between them they ended the one inside.
    assert_eq!(fake.killed(), [5]);
    let mut names: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let later = later
        .tmp()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut expected = vec!["0123".to_string(), format!("{later}.mark"), later];
    expected.sort();
    assert_eq!(names, expected);
    std::fs::remove_dir_all(&tasks).unwrap();
}

#[test]
fn a_place_whose_sweep_isnt_clean_is_kept_for_the_next_launch() {
    let tasks = scratch("kept");
    let root = tasks.join(ROOT);
    let place = Place::make(&root).unwrap();
    let left = left_in(&root);
    let mut fake = Fake::with(&[(5, true)]);
    fake.missing = true;
    let cleaned = clean_up_by(&fake, &root, &left);
    assert_eq!(
        cleaned,
        Cleaned {
            ended: 0,
            removed: 0,
            kept: 1,
            clean: Vec::new(),
        }
    );
    assert!(place.tmp().is_dir() && place.tmp().with_extension("mark").is_file());
    std::fs::remove_dir_all(&tasks).unwrap();
}

/// The sweep on the Mac, around stand-ins only: the system's shell started through ilo's launcher
/// under rules that allow a mark of the test's own. Nothing outside the test holds its marks, so
/// nothing else can be ended.
#[cfg(target_os = "macos")]
mod mac {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Child, Stdio};

    use super::*;
    use crate::agents::wall::launch::Rules;

    /// A task's rules as far as marks go: none of ilo's names but the wall's own.
    const MARKED: &str = "(version 1)(allow default)\
        (deny mach-lookup (global-name-prefix \"ilo.mark.\"))\
        (allow mach-lookup (global-name (param \"MARK\")))";

    /// The shell inside the wall marked `mark`, running `script`, and the first line it says.
    fn inside(mark: &Mark, script: &str) -> (Child, String) {
        let rules = Rules {
            text: MARKED.into(),
            params: vec![("MARK", PathBuf::from(mark.as_cstr().to_str().unwrap()))],
        };
        let wall = Wall::inside(&rules, &[]).unwrap();
        let mut command = wall.command(Path::new("/bin/sh"));
        command
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let (mut child, rising) = wall.start(&mut command).unwrap();
        assert_eq!(rising.expect("a walled start").risen(), Ok(()));
        let mut said = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut said)
            .unwrap();
        (child, said.trim().to_string())
    }

    fn held(pid: u32, mark: &Mark) -> bool {
        Os.holds(pid, mark.as_cstr(), CONTROL) == Some(true)
    }

    #[test]
    fn a_sweep_ends_what_left_the_group_and_nothing_in_another_wall() {
        let (mark, other) = (Mark::new(), Mark::new());
        // A job of its own group, which outlives the shell's group being ended.
        let (mut shell, job) = inside(&mark, "set -m; sleep 60 & echo $!; read line");
        let job: u32 = job.parse().unwrap();
        let (mut beside, _) = inside(&other, "echo up; read line");
        assert!(held(shell.id(), &mark) && held(job, &mark));
        assert!(!held(std::process::id(), &mark) && !held(beside.id(), &mark));
        // `set -m` gave the job a group of its own, so ending the shell's group leaves it running.
        let group = |pid: u32| unsafe { libc::getpgid(pid as libc::pid_t) };
        assert!(group(job) > 0 && group(job) != group(shell.id()));
        let swept = sweep(&mark).unwrap();
        assert!(swept.ended >= 2, "{swept:?}");
        assert_eq!(shell.wait().unwrap().signal(), Some(libc::SIGKILL));
        assert!(!held(job, &mark));
        assert_eq!(sweep(&mark), Ok(Swept { ended: 0, walks: 1 }));
        // The other wall's shell is still there.
        assert!(beside.try_wait().unwrap().is_none());
        _ = beside.kill();
        _ = beside.wait();
    }
}
