//! Where ilo's update stands, as the system's updater says (docs/ux-spec.md › Updates): Sparkle on
//! the Mac, WinSparkle on Windows, Flatpak's portal on Linux. Settings › About's version row says
//! it in plain words and finishes it, and Check for Updates… reads Restart to Update once one is
//! ready. There's no updater here: each OS's own says what happened, through its delegate or
//! callbacks, and this keeps what it last said and asks it to check or to restart.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Global, Pixels, SharedString, Subscription, Window, canvas, div,
    prelude::*, px,
};

use crate::settings_rows::{Look, button, detailed_row, drawn};
use crate::tokens::settings::PADDING;

/// What updates this copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Updater {
    /// Nothing: a dev build.
    #[default]
    None,
    /// The Microsoft Store, which updates a copy installed from it.
    Store,
    /// ilo's own: Sparkle, WinSparkle or Flatpak's portal.
    Own,
}

/// Where the update stands, as the updater last said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    /// Nothing said yet, or a check ended with nothing to say, as when the user closed the
    /// updater's window or skipped the version it offered.
    #[default]
    Idle,
    Checking,
    /// There's no newer version.
    Current,
    /// A newer version, named if the updater names it, which the updater's own window offers.
    Found(Option<SharedString>),
    Downloading(Option<SharedString>),
    /// Downloaded and waiting: a restart installs it.
    Ready(Option<SharedString>),
    /// The check failed, or once an update was `found`, its download or install did.
    Failed { found: bool },
}

impl Stage {
    /// The word the log has for it, which check/updates.sh waits for.
    fn name(&self) -> String {
        let named = |word: &str, version: &Option<SharedString>| match version {
            Some(version) => format!("{word} {version}"),
            None => word.to_string(),
        };
        match self {
            Stage::Idle => "idle".into(),
            Stage::Checking => "checking".into(),
            Stage::Current => "current".into(),
            Stage::Found(version) => named("found", version),
            Stage::Downloading(version) => named("downloading", version),
            Stage::Ready(version) => named("ready", version),
            Stage::Failed { found: false } => "failed to check".into(),
            Stage::Failed { found: true } => "failed to update".into(),
        }
    }
}

/// The row's button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Check,
    Restart,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Action::Check => "check",
            Action::Restart => "restart",
        }
    }
}

/// What the row and Check for Updates… ask of the updater.
pub trait Host {
    /// Asks the updater to look now. It says what it finds through `changed`.
    fn check(&self, cx: &mut App);
    /// Installs the update that's ready and relaunches into it.
    fn restart(&self, cx: &mut App);
}

#[derive(Default)]
struct Updates {
    updater: Updater,
    stage: Stage,
    host: Option<Rc<dyn Host>>,
}

impl Global for Updates {}

/// Who updates this copy, and how to ask them.
pub fn start(updater: Updater, host: Rc<dyn Host>, cx: &mut App) {
    let updates = cx.default_global::<Updates>();
    updates.updater = updater;
    updates.host = Some(host);
}

/// The updater says the update is at `stage`. A ready update stays ready until it's installed or
/// fails, as no check would find anything newer to show first. A failure's `found` is where it
/// came from: after an update was found, it's the update's, else the check's.
pub fn changed(stage: Stage, cx: &mut App) {
    let previous = cx.try_global::<Updates>().map(|updates| updates.stage.clone()).unwrap_or_default();
    let stage = match stage {
        Stage::Failed { .. } => Stage::Failed {
            found: matches!(previous, Stage::Found(_) | Stage::Downloading(_) | Stage::Ready(_)),
        },
        Stage::Idle | Stage::Checking | Stage::Current | Stage::Found(_)
            if matches!(previous, Stage::Ready(_)) =>
        {
            return;
        }
        stage => stage,
    };
    if stage != previous {
        set(stage, cx);
    }
}

fn set(stage: Stage, cx: &mut App) {
    crate::log!("browser: update stage {}", stage.name());
    cx.default_global::<Updates>().stage = stage;
}

/// Whether the update a quit started installing, build `build`, went in, as this launch, build
/// `own`, is it or newer: an older one is the copy it was to replace (#1459).
pub fn went_in(build: u64, own: u64) -> bool {
    own >= build
}

/// The update the last quit started installing didn't go in, so the row says so, with Try Again,
/// as for a failed download.
pub fn didnt_install(cx: &mut App) {
    set(Stage::Failed { found: true }, cx);
}

pub fn stage(cx: &App) -> Stage {
    cx.try_global::<Updates>().map(|updates| updates.stage.clone()).unwrap_or_default()
}

pub fn updater(cx: &App) -> Updater {
    cx.try_global::<Updates>().map(|updates| updates.updater).unwrap_or_default()
}

/// Whether an update is downloaded and waits for a restart.
pub fn ready(cx: &App) -> bool {
    matches!(stage(cx), Stage::Ready(_))
}

/// Check for Updates…'s name in the app menu and the command bar, which once an update is ready
/// restarts into it.
pub fn command_title(cx: &App) -> &'static str {
    title(ready(cx))
}

/// Check for Updates…'s name, Restart to Update while an update is `ready`.
pub fn title(ready: bool) -> &'static str {
    if ready { RESTART } else { CHECK_FOR_UPDATES }
}

pub const CHECK_FOR_UPDATES: &str = "Check for Updates…";

pub const RESTART: &str = "Restart to Update";

/// Calls `changed` each time the updater says something new.
pub fn observe(cx: &mut App, mut changed: impl FnMut(&mut App) + 'static) -> Subscription {
    cx.observe_global::<Updates>(move |cx| changed(cx))
}

/// Calls `changed` on `view` each time the updater says something new.
pub fn observe_view<V: 'static>(
    cx: &mut Context<V>,
    changed: impl Fn(&mut V, &mut Context<V>) + 'static,
) -> Subscription {
    cx.observe_global::<Updates>(changed)
}

/// Does `action` if it's still the row's, as a click can come after the stage moved on.
pub fn act(action: Action, cx: &mut App) {
    let Some(updates) = cx.try_global::<Updates>() else {
        return;
    };
    if words(updates.updater, &updates.stage).1.map(|(offered, _)| offered) != Some(action) {
        return;
    }
    let Some(host) = updates.host.clone() else {
        return;
    };
    crate::log!("browser: update {} asked", action.name());
    match action {
        Action::Check => host.check(cx),
        Action::Restart => host.restart(cx),
    }
}

/// What the row says at `stage` of a copy `updater` updates, if anything, and its button, if it
/// has one, with its title.
pub fn words(
    updater: Updater,
    stage: &Stage,
) -> (Option<SharedString>, Option<(Action, &'static str)>) {
    const CHECK: Option<(Action, &str)> = Some((Action::Check, "Check for Updates"));
    // Flatpak's portal doesn't name the version.
    fn named(version: &Option<SharedString>, words: String, unnamed: &str) -> Option<SharedString> {
        Some(if version.is_some() { words.into() } else { SharedString::from(unnamed.to_string()) })
    }
    let app = env!("APP_NAME");
    match updater {
        Updater::None => return (None, None),
        Updater::Store => {
            return (Some(concat!("The Microsoft Store updates ", env!("APP_NAME")).into()), None);
        }
        Updater::Own => {}
    }
    let v = |version: &Option<SharedString>| version.clone().unwrap_or_default();
    match stage {
        Stage::Idle => (None, CHECK),
        Stage::Checking => (Some("Checking…".into()), None),
        Stage::Current => (Some(concat!(env!("APP_NAME"), " is up to date").into()), CHECK),
        Stage::Found(found) => {
            (named(found, format!("{app} {} is available", v(found)), "An update is available"), None)
        }
        Stage::Downloading(found) => {
            (named(found, format!("Downloading {}…", v(found)), "Downloading the update…"), None)
        }
        Stage::Ready(found) => (
            named(found, format!("{app} {} is ready", v(found)), "The update is ready"),
            Some((Action::Restart, RESTART)),
        ),
        Stage::Failed { found: false } => {
            (Some("Couldn’t check for updates".into()), Some((Action::Check, "Try Again")))
        }
        Stage::Failed { found: true } => {
            (Some("The update didn’t install".into()), Some((Action::Check, "Try Again")))
        }
    }
}

/// Settings › About's first row: the app's name over its version, and at its end where its update
/// stands and the button that moves it on.
pub struct VersionRow {
    version: SharedString,
    /// Where its button is drawn, logged for check.sh to click.
    logged: Rc<RefCell<Vec<String>>>,
    /// Where its button is drawn, for the tests to click.
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    _watch: Subscription,
}

impl VersionRow {
    pub fn new(version: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            version: version.into(),
            logged: Rc::default(),
            drawn: Rc::default(),
            _watch: observe_view(cx, |_, cx| cx.notify()),
        }
    }

    /// Where its button was drawn last, if it has one.
    pub fn button_bounds(&self) -> Option<Bounds<Pixels>> {
        self.drawn.get()
    }
}

impl Render for VersionRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (said, action) = words(updater(cx), &stage(cx));
        let look = Look::settings();
        self.drawn.set(None);
        let button = action.map(|(action, title)| {
            let drawn_at = self.drawn.clone();
            let at = canvas(|_, _, _| {}, move |bounds, _, _, _| drawn_at.set(Some(bounds)))
                .absolute()
                .inset_0();
            button(&look, "update", title)
                .relative()
                .when(said.is_some(), |button| button.ml(px(PADDING)))
                .child(drawn(format!("update {}", action.name()), self.logged.clone()))
                .child(at)
                .on_click(move |_, _, cx| act(action, cx))
        });
        let control = (said.is_some() || button.is_some()).then(|| {
            div()
                .flex()
                .items_center()
                .children(said.map(|said| div().text_color(look.detail).child(said)))
                .children(button)
                .into_any_element()
        });
        detailed_row(env!("APP_NAME"), self.version.clone(), control)
    }
}

#[cfg(test)]
mod tests;
