//! The keys of an agent's question card (#3869). Nothing answers a card from the page, where
//! someone typing may never have seen it arrive: Focus Agent Question's key (⌥⇧⌘A on the Mac,
//! Alt+Shift+A elsewhere, Chrome's Focus Inactive Popup) moves the keyboard to the card and rings
//! its primary button, and only then do ↩ and Space press the ringed button, Tab and ⇧Tab move
//! along the buttons and "See the whole request", and Escape answers as the card's × does. The key
//! again gives the keyboard back where it was and answers nothing, as the window stopping being
//! the active one does. Only the user's keys come here: an agent's go to its page (keys.rs's
//! tab_key_before_page).

use std::time::{Duration, Instant};

use gpui::accesskit::{Live, Role};
use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, FontWeight, Hsla, KeyBinding, Rgba, Stateful, Task,
    Window, actions, canvas, div, prelude::*, px, svg,
};

use super::TITLE;
use super::agent_question::{Button, Does, buttons};
use super::permission::{Look, card_button_as, card_close};
use crate::tokens::dialog::{
    CHIP_HEIGHT, CHIP_RADIUS, ESC_FILL, ESC_TEXT, RETURN_BORDER, RETURN_FILL, RETURN_GLYPH,
    RETURN_WIDTH,
};
use crate::tokens::shell::permission::{BUTTON_GAP, CLOSE, CLOSE_INSET, LINE, PADDING};
use crate::tokens::text;

/// Between a button's label and its chips.
const CHIP_GAP: f32 = 6.;
/// The ESC chip's width, its text in 6.5 from each side at the small size.
const ESC_WIDTH: f32 = 36.;
/// Room a row of buttons keeps spare before taking a chip, against rounding in its layout.
const ROOM_SLACK: f32 = 0.5;
/// Each side of a button with a chip, half a card button's, as its label and chip sit centred: at
/// the sidebar's narrowest, Allow Once has its row to itself and room for ↩ only so.
const CHIPPED_SIDE: f32 = BUTTON_GAP / 2.;

/// What card_chips and card_key_close read from the last frame: the width of the card's row of
/// buttons and where it ends, each button's top and its label's width, and where the card's top
/// line ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct CardRoom {
    row: f32,
    row_end: f32,
    top_end: f32,
    buttons: [(f32, f32); MOST_BUTTONS],
}

impl CardRoom {
    /// Whether ESC fits in the ×'s place beside the top line: the head and title keep room for
    /// the × only, and a short line leaves room for ESC too.
    fn esc_beside_top(&self) -> bool {
        let esc_start = self.row_end + PADDING - CLOSE_INSET - ESC_WIDTH;
        self.row_end > 0. && self.top_end + CHIP_GAP <= esc_start - ROOM_SLACK
    }
}
use super::commands::{key_name, keystroke_keys};
use super::{Shell, commands::Command, keys::user_keys};
use crate::agents::{Answer, Question};

/// How long after the keyboard reaches a card, and after the key before, a key that would answer
/// is ignored. Chrome's prompts ignore input inside the double-click interval for the same reason
/// (InputEventActivationProtector): a key meant for what was there, typed on or held down, lands
/// on a button that has just appeared. 500 ms is the Mac's default interval; the system's isn't
/// read.
pub(super) const PROTECT: Duration = Duration::from_millis(500);

/// The card's key context, whose bindings come before the window's while a button has the
/// keyboard.
const CONTEXT: &str = "AgentCard";

/// Most buttons a card has: Never, Allow Once and Always Allow.
pub(super) const MOST_BUTTONS: usize = 3;

actions!(agent_card_keys, [CardPress, CardNext, CardBack, CardLeave]);

pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", CardPress, Some(CONTEXT)),
        KeyBinding::new("space", CardPress, Some(CONTEXT)),
        KeyBinding::new("tab", CardNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", CardBack, Some(CONTEXT)),
        KeyBinding::new("escape", CardLeave, Some(CONTEXT)),
    ]);
}

/// Where the keyboard was before it came to the card, where it goes back.
#[derive(Clone)]
enum Came {
    Page,
    /// The UI, and what of it had the keyboard, read as it leaves.
    Ui(Option<FocusHandle>),
}

/// The keyboard on a card's buttons.
pub(super) struct CardKeys {
    question: u64,
    /// The card's buttons' labels as the keyboard came, so a card changing them under its id
    /// rings its primary again.
    labels: Vec<&'static str>,
    /// The tab shown as the keyboard came: another one shown takes the keyboard to its page.
    selected: u64,
    /// The ringed stop: a button, by its place in `buttons`, or "See the whole request" as the
    /// place after the last (`stops`).
    at: usize,
    came: Came,
    /// When the keyboard came, then the last key that would have answered or moved the ring,
    /// whether it did or not.
    last: Instant,
    /// The frame still has to give the ringed stop the keyboard.
    due: bool,
    /// The frame still has to give the keyboard back, answering nothing, and why.
    leaving: Option<&'static str>,
    /// Says when `PROTECT` has passed since `last`, for the checks to wait on rather than count
    /// time themselves.
    _ready: Option<Task<()>>,
}

/// Where a card shows Escape's chip while it has the keyboard: on the button whose answer
/// Escape's is, or in the ×'s place when it's the ×'s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EscapeMark {
    Close,
    Button(usize),
}

/// What Escape does on `question`'s card: the ×'s Dismiss, which keeps nothing and lets the agent
/// ask again; on a card asking for a tab back, Keep. A handoff has neither, as both its buttons
/// change something, so there it gives the keyboard back.
fn escape_does(question: &Question) -> Option<Does> {
    match (question.handoff, question.back) {
        (None, _) => Some(Does::Answer(Answer::Dismiss)),
        (Some(_), true) => Some(Does::Answer(Answer::Never)),
        (Some(_), false) => None,
    }
}

/// The button the keyboard rings first on `question`'s card, its primary: Always Allow, which
/// reaches further than it looks, never is.
fn primary(question: &Question) -> usize {
    (buttons(question).iter())
        .position(|button| button.look == Look::Primary)
        .unwrap_or(0)
}

/// Where `question`'s card marks Escape, if Escape answers it.
pub(super) fn escape_mark(question: &Question) -> Option<EscapeMark> {
    let does = escape_does(question)?;
    Some(
        match buttons(question).iter().position(|b| b.does == does) {
            Some(at) => EscapeMark::Button(at),
            None => EscapeMark::Close,
        },
    )
}

/// The key `keystroke` in words, as a screen reader should say it: Option Shift Command A on the
/// Mac, Alt Shift A elsewhere, Control for ⌃ and Ctrl, with no glyphs or "+" to be read aloud.
fn spoken(keystroke: &str) -> String {
    let words = |key: &str| match key {
        "⌃" | "Ctrl" => "Control".to_string(),
        "⌥" => "Option".into(),
        "⇧" => "Shift".into(),
        "⌘" => "Command".into(),
        key => SPOKEN_KEYS
            .iter()
            .find(|&&name| key_name(name, false) == key)
            .map_or(key.to_string(), |&name| key_name(name, true)),
    };
    (keystroke_keys(keystroke).iter())
        .map(|key| words(key))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Keys the Mac names by a symbol, whose word key_name gives off the Mac.
const SPOKEN_KEYS: [&str; 13] = [
    "enter",
    "escape",
    "up",
    "down",
    "left",
    "right",
    "tab",
    "backspace",
    "delete",
    "pageup",
    "pagedown",
    "home",
    "end",
];

/// A card's title as an assistive app hears it: ending in a full stop, so the description after it
/// doesn't run on from its last word.
pub(super) fn heard_title(title: &str) -> String {
    match title.trim_end().ends_with(['.', '?', '!']) {
        true => title.to_string(),
        false => format!("{}.", title.trim_end()),
    }
}

/// The card's key `A`, which `does` while a card's stop has the keyboard by Focus Agent
/// Question, and is passed on otherwise. Passed on too once the keys are leaving, before the frame
/// that gives the keyboard back: a window left for another app and hidden draws none, and a key as
/// the user comes back would otherwise answer.
fn card_key<A: gpui::Action>(
    does: fn(&mut Shell, &mut Window, &mut Context<Shell>),
    cx: &Context<Shell>,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |shell, _: &A, window, cx| {
        let held = shell
            .card_keys
            .as_ref()
            .is_some_and(|keys| keys.leaving.is_none());
        match held && shell.card_focused(window).is_some() {
            true => does(shell, window, cx),
            false => cx.propagate(),
        }
    })
}

/// The card's stops in the order they're drawn, so Tab's: "See the whole request" on a request's
/// card, as `count`, the place after the last button, then the buttons.
fn stops(question: &Question) -> Vec<usize> {
    let count = buttons(question).len();
    let see = (!question.changes.is_empty()).then_some(count);
    see.into_iter().chain(0..count).collect()
}

/// Under the checks, once `PROTECT` has passed, says so while the keyboard is still on the card.
/// Dropped, as the next key replaces it, it says nothing.
fn ready(cx: &mut Context<Shell>) -> Option<Task<()>> {
    crate::checked().then(|| {
        cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(PROTECT).await;
            shell
                .update(cx, |shell, _| {
                    if let Some(keys) = &shell.card_keys {
                        // check.sh waits for this line.
                        log!("browser: agent card keys ready on {}", keys.question);
                    }
                })
                .ok();
        })
    })
}

impl Shell {
    /// Focus Agent Question: the keyboard to the card shown, its primary button ringed, or with
    /// the keyboard on it already, back where it was. A held key's repeats do nothing, where the
    /// host can tell them, as the key would otherwise go back and forth.
    pub(super) fn focus_agent_question(&mut self, from: &str, cx: &mut Context<Self>) {
        if from == "ui" && self.browser.key_repeated() {
            return;
        }
        self.focus_agent_question_from(from == "page", cx)
    }

    fn focus_agent_question_from(&mut self, from_page: bool, cx: &mut Context<Self>) {
        match &mut self.card_keys {
            // From the page, the page had the keyboard, so the card's keys had ended with nothing
            // telling the shell: the key gives the card the keyboard again.
            Some(_) if from_page => self.card_keys = None,
            Some(keys) => {
                keys.leaving = Some("key");
                return cx.notify();
            }
            None => {}
        }
        let Some(question) = self.agent_question() else {
            return;
        };
        let list = buttons(question);
        let at = primary(question);
        let id = question.id;
        let labels = list.iter().map(|button| button.label).collect();
        // check.sh waits for this line.
        log!("browser: agent card keys on {id} {}", list[at].label);
        // From the command bar or a menu the page may have had the keyboard, as before Focus
        // Sidebar, and the UI takes it either way, so the ring is never on a card the keys miss.
        let page = from_page
            || (self.browser.page_has_keyboard(cx)).unwrap_or(self.page_keyboard.is_some());
        self.take_keyboard(cx);
        self.card_keys = Some(CardKeys {
            question: id,
            labels,
            selected: self.selected,
            at,
            came: match page {
                true => Came::Page,
                false => Came::Ui(None),
            },
            last: cx.background_executor().now(),
            due: true,
            leaving: None,
            _ready: ready(cx),
        });
        cx.notify();
    }

    /// Focus Agent Question's key from the page, kept from it only while a card shows: the page
    /// gets it back once none does, and no other key reaches the card from the page.
    /// A held key's `repeat`s stay from the page and do nothing.
    pub fn run_agent_card_page_key(
        &mut self,
        keystroke: &str,
        repeat: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.agent_question().is_none()
            || user_keys(cx).command_for(keystroke, false) != Some(Command::FocusAgentQuestion)
        {
            return false;
        }
        if !repeat {
            self.run(Command::FocusAgentQuestion, "page", cx);
        }
        true
    }

    /// The window stopped being the active one for another app's: the card's keys end and the
    /// keyboard goes back where it was, so coming back the user asks again with the key.
    /// On the Mac and Linux GPUI says so (`why` "window"); on Windows, where its window inside the
    /// frame hears no activation, the host's word that the keyboard left for another app does
    /// (keyboard_left, "app").
    pub(super) fn card_keys_window_left(&mut self, why: &'static str, cx: &mut Context<Self>) {
        if let Some(keys) = &mut self.card_keys {
            keys.leaving = Some(why);
            cx.notify();
        }
    }

    /// GPUI says the window stopped being active. Decided once the events that came with it are
    /// handled, not at a frame, which a hidden window may never draw: still inactive then, the
    /// window was left. Sway's leave and enter as a keyboard comes or its keymap changes come in
    /// one flush, which one read takes and one dispatch handles, so the window is active again by
    /// then and the keys stay. Left for one of the app's own windows, as the command bar, a menu or
    /// Peek over it, the keys end where they are and the keyboard stays that window's: given back
    /// now, the page would take it from under the overlay.
    pub(super) fn card_keys_window_inactive(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.card_keys.is_none() {
            return;
        }
        cx.spawn_in(window, async move |shell, cx| {
            shell
                .update_in(cx, |shell, window, cx| {
                    if window.is_window_active() {
                        return;
                    }
                    if !shell.browser.app_active() {
                        return shell.card_keys_window_left("window", cx);
                    }
                    if shell.card_keys.take().is_some() {
                        log!("browser: agent card keys left by window");
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }

    /// The page has the keyboard, which GPUI's focus doesn't follow, so the card's keys have
    /// ended: the key from the page gives the card the keyboard again rather than back.
    pub fn leave_card_keys(&mut self, cx: &mut Context<Self>) {
        if self.card_keys.take().is_some() {
            log!("browser: agent card keys left by page");
            cx.notify();
        }
    }

    /// The button the keyboard rings on the card, if it's there.
    pub(super) fn agent_card_ring(&self) -> Option<usize> {
        self.card_keys.as_ref().map(|keys| keys.at)
    }

    /// What a card says to an assistive app as it arrives, after its title: the key that moves the
    /// keyboard to it, as it's bound now. None if it's bound to none.
    /// What a screen reader says as a card arrives, wherever its cursor is: the card's name and
    /// the key's line. A dialog nobody moved to isn't read, by NVDA at least, so the card says it
    /// as the command bar and toasts do, through a live region, drawing nothing: the Mac's
    /// AccessKit announces a region's value, and Windows' and Linux's its name (#3236).
    pub(super) fn card_announcer(heard: &str, described: Option<&str>) -> impl IntoElement {
        let said = match described {
            Some(line) => format!("{heard} {line}"),
            None => heard.to_owned(),
        };
        div()
            .id("agent-question-said")
            .absolute()
            .size_0()
            .role(Role::Status)
            .aria_label(said.clone())
            .aria_value(said)
            .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
    }

    pub(super) fn agent_card_described(&self, cx: &App) -> Option<String> {
        let keys = user_keys(cx).shown_keys(Command::FocusAgentQuestion);
        let key = keys.first()?.first()?;
        Some(format!("Press {} to answer.", spoken(key)))
    }

    /// Before the frame's drawn: gives the ringed stop the keyboard, and gives it back where it
    /// came from once the card's answered, gone or covered, or the key asked, or the window's no
    /// longer the active one. Something else taking the keyboard, a click, a shortcut or the page,
    /// ends the keys where they went; an assistive app moving it to another stop moves the ring.
    pub(super) fn step_card_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let showing = self.agent_question().map(|q| {
            let labels: Vec<_> = buttons(q).iter().map(|button| button.label).collect();
            (q.id, labels, stops(q), primary(q))
        });
        let focused = self.card_focused(window);
        let on_buttons = self.agent_card_buttons.iter().any(|h| h.is_focused(window));
        // The page has the keyboard, which GPUI's focus doesn't follow: the shell's one judge of
        // that, the core's word on Linux, else the host's that the page took it, or another tab
        // shown, whose page takes it as it shows.
        let page = self.page_has_keyboard(window, cx);
        let selected = self.selected;
        let now = cx.background_executor().now();
        let Some(keys) = &mut self.card_keys else {
            // A click gives a button the keyboard too. With the card gone no drawn element holds
            // it, and GPUI matches a key's bindings from the focused element, so the window takes
            // it, as from "See the whole request" (#3715).
            if on_buttons && showing.is_none() {
                window.focus(&self.focus_handle, cx);
            }
            return;
        };
        let why = match showing {
            _ if keys.leaving.is_some() => keys.leaving.unwrap_or("key"),
            _ if !keys.due && (page || keys.selected != selected) => {
                self.card_keys = None;
                log!("browser: agent card keys left by page");
                return cx.notify();
            }
            Some((id, labels, stops, first)) if id == keys.question => {
                // The card changed its buttons under its id: its primary is ringed again, after
                // the protection, as for a card just come.
                if labels != keys.labels {
                    keys.labels = labels;
                    keys.at = first;
                    keys.due = true;
                    keys.last = now;
                    keys._ready = ready(cx);
                    log!("browser: agent card keys on {id} {}", keys.labels[first]);
                }
                if !stops.contains(&keys.at) {
                    keys.at = first;
                    keys.due = true;
                }
                if std::mem::take(&mut keys.due) {
                    if let Came::Ui(None) = keys.came {
                        keys.came = Came::Ui(window.focused(cx).filter(|focused| {
                            !self.agent_card_buttons.iter().any(|h| h == focused)
                                && *focused != self.agent_request_see
                        }));
                    }
                    let at = keys.at;
                    let to = self.card_stop(at);
                    window.focus(&to, cx);
                } else if let Some(at) = focused.filter(|&at| at != keys.at) {
                    self.card_ring_to(at, false, cx);
                } else if focused.is_none() {
                    self.card_keys = None;
                    log!("browser: agent card keys left by focus");
                }
                return;
            }
            _ => "card",
        };
        let Some(keys) = self.card_keys.take() else {
            return;
        };
        // check.sh waits for this line.
        log!("browser: agent card keys back by {why}");
        // The card gone, its stops are still focused until the keyboard goes back.
        if !on_buttons && !self.agent_request_see.is_focused(window) && !keys.due {
            return;
        }
        match keys.came {
            Came::Page => {
                // No drawn element holds the keyboard as the page takes it (#3715).
                window.focus(&self.focus_handle, cx);
                self.browser.focus_page(cx);
            }
            // What had the keyboard may have gone with the card's frame, as a field closed under
            // it: the window has it then, as with nothing before.
            Came::Ui(Some(was)) if self.focus_handle.contains(&was, window) => {
                window.focus(&was, cx)
            }
            Came::Ui(_) => window.focus(&self.focus_handle, cx),
        }
    }

    /// Whether a key that would answer comes long enough after the keyboard reached the card, and
    /// after the key before it: a key held down, or typed on from what was there, doesn't answer.
    fn card_key_counts(&mut self, cx: &mut Context<Self>) -> bool {
        let now = cx.background_executor().now();
        let task = ready(cx);
        let Some(keys) = &mut self.card_keys else {
            return false;
        };
        let after = now.saturating_duration_since(keys.last);
        keys.last = now;
        keys._ready = task;
        if after < PROTECT {
            // check.sh waits for this line.
            log!(
                "browser: agent card key ignored, {} ms after the last",
                after.as_millis()
            );
            return false;
        }
        true
    }

    /// ↩ or Space: presses the ringed stop, past the protection. Where the keyboard is on
    /// another stop than the ringed one, as an assistive app's focus moves GPUI's without the
    /// ring, it presses nothing: the ring moves to it, and the protection starts over, so the key
    /// never answers a button other than the one the user hears.
    fn card_press(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.card_focused(window);
        if let Some(at) = focused.filter(|&at| Some(at) != self.agent_card_ring()) {
            log!("browser: agent card key pressed nothing, the keyboard on another stop");
            return self.card_ring_to(at, false, cx);
        }
        if !self.card_key_counts(cx) {
            return;
        }
        let Some(at) = self.agent_card_ring() else {
            return;
        };
        let Some(question) = self.agent_question() else {
            return;
        };
        let Some(button) = buttons(question).get(at).copied() else {
            if stops(question).contains(&at) {
                log!("browser: agent card key pressed See the whole request");
                self.see_whole_request(window, cx);
            }
            return;
        };
        log!("browser: agent card key pressed {}", button.label);
        self.press_card(button.does, cx);
    }

    fn card_leave(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if !self.card_key_counts(cx) {
            return;
        }
        match self.agent_question().map(escape_does) {
            Some(Some(does)) => {
                log!("browser: agent card key escape");
                self.press_card(does, cx);
            }
            Some(None) => self.focus_agent_question_from(false, cx),
            None => {}
        }
    }

    /// Tab and ⇧Tab: the ring to the next stop or the one before, round from the last to the
    /// first.
    fn card_move(&mut self, by: isize, _: &mut Window, cx: &mut Context<Self>) {
        let Some(stops) = self.agent_question().map(stops) else {
            return;
        };
        let Some(at) = self.agent_card_ring() else {
            return;
        };
        let from = stops.iter().position(|&stop| stop == at).unwrap_or(0) as isize;
        let to = stops[(from + by).rem_euclid(stops.len() as isize) as usize];
        self.card_ring_to(to, true, cx);
    }

    /// The ring to the stop `at`, which the frame gives the keyboard if `due`, or which has it. A
    /// key that would answer is ignored inside the protection after it, as after the keyboard
    /// came: a key typed on as the ring moved would press a button the user hasn't heard.
    fn card_ring_to(&mut self, at: usize, due: bool, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let task = ready(cx);
        let Some(keys) = &mut self.card_keys else {
            return;
        };
        keys.at = at;
        keys.due = due;
        keys.last = now;
        keys._ready = task;
        let id = keys.question;
        let label = (self.agent_question())
            .and_then(|q| buttons(q).get(at).map(|button| button.label))
            .unwrap_or("See the whole request");
        // check.sh waits for this line.
        log!("browser: agent card keys on {id} {label}");
        cx.notify();
    }

    /// The handle of the card's stop `at`.
    fn card_stop(&self, at: usize) -> FocusHandle {
        match self.agent_card_buttons.get(at) {
            Some(button) if self.agent_question().map_or(0, |q| buttons(q).len()) > at => {
                button.clone()
            }
            _ => self.agent_request_see.clone(),
        }
    }

    /// Which of the key chips `at`'s button shows, ↩ and ESC: those its row of the last frame had
    /// room for beside every label whole, ↩ first. So the chips never wrap a label or a row, and
    /// the card keeps its height as the keyboard comes and goes; where there's no room, at the
    /// narrowest sidebar, the ring alone marks the button.
    fn card_chips(&self, at: usize, count: usize, escape: Option<EscapeMark>) -> (bool, bool) {
        let Some(ringed) = self.agent_card_ring() else {
            return (false, false);
        };
        let room = self.agent_card_room.get();
        let row: Vec<usize> = (0..count.min(MOST_BUTTONS))
            .filter(|&i| room.buttons[i].0 == room.buttons[at].0)
            .collect();
        let mut used = (row.iter())
            .map(|&i| room.buttons[i].1 + 2. * BUTTON_GAP)
            .sum::<f32>()
            + row.len().saturating_sub(1) as f32 * BUTTON_GAP;
        let escaped = match escape {
            Some(EscapeMark::Button(i)) => Some(i),
            _ => None,
        };
        let mut chipped = [false; MOST_BUTTONS];
        let mut fits = |on: Option<usize>, chip: f32| {
            let Some(i) = on.filter(|i| row.contains(i)) else {
                return false;
            };
            let sides = if chipped[i] {
                0.
            } else {
                2. * (BUTTON_GAP - CHIPPED_SIDE)
            };
            let fits = used + chip + CHIP_GAP - sides <= room.row - ROOM_SLACK;
            if fits {
                used += chip + CHIP_GAP - sides;
                chipped[i] = true;
            }
            fits && i == at
        };
        (fits(Some(ringed), RETURN_WIDTH), fits(escaped, ESC_WIDTH))
    }

    /// Measures the row of a card's buttons, for card_chips. Its child, under the buttons.
    pub(super) fn card_room_probe(&self) -> impl IntoElement {
        let room = self.agent_card_room.clone();
        canvas(
            move |bounds, _, _| {
                let mut all = room.get();
                all.row = f32::from(bounds.size.width);
                all.row_end = f32::from(bounds.right());
                room.set(all);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    }

    /// `words`, the card's top line, drawn as wide as they are, so card_key_close knows where
    /// they end.
    pub(super) fn card_top_line(&self, words: impl IntoElement) -> Div {
        let room = self.agent_card_room.clone();
        let probe = canvas(
            move |bounds, _, _| {
                let mut all = room.get();
                all.top_end = f32::from(bounds.right());
                room.set(all);
            },
            |_, _, _, _| {},
        );
        div().flex().child(
            div()
                .relative()
                .min_w_0()
                .child(words)
                .child(probe.absolute().size_full()),
        )
    }

    /// The card's button `button`, `at` of `count` in its row, with the keyboard's marks while
    /// the card has it: the ring, drawn outside the button as paint, and the delete dialog's chips
    /// after its label where card_chips finds them room, ↩ on the ringed button and ESC on
    /// Escape's.
    pub(super) fn card_key_button(
        &self,
        at: usize,
        count: usize,
        button: Button,
        escape: Option<EscapeMark>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let ringed = self.agent_card_ring();
        let palette = &self.palette;
        let ink: Hsla = match button.look {
            Look::Primary => palette.accent_label.into(),
            _ => palette.text(TITLE),
        };
        let (enter, esc) = self.card_chips(at, count, escape);
        let chip = || {
            div()
                .flex_none()
                .h(px(CHIP_HEIGHT))
                .my(px(-((CHIP_HEIGHT - LINE).max(0.)) / 2.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CHIP_RADIUS))
        };
        let enter = enter.then(|| {
            chip()
                .w(px(RETURN_WIDTH))
                .border_1()
                .border_color(ink.opacity(RETURN_BORDER))
                .bg(ink.opacity(RETURN_FILL))
                .child(
                    svg()
                        .path("icons/corner-down-left.svg")
                        .size(px(RETURN_GLYPH))
                        .text_color(ink),
                )
        });
        let esc = esc.then(|| self.esc_chip(chip()));
        let chipped = enter.is_some() || esc.is_some();
        let room = self.agent_card_room.clone();
        let words = div()
            .relative()
            .when(chipped, |words| words.whitespace_nowrap())
            .child(button.label)
            .child(
                canvas(
                    move |bounds, _, _| {
                        let mut all = room.get();
                        all.buttons[at].1 = f32::from(bounds.size.width);
                        room.set(all);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        let room = self.agent_card_room.clone();
        let row = canvas(
            move |bounds, _, _| {
                let mut all = room.get();
                all.buttons[at].0 = f32::from(bounds.origin.y);
                room.set(all);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let label = div()
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .child(words)
            .children(enter)
            .children(esc);
        let accent: Rgba = palette.accent.into();
        card_button_as(palette, button.id, label, button.look)
            .when(chipped, |b| b.px(px(CHIPPED_SIDE)))
            .relative()
            .child(row)
            .when(ringed == Some(at), |mut b| {
                let style = std::mem::take(Styled::style(&mut b));
                *Styled::style(&mut b) = crate::settings_rows::ring_round(accent)(style);
                b
            })
            .track_focus(&self.agent_card_buttons[at])
            .when(self.a11y, |control| {
                crate::settings_rows::named_button(control, button.label)
            })
            .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                // GPUI clicks a focused element on ↩ or Space let go; a key answers a button only
                // through the card's keys, past the protection. An assistive app's press is a
                // pointer's.
                if !event.is_keyboard() {
                    shell.press_card(button.does, cx)
                }
            }))
    }

    /// The ESC chip, on `chip`.
    fn esc_chip(&self, chip: Div) -> Div {
        chip.w(px(ESC_WIDTH))
            .bg(self.palette.plate(ESC_FILL))
            .text_size(px(text::SMALL))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.palette.on_overlay(ESC_TEXT))
            .child("ESC")
    }

    /// The card's × or, while the keyboard's on the card and Escape is the ×'s, ESC in its place
    /// where the top line leaves it room, pressed as the × is. Where it doesn't, the × stays and
    /// Escape still answers as it does.
    pub(super) fn card_key_close(
        &self,
        escape: Option<EscapeMark>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let esc = self.agent_card_ring().is_some()
            && escape == Some(EscapeMark::Close)
            && self.agent_card_room.get().esc_beside_top();
        let close = match esc {
            true => div()
                .id("agent-question-close")
                .absolute()
                .top(px(CLOSE_INSET))
                .right(px(CLOSE_INSET))
                .h(px(CLOSE))
                .flex()
                .items_center()
                .child(
                    self.esc_chip(
                        div()
                            .h(px(CHIP_HEIGHT))
                            .flex()
                            .items_center()
                            .rounded(px(CHIP_RADIUS)),
                    ),
                ),
            false => card_close(&self.palette, "agent-question-close"),
        };
        close
            .when(self.a11y, |close| {
                crate::settings_rows::named_button(close, "Close")
            })
            .on_click(cx.listener(|shell, event: &ClickEvent, _, cx| {
                if !event.is_keyboard() {
                    shell.press_card(Does::Answer(Answer::Dismiss), cx)
                }
            }))
    }

    /// The card's element taking its keys while one of its buttons has them through Focus Agent
    /// Question. Otherwise, as with "See the whole request" or a button a press gave the
    /// keyboard, its keys are what they'd be without the card's: a key answers a button only
    /// through these.
    pub(super) fn card_keys_on<E: gpui::InteractiveElement + FluentBuilder>(
        &self,
        card: E,
        cx: &Context<Self>,
    ) -> E {
        card.when(self.card_keys.is_some(), |card| {
            card.key_context(CONTEXT)
                .on_action(card_key::<CardPress>(Shell::card_press, cx))
                .on_action(card_key::<CardNext>(
                    |shell, window, cx| shell.card_move(1, window, cx),
                    cx,
                ))
                .on_action(card_key::<CardBack>(
                    |shell, window, cx| shell.card_move(-1, window, cx),
                    cx,
                ))
                .on_action(card_key::<CardLeave>(Shell::card_leave, cx))
        })
    }

    /// The card's stop with GPUI's keyboard, as `CardKeys::at` counts them.
    fn card_focused(&self, window: &Window) -> Option<usize> {
        let question = self.agent_question()?;
        (stops(question).into_iter()).find(|&at| self.card_stop(at).is_focused(window))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use super::PROTECT;
    use crate::agents::request::Request;
    use crate::agents::{Answer, Question};
    use crate::shell::{Folders, Shell, commands::Command, recorder::Recorder};
    use crate::tokens::shell::permission::{BUTTON_HEIGHT, PADDING};

    const KEY: &str = "alt-shift-cmd-a";

    fn question(id: u64) -> Question {
        Question {
            id,
            agent: "Claude Code".into(),
            from: "claude-code".into(),
            what: "click “Send” on mail.example".into(),
            asks: "click “Send”".into(),
            site: "mail.example".into(),
            why: "It sends a message.".into(),
            ..Question::default()
        }
    }

    fn request(id: u64) -> Question {
        let change = Request {
            method: "PUT".into(),
            url: "https://team.example.com/api/note".into(),
            path: "/api/note".into(),
            headers: Vec::new(),
            body: String::new(),
            what: "Replace the note.".into(),
        };
        Question {
            what: "make a change on team.example.com".into(),
            why: "Changes ask every time.".into(),
            site: "team.example.com".into(),
            changes: vec![change].into(),
            ..question(id)
        }
    }

    /// Every card the agents ask with: a confirmation, a fenced site's, a payment's, a request's,
    /// a handoff and a tab asked back, each with its buttons' labels.
    fn cards() -> Vec<(&'static str, Question, Vec<&'static str>)> {
        vec![
            (
                "confirmation",
                question(1),
                vec!["Never", "Allow Once", "Always Allow"],
            ),
            (
                "fenced",
                Question {
                    fenced: true,
                    ..question(2)
                },
                vec!["Never", "Allow Once"],
            ),
            (
                "each time",
                Question {
                    each_time: true,
                    ..question(3)
                },
                vec!["Deny", "Allow"],
            ),
            ("request", request(4), vec!["Deny", "Allow"]),
            (
                "handoff",
                Question {
                    handoff: Some(10),
                    ..question(5)
                },
                vec!["Hand Back", "Show Tab"],
            ),
            (
                "tab back",
                Question {
                    handoff: Some(10),
                    back: true,
                    ..question(6)
                },
                vec!["Keep", "Hand Back"],
            ),
        ]
    }

    fn window_with(
        cx: &mut TestAppContext,
    ) -> (Rc<Recorder>, Entity<Shell>, &mut VisualTestContext) {
        cx.update(crate::shell::commands::bind_keys);
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
        });
        cx.run_until_parked();
        (host, shell, cx)
    }

    fn ask(shell: &Entity<Shell>, cx: &mut VisualTestContext, question: Question) {
        shell.update(cx, |shell, cx| shell.set_agent_question(Some(question), cx));
        cx.run_until_parked();
    }

    /// The key as the page hands it over, before the page could keep it.
    fn from_page(shell: &Entity<Shell>, cx: &mut VisualTestContext, key: &str) -> bool {
        let kept = shell.update(cx, |shell, cx| {
            shell.run_agent_card_page_key(key, false, cx)
        });
        cx.run_until_parked();
        kept
    }

    /// The card's button with GPUI's keyboard, by its place in the card's buttons.
    fn keyed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<usize> {
        let handles = shell.read_with(cx, |shell, _| shell.agent_card_buttons.clone());
        cx.update(|window, _| handles.iter().position(|h| h.is_focused(window)))
    }

    /// The card's stop with GPUI's keyboard, a button by its place or "See the whole request"
    /// as the place after the last.
    fn stop(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<usize> {
        let shell = shell.clone();
        cx.update(|window, cx| shell.read(cx).card_focused(window))
    }

    fn ring(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<usize> {
        shell.read_with(cx, |shell, _| shell.agent_card_ring())
    }

    fn answers(host: &Recorder) -> Vec<String> {
        (host.take().into_iter())
            .filter(|did| did.starts_with("answer agent"))
            .collect()
    }

    fn wait(cx: &mut VisualTestContext, time: Duration) {
        cx.executor().advance_clock(time);
        cx.run_until_parked();
    }

    /// The key from the page gives the card the keyboard on its primary button and answers
    /// nothing; again, with the keyboard on the card, it gives the keyboard back to the page and
    /// still answers nothing.
    #[gpui::test]
    fn the_key_moves_the_keyboard_and_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        assert!(from_page(&shell, cx, KEY), "the page kept the key");
        let did = host.take();
        assert!(did.contains(&"focus ui".to_string()), "{did:?}");
        assert!(
            !did.iter().any(|d| d.starts_with("answer agent")),
            "{did:?}"
        );
        assert_eq!(keyed(&shell, cx), Some(1), "Allow Once hasn't the keyboard");

        // The key again reaches the card through the window's bindings, as the card has it.
        wait(cx, PROTECT);
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        let did = host.take();
        assert!(did.contains(&"focus page".to_string()), "{did:?}");
        assert!(
            !did.iter().any(|d| d.starts_with("answer agent")),
            "{did:?}"
        );
        assert_eq!(keyed(&shell, cx), None);
        let asked = shell.read_with(cx, |shell, _| shell.agent_question().map(|q| q.id));
        assert_eq!(asked, Some(7), "the card went unanswered");
    }

    /// From the UI the key gives the keyboard back to what had it there, not to the page.
    #[gpui::test]
    fn the_key_again_gives_the_ui_its_keyboard_back(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let root = shell.read_with(cx, |shell, _| shell.focus_handle.clone());
        cx.update(|window, cx| window.focus(&root, cx));
        ask(&shell, cx, question(7));
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        assert_eq!(keyed(&shell, cx), Some(1));
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        cx.update(|window, _| assert!(root.is_focused(window), "the keyboard went elsewhere"));
        let did = host.take();
        assert!(!did.contains(&"focus page".to_string()), "{did:?}");
        assert!(
            !did.iter().any(|d| d.starts_with("answer agent")),
            "{did:?}"
        );
    }

    /// ↩ inside 500 ms of the keyboard reaching the card answers nothing, as a ↩ typed in a form
    /// as the card came; one after it answers what a click on the ringed button does, and the
    /// keyboard goes back to the page.
    #[gpui::test]
    fn enter_straight_after_the_key_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        host.take();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        wait(cx, PROTECT - Duration::from_millis(1));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), Vec::<String>::new());
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let did = host.take();
        // As the click on Allow Once logs it (the_card_asks_and_its_buttons_answer).
        assert!(did.contains(&"answer agent 7 Once".to_string()), "{did:?}");
        assert!(did.contains(&"focus page".to_string()), "{did:?}");
        assert_eq!(keyed(&shell, cx), None);
    }

    /// A held ↩ repeats far inside 500 ms, so however long it's held it answers nothing.
    #[gpui::test]
    fn a_held_enter_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        for _ in 0..90 {
            cx.simulate_keystrokes("enter");
            wait(cx, Duration::from_millis(33));
        }
        cx.simulate_keystrokes("space");
        cx.run_until_parked();
        assert_eq!(answers(&host), Vec::<String>::new());
        assert_eq!(keyed(&shell, cx), Some(1), "the keyboard left the card");
    }

    /// ↩ from the page with a card showing is the page's, as is the key with no card; and ⌃⇧A,
    /// the key's way of being written off the Mac, is no key on it, where it selects to a field's
    /// start.
    #[gpui::test]
    fn only_the_key_comes_from_the_page_and_only_with_a_card(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        assert!(!from_page(&shell, cx, KEY), "the key was kept with no card");
        ask(&shell, cx, question(7));
        for key in ["enter", "space", "escape", "tab", "ctrl-shift-a"] {
            assert!(!from_page(&shell, cx, key), "{key} was kept from the page");
        }
        let did = host.take();
        let moved = |d: &&String| d.starts_with("focus") || d.starts_with("answer agent");
        assert_eq!(did.iter().find(moved), None, "{did:?}");
        assert_eq!(keyed(&shell, cx), None);
        let available = shell.read_with(cx, |shell, _| shell.offers(Command::FocusAgentQuestion));
        assert!(available, "the command bar wouldn't list it with a card");
    }

    /// The page taking the keyboard ends the card's keys, so the key from the page gives the card
    /// the keyboard again rather than back.
    #[gpui::test]
    fn after_a_click_in_the_page_the_key_comes_to_the_card_again(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        shell.update(cx, |shell, cx| shell.leave_card_keys(cx));
        host.take();
        from_page(&shell, cx, KEY);
        let did = host.take();
        assert!(did.contains(&"focus ui".to_string()), "{did:?}");
        assert!(!did.contains(&"focus page".to_string()), "{did:?}");
        assert_eq!(keyed(&shell, cx), Some(1));
    }

    /// On every card the keyboard lands on the primary button, never Always Allow; Tab and ⇧Tab
    /// move along all the stops as they're drawn, "See the whole request" on a request's card too,
    /// round from the last to the first; and ↩ presses the one ringed as a click does.
    #[gpui::test]
    fn every_cards_buttons_by_tab(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        for (card, asked, labels) in cards() {
            let id = asked.id;
            let shown: Vec<_> = (super::buttons(&asked).iter()).map(|b| b.label).collect();
            assert_eq!(shown, labels, "{card}");
            ask(&shell, cx, asked);
            from_page(&shell, cx, KEY);
            let primary = labels.len() - if labels.len() == 3 { 2 } else { 1 };
            assert_eq!(
                keyed(&shell, cx),
                Some(primary),
                "{card} rang another first"
            );
            assert_ne!(labels[primary], "Always Allow");
            let order = match card {
                // "See the whole request" is drawn above the buttons, as the place after them.
                "request" => vec![labels.len(), 0, 1],
                _ => (0..labels.len()).collect(),
            };
            let mut place = order.iter().position(|&at| at == primary).unwrap();
            for _ in 0..order.len() {
                cx.simulate_keystrokes("tab");
                cx.run_until_parked();
                place = (place + 1) % order.len();
                assert_eq!(stop(&shell, cx), Some(order[place]), "{card} after tab");
            }
            cx.simulate_keystrokes("shift-tab");
            cx.run_until_parked();
            place = (place + order.len() - 1) % order.len();
            let at = order[place];
            assert_eq!(stop(&shell, cx), Some(at), "{card} after shift-tab");
            host.take();
            wait(cx, PROTECT);
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            let did = host.take();
            let pressed = match (card, labels[at]) {
                (_, "Show Tab") => "select 10".to_string(),
                (_, "Hand Back") => "hand back 10".to_string(),
                ("tab back", "Keep") => format!("answer agent {id} Never"),
                (_, "Never") => format!("answer agent {id} Never"),
                (_, "Deny") => format!("answer agent {id} Dismiss"),
                (_, label) => panic!("{card}: {label} rang after shift-tab"),
            };
            assert!(did.contains(&pressed), "{card}: {did:?}");
            shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
            cx.run_until_parked();
            host.take();
        }
    }

    /// Escape answers as the card leaves things: Dismiss, the ×'s, which lets the agent ask
    /// again; Keep on a tab asked back. A handoff's buttons both change something, so there it
    /// gives the keyboard back and answers nothing.
    #[gpui::test]
    fn escape_leaves_things_as_they_are(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        for (card, asked, _) in cards() {
            let id = asked.id;
            ask(&shell, cx, asked);
            from_page(&shell, cx, KEY);
            host.take();
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            assert_eq!(answers(&host), Vec::<String>::new(), "{card} inside 500 ms");
            wait(cx, PROTECT);
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            let did = host.take();
            let answered = match card {
                "handoff" => None,
                "tab back" => Some(Answer::Never),
                _ => Some(Answer::Dismiss),
            };
            let said: Vec<_> = (did.iter())
                .filter(|d| d.starts_with("answer agent"))
                .collect();
            match answered {
                Some(answer) => assert_eq!(said, [&format!("answer agent {id} {answer:?}")]),
                None => assert!(said.is_empty(), "{card}: {did:?}"),
            }
            assert!(did.contains(&"focus page".to_string()), "{card}: {did:?}");
            assert_eq!(keyed(&shell, cx), None, "{card}");
            shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
            cx.run_until_parked();
            host.take();
        }
    }

    /// A card that goes unanswered by the key, as the agent's client going, gives the keyboard
    /// back to the page.
    #[gpui::test]
    fn a_card_gone_gives_the_keyboard_back(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        host.take();
        shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
        cx.run_until_parked();
        assert!(host.take().contains(&"focus page".to_string()));
        assert_eq!(keyed(&shell, cx), None);
    }

    /// With the keyboard on the card, the window's shortcuts work, as ⌘T opening a new tab.
    #[gpui::test]
    fn window_shortcuts_work_from_the_card(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        host.bar_open.set(false);
        cx.simulate_keystrokes("cmd-t");
        cx.run_until_parked();
        assert!(host.bar_open.take(), "⌘T from the card opened no new tab");
    }

    /// A click on the ringed button answers as the key does, the same line in the log.
    #[gpui::test]
    fn a_click_answers_as_the_key_does(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
        let card = card.expect("the card wasn't drawn");
        let above = card.bottom() - px(PADDING + BUTTON_HEIGHT * 1.5 + 8.);
        host.take();
        cx.simulate_click(
            point(card.left() + card.size.width * 0.75, above),
            Modifiers::none(),
        );
        cx.run_until_parked();
        let clicked = answers(&host);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), clicked);
        assert_eq!(clicked, ["answer agent 7 Once"]);
    }

    /// An agent's ask for a file rings Choose File…, and ↩ on it opens the dialog over the card's
    /// window as its click does: the key and the click take one path, which has every button's.
    #[gpui::test]
    fn enter_on_choose_file_chooses_as_the_click_does(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let upload = Question {
            what: "add a file to “Résumé” on example.com".into(),
            site: "example.com".into(),
            each_time: true,
            file: Some(crate::agents::FileAsk {
                field: 3,
                many: false,
            }),
            ..question(7)
        };
        ask(&shell, cx, upload);
        from_page(&shell, cx, KEY);
        assert_eq!(
            keyed(&shell, cx),
            Some(1),
            "Choose File… hasn't the keyboard"
        );
        wait(cx, PROTECT);
        host.take();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let window = shell.read_with(cx, |shell, _| shell.browser.window());
        let did = host.take();
        assert!(
            did.contains(&format!("choose agent file 7 in {window}")),
            "{did:?}"
        );
        assert!(
            !did.iter().any(|d| d.starts_with("answer agent")),
            "{did:?}"
        );
    }

    /// The keyboard's marks are paint: the card keeps its size as it comes and as the ring and ↩
    /// move along, and at the narrowest sidebar every button, Never beside Allow Once's ↩ too,
    /// stays whole inside the card, as tall as it was.
    #[gpui::test]
    fn the_keyboards_marks_change_no_cards_size_or_label(cx: &mut TestAppContext) {
        use crate::tokens::shell::{SIDEBAR_MIN, SIDEBAR_WIDTH};
        use gpui::accesskit::Role;
        let (_, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        for width in [SIDEBAR_WIDTH, SIDEBAR_MIN] {
            shell.update(cx, |shell, cx| {
                shell.sidebar_width = width;
                cx.notify();
            });
            for (card, asked, labels) in cards() {
                ask(&shell, cx, asked);
                // The card, and each button's left, top, right and bottom, in the card's order.
                // The tree's bounds are the screen's pixels.
                let scale = f64::from(cx.update(|window, _| window.scale_factor()));
                let drawn = |cx: &mut VisualTestContext| {
                    let plate = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
                    let tree = cx.a11y_tree().expect("GPUI built a tree");
                    let buttons: Vec<[f64; 4]> = (labels.iter())
                        .map(|label| {
                            let (_, node) = (tree.nodes.iter())
                                .find(|(_, n)| n.role() == Role::Button && n.label() == Some(label))
                                .unwrap_or_else(|| panic!("{card}: no {label} button"));
                            let r = node.bounds().expect("a button with no bounds");
                            [r.x0, r.y0, r.x1, r.y1].map(|at| at / scale)
                        })
                        .collect();
                    (plate.expect("card not drawn"), buttons)
                };
                let (before, was) = drawn(cx);
                from_page(&shell, cx, KEY);
                for _ in 0..labels.len() {
                    let (now, buttons) = drawn(cx);
                    assert_eq!(
                        now.size, before.size,
                        "{card} at {width}: the card changed size"
                    );
                    let (left, right) = (
                        f64::from(f32::from(now.left()) + PADDING) - 0.5,
                        f64::from(f32::from(now.right()) - PADDING) + 0.5,
                    );
                    for (at, (b, label)) in buttons.iter().zip(&labels).enumerate() {
                        assert_eq!(
                            b[3] - b[1],
                            was[at][3] - was[at][1],
                            "{card} at {width}: {label} grew"
                        );
                        assert!(
                            b[0] >= left && b[2] <= right,
                            "{card} at {width}: {label} {b:?} runs past the card {now:?}"
                        );
                        if let Some(next) = buttons.get(at + 1).filter(|n| n[1] == b[1]) {
                            assert!(
                                b[2] <= next[0],
                                "{card} at {width}: {label} overlaps the next"
                            );
                        }
                    }
                    cx.simulate_keystrokes("tab");
                    cx.run_until_parked();
                }
                shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
                cx.run_until_parked();
            }
        }
    }

    /// ESC sits where Escape's answer is while the card has the keyboard: in the ×'s place, on
    /// Deny where it's the Dismiss, on Keep, and nowhere on a handoff.
    #[gpui::test]
    fn esc_marks_escapes_answer(cx: &mut TestAppContext) {
        use super::{EscapeMark, escape_mark};
        let marks: Vec<_> = (cards().into_iter())
            .map(|(card, asked, _)| (card, escape_mark(&asked)))
            .collect();
        assert_eq!(
            marks,
            [
                ("confirmation", Some(EscapeMark::Close)),
                ("fenced", Some(EscapeMark::Close)),
                ("each time", Some(EscapeMark::Button(0))),
                ("request", Some(EscapeMark::Button(0))),
                ("handoff", None),
                ("tab back", Some(EscapeMark::Button(0))),
            ]
        );
        let _ = cx;
    }

    /// A card arriving is an alert named by its title, ending in a full stop, and described by the
    /// key that moves the keyboard to it in words, as bound now: another key changes the line and
    /// no key drops it.
    #[gpui::test]
    fn the_card_says_the_key_bound_now(cx: &mut TestAppContext) {
        use gpui::accesskit::{Live, Role};
        let (_, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        ask(&shell, cx, question(7));
        // The alert's name and description, and that its live region says both as it arrives.
        let heard = |cx: &mut VisualTestContext| {
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let (_, alert) = (tree.nodes.iter())
                .find(|(_, node)| node.role() == Role::AlertDialog)
                .expect("the card isn't an alert");
            let (label, described) = (
                alert.label().unwrap_or("").to_string(),
                alert.description().map(str::to_string),
            );
            let said = match &described {
                Some(line) => format!("{label} {line}"),
                None => label.clone(),
            };
            assert!(
                (tree.nodes.iter()).any(|(_, node)| node.role() == Role::Status
                    && node.live() == Some(Live::Polite)
                    && node.label() == Some(said.as_str())),
                "no live region says {said:?} as the card arrives"
            );
            (label, described)
        };
        // The site's line first, as it's drawn above the sentence.
        let title = "On mail.example. Claude Code wants to click “Send”.".to_string();
        let line = |words: &str| Some(format!("Press {words} to answer."));
        assert_eq!(heard(cx), (title.clone(), line("Option Shift Command A")));
        cx.update(|_, cx| {
            crate::shell::keys::change_keys(cx, |keys| {
                keys.set_key(Command::FocusAgentQuestion, 0, vec!["ctrl-alt-k".into()])
            })
        })
        .expect("the key was refused");
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(heard(cx), (title.clone(), line("Control Option K")));
        cx.update(|_, cx| {
            crate::shell::keys::change_keys(cx, |keys| {
                keys.set_key(Command::FocusAgentQuestion, 0, Vec::new())
            })
        })
        .expect("clearing the key was refused");
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(heard(cx), (title, None));
        // A request card is heard the same way.
        ask(&shell, cx, request(8));
        assert_eq!(
            heard(cx).0,
            "Claude Code asks to make a change on team.example.com."
        );
    }

    /// Where the row has room, as in the widest sidebar, ↩ is on the ringed button and ESC on
    /// Escape's, and in the narrowest neither is ever on another. The test's text runs wider than
    /// Inter or SF, so whether ↩ fits beside Allow Once there is the captures' to show.
    /// ESC takes the ×'s place only where the card's top line ends short of it, so it never sits
    /// on the site or the title; elsewhere the × stays, and Escape still answers as it does.
    #[gpui::test]
    fn esc_takes_the_close_only_beside_a_short_top_line(cx: &mut TestAppContext) {
        use super::CHIP_GAP;
        use crate::tokens::shell::permission::CLOSE;
        use crate::tokens::shell::{SIDEBAR_MAX, SIDEBAR_MIN};
        use gpui::accesskit::Role;
        let (_, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        let long = "a.long.name.of.a.site.on.the.mail.example";
        let cases = [
            ("a short site, wide", SIDEBAR_MAX, question(1), true),
            (
                "a short site, fenced",
                SIDEBAR_MAX,
                Question {
                    fenced: true,
                    ..question(2)
                },
                true,
            ),
            (
                "a long site",
                SIDEBAR_MIN,
                Question {
                    site: long.into(),
                    ..question(3)
                },
                false,
            ),
            (
                "no site, a title that wraps",
                SIDEBAR_MIN,
                Question {
                    site: "".into(),
                    ..question(4)
                },
                false,
            ),
        ];
        for (case, width, asked, esc) in cases {
            shell.update(cx, |shell, cx| {
                shell.sidebar_width = width;
                cx.notify();
            });
            ask(&shell, cx, asked);
            from_page(&shell, cx, KEY);
            let scale = f64::from(cx.update(|window, _| window.scale_factor()));
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let (_, close) = (tree.nodes.iter())
                .find(|(_, n)| n.role() == Role::Button && n.label() == Some("Close"))
                .unwrap_or_else(|| panic!("{case}: no Close"));
            let r = close.bounds().expect("a Close with no bounds");
            let (left, wide) = (r.x0 / scale, (r.x1 - r.x0) / scale);
            let top_end = shell.read_with(cx, |shell, _| shell.agent_card_room.get().top_end);
            // ESC's place is wider than the ×'s.
            let drawn = wide > f64::from(CLOSE) + 0.5;
            assert_eq!(
                drawn, esc,
                "{case}: ESC drawn {drawn}, top line ends at {top_end}"
            );
            if drawn {
                assert!(
                    f64::from(top_end + CHIP_GAP) <= left + 0.5,
                    "{case}: ESC at {left} sits on the top line, which ends at {top_end}"
                );
            }
            from_page(&shell, cx, KEY);
            shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
            cx.run_until_parked();
        }
    }

    #[gpui::test]
    fn the_chips_show_where_the_row_has_room(cx: &mut TestAppContext) {
        use super::{EscapeMark, escape_mark};
        use crate::tokens::shell::{SIDEBAR_MAX, SIDEBAR_MIN};
        let (_, shell, cx) = window_with(cx);
        for width in [SIDEBAR_MAX, SIDEBAR_MIN] {
            shell.update(cx, |shell, cx| {
                shell.sidebar_width = width;
                cx.notify();
            });
            for (card, asked, labels) in cards() {
                let escape = escape_mark(&asked);
                ask(&shell, cx, asked);
                from_page(&shell, cx, KEY);
                let ringed = keyed(&shell, cx).expect("no button has the keyboard");
                let chips: Vec<_> = shell.read_with(cx, |shell, _| {
                    (0..labels.len())
                        .map(|at| shell.card_chips(at, labels.len(), escape))
                        .collect()
                });
                let wanted: Vec<_> = (0..labels.len())
                    .map(|at| (at == ringed, escape == Some(EscapeMark::Button(at))))
                    .collect();
                if width == SIDEBAR_MAX {
                    assert_eq!(chips, wanted, "{card} at {width}");
                } else {
                    for (at, (&chip, &mark)) in chips.iter().zip(&wanted).enumerate() {
                        assert!(
                            (!chip.0 || mark.0) && (!chip.1 || mark.1),
                            "{card} at {width}: a chip on button {at}: {chips:?}"
                        );
                    }
                }
                from_page(&shell, cx, KEY);
                shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
                cx.run_until_parked();
            }
        }
    }

    /// `key` pressed and let go: GPUI clicks a focused element as ↩ or Space is let go, which its
    /// test keystrokes never do.
    fn press(cx: &mut VisualTestContext, key: &'static str) {
        let keystroke = gpui::Keystroke::parse(key).expect("a key");
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
        cx.run_until_parked();
    }

    /// A button a press gave the keyboard, with no key asked, takes no answer from ↩ or Space,
    /// let go too, however long after: a key answers a button only through the card's keys.
    #[gpui::test]
    fn a_button_a_press_gave_the_keyboard_takes_no_key(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        let once = shell.read_with(cx, |shell, _| shell.agent_card_buttons[1].clone());
        cx.update(|window, cx| window.focus(&once, cx));
        cx.run_until_parked();
        host.take();
        for key in ["enter", "space"] {
            press(cx, key);
            wait(cx, PROTECT * 2);
            press(cx, key);
        }
        assert_eq!(answers(&host), Vec::<String>::new());
        let asked = shell.read_with(cx, |shell, _| shell.agent_question().map(|q| q.id));
        assert_eq!(asked, Some(7), "a key answered the card");
    }

    /// With the card's keys, ↩ let go inside the protection answers nothing either: only the
    /// card's key down does, past it.
    #[gpui::test]
    fn enter_let_go_inside_the_protection_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        host.take();
        press(cx, "enter");
        press(cx, "space");
        assert_eq!(answers(&host), Vec::<String>::new());
        assert_eq!(keyed(&shell, cx), Some(1));
        wait(cx, PROTECT);
        press(cx, "enter");
        assert_eq!(answers(&host), ["answer agent 7 Once"]);
    }

    /// The window stopping being the active one, as for another app, ends the card's keys and
    /// gives the keyboard back where it was, so a key typed there, or coming back, never reaches
    /// the card: coming back, the user asks again with the key. On Windows the host's word that
    /// the keyboard left for another app does it, as GPUI's window there hears no activation.
    #[gpui::test]
    fn leaving_the_window_ends_the_cards_keys(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        host.take();
        cx.deactivate_window();
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), None, "the ring outlived the window");
        assert!(host.take().contains(&"focus page".to_string()));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), None, "the ring came back with the window");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), Vec::<String>::new());

        // Windows: the keyboard left the frame with no page of ours taking it.
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        host.take();
        shell.update_in(cx, |shell, window, cx| {
            shell.keyboard_left(0, false, window, cx)
        });
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), None, "the ring outlived the app");
        assert!(host.take().contains(&"focus page".to_string()));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), Vec::<String>::new());
    }

    /// One of the app's own windows taking the keyboard, as the command bar does over the window,
    /// ends the card's keys where they are: the page isn't given the keyboard from under it, and
    /// the window active again has no ring, ↩ answering nothing.
    #[gpui::test]
    fn an_overlay_of_the_apps_ends_the_cards_keys_where_they_are(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        host.take();
        host.app_active.set(true);
        cx.deactivate_window();
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), None, "the ring outlived the window");
        assert!(
            !host.take().contains(&"focus page".to_string()),
            "the page was given the keyboard under the app's own window"
        );
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), None, "the ring came back with the window");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), Vec::<String>::new());
        assert!(!host.take().contains(&"focus page".to_string()));
    }

    /// A key reaching the window after it was left for another app, before the frame that gives
    /// the keyboard back, as coming back to a window that drew nothing while hidden: it answers
    /// nothing, long past the protection, and goes on as a key the card's keys don't have.
    #[gpui::test]
    fn a_key_after_the_window_was_left_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        assert!(ring(&shell, cx).is_some());
        host.take();
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| shell.card_keys_window_left("window", cx));
            window.dispatch_action(Box::new(super::CardPress), cx);
        });
        cx.run_until_parked();
        assert_eq!(
            answers(&host),
            Vec::<String>::new(),
            "a key after the window was left answered the card"
        );
        assert_eq!(ring(&shell, cx), None);
    }

    /// GPUI saying the window stopped being active and active again in one batch of events, as
    /// sway's leave and enter as a keyboard comes: a frame drawn between, while it was inactive,
    /// decides nothing, and the card keeps its keys, ↩ answering. The leave goes on the other
    /// window and back, each change its own task, as the platform's events are.
    #[gpui::test]
    fn leaving_and_entering_at_once_keeps_the_cards_keys(cx: &mut TestAppContext) {
        struct Blank;
        impl gpui::Render for Blank {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                gpui::div()
            }
        }
        let (host, shell, cx) = window_with(cx);
        let other = cx.update(|_, cx| {
            cx.open_window(Default::default(), |_, cx| {
                gpui::AppContext::new(cx, |_| Blank)
            })
            .unwrap()
        });
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        let ringed = ring(&shell, cx);
        host.take();
        cx.update(|window, cx| {
            other
                .update(cx, |_, other, _| other.activate_window())
                .unwrap();
            window.activate_window();
        });
        cx.run_until_parked();
        assert_eq!(
            ring(&shell, cx),
            ringed,
            "the ring went with the window active again"
        );
        assert!(!host.take().contains(&"focus page".to_string()));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Once"]);
    }

    /// A click in the page ends the card's keys as the host says the page took the keyboard,
    /// before any key of the page's: the ring goes, and the key from the page brings it back.
    #[gpui::test]
    fn a_click_in_the_page_ends_the_cards_keys(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        shell.update_in(cx, |shell, window, cx| shell.page_took_keyboard(window, cx));
        cx.run_until_parked();
        let ring = shell.read_with(cx, |shell, _| shell.agent_card_ring());
        assert_eq!(ring, None, "the card kept its keys and its ring");
        host.take();
        assert!(from_page(&shell, cx, KEY));
        assert!(host.take().contains(&"focus ui".to_string()));
        assert_eq!(keyed(&shell, cx), Some(1));
    }

    /// From the command bar, the page having had the keyboard, the UI takes it with the ring, and
    /// the key again gives it back to the page.
    #[gpui::test]
    fn from_the_command_bar_the_ui_takes_the_keyboard(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        host.page_keys.set(Some(true));
        ask(&shell, cx, question(7));
        host.take();
        shell.update(cx, |shell, cx| {
            shell.run(Command::FocusAgentQuestion, "command bar", cx)
        });
        cx.run_until_parked();
        assert!(host.take().contains(&"focus ui".to_string()));
        assert_eq!(keyed(&shell, cx), Some(1));
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        assert!(host.take().contains(&"focus page".to_string()));
        assert_eq!(keyed(&shell, cx), None);
    }

    /// A held key's repeats leave the keyboard where its press put it, from the UI or the page.
    #[gpui::test]
    fn a_held_key_moves_the_keyboard_once(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let root = shell.read_with(cx, |shell, _| shell.focus_handle.clone());
        cx.update(|window, cx| window.focus(&root, cx));
        ask(&shell, cx, question(7));
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        assert_eq!(keyed(&shell, cx), Some(1));
        host.key_repeated.set(true);
        for _ in 0..3 {
            cx.simulate_keystrokes(KEY);
            cx.run_until_parked();
        }
        assert_eq!(keyed(&shell, cx), Some(1), "a repeat moved the keyboard");
        host.key_repeated.set(false);
        shell.update(cx, |shell, cx| shell.leave_card_keys(cx));
        cx.run_until_parked();
        host.take();
        let kept = shell.update(cx, |shell, cx| shell.run_agent_card_page_key(KEY, true, cx));
        cx.run_until_parked();
        assert!(kept, "the page got the key's repeat");
        assert!(!host.take().contains(&"focus ui".to_string()));
        let ring = shell.read_with(cx, |shell, _| shell.agent_card_ring());
        assert_eq!(ring, None, "the repeat rang the card");
    }

    /// The card's room is the last frame's for its own card only: a new card starts with none,
    /// so ESC never takes the ×'s place by the last card's line.
    #[gpui::test]
    fn a_new_card_starts_with_no_room(cx: &mut TestAppContext) {
        use crate::tokens::shell::SIDEBAR_MAX;
        let (_, shell, cx) = window_with(cx);
        shell.update(cx, |shell, cx| {
            shell.sidebar_width = SIDEBAR_MAX;
            cx.notify();
        });
        ask(&shell, cx, question(7));
        let drawn = shell.read_with(cx, |shell, _| shell.agent_card_room.get());
        assert_ne!(
            drawn,
            super::CardRoom::default(),
            "the card's room wasn't read"
        );
        let fresh = shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(question(8)), cx);
            shell.agent_card_room.get()
        });
        assert_eq!(fresh, super::CardRoom::default());
    }

    /// The keyboard goes back where it was in the UI when the card's answered or the key asked;
    /// where that's no longer drawn, the window takes it, as with nothing before.
    #[gpui::test]
    fn the_keyboard_goes_back_where_it_was_or_to_the_window(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let root = shell.read_with(cx, |shell, _| shell.focus_handle.clone());
        cx.update(|window, cx| window.focus(&root, cx));
        ask(&shell, cx, question(7));
        cx.simulate_keystrokes(KEY);
        cx.run_until_parked();
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Once"]);
        cx.update(|window, _| assert!(root.is_focused(window), "the keyboard went elsewhere"));

        // What had the keyboard is gone by the time it goes back.
        let gone = cx.update(|_, cx| cx.focus_handle());
        cx.update(|window, cx| window.focus(&gone, cx));
        ask(&shell, cx, question(8));
        shell.update(cx, |shell, cx| {
            shell.run(Command::FocusAgentQuestion, "ui", cx)
        });
        cx.run_until_parked();
        assert_eq!(keyed(&shell, cx), Some(1));
        shell.update(cx, |shell, cx| {
            shell.run(Command::FocusAgentQuestion, "ui", cx)
        });
        cx.run_until_parked();
        cx.update(|window, _| {
            assert!(
                !gone.is_focused(window),
                "the keyboard went to nothing drawn"
            );
            assert!(root.is_focused(window), "the window hasn't the keyboard");
        });
    }

    /// A click on ESC in the ×'s place answers as the × does.
    #[gpui::test]
    fn a_click_on_esc_answers_as_the_close_does(cx: &mut TestAppContext) {
        use crate::tokens::shell::SIDEBAR_MAX;
        use gpui::accesskit::Role;
        let (host, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        shell.update(cx, |shell, cx| {
            shell.sidebar_width = SIDEBAR_MAX;
            cx.notify();
        });
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        let scale = f64::from(cx.update(|window, _| window.scale_factor()));
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, close) = (tree.nodes.iter())
            .find(|(_, n)| n.role() == Role::Button && n.label() == Some("Close"))
            .expect("no Close");
        let r = close.bounds().expect("a Close with no bounds");
        let wide = (r.x1 - r.x0) / scale;
        assert!(
            wide > f64::from(crate::tokens::shell::permission::CLOSE) + 0.5,
            "no ESC"
        );
        let at = |v: f64| px((v / scale) as f32);
        host.take();
        cx.simulate_click(
            point(at((r.x0 + r.x1) / 2.), at((r.y0 + r.y1) / 2.)),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Dismiss"]);
    }

    /// The command bar offers Focus Agent Question only while a card asks.
    #[gpui::test]
    fn the_command_is_offered_only_with_a_card(cx: &mut TestAppContext) {
        let (_, shell, cx) = window_with(cx);
        let offered = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |s, _| s.offers(Command::FocusAgentQuestion))
        };
        assert!(!offered(cx), "offered with no card");
        ask(&shell, cx, question(7));
        assert!(offered(cx), "not offered with a card");
        shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
        cx.run_until_parked();
        assert!(!offered(cx), "offered after the card went");
    }

    /// The agent's name is heard as plain text, as the request card's: a line break or a control
    /// character the client put in it says nothing of its own.
    #[gpui::test]
    fn the_agents_name_is_heard_plain(cx: &mut TestAppContext) {
        use gpui::accesskit::Role;
        let (_, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        let asked = Question {
            agent: "Claude\n\u{7}Code\u{202E}".into(),
            ..question(7)
        };
        ask(&shell, cx, asked);
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, alert) = (tree.nodes.iter())
            .find(|(_, node)| node.role() == Role::AlertDialog)
            .expect("the card isn't an alert");
        assert_eq!(
            alert.label(),
            Some("On mail.example. Claude Code wants to click “Send”.")
        );
    }

    /// ↩ presses the stop GPUI's keyboard is on and the user hears, never another the ring was
    /// on: an assistive app moving the focus to Never moves the ring there, ↩ straight after
    /// answers nothing, and once the protection's passed ↩ answers Never.
    #[gpui::test]
    fn a_press_answers_only_the_stop_the_keyboard_is_on(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        host.take();
        let never = shell.read_with(cx, |shell, _| shell.agent_card_buttons[0].clone());
        // As the key comes before the frame after the move, and then after it.
        cx.update(|window, cx| window.focus(&never, cx));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            answers(&host),
            Vec::<String>::new(),
            "↩ answered off the ring"
        );
        assert_eq!(
            ring(&shell, cx),
            Some(0),
            "the ring didn't follow the keyboard"
        );
        let once = shell.read_with(cx, |shell, _| shell.agent_card_buttons[1].clone());
        wait(cx, PROTECT);
        cx.update(|window, cx| window.focus(&once, cx));
        cx.run_until_parked();
        assert_eq!(ring(&shell, cx), Some(1), "the frame didn't move the ring");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            answers(&host),
            Vec::<String>::new(),
            "↩ inside the protection answered"
        );
        wait(cx, PROTECT);
        cx.update(|window, cx| window.focus(&never, cx));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Never"]);
    }

    /// The page taking the keyboard where GPUI's focus doesn't see it, as the core says on Linux
    /// or another tab shown takes it to its page, ends the card's keys: no ring is left while ↩
    /// goes to the page. The key from the page then rings the card again, with any keys the shell
    /// still held taken as ended.
    #[gpui::test]
    fn the_page_taking_the_keyboard_unseen_ends_the_cards_keys(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        host.page_keys.set(Some(true));
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        assert_eq!(ring(&shell, cx), Some(1));
        host.page_keys.set(Some(true));
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(
            ring(&shell, cx),
            None,
            "the ring stayed with the page's keyboard"
        );
        host.page_keys.set(None);

        from_page(&shell, cx, KEY);
        assert_eq!(ring(&shell, cx), Some(1));
        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 11],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(11, cx);
        });
        cx.run_until_parked();
        assert_eq!(
            ring(&shell, cx),
            None,
            "the ring stayed as another tab showed"
        );

        // Keys the shell still holds as the key comes from the page had ended unseen.
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        from_page(&shell, cx, KEY);
        host.take();
        from_page(&shell, cx, KEY);
        let did = host.take();
        assert!(did.contains(&"focus ui".to_string()), "{did:?}");
        assert!(!did.contains(&"focus page".to_string()), "{did:?}");
        assert_eq!(keyed(&shell, cx), Some(1));
    }

    /// Tab and ⇧Tab start the protection over, as the keyboard coming does: ↩ straight after the
    /// ring moved answers nothing, so a ↩ typed on never presses a button just ringed.
    #[gpui::test]
    fn moving_the_ring_starts_the_protection_over(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        host.take();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            answers(&host),
            Vec::<String>::new(),
            "↩ pressed Always Allow"
        );
        wait(cx, PROTECT);
        cx.simulate_keystrokes("shift-tab");
        cx.simulate_keystrokes("shift-tab");
        cx.run_until_parked();
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Never"]);
    }

    /// A card that changes its buttons under its id, as the Agents can send it again changed,
    /// rings its primary again after the protection, so the ring never stays on a place that's
    /// now another button.
    #[gpui::test]
    fn a_card_changing_its_buttons_rings_its_primary_again(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        ask(&shell, cx, question(7));
        from_page(&shell, cx, KEY);
        wait(cx, PROTECT);
        cx.simulate_keystrokes("shift-tab");
        cx.run_until_parked();
        assert_eq!(keyed(&shell, cx), Some(0));
        wait(cx, PROTECT);
        let changed = Question {
            each_time: true,
            ..question(7)
        };
        ask(&shell, cx, changed);
        assert_eq!(keyed(&shell, cx), Some(1), "Allow isn't ringed");
        assert_eq!(ring(&shell, cx), Some(1));
        host.take();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            answers(&host),
            Vec::<String>::new(),
            "↩ answered the changed card at once"
        );
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(answers(&host), ["answer agent 7 Once"]);
    }

    /// Run() keeps no window from Focus Agent Question: a private window's and a Little ilo
    /// window's filters let it through, so the key kept from the page always does what it says.
    #[gpui::test]
    fn every_window_runs_the_command(cx: &mut TestAppContext) {
        use crate::shell::{commands::private_runs, little::little_runs};
        assert!(private_runs(Command::FocusAgentQuestion));
        assert!(little_runs(Command::FocusAgentQuestion));
        let _ = cx;
    }

    /// With no card the command does nothing, from the page, the UI or the bar: the keyboard stays
    /// and nothing is ringed.
    #[gpui::test]
    fn the_command_with_no_card_does_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        host.take();
        for from in ["page", "ui", "command bar"] {
            shell.update(cx, |shell, cx| {
                shell.run(Command::FocusAgentQuestion, from, cx)
            });
            cx.run_until_parked();
            let did = host.take();
            assert!(
                !did.iter().any(|d| d.starts_with("focus")),
                "{from}: {did:?}"
            );
            assert_eq!(ring(&shell, cx), None, "{from}");
        }
    }

    /// "See the whole request" is a stop of the card's keys: ↩ on it, past the protection, shows
    /// the whole request, so the request can be read before it's allowed.
    #[gpui::test]
    fn see_is_a_stop_of_the_cards_keys(cx: &mut TestAppContext) {
        let (_, shell, cx) = window_with(cx);
        struct Shown;
        impl crate::agent_request::Host for Shown {
            fn show(&self, _: u64, _: f32, _: f32, _: f32) {}
            fn set_height(&self, _: f32) {}
            fn hide(&self) {}
        }
        let whole = cx.update(|_, cx| crate::agent_request::open(Rc::new(Shown), cx));
        whole.expect("the whole request's window didn't open");
        ask(&shell, cx, request(4));
        from_page(&shell, cx, KEY);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(stop(&shell, cx), Some(2), "Tab from Allow didn't reach See");
        assert_eq!(ring(&shell, cx), Some(2));
        wait(cx, PROTECT);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let showing = cx.update(|_, cx| crate::agent_request::showing(cx));
        assert_eq!(showing, Some(4), "↩ on See didn't show the whole request");
    }
}
