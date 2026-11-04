//! The needs' host (browser::needs_you): the count and the notifications through the platform
//! layer, and a notification's click, which brings the regular window in front forward and opens
//! the task's card there, as a click on the card does.

use super::*;

pub(crate) struct NeedsYou;

impl browser::needs_you::Host for NeedsYou {
    fn badge(&self, count: usize) {
        unsafe { platform::host_needs_you_badge(count.min(i32::MAX as usize) as c_int) };
    }

    fn active(&self) -> bool {
        unsafe { platform::host_app_active() }
    }

    fn notify(&self, id: u64, title: &str, body: &str) {
        #[cfg(not(target_os = "linux"))]
        {
            let (title, body) = (c_string(title), c_string(body));
            unsafe { platform::host_needs_you_notify(id, title.as_ptr(), body.as_ptr()) };
        }
        #[cfg(target_os = "linux")]
        {
            let (title, body) = (title.to_string(), body.to_string());
            update_app_later(move |cx| desktop::notify(id, title, body, cx));
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn c_string(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// Starts following the tasks' needs, with the clicks on their notifications.
pub(crate) fn start(cx: &mut App) {
    browser::needs_you::open(Rc::new(NeedsYou), cx);
    #[cfg(not(target_os = "linux"))]
    unsafe {
        platform::host_watch_needs_you(clicked)
    };
    #[cfg(target_os = "linux")]
    desktop::start(cx);
}

#[cfg(not(target_os = "linux"))]
extern "C" fn clicked(id: u64) {
    update_app_later(move |cx| open_clicked(id, cx));
}

/// Notification `id`'s click: its task's card, in the regular window in front, brought forward.
fn open_clicked(id: u64, cx: &mut App) {
    let Some(agent) = browser::needs_you::clicked(id, cx) else {
        return;
    };
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned())
    else {
        return;
    };
    unsafe { platform::host_activate(window.ui_view) };
    update_window_in(window.id, cx, |shell, cx| shell.show_needed_task(agent, cx));
}

/// Linux's, over D-Bus: the desktop's notification server, or the portal's in a Flatpak.
#[cfg(target_os = "linux")]
mod desktop {
    use std::cell::RefCell;

    use futures_lite::StreamExt;
    use gpui::App;

    use browser::log;
    use browser::needs_you::desktop::Desktop;

    thread_local! {
        static DESKTOP: RefCell<Option<Desktop>> = const { RefCell::new(None) };
    }

    pub(super) fn start(cx: &mut App) {
        cx.spawn(async move |cx| {
            let (desktop, mut clicks) = match Desktop::open(browser::flatpak::in_flatpak()).await {
                Ok(opened) => opened,
                Err(error) => {
                    // check.sh reads this line.
                    log!("browser: needs you has no notifications: {error}");
                    return;
                }
            };
            DESKTOP.set(Some(desktop));
            while let Some(id) = clicks.next().await {
                cx.update(|cx| super::open_clicked(id, cx));
            }
        })
        .detach();
    }

    pub(super) fn notify(id: u64, title: String, body: String, cx: &mut App) {
        let Some(desktop) = DESKTOP.with_borrow(Clone::clone) else {
            return;
        };
        cx.spawn(async move |_| {
            let posted = desktop.notify(id, &title, &body).await;
            // check.sh reads this line.
            match posted {
                Ok(()) => log!("browser: needs you posted {id}"),
                Err(error) => log!("browser: needs you posted {id}: {error}"),
            }
        })
        .detach();
    }
}
