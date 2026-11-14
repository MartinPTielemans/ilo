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
use std::time::Duration;
use web_time::Instant;

use gpui::{
    AnyElement, Bounds, Context, Entity, KeystrokeEvent, MouseButton, PathBuilder, Pixels, Rgba,
    SharedString, Subscription, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
};

use super::{
    Command, Shell, cubic_bezier,
    display_face::{cap_top, display_text},
    fade_at,
};
use crate::account::{self, Account, AccountEvent, Status};
use crate::default_browser::{self, Os};
use crate::import::{Corner, Guide, ImportEvent, ImportView, Look};
use crate::settings_rows::{SLIDE, group, switch};
use crate::theme::Theme;
use crate::theme_picker::SWATCHES;
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
use crate::tokens::shell::onboarding::TAGLINE_GAP;
use crate::tokens::shell::onboarding::TAGLINE_LINE;
use crate::tokens::shell::onboarding::TAGLINE_SIZE;
use crate::tokens::shell::onboarding::TITLE_LINE;
use crate::tokens::shell::onboarding::TITLE_SIZE;
use crate::tokens::shell::onboarding::WORDMARK_SCALE;

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
    /// The first Space's colour, from the theme picker's swatches.
    Color,
    /// Signing in to sync: Settings › Sync's view (crate::account); left out in a build that names
    /// no sync server, and once the browser is signed in.
    Account,
    /// Settings' opt-in switches, both off; left out when neither is offered.
    OptIns,
}

const STEPS: [Step; 7] = [
    Step::Intro,
    Step::Import,
    Step::Favorites,
    Step::DefaultBrowser,
    Step::Color,
    Step::Account,
    Step::OptIns,
];

/// The opt-ins the first launch offers, each Settings' own switch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum OptIn {
    CrashReports,
    Agents,
}

impl OptIn {
    const ALL: [OptIn; 2] = [OptIn::CrashReports, OptIn::Agents];

    /// What check.sh calls it.
    fn name(self) -> &'static str {
        match self {
            OptIn::CrashReports => "crash-reports",
            OptIn::Agents => "agents",
        }
    }

    /// Its switch's name and its one sentence: the name Settings' row has, as it's that switch.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            OptIn::CrashReports => (
                "Send crash reports",
                concat!(
                    "When ",
                    env!("APP_NAME"),
                    " crashes, it sends a report of what went wrong, so it can be fixed."
                ),
            ),
            // Settings' own line while it's off.
            OptIn::Agents => (
                "Let agents use the browser",
                "They work in a Space of their own, which you can watch, and never see your \
                 passwords, cookies or autofill.",
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
            Step::Color => "color",
            Step::Account => "account",
            Step::OptIns => "opt-ins",
        }
    }

    /// The step's one message on `os`: its title, its line and its button's words.
    fn message(self, os: Os) -> (&'static str, gpui::SharedString, &'static str) {
        let (title, line, button) = match self {
            Step::Intro => ("", "", ""),
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
            Step::Color => (
                "Pick a colour for your first Space.",
                "The window takes it. Each Space can have its own, from its name in the sidebar.",
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
/// The intro holds this long from its first frame, then the first step comes.
const INTRO_END: f32 = 3.4;
/// With Reduce Motion, the wordmark and its line fade in whole together, and hold.
const STILL_FADE: f32 = 0.4;
const STILL_END: f32 = 2.;
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
    /// The system it runs on, for the default browser's step.
    os: Os,
    /// The opt-in switch last turned, and when, for its knob's slide.
    switched: Option<(OptIn, Instant)>,
    /// Where the opt-ins' rows and switches are drawn (opt_in_frame).
    frames: Frames,
    /// Where the step's button is drawn, once the step has settled (button_frame).
    pub(super) button: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// When the step came, which its motion runs from.
    pub(super) since: Instant,
    /// The step before, leaving as this one comes, and how far into it it was when it left.
    leaving: Option<(Step, f32)>,
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
    /// Wakes the window at the intro's end, which holds still until then.
    hold: Option<gpui::Task<()>>,
    /// The account step's view, while the step is offered.
    account: Option<Signing>,
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
        let opt_ins = !self.opt_ins(cx).is_empty();
        let account = self.signing(cx);
        let steps: Vec<Step> = STEPS
            .into_iter()
            .filter(|&step| match step {
                Step::Import => import,
                // An import begun brings it in (import_event).
                Step::Favorites => false,
                Step::DefaultBrowser => !default_browser,
                Step::Account => account.is_some(),
                Step::OptIns => opt_ins,
                _ => true,
            })
            .collect();
        let names: Vec<&str> = steps.iter().map(|step| step.name()).collect();
        // check.sh waits for these lines.
        log!("browser: onboarding started");
        log!("browser: onboarding steps {}", names.join(" "));
        log!("browser: onboarding step intro");
        self.onboarding = Some(Onboarding {
            steps,
            step: Step::Intro,
            picked: None,
            os: self.browser.os(),
            switched: None,
            frames: Frames::default(),
            button: Rc::default(),
            since: Instant::now(),
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
            hold: None,
            account,
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
        let Some(onboarding) = self.onboarding.as_mut().filter(|o| o.ending.is_none()) else {
            return;
        };
        let step = onboarding.step;
        log!("browser: onboarding left {} via {how}", step.name());
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
        let at = onboarding.steps.iter().position(|&each| each == step);
        match at.and_then(|at| onboarding.steps.get(at + 1)).copied() {
            Some(next) => {
                let elapsed = onboarding.since.elapsed().as_secs_f32();
                onboarding.leaving = Some((step, elapsed));
                onboarding.step = next;
                onboarding.since = Instant::now();
                onboarding.settled = false;
                log!("browser: onboarding step {}", next.name());
            }
            None => onboarding.ending = Some(Instant::now()),
        }
        cx.notify();
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
            OptIn::CrashReports => browser.set_crash_reports(!on, cx),
            OptIn::Agents => browser.set_agents(!on, cx),
        });
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.switched = Some((opt_in, Instant::now()));
        }
        cx.notify();
    }

    /// Picks swatch `index` at the colour step.
    fn onboarding_pick(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.picked = Some(index.min(SWATCHES.len() - 1));
            cx.notify();
        }
    }

    /// A key in the window: any skips the intro, and in a step Esc skips it and return is its
    /// button; at the colour step the arrows go along the swatches. A shortcut still does its own
    /// thing first.
    fn onboarding_key(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((step, picked)) = self.onboarding.as_ref().map(|o| (o.step, o.picked)) else {
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
            (Step::Intro, _) => self.onboarding_next("key", cx),
            (Step::Import, "escape") => self.import_escape("escape", cx),
            (_, "escape") => self.onboarding_next("escape", cx),
            // From none picked, → starts at the first and ← at the last.
            (Step::Color, "right") => self.onboarding_pick(picked.map_or(0, |at| at + 1), cx),
            (Step::Color, "left") => {
                self.onboarding_pick(picked.map_or(SWATCHES.len(), |at| at.max(1)) - 1, cx)
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
            (_, "enter") if !key.modifiers.modified() => {
                self.onboarding_continue("return", window, cx)
            }
            _ => {}
        }
    }

    /// The flow over the whole window while it shows, stepping its motion; none otherwise, at no
    /// cost but the check.
    pub(super) fn onboarding_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let onboarding = self.onboarding.as_mut()?;
        // A page shown covers it, so it has ended.
        let ended = onboarding
            .ending
            .is_some_and(|at| at.elapsed().as_secs_f32() >= END_FADE);
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
            let browser = self.browser.clone();
            cx.defer(move |cx| browser.focus_ui(cx));
        }
        let still = cx.reduce_motion();
        let elapsed = onboarding.since.elapsed().as_secs_f32();
        let step = onboarding.step;
        let came = match step {
            Step::Intro => Drawn::at(elapsed, still).done(),
            _ => elapsed >= STEP_MOTION,
        };
        if came && !std::mem::replace(&mut onboarding.settled, true) {
            // check.sh waits for this line before it captures the step.
            log!("browser: onboarding step {} settled", step.name());
        }
        if step == Step::Intro && elapsed >= if still { STILL_END } else { INTRO_END } {
            self.onboarding_next("time", cx);
            return self.onboarding_layer(window, cx);
        }
        let onboarding = self.onboarding.as_mut()?;
        let motion = (elapsed / STEP_MOTION).min(1.);
        if motion >= 1. {
            onboarding.leaving = None;
        }
        // Frames while something moves; the intro's hold waits on a timer instead.
        let ending = onboarding.ending.map(|at| at.elapsed().as_secs_f32());
        let sliding = onboarding
            .switched
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < SLIDE);
        if !came || onboarding.leaving.is_some() || ending.is_some() || sliding {
            window.request_animation_frame();
        } else if step == Step::Intro && onboarding.hold.is_none() {
            let end = if still { STILL_END } else { INTRO_END };
            let left = Duration::from_secs_f32((end - elapsed).max(0.));
            onboarding.hold = Some(cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(left).await;
                // A timer can end a little before the flow's clock does, as Windows' do (#1026),
                // so the frame it wakes waits again for what's left.
                shell
                    .update(cx, |shell, cx| {
                        if let Some(onboarding) = shell.onboarding.as_mut() {
                            onboarding.hold = None;
                        }
                        cx.notify()
                    })
                    .ok();
            }));
        }
        let light = onboarding.light.unwrap_or_else(|| {
            matches!(
                window.appearance(),
                gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight
            )
        });
        let leaving = onboarding.leaving;
        let colors = Colors::of(light);
        // The step comes from the right as the one before leaves to the left, but for a fade with
        // Reduce Motion. The intro, first, is simply there.
        let eased = ease_out(motion);
        let shift = if still || leaving.is_none() {
            0.
        } else {
            SHIFT
        };
        let coming = self
            .step_content(step, elapsed, still, colors, window, cx)
            .left(px(shift * (1. - eased)))
            .opacity(if leaving.is_some() { eased } else { 1. });
        let gone = leaving.map(|(step, at)| {
            self.step_content(step, at, still, colors, window, cx)
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
        let skip = (step != Step::Intro && !self.keeping_words(cx) && corner != Corner::None)
            .then(|| self.skip_button(corner == Corner::Back, colors, cx));
        // The import under way, or what it brought, quiet at the bottom left once its step is left.
        let import_line = (step != Step::Import && step != Step::Intro)
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
        Some(
            div()
                .id("onboarding")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(colors.ground)
                .occlude()
                .opacity(opacity)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        if shell
                            .onboarding
                            .as_ref()
                            .is_some_and(|o| o.step == Step::Intro)
                        {
                            shell.onboarding_next("click", cx);
                        }
                    }),
                )
                .children(gone)
                .child(coming)
                .children(skip)
                .children(import_line)
                .into_any_element(),
        )
    }

    /// A step's message, centred in the window, `t` seconds into it.
    fn step_content(
        &self,
        step: Step,
        t: f32,
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
        if step == Step::Intro {
            let drawn = Drawn::at(t, still);
            return layer.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(wordmark(drawn, colors))
                    .child(
                        display_text("a good tool.", TAGLINE_SIZE, TAGLINE_LINE, cx)
                            .mt(px(TAGLINE_GAP - cap_top(TAGLINE_SIZE, TAGLINE_LINE)))
                            .text_color(colors.ink(QUIET))
                            .opacity(drawn.tagline),
                    ),
            );
        }
        let os = self
            .onboarding
            .as_ref()
            .map_or(Os::Mac, |onboarding| onboarding.os);
        let (title, line, button) = step.message(os);
        let switches = (step == Step::OptIns).then(|| self.opt_in_rows(colors, cx));
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
            Step::Account
                if account
                    .as_ref()
                    .is_none_or(|view| view.read(cx).state().status != Status::SignedOut) =>
            {
                ""
            }
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
        let swatches = (step == Step::Color).then(|| {
            let picked = self
                .onboarding
                .as_ref()
                .and_then(|onboarding| onboarding.picked);
            swatch_row(picked, colors, cx)
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
        let button = (!button.is_empty()).then(|| {
            div()
                .id("onboarding-continue")
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
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.onboarding_continue("button", window, cx)
                }))
                .child(button)
                .child(chip(
                    super::key_name("enter", super::OFF_THE_MAC),
                    with_alpha(PAPER, CHIP),
                ))
                .children(drawn.map(|(at, settled)| button_frame(at, settled)))
        });
        layer.child(
            div()
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
                .children(switches)
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
        let switched = onboarding.switched;
        // Where they're drawn is kept once the step has come, not while it slides in.
        let settled = onboarding.step == Step::OptIns && onboarding.settled;
        let frames = onboarding.frames.clone();
        // The import step's boxes, the ink a little over the ground.
        let surface = colors.ground.blend(colors.ink(IMPORT_SURFACE));
        let look = Look::on(surface, colors.ink(1.), rgb(SIGNAL));
        let rows = self.opt_ins(cx).into_iter().map(|(opt_in, on)| {
            let slid = match switched {
                Some((turned, at)) if turned == opt_in => {
                    (at.elapsed().as_secs_f32() / SLIDE).min(1.)
                }
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
                .child(switch(&look, on, slid, look.accent))
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
        div()
            .id("onboarding-skip")
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
fn swatch_row(
    picked: Option<usize>,
    colors: Colors,
    cx: &mut Context<Shell>,
) -> impl IntoElement + use<> {
    // Each swatch's ring and the gap inside it, on both sides.
    let ringed = SWATCH + 4. * SWATCH_RING;
    div()
        .mt(px(SWATCH_GAP))
        .flex()
        .gap(px(SWATCH_PITCH - ringed))
        .children(SWATCHES.iter().enumerate().map(|(index, &color)| {
            let ring = if picked == Some(index) {
                colors.ink(1.)
            } else {
                colors.ink(0.)
            };
            div()
                .id(("onboarding-swatch", index))
                .p(px(SWATCH_RING))
                .rounded_full()
                .border(px(SWATCH_RING))
                .border_color(ring)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| shell.onboarding_pick(index, cx)))
                .child(
                    div()
                        .size(px(SWATCH))
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

/// The wordmark, `drawn` as far as it is, at the flow's size.
fn wordmark(drawn: Drawn, colors: Colors) -> impl IntoElement {
    let scale = WORDMARK_SCALE;
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
