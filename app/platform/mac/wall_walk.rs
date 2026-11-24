//! The Mac's two answers for ending what a walled task left running (#4101): which processes
//! there are, and whether one is inside a given wall. The system answers the second from the
//! process's own rules, so nothing the process says or does is taken for it. This file only
//! asks: what is walked and what is done with an answer are its caller's.
//!
//! A wall is known by its mark, a service name that its rules alone allow looking up and that
//! names no service, beside a control, a second such name that no wall's rules allow.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::io;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

/// The system's `sandbox_check`, which no header declares. It asks a process's own rules about
/// one operation, or with no operation whether the process has rules at all.
type Ask = unsafe extern "C" fn(libc::pid_t, *const c_char, c_int, ...) -> c_int;

const ALLOWED: c_int = 0;
const REFUSED: c_int = 1;
/// The filter that reads the question's one value as a service's global name.
const GLOBAL_NAME: c_int = 2;

struct Check {
    ask: Ask,
    /// Keeps a refused question out of the system's record of refusals. Its value is the
    /// system's own, so it's read from there.
    no_report: c_int,
}

/// The call and its flag by these names, among all the process has loaded, or None where the
/// system has one of them no more. Nothing is linked by name, so a system without them still
/// starts ilo.
fn find(call: &CStr, flag: &CStr) -> Option<Check> {
    // SAFETY: dlsym reads the two names, each ending in a zero byte, and answers null for a name
    // that isn't there.
    let (call, flag) = unsafe {
        (
            libc::dlsym(libc::RTLD_DEFAULT, call.as_ptr()),
            libc::dlsym(libc::RTLD_DEFAULT, flag.as_ptr()),
        )
    };
    if call.is_null() || flag.is_null() {
        return None;
    }
    // SAFETY: the names found are only ever the system's two: a C function of `Ask`'s shape, and
    // an int that is never written.
    unsafe {
        Some(Check {
            ask: std::mem::transmute::<*mut c_void, Ask>(call),
            no_report: *flag.cast::<c_int>(),
        })
    }
}

/// Looked up at the first question and kept: the answer is the same for the whole run.
fn system() -> Option<&'static Check> {
    static FOUND: OnceLock<Option<Check>> = OnceLock::new();
    FOUND
        .get_or_init(|| find(c"sandbox_check", c"SANDBOX_CHECK_NO_REPORT"))
        .as_ref()
}

impl Check {
    /// Whether the process has rules at all. One that is gone reads as having them.
    fn walled(&self, pid: libc::pid_t) -> bool {
        // SAFETY: with no operation the call reads nothing past these three. It answers 1 for a
        // process that has rules.
        unsafe { (self.ask)(pid, null(), 0) == 1 }
    }

    /// What the process's rules say to looking `name` up. A process with no rules is allowed
    /// everything, and one that is gone is refused everything.
    fn lookup(&self, pid: libc::pid_t, name: &CStr) -> c_int {
        let filter = GLOBAL_NAME | self.no_report;
        // SAFETY: the operation and the name end in a zero byte, and the name is the one value
        // this filter reads.
        unsafe { (self.ask)(pid, c"mach-lookup".as_ptr(), filter, name.as_ptr()) }
    }
}

/// Whether the process `pid` is inside the wall whose rules allow `mark`: it has rules, they
/// allow looking `mark` up and they refuse `control`. All three are asked, as each alone is also
/// true of a process in no such wall: one that is gone has rules, one with no rules is allowed
/// the mark, and so is one whose rules allow every name. An answer that is neither yes nor no
/// counts against the process being inside, so an error never names one. None where the system's
/// call is missing, and nothing can be asked.
pub fn holds(pid: u32, mark: &CStr, control: &CStr) -> Option<bool> {
    holds_by(system(), pid, mark, control)
}

fn holds_by(check: Option<&Check>, pid: u32, mark: &CStr, control: &CStr) -> Option<bool> {
    let check = check?;
    // Zero and a number past a pid's are no one process: to a signal they are a whole group.
    let Ok(pid @ 1..) = libc::pid_t::try_from(pid) else {
        return Some(false);
    };
    Some(
        check.walled(pid)
            && check.lookup(pid, mark) == ALLOWED
            && check.lookup(pid, control) == REFUSED,
    )
}

/// Every process there is now, by its id. One started since isn't in it, so a single list is
/// never the whole answer for a caller that ends what it finds.
pub fn pids() -> io::Result<Vec<u32>> {
    let mut list: Vec<libc::pid_t> = Vec::new();
    loop {
        // SAFETY: with no buffer nothing is written: the answer is how many ids a list needs
        // room for, which is some more than there are processes.
        let room = unsafe { libc::proc_listallpids(null_mut(), 0) };
        if room <= 0 {
            return Err(io::Error::last_os_error());
        }
        list.resize(room as usize, 0);
        let bytes = c_int::try_from(size_of_val(list.as_slice())).unwrap_or(c_int::MAX);
        // SAFETY: at most `bytes` are written, which is no more than `list` holds.
        let listed = unsafe { libc::proc_listallpids(list.as_mut_ptr().cast(), bytes) };
        if listed <= 0 {
            return Err(io::Error::last_os_error());
        }
        // A full list may have left some out, the processes started since the room was asked.
        if (listed as usize) < list.len() {
            list.truncate(listed as usize);
            break;
        }
    }
    // Zero is the kernel's own, and to a signal it's the caller's whole group.
    let pids = list.into_iter().filter(|pid| *pid > 0);
    Ok(pids.map(|pid| pid as u32).collect())
}

#[cfg(test)]
mod tests {
    //! Stand-ins only: the system's shell, started through ilo's launcher under rules the tests
    //! write. No name here is a service's, and nothing but a stand-in is ever ended.

    use std::ffi::CString;
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Stdio};

    use super::*;
    use crate::agents::wall::Wall;
    use crate::agents::wall::launch::Rules;

    /// Rules that allow everything, every name too.
    const OPEN: &str = "(version 1)(allow default)";

    /// A wall's rules as far as a mark goes: none of the tests' names but the wall's own.
    fn marked() -> String {
        format!(
            "{OPEN}(deny mach-lookup (global-name-prefix \"ilo.test.walk.\"))\
             (allow mach-lookup (global-name (param \"MARK\")))"
        )
    }

    /// A name of this run's, which no service has.
    fn name(of: &str) -> CString {
        CString::new(format!("ilo.test.walk.{}.{of}", std::process::id())).unwrap()
    }

    /// The shell inside a wall, waiting for its input to end. Ended with the test.
    struct StandIn(Child);

    impl StandIn {
        fn inside(rules: &str, mark: &CStr) -> StandIn {
            let rules = Rules {
                text: rules.into(),
                params: vec![("MARK", PathBuf::from(mark.to_str().unwrap()))],
            };
            let wall = Wall::inside(None, &rules, &[]).unwrap();
            let mut command = wall.command(Path::new("/bin/sh"));
            command
                .args(["-c", "echo up; read line"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let (mut child, rising) = wall.start(&mut command).unwrap();
            assert_eq!(rising.expect("a walled start").risen(), Ok(()));
            let mut said = String::new();
            let mut out = BufReader::new(child.stdout.take().unwrap());
            out.read_line(&mut said).unwrap();
            assert_eq!(said, "up\n");
            StandIn(child)
        }

        fn pid(&self) -> u32 {
            self.0.id()
        }
    }

    impl Drop for StandIn {
        fn drop(&mut self) {
            _ = self.0.kill();
            _ = self.0.wait();
        }
    }

    #[test]
    fn a_process_inside_the_marked_wall_holds_its_mark() {
        let (mark, control) = (name("held"), name("held-control"));
        let stand_in = StandIn::inside(&marked(), &mark);
        assert_eq!(holds(stand_in.pid(), &mark, &control), Some(true));
    }

    #[test]
    fn a_process_inside_another_wall_holds_its_own_mark_alone() {
        let (mark, other, control) = (name("one"), name("other"), name("one-control"));
        let stand_in = StandIn::inside(&marked(), &other);
        assert_eq!(holds(stand_in.pid(), &mark, &control), Some(false));
        assert_eq!(holds(stand_in.pid(), &other, &control), Some(true));
    }

    #[test]
    fn rules_that_allow_every_name_hold_no_mark() {
        let (mark, control) = (name("any"), name("any-control"));
        let stand_in = StandIn::inside(OPEN, &mark);
        let (check, pid) = (system().unwrap(), stand_in.pid() as libc::pid_t);
        // It has rules and they allow the mark: the control alone tells it from the wall's own.
        assert!(check.walled(pid));
        assert_eq!(check.lookup(pid, &mark), ALLOWED);
        assert_eq!(holds(stand_in.pid(), &mark, &control), Some(false));
    }

    #[test]
    fn a_process_in_no_wall_and_one_that_is_gone_hold_no_mark() {
        let (mark, control) = (name("none"), name("none-control"));
        assert_eq!(holds(std::process::id(), &mark, &control), Some(false));
        let mut gone = crate::child::command("/usr/bin/true").spawn().unwrap();
        gone.wait().unwrap();
        // It reads as having rules, which is why having them is never the answer by itself.
        assert!(system().unwrap().walled(gone.id() as libc::pid_t));
        assert_eq!(holds(gone.id(), &mark, &control), Some(false));
        for no_process in [0, u32::MAX] {
            assert_eq!(holds(no_process, &mark, &control), Some(false));
        }
    }

    #[test]
    fn a_system_without_the_call_or_its_flag_gives_no_answer() {
        let (call, flag) = (c"sandbox_check", c"SANDBOX_CHECK_NO_REPORT");
        let (mark, control) = (name("unasked"), name("unasked-control"));
        for (call, flag) in [(c"ilo_no_such_call", flag), (call, c"ILO_NO_SUCH_FLAG")] {
            let found = find(call, flag);
            assert!(found.is_none());
            let asked = holds_by(found.as_ref(), std::process::id(), &mark, &control);
            assert_eq!(asked, None);
        }
        assert!(find(call, flag).is_some());
    }

    #[test]
    fn the_list_has_this_process_and_one_it_started_and_no_zero() {
        let mark = name("listed");
        let stand_in = StandIn::inside(OPEN, &mark);
        let listed = pids().unwrap();
        assert!(listed.contains(&std::process::id()) && listed.contains(&stand_in.pid()));
        assert!(!listed.contains(&0));
    }
}
