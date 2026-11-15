//! Boosts (#695, browser::boosts) between the app and the core: each profile's are read at launch
//! and given to the core, and again as their folder changes, as with a save in the user's editor;
//! Zap's clicks come back from the core to the window of their tab; Settings turns them all off.
//! The user's alone: the agents' MCP server has no Boost tool, and the core's Boost calls are made
//! only from here and the window's UI.

use browser::boosts;

use super::*;

/// Settings' switch, a file in the profile directory while it's off, as the app's other switches.
const NO_BOOSTS: &str = "no-boosts";

thread_local! {
    static READ_SWITCH: Cell<bool> = const { Cell::new(false) };
}

/// Whether Settings has every Boost off.
pub(crate) fn all_off() -> bool {
    std::path::Path::new(&format!("{}/{NO_BOOSTS}", crate::profile_dir())).exists()
}

/// Has the core show each site's CSS and arrangement as they changed.
pub(crate) fn show(changes: Vec<boosts::Change>) {
    for change in changes {
        let css = match change.shows {
            boosts::Shows::Css(css) => css,
            boosts::Shows::Arrangement(bytes) => {
                let state = set_arrangement(change.profile, &change.site, &bytes);
                if !bytes.is_empty() {
                    boosts::with(|boosts| boosts.set_arranged(change.profile, &change.site, state));
                }
                continue;
            }
        };
        let (Ok(site), Ok(css)) = (CString::new(change.site.as_str()), CString::new(css)) else {
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

/// Gives the core `site`'s arrangement, as Browser::set_arrangement, or takes it away when empty.
pub(crate) fn set_arrangement(profile: u64, site: &str, bytes: &str) -> boosts::Arranged {
    let Ok(site_c) = CString::new(site) else {
        return boosts::Arranged::Refused;
    };
    let state = match bytes.is_empty() {
        true => unsafe {
            bc::bc_profile_set_arrangement(profile, site_c.as_ptr(), std::ptr::null(), 0)
        },
        false => unsafe {
            let bytes_ptr = bytes.as_ptr().cast::<c_char>();
            bc::bc_profile_set_arrangement(profile, site_c.as_ptr(), bytes_ptr, bytes.len())
        },
    };
    let state = match state as u32 {
        bc::BC_ARRANGEMENT_ON => boosts::Arranged::On,
        bc::BC_ARRANGEMENT_NEW => boosts::Arranged::New,
        bc::BC_ARRANGEMENT_CHANGED => boosts::Arranged::Changed,
        _ => boosts::Arranged::Refused,
    };
    log!(
        "browser: arrangement for {site} ({} bytes) {state:?}",
        bytes.len()
    );
    state
}

/// Turn On, or a change put back: the core records `bytes` as the user's for `site` and places
/// them, as Browser::keep_arrangement.
pub(crate) fn keep_arrangement(profile: u64, site: &str, bytes: &str) -> bool {
    let Ok(site_c) = CString::new(site) else {
        return false;
    };
    let bytes_ptr = bytes.as_ptr().cast::<c_char>();
    let kept = unsafe {
        bc::bc_profile_turn_on_arrangement(profile, site_c.as_ptr(), bytes_ptr, bytes.len())
    };
    log!(
        "browser: arrangement for {site} ({} bytes) kept {kept}",
        bytes.len()
    );
    kept
}

/// The look the core gives what Arrange lifts off the page: the app's popover elevation, once,
/// before the first page loads.
pub(crate) fn set_arrange_lift() {
    let lift = browser::elevation::Level::Popover.lift();
    let lift = bc::bc_arrange_lift {
        rim: lift.rim,
        outline: lift.outline,
        shadow: lift.shadow,
        outline_blur: lift.outline_blur.into(),
        shadow_y: lift.shadow_y.into(),
        shadow_blur: lift.shadow_blur.into(),
    };
    unsafe { bc::bc_arrange_set_lift(&lift) };
}

/// Arrange ended on the tab without the app asking, as a new page ends it.
pub(crate) unsafe extern "C" fn tab_arrange_ended(_: *mut c_void, tab: bc::bc_id) {
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_ended(tab, cx));
        }
    });
}

/// Where the user is in the tab's Arrange, once a step changes and as a scroll starts and rests:
/// for the mode's keys, ⌘Z's history and the mini bar over the thing selected.
pub(crate) unsafe extern "C" fn tab_arrange_state(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_arrange_state,
) {
    use boosts::arrange::{Element, Outcome, Refused, State, Step};
    let state = unsafe { &*state };
    let text = |text: *const c_char| {
        if text.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
        }
    };
    let element = |element: &bc::bc_arrange_element| Element {
        kind: text(element.kind),
        name: text(element.name),
    };
    let r = &state.box_;
    let state = State {
        step: match state.step as u32 {
            bc::BC_ARRANGE_CARRYING => Step::Carrying,
            bc::BC_ARRANGE_PREVIEW => Step::Preview,
            bc::BC_ARRANGE_SELECTED => Step::Selected,
            _ => Step::Pointing,
        },
        element: element(&state.element),
        change: match state.change as u32 {
            bc::BC_ARRANGE_SETTLES => Outcome::Settles,
            bc::BC_ARRANGE_TRADES => Outcome::Trades,
            bc::BC_ARRANGE_FLOATS => Outcome::Floats,
            bc::BC_ARRANGE_REFUSED => Outcome::Refused,
            _ => Outcome::None,
        },
        before: element(&state.before),
        after: element(&state.after),
        refused: match state.refused as u32 {
            bc::BC_ARRANGE_DOESNT_FIT => Refused::DoesntFit,
            bc::BC_ARRANGE_NOT_SHOWN => Refused::NotShown,
            bc::BC_ARRANGE_COVERED => Refused::Covered,
            _ => Refused::Fits,
        },
        on_screen: state.on_screen,
        can_undo: state.can_undo,
        can_reset: state.can_reset,
        // A box 0 wide is none: nothing selected, or the page scrolling.
        selection: (r.width > 0.).then(|| [r.x, r.y, r.width, r.height].map(|v| v as f32)),
    };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_state(tab, state, cx));
        }
    });
}

/// The user changed the site's arrangement in the tab's Arrange: all of it, or empty after Reset.
pub(crate) unsafe extern "C" fn tab_arrange_saved(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    arrangement: *const c_char,
    size: usize,
) {
    if site.is_null() {
        return;
    }
    let site = unsafe { CStr::from_ptr(site) }
        .to_string_lossy()
        .into_owned();
    let bytes = match arrangement.is_null() || size == 0 {
        true => String::new(),
        false => {
            let bytes = unsafe { std::slice::from_raw_parts(arrangement.cast::<u8>(), size) };
            // Not text, it's no arrangement the app writes.
            let Ok(bytes) = std::str::from_utf8(bytes) else {
                return;
            };
            bytes.to_owned()
        }
    };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_saved(tab, &site, &bytes, cx));
        }
    });
}

/// Which of the site's saved changes found nothing on the tab's page, by index.
pub(crate) unsafe extern "C" fn tab_arrangement_fits(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    misfits: *const u32,
    count: usize,
) {
    if site.is_null() {
        return;
    }
    let site = unsafe { CStr::from_ptr(site) }
        .to_string_lossy()
        .into_owned();
    let misfits: Vec<usize> = match misfits.is_null() || count == 0 {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts(misfits, count) }
            .iter()
            .map(|&each| each as usize)
            .collect(),
    };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| {
                shell.arrangement_fits(tab, &site, misfits, cx)
            });
        }
    });
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
    if let Some(watched) = boosts::with(|boosts| boosts.folder(profile)) {
        boosts::watch_in(watched, cx, move |cx| folder_changed(profile, cx));
    }
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
