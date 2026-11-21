//! A hook's processes, killed together, on the Mac and Linux (#3668): the shell runs in a process
//! group of its own, which everything it starts joins unless it leaves it.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

/// A started command's process group.
pub struct Group(libc::pid_t);

/// Sets `command` to start in a group of its own, whose id is its process's.
pub fn prepare(command: &mut Command) {
    command.process_group(0);
}

/// The group `child` leads, as `prepare` started it.
pub fn of(child: &Child) -> io::Result<Group> {
    let pid = libc::pid_t::try_from(child.id()).map_err(io::Error::other)?;
    Ok(Group(pid))
}

impl Group {
    /// Kills every process left in the group.
    pub fn kill(&self) {
        unsafe { libc::killpg(self.0, libc::SIGKILL) };
    }
}
