//! Agent tasks' cards at the top of each Space's Today (Agent task). A card is a tab row with a
//! line under its title: the favicon's column holds the task's state, Signal's status dot
//! breathing while it works, ringed and still while it waits for the user, a tick drawing in, as
//! Copy URL's check does, once it's done, and a hollow ring once it stopped. The line says what
//! it's doing, its question, or its result's line, and its chips, the answers, Retry, Copy or Open,
//! sit under that. While ⌘T asks for a task, Today holds an empty slot at its top, which the card
//! fills as the bar closes. The Agents send a window its cards only when one changed, so a card
//! redraws only with its task.

use std::time::Instant;

use gpui::{
    AnyElement, App, Context, FontWeight, MouseButton, SharedString, WindowTextSystem, div,
    prelude::*, px, rgb, svg,
};

use super::{
    CLOSE_GLYPH, CLOSE_HOVERED, CLOSE_RIGHT, CLOSE_SIZE, Curve, Edit, FAVICON_SIZE, FAVICON_X,
    HOVERED, LOADING_SIZE, NAV_RADIUS, PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS, ROW_HEIGHT,
    ROW_PITCH, SECONDARY, STATUS_AGENT, STATUS_DOT, Shell, Slot, TITLE, TITLE_END_INSET,
    TITLE_GAP, What,
    rows::ringed_dot,
    selected_plate,
    swap::{CHECK, NONE, Swapped},
    title,
};
use crate::agents::task::{Card, Dot, TaskAction, open_label};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::URL_FIELD;
use crate::tokens::shell::task::{
    BOTTOM, CHIP_END, CHIP_GAP, CHIP_HEIGHT, CHIP_X, CHIPS_TOP, HOLLOW, HOLLOW_STROKE, LINE,
    STATE_TOP, STATUS_GAP, STATUS_LINE, TOP,
};
use crate::tokens::text;
use crate::tooltip::text_width;

/// A task's number, from its agent's id, `task-<n>`.
pub(super) fn number(agent: &str) -> u64 {
    agent.strip_prefix("task-").and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// The rows a card's chips wrap to, at most.
const CHIP_ROWS: usize = 2;

/// How a card lays out in the sidebar: its status's lines, and its chips, by their place in
/// `chips`, row by row.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Shape {
    lines: usize,
    rows: Vec<Vec<usize>>,
}

/// A chip on a card: its words, and what it does.
#[derive(Clone)]
enum Chip {
    Answer(SharedString),
    Retry,
    Copy(&'static str),
    Open(SharedString),
}

impl Chip {
    fn label(&self) -> SharedString {
        match self {
            Chip::Answer(answer) => answer.clone(),
            Chip::Retry => "Retry".into(),
            Chip::Copy(_) => "Copy".into(),
            Chip::Open(label) => label.clone(),
        }
    }
}

/// A card's chips: the ask's answers, Limit's Retry, Copy for the command that fixes a failure, or
/// Done's Open <site> or Open 3 pages.
fn chips(card: &Card) -> Vec<Chip> {
    let mut chips: Vec<Chip> = card.answers.iter().cloned().map(Chip::Answer).collect();
    if card.retry {
        chips.push(Chip::Retry);
    }
    if let Some(command) = card.command {
        chips.push(Chip::Copy(command));
    }
    if let Some(label) = open_label(&card.open) {
        chips.push(Chip::Open(label));
    }
    chips
}

/// The column the title and the status start on, from the plate's left edge.
const COLUMN: f32 = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;

/// How `card` lays out in a sidebar `width` wide, with `measure` giving how wide a text is at a
/// size and weight. The agent's question takes a second line if it needs one. The chips wrap to
/// two rows; past that, or with an answer wider than the room, none show, and the card's click
/// opens the conversation, where they do. A chip of its own, such as Open, is cut to the room.
fn shape(card: &Card, width: f32, measure: &dyn Fn(&SharedString, f32, FontWeight) -> f32) -> Shape {
    let plate = width - 2. * PLATE_INSET;
    let status = plate - COLUMN - TITLE_END_INSET;
    let lines = match card.asks && measure(&card.status, text::SMALL, FontWeight::NORMAL) > status {
        true => 2,
        false => 1,
    };
    let room = plate - CHIP_END - (COLUMN - CHIP_X);
    let chips = chips(card);
    if chips.len() == 1 && !matches!(chips[0], Chip::Answer(_)) {
        return Shape { lines, rows: vec![vec![0]] };
    }
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut left = room;
    for (index, chip) in chips.iter().enumerate() {
        let wide = measure(&chip.label(), text::SMALL, FontWeight::MEDIUM).ceil() + 2. * CHIP_X;
        if wide > room {
            return Shape { lines, rows: Vec::new() };
        }
        match rows.last_mut() {
            Some(row) if wide + CHIP_GAP <= left => {
                row.push(index);
                left -= wide + CHIP_GAP;
            }
            _ => {
                rows.push(vec![index]);
                left = room - wide;
            }
        }
    }
    if rows.len() > CHIP_ROWS {
        rows.clear();
    }
    Shape { lines, rows }
}

/// A card's plate's height in this shape, or the ask's slot's with none, which is a card's with a
/// line of status and no chips.
fn plate_height(shape: Option<&Shape>) -> f32 {
    let (lines, rows) = shape.map_or((1, 0), |shape| (shape.lines, shape.rows.len()));
    let chips = match rows {
        0 => 0.,
        rows => CHIPS_TOP + rows as f32 * CHIP_HEIGHT + (rows - 1) as f32 * CHIP_GAP,
    };
    TOP + LINE + STATUS_GAP + lines as f32 * STATUS_LINE + chips + BOTTOM
}

/// A card's height in the list, its plate's and the room around it a row's has.
fn card_height(shape: Option<&Shape>) -> f32 {
    plate_height(shape) + ROW_HEIGHT - PLATE_HEIGHT
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
            self.close_task(cx);
        }
        self.lay_out_tasks(cx);
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
        self.lay_out_tasks(cx);
        cx.notify();
    }

    /// Puts the ask's slot and the cards at the top of each Space's Today, each as tall as its
    /// shape in the sidebar's width, and slides the rows of the Space shown to their places.
    pub(super) fn lay_out_tasks(&mut self, cx: &App) {
        let before = self.layout();
        let width = self.sidebar_width;
        let system = self.text_system.get_or_insert_with(|| WindowTextSystem::new(cx.text_system().clone()));
        let system: &WindowTextSystem = system;
        let measure = |text: &SharedString, size: f32, weight: FontWeight| text_width(text, size, weight, system);
        let shapes: Vec<(SharedString, Shape)> = (self.task_cards.iter())
            .map(|card| (card.agent.clone(), shape(card, width, &measure)))
            .collect();
        for space in &mut self.spaces {
            let ask = (self.ask_slot == Some(space.id)).then(|| (Slot::Ask, slot(None)));
            let cards = (self.task_cards.iter().zip(&shapes).filter(|(card, _)| card.home == space.id))
                .map(|(card, (_, shape))| (Slot::Task(number(&card.agent)), slot(Some(shape))));
            space.tasks = ask.into_iter().chain(cards).collect();
        }
        self.task_shapes = shapes;
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

    /// A task's card: a tab row with a status line under its title, and its chips under that.
    fn task_card(&self, card: &Card, cx: &mut Context<Self>) -> AnyElement {
        let agent = card.agent.clone();
        let hovered = self.task_hovered.as_ref() == Some(&agent);
        let shown = self.talk_agent() == Some(agent.clone());
        let shape = (self.task_shapes.iter())
            .find(|(each, _)| *each == agent)
            .map_or_else(Shape::default, |(_, shape)| shape.clone());
        let state = div()
            .absolute()
            .left(px(FAVICON_X - PLATE_INSET))
            .top(px(STATE_TOP))
            .size(px(FAVICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .child(self.task_state(card));
        let end = match hovered {
            true => CLOSE_SIZE + CLOSE_RIGHT,
            false => TITLE_END_INSET,
        };
        let top_line = div()
            .h(px(LINE))
            .pl(px(COLUMN))
            .pr(px(end))
            .flex()
            .items_center()
            .child(title(&self.palette, card.title.clone(), shown));
        let status = div()
            .mt(px(STATUS_GAP))
            .pl(px(COLUMN))
            .pr(px(TITLE_END_INSET))
            .text_size(px(text::SMALL))
            .line_height(px(STATUS_LINE))
            .text_color(self.palette.text(SECONDARY));
        let status = match shape.lines {
            1 => status.child(div().truncate().child(card.status.clone())),
            lines => status.child(div().line_clamp(lines).text_ellipsis().child(card.status.clone())),
        };
        let plate = div()
            // Its conversation shows, as a selected tab's page does.
            .when(shown, |plate| selected_plate(&self.palette, plate, 1., 0., false))
            .id(("task-plate", number(&agent)))
            .relative()
            .size_full()
            .pt(px(TOP))
            .flex()
            .flex_col()
            .rounded(px(PLATE_RADIUS))
            .when(hovered, |plate| plate.bg(self.palette.plate(HOVERED)))
            .child(state)
            .child(top_line)
            .child(status)
            .children(self.task_chips(card, &shape, cx))
            .children(hovered.then(|| self.task_close(agent.clone(), cx)))
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

    /// The task's state in the favicon's column: Signal's dot breathing while it works, ringed
    /// and still while it waits on the user, a tick once it's done, a hollow ring once it stopped.
    fn task_state(&self, card: &Card) -> AnyElement {
        match card.dot {
            Dot::Working => div()
                .size(px(STATUS_DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
                .opacity(self.breath.strength())
                .into_any_element(),
            Dot::Waiting => ringed_dot(STATUS_AGENT, STATUS_DOT).into_any_element(),
            Dot::Done => {
                let what = Swapped::Task(number(&card.agent));
                self.swaps.glyph(what, CHECK, LOADING_SIZE, rgb(STATUS_AGENT).into())
            }
            Dot::Ended => div()
                .size(px(HOLLOW))
                .rounded_full()
                .border(px(HOLLOW_STROKE))
                .border_color(self.palette.text(SECONDARY))
                .into_any_element(),
        }
    }

    /// The ✕ under the pointer, a tab row's, which dismisses the task.
    fn task_close(&self, agent: SharedString, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let color = self.palette.text(SECONDARY);
        div()
            .id(("task-close", number(&agent)))
            .absolute()
            .top(px((PLATE_HEIGHT - CLOSE_SIZE) / 2.))
            .right(px(CLOSE_RIGHT))
            .size(px(CLOSE_SIZE))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            // A tab row's ✕: the bars as one glyph, so they don't darken where they cross.
            .child(
                svg()
                    .path(super::assets::bars_path(45.))
                    .flex_none()
                    .size(px(CLOSE_GLYPH))
                    .text_color(color),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                // check.sh reads this line.
                log!("browser: command dismiss-task via sidebar");
                shell.dismiss_task(agent.clone(), cx);
            }))
    }

    /// The card's chips, in the rows its shape gives them, the first one's words on the title's
    /// column.
    fn task_chips(&self, card: &Card, shape: &Shape, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if shape.rows.is_empty() {
            return None;
        }
        let chips = chips(card);
        let n = number(&card.agent);
        let rows = shape.rows.iter().map(|row| {
            let row = row.iter().filter_map(|&index| {
                let chip = chips.get(index)?.clone();
                let id = ("task-chip", n * 8 + index as u64);
                let element = card_chip(&self.palette, id, chip.label());
                let element = match chip {
                    Chip::Answer(answer) => element.on_click(act(card.agent.clone(), TaskAction::Answer(answer), cx)),
                    Chip::Retry => element.on_click(act(card.agent.clone(), TaskAction::Retry, cx)),
                    Chip::Open(_) => element.on_click(act(card.agent.clone(), TaskAction::Open, cx)),
                    Chip::Copy(command) => element.on_click(cx.listener(move |shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.copy(command, "Command Copied", cx);
                    })),
                };
                Some(element)
            });
            div().flex().gap(px(CHIP_GAP)).children(row)
        });
        Some(
            div()
                .mt(px(CHIPS_TOP))
                .pl(px(COLUMN - CHIP_X))
                .pr(px(CHIP_END))
                .flex()
                .flex_col()
                .gap(px(CHIP_GAP))
                .children(rows),
        )
    }

    /// ✕, or ⌘W in its conversation: the task goes, as ⌘Z and ⇧⌘T bring back.
    pub(super) fn dismiss_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        self.task_hovered = None;
        self.browser.task(agent.clone(), TaskAction::Dismiss, cx);
        self.keep(What::DISMISS_TASK, Some(Edit::RestoreTask(agent.clone())), cx);
        self.closed_preview = None;
        self.closed_pinned = None;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        self.closed_task = Some((agent, now.unwrap_or_default().as_secs_f64() * 1000.));
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

/// A chip on a card, the sidebar's scale: the URL field's plate at a control's corners, as wide
/// as its words, cut to the room it has.
fn card_chip(
    palette: &super::Palette,
    id: impl Into<gpui::ElementId>,
    label: SharedString,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .min_w_0()
        .h(px(CHIP_HEIGHT))
        .px(px(CHIP_X))
        .flex()
        .items_center()
        .rounded(px(NAV_RADIUS))
        .bg(palette.plate(URL_FIELD))
        .hover(|style| style.bg(palette.state(URL_FIELD, STATE_HOVERED)))
        .active(|style| style.bg(palette.state(URL_FIELD, STATE_PRESSED)))
        .text_size(px(text::SMALL))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text(TITLE))
        .child(div().truncate().child(label))
}

/// A chip's click, which does `action` to the task.
pub(super) fn act(
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
fn slot(shape: Option<&Shape>) -> f32 {
    card_height(shape) + ROW_PITCH - ROW_HEIGHT
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use gpui::{FontWeight, SharedString};

    use super::{Card, Dot, Shape, card_height, chips, shape, slot};
    use crate::agents::runner::Cli;
    use crate::shell::{
        Command, Folders, PLATE_HEIGHT, PLATE_INSET, ROW_HEIGHT, Shell, Slot, recorder::Recorder,
    };
    use crate::tokens::shell::task::{CHIP_HEIGHT, CHIPS_TOP, LINE, STATUS_GAP, STATUS_LINE, TOP};

    fn card(n: u64, dot: Dot) -> Card {
        Card {
            agent: format!("task-{n}").into(),
            home: 1,
            space: 100 + n,
            cli: Cli::Claude,
            title: "Find a flight to Lisbon".into(),
            status: "Searching flights".into(),
            asks: false,
            dot,
            answers: Vec::new(),
            question: None,
            open: Vec::new(),
            retry: false,
            command: None,
        }
    }

    /// The agent's question takes a second line when it needs one, and nothing else does. Its
    /// chips wrap to two rows; past that, or with an answer wider than the room, none show, and
    /// the card's click opens the conversation, where they do. Open, a chip of its own, always
    /// shows, cut to the room.
    #[test]
    fn a_card_shapes_to_its_question_and_its_chips() {
        // Every character 6 pt wide, in a sidebar 240 wide: 180 for the status, 191 for chips.
        let measure = |text: &SharedString, _: f32, _: FontWeight| text.chars().count() as f32 * 6.;
        let answers = |words: &[&str]| words.iter().map(|word| SharedString::from(word.to_string())).collect();
        let mut asks = card(1, Dot::Waiting);
        asks.asks = true;
        asks.status = "Both are open Saturday. Which would you rather visit?".into();
        asks.answers = answers(&["Gulbenkian", "MAAT"]);
        assert_eq!(shape(&asks, 240., &measure), Shape { lines: 2, rows: vec![vec![0, 1]] });
        let mut working = card(2, Dot::Working);
        working.status = asks.status.clone();
        assert_eq!(shape(&working, 240., &measure), Shape { lines: 1, rows: Vec::new() });

        asks.answers = answers(&["Museum 1", "Museum 2", "Museum 3", "Museum 4"]);
        assert_eq!(shape(&asks, 240., &measure).rows, [vec![0, 1], vec![2, 3]]);
        asks.answers = answers(&["Museum 1", "Museum 2", "Museum 3", "Museum 4", "Museum 5"]);
        assert!(shape(&asks, 240., &measure).rows.is_empty(), "a third row");
        asks.answers = answers(&["The Calouste Gulbenkian Museum's modern wing"]);
        assert!(shape(&asks, 240., &measure).rows.is_empty(), "an answer wider than the room");

        let mut done = card(3, Dot::Done);
        done.open = vec![(30, "tickets.gulbenkian.pt.example.org".into())];
        assert_eq!(shape(&done, 240., &measure).rows, [vec![0]]);
        assert_eq!(chips(&done)[0].label().as_ref(), "Open tickets.gulbenkian.pt.example.org");
        done.open.extend([(31, "maat.pt".into()), (32, "gulbenkian.pt".into())]);
        assert_eq!(chips(&done)[0].label().as_ref(), "Open 3 pages");
        let mut failed = card(4, Dot::Ended);
        failed.command = Some("claude auth login");
        assert_eq!(chips(&failed)[0].label().as_ref(), "Copy");
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
        let mut limit = card(2, Dot::Ended);
        limit.retry = true;
        let mut elsewhere = card(3, Dot::Working);
        elsewhere.home = 2;
        shell.update(cx, |shell, cx| shell.set_task_cards(vec![limit.clone(), elsewhere], cx));
        let shaped = shell.read_with(cx, |shell, _| shell.task_shapes[0].1.clone());
        assert!(card_height(Some(&shaped)) > card_height(None));
        assert_eq!(top(&shell, Slot::Tab(10), cx), Some(first + slot(Some(&shaped))));
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

        // A page closed after the dismissal, a task's own too, is what ⇧⌘T reopens.
        shell.update(cx, |shell, cx| {
            shell.dismiss_task("task-1".into(), cx);
            let later = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
            let archived = later.unwrap().as_secs_f64() * 1000. + 1.;
            let tab = crate::shell::library::Archived {
                id: 5,
                url: "https://example.com".into(),
                title: "Example".into(),
                archived,
                little: false,
            };
            shell.set_archive(vec![tab], cx);
            host.take();
            shell.reopen_closed(cx);
        });
        assert_eq!(host.take(), ["reopen closed tab"]);
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
        let mut limit = card(4, Dot::Ended);
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
        let y = list + card + (ROW_HEIGHT - PLATE_HEIGHT) / 2. + TOP + LINE + STATUS_GAP
            + STATUS_LINE + CHIPS_TOP + CHIP_HEIGHT / 2.;
        let x = PLATE_INSET + 40.;
        host.take();
        cx.simulate_click(point(px(x), px(y)), Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"task task-4 Retry".to_string()));
    }
}
