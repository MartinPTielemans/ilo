//! Agent tasks' cards at the top of each Space's Today (Agent task). A card is a tab row two lines
//! tall: the agents' mark in the favicon's column and the task's title in the title's, over what
//! it's doing, waiting for or found. The dot at its end is the Agent Space's, Signal green,
//! breathing while the task works and still while it waits for the user, and it draws into a tick,
//! as Copy URL's check does, once the task is done. While ⌘T asks for a task, Today holds an empty
//! slot at its top, which the card fills as the bar closes. The Agents send a window its cards only
//! when one changed, so a card redraws only with its task.

use std::time::Instant;

use gpui::{AnyElement, Context, MouseButton, SharedString, div, prelude::*, px, rgb, svg};

use super::{
    CLOSE_GLYPH, CLOSE_HOVERED, CLOSE_RIGHT, CLOSE_SIZE, Curve, Edit, FAVICON_SIZE, FAVICON_X,
    HOVERED, LOADING_SIZE, NAV_RADIUS, PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS, ROW_HEIGHT,
    ROW_PITCH, SECONDARY, STATUS_AGENT, STATUS_DOT, Shell, Slot, TITLE_END_INSET, TITLE_GAP, What,
    permission::card_button,
    rows::ringed_dot,
    selected_plate,
    swap::{CHECK, CLOSE, NONE, Swapped},
    title,
};
use crate::agents::task::{Card, Dot, TaskAction};
use crate::command_bar::AGENT_GLYPH;
use crate::tokens::shell::permission::{BUTTON_GAP, BUTTON_HEIGHT};
use crate::tokens::shell::task::{BUTTONS_BOTTOM, BUTTONS_TOP, HEIGHT, LINE, MARK, TITLE_TOP};
use crate::tokens::text;

/// A task's number, from its agent's id, `task-<n>`.
pub(super) fn number(agent: &str) -> u64 {
    agent.strip_prefix("task-").and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// Whether the card has a row of buttons under its status.
fn has_buttons(card: &Card) -> bool {
    !card.answers.is_empty() || card.retry || card.command.is_some() || card.open.is_some()
}

/// A card's height in the list, or the ask's slot's with none, which is a card's without buttons.
fn card_height(card: Option<&Card>) -> f32 {
    let buttons = card.is_some_and(has_buttons);
    HEIGHT + if buttons { BUTTONS_TOP + BUTTON_HEIGHT + BUTTONS_BOTTOM - bottom_padding() } else { 0. }
}

/// The room under a card's status line without buttons.
fn bottom_padding() -> f32 {
    HEIGHT - (ROW_HEIGHT - PLATE_HEIGHT) - TITLE_TOP - 2. * LINE
}

impl Shell {
    /// The Agents' cards for every Space, newest first. One new in the Space the ask's slot is
    /// open in takes the slot, so the rows under it don't move.
    pub fn set_task_cards(&mut self, cards: Vec<Card>, cx: &mut Context<Self>) {
        if cards == self.task_cards {
            return;
        }
        let now = Instant::now();
        for card in &cards {
            let was = self.task_cards.iter().find(|old| old.agent == card.agent);
            if was.is_none_or(|was| (&was.status, was.dot) != (&card.status, card.dot)) {
                // check.sh reads this line.
                log!("browser: task card {} {:?} {}", card.agent, card.dot, card.status);
            }
            // The dot draws into a tick, unless motion is reduced, when the tick is there at once.
            let done = card.dot == Dot::Done && was.is_some_and(|was| was.dot != Dot::Done);
            if done && !cx.reduce_motion() {
                self.swaps.start(Swapped::Task(number(&card.agent)), NONE, CHECK, now);
            }
        }
        let new = |card: &&Card| !self.task_cards.iter().any(|old| old.agent == card.agent);
        if let Some(home) = self.ask_slot
            && cards.iter().filter(new).any(|card| card.home == home)
        {
            self.ask_slot = None;
        }
        self.task_cards = cards;
        // A conversation whose card went, dismissed, goes with it.
        if let Some(agent) = self.talk_agent()
            && !self.task_cards.iter().any(|card| card.agent == agent)
        {
            self.leave_talk(cx);
        }
        self.lay_out_tasks();
        cx.notify();
    }

    /// ⌘T's ask shows over this window, or no longer does: Today opens an empty slot at its top for
    /// the task's card, or closes it.
    pub fn set_ask_slot(&mut self, shown: bool, cx: &mut Context<Self>) {
        let slot = shown.then_some(self.space);
        if slot == self.ask_slot {
            return;
        }
        // check.sh reads this line.
        log!("browser: ask slot {}", shown as u8);
        self.ask_slot = slot;
        self.lay_out_tasks();
        cx.notify();
    }

    /// Puts the ask's slot and the cards at the top of each Space's Today, and slides the rows of
    /// the Space shown to their places.
    fn lay_out_tasks(&mut self) {
        let before = self.layout();
        for space in &mut self.spaces {
            let ask = (self.ask_slot == Some(space.id)).then(|| (Slot::Ask, slot(None)));
            let cards = (self.task_cards.iter().filter(|card| card.home == space.id))
                .map(|card| (Slot::Task(number(&card.agent)), slot(Some(card))));
            space.tasks = ask.into_iter().chain(cards).collect();
        }
        if self.layout() != before {
            self.move_rows(before, Curve::Slide);
        }
    }

    /// The ask's slot and the task cards of the Space shown, for the list, in their places.
    pub(super) fn task_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(space) = self.shown_space() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for &(slot, height) in &space.tasks {
            let row = match slot {
                Slot::Task(n) => {
                    let found = self.task_cards.iter().find(|card| number(&card.agent) == n);
                    match found {
                        Some(card) => self.task_card(card, cx),
                        None => continue,
                    }
                }
                _ => div().into_any_element(),
            };
            rows.push(
                div()
                    .flex_none()
                    .h(px(height))
                    .relative()
                    .top(px(self.slide(slot)))
                    .child(row)
                    .into_any_element(),
            );
        }
        rows
    }

    /// A task's card: a tab row with its status under the title, and its buttons under that.
    fn task_card(&self, card: &Card, cx: &mut Context<Self>) -> AnyElement {
        let agent = card.agent.clone();
        let hovered = self.task_hovered.as_ref() == Some(&agent);
        let end = match hovered {
            true => self.task_close(agent.clone(), cx).into_any_element(),
            false => self.task_dot(card),
        };
        let mark = svg().path(AGENT_GLYPH).size(px(MARK)).text_color(self.palette.text(SECONDARY));
        let status = div()
            .h(px(LINE))
            .min_w_0()
            .truncate()
            .text_size(px(text::SMALL))
            .text_color(self.palette.text(SECONDARY))
            .child(card.status.clone());
        let top_line = div()
            .h(px(LINE))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .ml(px(FAVICON_X - PLATE_INSET))
                    .mr(px(TITLE_GAP))
                    .size(px(FAVICON_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(mark),
            )
            .child(title(&self.palette, card.title.clone(), false))
            .child(end);
        let column = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;
        let plate = div()
            // Its conversation shows, as a selected tab's page does.
            .when(self.talk_agent() == Some(agent.clone()), |plate| {
                selected_plate(&self.palette, plate, 1., 0., false)
            })
            .id(("task-plate", number(&agent)))
            .size_full()
            .pt(px(TITLE_TOP))
            .flex()
            .flex_col()
            .rounded(px(PLATE_RADIUS))
            .when(hovered, |plate| plate.bg(self.palette.plate(HOVERED)))
            .child(top_line)
            .child(div().pl(px(column)).pr(px(TITLE_END_INSET)).child(status))
            .children(self.task_buttons(card, column, cx))
            .on_hover(cx.listener({
                let agent = agent.clone();
                move |shell, hovered: &bool, _, cx| {
                    if *hovered {
                        shell.task_hovered = Some(agent.clone());
                    } else if shell.task_hovered.as_ref() == Some(&agent) {
                        shell.task_hovered = None;
                    }
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |shell, _, _, cx| shell.show_task(agent.clone(), cx)));
        div()
            .size_full()
            .px(px(PLATE_INSET))
            .py(px((ROW_HEIGHT - PLATE_HEIGHT) / 2.))
            .pb(px((ROW_HEIGHT - PLATE_HEIGHT) / 2. + ROW_PITCH - ROW_HEIGHT))
            .child(plate)
            .into_any_element()
    }

    /// The dot at the card's end, where a tab's status dot is: breathing while the task works,
    /// still while it waits for the user, and a tick once it's done.
    fn task_dot(&self, card: &Card) -> AnyElement {
        let dot = match card.dot {
            Dot::Working => ringed_dot(STATUS_AGENT, STATUS_DOT)
                .opacity(self.breath.strength())
                .into_any_element(),
            Dot::Waiting => ringed_dot(STATUS_AGENT, STATUS_DOT).into_any_element(),
            Dot::Done => {
                let what = Swapped::Task(number(&card.agent));
                self.swaps.glyph(what, CHECK, LOADING_SIZE, rgb(STATUS_AGENT).into())
            }
            Dot::None => div().into_any_element(),
        };
        div()
            .flex_none()
            .ml(px(4.))
            .mr(px(TITLE_END_INSET))
            .size(px(LOADING_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .child(dot)
            .into_any_element()
    }

    /// The ✕ under the pointer, as a tab row's, which dismisses the task.
    fn task_close(&self, agent: SharedString, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let color = self.palette.text(SECONDARY).into();
        div()
            .id(("task-close", number(&agent)))
            .flex_none()
            .ml(px(4.))
            .mr(px(CLOSE_RIGHT))
            .size(px(CLOSE_SIZE))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            .child(self.swaps.glyph(Swapped::Task(number(&agent)), CLOSE, CLOSE_GLYPH, color))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                // check.sh reads this line.
                log!("browser: command dismiss-task via sidebar");
                shell.dismiss_task(agent.clone(), cx);
            }))
    }

    /// The card's buttons, in the title's column: the ask's answers, Limit's Retry, Copy for the
    /// command that fixes a failure, or Done's Open <site>.
    fn task_buttons(
        &self,
        card: &Card,
        column: f32,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        if !has_buttons(card) {
            return None;
        }
        let agent = card.agent.clone();
        let n = number(&agent);
        let act = |action, cx: &mut Context<Self>| act(agent.clone(), action, cx);
        let mut buttons = Vec::new();
        for (index, answer) in card.answers.iter().enumerate() {
            let id = ("task-answer", n * 8 + index as u64);
            let action = TaskAction::Answer(answer.clone());
            buttons.push(card_button(&self.palette, id, answer.clone()).on_click(act(action, cx)));
        }
        if card.retry {
            let retry = card_button(&self.palette, ("task-retry", n), "Retry");
            buttons.push(retry.on_click(act(TaskAction::Retry, cx)));
        }
        if let Some(command) = card.command {
            let copy = card_button(&self.palette, ("task-copy", n), "Copy Command");
            buttons.push(copy.on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.copy(command, "Command Copied", cx);
            })));
        }
        if let Some((_, site)) = &card.open {
            let open = card_button(&self.palette, ("task-open", n), format!("Open {site}"));
            buttons.push(open.on_click(act(TaskAction::Open, cx)));
        }
        Some(
            div()
                .mt(px(BUTTONS_TOP))
                .pl(px(column))
                .pr(px(TITLE_END_INSET))
                .flex()
                .gap(px(BUTTON_GAP))
                .children(buttons),
        )
    }

    /// ✕, or ⌘W in its conversation: the task goes, as ⌘Z and ⇧⌘T bring back.
    pub(super) fn dismiss_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        self.task_hovered = None;
        self.browser.task(agent.clone(), TaskAction::Dismiss, cx);
        self.keep(What::DISMISS_TASK, Some(Edit::RestoreTask(agent.clone())), cx);
        self.closed_preview = None;
        self.closed_pane = None;
        self.closed_task = Some(agent);
    }

    /// Clear's part: the Space's done tasks go. Returns their agents.
    pub(super) fn clear_done_tasks(&mut self, cx: &mut Context<Self>) -> Vec<SharedString> {
        let done: Vec<SharedString> = (self.task_cards.iter())
            .filter(|card| card.home == self.space && card.dot == Dot::Done)
            .map(|card| card.agent.clone())
            .collect();
        for agent in &done {
            self.browser.task(agent.clone(), TaskAction::Dismiss, cx);
        }
        done
    }

}

/// A card's button's click, which does `action` to the task.
fn act(
    agent: SharedString,
    action: TaskAction,
    cx: &mut Context<Shell>,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static + use<> {
    let shell = cx.weak_entity();
    move |_, _, cx| {
        cx.stop_propagation();
        let act = |shell: &mut Shell, cx: &mut Context<Shell>| {
            shell.browser.task(agent.clone(), action.clone(), cx);
        };
        shell.update(cx, act).ok();
    }
}

/// A card's place in the list: its height and the gap under it, as a tab row's.
fn slot(card: Option<&Card>) -> f32 {
    card_height(card) + ROW_PITCH - ROW_HEIGHT
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use super::{Card, Dot, card_height, slot};
    use crate::agents::runner::Cli;
    use crate::shell::{
        Command, Folders, PLATE_HEIGHT, PLATE_INSET, ROW_HEIGHT, Shell, Slot, recorder::Recorder,
    };
    use crate::tokens::shell::permission::BUTTON_HEIGHT;
    use crate::tokens::shell::task::{BUTTONS_TOP, LINE, TITLE_TOP};

    fn card(n: u64, dot: Dot) -> Card {
        Card {
            agent: format!("task-{n}").into(),
            home: 1,
            cli: Cli::Claude,
            title: "Find a flight to Lisbon".into(),
            status: "Searching flights".into(),
            dot,
            answers: Vec::new(),
            question: None,
            open: None,
            retry: false,
            command: None,
        }
    }

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
        });
        cx.run_until_parked();
        (shell, host, cx)
    }

    fn top(shell: &Entity<Shell>, slot: Slot, cx: &mut VisualTestContext) -> Option<f32> {
        shell.read_with(cx, |shell, _| shell.layout().get(&slot).copied())
    }

    /// While ⌘T asks for a task, Today opens a slot at its top, and the task's card fills it, so
    /// the tabs under it stay where the slot put them. Without a card, the slot closes again.
    #[gpui::test]
    fn the_ask_opens_a_slot_the_card_fills(cx: &mut TestAppContext) {
        let (shell, _, cx) = shell(cx);
        let first = top(&shell, Slot::Tab(10), cx).unwrap();
        shell.update(cx, |shell, cx| shell.set_ask_slot(true, cx));
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first + slot(None)));
        assert_eq!(top(&shell, Slot::Ask, cx), Some(first));

        shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, Dot::Working)], cx));
        assert_eq!(top(&shell, Slot::Ask, cx), None, "the slot stayed beside the card");
        assert_eq!(top(&shell, Slot::Task(1), cx), Some(first));
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first + slot(None)), "the tabs moved");
        // The bar closing after the card came changes nothing.
        shell.update(cx, |shell, cx| shell.set_ask_slot(false, cx));
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first + slot(None)));

        shell.update(cx, |shell, cx| {
            shell.set_task_cards(Vec::new(), cx);
            shell.set_ask_slot(true, cx);
            shell.set_ask_slot(false, cx);
        });
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first));
    }

    /// A card with buttons is taller by their row, and a card in another Space takes no room.
    #[gpui::test]
    fn a_card_takes_room_in_its_own_space(cx: &mut TestAppContext) {
        let (shell, _, cx) = shell(cx);
        let first = top(&shell, Slot::Tab(10), cx).unwrap();
        let mut limit = card(2, Dot::None);
        limit.retry = true;
        let mut elsewhere = card(3, Dot::Working);
        elsewhere.home = 2;
        shell.update(cx, |shell, cx| shell.set_task_cards(vec![limit.clone(), elsewhere], cx));
        assert!(card_height(Some(&limit)) > card_height(None));
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first + slot(Some(&limit))));
        assert_eq!(top(&shell, Slot::Task(3), cx), None);
    }

    /// ✕ dismisses a task, as ⌘Z brings it back and ⌘⇧Z dismisses it again, and ⇧⌘T brings back
    /// the task dismissed last.
    #[gpui::test]
    fn a_dismissed_task_comes_back(cx: &mut TestAppContext) {
        let (shell, host, cx) = shell(cx);
        shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, Dot::Working)], cx));
        host.take();
        shell.update(cx, |shell, cx| {
            shell.dismiss_task("task-1".into(), cx);
            assert!(shell.undo(cx));
            assert!(shell.redo(cx));
        });
        let done = host.take();
        let tasks: Vec<_> = done.iter().filter(|line| line.starts_with("task ")).collect();
        assert_eq!(tasks, ["task task-1 Dismiss", "task task-1 Restore", "task task-1 Dismiss"]);

        shell.update(cx, |shell, cx| shell.reopen_closed(cx));
        assert!(host.take().contains(&"task task-1 Restore".to_string()));
        // It's back, so ⌘Z has no dismissal of it left to undo.
        shell.update(cx, |shell, cx| shell.undo(cx));
        assert!(!host.take().iter().any(|line| line.starts_with("task ")));
    }

    /// Clear takes the Space's done tasks with its Today tabs and leaves the working ones, and one
    /// ⌘Z brings both back.
    #[gpui::test]
    fn clear_takes_the_done_tasks(cx: &mut TestAppContext) {
        let (shell, host, cx) = shell(cx);
        let cards = vec![card(5, Dot::Done), card(6, Dot::Working)];
        shell.update(cx, |shell, cx| shell.set_task_cards(cards, cx));
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::Clear, "test", cx));
        let cleared = host.take();
        assert!(cleared.contains(&"task task-5 Dismiss".to_string()), "{cleared:?}");
        assert!(cleared.contains(&"clear".to_string()));
        assert!(!cleared.iter().any(|line| line.starts_with("task task-6")));
        shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
        let undone = host.take();
        assert!(undone.contains(&"task task-5 Restore".to_string()), "{undone:?}");
        assert!(undone.contains(&"undo clear".to_string()));
    }

    /// A card's buttons sit in its title's column under its status, and each does its action.
    #[gpui::test]
    fn the_card_buttons_act(cx: &mut TestAppContext) {
        let (shell, host, cx) = shell(cx);
        let mut limit = card(4, Dot::None);
        limit.retry = true;
        shell.update(cx, |shell, cx| {
            shell.set_task_cards(vec![limit], cx);
            // The tabs have slid down under the card.
            shell.slides.clear();
        });
        cx.run_until_parked();
        let (list, card) = shell.read_with(cx, |shell, _| {
            (shell.drawn_list_top(), shell.layout()[&Slot::Task(4)])
        });
        let y = list + card + (ROW_HEIGHT - PLATE_HEIGHT) / 2. + TITLE_TOP + 2. * LINE
            + BUTTONS_TOP + BUTTON_HEIGHT / 2.;
        let x = PLATE_INSET + 40.;
        host.take();
        cx.simulate_click(point(px(x), px(y)), Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"task task-4 Retry".to_string()));
    }
}
