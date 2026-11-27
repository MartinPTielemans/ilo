//! A composer's row of choices, one code for every composer: the grown bar's, a thread's stage's
//! and the panel's beside a page (spec-bar-composer.md, spec-thread-stage.md). Each draws it at
//! its own sizes (`Form`) and answers its triggers itself; the items, their order, their plates
//! and the level menu's rows are this file's alone.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    Pixels, Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, canvas, div,
    prelude::FluentBuilder, px, svg,
};

use crate::agents::tier::Tier;
use crate::shell::MenuEntry;
use crate::tokens::command_bar::{
    CHIP_RADIUS, CHOICES_GAP, CHOICES_HEIGHT, CHOICES_LEFT, CHOICES_RIGHT, CHOICES_UNDER,
    MODEL_GAP, MODEL_LEFT, MODEL_MARK, START_ARROW, START_SIZE, START_SPACE, TRIGGER_CHEVRON,
    TRIGGER_GAP, TRIGGER_HEIGHT, TRIGGER_LINE, TRIGGER_TEXT, TRIGGER_X,
};

/// The line under the levels every level holds to, which picks nothing.
pub const NEVER_GETS: &str = "A thread never gets passwords, cookies or your other tabs.";

/// The sizes one composer draws its row at.
#[derive(Clone, Copy, Debug)]
pub struct Form {
    pub height: f32,
    /// Of `height`, what's under the items.
    pub under: f32,
    pub left: f32,
    pub right: f32,
    pub gap: f32,
    /// The level trigger's inset at each side, and its words' size and line.
    pub level_x: f32,
    pub text: f32,
    pub line: f32,
    /// Whether the model trigger leads with its assistant's mark, and its words are in text-2.
    pub mark: bool,
    pub model_dim: bool,
    /// The round button, its arrow, and Stop's square with its corners.
    pub round: f32,
    pub arrow: f32,
    pub square: f32,
    pub square_radius: f32,
    /// What stands between the round button and the row's other items, beyond the row's gap.
    pub round_space: f32,
    /// Whether the model trigger draws the model's own word alone, without its assistant's name,
    /// its whole words still spoken; it's then the one item that gives way where the row hasn't
    /// room, cut at its end (spec-bar-composer.md, "The model's trigger in the panel beside a page").
    pub model_word: bool,
}

impl Form {
    /// The grown bar's row.
    pub const BAR: Form = Form {
        height: CHOICES_HEIGHT,
        under: CHOICES_UNDER,
        left: CHOICES_LEFT,
        right: CHOICES_RIGHT,
        gap: CHOICES_GAP,
        level_x: TRIGGER_X,
        text: TRIGGER_TEXT,
        line: TRIGGER_LINE,
        mark: true,
        model_dim: false,
        round: START_SIZE,
        arrow: START_ARROW,
        square: 0.,
        square_radius: 0.,
        round_space: START_SPACE,
        model_word: false,
    };
}

/// The round button at the row's end: Start or Send, live with words; or Stop, while the thread
/// works and nothing is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Go { ready: bool },
    Stop,
}

/// What the row is drawn in: its words, its dimmer words, a trigger's plate and the idle round
/// button's fill, and `primary` with what's on it.
#[derive(Clone, Copy)]
pub struct Inks {
    pub text: Hsla,
    pub dim: Hsla,
    pub plate: Hsla,
    pub primary: Hsla,
    pub on_primary: Hsla,
}

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A trigger: its element's id, where it was drawn for its menu to hang from, and what a click
/// does, if anything: a trigger whose choice a thread can't change reads it and opens nothing.
pub struct Trigger {
    pub id: &'static str,
    pub at: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub click: Option<Click>,
}

/// One row, as a composer has it now.
pub struct Row {
    pub form: Form,
    pub inks: Inks,
    pub a11y: bool,
    /// The plus before the level (`crate::plus`), disabled in its place where it adds nothing;
    /// none where a composer draws none.
    pub plus: Option<Stateful<Div>>,
    /// What the plus added, between it and the level.
    pub added: Vec<Added>,
    pub level: (Tier, Trigger),
    /// The model's words, with the assistant's mark where the form leads with it, then the model's
    /// own word, which a form that says less draws in their place; none where the app doesn't
    /// know the model, an outside agent's session.
    pub model: Option<(Option<&'static str>, SharedString, SharedString, Trigger)>,
    /// The narrowest the model's own word gets where the form lets it give way: the width of its
    /// first word.
    pub model_least: f32,
    /// The effort's words and trigger, after the model, for an assistant whose program takes one;
    /// none for any other.
    pub effort: Option<(SharedString, Trigger)>,
    /// What stands between the level and the spring: what a thread holds, such as its folder.
    pub held: Vec<AnyElement>,
    /// The trigger whose menu is open, which alone wears its plate.
    pub open: Option<&'static str>,
    pub round: (&'static str, SharedString, Round, Click),
    /// Where the round button was drawn, told each frame.
    pub round_at: Option<Box<dyn Fn(Bounds<Pixels>) + 'static>>,
}

/// What the plus added, as a trigger: its words, which open its menu. One that `gives_way`
/// shrinks first, its words cut short, where the row hasn't room.
pub struct Added {
    pub said: SharedString,
    pub trigger: Trigger,
    pub gives_way: bool,
}

/// How a composer's plus is drawn among its row's triggers: square as a trigger is tall, its
/// glyph `glyph` wide in the row's dim ink, on its `plate` while its menu is open, and its menu
/// `drop` under it.
pub fn plus_look(dim: Hsla, plate: Hsla, glyph: f32, drop: f32, a11y: bool) -> crate::plus::Look {
    crate::plus::Look {
        side: TRIGGER_HEIGHT,
        radius: CHIP_RADIUS,
        glyph,
        ink: dim,
        open: plate,
        off: crate::tokens::command_bar::PLUS_OFF,
        drop,
        a11y,
    }
}

/// The row: the plus and what it added, the level, what the thread holds, a spring, the model,
/// the effort, then the round button.
pub fn row(row: Row) -> gpui::Div {
    let Row {
        form,
        inks,
        a11y,
        plus,
        added,
        level: (tier, level),
        model,
        model_least,
        effort,
        held,
        open,
        round: (round_id, round_said, round, go),
        round_at,
    } = row;
    let words = |said: SharedString, color: Hsla| {
        div()
            .flex_none()
            .text_size(px(form.text))
            .line_height(px(form.line))
            .text_color(color)
            .child(said)
    };
    let trigger = |trigger: Trigger| {
        let Trigger { id, at, click } = trigger;
        let opens = click.is_some();
        div()
            .id(id)
            .relative()
            .h(px(TRIGGER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .rounded(px(CHIP_RADIUS))
            // A trigger has no fill at rest, only while its menu is open.
            .when(open == Some(id), |shown| shown.bg(inks.plate))
            .child(drawn_at(at))
            .when_some(click, |trigger, click| trigger.on_click(click))
            .when(!opens, |trigger| trigger.cursor_default())
    };
    let added = added.into_iter().map(|added| {
        let Added {
            said,
            trigger: at,
            gives_way,
        } = added;
        trigger(at)
            .when(a11y, |added| {
                added.role(Role::Button).aria_label(said.clone())
            })
            .when(gives_way, |added| added.flex_shrink(1.).min_w_0())
            .px(px(TRIGGER_X))
            .gap(px(TRIGGER_GAP))
            .child(words(said, inks.text).when(gives_way, |words| {
                words.flex_shrink(1.).min_w_0().truncate()
            }))
            .child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, inks.dim))
    });
    let level_said: SharedString = tier.label().into();
    let level = trigger(level)
        .when(a11y, |level| {
            level.role(Role::Button).aria_label(level_said.clone())
        })
        .px(px(form.level_x))
        .gap(px(TRIGGER_GAP))
        .child(words(level_said, inks.text))
        .child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, inks.text));
    let model_ink = if form.model_dim { inks.dim } else { inks.text };
    let model = model.map(|(mark, said, word, model)| {
        let mark = mark.filter(|_| form.mark);
        let drawn = if form.model_word { word } else { said.clone() };
        // A chevron, and a button to a screen reader, only on a trigger that opens; one that
        // can't is the model's words alone, spoken whole where fewer are drawn.
        let opens = model.click.is_some();
        let fewer = drawn != said;
        let left = if mark.is_some() {
            MODEL_LEFT
        } else {
            form.level_x
        };
        // Where it gives way, the trigger is never narrower than its insets, its mark, its
        // chevron and the least of its words: nowrap words would hold it at their whole width.
        let least = left
            + form.level_x
            + model_least
            + mark.map_or(0., |_| MODEL_MARK + MODEL_GAP)
            + if opens {
                TRIGGER_GAP + TRIGGER_CHEVRON
            } else {
                0.
            };
        trigger(model)
            .when(a11y && opens, |model| {
                model.role(Role::Button).aria_label(said.clone())
            })
            .when(a11y && !opens && fewer, |model| {
                model.role(Role::Label).aria_label(said.clone())
            })
            .pl(px(left))
            .pr(px(form.level_x))
            // Its chevron stands the level's gap after its words; its mark, the model's own.
            .gap(px(TRIGGER_GAP))
            // The row's spring, so nothing stands between it and the level but the row's gap.
            .ml_auto()
            // Where the form lets them, the model's words give way first, cut at their end, and
            // the trigger with them; its name to a screen reader stays whole.
            .when(form.model_word, |model| {
                model.flex_shrink(1.).min_w(px(least))
            })
            .children(mark.map(|mark| {
                div()
                    .flex_none()
                    .mr(px(MODEL_GAP - TRIGGER_GAP))
                    .child(glyph(mark, MODEL_MARK, inks.text))
            }))
            .child(words(drawn, model_ink).when(form.model_word, |words| {
                words.flex_shrink(1.).min_w(px(model_least)).truncate()
            }))
            .when(opens, |model| {
                model.child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, inks.dim))
            })
    });
    // The effort wears the dim ink before a pick and after, as the model's chevron does.
    let effort = effort.map(|(said, effort)| {
        trigger(effort)
            .when(a11y, |effort| {
                effort.role(Role::Button).aria_label(said.clone())
            })
            .px(px(TRIGGER_X))
            .gap(px(TRIGGER_GAP))
            .child(words(said, inks.dim))
            .child(glyph("icons/chevron-down.svg", TRIGGER_CHEVRON, inks.dim))
    });
    let round = round_button(
        form,
        inks,
        a11y,
        (round_id, round_said, round, go),
        round_at,
    )
    .ml(px(form.round_space));
    div()
        .h(px(form.height))
        .flex_none()
        .flex()
        .items_center()
        .pb(px(form.under))
        .pl(px(form.left))
        .pr(px(form.right))
        .gap(px(form.gap))
        .children(plus)
        .children(added)
        .child(level)
        .children(held)
        .when(model.is_none(), |row| row.child(div().flex_1()))
        .children(model)
        .children(effort)
        .child(round)
}

/// The round button in `form`: Start or Send, or Stop, an icon button (#4402), bare with its
/// square in the text's ink and a plate under the pointer, told where it's drawn by `at`.
pub fn round_button(
    form: Form,
    inks: Inks,
    a11y: bool,
    (id, said, round, go): (&'static str, SharedString, Round, Click),
    at: Option<Box<dyn Fn(Bounds<Pixels>) + 'static>>,
) -> Stateful<Div> {
    let (fill, ink) = match round {
        Round::Go { ready: true } => (inks.primary, inks.on_primary),
        Round::Go { ready: false } => (inks.plate, inks.dim),
        Round::Stop => (gpui::transparent_black(), inks.text),
    };
    let face = match round {
        Round::Stop => div()
            .size(px(form.square))
            .rounded(px(form.square_radius))
            .bg(ink)
            .into_any_element(),
        Round::Go { .. } => glyph("icons/arrow-up.svg", form.arrow, ink),
    };
    div()
        .id(id)
        .when(a11y, |round| round.role(Role::Button).aria_label(said))
        .relative()
        .size(px(form.round))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(fill)
        .when(round == Round::Stop, |stop| {
            stop.hover(move |stop| stop.bg(ink.opacity(crate::tokens::button::ICON_HOVERED)))
        })
        .on_click(go)
        .child(face)
        .when_some(at, |round, told| {
            round.child(
                canvas(move |bounds, _, _| told(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
        })
}

/// The level trigger's menu: the three levels, `current` checked, then the line every level holds
/// to, which picks nothing. Its rows' indexes are `Tier::ALL`'s.
pub fn level_entries(current: Tier) -> Vec<MenuEntry> {
    let mut items: Vec<MenuEntry> = (Tier::ALL.into_iter())
        .map(|tier| MenuEntry {
            checked: tier == current,
            ..MenuEntry::named(tier.label(), true)
        })
        .collect();
    items.push(MenuEntry::line());
    items.push(MenuEntry::named(NEVER_GETS, false));
    items
}

/// Records where the element it's put in is drawn, for a menu to hang from it.
pub fn drawn_at(at: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

/// The glyph at `path`, `size` square, in `color`.
fn glyph(path: &'static str, size: f32, color: Hsla) -> AnyElement {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}
