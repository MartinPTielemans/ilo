//! Linux's Reduce Motion: the desktop's own setting, read through the settings portal, which a
//! Flatpak reaches too, as the Mac's and Windows' hosts read theirs (host_reduce_motion). GNOME
//! keeps it as animations turned off, KDE as an animation speed of 0. A desktop with neither, such
//! as sway's or Hyprland's, has GTK's settings file say it, with gtk-enable-animations; with none
//! of those, the motion stays on.

use std::path::PathBuf;

use gpui::App;
// zbus's, as this crate has no futures of its own.
use zbus::export::futures_core::Stream;
use zbus::zvariant::OwnedValue;

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";
const GNOME: (&str, &str) = ("org.gnome.desktop.interface", "enable-animations");
const KDE: (&str, &str) = ("org.kde.kdeglobals.KDE", "AnimationDurationFactor");

/// Reads the setting away from the UI and gives GPUI the answer once it comes.
pub fn read(cx: &mut App) {
    cx.spawn(async move |cx| {
        if let Some(reduce) = reduced().await {
            cx.update(|cx| crate::set_reduce_motion(reduce, cx));
        }
    })
    .detach();
}

/// Reads it again each time the portal says the desktop's changed. The wait is the session bus's,
/// woken only by a change.
pub fn watch(cx: &mut App) {
    cx.spawn(async move |cx| {
        let Ok(connection) = zbus::Connection::session().await else {
            return;
        };
        let Ok(proxy) = zbus::Proxy::new(&connection, PORTAL, PATH, SETTINGS).await else {
            return;
        };
        let Ok(changes) = proxy.receive_signal("SettingChanged").await else {
            return;
        };
        let mut changes = std::pin::pin!(changes);
        while let Some(change) = std::future::poll_fn(|cx| changes.as_mut().poll_next(cx)).await {
            let said = change.body().deserialize::<(String, String, OwnedValue)>();
            let ours = said.is_ok_and(|(namespace, key, _)| {
                [GNOME, KDE].contains(&(namespace.as_str(), key.as_str()))
            });
            if ours && let Some(reduce) = reduced().await {
                cx.update(|cx| crate::set_reduce_motion(reduce, cx));
            }
        }
    })
    .detach();
}

async fn reduced() -> Option<bool> {
    match from_portal().await {
        Some(reduce) => Some(reduce),
        None => from_gtk_settings(),
    }
}

async fn from_portal() -> Option<bool> {
    let connection = zbus::Connection::session().await.ok()?;
    let read = async |(namespace, key): (&str, &str)| -> Option<OwnedValue> {
        let body = &(namespace, key);
        let reply = connection.call_method(Some(PORTAL), PATH, Some(SETTINGS), "ReadOne", body);
        reply.await.ok()?.body().deserialize::<OwnedValue>().ok()
    };
    if let Some(on) = read(GNOME).await {
        return bool::try_from(on).ok().map(|on| !on);
    }
    let speed = read(KDE).await?;
    f64::try_from(speed).ok().map(|factor| factor == 0.)
}

/// GTK 4's settings file, else GTK 3's, as a GTK app reads them.
fn from_gtk_settings() -> Option<bool> {
    let home = || std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"));
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(home)?;
    ["gtk-4.0", "gtk-3.0"].into_iter().find_map(|gtk| {
        let settings = std::fs::read_to_string(config.join(gtk).join("settings.ini")).ok()?;
        animations_off(&settings)
    })
}

/// What a settings file's gtk-enable-animations says, in its [Settings] group, as GLib reads a
/// key file's true and false.
fn animations_off(settings: &str) -> Option<bool> {
    let mut group = "";
    for line in settings.lines().map(str::trim) {
        if line.starts_with('[') {
            group = line;
        } else if group == "[Settings]"
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "gtk-enable-animations"
        {
            return match value.trim() {
                "false" | "0" => Some(true),
                "true" | "1" => Some(false),
                _ => None,
            };
        }
    }
    None
}
