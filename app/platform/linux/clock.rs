//! Linux's 12- or 24-hour clock, as the Mac's and Windows' hosts read theirs
//! (host_twelve_hour_clock): GNOME's clock-format, through the settings portal, which a Flatpak
//! reaches too. A desktop without it, such as KDE's, sway's or Hyprland's, shows times as its
//! locale's time format does.

use std::ffi::CStr;

use gpui::App;
use zbus::zvariant::OwnedValue;

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";

/// Reads the setting away from the UI and gives the app the answer once it comes.
pub fn read(cx: &mut App) {
    cx.spawn(async move |_| {
        let twelve = match from_portal().await {
            Some(twelve) => twelve,
            None => from_locale(),
        };
        browser::clock::set_twelve_hour(twelve);
    })
    .detach();
}

async fn from_portal() -> Option<bool> {
    let connection = zbus::Connection::session().await.ok()?;
    let body = &("org.gnome.desktop.interface", "clock-format");
    let reply = connection.call_method(Some(PORTAL), PATH, Some(SETTINGS), "ReadOne", body);
    let format = reply.await.ok()?.body().deserialize::<OwnedValue>().ok()?;
    match String::try_from(format).ok()?.as_str() {
        "12h" => Some(true),
        "24h" => Some(false),
        _ => None,
    }
}

/// The time format of the locale LC_TIME, LC_ALL or LANG names, as a C program's strftime("%X")
/// would show it.
fn from_locale() -> bool {
    // SAFETY: the locale is this function's own, freed once its format is read.
    unsafe {
        let locale = libc::newlocale(libc::LC_TIME_MASK, c"".as_ptr(), std::ptr::null_mut());
        if locale.is_null() {
            return false;
        }
        let format = libc::nl_langinfo_l(libc::T_FMT, locale);
        let twelve = !format.is_null()
            && browser::clock::twelve_hour_format(&CStr::from_ptr(format).to_string_lossy());
        libc::freelocale(locale);
        twelve
    }
}
