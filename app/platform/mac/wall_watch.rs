//! The system's own word on what a program ilo started did next (#4101): became another program,
//! by an exec, or ended first. A kernel queue of its own watches the one process and is asked
//! once, so nothing the process writes is taken for it, and nothing waits by a timer.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr::{null, null_mut};

/// What the process watched did next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// It became the program it was to start.
    Exec,
    /// It ended first: with this exit code, or with none if a signal ended it.
    Exit(Option<i32>),
}

/// A process watched, until it's asked what the process did. Dropped, it watches no more.
pub struct Watch {
    queue: OwnedFd,
}

/// Watches the child with the id `pid` from here on, or answers None if it has ended already: its
/// end is then its parent's to take.
pub fn watch(pid: u32) -> io::Result<Option<Watch>> {
    // SAFETY: kqueue takes nothing, and the descriptor it gives is owned from here.
    let queue = unsafe { libc::kqueue() };
    if queue < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a descriptor just made, which nothing else holds.
    let queue = unsafe { OwnedFd::from_raw_fd(queue) };
    let change = libc::kevent {
        ident: pid as usize,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD,
        fflags: libc::NOTE_EXEC | libc::NOTE_EXIT | libc::NOTE_EXITSTATUS,
        data: 0,
        udata: null_mut(),
    };
    // SAFETY: one change read from `change`, and no event written, as none is asked for.
    let added = unsafe { libc::kevent(queue.as_raw_fd(), &change, 1, null_mut(), 0, null()) };
    if added < 0 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            // The kernel finds no process once its exit is under way, so none is missed: either
            // the watch is on before the exit, or this says so.
            Some(libc::ESRCH) => Ok(None),
            // The change is made before a signal can interrupt the call.
            Some(libc::EINTR) => Ok(Some(Watch { queue })),
            _ => Err(error),
        };
    }
    Ok(Some(Watch { queue }))
}

impl Watch {
    /// Waits for what the process does next. An exec counts even if the program it became has
    /// ended since.
    pub fn end(self) -> io::Result<End> {
        // SAFETY: a kevent is plain numbers and a pointer nothing reads, so all zeros is one.
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: no change, and at most the one event written to `event`.
            let got =
                unsafe { libc::kevent(self.queue.as_raw_fd(), null(), 0, &mut event, 1, null()) };
            if got == 1 {
                break;
            }
            let error = io::Error::last_os_error();
            if got < 0 && error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if event.fflags & libc::NOTE_EXEC != 0 {
            return Ok(End::Exec);
        }
        let status = event.data as i32;
        Ok(End::Exit(
            libc::WIFEXITED(status).then(|| libc::WEXITSTATUS(status)),
        ))
    }

    /// Whether the process has done something since the watch began, without waiting for it.
    #[cfg(test)]
    pub fn pending(&self) -> bool {
        let now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: as in `end`, with a wait of no time.
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        unsafe { libc::kevent(self.queue.as_raw_fd(), null(), 0, &mut event, 1, &now) > 0 }
    }

    /// The queue's descriptor, for the test that it's closed with the watch.
    #[cfg(test)]
    pub fn descriptor(&self) -> i32 {
        self.queue.as_raw_fd()
    }
}
