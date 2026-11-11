//! Linux's notifications for the needs: the desktop's notification server,
//! org.freedesktop.Notifications, or in a Flatpak the portal's, which takes them to it. Each comes
//! back as `Clicked` with its need's id when the user clicks it.

use std::collections::HashMap;

use futures_lite::{Stream, StreamExt};
use zbus::zvariant::Value;

const SERVER: &str = "org.freedesktop.Notifications";
const SERVER_PATH: &str = "/org/freedesktop/Notifications";
const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_NOTIFICATION: &str = "org.freedesktop.portal.Notification";
/// The portal's default action, which isn't the app's own (`app.`), so its click comes back as
/// ActionInvoked rather than as a D-Bus activation the browser doesn't take.
const OPEN: &str = "open";
/// The Flatpak's desktop entry (branding/id), which main.rs names in CHROME_DESKTOP unless the
/// native install's launcher named its own.
const APP_ID: &str = include_str!("../../branding/id").trim_ascii();

/// The desktop entry the desktop knows ilo by, for the notification's icon and name: the one
/// CHROME_DESKTOP names, as the windows are, without its ".desktop".
fn desktop_entry() -> String {
    let named = std::env::var("CHROME_DESKTOP").unwrap_or_default();
    match named.strip_suffix(".desktop") {
        Some(entry) if !entry.is_empty() => entry.to_string(),
        _ => APP_ID.to_string(),
    }
}

/// The notifications' way to the desktop.
#[derive(Clone)]
pub struct Desktop {
    connection: zbus::Connection,
    portal: bool,
    /// The server's id for each notification, with its need's.
    posted: std::rc::Rc<std::cell::RefCell<Vec<(u32, u64)>>>,
}

impl Desktop {
    /// The session's, through the portal if `portal`, as in a Flatpak.
    pub async fn open(portal: bool) -> zbus::Result<(Desktop, impl Stream<Item = u64>)> {
        Desktop::on(zbus::Connection::session().await?, portal).await
    }

    pub async fn on(
        connection: zbus::Connection,
        portal: bool,
    ) -> zbus::Result<(Desktop, impl Stream<Item = u64>)> {
        let (interface, path) = match portal {
            true => (PORTAL_NOTIFICATION, PORTAL_PATH),
            false => (SERVER, SERVER_PATH),
        };
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(interface)?
            .path(path)?
            .member("ActionInvoked")?
            .build();
        let signals = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;
        let desktop = Desktop {
            connection,
            portal,
            posted: Default::default(),
        };
        let posted = desktop.posted.clone();
        let clicks = signals.filter_map(move |message| {
            let message = message.ok()?;
            let body = message.body();
            match portal {
                true => {
                    let (id, _, _): (String, String, Vec<zbus::zvariant::OwnedValue>) =
                        body.deserialize().ok()?;
                    id.strip_prefix("needs-you-")?.parse().ok()
                }
                false => {
                    let (server, _): (u32, String) = body.deserialize().ok()?;
                    let posted = posted.borrow();
                    (posted.iter())
                        .find(|(id, _)| *id == server)
                        .map(|(_, need)| *need)
                }
            }
        });
        Ok((desktop, clicks))
    }

    /// Posts need `id`'s notification, with no sound, at normal urgency.
    pub async fn notify(&self, id: u64, title: &str, body: &str) -> zbus::Result<()> {
        if self.portal {
            let notification: HashMap<&str, Value> = HashMap::from([
                ("title", Value::from(title)),
                ("body", Value::from(body)),
                ("default-action", Value::from(OPEN)),
            ]);
            let name = format!("needs-you-{id}");
            self.connection
                .call_method(
                    Some(PORTAL),
                    PORTAL_PATH,
                    Some(PORTAL_NOTIFICATION),
                    "AddNotification",
                    &(name.as_str(), notification),
                )
                .await?;
            return Ok(());
        }
        let entry = desktop_entry();
        let hints: HashMap<&str, Value> = HashMap::from([
            ("desktop-entry", Value::from(entry.as_str())),
            ("suppress-sound", Value::from(true)),
            ("urgency", Value::from(1u8)),
        ]);
        // "default" is the notification's own click, as a server names it.
        let actions = vec!["default", "Open"];
        let reply = self
            .connection
            .call_method(
                Some(SERVER),
                SERVER_PATH,
                Some(SERVER),
                "Notify",
                &(
                    env!("APP_NAME"),
                    0u32,
                    entry.as_str(),
                    title,
                    body,
                    actions,
                    hints,
                    -1i32,
                ),
            )
            .await?;
        let server: u32 = reply.body().deserialize()?;
        let mut posted = self.posted.borrow_mut();
        posted.push((server, id));
        if posted.len() > 64 {
            posted.remove(0);
        }
        Ok(())
    }
}
