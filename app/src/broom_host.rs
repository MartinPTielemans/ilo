//! The broom on the New Tab line (#4252): what it shows in each window, kept so a draw only reads
//! it, and the user's sort of today's tabs into folders, which their own Claude Code runs
//! (browser::broom). Only the user's click, the menu item or the bar entry starts one: no agent
//! tool, page or typed text reaches it, and a private window has none.

use super::*;
use browser::agent_history::{self, Past, Sorted};
use browser::broom::{self, Folder, Run};
use browser::shell::Broom;
use gpui::{Entity, Task};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

/// How long Couldn't sort stands in Clear's place (Z29).
const FAILED_FOR: Duration = Duration::from_secs(4);

/// The assistant a sort runs, by its id, as its entry in the 30-day file names it.
const ASSISTANT: &str = "claude";

thread_local! {
    /// What each window's broom shows, by the window's core id; a window whose broom isn't drawn
    /// has none.
    static SHOWN: RefCell<HashMap<bc::bc_id, Broom>> = RefCell::default();
    /// Whether each Space has the two tabs a sort needs, as the pass last found, by its core id.
    /// A Space no pass has looked at since it changed has none.
    static SORTS: RefCell<HashMap<bc::bc_id, bool>> = RefCell::default();
    /// The one sort running.
    static RUNNING: RefCell<Option<Sorting>> = const { RefCell::new(None) };
    /// The windows a sort that moved nothing ended in, each until its timer, which goes with it.
    static FAILED: RefCell<HashMap<bc::bc_id, Task<()>>> = RefCell::default();
    /// The profiles whose Sort tabs into folders the user turned off in Settings (Z27), by their
    /// core id. Its row is drawn only where the broom can be, which needs the core's today's
    /// folders, and that core line keeps it in the profile's prefs in this one's place.
    static OFF: RefCell<HashSet<bc::bc_id>> = RefCell::default();
}

/// A sort under way: the window it was started in, the Space it sorts, and its run.
struct Sorting {
    window: bc::bc_id,
    space: bc::bc_id,
    run: Arc<Run>,
}

/// What `window`'s broom shows, as last kept (Browser::broom).
pub(crate) fn shown(window: bc::bc_id) -> Option<Broom> {
    SHOWN.with_borrow(|shown| shown.get(&window).copied())
}

/// ⌘T or any other change to `space`'s rows: the pass, again, for a Space a window shows, and the
/// windows' brooms follow. A Space none shows is looked at when one first does.
pub(crate) fn space_changed(space: bc::bc_id, cx: &mut App) {
    let shell = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .filter(|window| window.regular())
            .find_map(|window| {
                (window.shell.upgrade()).filter(|shell| shell.read(cx).space() == space)
            })
    });
    match shell {
        Some(shell) => pass(space, &shell, cx),
        None => _ = SORTS.with_borrow_mut(|sorts| sorts.remove(&space)),
    }
    settle_all(cx);
}

/// `window` shows `space`: the pass, if it's the first time since the Space changed, and its
/// broom follows. Launch's first answer is this, as each window first shows its Space.
pub(crate) fn window_space_shown(window: bc::bc_id, space: bc::bc_id, cx: &mut App) {
    let Some(shown) = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .find(|each| each.id == window && each.regular())
            .map(|each| each.shell.clone())
    }) else {
        return settle_all(cx);
    };
    if SORTS.with_borrow(|sorts| !sorts.contains_key(&space))
        && let Some(shell) = shown.upgrade()
    {
        pass(space, &shell, cx);
    }
    settle_all(cx);
}

/// A tab's page turned web or not, which changes what a sort counts: each Space a window shows is
/// looked at again, and the others when they're next shown.
pub(crate) fn page_counts_changed(cx: &mut App) {
    SORTS.with_borrow_mut(HashMap::clear);
    let shells: Vec<_> = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .filter(|window| window.regular())
            .filter_map(|window| window.shell.upgrade())
            .collect()
    });
    for shell in shells {
        let space = shell.read(cx).space();
        if SORTS.with_borrow(|sorts| !sorts.contains_key(&space)) {
            pass(space, &shell, cx);
        }
    }
    settle_all(cx);
}

/// Whether the core can put a sort's answer in place, as today's folders: known without a run,
/// and part of the one answer that makes the broom be. This core can't, as its header has no
/// `bc_folder_create_today`. Nor can Windows, till its run is killed as a job object, whole.
fn makes_today_folders() -> bool {
    let core = false;
    core && cfg!(not(target_os = "windows"))
}

/// Whether the broom can be at all: the core makes today's folders, and the user's own Claude
/// Code, signed in, runs their tasks. Where it can't, no window has a broom, so no menu item, bar
/// entry or click starts a sort, and Settings draws no row for it.
pub(crate) fn can_be(cx: &App) -> bool {
    makes_today_folders()
        && browser::agents::global(cx).is_some_and(|agents| agents.read(cx).sorts())
}

/// What the Agents know changed, such as Claude Code signing out or the inside switch turning
/// off: the brooms follow, with no pass, and a sort under way that can no longer run ends, its
/// program's whole group with it, moving nothing.
pub(crate) fn agents_changed(cx: &mut App) {
    if !can_be(cx) {
        let ending = RUNNING.with_borrow_mut(Option::take);
        if let Some(sorting) = ending {
            sorting.run.end();
        }
    }
    settle_all(cx);
}

/// The pass: whether `space` has two tabs a sort would hand over, from what `shell` holds, looking
/// no further than the second.
fn pass(space: bc::bc_id, shell: &Entity<Shell>, cx: &App) {
    let sorts = shell.read(cx).broom_sorts(space);
    SORTS.with_borrow_mut(|kept| kept.insert(space, sorts));
}

/// Each window's broom as it now is, from what's kept, each window drawn again only if its own
/// changed.
fn settle_all(cx: &mut App) {
    let sorts = can_be(cx);
    // Telling a shell to draw again only marks it, so nothing here reaches the windows' list.
    WINDOWS.with_borrow(|windows| {
        for window in windows {
            let Some(shell) = window.shell.upgrade() else {
                continue;
            };
            let space = Some(shell.read(cx).space()).filter(|&space| {
                sorts && window.regular() && shell.read(cx).broom_space(space) && !off(space)
            });
            let now = space.map(|space| answer(window.id, space));
            let was = SHOWN.with_borrow_mut(|shown| match now {
                Some(now) => shown.insert(window.id, now),
                None => shown.remove(&window.id),
            });
            if was != now {
                shell.update(cx, |_, cx| cx.notify());
            }
        }
    });
}

/// Whether `space`'s profile has Sort tabs into folders turned off. Asks the core whose profile it
/// is only once the user has turned one off, so ⌘T's pass reaches no core call.
fn off(space: bc::bc_id) -> bool {
    OFF.with_borrow(|off| !off.is_empty() && off.contains(&unsafe { bc::bc_space_profile(space) }))
}

/// Whether `profile` sorts tabs into folders, as Settings' switch shows it.
pub(crate) fn on(profile: bc::bc_id) -> bool {
    OFF.with_borrow(|off| !off.contains(&profile))
}

/// Settings' switch for `profile`: off, its windows' brooms go, and a sort under way of one of its
/// Spaces ends, moving nothing.
pub(crate) fn set_on(profile: bc::bc_id, on: bool, cx: &mut App) {
    OFF.with_borrow_mut(|off| match on {
        true => off.remove(&profile),
        false => off.insert(profile),
    });
    let ending = RUNNING.with_borrow_mut(|running| {
        running
            .take_if(|running| !on && unsafe { bc::bc_space_profile(running.space) } == profile)
            .map(|running| running.run)
    });
    if let Some(run) = ending {
        run.end();
    }
    settle_all(cx);
}

/// What the broom of `window`, showing `space`, shows: one sort runs at a time, so while one does,
/// another Space's broom takes no click.
fn answer(window: bc::bc_id, space: bc::bc_id) -> Broom {
    let running = RUNNING.with_borrow(|running| running.as_ref().map(|running| running.space));
    match running {
        Some(sorted) if sorted == space => Broom::Sorting,
        Some(_) => Broom::NothingToSort,
        None if FAILED.with_borrow(|failed| failed.contains_key(&window)) => Broom::Failed,
        None => match SORTS.with_borrow(|sorts| sorts.get(&space).copied()) {
            Some(true) => Broom::Idle,
            _ => Broom::NothingToSort,
        },
    }
}

/// The user's click on `window`'s broom, its menu item or its bar entry (Browser::start_broom):
/// sorts the Today tabs of the Space it shows, if its broom takes a click.
pub(crate) fn start(window: bc::bc_id, cx: &mut App) {
    if !can_be(cx) || !shown(window).is_some_and(Broom::sorts) {
        return;
    }
    let Some(shell) = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .find(|each| each.id == window && each.regular())
            .and_then(|each| each.shell.upgrade())
    }) else {
        return;
    };
    let program = browser::agents::global(cx).and_then(|agents| agents.read(cx).sort_program());
    let Some(program) = program else {
        return;
    };
    let space = shell.read(cx).space();
    let (handed, tabs): (Vec<u64>, Vec<broom::Tab>) =
        shell.read(cx).broom_tabs(space).into_iter().unzip();
    let Some(list) = broom::handed(&tabs) else {
        return;
    };
    drop(tabs);
    FAILED.with_borrow_mut(|failed| failed.remove(&window));
    let Ok(run) = Run::new(broom::CAP) else {
        return failed(window, cx);
    };
    let run = Arc::new(run);
    RUNNING.set(Some(Sorting {
        window,
        space,
        run: run.clone(),
    }));
    settle_all(cx);
    let started = browser::usage::now();
    let count = handed.len();
    let answer = cx.background_spawn({
        let run = run.clone();
        async move { run.answer(&program, &list, count) }
    });
    cx.spawn(async move |cx| {
        let answer = answer.await;
        cx.update(|cx| ended(run, handed, answer, started, cx));
    })
    .detach();
}

/// A sort's run answered, failed or was ended: its folders go in, if it's still the sort running.
fn ended(
    run: Arc<Run>,
    handed: Vec<u64>,
    answer: Result<Vec<Folder>, broom::Failed>,
    started: u64,
    cx: &mut App,
) {
    let mine = RUNNING.with_borrow(|running| {
        running
            .as_ref()
            .is_some_and(|running| Arc::ptr_eq(&running.run, &run))
    });
    // Ended as its window closed or the browser quit: nothing moves.
    if !mine {
        return;
    }
    let Some(Sorting { window, space, .. }) = RUNNING.take() else {
        return;
    };
    let shell = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .find(|each| each.id == window && each.regular())
            .and_then(|each| each.shell.upgrade())
    });
    // Not handed over, so no line says it was: the run didn't start or read the program's
    // version, or the program was too old to take it.
    let given = !matches!(
        answer,
        Err(broom::Failed::NotStarted | broom::Failed::TooOld)
    );
    let sorted = match (answer, shell) {
        (Ok(folders), Some(shell)) => shell.update(cx, |shell, cx| {
            shell.sort_into_folders(space, &handed, folders, cx)
        }),
        _ => None,
    };
    if given {
        let (moved, folders) = sorted.unwrap_or_default();
        let sorted = Sorted {
            handed: handed.len(),
            moved,
            folders,
        };
        keep(
            space,
            Past::sort(ASSISTANT, started, browser::usage::now(), sorted),
        );
    }
    match sorted {
        Some(_) => settle_all(cx),
        None => failed(window, cx),
    }
}

/// A sort's entry, in the 30-day file of the profile of the Space it sorted.
fn keep(space: bc::bc_id, past: Past) {
    let profile = unsafe { bc::bc_space_profile(space) };
    if profile == 0 {
        return;
    }
    let kept = agent_history::read(&bar_picks_host::sessions(profile));
    let kept = agent_history::kept(kept, past, browser::usage::now());
    bar_picks_host::set_sessions(profile, &agent_history::write(&kept));
}

/// A sort in `window` moved nothing: Couldn't sort stands there, for a while, with one timer,
/// which goes if the window does.
fn failed(window: bc::bc_id, cx: &mut App) {
    let timer = cx.spawn(async move |cx| {
        cx.background_executor().timer(FAILED_FOR).await;
        cx.update(|cx| {
            FAILED.with_borrow_mut(|failed| failed.remove(&window));
            settle_all(cx);
        });
    });
    FAILED.with_borrow_mut(|failed| failed.insert(window, timer));
    settle_all(cx);
}

/// `window` closed: a sort it started ends, moving nothing, and what it showed goes.
pub(crate) fn window_closed(window: bc::bc_id, cx: &mut App) {
    SHOWN.with_borrow_mut(|shown| shown.remove(&window));
    // Dropping its timer cancels it.
    let timer = FAILED.with_borrow_mut(|failed| failed.remove(&window));
    drop(timer);
    let ending = RUNNING.with_borrow_mut(|running| {
        running
            .take_if(|running| running.window == window)
            .map(|running| running.run)
    });
    if let Some(run) = ending {
        run.end();
        // Another window showing the Space it sorted shows it ready again.
        settle_all(cx);
    }
}

/// The browser quits: a sort under way ends, what it started with it.
pub(crate) fn quitting() {
    if let Some(running) = RUNNING.take() {
        running.run.end();
    }
}
