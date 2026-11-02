//! An agent's question: what it would do that asks the user first, such as an order, a message
//! sent or a bank's site, in a card under the URL field of every window, where a page's question
//! shows, until the user answers. The agent works in tabs the user isn't looking at, so the card
//! doesn't wait for its tab to be shown. A page's own question goes first, as it's about the page
//! in front of the user. The Agents keep what waits on the answer (agents::Agents::answer). A
//! handoff asks the same way: the agent needs the user in a tab, which Show Tab shows and Hand Back
//! gives back. So does an agent asking for a tab the user took over: Keep, or Hand Back.

use gpui::{Context, FontWeight, div, prelude::*, px, rgba};

use super::{
    Shell, TITLE,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, card_button, card_close, card_listener},
};
use crate::agents::{Answer, Question};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL};

impl Shell {
    /// The Agents ask the user `question`, or no longer ask (None).
    pub fn set_agent_question(&mut self, question: Option<Question>, cx: &mut Context<Self>) {
        if self.agent_question == question {
            return;
        }
        let asked = |old: &&Question| question.as_ref().map(|new| new.id) != Some(old.id);
        if let Some(gone) = self.agent_question.as_ref().filter(asked) {
            // check.sh reads this line, as the agent's client goes with its question unanswered.
            log!("browser: agent question {} gone", gone.id);
        }
        self.agent_question = question;
        // The question needs the sidebar, so a hidden one comes out with it.
        if self.agent_question.is_some() && self.can_reveal() {
            self.set_revealed(true, cx);
        }
        cx.notify();
    }

    /// The agent's question the card shows, unless a page's question has the place.
    pub(super) fn agent_question(&self) -> Option<&Question> {
        self.agent_question.as_ref().filter(|_| self.question().is_none())
    }

    fn answer_agent(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            self.browser.answer_agent(question.id, answer, cx);
            cx.notify();
        }
    }

    /// The agent's question, as a card under the URL field.
    pub(super) fn agent_question_card(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let Some(question) = self.agent_question() else {
            // Drawn again, it's logged again.
            self.agent_question_drawn.set(None);
            return None;
        };
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        if let Some(tab) = question.handoff {
            let first = match question.back {
                true => card_button(&self.palette, "agent-keep", "Keep").on_click(
                    cx.listener(|shell, _, _, cx| shell.answer_agent(Answer::Never, cx)),
                ),
                false => card_button(&self.palette, "agent-handoff-show", "Show Tab")
                    .on_click(cx.listener(move |shell, _, _, cx| shell.select(tab, cx))),
            };
            let back = card_button(&self.palette, "agent-handoff-back", "Hand Back")
                .on_click(cx.listener(move |shell, _, _, cx| shell.browser.hand_back(tab, cx)));
            let title = match question.back {
                true => format!("{} asks for a tab", question.agent),
                false => format!("{} needs you", question.agent),
            };
            return Some(
                self.plate("agent-question")
                    .child(listener)
                    .child(
                        div()
                            .text_color(self.palette.text(TITLE))
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(div().text_color(rgba(DETAIL)).child(question.why.clone()))
                    .child(
                        div()
                            .mt(px(BUTTONS_TOP))
                            .flex()
                            .gap(px(BUTTON_GAP))
                            .child(first)
                            .child(back),
                    )
                    .into_any_element(),
            );
        }
        let button = |id: &'static str, label: &'static str, answer: Answer| {
            card_button(&self.palette, id, label)
                .on_click(cx.listener(move |shell, _, _, cx| shell.answer_agent(answer, cx)))
        };
        let close = card_close(&self.palette, "agent-question-close")
            .on_click(cx.listener(|shell, _, _, cx| shell.answer_agent(Answer::Dismiss, cx)));
        Some(
            self.plate("agent-question")
                .child(listener)
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("{} wants to {}", question.agent, question.what)),
                )
                .child(div().text_color(rgba(DETAIL)).child(question.why.clone()))
                // Three labels don't fit a row of the sidebar at its default width, so the last
                // takes a row of its own, as wide as the card, rather than running past its edge.
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .flex_wrap()
                        .gap(px(BUTTON_GAP))
                        .child(button("agent-question-never", "Never", Answer::Never))
                        .child(button("agent-question-once", "Allow Once", Answer::Once))
                        .child(button("agent-question-always", "Always Allow", Answer::Always)),
                )
                .child(close)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext, point, px};

    use crate::agents::Question;
    use crate::shell::{Folders, Page, Question as PageQuestion, Shell, recorder::Recorder};
    use crate::tokens::shell::permission::{BUTTON_GAP, BUTTON_HEIGHT, PADDING};

    fn question(id: u64) -> Question {
        Question {
            id,
            agent: "Claude Code".into(),
            from: "claude-code".into(),
            what: "click “Place order” on shop.example".into(),
            why: "It looks like a payment or an order.".into(),
            handoff: None,
            back: false,
        }
    }

    /// The card asks whatever tab the window shows, a page's own question goes first, and its
    /// buttons answer, all inside it at the sidebar's width: Never and Allow Once side by side,
    /// and Always Allow in the row under them, from the card's left edge to its right.
    #[gpui::test]
    fn the_card_asks_and_its_buttons_answer(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            let page = Page { url: "https://example.com/".into(), ..Page::default() };
            shell.set_page(10, page, cx);
            shell.set_selected(10, cx);
            shell.set_agent_question(Some(question(7)), cx);
        });
        cx.run_until_parked();
        // The card and the middles of its last row and the row above it.
        let rows = |shell: &gpui::Entity<Shell>, cx: &mut gpui::VisualTestContext| {
            let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
            let card = card.expect("the card wasn't drawn");
            let last = card.bottom() - px(PADDING + BUTTON_HEIGHT / 2.);
            (card, last, last - px(BUTTON_HEIGHT + BUTTON_GAP))
        };
        let (card, _, above) = rows(&shell, cx);
        cx.simulate_click(point(card.left() + card.size.width * 0.75, above), Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 7 Once".to_string()));
        shell.read_with(cx, |shell, _| assert!(shell.agent_question().is_none()));

        shell.update(cx, |shell, cx| {
            let asked = PageQuestion { site: "example.com".into(), permissions: Vec::new() };
            shell.set_question(10, Some(asked), cx);
            shell.set_agent_question(Some(question(8)), cx);
            assert!(shell.agent_question().is_none(), "the agent's card covered the page's");
            shell.set_question(10, None, cx);
            assert_eq!(shell.agent_question().map(|question| question.id), Some(8));
        });
        cx.run_until_parked();
        let (card, last, above) = rows(&shell, cx);
        cx.simulate_click(point(card.left() + px(PADDING + 4.), last), Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 8 Always".to_string()));

        shell.update(cx, |shell, cx| shell.set_agent_question(Some(question(9)), cx));
        cx.run_until_parked();
        cx.simulate_click(point(card.left() + card.size.width * 0.25, above), Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 9 Never".to_string()));
    }
}
