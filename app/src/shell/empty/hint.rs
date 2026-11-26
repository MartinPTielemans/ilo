//! First run's last word (first-run v9 §10): under the landing field, once, the key that brings
//! the field back, gone at the first key. Its "seen" is `Shell::in_hint` alone, never kept and
//! never what was typed.

use gpui::{App, Context, div, prelude::*, px};

use super::super::{Command, Shell, commands::ShellHandle, keys::user_keys};
use super::composer::{group_top, mark_top};
use crate::command_bar;
use crate::tokens::command_bar::HEIGHT as BAR_HEIGHT;
use crate::tokens::shell::empty::{HINT_GAP, MARK_SIZE, WORDS_SIZE};

/// What the hint says after the new tab's key.
const BRINGS_IT_BACK: &str = "brings this back anywhere.";

impl Shell {
    /// Puts the hint under the landing field as first run ends, to go at the first key anywhere.
    pub(in crate::shell) fn show_hint(&mut self, cx: &mut Context<Self>) {
        self.in_hint = true;
        self.hint_keys = Some(cx.observe_keystrokes(|shell, event, _, cx| {
            let key = event.keystroke.key.as_str();
            if ["shift", "control", "alt", "platform", "function"].contains(&key) {
                return;
            }
            shell.in_hint = false;
            // Not dropped inside its own call.
            let keys = shell.hint_keys.take();
            cx.defer(move |_| drop(keys));
            cx.notify();
        }));
    }

    /// The hint goes, for good.
    pub(in crate::shell) fn drop_hint(&mut self) {
        self.in_hint = false;
        self.hint_keys = None;
    }

    /// The hint under a landing field whose top is `top` on a card `width` wide, while it shows.
    pub(in crate::shell) fn landing_hint(
        &self,
        top: f32,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        if !self.in_hint {
            return None;
        }
        let keys = user_keys(cx).label(Command::NewTab)?;
        let palette = self.palette;
        Some(
            div()
                .debug_selector(|| "landing-hint".into())
                .absolute()
                .top(px(top + BAR_HEIGHT + HINT_GAP))
                .left_0()
                .w(px(width))
                .flex()
                .justify_center()
                .text_size(px(WORDS_SIZE))
                .text_color(palette.gray(palette.tertiary()))
                .child(format!("{keys} {BRINGS_IT_BACK}")),
        )
    }

    /// Where ⌘T's bar's field sits on the card when the card doesn't rest on its own: its top, in
    /// the card's frame whose top is `card_top`.
    pub(in crate::shell) fn opened_bar_top(&self, card_top: f32) -> f32 {
        command_bar::centered_top(f32::from(self.viewport.height)) - card_top
    }

    /// Where the empty page's mark sits over its landing field in a browser window `viewport`
    /// across, with no tasks under the field, as first run's exit lands its knot there: its top
    /// left. The page's frame comes from the sidebar's kept width and side, which first run's
    /// window doesn't draw, so nothing is drawn or measured for it.
    pub(in crate::shell) fn landing_mark(&self, viewport: gpui::Size<gpui::Pixels>) -> Option<(f32, f32)> {
        let (frame, _) = self.page_frame(viewport);
        let (left, top) = (f32::from(frame.left()), f32::from(frame.top()));
        let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
        let mark = mark_top(group_top(height, false, false), false)?;
        Some((left + (width - MARK_SIZE) / 2., top + mark))
    }
}

/// Something went into a bar's field, an input method's first marked text too, which no key
/// observer hears: the hint goes. Nothing of what went in is read.
pub fn field_typed(cx: &mut App) {
    let Some(shell) = (cx.try_global::<ShellHandle>()).and_then(|handle| handle.0.upgrade()) else {
        return;
    };
    if !shell.read(cx).in_hint {
        return;
    }
    cx.defer(move |cx| {
        shell.update(cx, |shell, cx| {
            shell.drop_hint();
            cx.notify();
        })
    });
}
