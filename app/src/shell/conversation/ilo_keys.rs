//! The keyboard on an ilo line's buttons (#3955). Nothing presses one from the reply field or the
//! page, where someone typing may never have seen the line come: the user's Focus command (Focus
//! Agent Question's key, ⌥⇧⌘A on the Mac, Alt+Shift+A elsewhere) moves the keyboard to the newest
//! line that offers anything and rings its first button, and only then do ↩ and Space press the
//! ringed button, Tab and ⇧Tab move along the line's buttons, and Escape, or the key again, gives
//! the keyboard back where it was and presses nothing. A key that would press counts only PROTECT
//! after the keyboard came and after the key before, so one typed on or held from what was there
//! presses nothing.

use std::time::Instant;

use gpui::{App, Context, FocusHandle, Focusable, KeyBinding, Window, actions};

use super::TalkView;
use crate::agents::task::Line;
use crate::agents::task::ilo::{self, IloButton};
use crate::agents::tier::PROTECT;

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
            page,
            was: None,
            last: Instant::now(),
            due: true,
            leaving: None,
        });
        cx.notify();
        true
    }

    /// The keyboard went elsewhere, to the page or another window: the keys end where it went.
    pub(super) fn ilo_keys_left(&mut self, why: &'static str, cx: &mut Context<Self>) {
        if self.ilo_keys.take().is_some() {
            log!("browser: ilo keys left by {why}");
            cx.notify();
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
        let keyed = self.keyed_line(self.beside(cx));
        let Some(keys) = &mut self.ilo_keys else {
            return;
        };
        let why = match keyed {
            _ if keys.leaving.is_some() => keys.leaving.unwrap_or("key"),
            Some((line, _, buttons)) if line == keys.line => {
                keys.at = keys.at.min(buttons.len() - 1);
                let ringed = &self.ilo_stops[keys.at];
                if std::mem::take(&mut keys.due) {
                    keys.was = window.focused(cx);
                    window.focus(ringed, cx);
                } else if !self.ilo_stops.iter().any(|stop| stop.is_focused(window)) {
                    self.ilo_keys = None;
                    log!("browser: ilo keys left by focus");
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

    pub(super) fn ilo_press(&mut self, _: &IloPress, _: &mut Window, cx: &mut Context<Self>) {
        let (Some(at), Some((line, tab, buttons))) = (
            self.ilo_keys.as_ref().map(|keys| keys.at),
            self.keyed_line(self.beside(cx)),
        ) else {
            return;
        };
        if !self.ilo_key_counts() {
            return;
        }
        if let Some(&button) = buttons.get(at) {
            log!("browser: ilo key pressed {button:?}");
            self.press_ilo((line, button), tab, cx);
        }
    }

    fn ilo_move(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = (self.keyed_line(self.beside(cx))).map_or(0, |(_, _, buttons)| buttons.len());
        let Some(keys) = &mut self.ilo_keys else {
            return;
        };
        if count == 0 {
            return;
        }
        keys.at = (keys.at as isize + by).rem_euclid(count as isize) as usize;
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
    /// Focus Agent Question: the keyboard to the newest ilo line of the conversation shown, or with
    /// the keyboard on it already, back where it was. A held key's repeats do nothing, where the
    /// host can tell them, as the key would otherwise go back and forth.
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
        let beside = self.watched_task_page().map(|(_, tab)| tab);
        if view.update(cx, |view, cx| view.focus_ilo(page, beside, cx)) {
            self.take_keyboard(cx);
        }
    }

    /// Focus Agent Question's key from the page, kept from it only while a conversation shows:
    /// the page gets it back otherwise, and no other key reaches a line from the page. A held
    /// key's `repeat`s stay from the page and do nothing.
    pub fn run_ilo_page_key(
        &mut self,
        keystroke: &str,
        repeat: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        use super::super::{commands::Command, keys::user_keys};
        if self.talk.is_none()
            || user_keys(cx).command_for(keystroke, false) != Some(Command::FocusAgentQuestion)
        {
            return false;
        }
        if !repeat {
            self.run(Command::FocusAgentQuestion, "page", cx);
        }
        true
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
