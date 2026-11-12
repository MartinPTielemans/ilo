//! Pages that break (docs/ux-spec.md › Pages that break). A page whose renderer died leaves its pane
//! to a card of ours with Reload, as the core takes its view out of the window (`Window::LayOutPage`);
//! one that stops answering gets a note in the toast, over the page, with Wait and Reload; and the
//! core draws its own page for a load that failed (core/error_page.cc). The tab's row has a mark
//! while its page has died or hung. The core reports which (bc_tab_state.trouble), and agents read
//! the same.

use web_time::Instant;

use gpui::{Context, Div, FontWeight, MouseButton, Stateful, Window, div, prelude::*, px, svg};

use super::{
    SECONDARY, SPEAKER_DRAWN, SPEAKER_GAP, SPEAKER_SIZE, Shell, TITLE, Trouble,
    empty::FADE,
    fade_at,
    panes::pane_frames,
    permission::{BUTTONS_TOP, card_button, card_listener},
};

/// Ours, as neither Arc nor Dia was recorded with a page that broke: one line, one button.
const STOPPED: &str = "This page stopped";
pub const RELOAD: &str = "Reload";
pub const NOT_RESPONDING: &str = "Not responding";
pub const WAIT: &str = "Wait";
use crate::tokens::shell::trouble::BUTTON_INSET;
use crate::tokens::shell::trouble::LINE;
use crate::tokens::shell::trouble::WORDS_SIZE;
use crate::tokens::text;
/// Lucide's circle-alert, in the speaker's place in the row.
pub(super) const MARK: &str = "icons/circle-alert.svg";

/// A card shown in a dead page's pane.
#[derive(Default)]
pub(super) struct CrashCard {
    /// When it was first drawn since the tab was shown, for it to fade in from.
    since: Option<Instant>,
    /// The fade has ended and said so.
    settled: bool,
}

impl Shell {
    /// The core reported a new trouble for `tab`'s page.
    pub(super) fn trouble_changed(&mut self, tab: u64, trouble: Trouble, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: page {tab} trouble {}", name(trouble));
        match trouble {
            Trouble::Crashed => {
                self.crash_cards.entry(tab).or_default();
            }
            _ => {
                self.crash_cards.remove(&tab);
            }
        }
        self.note_hang(cx);
    }

    /// Shows the note for the tab shown while its page is hung, and takes it away once it isn't,
    /// or another tab shows. A window behind another notes it once it comes in front, as the
    /// toast shows over the window in front and would tell of a page it doesn't show (#1432).
    pub(super) fn note_hang(&mut self, cx: &mut Context<Self>) {
        let tab = self.selected;
        let hung = tab != 0 && self.page(tab).trouble == Trouble::Hung && self.browser.in_front();
        let hung = hung.then_some(tab);
        if hung == self.hang_noted {
            return;
        }
        if let Some(before) = std::mem::replace(&mut self.hang_noted, hung) {
            self.browser.hang_note(before, false, cx);
        }
        if let Some(tab) = hung {
            self.browser.hang_note(tab, true, cx);
        }
    }

    /// Starts the fade of each card shown, and has it start again once its tab is shown again.
    pub(super) fn step_crash_cards(&mut self, window: &mut Window) {
        if self.crash_cards.is_empty() {
            return;
        }
        let panes = self.panes(self.selected);
        let now = Instant::now();
        for (&tab, card) in &mut self.crash_cards {
            if !panes.contains(&tab) {
                *card = CrashCard::default();
                continue;
            }
            let since = *card.since.get_or_insert(now);
            if now.duration_since(since).as_secs_f32() < FADE {
                window.request_animation_frame();
            } else if !std::mem::replace(&mut card.settled, true) {
                // check.sh waits for this line before it captures the card.
                log!("browser: crash card shown {tab}");
            }
        }
    }

    /// The card in each pane shown whose page died: its one line and Reload, centred, fading in as
    /// the empty card's line does. A press on it shows its tab, as one on a page does.
    pub(super) fn crash_cards(&self, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        if self.crash_cards.is_empty() || self.selected == 0 {
            return Vec::new();
        }
        let (frame, _) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        let now = Instant::now();
        pane_frames(frame, panes.len(), stacked, &shares)
            .into_iter()
            .zip(panes)
            .filter_map(|(bounds, tab)| {
                let card = self.crash_cards.get(&tab)?;
                let opacity = card.since.map_or(0., |since| {
                    fade_at(
                        0.,
                        1.,
                        now.saturating_duration_since(since).as_secs_f32(),
                        FADE,
                    )
                });
                let reload = card_button(&self.palette, "crash-reload", RELOAD)
                    // check.sh clicks it where this says it is.
                    .child(card_listener("crash reload", &self.crash_reload_drawn, tab))
                    .flex_none()
                    .px(px(BUTTON_INSET))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        // check.sh reads this line.
                        log!("browser: crash card reload {tab}");
                        shell.browser.reload(tab, false, cx);
                    }));
                let words = div()
                    .text_size(px(WORDS_SIZE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.palette.text(TITLE))
                    .child(STOPPED);
                Some(
                    div()
                        .id(("crash-card", tab))
                        .absolute()
                        .left(bounds.left())
                        .top(bounds.top())
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        // The plates' type, so the button is theirs.
                        .text_size(px(text::BODY))
                        .line_height(px(LINE))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |shell, _, _, cx| {
                                // Not the window's background, whose drag moves the window.
                                cx.stop_propagation();
                                if shell.selected != tab {
                                    shell.select(tab, cx);
                                }
                            }),
                        )
                        .child(card_listener("crash card", &self.crash_drawn, tab))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .opacity(opacity)
                                .child(words)
                                .child(div().mt(px(BUTTONS_TOP)).flex().child(reload)),
                        ),
                )
            })
            .collect()
    }

    /// The mark in the speaker's place in `tab`'s row while its page has died or hung. The row
    /// still shows the tab, and its card or note says what to do.
    pub(super) fn trouble_mark(&self, tab: u64) -> Option<Div> {
        matches!(self.page(tab).trouble, Trouble::Crashed | Trouble::Hung).then(|| {
            div()
                .flex_none()
                .mr(px(SPEAKER_GAP))
                .size(px(SPEAKER_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(MARK)
                        .size(px(SPEAKER_DRAWN))
                        .text_color(self.palette.text(SECONDARY)),
                )
        })
    }
}

/// How the logs name `trouble`.
fn name(trouble: Trouble) -> String {
    match trouble {
        Trouble::Fine => "fine".into(),
        Trouble::Crashed => "crashed".into(),
        Trouble::Hung => "hung".into(),
        Trouble::NetworkError(code) => format!("network {code}"),
    }
}

#[cfg(test)]
mod tests;
