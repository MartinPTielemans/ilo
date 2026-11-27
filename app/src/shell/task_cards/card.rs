//! A task's line in Today's list, the one place the list puts it (#4095). It is a tab's line: its
//! mark and title, with no fill or outline but the shown thread's shape, and while it needs the
//! user a second line in the product's words. Its mark and second line come from its task's state
//! alone; the title is the only words of the assistant's on it. An ended thread's chip is in its
//! conversation, under its last line.

use gpui::{
    AnyElement, FontWeight, Hsla, MouseButton, SharedString, Window, canvas, div, prelude::*, px,
    rgb,
};

use super::glyph::{self, Mark, MarkInks, State};
use super::{Form, SLOT, Shell, clock, drawn_listener};
use crate::agents::task::{Card, Dot, NEEDS_YOU};
use crate::palette::Palette;
use crate::shell::conversation::stage_fill;
use crate::shell::page_ends::StripStop;
use crate::shell::{PLATE_INSET, Slot, TILE, TITLE, dashed_plate};
use crate::tokens::shell::task::SLOT_DASH;
use crate::tokens::shell::task_card::motion::TURN;
use crate::tokens::shell::task_card::{
    DOT, FROM_INK, FROM_MARK, GLYPH, GLYPH_GAP, LINE_X, NEEDS_LINE, ON_PRIMARY, ON_PRIMARY_LIGHT,
    RADIUS, TEXT_2, TEXT_2_LIGHT, TITLE_LINE,
};
use crate::tokens::text;

/// A waiting line's second line: the state's word, then the site of the page it waits on, the
/// host alone, which a long one cuts and never the word (words table, N1).
fn needs_you(site: &str) -> SharedString {
    match site.is_empty() {
        true => NEEDS_YOU.into(),
        false => format!("{NEEDS_YOU} · {site}").into(),
    }
}

/// What the dot on a finished thread's line says: its answer hasn't been looked at here (words
/// table, N2).
pub(super) const UNREAD: &str = "Unread";

/// What a thread's row is called after its name and state, as a tab's is called a tab (N3).
pub(super) const THREAD: &str = "Thread";

/// What a card says to the screen reader after its title, as drawn (N3): at work the state's
/// word, which its mark draws; waiting its second line; finished and not seen, the dot's word
/// alone; at rest nothing.
pub(super) fn spoken(card: &Card, form: Form, site: &str) -> SharedString {
    match form {
        Form::Working => card.word.after_name().into(),
        Form::Waits => second_line(card, form, site),
        Form::Unseen => UNREAD.into(),
        Form::Rest => SharedString::default(),
    }
}

/// A waiting line's second line: the product's words alone, never the assistant's or a page's.
/// Empty for a line in any other `form`, which has none.
pub(super) fn second_line(_card: &Card, form: Form, site: &str) -> SharedString {
    match form {
        Form::Waits => needs_you(site),
        _ => SharedString::default(),
    }
}

/// What a card shows of its task's state: its mark, its second line and its dot, read from the
/// state alone, which the drawing and the tests both take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Look {
    pub(super) mark: Option<Mark>,
    /// The line under the title that says it needs the user.
    pub(super) needs: bool,
    pub(super) dot: bool,
    /// The outside app's mark at the line's right.
    pub(super) from: bool,
}

/// The product's mark for an outside app's session, in `palette`'s ink at `from-ink`: its card's
/// and its panel's.
pub(in crate::shell) fn from_mark(palette: &Palette) -> impl IntoElement + use<> {
    let colour = Hsla::from(palette.ink).opacity(FROM_INK);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| glyph::paint_from_mark(bounds, colour, window),
    )
    .flex_none()
    .size(px(FROM_MARK))
}

/// Whether `card` is an outside app's session rather than a thread: from the kind of its connection
/// alone, a session with no task's token, which is also why the user doesn't reply to it here.
/// Nothing the app says, its name included, decides it.
pub(super) fn outside(card: &Card) -> bool {
    !card.replies
}

/// What `card` in `form` shows of its task's state. An ended task has today's glyph, which isn't a
/// card's mark.
pub(super) fn look(card: &Card, form: Form, unseen: bool) -> Look {
    Look {
        mark: mark(card.dot, unseen),
        needs: form == Form::Waits,
        dot: form == Form::Unseen,
        from: outside(card),
    }
}

/// A line's frame: a shape of its own in its stage's colour, or none, as a tab's among the tabs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Framed {
    Own,
    Plain,
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
                let look = glyph::Look::of(State::from(dot));
                let inks = self.glyph_inks();
                let paint = move |bounds, _, window: &mut Window, _: &mut gpui::App| {
                    glyph::paint(look, bounds, inks, false, window)
                };
                return canvas(|_, _, _| {}, paint)
                    .flex_none()
                    .size(px(GLYPH))
                    .into_any_element();
            }
        };
        self.mark_canvas(mark)
    }

    /// `mark` in the card's glyph box, in the window's ink.
    pub(super) fn mark_canvas(&self, mark: Mark) -> AnyElement {
        let inks = self.mark_inks();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| glyph::paint_mark(mark, bounds, inks, window),
        )
        .flex_none()
        .size(px(GLYPH))
        .into_any_element()
    }

    /// Text-2: the ink a step down, a step stronger on a light window.
    fn rest_ink(&self) -> Hsla {
        let palette = &self.palette;
        palette.text(palette.toned(TEXT_2, TEXT_2_LIGHT))
    }

    /// The card numbered `n` in the list of the Space shown, in its place: its line, or ⌘T's
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
        let block = block.h(px(placed.room));
        if n == SLOT {
            return (block.px(px(PLATE_INSET)))
                .child(self.slot_plate())
                .into_any_element();
        }
        let Some(card) = self
            .task_cards
            .iter()
            .find(|card| card.agent == placed.agent)
        else {
            return block.px(px(PLATE_INSET)).into_any_element();
        };
        let framed = self.card_framed(card);
        let block = block.px(px(PLATE_INSET));
        let now = clock();
        let coming = self.task_list.coming.get(&card.agent);
        let rest = self.slide(Slot::Task(n)) == 0. && coming.is_none_or(|ease| !ease.moving(now));
        let opacity = coming.map_or(1., |ease| ease.at(now));
        let body = self.card_line(card, placed.form, framed == Framed::Own);
        let agent = card.agent.clone();
        // An outside app's name after the title, then what the card says of its state, as drawn.
        let mut name = card.title.to_string();
        if outside(card) {
            name = format!("{name}, {}", card.name);
        }
        let said = self.said(card, placed.form);
        if !said.is_empty() {
            name = format!("{name}, {said}");
        }
        let name = SharedString::from(name);
        let tip = outside(card).then(|| card.name.clone());
        let frame = div()
            .id(("task-plate", n))
            .relative()
            .h(px(placed.room))
            .rounded(px(RADIUS))
            .opacity(opacity)
            .cursor_pointer();
        let frame = match framed {
            Framed::Own => {
                let shape = self.in_own_colour(stage_fill(&self.palette), 1., RADIUS);
                // Placed from the frame's corner: left to itself it starts inside the room.
                frame.child(div().absolute().inset_0().child(shape))
            }
            Framed::Plain => {
                let hover = self.palette.hovered_row();
                frame.hover(move |style| style.bg(hover))
            }
        };
        let drawn = drawn_listener(
            &self.task_list.plates_drawn,
            &self.task_list.rested,
            n,
            rest,
        );
        let frame = self
            .strip_stop(frame, StripStop::Header(n), name, RADIUS)
            .when(self.a11y, |frame| {
                frame.a11y_synthetic_children(|builder| {
                    builder.parent_node().set_role_description(THREAD)
                })
            })
            .child(drawn)
            .child(body)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when_some(tip, |frame, tip| {
                frame.on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                    let drawn = shell.task_list.plates_drawn.borrow().get(&n).copied();
                    let tip = drawn.filter(|_| *hovered).map(|frame| {
                        let anchor = crate::tooltip::Anchor::under(frame, shell.viewport);
                        (crate::tooltip::Tip::named(tip.clone()), anchor)
                    });
                    shell.hover_tip(tip, cx);
                }))
            })
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command show-task via task card");
                shell.show_line(agent.clone(), cx)
            }));
        block.child(frame).into_any_element()
    }

    /// How `card` is framed: in its stage's colour while its thread is the one shown and the
    /// stage starts at the sidebar's edge; otherwise as a tab's line, with none.
    pub(super) fn card_framed(&self, card: &Card) -> Framed {
        match self.beside_page_area() && self.talk_agent().as_ref() == Some(&card.agent) {
            true => Framed::Own,
            false => Framed::Plain,
        }
    }

    /// What a card says of its state to the screen reader, after its title.
    fn said(&self, card: &Card, form: Form) -> SharedString {
        let site = self.waiting_site(card);
        spoken(card, form, &site)
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
            .h(px(super::slot_room()))
            .rounded(px(RADIUS))
            .bg(self.palette.plate(TILE).opacity(fill))
            .children(dashes)
            .child(drawn)
    }

    /// A card's title on its line in `ink`: one line, cut with an ellipsis.
    fn card_title(&self, card: &Card, weight: FontWeight, ink: Hsla) -> impl IntoElement + use<> {
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(text::BODY))
            .line_height(px(TITLE_LINE))
            .font_weight(weight)
            .text_color(ink)
            .child(card.title.clone())
    }

    /// A card as one line, a tab's: its mark and its title, set heavier while its thread is the
    /// one shown, as the open tab's is; while it needs the user, the line that says so under the
    /// title, in the text's ink, nothing in it to press.
    fn card_line(&self, card: &Card, form: Form, shown: bool) -> AnyElement {
        let weight = match shown {
            true => FontWeight::SEMIBOLD,
            false => FontWeight::NORMAL,
        };
        // At rest the title steps down to text-2, as a thread that needs nothing more; shown, it
        // reads on its stage as the shown tab's does.
        let ink = match form == Form::Rest && !shown {
            true => self.rest_ink(),
            false => self.palette.text(TITLE),
        };
        let unseen = form == Form::Unseen;
        let dot = unseen.then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(Hsla::from(self.palette.ink))
        });
        // An outside app's mark at the right; its name is the line's tooltip and spoken label, not
        // drawn.
        let from = outside(card).then(|| from_mark(&self.palette));
        let first = div()
            .h(px(TITLE_LINE))
            .flex()
            .items_center()
            .gap(px(GLYPH_GAP))
            .child(self.card_mark(card.dot, unseen))
            .child(self.card_title(card, weight, ink))
            .children(dot)
            .children(from);
        let needs = look(card, form, unseen).needs.then(|| {
            let words = second_line(card, form, &self.waiting_site(card));
            div()
                .h(px(NEEDS_LINE))
                .pl(px(GLYPH + GLYPH_GAP))
                .truncate()
                .text_size(px(text::SMALL))
                .line_height(px(NEEDS_LINE))
                .text_color(self.palette.text(TITLE))
                .child(words)
        });
        // The title where it sits on any line, and the second line under it, as much room above
        // the pair as below.
        div()
            .size_full()
            .px(px(LINE_X))
            .flex()
            .flex_col()
            .justify_center()
            .child(first)
            .children(needs)
            .into_any_element()
    }
}
