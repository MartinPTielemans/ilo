//! Tabs: opening and splitting them, what the core says of their pages, and what's dragged in
//! and out of a window.

use super::*;

/// Opens a tab loading `url` at the top of the Today tabs of the Space `window` shows, and shows
/// it there.
pub(crate) fn open_tab(window: bc::bc_id, url: &str) {
    open_tab_typed(window, url, "");
}

/// As `open_tab`, for the address the user typed as `typed`: an http address written out loads as
/// written, as Chrome's address bar loads it (`bc_tab_open_typed`).
pub(crate) fn open_tab_typed(window: bc::bc_id, url: &str, typed: &str) {
    let tab = open_typed(window, url, typed, false);
    if tab != 0 {
        unsafe { bc::bc_window_select(window, tab) };
    }
}

/// Opens a tab loading `url` at the top of the Today tabs of the Space `window` shows, behind the
/// one shown, or with `when_shown` loading it only once it's first shown. Returns it, or 0.
pub(crate) fn open_tab_behind(window: bc::bc_id, url: &str, when_shown: bool) -> u64 {
    open_typed(window, url, "", when_shown)
}

/// As `open_tab_behind`, for the address the user typed as `typed`, as `open_tab_typed` reads it.
pub(crate) fn open_typed(window: bc::bc_id, url: &str, typed: &str, when_shown: bool) -> u64 {
    match (CString::new(url), CString::new(typed)) {
        (Ok(url), Ok(typed)) => unsafe {
            bc::bc_tab_open_typed(window, url.as_ptr(), when_shown, typed.as_ptr())
        },
        _ => 0,
    }
}

/// Puts `tab` in a split view with `with`, on its right, and shows it in `window`. The pane being
/// added for it, if there is one, gives the split its room first, so it shows at once at its place.
/// ⌘Z takes it back out, or closes it if it was `opened` for the split.
pub(crate) fn split_tab(window: bc::bc_id, tab: u64, with: u64, opened: bool, cx: &mut App) {
    shell::pane_added(tab, with, opened, cx);
    unsafe {
        bc::bc_tab_split_with(tab, with, false);
        bc::bc_window_select(window, tab);
    }
}

/// `tab`, opened to join a split view in `window`, joined it: that window's ⌘Z closes it.
pub(crate) fn split_opened(window: bc::bc_id, tab: u64, cx: &mut App) {
    if let Some(shell) = shell_of(window) {
        shell
            .update(cx, |shell, cx| shell.split_opened(tab, cx))
            .ok();
    }
}

/// The shell of browser window `window`, if it's open.
pub(crate) fn shell_of(window: bc::bc_id) -> Option<WeakEntity<Shell>> {
    WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|each| each.id == window)
            .map(|each| each.shell.clone())
    })
}

/// Opens a tab loading `url` in a split view with `with`, on its right, and shows it in `window`,
/// for the address the user typed as `typed`, as `open_tab_typed` reads it.
pub(crate) fn open_split(window: bc::bc_id, url: &str, typed: &str, with: u64, cx: &mut App) {
    let tab = open_typed(window, url, typed, false);
    if tab != 0 {
        split_tab(window, tab, with, true, cx);
    }
}

/// Makes a Space right after `after`, else after the others.
pub(crate) fn create_space(name: &str, after: bc::bc_id) -> bc::bc_id {
    let name = CString::new(name).unwrap_or_default();
    unsafe { bc::bc_space_create(name.as_ptr(), after) }
}

/// The URL the core's `get` copies for `text`.
pub(crate) fn url_for(
    text: &str,
    get: impl Fn(*const c_char, *mut c_char, usize) -> usize,
) -> Option<String> {
    let text = CString::new(text).ok()?;
    // Long enough for most, so the core reads what was typed once.
    let mut url = vec![0u8; 2048];
    let length = get(text.as_ptr(), url.as_mut_ptr().cast(), url.len());
    if length >= url.len() {
        url.resize(length + 1, 0);
        get(text.as_ptr(), url.as_mut_ptr().cast(), url.len());
    }
    url.truncate(length);
    (length > 0).then(|| String::from_utf8_lossy(&url).into_owned())
}

// The ⌃Tab walk and the file's drag were the user's in the window in front.
pub(crate) extern "C" fn control_released() {
    update_window(front().0, |shell, cx| shell.control_released(cx));
}

pub(crate) extern "C" fn file_preview(
    download: u64,
    png: *const u8,
    size: usize,
    width: f64,
    height: f64,
    image: bool,
) {
    let preview = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        let png = Arc::new(Image::from_bytes(ImageFormat::Png, png));
        FilePreview {
            image: png,
            width: width as f32,
            height: height as f32,
            photo: image,
        }
    });
    let kind = match preview {
        Some(_) if image => "picture",
        Some(_) => "icon",
        None => "none",
    };
    log!("browser: download {download} preview {kind} {width}x{height}");
    update_shells(|shell, cx| shell.set_download_preview(download, preview.clone(), cx));
}

pub(crate) extern "C" fn file_drag_ended(taken: bool) {
    update_window(front().0, |shell, cx| shell.file_drag_ended(taken, cx));
}

/// The system's drag of `tab` out of a window ended, `how` host_drag_tab says, let go at (`x`, `y`)
/// on the screen where `placed`.
pub(crate) extern "C" fn tab_drag_ended(tab: u64, how: c_int, placed: bool, x: f64, y: f64) {
    let ended = match how {
        platform::TAB_TAKEN => shell::WindowDragEnd::Taken,
        platform::TAB_LET_GO => {
            shell::WindowDragEnd::LetGo(placed.then(|| point(x as f32, y as f32)))
        }
        _ => shell::WindowDragEnd::Cancelled,
    };
    update_shells(|shell, cx| shell.window_drag_ended(tab, ended, cx));
}

pub(crate) extern "C" fn dragged(
    ui_view: *mut c_void,
    phase: c_int,
    x: f64,
    y: f64,
    items: *const platform::host_dragged,
    count: usize,
) -> bool {
    let items = match items.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(items, count) },
    };
    let position = point(px(x as f32), px(y as f32));
    // A tab dragged out of one of the app's windows (host_drag_tab).
    if let Some(tab) = items.iter().map(|item| item.tab).find(|&tab| tab != 0) {
        let window = window_with_view(ui_view);
        let answer = update_window(window, |shell, cx| match phase {
            platform::DRAG_OVER => shell.drag_tab_in(position, tab, cx),
            platform::DRAG_DROP => shell.drop_tab_in(position, tab, cx),
            _ => {
                shell.drag_away(cx);
                false
            }
        });
        return answer.unwrap_or(false);
    }
    let items: Vec<_> = items
        .iter()
        .map(|item| {
            (
                unsafe { CStr::from_ptr(item.url) }.to_string_lossy(),
                item.file,
            )
        })
        .collect();
    let items = items.iter().map(|(url, file)| (url.as_ref(), *file));
    let window = window_with_view(ui_view);
    let search = |text: &str| {
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_search_url(window, std::ptr::null(), text, url, capacity)
        })
    };
    let urls = shell::dropped_urls(items, search);
    let answer = update_window(window, |shell, cx| match phase {
        platform::DRAG_OVER => !urls.is_empty() && shell.drag_in(position, cx),
        platform::DRAG_DROP => shell.drop_in(position, &urls, cx),
        _ => {
            shell.drag_away(cx);
            false
        }
    });
    answer.unwrap_or(false)
}

pub(crate) unsafe extern "C" fn window_selected(_: *mut c_void, window: bc::bc_id, tab: bc::bc_id) {
    // check.sh reads this line, as a tab's page moves between windows.
    log!("browser: window {window} shows {tab}");
    browser::log_handover(
        "window shows the tab",
        &format!("window {window}, tab {tab}"),
    );
    update_window(window, |shell, cx| {
        shell.set_selected(tab, cx);
        find_bar::tab_shown(window, tab, cx);
        zoom_hud::tab_shown(tab, cx);
        offer::tab_shown(window, tab, cx);
    });
}

pub(crate) unsafe extern "C" fn window_page_elsewhere(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
) {
    // check.sh reads this line.
    log!("browser: window {window} page elsewhere {tab}");
    let (stage, detail) = match tab {
        0 => ("taker told", format!("window {window}")),
        _ => ("giver told", format!("window {window}, tab {tab}")),
    };
    browser::log_handover(stage, &detail);
    update_window(window, |shell, cx| shell.page_elsewhere(tab, cx));
    update_app(|cx| offer::page_elsewhere(window, tab != 0, cx));
}

// check.sh reads these lines. Test pages report what reached them through their title.
pub(crate) unsafe extern "C" fn tab_loaded(_: *mut c_void, tab: bc::bc_id, url: *const c_char) {
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy();
    log!("browser: loaded {tab} {url}");
    let owned = url.to_string();
    update_app_later(move |cx| crate::sign_in_close::tab_loaded(tab, &owned, cx));
    sample_foot_bar(tab);
}

/// check.sh's foot-bar run: BROWSER_FOOT_BAR=arrange, =point or =long shows the foot bar with made-up
/// words (`foot_bar::sample`) over the window of the first page that loads, as a mode will, so the
/// host's window for it shows on each OS before a mode uses it.
fn sample_foot_bar(tab: bc::bc_id) {
    static KIND: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    thread_local!(static SHOWN: Cell<bool> = const { Cell::new(false) });
    let Some(kind) = KIND.get_or_init(|| std::env::var("BROWSER_FOOT_BAR").ok()) else {
        return;
    };
    let Some(bar) = browser::foot_bar::sample(kind).filter(|_| !SHOWN.replace(true)) else {
        return;
    };
    update_app(move |cx| {
        let Some(window) = tab_window(tab, cx) else {
            return;
        };
        let Some(shell) = window.shell.upgrade() else {
            return;
        };
        let Ok(content) = window
            .handle
            .update(cx, |_, window, _| window.viewport_size())
        else {
            return;
        };
        let page = shell.read(cx).page_frame(content).0;
        let id = window.id;
        with_overlays(
            move |cx| {
                browser::foot_bar::show(id, page, content, bar, cx);
                log!("browser: foot bar shown over {id}");
            },
            cx,
        );
    });
}

pub(crate) unsafe extern "C" fn tab_state_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_tab_state,
) {
    let state = unsafe { &*state };
    let page = Page {
        url: unsafe { CStr::from_ptr(state.url) }
            .to_string_lossy()
            .into_owned()
            .into(),
        can_go_back: state.can_go_back,
        can_go_forward: state.can_go_forward,
        loading: state.loading,
        unloaded: state.unloaded,
        away_from_pinned_url: state.away_from_pinned_url,
        audible: state.audible,
        muted: state.muted,
        volume: Volume(state.volume as f32),
        icon: unsafe { CStr::from_ptr(state.icon) }
            .to_string_lossy()
            .into_owned()
            .into(),
        trouble: match state.trouble as u32 {
            bc::BC_PAGE_CRASHED => Trouble::Crashed,
            bc::BC_PAGE_HUNG => Trouble::Hung,
            bc::BC_PAGE_NETWORK_ERROR => Trouble::NetworkError(state.net_error),
            _ => Trouble::Fine,
        },
        system_engine: state.system_engine,
        camera: state.camera,
        microphone: state.microphone,
        sharing: state.sharing,
        shared: state.shared,
        share_instead: state.share_instead,
        media: state.media,
        media_playing: state.media_playing,
        media_title: unsafe { CStr::from_ptr(state.media_title) }
            .to_string_lossy()
            .into_owned()
            .into(),
        media_artist: unsafe { CStr::from_ptr(state.media_artist) }
            .to_string_lossy()
            .into_owned()
            .into(),
        media_previous: state.media_previous,
        media_next: state.media_next,
        media_picture_in_picture: state.media_picture_in_picture,
        media_video: state.media_video,
        developer_mode: state.developer_mode,
    };
    if unsafe { bc::bc_tab_window(tab) } == 0 {
        update_app(|cx| link_preview::set_url(tab, page.url.clone(), cx));
    }
    update_shells(|shell, cx| shell.set_page(tab, page.clone(), cx));
}

pub(crate) unsafe extern "C" fn tab_title_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    title: *const c_char,
) {
    let title = unsafe { CStr::from_ptr(title) }
        .to_string_lossy()
        .into_owned();
    log!("browser: title {tab} {title}");
    update_shells(|shell, cx| shell.set_title(tab, title.clone().into(), cx));
}

pub(crate) unsafe extern "C" fn tab_favicon_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    png: *const u8,
    size: usize,
) {
    let favicon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    log!("browser: favicon {tab} {size} bytes");
    // A tab of no window's is the link preview's, which shows its site's.
    if unsafe { bc::bc_tab_window(tab) } == 0 {
        update_app(|cx| link_preview::set_favicon(tab, favicon.clone(), cx));
    }
    update_shells(|shell, cx| shell.set_favicon(tab, favicon.clone(), cx));
}

pub(crate) unsafe extern "C" fn tab_history(
    _: *mut c_void,
    tab: bc::bc_id,
    forward: bool,
    entries: *const bc::bc_history_entry,
    count: usize,
) {
    let entries = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(entries, count) },
    };
    let text = |text: *const c_char| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    };
    let pages: Vec<_> = (entries.iter())
        .map(|entry| shell::HistoryPage {
            offset: entry.offset,
            url: text(entry.url).into(),
            title: text(entry.title).into(),
            favicon: (!entry.favicon.is_null()).then(|| {
                let png = unsafe { std::slice::from_raw_parts(entry.favicon, entry.favicon_size) };
                Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
            }),
        })
        .collect();
    let way = if forward { "forward" } else { "back" };
    log!("browser: history {tab} {way} {}", pages.len());
    update_shells(|shell, cx| shell.show_history(tab, forward, pages.clone(), cx));
}

pub(crate) unsafe extern "C" fn page_favicon(
    _: *mut c_void,
    url: *const c_char,
    png: *const u8,
    size: usize,
) {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    let favicon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    update_app(|cx| command_bar::page_favicon(&url, favicon.clone(), cx));
    update_shells(|shell, cx| shell.set_page_favicon(url.clone().into(), favicon.clone(), cx));
}

pub(crate) unsafe extern "C" fn tab_opened(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
    opener: bc::bc_id,
    foreground: bool,
) {
    let side = if foreground {
        "foreground"
    } else {
        "background"
    };
    log!("browser: tab {tab} opened by {opener} in the {side}");
    if opener != 0 {
        agents_core::update(move |agents, cx| agents.tab_opened(tab, opener, cx));
    }
    let little = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|each| each.id == window)
            .and_then(|each| each.little)
    });
    if let Some(home) = little.filter(|_| opener != 0) {
        match take_popup(tab) {
            Some(asked) => {
                update_app_later(move |cx| little_popup_opened(window, tab, home, asked, cx))
            }
            None => update_app_later(move |cx| little_tab_opened(window, tab, home, cx)),
        }
        return;
    }
    // A pop-up elsewhere is a tab of its window, as decision 3 has it.
    take_popup(tab);
    // A link ⌥⌘-clicked, or opened in a split view from the page's menu, joined its page's split,
    // which ⌘Z takes back as it does the user's other ways in.
    let mut split = [0; 3];
    let count = unsafe { bc::bc_tab_split(tab, split.as_mut_ptr(), split.len()) };
    if opener != 0 && count > 1 && split[..count.min(3)].contains(&opener) {
        update_app_later(move |cx| split_opened(window, tab, cx));
    }
}

// Called while Chromium closes the tab, which can be inside a GPUI update that asked for it, so
// the shell hears of it from space_changed instead, and Site Controls once that update is over.
pub(crate) unsafe extern "C" fn tab_closed(_: *mut c_void, tab: bc::bc_id) {
    log!("browser: tab {tab} closed");
    find_bar::forget(tab);
    offer::forget(tab);
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let executor = cx.foreground_executor().clone();
        executor
            .spawn(async move {
                // Outside the update, as closing the pop-up's window calls back into GPUI.
                popup_closed(tab);
                crate::sign_in_close::tab_closed(tab);
                cx.update(|cx| {
                    site_controls::tab_closed(tab, cx);
                    capture::tab_closed(tab, cx);
                    link_preview::tab_closed(tab, cx);
                    browser::agents::tab_closed(tab, cx);
                })
            })
            .detach();
    }
}

pub(crate) unsafe extern "C" fn tab_find_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_find_state,
) {
    let state = unsafe { &*state };
    let find = find_bar::Find {
        open: state.open,
        text: unsafe { CStr::from_ptr(state.text) }
            .to_string_lossy()
            .into_owned(),
        matches: state.matches,
        selected: state.selected,
    };
    update_app(|cx| find_bar::changed(tab, find, cx));
}

pub(crate) unsafe extern "C" fn tab_fullscreen_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    fullscreen: bool,
) {
    // check.sh times the steps from the core's "asked" line to the window's and the UI's.
    log!(
        "browser: full screen {} told at {:.1} ms",
        if fullscreen { "enter" } else { "exit" },
        browser::uptime_ms()
    );
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return;
    };
    cx.update(|cx| {
        let Some(BrowserWindow {
            id, shell, handle, ..
        }) = tab_window(tab, cx)
        else {
            return;
        };
        // A page leaves full screen as its tab is switched away from or closes, and only one page
        // is full screen at a time.
        let shown = shell
            .read_with(cx, |shell, _| shell.selected() == tab)
            .unwrap_or(false);
        if fullscreen && !shown {
            return;
        }
        // Told before AppKit resizes a window going full screen with the page, or back, so the
        // resize lays the page out once, at its new size. Otherwise it's laid out now.
        cx.update_window(handle, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.set_page_fullscreen(fullscreen, window, cx);
                let viewport = window.viewport_size();
                if let Some(frame) = shell.window_page_frame(viewport) {
                    set_frames(id, shell, frame, viewport);
                }
            })
        })
        .ok();
    });
    log!(
        "browser: page {} full screen",
        if fullscreen { "entered" } else { "left" }
    );
}
