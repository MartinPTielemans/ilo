//! A new profile's first launch (docs/ux-spec.md › Onboarding): ilo's intro, then one message a
//! screen, over the whole window on the brand's ground. Only a launch with nothing to bring back
//! starts it, and Help › Welcome to ilo plays it again, so a normal launch never draws or loads any
//! of it. Any key or click skips the intro; Esc or Skip goes on from a step, and the last step's
//! leaves the flow for the first Space's welcome. A page shown ends it, as the page would cover it.
//! A step's button does what it offers, such as asking the system to make ilo the default browser,
//! giving the first Space its colour or signing in to sync; Skip doesn't. The flow ends with the
//! command bar up over the first Space, and a line on the card under it saying what the bar is.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Entity, KeyboardButton, KeyboardClickEvent,
    KeystrokeEvent, MouseButton, PathBuilder, Pixels, Rgba, SharedString, Subscription, Window,
    accesskit::{Role, Toggled},
    canvas, div, fill, point,
    prelude::*,
    px, rgb, size,
};

use super::{Command, Shell, cubic_bezier, display_face::display_text, fade_at};
use crate::account::{self, Account, AccountEvent, Status};
use crate::default_browser::{self, Os};
use crate::import::{Corner, Guide, ImportEvent, ImportView, Look};
use crate::settings_rows::{self, SLIDE, group, named_button, ring, stop_switch};
use crate::theme::Theme;
use crate::theme_picker::{SWATCHES, swatch_name};
use crate::tokens::palette::{INK, PAPER};
use crate::tokens::settings::PADDING;
use crate::tokens::shell::onboarding::BUTTON_GAP;
use crate::tokens::shell::onboarding::BUTTON_HEIGHT;
use crate::tokens::shell::onboarding::BUTTON_INSET;
use crate::tokens::shell::onboarding::BUTTON_RADIUS;
use crate::tokens::shell::onboarding::BUTTON_SIZE;
use crate::tokens::shell::onboarding::CHIP;
use crate::tokens::shell::onboarding::CHIP_GAP;
use crate::tokens::shell::onboarding::CHIP_HEIGHT;
use crate::tokens::shell::onboarding::CHIP_INSET;
use crate::tokens::shell::onboarding::CHIP_RADIUS;
use crate::tokens::shell::onboarding::ENGINE_DOT;
use crate::tokens::shell::onboarding::ENGINE_DOT_CENTER;
use crate::tokens::shell::onboarding::ENGINE_ROW;
use crate::tokens::shell::onboarding::IMPORT_HOLD;
use crate::tokens::shell::onboarding::IMPORT_SURFACE;
use crate::tokens::shell::onboarding::IMPORT_WIDTH;
use crate::tokens::shell::onboarding::LINE_GAP;
use crate::tokens::shell::onboarding::LINE_LINE;
use crate::tokens::shell::onboarding::LINE_SIZE;
use crate::tokens::shell::onboarding::OPT_IN_LINE;
use crate::tokens::shell::onboarding::OPT_IN_WIDTH;
use crate::tokens::shell::onboarding::QUIET;
use crate::tokens::shell::onboarding::SHIFT;
use crate::tokens::shell::onboarding::SIGNAL;
use crate::tokens::shell::onboarding::SKIP_CHIP;
use crate::tokens::shell::onboarding::SKIP_MARGIN;
use crate::tokens::shell::onboarding::SKIP_SIZE;
use crate::tokens::shell::onboarding::SWATCH;
use crate::tokens::shell::onboarding::SWATCH_EDGE;
use crate::tokens::shell::onboarding::SWATCH_GAP;
use crate::tokens::shell::onboarding::SWATCH_PITCH;
use crate::tokens::shell::onboarding::SWATCH_RING;
use crate::tokens::shell::onboarding::TITLE_LINE;
use crate::tokens::shell::onboarding::TITLE_SIZE;
use crate::transparency::{self, Transparency};

mod loops;
mod sheet;

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
    /// Signing in to sync: Settings › Sync's view (crate::account); left out in a build that names
    /// no sync server, and once the browser is signed in.
    Account,
    /// Settings' opt-in switches, both off; left out when neither is offered.
    OptIns,
}

const STEPS: [Step; 9] = [
    Step::Intro,
    Step::Import,
    Step::Favorites,
    Step::DefaultBrowser,
    Step::SearchEngine,
    Step::Color,
    // After the colour, as a Space's glow reads through the same glass.
    Step::Transparency,
    Step::Account,
    Step::OptIns,
];

/// The opt-ins the first launch offers, each Settings' own switch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum OptIn {
    /// Off until the user turns it on, and set only as they leave the step, so nothing is sent
    /// while it shows (#2978).
    DailyCount,
    CrashReports,
    Agents,
}

impl OptIn {
    const ALL: [OptIn; 3] = [OptIn::DailyCount, OptIn::CrashReports, OptIn::Agents];

    /// What check.sh calls it.
    fn name(self) -> &'static str {
        match self {
            OptIn::DailyCount => "daily-count",
            OptIn::CrashReports => "crash-reports",
            OptIn::Agents => "agents",
        }
    }

    /// Its switch's name and its one sentence: the name Settings' row has, as it's that switch.
    fn words(self) -> (&'static str, SharedString) {
        match self {
            OptIn::DailyCount => (
                "Send a daily usage count",
                crate::daily_count::words(env!("APP_NAME")).into(),
            ),
            OptIn::CrashReports => (
                "Send crash reports",
                concat!(
                    "When ",
                    env!("APP_NAME"),
                    " crashes, it sends a report of what went wrong, so it can be fixed."
                )
                .into(),
            ),
            // Settings' own line while it's off.
            OptIn::Agents => (
                "Let agents use the browser",
                "They work in tabs of their own, which you can watch, and never see your \
                 passwords, cookies or autofill."
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
            Step::Account => "account",
            Step::OptIns => "opt-ins",
        }
    }

    /// The step's one message on `os`: its title, its line and its button's words.
    fn message(self, os: Os) -> (&'static str, gpui::SharedString, &'static str) {
        let (title, line, button) = match self {
            // The sheet's first page (#4093): what the flow is, and that each step can be skipped.
            Step::Intro => (
                concat!("Welcome to ", env!("APP_NAME"), "."),
                "A good tool. A minute to make it yours, and every step can be skipped.",
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
                let title = concat!("Make ", env!("APP_NAME"), " your default browser.");
                return (title, line.into(), button);
            }
            // The engine that searches until one is picked is already the filled dot, so the line
            // needn't name it.
            Step::SearchEngine => (
                "Pick a search engine.",
                "What you type that isn't an address searches with it. You can change it later in \
                 Settings.",
                "Continue",
            ),
            Step::Color => (
                "Pick a colour for your first Space.",
                "The window takes it. Each Space can have its own, from its name in the sidebar.",
                "Continue",
            ),
            Step::Transparency => (
                "How much should show through?",
                concat!(
                    env!("APP_NAME"),
                    "'s window is glass. Pick one to see the window change right away."
                ),
                "Continue",
            ),
            // Its button only while signed out; the view's own buttons are the step's after.
            Step::Account => (
                "Your Spaces on every computer.",
                "Spaces, bookmarks and passwords sync encrypted end to end, and \
                 Settings has it too.",
                "Sign In",
            ),
            Step::OptIns => (
                "Up to you.",
                "Each stays off until you turn it on, and Settings has it too.",
                "Continue",
            ),
        };
        (title, line.into(), button)
    }
}

/// The intro's timeline (*Ours*, docs/ux-spec.md › Motion › Intro), in seconds from its first
/// frame, each part's start and length. The strokes draw on the check's ease-in-out, as a pen's
/// would, the stems from their feet and the ring from its top, the way a hand writes "ilo"; the dot
/// lands last, in the ink, and takes the green, and then the line comes.
const STEM_I: (f32, f32) = (0.2, 0.4);
const STEM_L: (f32, f32) = (0.32, 0.48);
const RING: (f32, f32) = (0.48, 0.62);
const DOT: (f32, f32) = (1.1, 0.24);
const SIGNAL_IN: (f32, f32) = (1.34, 0.3);
const TAGLINE_IN: (f32, f32) = (1.64, 0.4);
/// With Reduce Motion, the wordmark and its line fade in whole together.
const STILL_FADE: f32 = 0.4;
/// A step comes in as the one before leaves, the icon picker's tabs' 200 ms ease-out.
const STEP_MOTION: f32 = 0.2;
/// The flow fades from over the first Space at the end, on the scroll edge's curve.
const END_FADE: f32 = 0.2;

/// How much of each part of the wordmark and its line is drawn, from 0 to 1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Drawn {
    pub stem_i: f32,
    pub stem_l: f32,
    pub ring: f32,
    pub dot: f32,
    pub signal: f32,
    pub tagline: f32,
}

impl Drawn {
    /// The intro `t` seconds in, or `still`, under Reduce Motion, all of it fading in at once.
    pub(super) fn at(t: f32, still: bool) -> Drawn {
        // Each part's curve, exactly at its ends, so a part not yet started isn't drawn at all.
        let along = |(start, length): (f32, f32), curve: fn(f32) -> f32| match (t - start) / length
        {
            x if x <= 0. => 0.,
            x if x >= 1. => 1.,
            x => curve(x),
        };
        if still {
            let fade = along((0., STILL_FADE), fade);
            return Drawn {
                stem_i: 1.,
                stem_l: 1.,
                ring: 1.,
                dot: 1.,
                signal: 1.,
                tagline: fade,
            };
        }
        Drawn {
            stem_i: along(STEM_I, ease_in_out),
            stem_l: along(STEM_L, ease_in_out),
            ring: along(RING, ease_in_out),
            dot: along(DOT, ease_out),
            signal: along(SIGNAL_IN, ease_in_out),
            tagline: along(TAGLINE_IN, fade),
        }
    }

    /// Whether it's all there.
    fn done(&self) -> bool {
        self.tagline >= 1.
    }
}

/// The account isn't signed in, only waiting perhaps for the sign-in window, as the core said last,
/// or before it has said.
fn signed_out(state: &account::State) -> bool {
    matches!(state.status, Status::SignedOut | Status::SigningIn)
}

/// The check draw's curve, CSS's ease-in-out.
fn ease_in_out(x: f32) -> f32 {
    cubic_bezier(0.42, 0., 0.58, 1., x)
}

/// The scroll edge's fade, as the empty card's line comes.
fn fade(x: f32) -> f32 {
    fade_at(0., 1., x, 1.)
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
    switched: Option<(OptIn, Instant)>,
    /// That switch's knob has slid all the way, which check.sh hears once.
    slid: bool,
    /// ⇥ put the keyboard on the opt-ins' switches, which it leaves with them (onboarding_layer).
    on_switches: bool,
    /// The intro holds the keyboard itself, as does a step left with no control, so an assistive
    /// app, which follows the keyboard, reads each by its title and line as it comes (#3151).
    focus: gpui::FocusHandle,
    /// ⇥'s stops at the step's button and at its corner's Skip or Back, which ↩ and Space press.
    continue_stop: gpui::FocusHandle,
    skip_stop: gpui::FocusHandle,
    /// The sheet's Back, before its Skip.
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
    /// GPUI's clock, which the flow's motion and the intro's hold are timed by: the real one in the
    /// app, and in a test the one its timers run on, so the two never part (#3248).
    clock: gpui::BackgroundExecutor,
    /// When the step came, which its motion runs from.
    pub(super) since: Instant,
    /// The step before, leaving as this one comes, and how far into it it was when it left.
    leaving: Option<(Step, f32)>,
    /// That step was left with Back, so it leaves to the right and this one comes from the left.
    backward: bool,
    /// When the step's loop started, while it plays: only on a visible window, never with Reduce
    /// Motion, and never as the flow ends (loops).
    loop_since: Option<Instant>,
    /// The loop held this many seconds in, for the preview's captures, asking for no frames.
    held: Option<f32>,
    /// When the last step was left, for the flow's fade.
    pub(super) ending: Option<Instant>,
    /// Light or dark where check.sh says, else the window's.
    light: Option<bool>,
    /// The step has come all the way and said so.
    settled: bool,
    /// Keys in this window go on from a step, while the flow shows.
    keys: Option<gpui::Subscription>,
    /// The flow has given the window's keyboard to the UI.
    keyboard: bool,
    /// The account step's view, while the step is offered.
    account: Option<Signing>,
    /// The daily usage count's switch as turned at the opt-ins step, None while untouched: it's
    /// set as the step is left (onboarding_next), so turned on and off again it sends nothing.
    count: Option<bool>,
}

impl Onboarding {
    /// Seconds since `at` on the flow's clock.
    fn secs_since(&self, at: Instant) -> f32 {
        self.clock.now().saturating_duration_since(at).as_secs_f32()
    }
}

impl Shell {
    /// Starts the flow at the intro in this window, which shows no page. `appearance` is check.sh's
    /// "light" or "dark", else the window's own.
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
                Step::OptIns => opt_ins,
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
            on_switches: false,
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
            clock: cx.background_executor().clone(),
            since: cx.background_executor().now(),
            leaving: None,
            backward: false,
            loop_since: None,
            held: None,
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
            count: None,
        });
        cx.notify();
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

    /// Help › Welcome to ilo: the flow again, here if the window shows no page, which GPUI can't
    /// draw over, else in a new window as ⌘N's, which shows none.
    pub(super) fn welcome_again(&mut self, cx: &mut Context<Self>) {
        match self.selected {
            0 => self.start_onboarding(None, cx),
            _ => self.browser.welcome_window(cx),
        }
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
        match at.and_then(|at| onboarding.steps.get(at + 1)).copied() {
            Some(next) => {
                let elapsed = onboarding.secs_since(onboarding.since);
                onboarding.leaving = Some((step, elapsed));
                onboarding.backward = false;
                onboarding.step = next;
                onboarding.since = onboarding.clock.now();
                onboarding.loop_since = None;
                onboarding.settled = false;
                log!("browser: onboarding step {}", next.name());
            }
            None => onboarding.ending = Some(onboarding.clock.now()),
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
        onboarding.backward = true;
        onboarding.step = before;
        onboarding.since = onboarding.clock.now();
        onboarding.loop_since = None;
        onboarding.settled = false;
        log!("browser: onboarding step {}", before.name());
        if step == Step::Color {
            self.onboarding_unpaint();
        }
        cx.notify();
    }

    /// The welcome's Skip Setup, and its Esc: the flow ends there, as it does after its last step,
    /// with nothing set.
    fn onboarding_skip_setup(&mut self, how: &str, cx: &mut Context<Self>) {
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

    /// The step's loop is playing, and asking for frames.
    #[cfg(test)]
    pub(super) fn onboarding_looping(&self) -> bool {
        (self.onboarding.as_ref()).is_some_and(|onboarding| onboarding.loop_since.is_some())
    }

    /// Holds the step's loop `at` seconds in, or lets it play from there with none, for the
    /// preview's captures and frames.
    pub fn onboarding_hold_loop(&mut self, at: Option<f32>, cx: &mut Context<Self>) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.held = at;
            // The preview's frames start on this line.
            log!(
                "browser: onboarding loop {}",
                if at.is_some() { "held" } else { "plays" }
            );
            cx.notify();
        }
    }

    /// Turns the daily count's switch on at the opt-ins step, for the preview's captures.
    pub fn onboarding_count_on(&mut self, cx: &mut Context<Self>) {
        self.onboarding_switch(OptIn::DailyCount, cx);
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
    pub(super) fn opt_ins(&self, cx: &gpui::App) -> Vec<(OptIn, bool)> {
        let on = |opt_in| match opt_in {
            OptIn::DailyCount => {
                let kept = self.browser.daily_count(cx)?;
                Some(
                    self.onboarding
                        .as_ref()
                        .and_then(|o| o.count)
                        .unwrap_or(kept),
                )
            }
            OptIn::CrashReports => self.browser.crash_reports(cx),
            OptIn::Agents => self.browser.agents(cx),
        };
        OptIn::ALL
            .into_iter()
            .filter_map(|opt_in| Some((opt_in, on(opt_in)?)))
            .collect()
    }

    /// Turns an opt-in's switch, which is Settings' own, so it's set at once.
    pub(super) fn onboarding_switch(&mut self, opt_in: OptIn, cx: &mut Context<Self>) {
        let Some(&(_, on)) = self.opt_ins(cx).iter().find(|(each, _)| *each == opt_in) else {
            return;
        };
        // check.sh reads this line.
        log!(
            "browser: onboarding {} {}",
            opt_in.name(),
            if on { "off" } else { "on" }
        );
        // After this update, as turning agents off ends their ask, which reads every Shell (#2459).
        let browser = self.browser.clone();
        cx.defer(move |cx| match opt_in {
            OptIn::DailyCount => {}
            OptIn::CrashReports => browser.set_crash_reports(!on, cx),
            OptIn::Agents => browser.set_agents(!on, cx),
        });
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.switched = Some((opt_in, onboarding.clock.now()));
            onboarding.slid = false;
            if opt_in == OptIn::DailyCount {
                onboarding.count = Some(!on);
            }
        }
        cx.notify();
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

    /// The flow shows its transparency step, or moves to or from it, over the window's glass, so
    /// the shell draws nothing of its own under it.
    pub(super) fn onboarding_bare(&self) -> bool {
        self.onboarding.as_ref().is_some_and(|onboarding| {
            onboarding.ending.is_none()
                && (onboarding.step == Step::Transparency
                    || (onboarding.leaving).is_some_and(|(step, _)| step == Step::Transparency))
        })
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

    /// A key in the window: in a step Esc skips it and return is its button, but at the welcome
    /// Esc skips the whole setup; at the colour step the arrows go along the swatches. A shortcut still does its own
    /// thing first.
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
        // The account's fields keep their own ↩ and Esc, and its words go only with Done.
        let field = event
            .context_stack
            .iter()
            .any(|context| context.contains("AccountField"));
        if step == Step::Account && (field || self.keeping_words(cx)) {
            return;
        }
        match (step, key.key.as_str()) {
            (Step::Intro, "escape") => self.onboarding_skip_setup("escape", cx),
            (Step::Import, "escape") => self.import_escape("escape", cx),
            (_, "escape") => self.onboarding_next("escape", cx),
            // From none picked, → starts at the first and ← at the last.
            (Step::Color, "right") => self.onboarding_pick(picked.map_or(0, |at| at + 1), cx),
            (Step::Color, "left") => {
                self.onboarding_pick(picked.map_or(SWATCHES.len(), |at| at.max(1)) - 1, cx)
            }
            (Step::Transparency, "left" | "up") => self.onboarding_level(-1, cx),
            (Step::Transparency, "right" | "down") => self.onboarding_level(1, cx),
            // ↑ and ↓ move the engine's pick, as they do the import's list's.
            (Step::SearchEngine, "down") => {
                self.onboarding_pick_engine(engine.map_or(0, |at| at + 1), cx)
            }
            (Step::SearchEngine, "up") => {
                self.onboarding_pick_engine(engine.map_or(0, |at| at.saturating_sub(1)), cx)
            }
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
                if let Some(onboarding) = self.onboarding.as_mut() {
                    onboarding.on_switches = step == Step::OptIns;
                }
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
            Step::Color | Step::SearchEngine | Step::Transparency => onboarding.first_stop.clone(),
            step if self.onboarding_has_button(step, cx) => onboarding.continue_stop.clone(),
            _ => onboarding.focus.clone(),
        })
    }

    /// The flow over the whole window while it shows, stepping its motion; none otherwise, at no
    /// cost but the check.
    pub(super) fn onboarding_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let rest = self.onboarding_rest(cx);
        let onboarding = self.onboarding.as_mut()?;
        // The switches go with their step, and a keyboard left on one would leave the shell's own
        // keys, ⌘T among them, with nowhere to go off the Mac, so it goes back to the shell.
        let left = onboarding.step != Step::OptIns || onboarding.ending.is_some();
        // So do the step and its stops as the flow ends.
        let held = onboarding.focus.is_focused(window)
            || onboarding.continue_stop.is_focused(window)
            || onboarding.skip_stop.is_focused(window)
            || onboarding.back_stop.is_focused(window)
            || onboarding.first_stop.is_focused(window);
        if (left && std::mem::take(&mut onboarding.on_switches))
            || (onboarding.ending.is_some() && held)
        {
            window.focus(&self.focus_handle, cx);
        }
        // A page shown covers it, so it has ended.
        let ended = onboarding
            .ending
            .is_some_and(|at| onboarding.secs_since(at) >= END_FADE);
        if ended || self.selected != 0 {
            self.onboarding = None;
            // check.sh waits for this line.
            log!("browser: onboarding done");
            // In: the first Space with the command bar up, as ⌘T has it, once this frame is drawn.
            // Not for a quit, whose closing window can draw the frame the fade ends in.
            if ended && self.selected == 0 && !super::quit_gone_ahead() {
                self.in_hint = true;
                cx.defer_in(window, |shell, _, cx| {
                    shell.run(Command::NewTab, "onboarding", cx)
                });
            }
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
        // A window that opens with no page gives its keyboard to Chromium's view, so the flow's
        // keys would never reach it. By the flow's first frame on screen the window is key, and
        // the keyboard stays the UI's. Deferred, as AppKit tells GPUI the change at once.
        if window.is_visible() && !std::mem::replace(&mut onboarding.keyboard, true) {
            cx.defer_in(window, |shell, _, cx| shell.take_keyboard(cx));
        }
        // Each step takes the keyboard as it comes, from the window or from the step before, so an
        // assistive app reads it; never from a field that has it.
        let resting = window.focused(cx).is_none_or(|focus| {
            focus == self.focus_handle
                || focus == onboarding.focus
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
        let still = cx.reduce_motion();
        let elapsed = onboarding.secs_since(onboarding.since);
        let step = onboarding.step;
        let came = match step {
            Step::Intro => Drawn::at(elapsed, still).done(),
            _ => elapsed >= STEP_MOTION,
        };
        if came && !std::mem::replace(&mut onboarding.settled, true) {
            // check.sh waits for this line before it captures the step.
            log!("browser: onboarding step {} settled", step.name());
        }
        let motion = (elapsed / STEP_MOTION).min(1.);
        if motion >= 1. {
            onboarding.leaving = None;
        }
        // Frames while something moves, and while the step's loop plays (loops).
        let ending = onboarding.ending.map(|at| onboarding.secs_since(at));
        let looping = loops::has_loop(step)
            && !still
            && ending.is_none()
            && onboarding.held.is_none()
            && window.is_visible();
        match looping {
            true => _ = *onboarding.loop_since.get_or_insert(onboarding.clock.now()),
            false => onboarding.loop_since = None,
        }
        let played = (onboarding.loop_since).map(|at| onboarding.secs_since(at));
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
        onboarding.drawing =
            !came || onboarding.leaving.is_some() || ending.is_some() || sliding || looping;
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
        let backward = onboarding.backward;
        let colors = Colors::of(light);
        // The step comes from the right as the one before leaves to the left, but for a fade with
        // Reduce Motion. The intro, first, is simply there.
        let eased = ease_out(motion);
        let shift = match (still || leaving.is_none(), backward) {
            (true, _) => 0.,
            (false, false) => SHIFT,
            (false, true) => -SHIFT,
        };
        let looped = onboarding.held.unwrap_or_else(|| loops::at(played, still));
        let coming = self
            .step_content(step, elapsed, looped, still, colors, window, cx)
            .left(px(shift * (1. - eased)))
            .opacity(if leaving.is_some() { eased } else { 1. });
        let gone = leaving.map(|(step, at)| {
            self.step_content(step, at, loops::at(None, true), still, colors, window, cx)
                .left(px(-shift * eased))
                .opacity(1. - eased)
        });
        // The recovery words show once, so only their Done goes on from them.
        // At the import, the corner says what Esc does in the state its view shows.
        let corner = match step {
            Step::Import => self
                .import_guide(cx)
                .map_or(Corner::Skip, |guide| guide.corner),
            _ => Corner::Skip,
        };
        // A step on the sheet has its own in its foot.
        let skip = (!sheet::on_sheet(step) && !self.keeping_words(cx) && corner != Corner::None)
            .then(|| self.skip_button(corner == Corner::Back, colors, cx));
        // The import under way, or what it brought, quiet at the bottom left once its step is left.
        let import_line = (step != Step::Import && !sheet::on_sheet(step))
            .then(|| self.import_line(cx))
            .flatten()
            .map(|said| {
                div()
                    .absolute()
                    .left(px(SKIP_MARGIN))
                    .bottom(px(SKIP_MARGIN))
                    .h(px(CHIP_HEIGHT))
                    .flex()
                    .items_center()
                    .text_size(px(SKIP_SIZE))
                    .text_color(colors.ink(QUIET))
                    .child(said)
            });
        let opacity = ending.map_or(1., |at| fade_at(1., 0., at, END_FADE));
        // At the transparency step the ground gives way to the window's own glass, at the level
        // picked, so the whole window over what's behind it is the step's preview; on the sheet,
        // to the window itself, which the sheet floats over. It comes back as the step goes.
        let bare = |step: Step| match step == Step::Transparency || sheet::on_sheet(step) {
            true => 1.,
            false => 0.,
        };
        let glass = match leaving {
            Some((left, _)) => bare(left) + (bare(step) - bare(left)) * eased,
            None => bare(step),
        };
        let ground = Rgba {
            a: colors.ground.a * (1. - glass),
            ..colors.ground
        };
        Some(
            div()
                .id("onboarding")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(ground)
                .occlude()
                .opacity(opacity)
                .children(gone)
                .child(coming)
                .children(skip)
                .children(import_line)
                .into_any_element(),
        )
    }

    /// A step's message, centred in the window, `t` seconds into it, its loop `looped` seconds
    /// into its own.
    #[allow(clippy::too_many_arguments)]
    fn step_content(
        &self,
        step: Step,
        t: f32,
        looped: f32,
        still: bool,
        colors: Colors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let layer = div()
            .absolute()
            .top_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center();
        // A screen reader reads the step shown as a dialog called by its title, with its line, and
        // the step leaving under it, still drawn as it slides, as nothing.
        let named = self.a11y
            && (self.onboarding.as_ref()).is_some_and(|onboarding| onboarding.step == step);
        let os = self
            .onboarding
            .as_ref()
            .map_or(Os::Mac, |onboarding| onboarding.os);
        let (title, line, button) = step.message(os);
        let switches = (step == Step::OptIns).then(|| self.opt_in_rows(colors, cx));
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
            let (swatch, pitch) = match sheet::on_sheet(step) {
                true => (sheet::SWATCH, sheet::SWATCH_PITCH),
                false => (SWATCH, SWATCH_PITCH),
            };
            swatch_row(
                picked,
                self.first_stop(step),
                (swatch, pitch),
                colors,
                self.a11y,
                cx,
            )
        });
        let chip = |keys: String, fill: Rgba| {
            div()
                .h(px(CHIP_HEIGHT))
                .px(px(CHIP_INSET))
                .flex()
                .items_center()
                .rounded(px(CHIP_RADIUS))
                .bg(fill)
                .child(keys)
        };
        // A view of Settings' own, its boxes a little into the ink from the ground, in the ink, as
        // Settings' are.
        let look = Look::on(
            colors.ground.blend(colors.ink(IMPORT_SURFACE)),
            colors.ink(1.),
            rgb(SIGNAL),
        );
        let embedded = |view: gpui::AnyView| {
            div()
                .mt(px(BUTTON_GAP))
                .w(px(IMPORT_WIDTH))
                .text_size(px(crate::tokens::text::BODY))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(view)
        };
        let import = (step == Step::Import)
            .then(|| {
                self.importing
                    .as_ref()
                    .map(|importing| importing.view.clone())
            })
            .flatten()
            .map(|view| {
                view.update(cx, |view, cx| view.set_look(look, cx));
                embedded(view.into())
            });
        // Under the step's Sign In, its server's line close by, as it names where Sign In goes.
        let signed_out = !button.is_empty();
        let account = account.map(|view| {
            view.update(cx, |view, cx| view.set_look(look, cx));
            embedded(view.into()).when(signed_out, |body| body.mt(px(LINE_GAP)))
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
        let button = (!button.is_empty()).then(|| {
            div()
                .id("onboarding-continue")
                .when(self.a11y, |control| named_button(control, button.clone()))
                .when_some(stops.clone(), |control, (stop, _)| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .focus_visible(ring(look.ink))
                .relative()
                .mt(px(BUTTON_GAP))
                .h(px(BUTTON_HEIGHT))
                .px(px(BUTTON_INSET))
                .flex()
                .items_center()
                .gap(px(CHIP_GAP))
                .rounded(px(BUTTON_RADIUS))
                .bg(rgb(SIGNAL))
                .text_color(rgb(PAPER))
                .text_size(px(BUTTON_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                // The dialog's Delete darkens as much under the pointer.
                .hover(|style| style.opacity(0.9))
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
                .child(chip(
                    super::key_name("enter", super::OFF_THE_MAC),
                    with_alpha(PAPER, CHIP),
                ))
                .children(drawn.map(|(at, settled)| button_frame(at, settled)))
                .when(sheet::on_sheet(step), sheet::go)
        });
        let focus = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| onboarding.focus.clone());
        if sheet::on_sheet(step) {
            let title = display_text(title, sheet::TITLE_SIZE, sheet::TITLE_LINE, cx);
            let left = self.sheet_left(step, t, still, (title, line), colors);
            let left = left
                .children(swatches)
                .child(self.sheet_foot(step, button, colors, cx));
            let art = self.sheet_art(step, looped, colors);
            let dialog = |pane: gpui::Stateful<gpui::Div>| {
                pane.when_some(focus, |content, focus| content.track_focus(&focus))
                    .when(named, |content| {
                        let (title, line) = said;
                        content
                            .role(Role::Dialog)
                            .aria_label(title)
                            .aria_description(line)
                    })
            };
            return sheet::sheet(
                colors,
                ("onboarding-step", step as usize),
                dialog,
                left,
                art,
            );
        }
        layer.child(
            div()
                .id(("onboarding-step", step as usize))
                .when_some(focus, |content, focus| content.track_focus(&focus))
                .when(named, |content| {
                    let (title, line) = said;
                    content
                        .role(Role::Dialog)
                        .aria_label(title)
                        .aria_description(line)
                })
                .flex()
                .flex_col()
                .items_center()
                // The import's words run as wide as its view, as its states' lines are longer.
                .max_w(px(if step == Step::Import {
                    IMPORT_WIDTH
                } else {
                    480.
                }))
                .child(display_text(title, TITLE_SIZE, TITLE_LINE, cx).text_color(colors.ink(1.)))
                .child(
                    div()
                        .mt(px(LINE_GAP))
                        .text_center()
                        .text_size(px(LINE_SIZE))
                        .line_height(px(LINE_LINE))
                        .text_color(colors.ink(QUIET))
                        .child(line),
                )
                .children(swatches)
                .children(level)
                .children(switches)
                .children(engines)
                .children(favorites)
                .children(import)
                .children(button)
                .children(account),
        )
    }

    /// The opt-ins offered in one box, as Settings' Privacy has them: a row each of its name over
    /// its sentence, wrapping in the box, and Settings' switch in the green at the box's right edge,
    /// centred on the row. A click anywhere on a row, the switch included, turns it.
    fn opt_in_rows(&self, colors: Colors, cx: &mut Context<Self>) -> gpui::Div {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div();
        };
        let switched = onboarding
            .switched
            .map(|(turned, at)| (turned, onboarding.secs_since(at)));
        // Where they're drawn is kept once the step has come, not while it slides in.
        let settled = onboarding.step == Step::OptIns && onboarding.settled;
        let frames = onboarding.frames.clone();
        // The import step's boxes, the ink a little over the ground.
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        let look = Look::on(surface, colors.ink(1.), rgb(SIGNAL));
        let rows = self.opt_ins(cx).into_iter().map(|(opt_in, on)| {
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
                .child(div().text_color(colors.ink(QUIET)).child(sentence));
            let control = div()
                .relative()
                .flex_none()
                .ml(px(PADDING))
                .child(
                    stop_switch(&look, "switch", name, on, slid, look.accent, self.a11y)
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
            .mt(px(BUTTON_GAP))
            .w(px(OPT_IN_WIDTH))
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
            .child(group(&look, rows))
    }

    /// The search engines offered in the opt-ins' box, a row each of a radio's dot and the engine's
    /// name, the one picked filled in the green and its name medium. A click on a row picks it, and
    /// the list is one stop for ⇥, its arrows moving the pick, as the swatches are.
    fn engine_rows(&self, colors: Colors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div().into_any_element();
        };
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        let look = Look::on(surface, colors.ink(1.), rgb(SIGNAL));
        let a11y = self.a11y;
        let rows = (onboarding.engines.0.iter().enumerate()).map(|(index, (_, name))| {
            let on = onboarding.engine == Some(index);
            let dot = div()
                .flex_none()
                .size(px(ENGINE_DOT))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .map(|dot| match on {
                    true => dot.bg(rgb(SIGNAL)).child(
                        div()
                            .size(px(ENGINE_DOT_CENTER))
                            .rounded_full()
                            .bg(rgb(PAPER)),
                    ),
                    false => dot.border_1().border_color(colors.ink(QUIET)),
                });
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
            .mt(px(BUTTON_GAP))
            .w(px(OPT_IN_WIDTH))
            .text_size(px(crate::tokens::text::BODY))
            .text_color(look.ink)
            .into_any_element()
    }

    /// The favorites offered, as the import view's tiles at its width, on the flow's ground, a
    /// click ticking or unticking one. Tiles like the favorites they become say where they go.
    fn favorite_tiles(
        &self,
        colors: Colors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
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
        Some(
            div()
                .mt(px(BUTTON_GAP))
                .w(px(IMPORT_WIDTH))
                .flex()
                .flex_col()
                .items_center()
                .text_size(px(crate::tokens::text::BODY))
                .child(div().w_full().child(tiles)),
        )
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
            .mt(px(BUTTON_GAP))
            .w(px(IMPORT_WIDTH))
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

    /// The corner's Skip, or at a state of the import with somewhere to go back to, Back.
    fn skip_button(
        &self,
        back: bool,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let chip = div()
            .h(px(CHIP_HEIGHT))
            .px(px(CHIP_INSET))
            .flex()
            .items_center()
            .rounded(px(CHIP_RADIUS))
            .bg(colors.ink(SKIP_CHIP))
            .child("esc");
        let stop = (self.onboarding.as_ref()).map(|onboarding| onboarding.skip_stop.clone());
        div()
            .id("onboarding-skip")
            .when(self.a11y, |control| {
                named_button(control, if back { "Back" } else { "Skip" })
            })
            .when_some(stop, |control, stop| {
                control.track_focus(&stop).focus_from_keys_only()
            })
            .rounded(px(CHIP_RADIUS))
            .focus_visible(ring(colors.ink(1.)))
            .absolute()
            .right(px(SKIP_MARGIN))
            .bottom(px(SKIP_MARGIN))
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .text_size(px(SKIP_SIZE))
            .text_color(colors.ink(QUIET))
            // A step brighter under the pointer, as a quiet control does.
            .hover(move |style| style.text_color(colors.ink(1.)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| match back {
                true => shell.import_escape("back", cx),
                false => shell.onboarding_next("skip", cx),
            }))
            .child(if back { "Back" } else { "Skip" })
            .child(chip)
            .children(self.place(
                self.onboarding.as_ref().map_or(Step::Intro, |o| o.step),
                "skip",
            ))
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

/// Where the step shown draws a control, by its name (Shell::place).
pub(super) type Places = Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>;

/// Where each opt-in's row and switch is drawn, by the opt-in and "row" or "switch".
type Frames = Rc<RefCell<HashMap<(OptIn, &'static str), Bounds<Pixels>>>>;

/// Keeps where an opt-in's `part` is drawn once the step has `settled`, and logs where its switch
/// is each time it moves, as "browser: onboarding agents at x y", for check.sh to click it there.
fn opt_in_frame(
    frames: Frames,
    opt_in: OptIn,
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
        .when(a11y, |row| row.role(Role::RadioGroup).aria_label("Colour"))
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

/// The brand's wordmark (ilo-studio's brand/logo/wordmark.svg), in its own units: a box from
/// (7, 16), 214 × 154, holding the i's dot and stem, the l's stem, and the o's ring.
const BOX: (f32, f32, f32, f32) = (7., 16., 214., 154.);
const DOT_AT: (f32, f32, f32) = (19., 31., 15.);
/// Each stem's left, top and bottom; both are 24 wide with round ends.
const STEMS: [(f32, f32, f32); 2] = [(7., 70., 170.), (67., 20., 170.)];
const STEM_WIDTH: f32 = 24.;
/// The ring's middle, radius and stroke.
const RING_AT: (f32, f32, f32, f32) = (171., 120., 38., 24.);
/// Straight stretches a whole ring is drawn in.
const RING_STRETCHES: usize = 96;

/// The wordmark, `drawn` as far as it is, `scale` times its own units.
fn wordmark(drawn: Drawn, colors: Colors, scale: f32) -> impl IntoElement {
    let ink = colors.ink(1.);
    let dot_color = mix(ink, rgb(SIGNAL), drawn.signal);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let at = |x: f32, y: f32| {
                bounds.origin + point(px((x - BOX.0) * scale), px((y - BOX.1) * scale))
            };
            // The stems grow up from their feet, round at both ends.
            for ((left, top, foot), way) in STEMS.into_iter().zip([drawn.stem_i, drawn.stem_l]) {
                let height = (foot - top) * way;
                if height <= 0. {
                    continue;
                }
                let stem = Bounds::new(
                    at(left, foot - height),
                    size(px(STEM_WIDTH * scale), px(height * scale)),
                );
                let radius = px(STEM_WIDTH.min(height) / 2. * scale);
                window.paint_quad(fill(stem, ink).corner_radii(radius));
            }
            // The ring from its top, anticlockwise as a hand writes an o, with round ends, which
            // GPUI's stroke doesn't draw, as the check's.
            let (cx_, cy, radius, stroke) = RING_AT;
            let stretches = (RING_STRETCHES as f32 * drawn.ring).ceil() as usize;
            if stretches > 0 {
                let point_at = |i: usize| {
                    let angle = -std::f32::consts::FRAC_PI_2
                        - std::f32::consts::TAU * drawn.ring * i as f32 / stretches as f32;
                    at(cx_ + radius * angle.cos(), cy + radius * angle.sin())
                };
                let mut path = PathBuilder::stroke(px(stroke * scale));
                path.move_to(point_at(0));
                for i in 1..=stretches {
                    path.line_to(point_at(i));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, ink);
                }
                let width = px(stroke * scale);
                for end in [point_at(0), point_at(stretches)] {
                    let cap = Bounds::centered_at(end, size(width, width));
                    window.paint_quad(fill(cap, ink).corner_radii(width / 2.));
                }
            }
            // The dot lands, growing about its middle, and takes the green.
            if drawn.dot > 0. {
                let (x, y, radius) = DOT_AT;
                let side = px(2. * radius * scale * drawn.dot);
                let dot = Bounds::centered_at(at(x, y), size(side, side));
                window.paint_quad(fill(dot, dot_color).corner_radii(side / 2.));
            }
        },
    )
    .w(px(BOX.2 * scale))
    .h(px(BOX.3 * scale))
}

/// `from` taking on `to`, `t` of the way.
fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let at = |a: f32, b: f32| a + (b - a) * t;
    Rgba {
        r: at(from.r, to.r),
        g: at(from.g, to.g),
        b: at(from.b, to.b),
        a: at(from.a, to.a),
    }
}

#[cfg(test)]
mod tests;
