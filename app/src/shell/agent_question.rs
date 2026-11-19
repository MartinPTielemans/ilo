//! An agent's question: what it would do that asks the user first, such as an order, a message
//! sent or a bank's site, in a card under the URL field of every window, where a page's question
//! shows, until the user answers. The agent works in tabs the user isn't looking at, so the card
//! doesn't wait for its tab to be shown. A page's own question goes first, as it's about the page
//! in front of the user. The Agents keep what waits on the answer (agents::Agents::answer). A
//! handoff asks the same way: the agent needs the user in a tab, which Show Tab shows and Hand Back
//! gives back. So does an agent asking for a tab the user took over: Keep, or Hand Back. An agent's
//! HTTP requests that may change something ask each time (#3548): each change in the agent's own
//! words, marked as its, over what the browser knows from its method alone, so a delete reads as
//! one whatever the words say; the whole request one press away; Deny, or Allow.

use std::rc::Rc;

use gpui::{Context, FontWeight, Window, accesskit::Role, div, prelude::*, px};

use super::{
    Shell, TITLE,
    permission::{
        BESIDE_CLOSE, BUTTONS_TOP, Look, card_button_as, card_close, card_listener, part_listener,
    },
};
use crate::agent_request::{self, Closed};
use crate::agents::request::{self, Does as Kind, Request};
use crate::agents::{Answer, Question};
use crate::palette::Tone;
use crate::tokens::menu::{DARK_DESTRUCTIVE, LIGHT_DESTRUCTIVE};
use crate::tokens::shell::PLATE_INSET;
use crate::tokens::shell::permission::{
    BUTTON_GAP, CHANGE_TOP, DETAIL, FACTS_LINE, FACTS_TOP, NUMBER, SEE_RING, SEE_TOP,
};
use crate::tokens::text;

/// The most a request's card has around its changes: its head, why it asks, "See the whole
/// request" and its buttons, with its padding, at the sidebar's least width. The changes scroll in
/// what's left, so those stay in view.
const AROUND_CHANGES: f32 = 190.;

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
        let was = self.agent_question.as_ref().map(|old| old.id);
        if let Some(asked) = question.as_ref().filter(|new| Some(new.id) != was) {
            // check.sh reads these lines: each change as the card shows it, the agent's words
            // and then the browser's.
            for (at, change) in asked.changes.iter().enumerate() {
                log!(
                    "browser: agent question {} change {}: “{}” {}",
                    asked.id,
                    at + 1,
                    request::shown(&request::hide_secrets(&change.what, false).0),
                    change.kind().facts(&asked.site)
                );
            }
        }
        self.agent_question = question;
        // The whole request goes with its question.
        let keep = self.agent_question.as_ref().map(|question| question.id);
        agent_request::close(keep, cx);
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

    fn answer_agent(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            self.browser.answer_agent(question.id, answer, cx);
            // The whole request goes with its answer: the Agents' None after it changes nothing.
            agent_request::close(None, cx);
            cx.notify();
        }
    }

    /// The agent's question, as a card under the URL field.
    pub(super) fn agent_question_card(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        // The whole request shows only beside its question's card, so one a page's question covers
        // goes, as one gone or answered does.
        let (window, keep) = (self.browser.window(), self.agent_question().map(|q| q.id));
        if agent_request::showing_over(cx)
            .is_some_and(|(over, question)| over == window && Some(question) != keep)
        {
            cx.defer(move |cx| agent_request::follow(window, keep, cx));
        }
        let Some(question) = self.agent_question() else {
            // Drawn again, it's logged again.
            self.agent_question_drawn.set(None);
            return None;
        };
        if !question.changes.is_empty() {
            return Some(self.request_card(question, cx));
        }
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        let buttons = buttons(question).into_iter().map(|button| {
            let row = button.look == Look::Text;
            card_button_as(&self.palette, button.id, button.label, button.look)
                .when(row, |button| button.flex_basis(gpui::relative(1.)))
                .on_click(cx.listener(move |shell, _, _, cx| match button.does {
                    Does::Answer(answer) => shell.answer_agent(answer, cx),
                    Does::Show(tab) => shell.select(tab, cx),
                    Does::HandBack(tab) => shell.browser.hand_back(tab, cx),
                }))
        });
        let title = match (question.handoff, question.back) {
            (Some(_), true) => format!("{} asks for a tab", question.agent),
            (Some(_), false) => format!("{} needs you", question.agent),
            (None, _) => format!("{} wants to {}", question.agent, question.what),
        };
        let close = question.handoff.is_none().then(|| {
            card_close(&self.palette, "agent-question-close")
                .on_click(cx.listener(|shell, _, _, cx| shell.answer_agent(Answer::Dismiss, cx)))
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

    /// The card of an agent's requests that may change something: the agent's own sentence for
    /// each, quoted under its name, over the browser's facts from the method; why it asks; the
    /// whole request; Deny or Allow.
    fn request_card(&self, question: &Question, cx: &mut Context<Self>) -> gpui::AnyElement {
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        let (detail, ink) = (self.palette.on_overlay(DETAIL), self.palette.text(TITLE));
        let red: gpui::Hsla = match self.palette.tone == Tone::Light {
            true => gpui::rgba(LIGHT_DESTRUCTIVE).into(),
            false => gpui::rgba(DARK_DESTRUCTIVE).into(),
        };
        let a11y = self.a11y;
        // A client's name is its own words too.
        let agent = request::plain(&question.agent);
        let small = |line: gpui::Div| line.text_size(px(text::SMALL)).line_height(px(FACTS_LINE));
        let facts = |change: &Request, alone: bool| {
            let kind = change.kind();
            let words = match alone {
                true => kind.facts(&question.site),
                false => kind.short_facts().to_string(),
            };
            let line = small(div().mt(px(FACTS_TOP)));
            match kind {
                Kind::Deletes => line.text_color(red).font_weight(FontWeight::MEDIUM),
                _ => line.text_color(detail),
            }
            .child(words)
        };
        let quoted = |change: &Request| {
            div()
                .text_color(ink)
                .child(format!("“{}”", request::shown(&change.what)))
        };
        let said = |change: &Request, alone: bool| {
            let facts = match alone {
                true => change.kind().facts(&question.site),
                false => change.kind().short_facts().to_string(),
            };
            // The facts are the browser's own, and said so, so a sentence can't pass for them.
            format!(
                "{agent} says: {}. {} sees: {facts}.",
                change.what.trim_end_matches('.'),
                env!("APP_NAME")
            )
        };
        let head = |words: String| {
            small(div())
                .pr(px(BESIDE_CLOSE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(detail)
                .child(words)
        };
        let count = question.changes.len();
        let (head, changes) = match question.changes.as_slice() {
            [one] => (
                head(format!("{agent} says")),
                vec![
                    div()
                        .id(("agent-change", 0usize))
                        .mt(px(2.))
                        .child(quoted(one))
                        .child(facts(one, true))
                        .when(a11y, |change| {
                            change.role(Role::Group).aria_label(said(one, true))
                        }),
                ],
            ),
            many => (
                head(format!(
                    "{agent} asks for {count} changes on {}",
                    question.site
                )),
                (many.iter().enumerate())
                    .map(|(at, change)| {
                        div()
                            .id(("agent-change", at))
                            .mt(px(CHANGE_TOP))
                            .flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .w(px(NUMBER))
                                    .flex_none()
                                    .text_color(detail)
                                    .child(format!("{}", at + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(quoted(change))
                                    .child(facts(change, false)),
                            )
                            .when(a11y, |row| {
                                let spoken =
                                    format!("{} of {count}. {}", at + 1, said(change, false));
                                row.role(Role::Group).aria_label(spoken)
                            })
                    })
                    .collect(),
            ),
        };
        let (top, _) = self.plate_at();
        let room = f32::from(self.viewport.height) - top - PLATE_INSET - AROUND_CHANGES;
        let changes = div()
            .id("agent-question-changes")
            .max_h(px(room.max(3. * FACTS_LINE)))
            .overflow_y_scroll()
            .children(changes);
        let showing = agent_request::showing(cx) == Some(question.id);
        let see = div()
            .id("agent-request-see")
            .mt(px(SEE_TOP))
            .track_focus(&self.agent_request_see)
            .focus_visible(crate::settings_rows::ring(self.palette.accent.into()))
            .rounded(px(SEE_RING))
            .text_color(self.palette.accent)
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .relative()
            .child(part_listener(
                "agent request see".into(),
                &self.agent_request_see_drawn,
                question.id,
            ))
            .child("See the whole request")
            .when(a11y, |see| {
                crate::settings_rows::named_button(see, "See the whole request")
                    .aria_expanded(showing)
            })
            .on_click(cx.listener(|shell, _, window, cx| shell.see_whole_request(window, cx)));
        let buttons = buttons(question).into_iter().map(|button| {
            card_button_as(&self.palette, button.id, button.label, button.look)
                .when(a11y, |control| {
                    crate::settings_rows::named_button(control, button.label)
                })
                .on_click(cx.listener(move |shell, _, _, cx| match button.does {
                    Does::Answer(answer) => shell.answer_agent(answer, cx),
                    _ => {}
                }))
        });
        let close = card_close(&self.palette, "agent-question-close")
            .when(a11y, |close| {
                crate::settings_rows::named_button(close, "Close")
            })
            .on_click(cx.listener(|shell, _, _, cx| shell.answer_agent(Answer::Dismiss, cx)));
        let named = format!("{agent} asks to {}", question.what);
        self.plate("agent-question")
            .child(listener)
            .when(a11y, |card| card.role(Role::AlertDialog).aria_label(named))
            .child(head)
            .child(changes)
            .child(
                div()
                    .mt(px(SEE_TOP))
                    .text_color(detail)
                    .child(question.why.clone()),
            )
            .child(see)
            .child(
                div()
                    .mt(px(BUTTONS_TOP))
                    .flex()
                    .gap(px(BUTTON_GAP))
                    .children(buttons),
            )
            .child(close)
            .into_any_element()
    }

    /// Shows the agent's request whole, beside its card, with the keyboard; Escape there gives it
    /// back to "See the whole request".
    fn see_whole_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(question) = self.agent_question() else {
            return;
        };
        let Some(card) = self.agent_question_drawn.get() else {
            return;
        };
        let left = agent_request::beside(
            f32::from(card.left()),
            f32::from(card.right()),
            self.right,
            PLATE_INSET,
        );
        let top = f32::from(card.top());
        let room = agent_request::room(top, f32::from(window.viewport_size().height));
        let shell = cx.entity().downgrade();
        let handle = window.window_handle();
        let closed = Rc::new(move |how: Closed, cx: &mut gpui::App| {
            if how != Closed::Escape {
                return;
            }
            let shell = shell.clone();
            _ = handle.update(cx, |_, window, cx| {
                _ = shell.update(cx, |shell, cx| {
                    // From the page too, if it had the keyboard before the request was opened.
                    shell.browser.focus_ui(cx);
                    window.focus(&shell.agent_request_see, cx);
                    // check.sh reads this line.
                    log!("browser: agent request keyboard back on see");
                    cx.notify();
                });
            });
        });
        agent_request::show(
            agent_request::Shown {
                window: self.browser.window(),
                question: question.id,
                agent: request::plain(&question.agent).into(),
                site: question.site.clone(),
                changes: question.changes.clone(),
                left,
                top,
                room,
                closed,
            },
            cx,
        );
        cx.notify();
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
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext, point, px};

    use super::BUTTONS_TOP;
    use crate::agent_request;
    use crate::agents::{Question, request::Request};
    use crate::shell::{Folders, Page, Question as PageQuestion, Shell, recorder::Recorder};
    use crate::tokens::shell::permission::{BUTTON_GAP, BUTTON_HEIGHT, LINE, PADDING};

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

    fn change(method: &str, path: &str, what: &str) -> Request {
        Request {
            method: method.into(),
            does: method.into(),
            url: format!("https://team.example.com{path}"),
            path: path.into(),
            headers: Vec::new(),
            body: String::new(),
            what: what.into(),
        }
    }

    fn requests(id: u64, changes: Vec<Request>) -> Question {
        Question {
            what: "make a change on team.example.com".into(),
            why: "Changes ask every time.".into(),
            site: "team.example.com".into(),
            changes,
            ..question(id)
        }
    }

    fn window_with(
        cx: &mut TestAppContext,
    ) -> (
        Rc<Recorder>,
        gpui::Entity<Shell>,
        &mut gpui::VisualTestContext,
    ) {
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
        (host, shell, cx)
    }

    /// A request's changes ask every time, so their card has Deny and Allow side by side in its
    /// last row, and nothing above it answers.
    #[gpui::test]
    fn a_requests_card_allows_once_or_denies(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let changes = |id: u64| {
            requests(
                id,
                vec![
                    change("PATCH", "/api/items/1", "Close item 1."),
                    change("PATCH", "/api/items/2", "Close item 2."),
                    change("DELETE", "/api/items/7", "Tidy up the old draft."),
                ],
            )
        };
        shell.update(cx, |shell, cx| {
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

    /// Readers hear the card as an alert named by what the agent asks, then each change as a group:
    /// the agent's whole sentence, not the card's clip, and the browser's facts, said as its own.
    /// Then "See the whole request", collapsed, Deny and Allow, and Close (#3548).
    #[gpui::test]
    fn readers_hear_each_change_whole_in_the_cards_order(cx: &mut TestAppContext) {
        use gpui::accesskit::{NodeId, Role};
        let (_, shell, cx) = window_with(cx);
        let long = "Close item 4127 as a duplicate of 4130, keeping its comments and its watchers, \
            and say so on 4130 so whoever reads it next knows where the history went.";
        assert!(long.chars().count() > 120, "the card would show it whole");
        let read = |cx: &mut gpui::VisualTestContext| {
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let nodes: std::collections::HashMap<NodeId, gpui::accesskit::Node> =
                tree.nodes.into_iter().collect();
            let (alert, _) = (nodes.iter())
                .find(|(_, node)| node.role() == Role::AlertDialog)
                .expect("the card isn't an alert");
            let mut heard = vec![format!(
                "{:?} {}",
                Role::AlertDialog,
                nodes[alert].label().unwrap_or("")
            )];
            let mut next: Vec<NodeId> = nodes[alert].children().iter().rev().copied().collect();
            while let Some(id) = next.pop() {
                let node = &nodes[&id];
                if let (Role::Group | Role::Button, Some(label)) = (node.role(), node.label()) {
                    let expanded = match node.is_expanded() {
                        Some(true) => ", expanded",
                        Some(false) => ", collapsed",
                        None => "",
                    };
                    heard.push(format!("{:?} {label}{expanded}", node.role()));
                }
                next.extend(node.children().iter().rev().copied());
            }
            heard
        };

        shell.update(cx, |shell, cx| {
            let one = change("PATCH", "/api/items/4127", long);
            shell.set_agent_question(Some(requests(7, vec![one])), cx);
        });
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let (whole, app) = (long.trim_end_matches('.'), env!("APP_NAME"));
        assert_eq!(
            read(cx),
            [
                "AlertDialog Claude Code asks to make a change on team.example.com".to_string(),
                format!(
                    "Group Claude Code says: {whole}. {app} sees: Changes something on team.example.com."
                ),
                "Button See the whole request, collapsed".to_string(),
                "Button Deny".to_string(),
                "Button Allow".to_string(),
                "Button Close".to_string(),
            ]
        );

        shell.update(cx, |shell, cx| {
            let changes = vec![
                change("PATCH", "/api/items/1", "Close item 1."),
                change("DELETE", "/api/items/7", "Tidy up the old draft."),
            ];
            shell.set_agent_question(Some(requests(8, changes)), cx);
        });
        cx.run_until_parked();
        let heard = read(cx);
        assert_eq!(
            heard[1..3],
            [
                format!(
                    "Group 1 of 2. Claude Code says: Close item 1. {app} sees: Changes something."
                ),
                format!(
                    "Group 2 of 2. Claude Code says: Tidy up the old draft. {app} sees: Deletes \
                     something."
                ),
            ]
        );
    }

    #[derive(Default)]
    struct Shown(RefCell<Vec<String>>);

    impl agent_request::Host for Shown {
        fn show(&self, window: u64, _: f32, _: f32, _: f32) {
            self.0.borrow_mut().push(format!("show over {window}"));
        }
        fn set_height(&self, _: f32) {}
        fn hide(&self) {
            self.0.borrow_mut().push("hide".into());
        }
    }

    /// "See the whole request" shows it with the keyboard, and Escape there closes it and gives
    /// the keyboard back to the button: the question is still asked, and nothing was sent.
    #[gpui::test]
    fn escape_in_the_whole_request_closes_it_and_answers_nothing(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let shown = Rc::new(Shown::default());
        let whole = {
            let shown = shown.clone();
            cx.update(|_, cx| agent_request::open(shown, cx))
        };
        let whole = whole.expect("the whole request's window didn't open");
        shell.update(cx, |shell, cx| {
            let one = change("DELETE", "/api/items/7", "Tidy up the old draft.");
            shell.set_agent_question(Some(requests(7, vec![one])), cx);
        });
        cx.run_until_parked();
        let see = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
        let card = see.expect("the card wasn't drawn");
        // "See the whole request" is the row over the buttons' row.
        let row = card.bottom() - px(PADDING + BUTTON_HEIGHT + BUTTONS_TOP + LINE / 2.);
        cx.simulate_click(
            point(card.left() + px(PADDING + 8.), row),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), Some(7));
        assert!(
            shown
                .0
                .borrow()
                .iter()
                .any(|did| did.starts_with("show over"))
        );

        whole
            .update(cx, |_, window, cx| {
                window.dispatch_action(Box::new(agent_request::Dismiss), cx)
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), None);
        assert!(shown.0.borrow().contains(&"hide".to_string()));
        assert!(
            !host
                .take()
                .iter()
                .any(|did| did.starts_with("answer agent")),
            "Escape answered the question"
        );
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.agent_question().map(|question| question.id), Some(7))
        });
        let back = shell.read_with(cx, |shell, _| shell.agent_request_see.clone());
        cx.update(|window, _| assert!(back.is_focused(window), "the keyboard isn't on See"));

        // Answered, the question takes the whole request with it.
        cx.simulate_click(
            point(card.left() + px(PADDING + 8.), row),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), Some(7));
        shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), None);
    }

    /// The whole request shows only beside its question's card: it goes when the question is
    /// answered, refused by the 5-minute rule (the Agents ask the next, or nothing), gone with its
    /// client, or covered by a page's question taking the card's place, though the Agents say
    /// nothing more of it after an answer.
    #[gpui::test]
    fn the_whole_request_goes_with_its_card(cx: &mut TestAppContext) {
        let (host, shell, cx) = window_with(cx);
        let shown = Rc::new(Shown::default());
        let opened = {
            let shown = shown.clone();
            cx.update(|_, cx| agent_request::open(shown, cx))
        };
        opened.expect("the whole request's window didn't open");
        let open = |id: u64, cx: &mut gpui::VisualTestContext| {
            shell.update(cx, |shell, cx| {
                let one = change("DELETE", "/api/items/7", "Tidy up the old draft.");
                shell.set_agent_question(Some(requests(id, vec![one])), cx);
            });
            cx.run_until_parked();
            shell.update_in(cx, |shell, window, cx| shell.see_whole_request(window, cx));
            cx.run_until_parked();
            assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), Some(id));
        };
        let card = |cx: &mut gpui::VisualTestContext| {
            let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
            card.expect("the card wasn't drawn")
        };

        // Answered: Deny, with the whole request open, and the Agents' None that follows.
        open(7, cx);
        let card_at = card(cx);
        let last = card_at.bottom() - px(PADDING + BUTTON_HEIGHT / 2.);
        cx.simulate_click(
            point(card_at.left() + card_at.size.width * 0.25, last),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 7 Dismiss".to_string()));
        assert_eq!(
            cx.update(|_, cx| agent_request::showing(cx)),
            None,
            "answered"
        );
        shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
        cx.run_until_parked();

        // Refused by the 5-minute rule, as the Agents ask the next question in its place.
        open(8, cx);
        shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(question(9)), cx)
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| agent_request::showing(cx)),
            None,
            "refused"
        );

        // Gone with its client, as the Agents ask nothing.
        open(10, cx);
        shell.update(cx, |shell, cx| shell.set_agent_question(None, cx));
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| agent_request::showing(cx)), None, "gone");

        // A page's question takes the card's place, and the agent's is still asked.
        open(11, cx);
        shell.update(cx, |shell, cx| {
            let asked = PageQuestion {
                site: "example.com".into(),
                permissions: Vec::new(),
            };
            shell.set_question(10, Some(asked), cx);
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| agent_request::showing(cx)),
            None,
            "covered"
        );
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.agent_question.as_ref().map(|q| q.id), Some(11))
        });
        let hides = shown.0.borrow().iter().filter(|did| *did == "hide").count();
        assert_eq!(hides, 4);
    }

    /// With the sidebar on the right, the whole request opens to the card's left, toward the page,
    /// as the theme picker does; on the left, past its right.
    #[test]
    fn the_whole_request_opens_toward_the_page() {
        use crate::tokens::shell::PLATE_INSET;
        let (left, right) = (900., 1140.);
        let on_left = agent_request::beside(20., 260., false, PLATE_INSET);
        assert_eq!(on_left, 260. + 2. * PLATE_INSET);
        let on_right = agent_request::beside(left, right, true, PLATE_INSET);
        assert_eq!(on_right + agent_request::WIDTH, left - 2. * PLATE_INSET);
    }
}
