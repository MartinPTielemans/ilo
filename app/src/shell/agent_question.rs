//! An agent's question: what it would do that asks the user first, such as an order, a message
//! sent or a bank's site, in a card under the URL field of every window, where a page's question
//! shows, until the user answers. The agent works in tabs the user isn't looking at, so the card
//! doesn't wait for its tab to be shown. A page's own question goes first, as it's about the page
//! in front of the user. The Agents keep what waits on the answer (agents::Agents::answer). A
//! handoff asks the same way: the agent needs the user in a tab, which Show Tab shows and Hand Back
//! gives back. So does an agent asking for a tab the user took over: Keep, or Hand Back. An agent's
//! HTTP requests that may change something ask each time, listing the changes: Deny, or Allow.
//! An ask to point (#2352) asks in the agent's words: Not Now, or Point, which starts pointing in
//! the tab in front of the user, and dims, saying why on hover, where it can't.

use gpui::{Context, FontWeight, div, prelude::*, px};

use crate::tokens::foot_bar::DISABLED;

use super::{
    Shell, TITLE,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener},
};
use crate::agents::{Answer, Question};
use crate::tokens::shell::permission::{BUTTON_GAP, CHANGE_GAP, DETAIL};

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

    /// The agent's question the card shows, unless a page's question or chooser has the place.
    pub(super) fn agent_question(&self) -> Option<&Question> {
        self.agent_question.as_ref().filter(|_| !self.page_asks())
    }

    /// Not Now on an ask to point: the agent hears nothing was pointed at.
    fn point_unanswered(&mut self, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            // check.sh reads this line.
            log!("browser: point {} not now", question.id);
            self.browser.answer_point(question.id, None, cx);
            cx.notify();
        }
    }

    fn answer_agent(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            self.browser.answer_agent(question.id, answer, cx);
            cx.notify();
        }
    }

    /// The agent's question, as a card under the URL field.
    pub(super) fn agent_question_card(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Some(question) = self.agent_question() else {
            // Drawn again, it's logged again.
            self.agent_question_drawn.set(None);
            return None;
        };
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        // Point can't start in every tab, which the user learns here, never the agent.
        let can_point = match question.point {
            true => self.may_point_here(&question.from),
            false => 0,
        };
        let buttons = buttons(question).into_iter().map(|button| {
            let row = button.look == Look::Text;
            let dim = button.does == Does::Point && can_point != 0;
            card_button_as(&self.palette, button.id, button.label, button.look)
                .when(row, |each| each.flex_basis(gpui::relative(1.)))
                // Dimmed, it stays where it is and says why on hover.
                .when(dim, |each| {
                    each.opacity(DISABLED)
                        .child(self.button_frame(button.id))
                        .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                            let frame = shell.button_frames.borrow().get(button.id).copied();
                            shell.point_tip(frame, *hovered, can_point, cx)
                        }))
                })
                .on_click(cx.listener(move |shell, _, _, cx| match button.does {
                    Does::Answer(answer) => shell.answer_agent(answer, cx),
                    Does::Show(tab) => shell.select(tab, cx),
                    Does::HandBack(tab) => shell.browser.hand_back(tab, cx),
                    Does::NotNow => shell.point_unanswered(cx),
                    Does::Point => {
                        if let Some(question) = shell.agent_question.clone() {
                            shell.press_point(question.id, &question.from, cx);
                        }
                    }
                }))
        });
        let title = match (question.handoff, question.back) {
            _ if question.point => format!("{} asks", question.agent),
            (Some(_), true) => format!("{} asks for a tab", question.agent),
            (Some(_), false) => format!("{} needs you", question.agent),
            (None, _) => format!("{} wants to {}", question.agent, question.what),
        };
        // The agent's question, in its words, under who asks.
        let asked = question.point.then(|| {
            div()
                .text_color(self.palette.text(TITLE))
                .child(question.what.clone())
        });
        let close = (question.handoff.is_none() && !question.point).then(|| {
            card_close(&self.palette, "agent-question-close")
                .on_click(cx.listener(|shell, _, _, cx| shell.answer_agent(Answer::Dismiss, cx)))
        });
        // A request's changes, which ask every time: what each does, and Allow is for them alone.
        let changes = question.changes.iter().map(|(line, sends)| {
            let change = div()
                .mt(px(CHANGE_GAP))
                .text_color(self.palette.on_overlay(DETAIL))
                .child(line.clone());
            change.when(!sends.is_empty(), |change| {
                change.child(div().child(sends.clone()))
            })
        });
        let more = (question.more > 0).then(|| {
            div()
                .mt(px(CHANGE_GAP))
                .text_color(self.palette.on_overlay(DETAIL))
                .child(format!("and {} more", question.more))
        });
        Some(
            self.plate("agent-question")
                .child(listener)
                .child(
                    div()
                        .when(close.is_some(), |title| title.pr(px(BESIDE_CLOSE)))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children(asked)
                .children(changes)
                .children(more)
                .child(
                    div()
                        .text_color(self.palette.on_overlay(DETAIL))
                        .child(question.why.clone()),
                )
                // Three labels don't fit a row of the sidebar at its default width, so the last
                // takes a row of its own, as wide as the card, rather than running past its edge.
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .flex_wrap()
                        .gap(px(BUTTON_GAP))
                        .children(buttons),
                )
                .children(close)
                .into_any_element(),
        )
    }
}

/// What a card's button does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Does {
    Answer(Answer),
    /// Shows the agent's tab, where the user does the step it handed them.
    Show(u64),
    /// Gives the tab back to the agent.
    HandBack(u64),
    /// An ask to point's Not Now and Point.
    NotNow,
    Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Button {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) look: Look,
    does: Does,
}

/// The card's buttons, in order: the answer that leaves things as they are on the quiet plate, then
/// the answer the agent waits on, the one primary, on the right as the dialog keeps its action;
/// and for a confirmation Always Allow, which reaches further than it looks, as its words alone on
/// a row of their own.
pub(super) fn buttons(question: &Question) -> Vec<Button> {
    let button = |id, label, look, does| Button {
        id,
        label,
        look,
        does,
    };
    match question.handoff {
        _ if question.point => vec![
            button("agent-not-now", "Not Now", Look::Quiet, Does::NotNow),
            button("agent-point", "Point", Look::Primary, Does::Point),
        ],
        Some(tab) if question.back => vec![
            button(
                "agent-keep",
                "Keep",
                Look::Quiet,
                Does::Answer(Answer::Never),
            ),
            button(
                "agent-handoff-back",
                "Hand Back",
                Look::Primary,
                Does::HandBack(tab),
            ),
        ],
        Some(tab) => vec![
            button(
                "agent-handoff-back",
                "Hand Back",
                Look::Quiet,
                Does::HandBack(tab),
            ),
            button(
                "agent-handoff-show",
                "Show Tab",
                Look::Primary,
                Does::Show(tab),
            ),
        ],
        // A request's changes: Allow sends them alone, so there's no Always.
        None if !question.changes.is_empty() => vec![
            button(
                "agent-question-deny",
                "Deny",
                Look::Quiet,
                Does::Answer(Answer::Dismiss),
            ),
            button(
                "agent-question-allow",
                "Allow",
                Look::Primary,
                Does::Answer(Answer::Once),
            ),
        ],
        None => vec![
            button(
                "agent-question-never",
                "Never",
                Look::Quiet,
                Does::Answer(Answer::Never),
            ),
            button(
                "agent-question-once",
                "Allow Once",
                Look::Primary,
                Does::Answer(Answer::Once),
            ),
            button(
                "agent-question-always",
                "Always Allow",
                Look::Text,
                Does::Answer(Answer::Always),
            ),
        ],
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
            ..Question::default()
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
            let page = Page {
                url: "https://example.com/".into(),
                ..Page::default()
            };
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
        cx.simulate_click(
            point(card.left() + card.size.width * 0.75, above),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 7 Once".to_string()));
        shell.read_with(cx, |shell, _| assert!(shell.agent_question().is_none()));

        shell.update(cx, |shell, cx| {
            let asked = PageQuestion {
                site: "example.com".into(),
                permissions: Vec::new(),
            };
            shell.set_question(10, Some(asked), cx);
            shell.set_agent_question(Some(question(8)), cx);
            assert!(
                shell.agent_question().is_none(),
                "the agent's card covered the page's"
            );
            shell.set_question(10, None, cx);
            assert_eq!(shell.agent_question().map(|question| question.id), Some(8));
        });
        cx.run_until_parked();
        let (card, last, above) = rows(&shell, cx);
        cx.simulate_click(
            point(card.left() + px(PADDING + 4.), last),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 8 Always".to_string()));

        shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(question(9)), cx)
        });
        cx.run_until_parked();
        cx.simulate_click(
            point(card.left() + card.size.width * 0.25, above),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 9 Never".to_string()));
    }

    /// A request's changes ask every time, so their card has Deny and Allow side by side in its
    /// last row, and nothing to answer for good above it.
    #[gpui::test]
    fn a_requests_card_allows_once_or_denies(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let changes = |id: u64| Question {
            what: "make 4 changes on team.example.com".into(),
            why: "Changes ask every time.".into(),
            changes: vec![
                ("PATCH /api/items/1".into(), "status: Closed".into()),
                ("PATCH /api/items/2".into(), "status: Closed".into()),
                ("DELETE /api/items/7".into(), "".into()),
            ],
            more: 1,
            ..question(id)
        };
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
            shell.set_agent_question(Some(changes(7)), cx);
        });
        cx.run_until_parked();
        let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
        let card = card.expect("the card wasn't drawn");
        let last = card.bottom() - px(PADDING + BUTTON_HEIGHT / 2.);
        let above = last - px(BUTTON_HEIGHT + BUTTON_GAP);
        cx.simulate_click(
            point(card.left() + card.size.width * 0.75, above),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(
            !host
                .take()
                .iter()
                .any(|did| did.starts_with("answer agent")),
            "above answered"
        );
        cx.simulate_click(
            point(card.left() + card.size.width * 0.75, last),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 7 Once".to_string()));

        shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(changes(8)), cx)
        });
        cx.run_until_parked();
        cx.simulate_click(
            point(card.left() + card.size.width * 0.25, last),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 8 Dismiss".to_string()));
    }
}
