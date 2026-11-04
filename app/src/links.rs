//! Links from other apps and from pages' menus, and the windows and Spaces they open in.

use super::*;

/// Chrome's Clear browsing data dialog, over its settings page, in the profile of the tab's Space.
pub(crate) const CLEAR_BROWSING_DATA: &str = "chrome://settings/clearBrowserData";

/// Links another app sent: a link clicked in Mail, `open https://…`, an .html file from Finder.
/// Each opens as a new tab in the window in front, as a link to a new tab does, and that window
/// comes to the front. Those that launched the browser wait for its first window.
pub(crate) unsafe extern "C" fn open_links(
    _: *mut c_void,
    urls: *const *const c_char,
    count: usize,
) {
    let urls: Vec<String> = unsafe { std::slice::from_raw_parts(urls, count) }
        .iter()
        .map(|&url| {
            unsafe { CStr::from_ptr(url) }
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let launching = LAUNCH_LINKS.with_borrow_mut(|links| {
        links
            .as_mut()
            .map(|links| links.extend(urls.iter().cloned()))
            .is_some()
    });
    if launching {
        return;
    }
    // A private window's session is the user's on purpose, so a link from elsewhere goes to the
    // regular window in front, as Chrome's does.
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned())
    else {
        return;
    };
    let shown = open_links_in(window.id, &urls);
    let ui_view = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|w| w.id == shown)
            .map_or(window.ui_view, |w| w.ui_view)
    });
    unsafe { platform::host_activate(ui_view) };
    // AppKit hands over the links that launched the browser all at once, so the first window
    // needn't wait for it to finish.
    finished_launching();
}

/// The user launched the browser again with no link, from the Start menu, the taskbar or a
/// shortcut on Windows, or the desktop's launcher on Linux: the regular window in front comes
/// forward, restored if minimized, as the Mac's Dock brings its windows. One is always open, as the
/// browser quits with the last (window_closed), but while it launches, whose first window comes to
/// the front anyway.
pub(crate) unsafe extern "C" fn launched_again(_: *mut c_void) {
    if LAUNCH_LINKS.with_borrow(Option::is_some) {
        return;
    }
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned())
    else {
        return;
    };
    // check.sh reads this line.
    log!("browser: launched again: window {} to the front", window.id);
    unsafe { platform::host_activate(window.ui_view) };
}

/// Another launch asked for a new window, private or not (the desktop file's New Window and New
/// Incognito Window, Windows' jump list): File's New Window or New Incognito Window runs in the
/// regular window in front, as ⌘N or ⇧⌘N there would, and as the Mac's Dock items do.
pub(crate) unsafe extern "C" fn app_new_window(_: *mut c_void, private_window: bool) {
    // check.sh reads this line.
    log!(
        "browser: launched again for a new window, private {}",
        private_window as u8
    );
    update_app_later(move |cx| new_window_in_front(private_window, "launch", cx));
}

/// Runs File's New Window, or with `private_window` New Incognito Window, in the regular window in
/// front, from `from`.
pub(crate) fn new_window_in_front(private_window: bool, from: &str, cx: &mut App) {
    let command = match private_window {
        true => shell::Command::NewPrivateWindow,
        false => shell::Command::NewWindow,
    };
    let front = WINDOWS.with_borrow(|windows| {
        (windows.iter().find(|w| w.regular())).and_then(|w| w.shell.upgrade())
    });
    if let Some(shell) = front {
        shell.update(cx, |shell, cx| shell.run(command, from, cx));
    }
}

/// Whether this launch's own command line asks for a private window, as the desktop file's or the
/// jump list's New Incognito Window does when ilo wasn't running: it opens over the first window.
pub(crate) fn launch_asks_private_window() -> bool {
    std::env::args().any(|arg| arg == "--incognito")
}

/// Opens a tab loading `url` in the front window and shows it: in the Space the window shows if
/// it's one of the user's in `profile`, else in the first of theirs that is, which the window then
/// shows, as a profile's own page, such as Chrome's password manager, shows that profile's data.
/// With none, in the Space shown, unless it's an agent's, which none of the user's tabs go in.
pub(crate) fn open_in_profile(profile: bc::bc_id, url: &str) {
    let window = front().0;
    if window == 0 {
        return;
    }
    let users = |space: bc::bc_id| space != 0 && name(bc::bc_space_agent, space).is_empty();
    let theirs =
        |space: bc::bc_id| users(space) && unsafe { bc::bc_space_profile(space) } == profile;
    let shown = unsafe { bc::bc_window_space(window) };
    let space = match theirs(shown) {
        true => Some(shown),
        false => ids(bc::bc_spaces).into_iter().find(|&space| theirs(space)),
    };
    let Some(space) = space.or(users(shown).then_some(shown)) else {
        return;
    };
    if space != shown {
        unsafe { bc::bc_window_show_space(window, space) };
    }
    open_tab(window, url);
}

/// Opens Chrome's Clear browsing data dialog for `profile`, as Arc's Settings › Profiles does, in
/// the regular window in front, showing a Space of the profile first if it shows another's. Returns
/// whether it did: not for a profile none of the user's Spaces is in, as its tabs would have none.
pub fn clear_browsing_data(profile: u64) -> bool {
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned())
    else {
        return false;
    };
    let users = |space: bc::bc_id| {
        let theirs = unsafe { bc::bc_space_profile(space) } == profile;
        theirs && name(bc::bc_space_agent, space).is_empty()
    };
    let shown = unsafe { bc::bc_window_space(window.id) };
    let Some(space) = std::iter::once(shown)
        .chain(ids(bc::bc_spaces))
        .find(|&space| users(space))
    else {
        return false;
    };
    if space != shown {
        unsafe { bc::bc_window_show_space(window.id, space) };
    }
    open_tab(window.id, CLEAR_BROWSING_DATA);
    unsafe { platform::host_activate(window.ui_view) };
    // check.sh reads this line.
    log!("browser: clear browsing data of profile {profile} in space {space}");
    true
}

/// Opens `urls` as new tabs in `front`, the regular window in front, in the Space it shows, or in
/// the user's it showed last if that's an agent's, which none of the user's tabs go in, else where
/// a link rule sends them. The last one opened shows, and the window it's in is returned. Those
/// for Little ilo, by default all of them, each open in one of their own once this is over, as
/// it can be a window's opening.
pub(crate) fn open_links_in(front: bc::bc_id, urls: &[String]) -> bc::bc_id {
    let mut window = front;
    let mut little = Vec::new();
    for url in urls {
        let routed = link_rules_host::route(url);
        if routed == browser::link_rules::LITTLE {
            little.push(url.clone());
            continue;
        }
        let (shown, tab, space) = open_link_in(front, url, routed);
        window = shown;
        if space == 0 {
            continue;
        }
        // check.sh reads this line.
        log!("browser: link from another app {url}: tab {tab} in space {space} of window {window}");
    }
    if !little.is_empty() {
        update_app_later(move |cx| open_links_in_little(front, little, cx));
    }
    window
}

/// Opens `url` as a new tab, shown, in the Space `routed`, a link rule's, in the regular window
/// showing it, else in `front` switched to it; with `routed` 0, in `front`'s user's Space. Returns
/// the window, the tab and the Space, 0 if there's no Space for it.
pub(crate) fn open_link_in(
    front: bc::bc_id,
    url: &str,
    routed: bc::bc_id,
) -> (bc::bc_id, u64, bc::bc_id) {
    let showing = (routed != 0).then(|| {
        WINDOWS.with_borrow(|windows| {
            let shows = |w: &&BrowserWindow| unsafe { bc::bc_window_space(w.id) } == routed;
            windows
                .iter()
                .filter(|w| w.regular())
                .find(shows)
                .map(|w| w.id)
        })
    });
    let window = showing.flatten().unwrap_or(front);
    let space = match routed {
        0 => users_space_of(window),
        routed => routed,
    };
    if space == 0 {
        return (window, 0, 0);
    }
    if space != unsafe { bc::bc_window_space(window) } {
        unsafe { bc::bc_window_show_space(window, space) };
    }
    let tab = open_tab_behind(window, url, false);
    if tab != 0 {
        unsafe { bc::bc_window_select(window, tab) };
    }
    (window, tab, space)
}

/// The Space `window` shows, or the user's it showed last if that's an agent's, else the user's
/// first, or 0.
pub(crate) fn users_space_of(window: bc::bc_id) -> bc::bc_id {
    let users = |space: bc::bc_id| space != 0 && name(bc::bc_space_agent, space).is_empty();
    let shown = unsafe { bc::bc_window_space(window) };
    if users(shown) {
        return shown;
    }
    let last = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|w| w.id == window)
            .map_or(0, |w| w.users_space)
    });
    let spaces = ids(bc::bc_spaces);
    match spaces.contains(&last) && users(last) {
        true => last,
        false => spaces.into_iter().find(|&space| users(space)).unwrap_or(0),
    }
}

/// The page menu's Open Link in New Window and Open Link in Incognito Window: `url` opens in a
/// window from `window` as ⌘N's, or with `private` in a private one in `profile`'s session as
/// ⇧⌘N's, and shows there.
pub(crate) unsafe extern "C" fn window_open_link(
    _: *mut c_void,
    window: bc::bc_id,
    url: *const c_char,
    profile: bc::bc_id,
    private: bool,
) {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    update_app(|cx| {
        // A regular window takes its Spaces from a regular one, as ⌘N's does from a private one.
        let from = WINDOWS.with_borrow(|windows| {
            let this = windows
                .iter()
                .find(|w| w.id == window && (private || w.regular()));
            this.or_else(|| windows.iter().find(|w| w.regular()))
                .cloned()
        });
        let private = private.then_some(Own::Private(profile));
        if let Some(opened) = from.and_then(|from| open_window(Some(from), private, cx)) {
            open_tab(opened, &url);
        }
    });
}
