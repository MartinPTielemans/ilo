//! Boosts (#695, browser::boosts) between the app and the core: each profile's are read at launch
//! and given to the core, and again as their folder changes, as with a save in the user's editor;
//! Zap's clicks come back from the core to the window of their tab; Settings turns them all off.
//! The user's alone: the agents' MCP server has no Boost tool, and the core's Boost calls are made
//! only from here and the window's UI.

use futures::{StreamExt, channel::mpsc};

use browser::boosts;

use super::*;

/// Settings' switch, a file in the profile directory while it's off, as the app's other switches.
const NO_BOOSTS: &str = "no-boosts";

thread_local! {
    /// Where the watchers' threads say a profile's folder changed, once the first is started.
    static TOLD: RefCell<Option<mpsc::UnboundedSender<u64>>> = const { RefCell::new(None) };
    static READ_SWITCH: Cell<bool> = const { Cell::new(false) };
}

/// Whether Settings has every Boost off.
pub(crate) fn all_off() -> bool {
    std::path::Path::new(&format!("{}/{NO_BOOSTS}", crate::profile_dir())).exists()
}

/// Has the core show each site's CSS as it changed.
pub(crate) fn show(changes: Vec<boosts::Change>) {
    for change in changes {
        let (Ok(site), Ok(css)) = (CString::new(change.site.as_str()), CString::new(change.css))
        else {
            continue;
        };
        // check.sh reads this line.
        log!(
            "browser: boost shown {} ({} bytes)",
            change.site,
            css.as_bytes().len()
        );
        unsafe { bc::bc_profile_set_boost(change.profile, site.as_ptr(), css.as_ptr()) };
    }
}

/// Reads `profile`'s Boosts, has the core show them and watches their folder, once a launch.
pub(crate) fn load(profile: u64, cx: &mut App) {
    if boosts::with(|boosts| boosts.loaded(profile)) {
        return;
    }
    let Some(folder) = bar_picks_host::folder(profile) else {
        return;
    };
    if !READ_SWITCH.replace(true) {
        let off = all_off();
        boosts::with(|boosts| boosts.set_all_off(off));
    }
    let changes = boosts::with(|boosts| boosts.load(profile, folder));
    let count = boosts::with(|boosts| boosts.list(profile).len());
    // check.sh reads this line.
    log!("browser: boosts of profile {profile} read: {count}");
    show(changes);
    let told = told(cx);
    if let Some(watched) = boosts::with(|boosts| boosts.folder(profile)) {
        boosts::watch(watched, move || {
            let _ = told.unbounded_send(profile);
        });
    }
}

/// The channel the watchers tell the UI thread on, made the first time.
fn told(cx: &mut App) -> mpsc::UnboundedSender<u64> {
    TOLD.with_borrow_mut(|told| {
        told.get_or_insert_with(|| {
            let (sender, mut changed) = mpsc::unbounded::<u64>();
            cx.spawn(async move |cx| {
                while let Some(profile) = changed.next().await {
                    cx.update(|cx| folder_changed(profile, cx));
                }
            })
            .detach();
            sender
        })
        .clone()
    })
}

/// A file in `profile`'s Boosts folder changed: what it changed shows, and the windows draw what
/// they show of it again.
fn folder_changed(profile: u64, cx: &mut App) {
    let changes = boosts::with(|boosts| boosts.reload(profile));
    if changes.is_empty() {
        return;
    }
    show(changes);
    update_shells_in(cx, |shell, cx| shell.boosts_changed(cx));
}

/// The profiles changed: those not read yet are, as at launch.
pub(crate) fn profiles_changed(profiles: &[(u64, String)]) {
    let profiles: Vec<u64> = profiles.iter().map(|(id, _)| *id).collect();
    update_app_later(move |cx| {
        for profile in profiles {
            load(profile, cx);
        }
    });
}

/// Settings' switch: every Boost off, or each as its site has it.
pub(crate) fn set_all_on(on: bool, cx: &mut App) {
    let path = format!("{}/{NO_BOOSTS}", crate::profile_dir());
    _ = match on {
        true => std::fs::remove_file(path),
        false => std::fs::write(path, ""),
    };
    READ_SWITCH.set(true);
    show(boosts::with(|boosts| boosts.set_all_off(!on)));
    update_shells_in(cx, |shell, cx| shell.boosts_changed(cx));
}

/// Opens `path` in the user's text editor: the Mac's default one, Notepad on Windows, and what
/// the desktop opens it with on Linux.
pub(crate) fn edit_file(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    run("open", &["-t".as_ref(), path.as_os_str()]);
    #[cfg(target_os = "windows")]
    run("notepad.exe", &[path.as_os_str()]);
    #[cfg(target_os = "linux")]
    run("xdg-open", &[path.as_os_str()]);
}

/// Shows `path` in the system's file manager: selected in the Finder and Explorer, its folder on
/// Linux.
pub(crate) fn show_file(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    run("open", &["-R".as_ref(), path.as_os_str()]);
    #[cfg(target_os = "windows")]
    {
        let mut select = std::ffi::OsString::from("/select,");
        select.push(path.as_os_str());
        run("explorer.exe", &[select.as_os_str()]);
    }
    #[cfg(target_os = "linux")]
    if let Some(folder) = path.parent() {
        run("xdg-open", &[folder.as_os_str()]);
    }
}

/// Starts `program` with `args`, and waits for it on a thread of its own, so it leaves no zombie.
fn run(program: &str, args: &[&std::ffi::OsStr]) {
    match browser::child::command(program).args(args).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(error) => log!("couldn't run {program}: {error}"),
    }
}

/// What the user clicked with Zap on, as a selector, or null for the page itself.
pub(crate) unsafe extern "C" fn tab_boost_zapped(
    _: *mut c_void,
    tab: bc::bc_id,
    selector: *const c_char,
) {
    let selector = (!selector.is_null()).then(|| {
        unsafe { CStr::from_ptr(selector) }
            .to_string_lossy()
            .into_owned()
    });
    // After the update under way: the click comes from a call the core makes inside one.
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| {
                shell.boost_zapped(tab, selector.as_deref(), cx)
            });
        }
    });
}

/// Zap ended on the tab's page, as a new page does.
pub(crate) unsafe extern "C" fn tab_boost_zap_ended(_: *mut c_void, tab: bc::bc_id) {
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.boost_zap_ended(tab, cx));
        }
    });
}
