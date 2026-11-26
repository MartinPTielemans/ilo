//! A new profile's first launch (spec-first-run.md): a welcome, then one step a screen, in a window
//! of its own (window.rs) that is the only one on screen until it ends. Only a launch with nothing
//! to bring back starts it, and Help › Welcome plays it again, so a normal launch never draws or
//! loads any of it. ↩ is a step's button, Skip goes on from it and Esc goes back; the last step's
//! button, Skip Setup and the window's close end the flow, and the browser window comes. A step's
//! button does what it offers, such as asking the system to make the browser the default, giving
//! the first Space its colour or signing in to sync; Skip doesn't. The flow ends with the command
//! bar up over the first Space, and a line on the card under it saying what the bar is.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Entity, KeyboardButton, KeyboardClickEvent,
    KeystrokeEvent, MouseButton, Pixels, Rgba, SharedString, Subscription, Window,
    accesskit::{Role, Toggled},
    canvas, div,
    prelude::*,
    px, rgb,
};

use super::{Command, Setting, Shell, cubic_bezier, display_face::display_text, fade_at};
use crate::account::{self, Account, AccountEvent, Status};
use crate::default_browser::{self, Os};
use crate::import::{Guide, ImportEvent, ImportView, Look};
use crate::settings_rows::{self, SLIDE, group, named_button, ring, stop_switch};
use crate::theme::Theme;
use crate::theme_picker::{SWATCHES, swatch_name};
use crate::tokens::palette::{INK, PAPER};
use crate::tokens::settings::PADDING;
use crate::tokens::shell::onboarding::ENGINE_DOT;
use crate::tokens::shell::onboarding::ENGINE_DOT_CENTER;
use crate::tokens::shell::onboarding::ENGINE_ROW;
use crate::tokens::shell::onboarding::IMPORT_HOLD;
use crate::tokens::shell::onboarding::IMPORT_SURFACE;
use crate::tokens::shell::onboarding::OPT_IN_LINE;
use crate::tokens::shell::onboarding::QUIET;
use crate::tokens::shell::onboarding::SIGNAL;
use crate::tokens::shell::onboarding::SWATCH_EDGE;
use crate::tokens::shell::onboarding::SWATCH_GAP;
use crate::tokens::shell::onboarding::SWATCH_RING;
use crate::tokens::shell::onboarding::window as frame_tokens;
use crate::transparency::{self, Transparency};

mod frame;
mod picture;
mod window;

pub(super) use window::FirstRun;

/// The flow's steps, in the order they come.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Step {
    Intro,
    /// What comes along from another browser: the import view (crate::import), which Settings ›
    /// Import shows too; left out when it finds no browser to import from.
    Import,
    /// The sites visited most in the history an import read, ticked, to make favorites (owner,
    /// 2026-10-29); there only once an import has read them, and left out when it offers none.
    Favorites,
    /// Asks the system to open links with ilo; left out when it already does.
    DefaultBrowser,
    /// The first profile's search engine, from Settings' list; left out when it comes while the
    /// core has none to offer, as while a policy picks it.
    SearchEngine,
    /// The first Space's colour, from the theme picker's swatches.
    Color,
    /// How much of what's behind the window shows through its glass: the app's one level, which
    /// the window takes at once. Left out where nothing blurs what's behind the window.
    Transparency,
    /// How long today's tabs wait before they're put away, as a bar of Settings' times, and Settings'
    /// Reopen your windows and tabs; left out until the core has the profile's time.
    Tidy,
    /// Settings' ad blocking and its offer to save passwords, as they are, each turned here at once;
    /// left out when Settings offers neither.
    Private,
    /// Signing in to sync: Settings › Sync's view (crate::account); left out in a build that names
    /// no sync server, and once the browser is signed in.
    Account,
    /// The assistants tasks run on, as Settings › Agents' Your assistant has them (#4093), each
    /// signing in with its maker's own sign-in.
    Assistant,
    /// Settings' opt-in switches, both off; left out when neither is offered.
    OptIns,
    /// The three keys that carry most of it, as the app's keys are then, and the way in.
    Keys,
}

/// In the drawing's order: the assistant asked early (the owner's), then what comes along, the
/// browser's own settings, the Space's look, sync, the opt-ins and the keys.
const STEPS: [Step; 13] = [
    Step::Intro,
    Step::Assistant,
    Step::Import,
    Step::Favorites,
    Step::DefaultBrowser,
    Step::SearchEngine,
    Step::Color,
    // After the colour, as a Space's glow reads through the same glass.
    Step::Transparency,
    Step::Tidy,
    Step::Private,
    Step::Account,
    Step::OptIns,
    Step::Keys,
];

/// A switch the first launch shows, each Settings' own.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Switch {
    /// Off until the user turns it on, and set only as they leave the step, so nothing is sent
    /// while it shows (#2978).
    DailyCount,
    CrashReports,
    /// Settings › Other apps' switch, on both paths: an outside app is another kind of agent
    /// from the assistant the assistant step asks about, so that answer doesn't touch it.
    OtherApps,
    /// One the host reads and sets for the Space's profile, turned at once.
    Setting(Setting),
}

impl Switch {
    /// The switches `step` shows, in its order: the opt-ins' and the tidy and private steps'.
    // The private step's cookie notices wait for a row in Settings, and its secure connections
    // for a setting in the build.
    fn of(step: Step) -> &'static [Switch] {
        match step {
            Step::OptIns => &[Switch::DailyCount, Switch::CrashReports, Switch::OtherApps],
            Step::Tidy => &[Switch::Setting(Setting::RestoreWindows)],
            Step::Private => &[
                Switch::Setting(Setting::AdBlocking),
                Switch::Setting(Setting::SavePasswords),
            ],
            _ => &[],
        }
    }

    /// What check.sh calls it.
    fn name(self) -> &'static str {
        match self {
            Switch::DailyCount => "daily-count",
            Switch::CrashReports => "crash-reports",
            Switch::OtherApps => "other-apps",
            Switch::Setting(Setting::RestoreWindows) => "restore-windows",
            Switch::Setting(Setting::AdBlocking) => "ad-blocking",
            Switch::Setting(Setting::SavePasswords) => "save-passwords",
        }
    }

    /// Its switch's name and its one sentence, if it has one: the opt-ins' the names Settings'
    /// rows have, as they're those switches, and the drawing's for the others.
    fn words(self) -> (&'static str, SharedString) {
        match self {
            Switch::Setting(Setting::RestoreWindows) => ("Open where you left off", "".into()),
            Switch::Setting(Setting::AdBlocking) => {
                ("Block ads and trackers", "Built in, on every site".into())
            }
            // Settings' name: the drawing's "Save and fill" and its Touch ID aren't what it sets.
            // The spec gives only the Mac's line, so the others go without.
            Switch::Setting(Setting::SavePasswords) => (
                "Offer to save passwords",
                if cfg!(target_os = "macos") {
                    "Kept on this Mac"
                } else {
                    ""
                }
                .into(),
            ),
            Switch::DailyCount => (
                "Send a daily usage count",
                crate::daily_count::words(env!("PRODUCT_NAME")).into(),
            ),
            Switch::OtherApps => (
                concat!("Let other apps' agents use ", env!("APP_NAME")),
                "Editors, terminal tools and scripts on this computer".into(),
            ),
            Switch::CrashReports => (
                "Send crash reports",
                concat!(
                    "When ",
                    env!("PRODUCT_NAME"),
                    " crashes, it sends a report of what went wrong, so it can be fixed."
                )
                .into(),
            ),
        }
    }
}

impl Step {
    /// What check.sh calls it.
    fn name(self) -> &'static str {
        match self {
            Step::Intro => "intro",
            Step::Import => "import",
            Step::Favorites => "favorites",
            Step::DefaultBrowser => "default-browser",
            Step::SearchEngine => "search-engine",
            Step::Color => "color",
            Step::Transparency => "transparency",
            Step::Tidy => "tidy",
            Step::Private => "private",
            Step::Account => "account",
            Step::Assistant => "assistant",
            Step::OptIns => "opt-ins",
            Step::Keys => "keys",
        }
    }

    /// The step's one message on `os`: its title, its line and its button's words.
    fn message(self, os: Os) -> (&'static str, gpui::SharedString, &'static str) {
        let (title, line, button) = match self {
            // The sheet's first page (#4093): what the flow is, and that each step can be skipped.
            Step::Intro => (
                concat!("Welcome to ", env!("PRODUCT_NAME"), "."),
                "A few steps to make it yours, and you can skip any of them.",
                "Get Started",
            ),
            // The import view's own buttons are the step's.
            Step::Import => (
                "Bring your stuff.",
                "Spaces, pinned tabs, folders and favorites from the browser you used.",
                "",
            ),
            // Worked out from the pages the import read, here; the button is Continue while none
            // is ticked (favorites_button).
            Step::Favorites => {
                let here = match os {
                    Os::Mac => "this Mac",
                    _ => "this computer",
                };
                let line =
                    format!("From your history, worked out on {here}. Nothing is sent anywhere.");
                return ("Your favorites.", line.into(), "Add Favorites");
            }
            // What the system does and its way back are Settings' row's, each system's own. A
            // Flatpak can't set it, so the step says where the desktop does, and goes on.
            Step::DefaultBrowser => {
                let line = format!(
                    "Links from other apps open here. {}",
                    default_browser::detail(os, false)
                );
                let button = match default_browser::offers(os, false) {
                    true => "Make Default",
                    false => "Continue",
                };
                let title = concat!("Make ", env!("PRODUCT_NAME"), " your default browser.");
                return (title, line.into(), button);
            }
            // The engine that searches until one is picked is already the filled dot (P-search).
            Step::SearchEngine => (
                "Pick a search engine.",
                "You can change it later in Settings.",
                "Continue",
            ),
            Step::Color => (
                "Pick a color for your first Space.",
                "The window takes it. Each Space can have its own, from its name in the sidebar.",
                "Continue",
            ),
            Step::Transparency => (
                "How much should show through?",
                // D5: over a light desktop the window changes too little to see "right away".
                concat!(
                    env!("PRODUCT_NAME"),
                    "'s window is glass, blurring what's behind it."
                ),
                "Continue",
            ),
            Step::Tidy => (
                "Tabs that tidy themselves.",
                "Tabs you leave open are put away after a while, so the sidebar stays short. \
                 Pinned tabs stay, and nothing is lost.",
                "Continue",
            ),
            Step::Private => (
                "Private from the start.",
                "Turn off what you don't want, here or for one site from its address.",
                "Continue",
            ),
            // Its button only while signed out; the view's own buttons are the step's after.
            Step::Account => (
                "Your Spaces on every computer.",
                "Spaces, bookmarks and passwords, encrypted end to end.",
                "Sign In",
            ),
            Step::Assistant => (
                "An assistant, or just the browser?",
                concat!(
                    env!("APP_NAME"),
                    " has no AI of its own. An assistant is your own Claude or ChatGPT, working here."
                ),
                "Continue",
            ),
            Step::OptIns => (
                "Up to you.",
                "Each stays off until you turn it on, and Settings has it too.",
                "Continue",
            ),
            // Z20's weaving word, not the name: it stays as the name changes.
            Step::Keys => (
                "You're woven in.",
                "Three keys carry most of it.",
                concat!("Open ", env!("PRODUCT_NAME")),
            ),
        };
        (title, line.into(), button)
    }
}

/// A step comes in as the one before goes, cross-fading, and the welcome fades in with the
/// window, but for Reduce Motion: the icon picker's tabs' 200 ms ease-out.
const STEP_MOTION: f32 = 0.2;
/// The flow fades from its window at the end, on the scroll edge's curve.
const END_FADE: f32 = 0.2;

/// The account isn't signed in, only waiting perhaps for the sign-in window, as the core said last,
/// or before it has said.
fn signed_out(state: &account::State) -> bool {
    matches!(state.status, Status::SignedOut | Status::SigningIn)
}

/// The row motion's ease-out, as a dot settling into place.
fn ease_out(x: f32) -> f32 {
    cubic_bezier(0., 0., 0.58, 1., x)
}

/// The import view the first launch shows, and what it tells the flow.
pub(super) struct Importing {
    pub(super) view: Entity<ImportView>,
    /// What the last import that finished said, for the flow's quiet line.
    pub(super) said: Option<gpui::SharedString>,
    _events: Subscription,
}

/// The account step's view, and what it tells the flow.
struct Signing {
    view: Entity<Account>,
    _events: Subscription,
    _synced: Subscription,
}

/// The flow while it shows.
pub(super) struct Onboarding {
    /// The steps this launch shows, in order: STEPS but for those already done.
    steps: Vec<Step>,
    pub(super) step: Step,
    /// The swatch picked at the colour step, if one is.
    pub(super) picked: Option<usize>,
    /// The level the transparency step shows, which the app is at, as a pick applies at once; and
    /// the one it was at as the flow began, which Skip puts back.
    pub(super) level: Transparency,
    before: Transparency,
    /// Where the picker's pictures were drawn, its own log as the picker asks outside Settings.
    level_drawn: settings_rows::Drawn,
    /// The search engines offered, each its keyword and name, and the profile's default's keyword,
    /// read until the core has them: a new profile's load after the flow starts.
    engines: (Vec<(SharedString, SharedString)>, SharedString),
    /// The engine picked at its step, the default's until the user moves it. Nothing is set until
    /// Continue.
    engine: Option<usize>,
    /// The system it runs on, for the default browser's step.
    os: Os,
    /// The opt-in switch last turned, and when, for its knob's slide.
    switched: Option<(Switch, Instant)>,
    /// That switch's knob has slid all the way, which check.sh hears once.
    slid: bool,
    /// The intro holds the keyboard itself, as does a step left with no control, so an assistive
    /// app, which follows the keyboard, reads each by its title and line as it comes (#3151).
    focus: gpui::FocusHandle,
    /// ⇥'s stops at the step's button and at the foot's Skip, which ↩ and Space press.
    continue_stop: gpui::FocusHandle,
    skip_stop: gpui::FocusHandle,
    /// The foot's Back, before its Skip.
    back_stop: gpui::FocusHandle,
    /// The stop of the step's choice, where its keyboard rests as it comes: its swatches or its
    /// engines (onboarding_rest).
    first_stop: gpui::FocusHandle,
    /// The step that last took the keyboard as it came.
    focused_step: Option<Step>,
    /// Where the opt-ins' rows and switches are drawn (opt_in_frame).
    frames: Frames,
    /// Where the step shown draws its Skip and Back, by name (Shell::place).
    pub(super) places: Places,
    /// Its last frame asked for another: something moves, or its loop plays.
    pub(super) drawing: bool,
    /// Where the step's button is drawn, once the step has settled (button_frame).
    pub(super) button: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Where the foot's Skip is drawn, once the step has settled (skip_frame).
    pub(super) skip: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// GPUI's clock, which the flow's motion and the intro's hold are timed by: the real one in the
    /// app, and in a test the one its timers run on, so the two never part (#3248).
    clock: gpui::BackgroundExecutor,
    /// When the step came, which its motion runs from.
    pub(super) since: Instant,
    /// The step before, fading as this one comes, and how far into it it was when it left.
    leaving: Option<(Step, f32)>,
    /// When the last step was left, for the flow's fade.
    pub(super) ending: Option<Instant>,
    /// Light or dark where check.sh says, else the window's.
    light: Option<bool>,
    /// The step has come all the way and said so.
    settled: bool,
    /// Keys in first run's window go on from a step and back, while the flow shows.
    keys: Option<gpui::Subscription>,
    /// First run's window has been on screen, so a step can take its keyboard.
    keyboard: bool,
    /// The account step's view, while the step is offered.
    account: Option<Signing>,
    /// The assistants' programs have been looked for since the assistant step last showed.
    looked: bool,
    /// The assistant step's rows follow the Agents, as a sign-in under way changes them.
    _assistants: Option<Subscription>,
    /// The assistants' rows the preview shows, and the one marked, as it has no Agents to read.
    shown_assistants: Option<(
        Vec<(crate::agents::runner::Cli, crate::agents::sign_in::Shown)>,
        Option<crate::agents::runner::Cli>,
    )>,
    /// The daily usage count's switch as turned at the opt-ins step, None while untouched: it's
    /// set as the step is left (onboarding_next), so turned on and off again it sends nothing.
    count: Option<bool>,
    /// The tidy step's times, while the core has the profile's.
    pub(super) tidy: Option<Tidy>,
}

/// The tidy step's bar: Settings' times, each with the drawing's word for it, the profile's own
/// as the flow began, and the one the bar is at, set only as Continue goes on, as a shorter time
/// puts tabs away at once.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tidy {
    pub(super) stops: Vec<(f64, &'static str)>,
    was: f64,
    pub(super) at: usize,
}

impl Tidy {
    /// Settings' `offered` times that have a word, at the one nearest `was`; None with none.
    fn of(was: f64, offered: &[f64]) -> Option<Tidy> {
        let stops: Vec<(f64, &'static str)> = (offered.iter())
            .filter_map(|&seconds| Some((seconds, stop_words(seconds)?)))
            .collect();
        // Never is past every time.
        let far = |seconds: f64| if seconds < 0. { f64::MAX } else { seconds };
        let at = (0..stops.len()).min_by(|&a, &b| {
            let off = |at: usize| (far(stops[at].0) - far(was)).abs();
            off(a).total_cmp(&off(b))
        })?;
        Some(Tidy { stops, was, at })
    }

    /// The time the bar is at, in seconds.
    fn seconds(&self) -> f64 {
        self.stops[self.at].0
    }
}

/// The drawing's word for a time of Settings', which says them in hours and days.
fn stop_words(seconds: f64) -> Option<&'static str> {
    let day = 24. * 3600.;
    Some(match seconds {
        _ if seconds < 0. => "Never",
        _ if seconds == day / 2. => "12 hours",
        _ if seconds == day => "1 day",
        _ if seconds == 7. * day => "1 week",
        _ if seconds == 30. * day => "1 month",
        _ => return None,
    })
}

impl Onboarding {
    /// Seconds since `at` on the flow's clock.
    fn secs_since(&self, at: Instant) -> f32 {
        self.clock.now().saturating_duration_since(at).as_secs_f32()
    }
}

impl Shell {
    /// Starts the flow at the welcome in first run's window, opening it unless it's open.
    /// `appearance` is check.sh's "light" or "dark", else the window's own.
    pub fn start_onboarding(&mut self, appearance: Option<&str>, cx: &mut Context<Self>) {
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
                self.importing = Some(Importing {
                    view,
                    said: None,
                    _events: events,
                });
            }
        }
        let import = self.importing.as_ref().is_some_and(|importing| {
            let view = importing.view.read(cx);
            !view.is_empty() || view.running().is_some()
        });
        let default_browser = self.browser.is_default_browser();
        // A new profile's engines load after the flow starts, so the step is planned before the
        // core has them, and onboarding_next reads them until it does.
        let engines = self.browser.search_engines(self.space);
        let engine = (engines.0.iter()).position(|(keyword, _)| *keyword == engines.1);
        let opt_ins = !self.opt_ins(cx).is_empty();
        let private = !self.switches(Step::Private, cx).is_empty();
        let tidy = (self.browser.archive_after(self.space))
            .and_then(|(was, offered)| Tidy::of(was, &offered));
        let account = self.signing(cx);
        let steps: Vec<Step> = STEPS
            .into_iter()
            .filter(|&step| match step {
                Step::Import => import,
                // An import begun brings it in (import_event).
                Step::Favorites => false,
                Step::DefaultBrowser => !default_browser,
                Step::SearchEngine => true,
                Step::Account => account.is_some(),
                // Left out, as the agents' switch is, in a build without the agents' server.
                Step::Assistant => self.browser.threads(cx).is_some(),
                Step::OptIns => opt_ins,
                Step::Tidy => tidy.is_some(),
                Step::Private => private,
                // Over no glass there's nothing to show through.
                Step::Transparency => self.blurred,
                _ => true,
            })
            .collect();
        let names: Vec<&str> = steps.iter().map(|step| step.name()).collect();
        // check.sh waits for these lines.
        log!("browser: onboarding started");
        log!("browser: onboarding steps {}", names.join(" "));
        log!("browser: onboarding step intro");
        let level = transparency::level(cx);
        self.onboarding = Some(Onboarding {
            steps,
            step: Step::Intro,
            picked: None,
            level,
            before: level,
            level_drawn: Default::default(),
            engines,
            engine,
            os: self.browser.os(),
            switched: None,
            slid: false,
            focus: cx.focus_handle(),
            continue_stop: cx.focus_handle().tab_stop(true),
            skip_stop: cx.focus_handle().tab_stop(true),
            back_stop: cx.focus_handle().tab_stop(true),
            first_stop: cx.focus_handle().tab_stop(true),
            focused_step: None,
            frames: Frames::default(),
            places: Places::default(),
            drawing: false,
            button: Rc::default(),
            skip: Rc::default(),
            clock: cx.background_executor().clone(),
            since: cx.background_executor().now(),
            leaving: None,
            ending: None,
            light: appearance.and_then(|name| match name {
                "light" => Some(true),
                "dark" => Some(false),
                _ => None,
            }),
            settled: false,
            keys: None,
            keyboard: false,
            account,
            looked: false,
            _assistants: (crate::agents::global(cx))
                .map(|agents| cx.observe(&agents, |_, _, cx| cx.notify())),
            shown_assistants: None,
            count: None,
            tidy,
        });
        // Opened outside this update, as the window draws the shell's flow as it opens.
        let shell = cx.entity();
        cx.defer(move |cx| {
            let open = (shell.read(cx).first_run).filter(|handle| {
                let handle: gpui::AnyWindowHandle = (*handle).into();
                cx.windows().contains(&handle)
            });
            let Some(handle) = open.or_else(|| FirstRun::open(shell.clone(), cx)) else {
                log!("browser: first run's window didn't open");
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

    /// First run's window has the keyboard, with the flow in it.
    pub(super) fn first_run_key(&self, cx: &App) -> bool {
        self.onboarding.is_some()
            && (self.first_run).is_some_and(|handle| cx.active_window() == Some(handle.into()))
    }

    /// First run is showing, so the browser window it opened at launch waits for it to end.
    pub fn first_run_open(&self) -> bool {
        self.onboarding.is_some()
    }

    /// Settings › Sync's view for the account step, following the account as the core says it is;
    /// none in a build that names no server, or with the browser signed in already.
    fn signing(&mut self, cx: &mut Context<Self>) -> Option<Signing> {
        let synced = account::synced(cx);
        let host = self
            .browser
            .account_host()
            .filter(|_| signed_out(&synced))?;
        account::bind_keys(cx);
        let view = cx.new(|cx| {
            let mut view = Account::new(host, cx).as_step();
            view.set_state(synced, cx);
            view
        });
        let events = cx.subscribe(&view, |shell, _, event, cx| match event {
            AccountEvent::Finished if shell.onboarding_on(Step::Account) => {
                shell.onboarding_next("account", cx)
            }
            AccountEvent::Finished => {}
        });
        let synced = cx.observe_global::<account::Synced>(|shell, cx| {
            if let Some(view) = shell.account_view() {
                let state = account::synced(cx);
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

    fn account_view(&self) -> Option<Entity<Account>> {
        let signing = self.onboarding.as_ref()?.account.as_ref()?;
        Some(signing.view.clone())
    }

    /// The flow shows `step`, and isn't ending.
    fn onboarding_on(&self, step: Step) -> bool {
        self.onboarding
            .as_ref()
            .is_some_and(|o| o.step == step && o.ending.is_none())
    }

    /// The account's recovery words show at its step, which show once, so only Done goes on.
    fn keeping_words(&self, cx: &gpui::App) -> bool {
        self.onboarding_on(Step::Account)
            && self
                .account_view()
                .is_some_and(|view| view.read(cx).keeping_words())
    }

    /// Help › Welcome: the flow again from its welcome, in its own window over this one.
    pub(super) fn welcome_again(&mut self, cx: &mut Context<Self>) {
        self.start_onboarding(None, cx);
    }

    /// Goes on from the step shown, to the next or out of the flow after the last, `how` saying
    /// what did it for check.sh.
    pub(super) fn onboarding_next(&mut self, how: &str, cx: &mut Context<Self>) {
        // The favorites come only while an import has some to offer.
        let favorites = self.importing.as_ref().is_some_and(|importing| {
            importing
                .view
                .read(cx)
                .suggested()
                .is_some_and(|tabs| !tabs.is_empty())
        });
        // The core reads the account after the flow may have started, and it can be signed in by
        // then.
        let signed_in = !signed_out(&account::synced(cx));
        let engines = (self.onboarding.as_ref())
            .filter(|o| o.engines.0.is_empty() && o.steps.contains(&Step::SearchEngine))
            .map(|_| self.browser.search_engines(self.space));
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        let step = onboarding.step;
        log!("browser: onboarding left {} via {how}", step.name());
        // Skipped, the app goes back to the level it had, so nothing new is kept.
        let unpicked = step == Step::Transparency
            && matches!(how, "escape" | "skip")
            && onboarding.level != onboarding.before;
        if unpicked {
            onboarding.level = onboarding.before;
            log!(
                "browser: onboarding transparency {}",
                onboarding.before.key()
            );
            transparency::set(onboarding.before, cx);
        }
        // Skipped, the colour the window was showing goes, as nothing was picked.
        let unpainted = step == Step::Color && matches!(how, "escape" | "skip");
        // The count's switch is set as its step is left, whichever way: nothing is sent while the
        // step shows, and turned on and off again it's as it was (#2978).
        let count = (step == Step::OptIns)
            .then(|| onboarding.count.take())
            .flatten();
        if !favorites {
            onboarding
                .steps
                .retain(|&each| each != Step::Favorites || each == step);
        }
        if signed_in {
            onboarding
                .steps
                .retain(|&each| each != Step::Account || each == step);
        }
        if let Some(engines) = engines {
            onboarding.engine = (engines.0.iter()).position(|(keyword, _)| *keyword == engines.1);
            onboarding.engines = engines;
        }
        let at = onboarding.steps.iter().position(|&each| each == step);
        // With none to offer when its turn comes, the search engine's step is left out.
        if at.and_then(|at| onboarding.steps.get(at + 1)) == Some(&Step::SearchEngine)
            && onboarding.engines.0.is_empty()
        {
            onboarding.steps.retain(|&each| each != Step::SearchEngine);
        }
        let shown = match at.and_then(|at| onboarding.steps.get(at + 1)).copied() {
            Some(next) => {
                let elapsed = onboarding.secs_since(onboarding.since);
                onboarding.leaving = Some((step, elapsed));
                onboarding.step = next;
                onboarding.since = onboarding.clock.now();
                onboarding.settled = false;
                log!("browser: onboarding step {}", next.name());
                Some(next)
            }
            None => {
                onboarding.ending = Some(onboarding.clock.now());
                None
            }
        };
        if let Some(shown) = shown {
            self.onboarding_shown(shown, cx);
        }
        if unpainted {
            self.onboarding_unpaint();
        }
        if let Some(on) = count.filter(|&on| self.browser.daily_count(cx) != Some(on)) {
            // check.sh reads this line; the first count, with first set, goes from here.
            log!(
                "browser: onboarding daily-count set {}",
                if on { "on" } else { "off" }
            );
            self.browser.set_daily_count(on, cx);
        }
        cx.notify();
    }

    /// Back to the step before the one shown, which comes from the left, `how` saying what did it
    /// for check.sh. A colour shown but not kept goes.
    fn onboarding_back(&mut self, how: &str, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        let step = onboarding.step;
        let at = onboarding.steps.iter().position(|&each| each == step);
        let Some(before) = (at.and_then(|at| at.checked_sub(1)))
            .and_then(|at| onboarding.steps.get(at))
            .copied()
        else {
            return;
        };
        log!("browser: onboarding left {} via {how}", step.name());
        let elapsed = onboarding.secs_since(onboarding.since);
        onboarding.leaving = Some((step, elapsed));
        onboarding.step = before;
        onboarding.since = onboarding.clock.now();
        onboarding.settled = false;
        log!("browser: onboarding step {}", before.name());
        if step == Step::Color {
            self.onboarding_unpaint();
        }
        self.onboarding_shown(before, cx);
        cx.notify();
    }

    /// As the assistants' step shows with agents on, ilo looks for them once, off the UI thread,
    /// as Settings › Agents does as it opens, so each row says what it found, not that it's
    /// checking. With agents off nothing of an assistant runs until the user says yes there.
    fn onboarding_shown(&mut self, step: Step, cx: &mut Context<Self>) {
        if step != Step::Assistant {
            return;
        }
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.looked = false;
        }
        self.onboarding_look(cx);
    }

    /// Looks for the assistants' programs, once a showing of their step, only with agents on.
    fn onboarding_look(&mut self, cx: &mut Context<Self>) {
        let agents_on = self.browser.threads(cx) == Some(true);
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        if onboarding.step != Step::Assistant || onboarding.looked || !agents_on {
            return;
        }
        onboarding.looked = true;
        if let Some(agents) = crate::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.look_for_task_agents(cx));
        }
    }

    /// Skip Setup, at the welcome or as first run's window closes: the flow ends there, as it does
    /// after its last step, with nothing set.
    pub(super) fn onboarding_skip_setup(&mut self, how: &str, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        // check.sh reads these lines.
        log!(
            "browser: onboarding left {} via {how}",
            onboarding.step.name()
        );
        log!("browser: onboarding skipped setup");
        onboarding.ending = Some(onboarding.clock.now());
        cx.notify();
    }

    /// The colour step's pick, shown on the window but not kept, goes.
    fn onboarding_unpaint(&mut self) {
        if self
            .theme_preview
            .is_some_and(|(space, _)| space == self.space)
        {
            self.theme_preview = None;
        }
    }

    /// Picks the `index`th swatch at the color step, as a click does, for the preview's captures.
    pub fn onboarding_pick_swatch(&mut self, index: usize, cx: &mut Context<Self>) {
        self.onboarding_pick(index, cx);
    }

    /// Shows the assistants' step with `rows`, `marked` the one tasks run on, with agents on, for
    /// the preview's captures: the preview has no Agents for them to come from.
    pub fn onboarding_show_assistants(
        &mut self,
        rows: Vec<(crate::agents::runner::Cli, crate::agents::sign_in::Shown)>,
        marked: Option<crate::agents::runner::Cli>,
        cx: &mut Context<Self>,
    ) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.shown_assistants = Some((rows, marked));
            cx.notify();
        }
        // The rows are the with-an-assistant path's.
        self.onboarding_agents(true, cx);
    }

    /// Turns the daily count's switch on at the opt-ins step, for the preview's captures.
    pub fn onboarding_count_on(&mut self, cx: &mut Context<Self>) {
        self.onboarding_switch(Switch::DailyCount, cx);
    }

    /// Starts the flow in `appearance` at `step`, check.sh's name for it or its number from 1 in
    /// the whole flow, for the preview. The transparency step shows on a host without glass too,
    /// as on one with it, so it can be looked at anywhere.
    pub fn onboarding_at(&mut self, appearance: Option<&str>, step: &str, cx: &mut Context<Self>) {
        let numbered = (step.parse::<usize>().ok())
            .and_then(|number| STEPS.get(number.checked_sub(1)?))
            .map(|step| step.name());
        let name = numbered.unwrap_or(step);
        if name == Step::Transparency.name() {
            self.blurred = true;
        }
        self.start_onboarding(appearance, cx);
        self.onboarding_to(name, cx);
    }

    /// Goes on through the flow to the step check.sh calls `name`, for the preview's captures.
    pub fn onboarding_to(&mut self, name: &str, cx: &mut Context<Self>) {
        while let Some(onboarding) = self.onboarding.as_ref()
            && onboarding.ending.is_none()
            && onboarding.step.name() != name
        {
            self.onboarding_next("preview", cx);
        }
    }

    /// The step's button: what it offers, then on, as Skip goes.
    fn onboarding_continue(&mut self, how: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_ref().filter(|o| o.ending.is_none()) else {
            return;
        };
        match (onboarding.step, onboarding.picked) {
            // The import's button does what its view's state is for; the view says when it has
            // read what it brings, which goes on.
            (Step::Import, _) => {
                log!("browser: onboarding import button via {how}");
                if let Some(importing) = self.importing.as_ref() {
                    importing.view.update(cx, |view, cx| view.press(cx));
                }
                return cx.notify();
            }
            // Signed in, the view says so and the flow goes on (signing); until then it stays.
            (Step::Account, _) => {
                if let Some(view) = self.account_view()
                    && view.read(cx).state().status == Status::SignedOut
                {
                    log!("browser: onboarding sign in via {how}");
                    view.update(cx, |view, cx| view.sign_in(window, cx));
                }
                return;
            }
            (Step::DefaultBrowser, _) if default_browser::offers(onboarding.os, false) => {
                self.browser.make_default_browser(cx)
            }
            // Only a pick that isn't the default already: the default stays the core's fallback,
            // which a recommended policy outranks, rather than becoming the user's choice.
            (Step::SearchEngine, _) => {
                let (engines, default) = &onboarding.engines;
                let picked = (onboarding.engine)
                    .and_then(|at| engines.get(at))
                    .filter(|(keyword, _)| keyword != default);
                if let Some((keyword, name)) = picked.cloned() {
                    self.browser
                        .set_search_engine(self.space, &keyword, &name, cx);
                }
            }
            (Step::Favorites, _) => {
                let (ticked, space) = (self.favorites_ticked(cx), self.space);
                if let Some(importing) = self.importing.as_ref() {
                    importing
                        .view
                        .update(cx, |view, _| view.add_offered(space, &ticked));
                }
            }
            (Step::Color, Some(picked)) => {
                let color = SWATCHES[picked];
                // check.sh reads this line.
                log!("browser: onboarding picked color {color:06x}");
                self.theme_changed(self.space, Theme::of_color(color), true, cx);
            }
            // Set only if it moved, as Settings sets it, which puts away at once the tabs that have
            // waited that long.
            (Step::Tidy, _) => {
                if let Some(tidy) = onboarding.tidy.as_ref().filter(|t| t.seconds() != t.was) {
                    log!("browser: onboarding tidy set {}", tidy.stops[tidy.at].1);
                    self.browser
                        .set_archive_after(self.space, tidy.seconds(), cx);
                }
            }
            // Picked, it was applied and kept at once.
            (Step::Transparency, _) => {
                log!(
                    "browser: onboarding transparency kept {}",
                    onboarding.level.key()
                );
            }
            _ => {}
        }
        self.onboarding_next(how, cx);
    }

    /// What the import view says: an import's Spaces are in, so the step holds on them a moment and
    /// goes on while the rest runs; or it ended, which the flow's quiet line then says; or the user
    /// is done with it, which goes on too. Until its Spaces are in, the step stays, as the view may
    /// ask the user for Full Disk Access, wait for a profile that hasn't opened, or stop, and a
    /// passwords file's end offers to delete the file.
    fn import_event(&mut self, event: &ImportEvent, cx: &mut Context<Self>) {
        if let Some(importing) = self.importing.as_mut() {
            importing.said = match event {
                ImportEvent::Finished(said) => Some(said.clone()),
                _ => importing.said.take(),
            };
        }
        let step = self.onboarding.as_ref().map(|onboarding| onboarding.step);
        let on_it = step == Some(Step::Import);
        if let Some(onboarding) = self.onboarding.as_mut().filter(|_| on_it)
            && *event == ImportEvent::Started
            && !onboarding.steps.contains(&Step::Favorites)
        {
            let at = onboarding
                .steps
                .iter()
                .position(|&each| each == Step::Import);
            onboarding
                .steps
                .insert(at.map_or(0, |at| at + 1), Step::Favorites);
        }
        match event {
            // The step holds on the Spaces once they're in, as they tick, then goes on by itself.
            ImportEvent::Placed if on_it => {
                let hold = std::time::Duration::from_secs_f32(IMPORT_HOLD);
                cx.spawn(async move |shell, cx| {
                    cx.background_executor().timer(hold).await;
                    shell
                        .update(cx, |shell, cx| {
                            let step = shell.onboarding.as_ref().map(|onboarding| onboarding.step);
                            if step == Some(Step::Import) {
                                shell.onboarding_next("import", cx);
                            }
                        })
                        .ok();
                })
                .detach();
            }
            ImportEvent::Done if on_it => self.onboarding_next("import", cx),
            ImportEvent::Skipped if on_it => self.onboarding_next("skip", cx),
            _ => cx.notify(),
        }
    }

    /// The favorites offered that are ticked, as ↩ adds them.
    fn favorites_ticked(&self, cx: &gpui::App) -> Vec<crate::import::found::Tab> {
        (self.importing.as_ref())
            .map_or_else(Vec::new, |importing| importing.view.read(cx).ticked())
    }

    /// The flow's quiet line about the import: what it's doing, or what it brought in.
    pub(super) fn import_line(&self, cx: &gpui::App) -> Option<gpui::SharedString> {
        let importing = self.importing.as_ref()?;
        importing
            .view
            .read(cx)
            .running()
            .or_else(|| importing.said.clone())
    }

    /// The opt-ins offered, each with whether it's on.
    pub(super) fn opt_ins(&self, cx: &gpui::App) -> Vec<(Switch, bool)> {
        self.switches(Step::OptIns, cx)
    }

    /// The switches `step` offers, each with whether it's on.
    pub(super) fn switches(&self, step: Step, cx: &gpui::App) -> Vec<(Switch, bool)> {
        let on = |opt_in| match opt_in {
            Switch::DailyCount => {
                let kept = self.browser.daily_count(cx)?;
                Some(
                    self.onboarding
                        .as_ref()
                        .and_then(|o| o.count)
                        .unwrap_or(kept),
                )
            }
            Switch::CrashReports => self.browser.crash_reports(cx),
            Switch::OtherApps => self.browser.other_apps(cx),
            Switch::Setting(setting) => self.browser.setting(self.space, setting, cx),
        };
        (Switch::of(step).iter())
            .filter_map(|&opt_in| Some((opt_in, on(opt_in)?)))
            .collect()
    }

    /// Turns a switch of the step shown, which is Settings' own, so it's set at once.
    pub(super) fn onboarding_switch(&mut self, opt_in: Switch, cx: &mut Context<Self>) {
        let Some(step) = self.onboarding.as_ref().map(|onboarding| onboarding.step) else {
            return;
        };
        let switches = self.switches(step, cx);
        let Some(&(_, on)) = switches.iter().find(|(each, _)| *each == opt_in) else {
            return;
        };
        // check.sh reads this line.
        log!(
            "browser: onboarding {} {}",
            opt_in.name(),
            if on { "off" } else { "on" }
        );
        let (browser, space) = (self.browser.clone(), self.space);
        cx.defer(move |cx| match opt_in {
            Switch::DailyCount => {}
            Switch::CrashReports => browser.set_crash_reports(!on, cx),
            Switch::OtherApps => browser.set_other_apps(!on, cx),
            Switch::Setting(setting) => browser.set_setting(space, setting, !on, cx),
        });
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.switched = Some((opt_in, onboarding.clock.now()));
            onboarding.slid = false;
            if opt_in == Switch::DailyCount {
                onboarding.count = Some(!on);
            }
        }
        cx.notify();
    }

    /// The assistant step's pick: with an assistant turns agents on, just the browser turns them
    /// off, Settings › Agents' one switch, set at once as the step's other switches are.
    pub(super) fn onboarding_agents(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.browser.threads(cx) != Some(!on) {
            return;
        }
        log!(
            "browser: onboarding agents {}",
            if on { "on" } else { "off" }
        );
        let (browser, shell) = (self.browser.clone(), cx.entity());
        // Outside this update, as the host reads the shell as it turns agents off.
        cx.defer(move |cx| {
            browser.set_threads(on, cx);
            shell.update(cx, |shell, cx| {
                shell.onboarding_look(cx);
                cx.notify();
            });
        });
    }

    /// Picks swatch `index` at the colour step.
    /// Picks the search engine at `index`, which sets nothing until Continue.
    fn onboarding_pick_engine(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            let last = onboarding.engines.0.len().saturating_sub(1);
            onboarding.engine = Some(index.min(last));
            cx.notify();
        }
    }

    /// Puts the tidy step's bar at stop `at`, stopping at either end. Nothing is set until Continue.
    pub(super) fn onboarding_tidy(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(tidy) = self.onboarding.as_mut().and_then(|o| o.tidy.as_mut()) else {
            return;
        };
        let at = at.min(tidy.stops.len() - 1);
        if tidy.at != at {
            tidy.at = at;
            // check.sh reads this line.
            log!("browser: onboarding tidy at {}", tidy.stops[at].1);
            cx.notify();
        }
    }

    /// Moves the tidy step's bar `by` stops along.
    fn onboarding_tidy_by(&mut self, by: isize, cx: &mut Context<Self>) {
        let at = (self.onboarding.as_ref()).and_then(|o| Some(o.tidy.as_ref()?.at));
        if let Some(at) = at {
            self.onboarding_tidy(at.saturating_add_signed(by), cx);
        }
    }

    /// Moves the transparency step's pick `by` along, stopping at either end.
    fn onboarding_level(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(at) = (self.onboarding.as_ref())
            .and_then(|o| Transparency::ALL.iter().position(|&level| level == o.level))
        else {
            return;
        };
        let last = Transparency::ALL.len() as isize - 1;
        let to = (at as isize + by).clamp(0, last) as usize;
        self.onboarding_transparency(Transparency::ALL[to], cx);
    }

    /// Picks `level` at the transparency step, by a key or the picker: Settings' own way of
    /// setting it, so the window, its menus and the command bar take it at once.
    fn onboarding_transparency(&mut self, level: Transparency, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.level != level) else {
            return;
        };
        onboarding.level = level;
        log!("browser: onboarding transparency {}", level.key());
        transparency::set(level, cx);
        cx.notify();
    }

    fn onboarding_pick(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        let picked = index.min(SWATCHES.len() - 1);
        onboarding.picked = Some(picked);
        // The window shows it at once, as the theme picker does; Continue keeps it.
        if onboarding.step == Step::Color {
            let theme = Theme::of_color(SWATCHES[picked]);
            self.theme_changed(self.space, theme, false, cx);
        }
        cx.notify();
    }

    /// A key in first run's window: ↩ is the step's button and Esc goes back a step, but at the
    /// welcome, the first, does nothing; ⌘W, as the window's close, is Skip Setup. At the colour
    /// step the arrows go along the swatches.
    fn onboarding_key(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((step, picked, engine)) =
            (self.onboarding.as_ref()).map(|o| (o.step, o.picked, o.engine))
        else {
            return;
        };
        let key = &event.keystroke;
        if ["shift", "control", "alt", "platform", "function"].contains(&key.key.as_str()) {
            return;
        }
        let modifiers = key.modifiers;
        if key.key == "w" && modifiers.secondary(cx) && !modifiers.shift && !modifiers.alt {
            return self.onboarding_skip_setup("close", cx);
        }
        // The account's fields keep their own ↩ and Esc, and its words go only with Done.
        let field = event
            .context_stack
            .iter()
            .any(|context| context.contains("AccountField"));
        if step == Step::Account && (field || self.keeping_words(cx)) {
            return;
        }
        match (step, key.key.as_str()) {
            (Step::Intro, "escape") => {}
            (Step::Import, "escape") => self.import_back("escape", cx),
            (_, "escape") => self.onboarding_back("escape", cx),
            // From none picked, → starts at the first and ← at the last.
            (Step::Color, "right") => self.onboarding_pick(picked.map_or(0, |at| at + 1), cx),
            (Step::Color, "left") => {
                self.onboarding_pick(picked.map_or(SWATCHES.len(), |at| at.max(1)) - 1, cx)
            }
            (Step::Transparency, "left" | "up") => self.onboarding_level(-1, cx),
            (Step::Transparency, "right" | "down") => self.onboarding_level(1, cx),
            // Its bar, as the colour's and the level's arrows move theirs.
            (Step::Tidy, "left" | "down") => self.onboarding_tidy_by(-1, cx),
            (Step::Tidy, "right" | "up") => self.onboarding_tidy_by(1, cx),
            // ↑ and ↓ move the engine's pick, as they do the import's list's.
            (Step::SearchEngine, "down") => {
                self.onboarding_pick_engine(engine.map_or(0, |at| at + 1), cx)
            }
            (Step::SearchEngine, "up") => {
                self.onboarding_pick_engine(engine.map_or(0, |at| at.saturating_sub(1)), cx)
            }
            // ↑ and ↓ move the assistant step's pick, between its two choices.
            (Step::Assistant, "up") => self.onboarding_agents(true, cx),
            (Step::Assistant, "down") => self.onboarding_agents(false, cx),
            // ↩ is the import's button for the state its view shows, and ↑ ↓ move its list's pick.
            (Step::Import, "enter") if !key.modifiers.modified() => {
                self.onboarding_continue("return", window, cx)
            }
            (Step::Import, "up" | "down") => {
                let by = if key.key == "up" { -1 } else { 1 };
                if let Some(importing) = self.importing.as_ref() {
                    importing.view.update(cx, |view, cx| view.move_pick(by, cx));
                }
            }
            (Step::Import, _) => {}
            // ⇥ and ⇧⇥ go along the step's stops: the swatches, the opt-ins' switches, its button
            // and Skip, the window's only stops. Space turns the switch the keyboard is on, as
            // Settings' does, and ↩ stays Continue there (#2985).
            (_, "tab") => {
                if key.modifiers.shift {
                    window.focus_prev(cx)
                } else {
                    window.focus_next(cx)
                }
            }
            // On Skip, ↩ is Skip's press, its click. On the step's button it's still the step's ↩,
            // which the button leaves to this.
            (_, "enter") if !key.modifiers.modified() && !self.onboarding_skip_keyed(window) => {
                self.onboarding_continue("return", window, cx)
            }
            _ => {}
        }
    }

    /// The keyboard is on the step's Skip or Back, which take ↩ and Space as their press.
    fn onboarding_skip_keyed(&self, window: &Window) -> bool {
        (self.onboarding.as_ref()).is_some_and(|onboarding| {
            onboarding.skip_stop.is_focused(window) || onboarding.back_stop.is_focused(window)
        })
    }

    /// The step shown has its button: all do, but the account's goes once it's signing or signed
    /// in, and the import's in a state with none (import_guide).
    fn onboarding_has_button(&self, step: Step, cx: &gpui::App) -> bool {
        match step {
            Step::Import => (self.import_guide(cx))
                .and_then(|guide| guide.button)
                .is_some_and(|words| !words.is_empty()),
            Step::Account => (self.account_view())
                .is_some_and(|view| view.read(cx).state().status == Status::SignedOut),
            _ => true,
        }
    }

    /// Where the keyboard rests as the step shown comes: on a control, never on the step's
    /// dialog, as the dialog pattern has it. Orca keeps a dialog it's on as its place when a key
    /// leaves it, and then drops the next step's arrival as already its place, saying nothing of it
    /// (#3498). The colour's and the engine's choice are the step, so it rests on them; any other
    /// rests on its button, the opt-ins' too, as a switch there is a yes to sending something that
    /// a stray Space mustn't give. Only a step left with no control rests on its dialog.
    fn onboarding_rest(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        let onboarding = self.onboarding.as_ref()?;
        Some(match onboarding.step {
            Step::Color | Step::SearchEngine | Step::Transparency | Step::Tidy => {
                onboarding.first_stop.clone()
            }
            step if self.onboarding_has_button(step, cx) => onboarding.continue_stop.clone(),
            _ => onboarding.focus.clone(),
        })
    }

    /// The flow in first run's `window` while it shows, stepping its motion; none once it has
    /// ended, as the window goes.
    pub(super) fn onboarding_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let rest = self.onboarding_rest(cx);
        let glass = self.blurred;
        let onboarding = self.onboarding.as_mut()?;
        let ended = onboarding
            .ending
            .is_some_and(|at| onboarding.secs_since(at) >= END_FADE);
        if ended {
            self.onboarding = None;
            // check.sh waits for this line.
            log!("browser: onboarding done");
            // The window goes and the browser window comes, once this frame is drawn, with the
            // first Space and the command bar up, as ⌘T has it. Not for a quit, whose closing
            // window can draw the frame the fade ends in.
            let first_run = self.first_run;
            let shell = cx.entity();
            let quit = super::quit_gone_ahead();
            if !quit {
                self.in_hint = true;
            }
            cx.defer(move |cx| {
                shell.update(cx, |shell, cx| {
                    if let Some(handle) = first_run {
                        shell.browser.end_first_run(handle.into(), cx);
                    }
                    if !quit {
                        shell.run(Command::NewTab, "onboarding", cx);
                    }
                })
            });
            return None;
        }
        if onboarding.keys.is_none() {
            let id = window.window_handle().window_id();
            onboarding.keys = Some(cx.observe_keystrokes(move |shell, event, window, cx| {
                if window.window_handle().window_id() == id {
                    shell.onboarding_key(event, window, cx);
                }
            }));
        }
        // A step takes the keyboard once the window is on screen.
        if window.is_visible() {
            onboarding.keyboard = true;
        }
        // Each step takes the keyboard as it comes, from the window or from the step before, so an
        // assistive app reads it; never from a field that has it.
        let resting = window.focused(cx).is_none_or(|focus| {
            focus == onboarding.focus
                || focus == onboarding.continue_stop
                || focus == onboarding.skip_stop
                || focus == onboarding.back_stop
                || focus == onboarding.first_stop
        });
        if let Some(rest) = rest
            && onboarding.keyboard
            && onboarding.ending.is_none()
            && resting
            && onboarding.focused_step != Some(onboarding.step)
        {
            onboarding.focused_step = Some(onboarding.step);
            window.focus(&rest, cx);
        }
        let still = crate::motion::still(cx);
        let elapsed = onboarding.secs_since(onboarding.since);
        let step = onboarding.step;
        let came = elapsed >= STEP_MOTION;
        if came && !std::mem::replace(&mut onboarding.settled, true) {
            // check.sh waits for this line before it captures the step.
            log!("browser: onboarding step {} settled", step.name());
        }
        let motion = (elapsed / STEP_MOTION).min(1.);
        if motion >= 1. {
            onboarding.leaving = None;
        }
        // Frames while something moves; the picture is still.
        let ending = onboarding.ending.map(|at| onboarding.secs_since(at));
        let sliding = onboarding
            .switched
            .is_some_and(|(_, at)| onboarding.secs_since(at) < SLIDE);
        // check.sh captures a turned switch on this line, with its knob at the end of its slide.
        if let Some((opt_in, _)) = onboarding.switched
            && !sliding
            && !std::mem::replace(&mut onboarding.slid, true)
        {
            log!("browser: onboarding {} slid", opt_in.name());
        }
        onboarding.drawing = !came || onboarding.leaving.is_some() || ending.is_some() || sliding;
        if onboarding.drawing {
            window.request_animation_frame();
        }
        // Each frame says again where its controls are.
        onboarding.places.borrow_mut().clear();
        let light = onboarding.light.unwrap_or_else(|| {
            matches!(
                window.appearance(),
                gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight
            )
        });
        let leaving = onboarding.leaving;
        let colors = Colors::of(light);
        // The step cross-fades with the one before; nothing slides. The welcome fades in with the
        // window, but for Reduce Motion.
        let eased = ease_out(motion);
        let fades_in = leaving.is_some() || (step == Step::Intro && !still);
        // The step coming takes every click, so one on its Skip doesn't press the leaving step's too.
        let coming = self
            .step_content(step, colors, window, cx)
            .opacity(if fades_in { eased } else { 1. })
            .occlude();
        let gone = leaving.map(|(step, _)| {
            self.step_content(step, colors, window, cx)
                .opacity(1. - eased)
        });
        let opacity = ending.map_or(1., |at| fade_at(1., 0., at, END_FADE));
        Some(
            div()
                .id("onboarding")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(frame::Inks::of(colors).ground(glass))
                .occlude()
                .child(
                    div()
                        .size_full()
                        .opacity(opacity)
                        .children(gone)
                        .child(coming),
                )
                .into_any_element(),
        )
    }

    /// A step in the window's frame.
    fn step_content(
        &self,
        step: Step,
        colors: Colors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        // A screen reader reads the step shown as a dialog called by its title, with its line, and
        // the step leaving under it, still drawn as it fades, as nothing.
        let named = self.a11y
            && (self.onboarding.as_ref()).is_some_and(|onboarding| onboarding.step == step);
        let os = self
            .onboarding
            .as_ref()
            .map_or(Os::Mac, |onboarding| onboarding.os);
        let (title, line, button) = step.message(os);
        let switches = matches!(step, Step::OptIns | Step::Private)
            .then(|| self.opt_in_rows(step, colors, cx));
        let tidy = (step == Step::Tidy).then(|| self.tidy_step(colors, cx));
        // Taller than its room, the list scrolls in it, and the foot stays where every step has it.
        // Just the browser folds the assistants' rows away for its line.
        // Under the two choices a hint, then with an assistant its rows.
        let assistants = (step == Step::Assistant).then(|| {
            let agents = self.browser.threads(cx) == Some(true);
            let hint = match agents {
                true => "Sign in now, or later in Settings.",
                false => "Turn an assistant on later in Settings › Threads.",
            };
            frame::choice(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(PADDING))
                    .child(self.agents_rows(colors, cx))
                    .child(
                        div()
                            .w_full()
                            .px(px(PADDING))
                            .text_color(colors.ink(QUIET))
                            .child(hint),
                    )
                    .children(agents.then(|| self.assistant_rows(step, colors, cx))),
            )
        });
        let engines = (step == Step::SearchEngine).then(|| self.engine_rows(colors, cx));
        let favorites = (step == Step::Favorites)
            .then(|| self.favorite_tiles(colors, window, cx))
            .flatten();
        let account = (step == Step::Account)
            .then(|| self.account_view())
            .flatten();
        // Add 7 Favorites, as many as are ticked, else Continue, as ↩ then adds none.
        let add_favorites = (self.importing.as_ref())
            .map(|importing| importing.view.read(cx).add_favorites_title());
        let button = match step {
            Step::Favorites if self.favorites_ticked(cx).is_empty() => "Continue",
            Step::Favorites => add_favorites.as_deref().unwrap_or(button),
            Step::Account if !self.onboarding_has_button(step, cx) => "",
            _ => button,
        };
        // The import's words, button and corner follow the state its view shows (guide).
        let guide = (step == Step::Import)
            .then(|| self.import_guide(cx))
            .flatten();
        let (title, line, button): (SharedString, SharedString, SharedString) = match guide {
            Some(guide) => (
                guide.title.into(),
                guide.line.into(),
                guide.button.unwrap_or_default().into(),
            ),
            None => (title.into(), line, button.into()),
        };
        let level = (step == Step::Transparency).then(|| self.level_slot(step, colors, cx));
        let swatches = (step == Step::Color).then(|| {
            let picked = self
                .onboarding
                .as_ref()
                .and_then(|onboarding| onboarding.picked);
            swatch_row(
                picked,
                self.first_stop(step),
                (frame::SWATCH, frame::SWATCH_PITCH),
                colors,
                self.a11y,
                cx,
            )
        });
        // A view of Settings' own, its boxes a little into the ink from the ground, in the ink, as
        // Settings' are.
        let look = Look::on(
            colors.ground.blend(colors.ink(IMPORT_SURFACE)),
            colors.ink(1.),
            rgb(SIGNAL),
        );
        let import = (step == Step::Import)
            .then(|| {
                self.importing
                    .as_ref()
                    .map(|importing| importing.view.clone())
            })
            .flatten()
            .map(|view| {
                view.update(cx, |view, cx| view.set_look(look, cx));
                frame::choice(gpui::AnyView::from(view))
            });
        let account = account.map(|view| {
            view.update(cx, |view, cx| view.set_look(look, cx));
            frame::choice(gpui::AnyView::from(view))
        });
        let drawn = (self.onboarding.as_ref())
            .map(|onboarding| (onboarding.button.clone(), onboarding.settled));
        let said = (title.clone(), line.clone());
        // Only the step shown's: the one leaving, drawn under it, has none.
        let stops = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| {
                (
                    onboarding.continue_stop.clone(),
                    onboarding.skip_stop.clone(),
                )
            });
        let inks = frame::Inks::of(colors);
        let button = (!button.is_empty()).then(|| {
            let control = div()
                .id("onboarding-continue")
                .when(self.a11y, |control| named_button(control, button.clone()))
                .when_some(stops.clone(), |control, (stop, _)| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                .child(button)
                .children(drawn.map(|(at, settled)| button_frame(at, settled)));
            let key = super::key_name("enter", super::OFF_THE_MAC);
            frame::main_button(control, Some(key.into()), inks)
        });
        let focus = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| onboarding.focus.clone());
        // The import under way, or what it brought, quiet under the step's own choice once its step
        // is left, at the one step with room under it.
        let import_line = (step == Step::Transparency)
            .then(|| self.import_line(cx))
            .flatten()
            .map(|said| {
                div()
                    .pt(px(frame_tokens::BODY_TOP))
                    .text_size(px(frame_tokens::LINE_SIZE))
                    .line_height(px(frame_tokens::LINE_LINE))
                    .text_color(inks.text(inks.faint))
                    .child(said)
            });
        let left = self.frame_left(step, (title, line), colors, cx);
        let left = left
            .children(swatches)
            .children(level)
            .children(import_line)
            .children(import)
            .children(favorites)
            .children(engines)
            .children(account)
            .children(assistants)
            .children(switches)
            .children(tidy)
            .children((step == Step::Keys).then(|| self.keys_step(colors, cx)))
            .child(self.frame_foot(step, button, colors, cx));
        let art = self.frame_art(step, colors, cx);
        let dialog = |pane: gpui::Stateful<gpui::Div>| {
            pane.when_some(focus, |content, focus| content.track_focus(&focus))
                .when(named, |content| {
                    let (title, line) = said;
                    content
                        .role(Role::Dialog)
                        .aria_label(title)
                        .when(!line.is_empty(), |content| content.aria_description(line))
                })
        };
        frame::frame(
            colors,
            ("onboarding-step", step as usize),
            dialog,
            left,
            art,
        )
    }

    /// The switches `step` offers in one box, as Settings' Privacy has them: a row each of its name over
    /// its sentence, wrapping in the box, and Settings' switch in the ink at the box's right edge,
    /// centred on the row. A click anywhere on a row, the switch included, turns it.
    fn opt_in_rows(&self, step: Step, colors: Colors, cx: &mut Context<Self>) -> gpui::Div {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div();
        };
        let switched = onboarding
            .switched
            .map(|(turned, at)| (turned, onboarding.secs_since(at)));
        // Where they're drawn is kept once the step has come, not while it slides in.
        let settled = onboarding.step == step && onboarding.settled;
        let frames = onboarding.frames.clone();
        // The import step's boxes, the ink a little over the ground.
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        // On is the text's ink with a knob of the ground's, as Settings' are: no green on a
        // control (the owner's, Notes 28).
        let look = Look {
            knob_on: Rgba {
                a: 1.,
                ..colors.ground
            },
            ..Look::on(surface, colors.ink(1.), colors.ink(1.))
        };
        let rows = self.switches(step, cx).into_iter().map(|(opt_in, on)| {
            let slid = match switched {
                Some((turned, since)) if turned == opt_in => (since / SLIDE).min(1.),
                _ => 1.,
            };
            let (name, sentence) = opt_in.words();
            // Its text may take the row's width but the switch's, so a long sentence wraps rather
            // than pushing the switch out of the row, where a click on it would miss (#1286).
            let words = div()
                .flex_1()
                .min_w_0()
                .line_height(px(OPT_IN_LINE))
                .child(div().font_weight(gpui::FontWeight::MEDIUM).child(name))
                .when(!sentence.is_empty(), |words| {
                    words.child(div().text_color(colors.ink(QUIET)).child(sentence))
                });
            let control = div()
                .relative()
                .flex_none()
                .ml(px(PADDING))
                .child(
                    stop_switch(&look, "switch", name, on, slid, look.on, self.a11y)
                        // A press is its row's click; here only Space, as ↩ is the step's Continue.
                        .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                            if matches!(
                                event,
                                ClickEvent::Keyboard(KeyboardClickEvent {
                                    button: KeyboardButton::Space,
                                    ..
                                })
                            ) {
                                shell.onboarding_switch(opt_in, cx);
                            }
                        })),
                )
                .child(opt_in_frame(frames.clone(), opt_in, "switch", settled));
            let row = div()
                .id(opt_in.name())
                .relative()
                .p(px(PADDING))
                .flex()
                .items_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| shell.onboarding_switch(opt_in, cx)))
                .child(words)
                .child(control)
                .child(opt_in_frame(frames.clone(), opt_in, "row", settled));
            div().child(row)
        });
        div()
            .w_full()
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
            .child(group(&look, rows))
    }

    /// The assistants tasks run on, as Settings › Models has them, one component in both places:
    /// each signs in with its maker's own sign-in. Other apps' agents have no row in first run
    /// (the lead's, 25 Nov), only in Settings. Under them, room kept for the switch the sign-in's
    /// head brings, with nothing in it until
    /// then. Without the app's Agents, as in a test's shell, only that room.
    fn assistant_rows(&self, step: Step, colors: Colors, cx: &mut Context<Self>) -> gpui::Div {
        use crate::agents::runner::Cli;
        use crate::settings_rows::{Act, assistant_row};
        let look = Look::on(
            colors.ground.blend(colors.ink(IMPORT_SURFACE)),
            colors.ink(1.),
            rgb(SIGNAL),
        );
        let shell = cx.weak_entity();
        let a11y = self.a11y;
        let rows = crate::agents::global(cx).map(|agents| {
            let model = agents.read(cx);
            let marked = model.default_task_agent();
            let shown = Cli::ALL.map(|cli| (cli, model.assistant_row(cli)));
            let rows = shown.map(|(cli, shown)| {
                let (agents, shell) = (agents.clone(), shell.clone());
                // As Settings' rows do, but for its ⋯ menu, which is Settings' own, so it opens
                // there.
                let act = move |act: Act, _: &mut Window, cx: &mut gpui::App| match act {
                    Act::Get => {
                        log!("browser: assistant install page {}", cli.id());
                        let url = [cli.install_page().to_string()];
                        let open = |shell: &mut Shell, cx: &mut Context<Shell>| {
                            shell.browser.open_at(&url, false, 0, 0, cx);
                        };
                        shell.update(cx, open).ok();
                    }
                    Act::SignIn => agents.update(cx, |agents, cx| agents.sign_in(cli, cx)),
                    Act::Cancel => agents.update(cx, |agents, _| agents.cancel_sign_in(cli)),
                    Act::Mark => {
                        log!("browser: tasks run on {}", cli.id());
                        agents.update(cx, |agents, cx| agents.set_default_task_agent(cli, cx))
                    }
                    Act::Menu => {
                        let open = |shell: &mut Shell, cx: &mut Context<Shell>| {
                            shell.browser.agents_settings(cx);
                        };
                        shell.update(cx, open).ok();
                    }
                };
                assistant_row(
                    &look,
                    cli,
                    &shown,
                    marked == Some(cli),
                    a11y,
                    div(),
                    div(),
                    Rc::new(act),
                )
                .children(self.place(step, cli.id()))
            });
            group(&look, rows)
        });
        let rows = rows.or_else(|| {
            let (shown, marked) = self.onboarding.as_ref()?.shown_assistants.clone()?;
            let rows = shown.iter().map(|(cli, shown)| {
                let act = Rc::new(|_: Act, _: &mut Window, _: &mut gpui::App| {});
                assistant_row(
                    &look,
                    *cli,
                    shown,
                    marked == Some(*cli),
                    a11y,
                    div(),
                    div(),
                    act,
                )
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
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(crate::tokens::settings::ROW_HEIGHT))
                    .children(self.place(step, "switch-place")),
            )
    }

    /// The search engines offered in the opt-ins' box, a row each of a radio's dot and the engine's
    /// name, the one picked filled in the green and its name medium. A click on a row picks it, and
    /// the list is one stop for ⇥, its arrows moving the pick, as the swatches are. The pick is a
    /// dot in the ink at the row's end, and the box is as wide as the step's column.
    fn engine_rows(&self, colors: Colors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div().into_any_element();
        };
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        let look = Look::on(surface, colors.ink(1.), rgb(SIGNAL));
        let a11y = self.a11y;
        let rows = (onboarding.engines.0.iter().enumerate()).map(|(index, (_, name))| {
            let on = onboarding.engine == Some(index);
            let dot = radio_dot(on, colors);
            let toggled = match on {
                true => Toggled::True,
                false => Toggled::False,
            };
            let row = div()
                .id(("onboarding-engine", index))
                .when(a11y, |row| {
                    row.role(Role::RadioButton)
                        .aria_label(name.clone())
                        .aria_toggled(toggled)
                        .when(on, |row| row.aria_active_descendant())
                })
                .h(px(ENGINE_ROW))
                .px(px(PADDING))
                .flex()
                .items_center()
                .gap(px(PADDING))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |shell, _, _, cx| shell.onboarding_pick_engine(index, cx)),
                )
                .flex_row_reverse()
                .justify_between()
                .child(dot)
                .child(
                    div()
                        .when(on, |name| name.font_weight(gpui::FontWeight::MEDIUM))
                        .child(name.clone()),
                );
            div().child(row)
        });
        // The ring is the box's own, as a button's is, drawn over its plate: on a frame round it,
        // the box would cover it.
        group(&look, rows)
            .id("onboarding-engines")
            .when(a11y, |list| {
                list.role(Role::RadioGroup).aria_label("Search engine")
            })
            .tab_index(0)
            .when_some(self.first_stop(Step::SearchEngine), |list, stop| {
                list.track_focus(&stop)
            })
            .focus_from_keys_only()
            .focus_visible(ring(colors.ink(1.)))
            .w_full()
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
            .into_any_element()
    }

    /// The assistant step's two choices, rows as the engines' are, the pick's dot filled: with an
    /// assistant, as agents are on, or just the browser, as they're off. A click on a row picks it,
    /// and ↑ ↓ move the pick from anywhere in the step (onboarding_key).
    fn agents_rows(&self, colors: Colors, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        let look = Look::on(surface, colors.ink(1.), rgb(SIGNAL));
        let agents = self.browser.threads(cx) == Some(true);
        let a11y = self.a11y;
        let rows = [
            (
                true,
                "With an assistant",
                "Works in its own tabs, on your plan",
            ),
            (
                false,
                "Just the browser",
                concat!(
                    "No assistant in ",
                    env!("APP_NAME"),
                    ", until you turn one on"
                ),
            ),
        ]
        .map(|(choice, name, sentence)| {
            let on = agents == choice;
            let row = div()
                .id(("onboarding-agents", usize::from(choice)))
                .when(a11y, |row| {
                    row.role(Role::RadioButton)
                        .aria_label(name)
                        .aria_toggled(match on {
                            true => Toggled::True,
                            false => Toggled::False,
                        })
                })
                .p(px(PADDING))
                .flex()
                .items_center()
                .gap(px(PADDING))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| shell.onboarding_agents(choice, cx)))
                .flex_row_reverse()
                .justify_between()
                .child(radio_dot(on, colors))
                // Its name over its line, as the opt-ins' rows have theirs.
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .line_height(px(OPT_IN_LINE))
                        .child(div().font_weight(gpui::FontWeight::MEDIUM).child(name))
                        .child(div().text_color(colors.ink(QUIET)).child(sentence)),
                );
            div().child(row)
        });
        group(&look, rows)
            .id("onboarding-agents")
            .when(a11y, |list| list.role(Role::RadioGroup))
            .w_full()
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
    }

    /// The favorites offered, as the import view's tiles across the sheet's left half, scrolling
    /// there above its foot, a click ticking or unticking one. Tiles like the favorites they become
    /// say where they go.
    fn favorite_tiles(
        &self,
        colors: Colors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let look = Look::on(
            colors.ground.blend(colors.ink(IMPORT_SURFACE)),
            colors.ink(1.),
            rgb(SIGNAL),
        );
        let view = self.importing.as_ref()?.view.clone();
        let tiles = view.update(cx, |view, cx| {
            view.set_look(look, cx);
            view.favorite_tiles(window, cx)
        })?;
        Some(frame::choice(tiles).into_any_element())
    }

    /// Skip, quiet at the window's bottom right, with Esc's chip.
    /// What the import view shows the first launch for its state, while the step has one.
    fn import_guide(&self, cx: &gpui::App) -> Option<Guide> {
        self.onboarding.as_ref()?;
        let importing = self.importing.as_ref()?;
        Some(importing.view.read(cx).guide())
    }

    /// Settings › Appearance's picker, the one form of it, at the level shown. The keyboard rests
    /// on it as the step comes, and the arrows move the pick from there, as the colour's do.
    fn level_slot(
        &self,
        step: Step,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (level, drawn) = (self.onboarding.as_ref())
            .map_or_else(Default::default, |o| (o.level, o.level_drawn.clone()));
        let look = settings_rows::Look::settings(colors.ink == PAPER);
        let shell = cx.weak_entity();
        let picker = transparency::picker(level, &look, drawn, move |level, _, cx| {
            shell
                .update(cx, |shell, cx| shell.onboarding_transparency(level, cx))
                .ok();
        })
        .centred();
        div()
            .id("onboarding-transparency")
            .when_some(self.first_stop(step), |slot, stop| slot.track_focus(&stop))
            .w_full()
            .child(picker)
    }

    /// The stop of the choice of `step`, while it's the step shown: the one leaving, drawn under
    /// it, has none.
    fn first_stop(&self, step: Step) -> Option<gpui::FocusHandle> {
        (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| onboarding.first_stop.clone())
    }

    /// Esc at the import: the view's own first, back from a browser's choices or the wait for
    /// access, or Cancel while its import can stop; else it skips the step.
    fn import_escape(&mut self, how: &str, cx: &mut Context<Self>) {
        let left = (self.importing.as_ref())
            .is_some_and(|importing| importing.view.update(cx, |view, cx| view.escape(cx)));
        match left {
            true => cx.notify(),
            false => self.onboarding_next(how, cx),
        }
    }

    /// Esc at the import: the view's own first, back from a browser's choices or the wait for
    /// access, or Cancel while its import can stop; else back a step.
    fn import_back(&mut self, how: &str, cx: &mut Context<Self>) {
        let left = (self.importing.as_ref())
            .is_some_and(|importing| importing.view.update(cx, |view, cx| view.escape(cx)));
        match left {
            true => cx.notify(),
            false => self.onboarding_back(how, cx),
        }
    }

    /// Keeps where the step shown draws `name`, as much of it as shows, for a test to click it;
    /// the step leaving, drawn under it, keeps none.
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
/// it there. A step coming forgets it, so each step logs its own.
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

/// A radio's dot: filled in the ink with the ground at its centre when `on`, else a ring.
fn radio_dot(on: bool, colors: Colors) -> gpui::Div {
    div()
        .flex_none()
        .size(px(ENGINE_DOT))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .map(|dot| match on {
            true => {
                dot.bg(colors.ink(1.))
                    .child(div().size(px(ENGINE_DOT_CENTER)).rounded_full().bg(Rgba {
                        a: 1.,
                        ..colors.ground
                    }))
            }
            false => dot.border_1().border_color(colors.ink(QUIET)),
        })
}

/// Where each opt-in's row and switch is drawn, by the opt-in and "row" or "switch".
type Frames = Rc<RefCell<HashMap<(Switch, &'static str), Bounds<Pixels>>>>;

/// Keeps where an opt-in's `part` is drawn once the step has `settled`, and logs where its switch
/// is each time it moves, as "browser: onboarding agents at x y", for check.sh to click it there.
fn opt_in_frame(
    frames: Frames,
    opt_in: Switch,
    part: &'static str,
    settled: bool,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let moved =
                settled && frames.borrow_mut().insert((opt_in, part), bounds) != Some(bounds);
            if moved && part == "switch" {
                let at = bounds.center();
                log!(
                    "browser: onboarding {} at {:.0} {:.0}",
                    opt_in.name(),
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

/// The theme picker's swatches in a row, the one picked ringed in the ink, each with a hairline
/// edge so the lightest still reads on Paper.
/// A screen reader reads them as one choice, a radio button each called by its colour.
fn swatch_row(
    picked: Option<usize>,
    first: Option<gpui::FocusHandle>,
    (swatch, pitch): (f32, f32),
    colors: Colors,
    a11y: bool,
    cx: &mut Context<Shell>,
) -> impl IntoElement + use<> {
    // Each swatch's ring and the gap inside it, on both sides.
    let ringed = swatch + 4. * SWATCH_RING;
    div()
        .id("onboarding-swatches")
        .when(a11y, |row| row.role(Role::RadioGroup).aria_label("Color"))
        // One stop, the arrows going along it, with the swatch picked read as they move.
        .tab_index(0)
        .when_some(first, |row, stop| row.track_focus(&stop))
        .focus_from_keys_only()
        .rounded_full()
        .focus_visible(ring(colors.ink(1.)))
        .mt(px(SWATCH_GAP))
        .flex()
        .gap(px(pitch - ringed))
        .children(SWATCHES.iter().enumerate().map(|(index, &color)| {
            let ring = if picked == Some(index) {
                colors.ink(1.)
            } else {
                colors.ink(0.)
            };
            let toggled = match picked == Some(index) {
                true => Toggled::True,
                false => Toggled::False,
            };
            div()
                .id(("onboarding-swatch", index))
                .when(a11y, |swatch| {
                    swatch
                        .role(Role::RadioButton)
                        .aria_label(swatch_name(color))
                        .aria_toggled(toggled)
                        .when(picked == Some(index), |swatch| {
                            swatch.aria_active_descendant()
                        })
                })
                .p(px(SWATCH_RING))
                .rounded_full()
                .border(px(SWATCH_RING))
                .border_color(ring)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| shell.onboarding_pick(index, cx)))
                .child(
                    div()
                        .size(px(swatch))
                        .rounded_full()
                        .bg(rgb(color))
                        .border_1()
                        .border_color(colors.ink(SWATCH_EDGE)),
                )
        }))
}

/// First run's main button for the welcome card, outside the flow: `button` with its label in it
/// and `key` at its end, in a light window's inks or a dark one's.
pub(super) fn main_button(
    button: gpui::Stateful<gpui::Div>,
    key: Option<SharedString>,
    light: bool,
) -> gpui::Stateful<gpui::Div> {
    frame::main_button(button, key, frame::Inks::of(Colors::of(light)))
}

/// The flow's two colours: the ground, and the ink drawn on it.
#[derive(Clone, Copy)]
struct Colors {
    ground: Rgba,
    ink: u32,
}

impl Colors {
    /// Paper with Ink on it in a light appearance, the other way round in a dark one, as the
    /// app icon's tile and stem.
    fn of(light: bool) -> Colors {
        match light {
            true => Colors {
                ground: rgb(PAPER),
                ink: INK,
            },
            false => Colors {
                ground: rgb(INK),
                ink: PAPER,
            },
        }
    }

    fn ink(&self, alpha: f32) -> Rgba {
        with_alpha(self.ink, alpha)
    }
}

fn with_alpha(color: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(color)
    }
}

#[cfg(test)]
mod tests;
