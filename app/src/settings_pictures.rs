//! Settings' rows chosen from drawings: a row of small pictures under the row's line, the chosen
//! one ringed and its name beneath in the row's ink. A click on one picks it, and so do ← and →
//! while the row has the keyboard; an assistive app reads them as options with one chosen.
//!
//! Sidebar position (#2407) draws the window with the sidebar on each side, Appearance (#2429)
//! draws it light, dark, and split between the two for Automatic, and Share favorites (#2430)
//! draws three Spaces' favorites, alike where they're shared.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, FontWeight, KeyDownEvent, Rgba, Window,
    accesskit::{Action, Role, Toggled},
    div, point,
    prelude::*,
    px, rgb, svg,
};

use crate::palette::{INK, PAPER};
use crate::settings_rows::{Look, drawn, ring};
use crate::shell::FavoritesScope;
use crate::theme::Appearance;
use crate::tokens::command_bar::site_colors::{
    AMAZON, APPLE_MUSIC, ASANA, DROPBOX, FIGMA, GMAIL, GOOGLE_CALENDAR, GOOGLE_DRIVE, GOOGLE_MAPS,
    LINKEDIN, REDDIT, SPOTIFY,
};
use crate::tokens::settings::{
    SHARE_MARK, SHARE_MARK_SQUARE, SHARE_PLATE, SHARE_PLATE_GAP, SHARE_PLATE_PADDING,
    SHARE_PLATE_RADIUS, SHARE_PROFILES_GAP, SHARE_RING, SHARE_SPACE_GAP, SHARE_SPACE_HEIGHT,
    SHARE_SPACE_PADDING, SHARE_SPACE_PADDING_X, SHARE_SPACE_RADIUS, SHARE_SPACE_WIDTH,
    SHARE_SPACES, SHARE_TAB_GAP, SHARE_TAB_HEIGHT, SHARE_TILE, SHARE_TILE_GAP, SHARE_TILE_HEIGHT,
    SHARE_TILE_LIGHT, SHARE_TILE_RADIUS, SHARE_TILE_RIM, SHARE_TILE_WIDTH, SHARE_TINT,
    SHARE_TINT_LIGHT,
};
use crate::tokens::settings::{
    SIDE_BAR, SIDE_BAR_FIRST, SIDE_BUTTON, SIDE_BUTTON_GAP, SIDE_BUTTONS, SIDE_GAP, SIDE_GLYPH,
    SIDE_GLYPH_GAP, SIDE_HEADING, SIDE_HEIGHT, SIDE_INSET, SIDE_LABEL_GAP, SIDE_MAC_CLOSE,
    SIDE_MAC_MINIMIZE, SIDE_MAC_ZOOM, SIDE_OTHER, SIDE_PAGE, SIDE_PAGE_LIGHT, SIDE_PAGE_PADDING,
    SIDE_PAGE_RADIUS, SIDE_PAGE_RING, SIDE_RADIUS, SIDE_RING, SIDE_RING_GAP, SIDE_ROOM,
    SIDE_ROWS_GAP, SIDE_SIDEBAR, SIDE_SIDEBAR_PADDING, SIDE_TEXT, SIDE_WIDTH, SIDE_WINDOW,
    SIDE_WINDOW_LIGHT,
};
use crate::tokens::shell::{WINDOW_BUTTON_CLOSE, WINDOW_BUTTON_MINIMIZE, WINDOW_BUTTON_ZOOM};
use crate::tokens::text::SMALL;

/// The window's buttons as this OS has them, which each drawing shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Buttons {
    /// AppKit's three: close, minimize, full screen; mirrored as a unit on the right.
    Mac,
    /// The caption group ilo draws: minimize, maximize, close in that order on either side, close
    /// at the corner on the right.
    Windows,
    /// The three ilo draws on its frameless window, mirrored as a unit on the right.
    Linux,
}

impl Buttons {
    /// This OS's.
    pub const HERE: Buttons = if cfg!(target_os = "macos") {
        Buttons::Mac
    } else if cfg!(target_os = "windows") {
        Buttons::Windows
    } else {
        Buttons::Linux
    };
}

/// One of a row's pictures: its name, under it and as an assistive app says it, and its drawing.
pub struct Choice {
    pub title: &'static str,
    pub drawing: gpui::Div,
}

/// The choice a key moves to among `count`, if it's ← or → alone and there's one that way.
pub fn keyed(event: &KeyDownEvent, chosen: usize, count: usize) -> Option<usize> {
    let keys = &event.keystroke;
    if keys.modifiers.modified() {
        return None;
    }
    match keys.key.as_str() {
        "left" => chosen.checked_sub(1),
        "right" => Some(chosen + 1).filter(|&next| next < count),
        _ => None,
    }
}

/// The row's pictures, `chosen` ringed, on a row's plate in `look`, the others with a hairline in
/// the window's `base` ink. `name` is the row's, which an assistive app reads for the group, and
/// `logged` is Settings' (settings_rows::drawn), so check.sh can click each picture as "<name>
/// option <title>", the name in lower case. `pick` is called with the index picked, by a click or
/// a key, only when it isn't the one chosen.
#[allow(clippy::too_many_arguments)]
pub fn pictures(
    look: &Look,
    base: Rgba,
    name: &'static str,
    chosen: usize,
    choices: Vec<Choice>,
    a11y: bool,
    logged: Rc<RefCell<Vec<String>>>,
    pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let pick = Rc::new(pick);
    let keyed_pick = pick.clone();
    let count = choices.len();
    let logged_name = name.to_lowercase();
    let options = choices.into_iter().enumerate().map(|(index, choice)| {
        let is_chosen = index == chosen;
        let pick = pick.clone();
        let label = div()
            .text_size(px(SMALL))
            .text_color(if is_chosen { look.ink } else { look.detail })
            .when(is_chosen, |label| label.font_weight(FontWeight::MEDIUM))
            .child(choice.title);
        // GPUI paints a later shadow over an earlier one, so the plate's gap goes over the accent.
        let ring = match is_chosen {
            true => vec![
                spread(look.accent, SIDE_RING_GAP + SIDE_RING),
                spread(look.surface, SIDE_RING_GAP),
            ],
            false => vec![spread(base.opacity(SIDE_OTHER), 1.)],
        };
        div()
            .id((name, index))
            .when(a11y, |option| {
                let toggled = if is_chosen {
                    Toggled::True
                } else {
                    Toggled::False
                };
                option
                    .role(Role::RadioButton)
                    .aria_label(choice.title)
                    .aria_toggled(toggled)
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().add_action(Action::Click)
                    })
            })
            .flex()
            .flex_col()
            .items_center()
            .gap(px(SIDE_LABEL_GAP))
            .child(choice.drawing.shadow(ring).relative().child(drawn(
                format!("{logged_name} option {}", choice.title),
                logged.clone(),
            )))
            .child(label)
            .on_click(move |_, window, cx| {
                if !is_chosen {
                    pick(index, window, cx)
                }
            })
            .into_any_element()
    });
    div()
        .id(name)
        .when(a11y, |row| row.role(Role::RadioGroup).aria_label(name))
        .tab_index(0)
        .focus_from_keys_only()
        .rounded(px(SIDE_RADIUS + SIDE_ROOM))
        .focus_visible(ring(look.ink))
        .p(px(SIDE_ROOM))
        .flex()
        .gap(px(SIDE_GAP))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if let Some(index) = keyed(event, chosen, count) {
                cx.stop_propagation();
                keyed_pick(index, window, cx);
            }
        })
        .children(options)
}

/// A side's name, as the drawing's label and an assistive app say it.
pub fn title(right: bool) -> &'static str {
    if right { "Right" } else { "Left" }
}

/// Sidebar position's pair, `right` chosen, its drawings in the window's `base` ink (the paper in
/// dark, the ink in light). `pick` is called with the side picked.
#[allow(clippy::too_many_arguments)]
pub fn picker(
    look: &Look,
    base: Rgba,
    dark: bool,
    right: bool,
    buttons: Buttons,
    a11y: bool,
    logged: Rc<RefCell<Vec<String>>>,
    pick: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let choices = [false, true]
        .map(|side| Choice {
            title: title(side),
            drawing: window(base, dark, side, buttons),
        })
        .into();
    pictures(
        look,
        base,
        "Sidebar position",
        usize::from(right),
        choices,
        a11y,
        logged,
        move |index, window, cx| pick(index == 1, window, cx),
    )
}

/// Appearance's choices, in the order they're drawn.
pub const APPEARANCES: [Appearance; 3] =
    [Appearance::Automatic, Appearance::Light, Appearance::Dark];

/// An appearance's name, as its picture's label, View › Appearance's items and an assistive app
/// say it.
pub fn appearance_title(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Automatic => "Automatic",
        Appearance::Light => "Light",
        Appearance::Dark => "Dark",
    }
}

/// Appearance's three, `chosen` ringed: the window as Light and as Dark draw it whatever Settings'
/// own appearance, and Automatic's split down the middle, its left light and its right dark.
/// `base` is Settings' ink, for the others' hairline. `pick` is called with the appearance picked.
pub fn appearance_picker(
    look: &Look,
    base: Rgba,
    chosen: Appearance,
    buttons: Buttons,
    a11y: bool,
    logged: Rc<RefCell<Vec<String>>>,
    pick: impl Fn(Appearance, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let light = || window(rgb(INK), false, false, buttons);
    let dark = || window(rgb(PAPER), true, false, buttons);
    let choices = APPEARANCES
        .map(|appearance| Choice {
            title: appearance_title(appearance),
            drawing: match appearance {
                Appearance::Light => light(),
                Appearance::Dark => dark(),
                Appearance::Automatic => {
                    let half = SIDE_WIDTH / 2.;
                    div()
                        .flex_none()
                        .w(px(SIDE_WIDTH))
                        .h(px(SIDE_HEIGHT))
                        .rounded(px(SIDE_RADIUS))
                        .child(light())
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left(px(half))
                                .w(px(half))
                                .h(px(SIDE_HEIGHT))
                                .overflow_hidden()
                                .child(dark().absolute().top_0().left(px(-half))),
                        )
                }
            },
        })
        .into();
    let index = APPEARANCES.iter().position(|&each| each == chosen);
    pictures(
        look,
        base,
        "Appearance",
        index.unwrap_or(0),
        choices,
        a11y,
        logged,
        move |index, window, cx| pick(APPEARANCES[index], window, cx),
    )
}

/// Share favorites' three, `chosen` ringed, drawn in Settings' look (`dark`, its ink `base`).
/// `pick` is called with the scope picked.
#[allow(clippy::too_many_arguments)]
pub fn favorites_picker(
    look: &Look,
    base: Rgba,
    dark: bool,
    chosen: FavoritesScope,
    a11y: bool,
    logged: Rc<RefCell<Vec<String>>>,
    pick: impl Fn(FavoritesScope, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let choices = FavoritesScope::ALL
        .map(|scope| Choice {
            title: scope.caption(),
            drawing: favorites(base, dark, scope),
        })
        .into();
    let index = FavoritesScope::ALL.iter().position(|&each| each == chosen);
    pictures(
        look,
        base,
        "Share favorites",
        index.unwrap_or(0),
        choices,
        a11y,
        logged,
        move |index, window, cx| pick(FavoritesScope::ALL[index], window, cx),
    )
}

/// A favorites list's marks: four sites' colours and the list's own shape.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mark {
    Dot,
    Square,
    Ring,
}

/// The three lists a drawing can show, each its sites' colours (the command bar's) and its shape.
const LISTS: [([u32; 4], Mark); 3] = [
    ([GMAIL, FIGMA, SPOTIFY, DROPBOX], Mark::Dot),
    ([GOOGLE_DRIVE, AMAZON, LINKEDIN, APPLE_MUSIC], Mark::Square),
    ([GOOGLE_MAPS, REDDIT, GOOGLE_CALENDAR, ASANA], Mark::Ring),
];

/// Which list each of the three Spaces shows under `scope`: the first two are one profile's, the
/// third another's.
fn lists(scope: FavoritesScope) -> [usize; 3] {
    match scope {
        FavoritesScope::Every => [0, 0, 0],
        FavoritesScope::Profile => [0, 0, 1],
        FavoritesScope::Space => [0, 1, 2],
    }
}

/// Three Spaces' sidebars in the window, on their profiles' plates, each with the favorites
/// `scope` gives it.
fn favorites(base: Rgba, dark: bool, scope: FavoritesScope) -> gpui::Div {
    let lists = lists(scope);
    let plate = |spaces: Vec<gpui::Div>| {
        div()
            .flex()
            .gap(px(SHARE_PLATE_GAP))
            .p(px(SHARE_PLATE_PADDING))
            .rounded(px(SHARE_PLATE_RADIUS))
            .bg(base.opacity(SHARE_PLATE))
            .children(spaces)
    };
    let space = |index: usize| space(base, dark, SHARE_SPACES[index], LISTS[lists[index]]);
    div()
        .flex_none()
        .w(px(SIDE_WIDTH))
        .h(px(SIDE_HEIGHT))
        .rounded(px(SIDE_RADIUS))
        .bg(rgb(if dark { SIDE_WINDOW } else { SIDE_WINDOW_LIGHT }))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(SHARE_PROFILES_GAP))
        .child(plate(vec![space(0), space(1)]))
        .child(plate(vec![space(2)]))
}

/// One Space's sidebar in its `color`: its four favorites' tiles, then three tabs.
fn space(base: Rgba, dark: bool, color: u32, (sites, mark): ([u32; 4], Mark)) -> gpui::Div {
    let tint = if dark { SHARE_TINT } else { SHARE_TINT_LIGHT };
    let tile_fill = match dark {
        true => base.opacity(SHARE_TILE),
        false => rgb(SHARE_TILE_LIGHT),
    };
    let tile = |site: u32| {
        let shape = div().flex_none().size(px(SHARE_MARK));
        let shape = match mark {
            Mark::Dot => shape.rounded_full().bg(rgb(site)),
            Mark::Square => shape.rounded(px(SHARE_MARK_SQUARE)).bg(rgb(site)),
            Mark::Ring => shape
                .rounded_full()
                .border(px(SHARE_RING))
                .border_color(rgb(site)),
        };
        div()
            .flex_none()
            .w(px(SHARE_TILE_WIDTH))
            .h(px(SHARE_TILE_HEIGHT))
            .rounded(px(SHARE_TILE_RADIUS))
            .bg(tile_fill)
            .shadow(vec![spread(base.opacity(SHARE_TILE_RIM), 0.5)])
            .flex()
            .items_center()
            .justify_center()
            .child(shape)
    };
    let row = |pair: &[u32]| {
        div()
            .flex()
            .gap(px(SHARE_TILE_GAP))
            .children(pair.iter().map(|&site| tile(site)))
    };
    let tabs = [0.86, 0.7, 0.78].map(|width| {
        div()
            .flex_none()
            .h(px(SHARE_TAB_HEIGHT))
            .w(gpui::relative(width))
            .rounded_full()
            .bg(base.opacity(SIDE_BAR))
    });
    div()
        .flex_none()
        .w(px(SHARE_SPACE_WIDTH))
        .h(px(SHARE_SPACE_HEIGHT))
        .rounded(px(SHARE_SPACE_RADIUS))
        .bg(rgb(color).opacity(tint))
        .py(px(SHARE_SPACE_PADDING))
        .px(px(SHARE_SPACE_PADDING_X))
        .flex()
        .flex_col()
        .gap(px(SHARE_SPACE_GAP))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(SHARE_TILE_GAP))
                .child(row(&sites[..2]))
                .child(row(&sites[2..])),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(SHARE_TAB_GAP))
                .px(px(1.))
                .children(tabs),
        )
}

/// The window drawn: its material with the sidebar on `right` or not, its buttons at its top,
/// and the page's card on the other side.
fn window(base: Rgba, dark: bool, right: bool, buttons: Buttons) -> gpui::Div {
    let bar = |height: f32, width: f32, alpha: f32| {
        div()
            .flex_none()
            .h(px(height))
            .w(gpui::relative(width))
            .rounded_full()
            .bg(base.opacity(alpha))
    };
    let head = div()
        .flex_none()
        .h(px(SIDE_BUTTONS))
        .flex()
        .items_center()
        .when(right, |head| head.justify_end())
        .children(window_buttons(base, right, buttons));
    let sidebar = div()
        .flex_none()
        .w(px((SIDE_WIDTH * SIDE_SIDEBAR).round()))
        .flex()
        .flex_col()
        .gap(px(SIDE_ROWS_GAP))
        .py(px(SIDE_SIDEBAR_PADDING))
        .px(px(SIDE_INSET))
        .child(head)
        .children(
            [0.8, 0.64, 0.72, 0.56, 0.68]
                .iter()
                .enumerate()
                .map(|(index, &width)| {
                    let alpha = if index == 0 { SIDE_BAR_FIRST } else { SIDE_BAR };
                    bar(4., width, alpha)
                }),
        );
    let page_fill = rgb(if dark { SIDE_PAGE } else { SIDE_PAGE_LIGHT });
    let page = div()
        .flex_1()
        .my(px(SIDE_INSET))
        .map(|page| match right {
            true => page.ml(px(SIDE_INSET)),
            false => page.mr(px(SIDE_INSET)),
        })
        .rounded(px(SIDE_PAGE_RADIUS))
        .bg(page_fill)
        .shadow(vec![spread(base.opacity(SIDE_PAGE_RING), 0.5)])
        .flex()
        .flex_col()
        .gap(px(SIDE_ROWS_GAP))
        .p(px(SIDE_PAGE_PADDING))
        .child(bar(5., 0.55, SIDE_HEADING))
        .children(
            [0.9, 0.82, 0.88, 0.6]
                .iter()
                .map(|&width| bar(3., width, SIDE_TEXT)),
        );
    div()
        .flex_none()
        .w(px(SIDE_WIDTH))
        .h(px(SIDE_HEIGHT))
        .rounded(px(SIDE_RADIUS))
        .overflow_hidden()
        .bg(rgb(if dark { SIDE_WINDOW } else { SIDE_WINDOW_LIGHT }))
        .flex()
        .map(|window| match right {
            true => window.child(page).child(sidebar),
            false => window.child(sidebar).child(page),
        })
}

/// The window's buttons in a drawing, close at the corner on either side.
fn window_buttons(base: Rgba, right: bool, buttons: Buttons) -> Vec<AnyElement> {
    let dot = |color: u32| {
        div()
            .flex_none()
            .size(px(SIDE_BUTTON))
            .rounded_full()
            .bg(rgb(color))
    };
    let (colors, gap) = match buttons {
        Buttons::Mac => (
            [SIDE_MAC_CLOSE, SIDE_MAC_MINIMIZE, SIDE_MAC_ZOOM],
            SIDE_BUTTON_GAP,
        ),
        Buttons::Linux => (
            [
                WINDOW_BUTTON_CLOSE,
                WINDOW_BUTTON_MINIMIZE,
                WINDOW_BUTTON_ZOOM,
            ],
            SIDE_BUTTON_GAP,
        ),
        Buttons::Windows => {
            let glyphs = [
                "icons/window-windows-minimize.svg",
                "icons/window-windows-maximize.svg",
                "icons/window-windows-close.svg",
            ];
            let row = div()
                .flex()
                .gap(px(SIDE_GLYPH_GAP))
                .children(glyphs.map(|glyph| {
                    svg()
                        .flex_none()
                        .path(glyph)
                        .size(px(SIDE_BUTTON))
                        .text_color(base.opacity(SIDE_GLYPH))
                }));
            return vec![row.into_any_element()];
        }
    };
    let mut order = colors.to_vec();
    if right {
        order.reverse();
    }
    vec![
        div()
            .flex()
            .gap(px(gap))
            .children(order.into_iter().map(dot))
            .into_any_element(),
    ]
}

/// A ring `width` out from an element's edge, in `color`.
fn spread(color: Rgba, width: f32) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset: false,
    }
}

#[cfg(test)]
mod tests;
