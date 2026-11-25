//! A task's card in Today's list, the one place the list puts it (spec-thread-card). Whole, it is
//! an outlined card: its mark and title, a line in ilo's words under them, and its pages or its one
//! button under that. Folded, it is one outlined line. Its rim, mark and button come from its
//! task's state alone; the title is the only words of the assistant's on it.

use gpui::{
    AnyElement, FontWeight, HighlightStyle, Hsla, MouseButton, SharedString, StyledText, Window,
    canvas, div, prelude::*, px, rgb,
};

use super::glyph::{self, Glyph, Mark, MarkInks, State};
use super::{
    Chip, Form, PageWord, SLOT, Shell, chip, clock, drawn_listener, number, state_lead, state_line,
};
use crate::agents::task::ilo::IloButton;
use crate::agents::task::{Card, Dot, NEEDS_YOU};
use crate::shell::conversation::words;
use crate::shell::page_ends::StripStop;
use crate::shell::{FAVICON_SIZE, PLATE_INSET, Slot, TILE, TITLE, dashed_plate};
use crate::tokens::shell::FAVICON_DRAWN;
use crate::tokens::shell::task::{CHIP_HEIGHT, CHIP_X, PAGE_RADIUS, SLOT_DASH};
use crate::tokens::shell::task_card::motion::TURN;
use crate::tokens::shell::task_card::{
    BUTTON_HEIGHT, BUTTON_LINE, BUTTON_RADIUS, BUTTON_X, COUNT_LINE, DONE_LINE, DOT, FILL, GAP,
    GLYPH, GLYPH_GAP, ICON, ICON_GAP, LINE_AROUND, LINE_X, LIVE_LINE, ON_PRIMARY, ON_PRIMARY_LIGHT,
    OUTLINE, OUTLINE_WIDTH, PADDING, RADIUS, RIM, RIM_WIDTH, TEXT_2, TEXT_2_LIGHT, TEXT_3,
    TEXT_3_LIGHT, TITLE_LINE, UNDER, UNVISITED,
};
use crate::tokens::text;

/// A waiting card's second line: the site of the page it waits on and the state's word (words
/// table, Z5).
fn needs_you(site: &str) -> SharedString {
    match site.is_empty() {
        true => NEEDS_YOU.into(),
        false => format!("{site} · {NEEDS_YOU}").into(),
    }
}

/// A done card's second line, with no finding: when it ended (Z6, after the wording pass).
fn done_at(card: &Card) -> SharedString {
    match card.ended_at.is_empty() {
        true => state_line(card),
        false => format!("Done · {}", card.ended_at).into(),
    }
}

/// Where `text`'s first sentence ends: after its first `.`, `!` or `?` that ends a word, or at its
/// end.
pub(super) fn first_sentence(text: &str) -> usize {
    let ends = text.char_indices().find(|&(at, c)| {
        matches!(c, '.' | '!' | '?') && text[at + c.len_utf8()..].starts_with(' ')
    });
    ends.map_or(text.len(), |(at, c)| at + c.len_utf8())
}

/// How many of its pages a task at work has been on, of them all (Z1), left out while it has one.
pub(super) fn count(card: &Card) -> Option<SharedString> {
    let all = card.pages.len();
    let visited = (card.pages.iter())
        .filter(|tab| card.visited.contains(tab))
        .count();
    (all > 1).then(|| format!("{visited} of {all}").into())
}

/// The second line of a whole card in `form`: ilo's words alone.
pub(super) fn second_line(card: &Card, form: Form, site: &str) -> SharedString {
    match form {
        Form::Working if !card.status.is_empty() => card.status.clone(),
        Form::Waits => needs_you(site),
        Form::DoneUnseen if !card.finding.is_empty() => card.finding.clone(),
        Form::DoneUnseen => done_at(card),
        _ => state_line(card),
    }
}

/// What a card shows of its task's state: its mark, its rim, its button and its dot, read from
/// the state alone, which the drawing and the tests both take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Look {
    pub(super) mark: Option<Mark>,
    pub(super) rim: bool,
    pub(super) button: bool,
    pub(super) dot: bool,
}

/// What `card` in `form` shows of its task's state. An ended task has today's glyph, which isn't a
/// card's mark.
pub(super) fn look(card: &Card, form: Form, unseen: bool) -> Look {
    Look {
        mark: mark(card.dot, unseen),
        rim: form == Form::Waits,
        button: form == Form::Waits && !card.pages.is_empty(),
        dot: form == Form::DoneUnseen,
    }
}

/// The mark of a task in state `dot`, its ring at the top: none for an ended one, which has
/// today's glyph.
fn mark(dot: Dot, unseen: bool) -> Option<Mark> {
    match dot {
        Dot::Working => Some(Mark::Ring { turns: 0. }),
        Dot::Waiting => Some(Mark::Pause),
        Dot::Done => Some(Mark::Check { unseen }),
        Dot::Ended => None,
    }
}

impl Shell {
    /// The marks' colours.
    fn mark_inks(&self) -> MarkInks {
        MarkInks {
            ink: Hsla::from(self.palette.ink),
            on_primary: self.on_primary(),
        }
    }

    /// A filled button's label's ink.
    fn on_primary(&self) -> Hsla {
        rgb(self.palette.toned(ON_PRIMARY, ON_PRIMARY_LIGHT)).into()
    }

    /// How far round the ring at work is now: it holds at the top with motion reduced.
    fn ring_turns(&self) -> f32 {
        match self.task_list.still {
            true => 0.,
            false => {
                let gone = clock().saturating_duration_since(self.task_list.epoch);
                (gone.as_secs_f32() / TURN).fract()
            }
        }
    }

    /// A card's mark for its task's state, in the card's glyph box: an ended task's is today's
    /// glyph at rest.
    pub(super) fn card_mark(&self, dot: Dot, unseen: bool) -> AnyElement {
        let mark = match mark(dot, unseen) {
            Some(Mark::Ring { .. }) => Mark::Ring {
                turns: self.ring_turns(),
            },
            Some(mark) => mark,
            None => {
                let now = clock();
                let drawn = Glyph::new(State::from(dot), now, true).drawn(now);
                let inks = self.glyph_inks();
                let paint = move |bounds, _, window: &mut Window, _: &mut gpui::App| {
                    glyph::paint(&drawn, bounds, inks, false, window)
                };
                return canvas(|_, _, _| {}, paint)
                    .flex_none()
                    .size(px(GLYPH))
                    .into_any_element();
            }
        };
        let inks = self.mark_inks();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| glyph::paint_mark(mark, bounds, inks, window),
        )
        .flex_none()
        .size(px(GLYPH))
        .into_any_element()
    }

    /// Text-2 and text-3: the ink a step and two down, each a step stronger on a light window.
    fn card_inks(&self) -> (Hsla, Hsla) {
        let palette = &self.palette;
        (
            palette.text(palette.toned(TEXT_2, TEXT_2_LIGHT)),
            palette.text(palette.toned(TEXT_3, TEXT_3_LIGHT)),
        )
    }

    /// The card numbered `n` in the list of the Space shown, in its place: whole or a line, or ⌘T's
    /// slot, sliding with the rows as they make room.
    pub(in crate::shell) fn task_card(&self, n: u64, cx: &mut Context<Self>) -> AnyElement {
        let placed = (self.shown_space())
            .and_then(|space| space.placed.iter().find(|card| card.n == n))
            .cloned();
        let block = div()
            .id(("task-card", n))
            .flex_none()
            .relative()
            .top(px(self.slide(Slot::Task(n))));
        let Some(placed) = placed else {
            return block.into_any_element();
        };
        let block = block.h(px(placed.room)).px(px(PLATE_INSET));
        if n == SLOT {
            return block.child(self.slot_plate()).into_any_element();
        }
        let Some(card) = self
            .task_cards
            .iter()
            .find(|card| card.agent == placed.agent)
        else {
            return block.into_any_element();
        };
        let now = clock();
        let coming = self.task_list.coming.get(&card.agent);
        let rest = self.slide(Slot::Task(n)) == 0. && coming.is_none_or(|ease| !ease.moving(now));
        let opacity = coming.map_or(1., |ease| ease.at(now));
        let body = match placed.form.line() {
            true => self.card_line(card, placed.form),
            false => self.card_whole(card, placed.form, cx),
        };
        let unseen = self.task_list.unseen.contains(&card.agent);
        let (outline, width) = match look(card, placed.form, unseen).rim {
            true => (RIM, RIM_WIDTH),
            false => (OUTLINE, OUTLINE_WIDTH),
        };
        let height = placed.room
            - if placed.form.line() {
                2. * LINE_AROUND
            } else {
                UNDER
            };
        let agent = card.agent.clone();
        let name: SharedString = format!("{}, {}", card.title, self.said(card, placed.form)).into();
        let frame = div()
            .id(("task-plate", n))
            .relative()
            .h(px(height))
            .when(placed.form.line(), |frame| frame.mt(px(LINE_AROUND)))
            .rounded(px(RADIUS))
            .border(px(width))
            .border_color(Hsla::from(self.palette.ink).opacity(outline))
            .bg(self.palette.plate(FILL))
            .opacity(opacity)
            .cursor_pointer();
        let drawn = drawn_listener(
            &self.task_list.plates_drawn,
            &self.task_list.rested,
            n,
            rest,
        );
        let frame = self
            .strip_stop(frame, StripStop::Header(n), name, RADIUS)
            .child(drawn)
            .child(body)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command show-task via task card");
                shell.show_group(agent.clone(), cx)
            }));
        block.child(frame).into_any_element()
    }

    /// What a card says of its state to the screen reader, after its title.
    fn said(&self, card: &Card, form: Form) -> SharedString {
        let site = self.waiting_site(card);
        second_line(card, form, &site)
    }

    /// The site of the page a waiting task waits on, as the rail names it.
    fn waiting_site(&self, card: &Card) -> String {
        (self.waiting_page(card))
            .map(|tab| crate::agents::site(&self.page(tab).url))
            .unwrap_or_default()
    }

    /// The slot ⌘T's bar folds into: its place in dashes, which go as the bar's plate lands in it
    /// and its fill takes over.
    fn slot_plate(&self) -> impl IntoElement + use<> {
        let (fill, dashed) = self.landing_slot(clock());
        let dashes = (dashed > 0.).then(|| {
            let color = self.palette.glyph(SLOT_DASH);
            dashed_plate(color.opacity(dashed), RADIUS)
                .absolute()
                .inset_0()
        });
        let rest = !self.task_list.fill.moving(clock());
        let drawn = drawn_listener(
            &self.task_list.plates_drawn,
            &self.task_list.rested,
            SLOT,
            rest,
        );
        div()
            .relative()
            .h(px(super::slot_room() - UNDER))
            .rounded(px(RADIUS))
            .bg(self.palette.plate(TILE).opacity(fill))
            .children(dashes)
            .child(drawn)
    }

    /// A card's title on its line: one line, cut with an ellipsis.
    fn card_title(&self, card: &Card, weight: FontWeight) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(text::BODY))
            .line_height(px(TITLE_LINE))
            .font_weight(weight)
            .text_color(self.palette.text(TITLE))
            .child(card.title.clone())
    }

    /// A page's icon, from memory alone, at `alpha`.
    fn card_icon(&self, tab: u64, alpha: f32) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .size(px(ICON))
            .flex()
            .items_center()
            .justify_center()
            .opacity(alpha)
            .child(self.tab_icon_sized(tab, ICON * FAVICON_SIZE / FAVICON_DRAWN))
    }

    /// A whole card's rows: the mark and title, ilo's line, and its pages or its button.
    fn card_whole(&self, card: &Card, form: Form, cx: &mut Context<Self>) -> AnyElement {
        let unseen = form == Form::DoneUnseen || form == Form::Ended;
        let (text_2, text_3) = self.card_inks();
        let width = if form == Form::Waits {
            RIM_WIDTH
        } else {
            OUTLINE_WIDTH
        };
        let dot = look(card, form, unseen).dot.then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(Hsla::from(self.palette.ink))
        });
        let first = div()
            .h(px(TITLE_LINE))
            .flex()
            .items_center()
            .gap(px(GLYPH_GAP))
            .child(self.card_mark(card.dot, unseen))
            .child(self.card_title(card, FontWeight::SEMIBOLD))
            .children(dot);
        let words = self.said(card, form);
        let finding = form == Form::DoneUnseen && !card.finding.is_empty();
        let second = match finding {
            true => self.card_finding(card, text_2).into_any_element(),
            false => {
                let lead = state_lead(&words);
                let line = match form {
                    Form::DoneUnseen => DONE_LINE,
                    _ => LIVE_LINE,
                };
                let led = HighlightStyle {
                    color: Some(self.palette.text(TITLE)),
                    font_weight: Some(FontWeight::MEDIUM),
                    ..HighlightStyle::default()
                };
                div()
                    .h(px(line))
                    .truncate()
                    .text_size(px(text::SMALL))
                    .line_height(px(line))
                    .text_color(text_2)
                    .child(match lead {
                        0 => StyledText::new(words),
                        lead => StyledText::new(words).with_highlights([(0..lead, led)]),
                    })
                    .into_any_element()
            }
        };
        let third = self.card_third(card, form, text_3, cx);
        div()
            .size_full()
            .p(px(PADDING - width))
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(first)
            .child(div().pl(px(GLYPH + GLYPH_GAP)).child(second))
            .children(third.map(|third| div().pl(px(GLYPH + GLYPH_GAP)).child(third)))
            .into_any_element()
    }

    /// A done card's finding, the agent's own words: its first sentence in the title's ink and the
    /// rest in text-2, wrapped to the lines the list made room for and cut there. No word of it is
    /// set heavier, as ilo's lead is, so it can't pose as ilo's.
    fn card_finding(&self, card: &Card, text_2: Hsla) -> impl IntoElement + use<> {
        let lines = (self.task_list.finding_lines.get(&card.agent))
            .copied()
            .unwrap_or(1);
        let words = card.finding.clone();
        let first = first_sentence(&words);
        let ink = HighlightStyle {
            color: Some(self.palette.text(TITLE)),
            ..HighlightStyle::default()
        };
        div()
            .h(px(DONE_LINE * lines as f32))
            .overflow_hidden()
            .text_size(px(text::SMALL))
            .line_height(px(DONE_LINE))
            .text_color(text_2)
            .line_clamp(lines)
            .text_ellipsis()
            .child(StyledText::new(words).with_highlights([(0..first, ink)]))
    }

    /// A whole card's third row, if it has one: at work its pages, those not yet visited faint,
    /// and how many it has been on; waiting the page it waits on and Show Page; done or ended the
    /// pages it visited, and an ended task's chip.
    fn card_third(
        &self,
        card: &Card,
        form: Form,
        text_3: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let row = || div().flex().items_center().gap(px(ICON_GAP));
        let visited = || {
            (card.pages.iter())
                .filter(|tab| card.visited.contains(tab))
                .map(|&tab| self.card_icon(tab, 1.))
                .collect::<Vec<_>>()
        };
        let n = number(&card.agent);
        match form {
            Form::Working if !card.pages.is_empty() => {
                let icons = card.pages.iter().map(|&tab| {
                    let alpha = if card.visited.contains(&tab) {
                        1.
                    } else {
                        UNVISITED
                    };
                    self.card_icon(tab, alpha)
                });
                let count = count(card).map(|count| {
                    div()
                        .flex_none()
                        .text_size(px(text::SMALL))
                        .line_height(px(COUNT_LINE))
                        .text_color(text_3)
                        .child(count)
                });
                let row = row()
                    .h(px(ICON.max(COUNT_LINE)))
                    .children(icons)
                    .child(div().flex_1())
                    .children(count);
                Some(row.into_any_element())
            }
            Form::Waits if look(card, form, false).button => {
                let tab = self.waiting_page(card)?;
                let agent = card.agent.clone();
                let label: SharedString = words::button(IloButton::ShowPage).into();
                let button = div()
                    .id(("task-show-page", n))
                    .flex_none()
                    .h(px(BUTTON_HEIGHT))
                    .px(px(BUTTON_X))
                    .flex()
                    .items_center()
                    .rounded(px(BUTTON_RADIUS))
                    .bg(Hsla::from(self.palette.ink))
                    .text_size(px(text::SMALL))
                    .line_height(px(BUTTON_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.on_primary());
                let button = self
                    .strip_stop(button, StripStop::Chip(n), label.clone(), BUTTON_RADIUS)
                    .child(label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.card_button(agent.clone(), cx);
                    }));
                let row = row()
                    .h(px(BUTTON_HEIGHT))
                    .child(self.card_icon(tab, 1.))
                    .child(div().flex_1())
                    .child(button);
                Some(row.into_any_element())
            }
            Form::DoneUnseen if !card.visited.is_empty() => {
                Some(row().h(px(ICON)).children(visited()).into_any_element())
            }
            Form::Ended => {
                let chip = chip(card).map(|chip| self.card_chip(card, chip, cx));
                if chip.is_none() && card.visited.is_empty() {
                    return None;
                }
                let row = row()
                    .h(px(if chip.is_some() {
                        CHIP_HEIGHT.max(ICON)
                    } else {
                        ICON
                    }))
                    .children(visited())
                    .child(div().flex_1())
                    .children(chip);
                Some(row.into_any_element())
            }
            _ => None,
        }
    }

    /// An ended task's one chip, today's: its words on a quiet plate, not a filled button.
    fn card_chip(&self, card: &Card, chip: Chip, cx: &mut Context<Self>) -> AnyElement {
        let agent = card.agent.clone();
        let n = number(&agent);
        let hover = self.palette.plate(crate::shell::HOVERED + TILE);
        let button = div()
            .id(("task-chip", n))
            .flex_none()
            .min_w_0()
            .h(px(CHIP_HEIGHT))
            .px(px(CHIP_X))
            .flex()
            .items_center()
            .rounded(px(PAGE_RADIUS))
            .bg(self.palette.plate(crate::shell::HOVERED))
            .hover(move |style| style.bg(hover))
            .text_size(px(text::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .text_color(self.palette.text(TITLE));
        self.strip_stop(button, StripStop::Chip(n), chip.label(), PAGE_RADIUS)
            .child(div().truncate().child(chip.label()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.run_chip(agent.clone(), chip, cx);
            }))
            .into_any_element()
    }

    /// A card folded to one line: its mark, its title, and, at work, the icon of the page it's on.
    fn card_line(&self, card: &Card, form: Form) -> AnyElement {
        let weight = match form {
            Form::WorkingLine => FontWeight::SEMIBOLD,
            _ => FontWeight::NORMAL,
        };
        let on = (form == Form::WorkingLine)
            .then(|| self.working_page(card))
            .flatten()
            .map(|tab| self.card_icon(tab, 1.));
        div()
            .size_full()
            .px(px(LINE_X - OUTLINE_WIDTH))
            .flex()
            .items_center()
            .gap(px(GLYPH_GAP))
            .child(self.card_mark(card.dot, false))
            .child(self.card_title(card, weight))
            .children(on)
            .into_any_element()
    }

    /// The page a task at work is on: the one with the outline, else its last step's.
    fn working_page(&self, card: &Card) -> Option<u64> {
        let outlined = |tab: &&u64| {
            let mark = self.page_mark(card, **tab);
            mark.outline && mark.word == PageWord::Working
        };
        (card.pages.iter().find(outlined))
            .or(self.agents_at.get(&card.agent))
            .or(card.pages.first())
            .copied()
    }
}
