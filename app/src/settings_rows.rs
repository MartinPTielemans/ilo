//! The Settings window's groups of rows and their controls, which the views it shows from the
//! library, such as the import, draw with too.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{BoxShadow, Rgba, SharedString, canvas, div, point, prelude::*, px, rgb, rgba, svg};

use crate::palette::{INK, NO_THEME_ACCENT, OVERLAY_DARK, PAPER};
use crate::tokens::palette::{ACCENT_HOVERED, ACCENT_PRESSED, DIALOG_LIGHT};
use crate::tokens::radius;
use crate::tokens::settings::{
    BUTTON_GLYPH, CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHEVRONS, CHEVRONS_ALPHA,
    CHEVRONS_END, CONTROL, CONTROL_GAP, CONTROL_HEIGHT, CONTROL_HOVERED, CONTROL_HOVERED_LIGHT,
    CONTROL_LIGHT, CONTROL_PADDING, CONTROL_PRESSED, CONTROL_PRESSED_LIGHT, CONTROL_RADIUS, DETAIL,
    DETAIL_LINE, DETAIL_TOP, END_GAP, GLYPH, GLYPH_OFF, GLYPH_ON, GLYPH_PLATE, GLYPH_PLATE_FILL,
    GLYPH_PLATE_GAP, GROUP, GROUP_LIGHT, HEADING, KNOB, KNOB_ALPHA, KNOB_INSET, KNOB_SHADOW, LINE,
    NAME, NAME_OFF, ON_CONTROL, ON_DETAIL, ON_HOVERED, ON_LINE, ON_PRESSED, ON_SWITCH_OFF, PADDING,
    ROW_HEIGHT, ROW_X, SWITCH_HEIGHT, SWITCH_OFF, SWITCH_OFF_LIGHT, SWITCH_WIDTH,
};
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
        // The keyboard reaches it with ⇥, and Space or Return clicks it.
        .tab_index(0)
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

#[cfg(test)]
mod tests {
    use super::*;

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
