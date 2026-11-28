//! A new profile's first launch (spec-first-run.md, first-run-v9): one card over the desktop, in a
//! window of its own (window.rs) that is the only one on screen until it ends, with up to seven
//! steps, each shown only when it has something to ask. Only a launch with nothing to bring back
//! starts it, and Help › Welcome plays it again, so a normal launch never draws or loads any of it.
//!
//! ↩ is a step's main button and nothing where a step has none; ⌘[ goes back; Esc only cancels a
//! wait under way (a sign-in, an import), so it can never pass a step. The window has no close, and
//! ⌘W closes nothing. A step's button does what it offers; passing a step, Back and Not Now set
//! nothing. Nothing about the run is kept or sent: the stderr lines a check waits on name a step by
//! its number, never what was picked or typed.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Entity, KeyboardButton, KeyboardClickEvent,
    KeystrokeEvent, MouseButton, Pixels, Rgba, SharedString, Subscription, Window, accesskit::Role,
    canvas, div, prelude::*, px, rgb,
};

use super::{Command, Shell, cubic_bezier, display_face::display_text};
use crate::account::{self, Account, AccountEvent, Problem, Status};
use crate::default_browser::{self, Os};
use crate::import::{ImportEvent, ImportView, Look};
use crate::product::PRODUCT;
use crate::settings_rows::{self, group, named_button, stop_switch};
use crate::tokens::shell::onboarding::SIGNAL;
use crate::tokens::shell::onboarding::card as t;
use card::Inks;

mod bring;
mod card;
mod exit;
mod other_apps;
mod resume;
mod space;
mod window;

pub use resume::waiting as first_run_waiting;

pub(super) use window::FirstRun;

/// The card's steps (v9 §0), in the order they come.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Step {
    /// The Welcome, which is the account: Settings › Sync's view (crate::account) behind its
    /// Continue. Left out in a build that names no sync server, and once the browser is signed in.
    Account,
    /// Your assistants: the rows Settings › Models has, each signing in with its maker's own
    /// sign-in; `Just the Browser` for neither. Left out, as the threads' switch is, in a build
    /// without the agents' server.
    Assistant,
    /// Bring your stuff: the import view (crate::import); left out when it finds no browser.
    Import,
    /// The import's second state: the sites visited most, ticked, to make favorites; there only
    /// once an import has read them. It shares the import's place in the progress.
    Favorites,
    /// Your first Space: its name and its look, on the theme panel's own parts.
    Space,
    /// Other apps: two ways in, the command with its skill and MCP, each with Settings › Other
    /// apps' own switch, only when Claude Code or Codex is found here.
    OtherApps,
    /// You're woven in: the default browser and Help improve.
    Done,
}

const STEPS: [Step; 7] = [
    Step::Account,
    Step::Assistant,
    Step::Import,
    Step::Favorites,
    Step::Space,
    Step::OtherApps,
    Step::Done,
];

impl Step {
    /// The step's number in the design, what its stderr lines and check.sh call it: never a pick.
    fn number(self) -> &'static str {
        match self {
            Step::Account => "1",
            Step::Assistant => "2",
            Step::Import => "3",
            Step::Favorites => "3b",
            Step::Space => "4",
            Step::OtherApps => "5",
            Step::Done => "6",
        }
    }

    /// The step's place in the design, 1 to 6, the favorites in the import's: what ⌘Q keeps.
    fn place(self) -> u8 {
        match self {
            Step::Account => 1,
            Step::Assistant => 2,
            Step::Import | Step::Favorites => 3,
            Step::Space => 4,
            Step::OtherApps => 5,
            Step::Done => 6,
        }
    }

    /// The step's heading and its one line.
    fn message(self, os: Os) -> (SharedString, SharedString) {
        let (title, line): (SharedString, SharedString) = match self {
            Step::Account => (
                PRODUCT.into(),
                "Sign in to keep your Spaces on every device.".into(),
            ),
            Step::Assistant => (
                "Your assistants".into(),
                format!("{PRODUCT} has none of its own. Bring yours.").into(),
            ),
            Step::Import => (
                "Bring your stuff".into(),
                format!("From a browser on {}.", here(os)).into(),
            ),
            Step::Favorites => ("Your favorites".into(), "Click one to leave it out.".into()),
            Step::Space => ("Your first Space".into(), "".into()),
            Step::OtherApps => (
                "Other apps".into(),
                "Agents you run elsewhere can browse here, in tabs of their own.".into(),
            ),
            // The one weaving word on the card's own copy (v9 §1).
            Step::Done => ("You're woven in.".into(), "".into()),
        };
        (title, line)
    }

    /// The split steps: controls in a column at the left, the picture's plane at the right. Only a
    /// step whose picture is in, so no blank plane is ever shown: v9 splits Assistant and Space,
    /// and until Assistant's picture lands its own column is centred as Bring your stuff's is.
    fn split(self) -> bool {
        Self::PICTURED.contains(&self)
    }

    /// The steps whose picture is in.
    const PICTURED: &[Step] = &[Step::Space];
}

/// The switches' rows on the card. On is the text's ink with a knob of the card's, as Settings'
/// are: no green on a control. A row's second line is text-2, as Look::on's mix over the card's
/// see-through ground would all but vanish.
fn switch_look(inks: Inks) -> Look {
    Look {
        knob_on: inks.fill(true),
        detail: inks.text_2(),
        ..Look::on(inks.group(), inks.text(1.), inks.text(1.))
    }
}

/// What the computer is called in a line.
/// A row across the card at `y`, its content centred.
fn welcome_centred(y: f32) -> gpui::Div {
    card::at(0., y).w(px(t::WIDTH)).flex().justify_center()
}

/// A row of buttons across the card at `y`, centred, the foot's gap between them.
fn welcome_buttons(y: f32) -> gpui::Div {
    welcome_centred(y).items_center().gap(px(t::FOOT_GAP))
}

/// The line under the Welcome's button, `gap` below it, its parts `between` apart.
fn welcome_line(gap: f32, between: f32, inks: Inks) -> gpui::Div {
    let [height, _, _, _] = t::SMALL_BUTTON;
    div()
        .mt(px(gap))
        .min_h(px(height))
        .flex()
        .justify_center()
        .items_center()
        .gap(px(between))
        .text_size(px(t::BUTTON_TEXT))
        .text_color(inks.text_2())
}

fn here(os: Os) -> &'static str {
    match os {
        Os::Mac => "this Mac",
        _ => "this computer",
    }
}

/// What the Welcome shows of the account (v9 §3), each a cut inside the same card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Welcome {
    /// Sign in or sign up, or Not Now.
    Form,
    /// The sign-in window is open, where the address, the code and the passkey are the server's.
    SigningIn,
    /// A first device's recovery words, shown once.
    Words,
    /// Waiting for a signed-in device to add this one.
    Approve,
    /// No device is asked now: one declined, none answered in time, or the user stopped asking.
    Again,
    /// The recovery words' field.
    Restore,
    /// Another device added this one, which says so before going on.
    Syncing,
}

/// A switch the card shows, each Settings' own.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Switch {
    /// Other apps' command, with its skill, added or removed at once.
    Command,
    /// Other apps' MCP: each program found connected or disconnected at once.
    Mcp,
    /// Help improve: the daily usage count and crash reports together, off until the user turns it
    /// on, and set only as the last step is left, so nothing is sent while it shows (#2978).
    Help,
}

impl Switch {
    /// What check.sh calls it.
    fn name(self) -> &'static str {
        match self {
            Switch::Command => "command",
            Switch::Mcp => "mcp",
            Switch::Help => "help-improve",
        }
    }

    /// Its row's name and words.
    fn words(self) -> (SharedString, SharedString) {
        match self {
            Switch::Command => (
                format!("The {} command", crate::agents::cli::LINK).into(),
                "Stays out of the way until an agent needs a page.".into(),
            ),
            Switch::Mcp => (
                "MCP".into(),
                "Always in an app's tool list. Needs no terminal.".into(),
            ),
            Switch::Help => (
                format!("Help improve {PRODUCT}").into(),
                "An anonymous use count a day and crash reports.".into(),
            ),
        }
    }
}

/// How long the exit takes where the host doesn't grow first run's window.
#[cfg(test)]
const END_FADE: f32 = crate::tokens::shell::onboarding::exit::CROSS_FADE;

/// The account isn't signed in, only waiting perhaps for the sign-in window, as the core said last,
/// or before it has said.
fn signed_out(state: &account::State) -> bool {
    matches!(state.status, Status::SignedOut | Status::SigningIn)
}

/// The card's arrival curve, ARRIVE.
fn arrive(x: f32) -> f32 {
    let [x1, y1, x2, y2] = t::ARRIVE;
    cubic_bezier(x1, y1, x2, y2, x.clamp(0., 1.))
}

/// The progress knot's and the entry's draw: ease-in-out.
fn ease_in_out(x: f32) -> f32 {
    cubic_bezier(0.42, 0., 0.58, 1., x.clamp(0., 1.))
}

fn lerp(from: f32, to: f32, at: f32) -> f32 {
    from + (to - from) * at
}

/// The import view the first launch shows, and what it tells the flow.
pub(super) struct Importing {
    pub(super) view: Entity<ImportView>,
    /// What the last import that finished said.
    pub(super) said: Option<gpui::SharedString>,
    _events: Subscription,
    /// The card draws the view's state itself (bring.rs), so it draws again as the view changes.
    _drawn: Subscription,
}

/// The account step's view, and what it tells the flow.
struct Signing {
    view: Entity<Account>,
    _events: Subscription,
    _synced: Subscription,
}

/// The card while it shows.
pub(super) struct Onboarding {
    /// The steps this run shows, in order: those of STEPS with something to ask.
    steps: Vec<Step>,
    pub(super) step: Step,
    /// Your first Space's name and look, once its step has come.
    pub(super) space: Option<space::SpaceEditor>,
    /// The system it runs on.
    os: Os,
    /// The switch last turned, and when, for its knob's slide.
    switched: Option<(Switch, Instant)>,
    /// That switch's knob has slid all the way, which check.sh hears once.
    slid: bool,
    /// The step's dialog, which holds the keyboard where a step has nothing for it to rest on.
    focus: gpui::FocusHandle,
    /// ⇥'s stops: the main button, the trailing bare button (Skip, Not Now, Just the Browser),
    /// Back, and a row's button.
    continue_stop: gpui::FocusHandle,
    skip_stop: gpui::FocusHandle,
    back_stop: gpui::FocusHandle,
    row_stop: gpui::FocusHandle,
    /// The Welcome's Use Recovery Words, beside its other buttons.
    words_stop: gpui::FocusHandle,
    /// The step that last took the keyboard as it came.
    focused_step: Option<Step>,
    /// Where the switches' rows and knobs are drawn (opt_in_frame).
    frames: Frames,
    /// Where the step shown draws its controls, by name (Shell::place).
    pub(super) places: Places,
    /// Its last frame asked for another: something moves.
    pub(super) drawing: bool,
    /// Where the step's main button is drawn, once the step has settled (button_frame).
    pub(super) button: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where the foot's Skip is drawn, once the step has settled (skip_frame).
    pub(super) skip: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// GPUI's clock, which the card's motion is timed by: the real one in the app, and in a test
    /// the one its timers run on.
    clock: gpui::BackgroundExecutor,
    /// When the step came, which its fade runs from.
    pub(super) since: Instant,
    /// When the entry began, while it plays (v9 §2).
    entry: Option<Instant>,
    /// The card's size and corners the host last shaped the window to, so it's told only of a
    /// change: on the entry's frames, the exit's, and once as it ends.
    shaped: Option<(f32, f32, f32)>,
    /// How many frames the card has drawn, told to the host with each shape for the checks' log of
    /// the frame its glass goes out with.
    drawn: u64,
    /// Help › Welcome plays the card again, which keeps no step to resume at.
    replay: bool,
    /// The assistants whose Sign In was pressed, whose rows then say what the press learned.
    pressed: Vec<crate::agents::runner::Cli>,
    /// The assistants whose Sign In waits for the look its press started, to sign in once it ends.
    signing: Vec<crate::agents::runner::Cli>,
    /// The progress knot's fraction as it last left, the one it's going to, and when it set off.
    progress: (f32, f32, Instant),
    /// When the last step was left, for the card's fade.
    pub(super) ending: Option<Instant>,
    /// The host's answer, once the exit has asked, to whether it grows first run's window.
    grows: Option<bool>,
    /// Whether the exit grows the card to the browser window, as the press that ended the flow
    /// found: the window's content and the new tab's bar, opened then, wait under the card until
    /// the window takes its place.
    grown_exit: bool,
    /// Light or dark where check.sh says, else the window's.
    light: Option<bool>,
    /// The step has come all the way and said so.
    settled: bool,
    /// Keys in first run's window, while the card shows.
    keys: Option<gpui::Subscription>,
    /// First run's window has been on screen, so a step can take its keyboard.
    keyboard: bool,
    /// The account step's view, while the step is offered.
    account: Option<Signing>,
    /// The account came back as one that exists (asking for its words or another device's
    /// approval), so the steps about a new profile's things are left out.
    returning: bool,
    /// When another device added this one, while the Welcome says its Spaces are syncing.
    syncing: Option<Instant>,
    /// The assistant step's rows follow the Agents, as a sign-in under way changes them.
    _assistants: Option<Subscription>,
    /// The assistants' rows the preview shows, as it has no Agents to read.
    shown_assistants: Option<Vec<(crate::agents::runner::Cli, crate::agents::sign_in::Shown)>>,
    /// Help improve as turned at the last step, None while untouched: it's set as the step is left.
    help: Option<bool>,
    /// When the command's line to paste was last copied, which Copy Line says for a moment.
    copied: Option<Instant>,
    /// The default browser's row: shown when the browser wasn't the default as the card began.
    default_row: bool,
    /// Bring your stuff's motion: the flight, the strands as each kind is written, All here.
    bring: bring::BringMotion,
    /// Other apps' picture under its cards.
    ways: other_apps::Motion,
}

impl Onboarding {
    /// Seconds since `at` on the card's clock.
    fn secs_since(&self, at: Instant) -> f32 {
        self.clock.now().saturating_duration_since(at).as_secs_f32()
    }

    /// The steps the progress counts: each that exists, the favorites in the import's place.
    fn slots(&self) -> Vec<Step> {
        (self.steps.iter().copied())
            .filter(|&step| step != Step::Favorites)
            .collect()
    }

    /// How far the progress knot is drawn at `step`: its index over the last index.
    fn fraction(&self, step: Step) -> f32 {
        let step = match step {
            Step::Favorites => Step::Import,
            step => step,
        };
        let slots = self.slots();
        let last = slots.len().saturating_sub(1);
        let at = slots.iter().position(|&each| each == step).unwrap_or(0);
        match last {
            0 => 1.,
            _ => at as f32 / last as f32,
        }
    }

    /// The progress knot's fraction now, 500 ms along its way to the step's; at once when `still`.
    fn progress_now(&self, still: bool) -> f32 {
        let (from, to, since) = self.progress;
        match still {
            true => to,
            false => lerp(
                from,
                to,
                ease_in_out(self.secs_since(since) / t::PROGRESS_MOTION),
            ),
        }
    }

    /// Moves to `step`, its content coming in with a cut and a fade, the progress knot setting off.
    fn show(&mut self, step: Step, still: bool) {
        let now = self.clock.now();
        self.progress = (self.progress_now(still), self.fraction(step), now);
        self.step = step;
        self.since = now;
        self.settled = false;
    }

    /// How long a step's content takes to come.
    fn fade(still: bool) -> f32 {
        if still { t::STILL_FADE } else { t::STEP_FADE }
    }

    /// The entry's length.
    fn entry_length(still: bool) -> f32 {
        let [grow_at, grow_for] = t::ENTRY_GROW;
        let [rest_from, rest_for] = t::ENTRY_REST;
        match still {
            true => t::STILL_FADE,
            false => grow_at + grow_for.max(rest_from + rest_for),
        }
    }

    /// Ends the entry where it is, with the step shown whole.
    fn end_entry(&mut self) {
        if self.entry.take().is_some() {
            let shown = Duration::from_secs_f32(t::STEP_FADE);
            let now = self.clock.now();
            self.since = now.checked_sub(shown).unwrap_or(now);
            self.progress = (self.progress.1, self.progress.1, now);
        }
    }
}

impl Shell {
    /// Starts the card at its first step in first run's window, opening it unless it's open.
    /// `appearance` is check.sh's "light" or "dark", else the window's own.
    pub fn start_onboarding(&mut self, appearance: Option<&str>, cx: &mut Context<Self>) {
        self.begin_onboarding(appearance, false, cx);
    }

    /// Starts the card, as first run or, `replay`, as Help › Welcome plays it again: a replay
    /// neither resumes at a kept step nor keeps one, so one quit halfway never opens first run by
    /// itself at the next launch.
    fn begin_onboarding(&mut self, appearance: Option<&str>, replay: bool, cx: &mut Context<Self>) {
        // Once a window, as an import under way in it goes on; played again, it lists the browsers
        // anew.
        match &mut self.importing {
            Some(importing) => {
                importing.said = None;
                importing.view.update(cx, |view, cx| view.refresh(cx));
            }
            None => {
                let view = cx.new(|_| ImportView::new(self.browser.import_host(), false).guided());
                let events =
                    cx.subscribe(&view, |shell, _, event, cx| shell.import_event(event, cx));
                let drawn = cx.observe(&view, |_, _, cx| cx.notify());
                self.importing = Some(Importing {
                    view,
                    said: None,
                    _events: events,
                    _drawn: drawn,
                });
            }
        }
        let import = self.importing.as_ref().is_some_and(|importing| {
            let view = importing.view.read(cx);
            !view.is_empty() || view.running().is_some()
        });
        let account = self.signing(cx);
        // Claude Code or Codex found here by the folder or file each makes in the user's home, a
        // stat once as the card starts, which runs nothing (floor 1). Only with the command
        // connected does that look also ask the login shell, so then the card keeps to what's held.
        self.browser.start_agents(cx);
        let found_apps = crate::agents::global(cx).is_some_and(|agents| {
            if !agents.read(cx).is_connected(crate::agents::Client::Command) {
                agents.update(cx, |agents, cx| agents.look_for_clients(cx));
            }
            !agents.read(cx).skill_programs().is_empty()
        });
        let steps: Vec<Step> = STEPS
            .into_iter()
            .filter(|&step| match step {
                Step::Account => account.is_some(),
                Step::Assistant => self.browser.threads(cx).is_some(),
                Step::Import => import,
                // An import begun brings it in (import_event).
                Step::Favorites => false,
                Step::Space => true,
                Step::OtherApps => found_apps,
                Step::Done => true,
            })
            .collect();
        // A run left under way resumes at its step, or the next that this launch shows; one out
        // of range is past every step but the last, which always shows.
        let kept = if replay { None } else { resume::read(cx) };
        let first = match kept {
            Some(kept) => (steps.iter().copied())
                .find(|step| step.place() >= kept)
                .unwrap_or(Step::Done),
            None => steps[0],
        };
        if !replay {
            resume::keep(first.place(), cx);
        }
        let numbers: Vec<&str> = steps.iter().map(|step| step.number()).collect();
        // check.sh waits for these lines.
        log!("browser: onboarding started");
        log!("browser: onboarding steps {}", numbers.join(" "));
        log!("browser: onboarding step {}", first.number());
        let now = cx.background_executor().now();
        let mut onboarding = Onboarding {
            steps,
            step: first,
            space: None,
            os: self.browser.os(),
            switched: None,
            slid: false,
            focus: cx.focus_handle(),
            continue_stop: cx.focus_handle().tab_stop(true),
            skip_stop: cx.focus_handle().tab_stop(true),
            back_stop: cx.focus_handle().tab_stop(true),
            row_stop: cx.focus_handle().tab_stop(true),
            words_stop: cx.focus_handle().tab_stop(true),
            focused_step: None,
            frames: Frames::default(),
            places: Places::default(),
            drawing: false,
            button: Rc::default(),
            skip: Rc::default(),
            clock: cx.background_executor().clone(),
            since: now,
            entry: Some(now),
            shaped: None,
            drawn: 0,
            replay,
            pressed: Vec::new(),
            signing: Vec::new(),
            progress: (0., 0., now),
            ending: None,
            grows: None,
            grown_exit: false,
            light: appearance.and_then(|name| match name {
                "light" => Some(true),
                "dark" => Some(false),
                _ => None,
            }),
            settled: false,
            keys: None,
            keyboard: false,
            account,
            returning: matches!(
                account::synced(cx).status,
                Status::NeedsSecret | Status::Pairing
            ),
            syncing: None,
            _assistants: (crate::agents::global(cx)).map(|agents| {
                cx.observe(&agents, |shell, _, cx| {
                    shell.onboarding_sign_in_looked(cx);
                    cx.notify();
                })
            }),
            shown_assistants: None,
            help: None,
            copied: None,
            default_row: !self.browser.is_default_browser(),
            bring: Default::default(),
            ways: Default::default(),
        };
        let fraction = onboarding.fraction(first);
        onboarding.progress = (fraction, fraction, now);
        self.onboarding = Some(onboarding);
        // Opened outside this update, as the window draws the shell's card as it opens.
        let shell = cx.entity();
        cx.defer(move |cx| {
            let open = (shell.read(cx).first_run).filter(|handle| {
                let handle: gpui::AnyWindowHandle = (*handle).into();
                cx.windows().contains(&handle)
            });
            let Some(handle) = open.or_else(|| FirstRun::open(shell.clone(), cx)) else {
                log!("browser: first run's window didn't open");
                shell.update(cx, |shell, cx| shell.first_run_gone(cx));
                return;
            };
            shell.update(cx, |shell, cx| {
                shell.first_run = Some(handle);
                if shell.onboarding.is_some() {
                    shell.browser.show_first_run(handle.into(), cx);
                }
            });
        });
        cx.notify();
    }

    /// First run's window has the keyboard, with the card in it.
    pub(super) fn first_run_key(&self, cx: &App) -> bool {
        self.onboarding.is_some()
            && (self.first_run).is_some_and(|handle| cx.active_window() == Some(handle.into()))
    }

    /// First run is showing, so the browser window it opened at launch waits for it to end.
    pub fn first_run_open(&self) -> bool {
        self.onboarding.is_some()
    }

    /// First run's window went, or never opened, while first run showed: it ends there, without
    /// its exit, and the browser window it kept waiting comes on screen, so no way out of first
    /// run leaves that window held. Not for a quit, which takes every window.
    pub(super) fn first_run_gone(&mut self, cx: &mut Context<Self>) {
        if self.onboarding.is_none() || super::quit_gone_ahead() {
            return;
        }
        self.onboarding = None;
        self.first_run = None;
        // check.sh reads this line.
        log!("browser: first run ended without its exit");
        let browser = self.browser.clone();
        cx.defer(move |cx| browser.end_first_run(None, cx));
        cx.notify();
    }

    /// Settings › Sync's view for the Welcome, following the account as the core says it is; none
    /// in a build that names no server, or with the browser syncing already. An account still
    /// waiting for its words or another device's approval, as a ⌘Q there leaves it, has the
    /// Welcome again, which goes on from there.
    fn signing(&mut self, cx: &mut Context<Self>) -> Option<Signing> {
        let synced = account::synced(cx);
        let host = self
            .browser
            .account_host()
            .filter(|_| synced.status != Status::On)?;
        account::bind_keys(cx);
        let view = cx.new(|cx| {
            let mut view = Account::new(host, cx).as_step();
            view.set_state(synced, cx);
            view
        });
        let events = cx.subscribe(&view, |shell, _, event, cx| match event {
            AccountEvent::Finished if shell.onboarding_on(Step::Account) => {
                shell.onboarding_signed_in(cx)
            }
            AccountEvent::Finished => {}
        });
        let synced = cx.observe_global::<account::Synced>(|shell, cx| {
            let state = account::synced(cx);
            // An account that asks for its words or another device's approval exists already.
            if matches!(state.status, Status::NeedsSecret | Status::Pairing)
                && let Some(onboarding) = shell.onboarding.as_mut()
            {
                onboarding.returning = true;
            }
            if let Some(view) = shell.account_view() {
                view.update(cx, |view, cx| view.set_state(state, cx));
            }
            cx.notify();
        });
        Some(Signing {
            view,
            _events: events,
            _synced: synced,
        })
    }

    /// The account is set up. One another device added says its Spaces are syncing for a moment
    /// first (v9 §3.4); a first device goes on at once from its words.
    fn onboarding_signed_in(&mut self, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        if !onboarding.returning {
            return self.onboarding_next(cx);
        }
        let now = onboarding.clock.now();
        onboarding.syncing = Some(now);
        let hold = onboarding
            .clock
            .timer(Duration::from_secs_f32(t::SYNCING_HOLD));
        cx.spawn(async move |shell, cx| {
            hold.await;
            shell
                .update(cx, |shell, cx| {
                    let held = (shell.onboarding.as_mut())
                        .filter(|onboarding| onboarding.syncing == Some(now))
                        .is_some_and(|onboarding| onboarding.syncing.take().is_some());
                    if held && shell.onboarding_on(Step::Account) {
                        shell.onboarding_next(cx);
                    }
                })
                .ok();
        })
        .detach();
        cx.notify();
    }

    /// What the Welcome shows of the account, as its view says it is.
    fn welcome_shows(&self, cx: &gpui::App) -> Welcome {
        if (self.onboarding.as_ref()).is_some_and(|onboarding| onboarding.syncing.is_some()) {
            return Welcome::Syncing;
        }
        let Some(view) = self.account_view() else {
            return Welcome::Form;
        };
        let view = view.read(cx);
        let state = view.state();
        if view.keeping_words() {
            return Welcome::Words;
        }
        if view.entering().is_some() {
            return Welcome::Restore;
        }
        match state.status {
            Status::SignedOut => Welcome::Form,
            Status::SigningIn => Welcome::SigningIn,
            Status::Pairing => Welcome::Approve,
            // It asks the others as it shows, once.
            Status::NeedsSecret if state.problem.is_none() && !view.asked() => Welcome::Approve,
            Status::NeedsSecret => Welcome::Again,
            Status::FirstDevice => Welcome::Words,
            Status::On => Welcome::Syncing,
        }
    }

    fn account_view(&self) -> Option<Entity<Account>> {
        let signing = self.onboarding.as_ref()?.account.as_ref()?;
        Some(signing.view.clone())
    }

    /// The card shows `step`, and isn't ending.
    fn onboarding_on(&self, step: Step) -> bool {
        self.onboarding
            .as_ref()
            .is_some_and(|o| o.step == step && o.ending.is_none())
    }

    /// The account's recovery words show at the Welcome, which show once, so only their own
    /// button goes on.
    fn keeping_words(&self, cx: &gpui::App) -> bool {
        self.onboarding_on(Step::Account)
            && self
                .account_view()
                .is_some_and(|view| view.read(cx).keeping_words())
    }

    /// Help › Welcome: the card again from its first step, in its own window over this one.
    pub(super) fn welcome_again(&mut self, cx: &mut Context<Self>) {
        self.begin_onboarding(None, true, cx);
    }

    /// Goes on from the step shown, to the next or out of the card after the last. Its line for
    /// check.sh says the step's number alone, never what was picked: the check drives the key.
    pub(super) fn onboarding_next(&mut self, cx: &mut Context<Self>) {
        // The favorites come only while an import has some to offer.
        let favorites = self.importing.as_ref().is_some_and(|importing| {
            (importing.view.read(cx).suggested()).is_some_and(|tabs| !tabs.is_empty())
        });
        // The core reads the account after the card may have started, and it can be signed in by
        // then.
        let signed_in = !signed_out(&account::synced(cx));
        let still = crate::motion::still(cx);
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        onboarding.end_entry();
        let step = onboarding.step;
        log!("browser: onboarding left {}", step.number());
        // Help improve is set as its step is left, whichever way: nothing is sent while the step
        // shows, and turned on and off again it's as it was (#2978).
        let help = (step == Step::Done)
            .then(|| onboarding.help.take())
            .flatten();
        if !favorites {
            (onboarding.steps).retain(|&each| each != Step::Favorites || each == step);
        }
        if signed_in {
            (onboarding.steps).retain(|&each| each != Step::Account || each == step);
        }
        // A returning account's Spaces come back through sync, so what a new profile is asked
        // about them is left out (v9 §0).
        if onboarding.returning {
            onboarding.steps.retain(|&each| {
                !matches!(each, Step::Import | Step::Favorites | Step::Space) || each == step
            });
        }
        let at = onboarding.steps.iter().position(|&each| each == step);
        match at.and_then(|at| onboarding.steps.get(at + 1)).copied() {
            Some(next) => {
                onboarding.show(next, still);
                if !onboarding.replay {
                    resume::keep(next.place(), cx);
                }
                log!("browser: onboarding step {}", next.number());
            }
            // Ended, there's nothing to resume, whether or not a quit then closes the card.
            None => {
                onboarding.ending = Some(onboarding.clock.now());
                // Asked here at the press, never from a draw, so a host that grows the window
                // has a turn to before the grow's first frame; never with Reduce Motion.
                if !still {
                    let grows = self.browser.grows_first_run();
                    // check.sh reads it: the exit's frames are the grow's only when it grows.
                    log!(
                        "browser: onboarding exit {}",
                        if grows { "grows" } else { "fades" }
                    );
                    onboarding.grows = Some(grows);
                }
                onboarding.grown_exit = onboarding.grown(still);
                if !onboarding.replay {
                    resume::clear(cx);
                }
            }
        }
        // Where the card grows to the browser window, the window's first Space and its bar, as ⌘T
        // has it, open now, under the card, so the window takes the card's place with them drawn
        // and they come in with its content (content_shown).
        if self.onboarding.as_ref().is_some_and(|o| o.grown_exit) {
            self.show_hint(cx);
            self.run(Command::NewTab, "onboarding", cx);
        }
        if let Some(on) = help {
            if self.browser.daily_count(cx).is_some_and(|kept| kept != on) {
                self.browser.set_daily_count(on, cx);
            }
            if self
                .browser
                .crash_reports(cx)
                .is_some_and(|kept| kept != on)
            {
                self.browser.set_crash_reports(on, cx);
            }
        }
        cx.notify();
    }

    /// Back to the step before the one shown, its line saying the step's number alone. What was
    /// picked stays picked.
    fn onboarding_back(&mut self, cx: &mut Context<Self>) {
        let still = crate::motion::still(cx);
        let signed_in = !signed_out(&account::synced(cx));
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        let step = onboarding.step;
        // The favorites come after the import has written, so there's no going back over it.
        if step == Step::Favorites {
            return;
        }
        if step == Step::Space {
            self.space_leave(cx);
        }
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        // Signed in since, the Welcome has nothing to ask.
        if signed_in {
            (onboarding.steps).retain(|&each| each != Step::Account || each == step);
        }
        let at = onboarding.steps.iter().position(|&each| each == step);
        let Some(before) = (at.and_then(|at| at.checked_sub(1)))
            .and_then(|at| onboarding.steps.get(at))
            .copied()
            .map(|before| match before {
                Step::Favorites => Step::Import,
                before => before,
            })
        else {
            return;
        };
        onboarding.end_entry();
        log!("browser: onboarding left {}", step.number());
        onboarding.show(before, still);
        if !onboarding.replay {
            resume::keep(before.place(), cx);
        }
        log!("browser: onboarding step {}", before.number());
        cx.notify();
    }

    /// Shows the assistants' step with `rows`, each as a press would leave it, for the preview's
    /// captures: the preview has no Agents for them to come from.
    pub fn onboarding_show_assistants(
        &mut self,
        rows: Vec<(crate::agents::runner::Cli, crate::agents::sign_in::Shown)>,
        cx: &mut Context<Self>,
    ) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            let rows = rows.into_iter();
            let rows = rows.map(|(cli, shown)| (cli, card_words(cli, shown, true)));
            onboarding.shown_assistants = Some(rows.collect());
            cx.notify();
        }
    }

    /// Turns Help improve on at the last step, for the preview's captures.
    pub fn onboarding_count_on(&mut self, cx: &mut Context<Self>) {
        self.onboarding_switch(Switch::Help, cx);
    }

    /// Starts the card in `appearance` at `step`, its number (1 to 7, or 3b), for the preview,
    /// past the entry. The Space step's transparency shows on a host without glass too, so it can
    /// be looked at anywhere.
    pub fn onboarding_at(&mut self, appearance: Option<&str>, step: &str, cx: &mut Context<Self>) {
        if step == Step::Space.number() {
            self.blurred = true;
        }
        self.start_onboarding(appearance, cx);
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.end_entry();
        }
        self.onboarding_to(step, cx);
    }

    /// Goes on through the card to the step numbered `number`, for the preview's captures.
    pub fn onboarding_to(&mut self, number: &str, cx: &mut Context<Self>) {
        while let Some(onboarding) = self.onboarding.as_ref()
            && onboarding.ending.is_none()
            && onboarding.step.number() != number
        {
            self.onboarding_next(cx);
        }
    }

    /// The step's main button: what it offers, then on.
    fn onboarding_continue(&mut self, how: &str, window: &mut Window, cx: &mut Context<Self>) {
        let signed_in = self.assistants_signed_in(cx);
        let Some(onboarding) = self.onboarding.as_ref().filter(|o| o.ending.is_none()) else {
            return;
        };
        match onboarding.step {
            // The import's button does what its view's state is for; the view says when it has
            // read what it brings, which goes on.
            // Import brings the kinds on; with none on it's dimmed, and ↩ does nothing (v9 §5).
            Step::Import => {
                log!("browser: onboarding import button via {how}");
                // Stalled, the step's button is Try Again.
                if (self.bring_view()).is_some_and(|view| view.read(cx).bring_stalled()) {
                    self.bring_try_again(cx);
                    return cx.notify();
                }
                let ready = (self.bring_view()).is_some_and(|view| view.read(cx).bring_ready());
                if ready {
                    self.bring_import(cx);
                }
                return cx.notify();
            }
            // Signed in, the view says so and the card goes on (signing); until then it stays.
            Step::Account => {
                let Some(view) = self.account_view() else {
                    return;
                };
                match self.welcome_shows(cx) {
                    Welcome::Form => {
                        log!("browser: onboarding sign in via {how}");
                        view.update(cx, |view, cx| view.sign_in(window, cx));
                    }
                    Welcome::Words => view.update(cx, |view, cx| view.words_kept(cx)),
                    Welcome::Restore => view.update(cx, |view, cx| view.keep_words(cx)),
                    _ => {}
                }
                return cx.notify();
            }
            // With nobody signed in the step has no main button, so ↩ does nothing there.
            Step::Assistant if !signed_in => return,
            Step::Favorites => {
                let (ticked, space) = (self.favorites_ticked(cx), self.space);
                if let Some(importing) = self.importing.as_ref() {
                    importing
                        .view
                        .update(cx, |view, _| view.add_offered(space, &ticked));
                }
            }
            Step::Space => self.space_keep(cx),
            _ => {}
        }
        self.onboarding_next(cx);
    }

    /// Just the Browser: the threads' switch off, and on. Nothing else, so Other apps' switch is
    /// as it was.
    fn onboarding_just_the_browser(&mut self, cx: &mut Context<Self>) {
        if self.browser.threads(cx) == Some(true) {
            let browser = self.browser.clone();
            // Outside this update, as the host reads the shell as it turns threads off.
            cx.defer(move |cx| browser.set_threads(false, cx));
        }
        self.onboarding_next(cx);
    }

    /// A row's Sign In: nothing of the assistant's program runs before it. A program the browser
    /// already knows signs in at once; else the press looks for that one alone, and it signs in
    /// as the look ends (`onboarding_sign_in_looked`).
    fn onboarding_sign_in(&mut self, cli: crate::agents::runner::Cli, cx: &mut Context<Self>) {
        use crate::agents::ready::Ready;
        let (Some(agents), Some(onboarding)) =
            (crate::agents::global(cx), self.onboarding.as_mut())
        else {
            return;
        };
        if !onboarding.pressed.contains(&cli) {
            onboarding.pressed.push(cli);
        }
        // Signed out it signs in; missing where its maker's installer is known, it installs first
        // (AS1). Otherwise its look comes first, and the row says what that found.
        let known = match agents.read(cx).found(cli) {
            Ready::SignedOut => true,
            Ready::NotFound => cli.installs(),
            _ => false,
        };
        if !known && !onboarding.signing.contains(&cli) {
            onboarding.signing.push(cli);
        }
        agents.update(cx, |agents, cx| match known {
            true => agents.sign_in(cli, cx),
            false => agents.look_up_assistant(cli, cx),
        });
        cx.notify();
    }

    /// Signs in each assistant whose press waited on its look, now that the look has found it
    /// signed out; one found missing or signed in already is left as the look found it.
    fn onboarding_sign_in_looked(&mut self, cx: &mut Context<Self>) {
        use crate::agents::ready::Ready;
        let (Some(agents), Some(onboarding)) =
            (crate::agents::global(cx), self.onboarding.as_mut())
        else {
            return;
        };
        let found = |cli| agents.read(cx).found(cli);
        let (ended, waiting): (Vec<_>, Vec<_>) =
            (onboarding.signing.iter()).partition(|&&cli| found(cli) != Ready::Unknown);
        onboarding.signing = waiting;
        let sign_in: Vec<_> = (ended.into_iter())
            .filter(|&cli| found(cli) == Ready::SignedOut)
            .collect();
        if !sign_in.is_empty() {
            cx.defer(move |cx| {
                agents.update(cx, |agents, cx| {
                    sign_in.into_iter().for_each(|cli| agents.sign_in(cli, cx))
                })
            });
        }
    }

    /// Someone is signed in on a row of the assistant step.
    fn assistants_signed_in(&self, cx: &gpui::App) -> bool {
        use crate::agents::runner::Cli;
        use crate::agents::sign_in::Offer;
        match crate::agents::global(cx) {
            Some(agents) => {
                let agents = agents.read(cx);
                (Cli::ALL.iter()).any(|&cli| agents.assistant_row(cli).offer == Offer::SignedIn)
            }
            None => (self.onboarding.as_ref())
                .and_then(|onboarding| onboarding.shown_assistants.as_ref())
                .is_some_and(|rows| rows.iter().any(|(_, row)| row.offer == Offer::SignedIn)),
        }
    }

    /// What the import view says: an import's Spaces are in, so the step holds on them a moment and
    /// goes on while the rest runs; or it ended; or the user is done with it, which goes on too.
    fn import_event(&mut self, event: &ImportEvent, cx: &mut Context<Self>) {
        if let Some(importing) = self.importing.as_mut() {
            importing.said = match event {
                ImportEvent::Finished(said) => Some(said.clone()),
                _ => importing.said.take(),
            };
        }
        let on_it = self.onboarding_on(Step::Import);
        if let Some(onboarding) = self.onboarding.as_mut().filter(|_| on_it)
            && *event == ImportEvent::Started
            && !onboarding.steps.contains(&Step::Favorites)
        {
            let at = (onboarding.steps.iter()).position(|&each| each == Step::Import);
            (onboarding.steps).insert(at.map_or(0, |at| at + 1), Step::Favorites);
        }
        match event {
            // Its kinds draw in as each is written, and once all are, the card goes on (bring.rs).
            ImportEvent::Finished(_) if on_it => {
                self.bring_heard(cx);
                cx.notify();
            }
            ImportEvent::Done if on_it => self.onboarding_next(cx),
            ImportEvent::Skipped if on_it => self.onboarding_next(cx),
            _ => cx.notify(),
        }
    }

    /// The favorites offered that are ticked, as ↩ adds them.
    fn favorites_ticked(&self, cx: &gpui::App) -> Vec<crate::import::found::Tab> {
        (self.importing.as_ref())
            .map_or_else(Vec::new, |importing| importing.view.read(cx).ticked())
    }

    /// The switches `step` offers, each with whether it's on.
    pub(super) fn switches(&self, step: Step, cx: &gpui::App) -> Vec<(Switch, bool)> {
        let switches: &[Switch] = match step {
            Step::OtherApps => &[Switch::Command, Switch::Mcp],
            Step::Done => &[Switch::Help],
            _ => &[],
        };
        let agents = crate::agents::global(cx);
        let on = |switch| match switch {
            Switch::Command => Some(other_apps::command(agents.as_ref()?.read(cx)).on),
            Switch::Mcp => Some(other_apps::mcp(agents.as_ref()?.read(cx)).on),
            // Offered while either of its settings is; shown as turned, else on only where both
            // are kept on.
            Switch::Help => {
                let (count, crashes) =
                    (self.browser.daily_count(cx), self.browser.crash_reports(cx));
                if count.is_none() && crashes.is_none() {
                    return None;
                }
                let kept = count.unwrap_or(true) && crashes.unwrap_or(true);
                Some(
                    (self.onboarding.as_ref())
                        .and_then(|o| o.help)
                        .unwrap_or(kept),
                )
            }
        };
        (switches.iter())
            .filter_map(|&switch| Some((switch, on(switch)?)))
            .collect()
    }

    /// Turns a switch of the step shown. Other apps' are Settings' own, so they're set at once;
    /// Help improve is set as its step is left.
    pub(super) fn onboarding_switch(&mut self, switch: Switch, cx: &mut Context<Self>) {
        let Some(step) = self.onboarding.as_ref().map(|onboarding| onboarding.step) else {
            return;
        };
        let switches = self.switches(step, cx);
        let Some(&(_, on)) = switches.iter().find(|(each, _)| *each == switch) else {
            return;
        };
        // A way's button presses only while it shows; undoing one is Settings'.
        if matches!(switch, Switch::Command | Switch::Mcp) {
            if !on {
                self.onboarding_way(switch, cx);
            }
            return;
        }
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.switched = Some((switch, onboarding.clock.now()));
            onboarding.slid = false;
            if switch == Switch::Help {
                onboarding.help = Some(!on);
            }
        }
        cx.notify();
    }

    /// A key in first run's window. Any key during the entry ends it. ↩ is the main button, but
    /// never from a field, whose ↩ is its own (and an input method's, which confirms the text
    /// first); ⌘[ goes back; Esc cancels a wait and nothing else; ⌘W does nothing.
    fn onboarding_key(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if ["shift", "control", "alt", "platform", "function"].contains(&key.key.as_str()) {
            return;
        }
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        // The exit has the keyboard to itself: no key presses a step's button as it plays.
        if onboarding.ending.is_some() {
            return;
        }
        if onboarding.entry.is_some() {
            onboarding.end_entry();
            return cx.notify();
        }
        let step = onboarding.step;
        let modifiers = key.modifiers;
        if modifiers.secondary(cx) {
            if key.key == "[" && !modifiers.shift && !modifiers.alt {
                self.onboarding_back(cx);
            }
            return;
        }
        let field = (event.context_stack.iter())
            .any(|context| context.contains("AccountField") || context.contains("TextInput"));
        if field {
            return;
        }
        match (step, key.key.as_str()) {
            (_, "escape") => self.onboarding_escape(cx),
            // Along the palettes, from none chosen → starting at the first and ← at the last.
            (Step::Space, "right") => self.space_palette_by(1, cx),
            (Step::Space, "left") => self.space_palette_by(-1, cx),
            (_, "tab") => match modifiers.shift {
                true => window.focus_prev(cx),
                false => window.focus_next(cx),
            },
            // On a bare button or a row's, ↩ is that button's press, its click.
            (_, "enter") if !modifiers.modified() && !self.onboarding_other_keyed(window) => {
                self.onboarding_continue("return", window, cx)
            }
            _ => {}
        }
    }

    /// Esc: cancels a wait under way, an import or an assistant's sign-in, and otherwise does
    /// nothing, so it can never pass a step or lose the recovery words.
    fn onboarding_escape(&mut self, cx: &mut Context<Self>) {
        use crate::agents::runner::Cli;
        use crate::agents::sign_in::Offer;
        match self.onboarding.as_ref().map(|onboarding| onboarding.step) {
            // The sign-in window, or the wait for another device to add this one.
            Some(Step::Account)
                if matches!(
                    self.welcome_shows(cx),
                    Welcome::SigningIn | Welcome::Approve
                ) =>
            {
                if let Some(view) = self.account_view() {
                    view.update(cx, |view, cx| view.cancel(cx));
                }
            }
            Some(Step::Import) => {
                if let Some(importing) = self.importing.as_ref()
                    && importing.view.read(cx).running().is_some()
                {
                    importing.view.update(cx, |view, cx| view.escape(cx));
                    cx.notify();
                }
            }
            Some(Step::Assistant) => {
                if let Some(agents) = crate::agents::global(cx) {
                    agents.update(cx, |agents, _| {
                        for cli in Cli::ALL {
                            if agents.assistant_row(cli).offer == Offer::Cancel {
                                agents.cancel_sign_in(cli);
                            }
                        }
                    });
                }
            }
            _ => {}
        }
    }

    /// The keyboard is on a bare button or a row's button, which take ↩ and Space as their press.
    fn onboarding_other_keyed(&self, window: &Window) -> bool {
        (self.onboarding.as_ref()).is_some_and(|onboarding| {
            [
                &onboarding.skip_stop,
                &onboarding.back_stop,
                &onboarding.row_stop,
                &onboarding.words_stop,
            ]
            .iter()
            .any(|stop| stop.is_focused(window))
        })
    }

    /// The step shown has a main button: the Welcome while it asks to sign in, the assistants once
    /// someone is signed in, the import in a state with one, and every other step.
    fn onboarding_has_button(&self, step: Step, cx: &gpui::App) -> bool {
        match step {
            Step::Import => (self.bring_view()).is_some_and(|view| view.read(cx).bring_offers()),
            Step::Account => matches!(
                self.welcome_shows(cx),
                Welcome::Form | Welcome::Words | Welcome::Restore
            ),
            Step::Assistant => self.assistants_signed_in(cx),
            _ => true,
        }
    }

    /// Where the keyboard rests as the step shown comes: on its choice where the choice is the
    /// step, on its main button where it has one, else on the step's dialog, so nothing is focused
    /// and ↩ does nothing (v9 §4). Your first Space rests on Next, its arrows going along the
    /// palettes.
    fn onboarding_rest(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        let onboarding = self.onboarding.as_ref()?;
        Some(match onboarding.step {
            step if self.onboarding_has_button(step, cx) => onboarding.continue_stop.clone(),
            _ => onboarding.focus.clone(),
        })
    }

    /// How far the browser window's content has come in, 0 to 1: none while first run's card
    /// grows to the window, which shows its tint alone under it, then in over `reveal` from the
    /// frame the window takes the card's place (first-run v9 §10). Whole otherwise.
    pub(super) fn content_shown(&self) -> f32 {
        if self.onboarding.as_ref().is_some_and(|o| o.grown_exit) {
            return 0.;
        }
        let Some(since) = self.content_since else {
            return 1.;
        };
        let at = self
            .clock
            .now()
            .saturating_duration_since(since)
            .as_secs_f32()
            / crate::tokens::shell::onboarding::exit::REVEAL;
        if at >= 1. { 1. } else { ease_in_out(at) }
    }

    /// The card in first run's `window` while it shows, stepping its motion; none once it has
    /// ended, as the window goes.
    pub(super) fn onboarding_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let rest = self.onboarding_rest(cx);
        let glass = self.blurred;
        let still = crate::motion::still(cx);
        // The grown card ends on the browser window's ground with none of its content, its tint,
        // so the window takes its place unseen.
        let grown_fill = self.window_tint();
        let viewport = window.viewport_size();
        let viewport = (f32::from(viewport.width), f32::from(viewport.height));
        let onboarding = self.onboarding.as_mut()?;
        let light = onboarding.light.unwrap_or_else(|| {
            matches!(
                window.appearance(),
                gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight
            )
        });
        let inks = Inks::of(light);
        let exiting = onboarding.exit_clock(window.is_visible());
        let ended = exiting.is_some_and(|at| onboarding.exit_ended(at, still, viewport));
        if ended {
            // A grown card stays drawn on the frame it ends on, at the window's frame and corners
            // and on its tint alone, and the browser window takes its place whole as that frame
            // is drawn, its content coming in from there: so only one glass is ever on screen,
            // and no frame shows neither. A faded card's last frame draws none.
            let last = onboarding.grown(still).then(|| {
                let plate = exit::frame(onboarding.exit_length(still), true, viewport).plate;
                let (_, _, w, h, radius) = plate;
                onboarding.drawn += 1;
                if onboarding.shaped != Some((w, h, radius)) {
                    self.browser.shape_first_run(w, h, radius, onboarding.drawn);
                }
                log!(
                    "browser: onboarding card frame {} drawn {w:.1} {h:.1}",
                    onboarding.drawn
                );
                (plate, (w, h, radius))
            });
            let grown_exit = onboarding.grown_exit;
            self.onboarding = None;
            if last.is_some() {
                self.content_since = Some(self.clock.now());
            }
            // check.sh waits for this line.
            log!("browser: onboarding done");
            // The window goes and the browser window comes, once this frame is drawn, with the
            // first Space and the command bar up, as ⌘T has it, unless they came at the press.
            // Not for a quit, whose closing window can draw the frame the fade ends in.
            let first_run = self.first_run;
            let shell = cx.entity();
            let opens_bar = !super::quit_gone_ahead() && !grown_exit;
            if opens_bar {
                self.show_hint(cx);
            }
            cx.defer(move |cx| {
                // Outside the shell's update, as the host draws the browser window, and with it
                // the shell, before putting it on screen.
                if let Some(handle) = first_run {
                    let browser = shell.read(cx).browser.clone();
                    browser.end_first_run(Some(handle.into()), cx);
                }
                if opens_bar {
                    shell.update(cx, |shell, cx| shell.run(Command::NewTab, "onboarding", cx));
                }
            });
            // The browser window draws its content's first frame.
            cx.notify();
            #[cfg(test)]
            exit::DRAWN.with_borrow_mut(|drawn| {
                drawn.push(last.map(|(plate, glass)| (plate, grown_fill, glass)))
            });
            return last.map(|(plate, _)| {
                div()
                    .absolute()
                    .inset_0()
                    .child(card::plate(plate, grown_fill, inks.rim()))
                    .into_any_element()
            });
        }
        if onboarding.keys.is_none() {
            let id = window.window_handle().window_id();
            onboarding.keys = Some(cx.observe_keystrokes(move |shell, event, window, cx| {
                if window.window_handle().window_id() == id {
                    shell.onboarding_key(event, window, cx);
                }
            }));
        }
        if window.is_visible() {
            onboarding.keyboard = true;
        }
        // The entry plays from the first frame the window is seen, and ends on its own.
        let entry = match onboarding.entry {
            Some(_) if !onboarding.keyboard => {
                onboarding.entry = Some(onboarding.clock.now());
                Some(0.)
            }
            Some(at) => Some(onboarding.secs_since(at)),
            None => None,
        };
        let entry = entry.filter(|&at| at < Onboarding::entry_length(still));
        if entry.is_none() {
            onboarding.end_entry();
        }
        // Each step takes the keyboard as it comes, so an assistive app reads it; never from a
        // field that has it.
        let resting = window.focused(cx).is_none_or(|focus| {
            [
                &onboarding.focus,
                &onboarding.continue_stop,
                &onboarding.skip_stop,
                &onboarding.back_stop,
                &onboarding.row_stop,
            ]
            .contains(&&focus)
        });
        if let Some(rest) = rest
            && onboarding.keyboard
            && entry.is_none()
            && onboarding.ending.is_none()
            && resting
            && onboarding.focused_step != Some(onboarding.step)
        {
            onboarding.focused_step = Some(onboarding.step);
            window.focus(&rest, cx);
        }
        let step = onboarding.step;
        let fade = Onboarding::fade(still);
        let elapsed = onboarding.secs_since(onboarding.since);
        let came = entry.is_none() && elapsed >= fade;
        if came && !std::mem::replace(&mut onboarding.settled, true) {
            // check.sh waits for this line before it captures the step.
            log!("browser: onboarding step {} settled", step.number());
        }
        let sliding = (onboarding.switched)
            .is_some_and(|(_, at)| onboarding.secs_since(at) < settings_rows::SLIDE);
        // check.sh captures a turned switch on this line, with its knob at the end of its slide.
        if onboarding.switched.is_some()
            && !sliding
            && !std::mem::replace(&mut onboarding.slid, true)
        {
            log!("browser: onboarding switch slid");
        }
        let progressing =
            !still && onboarding.secs_since(onboarding.progress.2) < t::PROGRESS_MOTION;
        let ending = onboarding.ending.map(|at| onboarding.secs_since(at));
        let bringing = onboarding.bring.moving || onboarding.ways.moving;
        onboarding.drawing =
            !came || entry.is_some() || ending.is_some() || sliding || progressing || bringing;
        if onboarding.drawing {
            window.request_animation_frame();
        }
        // Each frame says again where its controls are.
        onboarding.places.borrow_mut().clear();
        // The entry (v9 §2): the plate holds the knot drawing itself, then grows to the card as
        // the knot settles at its place and the rest fades in. With Reduce Motion, a fade.
        let (width, height) = (t::WIDTH, t::HEIGHT);
        let full = (0., 0., width, height, t::RADIUS);
        let (plate, card_opacity, content_opacity, entry_knot) = match entry {
            None => (full, 1., (elapsed / fade).min(1.), None),
            Some(at) if still => {
                let shown = (at / t::STILL_FADE).min(1.);
                (full, shown, shown, None)
            }
            Some(at) => {
                let [plate, plate_radius] = t::ENTRY_PLATE;
                let [grow_at, grow_for] = t::ENTRY_GROW;
                let [rest_from, rest_for] = t::ENTRY_REST;
                let grown = arrive((at - grow_at) / grow_for);
                let (w, h) = (lerp(plate, width, grown), lerp(plate, height, grown));
                let rested = ((at - grow_at - rest_from) / rest_for).clamp(0., 1.);
                let drawn = ease_in_out(at / t::ENTRY_DRAW);
                // The knot lands at the Welcome's place, or where there is none, fades with
                // the plate's growth.
                let [knot, knot_top] = t::WELCOME_KNOT;
                let (to_size, to_y, fades) = match step {
                    Step::Account => (knot, knot_top + knot / 2., false),
                    _ => (t::ENTRY_KNOT, height / 2., true),
                };
                let size = lerp(t::ENTRY_KNOT, to_size, grown);
                let y = lerp(height / 2., to_y, grown);
                let knot = (size, width / 2. - size / 2., y - size / 2., drawn);
                let knot = (knot, if fades { 1. - grown } else { 1. });
                (
                    (
                        (width - w) / 2.,
                        (height - h) / 2.,
                        w,
                        h,
                        lerp(plate_radius, t::RADIUS, grown),
                    ),
                    1.,
                    rested,
                    Some(knot),
                )
            }
        };
        // As the window grows, the card fills it, shaped each frame (first-run v9 §10).
        let exit = exiting.map(|at| exit::frame(at, onboarding.grown(still), viewport));
        let plate = exit.as_ref().map_or(plate, |exit| exit.plate);
        let (_, _, w, h, radius) = plate;
        let shape = (w, h, radius);
        let before = onboarding.shaped.replace(shape);
        onboarding.drawn += 1;
        // The host commits the glass's shape as it's told it, so it goes out with the frame that
        // draws the card in it, and fill and glass grow together.
        if before != Some(shape) {
            self.browser.shape_first_run(w, h, radius, onboarding.drawn);
        }
        // check.sh reads the exit's frames against the host's lines for its glass.
        if exit.is_some() {
            let (_, _, w, h, _) = plate;
            log!(
                "browser: onboarding card frame {} drawn {w:.1} {h:.1}",
                onboarding.drawn
            );
        }
        if step == Step::Space {
            self.space_editor(light, window, cx);
        }
        let content = self.step_content(step, inks, entry.is_some(), window, cx);
        let onboarding = self.onboarding.as_ref()?;
        let entry_knot = entry_knot.map(|((size, x, y, drawn), opacity)| {
            card::at(x, y)
                .opacity(opacity)
                .child(card::knot(size, drawn, inks.text(1.)))
        });
        let opacity = exit.as_ref().map_or(1., |exit| exit.card);
        let content_opacity = content_opacity * exit.as_ref().map_or(1., |exit| exit.content);
        let fill = exit.as_ref().map_or(inks.fill(glass), |exit| {
            exit::crossed(inks.fill(glass), grown_fill, exit.tint)
        });
        #[cfg(test)]
        exit::DRAWN.with_borrow_mut(|drawn| drawn.push(Some((plate, fill, shape))));
        let ends_entry = onboarding.entry.is_some();
        Some(
            div()
                .id("onboarding")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .opacity(opacity * card_opacity)
                .font_family(crate::face::UI)
                // A press on the card away from its controls moves the window, as it has no title
                // bar; each control stops its own press, or the move would take its click.
                // Not while it leaves, when the card may be the whole window.
                .when(exiting.is_none(), |card| {
                    card.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                })
                .child(card::plate(plate, fill, inks.rim()))
                .child(match exit.as_ref().map(|exit| exit.offset) {
                    // Centred in the window as the card is, however the host has grown it.
                    Some((left, top)) => card::at(left, top)
                        .w(px(t::WIDTH))
                        .h(px(t::HEIGHT))
                        .opacity(content_opacity)
                        .child(content),
                    None => div()
                        .absolute()
                        .inset_0()
                        .opacity(content_opacity)
                        .child(content),
                })
                .children(entry_knot)
                // While the card leaves, a click presses nothing on it and moves no window.
                .when(exiting.is_some(), |card| {
                    card.child(
                        div()
                            .id("onboarding-exit")
                            .absolute()
                            .inset_0()
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                    )
                })
                // While the entry plays, a click anywhere ends it and presses nothing under it.
                .when(ends_entry, |card| {
                    card.child(
                        div()
                            .id("onboarding-entry")
                            .absolute()
                            .inset_0()
                            .occlude()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|shell, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(onboarding) = shell.onboarding.as_mut() {
                                        onboarding.end_entry();
                                    }
                                    cx.notify();
                                }),
                            ),
                    )
                })
                .into_any_element(),
        )
    }

    /// A step on the card.
    fn step_content(
        &mut self,
        step: Step,
        inks: Inks,
        entering: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        // Bring your stuff draws what the import view has, over the whole card.
        let bring = matches!(step, Step::Import | Step::Favorites)
            .then(|| self.bring_now(window, cx))
            .flatten();
        // Other apps' ways, and the picture following them.
        let ways = (step == Step::OtherApps)
            .then(|| self.ways_now(window, cx))
            .flatten();
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div().id("onboarding-step");
        };
        let os = onboarding.os;
        let still = crate::motion::still(cx);
        let progress = onboarding.progress_now(still);
        let first = onboarding.steps.first() == Some(&step);
        let focus = onboarding.focus.clone();
        let (title, line) = match (step, self.bring_words(bring.as_ref())) {
            // While an import runs its line names the kind being written, then All here.
            (Step::Import, Some(words)) => (step.message(os).0, words),
            _ => step.message(os),
        };
        let whole = matches!(step, Step::Import | Step::Favorites | Step::OtherApps);
        let said = (title.clone(), line.clone());
        let body = match step {
            Step::Account => self.welcome(inks, entering, window, cx),
            Step::Done => self.done(inks, first, cx),
            _ => {
                let (controls, trail, skip, back) =
                    self.step_parts(step, bring.as_ref(), ways, inks, window, cx);
                let (controls, over) = match whole {
                    true => (div().into_any_element(), Some(controls)),
                    false => (controls, None),
                };
                let span = card::foot_span(step);
                let back = back.filter(|_| !first).map(|back| back.into_any_element());
                let mut trail_parts = Vec::new();
                trail_parts.extend(skip);
                trail_parts.extend(trail);
                // Your first Space's editor starts under its heading, as the mock's does.
                let top = match step {
                    Step::Space => t::SPACE_EDITOR[0],
                    _ => t::BODY_TOP,
                };
                let column = match step.split() {
                    true => span.1 - span.0,
                    false => card::column(step),
                };
                let body = match step.split() {
                    true => card::at(span.0, top).w(px(column)),
                    false => card::at((t::WIDTH - column) / 2., top).w(px(column)),
                };
                let [foot_top, _] = t::FOOT;
                div()
                    .size_full()
                    .children(step.split().then(|| {
                        // The plane is placed absolutely, out of the flow, so the heading after it
                        // keeps its place at the card's top.
                        card::plane(inks)
                            .children(
                                (step == Step::Space)
                                    .then(|| self.space_picture(cx))
                                    .flatten(),
                            )
                            .children(self.place(step, "plane"))
                    }))
                    .child(
                        card::heading((title, line), !step.split(), inks, cx)
                            .size_full()
                            .children(self.place(step, "heading")),
                    )
                    .child(
                        body.h(px(foot_top - top - t::FOOT_GAP))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .flex()
                            .flex_col()
                            .text_size(px(crate::tokens::text::BODY))
                            .text_color(inks.text(1.))
                            .children(self.place(step, "column"))
                            .child(controls),
                    )
                    .children(over)
                    .child(card::foot(span, back, Some(progress), trail_parts, inks))
            }
        };
        // A screen reader reads the step as a dialog called by its title, with its line.
        let named = self.a11y;
        div()
            .id(("onboarding-step", step as usize))
            .absolute()
            .inset_0()
            .track_focus(&focus)
            .when(named, |content| {
                let (title, line) = said;
                content
                    .role(Role::Dialog)
                    .aria_label(title)
                    .when(!line.is_empty(), |content| content.aria_description(line))
            })
            .child(body)
    }

    /// A step's controls, its main button, its Skip and its Back.
    fn step_parts(
        &mut self,
        step: Step,
        bring: Option<&crate::import::Bring>,
        ways: Option<[other_apps::Way; 2]>,
        inks: Inks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (
        AnyElement,
        Option<AnyElement>,
        Option<AnyElement>,
        Option<gpui::Stateful<gpui::Div>>,
    ) {
        let _ = window;
        let running = matches!(
            bring.map(|bring| &bring.state),
            Some(crate::import::Bringing::Running { .. })
        );
        let stoppable = matches!(
            bring.map(|bring| &bring.state),
            Some(crate::import::Bringing::Running {
                stoppable: true,
                done: false,
                ..
            })
        );
        let controls = match step {
            Step::Assistant => self.assistant_rows(step, inks, cx).into_any_element(),
            Step::Import => self.bring_controls(bring, inks, cx),
            Step::Favorites => self.bring_tiles(bring, inks, cx),
            Step::Space => self.space_controls(inks).into_any_element(),
            Step::OtherApps => self.other_apps_ways(ways, inks, cx),
            _ => div().into_any_element(),
        };
        let signed_in = self.assistants_signed_in(cx);
        let ticked = self.favorites_ticked(cx).len();
        let words: Option<SharedString> = match step {
            Step::Assistant if !signed_in => None,
            Step::Import if running => None,
            Step::Import => Some("Import".into()),
            Step::Favorites if ticked > 0 => Some(format!("Add {ticked}").into()),
            _ => Some("Next".into()),
        };
        let ready = (step != Step::Import)
            || (self.bring_view()).is_some_and(|view| view.read(cx).bring_ready());
        let main = words.map(|words| {
            (self.main(step, words, inks, cx))
                .when(!ready, |main| main.opacity(t::BRING_OFF))
                .into_any_element()
        });
        // While an import can still leave everything out, a quiet Cancel stands for the foot; Esc
        // is the same (v9 §5).
        let main = main.or_else(|| {
            (step == Step::Import && stoppable).then(|| {
                card::quiet("onboarding-cancel", "Cancel".into(), inks)
                    .when(self.a11y, |control| named_button(control, "Cancel"))
                    .on_click(cx.listener(|shell, _, _, cx| shell.onboarding_escape(cx)))
                    .relative()
                    .children(self.place(step, "cancel"))
                    .into_any_element()
            })
        });
        // Neither assistant is the foot's quiet button, not a third row (v9 §4).
        let main = main.or_else(|| {
            (step == Step::Assistant).then(|| {
                card::quiet(
                    "onboarding-just-the-browser",
                    "Just the Browser".into(),
                    inks,
                )
                .when(self.a11y, |control| {
                    named_button(control, "Just the Browser")
                })
                .when_some(self.step_stop(step, |o| &o.skip_stop), |control, stop| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_click(cx.listener(|shell, _, _, cx| shell.onboarding_just_the_browser(cx)))
                .relative()
                .children(self.place(step, "just-the-browser"))
                .into_any_element()
            })
        });
        let skip_label = match step {
            Step::Import if !running => Some("Skip"),
            Step::Favorites if ticked > 0 => Some("Skip"),
            _ => None,
        };
        let skip = skip_label.map(|label| {
            let frame = (self.onboarding.as_ref())
                .filter(|onboarding| onboarding.step == step)
                .map(|onboarding| skip_frame(onboarding.skip.clone(), onboarding.settled));
            card::bare("onboarding-skip", label.into(), inks)
                .when(self.a11y, |control| named_button(control, label))
                .when_some(self.step_stop(step, |o| &o.skip_stop), |control, stop| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_click(cx.listener(|shell, _, _, cx| shell.onboarding_next(cx)))
                .relative()
                .children(self.place(step, "skip"))
                .children(frame)
                .into_any_element()
        });
        // None shows while the import runs; the favorites come after the import has written.
        let back = match step {
            Step::Favorites => None,
            Step::Import if running => None,
            _ => Some(self.back(step, inks, cx)),
        };
        (controls, main, skip, back)
    }

    /// A step's handle for a stop, while it's the step shown: the one leaving has none.
    fn step_stop(
        &self,
        step: Step,
        which: impl Fn(&Onboarding) -> &gpui::FocusHandle,
    ) -> Option<gpui::FocusHandle> {
        (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| which(onboarding).clone())
    }

    /// The foot's Back.
    fn back(&self, step: Step, inks: Inks, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        card::bare("onboarding-back", "Back".into(), inks)
            .when(self.a11y, |control| named_button(control, "Back"))
            .when_some(self.step_stop(step, |o| &o.back_stop), |control, stop| {
                control.track_focus(&stop).focus_from_keys_only()
            })
            .on_click(cx.listener(|shell, _, _, cx| shell.onboarding_back(cx)))
            .relative()
            .children(self.place(step, "back"))
    }

    /// The step's main button with `words`.
    fn main(
        &self,
        step: Step,
        words: SharedString,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let drawn = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| (onboarding.button.clone(), onboarding.settled));
        let control = div()
            .id("onboarding-continue")
            .when(self.a11y, |control| named_button(control, words.clone()))
            .when_some(
                self.step_stop(step, |o| &o.continue_stop),
                |control, stop| control.track_focus(&stop).focus_from_keys_only(),
            )
            // ↩ on it is the step's ↩, which goes on as the key goes down (onboarding_key).
            .on_click(cx.listener(|shell, event: &ClickEvent, window, cx| {
                let returned = matches!(
                    event,
                    ClickEvent::Keyboard(KeyboardClickEvent {
                        button: KeyboardButton::Enter,
                        ..
                    })
                );
                if !returned {
                    shell.onboarding_continue("button", window, cx)
                }
            }))
            .child(words)
            .children(drawn.map(|(at, settled)| button_frame(at, settled)))
            .children(self.place(step, "main"));
        let key = super::key_name("enter", super::OFF_THE_MAC);
        card::main_button(control, Some(key.into()), inks)
    }

    /// The Welcome (v9 §3): the knot, the name and its line, then the account as it is: the form
    /// (Continue opens the sign-in window, where the address, the code and the passkey are the
    /// server's page's), a first device's words, another device's approval, or the words' field;
    /// Not Now at the bottom where the step can be passed.
    fn welcome(
        &mut self,
        inks: Inks,
        entering: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if let Some(view) = self.account_view() {
            // The switches' look: its second lines in text-2, its signed-in dot in the ink.
            let look = switch_look(inks);
            view.update(cx, |view, cx| {
                view.set_look(look, cx);
                // Signed in, it asks nothing more (the devices' list is Settings').
                if view.state().status != Status::On {
                    view.showing(window, cx);
                }
            });
        }
        let shows = self.welcome_shows(cx);
        let body = match shows {
            Welcome::Form | Welcome::SigningIn | Welcome::Syncing => {
                self.welcome_form(shows, inks, entering, cx)
            }
            Welcome::Words => self.welcome_words(inks, cx),
            Welcome::Approve | Welcome::Again | Welcome::Restore => {
                self.welcome_approve(shows, inks, cx)
            }
        };
        let bottom = match shows {
            Welcome::Words => Some(
                self.main(Step::Account, "I've Saved Them".into(), inks, cx)
                    .into_any_element(),
            ),
            Welcome::Form | Welcome::SigningIn | Welcome::Again | Welcome::Restore => {
                Some(self.welcome_not_now(inks, cx).into_any_element())
            }
            Welcome::Approve | Welcome::Syncing => None,
        };
        let [_, foot_height] = t::FOOT;
        div().size_full().child(body).children(bottom.map(|bottom| {
            welcome_centred(t::WELCOME_NOT_NOW - foot_height / 2.)
                .h(px(foot_height))
                .items_center()
                .child(bottom)
        }))
    }

    /// Not Now: on without an account, closing a sign-in window still open, so nothing signs in
    /// behind the card.
    fn welcome_not_now(&self, inks: Inks, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        card::bare("onboarding-not-now", "Not Now".into(), inks)
            .when(self.a11y, |control| named_button(control, "Not Now"))
            .when_some(
                self.step_stop(Step::Account, |o| &o.skip_stop),
                |control, stop| control.track_focus(&stop).focus_from_keys_only(),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                if shell.welcome_shows(cx) == Welcome::SigningIn
                    && let Some(view) = shell.account_view()
                {
                    view.update(cx, |view, cx| view.cancel(cx));
                }
                shell.onboarding_next(cx)
            }))
            .relative()
            .children(self.place(Step::Account, "not-now"))
    }

    /// The form, its knot, name and line, and Continue; while the sign-in window is open, what to
    /// do there and Cancel; once another device has added this one, that its Spaces are syncing.
    fn welcome_form(
        &self,
        shows: Welcome,
        inks: Inks,
        entering: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let os = self.onboarding.as_ref().map_or(Os::Mac, |o| o.os);
        let (name, line) = Step::Account.message(os);
        let problem = (self.account_view())
            .and_then(|view| view.read(cx).state().problem)
            .filter(|_| shows == Welcome::Form);
        let line: SharedString = match (shows, problem) {
            (Welcome::Syncing, _) => "Your Spaces are syncing.".into(),
            (_, Some(problem)) => account::problem_text(problem).into(),
            _ => line,
        };
        let [knot, knot_top] = t::WELCOME_KNOT;
        let [name_top, name_size, name_line] = t::WELCOME_NAME;
        let [_, line_size, line_line] = t::LINE;
        let [column, column_top] = t::WELCOME_COLUMN;
        let [new_gap, new_words_gap] = t::WELCOME_NEW;
        let [button_height, _, button_size] = t::WELCOME_BUTTON;
        let column_items: Vec<AnyElement> = match shows {
            // One sign-in window both signs in and makes an account (account.rs sign_in, the
            // core's one call), so there's no Sign Up of its own.
            Welcome::Form => vec![
                card::welcome_button(self.main(Step::Account, "Continue".into(), inks, cx))
                    .into_any_element(),
            ],
            Welcome::SigningIn => {
                let cancel = card::bare("onboarding-cancel", "Cancel".into(), inks)
                    .when(self.a11y, |control| named_button(control, "Cancel Sign In"))
                    .when_some(
                        self.step_stop(Step::Account, |o| &o.row_stop),
                        |control, stop| control.track_focus(&stop).focus_from_keys_only(),
                    )
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(view) = shell.account_view() {
                            view.update(cx, |view, cx| view.cancel(cx));
                        }
                    }))
                    .relative()
                    .children(self.place(Step::Account, "cancel"));
                vec![
                    div()
                        .h(px(button_height))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(button_size))
                        .text_color(inks.text_2())
                        .child(account::FINISH_SIGN_IN)
                        .into_any_element(),
                    welcome_line(new_gap, new_words_gap, inks)
                        .child(cancel)
                        .into_any_element(),
                ]
            }
            _ => Vec::new(),
        };
        div()
            .size_full()
            .when(!entering, |welcome| {
                welcome.child(
                    card::at((t::WIDTH - knot) / 2., knot_top)
                        .child(card::whole_knot(knot, inks.text(1.))),
                )
            })
            .child(
                welcome_centred(name_top)
                    .child(display_text(name, name_size, name_line, cx).text_color(inks.text(1.))),
            )
            .child(
                welcome_centred(t::WELCOME_LINE)
                    .text_size(px(line_size))
                    .line_height(px(line_line))
                    .text_color(inks.text_2())
                    .child(line),
            )
            .child(
                card::at((t::WIDTH - column) / 2., column_top)
                    .w(px(column))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .flex()
                    .flex_col()
                    .text_size(px(crate::tokens::text::BODY))
                    .text_color(inks.text(1.))
                    .children(column_items),
            )
    }

    /// A first device's recovery words (v9 §3.3), shown once: in columns on a plate to write
    /// down, Copy, and the note on where a copy went while it's there. Their main button is at
    /// the bottom (welcome).
    fn welcome_words(&self, inks: Inks, cx: &mut Context<Self>) -> gpui::Div {
        let Some(view) = self.account_view() else {
            return div();
        };
        let now = cx.background_executor().now();
        let (words, (copied, noted)) = {
            let view = view.read(cx);
            let words = view
                .shown_words()
                .map(<[String]>::to_vec)
                .unwrap_or_default();
            (words, view.copied(now))
        };
        let [x, y, width, radius] = t::WORDS_PLATE;
        let [down, across] = t::WORDS_PADDING;
        let [columns, line, size, number] = t::WORDS_GRID;
        let columns = columns as usize;
        let rows = words.len().div_ceil(columns);
        let mono = crate::face::mono(&cx.text_system().all_font_names());
        // Down each column, as they're written down.
        let column = |c: usize| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .children((0..rows).filter_map(|r| {
                    let index = c * rows + r;
                    let word = words.get(index)?;
                    Some(
                        div()
                            .h(px(line))
                            .flex()
                            .child(
                                div()
                                    .w(px(number))
                                    .text_color(inks.text(t::TEXT_3))
                                    .child(format!("{}", index + 1)),
                            )
                            .child(word.clone()),
                    )
                }))
        };
        let copy_words = if copied { "Copied" } else { "Copy" };
        let copy = card::small("onboarding-copy", copy_words.into(), inks)
            .when(self.a11y, |control| {
                named_button(
                    control,
                    match copied {
                        true => "Recovery Words Copied",
                        false => "Copy Recovery Words",
                    },
                )
            })
            .when_some(
                self.step_stop(Step::Account, |o| &o.row_stop),
                |control, stop| control.track_focus(&stop).focus_from_keys_only(),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                if let Some(view) = shell.account_view() {
                    view.update(cx, |view, cx| view.copy_words(cx));
                }
            }))
            .relative()
            .children(self.place(Step::Account, "copy"));
        let [_, line_size, line_line] = t::LINE;
        div()
            .size_full()
            .child(card::heading(
                (
                    "Your recovery words".into(),
                    "The only way back in if you lose every device. Shown once.".into(),
                ),
                true,
                inks,
                cx,
            ))
            .child(
                card::at(x, y)
                    .w(px(width))
                    .rounded(px(radius))
                    .bg(inks.field())
                    .py(px(down))
                    .px(px(across))
                    .flex()
                    .font_family(mono)
                    .text_size(px(size))
                    .line_height(px(line))
                    .text_color(inks.text(1.))
                    .children((0..columns).map(column)),
            )
            .child(welcome_centred(t::WORDS_COPY).child(copy))
            .when(noted, |words| {
                words.child(
                    welcome_centred(t::WORDS_NOTE)
                        .text_size(px(line_size))
                        .line_height(px(line_line))
                        .text_color(inks.text_2())
                        .child(account::COPIED_NOTE),
                )
            })
    }

    /// A known account on a new device (v9 §3.4): Approve this Mac with the code a signed-in
    /// device shows, while it waits; once none is asked, why, with Ask Again; and the recovery
    /// words' field, Use Recovery Words' way instead.
    fn welcome_approve(&self, shows: Welcome, inks: Inks, cx: &mut Context<Self>) -> gpui::Div {
        let Some(view) = self.account_view() else {
            return div();
        };
        let os = self.onboarding.as_ref().map_or(Os::Mac, |o| o.os);
        let (state, entering) = {
            let view = view.read(cx);
            (view.state().clone(), view.entering())
        };
        let [knot, knot_top] = t::WELCOME_KNOT;
        let [code_top, code_size, code_line] = t::APPROVE_CODE;
        let [_, line_size, line_line] = t::LINE;
        let [column, _] = t::WELCOME_COLUMN;
        let [gap, _] = t::WELCOME_NEW;
        let title: SharedString = match shows {
            Welcome::Restore => "Enter your recovery words".into(),
            _ => format!("Approve {}", here(os)).into(),
        };
        let line: SharedString = match shows {
            Welcome::Approve => "Check that a signed-in device shows:".into(),
            _ => "".into(),
        };
        let said_line = |text: SharedString| {
            welcome_centred(t::APPROVE_LINE)
                .text_size(px(line_size))
                .line_height(px(line_line))
                .text_color(inks.text_2())
                .child(text)
        };
        let use_words = card::bare("onboarding-use-words", "Use Recovery Words".into(), inks)
            .when(self.a11y, |control| {
                named_button(control, "Use Recovery Words")
            })
            .when_some(
                self.step_stop(Step::Account, |o| &o.words_stop),
                |control, stop| control.track_focus(&stop).focus_from_keys_only(),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                if let Some(view) = shell.account_view() {
                    view.update(cx, |view, cx| view.enter_words(cx));
                }
            }))
            .relative()
            .children(self.place(Step::Account, "use-words"));
        let mut parts: Vec<AnyElement> = Vec::new();
        match shows {
            Welcome::Approve => {
                // The code once a device has answered, the build's blanks until then.
                let code = match state.code.is_empty() {
                    true => card::heading_text("––– –––", code_size, code_line, cx)
                        .text_color(inks.text(t::TEXT_3)),
                    false => {
                        card::heading_text(account::grouped(&state.code), code_size, code_line, cx)
                            .text_color(inks.text(1.))
                    }
                };
                parts.push(welcome_centred(code_top).child(code).into_any_element());
                parts.push(said_line("Waiting…".into()).into_any_element());
                parts.push(
                    welcome_buttons(t::APPROVE_BUTTONS)
                        .child(use_words)
                        .into_any_element(),
                );
            }
            Welcome::Again => {
                let why = state
                    .problem
                    .map_or(account::ASK_ANOTHER, account::problem_text);
                let again = card::quiet("onboarding-ask-again", "Ask Again".into(), inks)
                    .when(self.a11y, |control| named_button(control, "Ask Again"))
                    .when_some(
                        self.step_stop(Step::Account, |o| &o.row_stop),
                        |control, stop| control.track_focus(&stop).focus_from_keys_only(),
                    )
                    .on_click(cx.listener(|shell, _, _, cx| {
                        if let Some(view) = shell.account_view() {
                            view.update(cx, |view, cx| view.ask_again(cx));
                        }
                    }))
                    .relative()
                    .children(self.place(Step::Account, "ask-again"));
                parts.push(said_line(why.into()).into_any_element());
                parts.push(
                    welcome_buttons(t::APPROVE_BUTTONS)
                        .child(again)
                        .child(use_words)
                        .into_any_element(),
                );
            }
            Welcome::Restore => {
                let [field_top, field_height, field_radius, field_inset] = t::RESTORE_FIELD;
                let refused = entering.as_ref().is_some_and(|(_, refused)| *refused);
                let why = match (refused, state.problem) {
                    (true, _) => Some(account::REFUSED_WORDS),
                    (false, Some(Problem::WrongWords)) => {
                        Some(account::problem_text(Problem::WrongWords))
                    }
                    _ => None,
                };
                let field = entering.map(|(input, _)| {
                    div()
                        .h(px(field_height))
                        .px(px(field_inset))
                        .flex()
                        .items_center()
                        .rounded(px(field_radius))
                        .bg(inks.field())
                        .key_context("AccountField")
                        .on_action(cx.listener(|shell, _: &account::Keep, _, cx| {
                            let Some(view) = shell.account_view() else {
                                return;
                            };
                            let field = view.read(cx).open_field();
                            if crate::dismissal::submits(&field, cx) {
                                view.update(cx, |view, cx| view.keep_words(cx));
                            }
                        }))
                        .on_action(cx.listener(|shell, _: &account::Leave, window, cx| {
                            let Some(view) = shell.account_view() else {
                                return;
                            };
                            let field = view.read(cx).open_field();
                            if crate::dismissal::Close::escape_from(&field, cx).is_some() {
                                view.update(cx, |view, cx| view.leave_field(window, cx));
                            }
                        }))
                        .text_size(px(crate::tokens::text::BODY))
                        .text_color(inks.text(1.))
                        .relative()
                        .children(self.place(Step::Account, "words-field"))
                        .child(input)
                });
                parts.push(
                    card::at((t::WIDTH - column) / 2., field_top)
                        .w(px(column))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .flex()
                        .flex_col()
                        .gap(px(gap / 2.))
                        .children(field)
                        .child(card::welcome_button(self.main(
                            Step::Account,
                            "Restore".into(),
                            inks,
                            cx,
                        )))
                        .into_any_element(),
                );
                parts.extend(why.map(|why| said_line(why.into()).into_any_element()));
            }
            _ => {}
        }
        div()
            .size_full()
            .child(
                card::at((t::WIDTH - knot) / 2., knot_top)
                    .child(card::whole_knot(knot, inks.text(1.))),
            )
            .child(card::heading_at(
                (title, line),
                true,
                t::APPROVE_HEADING,
                inks,
                cx,
            ))
            .children(parts)
    }

    /// You're woven in (v9 §9): the knot whole, the heading, the default browser's row and Help
    /// improve, then Start Browsing on the foot's line, Back at its start.
    fn done(&mut self, inks: Inks, first: bool, cx: &mut Context<Self>) -> gpui::Div {
        let os = self.onboarding.as_ref().map_or(Os::Mac, |o| o.os);
        let (title, _) = Step::Done.message(os);
        let [knot, knot_top] = t::DONE_KNOT;
        let [_, _, size, height] = t::HEADING;
        let [group_top, group_width] = t::DONE_GROUP;
        let [foot_top, foot_height] = t::FOOT;
        let mut rows: Vec<AnyElement> = Vec::new();
        let shown = self.onboarding.as_ref().is_some_and(|o| o.default_row);
        if shown {
            rows.push(self.default_row(os, inks, cx).into_any_element());
        }
        rows.extend(
            (self.switch_list(Step::Done, inks, cx).into_iter()).map(IntoElement::into_any_element),
        );
        let main = self
            .main(Step::Done, "Start Browsing".into(), inks, cx)
            .into_any_element();
        let back = (!first).then(|| self.back(Step::Done, inks, cx));
        div()
            .size_full()
            .child(
                card::at((t::WIDTH - knot) / 2., knot_top)
                    .child(card::whole_knot(knot, inks.text(1.))),
            )
            .child(
                card::at(0., t::DONE_HEADING)
                    .w(px(t::WIDTH))
                    .flex()
                    .justify_center()
                    .child(card::heading_text(title, size, height, cx).text_color(inks.text(1.))),
            )
            .when(!rows.is_empty(), |done| {
                done.child(
                    card::at((t::WIDTH - group_width) / 2., group_top).child(card::rows(
                        rows,
                        group_width,
                        inks,
                    )),
                )
            })
            .child(
                card::at(0., foot_top)
                    .w(px(t::WIDTH))
                    .h(px(foot_height))
                    .flex()
                    .justify_center()
                    .child(main),
            )
            .children(back.map(|back| {
                card::at(t::FOOT_SIDE, foot_top)
                    .h(px(foot_height))
                    .child(back)
            }))
    }

    /// The default browser's row: Make Default while the system can be asked, a check disc once the
    /// browser is the default; where the system can't be asked, where the desktop sets it.
    fn default_row(&self, os: Os, inks: Inks, cx: &mut Context<Self>) -> gpui::Div {
        let is = self.browser.is_default_browser();
        let offers = default_browser::offers(os, false);
        let (name, words): (SharedString, SharedString) = match is {
            true => (
                "Default browser".into(),
                format!("{PRODUCT} is your default browser.").into(),
            ),
            false if offers => (
                format!("Make {PRODUCT} your default browser").into(),
                "Links from other apps open here.".into(),
            ),
            false => (
                format!("Make {PRODUCT} your default browser").into(),
                default_browser::detail(os, false).into(),
            ),
        };
        let end = match (is, offers) {
            (true, _) => Some(card::check_disc(inks).into_any_element()),
            (false, true) => Some(
                card::quiet("onboarding-make-default", "Make Default".into(), inks)
                    .when(self.a11y, |control| named_button(control, "Make Default"))
                    .when_some(
                        self.step_stop(Step::Done, |o| &o.row_stop),
                        |control, stop| control.track_focus(&stop).focus_from_keys_only(),
                    )
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.browser.make_default_browser(cx);
                        cx.notify();
                    }))
                    .relative()
                    .children(self.place(Step::Done, "make-default"))
                    .into_any_element(),
            ),
            (false, false) => None,
        };
        card::row(name, words, end, inks)
    }

    /// A row each of the step's switches: its name over its words, and Settings' switch at the
    /// end. A click anywhere on a row, the switch included, turns it.
    fn switch_list(&self, step: Step, inks: Inks, cx: &mut Context<Self>) -> Vec<gpui::Div> {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return Vec::new();
        };
        let switched =
            (onboarding.switched).map(|(turned, at)| (turned, onboarding.secs_since(at)));
        let settled = onboarding.step == step && onboarding.settled;
        let frames = onboarding.frames.clone();
        let look = switch_look(inks);
        let a11y = self.a11y;
        (self.switches(step, cx).into_iter())
            .map(|(switch, on)| {
                let slid = match switched {
                    Some((turned, since)) if turned == switch => {
                        (since / settings_rows::SLIDE).min(1.)
                    }
                    _ => 1.,
                };
                let (name, words) = switch.words();
                let control = div()
                    .relative()
                    .flex_none()
                    .child(
                        stop_switch(&look, "switch", name.clone(), on, slid, look.on, a11y)
                            // A press is its row's click; here only Space, as ↩ is the step's.
                            .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                                if matches!(
                                    event,
                                    ClickEvent::Keyboard(KeyboardClickEvent {
                                        button: KeyboardButton::Space,
                                        ..
                                    })
                                ) {
                                    shell.onboarding_switch(switch, cx);
                                }
                            })),
                    )
                    .child(opt_in_frame(frames.clone(), switch, "switch", settled))
                    .into_any_element();
                div()
                    .id(switch.name())
                    .relative()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(
                        cx.listener(move |shell, _, _, cx| shell.onboarding_switch(switch, cx)),
                    )
                    .child(card::row(name, words, Some(control), inks))
                    .child(opt_in_frame(frames.clone(), switch, "row", settled))
            })
            .map(|row| div().child(row))
            .collect()
    }

    /// The assistants, as Settings › Models has them, in first run's form of its row (v9 §4): each
    /// signs in with its maker's own sign-in. Without the app's Agents, as in a test's shell, the
    /// rows the preview gave, or none.
    fn assistant_rows(&self, step: Step, inks: Inks, cx: &mut Context<Self>) -> gpui::Div {
        use crate::agents::runner::Cli;
        use crate::settings_rows::{Act, first_run_row};
        // The line in text-2 and the mark in the text's ink (v9 §1, §4): mixed over the card's
        // see-through ground, as Look::on would, they'd all but vanish.
        let look = Look {
            detail: inks.text_2(),
            glyph: inks.text(1.),
            ..Look::on(inks.group(), inks.text(1.), rgb(SIGNAL))
        };
        let shell = cx.weak_entity();
        let a11y = self.a11y;
        let rows = crate::agents::global(cx).map(|agents| {
            let model = agents.read(cx);
            let pressed = (self.onboarding.as_ref()).map_or(&[][..], |o| &o.pressed[..]);
            let shown = Cli::ALL.map(|cli| (cli, card_row(model, cli, pressed.contains(&cli))));
            let rows = shown.map(|(cli, shown)| {
                let (agents, shell) = (agents.clone(), shell.clone());
                // As Settings' rows do. The card draws neither the mark nor a ⋯ (#4448), so
                // nothing here opens Settings.
                let act = move |act: Act, _: &mut Window, cx: &mut gpui::App| match act {
                    Act::Get => {
                        let url = [cli.install_page().to_string()];
                        let open = |shell: &mut Shell, cx: &mut Context<Shell>| {
                            shell.browser.open_at(&url, false, 0, 0, cx);
                        };
                        shell.update(cx, open).ok();
                    }
                    Act::SignIn => {
                        shell
                            .update(cx, |shell, cx| shell.onboarding_sign_in(cli, cx))
                            .ok();
                    }
                    Act::Cancel => agents.update(cx, |agents, _| agents.cancel_sign_in(cli)),
                    Act::Mark | Act::Menu => {}
                };
                first_run_row(
                    &look,
                    cli,
                    &shown,
                    a11y,
                    card::check_disc(inks),
                    Rc::new(act),
                )
                .children(self.place(step, cli.id()))
            });
            group(&look, rows)
        });
        let rows = rows.or_else(|| {
            let shown = self.onboarding.as_ref()?.shown_assistants.clone()?;
            let rows = shown.iter().map(|(cli, shown)| {
                let act = Rc::new(|_: Act, _: &mut Window, _: &mut gpui::App| {});
                first_run_row(&look, *cli, shown, a11y, card::check_disc(inks), act)
                    .children(self.place(step, cli.id()))
            });
            Some(group(&look, rows))
        });
        div()
            .w_full()
            .flex()
            .flex_col()
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
            .children(rows)
    }

    /// Keeps where the step shown draws `name`, as much of it as shows, for a test to click it.
    pub(super) fn place(&self, step: Step, name: &'static str) -> Option<impl IntoElement + use<>> {
        let places = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| onboarding.places.clone())?;
        let frame = canvas(
            move |bounds, window, _| {
                let shown = bounds.intersect(&window.content_mask().bounds);
                places.borrow_mut().insert(name, shown);
            },
            |_, _, _, _| {},
        );
        Some(frame.absolute().inset_0())
    }
}

/// Keeps where the step's button is drawn once the step has `settled`, for a test to click it.
fn button_frame(at: Rc<Cell<Option<Bounds<Pixels>>>>, settled: bool) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if settled {
                at.set(Some(bounds));
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Keeps where the foot's Skip is drawn once the step has `settled`, and logs where it is as each
/// step settles and each time it moves, as "browser: onboarding skip at x y", for check.sh to click
/// it there.
fn skip_frame(at: Rc<Cell<Option<Bounds<Pixels>>>>, settled: bool) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if !settled {
                at.set(None);
            } else if at.replace(Some(bounds)) != Some(bounds) {
                let centre = bounds.center();
                log!(
                    "browser: onboarding skip at {:.0} {:.0}",
                    f32::from(centre.x),
                    f32::from(centre.y)
                );
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Where the step shown draws a control, by its name (Shell::place).
pub(super) type Places = Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>;

/// Where each switch's row and knob is drawn, by the switch and "row" or "switch".
type Frames = Rc<RefCell<HashMap<(Switch, &'static str), Bounds<Pixels>>>>;

/// Keeps where a switch's `part` is drawn once the step has `settled`, and logs where its knob is
/// each time it moves, as "browser: onboarding command at x y", for check.sh to click it there.
fn opt_in_frame(
    frames: Frames,
    switch: Switch,
    part: &'static str,
    settled: bool,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let moved =
                settled && frames.borrow_mut().insert((switch, part), bounds) != Some(bounds);
            if moved && matches!(part, "switch" | "button") {
                let at = bounds.center();
                log!(
                    "browser: onboarding {} at {:.0} {:.0}",
                    switch.name(),
                    f32::from(at.x),
                    f32::from(at.y)
                );
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// First run's main button for the welcome card, outside the card: `button` with its label in it
/// and `key` at its end, if it has one, in a light window's inks or a dark one's.
pub(super) fn main_button(
    button: gpui::Stateful<gpui::Div>,
    key: Option<SharedString>,
    light: bool,
) -> gpui::Stateful<gpui::Div> {
    card::main_button(button, key, card::Inks::of(light))
}

fn with_alpha(color: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(color)
    }
}

#[cfg(test)]
mod tests;

/// An assistant's row on the card. Until its Sign In is pressed it draws from what the browser
/// already holds, running nothing: its maker's plan and Sign In, or the sign-in the browser knows
/// of. Once pressed, it says what the press learned, in the card's words.
fn card_row(
    agents: &crate::agents::Agents,
    cli: crate::agents::runner::Cli,
    pressed: bool,
) -> crate::agents::sign_in::Shown {
    use crate::agents::{ready::Ready, sign_in::Offer};
    let shown = agents.assistant_row(cli);
    let known = matches!(
        shown.offer,
        Offer::Cancel | Offer::SignedIn | Offer::TryAgain | Offer::TryInstall
    ) || agents.found(cli) == Ready::SignedIn;
    card_words(cli, shown, pressed || known)
}

/// What an assistant's row says on the card (v9 §4), from what Settings › Models' row would:
/// its maker's plan until `learned` is what the browser holds or a press found, then the wait,
/// a failed or closed sign-in, or its maker's plan again once signed in, as no use is reported.
/// A program its row installs says so, as Settings' does (AS2–AS4); Settings' other words (a
/// program with no installer known, a sign-in that needs a terminal) aren't the card's: its Sign In
/// does what Settings' does.
fn card_words(
    cli: crate::agents::runner::Cli,
    shown: crate::agents::sign_in::Shown,
    learned: bool,
) -> crate::agents::sign_in::Shown {
    use crate::agents::sign_in::Offer;
    let offer = match learned {
        true => shown.offer,
        false => Offer::SignIn,
    };
    let line = match offer {
        // The wait's own line, which Settings › Models says too (FR19), and an install's (AS2–AS4).
        Offer::Cancel | Offer::Install | Offer::TryInstall => shown.line,
        Offer::TryAgain => "Sign-in didn't finish.".into(),
        _ => format!("Your {} plan", cli.account_name()),
    };
    crate::agents::sign_in::Shown { line, offer }
}
