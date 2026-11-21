//! A hook's processes, killed together, on Windows (#3668): cmd.exe goes in a job object as soon
//! as it starts, and what it starts after that joins the job.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};

/// A started command's job.
pub struct Group(HANDLE);

/// Nothing to set before it starts: the job is made once it has.
pub fn prepare(_: &mut Command) {}

/// A job holding `child`, which what it starts from now on joins.
pub fn of(child: &Child) -> io::Result<Group> {
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(io::Error::last_os_error());
    }
    let group = Group(job);
    if unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(group)
}

impl Group {
    /// Kills every process in the job.
    pub fn kill(&self) {
        unsafe { TerminateJobObject(self.0, 1) };
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
