//! The Settings window's groups of rows and their controls, which the views it shows from the
//! library, such as the import, draw with too.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    BoxShadow, Rgba, SharedString,
    accesskit::{HasPopup, Role, Toggled},
    canvas, div, point,
    prelude::*,
    px, rgb, rgba, svg,
};

use crate::agents::request::Sent;
use crate::palette::{INK, NO_THEME_ACCENT, OVERLAY_DARK, PAPER};
use crate::tokens::palette::{ACCENT_HOVERED, ACCENT_PRESSED, DIALOG_LIGHT};
use crate::tokens::radius;
use crate::tokens::settings::{
    BUTTON_GLYPH, CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHEVRONS, CHEVRONS_ALPHA,
    CHEVRONS_END, CHIP, CHIP_FIXED, CHIP_FIXED_TEXT, CHIP_HEIGHT, CHIP_NONE_TEXT, CHIP_PADDING,
    CHIP_RADIUS, CHIP_TEXT, CHIP_WIDTH, CONTROL, CONTROL_GAP, CONTROL_HEIGHT, CONTROL_HOVERED,
    CONTROL_HOVERED_LIGHT, CONTROL_LIGHT, CONTROL_PADDING, CONTROL_PRESSED, CONTROL_PRESSED_LIGHT,
    CONTROL_RADIUS, DETAIL, DETAIL_LINE, DETAIL_TOP, END_GAP, GLYPH, GLYPH_OFF, GLYPH_ON,
    GLYPH_PLATE, GLYPH_PLATE_FILL, GLYPH_PLATE_GAP, GROUP, GROUP_LIGHT, HEADING, KNOB, KNOB_ALPHA,
    KNOB_INSET, KNOB_SHADOW, LINE, NAME, NAME_OFF, ON_CONTROL, ON_DETAIL, ON_HOVERED, ON_LINE,
    ON_PRESSED, ON_SWITCH_OFF, PADDING, RESTORE_RADIUS, ROW_HEIGHT, ROW_X, SWITCH_HEIGHT,
    SWITCH_OFF, SWITCH_OFF_LIGHT, SWITCH_WIDTH,
};
use crate::tokens::settings::{SENT_CHEVRON, SENT_GAP, SENT_INDENT, SENT_LABEL, SENT_LINES};
use crate::tokens::shell::{KEYED, KEYED_RING};
use crate::tokens::text::SMALL;

/// Fills the relative element it's put in and logs where that is drawn, its centre in the window,
/// as "browser: settings `what` at x y", each time it moves, for check.sh to click it there.
/// `logged` holds the lines logged, one a control; emptied, each is logged again where it's drawn.
pub fn drawn(what: String, logged: Rc<RefCell<Vec<String>>>) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, _, _| {
            let (x, y) = (f32::from(bounds.center().x), f32::from(bounds.center().y));
            let line = format!("browser: settings {what} at {x:.0} {y:.0}");
            let mut logged = logged.borrow_mut();
            // Once a place: forget where it was drawn before.
            let at = format!("browser: settings {what} at ");
            if !logged.contains(&line) {
                crate::log!("{line}");
                logged.retain(|each| !each.starts_with(&at));
                logged.push(line);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// What an assistive app reads of a button: a button called `name`, as Site Controls' are (#1381).
/// Put on while one asks (`Window::is_a11y_active`).
pub fn named_button<E: StatefulInteractiveElement>(control: E, name: impl Into<SharedString>) -> E {
    control.role(Role::Button).aria_label(name)
}

/// What an assistive app reads of a switch: a switch called by its row's `name`, on or off.
pub fn named_switch<E: StatefulInteractiveElement>(
    control: E,
    name: impl Into<SharedString>,
    on: bool,
) -> E {
    let toggled = if on { Toggled::True } else { Toggled::False };
    control
        .role(Role::Switch)
        .aria_label(name)
        .aria_toggled(toggled)
}

/// What an assistive app reads of a box to tick: a checkbox called `name`, ticked or not, as the
/// dialog's check row is.
pub fn named_check<E: StatefulInteractiveElement>(
    control: E,
    name: impl Into<SharedString>,
    on: bool,
) -> E {
    let toggled = if on { Toggled::True } else { Toggled::False };
    control
        .role(Role::CheckBox)
        .aria_label(name)
        .aria_toggled(toggled)
}

/// A button called `name` that's dimmed and takes no click until `enabled`, as an assistive app
/// says it is.
pub fn named_dimmed<E: StatefulInteractiveElement>(
    control: E,
    name: impl Into<SharedString>,
    enabled: bool,
) -> E {
    let control = named_button(control, name);
    match enabled {
        true => control,
        false => control.a11y_synthetic_children(|builder| builder.parent_node().set_disabled()),
    }
}

/// What an assistive app reads of a pop-up button: a button called by its row's `name`, its value
/// the item it shows, which says it opens a menu, as Site Controls' rows do.
pub fn named_popup<E: StatefulInteractiveElement>(
    control: E,
    name: impl Into<SharedString>,
    item: impl Into<SharedString>,
) -> E {
    control
        .role(Role::Button)
        .aria_label(name)
        .aria_value(item)
        .a11y_synthetic_children(|builder| builder.parent_node().set_has_popup(HasPopup::Menu))
}

/// The keyboard's ring, Focus Sidebar's: `shell.keyed-ring` wide inside the control's plate, in
/// the ink `ink` is in at `shell.keyed`, while it has the keyboard from a key. A click shows none.
pub fn ring(ink: Rgba) -> impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement {
    let color = Rgba { a: KEYED, ..ink };
    move |style| {
        style.shadow(vec![BoxShadow {
            color: color.into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(KEYED_RING),
            inset: true,
        }])
    }
}

/// The same ring round a control whose own plate fills it, as a switch's pill does, which would
/// cover a ring inside: just outside its edge, as Site Controls ring a round control.
pub fn ring_round(ink: Rgba) -> impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement {
    let color = Rgba { a: KEYED, ..ink };
    move |style| {
        style.shadow(vec![BoxShadow {
            color: color.into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(KEYED_RING),
            inset: false,
        }])
    }
}

/// Settings' push button: a tab stop as every button here is, which an assistive app, while one
/// asks (`a11y`), reads as a button called `title`.
pub fn stop_button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let title = title.into();
    button(look, id, title.clone()).when(a11y, |button| named_button(button, title))
}

/// Settings' switch, `on`, `slid` of the way there: ⇥ reaches it, Space or Return flips it as its
/// caller's click does, and an assistive app reads it as a switch called by its row's `name`.
pub fn stop_switch(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    name: impl Into<SharedString>,
    on: bool,
    slid: f32,
    accent: Rgba,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .when(a11y, |control| named_switch(control, name, on))
        .tab_index(0)
        .focus_from_keys_only()
        .rounded_full()
        .focus_visible(ring_round(look.ink))
        .relative()
        .child(switch(look, on, slid, accent))
}

/// Settings' pop-up button showing `item`: ⇥ reaches it, its caller opens its menu on a press or
/// on Space or Return, and an assistive app reads it by its row's `name`, with `item` its value.
pub fn stop_popup(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    name: impl Into<SharedString>,
    item: SharedString,
    width: f32,
    open: bool,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    popup(look, item.clone(), width, open)
        .relative()
        .id(id)
        .when(a11y, |control| named_popup(control, name, item))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
}

/// What a Shortcuts chip holds: keys that can change, keys that can't, or none.
#[derive(Clone, Copy)]
pub enum Chip {
    Keys,
    Fixed,
    None,
}

/// Keys as a chip, in `base`, the window's ink, as the command bar shows a command's: quieter for
/// keys that can't change, and quieter still for None.
pub fn chip(base: Rgba, keys: SharedString, kind: Chip) -> gpui::Div {
    let (fill, text) = match kind {
        Chip::Keys => (CHIP, CHIP_TEXT),
        Chip::Fixed => (CHIP_FIXED, CHIP_FIXED_TEXT),
        Chip::None => (CHIP, CHIP_NONE_TEXT),
    };
    div()
        .flex_none()
        .h(px(CHIP_HEIGHT))
        .min_w(px(CHIP_WIDTH))
        .px(px(CHIP_PADDING))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHIP_RADIUS))
        .bg(base.opacity(fill))
        .text_size(px(SMALL))
        .text_color(base.opacity(text))
        .child(keys)
}

/// A command's chip whose keys can change: ⇥ reaches it, Space or Return records new keys as its
/// caller's click does, and an assistive app reads it as its row does, "New Tab, ⌘T", by the row's
/// `name` and the `keys` it shows.
pub fn stop_chip(
    look: &Look,
    base: Rgba,
    id: impl Into<gpui::ElementId>,
    name: &str,
    keys: SharedString,
    kind: Chip,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let (hovered, pressed) = (look.hovered, look.pressed);
    let said = format!("{name}, {keys}");
    chip(base, keys, kind)
        .id(id)
        .when(a11y, |chip| named_button(chip, said))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .hover(move |chip| chip.bg(hovered))
        .active(move |chip| chip.bg(pressed))
}

/// ↺ beside a changed command's chip, which gives the command its row's `name` its own keys back:
/// ⇥ reaches it, and an assistive app reads it as "Restore Defaults for New Tab", as the page's
/// Restore Defaults does for every command.
pub fn stop_restore(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    name: &str,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let (hovered, pressed) = (look.hovered, look.pressed);
    let said = format!("Restore Defaults for {name}");
    div()
        .id(id)
        .when(a11y, |restore| named_button(restore, said))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .size(px(CHIP_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RESTORE_RADIUS))
        .hover(move |restore| restore.bg(hovered))
        .active(move |restore| restore.bg(pressed))
        .relative()
        .child(
            svg()
                .path("icons/rotate-ccw.svg")
                .size(px(BUTTON_GLYPH))
                .text_color(look.ink),
        )
}

/// A switch's knob slides across over 150 ms (ours).
pub const SLIDE: f32 = 0.15;

/// The colours rows are drawn in: Settings' own, or those of the ground another view puts them on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// A group's fill.
    pub surface: Rgba,
    /// A row's name.
    pub ink: Rgba,
    pub line: Rgba,
    /// A row's grey text, under its name or at its end.
    pub detail: Rgba,
    pub control: Rgba,
    /// A control under the pointer.
    pub hovered: Rgba,
    pub pressed: Rgba,
    pub switch_off: Rgba,
    /// A switch's knob while it's off. On, it's white on the accent.
    pub knob: Rgba,
    /// The plate a feature's glyph sits on, and the glyph while it's on.
    pub glyph_plate: Rgba,
    pub glyph: Rgba,
    /// A heading over a group, on the ground the groups are on.
    pub heading: Rgba,
    /// A group's edge, where its fill alone wouldn't stand off the ground.
    pub edge: Option<Rgba>,
    /// What's on, a switch's track or a box's tick, and the one button that does what the view
    /// is for: the host's own green.
    pub accent: Rgba,
}

impl Look {
    /// Settings' own, on its card: the paper over the overlay's fill in dark, the ink over the
    /// dialog's in light, as the window's other text is, and ilo's green, as the app's window
    /// isn't a Space's (#319).
    pub fn settings(dark: bool) -> Look {
        let (card, base) = match dark {
            true => (rgb(OVERLAY_DARK), rgb(PAPER)),
            false => (rgb(DIALOG_LIGHT), rgb(INK)),
        };
        let pick = |dark_alpha: f32, light_alpha: f32| if dark { dark_alpha } else { light_alpha };
        let surface = card.blend(base.opacity(pick(GROUP, GROUP_LIGHT)));
        let mix = |amount: f32| surface.blend(base.opacity(amount));
        let switch_off = mix(pick(SWITCH_OFF, SWITCH_OFF_LIGHT));
        Look {
            surface,
            ink: mix(NAME),
            line: mix(LINE),
            detail: mix(DETAIL),
            control: mix(pick(CONTROL, CONTROL_LIGHT)),
            hovered: mix(pick(CONTROL_HOVERED, CONTROL_HOVERED_LIGHT)),
            pressed: mix(pick(CONTROL_PRESSED, CONTROL_PRESSED_LIGHT)),
            switch_off,
            knob: match dark {
                true => switch_off.blend(base.opacity(KNOB_ALPHA)),
                false => gpui::white().into(),
            },
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: card.blend(base.opacity(HEADING)),
            edge: None,
            accent: rgb(NO_THEME_ACCENT),
        }
    }

    /// Groups filled with `surface`, in `ink`, the rest `ink` over `surface`, and edged in their
    /// line, as the first launch's ground has no card to set them off, with that ground's `accent`.
    pub fn on(surface: Rgba, ink: Rgba, accent: Rgba) -> Look {
        let mix = |amount: f32| surface.blend(ink.opacity(amount));
        let line = mix(ON_LINE);
        Look {
            surface,
            ink,
            line,
            detail: mix(ON_DETAIL),
            control: mix(ON_CONTROL),
            hovered: mix(ON_HOVERED),
            pressed: mix(ON_PRESSED),
            switch_off: mix(ON_SWITCH_OFF),
            knob: gpui::white().into(),
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: ink,
            edge: Some(line),
            accent,
        }
    }

    /// The name of a feature that's off, quieter than one that's on.
    pub fn off(&self) -> Rgba {
        self.surface.blend(self.ink.opacity(NAME_OFF / NAME))
    }

    /// Whether it's dark ink on a light ground, which its pop-ups' menus follow.
    pub fn light(&self) -> bool {
        let lum = |c: Rgba| c.r + c.g + c.b;
        lum(self.ink) < lum(self.surface)
    }
}

/// A group of `rows`, with a hairline between them from where their text starts.
pub fn group(look: &Look, rows: impl IntoIterator<Item = gpui::Div>) -> gpui::Div {
    group_lined(look, rows.into_iter().map(|row| (row, ROW_X)))
}

/// A group of rows, each with the hairline over it starting `x` in, as a feature's own settings
/// have theirs from where their text starts under its name.
pub fn group_lined(look: &Look, rows: impl IntoIterator<Item = (gpui::Div, f32)>) -> gpui::Div {
    let rows = rows.into_iter().enumerate().map(|(index, (row, x))| {
        let line = div()
            .absolute()
            .top_0()
            .left(px(x))
            .right(px(ROW_X))
            .h(px(1.))
            .bg(look.line);
        div()
            .relative()
            .child(row)
            .when(index > 0, |below| below.child(line))
    });
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::ROW))
        .bg(look.surface)
        .when_some(look.edge, |group, edge| group.border_1().border_color(edge))
        .children(rows)
}

/// A row's frame: at least as tall as a row with a line, its text 14 in and its content centred.
pub fn frame() -> gpui::Div {
    div()
        .min_h(px(ROW_HEIGHT))
        .px(px(ROW_X))
        .py(px(PADDING))
        .flex()
        .items_center()
}

/// A row's text: its name, and the line under it that says more, if any, which wraps.
pub fn text(look: &Look, name: impl Into<SharedString>, line: Option<SharedString>) -> gpui::Div {
    let line = line.map(|line| {
        div()
            .mt(px(DETAIL_TOP))
            .text_size(px(SMALL))
            .line_height(px(DETAIL_LINE))
            .text_color(look.detail)
            .child(line)
    });
    div()
        .flex_1()
        .min_w_0()
        .child(div().truncate().child(name.into()))
        .children(line)
}

/// A row's end, its controls 20 after its text and 8 apart.
pub fn end(controls: impl IntoIterator<Item = gpui::AnyElement>) -> gpui::Div {
    div()
        .flex_none()
        .ml(px(END_GAP))
        .flex()
        .items_center()
        .gap(px(CONTROL_GAP))
        .children(controls)
}

/// A row: its name, and its control at the end.
pub fn row(name: impl Into<SharedString>, control: gpui::AnyElement) -> gpui::Div {
    frame()
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
        .child(end([control]))
}

/// A row with a grey line under its name, and its control, if any, at the end.
pub fn detailed_row(
    look: &Look,
    name: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: Option<gpui::AnyElement>,
) -> gpui::Div {
    frame()
        .child(text(look, name, Some(detail.into())))
        .children(control.map(|c| end([c])))
}

/// A feature's glyph on its quiet plate, 12 before its name, quieter while it's off.
pub fn glyph_plate(look: &Look, glyph: &'static str, on: bool) -> gpui::Div {
    let color = if on {
        look.glyph
    } else {
        look.surface.blend(look.ink.opacity(GLYPH_OFF))
    };
    div()
        .flex_none()
        .size(px(GLYPH_PLATE))
        .mr(px(GLYPH_PLATE_GAP))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.glyph_plate)
        .child(svg().path(glyph).size(px(GLYPH)).text_color(color))
}

/// A push button, a step toward its pressed fill under the pointer, and that fill while pressed.
pub fn button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    lit(look, div().id(id).px(px(CONTROL_PADDING))).child(title.into())
}

/// A square push button with `glyph` on it, such as a list's + and −, lit as a button is.
pub fn icon_button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    glyph: &'static str,
) -> gpui::Stateful<gpui::Div> {
    lit(look, div().id(id).w(px(CONTROL_HEIGHT)).justify_center()).child(
        svg()
            .path(glyph)
            .size(px(BUTTON_GLYPH))
            .text_color(look.ink),
    )
}

/// The push button that does what the view is for, one at most in sight: white on the accent,
/// each channel darker under the pointer and darker again pressed, as every filled button is.
pub fn primary(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let shade = |by: f32| Rgba {
        r: look.accent.r * by,
        g: look.accent.g * by,
        b: look.accent.b * by,
        a: look.accent.a,
    };
    let (hovered, pressed) = (shade(ACCENT_HOVERED), shade(ACCENT_PRESSED));
    div()
        .id(id)
        .px(px(CONTROL_PADDING))
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.accent)
        .text_color(gpui::white())
        .hover(move |button| button.bg(hovered))
        .active(move |button| button.bg(pressed))
        .child(title.into())
}

/// A control's plate, lit under the pointer and pressed.
fn lit(look: &Look, control: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    let (hovered, pressed) = (look.hovered, look.pressed);
    control
        // The keyboard reaches it with ⇥, and Space or Return clicks it. A press clicks it and
        // leaves the keyboard where a press put it before it was a stop: with the view around it.
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(look.ink))
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.control)
        .hover(move |button| button.bg(hovered))
        .active(move |button| button.bg(pressed))
}

/// A pop-up button `width` wide showing `label`, with its chevrons at its end, lit as a button is
/// under the pointer and pressed while its menu is `open`. It opens on a press, as a pop-up
/// button does, which its caller listens for.
pub fn popup(look: &Look, label: impl Into<SharedString>, width: f32, open: bool) -> gpui::Div {
    let chevrons = svg()
        .flex_none()
        .path("icons/chevrons-up-down.svg")
        .size(px(CHEVRONS))
        .text_color(look.ink.opacity(CHEVRONS_ALPHA));
    div()
        .flex_none()
        .w(px(width))
        .h(px(CONTROL_HEIGHT))
        .pl(px(CONTROL_PADDING))
        .pr(px(CHEVRONS_END))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(CHEVRONS_END))
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { look.pressed } else { look.control })
        .when(!open, |popup| {
            let hovered = look.hovered;
            popup.hover(move |popup| popup.bg(hovered))
        })
        .child(div().min_w_0().truncate().child(label.into()))
        .child(chevrons)
}

/// A switch, on or off, with its knob `slid` of the way to that side.
pub fn switch(look: &Look, on: bool, slid: f32, accent: Rgba) -> impl IntoElement {
    let travel = SWITCH_WIDTH - KNOB - 2. * KNOB_INSET;
    let way = if on { slid } else { 1. - slid };
    let shadow = BoxShadow {
        color: rgba(KNOB_SHADOW).into(),
        offset: point(px(0.), px(1.)),
        blur_radius: px(2.),
        spread_radius: px(0.),
        inset: false,
    };
    div()
        .flex_none()
        .relative()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded_full()
        .bg(if on { accent } else { look.switch_off })
        .child(
            div()
                .absolute()
                .top(px(KNOB_INSET))
                .left(px(KNOB_INSET + travel * way))
                .size(px(KNOB))
                .rounded_full()
                .shadow(vec![shadow])
                .bg(if on { gpui::white().into() } else { look.knob }),
        )
}

/// A checkbox, ticked in `accent` with a `tick`, else a line round the switch's off fill.
pub fn checkbox(look: &Look, on: bool, accent: Rgba, tick: Rgba) -> gpui::Div {
    let tick = svg()
        .path("icons/check.svg")
        .size(px(CHECK_GLYPH))
        .text_color(tick);
    div()
        .flex_none()
        .size(px(CHECK_BOX))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHECK_RADIUS))
        .when(on, |ticked| ticked.bg(accent).child(tick))
        .when(!on, |empty| {
            empty.border_1().border_color(look.line).bg(look.switch_off)
        })
}

/// A row with a checkbox before its name, as a list to tick from has.
pub fn checked_row(
    look: &Look,
    on: bool,
    name: impl Into<SharedString>,
    accent: Rgba,
    tick: Rgba,
) -> gpui::Div {
    frame()
        .gap(px(CHECK_GAP))
        .child(checkbox(look, on, accent, tick))
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
}

/// How many of a request's `count` lines of what it sent an opened row shows, and whether "Show
/// all" takes the last of its places: every one if they fit, or once the user showed them all.
pub fn sent_shown(count: usize, all: bool) -> (usize, bool) {
    let most = SENT_LINES as usize;
    match all || count <= most {
        true => (count, false),
        false => (most - 1, true),
    }
}

/// The chevron's place at the end of a row in a list whose rows may open: down, or up while
/// `opened`, or empty for a row that doesn't open, so every row's time ends on one edge.
pub fn chevron_slot(opened: Option<bool>, color: gpui::Hsla) -> gpui::Div {
    let chevron = opened.map(|opened| chevron(opened, color));
    div()
        .w(px(SENT_CHEVRON))
        .flex_none()
        .flex()
        .items_center()
        .children(chevron)
}

fn chevron(up: bool, color: gpui::Hsla) -> gpui::Svg {
    svg()
        .path(match up {
            true => "icons/chevron-up.svg",
            false => "icons/chevron-down.svg",
        })
        .size(px(SENT_CHEVRON))
        .flex_none()
        .text_color(color)
}

/// A line in the detail's grey under an opened row's lines, saying what it leaves out and why.
pub fn sent_aside(text: &'static str, detail: gpui::Hsla) -> gpui::Div {
    div()
        .text_size(px(SMALL))
        .line_height(px(DETAIL_LINE))
        .text_color(detail)
        .child(text)
}

/// The label of an opened log row's address: "Page" for what Point sent, "To" for a request.
pub fn address_label(action: &str) -> &'static str {
    match action {
        "pointed" => "Page",
        _ => "To",
    }
}

/// What an agent's request shows opened under its title (#2326): `to`, "To" for a request and
/// "Page" for what Point sent (#2352), and its whole address, then "Sent", or "Not sent" if it was
/// refused, a line a field, cut at the edge until `all`, when a long value wraps whole. Past eight
/// lines the eighth is "Show all", which calls `show_all`. Its caller places it under the row's
/// title, at the title's start.
#[allow(clippy::too_many_arguments)]
pub fn sent_part(
    id: impl Into<gpui::ElementId>,
    (to, url): (&'static str, &str),
    sent: &[Sent],
    refused: bool,
    all: bool,
    (ink, detail): (gpui::Hsla, gpui::Hsla),
    a11y: bool,
    show_all: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    let small = |text: SharedString| {
        div()
            .text_size(px(SMALL))
            .line_height(px(DETAIL_LINE))
            .child(text)
    };
    let labelled = |label: &'static str, lines: Vec<gpui::AnyElement>| {
        div()
            .flex()
            .child(
                small(label.into())
                    .w(px(SENT_LABEL))
                    .flex_none()
                    .text_color(detail),
            )
            .child(div().flex_1().min_w_0().flex().flex_col().children(lines))
    };
    let cut = |line: gpui::Div| match all {
        true => line,
        false => line.truncate(),
    };
    let address = cut(small(url.to_string().into()).min_w_0().text_color(ink));
    let (shown, more) = sent_shown(sent.len(), all);
    let mut lines: Vec<gpui::AnyElement> = (sent.iter().take(shown))
        .map(|line| {
            let row = div()
                .flex()
                .min_w_0()
                .pl(px(SENT_INDENT * f32::from(line.depth)));
            let row = match (line.name.is_empty(), line.value.is_empty()) {
                // An object's name, over its fields.
                (false, true) => row.child(small(line.name.clone().into()).text_color(detail)),
                (true, _) => {
                    row.child(cut(small(line.value.clone().into()).min_w_0()).text_color(ink))
                }
                (false, false) => {
                    let value = match line.hidden() {
                        true => detail,
                        false => ink,
                    };
                    row.child(
                        small(format!("{}: ", line.name).into())
                            .flex_none()
                            .text_color(detail),
                    )
                    .child(cut(small(line.value.clone().into()).min_w_0()).text_color(value))
                }
            };
            row.into_any_element()
        })
        .collect();
    if more {
        let count = sent.len();
        let name = format!("Show all {count} lines");
        lines.push(
            div()
                .id(id)
                .tab_index(0)
                .focus_from_keys_only()
                .focus_visible(ring(ink.into()))
                .when(a11y, |more| named_button(more, name.clone()))
                .flex()
                .items_center()
                .gap(px(SENT_GAP))
                .text_color(detail)
                .child(small(name.into()))
                .child(chevron(false, detail))
                .on_click(move |_, window, cx| show_all(window, cx))
                .into_any_element(),
        );
    }
    let label = if refused { "Not sent" } else { "Sent" };
    div()
        .flex()
        .flex_col()
        .gap(px(SENT_GAP))
        .child(labelled(to, vec![address.into_any_element()]))
        .when(!lines.is_empty(), |part| part.child(labelled(label, lines)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request's opened row shows what it sent in eight lines at most, the eighth "Show all"
    /// when there are more, and every line once the user shows them all.
    #[test]
    fn what_was_sent_shows_in_eight_lines_until_all() {
        assert_eq!(sent_shown(0, false), (0, false));
        assert_eq!(sent_shown(8, false), (8, false));
        assert_eq!(sent_shown(9, false), (7, true));
        assert_eq!(sent_shown(200, false), (7, true));
        assert_eq!(sent_shown(200, true), (200, false));
    }

    /// A button, a switch and a pop-up, named as Settings names them while VoiceOver asks.
    struct Named;

    impl Render for Named {
        fn render(
            &mut self,
            window: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            let look = Look::settings(true);
            let a11y = window.is_a11y_active();
            let switch = div()
                .id("switch")
                .child(switch(&look, true, 1., rgb(0x2ea44f)));
            let pop_up = popup(&look, "12 hours", 155., false).id("archive");
            div()
                .child(
                    button(&look, "make-default", "Make Default")
                        .when(a11y, |button| named_button(button, "Make Default")),
                )
                .child(switch.when(a11y, |switch| {
                    named_switch(switch, "Ask before quitting", true)
                }))
                .child(pop_up.when(a11y, |pop_up| {
                    named_popup(pop_up, "Archive Today tabs after", "12 hours")
                }))
        }
    }

    /// VoiceOver reads Settings' controls by their rows' names: a button by its title, a switch
    /// on, and a pop-up with the item it shows, opening a menu (#1381). With no assistive app
    /// asking, there's no tree to read.
    #[gpui::test]
    fn voiceover_reads_the_controls_by_their_rows(cx: &mut gpui::TestAppContext) {
        use gpui::accesskit::{HasPopup, Role, Toggled};
        let (_, cx) = cx.add_window_view(|_, _| Named);
        cx.run_until_parked();
        assert!(cx.a11y_tree().is_none(), "a tree with nothing asking");
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let find = |role: Role, label: &str| {
            let found = tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == role && node.label() == Some(label));
            found
                .unwrap_or_else(|| panic!("no {role:?} called {label}"))
                .1
                .clone()
        };
        find(Role::Button, "Make Default");
        let switch = find(Role::Switch, "Ask before quitting");
        assert_eq!(switch.toggled(), Some(Toggled::True));
        let pop_up = find(Role::Button, "Archive Today tabs after");
        assert_eq!(pop_up.value(), Some("12 hours"));
        assert_eq!(pop_up.has_popup(), Some(HasPopup::Menu));
    }

    fn contrast(look: &Look, color: Rgba) -> f32 {
        let lum = |c: Rgba| c.r + c.g + c.b;
        (lum(color) - lum(look.surface)).abs()
    }

    /// In dark and light alike, each step of a control stands further off its group than the
    /// last, its line least and a name most, so a press shows more than the pointer does.
    #[test]
    fn each_step_stands_further_off_the_group() {
        for dark in [true, false] {
            let look = Look::settings(dark);
            let steps = [
                look.line,
                look.control,
                look.hovered,
                look.pressed,
                look.detail,
                look.ink,
            ];
            for pair in steps.windows(2) {
                assert!(
                    contrast(&look, pair[0]) < contrast(&look, pair[1]),
                    "{dark} {pair:?}"
                );
            }
            assert!(contrast(&look, look.off()) < contrast(&look, look.ink));
        }
    }

    /// Settings' quieter text still reads at WCAG AA's 4.5:1 for small text, in dark and light,
    /// on what it's drawn over: a row's line on its group, the page's sentence and a heading on
    /// the card, the keys that can't change and None on their chips, and in the column Chromium
    /// settings, the sections and the field's "Search" and ⌘F (#1636, where 0.4 read at 2.4:1).
    #[test]
    fn the_quiet_text_reads_in_dark_and_light() {
        use crate::palette::contrast;
        use crate::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use crate::tokens::settings::{FOOT_TEXT, SEARCH_HINT, SECTION_TEXT};
        use crate::tokens::shell::library::LIBRARY_PLATE;
        for dark in [true, false] {
            let look = Look::settings(dark);
            let (card, base, ground) = match dark {
                true => (rgb(OVERLAY_DARK), rgb(PAPER), rgb(STAND_IN)),
                false => (rgb(DIALOG_LIGHT), rgb(INK), rgb(LIGHT_STAND_IN)),
            };
            let on = |under: Rgba, alpha: f32| (under.blend(base.opacity(alpha)), under);
            let chip = |fill: f32, text: f32| on(look.surface.blend(base.opacity(fill)), text);
            let field = ground.blend(base.opacity(LIBRARY_PLATE));
            let quiet = [
                ("a row's line", (look.detail, look.surface)),
                ("an off feature's name", (look.off(), look.surface)),
                ("the page's sentence", on(card, DETAIL)),
                ("a heading", (look.heading, card)),
                ("keys that can't change", chip(CHIP_FIXED, CHIP_FIXED_TEXT)),
                ("None", chip(CHIP, CHIP_NONE_TEXT)),
                ("a section", on(ground, SECTION_TEXT)),
                ("Chromium settings", on(ground, FOOT_TEXT)),
                ("Search", on(field, DETAIL)),
                ("⌘F", on(field, SEARCH_HINT)),
            ];
            for (what, (text, under)) in quiet {
                let ratio = contrast(text, under);
                assert!(ratio >= 4.5, "{what} reads at {ratio:.2}:1, dark {dark}");
            }
        }
    }

    /// Dark is the paper over the dark card and light the ink over the light one.
    #[test]
    fn dark_is_light_on_dark() {
        let lum = |c: Rgba| c.r + c.g + c.b;
        let (dark, light) = (Look::settings(true), Look::settings(false));
        assert!(lum(dark.ink) > lum(dark.surface) && lum(light.ink) < lum(light.surface));
        assert!(!dark.light() && light.light());
        let on = Look::on(dark.surface, dark.ink, dark.accent);
        assert!(dark.edge.is_none() && on.edge.is_some());
    }
}

/// What an assistive app finds of a Settings view drawn in a test's window, once one asks: each
/// place ⇥ stops, front to back, and each control it can press, with the role and name it reads.
#[cfg(test)]
pub mod spoken {
    use gpui::accesskit::{Action, Role};
    use gpui::{InputEvent, VisualTestContext};

    /// A control as an assistive app reads it: its role, its name, and its value, if any.
    pub type Said = (Role, String, Option<String>);

    fn said(node: &gpui::accesskit::Node) -> Said {
        let name = node.label().unwrap_or_default().to_string();
        (node.role(), name, node.value().map(str::to_string))
    }

    /// Turns on what an assistive app sees, and lists what VoiceOver's press can press: every
    /// node that takes a click, a pop-up's and a switch's too.
    pub fn pressable(cx: &mut VisualTestContext) -> Vec<Said> {
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        (tree.nodes.iter())
            .filter(|(_, node)| node.supports_action(Action::Click))
            .map(|(_, node)| said(node))
            .collect()
    }

    /// Each place ⇥ stops, from the first round to it again, as an assistive app reads it.
    pub fn stops(cx: &mut VisualTestContext) -> Vec<Said> {
        cx.simulate_a11y_activation();
        cx.update(|window, cx| {
            window.activate_window();
            window.focus_next(cx);
        });
        cx.run_until_parked();
        let mut stops = Vec::new();
        let mut first = None;
        for _ in 0..40 {
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let Some((id, node)) = (tree.nodes.iter()).find(|(id, _)| *id == tree.focus) else {
                break;
            };
            if first == Some(*id) {
                break;
            }
            first.get_or_insert(*id);
            stops.push(said(node));
            cx.update(|window, cx| window.focus_next(cx));
            cx.run_until_parked();
        }
        stops
    }

    /// `key` pressed and let go, as a keyboard sends it: Space and Return click on the release.
    pub fn press(key: &'static str, cx: &mut VisualTestContext) {
        cx.simulate_keystrokes(key);
        let keystroke = gpui::Keystroke::parse(key).unwrap();
        cx.update(|window, cx| {
            window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
        });
        cx.run_until_parked();
    }

    /// Those of `said` an assistive app can't tell apart: no name, or no role of a control.
    pub fn unnamed(said: &[Said]) -> Vec<Said> {
        (said.iter())
            .filter(|(role, name, _)| {
                name.is_empty()
                    || matches!(role, Role::Window | Role::GenericContainer | Role::Unknown)
            })
            .cloned()
            .collect()
    }
}
