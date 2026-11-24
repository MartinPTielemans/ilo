//! An agent's question: what it would do that asks the user first, such as an order, a message
//! sent or a bank's site, in a card under the URL field of every window, where a page's question
//! shows, until the user answers. The agent works in tabs the user isn't looking at, so the card
//! doesn't wait for its tab to be shown. A page's own question goes first, as it's about the page
//! in front of the user. The Agents keep what waits on the answer (agents::Agents::answer). A
//! handoff asks the same way: the agent needs the user in a tab, which Show Tab shows and Hand Back
//! gives back. So does an agent asking for a tab the user took over: Keep, or Hand Back. An agent's
//! HTTP requests that may change something ask each time (#3548): each change in the agent's own
//! words, marked as its, over what the browser knows from its method and overrides, so a delete
//! reads as one whatever the words say; Deny, or Allow.

use gpui::{Context, FontWeight, accesskit::Role, div, prelude::*, px};

use super::agent_card_keys::{escape_mark, heard_title};
use super::{
    Shell, TITLE,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, Look, card_listener},
};
use crate::agents::request::{self, Does as Kind, Request};
use crate::agents::{Answer, Question};
use crate::palette::Tone;
use crate::tokens::menu::{DARK_DESTRUCTIVE, LIGHT_DESTRUCTIVE};
use crate::tokens::shell::PLATE_INSET;
use crate::tokens::shell::permission::{
    BUTTON_GAP, CHANGE_ALONE_TOP, CHANGE_TOP, DETAIL, FACTS_LINE, FACTS_TOP, NUMBER, NUMBER_GAP,
    SEE_TOP,
};
use crate::tokens::text;

/// The most a request's card has around its changes: its head, why it asks, "See the whole
/// request" and its buttons, with its padding, at the sidebar's least width. The changes scroll in
/// what's left, so those stay in view.
const AROUND_CHANGES: f32 = 190.;

/// A change's sentence on a card of several, marked as the agent's as the one change's head marks
/// it: "Claude: “Close item 1.”", with the mark's range, which is drawn in the detail.
fn marked(agent: &str, what: &str) -> (String, std::ops::Range<usize>) {
    let mark = format!("{agent}:");
    let end = mark.len();
    (format!("{mark} “{}”", request::shown(what)), 0..end)
}

/// A question card's head and sentence. An action on the tab's page has the tab's own site as
/// its head, "On shop.example", apart from the page's words in the sentence under it, which then
/// can't move it or pass for it (#3836). Other questions have their sentence alone.
fn head_and_title(question: &Question, agent: &str) -> (Option<String>, String) {
    match (question.handoff, question.back) {
        (Some(_), true) => (None, format!("{agent} asks for a tab")),
        (Some(_), false) => (None, format!("{agent} needs you")),
        (None, _) if !question.asks.is_empty() && !question.site.is_empty() => (
            Some(format!("On {}", question.site)),
            format!("{agent} wants to {}", question.asks),
        ),
        (None, _) => (None, format!("{agent} wants to {}", question.what)),
    }
}

impl Shell {
    /// The Agents ask the user `question`, or no longer ask (None).
    pub fn set_agent_question(&mut self, question: Option<Question>, cx: &mut Context<Self>) {
        // The Agents ask again whenever they change. A question's changes are compared by their
        // pointer, never their bodies (Question's PartialEq); its other fields are compared too, as
        // a handoff's agent name follows its Space under the same id.
        if self.agent_question == question {
            return;
        }
        let asked = |old: &&Question| question.as_ref().map(|new| new.id) != Some(old.id);
        if let Some(gone) = self.agent_question.as_ref().filter(asked) {
            // check.sh reads this line, as the agent's client goes with its question unanswered.
            log!("browser: agent question {} gone", gone.id);
        }
        let was = self.agent_question.as_ref().map(|old| old.id);
        if question.as_ref().map(|new| new.id) != was {
            // Another card's measures would place its chips; a new one's first frame has none.
            self.agent_card_room.set(Default::default());
        }
        if let Some(asked) = question.as_ref().filter(|new| Some(new.id) != was) {
            // check.sh reads these lines: each change as the card shows it, the agent's words
            // and then the browser's.
            for (at, change) in asked.changes.iter().enumerate() {
                log!(
                    "browser: agent question {} change {}: “{}” {}",
                    asked.id,
                    at + 1,
                    request::shown(&request::hide_said(&change.what).0),
                    change.kind().facts(&asked.site)
                );
            }
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

    /// What a card's button does, pressed or by its key: one path, so the activity log and the
    /// 30-day file record a key's answer as a click's.
    pub(super) fn press_card(&mut self, does: Does, cx: &mut Context<Self>) {
        match does {
            Does::Answer(answer) => self.answer_agent(answer, cx),
            Does::Show(tab) => self.select(tab, cx),
            Does::HandBack(tab) => self.browser.hand_back(tab, cx),
            Does::Choose => self.choose_agent_file(cx),
        }
    }

    fn answer_agent(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            self.browser.answer_agent(question.id, answer, cx);
            cx.notify();
        }
    }

    /// Choose File… on an agent's question for a page's file field: the card goes, and the
    /// system's file dialog opens over this window.
    fn choose_agent_file(&mut self, cx: &mut Context<Self>) {
        if let Some(question) = self.agent_question.take() {
            self.browser
                .choose_agent_file(question.id, self.browser.window(), cx);
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
        if !question.changes.is_empty() {
            return Some(self.request_card(question, cx));
        }
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        let a11y = self.a11y;
        let escape = escape_mark(question);
        let count = buttons(question).len();
        let buttons = buttons(question)
            .into_iter()
            .enumerate()
            .map(|(at, button)| {
                let row = button.look == Look::Text;
                self.card_key_button(at, count, button, escape, cx)
                    .when(row, |button| button.flex_basis(gpui::relative(1.)))
            });
        let (head, title) = head_and_title(question, &question.agent);
        // Heard in the order it's drawn: the site's line, then the sentence, with the agent's name
        // as plain text, as the request card's.
        let heard = match head_and_title(question, &request::plain(&question.agent)) {
            (Some(head), title) => format!("{} {}", heard_title(&head), heard_title(&title)),
            (None, title) => heard_title(&title),
        };
        // The head is drawn as the request card's is.
        let head = head.map(|head| {
            div()
                .id("agent-question-site")
                .pr(px(BESIDE_CLOSE))
                .text_size(px(text::SMALL))
                .line_height(px(FACTS_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(self.palette.on_overlay(DETAIL))
                .child(self.card_top_line(crate::address_wrap::text(&head)))
        });
        // Under a head, as a request's change is, and clear of × as the head keeps it.
        let headed = head.is_some();
        let close = (question.handoff.is_none()).then(|| self.card_key_close(escape, cx));
        let described = self.agent_card_described(cx);
        Some(
            self.card_keys_on(self.plate("agent-question"), cx)
                .child(listener)
                .when(a11y, |card| {
                    card.child(Self::card_announcer(&heard, described.as_deref()))
                        .role(Role::AlertDialog)
                        .aria_label(heard)
                        .when_some(described, |card, line| card.aria_description(line))
                })
                .children(head)
                .child(
                    div()
                        .id("agent-question-title")
                        .when(headed, |title| title.mt(px(CHANGE_ALONE_TOP)))
                        .when(close.is_some() && !headed, |title| {
                            title.pr(px(BESIDE_CLOSE))
                        })
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .map(|title_line| match headed {
                            true => title_line.child(crate::address_wrap::text(&title)),
                            false => title_line
                                .child(self.card_top_line(crate::address_wrap::text(&title))),
                        }),
                )
                .child(
                    div()
                        .text_color(self.palette.on_overlay(DETAIL))
                        // A handoff's host, such as a sign-in's, breaks between its parts (#3748).
                        .child(crate::address_wrap::text(&question.why)),
                )
                // Three labels don't fit a row of the sidebar at its default width, so the last
                // takes a row of its own, as wide as the card, rather than running past its edge.
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .flex_wrap()
                        .gap(px(BUTTON_GAP))
                        .relative()
                        .child(self.card_room_probe())
                        .children(buttons),
                )
                .children(close)
                .into_any_element(),
        )
    }

    /// The card of an agent's requests that may change something: the agent's own sentence for
    /// each, quoted under its name, over the browser's facts from the method; why it asks; Deny or
    /// Allow.
    fn request_card(&self, question: &Question, cx: &mut Context<Self>) -> gpui::AnyElement {
        let listener = card_listener("agent question", &self.agent_question_drawn, question.id);
        let (detail, ink) = (self.palette.on_overlay(DETAIL), self.palette.text(TITLE));
        let red: gpui::Hsla = match self.palette.tone == Tone::Light {
            true => gpui::rgba(LIGHT_DESTRUCTIVE).into(),
            false => gpui::rgba(DARK_DESTRUCTIVE).into(),
        };
        let a11y = self.a11y;
        #[cfg(test)]
        self.agent_question_rows.borrow_mut().clear();
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
            // The browser's facts come first and the agent's words after, said as theirs, so a
            // sentence that ends in facts of its own is heard after the browser's.
            format!(
                "{} sees: {facts}. {agent} says: {}.",
                env!("APP_NAME"),
                change.what.trim_end_matches('.')
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
        let (head, changes) = match &question.changes[..] {
            [one] => (
                head(format!("{agent} says")),
                vec![
                    div()
                        .id(("agent-change", 0usize))
                        .mt(px(CHANGE_ALONE_TOP))
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
                            .gap(px(NUMBER_GAP))
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
                                    .child({
                                        let (words, mark) = marked(&agent, &change.what);
                                        let quiet = gpui::HighlightStyle {
                                            color: Some(detail),
                                            ..gpui::HighlightStyle::default()
                                        };
                                        let words = gpui::StyledText::new(words)
                                            .with_highlights(vec![(mark.clone(), quiet)]);
                                        #[cfg(test)]
                                        self.agent_question_rows
                                            .borrow_mut()
                                            .push((words.layout().clone(), mark));
                                        div().text_color(ink).child(words)
                                    })
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
        let count = buttons(question).len();
        let escape = escape_mark(question);
        let buttons = buttons(question)
            .into_iter()
            .enumerate()
            .map(|(at, button)| self.card_key_button(at, count, button, escape, cx));
        let close = self.card_key_close(escape, cx);
        let named = heard_title(&format!("{agent} asks to {}", question.what));
        let described = self.agent_card_described(cx);
        self.card_keys_on(self.plate("agent-question"), cx)
            .child(listener)
            .when(a11y, |card| {
                card.child(Self::card_announcer(&named, described.as_deref()))
                    .role(Role::AlertDialog)
                    .aria_label(named)
                    .when_some(described, |card, line| card.aria_description(line))
            })
            .child(head)
            .child(changes)
            .child(
                div()
                    .mt(px(SEE_TOP))
                    .text_color(detail)
                    .child(crate::address_wrap::text(&question.why)),
            )
            .child(
                div()
                    .mt(px(BUTTONS_TOP))
                    .flex()
                    .gap(px(BUTTON_GAP))
                    .relative()
                    .child(self.card_room_probe())
                    .children(buttons),
            )
            .child(close)
            .into_any_element()
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
    /// Opens the system's file dialog for the page's file field the agent asks a file for.
    Choose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Button {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) look: Look,
    pub(super) does: Does,
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
    // The user chooses the file, or doesn't allow it, so there's nothing to allow for good.
    if let Some(file) = question.file {
        let choose = if file.many {
            "Choose Files…"
        } else {
            "Choose File…"
        };
        return vec![
            button(
                "agent-file-deny",
                "Don’t Allow",
                Look::Quiet,
                Does::Answer(Answer::Dismiss),
            ),
            button("agent-file-choose", choose, Look::Primary, Does::Choose),
        ];
    }
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
        // A site the user fenced asks every time it's on (#3724): Never stays, and an Always
        // would pass nothing.
        None if question.fenced => vec![
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
        ],
        // A request's changes, and a payment, ask every time: Allow lets them go alone, so there's
        // no Always.
        None if !question.changes.is_empty() || question.each_time => vec![
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

    use crate::agents::{Answer, Question, request::Request};
    use crate::shell::{Folders, Page, Question as PageQuestion, Shell, recorder::Recorder};
    use crate::tokens::shell::permission::{
        BUTTON_GAP, BUTTON_HEIGHT, CLOSE, CLOSE_INSET, PADDING,
    };

    fn question(id: u64) -> Question {
        Question {
            id,
            agent: "Claude Code".into(),
            from: "claude-code".into(),
            what: "click “Place order” on shop.example".into(),
            asks: "click “Place order”".into(),
            why: "It looks like a payment or an order.".into(),
            site: "shop.example".into(),
            ..Question::default()
        }
    }

    /// An action on the tab's page has the tab's own site as the card's head and its sentence
    /// without it, so a control's name that names another site stays in the sentence (#3836).
    /// A question that names no such site, and a handoff, have no head.
    #[test]
    fn the_tabs_site_is_the_cards_head() {
        assert_eq!(
            super::head_and_title(&question(1), &question(1).agent),
            (
                Some("On shop.example".to_string()),
                "Claude Code wants to click “Place order”".to_string()
            )
        );
        let named = Question {
            what: "click “Pay on bank.example” on shop.example".into(),
            asks: "click “Pay on bank.example”".into(),
            ..question(2)
        };
        assert_eq!(
            super::head_and_title(&named, &named.agent),
            (
                Some("On shop.example".to_string()),
                "Claude Code wants to click “Pay on bank.example”".to_string()
            )
        );
        let download = Question {
            what: "download report.docx from files.example.com".into(),
            asks: "".into(),
            ..question(3)
        };
        assert_eq!(
            super::head_and_title(&download, &download.agent),
            (
                None,
                "Claude Code wants to download report.docx from files.example.com".to_string()
            )
        );
        let handoff = Question {
            handoff: Some(10),
            ..question(4)
        };
        assert_eq!(
            super::head_and_title(&handoff, &handoff.agent),
            (None, "Claude Code needs you".to_string())
        );
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

    /// A question on a site the user fenced has Never and Allow Once side by side in its last row
    /// and no Always Allow, as the fence would pass none (#3724).
    #[gpui::test]
    fn a_fenced_sites_card_has_no_always(cx: &mut TestAppContext) {
        let fenced = |id| Question {
            what: "open mybank.example".into(),
            why: crate::agents::fence::why("mybank.example").into(),
            site: "mybank.example".into(),
            fenced: true,
            ..question(id)
        };
        let labels: Vec<_> = (super::buttons(&fenced(1)).iter())
            .map(|button| button.label)
            .collect();
        assert_eq!(labels, ["Never", "Allow Once"]);
        let (host, shell, cx) = window_with(cx);
        for (id, at, answered) in [(11, 0.75, "Once"), (12, 0.25, "Never")] {
            shell.update(cx, |shell, cx| {
                shell.set_agent_question(Some(fenced(id)), cx)
            });
            cx.run_until_parked();
            let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
            let card = card.expect("the card wasn't drawn");
            let last = card.bottom() - px(PADDING + BUTTON_HEIGHT / 2.);
            cx.simulate_click(
                point(card.left() + card.size.width * at, last),
                Modifiers::none(),
            );
            cx.run_until_parked();
            let answer = format!("answer agent {id} {answered}");
            assert!(host.take().contains(&answer), "no {answer}");
        }
    }

    fn change(method: &str, path: &str, what: &str) -> Request {
        Request {
            method: method.into(),
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
            changes: changes.into(),
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

    fn upload(id: u64, many: bool) -> Question {
        Question {
            what: "add a file to “Résumé” on example.com".into(),
            why: "You choose the file. Claude Code sees only what the page shows of it.".into(),
            site: "example.com".into(),
            each_time: true,
            file: Some(crate::agents::FileAsk { field: 3, many }),
            ..question(id)
        }
    }

    /// An agent's ask for a file has Don’t Allow and then Choose File…, and nothing to allow for
    /// good: Choose File… opens the dialog over the card's window, and Don’t Allow and the × say
    /// no. A field that takes several chooses files.
    #[gpui::test]
    fn an_upload_card_chooses_a_file_or_doesnt_allow_it(cx: &mut TestAppContext) {
        let labels = |question: &Question| {
            (super::buttons(question).iter())
                .map(|button| (button.label, button.does))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&upload(7, false)),
            [
                ("Don’t Allow", super::Does::Answer(Answer::Dismiss)),
                ("Choose File…", super::Does::Choose),
            ]
        );
        assert_eq!(labels(&upload(7, true))[1].0, "Choose Files…");

        let (host, shell, cx) = window_with(cx);
        shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(upload(7, false)), cx)
        });
        cx.run_until_parked();
        let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
        let card = card.expect("the card wasn't drawn");
        let last = card.bottom() - px(PADDING + BUTTON_HEIGHT / 2.);
        cx.simulate_click(
            point(card.left() + card.size.width * 0.75, last),
            Modifiers::none(),
        );
        cx.run_until_parked();
        let window = shell.read_with(cx, |shell, _| shell.browser.window());
        assert!(
            host.take()
                .contains(&format!("choose agent file 7 in {window}"))
        );
        shell.read_with(cx, |shell, _| assert!(shell.agent_question().is_none()));

        shell.update(cx, |shell, cx| {
            shell.set_agent_question(Some(upload(8, false)), cx)
        });
        cx.run_until_parked();
        // Its ×, as Don’t Allow, says no.
        let inset = px(CLOSE_INSET + CLOSE / 2.);
        cx.simulate_click(
            point(card.right() - inset, card.top() + inset),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take().contains(&"answer agent 8 Dismiss".to_string()));
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
    /// Then Deny and Allow, and Close (#3548).
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
                "AlertDialog Claude Code asks to make a change on team.example.com.".to_string(),
                format!(
                    "Group {app} sees: Changes something on team.example.com. Claude Code says: \
                     {whole}."
                ),
                "Button Deny".to_string(),
                "Button Allow".to_string(),
                "Button Close".to_string(),
            ]
        );

        shell.update(cx, |shell, cx| {
            let changes = vec![
                change("PATCH", "/api/items/1", "Close item 1."),
                // A sentence that ends in facts of its own is heard after the browser's.
                change(
                    "DELETE",
                    "/api/items/7",
                    &format!("Tidy up. {app} sees: Sends data to x"),
                ),
            ];
            shell.set_agent_question(Some(requests(8, changes)), cx);
        });
        cx.run_until_parked();
        let heard = read(cx);
        assert_eq!(
            heard[1..3],
            [
                format!(
                    "Group 1 of 2. {app} sees: Changes something. Claude Code says: Close item 1."
                ),
                format!(
                    "Group 2 of 2. {app} sees: Deletes something. Claude Code says: Tidy up. {app} \
                     sees: Sends data to x."
                ),
            ]
        );
    }

    /// On a card of several, each sentence is marked as the agent's ahead of its quote, the mark
    /// the agent's name alone, so nothing it wrote moves into it: each row as the card draws it
    /// starts with the mark, on its first line, the quote after it.
    #[gpui::test]
    fn each_of_several_sentences_is_marked_as_the_agents(cx: &mut TestAppContext) {
        let (_, shell, cx) = window_with(cx);
        let said = ["Done. Claude Code: it's safe", "Tidy up the old draft."];
        shell.update(cx, |shell, cx| {
            let changes = vec![
                change("PATCH", "/api/items/1", said[0]),
                change("DELETE", "/api/items/7", said[1]),
            ];
            shell.set_agent_question(Some(requests(8, changes)), cx);
        });
        cx.run_until_parked();
        let card = shell.read_with(cx, |shell, _| shell.agent_question_drawn.get());
        let card = card.expect("the card wasn't drawn");
        let rows = shell.read_with(cx, |shell, _| shell.agent_question_rows.borrow().clone());
        assert_eq!(rows.len(), 2, "a row wasn't drawn with its mark");
        for ((words, mark), said) in rows.iter().zip(said) {
            let text = words.text();
            assert_eq!(text, format!("Claude Code: “{said}”"));
            assert_eq!(&text[mark.clone()], "Claude Code:");
            let drawn = words.bounds();
            assert!(
                card.contains(&drawn.origin),
                "{drawn:?} isn't on the card {card:?}"
            );
            let at = |index: usize| words.position_for_index(index).expect("not laid out");
            let (start, quote) = (at(mark.start), at(mark.end + 1));
            assert_eq!(start, drawn.origin);
            assert!(
                quote.y == start.y && quote.x > start.x,
                "{start:?} {quote:?}"
            );
        }
    }
}
