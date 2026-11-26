//! The keyboard on an ilo line's buttons (#3955). Nothing presses one from the reply field or the
//! page, where someone typing may never have seen the line come: the user's Focus command (Focus
//! Agent Question's key, ⌥⇧⌘A on the Mac, Alt+Shift+A elsewhere) moves the keyboard to the newest
//! line that offers anything and rings its first button, and only then do ↩ and Space press the
//! ringed button, Tab and ⇧Tab move along the line's buttons, and Escape, or the key again, gives
//! the keyboard back where it was and presses nothing. A key that would press counts only PROTECT
//! after the keyboard came and after the key before, so one typed on or held from what was there
//! presses nothing. The keys are on a line only while the area draws its conversation: with a
//! page of the task's watched over it, or another view in its place, they end and the command
//! brings none, so no button is ringed, or pressed, out of sight.

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, KeyBinding, Pixels, Task, Window, actions,
};

use super::super::agent_bar::Bar;
use super::TalkView;
use crate::agents::task::Line;
use crate::agents::task::ilo::{self, IloButton};
use crate::agents::tier::PROTECT;
use crate::tokens::shell::conversation::ilo::ARM;

actions!(ilo_keys, [IloPress, IloNext, IloBack, IloLeave]);

/// The key context of the ringed button, whose bindings come before the window's.
pub(super) const CONTEXT: &str = "IloLine";

/// Most buttons a line has: Take Over and a step up, or Hand Back and Not Now.
pub(super) const MOST: usize = 3;

pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", IloPress, Some(CONTEXT)),
        KeyBinding::new("space", IloPress, Some(CONTEXT)),
        KeyBinding::new("tab", IloNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", IloBack, Some(CONTEXT)),
        KeyBinding::new("escape", IloLeave, Some(CONTEXT)),
    ]);
}

/// The keyboard on a line's buttons, while it is.
pub(super) struct IloKeys {
    /// The line's key.
    pub(super) line: usize,
    /// The ringed button, by its place on the line.
    pub(super) at: usize,
    /// The ringed button, which a press presses only while it's still at `at` on `line`.
    button: IloButton,
    /// Whether the page had the keyboard, which gets it back.
    page: bool,
    /// What had it in the UI, which gets it back.
    was: Option<FocusHandle>,
    /// When the keyboard came, or the last key that would press: the next counts PROTECT after.
    pub(super) last: Instant,
    /// Whether the ringed button is still to get the keyboard, at the next frame.
    due: bool,
    /// Why it's to go back, at the next frame: the key again, or the window left.
    leaving: Option<&'static str>,
    /// Under the checks only: what they were told of the guard on the ringed button, and the wait
    /// for its end.
    told: Told,
    wait: Option<Task<()>>,
}

/// What the checks read to press a line's button: the line's key, the button's own name and where
/// it stands. Nothing of what the line says, which may be a page's words or a site (#4198).
pub(super) fn place((line, button): (usize, IloButton), bounds: Bounds<Pixels>) -> String {
    format!(
        "browser: ilo line {line} {} at {:.0} {:.0} {:.0} {:.0}",
        super::words::button(button),
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    )
}

/// What a key on the ringed button waits out before it presses: `ilo_key_counts`' PROTECT after
/// the keyboard came or the key before, and for a button that widens, `press_ilo`'s moment its
/// place has stood. The checks are told when it ends, from these same instants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Guard {
    last: Instant,
    /// Only of a button that widens: since when it has stood where it is, none before it's drawn.
    stood: Option<Option<Instant>>,
}

impl Guard {
    /// `since` is when the button's place was last drawn anew.
    pub(super) fn of(last: Instant, button: IloButton, since: Option<Instant>) -> Self {
        Self {
            last,
            stood: button.widens().then_some(since),
        }
    }

    /// Whether a key `now` would press: what the press itself asks, of the same clock.
    fn passed(self, now: Instant) -> bool {
        now.saturating_duration_since(self.last) >= PROTECT
            && (self.stood).is_none_or(|stood| stood.is_some_and(|since| super::armed(since, now)))
    }

    /// When it ends, once the button is drawn.
    fn ends(self) -> Option<Instant> {
        let protect = self.last + PROTECT;
        match self.stood {
            None => Some(protect),
            Some(since) => Some(protect.max(since? + Duration::from_secs_f32(ARM))),
        }
    }
}

/// What the checks were told of a guard: nothing of a new one until it ends, and once of each.
#[derive(Default)]
pub(super) struct Told {
    of: Option<Guard>,
    ready: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum Tell {
    /// The guard has ended: the checks are told so, once.
    Ready,
    /// It ends then, to be asked again.
    Wait(Instant),
    Nothing,
}

impl Told {
    /// What to tell the checks of the `guard` as it is `now`, a frame after it was drawn or, with
    /// `woke`, at the end of the wait for it. A guard that started anew, as the line moved or a
    /// key came, is waited out anew.
    pub(super) fn step(&mut self, guard: Guard, now: Instant, woke: bool) -> Tell {
        let same = self.of == Some(guard);
        if same && self.ready {
            return Tell::Nothing;
        }
        self.of = Some(guard);
        self.ready = guard.passed(now);
        if self.ready {
            return Tell::Ready;
        }
        // Its wait is running.
        if same && !woke {
            return Tell::Nothing;
        }
        match guard.ends() {
            // A wait that ended a moment short of what the press asks is waited again.
            Some(end) => Tell::Wait(end.max(now + Duration::from_millis(1))),
            None => Tell::Nothing,
        }
    }
}

/// GPUI's focus left on a button the keys ended on (`ilo_keys_left`), which is no stop of the
/// window's from the next frame: where it goes then, a field of the UI's or the window itself.
pub(super) struct Left {
    to: Option<FocusHandle>,
}

impl TalkView {
    /// The newest of the conversation's ilo lines that offers anything, with what it offers, the
    /// page `beside` it or none.
    fn keyed_line(&self, beside: Option<u64>) -> Option<(usize, Option<u64>, Vec<IloButton>)> {
        (self.shown.as_ref()?.lines.iter().rev()).find_map(|line| match line {
            Line::Ilo(line) => {
                let buttons = ilo::drawn(line, beside);
                (!buttons.is_empty()).then(|| (line.key, ilo::tab(&line.kind), buttons))
            }
            _ => None,
        })
    }

    /// The Focus command: the keyboard to the newest line's first button, or with the keyboard on
    /// it already, back where it was. Whether there was a line to give it to. The shell, which
    /// asks while it's being updated, says which page is `beside` the conversation.
    pub(super) fn focus_ilo(
        &mut self,
        page: bool,
        beside: Option<u64>,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(keys) = &mut self.ilo_keys {
            keys.leaving = Some("key");
            cx.notify();
            return true;
        }
        let Some((line, _, buttons)) = self.keyed_line(beside) else {
            return false;
        };
        log!("browser: ilo keys on {line} {:?}", buttons[0]);
        self.ilo_keys = Some(IloKeys {
            line,
            at: 0,
            button: buttons[0],
            page,
            was: None,
            last: Instant::now(),
            due: true,
            leaving: None,
            told: Told::default(),
            wait: None,
        });
        cx.notify();
        true
    }

    /// The keyboard went elsewhere, to the page or another window: the keys end where it went.
    /// GPUI's focus is still on the button, and goes at the next frame where the key again would
    /// give the keyboard back, without the keyboard: to the window itself where the page has it,
    /// then or now, as a field of the UI's with the focus reads as typed in.
    pub(super) fn ilo_keys_left(&mut self, why: &'static str, cx: &mut Context<Self>) {
        let Some(keys) = self.ilo_keys.take() else {
            return;
        };
        log!("browser: ilo keys left by {why}");
        let field = !keys.page && why != "page";
        self.ilo_left = Some(Left {
            to: field.then(|| keys.was.unwrap_or_else(|| self.reply.focus_handle(cx))),
        });
        cx.notify();
    }

    /// The area no longer draws the conversation, for `why`: the keys end, and only the key
    /// brings them back. No frame of the conversation's follows, so the shell takes GPUI's focus
    /// from the button (`Shell::step_ilo_keys_drawn`).
    pub(super) fn ilo_keys_undrawn(&mut self, why: &'static str) {
        if self.ilo_keys.take().is_some() {
            log!("browser: ilo keys left by {why}");
        }
        self.ilo_left = None;
    }

    /// Whether GPUI's focus is on one of the buttons' handles.
    fn ilo_stop_focused(&self, window: &Window) -> bool {
        self.ilo_stops.iter().any(|stop| stop.is_focused(window))
    }

    /// GPUI's focus goes from a button the keys ended on (`ilo_keys_left`): a focus on a handle
    /// no frame draws hears no key of the window's, the Focus command's neither (#3869).
    fn ilo_focus_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(left) = self.ilo_left.take() else {
            return;
        };
        if !self.ilo_stop_focused(window) {
            return;
        }
        match (left.to, self.shell.upgrade()) {
            (Some(to), _) => window.focus(&to, cx),
            (None, Some(shell)) => window.focus(&shell.read(cx).focus_handle.clone(), cx),
            (None, None) => {}
        }
    }

    /// The window stopped being the active one: the keys end, and the keyboard goes back.
    pub(super) fn ilo_keys_leave(&mut self, why: &'static str, cx: &mut Context<Self>) {
        if let Some(keys) = &mut self.ilo_keys {
            keys.leaving = Some(why);
            cx.notify();
        }
    }

    /// The ringed button's place, if the keyboard is on `line`'s buttons.
    pub(super) fn ilo_ring(&self, line: usize) -> Option<usize> {
        (self.ilo_keys.as_ref())
            .filter(|keys| keys.line == line && keys.leaving.is_none())
            .map(|keys| keys.at)
    }

    /// Before the frame's drawn: gives the ringed button the keyboard, and gives it back where it
    /// came from once its line offers nothing more, or the key asked. Something else taking the
    /// keyboard, a click or a shortcut, ends the keys where it went.
    pub(super) fn step_ilo_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ilo_focus_left(window, cx);
        let keyed = self.keyed_line(self.beside(cx));
        let Some(keys) = &mut self.ilo_keys else {
            return;
        };
        let why = match keyed {
            _ if keys.leaving.is_some() => keys.leaving.unwrap_or("key"),
            Some((line, _, buttons)) if line == keys.line => {
                keys.at = keys.at.min(buttons.len() - 1);
                keys.button = buttons[keys.at];
                let ringed = &self.ilo_stops[keys.at];
                if std::mem::take(&mut keys.due) {
                    keys.was = window.focused(cx);
                    window.focus(ringed, cx);
                } else if !self.ilo_stops.iter().any(|stop| stop.is_focused(window)) {
                    self.ilo_keys = None;
                    log!("browser: ilo keys left by focus");
                }
                if crate::checked() {
                    // Once this frame has drawn the buttons where they stand.
                    cx.defer_in(window, |view, _, cx| view.tell_ilo_keys_ready(None, cx));
                }
                return;
            }
            _ => "line",
        };
        let Some(keys) = self.ilo_keys.take() else {
            return;
        };
        log!("browser: ilo keys back by {why}");
        if keys.page {
            if let Some(shell) = self.shell.upgrade() {
                shell.update(cx, |shell, cx| {
                    window.focus(&shell.focus_handle, cx);
                    shell.browser.focus_page(cx);
                });
            }
            return;
        }
        match keys.was {
            Some(was) => window.focus(&was, cx),
            None => window.focus(&self.reply.focus_handle(cx), cx),
        }
    }

    /// The guard on a key on the ringed button, as the last frame drew it.
    pub(super) fn ilo_guard(&self) -> Option<Guard> {
        let keys = self
            .ilo_keys
            .as_ref()
            .filter(|keys| keys.leaving.is_none())?;
        let drawn = self.ilo_drawn.borrow();
        let since = drawn
            .get(&(keys.line, keys.button))
            .map(|(_, since)| *since);
        Some(Guard::of(keys.last, keys.button, since))
    }

    /// Under the checks only, which wait on it rather than count: says when a key on the ringed
    /// button presses, at the guard's own end and never before, and again for a guard that
    /// started anew. `woke` is the guard whose wait ended.
    fn tell_ilo_keys_ready(&mut self, woke: Option<Guard>, cx: &mut Context<Self>) {
        let Some(guard) = self.ilo_guard() else {
            return;
        };
        let Some(keys) = &mut self.ilo_keys else {
            return;
        };
        match keys.told.step(guard, Instant::now(), woke == Some(guard)) {
            Tell::Ready => log!("browser: ilo keys ready on {}", keys.line),
            Tell::Wait(end) => {
                let wait = end.saturating_duration_since(Instant::now());
                keys.wait = Some(cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(wait).await;
                    (view.update(cx, |view, cx| view.tell_ilo_keys_ready(Some(guard), cx))).ok();
                }));
            }
            Tell::Nothing => {}
        }
    }

    /// Whether a key that would press comes PROTECT after the keyboard came and after the key
    /// before it.
    fn ilo_key_counts(&mut self) -> bool {
        let Some(keys) = &mut self.ilo_keys else {
            return false;
        };
        let now = Instant::now();
        let after = now.saturating_duration_since(keys.last);
        keys.last = now;
        if after < PROTECT {
            log!(
                "browser: ilo key ignored, {} ms after the last",
                after.as_millis()
            );
            return false;
        }
        true
    }

    /// The newest line offering anything, if it's still the one the keys are on, with the ringed
    /// button still at its place. A line that came since the ring was drawn ends the keys, as the
    /// user never saw its buttons ringed.
    fn ringed_line(&mut self, cx: &mut Context<Self>) -> Option<(Option<u64>, Vec<IloButton>)> {
        let keyed = self.keyed_line(self.beside(cx));
        let keys = self.ilo_keys.as_ref()?;
        match keyed {
            Some((line, tab, buttons))
                if line == keys.line && buttons.get(keys.at) == Some(&keys.button) =>
            {
                Some((tab, buttons))
            }
            _ => {
                self.ilo_keys_leave("line", cx);
                None
            }
        }
    }

    pub(super) fn ilo_press(&mut self, _: &IloPress, _: &mut Window, cx: &mut Context<Self>) {
        let Some((tab, _)) = self.ringed_line(cx) else {
            return;
        };
        if !self.ilo_key_counts() {
            return;
        }
        let Some(keys) = &self.ilo_keys else {
            return;
        };
        let (line, button) = (keys.line, keys.button);
        log!("browser: ilo key pressed {button:?}");
        self.press_ilo((line, button), tab, cx);
    }

    fn ilo_move(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, buttons)) = self.ringed_line(cx) else {
            return;
        };
        let Some(keys) = &mut self.ilo_keys else {
            return;
        };
        keys.at = (keys.at as isize + by).rem_euclid(buttons.len() as isize) as usize;
        keys.button = buttons[keys.at];
        keys.last = Instant::now();
        window.focus(&self.ilo_stops[keys.at], cx);
        cx.notify();
    }

    pub(super) fn ilo_next(&mut self, _: &IloNext, window: &mut Window, cx: &mut Context<Self>) {
        self.ilo_move(1, window, cx);
    }

    pub(super) fn ilo_back(&mut self, _: &IloBack, window: &mut Window, cx: &mut Context<Self>) {
        self.ilo_move(-1, window, cx);
    }

    pub(super) fn ilo_leave(&mut self, _: &IloLeave, _: &mut Window, cx: &mut Context<Self>) {
        self.ilo_keys_leave("escape", cx);
    }
}

impl super::super::Shell {
    /// Whether the area draws the conversation kept: its task's is the view there, with no page
    /// of the task's over it, or with one it stands beside.
    fn talk_drawn(&self) -> bool {
        (self.talk.as_ref()).is_some_and(|talk| self.task_open() == Some(talk.space))
            && (self.task_page_drawn().is_none() || self.task_standing())
    }

    /// Focus Agent Question: the keyboard to the newest ilo line of the conversation the area
    /// draws, or with the keyboard on it already, back where it was. A held key's repeats do
    /// nothing, where the host can tell them, as the key would otherwise go back and forth.
    ///
    /// Beside a page watched it keys the panel's line, and with no line asked there, the panel's
    /// field. Where the panel has neither, an outside app's thread with nothing asked, and with a
    /// page over the conversation, full screen or on its way back, nothing is in sight to take
    /// the keyboard: while the window has it the key goes back to the conversation and rings
    /// nothing, so the key is always a way back from the page; a page the user has, held from its
    /// agent or with the keyboard, keeps the view.
    pub(in crate::shell) fn focus_agent_question(&mut self, from: &str, cx: &mut Context<Self>) {
        if from == "ui" && self.browser.key_repeated() {
            return;
        }
        let Some(view) = self.talk.as_ref().map(|talk| talk.view.clone()) else {
            return;
        };
        // From the command bar or a menu the page may have had the keyboard, and the UI takes it
        // either way, so the ring is never on a button the keys miss.
        let page = from == "page"
            || (self.browser.page_has_keyboard(cx)).unwrap_or(self.page_keyboard.is_some());
        if !self.talk_drawn() {
            self.focus_key_leaves_page(page, cx);
            return;
        }
        let beside = self.watched_task_page().map(|(_, tab)| tab);
        if view.update(cx, |view, cx| view.focus_ilo(page, beside, cx)) {
            self.take_keyboard(cx);
            return;
        }
        // Nothing is asked. With the conversation alone the key has nothing to do; beside a page
        // it takes the panel's field, and the page is left where the panel shows none.
        if beside.is_none() {
            return;
        }
        if view.read(cx).takes_reply() {
            self.take_keyboard(cx);
            self.focus_talk = true;
            cx.notify();
        } else {
            self.focus_key_leaves_page(page, cx);
        }
    }

    /// The key with nothing of the conversation's in sight to take the keyboard: the page watched
    /// goes back into its picture, unless the user has it, held from its agent or with the
    /// keyboard in it (`page`).
    fn focus_key_leaves_page(&mut self, page: bool, cx: &mut Context<Self>) {
        let held = matches!(self.agent_bar(), Some(Bar::Held { .. }));
        if let Some((_, tab)) = self.watched_task_page().filter(|_| !page && !held) {
            log!("browser: focus key leaves task page {tab}");
            self.task_page_back(cx);
        }
    }

    /// Focus Agent Question's key from the page, kept from it only while the area draws a
    /// conversation, beside the page: the page gets it back otherwise, and no other key reaches
    /// a line from the page. A held key's `repeat`s stay from the page and do nothing.
    pub fn run_ilo_page_key(
        &mut self,
        keystroke: &str,
        repeat: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        use super::super::{commands::Command, keys::user_keys};
        if !self.talk_drawn()
            || user_keys(cx).command_for(keystroke, false) != Some(Command::FocusAgentQuestion)
        {
            return false;
        }
        if !repeat {
            self.run(Command::FocusAgentQuestion, "page", cx);
        }
        true
    }

    /// Before the frame's drawn: the keys on a line's buttons end once the area no longer draws
    /// their conversation, as a page of the task's is watched over it or another view takes its
    /// place, and GPUI's focus leaves the button for the window, as it leaves the reply field
    /// there (`focus_talk`). Nothing keys the line again but the command.
    pub(in crate::shell) fn step_ilo_keys_drawn(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(talk) = (self.talk.as_ref()).filter(|_| !self.talk_drawn()) else {
            return;
        };
        let view = talk.view.read(cx);
        let stray = view.ilo_stop_focused(window);
        if view.ilo_keys.is_none() && view.ilo_left.is_none() && !stray {
            return;
        }
        let why = match self.task_open() == Some(talk.space) {
            true => "watch",
            false => "area",
        };
        (talk.view.clone()).update(cx, |view, _| view.ilo_keys_undrawn(why));
        if stray {
            window.focus(&self.focus_handle, cx);
        }
    }

    /// The page took the keyboard, which GPUI's focus doesn't follow: the line's keys end there.
    pub fn leave_ilo_keys(&mut self, cx: &mut Context<Self>) {
        self.leave_ilo_keys_by("page", cx);
    }

    /// GPUI says the window stopped being active. Left for another app's, the keys end and the
    /// keyboard goes back where it was, so coming back the user asks again with the key; left for
    /// one of the app's own windows, as the command bar or a menu over it, they end where they
    /// are, as given back now the page would take the keyboard from under the overlay.
    pub(in crate::shell) fn ilo_keys_window_inactive(&mut self, cx: &mut Context<Self>) {
        match self.browser.app_active() {
            true => self.leave_ilo_keys_by("window", cx),
            false => self.ilo_keys_window_left("window", cx),
        }
    }

    fn leave_ilo_keys_by(&mut self, why: &'static str, cx: &mut Context<Self>) {
        if let Some(talk) = &self.talk {
            talk.view.update(cx, |view, cx| view.ilo_keys_left(why, cx));
        }
    }

    /// The window stopped being the active one for another app's: the keys end and the keyboard
    /// goes back where it was, so coming back the user asks again with the key.
    pub(in crate::shell) fn ilo_keys_window_left(
        &mut self,
        why: &'static str,
        cx: &mut Context<Self>,
    ) {
        if let Some(talk) = &self.talk {
            talk.view
                .update(cx, |view, cx| view.ilo_keys_leave(why, cx));
        }
    }
}
