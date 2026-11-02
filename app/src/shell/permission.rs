//! A page's question about permissions, such as a site asking for the camera: a card under the URL
//! field, over the sidebar's rows, until the user answers or the page stops asking. Chrome hangs
//! its prompt from the address bar's site icon, which in our layout is the sidebar's URL field.
//! With the sidebar hidden, the reveal brings it out with the question and keeps it out until the
//! question goes. The core keeps each tab's request while another tab is shown and asks again
//! once it's back.

use std::{cell::Cell, rc::Rc};

use gpui::{
    Bounds, Context, Div, FontWeight, Pixels, SharedString, Stateful, canvas, div,
    prelude::*, px, rgba, svg,
};

use crate::{
    palette::{Palette, STATE_HOVERED, STATE_PRESSED},
    settling::Settling,
};
use super::{
    NAV, NAV_RADIUS, PLATE_INSET, Shell, TITLE, URL_FIELD, URL_FIELD_HEIGHT,
    URL_FIELD_TOP,
};

use crate::tokens::shell::permission::GAP;
use crate::tokens::shell::permission::PADDING;
use crate::tokens::shell::permission::RADIUS;
use crate::tokens::shell::permission::LINE;
use crate::tokens::shell::permission::DETAIL;
use crate::tokens::shell::permission::BUTTON_HEIGHT;
use crate::tokens::shell::permission::BUTTON_GAP;
pub(super) use crate::tokens::shell::permission::BUTTONS_TOP;
use crate::tokens::shell::permission::CLOSE;
use crate::tokens::shell::permission::CLOSE_INSET;
use crate::tokens::shell::permission::CLOSE_GLYPH;
use crate::elevation::Level;
use crate::tokens::text;
/// The room a card's first line leaves for its ×.
pub(super) const BESIDE_CLOSE: f32 = CLOSE + CLOSE_INSET - PADDING;

/// What a tab's page asks, as the core reports it.
#[derive(Clone, PartialEq, Debug)]
pub struct Question {
    /// Who asks, as the user should see it.
    pub site: SharedString,
    /// What it asks for, each a phrase that follows "wants to", such as "Use your camera".
    pub permissions: Vec<SharedString>,
}

/// The user's answer to a page's question.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Answer {
    /// The site may, now and whenever it asks again.
    Allow,
    /// It may not, now or again.
    Block,
    /// Not now: it may ask again.
    Dismiss,
}

impl Shell {
    /// The core reports what `tab`'s page asks, or that it no longer asks (None).
    pub fn set_question(&mut self, tab: u64, question: Option<Question>, cx: &mut Context<Self>) {
        let changed = self.questions.get(&tab) != question.as_ref();
        match question {
            Some(question) => self.questions.insert(tab, question),
            None => self.questions.remove(&tab),
        };
        // A question that changes under the pointer is a new one to settle.
        if tab == self.selected && changed {
            self.question_settling.replace(None);
        }
        // The question needs the sidebar, so a hidden one comes out with it.
        if tab == self.selected && self.question().is_some() && self.can_reveal() {
            self.set_revealed(true, cx);
        }
        cx.notify();
    }

    /// The question of the tab shown, if its page asks one.
    pub(super) fn question(&self) -> Option<&Question> {
        self.questions.get(&self.selected)
    }

    fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        let tab = self.selected;
        let settled = (self.question_settling.borrow().as_ref())
            .is_some_and(|(shown, settling)| *shown == tab && settling.settled(cx));
        if answer != Answer::Dismiss && !settled {
            return;
        }
        if self.questions.remove(&tab).is_some() {
            self.browser.answer_permission(tab, answer, cx);
            cx.notify();
        }
    }

    /// The question of the tab shown, as a card under the URL field.
    pub(super) fn question_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let Some(question) = self.question() else {
            // Drawn again, it's logged again, and settles again.
            self.question_drawn.set(None);
            self.question_settling.replace(None);
            return None;
        };
        let mut settling = self.question_settling.borrow_mut();
        if settling.as_ref().is_none_or(|(shown, _)| *shown != self.selected) {
            let mut started = Settling::default();
            started.start(format!("question {}", self.selected), cx);
            *settling = Some((self.selected, started));
        }
        drop(settling);
        let button = |id: &'static str, label: &'static str, answer: Answer| {
            card_button(&self.palette, id, label)
                .on_click(cx.listener(move |shell, _, _, cx| shell.answer(answer, cx)))
        };
        let close = card_close(&self.palette, "question-close")
            .on_click(cx.listener(|shell, _, _, cx| shell.answer(Answer::Dismiss, cx)));
        let listener = card_listener("question", &self.question_drawn, self.selected);
        Some(
            self.plate("question")
                .child(listener)
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        // Kept together, so "to" doesn't wrap alone under a long site.
                        .child(format!("{} wants\u{a0}to", question.site)),
                )
                .children(question.permissions.iter().map(|permission| {
                    div().text_color(rgba(DETAIL)).child(permission.clone())
                }))
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .gap(px(BUTTON_GAP))
                        .child(button("question-block", "Block", Answer::Block))
                        .child(button("question-allow", "Allow", Answer::Allow)),
                )
                .child(close),
        )
    }
}

/// A card's ×, in its top corner, its glyph's edge on the text's.
pub(super) fn card_close(palette: &Palette, id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .top(px(CLOSE_INSET))
        .right(px(CLOSE_INSET))
        .size(px(CLOSE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(NAV_RADIUS))
        .hover(|style| style.bg(palette.plate(STATE_HOVERED)))
        .active(|style| style.bg(palette.plate(STATE_PRESSED)))
        .child(svg().path("icons/x.svg").size(px(CLOSE_GLYPH)).text_color(palette.glyph(NAV)))
}

/// A button on a card, as wide as its share of the row, as tall as a line of text and a half, and
/// taller if its words wrap.
pub(super) fn card_button(
    palette: &Palette,
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h(px(BUTTON_HEIGHT))
        .py(px((BUTTON_HEIGHT - LINE) / 2.))
        .px(px(BUTTON_GAP))
        .flex()
        .items_center()
        .justify_center()
        .text_center()
        .rounded(px(NAV_RADIUS))
        .bg(palette.plate(URL_FIELD))
        .hover(|style| style.bg(palette.state(URL_FIELD, STATE_HOVERED)))
        .active(|style| style.bg(palette.state(URL_FIELD, STATE_PRESSED)))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text(TITLE))
        .child(label.into())
}

impl Shell {
    /// The card the page's question and the DRM note are: under the URL field, as wide as it, in
    /// the window's fill with a line inside its edge.
    pub(super) fn plate(&self, id: &'static str) -> Stateful<Div> {
        div()
            .id(id)
            .absolute()
            .top(px(URL_FIELD_TOP + URL_FIELD_HEIGHT + GAP))
            .left(px(PLATE_INSET))
            .w(px(self.width() - 2. * PLATE_INSET))
            .p(px(PADDING))
            .rounded(px(RADIUS))
            .bg(self.window_fill())
            .border_1()
            .border_color(Level::Popover.rim())
            .shadow(Level::Popover.shadow())
            .block_mouse_except_scroll()
            .text_size(px(text::BODY))
            .line_height(px(LINE))
    }

}

/// Logs where a card is in its window, as `what` ("question") for `tab`, when that changes.
/// check.sh clicks the buttons from where the card is, inside its edge, which depends on how its
/// text wraps.
pub(super) fn card_listener(
    what: &'static str,
    drawn: &Rc<Cell<Option<Bounds<Pixels>>>>,
    tab: u64,
) -> impl IntoElement + use<> {
    let drawn = drawn.clone();
    canvas(move |bounds, _, _| logged_at(what, &drawn, tab, bounds), |_, _, _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

/// Logs where a card is in its window when that changes.
fn logged_at(
    what: &str,
    drawn: &Rc<Cell<Option<Bounds<Pixels>>>>,
    tab: u64,
    bounds: Bounds<Pixels>,
) {
    if drawn.get() == Some(bounds) {
        return;
    }
    drawn.set(Some(bounds));
    // check.sh reads this line.
    log!(
        "browser: {what} {tab} at {:.0} {:.0} {:.0} {:.0}",
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    );
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Modifiers, Point, Pixels, TestAppContext, point, px};

    use crate::{
        settling::SETTLING,
        shell::{Folders, Question, Shell, recorder::Recorder},
    };

    /// A page can time its question to come up under a double-click the user started, so for the
    /// card's first moment on screen a click on Allow does nothing, while × dismisses at once.
    #[gpui::test]
    fn the_card_takes_no_answer_for_good_for_its_first_moment(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let ask = |shell: &mut Shell, cx: &mut gpui::Context<Shell>| {
            let permissions = vec!["Use your camera".into()];
            shell.set_question(10, Some(Question { site: "example.com".into(), permissions }), cx);
        };
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_selected(10, cx);
            ask(shell, cx);
        });
        cx.run_until_parked();
        // Allow is at the right of the buttons along the card's bottom, × in its top corner.
        let at = |shell: &Shell| -> (Point<Pixels>, Point<Pixels>) {
            let card = shell.question_drawn.get().expect("the question wasn't drawn");
            let (right, bottom) = (card.origin.x + card.size.width, card.origin.y + card.size.height);
            let allow = point(card.origin.x + card.size.width * 0.75, bottom - px(26.));
            (allow, point(right - px(18.), card.origin.y + px(18.)))
        };
        let (allow, close) = shell.read_with(cx, |shell, _| at(shell));
        host.take();
        cx.simulate_click(allow, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), Vec::<String>::new());
        cx.executor().advance_clock(SETTLING);
        cx.simulate_click(allow, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["answer 10 Allow"]);
        // Asked again, it settles again, but × needn't wait.
        shell.update(cx, |shell, cx| ask(shell, cx));
        cx.run_until_parked();
        cx.simulate_click(allow, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), Vec::<String>::new());
        cx.simulate_click(close, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["answer 10 Dismiss"]);
    }
}
