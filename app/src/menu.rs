// Adapted from Zed's crates/ui/src/components/context_menu.rs
// (https://github.com/zed-industries/zed) at commit b36b283021c48f1712714b8a0e2ad6ffb578920b:
// the keyboard's walk over the selectable rows and the submenu's open and close. Copyright Zed
// Industries, Inc. Licensed under the GNU General Public License, version 3 or later.

//! ilo's own context menu, the same on every platform (#324): a panel of 24-pt rows, with
//! submenus, keys, type-to-select, press-drag-release and a safe triangle on the way to a
//! submenu. GPUI can't draw over the page's native view, so it's a window of its own that the host
//! shows over the browser window, as its child, with the keyboard, as the command bar is
//! (AGENTS.md › Over the page). What the menu decides is in `Tracking` and `lay_out`, which the
//! tests drive; what it asks of its window goes through `Host`.

use std::time::Instant;
use std::{rc::Rc, time::Duration};

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Focusable, Global, Hsla,
    KeyDownEvent, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, Radians, Rgba, Role, ScrollEvent, SharedString, Subscription, Task,
    TitlebarOptions, Transformation, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, img, point, prelude::*, px, rgb, rgba, size, svg,
};

use crate::{
    elevation::Reach,
    list_keys::TypeAhead,
    shell::{MenuEntry, Shell, keystroke_label},
    text_input::{self, FieldMenu, TextInput},
};

pub use crate::tokens::menu::*;

/// The panel's colour, 0xRRGGBBAA, opaque: what floats over a dark or a light window, where the
/// host has no material under it (#4090).
pub const DARK_FILL: u32 = crate::tokens::floating::FILL | 0xff;
pub const LIGHT_FILL: u32 = crate::tokens::floating::LIGHT_FILL | 0xff;

/// A release this close to the press that opened the menu leaves it open, as a click.
const DRAG_SLOP: f32 = 4.;

/// The colours of one look, 0xRRGGBBAA, which capture's menu shares.
pub struct Palette {
    pub fill: u32,
    pub rim: u32,
    pub label: u32,
    pub secondary: u32,
    pub line: u32,
    pub disabled: u32,
    pub destructive: u32,
}

const DARK: Palette = Palette {
    fill: DARK_FILL,
    rim: crate::tokens::floating::RIM,
    label: DARK_LABEL,
    secondary: DARK_SECONDARY,
    line: DARK_LINE,
    disabled: DARK_DISABLED,
    destructive: DARK_DESTRUCTIVE,
};

const LIGHT: Palette = Palette {
    fill: LIGHT_FILL,
    rim: crate::tokens::floating::LIGHT_RIM,
    label: LIGHT_LABEL,
    secondary: LIGHT_SECONDARY,
    line: LIGHT_LINE,
    disabled: LIGHT_DISABLED,
    destructive: LIGHT_DESTRUCTIVE,
};

/// A plate under a label: its fill, the fill while the button is down on it, and the label on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub fill: Rgba,
    pub pressed: Rgba,
    pub label: Rgba,
}

impl Highlight {
    /// The Space's accent, pressed moved away from the label the palette solves on it, so it reads
    /// at least as well pressed: what capture's markup marks chosen.
    pub fn of(palette: &crate::palette::Palette) -> Self {
        Highlight {
            fill: palette.accent,
            pressed: palette.accent_pressed().into(),
            label: palette.accent_label,
        }
    }

    /// The row under the pointer or the keys in a `light` or dark menu: a neutral plate in the
    /// label's ink, never the Space's accent (#4090), under the label in its whole ink.
    pub fn plate(light: bool) -> Self {
        let ink = rgba(if light { LIGHT_LABEL } else { DARK_LABEL });
        Highlight {
            fill: Rgba { a: PLATE, ..ink },
            pressed: Rgba {
                a: PLATE_PRESSED,
                ..ink
            },
            label: Rgba { a: 1., ..ink },
        }
    }
}

/// A Space's mark in a menu (#320): its theme's gradient, `from` at the top left to `to` at the
/// bottom right, as 0xRRGGBB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub from: u32,
    pub to: u32,
}

impl Swatch {
    /// The mark as a menu draws it on `panel`, ringed where its edge would fade into the panel.
    pub fn dot(self, panel: Rgba) -> gpui::Div {
        crate::palette::Swatch::of_two(rgb(self.from), rgb(self.to)).draw(SWATCH, &[panel])
    }
}

/// The menu's look: the panel's material, Ink in a dark window and Paper in a light one, and its
/// Space's accent, which capture's markup marks what's chosen in; a menu's rows take the plate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub light: bool,
    pub accent: Highlight,
}

impl Default for Look {
    fn default() -> Self {
        Look::dark()
    }
}

impl Look {
    /// The dark panel with no theme's accent, for a window that isn't a Space's, such as Settings.
    pub fn dark() -> Self {
        Look::plain(false)
    }

    /// The panel with no theme's accent, light or dark as its window is, as Settings' pop-ups'.
    pub fn plain(light: bool) -> Self {
        Look {
            light,
            accent: Highlight::of(&crate::palette::Palette::default()),
        }
    }

    pub fn palette(self) -> &'static Palette {
        if self.light { &LIGHT } else { &DARK }
    }

    /// The row under the pointer or the keys.
    pub fn plate(self) -> Highlight {
        Highlight::plate(self.light)
    }

    /// The secondary ink of a shortcut, a chevron, a detail or a heading: the look's where it
    /// reads over the material, or as much more as does (`palette::float_ink`).
    pub fn secondary(self) -> Rgba {
        let ink = rgba(self.palette().secondary);
        Rgba {
            a: crate::palette::Float::Menu.word(self.tone(), ink.a),
            ..ink
        }
    }

    /// A disabled row's ink: the look's at every level, held to no ratio, and never fainter.
    pub fn disabled(self) -> Rgba {
        rgba(self.palette().disabled)
    }

    /// The panel as drawn: frosted where the host blurs under the menu's window (#4090).
    pub fn fill(self) -> Rgba {
        crate::palette::Float::Menu.fill(self.tone())
    }

    fn tone(self) -> crate::palette::Tone {
        match self.light {
            true => crate::palette::Tone::Light,
            false => crate::palette::Tone::Dark,
        }
    }
}

/// `panel` in a `light` or dark menu's glass (#4090): its fill, frosted where the host blurs under
/// it, a light from the top, the rim inside its edge, and where it's `shadowed`, `radius` corners,
/// its shadows and the outline. Its children go over the light.
pub fn glass<E: Styled + ParentElement + gpui::prelude::FluentBuilder>(
    panel: E,
    light: bool,
    radius: f32,
    shadowed: bool,
) -> E {
    let look = Look::plain(light);
    let (top, end) = match light {
        true => (LIGHT_TOP_LIGHT, LIGHT_TOP_LIGHT_END),
        false => (DARK_TOP_LIGHT, DARK_TOP_LIGHT_END),
    };
    // GPUI draws a border over what's inside, and an inset shadow from the panel's edge under it,
    // so a line inside the 1-pt rim lies 2 pt in.
    let inner = |color: u32, y: f32, spread: f32| gpui::BoxShadow {
        color: rgba(color).into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(0.),
        spread_radius: px(spread),
        inset: true,
    };
    let mut shadows = match shadowed {
        true => crate::elevation::menu_shadow(),
        false => Vec::new(),
    };
    shadows.push(match light {
        true => inner(LIGHT_INNER_RIM, 0., 2.),
        false => inner(DARK_TOP_LINE, 2., 0.),
    });
    panel
        .when(shadowed, |panel| panel.rounded(px(radius)))
        .bg(look.fill())
        .border_1()
        .border_color(rgba(look.palette().rim))
        .shadow(shadows)
        .child(
            div()
                .absolute()
                .inset_0()
                .when(shadowed, |light| light.rounded(px(radius)))
                .bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(rgba(top), 0.),
                    gpui::linear_color_stop(rgba(end), TOP_LIGHT_STOP),
                )),
        )
}

/// What the menu asks of its window. Both are called once the menu's update is over, as putting a
/// window on screen or taking it off can draw it, which would need the menu while it's held.
pub trait Host {
    /// Puts the menu's window over the browser window it's for at `frame` in that window's
    /// content, as it draws now, with the keyboard. `panels` are its levels' panels in it, from its
    /// top left, open or not: the rest is their shadows' room, which a click goes through. Where
    /// the window can't blend, they're only the open ones, which it shows alone, and it's shown
    /// again as a submenu opens or closes.
    fn show(&self, frame: Bounds<Pixels>, panels: &[Bounds<Pixels>]);
    /// Puts the host's material under the open `panels`, each from the window's top left with its
    /// corner's radius, in a `light` or dark look, where the host has one for the menu (#4090).
    /// Asked after `show` and again as a submenu opens or closes, as the material goes under the
    /// panels that show and not the room round them.
    fn glass(&self, _panels: &[(Bounds<Pixels>, f32)], _light: bool) {}
    /// Takes it away, and gives the keyboard back to what had it.
    fn hide(&self);
    /// A right press closed the menu at `at` in the browser window's content, in the room round
    /// the panels. Where the window has the pointer over its whole frame, as the Mac's, the press
    /// would go on to the menu's own window, which nothing hears, so the host gives it to the
    /// window under it once this update is over, and returns that it does. A window that takes the
    /// pointer on its panels alone has nothing to do: the press is the other window's already.
    fn press_under(&self, _at: Point<Pixels>, _cx: &mut App) -> bool {
        false
    }
    /// Whether the window blends with what's under it. One that can't, on X11 with no compositor,
    /// shows its clear pixels black (#1479), so there the panels are opaque and square with no
    /// shadow, and the window holds them alone.
    fn blends(&self) -> bool {
        true
    }
}

/// A menu to show.
pub struct Request {
    pub items: Vec<MenuEntry>,
    /// The pointer, in the browser window's content: the menu's top left goes there.
    pub at: Point<Pixels>,
    /// Where it may go, in the same terms: the screen's work area.
    pub area: Bounds<Pixels>,
    pub look: Look,
    /// Called once it closes, with how it closed.
    pub chosen: Chosen,
}

pub type Chosen = Box<dyn FnOnce(Closed, &mut App)>;

/// How a menu closed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Closed {
    /// With the item at this index chosen.
    Chose(usize),
    /// With the user gone to another window or app, or its browser window changed under it.
    Left,
    /// With a press outside it, its keys, a shortcut or another menu taking its place.
    Otherwise,
}

/// What a row that stays does each time it's chosen (`MenuEntry::stays`), answering whether it
/// shows its check now.
pub type Stays = Rc<dyn Fn(&mut App) -> bool>;

/// The log's line for a pick that left the menu open (`MenuEntry::stays`).
const STAYED: &str = "browser: menu picked a row that stays";

/// How a menu closed, with `Otherwise`'s ways told apart, for the line the log has of it (#2873).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Closing {
    Chose(usize),
    PressedOutside,
    Left,
    Escape,
    Shortcut,
    ReleasedOutside,
    Replaced,
}

impl Closing {
    fn closed(self) -> Closed {
        match self {
            Closing::Chose(item) => Closed::Chose(item),
            Closing::Left => Closed::Left,
            _ => Closed::Otherwise,
        }
    }

    /// The log's line for the menu opened at `at`, in the browser window's content, closing so:
    /// where it opened says which menu, and the line names no item, so nothing of a page or a tab.
    fn line(self, at: Point<Pixels>) -> String {
        let how = match self {
            Closing::Chose(_) => "picked",
            Closing::PressedOutside => "pressed outside",
            Closing::Left => "left",
            Closing::Escape => "escape",
            Closing::Shortcut => "shortcut",
            Closing::ReleasedOutside => "released outside",
            Closing::Replaced => "another menu",
        };
        format!(
            "browser: menu at {} {} closed by {how}",
            f32::from(at.x),
            f32::from(at.y)
        )
    }
}

impl Closed {
    /// The item chosen, if one was.
    pub fn chosen(self) -> Option<usize> {
        match self {
            Closed::Chose(item) => Some(item),
            _ => None,
        }
    }
}

/// `chosen` as a menu's answer, for whoever hears only which item was chosen.
pub fn picked(chosen: impl FnOnce(Option<usize>, &mut App) + 'static) -> Chosen {
    Box::new(move |closed: Closed, cx: &mut App| chosen(closed.chosen(), cx))
}

/// One level of the menu, the root or a submenu, placed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Level {
    /// The entry whose submenu it is, none for the root.
    parent: Option<usize>,
    items: Vec<usize>,
    bounds: Bounds<Pixels>,
    /// Each item's top, from the panel's top.
    tops: Vec<f32>,
    /// Whether a column for checks comes before the titles: a level of choices, in quiet rows.
    checks: bool,
    /// The run of rows that scroll, where it's longer than `SCROLL_ROWS`.
    scroll: Option<Scroll>,
}

/// A run of rows marked `scrolls` that shows `SCROLL_ROWS` of them at a time, as the Space
/// button's menu shows its Spaces, with the rows after it kept in place under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Scroll {
    /// The run's first and last rows, in the level.
    first: usize,
    last: usize,
    /// Where the run starts, from the panel's top, how tall it shows and how tall it is.
    top: f32,
    shown: f32,
    whole: f32,
    /// How far down it's scrolled.
    by: f32,
}

impl Scroll {
    fn holds(&self, row: usize) -> bool {
        (self.first..=self.last).contains(&row)
    }

    /// Scrolled by `by`, as far as the run goes.
    fn to(mut self, by: f32) -> Scroll {
        self.by = by.clamp(0., self.whole - self.shown);
        self
    }
}

/// The run of `heights`' rows marked to scroll, its first and last, how tall `SCROLL_ROWS` of them
/// are and how tall it is, where it's longer than that.
fn scroll_run(heights: &[(bool, f32)]) -> Option<(usize, usize, f32, f32)> {
    let first = heights.iter().position(|&(scrolls, _)| scrolls)?;
    let last = first
        + heights[first..]
            .iter()
            .take_while(|&&(scrolls, _)| scrolls)
            .count()
        - 1;
    let run = &heights[first..=last];
    (run.len() > SCROLL_ROWS as usize).then(|| {
        let shown = run.iter().take(SCROLL_ROWS as usize).map(|&(_, h)| h).sum();
        (first, last, shown, run.iter().map(|&(_, h)| h).sum())
    })
}

/// How tall `entry` stands in its level, a line or a row, quiet in a level of choices.
fn row_height(entry: &MenuEntry, choices: bool) -> f32 {
    if entry.field {
        return FIELD_ROW;
    }
    match (entry.title.is_empty(), choices) {
        (true, _) => LINE,
        (false, true) => QUIET_ROW,
        (false, false) => ROW,
    }
}

impl Level {
    /// Whether row `at` takes the pointer at `point`: under it, and where the run of rows that
    /// scroll shows, if it's one of them.
    fn takes(&self, at: usize, point: Point<Pixels>, entries: &[MenuEntry]) -> bool {
        let shows = self
            .scroll
            .filter(|scroll| scroll.holds(at))
            .is_none_or(|scroll| {
                let y = f32::from(point.y - self.bounds.origin.y);
                (scroll.top..scroll.top + scroll.shown).contains(&y)
            });
        shows && self.row_bounds(at, entries).contains(&point)
    }

    /// The run scrolled as far as keeps row `at` in sight, if it's one of the run's.
    fn revealing(&self, at: usize, entries: &[MenuEntry]) -> Option<Scroll> {
        let scroll = self.scroll.filter(|scroll| scroll.holds(at))?;
        let top = self.tops[at] - scroll.top;
        let bottom = top + row_height(&entries[self.items[at]], self.checks);
        let by = scroll.by.min(top).max(bottom - scroll.shown);
        Some(scroll.to(by))
    }

    /// The panel's corner and its rows', quiet in a level of choices.
    fn radii(&self) -> (f32, f32) {
        match self.checks {
            true => (QUIET_RADIUS, QUIET_ROW_RADIUS),
            false => (RADIUS, ROW_RADIUS),
        }
    }

    fn row_bounds(&self, at: usize, entries: &[MenuEntry]) -> Bounds<Pixels> {
        let height = row_height(&entries[self.items[at]], self.checks);
        let by = (self.scroll.filter(|scroll| scroll.holds(at))).map_or(0., |scroll| scroll.by);
        let origin = self.bounds.origin + point(px(PADDING), px(self.tops[at] - by));
        Bounds::new(
            origin,
            size(self.bounds.size.width - px(2. * PADDING), px(height)),
        )
    }
}

/// Where each level of a menu goes, and the window that holds them with their shadows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    levels: Vec<Level>,
    pub(crate) frame: Bounds<Pixels>,
}

impl Layout {
    /// The level that's the submenu of `entry`.
    fn level_of(&self, entry: usize) -> Option<usize> {
        self.levels
            .iter()
            .position(|level| level.parent == Some(entry))
    }

    /// As laid out for a window that can't blend: each panel out to whole points, since a pixel its
    /// edge only partly covers would show black, and the window just round them, with no shadows.
    pub(crate) fn opaque(mut self) -> Layout {
        for level in &mut self.levels {
            let (origin, end) = (level.bounds.origin, level.bounds.bottom_right());
            level.bounds =
                Bounds::from_corners(origin.map(|at| at.floor()), end.map(|at| at.ceil()));
        }
        self.frame = framed(
            &self.levels,
            Reach {
                above: 0.,
                below: 0.,
                sides: 0.,
            },
        );
        self
    }
}

/// The window round the panels of `levels` and the `reach` of their shadows, out to whole points.
/// A window put at a fraction of one lands on a pixel, so each pointer move reached the menu twice,
/// a quarter point apart, and the second left the safe triangle.
fn framed(levels: &[Level], reach: Reach) -> Bounds<Pixels> {
    let union =
        (levels.iter().skip(1)).fold(levels[0].bounds, |all, level| all.union(&level.bounds));
    Bounds::from_corners(
        (union.origin - point(px(reach.sides), px(reach.above))).map(|at| at.floor()),
        (union.bottom_right() + point(px(reach.sides), px(reach.below))).map(|at| at.ceil()),
    )
}

/// How wide a level's panel is for titles and shortcuts `width_of` measures, between the
/// minimum and the maximum, where longer titles end in "…".
fn panel_width(
    entries: &[MenuEntry],
    items: &[usize],
    checks: bool,
    width_of: &dyn Fn(&str) -> f32,
) -> f32 {
    // An item's ⌥ alternate takes its row while ⌥ is held, so the panel fits both.
    let widest = (items
        .iter()
        .flat_map(|&at| std::iter::once(at).chain(alternate_of(entries, at))))
    .map(|at| &entries[at])
    .filter(|entry| !entry.title.is_empty())
    .map(|entry| {
        let trailing = match &entry.shortcut {
            Some(keys) => GAP + width_of(&keystroke_label(keys)),
            None => 0.,
        };
        let detail = (entry.detail.as_ref()).map_or(0., |detail| DETAIL_GAP + width_of(detail));
        width_of(&entry.title) + detail + trailing
    })
    .fold(0., f32::max);
    let submenus = items
        .iter()
        .any(|&at| entries.iter().any(|each| each.parent == Some(at)));
    let lead = if checks { CHECK + GAP } else { 0. };
    let swatches = items.iter().any(|&at| entries[at].swatch.is_some());
    let lead = lead + if swatches { SWATCH + GAP } else { 0. };
    let pages = items.iter().any(|&at| entries[at].page);
    let lead = lead + if pages { FAVICON + GAP } else { 0. };
    let icons = (items.iter()).any(|&at| entries[at].icon.is_some() || entries[at].mark.is_some());
    let lead = lead + if icons { ICON + GAP } else { 0. };
    let chevron = if submenus { GAP + CHEVRON } else { 0. };
    let out = items.iter().any(|&at| entries[at].out);
    let chevron = chevron + if out { GAP + OUT_ARROW } else { 0. };
    (2. * (PADDING + ROW_X) + lead + widest + chevron).clamp(MIN_WIDTH, MAX_WIDTH)
}

/// The item shown in place of `item` while ⌥ is held, as AppKit's alternates are: the one after it,
/// if that's marked so.
fn alternate_of(entries: &[MenuEntry], item: usize) -> Option<usize> {
    entries
        .get(item + 1)
        .is_some_and(|next| next.alternate)
        .then_some(item + 1)
}

/// The item that `item`'s row is while ⌥ is held, if `alt`, or else.
pub(crate) fn as_held(entries: &[MenuEntry], item: usize, alt: bool) -> usize {
    alternate_of(entries, item).filter(|_| alt).unwrap_or(item)
}

/// `origin` moved inside `area` by EDGE for a panel of `size`, as far as it fits.
fn clamped(origin: Point<Pixels>, size: (f32, f32), area: Bounds<Pixels>) -> Point<Pixels> {
    let (left, top) = (f32::from(area.left()) + EDGE, f32::from(area.top()) + EDGE);
    let (right, bottom) = (
        f32::from(area.right()) - EDGE,
        f32::from(area.bottom()) - EDGE,
    );
    let x = f32::from(origin.x).min(right - size.0).max(left);
    let y = f32::from(origin.y).min(bottom - size.1).max(top);
    point(px(x), px(y))
}

/// Whether `entry` asks its level for the checks' column: checked, or a row that stays, which may
/// be at any pick, so its title never moves under the pointer.
fn marked(entry: &MenuEntry) -> bool {
    entry.checked || entry.stays.is_some()
}

/// How wide the menu of `entries` is, without its submenus, as `lay_out` makes its panel in
/// `window`'s text, for a menu that hangs from a control by its right edge.
pub(crate) fn root_width(entries: &[MenuEntry], window: &Window) -> f32 {
    let items: Vec<usize> = (0..entries.len())
        .filter(|&at| entries[at].parent.is_none() && !entries[at].alternate)
        .collect();
    let checks = items.iter().any(|&at| marked(&entries[at]));
    panel_width(entries, &items, checks, &measure(window))
}

/// How tall the menu of `entries` is, without its submenus, as `lay_out` stacks its rows and lines,
/// so a control can open it with its foot where it wants.
pub(crate) fn height(entries: &[MenuEntry]) -> f32 {
    let top = (entries.iter()).filter(|entry| entry.parent.is_none() && !entry.alternate);
    let choices = top.clone().any(marked);
    let heights: Vec<(bool, f32)> =
        (top.map(|entry| (entry.scrolls, row_height(entry, choices)))).collect();
    let rows: f32 = heights.iter().map(|&(_, height)| height).sum();
    let hidden = scroll_run(&heights).map_or(0., |(_, _, shown, whole)| whole - shown);
    PADDING + rows - hidden + PADDING
}

/// Lays `entries` out as a menu with its top left at `at`, flipping up or left where `area` has
/// no room below or to the right, as AppKit's does, with each submenu beside the row that opens
/// it, flipping left and sliding up to fit.
pub(crate) fn lay_out(
    entries: &[MenuEntry],
    at: Point<Pixels>,
    area: Bounds<Pixels>,
    width_of: &dyn Fn(&str) -> f32,
) -> Layout {
    lay_out_wide(entries, at, area, width_of, 0., false)
}

/// Lays `entries` out as `lay_out` does, with the root's panel at least `least` wide, and a level
/// of choices if `choices`: a search's rows keep the panel and the rows they were narrowed in, as
/// the chosen row goes.
fn lay_out_wide(
    entries: &[MenuEntry],
    at: Point<Pixels>,
    area: Bounds<Pixels>,
    width_of: &dyn Fn(&str) -> f32,
    least: f32,
    choices: bool,
) -> Layout {
    let parents: Vec<Option<usize>> = std::iter::once(None)
        .chain(
            (0..entries.len())
                .filter(|&at| entries.iter().any(|e| e.parent == Some(at)))
                .map(Some),
        )
        .collect();
    let mut levels: Vec<Level> = Vec::new();
    for parent in parents {
        let items: Vec<usize> = (0..entries.len())
            .filter(|&at| entries[at].parent == parent && !entries[at].alternate)
            .collect();
        let checks =
            items.iter().any(|&item| marked(&entries[item])) || (choices && parent.is_none());
        let heights: Vec<(bool, f32)> = (items.iter())
            .map(|&item| (entries[item].scrolls, row_height(&entries[item], checks)))
            .collect();
        let run = scroll_run(&heights);
        let mut tops = Vec::with_capacity(items.len());
        let mut y = PADDING;
        let mut scroll = None;
        for (row, &(_, height)) in heights.iter().enumerate() {
            if let Some((first, _, shown, whole)) = run
                && row == first
            {
                scroll = Some(Scroll {
                    first,
                    last: run.map_or(first, |run| run.1),
                    top: y,
                    shown,
                    whole,
                    by: 0.,
                });
            }
            tops.push(y);
            y += height;
            // The rows after the run stand under what it shows.
            if let Some((_, last, shown, whole)) = run
                && row == last
            {
                y -= whole - shown;
            }
        }
        let width = panel_width(entries, &items, checks, width_of);
        let width = if parent.is_none() {
            width.max(least)
        } else {
            width
        };
        let size = (width, y + PADDING);
        let origin = match parent {
            None => {
                let (x, y) = (f32::from(at.x), f32::from(at.y));
                let x = if x + size.0 > f32::from(area.right()) - EDGE {
                    x - size.0
                } else {
                    x
                };
                let y = if y + size.1 > f32::from(area.bottom()) - EDGE {
                    y - size.1
                } else {
                    y
                };
                clamped(point(px(x), px(y)), size, area)
            }
            Some(parent) => {
                // A submenu's parent's level comes before it, as an entry's parent comes first.
                let Some(above) = levels.iter().find(|level| level.items.contains(&parent)) else {
                    continue;
                };
                let row = above.row_bounds(
                    above.items.iter().position(|&i| i == parent).unwrap_or(0),
                    entries,
                );
                let right = f32::from(above.bounds.right()) - OVERLAP;
                let x = if right + size.0 > f32::from(area.right()) - EDGE {
                    f32::from(above.bounds.left()) + OVERLAP - size.0
                } else {
                    right
                };
                clamped(point(px(x), row.origin.y - px(PADDING)), size, area)
            }
        };
        let bounds = Bounds::new(origin, gpui::size(px(size.0), px(size.1)));
        levels.push(Level {
            parent,
            items,
            bounds,
            tops,
            checks,
            scroll,
        });
    }
    let frame = framed(&levels, crate::elevation::menu_reach());
    Layout { levels, frame }
}

/// Lays `entries` out as `lay_out` does, but with the root's item marked `over` with its title's
/// top left at `at`, as a pop-up button's menu opens over the button's title.
pub(crate) fn lay_out_over(
    entries: &[MenuEntry],
    at: Point<Pixels>,
    area: Bounds<Pixels>,
    width_of: &dyn Fn(&str) -> f32,
) -> Layout {
    let layout = lay_out(entries, at, area, width_of);
    let root = &layout.levels[0];
    let Some(row) = root.items.iter().position(|&item| entries[item].over) else {
        return layout;
    };
    let lead = if root.checks { CHECK + GAP } else { 0. };
    // A taller row centres its title lower, so its title, not its top, lands on the button's.
    let bounds = root.row_bounds(row, entries);
    let lower = (f32::from(bounds.size.height) - ROW) / 2.;
    let title = bounds.origin + point(px(ROW_X + lead), px(lower));
    lay_out(entries, at - (title - root.bounds.origin), area, width_of)
}

/// What's under a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    /// An item's row, by level and entry.
    Row(usize, usize),
    /// A level's panel, between rows or on a line.
    Panel(usize),
    Outside,
}

/// A key the menu takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Up,
    Down,
    First,
    Last,
    /// Into the highlighted row's submenu.
    Right,
    /// Out of the submenu.
    Left,
    /// Return or space: picks the highlighted row.
    Choose,
    Escape,
    Char(char),
}

/// What the user did to the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Stay,
    Pick(usize),
    Escape,
    ReleasedOutside,
}

/// An open level and the item highlighted in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Open {
    level: usize,
    highlight: Option<usize>,
}

/// A menu from its press until it closes: which levels are open and what's highlighted, from the
/// pointer and the keys.
pub(crate) struct Tracking {
    entries: Vec<MenuEntry>,
    layout: Layout,
    /// Root first, then each submenu open in the one before.
    open: Vec<Open>,
    /// The press that opened the menu, until its release: where it was, or for a pop-up's menu,
    /// which opens over its button's title wherever the press was, where the pointer first moved
    /// to while it's held.
    press: Option<Option<Point<Pixels>>>,
    /// A button went down on the menu and hasn't come up.
    pressed: bool,
    pointer: Option<Point<Pixels>>,
    /// Since when the pointer has rested in the safe triangle, keeping the submenu open.
    resting: Option<Instant>,
    typed: TypeAhead,
}

/// Whether `at` is inside the triangle `a`, `b`, `c`.
pub(crate) fn in_triangle(
    at: Point<Pixels>,
    a: Point<Pixels>,
    b: Point<Pixels>,
    c: Point<Pixels>,
) -> bool {
    let cross = |p: Point<Pixels>, q: Point<Pixels>, r: Point<Pixels>| {
        f32::from(q.x - p.x) * f32::from(r.y - p.y) - f32::from(q.y - p.y) * f32::from(r.x - p.x)
    };
    let (d1, d2, d3) = (cross(a, b, at), cross(b, c, at), cross(c, a, at));
    let negative = d1 < 0. || d2 < 0. || d3 < 0.;
    let positive = d1 > 0. || d2 > 0. || d3 > 0.;
    !(negative && positive)
}

impl Tracking {
    pub(crate) fn new(entries: Vec<MenuEntry>, mut layout: Layout, at: Point<Pixels>) -> Self {
        // A pop-up's menu opens with its current item highlighted.
        let over = (layout.levels[0].items.iter().copied()).find(|&item| {
            entries[item].over && entries[item].enabled && !entries[item].title.is_empty()
        });
        // A run of rows that scrolls opens with its checked one in sight.
        let root = &layout.levels[0];
        let checked = (root.items.iter()).position(|&item| entries[item].checked);
        if let Some(scroll) = checked.and_then(|row| root.revealing(row, &entries)) {
            layout.levels[0].scroll = Some(scroll);
        }
        Tracking {
            entries,
            layout,
            open: vec![Open {
                level: 0,
                highlight: over,
            }],
            press: Some(over.is_none().then_some(at)),
            pressed: false,
            pointer: Some(at),
            resting: None,
            typed: TypeAhead::default(),
        }
    }

    /// The items whose submenus are open, the outermost first.
    fn open_submenus(&self) -> Vec<usize> {
        self.open[1..]
            .iter()
            .filter_map(|open| self.layout.levels[open.level].parent)
            .collect()
    }

    fn selectable(&self, entry: usize) -> bool {
        let entry = &self.entries[entry];
        entry.enabled && !entry.title.is_empty()
    }

    /// The item highlighted in each open level, root first.
    #[cfg(test)]
    pub(crate) fn highlights(&self) -> Vec<Option<usize>> {
        self.open.iter().map(|open| open.highlight).collect()
    }

    /// The titles of what's highlighted, root first, for the tests.
    #[cfg(test)]
    fn highlighted(&self) -> Vec<&str> {
        (self.open.iter().filter_map(|open| open.highlight))
            .map(|at| self.entries[at].title.as_ref())
            .collect()
    }

    fn hit(&self, at: Point<Pixels>) -> Hit {
        for open in self.open.iter().rev() {
            let level = &self.layout.levels[open.level];
            if !level.bounds.contains(&at) {
                continue;
            }
            let row = (0..level.items.len()).find(|&row| level.takes(row, at, &self.entries));
            return match row {
                Some(row)
                    if !self.entries[level.items[row]].title.is_empty()
                        && !self.entries[level.items[row]].field =>
                {
                    Hit::Row(open.level, level.items[row])
                }
                _ => Hit::Panel(open.level),
            };
        }
        Hit::Outside
    }

    /// The pointer is at `at`, which highlights what's under it and opens its submenu, unless it's
    /// on its way to the open submenu (`safe`).
    fn point_at(&mut self, at: Point<Pixels>) {
        let depth = |open: &[Open], level| open.iter().position(|each| each.level == level);
        match self.hit(at) {
            Hit::Row(level, item) => {
                let Some(k) = depth(&self.open, level) else {
                    return;
                };
                let sub = self.layout.level_of(item);
                if sub.is_some() && self.open.get(k + 1).map(|next| next.level) == sub {
                    // Back on the row whose submenu is open: it stays open.
                    self.open.truncate(k + 2);
                    self.open[k + 1].highlight = None;
                    self.open[k].highlight = Some(item);
                    return;
                }
                self.open.truncate(k + 1);
                let selectable = self.selectable(item);
                self.open[k].highlight = selectable.then_some(item);
                if let Some(level) = sub.filter(|_| selectable) {
                    self.open.push(Open {
                        level,
                        highlight: None,
                    });
                }
            }
            Hit::Panel(level) => {
                if let Some(k) = depth(&self.open, level) {
                    self.open.truncate(k + 1);
                    self.open[k].highlight = None;
                }
            }
            // A submenu's parent row stays highlighted, as its submenu is open.
            Hit::Outside => {
                if let Some(last) = self.open.last_mut() {
                    last.highlight = None;
                }
            }
        }
    }

    /// Whether the pointer's move from `from` to `to` is on its way to the open submenu: inside
    /// the triangle from where it was to the submenu's two near corners, off the row that opened
    /// it and not yet in the submenu.
    fn safe(&self, from: Point<Pixels>, to: Point<Pixels>) -> bool {
        let [.., parent, sub] = self.open[..] else {
            return false;
        };
        let level = &self.layout.levels[sub.level];
        if from == to || level.bounds.contains(&to) || parent.highlight != level.parent {
            return false;
        }
        if matches!(self.hit(to), Hit::Row(_, item) if Some(item) == level.parent) {
            return false;
        }
        let beside = &self.layout.levels[parent.level].bounds;
        let x = if level.bounds.left() >= beside.center().x {
            level.bounds.left()
        } else {
            level.bounds.right()
        };
        in_triangle(
            to,
            from,
            point(x, level.bounds.top()),
            point(x, level.bounds.bottom()),
        )
    }

    /// The pointer moved to `at` at `now`. Returns whether it's resting in the safe triangle, for
    /// `rested` to be called once SUBMENU_REST has passed.
    pub(crate) fn moved(&mut self, at: Point<Pixels>, now: Instant) -> bool {
        if let Some(press) = self.press.as_mut() {
            press.get_or_insert(at);
        }
        // A move to where the pointer already is, as AppKit can deliver one move twice, isn't one:
        // it would end a rest in the triangle before its time.
        if self.pointer == Some(at) {
            return false;
        }
        let from = self.pointer.replace(at);
        if from.is_some_and(|from| self.safe(from, at)) {
            self.resting = Some(now);
            return true;
        }
        self.resting = None;
        self.point_at(at);
        false
    }

    /// The pointer has rested since its last move: in the safe triangle for SUBMENU_REST, what's
    /// under it takes the highlight. Returns how much longer it has to rest when that's not yet:
    /// a timer can fire a little early, as Windows' do by up to a clock tick (#2158).
    pub(crate) fn rested(&mut self, now: Instant) -> Option<Duration> {
        let rested = now.duration_since(self.resting?);
        let rest = Duration::from_secs_f32(SUBMENU_REST);
        if rested < rest {
            return Some(rest - rested);
        }
        self.resting = None;
        if let Some(at) = self.pointer {
            self.point_at(at);
        }
        None
    }

    /// A button went down at `at`. Returns whether it's on the menu; outside, it closes it.
    pub(crate) fn pressed(&mut self, at: Point<Pixels>) -> bool {
        self.press = None;
        if self.hit(at) == Hit::Outside {
            return false;
        }
        self.pressed = true;
        self.pointer = Some(at);
        self.resting = None;
        self.point_at(at);
        true
    }

    /// The button came up at `at`: on an item it picks it, as does a release after a drag from
    /// the press that opened the menu, which outside closes it. The release of a click that opened
    /// it picks nothing, as AppKit's menus and pop-ups leave theirs open.
    pub(crate) fn released(&mut self, at: Point<Pixels>) -> Outcome {
        let opening = self.press.take();
        let pressed = std::mem::replace(&mut self.pressed, false);
        match opening {
            // A pop-up's press the pointer never moved from.
            Some(None) => return Outcome::Stay,
            Some(Some(press)) => {
                let (dx, dy) = (f32::from(at.x - press.x), f32::from(at.y - press.y));
                if dx.hypot(dy) <= DRAG_SLOP {
                    return Outcome::Stay;
                }
            }
            None if !pressed => return Outcome::Stay,
            None => {}
        }
        match self.hit(at) {
            // A click on a pop-up button lands on the item its menu opens over, and leaves it open.
            Hit::Row(_, item) if opening.is_some() && self.entries[item].over => Outcome::Stay,
            Hit::Row(_, item) if self.selectable(item) && self.layout.level_of(item).is_none() => {
                Outcome::Pick(item)
            }
            Hit::Outside if opening.is_some() => Outcome::ReleasedOutside,
            _ => Outcome::Stay,
        }
    }

    /// The selectable items of the deepest level, and its place in `open`.
    fn deepest(&mut self) -> (usize, Vec<usize>) {
        // A submenu the pointer opened has nothing highlighted, and the keys walk its parent.
        if self.open.len() > 1
            && self
                .open
                .last()
                .is_some_and(|last| last.highlight.is_none())
        {
            self.open.pop();
        }
        let k = self.open.len() - 1;
        let items = self.layout.levels[self.open[k].level].items.clone();
        (
            k,
            items
                .into_iter()
                .filter(|&item| self.selectable(item))
                .collect(),
        )
    }

    /// Opens the highlighted row's submenu, if it has one, with its first item highlighted.
    fn open_submenu(&mut self) -> bool {
        let Some(item) = self.open.last().and_then(|last| last.highlight) else {
            return false;
        };
        let Some(level) = self.layout.level_of(item) else {
            return false;
        };
        let first = (self.layout.levels[level].items.iter())
            .copied()
            .find(|&each| self.selectable(each));
        self.open.push(Open {
            level,
            highlight: first,
        });
        true
    }

    pub(crate) fn key(&mut self, key: Key, now: Instant) -> Outcome {
        if let Key::Char(c) = key {
            self.type_select(c, now);
            return Outcome::Stay;
        }
        let (k, items) = self.deepest();
        let current = self.open[k]
            .highlight
            .and_then(|item| items.iter().position(|&each| each == item));
        let to = |at: Option<usize>| at.and_then(|at| items.get(at).copied());
        match key {
            // At the ends they stay, as NSMenu's do.
            Key::Down => {
                let next = current.map_or(0, |at| (at + 1).min(items.len().saturating_sub(1)));
                if let Some(item) = to(Some(next)) {
                    self.open[k].highlight = Some(item);
                }
            }
            Key::Up => {
                let previous =
                    current.map_or(items.len().checked_sub(1), |at| Some(at.saturating_sub(1)));
                if let Some(item) = to(previous) {
                    self.open[k].highlight = Some(item);
                }
            }
            Key::First => {
                self.open[k].highlight = items.first().copied().or(self.open[k].highlight)
            }
            Key::Last => self.open[k].highlight = items.last().copied().or(self.open[k].highlight),
            Key::Right => {
                self.open_submenu();
            }
            Key::Left => {
                if self.open.len() > 1 {
                    self.open.pop();
                }
            }
            Key::Choose => {
                if let Some(item) = self.open[k].highlight
                    && !self.open_submenu()
                {
                    return Outcome::Pick(item);
                }
            }
            Key::Escape => {
                if self.open.len() == 1 {
                    return Outcome::Escape;
                }
                self.open.pop();
            }
            Key::Char(_) => {}
        }
        self.reveal();
        Outcome::Stay
    }

    /// Scrolls each open level's run as far as keeps its highlighted row in sight.
    fn reveal(&mut self) {
        for open in &self.open {
            let level = &self.layout.levels[open.level];
            let row = (open.highlight).and_then(|item| level.items.iter().position(|&i| i == item));
            if let Some(scroll) = row.and_then(|row| level.revealing(row, &self.entries)) {
                self.layout.levels[open.level].scroll = Some(scroll);
            }
        }
    }

    /// Whether `at` is over a run of rows that scrolls, in an open level.
    pub(crate) fn over_scroll(&self, at: Point<Pixels>) -> bool {
        (self.open.iter()).any(|open| {
            let level = &self.layout.levels[open.level];
            level.scroll.is_some_and(|scroll| {
                let y = f32::from(at.y - level.bounds.origin.y);
                level.bounds.contains(&at) && (scroll.top..scroll.top + scroll.shown).contains(&y)
            })
        })
    }

    /// The run of rows under `at` scrolled `down` points further, and what's under the pointer
    /// highlighted, as the rows moved under it.
    pub(crate) fn scrolled(&mut self, at: Point<Pixels>, down: f32) {
        for open in self.open.clone() {
            let level = &mut self.layout.levels[open.level];
            if let Some(scroll) = level.scroll.filter(|_| level.bounds.contains(&at)) {
                level.scroll = Some(scroll.to(scroll.by + down));
            }
        }
        self.point_at(at);
    }

    /// Type-to-select, as a list's type-ahead finds a row (list_keys.rs): in the deepest level,
    /// the first item whose title starts with what's been typed in the last second, ignoring case
    /// and accents, or with the same letter again, the next one that starts with it.
    fn type_select(&mut self, c: char, now: Instant) {
        let (k, items) = self.deepest();
        let at =
            (self.open[k].highlight).and_then(|item| items.iter().position(|&each| each == item));
        let titles: Vec<&str> = (items.iter())
            .map(|&item| self.entries[item].title.as_ref())
            .collect();
        if let Some(found) = self.typed.find(c, now, at, &titles) {
            self.open[k].highlight = Some(items[found]);
        }
    }

    /// The enabled item titled `title`, or with "Submenu › Item" the one in that submenu, as the
    /// driver chooses one.
    pub(crate) fn find(&self, title: &str) -> Option<usize> {
        let (parent, title) = match title.split_once(" › ") {
            Some((parent, title)) => (Some(parent), title),
            None => (None, title),
        };
        (0..self.entries.len()).find(|&at| {
            let entry = &self.entries[at];
            entry.enabled
                && entry.title == title
                && self.layout.level_of(at).is_none()
                && parent.is_none_or(|parent| {
                    entry
                        .parent
                        .is_some_and(|up| self.entries[up].title == parent)
                })
        })
    }

    /// Shows `entries`, a search's narrowing of the menu, laid out as `layout`, with the first row
    /// that matched what was typed highlighted, so ↩ picks it, and none where nothing was typed or
    /// nothing matched: ↩ never signs in or opens Settings for a search that found nothing. The
    /// pointer and the press stay as they were.
    fn narrow(&mut self, entries: Vec<MenuEntry>, layout: Layout, typed: bool) {
        self.entries = entries;
        self.layout = layout;
        let first = (self.layout.levels[0].items.iter().copied())
            .find(|&item| self.selectable(item) && !self.entries[item].kept);
        self.open = vec![Open {
            level: 0,
            highlight: first.filter(|_| typed),
        }];
        self.resting = None;
    }
}

/// The ink of `entry`'s title, `lit` or not, on `on_fill` where it's lit: a heading's and an
/// offer's the secondary ink, as the model menu's not-signed-in Sign In.
pub(crate) fn title_ink(entry: &MenuEntry, lit: bool, look: Look, on_fill: Rgba) -> Hsla {
    let palette = look.palette();
    match (lit, entry.enabled, entry.destructive) {
        _ if entry.header => look.secondary().into(),
        (true, ..) => on_fill.into(),
        (false, false, _) => look.disabled().into(),
        (false, true, true) => rgba(palette.destructive).into(),
        (false, true, false) if entry.offer => look.secondary().into(),
        (false, true, false) => rgba(palette.label).into(),
    }
}

/// The items of `entries` a search for `typed` leaves, by index: the field; each row whose title
/// holds what's typed, ignoring case, and each a search keeps (`MenuEntry::kept`); a heading while
/// a row under it stays; the lines between what's left; and what's in a kept row's submenu or
/// takes its place while ⌥ is held. An offer (`MenuEntry::offer`) goes once anything is typed, so a
/// heading over offers alone goes with them. Nothing typed leaves them all. A search that matches
/// nothing leaves only the field and the kept rows, with no words of its own.
pub(crate) fn narrowed(entries: &[MenuEntry], typed: &str) -> Vec<usize> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return (0..entries.len()).collect();
    }
    let root: Vec<usize> = (0..entries.len())
        .filter(|&at| entries[at].parent.is_none() && !entries[at].alternate)
        .collect();
    let row = |entry: &MenuEntry| !entry.header && !entry.title.is_empty() && !entry.field;
    let matches = |entry: &MenuEntry| {
        entry.field
            || (row(entry)
                && !entry.offer
                && (entry.kept || entry.title.to_lowercase().contains(&typed)))
    };
    let mut left: Vec<usize> = Vec::new();
    for (k, &at) in root.iter().enumerate() {
        let entry = &entries[at];
        let stays = if entry.header {
            // A heading heads the rows after it, up to the next heading or line.
            (root[k + 1..].iter())
                .map(|&next| &entries[next])
                .take_while(|next| !next.header && !next.title.is_empty())
                .any(|next| matches(next))
        } else {
            entry.title.is_empty() || matches(entry)
        };
        if stays {
            left.push(at);
        }
    }
    // No line first, last, after the field or after another.
    let mut root_left: Vec<usize> = Vec::new();
    for at in left {
        let line = entries[at].title.is_empty();
        let after_line_or_field = root_left
            .last()
            .is_none_or(|&last| entries[last].title.is_empty() || entries[last].field);
        if line && after_line_or_field {
            continue;
        }
        root_left.push(at);
    }
    if root_left
        .last()
        .is_some_and(|&last| entries[last].title.is_empty())
    {
        root_left.pop();
    }
    // A submenu's items and an item's alternate go with it, their parent coming first.
    let mut kept = vec![false; entries.len()];
    for &at in &root_left {
        kept[at] = true;
    }
    for at in 0..entries.len() {
        let entry = &entries[at];
        if entry.alternate && at > 0 && entry.parent.is_none() {
            kept[at] = kept[at - 1];
        } else if let Some(parent) = entry.parent {
            kept[at] = kept[parent];
        }
    }
    (0..entries.len()).filter(|&at| kept[at]).collect()
}

/// `entries` at the indices `left`, each submenu's items pointing at their parent's new place.
fn kept_entries(entries: &[MenuEntry], left: &[usize]) -> Vec<MenuEntry> {
    (left.iter())
        .map(|&at| MenuEntry {
            parent: (entries[at].parent)
                .and_then(|parent| left.iter().position(|&each| each == parent)),
            ..entries[at].clone()
        })
        .collect()
}

/// The menu's items as the host's menus log them, for check.sh: lines left out, a submenu's items
/// after their parent's title in place of it, and the dimmed and checked ones marked.
pub(crate) fn described(entries: &[MenuEntry]) -> String {
    described_held(entries, false)
}

/// The menu's items as `described` has them, with each item's ⌥ alternate in its place if `alt`.
pub(crate) fn described_held(entries: &[MenuEntry], alt: bool) -> String {
    let mark = |entry: &MenuEntry, title: String| {
        let title = match &entry.detail {
            Some(detail) => format!("{title} [{detail}]"),
            None => title,
        };
        let title = match (entry.page, entry.favicon.is_some()) {
            (false, _) => title,
            (true, true) => format!("{title} (favicon)"),
            (true, false) => format!("{title} (globe)"),
        };
        match (entry.enabled, entry.checked) {
            _ if entry.header => format!("{title} (header)"),
            (false, _) => format!("{title} (disabled)"),
            (true, true) => format!("{title} (checked)"),
            (true, false) => title,
        }
    };
    // A submenu's items after its title, however deep, as Move to › a Space › its folder.
    fn walk(
        entries: &[MenuEntry],
        parent: Option<usize>,
        path: &str,
        alt: bool,
        mark: &dyn Fn(&MenuEntry, String) -> String,
        titles: &mut Vec<String>,
    ) {
        for (at, entry) in entries.iter().enumerate() {
            if entry.parent != parent || entry.title.is_empty() || entry.alternate || entry.field {
                continue;
            }
            let title = format!("{path}{}", entry.title);
            if entries
                .iter()
                .any(|each| each.parent == Some(at) && !each.title.is_empty())
            {
                walk(entries, Some(at), &format!("{title} › "), alt, mark, titles);
            } else {
                let held = &entries[as_held(entries, at, alt)];
                titles.push(mark(held, format!("{path}{}", held.title)));
            }
        }
    }
    let mut titles = Vec::new();
    walk(entries, None, "", alt, &mark, &mut titles);
    titles.join(", ")
}

/// The key `event` is to the menu: arrows, Home and End (⌥↑ and ⌥↓ too), return, space, escape, or
/// a character for type-to-select. A menu has no text to compose, so a character an input method
/// gives a key, such as kana, selects as itself.
pub(crate) fn key_of(event: &KeyDownEvent) -> Option<Key> {
    let keystroke = &event.keystroke;
    let alt = keystroke.modifiers.alt;
    Some(match keystroke.key.as_str() {
        "up" if alt => Key::First,
        "down" if alt => Key::Last,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" | "pageup" => Key::First,
        "end" | "pagedown" => Key::Last,
        "right" => Key::Right,
        "left" => Key::Left,
        "enter" | "space" => Key::Choose,
        "escape" => Key::Escape,
        key => {
            let typed = keystroke.key_char.as_deref().unwrap_or(key);
            let mut chars = typed.chars();
            let c = chars
                .next()
                .filter(|c| !c.is_control() && chars.next().is_none())?;
            Key::Char(c)
        }
    })
}

/// The menu while it shows.
struct Shown {
    tracking: Tracking,
    /// Where it opened, in the browser window's content.
    at: Point<Pixels>,
    look: Look,
    /// Where its window's top left is in the browser window's content.
    origin: Point<Pixels>,
    chosen: Option<Chosen>,
    /// The highlighted item the log last named, and the items whose submenus it said were open.
    logged: Option<usize>,
    logged_open: Vec<usize>,
    /// Whether ⌥ is held, which shows each item's alternate in its place.
    alt: bool,
    /// Whether it has drawn on screen, and with ⌥ held, which the log says as it changes.
    drawn: Option<bool>,
    /// The items whose submenus it has drawn open since they last opened, which the log says.
    drawn_open: Vec<usize>,
    /// Whether its window blends with what's under it (`Host::blends`).
    blends: bool,
    /// Its search, where its first row is a field (`MenuEntry::field`).
    search: Option<Search>,
    /// Whether a drag in the browser window opened it and drives it (`lead_by_drag`): the
    /// pointer's moves and release are the drag's, which the browser window hears.
    led: bool,
}

/// Set for the next menu shown, which a drag in the browser window drives (`lead_by_drag`).
struct LedByDrag;

impl Global for LedByDrag {}

/// Whether the next menu shown was to be led by a drag, which only it is.
fn take_led(cx: &mut App) -> bool {
    let led = cx.has_global::<LedByDrag>();
    if led {
        cx.remove_global::<LedByDrag>();
    }
    led
}

/// A menu's search: what's typed in its field narrows its rows (`narrowed`). The text lives in the
/// field alone, in the menu's memory, and goes with the menu: never to the log, kept, or to anyone.
struct Search {
    field: Entity<TextInput>,
    /// Every item the menu was asked for.
    all: Vec<MenuEntry>,
    /// Each item shown's index in `all`.
    left: Vec<usize>,
    /// What the rows were last narrowed for.
    typed: String,
    /// The root's panel as it opened, whose top left and width a narrowing keeps, so the field
    /// stays where it is; and where the menu may go.
    root: Bounds<Pixels>,
    area: Bounds<Pixels>,
    /// Whether the root opened as a level of choices, which it stays.
    choices: bool,
    _typing: Subscription,
}

impl Shown {
    /// The panels the host's window holds, from its top left: every level's where it blends,
    /// else only those open, which it shows alone.
    fn panels(&self) -> Vec<Bounds<Pixels>> {
        let layout = &self.tracking.layout;
        let at = |level: usize| {
            let bounds = layout.levels[level].bounds;
            Bounds::new(bounds.origin - layout.frame.origin, bounds.size)
        };
        match self.blends {
            true => (0..layout.levels.len()).map(at).collect(),
            false => self
                .tracking
                .open
                .iter()
                .map(|open| at(open.level))
                .collect(),
        }
    }

    /// The open panels, from the host's window's top left, each with the radius it's drawn with:
    /// square where the window can't blend.
    fn glass(&self) -> Vec<(Bounds<Pixels>, f32)> {
        let layout = &self.tracking.layout;
        (self.tracking.open.iter())
            .map(|open| {
                let level = &layout.levels[open.level];
                let at = Bounds::new(level.bounds.origin - layout.frame.origin, level.bounds.size);
                (at, if self.blends { level.radii().0 } else { 0. })
            })
            .collect()
    }
}

/// The menu's window's view.
pub struct ContextMenu {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    shown: Option<Shown>,
    /// Where the window stays, when the host can't move it, as the preview's; else it goes where
    /// each menu does.
    fixed: Option<Point<Pixels>>,
    _rest: Option<Task<()>>,
    a11y: bool,
}

struct MenuWindow(WindowHandle<ContextMenu>);

impl gpui::Global for MenuWindow {}

fn options(bounds: Bounds<Pixels>, show: bool) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // The preview's has none, so it hasn't the window buttons the host's takes off.
        titlebar: (!show).then_some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // A panel hides while its app isn't the active one, which a preview a script started
        // isn't, so the preview's is a plain window over its own.
        kind: if show {
            WindowKind::Normal
        } else {
            WindowKind::Floating
        },
        show,
        focus: show,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The panels are drawn with their shadows, over nothing.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}

fn open_window(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    fixed: Option<Point<Pixels>>,
    cx: &mut App,
) -> Option<WindowHandle<ContextMenu>> {
    let window = cx.open_window(options(bounds, fixed.is_some()), |_, cx| {
        cx.new(|cx| ContextMenu {
            host,
            focus: cx.focus_handle(),
            shown: None,
            fixed,
            _rest: None,
            a11y: false,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(MenuWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the menu's window: {error}");
            None
        }
    }
}

/// Opens the menu's window, hidden, for `host` to show over a browser window, ahead of time as the
/// command bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<ContextMenu>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(MIN_WIDTH), px(MIN_WIDTH)));
    open_window(host, bounds, None, cx)
}

/// Opens the menu's window on screen at `bounds`, over a browser window there whose content is
/// the same, where it stays: for the preview, whose host can't move it.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    cx: &mut App,
) -> Option<WindowHandle<ContextMenu>> {
    open_window(host, bounds, Some(point(px(0.), px(0.))), cx)
}

fn update(
    cx: &mut App,
    change: impl FnOnce(&mut ContextMenu, &mut Window, &mut Context<ContextMenu>),
) {
    if let Some(window) = cx.try_global::<MenuWindow>().map(|menu| menu.0) {
        window.update(cx, change).ok();
    }
}

/// How wide a menu's text is in `window`'s text system.
fn measure(window: &Window) -> impl Fn(&str) -> f32 + use<> {
    let system = window.text_system().clone();
    move |text: &str| {
        let text = SharedString::from(text.to_string());
        crate::tooltip::text_width(&text, TEXT, gpui::FontWeight::NORMAL, &system)
    }
}

/// Shows the menu `request` asks for, closing one that shows.
pub fn show(request: Request, cx: &mut App) {
    crate::lazy_window::ready::<ContextMenu>(cx);
    let mut request = Some(request);
    // The window it opens from saw the press or key that opened it, with what was held then.
    let alt = (cx.active_window())
        .and_then(|window| {
            window
                .update(cx, |_, window, _| window.modifiers().alt)
                .ok()
        })
        .unwrap_or(false);
    update(cx, |menu, window, cx| {
        let Some(request) = request.take() else {
            return;
        };
        menu.close(Closing::Replaced, cx);
        let width_of = measure(window);
        let layout = lay_out_over(&request.items, request.at, request.area, &width_of);
        let blends = menu.host.blends();
        let layout = if blends { layout } else { layout.opaque() };
        // check.sh waits for this line before it chooses.
        log!("browser: menu shown {}", described(&request.items));
        let frame = layout.frame;
        let origin = menu.fixed.unwrap_or(frame.origin);
        let search = (request.items.iter().any(|item| item.field)).then(|| {
            let palette = request.look.palette();
            let colors = text_input::Colors {
                caret: Some(rgba(palette.label).into()),
                ..text_input::Colors::placeholder(rgba(palette.secondary))
            };
            let placeholder = (request.items.iter().find(|item| item.field))
                .map(|item| item.title.clone())
                .unwrap_or_default();
            let field = cx.new(|cx| TextInput::new(placeholder, cx).follows(move |_| colors));
            Search {
                _typing: cx.observe_in(&field, window, |menu, _, window, cx| {
                    menu.narrow(window, cx)
                }),
                field,
                all: request.items.clone(),
                left: (0..request.items.len()).collect(),
                typed: String::new(),
                root: layout.levels[0].bounds,
                area: request.area,
                choices: layout.levels[0].checks,
            }
        });
        let shown = Shown {
            tracking: Tracking::new(request.items, layout, request.at),
            at: request.at,
            look: request.look,
            origin,
            chosen: Some(request.chosen),
            logged: None,
            logged_open: Vec::new(),
            alt,
            drawn: None,
            drawn_open: Vec::new(),
            blends,
            search,
            led: take_led(cx),
        };
        let (panels, glass, light) = (shown.panels(), shown.glass(), shown.look.light);
        // A search's field has the keyboard, so the cursor is in it as the menu opens.
        let focus = (shown.search.as_ref()).map_or_else(
            || menu.focus.clone(),
            |search| search.field.focus_handle(cx),
        );
        menu.shown = Some(shown);
        window.focus(&focus, cx);
        // After a hide that `close` asked for, so the new menu stays.
        let host = menu.host.clone();
        cx.defer(move |_| {
            host.show(frame, &panels);
            host.glass(&glass, light);
        });
        cx.notify();
    });
}

/// Makes the next menu shown one a drag in the browser window drives, as a tab held over the
/// Space button opens its menu: the drag's moves highlight its rows (`dragged_over`) and the
/// browser window's drop asks what's highlighted (`drag_target`), as the menu doesn't take the
/// pointer from the drag. One that never shows is let go with the drag (`drag_ended`).
pub fn lead_by_drag(cx: &mut App) {
    cx.set_global(LedByDrag);
}

/// The drag leading the menu is at `at`, in the browser window's content: what's under it takes
/// the highlight.
pub fn dragged_over(at: Point<Pixels>, cx: &mut App) {
    update(cx, |menu, _, cx| {
        if let Some(shown) = menu.shown.as_mut().filter(|shown| shown.led) {
            shown.tracking.point_at(at);
            menu.changed(cx);
            cx.notify();
        }
    });
}

/// The item highlighted in the menu the drag leads, if one is.
pub fn drag_target(cx: &mut App) -> Option<usize> {
    let mut target = None;
    update(cx, |menu, _, _| {
        let shown = menu.shown.as_ref().filter(|shown| shown.led);
        target = shown.and_then(|shown| shown.tracking.open[0].highlight);
    });
    target
}

/// The drag leading the menu ended: the menu closes, choosing nothing, as the browser window
/// does what the drop asked itself.
pub fn drag_ended(cx: &mut App) {
    take_led(cx);
    update(cx, |menu, _, cx| {
        if menu.shown.as_ref().is_some_and(|shown| shown.led) {
            menu.close(Closing::Left, cx);
        }
    });
}

/// Highlights the first item of the menu that shows, as a menu opened from the keyboard (⇧F10)
/// does, so VoiceOver reads it with the menu.
pub fn highlight_first(cx: &mut App) {
    update(cx, |menu, _, cx| {
        let Some(shown) = menu.shown.as_mut() else {
            return;
        };
        if shown.tracking.open[0].highlight.is_none() {
            let outcome = shown.tracking.key(Key::First, Instant::now());
            menu.outcome(outcome, cx);
            menu.changed(cx);
        }
    });
}

/// What a pointer event the host saw anywhere in the app is, for `pointer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Moved,
    /// The left button, or another but the right.
    Pressed,
    RightPressed,
    Released,
    Scrolled,
}

/// The host saw `event` at `at` in the menu's window, wherever it happened, while the menu shows.
/// Returns whether the menu takes it, so the window under it never hears it.
pub fn pointer(event: Pointer, at: Point<Pixels>, cx: &mut App) -> bool {
    let mut taken = false;
    update(cx, |menu, _, cx| {
        taken = menu.pointer(event, at, cx);
        menu.changed(cx);
    });
    taken
}

/// The user left the menu without choosing: another window or app, or the browser window
/// resizing, moving or going full screen.
pub fn dismissed(cx: &mut App) {
    update(cx, |menu, _, cx| menu.close(Closing::Left, cx));
}

/// Chooses the enabled item titled `title`, as the driver does. Returns whether there was one.
pub fn choose(title: &str, cx: &mut App) -> bool {
    let mut chosen = false;
    update(cx, |menu, _, cx| {
        let found = menu
            .shown
            .as_ref()
            .and_then(|shown| shown.tracking.find(title));
        if let Some(item) = found {
            chosen = true;
            menu.pick(item, cx);
        }
    });
    chosen
}

/// Shows a menu over the app's window `window` for `field_menu`: the platform's part, which puts
/// ilo's menu over that window, filling in the request's `area`.
pub type ShowOver = dyn Fn(AnyWindowHandle, Request, &mut App);

/// Text fields' menus (`FieldMenu`) in ilo's menu, over the field's window, in its Space's look in a
/// browser window and dark elsewhere, once the update that asked is over. The field hears in its
/// window how the menu closed, which has the keyboard back by then unless the user left.
pub fn field_menu(show: Rc<ShowOver>) -> FieldMenu {
    let entries = entries_menu(show).0;
    FieldMenu(Rc::new(move |items, at, from_keys, window, chosen, cx| {
        let items = (items.into_iter())
            .map(|(title, enabled)| MenuEntry::named(title, enabled))
            .collect();
        entries(items, at, from_keys, window, chosen, cx)
    }))
}

/// Shows a menu of `MenuEntry`s, with their checks, headers and details, at `position` in `window`,
/// as `FieldMenu` shows a field's: for a view that isn't the browser window's own, such as the
/// command bar's composer, whose choices hang from it.
pub struct EntriesMenu(pub Rc<ShowEntries>);

pub type ShowEntries =
    dyn Fn(Vec<MenuEntry>, Point<Pixels>, bool, &Window, text_input::Chosen, &mut App);

impl Global for EntriesMenu {}

/// `EntriesMenu` in ilo's menu, as `field_menu` shows a field's.
pub fn entries_menu(show: Rc<ShowOver>) -> EntriesMenu {
    EntriesMenu(Rc::new(move |items, at, from_keys, window, chosen, cx| {
        let handle = window.window_handle();
        let show = show.clone();
        cx.defer(move |cx| {
            // Looked up once the update is over, in case the window closed meanwhile.
            let Ok(look) = cx.update_window(handle, |root, _, cx| {
                root.downcast::<Shell>()
                    .ok()
                    .map_or_else(Look::dark, |shell| shell.read(cx).menu_look())
            }) else {
                return;
            };
            let chosen = Box::new(move |closed: Closed, cx: &mut App| {
                cx.update_window(handle, |_, window, cx| chosen(closed, window, cx))
                    .ok();
            });
            show(
                handle,
                Request {
                    items,
                    at,
                    area: Bounds::default(),
                    look,
                    chosen,
                },
                cx,
            );
            if from_keys {
                highlight_first(cx);
            }
        });
    }))
}

impl ContextMenu {
    /// Closes the menu, if it shows, and says how once the update is over.
    fn close(&mut self, closing: Closing, cx: &mut Context<Self>) {
        let Some(mut shown) = self.shown.take() else {
            return;
        };
        let closed = match closing.closed() {
            Closed::Chose(item) => Closed::Chose(as_held(&shown.tracking.entries, item, shown.alt)),
            closed => closed,
        };
        self._rest = None;
        let host = self.host.clone();
        cx.defer(move |_| host.hide());
        // check.sh reads this line.
        match closed {
            Closed::Chose(item) => {
                log!("browser: menu chose {}", shown.tracking.entries[item].title)
            }
            _ => log!("browser: menu closed"),
        }
        log!("{}", closing.line(shown.at));
        // A search's rows are some of those asked for: the answer names the one asked for.
        let closed = match (closed, &shown.search) {
            (Closed::Chose(item), Some(search)) => Closed::Chose(search.left[item]),
            (closed, _) => closed,
        };
        if let Some(answer) = shown.chosen.take() {
            cx.defer(move |cx| answer(closed, cx));
        }
        cx.notify();
    }

    /// Chooses `item`, which closes the menu with it chosen. A row that stays (`MenuEntry::stays`)
    /// closes nothing: it does what it does once the update is over, as an answer is given, and
    /// the menu is left as it was, the row highlighted.
    fn pick(&mut self, item: usize, cx: &mut Context<Self>) {
        let stays = self.shown.as_ref().and_then(|shown| {
            let held = as_held(&shown.tracking.entries, item, shown.alt);
            Some((held, shown.tracking.entries[held].stays.clone()?))
        });
        let Some((held, stays)) = stays else {
            return self.close(Closing::Chose(item), cx);
        };
        // For check.sh to wait on. It names no item, as the closing's line doesn't (#2873).
        log!("{STAYED}");
        let menu = cx.weak_entity();
        cx.defer(move |cx| {
            let checked = stays(cx);
            menu.update(cx, |menu, cx| menu.stayed(held, &stays, checked, cx))
                .ok();
        });
        cx.notify();
    }

    /// The row at `item` did what `stays` does and shows its check if `checked`, where the menu
    /// still shows that row: what it did may have closed the menu or put another in its place.
    fn stayed(&mut self, item: usize, stays: &Stays, checked: bool, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let same = |entry: &&mut MenuEntry| {
            (entry.stays.as_ref()).is_some_and(|each| Rc::ptr_eq(each, stays))
        };
        let Some(entry) = shown.tracking.entries.get_mut(item).filter(same) else {
            return;
        };
        entry.checked = checked;
        // A search's rows are some of those asked for, which the next narrowing lays out again.
        let asked =
            (shown.search.as_mut()).and_then(|search| search.all.get_mut(*search.left.get(item)?));
        if let Some(asked) = asked {
            asked.checked = checked;
        }
        cx.notify();
    }

    fn outcome(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        match outcome {
            Outcome::Stay => cx.notify(),
            Outcome::Pick(item) => self.pick(item, cx),
            Outcome::Escape => self.close(Closing::Escape, cx),
            Outcome::ReleasedOutside => self.close(Closing::ReleasedOutside, cx),
        }
    }

    /// `at` in the window, in the browser window's content.
    fn in_content(&self, at: Point<Pixels>) -> Point<Pixels> {
        self.shown.as_ref().map_or(at, |shown| shown.origin + at)
    }

    /// A scroll over the menu's window, which only a run of rows that scrolls takes.
    fn wheel(&mut self, event: &ScrollEvent, window: &Window, cx: &mut Context<Self>) {
        let at = self.in_content(event.position);
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let down = -f32::from(event.delta.pixel_delta(window.line_height()).y);
        shown.tracking.scrolled(at, down);
        self.changed(cx);
        cx.notify();
    }

    fn pointer(&mut self, event: Pointer, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let at = self.in_content(at);
        let Some(shown) = self.shown.as_mut() else {
            return false;
        };
        // A drag in the browser window leads it, and has the pointer's moves and its release.
        if shown.led && event != Pointer::Scrolled {
            return false;
        }
        let tracking = &mut shown.tracking;
        match event {
            Pointer::Moved => {
                if tracking.moved(at, Instant::now()) {
                    self.wait_for_rest(cx);
                }
                cx.notify();
                true
            }
            Pointer::Pressed | Pointer::RightPressed => {
                if tracking.pressed(at) {
                    cx.notify();
                    return true;
                }
                // While a menu is open, a press outside it only closes it: the click goes no
                // further. The one exception is a right press, which goes on to open the menu of
                // what it's on, in this one's place. One in the window's room is taken where the
                // host carries it there itself, after the hide this close asks for.
                let room = Bounds::new(shown.origin, tracking.layout.frame.size);
                self.close(Closing::PressedOutside, cx);
                event == Pointer::Pressed
                    || (room.contains(&at) && self.host.clone().press_under(at, cx))
            }
            Pointer::Released => {
                let outcome = tracking.released(at);
                self.outcome(outcome, cx);
                true
            }
            // A scroll is the menu's wherever the pointer is: it stays open, and nothing under
            // the pointer outside it scrolls. Over rows that scroll it goes on to the menu's own
            // window, whose run of rows takes it.
            Pointer::Scrolled => !tracking.over_scroll(at),
        }
    }

    /// Once the pointer has rested in the safe triangle long enough, what's under it takes the
    /// highlight.
    fn wait_for_rest(&mut self, cx: &mut Context<Self>) {
        self._rest = Some(cx.spawn(async move |menu, cx| {
            let mut wait = Duration::from_secs_f32(SUBMENU_REST);
            loop {
                cx.background_executor().timer(wait).await;
                let more = menu.update(cx, |menu, cx| {
                    let more = menu.shown.as_mut()?.tracking.rested(Instant::now());
                    if more.is_none() {
                        menu.changed(cx);
                        cx.notify();
                    }
                    more
                });
                match more {
                    Ok(Some(more)) => wait = more,
                    _ => break,
                }
            }
        }));
    }

    /// Narrows the rows to what's typed in the search's field, once an input method has committed
    /// it and only if it changed: no more work per key than that, on the list the menu holds.
    fn narrow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let Some(search) = shown.search.as_mut() else {
            return;
        };
        let typed = search.field.read(cx).committed();
        if typed == search.typed {
            return;
        }
        search.left = narrowed(&search.all, &typed);
        let entries = kept_entries(&search.all, &search.left);
        let width = f32::from(search.root.size.width);
        let layout = lay_out_wide(
            &entries,
            search.root.origin,
            search.area,
            &measure(window),
            width,
            search.choices,
        );
        let layout = if shown.blends {
            layout
        } else {
            layout.opaque()
        };
        let frame = layout.frame;
        shown
            .tracking
            .narrow(entries, layout, !typed.trim().is_empty());
        search.typed = typed;
        shown.origin = self.fixed.unwrap_or(frame.origin);
        let (host, panels) = (self.host.clone(), shown.panels());
        let (glass, light) = (shown.glass(), shown.look.light);
        cx.defer(move |_| {
            host.show(frame, &panels);
            host.glass(&glass, light);
        });
        self.changed(cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        // While an input method composes in a search's field, every key is the composition's: ↩
        // that ends it picks nothing, and esc closes nothing.
        let search = (shown.search.as_ref()).map(|search| search.field.read(cx).composing());
        if search == Some(true) {
            // Nor do they go on into the field as text, which would end the composition.
            if matches!(event.keystroke.key.as_str(), "enter" | "escape") {
                cx.stop_propagation();
            }
            return;
        }
        let modifiers = event.keystroke.modifiers;
        // A shortcut closes the menu, as the menu shows them but doesn't run them. A search's
        // field takes its own, such as ⌘⌫, before they come here.
        if modifiers.platform || modifiers.control {
            return self.close(Closing::Shortcut, cx);
        }
        if let Some(key) = key_of(event) {
            // In a search, characters and space are what's typed, not type-to-select or a pick.
            let typed = matches!(key, Key::Char(_)) || event.keystroke.key == "space";
            if search.is_some() && typed {
                return;
            }
            if search.is_some() {
                cx.stop_propagation();
            }
            let outcome = shown.tracking.key(key, Instant::now());
            self.outcome(outcome, cx);
            self.changed(cx);
        }
    }

    /// Follows what the keys and the pointer did. Logs the highlight as it moves, with its row in
    /// the browser window's content, for check.sh to follow them: the deepest level's, else the
    /// row whose submenu shows. And each submenu as it opens, with its panel, and as it closes,
    /// when a window that can't blend is shown again with the panels open now.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let tracking = &shown.tracking;
        let open = tracking.open_submenus();
        if open != shown.logged_open {
            let (host, frame, panels) = (self.host.clone(), tracking.layout.frame, shown.panels());
            let (blends, glass, light) = (shown.blends, shown.glass(), shown.look.light);
            cx.defer(move |_| {
                if !blends {
                    host.show(frame, &panels);
                }
                host.glass(&glass, light);
            });
        }
        let kept = open
            .iter()
            .zip(&shown.logged_open)
            .take_while(|(a, b)| a == b)
            .count();
        for &closed in shown.logged_open[kept..].iter().rev() {
            log!(
                "browser: menu submenu {} hidden",
                tracking.entries[closed].title
            );
        }
        for &opened in &open[kept..] {
            let Some(level) = tracking.layout.level_of(opened) else {
                continue;
            };
            let at = tracking.layout.levels[level].bounds;
            log!(
                "browser: menu submenu {} shown at {:.0} {:.0} {:.0} {:.0}",
                tracking.entries[opened].title,
                f32::from(at.origin.x),
                f32::from(at.origin.y),
                f32::from(at.size.width),
                f32::from(at.size.height)
            );
        }
        shown.logged_open = open;
        let lit = tracking
            .open
            .iter()
            .rev()
            .find_map(|open| open.highlight.map(|item| (open.level, item)));
        if lit.map(|(_, item)| item) == shown.logged {
            return;
        }
        shown.logged = lit.map(|(_, item)| item);
        let Some((level, item)) = lit else {
            return log!("browser: menu highlights nothing");
        };
        let entry = &tracking.entries[item];
        let title = match entry.parent {
            Some(parent) => format!("{} › {}", tracking.entries[parent].title, entry.title),
            None => entry.title.to_string(),
        };
        let lay = &tracking.layout.levels[level];
        let row = lay
            .items
            .iter()
            .position(|&each| each == item)
            .unwrap_or_default();
        let at = lay.row_bounds(row, &tracking.entries);
        log!(
            "browser: menu highlights {title} at {:.0} {:.0} {:.0} {:.0}",
            f32::from(at.origin.x),
            f32::from(at.origin.y),
            f32::from(at.size.width),
            f32::from(at.size.height)
        );
    }

    fn row(
        &self,
        shown: &Shown,
        level: usize,
        row: usize,
        lit: Option<usize>,
        open: bool,
    ) -> gpui::AnyElement {
        let tracking = &shown.tracking;
        let palette = shown.look.palette();
        let lay = &tracking.layout.levels[level];
        let item = lay.items[row];
        let entry = &tracking.entries[as_held(&tracking.entries, item, shown.alt)];
        let bounds = lay.row_bounds(row, &tracking.entries);
        // A row of the run that scrolls stands in the run's own box, which hides what's out of it.
        let run = (lay.scroll.filter(|scroll| scroll.holds(row))).map_or(0., |scroll| scroll.top);
        let top = bounds.origin.y - lay.bounds.origin.y - px(run);
        let (_, row_radius) = lay.radii();
        let placed = div()
            .absolute()
            .left(px(PADDING))
            .top(top)
            .w(bounds.size.width)
            .h(bounds.size.height);
        if let Some(search) = shown.search.as_ref().filter(|_| entry.field) {
            let line = div().mx(px(ROW_X)).h(px(1.)).bg(rgba(palette.line));
            // The glyph stands in the checks' column and the text starts where the titles do.
            let glyph = svg()
                .path("icons/search.svg")
                .size(px(SEARCH_GLYPH))
                .flex_none()
                .text_color(rgba(palette.secondary));
            let column = div()
                .debug_selector(|| "menu-search-glyph".into())
                .w(px(CHECK))
                .flex_none()
                .flex()
                .justify_center()
                .child(glyph);
            return (placed.id(("menu-field", item)).flex().flex_col())
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap(px(GAP))
                        .pl(px(ROW_X))
                        .pr(px(ROW_X))
                        .text_color(rgba(palette.label))
                        .child(column)
                        .child(search.field.clone()),
                )
                .child(line)
                .into_any_element();
        }
        if entry.title.is_empty() {
            let line = div().mx(px(ROW_X)).h(px(1.)).bg(rgba(palette.line));
            return (placed.id(("menu-line", item)).flex().items_center())
                .when(self.a11y, |row| row.role(Role::Splitter))
                .child(div().flex_1().child(line))
                .into_any_element();
        }
        let lit = lit == Some(item);
        let highlight = shown.look.plate();
        let (fill, on_fill): (Rgba, Rgba) = match entry.destructive {
            true => (rgba(DESTRUCTIVE_FILL), rgba(DESTRUCTIVE_LABEL)),
            false => (highlight.fill, highlight.label),
        };
        let label = title_ink(entry, lit, shown.look, on_fill);
        let secondary: Hsla = match (lit, entry.enabled) {
            (true, _) => on_fill.into(),
            (false, false) => shown.look.disabled().into(),
            (false, true) => shown.look.secondary().into(),
        };
        let submenu = tracking.layout.level_of(item).is_some();
        let shortcut = entry.shortcut.as_deref().map(keystroke_label);
        let pressed = lit && tracking.pressed;
        let check = lay.checks.then(|| {
            (div()
                .debug_selector(move || format!("menu-check {item}"))
                .w(px(CHECK))
                .flex_none())
            .when(entry.checked, |slot| {
                slot.child(
                    svg()
                        .path("icons/check.svg")
                        .size(px(CHECK))
                        .text_color(label),
                )
            })
        });
        let swatch = entry
            .swatch
            .map(|swatch| swatch.dot(rgb(palette.fill >> 8)));
        let favicon = entry.page.then(|| match entry.favicon.clone() {
            Some(favicon) => img(favicon)
                .size(px(FAVICON))
                .flex_none()
                .into_any_element(),
            None => svg()
                .path("icons/globe.svg")
                .size(px(FAVICON))
                .flex_none()
                .text_color(secondary)
                .into_any_element(),
        });
        // A Space with no glyph of its own shows its first letter in the glyph's box.
        let icon = match (entry.icon, &entry.mark) {
            (Some(icon), _) => Some(
                svg()
                    .path(icon)
                    .size(px(ICON))
                    .flex_none()
                    .text_color(secondary)
                    .into_any_element(),
            ),
            (None, Some(mark)) => Some(
                div()
                    .size(px(ICON))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(ICON * crate::tokens::shell::SPACE_LETTER))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(secondary)
                    .child(mark.clone())
                    .into_any_element(),
            ),
            (None, None) => None,
        };
        let trailing = match (&shortcut, submenu) {
            (_, true) => Some(
                svg()
                    .path("icons/chevron-down.svg")
                    .size(px(CHEVRON))
                    .flex_none()
                    .text_color(secondary)
                    .with_transformation(Transformation::rotate(Radians(
                        -std::f32::consts::FRAC_PI_2,
                    )))
                    .into_any_element(),
            ),
            (Some(keys), false) => Some(
                div()
                    .flex_none()
                    .text_color(secondary)
                    .child(keys.clone())
                    .into_any_element(),
            ),
            (None, false) if entry.out => Some(
                svg()
                    .debug_selector(move || format!("menu-out {item}"))
                    .path("icons/arrow-up-right.svg")
                    .size(px(OUT_ARROW))
                    .flex_none()
                    .text_color(secondary)
                    .into_any_element(),
            ),
            (None, false) => None,
        };
        let a11y = self.a11y;
        let name = match &entry.detail {
            Some(detail) => format!("{}, {detail}", entry.title).into(),
            None => entry.title.clone(),
        };
        let enabled = entry.enabled || entry.header;
        let (checked, keys) = (entry.checked, shortcut.clone());
        let header = entry.header;
        let radio = checked && level != 0;
        // Where it stands among the level's items, as VoiceOver says it: "3 of 11".
        let titled = |items: &[usize]| {
            items
                .iter()
                .filter(|&&i| !tracking.entries[i].title.is_empty())
                .count()
        };
        let (position, of) = (titled(&lay.items[..row]) + 1, titled(&lay.items));
        placed
            .id(("menu-item", item))
            .rounded(px(row_radius))
            .px(px(ROW_X))
            .flex()
            .items_center()
            .gap(px(GAP))
            .when(lit, |row| row.bg(fill))
            .when(pressed && !entry.destructive, |row| {
                row.bg(highlight.pressed)
            })
            // Ink darkens Delete's red under its Paper label.
            .when(pressed && entry.destructive, |row| {
                row.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(row_radius))
                        .bg(rgba(PRESSED)),
                )
            })
            .children(check)
            .children(swatch)
            .children(favicon)
            .children(icon)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(label)
                    .when(entry.header, |title| {
                        title
                            .text_size(px(HEADER_TEXT))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .child(entry.title.clone())
                    .when_some(entry.detail.clone(), |title, detail| {
                        title
                            .flex()
                            .gap(px(DETAIL_GAP))
                            .child(div().flex_none().text_color(secondary).child(detail))
                    }),
            )
            .children(trailing)
            .when(a11y, |row| {
                let role = match (checked, radio) {
                    _ if header => Role::Heading,
                    (true, true) => Role::MenuItemRadio,
                    (true, false) => Role::MenuItemCheckBox,
                    _ => Role::MenuItem,
                };
                row.role(role)
                    .aria_label(name)
                    .aria_position_in_set(position)
                    .aria_size_of_set(of)
                    .when(checked, |row| {
                        row.aria_toggled(gpui::accesskit::Toggled::True)
                    })
                    .when_some(keys, |row, keys| row.aria_keyshortcuts(keys))
                    .when(submenu, |row| row.aria_expanded(open))
                    .when(lit, |row| row.aria_active_descendant())
                    .a11y_synthetic_children(move |builder| {
                        let node = builder.parent_node();
                        if !enabled {
                            node.set_disabled();
                        }
                        if submenu {
                            node.set_has_popup(gpui::accesskit::HasPopup::Menu);
                        }
                    })
            })
            .into_any_element()
    }

    fn panel(&self, shown: &Shown, depth: usize) -> gpui::AnyElement {
        let tracking = &shown.tracking;
        let open = tracking.open[depth];
        let level = &tracking.layout.levels[open.level];
        let at = level.bounds.origin - shown.origin;
        let next = tracking.open.get(depth + 1).map(|next| next.level);
        let row = |row: usize| {
            let opens = next
                .is_some_and(|next| tracking.layout.levels[next].parent == Some(level.items[row]));
            self.row(shown, open.level, row, open.highlight, opens)
        };
        let scrolls = |at: &usize| level.scroll.is_some_and(|scroll| scroll.holds(*at));
        let rows: Vec<gpui::AnyElement> = (0..level.items.len())
            .filter(|at| !scrolls(at))
            .map(row)
            .collect();
        let run = level.scroll.map(|scroll| {
            div()
                .id(("menu-scroll", open.level))
                .absolute()
                .left_0()
                .top(px(scroll.top))
                .w_full()
                .h(px(scroll.shown))
                .overflow_hidden()
                .children((scroll.first..=scroll.last).map(row))
        });
        let name = level.parent.map_or(SharedString::from("Menu"), |parent| {
            tracking.entries[parent].title.clone()
        });
        let panel = div()
            .id(("menu", open.level))
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(level.bounds.size.width)
            .h(level.bounds.size.height);
        // Where the window can't blend, a round corner or a shadow would show black.
        glass(panel, shown.look.light, level.radii().0, shown.blends)
            .text_size(px(TEXT))
            .children(rows)
            .children(run)
            // The root level's rows go under the window's own node, which has the keyboard.
            .when(self.a11y && level.parent.is_some(), |panel| {
                panel.role(Role::Menu).aria_label(name)
            })
            .into_any_element()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        // Keys go to the last frame's listeners, so a key typed before the window draws a menu it
        // was just given still reaches it.
        let shows = self.shown.is_some();
        // The menu has the keyboard, so VoiceOver says "menu" as it opens, and the highlighted row
        // as its active descendant as the keys move it.
        let root = div()
            .id("menu-window")
            .when(self.a11y && shows, |root| {
                root.role(Role::Menu).aria_label("Menu")
            })
            .size_full()
            .font_family(crate::face::UI)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|menu, event: &KeyDownEvent, _, cx| menu.key_down(event, cx)))
            .on_modifiers_changed(cx.listener(|menu, event: &ModifiersChangedEvent, _, cx| {
                if let Some(shown) = menu
                    .shown
                    .as_mut()
                    .filter(|shown| shown.alt != event.modifiers.alt)
                {
                    shown.alt = event.modifiers.alt;
                    cx.notify();
                }
            }));
        // GPUI draws the window only once the host has put it on screen, some frames after it's
        // asked to.
        if let Some(shown) = self.shown.as_mut()
            && shown.drawn != Some(shown.alt)
        {
            match window.is_visible() {
                // check.sh waits for these lines.
                true => match shown.drawn.replace(shown.alt) {
                    None => log!(
                        "browser: menu drawn {}",
                        described_held(&shown.tracking.entries, shown.alt)
                    ),
                    Some(_) => log!(
                        "browser: menu drawn with ⌥ {} {}",
                        if shown.alt { "held" } else { "up" },
                        described_held(&shown.tracking.entries, shown.alt)
                    ),
                },
                false => window.request_animation_frame(),
            }
        }
        // check.sh waits for these lines to capture a submenu.
        if let Some(shown) = self.shown.as_mut()
            && window.is_visible()
        {
            let open = shown.tracking.open_submenus();
            for &opened in open
                .iter()
                .filter(|opened| !shown.drawn_open.contains(opened))
            {
                log!(
                    "browser: menu submenu {} drawn",
                    shown.tracking.entries[opened].title
                );
            }
            shown.drawn_open = open;
        }
        let Some(shown) = self.shown.as_ref() else {
            return root;
        };
        let panels: Vec<_> = (0..shown.tracking.open.len())
            .map(|depth| self.panel(shown, depth))
            .collect();
        // On a platform whose host doesn't see the pointer first (Linux, the preview), the
        // window's own events drive the menu, and its check reads the log they leave.
        let on = |event: Pointer| {
            cx.listener(
                move |menu: &mut Self,
                      at: &Point<Pixels>,
                      _: &mut Window,
                      cx: &mut Context<Self>| {
                    menu.pointer(event, *at, cx);
                    menu.changed(cx);
                },
            )
        };
        let (moved, pressed, right) = (
            on(Pointer::Moved),
            on(Pointer::Pressed),
            on(Pointer::RightPressed),
        );
        let (released, right_released) = (on(Pointer::Released), on(Pointer::Released));
        root.on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
            moved(&event.position, window, cx)
        })
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| pressed(&event.position, window, cx),
        )
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, window, cx| right(&event.position, window, cx),
        )
        .on_mouse_up(
            MouseButton::Left,
            move |event: &MouseUpEvent, window, cx| released(&event.position, window, cx),
        )
        .on_mouse_up(
            MouseButton::Right,
            move |event: &MouseUpEvent, window, cx| right_released(&event.position, window, cx),
        )
        .on_scroll(
            cx.listener(|menu, event: &ScrollEvent, window, cx| menu.wheel(event, window, cx)),
        )
        .children(panels)
    }
}

#[cfg(test)]
mod tests;
