//! The hold on Linux (browser::awake), over D-Bus. The desktop's own, through the portal's Inhibit
//! with its Suspend flag: GNOME's and KDE's idle suspend read the session's inhibitors, and the
//! portal answers in a Flatpak too. Where no portal has one, logind's "idle" lock, which its
//! IdleAction reads and `systemd-inhibit --list` lists. Neither stops a closed lid or a suspend the
//! user asks for, and the display still blanks.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gpui::App;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, Value};

use browser::awake::REASON;
use browser::log;

/// The portal's Inhibit flag for a suspend.
const SUSPEND: u32 = 4;

/// A hold taken: the portal's request, which closing ends, or logind's lock, a file descriptor
/// that closing lets go.
enum Lock {
    Portal(zbus::Connection, OwnedObjectPath),
    Logind(OwnedFd),
}

thread_local! {
    /// What the Agents last asked for, the lock held for it, and whether one is being taken.
    static WANTED: Cell<bool> = const { Cell::new(false) };
    static TAKING: Cell<bool> = const { Cell::new(false) };
    static LOCK: RefCell<Option<Lock>> = const { RefCell::new(None) };
}

/// Takes the hold with `on`, or lets it go. A lock that comes after it was let go meanwhile goes
/// at once.
pub(crate) fn hold(on: bool, cx: &mut App) {
    WANTED.set(on);
    if !on {
        if let Some(lock) = LOCK.take() {
            let_go(lock, cx);
        }
        return;
    }
    if LOCK.with_borrow(Option::is_some) || TAKING.replace(true) {
        return;
    }
    cx.spawn(async move |cx| {
        let taken = take().await;
        TAKING.set(false);
        match taken {
            Ok(lock) if WANTED.get() => {
                log!("browser: awake lock {}", lock.name());
                LOCK.set(Some(lock));
            }
            Ok(lock) => cx.update(|cx| let_go(lock, cx)),
            Err(error) => log!("browser: awake has no lock: {error}"),
        }
    })
    .detach();
}

impl Lock {
    fn name(&self) -> &'static str {
        match self {
            Lock::Portal(..) => "portal",
            Lock::Logind(_) => "logind",
        }
    }
}

async fn take() -> zbus::Result<Lock> {
    match portal().await {
        Ok(lock) => Ok(lock),
        Err(_) => logind().await,
    }
}

async fn portal() -> zbus::Result<Lock> {
    let bus = zbus::Connection::session().await?;
    let options: HashMap<&str, Value> = HashMap::from([("reason", Value::from(REASON))]);
    let reply = bus
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Inhibit"),
            "Inhibit",
            &("", SUSPEND, options),
        )
        .await?;
    let request: OwnedObjectPath = reply.body().deserialize()?;
    Ok(Lock::Portal(bus, request))
}

async fn logind() -> zbus::Result<Lock> {
    let bus = zbus::Connection::system().await?;
    let reply = bus
        .call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "Inhibit",
            &("idle", env!("APP_NAME"), REASON, "block"),
        )
        .await?;
    Ok(Lock::Logind(reply.body().deserialize()?))
}

fn let_go(lock: Lock, cx: &mut App) {
    log!("browser: awake lock {} let go", lock.name());
    let (bus, request) = match lock {
        Lock::Portal(bus, request) => (bus, request),
        Lock::Logind(fd) => return drop(fd),
    };
    cx.spawn(async move |_| {
        let closed = bus
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                &request,
                Some("org.freedesktop.portal.Request"),
                "Close",
                &(),
            )
            .await;
        if let Err(error) = closed {
            log!("browser: awake lock portal didn't close: {error}");
        }
    })
    .detach();
}
