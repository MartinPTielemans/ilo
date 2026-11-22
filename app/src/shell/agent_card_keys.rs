//! The keys of an agent's question card (#3869). Nothing answers a card from the page, where
//! someone typing may never have seen it arrive: Focus Agent Question's key (⌥⇧⌘A on the Mac,
//! Alt+Shift+A elsewhere, Chrome's Focus Inactive Popup) moves the keyboard to the card and rings
//! its primary button, and only then do ↩ and Space press the ringed button, Tab and ⇧Tab move
//! along the buttons, and Escape answers as the card's × does. The key again gives the keyboard
//! back where it was and answers nothing. Only the user's keys come here: an agent's go to its page
//! (keys.rs's tab_key_before_page).

use std::time::Duration;

use web_time::Instant;

use gpui::{
    App, Context, Div, FocusHandle, FontWeight, Hsla, KeyBinding, Rgba, Stateful, Task, Window,
    actions, canvas, div, prelude::*, px, svg,
};

use super::TITLE;
use super::agent_question::{Button, Does, buttons};
use super::permission::{Look, card_button_as, card_close};
use crate::tokens::dialog::{
    CHIP_HEIGHT, CHIP_RADIUS, ESC_FILL, ESC_TEXT, RETURN_BORDER, RETURN_FILL, RETURN_GLYPH,
    RETURN_WIDTH,
};
use crate::tokens::shell::permission::{BUTTON_GAP, CLOSE, CLOSE_INSET, LINE};
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

/// What card_chips reads from the last frame: the width of the card's row of buttons, and each
/// button's top and its label's width.
#[derive(Clone, Copy, Default)]
pub(super) struct CardRoom {
    row: f32,
    buttons: [(f32, f32); MOST_BUTTONS],
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
    /// The ringed button, in `buttons`' order.
    at: usize,
    came: Came,
    /// When the keyboard came, then the last key that would have answered, whether it did or not.
    last: Instant,
    /// The frame still has to give the ringed button the keyboard.
    due: bool,
    /// The frame still has to give the keyboard back, answering nothing.
    leaving: bool,
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
    /// the keyboard on it already, back where it was.
    pub(super) fn focus_agent_question(&mut self, from_page: bool, cx: &mut Context<Self>) {
        if let Some(keys) = &mut self.card_keys {
            keys.leaving = true;
            return cx.notify();
        }
        let Some(question) = self.agent_question() else {
            return;
        };
        let list = buttons(question);
        // Always Allow, which reaches further than it looks, is never the first ringed.
        let at = (list.iter())
            .position(|button| button.look == Look::Primary)
            .unwrap_or(0);
        let id = question.id;
        // check.sh waits for this line.
        log!("browser: agent card keys on {id} {}", list[at].label);
        if from_page {
            self.take_keyboard(cx);
        }
        self.card_keys = Some(CardKeys {
            question: id,
            at,
            came: match from_page {
                true => Came::Page,
                false => Came::Ui(None),
            },
            last: cx.background_executor().now(),
            due: true,
            leaving: false,
            _ready: ready(cx),
        });
        cx.notify();
    }

    /// Focus Agent Question's key from the page, kept from it only while a card shows: the page
    /// gets it back once none does, and no other key reaches the card from the page.
    pub fn run_agent_card_page_key(&mut self, keystroke: &str, cx: &mut Context<Self>) -> bool {
        if self.agent_question().is_none()
            || user_keys(cx).command_for(keystroke, false) != Some(Command::FocusAgentQuestion)
        {
            return false;
        }
        self.run(Command::FocusAgentQuestion, "page", cx);
        true
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
    pub(super) fn agent_card_described(&self, cx: &App) -> Option<String> {
        let keys = user_keys(cx).shown_keys(Command::FocusAgentQuestion);
        let key = keys.first()?.first()?;
        Some(format!("Press {} to answer.", spoken(key)))
    }

    /// Before the frame's drawn: gives the ringed button the keyboard, and gives it back where it
    /// came from once the card's answered, gone or covered, or the key asked. Something else taking
    /// the keyboard, a click or a shortcut, ends the keys where they went.
    pub(super) fn step_card_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let showing = self.agent_question().map(|q| (q.id, buttons(q).len()));
        let on_card = self.agent_card_buttons.iter().any(|h| h.is_focused(window));
        let Some(keys) = &mut self.card_keys else {
            // A click gives a button the keyboard too. With the card gone no drawn element holds
            // it, and GPUI matches a key's bindings from the focused element, so the window takes
            // it, as from "See the whole request" (#3715).
            if on_card && showing.is_none() {
                window.focus(&self.focus_handle, cx);
            }
            return;
        };
        let why = match showing {
            _ if keys.leaving => "key",
            Some((id, count)) if id == keys.question && keys.at < count => {
                if std::mem::take(&mut keys.due) {
                    if let Came::Ui(None) = keys.came {
                        keys.came = Came::Ui(window.focused(cx).filter(|focused| {
                            !self.agent_card_buttons.iter().any(|h| h == focused)
                        }));
                    }
                    window.focus(&self.agent_card_buttons[keys.at], cx);
                } else if !on_card {
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
        if !on_card && !keys.due {
            return;
        }
        match keys.came {
            Came::Page => {
                // No drawn element holds the keyboard as the page takes it (#3715).
                window.focus(&self.focus_handle, cx);
                self.browser.focus_page(cx);
            }
            Came::Ui(Some(was)) => window.focus(&was, cx),
            Came::Ui(None) => window.focus(&self.focus_handle, cx),
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

    fn card_press(&mut self, cx: &mut Context<Self>) {
        if !self.card_key_counts(cx) {
            return;
        }
        let Some(at) = self.agent_card_ring() else {
            return;
        };
        let Some(button) = self
            .agent_question()
            .and_then(|q| buttons(q).get(at).copied())
        else {
            return;
        };
        log!("browser: agent card key pressed {}", button.label);
        self.press_card(button.does, cx);
    }

    fn card_leave(&mut self, cx: &mut Context<Self>) {
        if !self.card_key_counts(cx) {
            return;
        }
        match self.agent_question().map(escape_does) {
            Some(Some(does)) => {
                log!("browser: agent card key escape");
                self.press_card(does, cx);
            }
            Some(None) => self.focus_agent_question(false, cx),
            None => {}
        }
    }

    fn card_move(&mut self, by: isize, cx: &mut Context<Self>) {
        let count = self.agent_question().map_or(0, |q| buttons(q).len());
        let Some(keys) = self.card_keys.as_mut().filter(|_| count > 0) else {
            return;
        };
        keys.at = (keys.at as isize + by).rem_euclid(count as isize) as usize;
        keys.due = true;
        let (id, at) = (keys.question, keys.at);
        if let Some(button) = self.agent_question().map(|q| buttons(q)[at]) {
            log!("browser: agent card keys on {id} {}", button.label);
        }
        cx.notify();
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
                room.set(all);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
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
            .on_click(cx.listener(move |shell, _, _, cx| shell.press_card(button.does, cx)))
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

    /// The card's × or, while the keyboard's on the card and Escape is the ×'s, ESC in its place,
    /// pressed as the × is.
    pub(super) fn card_key_close(
        &self,
        escape: Option<EscapeMark>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let close = match self.agent_card_ring().is_some() && escape == Some(EscapeMark::Close) {
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
            .on_click(
                cx.listener(|shell, _, _, cx| shell.press_card(Does::Answer(Answer::Dismiss), cx)),
            )
    }

    /// The card's element taking its keys while one of its buttons has the keyboard.
    pub(super) fn card_keys_on<E: gpui::InteractiveElement>(
        &self,
        card: E,
        cx: &Context<Self>,
    ) -> E {
        card.key_context(CONTEXT)
            .on_action(cx.listener(|shell, _: &CardPress, _, cx| shell.card_press(cx)))
            .on_action(cx.listener(|shell, _: &CardNext, _, cx| shell.card_move(1, cx)))
            .on_action(cx.listener(|shell, _: &CardBack, _, cx| shell.card_move(-1, cx)))
            .on_action(cx.listener(|shell, _: &CardLeave, _, cx| shell.card_leave(cx)))
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
        let kept = shell.update(cx, |shell, cx| shell.run_agent_card_page_key(key, cx));
        cx.run_until_parked();
        kept
    }

    /// The card's button with GPUI's keyboard, by its place in the card's buttons.
    fn keyed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<usize> {
        let handles = shell.read_with(cx, |shell, _| shell.agent_card_buttons.clone());
        cx.update(|window, _| handles.iter().position(|h| h.is_focused(window)))
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
    /// move along all the buttons, round from the last to the first; and ↩ presses the one ringed
    /// as a click does.
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
            let mut at = primary;
            for _ in 0..labels.len() {
                cx.simulate_keystrokes("tab");
                cx.run_until_parked();
                at = (at + 1) % labels.len();
                assert_eq!(keyed(&shell, cx), Some(at), "{card} after tab");
            }
            cx.simulate_keystrokes("shift-tab");
            cx.run_until_parked();
            at = (at + labels.len() - 1) % labels.len();
            assert_eq!(keyed(&shell, cx), Some(at), "{card} after shift-tab");
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
        use gpui::accesskit::Role;
        let (_, shell, cx) = window_with(cx);
        cx.simulate_a11y_activation();
        ask(&shell, cx, question(7));
        let heard = |cx: &mut VisualTestContext| {
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let (_, alert) = (tree.nodes.into_iter())
                .find(|(_, node)| node.role() == Role::AlertDialog)
                .expect("the card isn't an alert");
            (
                alert.label().unwrap_or("").to_string(),
                alert.description().map(str::to_string),
            )
        };
        let title = "Claude Code wants to click “Send” on mail.example.".to_string();
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
}
