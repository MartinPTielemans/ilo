//! The process's memory in MB, as each OS counts it: on Windows its working set and its commit,
//! which is what ran out in #2069; elsewhere its resident set.

use serde_json::{Value, json};

#[cfg(windows)]
fn counters() -> windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    counters
}

#[cfg(not(target_os = "linux"))]
const MB: f64 = 1024. * 1024.;

/// What the process holds now.
#[cfg(windows)]
pub fn now() -> Value {
    let c = counters();
    json!({ "working_set_mb": c.WorkingSetSize as f64 / MB, "commit_mb": c.PagefileUsage as f64 / MB })
}

/// The most it has held.
#[cfg(windows)]
pub fn peak() -> Value {
    let c = counters();
    json!({
        "working_set_mb": c.PeakWorkingSetSize as f64 / MB,
        "commit_mb": c.PeakPagefileUsage as f64 / MB,
    })
}

#[cfg(target_os = "linux")]
fn status(field: &str) -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| rest.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.)
        / 1024.
}

#[cfg(target_os = "linux")]
pub fn now() -> Value {
    json!({ "resident_mb": status("VmRSS:") })
}

#[cfg(target_os = "linux")]
pub fn peak() -> Value {
    json!({ "resident_mb": status("VmHWM:") })
}

#[cfg(target_os = "macos")]
pub fn now() -> Value {
    // The Mac reports no current resident set without Mach calls; the peak stands in.
    peak()
}

#[cfg(target_os = "macos")]
pub fn peak() -> Value {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    // In bytes on the Mac.
    json!({ "resident_mb": usage.ru_maxrss as f64 / MB })
}
