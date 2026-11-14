//! The link preview's host, and the links dragged out of pages to a Space or a preview.

use super::*;

/// The link preview's requests, to the host's preview window and the core.
pub(crate) struct LinkPreviewHost {
    /// The view the host puts the preview's page in.
    pub(crate) holder: Cell<*mut c_void>,
    /// The browser window it shows over, until that window's page has the keyboard back.
    pub(crate) over: Cell<u64>,
}

impl link_preview::Host for LinkPreviewHost {
    fn place(&self, tab: u64, page: Bounds<Pixels>, radius: f32) {
        unsafe { bc::bc_tab_show_in(tab, self.holder.get(), rect(page), f64::from(radius)) };
    }

    fn show(&self, window: u64) {
        let ui_view = WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|each| each.id == window)
                .map(|each| each.ui_view)
        });
        if let Some(ui_view) = ui_view {
            self.over.set(window);
            unsafe { platform::host_show_link_preview(ui_view) };
        }
    }

    fn drew(&self, page: Bounds<Pixels>, radius: f32, opacity: f32) {
        let (left, top) = (f64::from(page.origin.x), f64::from(page.origin.y));
        let (width, height) = (f64::from(page.size.width), f64::from(page.size.height));
        let (radius, opacity) = (f64::from(radius), f64::from(opacity));
        unsafe { platform::host_link_preview_drew(left, top, width, height, radius, opacity) };
    }

    fn page_color(&self, window: u64, cx: &App) -> Option<gpui::Rgba> {
        let shell = WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|each| each.id == window)
                .map(|each| each.shell.clone())
        })?;
        let color = shell.read_with(cx, |shell, _| shell.page_color()).ok()??;
        Some(gpui::rgb(color))
    }

    fn page_under(&self) -> bool {
        // No window of the app's holds a page on Linux (link_preview_window.rs) or Windows
        // (platform/windows/host.cc).
        cfg!(any(target_os = "linux", target_os = "windows"))
    }

    fn release(&self) {
        unsafe { platform::host_release_link_preview() };
        // The window's page has the keyboard back from the preview's, which goes, or no page has
        // it, unless the user left for another window. Linux's host gives it as it releases;
        // Windows' can't name the window.
        let window = self.over.replace(0);
        if cfg!(target_os = "windows") && window != 0 && !LINK_PREVIEW_LEFT.get() {
            unsafe { bc::bc_window_focus_page(window) };
        }
    }

    fn hide(&self) {
        // Released as it left, unless it goes at once.
        self.release();
        unsafe { platform::host_hide_link_preview() };
    }

    fn close(&self, tab: u64) {
        unsafe { bc::bc_tab_close(tab, false) };
    }

    fn keep(&self, window: u64, tab: u64, archived: Option<u64>) {
        match archived {
            Some(archived) => unsafe { bc::bc_window_keep_archived_preview(window, tab, archived) },
            None => unsafe { bc::bc_window_keep_preview(window, tab) },
        }
    }

    fn kept_archived(&self, window: u64, archived: u64, cx: &mut App) {
        // check.sh waits for this line.
        log!("browser: restore archived {archived} from preview");
        let shell = WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|each| each.id == window)
                .map(|each| each.shell.clone())
        });
        if let Some(shell) = shell {
            shell
                .update(cx, |shell, cx| shell.restored_from_preview(cx))
                .ok();
        }
    }

    fn closed(&self, window: u64, closed: link_preview::Closed, undone: bool, cx: &mut App) {
        let shell = WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|each| each.id == window)
                .map(|each| each.shell.clone())
        });
        if let Some(shell) = shell {
            shell
                .update(cx, |shell, cx| shell.preview_closed(closed, undone, cx))
                .ok();
        }
    }

    fn tooltip(&self, tip: Option<(tooltip::Tip, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            // The preview opens from a click in the window in front, and its tooltips go over it.
            Some((tip, anchor)) => tooltip::hover(front().0, tip, anchor, cx),
            None => tooltip::dismiss(cx),
        }
    }
}

pub(crate) extern "C" fn link_preview_dismissed() {
    LINK_PREVIEW_LEFT.set(true);
    update_app(link_preview::dismissed);
    LINK_PREVIEW_LEFT.set(false);
}

thread_local! {
    /// Whether the host has a view for a link preview's page.
    pub(crate) static LINK_PREVIEWS: Cell<bool> = const { Cell::new(false) };
    /// The user left the preview for another window, which keeps the keyboard.
    pub(crate) static LINK_PREVIEW_LEFT: Cell<bool> = const { Cell::new(false) };
}

/// A link is dragged over `tab`'s page: the window showing it shows the split view's targets, and
/// takes the drag while it's on one, opening the link there when it's let go.
pub(crate) unsafe extern "C" fn tab_link_dragged(
    _: *mut c_void,
    tab: bc::bc_id,
    phase: c_int,
    url: *const c_char,
    x: f64,
    y: f64,
) -> bool {
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return false;
    };
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    let point = point(px(x as f32), px(y as f32));
    let Some(window) = cx.update(|cx| tab_window(tab, cx)) else {
        return false;
    };
    let answer = update_window(window.id, |shell, cx| match phase as u32 {
        bc::BC_LINK_DRAG_OVER => shell.link_over_page(tab, point, &url, cx),
        bc::BC_LINK_DRAG_DROP => shell.link_dropped(tab, point, cx),
        _ => {
            shell.link_left_page(cx);
            false
        }
    });
    answer.unwrap_or(false)
}

/// A link clicked in `tab`'s page leads to another site. A pinned tab's or a favorite's opens in
/// the link preview, over the window showing the tab, unless the user turned previews off or a link
/// rule sends it to another Space.
pub(crate) unsafe extern "C" fn tab_link_to_site(
    _: *mut c_void,
    tab: bc::bc_id,
    url: *const c_char,
    x: f64,
    y: f64,
) -> bool {
    if !LINK_PREVIEWS.get() || !unsafe { bc::bc_link_previews() } {
        return false;
    }
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return false;
    };
    let over = cx.update(|cx| {
        let window = tab_window(tab, cx)?;
        let shell = window.shell.upgrade()?;
        let shell = shell.read(cx);
        let page = shell
            .page_laid()
            .filter(|_| shell.shows(tab) && shell.previews_links_of(tab));
        Some((window.id, window.private, page?))
    });
    let Some((window, private, page)) = over else {
        return false;
    };
    let url = unsafe { CStr::from_ptr(url) }.to_owned();
    // A call's link opens as a tab of the Space shown, the one used last, as it outlasts a look.
    if link_preview::is_meeting(&url.to_string_lossy()) {
        // check.sh waits for this line.
        log!("browser: meeting link from {tab} opens as a tab");
        update_app_later(move |_| open_tab(window, &url.to_string_lossy()));
        return true;
    }
    // A link rule sends it to its Space instead, as one from another app, when that's another than
    // the one shown. Never from a private window, whose links stay off the record.
    let shown = unsafe { bc::bc_window_space(window) };
    let routed = match private {
        true => 0,
        false => link_rules_host::preview_route(&url.to_string_lossy(), shown),
    };
    if routed != 0 {
        update_app_later(move |_| {
            let url = url.to_string_lossy();
            let (to, opened, space) = open_link_in(window, &url, routed);
            // check.sh reads this line.
            log!("browser: link from {tab} {url}: tab {opened} in space {space} of window {to}");
            let ui_view = WINDOWS
                .with_borrow(|windows| windows.iter().find(|w| w.id == to).map(|w| w.ui_view));
            if let Some(ui_view) = ui_view.filter(|_| to != window) {
                unsafe { platform::host_activate(ui_view) };
            }
        });
        return true;
    }
    // Outside the navigation that asked, which the core cancels once this returns.
    update_app_later(move |cx| {
        let at = rect(link_preview::resting(page.0));
        let preview = unsafe { bc::bc_tab_open_preview(tab, url.as_ptr(), at) };
        if preview == 0 {
            log!("browser: link preview from {tab} not opened");
            return;
        }
        let link = page.0.origin + point(px(x as f32), px(y as f32));
        let url = url.to_string_lossy().into_owned().into();
        link_preview::show(window, preview, tab, url, page, link, None, cx);
    });
    true
}

/// The user asked for a link in `tab`'s page in a link preview, by ⇧-click or Open Link in Peek:
/// it opens over the window showing the tab, where the shell has one for it.
pub(crate) unsafe extern "C" fn tab_link_to_preview(
    _: *mut c_void,
    tab: bc::bc_id,
    url: *const c_char,
    x: f64,
    y: f64,
) -> bool {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned()
        .into();
    let at = point(px(x as f32), px(y as f32));
    update_tab_window(tab, |shell, cx| shell.preview_link(tab, url, at, cx)).unwrap_or(false)
}

/// A key from the link preview's page acts on the preview alone: done, and whether the page loses
/// it. None for another page's.
pub(crate) fn link_preview_key(
    tab: bc::bc_id,
    key: *const bc::bc_key,
    reserved: bool,
) -> Option<bool> {
    let window = unsafe { bc::bc_tab_window(tab) };
    let cx = CX.with_borrow(Clone::clone)?;
    // Read without an update, as every key a page gets comes here.
    let holds = cx.try_read_global(|preview: &link_preview::LinkPreviewWindow, cx| {
        preview.holds(window, tab, cx)
    });
    if holds != Some(true) {
        return None;
    }
    let keystroke = keystroke(unsafe { &*key });
    cx.update(|cx| link_preview::key(window, tab, &keystroke, reserved, cx))
}
