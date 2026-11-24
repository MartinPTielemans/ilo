//! The page an empty Space rests on (#4140): ilo's mark over a field that searches or, after ⇥,
//! asks an assistant, the line Ask adds under it, and the user's recent tasks. The field is the
//! command bar at rest (`command_bar::in_place`), whose input and rows this page places and whose
//! keys it takes. Nothing of the tasks is read for the page's first frame, and the bar asks the
//! core nothing until it's typed in.

use gpui::{
    BoxShadow, Context, Entity, FontWeight, Hsla, SharedString, Subscription, Window, div, point,
    prelude::*, px, rgb, rgba, svg,
};

use crate::agents::task::{Card, Dot};
use crate::command_bar::{CommandBar, InPlaceKey};
use crate::tokens::shell::composer::*;

use super::super::{STATUS_AGENT, Shell, commands::key_name, keys::OFF_THE_MAC};

/// What the field says in Search.
pub(in crate::shell) const SEARCH_WORDS: &str = "Search or enter address";
/// The switch's two sides.
const SEARCH: &str = "Search";
const ASK: &str = "Ask";
/// Over the cards.
const RECENT: &str = "Recent tasks";

/// What the field says in Ask, naming the assistant it runs on.
fn ask_words(assistant: Option<&str>) -> SharedString {
    format!(
        "Ask {} to do something",
        assistant.unwrap_or("an assistant")
    )
    .into()
}

/// The bar the page rests on, while the Space shows no page.
pub(in crate::shell) struct Resting {
    pub(in crate::shell) bar: Entity<CommandBar>,
    _keys: Subscription,
    /// The page has drawn its first frame, from when it draws the tasks.
    painted: bool,
}

impl Shell {
    /// Makes the bar the page rests on as the Space comes to show no page, and drops it as a page
    /// shows, or the welcome takes the card.
    pub(in crate::shell) fn step_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != 0 || self.welcome {
            if self.resting.take().is_some() {
                // check.sh reads this line.
                log!("browser: resting field dropped");
            }
            return;
        }
        if self.resting.is_some() {
            return;
        }
        let bar = self.browser.command_bar_in_place(window, cx);
        let keys = cx.subscribe_in(&bar, window, |shell: &mut Shell, _, key, window, cx| {
            shell.resting_key(*key, window, cx)
        });
        bar.update(cx, |bar, cx| {
            bar.set_on_ask(false, SEARCH_WORDS.into(), window, cx)
        });
        self.resting = Some(Resting {
            bar,
            _keys: keys,
            painted: false,
        });
        cx.on_next_frame(window, |shell, _, cx| {
            if let Some(resting) = &mut shell.resting {
                resting.painted = true;
                cx.notify();
            }
        });
        // check.sh reads this line.
        log!("browser: resting field made");
    }

    /// Gives the resting field the keyboard, as a click on the page does.
    pub(in crate::shell) fn focus_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            let focus = resting.bar.read(cx).field_focus(cx);
            window.focus(&focus, cx);
        }
    }

    /// The keys the resting bar hands the page: ⇥ flips the switch, Esc in Ask goes back to
    /// Search and in Search does nothing, and ↩ in Ask runs the task.
    fn resting_key(&mut self, key: InPlaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resting) = &self.resting else {
            return;
        };
        let on_ask = resting.bar.read(cx).on_ask();
        match key {
            InPlaceKey::Flip => self.set_on_ask(!on_ask, window, cx),
            InPlaceKey::Escape if on_ask => self.set_on_ask(false, window, cx),
            InPlaceKey::Escape => {}
            InPlaceKey::Run => self.run_composed(window, cx),
        }
    }

    /// The page's switch goes to Ask, or back to Search.
    pub(in crate::shell) fn set_on_ask(
        &mut self,
        on_ask: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(resting) = &self.resting else {
            return;
        };
        let bar = resting.bar.clone();
        bar.update(cx, |bar, cx| {
            let words = match on_ask {
                true => ask_words(bar.assistant(cx).as_ref().map(|agent| agent.name.as_ref())),
                false => SEARCH_WORDS.into(),
            };
            bar.set_on_ask(on_ask, words, window, cx)
        });
        // check.sh reads this line.
        log!("browser: composer on {}", if on_ask { ASK } else { SEARCH });
        cx.notify();
    }

    /// ↩ in Ask: what was typed runs as a task on the assistant, from this Space.
    fn run_composed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resting) = &self.resting else {
            return;
        };
        let bar = resting.bar.clone();
        let Some((agent, asked)) = bar.update(cx, |bar, cx| bar.take_task(window, cx)) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: composer runs a task on {agent}");
        let home = self.space();
        self.browser.start_task(&agent, &asked, home, cx);
    }

    /// The page in a card `width` wide: the mark, the field, Ask's line and the recent tasks.
    pub(in crate::shell) fn composer_page(
        &mut self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let resting = self.resting.as_ref()?;
        let (bar, painted) = (resting.bar.clone(), resting.painted);
        let palette = self.palette;
        let fill = palette.plate(FIELD_FILL);
        let field = bar.update(cx, |bar, cx| bar.field(fill, window, cx));
        let on_ask = bar.read(cx).on_ask();
        // A narrow window keeps the field inside the card's gutters.
        let field_width = FIELD_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let line_width = LINE_WIDTH.min(field_width - (FIELD_WIDTH - LINE_WIDTH));
        let tab = div()
            .flex_none()
            .mr(px(KEY_GAP))
            .opacity(KEY_OPACITY)
            .h(px(KEY_HEIGHT))
            .px(px(KEY_INSET))
            .flex()
            .items_center()
            .rounded(px(KEY_RADIUS))
            .bg(palette.plate(KEY_FILL))
            .text_size(px(KEY_SIZE))
            .text_color(palette.text(KEY_INK))
            .child(key_name("tab", OFF_THE_MAC));
        let field = div()
            .id("composer-field")
            .flex_none()
            .w(px(field_width))
            .h(px(FIELD_HEIGHT))
            .pl(px(FIELD_LEFT))
            .pr(px(FIELD_RIGHT))
            .flex()
            .items_center()
            .rounded(px(FIELD_RADIUS))
            .bg(fill)
            .border_1()
            .border_color(rgba(FIELD_RIM))
            .shadow(vec![
                shadow(rgba(FIELD_SHADOW_NEAR).into(), 1., 3.),
                shadow(rgba(FIELD_SHADOW_FAR).into(), 14., 36.),
            ])
            .text_size(px(FIELD_SIZE))
            .text_color(palette.text(1.))
            .child(div().flex_1().min_w_0().flex().child(field))
            .child(tab)
            .child(self.switch(on_ask, cx));
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(TOP))
            .child(super::mark(1., &palette).mb(px(MARK_GAP)))
            .child(field)
            .child(self.ask_line(on_ask, line_width, cx));
        let cards: Vec<Card> = match painted {
            true => (self.task_cards.iter())
                .filter(|card| card.replies)
                .take(CARDS as usize)
                .cloned()
                .collect(),
            false => Vec::new(),
        };
        if cards.is_empty() {
            return Some(page);
        }
        let label_width = LABEL_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let cards: Vec<_> = (cards.into_iter().enumerate())
            .map(|(index, card)| self.recent_card(index, card, cx))
            .collect();
        Some(
            page.child(
                div()
                    .w(px(label_width))
                    .mt(px(LABEL_TOP))
                    .mb(px(LABEL_BOTTOM))
                    .pl(px(LABEL_INSET))
                    .text_size(px(LABEL_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text(1.))
                    .child(RECENT),
            )
            .child(
                div()
                    .w(px(label_width))
                    .flex()
                    .gap(px(CARD_GAP))
                    .overflow_hidden()
                    .children(cards),
            ),
        )
    }

    /// Search | Ask, at the field's end: a click on a side flips to it.
    fn switch(&self, on_ask: bool, cx: &mut Context<Self>) -> gpui::Div {
        let palette = self.palette;
        let part = |words: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(words)
                .h(px(SWITCH_HEIGHT))
                .px(px(SWITCH_PART_INSET))
                .flex()
                .items_center()
                .rounded(px(SWITCH_PART_RADIUS))
                .when(on, |part| {
                    part.bg(palette.plate(SWITCH_ON))
                        .border_1()
                        .border_color(rgba(SWITCH_ON_RIM))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text(1.))
                })
                .when(!on, |part| part.text_color(palette.text(SWITCH_INK)))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.set_on_ask(words == ASK, window, cx)
                }))
                .child(words)
        };
        div()
            .flex_none()
            .p(px(SWITCH_INSET))
            .gap(px(SWITCH_INSET))
            .flex()
            .rounded(px(SWITCH_RADIUS))
            .bg(palette.plate(SWITCH_FILL))
            .text_size(px(SWITCH_SIZE))
            .child(part(SEARCH, !on_ask, cx))
            .child(part(ASK, on_ask, cx))
    }

    /// Ask's line under the field: add a page, the task's access, and the assistant and its model.
    /// It keeps its room in Search, so nothing moves as the switch flips.
    fn ask_line(&mut self, on_ask: bool, width: f32, cx: &mut Context<Self>) -> gpui::Div {
        let palette = self.palette;
        let line = div()
            .w(px(width))
            .mt(px(LINE_TOP))
            .h(px(CHIP_HEIGHT))
            .flex()
            .items_center()
            .text_size(px(CHIP_SIZE))
            .text_color(palette.text(CHIP_INK));
        if !on_ask {
            return line;
        }
        let chip = || {
            div()
                .flex_none()
                .h(px(CHIP_HEIGHT))
                .px(px(CHIP_INSET))
                .flex()
                .items_center()
                .gap(px(CHIP_GAP))
                .rounded(px(CHIP_RADIUS))
        };
        let glyph = |path: &'static str, size: f32| {
            svg()
                .flex_none()
                .path(path)
                .size(px(size))
                .text_color(palette.glyph(CHIP_INK))
        };
        let assistant = (self.resting.as_ref())
            .and_then(|resting| resting.bar.update(cx, |bar, cx| bar.assistant(cx)))
            .map(|agent| match agent.model {
                Some(model) => format!("{} · {model}", agent.name),
                None => agent.name.to_string(),
            });
        line.child(chip().child(glyph("icons/plus.svg", CHIP_GLYPH)))
            .child(
                chip()
                    .child(crate::agents::tier::Tier::default().label())
                    .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON)),
            )
            .child(div().flex_1())
            .children(assistant.map(|assistant| {
                chip()
                    .child(assistant)
                    .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON))
            }))
    }

    /// A recent task's card: its state, its title and where it is, over a plate with its site. A
    /// click opens it.
    fn recent_card(
        &self,
        index: usize,
        card: Card,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = self.palette;
        let state = match card.dot {
            Dot::Working => "Working",
            Dot::Waiting => "Needs you",
            Dot::Done => "Done",
            Dot::Ended => "Ended",
        };
        let dot = (card.dot == Dot::Working).then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
        });
        let site = card.open.first().map(|(_, site)| site.clone());
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
            .border_color(rgba(CARD_RIM))
            .overflow_hidden()
            .on_click(cx.listener(move |shell, _, _, cx| shell.show_task(agent.clone(), cx)))
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
                            .text_size(px(STEP_SIZE))
                            .line_height(px(STEP_LINE))
                            .text_color(palette.text(1.))
                            .whitespace_nowrap()
                            .truncate()
                            .child(card.status),
                    ),
            )
            .child(
                div()
                    .h(px(PAGE_HEIGHT))
                    .mx(px(PAGE_INSET))
                    .mb(px(PAGE_INSET))
                    .rounded(px(PAGE_RADIUS))
                    .bg(palette.plate(PAGE_FILL))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(STEP_SIZE))
                    .text_color(palette.text(STEP_INK))
                    .children(site),
            )
    }
}

/// A shadow straight down, `y` under and `blur` wide.
fn shadow(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}
