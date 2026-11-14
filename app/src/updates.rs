//! Where ilo's update stands, as the system's updater says (docs/ux-spec.md › Updates): Sparkle on
//! the Mac, ilo's own on Windows (platform/windows/updates.cc), Flatpak's portal on Linux. Settings › About's version row says
//! it in plain words and finishes it, and Check for Updates… reads Restart to Update once one is
//! ready. There's no updater here: each OS's own says what happened, through its delegate or
//! callbacks, and this keeps what it last said and asks it to check or to restart. Where the build
//! carries a nightly feed beside its own, About's next row chooses which of the two it reads.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use web_time::Instant;

use gpui::{
    App, Bounds, Context, Global, Pixels, SharedString, Subscription, Window, canvas, div,
    prelude::*, px, rgb,
};

use crate::palette::NO_THEME_ACCENT;
use crate::settings_rows::{
    Look, SLIDE, button, detailed_row, drawn, named_button, named_switch, switch,
};
use crate::tokens::settings::CONTROL_GAP;

/// What updates this copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Updater {
    /// Nothing: a dev build.
    #[default]
    None,
    /// The Microsoft Store, which updates a copy installed from it.
    Store,
    /// ilo's own: Sparkle, Windows' (platform/windows/updates.cc) or Flatpak's portal.
    Own,
}

/// The builds a copy is of, or takes: the releases, or each night's build of main, ahead of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Track {
    #[default]
    Release,
    Nightly,
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
    /// A newer version the updater won't download by itself, as Sparkle leaves one that needs the
    /// user's go-ahead: its own window offers it once asked (`Action::Show`).
    Available(Option<SharedString>),
    Downloading(Option<SharedString>),
    /// Downloaded and waiting: a restart installs it.
    Ready(Option<SharedString>),
    /// The check failed, or once an update was `found`, its download or install did.
    Failed {
        found: bool,
    },
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
            Stage::Available(version) => named("available", version),
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
    /// Opens the updater's own window on the update it found, which installs it from there.
    Show,
    Restart,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Action::Check => "check",
            Action::Show => "show",
            Action::Restart => "restart",
        }
    }
}

/// What the row and Check for Updates… ask of the updater.
pub trait Host {
    /// Asks the updater to look now. It says what it finds through `changed`.
    fn check(&self, cx: &mut App);
    /// Shows the update it found in its own window, which offers to install it.
    fn show(&self, cx: &mut App);
    /// Installs the update that's ready and relaunches into it.
    fn restart(&self, cx: &mut App);
    /// Has the updater read `track`'s feed from its next check on, and keeps the choice.
    fn follow(&self, track: Track, cx: &mut App);
    /// Turns the updater's checks on its schedule on or off, and keeps the choice, where the user
    /// can turn them off.
    fn check_automatically(&self, _on: bool, _cx: &mut App) {}
}

#[derive(Default)]
struct Updates {
    updater: Updater,
    stage: Stage,
    host: Option<Rc<dyn Host>>,
    /// The builds this one is of.
    built: Track,
    /// The builds the updater takes, where the build carries a nightly feed beside its own, so
    /// About offers the choice.
    followed: Option<Track>,
    /// Restart to Update was asked, and the quit that installs the update hasn't gone ahead yet.
    restarting: bool,
    /// Whether the updater checks on its schedule, where the user can turn that off.
    automatic: Option<bool>,
}

impl Global for Updates {}

/// Who updates this copy, and how to ask them.
pub fn start(updater: Updater, host: Rc<dyn Host>, cx: &mut App) {
    let updates = cx.default_global::<Updates>();
    updates.updater = updater;
    updates.host = Some(host);
}

/// This build is one of `built`'s, and where it carries a nightly feed, its updater takes
/// `followed`'s builds.
pub fn tracks(built: Track, followed: Option<Track>, cx: &mut App) {
    let updates = cx.default_global::<Updates>();
    updates.built = built;
    updates.followed = followed;
}

/// Whether the updater checks on its schedule, where the user can turn that off: on the Mac and
/// Windows, not a Flatpak's.
pub fn checks(automatic: Option<bool>, cx: &mut App) {
    cx.default_global::<Updates>().automatic = automatic;
}

/// Whether the updater checks on its own, where the user can turn that off.
pub fn automatic(cx: &App) -> Option<bool> {
    cx.try_global::<Updates>()?.automatic
}

/// Turns the updater's checks on its schedule on or off, as Settings › About's switch and the
/// update banner do.
pub fn set_automatic(on: bool, cx: &mut App) {
    let updates = cx.default_global::<Updates>();
    if updates.automatic.is_none_or(|automatic| automatic == on) {
        return;
    }
    updates.automatic = Some(on);
    if let Some(host) = updates.host.clone() {
        host.check_automatically(on, cx);
    }
}

/// The builds this one is of, which About's first row names.
pub fn built(cx: &App) -> Track {
    cx.try_global::<Updates>()
        .map(|updates| updates.built)
        .unwrap_or_default()
}

/// The builds the updater takes, where About offers the choice: a copy that updates itself and
/// carries a nightly feed.
/// Whether the app updates itself, with its own updater, rather than the Store or nothing.
pub fn own_updater(cx: &App) -> bool {
    cx.try_global::<Updates>()
        .is_some_and(|updates| updates.updater == Updater::Own)
}

pub fn followed(cx: &App) -> Option<Track> {
    let updates = cx.try_global::<Updates>()?;
    if updates.updater != Updater::Own {
        return None;
    }
    updates.followed
}

/// Has the updater take `track`'s builds, which on a nightly checks for one at once. Never an
/// older build: each updater installs only a build numbered above the one running, so a nightly
/// turned off keeps its build until a release passes it.
pub fn follow(track: Track, cx: &mut App) {
    if followed(cx).is_none_or(|followed| followed == track) {
        return;
    }
    let updates = cx.default_global::<Updates>();
    updates.followed = Some(track);
    let host = updates.host.clone();
    // check.sh waits for this line, as it does for Settings' other switches.
    let on = if track == Track::Nightly { "on" } else { "off" };
    crate::log!("browser: {NIGHTLY} {on}");
    let Some(host) = host else {
        return;
    };
    host.follow(track, cx);
    if track == Track::Nightly {
        host.check(cx);
    }
}

/// The nightly switch's name in the log and where it's drawn.
const NIGHTLY: &str = "nightly builds";

/// Its row's title, which an assistive app calls its switch.
const NIGHTLY_NAME: &str = "Get nightly builds";

/// What the nightly row says under its name: what a nightly is, or, turned off on one, that this
/// build stays until a release is newer.
pub fn nightly_words(built: Track, followed: Track) -> &'static str {
    if built == Track::Nightly && followed == Track::Release {
        concat!(
            env!("APP_NAME"),
            " keeps this build until a release is newer."
        )
    } else {
        concat!(
            "Each night’s build of ",
            env!("APP_NAME"),
            ", ahead of the next release."
        )
    }
}

/// The updater says the update is at `stage`. A ready update stays ready until it's installed or
/// fails, as no check would find anything newer to show first. A failure's `found` is where it
/// came from: after an update was found, it's the update's, else the check's.
pub fn changed(stage: Stage, cx: &mut App) {
    let previous = cx
        .try_global::<Updates>()
        .map(|updates| updates.stage.clone())
        .unwrap_or_default();
    let stage = match stage {
        Stage::Failed { .. } => Stage::Failed {
            found: matches!(
                previous,
                Stage::Found(_) | Stage::Available(_) | Stage::Downloading(_) | Stage::Ready(_)
            ),
        },
        Stage::Idle | Stage::Checking | Stage::Current | Stage::Found(_) | Stage::Available(_)
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
    let updates = cx.default_global::<Updates>();
    updates.restarting &= matches!(stage, Stage::Ready(_));
    updates.stage = stage;
}

/// Whether Restart to Update was asked and the restart hasn't been stopped: the sidebar's banner
/// says Restarting… until the app quits.
pub fn restarting(cx: &App) -> bool {
    cx.try_global::<Updates>()
        .is_some_and(|updates| updates.restarting)
}

/// The quit that Restart to Update started didn't go ahead, as a page asked to stay: the update
/// stays ready, and the banner offers the restart again.
pub fn restart_stopped(cx: &mut App) {
    if restarting(cx) {
        crate::log!("browser: update restart stopped");
        cx.default_global::<Updates>().restarting = false;
    }
}

/// Whether the update a quit started installing, build `build`, went in, as this launch, build
/// `own`, is it or newer: an older one is the copy it was to replace (#1459).
pub fn went_in(build: u64, own: u64) -> bool {
    own >= build
}

/// The release notes the launch of version `own` opens on a profile whose last version is kept in
/// `file`, which it then keeps: the version's changelog entry when it's newer than the last, and
/// nothing on the profile's first launch or a downgrade. Linux's, where an update can come from
/// outside ilo, as from GNOME Software or `flatpak update` (#1495).
pub fn first_launch_notes(file: &std::path::Path, own: &str) -> Option<String> {
    let last = std::fs::read_to_string(file).ok();
    let last = last.as_deref().map(str::trim);
    if last == Some(own) {
        return None;
    }
    _ = std::fs::write(file, own);
    newer(own, last?).then(|| format!("{}#{own}", crate::build_info::WHATS_NEW))
}

/// Settings › About's "Show what's new after an update" is off while this file is in the profile
/// directory: on by default, and on the nightly track a tab every morning is noise.
pub const NO_WHATS_NEW: &str = "no-whats-new";

/// The notes an update's first launch opens, `notes`, unless the user turned them off in the
/// profile directory `dir`. Help › What's New still opens them.
pub fn notes_to_open(notes: Option<String>, dir: &std::path::Path) -> Option<String> {
    let off = dir.join(NO_WHATS_NEW).exists();
    if off && let Some(url) = &notes {
        // check.sh reads this line.
        crate::log!("browser: update notes off: {url}");
    }
    notes.filter(|_| !off)
}

/// Whether version `own` comes after `last`, number by number: 0.1.10 after 0.1.9.
fn newer(own: &str, last: &str) -> bool {
    let numbers = |version: &str| -> Option<Vec<u64>> {
        version
            .split('.')
            .map(|number| number.parse().ok())
            .collect()
    };
    matches!((numbers(own), numbers(last)), (Some(own), Some(last)) if own > last)
}

/// The update the last quit started installing didn't go in, so the row says so, with Try Again,
/// as for a failed download.
pub fn didnt_install(cx: &mut App) {
    set(Stage::Failed { found: true }, cx);
}

pub fn stage(cx: &App) -> Stage {
    cx.try_global::<Updates>()
        .map(|updates| updates.stage.clone())
        .unwrap_or_default()
}

pub fn updater(cx: &App) -> Updater {
    cx.try_global::<Updates>()
        .map(|updates| updates.updater)
        .unwrap_or_default()
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

/// The button that opens the updater's window on an update it won't download by itself.
pub const SHOW: &str = "Show Update";

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
    if words(updates.updater, &updates.stage)
        .1
        .map(|(offered, _)| offered)
        != Some(action)
    {
        return;
    }
    let Some(host) = updates.host.clone() else {
        return;
    };
    crate::log!("browser: update {} asked", action.name());
    match action {
        Action::Check => host.check(cx),
        Action::Show => host.show(cx),
        Action::Restart => {
            cx.default_global::<Updates>().restarting = true;
            host.restart(cx)
        }
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
        Some(if version.is_some() {
            words.into()
        } else {
            SharedString::from(unnamed.to_string())
        })
    }
    let app = env!("APP_NAME");
    match updater {
        Updater::None => return (None, None),
        Updater::Store => {
            return (
                Some(concat!("The Microsoft Store updates ", env!("APP_NAME")).into()),
                None,
            );
        }
        Updater::Own => {}
    }
    let v = |version: &Option<SharedString>| version.clone().unwrap_or_default();
    match stage {
        Stage::Idle => (None, CHECK),
        Stage::Checking => (Some("Checking…".into()), None),
        Stage::Current => (
            Some(concat!(env!("APP_NAME"), " is up to date").into()),
            CHECK,
        ),
        Stage::Found(found) => (
            named(
                found,
                format!("{app} {} is available", v(found)),
                "An update is available",
            ),
            None,
        ),
        Stage::Available(found) => (
            named(
                found,
                format!("{app} {} is available", v(found)),
                "An update is available",
            ),
            Some((Action::Show, SHOW)),
        ),
        Stage::Downloading(found) => (
            named(
                found,
                format!("Downloading {}…", v(found)),
                "Downloading the update…",
            ),
            None,
        ),
        Stage::Ready(found) => (
            named(
                found,
                format!("{app} {} is ready", v(found)),
                "The update is ready",
            ),
            Some((Action::Restart, RESTART)),
        ),
        Stage::Failed { found: false } => (
            Some("Couldn’t check for updates".into()),
            Some((Action::Check, "Try Again")),
        ),
        Stage::Failed { found: true } => (
            Some("The update didn’t install".into()),
            Some((Action::Check, "Try Again")),
        ),
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
    /// The colours of the window it's in, dark until Settings says.
    look: Look,
    _watch: Subscription,
}

impl VersionRow {
    pub fn new(version: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            version: version.into(),
            logged: Rc::default(),
            drawn: Rc::default(),
            look: Look::settings(true),
            _watch: observe_view(cx, |_, cx| cx.notify()),
        }
    }

    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        self.look = look;
        cx.notify();
    }

    /// Where its button was drawn last, if it has one.
    pub fn button_bounds(&self) -> Option<Bounds<Pixels>> {
        self.drawn.get()
    }
}

impl Render for VersionRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (said, action) = words(updater(cx), &stage(cx));
        let look = self.look;
        let a11y = window.is_a11y_active();
        self.drawn.set(None);
        let button = action.map(|(action, title)| {
            let drawn_at = self.drawn.clone();
            let at = canvas(
                |_, _, _| {},
                move |bounds, _, _, _| drawn_at.set(Some(bounds)),
            )
            .absolute()
            .inset_0();
            // Try Again says what it tries again, as the line before it may not be read.
            let spoken = match title {
                "Try Again" => "Check for Updates Again",
                title => title,
            };
            button(&look, "update", title)
                .when(a11y, |button| named_button(button, spoken))
                .relative()
                .when(said.is_some(), |button| button.ml(px(CONTROL_GAP)))
                .child(drawn(
                    format!("update {}", action.name()),
                    self.logged.clone(),
                ))
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
        detailed_row(&look, env!("APP_NAME"), self.version.clone(), control)
    }
}

/// Settings › About's second row, where the build carries a nightly feed: Get nightly builds, a
/// line on what that means here, and its switch.
pub struct NightlyRow {
    /// Where its switch is drawn, logged for check.sh to click.
    logged: Rc<RefCell<Vec<String>>>,
    /// Where its switch is drawn, for the tests to click.
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// When the switch was last clicked, which its knob slides from.
    switched: Option<Instant>,
    /// The colours of the window it's in, dark until Settings says.
    look: Look,
    _watch: Subscription,
}

impl NightlyRow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            logged: Rc::default(),
            drawn: Rc::default(),
            switched: None,
            look: Look::settings(true),
            _watch: observe_view(cx, |_, cx| cx.notify()),
        }
    }

    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        self.look = look;
        cx.notify();
    }

    /// Where its switch was drawn last.
    pub fn switch_bounds(&self) -> Option<Bounds<Pixels>> {
        self.drawn.get()
    }
}

impl Render for NightlyRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn.set(None);
        let Some(followed) = followed(cx) else {
            return div().into_any_element();
        };
        let on = followed == Track::Nightly;
        let slid = self
            .switched
            .map_or(1., |at| (at.elapsed().as_secs_f32() / SLIDE).min(1.));
        if slid < 1. {
            window.request_animation_frame();
        }
        let drawn_at = self.drawn.clone();
        let at = canvas(
            |_, _, _| {},
            move |bounds, _, _, _| drawn_at.set(Some(bounds)),
        )
        .absolute()
        .inset_0();
        let control = div()
            .id(NIGHTLY)
            .when(window.is_a11y_active(), |control| {
                named_switch(control, NIGHTLY_NAME, on)
            })
            .relative()
            .child(switch(&self.look, on, slid, rgb(NO_THEME_ACCENT)))
            .child(drawn(format!("switch {NIGHTLY}"), self.logged.clone()))
            .child(at)
            .on_click(cx.listener(move |row, _, _, cx| {
                row.switched = Some(crate::motion::start(cx));
                follow(if on { Track::Release } else { Track::Nightly }, cx);
                cx.notify();
            }));
        let words = nightly_words(built(cx), followed);
        let control = Some(control.into_any_element());
        detailed_row(&self.look, NIGHTLY_NAME, words, control).into_any_element()
    }
}

#[cfg(test)]
mod tests;
