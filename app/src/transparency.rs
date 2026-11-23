//! Transparency: how much of what's behind ilo shows through its window, its menus and the command
//! bar, in three levels (#4129). Its picker is one component, shown in Settings › Appearance and in
//! the onboarding: three pictures of a small ilo, each drawn from its level's own tints over a
//! drawn desktop with a bright window on it, so each shows what its level gains and what it costs,
//! and a line under them that says what the chosen one does. Drawn, not live glass, so it's the same
//! on every OS. A click on a picture picks it, and so do ← and → while the three have the keyboard;
//! an assistive app reads them as options with one chosen, and the line as the group's description.

use std::rc::Rc;

use gpui::{
    App, BoxShadow, FontWeight, Hsla, IntoElement, KeyDownEvent, Rgba, Window,
    accesskit::{Action, Role, Toggled},
    black, div, linear_color_stop, linear_gradient, point,
    prelude::*,
    px, rgb, rgba, transparent_black, white,
};

use crate::palette::{INK, NO_THEME_ACCENT, OVERLAY_DARK, PAPER};
use crate::settings_pictures::keyed;
use crate::settings_rows::{Look, ring, stacked};
use crate::tokens::palette::{
    LIGHT_MATERIAL_OVER_BLACK, LIGHT_MATERIAL_OVER_WHITE, LIGHT_TINT, LIGHT_TINT_LESS,
    LIGHT_TINT_MORE, MATERIAL_OVER_BLACK, MATERIAL_OVER_WHITE, OVERLAY_FILL, OVERLAY_FILL_LESS,
    OVERLAY_FILL_MORE, OVERLAY_LIGHT, TINT, TINT_LESS, TINT_MORE,
};
use crate::tokens::text::SMALL;
use crate::tokens::transparency::{
    BRIGHT, BRIGHT_HEIGHT, BRIGHT_LINE, BRIGHT_LINE_FIRST, BRIGHT_LINE_GAP, BRIGHT_LINE_HEIGHT,
    BRIGHT_LINE_WIDTH, BRIGHT_LINE_X, BRIGHT_LINE_Y, BRIGHT_RADIUS, BRIGHT_WIDTH, BRIGHT_X,
    BRIGHT_Y, DESKTOP_ANGLE, DESKTOP_FROM, DESKTOP_LAST, DESKTOP_TO, DESKTOP_TO_STOP, DOT,
    DOT_ALPHA, DOT_GAP, DOT_INSET, EDGE, EDGE_WIDTH, GAP, HAIRLINE, HEIGHT, LABEL_GAP, LABEL_OFF,
    LINE, LINE_TOP, MENU_HAIRLINE, MENU_PADDING, MENU_PLATE, MENU_RADIUS, MENU_RIM, MENU_ROW,
    MENU_ROW_PADDING, MENU_ROW_RADIUS, MENU_SHADOW, MENU_SHADOW_BLUR, MENU_SHADOW_Y, MENU_WIDTH,
    MENU_X, MENU_Y, PAGE, PAGE_HEADING, PAGE_HEADING_HEIGHT, PAGE_HEADING_WIDTH, PAGE_LINE,
    PAGE_LINE_HEIGHT, PAGE_LINE_WIDTH, PAGE_LINE_X, PAGE_LINE_Y, PAGE_LINES, PAGE_RADIUS,
    PAGE_RIGHT, PAGE_TOP, PICTURE_RADIUS, QUIET, RING, RING_GAP, ROOM, ROW_HEIGHT, ROW_PADDING,
    ROW_PLATE, ROW_RADIUS, SIDEBAR_TOP, SIDEBAR_WIDTH, SIDEBAR_X, WINDOW_HAIRLINE, WINDOW_RADIUS,
    WINDOW_RIGHT, WINDOW_SHADOW, WINDOW_SHADOW_BLUR, WINDOW_SHADOW_Y, WINDOW_X, WINDOW_Y, WORDS,
};

/// How see-through ilo is. One value for the app, not per Space; Standard until the user picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transparency {
    Less,
    #[default]
    Standard,
    More,
}

impl Transparency {
    /// The levels in the order the picker draws them.
    pub const ALL: [Transparency; 3] = [
        Transparency::Less,
        Transparency::Standard,
        Transparency::More,
    ];

    /// Its name, under its picture and as an assistive app says it.
    pub fn title(self) -> &'static str {
        match self {
            Transparency::Less => "Less",
            Transparency::Standard => "Standard",
            Transparency::More => "More",
        }
    }

    /// What it does, in the line under the picker while it's chosen.
    pub fn line(self) -> &'static str {
        match self {
            Transparency::Less => {
                "A hint of what’s behind ilo. Every word keeps full contrast, whatever is behind."
            }
            Transparency::Standard => {
                "More of your desktop comes through. Grey text is a little fainter over a bright \
                 window."
            }
            Transparency::More => {
                "The most comes through. Grey text can be hard to read over a bright window."
            }
        }
    }

    /// How it's kept, and read back with `named`.
    pub fn key(self) -> &'static str {
        match self {
            Transparency::Less => "less",
            Transparency::Standard => "standard",
            Transparency::More => "more",
        }
    }

    pub fn named(key: &str) -> Option<Transparency> {
        Transparency::ALL.into_iter().find(|each| each.key() == key)
    }

    /// The window's black over its material in dark, or its white in light, at this level.
    pub fn tint(self, dark: bool) -> f32 {
        match (self, dark) {
            (Transparency::Less, true) => TINT_LESS,
            (Transparency::Standard, true) => TINT,
            (Transparency::More, true) => TINT_MORE,
            (Transparency::Less, false) => LIGHT_TINT_LESS,
            (Transparency::Standard, false) => LIGHT_TINT,
            (Transparency::More, false) => LIGHT_TINT_MORE,
        }
    }

    /// What floats over the window: its fill over the material, at this level.
    pub fn overlay_fill(self) -> f32 {
        match self {
            Transparency::Less => OVERLAY_FILL_LESS,
            Transparency::Standard => OVERLAY_FILL,
            Transparency::More => OVERLAY_FILL_MORE,
        }
    }

    fn index(self) -> usize {
        Transparency::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or_default()
    }
}

/// The setting's name and what it does, in Settings' row and as its search reads it.
pub const NAME: &str = "Transparency";
pub const ABOUT: &str =
    "How much of what’s behind ilo shows through its window, its menus and the command bar.";
/// Under Settings' group on the Mac, where the system's setting overrides every level.
pub const REDUCE_TRANSPARENCY_WINS: &str =
    "Your Mac’s Reduce Transparency still wins: with it on, ilo is solid at every level.";

/// The level the app is at and the folder it's kept in, read the first time it's asked.
struct Kept {
    level: Transparency,
    dir: String,
}

impl gpui::Global for Kept {}

/// The file the level is kept in, in the app's own folder: none while it's Standard.
const FILE: &str = "transparency";

/// The level kept in `dir`, Standard if none is, or if what's there names no level.
pub fn stored_in(dir: &str) -> Transparency {
    std::fs::read_to_string(format!("{dir}/{FILE}"))
        .ok()
        .and_then(|key| Transparency::named(key.trim()))
        .unwrap_or_default()
}

/// Keeps the level in `dir`, as the app keeps it in its profile's folder.
fn keep_in(dir: String, cx: &mut App) {
    let level = stored_in(&dir);
    cx.set_global(Kept { level, dir });
}

/// The level the app is at.
pub fn level(cx: &mut App) -> Transparency {
    if !cx.has_global::<Kept>() {
        keep_in(crate::profile_dir(), cx);
    }
    cx.global::<Kept>().level
}

/// The one way a level is set: it's kept, and every window draws again at it. What applies the
/// level to the window, its menus and the command bar reads `level` as it draws, so it needs no call
/// of its own; the window lane (#4128) hooks here if it needs one.
pub fn set(level: Transparency, cx: &mut App) {
    if !cx.has_global::<Kept>() {
        keep_in(crate::profile_dir(), cx);
    }
    let kept = cx.global_mut::<Kept>();
    kept.level = level;
    let path = format!("{}/{FILE}", kept.dir);
    _ = match level {
        Transparency::Standard => std::fs::remove_file(path),
        _ => std::fs::write(path, level.key()),
    };
    cx.refresh_windows();
}

/// Settings' row, where the host has `glass` behind the window: the setting's name, what it does,
/// and the picker under them across the row at the level the app is at, the pictures' edges in
/// line with the text.
pub fn row(look: &Look, glass: bool, cx: &mut App) -> Option<gpui::Div> {
    let chosen = level(cx);
    let picker = picker(chosen, !look.light(), |level, _, cx| set(level, cx));
    glass.then(|| stacked(look, NAME, ABOUT.into(), ROOM + RING + RING_GAP, picker))
}

/// Whether ilo's window has a material behind it for the levels to show, and so the setting to
/// pick one: the host's answer (host_has_window_material), the Mac's yes.
pub fn has_glass() -> bool {
    crate::palette::window_material()
}

/// What the picker calls with the level picked, by a click or a key, when it isn't the one chosen.
pub type Pick = Rc<dyn Fn(Transparency, &mut Window, &mut App)>;

/// The three pictures, `chosen` ringed, and the line under them, drawn in the dark look or the light
/// one as the window they're in is.
#[derive(IntoElement)]
pub struct Picker {
    chosen: Transparency,
    dark: bool,
    pick: Pick,
}

pub fn picker(
    chosen: Transparency,
    dark: bool,
    pick: impl Fn(Transparency, &mut Window, &mut App) + 'static,
) -> Picker {
    Picker {
        chosen,
        dark,
        pick: Rc::new(pick),
    }
}

impl RenderOnce for Picker {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let Picker { chosen, dark, pick } = self;
        let a11y = window.is_a11y_active();
        let ink = rgb(if dark { PAPER } else { INK });
        let options =
            Transparency::ALL.map(|level| option(level, level == chosen, dark, ink, a11y, &pick));
        let keyed_pick = pick.clone();
        let line = div()
            .mt(px(LINE_TOP))
            .mx(px(ROOM + RING + RING_GAP))
            .text_size(px(SMALL))
            .text_color(ink.opacity(LINE))
            .child(chosen.line());
        div()
            .child(
                div()
                    .id("transparency")
                    .when(a11y, |group| {
                        group
                            .role(Role::RadioGroup)
                            .aria_label("Transparency")
                            .aria_description(chosen.line())
                    })
                    .tab_index(0)
                    .focus_from_keys_only()
                    .rounded(px(PICTURE_RADIUS + RING_GAP + RING + ROOM))
                    .focus_visible(ring(ink))
                    .p(px(ROOM))
                    .flex()
                    .gap(px(GAP - 2. * (RING_GAP + RING)))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let to = keyed(event, chosen.index(), Transparency::ALL.len());
                        if let Some(index) = to {
                            cx.stop_propagation();
                            keyed_pick(Transparency::ALL[index], window, cx);
                        }
                    })
                    .children(options),
            )
            .child(line)
    }
}

/// One level's picture, ringed in the accent while it's chosen, and its name under it.
fn option(
    level: Transparency,
    chosen: bool,
    dark: bool,
    ink: Rgba,
    a11y: bool,
    pick: &Pick,
) -> gpui::Stateful<gpui::Div> {
    let pick = pick.clone();
    let ringed = div()
        .rounded(px(PICTURE_RADIUS + RING_GAP + RING))
        .border(px(RING))
        .border_color(match chosen {
            true => rgb(NO_THEME_ACCENT),
            false => transparent_black().into(),
        })
        .p(px(RING_GAP))
        .child(picture(level, dark, ink));
    let label = div()
        .mt(px(LABEL_GAP - RING_GAP - RING))
        .text_size(px(SMALL))
        .when(chosen, |label| label.font_weight(FontWeight::MEDIUM))
        .text_color(if chosen { ink } else { ink.opacity(LABEL_OFF) })
        .child(level.title());
    let toggled = if chosen {
        Toggled::True
    } else {
        Toggled::False
    };
    div()
        .id(("transparency", level.index()))
        .when(a11y, |option| {
            option
                .role(Role::RadioButton)
                .aria_label(level.title())
                .aria_toggled(toggled)
                .a11y_synthetic_children(|builder| builder.parent_node().add_action(Action::Click))
        })
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .child(ringed.w_full())
        .child(label)
        .on_click(move |_, window, cx| {
            if !chosen {
                pick(level, window, cx)
            }
        })
}

/// The material behind ilo's window, drawn as the fill that gives the colour it was measured at
/// over a white and over a black backdrop: what it lets through is the difference between them.
fn material(dark: bool) -> Rgba {
    let (white, black) = match dark {
        true => (rgb(MATERIAL_OVER_WHITE), rgb(MATERIAL_OVER_BLACK)),
        false => (
            rgb(LIGHT_MATERIAL_OVER_WHITE),
            rgb(LIGHT_MATERIAL_OVER_BLACK),
        ),
    };
    let alpha = 1. - (white.r - black.r);
    Rgba {
        r: black.r / alpha,
        g: black.g / alpha,
        b: black.b / alpha,
        a: alpha,
    }
}

/// A layer filling its parent, `color` over what's under it.
fn layer(color: impl Into<Hsla>) -> gpui::Div {
    div().absolute().inset_0().bg(color.into())
}

/// The ink `ink` at `alpha`, as a hairline or a plate.
fn at(ink: Rgba, alpha: f32) -> Rgba {
    ink.opacity(alpha)
}

/// A shadow under a floating thing: `color`, `y` down, blurred `blur`.
fn shadow(color: u32, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color: rgba(color).into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// A line `width` wide round an edge, in `color`, inside it or out.
fn edge(color: Rgba, width: f32, inset: bool) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset,
    }
}

/// A hairline round an edge, in `color`, inside it or out.
fn hairline(color: Rgba, inset: bool) -> BoxShadow {
    edge(color, HAIRLINE, inset)
}

/// A small ilo at `level` over the desktop: its window's glass at the level's tint, the sidebar's
/// rows on it, two at full strength and two quiet, the page, and a menu floating at the level's
/// fill. `ink` is the window's around the picture, for its hairline.
fn picture(level: Transparency, dark: bool, ink: Rgba) -> gpui::Div {
    let words = rgb(if dark { PAPER } else { INK });
    let tint = if dark { black() } else { white() };
    let desktop = div()
        .absolute()
        .inset_0()
        .bg(linear_gradient(
            DESKTOP_ANGLE,
            linear_color_stop(rgb(DESKTOP_FROM), 0.),
            linear_color_stop(rgb(DESKTOP_TO), DESKTOP_TO_STOP),
        ))
        .child(layer(transparent_black()).bg(linear_gradient(
            DESKTOP_ANGLE,
            linear_color_stop(rgb(DESKTOP_LAST).opacity(0.), DESKTOP_TO_STOP),
            linear_color_stop(rgb(DESKTOP_LAST), 1.),
        )));
    let bright_lines = [BRIGHT_LINE_FIRST, BRIGHT_LINE, BRIGHT_LINE]
        .into_iter()
        .enumerate()
        .map(|(index, color)| {
            div()
                .absolute()
                .left(px(BRIGHT_LINE_X))
                .top(px(BRIGHT_LINE_Y + index as f32 * BRIGHT_LINE_GAP))
                .w(px(BRIGHT_LINE_WIDTH))
                .h(px(BRIGHT_LINE_HEIGHT))
                .rounded_full()
                .bg(rgb(color))
        });
    let bright = div()
        .absolute()
        .left(px(BRIGHT_X))
        .top(px(BRIGHT_Y))
        .w(px(BRIGHT_WIDTH))
        .h(px(BRIGHT_HEIGHT))
        .rounded(px(BRIGHT_RADIUS))
        .bg(rgb(BRIGHT))
        .children(bright_lines);
    let glass = layer(material(dark)).child(layer(tint.opacity(level.tint(dark))));
    let dots = (0..3).map(|index| {
        div()
            .absolute()
            .left(px(DOT_INSET + index as f32 * (DOT + DOT_GAP)))
            .top(px(DOT_INSET))
            .size(px(DOT))
            .rounded_full()
            .bg(at(words, DOT_ALPHA))
    });
    let rows = [
        ("Reading list", true, false),
        ("Ferry times", false, false),
        ("Harbour café", false, true),
        ("Cliff path map", false, true),
    ]
    .map(|(name, shown, quiet)| {
        div()
            .h(px(ROW_HEIGHT))
            .px(px(ROW_PADDING))
            .rounded(px(ROW_RADIUS))
            .flex()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap()
            .when(shown, |row| row.bg(at(words, ROW_PLATE)))
            .text_color(if quiet { at(words, QUIET) } else { words })
            .child(name)
    });
    let sidebar = div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(px(SIDEBAR_WIDTH))
        .pt(px(SIDEBAR_TOP))
        .px(px(SIDEBAR_X))
        .flex()
        .flex_col()
        .children(dots)
        .children(rows);
    let page_lines = PAGE_LINES.map(|y| {
        div()
            .absolute()
            .left(px(PAGE_LINE_X + (PAGE_HEADING_WIDTH - PAGE_LINE_WIDTH) / 2.))
            .top(px(PAGE_LINE_Y + y))
            .w(px(PAGE_LINE_WIDTH))
            .h(px(PAGE_LINE_HEIGHT))
            .rounded_full()
            .bg(rgb(PAGE_LINE))
    });
    let page = div()
        .absolute()
        .left(px(SIDEBAR_WIDTH))
        .right(px(PAGE_RIGHT))
        .top(px(PAGE_TOP))
        .bottom_0()
        .rounded_t(px(PAGE_RADIUS))
        .bg(rgb(PAGE))
        .child(
            div()
                .absolute()
                .left(px(PAGE_LINE_X))
                .top(px(PAGE_LINE_Y))
                .w(px(PAGE_HEADING_WIDTH))
                .h(px(PAGE_HEADING_HEIGHT))
                .rounded_full()
                .bg(rgb(PAGE_HEADING)),
        )
        .children(page_lines);
    let fill = rgb(if dark { OVERLAY_DARK } else { OVERLAY_LIGHT });
    let items = [
        ("Copy Link", Some("⇧⌘C"), false),
        ("Pin Tab", None, true),
        ("Close", Some("⌘W"), false),
    ]
    .map(|(name, keys, under): (&str, Option<&str>, bool)| {
        div()
            .relative()
            .h(px(MENU_ROW))
            .px(px(MENU_ROW_PADDING))
            .rounded(px(MENU_ROW_RADIUS))
            .flex()
            .items_center()
            .justify_between()
            .whitespace_nowrap()
            .when(under, |item| item.bg(at(words, MENU_PLATE)))
            .child(name)
            .children(keys.map(|keys| div().text_color(at(words, QUIET)).child(keys)))
    });
    let menu = div()
        .absolute()
        .left(px(MENU_X))
        .top(px(MENU_Y))
        .w(px(MENU_WIDTH))
        .rounded(px(MENU_RADIUS))
        .shadow(vec![
            hairline(rgba(MENU_HAIRLINE), false),
            shadow(MENU_SHADOW, MENU_SHADOW_Y, MENU_SHADOW_BLUR),
        ])
        .child(layer(material(dark)).rounded(px(MENU_RADIUS)))
        .child(layer(fill.opacity(level.overlay_fill())).rounded(px(MENU_RADIUS)))
        .child(
            div()
                .relative()
                .p(px(MENU_PADDING))
                .rounded(px(MENU_RADIUS))
                .shadow(vec![hairline(rgba(MENU_RIM), true)])
                .text_color(words)
                .line_height(px(MENU_ROW))
                .children(items),
        );
    let window = div()
        .absolute()
        .left(px(WINDOW_X))
        .right(px(WINDOW_RIGHT))
        .top(px(WINDOW_Y))
        .bottom(px(-WINDOW_RADIUS))
        .rounded_t(px(WINDOW_RADIUS))
        .shadow(vec![
            hairline(rgba(WINDOW_HAIRLINE), false),
            shadow(WINDOW_SHADOW, WINDOW_SHADOW_Y, WINDOW_SHADOW_BLUR),
        ])
        .child(glass.rounded_t(px(WINDOW_RADIUS)))
        .child(sidebar)
        .child(page)
        .child(menu);
    div()
        .relative()
        .h(px(HEIGHT))
        .rounded(px(PICTURE_RADIUS))
        .overflow_hidden()
        .text_size(px(WORDS))
        .line_height(px(ROW_HEIGHT))
        .child(desktop.rounded(px(PICTURE_RADIUS)))
        .child(bright)
        .child(window)
        .child(
            layer(transparent_black())
                .rounded(px(PICTURE_RADIUS))
                .shadow(vec![edge(at(ink, EDGE), EDGE_WIDTH, true)]),
        )
}

#[cfg(test)]
mod tests;
