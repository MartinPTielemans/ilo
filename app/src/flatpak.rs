//! Updates on Linux, where ilo is the Flatpak on Flathub (docs/ux-spec.md › Updates): Flatpak's
//! portal says when Flathub has a newer build, ilo installs it beside the one running, without
//! waiting for the software centre, and Restart to Update at the sidebar's foot restarts into it. Outside a Flatpak, as in a dev
//! build, there's no portal to ask, and no updates.

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use futures_lite::{Stream, StreamExt, future};
use gpui::{App, Global};
use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue, Value};

use crate::dialog::{self, Question, Tone};

const PORTAL: &str = "org.freedesktop.portal.Flatpak";
const PATH: &str = "/org/freedesktop/portal/Flatpak";
const MONITOR: &str = "org.freedesktop.portal.Flatpak.UpdateMonitor";
/// Spawn's flag for the newest build installed, rather than the one running.
const LATEST_VERSION: u32 = 2;
// The update monitor's Progress statuses.
const RUNNING: u32 = 0;
const EMPTY: u32 = 1;
const DONE: u32 = 2;
const ERROR: u32 = 3;
/// What a restart runs in the newest build: `sh -c RESTART sh <lock> <command…>` waits for the
/// profile's lock to go, up to 30 s so one a crash left doesn't hold it back, as Chromium takes
/// over one whose process has gone, then runs the command.
const RESTART: &str = r#"lock=$1; shift; i=0
while [ -L "$lock" ] && [ $i -lt 300 ]; do sleep 0.1; i=$((i + 1)); done
exec "$@""#;

/// The Flatpak's command (`command:` in app/platform/linux/flatpak/app.ilo.browser.yml), which
/// starts chrome through cobalt and zypak. The running process's own argv starts /app/ilo/chrome,
/// which would come up without the sandbox zypak hands it.
const COMMAND: &str = "ilo";

/// Whether this is the Flatpak, which Flatpak marks with this file.
pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Where the update stands, as the user is told of it.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// None found.
    Current,
    Installing,
    /// Installed beside the running build, for the next launch.
    Ready,
    Failed(String),
}

/// What the portal says.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// Flatpak found a build: `local` installed and `remote` on Flathub, against the running one.
    Available { running: String, local: String, remote: String },
    Progress { status: u32, error: String },
}

impl State {
    /// The state after `event`, and whether to install the build it announces.
    pub fn after(&self, event: &Event) -> (State, bool) {
        match event {
            // The software centre has installed it already, so a restart is all it needs.
            Event::Available { running, local, .. } if local != running => (State::Ready, false),
            Event::Available { local, remote, .. } if remote != local => (State::Installing, true),
            Event::Available { .. } => (self.clone(), false),
            Event::Progress { status: RUNNING, .. } => (State::Installing, false),
            Event::Progress { status: EMPTY, .. } if *self == State::Installing => {
                (State::Current, false)
            }
            Event::Progress { status: DONE, .. } => (State::Ready, false),
            Event::Progress { status: ERROR, error } => (State::Failed(error.clone()), false),
            Event::Progress { .. } => (self.clone(), false),
        }
    }
}

impl Event {
    fn read(message: &zbus::Message) -> Option<Event> {
        let header = message.header();
        let info: HashMap<String, OwnedValue> = message.body().deserialize().ok()?;
        let text = |key: &str| {
            info.get(key).and_then(|value| value.downcast_ref::<&str>().ok()).unwrap_or("").to_string()
        };
        match header.member()?.as_str() {
            "UpdateAvailable" => Some(Event::Available {
                running: text("running-commit"),
                local: text("local-commit"),
                remote: text("remote-commit"),
            }),
            "Progress" => Some(Event::Progress {
                status: info.get("status")?.downcast_ref::<u32>().ok()?,
                error: text("error_message"),
            }),
            _ => None,
        }
    }
}

/// The portal's update monitor for this instance.
#[derive(Clone)]
pub struct Portal {
    connection: zbus::Connection,
    monitor: OwnedObjectPath,
}

/// What the update monitor says, from before anything could have been said.
pub struct Events(zbus::MessageStream);

impl Portal {
    /// The session's portal, which in a Flatpak is Flatpak's.
    pub async fn open_session() -> zbus::Result<(Portal, Events)> {
        Portal::open(zbus::Connection::session().await?).await
    }

    pub async fn open(connection: zbus::Connection) -> zbus::Result<(Portal, Events)> {
        let options: HashMap<&str, Value> = HashMap::new();
        let reply = connection
            .call_method(Some(PORTAL), PATH, Some(PORTAL), "CreateUpdateMonitor", &(options,))
            .await?;
        let monitor: OwnedObjectPath = reply.body().deserialize()?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(MONITOR)?
            .path(monitor.clone())?
            .build();
        let events = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;
        Ok((Portal { connection, monitor }, Events(events)))
    }

    /// Installs the newest build beside the running one; Progress says how it goes.
    pub async fn update(&self) -> zbus::Result<()> {
        let options: HashMap<&str, Value> = HashMap::new();
        let monitor = self.monitor.as_ref();
        self.connection.call_method(Some(PORTAL), monitor, Some(MONITOR), "Update", &("", options)).await?;
        Ok(())
    }

    /// Starts the newest build through the Flatpak's command, once this one has let go of its
    /// profile, whose `lock` Chromium holds while it runs: a second copy would hand its launch to
    /// this one and end. Asked as this one's restart goes ahead, never before, as nothing could
    /// take it back. Without this launch's arguments: cobalt adds its flags again, and the session
    /// brings back the pages a launch's URLs opened.
    pub async fn restart(&self, lock: &Path) -> zbus::Result<()> {
        let mut command = vec!["/bin/sh".to_string(), "-c".into(), RESTART.into(), "sh".into()];
        command.push(lock.to_string_lossy().into_owned());
        command.push(COMMAND.into());
        // Byte strings, ended with a NUL, as the portal reads them.
        let bytes = |text: &str| [text.as_bytes(), b"\0"].concat();
        let argv: Vec<Vec<u8>> = command.iter().map(|arg| bytes(arg)).collect();
        let fds: HashMap<u32, Fd> = HashMap::new();
        let envs: HashMap<&str, &str> = HashMap::new();
        let options: HashMap<&str, Value> = HashMap::new();
        let body = (bytes(""), argv, fds, envs, LATEST_VERSION, options);
        self.connection.call_method(Some(PORTAL), PATH, Some(PORTAL), "Spawn", &body).await?;
        Ok(())
    }
}

/// How long an install waits on Update, which answers only once the user has said whether ilo may
/// update itself: a desktop with nothing to ask it with never answers (#877).
pub const ANSWER: Duration = Duration::from_secs(120);

/// What an install that waited out `ANSWER` says.
pub const UNANSWERED: &str = concat!(
    "Flatpak asked whether ",
    env!("APP_NAME"),
    " may update itself, and nothing answered."
);

/// Follows the update monitor until the portal goes, installing each build it announces, and
/// again on each of `retries` after a failure, and tells `changed` each new state. An install
/// waits up to `answer` for the portal; an answer after that still moves the state on.
pub async fn watch(
    portal: &Portal,
    mut events: Events,
    mut retries: impl Stream<Item = ()> + Unpin,
    answer: Duration,
    mut changed: impl FnMut(&State),
) {
    enum Heard {
        Said(zbus::Result<zbus::Message>),
        Retry,
    }
    let mut state = State::Current;
    loop {
        let said = async { events.0.next().await.map(Heard::Said) };
        // Retries that have ended leave the events alone.
        let retry = async {
            match retries.next().await {
                Some(()) => Some(Heard::Retry),
                None => future::pending().await,
            }
        };
        let (next, install) = match future::or(said, retry).await {
            None => return,
            Some(Heard::Retry) if matches!(state, State::Failed(_)) => (State::Installing, true),
            Some(Heard::Retry) => continue,
            Some(Heard::Said(message)) => {
                let Some(event) = message.ok().as_ref().and_then(Event::read) else {
                    continue;
                };
                state.after(&event)
            }
        };
        if next != state {
            state = next;
            changed(&state);
        }
        if install && let Some(failed) = install_waiting(portal, answer).await {
            state = failed;
            changed(&state);
        }
    }
}

/// Asks the portal to install, waiting up to `answer`; the failure, if it fails or never answers.
async fn install_waiting(portal: &Portal, answer: Duration) -> Option<State> {
    let answered = async { Some(portal.update().await) };
    let unanswered = async {
        async_io::Timer::after(answer).await;
        None
    };
    match future::or(answered, unanswered).await {
        Some(Ok(())) => None,
        Some(Err(error)) => Some(State::Failed(error.to_string())),
        None => Some(State::Failed(UNANSWERED.into())),
    }
}

/// Where the update stands as the user is told, once the portal has answered, what Restart runs:
/// the browser's restart, then as it goes ahead, the portal's start of the newest build; and what
/// Try Again runs after a failure.
#[derive(Default)]
struct Told {
    state: Option<State>,
    restart: Option<Rc<dyn Fn(&mut App)>>,
    relaunch: Option<Rc<dyn Fn() -> bool>>,
    retry: Option<Rc<dyn Fn(&mut App)>>,
}

impl Global for Told {}

/// The portal answered: Restart runs `restart`.
pub fn on_restart(restart: impl Fn(&mut App) + 'static, cx: &mut App) {
    cx.default_global::<Told>().restart = Some(Rc::new(restart));
}

/// The portal answered: a restart that goes ahead with a build in runs `relaunch`, which starts
/// it in place of this one, true if it did.
pub fn on_relaunch(relaunch: impl Fn() -> bool + 'static, cx: &mut App) {
    cx.default_global::<Told>().relaunch = Some(Rc::new(relaunch));
}

/// The portal answered: Try Again, after a failed install, runs `retry`.
pub fn on_retry(retry: impl Fn(&mut App) + 'static, cx: &mut App) {
    cx.default_global::<Told>().retry = Some(Rc::new(retry));
}

/// Try Again's presses, as `watch` takes them; made here, as the crate that builds the app on
/// Linux has no futures of its own.
pub fn retries(cx: &mut App) -> futures::channel::mpsc::UnboundedReceiver<()> {
    let (retry, retries) = futures::channel::mpsc::unbounded();
    on_retry(move |_| _ = retry.unbounded_send(()), cx);
    retries
}

/// The update is at `state` now, which Settings › About's row says, and once a build is ready,
/// Restart to Update at the sidebar's foot, as on the Mac and Windows, rather than a dialog.
pub fn changed(state: State, cx: &mut App) {
    crate::updates::changed(state.stage(), cx);
    cx.default_global::<Told>().state = Some(state);
}

impl State {
    /// Where the row says the update stands. The portal names no version.
    fn stage(&self) -> crate::updates::Stage {
        use crate::updates::Stage;
        match self {
            State::Current => Stage::Current,
            State::Installing => Stage::Downloading(None),
            State::Ready => Stage::Ready(None),
            State::Failed(_) => Stage::Failed { found: true },
        }
    }
}

/// Restart, as the offer's: restarts the browser, once the portal has answered, into the newest
/// build if the restart goes ahead (`relaunch`).
pub fn restart(cx: &mut App) {
    if let Some(restart) = cx.default_global::<Told>().restart.clone() {
        restart(cx);
    }
}

/// A restart goes ahead: the newest build starts in place of this one, if one is in. True if it
/// did, so Chromium doesn't start this one again.
pub fn relaunch(cx: &mut App) -> bool {
    let told = cx.default_global::<Told>();
    let relaunch = told.relaunch.clone().filter(|_| told.state == Some(State::Ready));
    relaunch.is_some_and(|relaunch| relaunch())
}

/// How long the quit waits on the portal to start the newest build.
pub const SPAWN: Duration = Duration::from_secs(5);

/// Has the portal start the newest build once `lock` has gone, waiting up to `answer` for it to
/// say it has, as the quit can't go on before.
pub fn start_newest(portal: &Portal, lock: &Path, answer: Duration) -> Result<(), String> {
    let started = async { Some(portal.restart(lock).await) };
    let unanswered = async {
        async_io::Timer::after(answer).await;
        None
    };
    match async_io::block_on(future::or(started, unanswered)) {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => Err(error.to_string()),
        None => Err("Flatpak’s portal didn’t answer".into()),
    }
}

/// Try Again, after an install failed: true if it tried.
pub fn retry(cx: &mut App) -> bool {
    let told = cx.default_global::<Told>();
    let retry = told.retry.clone().filter(|_| matches!(told.state, Some(State::Failed(_))));
    retry.map(|retry| retry(cx)).is_some()
}

/// Check for Updates…. Flatpak's portal can't be asked to look now, as it looks every half hour
/// by itself, so this says where the update stands, offers the restart once a build is ready, or
/// another try after an install failed.
pub fn check(cx: &mut App) {
    let told = cx.default_global::<Told>();
    let failed = matches!(told.state, Some(State::Failed(_)));
    let action = if failed { told.retry.clone() } else { told.restart.clone() };
    let question = question(told.state.as_ref(), told.retry.is_some());
    dialog::pose(
        None,
        question,
        Box::new(move |_, cx| {
            if let Some(action) = action {
                action(cx)
            }
        }),
        cx,
    );
}

/// What Check for Updates… asks or says at `state`, None before the portal has answered, and
/// whether it offers to try again after a failure, which needs the portal.
pub fn question(state: Option<&State>, retries: bool) -> Question {
    let (message, detail) = match state {
        Some(State::Ready) => {
            return Question {
                action: "Restart".into(),
                cancel: "Later".into(),
                ..notice(
                    concat!("A new version of ", env!("APP_NAME"), " is ready"),
                    concat!("Restart ", env!("APP_NAME"), " to use it.").into(),
                )
            };
        }
        Some(State::Installing) => (
            concat!(env!("APP_NAME"), " is updating"),
            "Restart to Update shows in the sidebar once the new version is in.".to_string(),
        ),
        Some(State::Failed(error)) => {
            let failed = notice(concat!(env!("APP_NAME"), " couldn’t update"), error.clone());
            if !retries {
                return failed;
            }
            return Question { action: "Try Again".into(), cancel: "Later".into(), ..failed };
        }
        Some(State::Current) | None => (
            "No update yet",
            concat!(env!("APP_NAME"), " looks for one every half hour while it’s open.").into(),
        ),
    };
    notice(message, detail)
}

/// A dialog that only tells, with OK.
fn notice(message: &str, detail: String) -> Question {
    Question {
        icon: None,
        message: message.to_string().into(),
        detail: detail.into(),
        list: Vec::new(),
        action: "".into(),
        cancel: "OK".into(),
        tone: Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: crate::dialog::Return::Action,
    }
}

#[cfg(test)]
mod tests;
