//! AppleScript's way in (scripting.mm, ilo.sdef, #704): what the Mac's host asks for a script, which
//! the app answers from what each window's sidebar shows, and the commands, which go through the
//! UI's own paths, as a click's do: Shell's for a Space or a tab shown, ⌘N's window, ⌘T's tab, a
//! link from another app's. Settings › Agents' switch lets scripts in, and the Agents log each
//! Apple Event once (agents/scripts.rs).

use super::*;
use browser::agents::scripts;

/// Copies `text` into `out` as bc_space_name copies: up to `capacity` less one bytes, then a nul,
/// and returns its whole length.
fn copy_out(text: &str, out: *mut c_char, capacity: usize) -> usize {
    if !out.is_null() && capacity > 0 {
        let bytes = text.as_bytes();
        let length = bytes.len().min(capacity - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, length);
            *out.add(length) = 0;
        }
    }
    text.len()
}

fn ids_out(ids: &[u64], out: *mut u64, capacity: usize) -> usize {
    if !out.is_null() {
        let length = ids.len().min(capacity);
        unsafe { std::ptr::copy_nonoverlapping(ids.as_ptr(), out, length) };
    }
    ids.len()
}

fn text_in(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Logs the Apple Event `event`, refused for `refused` if that isn't empty, and says why it may
/// not go ahead, if it may not: the switch off, else `refused`.
fn logged(
    event: u64,
    sender: *const c_char,
    action: &str,
    site: &str,
    refused: &str,
) -> Result<(), String> {
    let by = text_in(sender);
    let mut why = Some(scripts::OFF.to_string());
    update_app(|cx| {
        if let Some(agents) = browser::agents::global(cx) {
            why = agents.update(cx, |agents, cx| {
                let on = agents.scripts_on();
                let allowed = agents.script(event, &by, action, site, refused, cx);
                match (allowed, on) {
                    (true, _) => None,
                    (false, false) => Some(scripts::OFF.to_string()),
                    (false, true) => Some(refused.to_string()),
                }
            });
        }
    });
    match why {
        None => Ok(()),
        Some(why) => Err(refusal(&why)),
    }
}

/// The regular window in front, whose Spaces every regular window lists, not a private one's.
fn front_regular() -> u64 {
    WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .find(|window| window.regular())
            .map_or(0, |window| window.id)
    })
}

/// What the front regular window's shell says, as every one lists the same Spaces and tabs.
fn read_front<R: Default>(read: impl FnOnce(&mut Shell) -> R) -> R {
    update_window(front_regular(), |shell, _| read(shell)).unwrap_or_default()
}

/// Whether `tab` is one of the user's Spaces' in `shell`, which a script may read.
fn users_tab(shell: &Shell, tab: u64) -> bool {
    (shell.script_spaces().into_iter()).any(|space| shell.script_tabs(space).contains(&tab))
}

extern "C" fn allowed(event: u64, sender: *const c_char, action: *const c_char) -> bool {
    logged(event, sender, &text_in(action), "", "").is_ok()
}

extern "C" fn windows(out: *mut u64, capacity: usize) -> usize {
    // The core's, front to back, which leaves private windows out, of those the app has.
    let open: Vec<u64> = WINDOWS.with_borrow(|windows| {
        (windows.iter())
            .filter(|window| !window.private)
            .map(|window| window.id)
            .collect()
    });
    let front_to_back: Vec<u64> = (crate::ids(bc::bc_windows).into_iter())
        .filter(|window| open.contains(window))
        .collect();
    ids_out(&front_to_back, out, capacity)
}

extern "C" fn window_space(window: u64) -> u64 {
    update_window(window, |shell, _| shell.script_space()).unwrap_or(0)
}

extern "C" fn window_tab(window: u64) -> u64 {
    let tab = unsafe { bc::bc_window_selected(window) };
    update_window(window, |shell, _| users_tab(shell, tab).then_some(tab))
        .flatten()
        .unwrap_or(0)
}

extern "C" fn window_name(window: u64, out: *mut c_char, capacity: usize) -> usize {
    let tab = window_tab(window);
    let name = match tab {
        0 => String::new(),
        tab => update_window(window, |shell, _| shell.title(tab).to_string()).unwrap_or_default(),
    };
    copy_out(&name, out, capacity)
}

extern "C" fn spaces(out: *mut u64, capacity: usize) -> usize {
    ids_out(&read_front(|shell| shell.script_spaces()), out, capacity)
}

extern "C" fn space_name(space: u64, out: *mut c_char, capacity: usize) -> usize {
    let name = read_front(|shell| shell.script_space_name(space).map(|name| name.to_string()));
    copy_out(&name.unwrap_or_default(), out, capacity)
}

extern "C" fn tabs(window: u64, space: u64, out: *mut u64, capacity: usize) -> usize {
    let tabs = update_window(window, |shell, _| shell.script_tabs(space)).unwrap_or_default();
    ids_out(&tabs, out, capacity)
}

extern "C" fn tab_title(tab: u64, out: *mut c_char, capacity: usize) -> usize {
    let title = read_front(|shell| users_tab(shell, tab).then(|| shell.title(tab).to_string()));
    copy_out(&title.unwrap_or_default(), out, capacity)
}

extern "C" fn tab_url(tab: u64, out: *mut c_char, capacity: usize) -> usize {
    let url = read_front(|shell| users_tab(shell, tab).then(|| shell.page(tab).url.to_string()));
    copy_out(&url.unwrap_or_default(), out, capacity)
}

extern "C" fn focus_space(
    event: u64,
    sender: *const c_char,
    window: u64,
    space: u64,
    why: *mut c_char,
    capacity: usize,
) -> bool {
    let name = update_window(window, |shell, _| shell.script_space_name(space)).flatten();
    let refused = match name {
        Some(_) => "",
        None => "no such Space",
    };
    let name = name.map(|name| name.to_string()).unwrap_or_default();
    if let Err(refused) = logged(event, sender, scripts::SHOW_SPACE, &name, refused) {
        copy_out(&refused, why, capacity);
        return false;
    }
    update_window(window, |shell, cx| shell.script_show_space(space, cx)).unwrap_or(false)
}

extern "C" fn select_tab(
    event: u64,
    sender: *const c_char,
    window: u64,
    tab: u64,
    why: *mut c_char,
    capacity: usize,
) -> bool {
    let title = update_window(window, |shell, _| {
        users_tab(shell, tab).then(|| shell.title(tab).to_string())
    })
    .flatten();
    let refused = match title {
        Some(_) => "",
        None => "no such tab",
    };
    if let Err(refused) = logged(
        event,
        sender,
        scripts::SHOW_TAB,
        &title.unwrap_or_default(),
        refused,
    ) {
        copy_out(&refused, why, capacity);
        return false;
    }
    update_window(window, |shell, cx| shell.script_select(tab, cx)).unwrap_or(false)
}

extern "C" fn new_window(
    event: u64,
    sender: *const c_char,
    why: *mut c_char,
    capacity: usize,
) -> u64 {
    if let Err(refused) = logged(event, sender, scripts::NEW_WINDOW, "", "") {
        copy_out(&refused, why, capacity);
        return 0;
    }
    let mut opened = 0;
    // As ⌘N's: from the regular window in front, showing its Space with no page.
    update_app(|cx| opened = open_window_after(front_regular(), cx).unwrap_or(0));
    opened
}

extern "C" fn new_tab(
    event: u64,
    sender: *const c_char,
    window: u64,
    space: u64,
    url: *const c_char,
    why: *mut c_char,
    capacity: usize,
) -> u64 {
    let url = text_in(url);
    let page = scripts::page_allowed(&url);
    let site = match page {
        true => site_of(&url),
        false => String::new(),
    };
    let refused = match page {
        true => "",
        false => "only an http or https page",
    };
    if let Err(refused) = logged(event, sender, scripts::NEW_TAB, &site, refused) {
        copy_out(&refused, why, capacity);
        return 0;
    }
    // At the application: as a link from another app, which Settings › Links sends where its
    // rules say, by default to Little ilo, which opens once this is over.
    if window == 0 {
        let shown = open_links_in(front().0, &[url]);
        return unsafe { bc::bc_window_selected(shown) };
    }
    if space != 0
        && !update_window(window, |shell, cx| shell.script_show_space(space, cx)).unwrap_or(false)
    {
        return 0;
    }
    // As ⌘T's tab with the address typed: at the top of Today in the Space shown, shown.
    let tab = open_tab_behind(window, &url, false);
    if tab != 0 {
        unsafe { bc::bc_window_select(window, tab) };
    }
    tab
}

/// Why a command was refused, as the script's error says it.
fn refusal(refused: &str) -> String {
    match refused {
        "" | scripts::OFF => "Scripts are off in ilo's Settings › Agents.".into(),
        why => {
            let mut why = why.to_string();
            why[..1].make_ascii_uppercase();
            format!("{why}.")
        }
    }
}

/// The host of `url`, as the log names a page.
fn site_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

static SCRIPTING: platform::host_scripting = platform::host_scripting {
    allowed,
    windows,
    window_space,
    window_tab,
    window_name,
    spaces,
    space_name,
    tabs,
    tab_title,
    tab_url,
    focus_space,
    select_tab,
    new_window,
    new_tab,
};

/// Answers scripts, on the Mac, from now on.
pub(crate) fn answer_scripts() {
    #[cfg(target_os = "macos")]
    unsafe {
        platform::host_set_scripting(&SCRIPTING)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = &SCRIPTING;
}
