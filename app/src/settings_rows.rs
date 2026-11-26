//! The Settings window's groups of rows and their controls, which the views it shows from the
//! library, such as the import, draw with too.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Context, Rgba, SharedString,
    accesskit::{HasPopup, Role, Toggled},
    canvas, div,
    prelude::*,
    px, rgb, rgba, svg,
};

use crate::agents::request::Sent;
pub use crate::keyboard_ring::{ring, ring_round};
use crate::palette::{
    CARD_PLATE, FIELD_PLATE, INK, OVERLAY_DARK, PAPER, TEXT_SECONDARY, darker, ink_states,
};
use crate::text_input::Colors;
use crate::tokens::palette::{ACCENT_HOVERED, ACCENT_PRESSED, DIALOG_LIGHT, LIGHT_TEXT_SECONDARY};
use crate::tokens::radius;
use crate::tokens::settings::form::{LIGHT_PAGE_LINE, PAGE_LINE};
use crate::tokens::settings::{
    BUTTON_GLYPH, CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHEVRONS, CHEVRONS_ALPHA,
    CHEVRONS_END, CHEVRONS_GAP, CHIP, CHIP_FIXED, CHIP_FIXED_TEXT, CHIP_HEIGHT, CHIP_NONE_TEXT,
    CHIP_PADDING, CHIP_RADIUS, CHIP_TEXT, CHIP_WIDTH, CONTROL, CONTROL_GAP, CONTROL_HEIGHT,
    CONTROL_HOVERED, CONTROL_HOVERED_LIGHT, CONTROL_LIGHT, CONTROL_PADDING, CONTROL_PRESSED,
    CONTROL_PRESSED_LIGHT, CONTROL_RADIUS, DETAIL_LINE, DETAIL_SIZE, DETAIL_TOP, END_GAP, GLYPH,
    GLYPH_OFF, GLYPH_ON, GLYPH_PLATE, GLYPH_PLATE_FILL, GLYPH_PLATE_GAP, KNOB, KNOB_INSET, LINE,
    LINE_ROW_HEIGHT, NAME, NAME_OFF, ON_CONTROL, ON_DETAIL, ON_HOVERED, ON_LINE, ON_PRESSED,
    ON_SWITCH_OFF, PADDING, POPUP_PADDING, RESTORE_RADIUS, ROW_HEIGHT, ROW_X, SIDE_ROOM,
    STACKED_GAP, SWITCH_HEIGHT, SWITCH_OFF, SWITCH_OFF_LIGHT, SWITCH_WIDTH,
};
use crate::tokens::settings::{
    FOLDER_GLYPH, FOLDER_LEFT, FOLDER_RIGHT, MARK, MARK_GAP, MARK_ROW, POPUP_WIDTH, PRIMARY,
    SCOPE_CHEVRON, SCOPE_GAP, SCOPE_HEIGHT, SCOPE_LEFT, SCOPE_LINE, SCOPE_RIGHT, SCOPE_TEXT,
    SEGMENT_HEIGHT, SEGMENT_INSET, SEGMENT_LINE, SEGMENT_RADIUS, SEGMENT_TEXT, SEGMENT_X, WORKING,
};
use crate::tokens::settings::{SENT_CHEVRON, SENT_GAP, SENT_INDENT, SENT_LABEL, SENT_LINES};
use crate::tokens::shell::ICON_INK;
use crate::tokens::text::SMALL;

mod assistant;
pub use assistant::{
    Act, SIGN_IN, SIGN_INS_STAY, another_agent_name_row, another_agent_row, assistant_name,
    assistant_name_alone, assistant_row, first_run_row,
};

/// The places `drawn` logged, one a control, shared by a view's controls.
pub type Drawn = Rc<RefCell<DrawnLog>>;

#[derive(Default)]
pub struct DrawnLog {
    logged: Vec<String>,
    /// What the frame being drawn has drawn so far, for `frame`.
    this_frame: Vec<String>,
}

impl DrawnLog {
    /// Starts a frame: what the last one didn't draw logs its place again when it's drawn, even
    /// where it was, as a press before that frame lands on what was there instead (#3887). A view
    /// calls it as it renders; one that doesn't logs a control again only where it moves.
    pub fn frame(&mut self) {
        self.logged = std::mem::take(&mut self.this_frame);
    }

    /// Forgets every place, so each is logged again where it's drawn. The frame's too, or the
    /// next `frame` would bring back what the last one drew (#3841's show, #3894).
    pub fn clear(&mut self) {
        self.logged.clear();
        self.this_frame.clear();
    }

    /// The lines logged, one a control.
    pub fn lines(&self) -> &[String] {
        &self.logged
    }

    /// Whether it logged `line`, which is `what`'s: "browser: settings `what` at x y", or with
    /// another view's word in "settings"' place.
    fn draw(&mut self, what: &str, line: String) -> bool {
        // Once a place: forget where it was drawn before.
        let at = line.find(&format!(" {what} at "));
        let at = line[..at.map_or(line.len(), |at| at + what.len() + 5)].to_owned();
        self.this_frame.retain(|each| !each.starts_with(&at));
        self.this_frame.push(line.clone());
        let new = !self.logged.contains(&line);
        if new {
            crate::log!("{line}");
            self.logged.retain(|each| !each.starts_with(&at));
            self.logged.push(line);
        }
        new
    }
}

/// Fills the relative element it's put in and logs where that is drawn, its centre in the window,
/// as "browser: settings `what` at x y", each time it moves, for check.sh to click it there, and
/// after a frame without it where the view starts each frame (`DrawnLog::frame`).
pub fn drawn(what: String, logged: Drawn) -> gpui::Canvas<()> {
    drawn_of("settings", what, logged)
}

/// `drawn` for a view that isn't Settings: the line is "browser: `of` `what` at x y".
pub fn drawn_of(of: &'static str, what: String, logged: Drawn) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, _, _| {
            let (x, y) = (f32::from(bounds.center().x), f32::from(bounds.center().y));
            let line = format!("browser: {of} {what} at {x:.0} {y:.0}");
            logged.borrow_mut().draw(&what, line);
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
    /// A segmented choice's track: the field's plate.
    pub field: Rgba,
    /// A control under the pointer.
    pub hovered: Rgba,
    pub pressed: Rgba,
    pub switch_off: Rgba,
    /// A switch's knob while it's off.
    pub knob: Rgba,
    /// What's on: a switch's track, a ticked box, a chosen radio. Settings' is neutral, the
    /// text colour (#4092); a look on another view's ground takes that ground's accent.
    pub on: Rgba,
    /// A switch's knob on `on`, and the tick on a ticked box.
    pub knob_on: Rgba,
    /// The plate a feature's glyph sits on, and the glyph while it's on.
    pub glyph_plate: Rgba,
    pub glyph: Rgba,
    /// A heading over a group, on the ground the groups are on.
    pub heading: Rgba,
    /// A group's edge, where its fill alone wouldn't stand off the ground.
    pub edge: Option<Rgba>,
    /// The one button that does what the view is for, and its label: Settings' is the text colour
    /// with the other look's ink on it, as the owner drew it; a look on another view's ground
    /// keeps that ground's accent with white.
    pub primary: Rgba,
    pub on_primary: Rgba,
    /// That button under the pointer and pressed: Settings', the text's ink, a step toward its
    /// label and a step further; a ground's accent each channel darker, as every filled button.
    pub primary_hovered: Rgba,
    pub primary_pressed: Rgba,
}

impl Look {
    /// Settings' own, on its card, as drawn (spec-settings.md › Shared controls): the paper over
    /// the overlay's fill in dark, the ink over the dialog's in light, as the window's other text
    /// is. Its groups are the palette's card plate of the ink in both looks, with the page's inner
    /// line. Its controls are neutral: a switch that's on is the text colour with the other look's
    /// ink for its knob, and one that's off the shown plate with a knob of the text colour. Its
    /// one main action is the text colour too, as the app's window isn't a Space's (#319).
    pub fn settings(dark: bool) -> Look {
        let (card, base, other) = match dark {
            true => (rgb(OVERLAY_DARK), rgb(PAPER), rgb(INK)),
            false => (rgb(DIALOG_LIGHT), rgb(INK), rgb(PAPER)),
        };
        let pick = |dark_alpha: f32, light_alpha: f32| if dark { dark_alpha } else { light_alpha };
        let surface = card.blend(base.opacity(CARD_PLATE));
        let mix = |amount: f32| surface.blend(base.opacity(amount));
        let ink = mix(NAME);
        let (primary_hovered, primary_pressed) = ink_states(mix(PRIMARY), other);
        Look {
            surface,
            ink,
            line: mix(LINE),
            detail: mix(pick(TEXT_SECONDARY, LIGHT_TEXT_SECONDARY)),
            control: mix(pick(CONTROL, CONTROL_LIGHT)),
            field: mix(FIELD_PLATE),
            hovered: mix(pick(CONTROL_HOVERED, CONTROL_HOVERED_LIGHT)),
            pressed: mix(pick(CONTROL_PRESSED, CONTROL_PRESSED_LIGHT)),
            switch_off: mix(pick(SWITCH_OFF, SWITCH_OFF_LIGHT)),
            knob: ink,
            on: base,
            knob_on: other,
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: ink,
            edge: Some(rgba(if dark { PAGE_LINE } else { LIGHT_PAGE_LINE })),
            primary: mix(PRIMARY),
            on_primary: other,
            primary_hovered,
            primary_pressed,
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
            field: mix(FIELD_PLATE),
            hovered: mix(ON_HOVERED),
            pressed: mix(ON_PRESSED),
            switch_off: mix(ON_SWITCH_OFF),
            knob: gpui::white().into(),
            on: accent,
            knob_on: gpui::white().into(),
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: ink,
            edge: Some(line),
            primary: accent,
            on_primary: gpui::white().into(),
            primary_hovered: darker(accent, ACCENT_HOVERED),
            primary_pressed: darker(accent, ACCENT_PRESSED),
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

    /// What a field among the rows draws in: its placeholder their grey, its caret their ink.
    pub fn field(&self) -> Colors {
        Colors {
            placeholder: self.detail.into(),
            caret: Some(self.ink.into()),
            ..Colors::default()
        }
    }
}

/// A field's colours as it draws, in the look its `owner` has then: the field stays as the rows
/// go from dark to light with their window.
pub fn field_colors<V: 'static>(
    owner: &Context<V>,
    look: fn(&V) -> Look,
) -> impl Fn(&App) -> Colors + 'static + use<V> {
    let owner = owner.weak_entity();
    move |cx| {
        owner
            .upgrade()
            .map_or_else(Colors::default, |owner| look(owner.read(cx)).field())
    }
}

/// A group of `rows`, with a hairline across it between them.
pub fn group(look: &Look, rows: impl IntoIterator<Item = gpui::Div>) -> gpui::Div {
    group_lined(look, rows.into_iter().map(|row| (row, 0.)))
}

/// A group of rows, each with the hairline over it starting `x` in, as a feature's own settings
/// have theirs from where their text starts under its name, and ending where it starts: across
/// the group from 0, else 14 short of its end.
pub fn group_lined(look: &Look, rows: impl IntoIterator<Item = (gpui::Div, f32)>) -> gpui::Div {
    let rows = rows.into_iter().enumerate().map(|(index, (row, x))| {
        let line = div()
            .absolute()
            .top_0()
            .left(px(x))
            .right(px(if x > 0. { ROW_X } else { 0. }))
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

/// A row's line's type, which says more under its name, and any line that describes as a row's
/// does.
pub fn detail_type() -> gpui::Div {
    div()
        .text_size(px(DETAIL_SIZE))
        .line_height(px(DETAIL_LINE))
}

/// A row's text: its name, at medium, and the line under it that says more, if any, which wraps,
/// a row with one at least 52 tall.
pub fn text(look: &Look, name: impl Into<SharedString>, line: Option<SharedString>) -> gpui::Div {
    let lined = line.is_some();
    let line = line.map(|line| {
        detail_type()
            .mt(px(DETAIL_TOP))
            .text_color(look.detail)
            .child(line)
    });
    div()
        .flex_1()
        .min_w_0()
        .when(lined, |text| {
            text.min_h(px(LINE_ROW_HEIGHT - 2. * PADDING))
                .flex()
                .flex_col()
                .justify_center()
        })
        .child(name_of(name))
        .children(line)
}

/// A row's name, at medium, on one line.
fn name_of(name: impl Into<SharedString>) -> gpui::Div {
    div()
        .truncate()
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(name.into())
}

/// A row's text whose line sets what the user or an agent types in `mono`, the face a terminal
/// shows it in, as the `ilo` command's row has (agents::cli::Detail). Its line to paste goes
/// under the row (`command_export`).
pub fn command_text(
    look: &Look,
    name: impl Into<SharedString>,
    detail: &crate::agents::cli::Detail,
    mono: &'static str,
) -> gpui::Div {
    let run = |len: usize, family: &'static str| gpui::TextRun {
        len,
        font: gpui::font(family),
        color: look.detail.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = &detail.line;
    let runs = match &detail.mono {
        Some(command) => vec![
            run(command.start, crate::face::UI),
            run(command.len(), mono),
            run(line.len() - command.end, crate::face::UI),
        ],
        None => vec![run(line.len(), crate::face::UI)],
    };
    let runs: Vec<_> = runs.into_iter().filter(|run| run.len > 0).collect();
    let small = || detail_type().mt(px(DETAIL_TOP)).text_color(look.detail);
    div()
        .flex_1()
        .min_w_0()
        .child(name_of(name))
        .child(small().child(gpui::StyledText::new(line.clone()).with_runs(runs)))
}

/// The line to paste under a `frame().flex_wrap()` row's text and controls, across the row from
/// its text's edge, so the shell profile's line stays one line down to Settings' least width. A
/// Flatpak's longer one wraps at its spaces, and Copy Line copies it whole.
pub fn command_export(
    look: &Look,
    export: impl Into<SharedString>,
    mono: &'static str,
) -> gpui::Div {
    div()
        .w_full()
        .mt(px(DETAIL_TOP))
        .pl(px(GLYPH_PLATE + GLYPH_PLATE_GAP))
        .font_family(mono)
        .text_size(px(SMALL))
        .line_height(px(DETAIL_LINE))
        .text_color(look.detail)
        .child(export.into())
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
        .child(name_of(name).flex_1().min_w_0())
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

/// A row whose control is drawn pictures (settings_pictures), such as Appearance: its name and
/// `line`, and the pictures at its end, their ring's room outside the row's padding so the
/// drawings keep to the row's edge (#4092).
pub fn pictured(
    look: &Look,
    name: impl Into<SharedString>,
    line: SharedString,
    pictures: impl IntoElement,
) -> gpui::Div {
    frame().child(text(look, name, Some(line))).child(
        div()
            .flex_none()
            .ml(px(END_GAP - SIDE_ROOM))
            .my(px(-SIDE_ROOM))
            .mr(px(-SIDE_ROOM))
            .child(pictures),
    )
}

/// A row whose control is too wide for its end, such as Transparency's pictures: its name and
/// `line`, then the control across the row under them, `inset` wider on each side than the text,
/// so the control's own room keeps what it draws in the text's column.
pub fn stacked(
    look: &Look,
    name: impl Into<SharedString>,
    line: SharedString,
    inset: f32,
    control: impl IntoElement,
) -> gpui::Div {
    frame()
        .flex_col()
        .items_stretch()
        .child(text(look, name, Some(line)))
        .child(div().mt(px(STACKED_GAP)).mx(px(-inset)).child(control))
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
    lit(look, div().id(id).px(px(CONTROL_PADDING)))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(title.into())
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

/// The push button that does what the view is for, one at most in sight: its label on the look's
/// primary at semibold, with the look's own steps under the pointer and pressed.
pub fn primary(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let (hovered, pressed) = (look.primary_hovered, look.primary_pressed);
    div()
        .id(id)
        .px(px(CONTROL_PADDING))
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.primary)
        .text_color(look.on_primary)
        .font_weight(gpui::FontWeight::SEMIBOLD)
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
/// under the pointer and pressed while its menu is `open`. Its text stays at its start, where its
/// menu puts the chosen item's. It opens on a press, as a pop-up button does, which its caller
/// listens for.
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
        .pl(px(POPUP_PADDING))
        .pr(px(CHEVRONS_END))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(CHEVRONS_GAP))
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { look.pressed } else { look.control })
        .when(!open, |popup| {
            let hovered = look.hovered;
            popup.hover(move |popup| popup.bg(hovered))
        })
        .child(div().min_w_0().truncate().child(label.into()))
        .child(chevrons)
}

/// A pop-up naming a folder, as Save downloads to's: the folder before its `name`, with no
/// chevrons, lit as a button is. Its caller opens the system's folder picker on a click, and an
/// assistive app, while one asks (`a11y`), reads it by its row's `spoken` name with the folder's.
pub fn folder_popup(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    spoken: impl Into<SharedString>,
    name: impl Into<SharedString>,
    a11y: bool,
) -> gpui::Stateful<gpui::Div> {
    let name: SharedString = name.into();
    let folder = svg()
        .flex_none()
        .path("icons/folder.svg")
        .size(px(FOLDER_GLYPH))
        .text_color(look.ink.opacity(ICON_INK));
    let control = div()
        .id(id)
        .max_w(px(POPUP_WIDTH))
        .pl(px(FOLDER_LEFT))
        .pr(px(FOLDER_RIGHT))
        .gap(px(CHEVRONS_GAP));
    lit(look, control)
        .when(a11y, |control| {
            control
                .role(Role::Button)
                .aria_label(spoken)
                .aria_value(name.clone())
        })
        .child(folder)
        .child(div().min_w_0().truncate().child(name))
}

/// A short choice of words: each of `choices` a segment in a track, the `chosen` one plated. An
/// assistive app, while one asks (`a11y`), reads each as a radio button by its words, chosen or
/// not. `each` gives a segment what it does when clicked, by its index.
pub fn segmented(
    look: &Look,
    id: impl Into<SharedString>,
    choices: &[SharedString],
    chosen: Option<usize>,
    a11y: bool,
    mut each: impl FnMut(usize, gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div>,
) -> gpui::Div {
    let id: SharedString = id.into();
    let (hovered, plate) = (look.control, look.pressed);
    let segments = choices.iter().enumerate().map(|(index, words)| {
        let on = chosen == Some(index);
        let segment = div()
            .id((id.clone(), index))
            .when(a11y, |segment| {
                let toggled = if on { Toggled::True } else { Toggled::False };
                segment
                    .role(Role::RadioButton)
                    .aria_label(words.clone())
                    .aria_toggled(toggled)
            })
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(ring(look.ink))
            .relative()
            .flex_none()
            .h(px(SEGMENT_HEIGHT))
            .px(px(SEGMENT_X))
            .flex()
            .items_center()
            .rounded(px(CONTROL_RADIUS))
            .when(on, |segment| {
                segment
                    .bg(plate)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(look.ink)
            })
            .when(!on, |segment| {
                segment
                    .text_color(look.detail)
                    .hover(move |segment| segment.bg(hovered))
            })
            .child(words.clone());
        each(index, segment)
    });
    let segments: Vec<_> = segments.collect();
    div()
        .flex_none()
        .flex()
        .items_center()
        .p(px(SEGMENT_INSET))
        .gap(px(SEGMENT_INSET))
        .rounded(px(SEGMENT_RADIUS))
        .bg(look.field)
        .text_size(px(SEGMENT_TEXT))
        .line_height(px(SEGMENT_LINE))
        .children(segments)
}

/// Whose settings a group shows, beside its heading: "For" and the profile's `name`, with a
/// chevron, on a button's plate, the shown plate while its menu is `open`. Its caller opens the
/// profiles' menu on a press.
pub fn scope(look: &Look, name: impl Into<SharedString>, open: bool) -> gpui::Div {
    let chevron = svg()
        .flex_none()
        .path("icons/chevron-down.svg")
        .size(px(SCOPE_CHEVRON))
        .text_color(look.ink.opacity(CHEVRONS_ALPHA));
    let hovered = look.hovered;
    div()
        .flex_none()
        .h(px(SCOPE_HEIGHT))
        .pl(px(SCOPE_LEFT))
        .pr(px(SCOPE_RIGHT))
        .flex()
        .items_center()
        .gap(px(SCOPE_GAP))
        .rounded_full()
        .text_size(px(SCOPE_TEXT))
        .line_height(px(SCOPE_LINE))
        .font_weight(gpui::FontWeight::NORMAL)
        .bg(if open { look.pressed } else { look.control })
        .when(!open, |chip| chip.hover(move |chip| chip.bg(hovered)))
        .child(div().text_color(look.detail).child("For"))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(look.ink)
                .child(name.into()),
        )
        .child(chevron)
}

/// A row with the app's mark before its text, as Default browser's, and its controls at its end.
pub fn mark_row(
    look: &Look,
    name: impl Into<SharedString>,
    line: Option<SharedString>,
    controls: Vec<gpui::AnyElement>,
) -> gpui::Div {
    let mark = svg()
        .flex_none()
        .path(crate::product::mark_path(MARK))
        .size(px(MARK))
        .mr(px(MARK_GAP))
        .text_color(look.ink);
    frame()
        .min_h(px(MARK_ROW))
        .child(mark)
        .child(text(look, name, line))
        .when(!controls.is_empty(), |row| row.child(end(controls)))
}

/// A switch, on or off, with its knob `slid` of the way to that side.
pub fn switch(look: &Look, on: bool, slid: f32, accent: Rgba) -> impl IntoElement {
    let travel = SWITCH_WIDTH - KNOB - 2. * KNOB_INSET;
    let way = if on { slid } else { 1. - slid };
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
                .bg(if on { look.knob_on } else { look.knob }),
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

/// The mark that picks one of a list, as Your assistant's picks the assistant tasks run on: a
/// checkbox, round.
pub fn mark(look: &Look, on: bool, accent: Rgba, tick: Rgba) -> gpui::Div {
    checkbox(look, on, accent, tick).rounded_full()
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

/// What an agent's request shows opened under its title (#2326): first what the agent said it
/// does, `said` ("Claude Code said: “…”"), whole and in one paragraph as the agent's words, never
/// the row's head (#3548); then "To" and its whole address, then "Sent", or "Not sent" if it was
/// refused, a line a field, cut at the edge until `all`, when a long value wraps whole. Past
/// eight lines the eighth is "Show all", which calls `show_all`. Its caller places it under the
/// row's title, at the title's start.
#[allow(clippy::too_many_arguments)]
pub fn sent_part(
    id: impl Into<gpui::ElementId>,
    said: Option<String>,
    url: &str,
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
    let to = cut(small(url.to_string().into()).min_w_0().text_color(ink));
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
        // Readers hear the agent's sentence first too, as it shows.
        .children(said.map(|said| {
            let line = small(said.clone().into()).text_color(ink);
            match a11y {
                true => line
                    .id("sent-said")
                    .role(gpui::accesskit::Role::Label)
                    .aria_label(said)
                    .into_any_element(),
                false => line.into_any_element(),
            }
        }))
        .child(labelled("To", vec![to.into_any_element()]))
        .when(!lines.is_empty(), |part| part.child(labelled(label, lines)))
}

/// The name of the row for the `kept` threads, which is their count (table KT1), and whether it
/// has Remove All…: not with none, where there is nothing to remove.
pub fn kept_row(kept: usize) -> (String, bool) {
    let name = match kept {
        0 => "No threads on this computer".to_string(),
        1 => "1 thread on this computer".to_string(),
        kept => format!("{kept} threads on this computer"),
    };
    (name, kept > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "browser: settings profile 2 at 484 210";

    /// A control drawn where it was logs once; gone for a frame, as a profile's name is while
    /// it's in a field, it logs again where it comes back, for check.sh to click after (#3887).
    #[test]
    fn a_control_back_after_a_frame_without_it_logs_its_place_again() {
        let mut log = DrawnLog::default();
        log.frame();
        assert!(log.draw("profile 2", NAME.into()));
        log.frame();
        assert!(
            !log.draw("profile 2", NAME.into()),
            "drawn again where it was"
        );
        log.frame();
        log.frame();
        assert!(
            log.lines().is_empty(),
            "a frame without it forgot its place"
        );
        assert!(log.draw("profile 2", NAME.into()), "back where it was");
    }

    /// Cleared, as Settings' log is at each show, a control logs again where it was, though the
    /// frame before the clear drew it there.
    #[test]
    fn a_cleared_log_logs_a_control_again_where_it_was() {
        let mut log = DrawnLog::default();
        log.frame();
        assert!(log.draw("profile 2", NAME.into()));
        log.frame();
        assert!(!log.draw("profile 2", NAME.into()));
        log.clear();
        log.frame();
        assert!(
            log.draw("profile 2", NAME.into()),
            "the clear came back with the frame"
        );
    }

    /// A view that never starts a frame logs a control again only where it moves.
    #[test]
    fn without_frames_a_control_logs_only_where_it_moves() {
        let mut log = DrawnLog::default();
        assert!(log.draw("profile 2", NAME.into()));
        assert!(!log.draw("profile 2", NAME.into()));
        assert!(log.draw("profile 2", "browser: settings profile 2 at 484 260".into()));
        assert_eq!(log.lines(), ["browser: settings profile 2 at 484 260"]);
    }

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
    /// last, a name most, so a press shows more than the pointer does. The line is no further
    /// off than a control's plate: the drawing gives the hairline and a button one strength.
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
            assert!(contrast(&look, look.line) <= contrast(&look, look.control));
            for pair in steps[1..].windows(2) {
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
    /// the card, the keys that can't change and None on their chips, and the sections and the
    /// field's "Search" and ⌘F (#1636, where 0.4 read at 2.4:1).
    #[test]
    fn the_quiet_text_reads_in_dark_and_light() {
        use crate::palette::contrast;
        use crate::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use crate::tokens::settings::{SEARCH_HINT, SEARCH_PLACEHOLDER, SECTION_TEXT};
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
                ("the page's sentence", (look.detail, card)),
                ("a heading", (look.heading, card)),
                ("keys that can't change", chip(CHIP_FIXED, CHIP_FIXED_TEXT)),
                ("None", chip(CHIP, CHIP_NONE_TEXT)),
                ("a section", on(ground, SECTION_TEXT)),
                ("Search", on(field, SEARCH_PLACEHOLDER)),
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
        let on = Look::on(dark.surface, dark.ink, dark.primary);
        assert!(on.edge.is_some());
    }

    /// In Settings what's on is the text's colour, a switch's knob on it the card's, never green
    /// (#4092): it stands off the group as far as a name does, and its knob reads on it at WCAG's
    /// 3:1 for a control. Off Settings' card, on is still the ground's accent.
    #[test]
    fn whats_on_is_the_text_colour_not_the_green() {
        for dark in [true, false] {
            let look = Look::settings(dark);
            assert!(
                contrast(&look, look.on) >= contrast(&look, look.ink) * 0.95,
                "{dark}: on is quieter than a name"
            );
            let knob = crate::palette::contrast(look.knob_on, look.on);
            assert!(knob >= 3., "{dark}: the knob reads at {knob}");
        }
        let settings = Look::settings(true);
        let signal = gpui::rgb(crate::tokens::shell::onboarding::SIGNAL);
        let elsewhere = Look::on(settings.surface, settings.ink, signal);
        assert_eq!(elsewhere.on, signal);
    }

    /// Settings' main button is the text colour, not the green (the owner's), and its label
    /// reads on it at WCAG's 4.5:1 for text in dark and light. Off Settings' card it keeps the
    /// ground's accent.
    #[test]
    fn the_main_button_is_the_text_colour_and_its_label_reads() {
        for dark in [true, false] {
            let look = Look::settings(dark);
            assert_eq!(look.primary, look.on, "{dark}");
            let label = crate::palette::contrast(look.on_primary, look.primary);
            assert!(label >= 4.5, "{dark}: the label reads at {label}");
        }
        let settings = Look::settings(true);
        let signal = gpui::rgb(crate::tokens::shell::onboarding::SIGNAL);
        let elsewhere = Look::on(settings.surface, settings.ink, signal);
        assert_eq!(elsewhere.primary, signal);
    }

    /// Settings' main button is the text's ink, which has no darker to go to in light: under the
    /// pointer it goes a step toward its label and pressed a step further, each a step that
    /// tells with the label still reading as text, in dark and light. A ground's accent goes each
    /// channel darker, as every filled button does.
    #[test]
    fn the_main_button_tells_under_the_pointer_in_both_looks() {
        use crate::palette::{TELLS, TEXT, contrast};
        for dark in [true, false] {
            let look = Look::settings(dark);
            let (hovered, pressed) = (look.primary_hovered, look.primary_pressed);
            assert_eq!(
                (hovered, pressed),
                ink_states(look.primary, look.on_primary),
                "{dark}"
            );
            let (rest, further) = (contrast(hovered, look.primary), contrast(pressed, hovered));
            assert!(rest >= TELLS, "{dark}: under the pointer tells at {rest}");
            assert!(further > 1., "{dark}: pressed is no further at {further}");
            for (name, fill) in [("hovered", hovered), ("pressed", pressed)] {
                let label = contrast(look.on_primary, fill);
                assert!(
                    label >= TEXT,
                    "{dark}: the label on {name} reads at {label}"
                );
            }
        }
        let settings = Look::settings(true);
        let signal = gpui::rgb(crate::tokens::shell::onboarding::SIGNAL);
        let elsewhere = Look::on(settings.surface, settings.ink, signal);
        assert_eq!(
            (elsewhere.primary_hovered, elsewhere.primary_pressed),
            (
                darker(signal, ACCENT_HOVERED),
                darker(signal, ACCENT_PRESSED)
            )
        );
    }

    /// The row for the threads kept is named by how many there are, in a form of its own for
    /// none, for one and for more, and offers Remove All… only where there is one to remove.
    #[test]
    fn the_kept_row_is_named_by_its_count_and_removes_only_what_there_is() {
        let (none, one, two, many) = (kept_row(0), kept_row(1), kept_row(2), kept_row(12));
        assert!(!none.1, "Remove All… with nothing kept");
        assert!(one.1 && two.1 && many.1);
        // None is said in words, one is one thread, and more are counted.
        assert!(!none.0.contains(|c: char| c.is_ascii_digit()), "{}", none.0);
        assert!(one.0.starts_with("1 thread ") && !one.0.contains("threads"));
        assert!(two.0.starts_with("2 threads ") && many.0.starts_with("12 threads "));
        // Where they are is said the same way in each.
        let (_, place) = one.0.split_once("thread").unwrap();
        for name in [&none.0, &two.0, &many.0] {
            assert!(name.ends_with(place), "{name}");
        }
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

/// A site's note in Settings › Agents' Site notes (#4122): the site's name over what its note
/// holds, and a button that opens the note's own text under them, as an agent gets it, and
/// closes it again. A press calls `toggle`.
pub fn site_note_row(
    look: &Look,
    note: &'static crate::agents::notes::Note,
    opened: bool,
    a11y: bool,
    logged: Drawn,
    toggle: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    use crate::agents::notes::{CLOSE, READ};
    let host = note.host;
    let title = if opened { CLOSE } else { READ };
    let id = gpui::ElementId::Name(format!("agent-note-read-{host}").into());
    let read = stop_button(look, id, title, a11y)
        .relative()
        .child(drawn(format!("note {host}"), logged))
        .on_click(toggle);
    let head = frame()
        .child(text(look, note.name, Some(note.about.into())))
        .child(end([read.into_any_element()]));
    // A line each, as the note's text has them: a part's name in a row's name's ink, its
    // sentences grey under it, so an open note reads as its three parts.
    let words = opened.then(|| {
        let lines = note.text.trim_end().lines().map(|line| {
            let ink = match crate::agents::notes::PARTS.contains(&line) {
                true => look.ink,
                false => look.detail,
            };
            div().min_h(px(DETAIL_LINE)).text_color(ink).child(line)
        });
        div()
            .px(px(ROW_X))
            .pb(px(PADDING))
            .text_size(px(SMALL))
            .line_height(px(DETAIL_LINE))
            .children(lines)
    });
    div().child(head).children(words)
}
