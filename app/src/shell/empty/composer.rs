//! The page an empty Space rests on (#4140): the command bar where ⌘T's opens, whose ⇥ grows it
//! into the composer in place, and under it the user's recent tasks. The bar is the same bar as
//! ⌘T's (`command_bar::in_place`), drawn in the page, with its own keys. Nothing of the tasks is
//! read for the page's first frame, and the bar asks the core nothing until it's typed in.

use gpui::{
    Bounds, Context, Corners, Entity, FontWeight, Hsla, Pixels, SharedString, Window, canvas, div,
    point, prelude::*, px, rgb, rgba,
};

use crate::agents::task::{Card, Dot, TaskAction, Word};
use crate::agents::tier::{ANY_SITE, ONLY_ADDED, Tier};
use crate::command_bar::{self, CommandBar};
use crate::palette::{Palette, Tone};
use crate::shell::ShowStarted;
use crate::tokens::command_bar::{HEIGHT as BAR_HEIGHT, WIDTH as BAR_WIDTH};
use crate::tokens::shell::composer::*;
use crate::tokens::text;

use super::super::{
    STATUS_AGENT, Shell,
    menus::{MenuAction, MenuEntry},
    task_area::covering,
};

/// Over the cards.
const LATEST: &str = "Pick up a thread";

/// The bar the page rests on, while the Space shows no page.
pub(in crate::shell) struct Resting {
    pub(in crate::shell) bar: Entity<CommandBar>,
    /// The page has drawn its first frame, from when it draws the tasks.
    painted: bool,
    /// Whether the field is owed the keyboard, which it takes once, at the first frame that may
    /// give it (`give_resting_keys`), so whatever the user moves the keyboard to after keeps it.
    wants_keys: Owed,
    /// The Space it rests in.
    space: u64,
}

/// Where a chip's menu hangs: 4 under the chip, its names under the chip's own word, or at the
/// click before the chip is drawn.
fn hung_under(chip: Option<Bounds<Pixels>>, clicked: gpui::Point<Pixels>) -> gpui::Point<Pixels> {
    let Some(chip) = chip else {
        return clicked;
    };
    let names = crate::menu::PADDING + crate::menu::ROW_X + crate::menu::CHECK + crate::menu::GAP;
    chip.origin + point(px(CHIP_INSET - names), chip.size.height + px(MENU_DROP))
}

/// The level's menu's last row, which opens Settings › Agents at Access.
const SETTINGS: &str = "Settings…";

/// Why the field is owed the keyboard, if it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owed {
    No,
    /// The Space came to show no page: the field takes the keyboard only from the window itself,
    /// never from a field or form the user is in.
    Shown,
    /// ⌘T or ⌘L asked for it.
    Asked,
}

impl Shell {
    /// Whether the card rests on the bar: the Space shows no page, the welcome hasn't the card,
    /// and the window isn't a Little ilo window's, which keeps its own bar inside it.
    pub(in crate::shell) fn rests_on_bar(&self) -> bool {
        self.selected == 0 && !self.welcome && self.little.is_none()
    }

    /// Makes the bar the page rests on as the Space comes to show no page, and drops it as a page
    /// shows, or the welcome takes the card.
    pub(in crate::shell) fn step_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.rests_on_bar() {
            if let Some(resting) = self.resting.take() {
                // GPUI's focus would go with the field, and the shortcuts with it, as the shell's
                // context holds them, until a click lands on the shell, which a header button's
                // never does. The shell holds it again; a page that took the keyboard keeps it.
                if resting.bar.read(cx).field_focus(cx).is_focused(window) {
                    if self.page_keyboard.is_some() && self.page_has_keyboard(window, cx) {
                        self.page_keyboard = Some(Some(self.focus_handle.downgrade()));
                    }
                    window.focus(&self.focus_handle, cx);
                }
                log!("browser: resting field dropped");
            }
            return;
        }
        let space = self.space();
        if let Some(resting) = &mut self.resting {
            // Another Space empty too: its field is owed the keyboard, once the swipe settles.
            if resting.space != space {
                resting.space = space;
                resting.wants_keys = Owed::Shown;
                let bar = resting.bar.clone();
                bar.update(cx, |bar, cx| bar.space_changed(window, cx));
            }
            return self.give_resting_keys(window, cx);
        }
        let bar = self.browser.command_bar_in_place(window, cx);
        self.resting = Some(Resting {
            bar,
            painted: false,
            wants_keys: Owed::Shown,
            space,
        });
        cx.on_next_frame(window, |shell, _, cx| {
            if let Some(resting) = &mut shell.resting {
                resting.painted = true;
                cx.notify();
            }
        });
        log!("browser: resting field made");
        self.give_resting_keys(window, cx);
    }

    /// ⌘T and ⌘L while the Space shows no page: the field takes the keyboard, and no bar opens
    /// over it (#4140). Whether it did.
    pub(in crate::shell) fn keys_to_resting(&mut self, cx: &mut Context<Self>) -> bool {
        // The field stays until the next frame drops it, so a page shown since isn't one.
        if !self.rests_on_bar() {
            return false;
        }
        let Some(resting) = &mut self.resting else {
            return false;
        };
        // The core shows a page the shell hasn't heard of yet, as at launch with a page to open:
        // the keys are that page's, and open the bar over it (#4268).
        if self.browser.shown_now() != 0 {
            return false;
        }
        resting.wants_keys = Owed::Asked;
        cx.notify();
        true
    }

    /// Gives the field the keyboard it's owed once the window is key, no overlay or Library is
    /// over it and no swipe is moving the pages: an overlay keeps the keys it has, and the field
    /// takes them only when the swipe to its Space has settled. Owed for being shown, it leaves
    /// the keyboard where the user put it, as in a Space's name or its form.
    fn give_resting_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still = self.swipe.is_none() && self.settle.is_none();
        let free = window.is_window_active()
            && !self.browser.command_bar_open(cx)
            && self.library.is_none();
        let root = self.focus_handle.clone();
        let Some(resting) =
            (self.resting.as_mut()).filter(|resting| resting.wants_keys != Owed::No)
        else {
            return;
        };
        if !(still && free) {
            return;
        }
        let focus = resting.bar.read(cx).field_focus(cx);
        let elsewhere = window
            .focused(cx)
            .is_some_and(|at| at != root && at != focus);
        let owed = std::mem::replace(&mut resting.wants_keys, Owed::No);
        if owed == Owed::Shown && elsewhere {
            return;
        }
        window.focus(&focus, cx);
        // GPUI's focus alone draws the caret, while the Mac sends a key to its window's first
        // responder, which a click gives the UI by itself and this hold gives it without one. Not
        // while the core already shows a page the shell hasn't heard of: the keys are that page's.
        if self.browser.shown_now() == 0 {
            self.hold_keyboard_for_page(cx);
        }
        log!("browser: resting field has the keyboard");
    }

    /// Gives the resting field the keyboard, as a click on the page does.
    pub(in crate::shell) fn focus_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            let focus = resting.bar.read(cx).field_focus(cx);
            window.focus(&focus, cx);
        }
    }

    /// What the browser knows of the assistants changed, as once it has looked for them: the
    /// page's bar learns them again.
    pub(in crate::shell) fn assistants_learned(&mut self, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            resting.bar.update(cx, |bar, cx| bar.agents_changed(cx));
        }
    }

    /// ⌘T's ↩ is starting a task from this window's Space, over the tab it shows: as the
    /// composer's does, its conversation takes that tab's place as its card comes in.
    pub fn bar_starting(&mut self) {
        self.show_started = Some(ShowStarted::Asked {
            home: self.space(),
            over: self.selected,
        });
    }

    /// The Agents' answer to the composer's or ⌘T's last ↩: the task it started, whose view alone
    /// opens as its card comes in, or None, when it started none and no other task's card opens a
    /// view.
    pub fn composed_started(&mut self, agent: Option<SharedString>) {
        let Some(ShowStarted::Asked { over, .. }) = self.show_started else {
            return;
        };
        self.show_started = agent.map(|agent| ShowStarted::Task { agent, over });
    }

    /// The composer's and ⌘T's ↩ show the task they started in the place of the page they were
    /// asked over, the empty page or a tab, while it still shows. The card comes before the window
    /// hears of the task's Space, which the core tells after the call that made it, so until then
    /// the start waits for that Space, by its task. `was`, the cards before these, tells the new
    /// ones while the Agents haven't said which task it started.
    pub(in crate::shell) fn show_started_task(
        &mut self,
        was: Option<&[SharedString]>,
        cx: &mut Context<Self>,
    ) {
        let started = |card: &&Card| match &self.show_started {
            Some(ShowStarted::Asked { home, .. }) => {
                card.home == *home && was.is_some_and(|was| !was.contains(&card.agent))
            }
            Some(ShowStarted::Task { agent, .. }) => card.agent == *agent,
            None => false,
        };
        let Some(card) = self.task_cards.iter().find(started) else {
            return;
        };
        let over = self.show_started.as_ref().map_or(0, ShowStarted::over);
        let (agent, home, space) = (card.agent.clone(), card.home, card.space);
        if space != 0 && self.task_space(space).is_none() {
            self.show_started = Some(ShowStarted::Task { agent, over });
            return;
        }
        self.show_started = None;
        if self.selected == over && self.space() == home && self.talk_agent().is_none() {
            self.show_task(agent, cx);
        }
    }

    /// The level chip's menu under a task's reply field, as the composer's: names only, the
    /// task's level checked, where it may go, and Settings. Only the user's pick here changes the
    /// task's level, through the press its refusal's button makes; there's no agent path to it. A
    /// running task can widen to any site but not narrow to the sites added, which it never had.
    pub(in crate::shell) fn task_tier_menu(
        &mut self,
        agent: SharedString,
        current: Tier,
        only: bool,
        chip: Option<Bounds<Pixels>>,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let press = |action: TaskAction| -> MenuAction {
            let agent = agent.clone();
            Box::new(move |shell, cx| shell.browser.task(agent, action, cx))
        };
        let mut items: Vec<(MenuEntry, MenuAction)> = (Tier::ALL.into_iter())
            .map(|tier| {
                (
                    MenuEntry::named(tier.label(), true).checked(tier == current),
                    press(TaskAction::PickTier(tier)),
                )
            })
            .collect();
        let none: fn() -> MenuAction = || Box::new(|_, _| {});
        items.push((MenuEntry::line(), none()));
        items.push((
            MenuEntry::named(ANY_SITE, true).checked(!only),
            press(TaskAction::PickAnySite),
        ));
        items.push((MenuEntry::named(ONLY_ADDED, only).checked(only), none()));
        items.push((MenuEntry::line(), none()));
        let settings: MenuAction = Box::new(|shell, cx| shell.browser.agent_access(cx));
        items.push((MenuEntry::new(SETTINGS, true), settings));
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The page in the card at `frame`: the bar at the height ⌘T's opens at, and under it the
    /// recent tasks, which its rows and its composer go over as they open, as ⌘T's over a page.
    pub(in crate::shell) fn resting_page(
        &mut self,
        frame: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let width = f32::from(frame.size.width);
        let resting = self.resting.as_ref()?;
        let (bar, painted) = (resting.bar.clone(), resting.painted);
        let palette = self.palette;
        let cards: Vec<Card> = match painted {
            true => self.latest_tasks(CARDS as usize),
            false => Vec::new(),
        };
        let row = command_bar::centered_row_middle(f32::from(self.viewport.height))
            - f32::from(frame.top());
        let plates = cards.iter().any(|card| self.plate_of(card).is_some());
        let top = group_top(row, f32::from(frame.size.height), !cards.is_empty(), plates);
        // A narrow window keeps the bar inside the card's gutters.
        let bar_width = BAR_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let bar = div()
            .absolute()
            .top(px(top))
            .left(px((width - bar_width) / 2.))
            .w(px(bar_width))
            .child(bar);
        let page = div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(top + BAR_HEIGHT));
        if cards.is_empty() {
            return Some(page.child(bar));
        }
        let label_width = LABEL_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let cards: Vec<_> = (cards.into_iter().enumerate())
            .map(|(index, card)| self.recent_card(index, card, cx))
            .collect();
        Some(
            page.child(
                div()
                    .w(px(label_width))
                    .line_height(px(LABEL_LINE))
                    .mt(px(LABEL_TOP))
                    .mb(px(LABEL_BOTTOM))
                    .pl(px(LABEL_INSET))
                    .text_size(px(LABEL_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text(1.))
                    .child(LATEST),
            )
            .child(
                div()
                    .w(px(label_width))
                    .flex()
                    .gap(px(CARD_GAP))
                    .overflow_hidden()
                    .children(cards),
            )
            // Last, so it's over the cards.
            .child(bar),
        )
    }

    /// A recent task's card: its state, its title and its last steps, the one under way in full
    /// ink (or its status, with no steps this turn), over the picture its last page's tile ended
    /// on. Once over, its one line is a done task's answer's first or, quiet, a stopped one's
    /// last finished step, if it has one; a task a run before left says when it ended and has
    /// none. A card with no picture ends after its words. A click opens it.
    fn recent_card(
        &self,
        index: usize,
        card: Card,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = self.palette;
        let state = match &card.ended_at {
            Some(at) => format!("{} · {at}", card.word.alone(&card.name)).into(),
            None => card.word.alone(&card.name),
        };
        let dot = (card.dot == Dot::Working).then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
        });
        let working = card.dot == Dot::Working;
        let lines: Vec<(SharedString, bool)> = match card.word {
            _ if card.ended_at.is_some() => Vec::new(),
            // A done task says its answer's first line; a stopped one only its last finished
            // step, quiet, or nothing after its title.
            Word::Done | Word::Stopped => (card.line.clone().into_iter())
                .map(|line| (line, card.word == Word::Done))
                .collect(),
            _ if card.steps.is_empty() => vec![(card.status.clone(), true)],
            _ => {
                let last = card.steps.len() - 1;
                (card.steps.iter().cloned().enumerate())
                    .map(|(at, step)| (step, working && at == last))
                    .collect()
            }
        };
        let lines = lines.into_iter().map(|(line, now)| {
            div()
                .whitespace_nowrap()
                .truncate()
                .text_color(match now {
                    true => palette.text(1.),
                    false => palette.text(STEP_INK),
                })
                .child(line)
        });
        let plate = self.plate_of(&card).map(|picture| {
            let corners = Corners::all(px(PAGE_RADIUS));
            div()
                .h(px(PAGE_HEIGHT))
                .mx(px(PAGE_INSET))
                .mb(px(PAGE_INSET))
                .rounded(px(PAGE_RADIUS))
                .bg(palette.plate(PAGE_FILL))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let drawn = covering(bounds, picture.size());
                            (window.paint_image(bounds, drawn, corners, picture.image(), 0, false))
                                .ok();
                        },
                    )
                    .size_full(),
                )
        });
        let agent = card.agent.clone();
        div()
            .id(("composer-task", index))
            .flex_none()
            .w(px(CARD_WIDTH))
            .flex()
            .flex_col()
            .rounded(px(CARD_RADIUS))
            .bg(palette.plate(CARD_FILL))
            .border_1()
            .border_color(rim(&palette, CARD_RIM))
            .overflow_hidden()
            .text_size(px(text::BODY))
            .on_click(cx.listener(move |shell, _, _, cx| shell.open_latest(agent.clone(), cx)))
            .child(
                div()
                    .h(px(CARD_WORDS))
                    .pt(px(CARD_TOP))
                    .px(px(CARD_INSET))
                    .pb(px(CARD_BOTTOM))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(STATE_HEIGHT))
                            .flex()
                            .items_center()
                            .gap(px(STATE_GAP))
                            .text_size(px(STATE_SIZE))
                            .text_color(match card.dot {
                                Dot::Waiting => palette.text(1.),
                                _ => palette.text(STATE_INK),
                            })
                            .children(dot)
                            .child(state),
                    )
                    .child(
                        div()
                            .my(px(TITLE_GAP))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text(1.))
                            .whitespace_nowrap()
                            .truncate()
                            .child(card.title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .text_size(px(STEP_SIZE))
                            .line_height(px(STEP_LINE))
                            .children(lines),
                    ),
            )
            .children(plate)
    }
}

/// How far under a card `height` tall the page's group starts, with its bar's input row's middle
/// `row` under the card's top as ⌘T's bar's is: there, or as far up as the group needs to fit,
/// through its cards when it has any (`plates` if one has its site's plate), never above `TOP`.
pub(in crate::shell) fn group_top(row: f32, height: f32, cards: bool, plates: bool) -> f32 {
    let card = match plates {
        // A card is its words and its plate inside its rim, 1 on each side.
        true => 2. + CARD_WORDS + PAGE_HEIGHT + PAGE_INSET,
        false => 2. + CARD_WORDS,
    };
    let below = match cards {
        true => LABEL_TOP + LABEL_LINE + LABEL_BOTTOM + card,
        false => 0.,
    };
    let level = row - BAR_HEIGHT / 2.;
    level.min(height - TOP - BAR_HEIGHT - below).max(TOP)
}

/// A plate's rim: white `dark` on a dark window, the ink on a light one, where white doesn't show.
fn rim(palette: &Palette, dark: u32) -> Hsla {
    match palette.tone {
        Tone::Dark => rgba(dark).into(),
        Tone::Light => palette.plate(LIGHT_RIM),
    }
}
