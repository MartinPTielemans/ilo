//! The page an empty Space rests on (#4140): ilo's mark over a field that searches or, after ⇥,
//! asks an assistant, the line Ask adds under it, and the user's recent tasks. The field is the
//! command bar at rest (`command_bar::in_place`), whose input and rows this page places and whose
//! keys it takes. Nothing of the tasks is read for the page's first frame, and the bar asks the
//! core nothing until it's typed in.

use gpui::{
    Bounds, BoxShadow, Context, Entity, FontWeight, Hsla, Pixels, SharedString, Subscription,
    Window, div, point, prelude::*, px, rgb, rgba, svg,
};

use crate::agents::task::{Card, Dot};
use crate::agents::tier::Tier;
use crate::command_bar::{self, CommandBar, InPlaceKey};
use crate::palette::{Palette, Tone};
use crate::tokens::shell::composer::*;
use crate::tokens::text;

use super::super::{
    STATUS_AGENT, Shell, URL_FIELD,
    commands::key_name,
    keys::OFF_THE_MAC,
    menus::{MenuAction, MenuEntry},
};

/// What the field says in Search.
pub(in crate::shell) const SEARCH_WORDS: &str = "Search or enter address";
/// The switch's two sides.
const SEARCH: &str = "Search";
const ASK: &str = "Ask";
/// Over the cards.
const LATEST: &str = "Latest tasks";

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
    /// Whether the field is owed the keyboard, which it takes once, at the first frame that may
    /// give it (`give_resting_keys`), so whatever the user moves the keyboard to after keeps it.
    wants_keys: Owed,
    /// The level the next task starts at: the user's pick on Ask's line, in this view's memory
    /// alone. It's Work on pages again after a run, out of Ask, and in another Space.
    tier: Tier,
    /// The Space the pick was made in.
    space: u64,
}

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
    /// Makes the bar the page rests on as the Space comes to show no page, and drops it as a page
    /// shows, or the welcome takes the card.
    pub(in crate::shell) fn step_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != 0 || self.welcome {
            if self.resting.take().is_some() {
                log!("browser: resting field dropped");
            }
            return;
        }
        let space = self.space();
        if let Some(resting) = &mut self.resting {
            // Another Space empty too: its field is owed the keyboard, once the swipe settles.
            if resting.space != space {
                (resting.tier, resting.space) = (Tier::default(), space);
                resting.wants_keys = Owed::Shown;
            }
            return self.give_resting_keys(window, cx);
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
            wants_keys: Owed::Shown,
            tier: Tier::default(),
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
        if self.selected != 0 || self.welcome {
            return false;
        }
        let Some(resting) = &mut self.resting else {
            return false;
        };
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
        log!("browser: resting field has the keyboard");
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
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !on_ask {
            resting.tier = Tier::default();
        }
        let bar = resting.bar.clone();
        bar.update(cx, |bar, cx| {
            let words = match on_ask {
                true => ask_words(bar.assistant(cx).as_ref().map(|agent| agent.name.as_ref())),
                false => SEARCH_WORDS.into(),
            };
            bar.set_on_ask(on_ask, words, window, cx)
        });
        log!("browser: composer on {}", if on_ask { ASK } else { SEARCH });
        cx.notify();
    }

    /// ↩ in Ask: what was typed runs as a task on the assistant, from this Space.
    fn run_composed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        let bar = resting.bar.clone();
        let Some((agent, asked)) = bar.update(cx, |bar, cx| bar.take_task(window, cx)) else {
            return;
        };
        let tier = std::mem::take(&mut resting.tier);
        log!("browser: composer runs a task on {agent}");
        let home = self.space();
        self.browser.start_task(&agent, &asked, home, tier, cx);
        cx.notify();
    }

    /// Puts the composer's level back to Work on pages.
    pub(in crate::shell) fn forget_composer_tier(&mut self) {
        if let Some(resting) = &mut self.resting {
            resting.tier = Tier::default();
        }
    }

    /// The level the next ↩ in Ask starts a task at.
    pub(in crate::shell) fn composer_tier(&self) -> Tier {
        self.resting
            .as_ref()
            .map(|resting| resting.tier)
            .unwrap_or_default()
    }

    /// The level chip's menu: the three levels, the one the next task starts at checked and under
    /// the pointer. A pick holds for the next run only.
    fn tier_menu(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let current = self.composer_tier();
        let items: Vec<(MenuEntry, MenuAction)> = (Tier::ALL.into_iter())
            .map(|tier| {
                let pick: MenuAction = Box::new(move |shell, cx| shell.pick_tier(tier, cx));
                (
                    MenuEntry::named(tier.label(), true).checked(tier == current),
                    pick,
                )
            })
            .collect();
        let over = Tier::ALL.iter().position(|&tier| tier == current);
        self.show_menu(items, position, over, cx);
    }

    /// The user's pick on the level menu, while the field is in Ask.
    pub(in crate::shell) fn pick_tier(&mut self, tier: Tier, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        resting.tier = tier;
        log!("browser: composer level {}", tier.label());
        cx.notify();
    }

    /// The page in the card at `frame`: the mark, the field, Ask's line and the recent tasks.
    pub(in crate::shell) fn composer_page(
        &mut self,
        frame: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let width = f32::from(frame.size.width);
        let resting = self.resting.as_ref()?;
        let (bar, painted) = (resting.bar.clone(), resting.painted);
        let palette = self.palette;
        let light = palette.tone == Tone::Light;
        let (fill, near, far) = match light {
            true => (
                Hsla::from(palette.underlay).blend(palette.plate(URL_FIELD)),
                LIGHT_FIELD_SHADOW_NEAR,
                LIGHT_FIELD_SHADOW_FAR,
            ),
            false => (
                palette.plate(FIELD_FILL),
                FIELD_SHADOW_NEAR,
                FIELD_SHADOW_FAR,
            ),
        };
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
            .border_color(rim(&palette, FIELD_RIM))
            .shadow(vec![
                shadow(rgba(near).into(), 1., 3.),
                shadow(rgba(far).into(), 14., 36.),
            ])
            .text_size(px(FIELD_SIZE))
            .text_color(palette.text(1.))
            .child(div().flex_1().min_w_0().flex().child(field))
            .child(tab)
            .child(self.switch(on_ask, cx));
        let cards: Vec<Card> = match painted {
            true => (self.task_cards.iter())
                .filter(|card| card.replies)
                .take(CARDS as usize)
                .cloned()
                .collect(),
            false => Vec::new(),
        };
        let row = command_bar::centered_row_middle(f32::from(self.viewport.height))
            - f32::from(frame.top());
        let plates = cards.iter().any(|card| card.last_site.is_some());
        let top = group_top(row, f32::from(frame.size.height), !cards.is_empty(), plates);
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(top))
            .child(super::mark(1., &palette).mb(px(MARK_GAP)))
            .child(field)
            .child(self.ask_line(on_ask, line_width, cx));
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
                    .id("composer-tier")
                    .on_click(cx.listener(|shell, event: &gpui::ClickEvent, _, cx| {
                        shell.tier_menu(event.position(), cx)
                    }))
                    .child(self.composer_tier().label())
                    .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON)),
            )
            .child(div().flex_1())
            .children(assistant.map(|assistant| {
                chip()
                    .child(assistant)
                    .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON))
            }))
    }

    /// A recent task's card: its state, its title and its last steps, the one under way in full
    /// ink (or its status, with no steps this turn), over a plate naming the site of its last
    /// page; a task that has reached none ends after its words. A click opens it.
    fn recent_card(
        &self,
        index: usize,
        card: Card,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = self.palette;
        let state = card.word.alone(&card.name);
        let dot = (card.dot == Dot::Working).then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
        });
        let working = card.dot == Dot::Working;
        let lines: Vec<(SharedString, bool)> = match card.steps.is_empty() {
            true => vec![(card.status.clone(), true)],
            false => {
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
        let plate = card.last_site.clone().map(|site| {
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
                .child(site)
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

/// How far under a card `height` tall the page's group starts, with its field's row's middle
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
    let above_field = super::MARK_HEIGHT + MARK_GAP;
    let group = above_field + FIELD_HEIGHT + LINE_TOP + CHIP_HEIGHT + below;
    let level = row - FIELD_HEIGHT / 2. - above_field;
    level.min(height - TOP - group).max(TOP)
}

/// A plate's rim: white `dark` on a dark window, the ink on a light one, where white doesn't show.
fn rim(palette: &Palette, dark: u32) -> Hsla {
    match palette.tone {
        Tone::Dark => rgba(dark).into(),
        Tone::Light => palette.plate(LIGHT_RIM),
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
