//! The keys of an agent's question card (#3869). Nothing answers a card from the page, where
//! someone typing may never have seen it arrive: Focus Agent Question's key (⌥⇧⌘A on the Mac,
//! Alt+Shift+A elsewhere, Chrome's Focus Inactive Popup) moves the keyboard to the card and rings
//! its primary button, and only then do ↩ and Space press the ringed button, Tab and ⇧Tab move
//! along the buttons, and Escape answers as the card's × does. The key again gives the keyboard
//! back where it was and answers nothing. Only the user's keys come here: an agent's go to its page
//! (keys.rs's tab_key_before_page).

use std::time::Duration;

use web_time::Instant;

use gpui::{App, Context, FocusHandle, KeyBinding, Window, actions};

use super::agent_question::{Does, buttons};
use super::permission::Look;
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
            // check.sh waits for this line.
            log!("browser: agent card keys left by page");
            cx.notify();
        }
    }

    /// The button the keyboard rings on the card, if it's there.
    pub(super) fn agent_card_ring(&self) -> Option<usize> {
        self.card_keys.as_ref().map(|keys| keys.at)
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
                    // check.sh waits for this line.
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
        let Some(keys) = &mut self.card_keys else {
            return false;
        };
        let after = now.saturating_duration_since(keys.last);
        keys.last = now;
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
}
