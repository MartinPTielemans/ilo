//! #2138, not for merge: the Rust UI's live heap by where it was allocated. One allocation in about
//! every 64 KiB allocated is sampled with its stack; a sample stands for 64 KiB, or its own size when
//! bigger. Every 10 s the log gets the exact live total and the sampled sites holding the most, with
//! the Unix time, for perf's memory readings to be read against.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering::Relaxed};

pub struct Sampling;

const INTERVAL: usize = 64 * 1024;
const SHARDS: usize = 64;

static BYTES: AtomicIsize = AtomicIsize::new(0);
static BLOCKS: AtomicIsize = AtomicIsize::new(0);

struct Sample {
    weight: usize,
    stack: u64,
}

static LIVE: [Mutex<Option<HashMap<usize, Sample>>>; SHARDS] =
    [const { Mutex::new(None) }; SHARDS];
static STACKS: Mutex<Option<HashMap<u64, Vec<usize>>>> = Mutex::new(None);

thread_local! {
    static INSIDE: Cell<bool> = const { Cell::new(false) };
    static UNTIL_SAMPLE: Cell<usize> = const { Cell::new(INTERVAL) };
}

fn shard(p: usize) -> usize {
    (p >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58
}

/// Runs `f` with this thread's own allocations left out, or not at all while they already are.
fn outside<R>(f: impl FnOnce() -> R) -> Option<R> {
    let entered = INSIDE.try_with(|inside| !inside.replace(true)).unwrap_or(false);
    if !entered {
        return None;
    }
    let r = f();
    _ = INSIDE.try_with(|inside| inside.set(false));
    Some(r)
}

fn record(p: *mut u8, size: usize) {
    BYTES.fetch_add(size as isize, Relaxed);
    BLOCKS.fetch_add(1, Relaxed);
    let due = UNTIL_SAMPLE
        .try_with(|left| {
            let now = left.get();
            if size >= now {
                left.set(INTERVAL);
                true
            } else {
                left.set(now - size);
                false
            }
        })
        .unwrap_or(false);
    if !due {
        return;
    }
    outside(|| {
        let mut ips = Vec::with_capacity(32);
        backtrace::trace(|frame| {
            ips.push(frame.ip() as usize);
            ips.len() < 32
        });
        let stack = ips
            .iter()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, ip| (h ^ *ip as u64).wrapping_mul(0x100_0000_01b3));
        STACKS
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .entry(stack)
            .or_insert(ips);
        let weight = size.max(INTERVAL);
        LIVE[shard(p as usize)]
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(p as usize, Sample { weight, stack });
    });
}

fn forget(p: *mut u8, size: usize) {
    BYTES.fetch_sub(size as isize, Relaxed);
    BLOCKS.fetch_sub(1, Relaxed);
    outside(|| {
        if let Some(live) = LIVE[shard(p as usize)].lock().unwrap().as_mut() {
            live.remove(&(p as usize));
        }
    });
}

unsafe impl GlobalAlloc for Sampling {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            record(p, layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            record(p, layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        forget(p, layout.size());
        unsafe { System.dealloc(p, layout) };
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, new) };
        if !q.is_null() {
            forget(p, layout.size());
            record(q, new);
        }
        q
    }
}

/// A stack's frames, named, without the sampler's and the allocator's own.
fn describe(ips: &[usize], names: &mut HashMap<usize, String>) -> Vec<String> {
    let mut frames = Vec::new();
    for ip in ips {
        let name = names.entry(*ip).or_insert_with(|| {
            let mut name = String::from("?");
            backtrace::resolve(*ip as *mut std::ffi::c_void, |symbol| {
                if let Some(n) = symbol.name() {
                    name = format!("{n:#}");
                }
                if let (Some(file), Some(line)) = (symbol.filename(), symbol.lineno()) {
                    let file = file.to_string_lossy();
                    let short = file.rsplit("/src/").next().unwrap_or(&file);
                    name = format!("{name} ({short}:{line})");
                }
            });
            name
        });
        let skip = ["heap_sites", "backtrace::", "alloc::alloc", "__rust", "__rdl", "alloc::raw_vec"]
            .iter()
            .any(|s| name.contains(s));
        if !skip {
            frames.push(name.clone());
        }
    }
    frames
}

pub fn start_logging() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let mut names = HashMap::new();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                outside(|| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0., |d| d.as_secs_f64());
                    let mut by_stack: HashMap<u64, (usize, usize)> = HashMap::new();
                    for shard in &LIVE {
                        if let Some(live) = shard.lock().unwrap().as_ref() {
                            for sample in live.values() {
                                let e = by_stack.entry(sample.stack).or_default();
                                e.0 += sample.weight;
                                e.1 += 1;
                            }
                        }
                    }
                    let sampled: usize = by_stack.values().map(|e| e.0).sum();
                    let mut top: Vec<_> = by_stack.into_iter().collect();
                    top.sort_by(|a, b| b.1.0.cmp(&a.1.0));
                    top.truncate(30);
                    let stacks: Vec<_> = {
                        let all = STACKS.lock().unwrap();
                        let all = all.as_ref();
                        top.iter()
                            .map(|(stack, e)| {
                                (*e, all.and_then(|all| all.get(stack)).cloned().unwrap_or_default())
                            })
                            .collect()
                    };
                    let mut out = format!(
                        "browser: rust heap at {now:.0}: {:.1} MiB in {} blocks, sampled {:.1} MiB",
                        BYTES.load(Relaxed) as f64 / 1048576.,
                        BLOCKS.load(Relaxed),
                        sampled as f64 / 1048576.
                    );
                    for ((bytes, samples), ips) in stacks {
                        let frames = describe(&ips, &mut names);
                        out.push_str(&format!(
                            "\nbrowser: rust heap site {:.2} MiB, {samples} samples: {}",
                            bytes as f64 / 1048576.,
                            frames.iter().take(14).cloned().collect::<Vec<_>>().join(" < ")
                        ));
                    }
                    crate::log!("{out}");
                });
            }
        });
    });
}
