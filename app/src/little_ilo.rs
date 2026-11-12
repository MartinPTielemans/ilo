//! Little ilo: its windows, their size and place, the Dock's menu and the menu bar's button.

use super::*;

/// The size the first Little ilo window opens at (Arc's).
pub(crate) const LITTLE_SIZE: (f64, f64) = (900., 640.);

/// Where the size of the Little ilo window closed last is kept, beside the launch frame.
pub(crate) fn little_size_path() -> String {
    format!("{}/little-size", profile_dir())
}

/// The size a Little ilo window opens at: the last one's, as Arc's, else Arc's first, no bigger
/// than the main screen.
pub(crate) fn little_size() -> (f64, f64) {
    let kept = std::fs::read_to_string(little_size_path())
        .ok()
        .and_then(|kept| {
            let mut numbers = kept.split_whitespace().map(|n| n.parse::<f64>().ok());
            Some((numbers.next()??, numbers.next()??))
        });
    let (width, height) = kept
        .filter(|&(w, h)| w > 0. && h > 0.)
        .unwrap_or(LITTLE_SIZE);
    match screens().first() {
        Some(screen) => (width.min(screen.width), height.min(screen.height)),
        None => (width, height),
    }
}

/// The regular window showing `home`, else the one in front, which ⌘O shows it in.
pub(crate) fn main_window_for(home: bc::bc_id) -> Option<bc::bc_id> {
    WINDOWS.with_borrow(|windows| {
        let regular = || windows.iter().filter(|w| w.regular());
        let showing = regular().find(|w| unsafe { bc::bc_window_space(w.id) } == home);
        showing.or_else(|| regular().next()).map(|w| w.id)
    })
}

/// The user's Space a Little ilo window's pages go to: its own, or, once that's gone or an agent's,
/// the one `main` shows.
pub(crate) fn little_home(home: bc::bc_id, main: bc::bc_id) -> bc::bc_id {
    let gone = !ids(bc::bc_spaces).contains(&home);
    match gone || !name(bc::bc_space_agent, home).is_empty() {
        true => users_space_of(main),
        false => home,
    }
}

/// A tab a Little ilo window's page opened, by ⌘-click, `target=_blank` or ⌥-click, goes to the
/// top of its Space's Today, behind, as the window has one page and no sidebar to show it in (Arc).
/// The window still asks what it would open another app for, as a sign-in's pop-up does (#1914).
pub(crate) fn little_tab_opened(window: bc::bc_id, tab: u64, home: bc::bc_id, cx: &mut App) {
    let home = little_home(home, main_window_for(home).unwrap_or(0));
    unsafe { bc::bc_tab_move_to_space(tab, home) };
    // check.sh reads this line.
    log!("browser: little window {window} put {tab} in space {home}");
    update_window_in(window, cx, |shell, cx| shell.little_sent(tab, cx));
}

thread_local! {
    /// What a page asked of the pop-up each tab is, from tab_popup, until its tab_opened comes.
    static POPUPS: RefCell<HashMap<u64, (bc::bc_rect, bool)>> = RefCell::default();
    /// Each pop-up's window, by its tab, with the Little ilo window whose page opened it.
    static POPUP_WINDOWS: RefCell<HashMap<u64, (bc::bc_id, bc::bc_id)>> = RefCell::default();
    /// The pop-ups' windows, until each closes: sized by their pages, not the user.
    static POPUP_IDS: RefCell<std::collections::HashSet<bc::bc_id>> = RefCell::default();
}

/// Whether `window`, closing, is a pop-up's, which it forgets: a sign-in's size is its page's, so
/// it isn't the next Little ilo window's to open at (save_little_size).
pub(crate) fn popup_window_closed(window: bc::bc_id) -> bool {
    POPUP_IDS.with_borrow_mut(|ids| ids.remove(&window))
}

/// The tab opened next in `window` is a pop-up its page asked for, with its content's size and,
/// if `placed`, where (bc_host's tab_popup).
pub(crate) unsafe extern "C" fn tab_popup(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
    opener: bc::bc_id,
    requested: bc::bc_rect,
    placed: bool,
) {
    let bc::bc_rect {
        x,
        y,
        width,
        height,
    } = requested;
    let at = if placed {
        format!(" at {x:.0} {y:.0}")
    } else {
        String::new()
    };
    // check.sh reads this line.
    log!("browser: tab {tab} popup of {opener} in window {window} asks {width:.0} {height:.0}{at}");
    POPUPS.with_borrow_mut(|popups| popups.insert(tab, (requested, placed)));
}

/// What the page asked of `tab`, if it's a pop-up, once: its tab_opened has come.
pub(crate) fn take_popup(tab: u64) -> Option<(bc::bc_rect, bool)> {
    POPUPS.with_borrow_mut(|popups| popups.remove(&tab))
}

/// A pop-up a Little ilo window's page asked for, as a sign-in's, opens as a Little ilo window of
/// its own over it, at the size the page asked, as Chrome's opens as a window (#1919): it's a tab
/// still (decision 3), in its home's profile, its page live, which the window's bar names. Else,
/// as a link's tab, it goes behind.
pub(crate) fn little_popup_opened(
    window: bc::bc_id,
    tab: u64,
    home: bc::bc_id,
    (asked, placed): (bc::bc_rect, bool),
    cx: &mut App,
) {
    let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == window).cloned());
    let Some(from) = from else {
        return little_tab_opened(window, tab, home, cx);
    };
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    let opener = match unsafe {
        platform::host_window_frame(from.ui_view, &mut x, &mut y, &mut width, &mut height)
    } {
        true => Frame {
            x,
            y,
            width,
            height,
        },
        false => {
            let (width, height) = little_size();
            let (x, y) = platform::centered(width, height);
            Frame {
                x,
                y,
                width,
                height,
            }
        }
    };
    let at = placed.then_some((asked.x, asked.y));
    let frame = window_frames::popup(opener, (asked.width, asked.height), at, &screens());
    let home = little_home(home, main_window_for(home).unwrap_or(0));
    let profile = unsafe { bc::bc_space_profile(home) };
    // Its page goes to its home's Today, where a Little ilo window takes it from.
    unsafe { bc::bc_tab_move_to_space(tab, home) };
    let at = bc::bc_rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
    };
    let opened = (profile != 0)
        .then(|| {
            open_window_as(
                Some(from),
                Some(Own::Little { profile, home }),
                None,
                Some(at),
                cx,
            )
        })
        .flatten();
    let Some(popup) = opened else {
        // check.sh reads this line.
        log!("browser: little window {window} put {tab} in space {home}");
        return;
    };
    if unsafe { bc::bc_window_take_tab(popup, tab) } == 0 {
        // After this update, as AppKit's will-close calls back into GPUI, which it holds.
        cx.foreground_executor()
            .spawn(async move { unsafe { bc::bc_window_close(popup) } })
            .detach();
        log!("browser: little window {window} put {tab} in space {home}");
        return;
    }
    POPUP_WINDOWS.with_borrow_mut(|windows| windows.insert(tab, (popup, window)));
    POPUP_IDS.with_borrow_mut(|ids| ids.insert(popup));
    // check.sh reads this line.
    log!(
        "browser: little window {window} popup {tab} in window {popup} at {:.0} {:.0} {:.0} {:.0}",
        frame.x,
        frame.y,
        frame.width,
        frame.height
    );
}

/// A pop-up's page closed, as a sign-in's closes itself once it's done: its window goes, and the
/// Little ilo window that opened it has the keyboard back. Never inside a GPUI update, as the
/// window's close calls back into GPUI (AppKit's will-close).
pub(crate) fn popup_closed(tab: u64) {
    let Some((popup, opener)) = POPUP_WINDOWS.with_borrow_mut(|windows| windows.remove(&tab))
    else {
        return;
    };
    let open = |id| WINDOWS.with_borrow(|windows| windows.iter().any(|w| w.id == id));
    if open(popup) {
        unsafe { bc::bc_window_close(popup) };
    }
    if open(opener) {
        unsafe { bc::bc_window_activate(opener) };
        // check.sh reads this line.
        log!("browser: little window {opener} has the keyboard back from {popup}");
    }
}

/// Where a Little ilo window opens: centred a little above the middle of the screen the pointer is
/// on, as Arc's, else of the one the windows open on.
pub(crate) fn little_frame() -> bc::bc_rect {
    let (width, height) = little_size();
    let screens = screens();
    let (mut x, mut y) = (0., 0.);
    let pointer = unsafe { platform::host_pointer(&mut x, &mut y) };
    let size = (width, height);
    let on_pointers = pointer.then(|| window_frames::on_screen_of((x, y), size, &screens));
    let frame = on_pointers.flatten().unwrap_or_else(|| {
        (x, y) = platform::centered(width, height);
        window_frames::raised(
            Frame {
                x,
                y,
                width,
                height,
            },
            &screens,
        )
    });
    bc::bc_rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
    }
}

/// Opens a Little ilo window from `from` for the user's Space `home`, in its profile, and returns
/// it.
pub(crate) fn open_little(from: BrowserWindow, home: bc::bc_id, cx: &mut App) -> Option<bc::bc_id> {
    let home = match home {
        0 => users_space_of(from.id),
        home => home,
    };
    let profile = unsafe { bc::bc_space_profile(home) };
    if profile == 0 {
        return None;
    }
    open_window(Some(from), Some(Own::Little { profile, home }), cx)
}

/// Opens each of `urls` in a Little ilo window of its own, for the user's Space `front` shows,
/// which comes to the front, as Arc opens a link from another app in Little Arc. A link one is
/// still open for brings that one to the front instead, as Arc's does.
pub(crate) fn open_links_in_little(front: bc::bc_id, urls: Vec<String>, cx: &mut App) {
    for url in urls {
        let open = WINDOWS.with_borrow(|windows| {
            let sent = |w: &&BrowserWindow| w.link.as_ref() == Some(&url);
            windows.iter().find(sent).map(|w| (w.id, w.ui_view))
        });
        if let Some((window, ui_view)) = open {
            // check.sh reads this line.
            log!("browser: link from another app {url}: little window {window} again");
            unsafe { platform::host_activate(ui_view) };
            continue;
        }
        let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == front).cloned());
        let Some(from) = from else {
            return;
        };
        // In the profile the user last picked for its site in Little ilo's bar, as Arc's Little
        // Arc does, in that profile's Space if the front window doesn't show one.
        let front_space = users_space_of(front);
        let home = match little_sites_host::profile(&url) {
            Some(profile) => {
                // check.sh reads this line.
                log!("browser: little ilo opens {url} in profile {profile}");
                home_in_profile(front_space, profile).unwrap_or(front_space)
            }
            None => front_space,
        };
        let Some(opened) = open_little(from, home, cx) else {
            continue;
        };
        let tab = open_tab_behind(opened, &url, false);
        if tab != 0 {
            unsafe { bc::bc_window_select(opened, tab) };
            crate::sign_in_close::link_tab_opened(opened, tab);
        }
        // check.sh reads this line.
        log!(
            "browser: link from another app {url}: tab {tab} in little window {opened} of space \
             {home}"
        );
        let ui_view = WINDOWS.with_borrow_mut(|windows| {
            let window = windows.iter_mut().find(|w| w.id == opened)?;
            window.link = Some(url);
            Some(window.ui_view)
        });
        if let Some(ui_view) = ui_view {
            unsafe { platform::host_activate(ui_view) };
        }
    }
}

/// The Little ilo windows open, as the Dock's menu opens: 0 with none, 1 with one on screen, 2
/// with all of them hidden.
pub(crate) extern "C" fn dock_little() -> c_int {
    if !unsafe { crate::bc::bc_little_windows() } {
        return -1;
    }
    WINDOWS.with_borrow(|windows| {
        let mut little = windows.iter().filter(|w| w.little.is_some()).peekable();
        if little.peek().is_none() {
            0
        } else if little.any(|w| unsafe { platform::host_window_shown(w.ui_view) }) {
            1
        } else {
            2
        }
    })
}

/// The Dock menu's item `item` was chosen: 0 New Little ilo Window, for the Space the regular
/// window in front shows, as a link from another app opens one; 1 Show All, the one used last in
/// front; 2 Hide All; 3 New Window and 4 New Incognito Window, which run File's from the regular
/// window in front, as ⌘N and ⇧⌘N there would.
pub(crate) extern "C" fn dock_chosen(item: c_int) {
    update_app_later(move |cx| {
        if item == 0 {
            return new_little_window("dock", cx);
        }
        if item == 3 || item == 4 {
            return new_window_in_front(item == 4, "dock", cx);
        }
        let shown = item == 1;
        let little: Vec<(bc::bc_id, *mut c_void)> = WINDOWS.with_borrow(|windows| {
            let little = windows.iter().filter(|w| w.little.is_some());
            little.map(|w| (w.id, w.ui_view)).collect()
        });
        // The one used last goes in front, last.
        for &(_, ui_view) in little.iter().rev() {
            unsafe { platform::host_set_window_shown(ui_view, shown) };
        }
        if let (true, Some(&(_, ui_view))) = (shown, little.first()) {
            unsafe { platform::host_activate(ui_view) };
        }
        let ids: Vec<String> = little.iter().map(|(id, _)| id.to_string()).collect();
        // check.sh reads this line.
        log!(
            "browser: dock {} little windows {}",
            if shown { "showed" } else { "hid" },
            ids.join(" ")
        );
    });
}

/// Opens a Little ilo window from outside the browser's windows, `from` the Dock's menu or the menu
/// bar's button, for the Space the regular window in front shows, as a link from another app opens
/// one, and puts it in front.
pub(crate) fn new_little_window(from: &str, cx: &mut App) {
    let front = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned());
    let Some(opened) = front.and_then(|front| open_little(front, 0, cx)) else {
        return;
    };
    // check.sh reads this line.
    log!("browser: {from} opened little window {opened}");
    let ui_view =
        WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == opened).map(|w| w.ui_view));
    if let Some(ui_view) = ui_view {
        unsafe { platform::host_activate(ui_view) };
    }
}

/// The menu bar's Little ilo button was clicked, or on Windows the notification area's icon.
pub(crate) extern "C" fn status_item_clicked() {
    let place =
        little_launch::button(little_launch::os()).map_or("menu bar", |button| button.place);
    update_app_later(move |cx| new_little_window(place, cx));
}

/// Where the choice to show Little ilo's button in the menu bar is kept, beside its size: there
/// while it's on. The app's, not a profile's, as the menu bar is.
pub(crate) fn little_in_menu_bar_path() -> String {
    format!("{}/little-in-menu-bar", profile_dir())
}

/// Whether the user turned on Little ilo's button in the menu bar, in Settings › General. Off on
/// a first launch, as Arc's is.
pub(crate) fn little_in_menu_bar() -> bool {
    std::path::Path::new(&little_in_menu_bar_path()).exists()
}

/// Puts Little ilo's button in the menu bar, or takes it out, and keeps the choice for the next
/// launch.
pub(crate) fn set_little_in_menu_bar(on: bool) {
    let path = little_in_menu_bar_path();
    _ = if on {
        std::fs::write(path, "")
    } else {
        std::fs::remove_file(path)
    };
    let clicked = (on && unsafe { crate::bc::bc_little_windows() })
        .then_some(status_item_clicked as extern "C" fn());
    unsafe { platform::host_set_little_status_item(clicked) };
}

/// Settings › Features' Little ilo switch: the core opens none while it's off, and every window's
/// menus, the command bar's rows and ⌥⌘-click on a row follow. The menu bar's button goes, its own
/// choice kept for when Little ilo is on again. The windows open stay.
pub(crate) fn set_little_windows(on: bool, cx: &mut gpui::App) {
    unsafe { crate::bc::bc_set_little_windows(on) };
    little_windows_shown(on, cx);
}

/// The menu bar's button and every window follow whether Little ilo is on, as the core has it now,
/// such as after the account brought it from another device.
pub(crate) fn little_windows_followed(cx: &mut gpui::App) {
    little_windows_shown(unsafe { crate::bc::bc_little_windows() }, cx);
}

fn little_windows_shown(on: bool, cx: &mut gpui::App) {
    let clicked = (on && little_in_menu_bar()).then_some(status_item_clicked as extern "C" fn());
    unsafe { platform::host_set_little_status_item(clicked) };
    crate::browser_windows::update_shells_in(cx, |shell, cx| shell.set_little_windows(on, cx));
}

/// ⌥⌘-click on a link in a page of `window`: `url` opens in a new Little ilo window, for the
/// Space `window` is for, as Arc's opens in Little Arc. `profile` is the page's, which the Space's
/// has.
pub(crate) unsafe extern "C" fn window_open_little(
    _: *mut c_void,
    window: bc::bc_id,
    url: *const c_char,
    profile: bc::bc_id,
) {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    update_app(|cx| {
        // A launch that asked for one, from the jump list or the launcher, names no window or link.
        if window == 0 && url.is_empty() {
            return new_little_window("command line", cx);
        }
        let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == window).cloned());
        let Some(from) = from else {
            return;
        };
        let home = from.little.unwrap_or_else(|| users_space_of(window));
        let Some(opened) = open_little(from, home, cx) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: link {url} in little window {opened} from {window} profile {profile}");
        open_tab(opened, &url);
    });
}

/// ⇧⌘T in `window` found a Little ilo window's tab closed last: it comes back in a new one for a
/// Space of `profile`, the one `window` is for if it's in it, as Arc's reopens it in Little Arc.
/// The user's Space a Little ilo window in `profile` is for: `own` if it's in that profile, else
/// the first of the user's Spaces that is, or None.
pub(crate) fn home_in_profile(own: bc::bc_id, profile: bc::bc_id) -> Option<bc::bc_id> {
    let users = |space: bc::bc_id| name(bc::bc_space_agent, space).is_empty();
    let in_profile = |space: bc::bc_id| unsafe { bc::bc_space_profile(space) } == profile;
    match own != 0 && in_profile(own) {
        true => Some(own),
        false => ids(bc::bc_spaces)
            .into_iter()
            .find(|&space| in_profile(space) && users(space)),
    }
}

pub(crate) unsafe extern "C" fn window_reopen_little(
    _: *mut c_void,
    window: bc::bc_id,
    closed: bc::bc_id,
    profile: bc::bc_id,
) {
    update_app(move |cx| {
        let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == window).cloned());
        let Some(from) = from else {
            return;
        };
        let own = from.little.unwrap_or_else(|| users_space_of(window));
        let Some(opened) =
            home_in_profile(own, profile).and_then(|home| open_little(from, home, cx))
        else {
            return;
        };
        let tab = unsafe { bc::bc_window_reopen_tab(opened, closed) };
        // check.sh reads this line.
        log!("browser: little window {opened} reopened {closed} as {tab}");
        if tab == 0 {
            unsafe { bc::bc_window_close(opened) };
        }
    });
}

/// Keeps the size of the Little ilo window of `ui_view` for the next one to open at, as Arc's
/// does. Out of full screen only, as for the launch frame.
pub(crate) fn save_little_size(ui_view: *mut c_void) {
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    if unsafe { platform::host_window_frame(ui_view, &mut x, &mut y, &mut width, &mut height) } {
        _ = std::fs::write(little_size_path(), format!("{width} {height}\n"));
    }
}
