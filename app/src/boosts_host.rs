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

/// Gives the core `site`'s arrangement, as Browser::set_arrangement, or takes it away for empty
/// bytes, and says what the core made of it.
pub(crate) fn set_arrangement(profile: u64, site: &str, bytes: &str) -> boosts::Arranged {
    let Ok(name) = CString::new(site) else {
        return boosts::Arranged::Refused;
    };
    let arrangement = match bytes.is_empty() {
        true => std::ptr::null(),
        false => bytes.as_ptr().cast::<c_char>(),
    };
    let state =
        unsafe { bc::bc_profile_set_arrangement(profile, name.as_ptr(), arrangement, bytes.len()) };
    let arranged = arranged(state);
    // check.sh reads this line.
    log!(
        "browser: arrangement for {site} ({} bytes) {arranged:?}",
        bytes.len()
    );
    arranged
}

/// The user's own edit of `site`'s arrangement, as Browser::keep_arrangement: Turn On, or a change
/// put back. False where the core refuses the bytes.
pub(crate) fn keep_arrangement(profile: u64, site: &str, bytes: &str) -> bool {
    let Ok(name) = CString::new(site) else {
        return false;
    };
    let arrangement = bytes.as_ptr().cast::<c_char>();
    let kept = unsafe {
        bc::bc_profile_turn_on_arrangement(profile, name.as_ptr(), arrangement, bytes.len())
    };
    log!(
        "browser: arrangement for {site} ({} bytes) {}",
        bytes.len(),
        if kept { "kept" } else { "refused" }
    );
    kept
}

/// A BC_ARRANGEMENT_ state, any other read as refused.
fn arranged(state: c_int) -> boosts::Arranged {
    match state as u32 {
        bc::BC_ARRANGEMENT_ON => boosts::Arranged::On,
        bc::BC_ARRANGEMENT_NEW => boosts::Arranged::New,
        bc::BC_ARRANGEMENT_CHANGED => boosts::Arranged::Changed,
        _ => boosts::Arranged::Refused,
    }
}

/// Turns Arrange on `tab`'s page on with `look`'s marks and motion, or off for none, as
/// Browser::arrange. Whether it's on.
pub(crate) fn arrange(tab: u64, look: Option<&boosts::arrange::Look>) -> bool {
    let Some(look) = look else {
        unsafe { bc::bc_tab_arrange(tab, false, std::ptr::null()) };
        return false;
    };
    // The words outlive the call, which copies them.
    let words: Vec<(CString, CString)> = (boosts::arrange::KINDS.iter())
        .filter_map(|(kind, word)| Some((CString::new(*kind).ok()?, CString::new(*word).ok()?)))
        .collect();
    let kinds: Vec<bc::bc_arrange_kind> = (words.iter())
        .map(|(kind, word)| bc::bc_arrange_kind {
            kind: kind.as_ptr(),
            word: word.as_ptr(),
        })
        .collect();
    let other = CString::new(boosts::arrange::OTHER).unwrap_or_default();
    let marks = bc::bc_arrange_look {
        accent: look.accent,
        label: look.label,
        ink: look.ink,
        line: f64::from(look.line),
        radius: f64::from(look.radius),
        kinds: kinds.as_ptr(),
        count: kinds.len(),
        other: other.as_ptr(),
        rest: f64::from(look.rest),
        room_omega: f64::from(look.room_omega),
        land_response: f64::from(look.land_response),
        land_damping: f64::from(look.land_damping),
    };
    unsafe { bc::bc_tab_arrange(tab, true, &marks) }
}

/// Asks Arrange on `tab`'s page for `ask`, as Browser::arrange_do.
pub(crate) fn arrange_do(tab: u64, ask: boosts::arrange::Ask) {
    use boosts::arrange::Ask;
    let action = match ask {
        Ask::Outer => bc::BC_ARRANGE_DO_OUTER,
        Ask::Inner => bc::BC_ARRANGE_DO_INNER,
        Ask::Undo => bc::BC_ARRANGE_DO_UNDO,
        Ask::Hide => bc::BC_ARRANGE_DO_HIDE,
        Ask::PutBack => bc::BC_ARRANGE_DO_PUT_BACK,
        Ask::StaysOnScreen => bc::BC_ARRANGE_DO_STAYS_ON_SCREEN,
        Ask::ScrollsWithPage => bc::BC_ARRANGE_DO_SCROLLS_WITH_PAGE,
        Ask::Reset => bc::BC_ARRANGE_DO_RESET,
    };
    log!("browser: arrange {tab} asked {ask:?}");
    unsafe { bc::bc_tab_arrange_do(tab, action as c_int) };
}

/// `text` from the core, or "" for none.
fn text(text: *const c_char) -> String {
    match text.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned(),
    }
}

fn element(element: &bc::bc_arrange_element) -> boosts::arrange::Element {
    boosts::arrange::Element {
        kind: text(element.kind),
        name: text(element.name),
    }
}

/// Where the user is in Arrange on `tab`'s page, read while the call lasts.
pub(crate) unsafe extern "C" fn tab_arrange_state(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_arrange_state,
) {
    use boosts::arrange::{Outcome, Refused, State, Step};
    let Some(state) = (unsafe { state.as_ref() }) else {
        return;
    };
    let step = match state.step as u32 {
        bc::BC_ARRANGE_CARRYING => Step::Carrying,
        bc::BC_ARRANGE_PREVIEW => Step::Preview,
        bc::BC_ARRANGE_SELECTED => Step::Selected,
        _ => Step::Pointing,
    };
    let outcome = |change: c_int| match change as u32 {
        bc::BC_ARRANGE_SETTLES => Outcome::Settles,
        bc::BC_ARRANGE_TRADES => Outcome::Trades,
        bc::BC_ARRANGE_FLOATS => Outcome::Floats,
        bc::BC_ARRANGE_REFUSED => Outcome::Refused,
        _ => Outcome::None,
    };
    let refused = match state.refused as u32 {
        bc::BC_ARRANGE_DOESNT_FIT => Refused::DoesntFit,
        bc::BC_ARRANGE_NOT_SHOWN => Refused::NotShown,
        bc::BC_ARRANGE_COVERED => Refused::Covered,
        _ => Refused::Fits,
    };
    let at = &state.box_;
    let selection = (at.width > 0.).then_some([at.x, at.y, at.width, at.height].map(|v| v as f32));
    let state = State {
        step,
        element: element(&state.element),
        change: outcome(state.change),
        before: element(&state.before),
        after: element(&state.after),
        refused,
        on_screen: state.on_screen,
        can_undo: state.can_undo,
        can_reset: state.can_reset,
        selection,
    };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_state(tab, state, cx));
        }
    });
}

/// Arrange ended on `tab`'s page without the app asking.
pub(crate) unsafe extern "C" fn tab_arrange_ended(_: *mut c_void, tab: bc::bc_id) {
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_ended(tab, cx));
        }
    });
}

/// The user changed `site`'s arrangement in `tab`'s Arrange: all of it, for its file, or none after
/// Reset.
pub(crate) unsafe extern "C" fn tab_arrange_saved(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    arrangement: *const c_char,
    size: usize,
) {
    let site = text(site);
    let bytes = match arrangement.is_null() || size == 0 {
        true => String::new(),
        false => {
            let bytes = unsafe { std::slice::from_raw_parts(arrangement.cast::<u8>(), size) };
            String::from_utf8_lossy(bytes).into_owned()
        }
    };
    update_app_later(move |cx| {
        if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
            shell.update(cx, |shell, cx| shell.arrange_saved(tab, &site, &bytes, cx));
        }
    });
}

/// Which of `site`'s changes found no element on `tab`'s page, by index.
pub(crate) unsafe extern "C" fn tab_arrangement_fits(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    misfits: *const u32,
    count: usize,
) {
    let site = text(site);
    let misfits = match misfits.is_null() {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts(misfits, count) }
            .iter()
            .map(|&index| index as usize)
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
