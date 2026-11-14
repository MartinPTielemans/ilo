// Adapted from Zed's crates/ui/src/components/context_menu.rs
// (https://github.com/zed-industries/zed) at commit b36b283021c48f1712714b8a0e2ad6ffb578920b:
// the keyboard's walk over the selectable rows and the submenu's open and close. Copyright Zed
// Industries, Inc. Licensed under the GNU General Public License, version 3 or later.

//! ilo's own context menu, the same on every platform (#324): an opaque panel of 24-pt rows, with
//! submenus, keys, type-to-select, press-drag-release and a safe triangle on the way to a
//! submenu. GPUI can't draw over the page's native view, so it's a window of its own that the host
//! shows over the browser window, as its child, with the keyboard, as the command bar is
//! (AGENTS.md › Over the page). What the menu decides is in `Tracking` and `lay_out`, which the
//! tests drive; what it asks of its window goes through `Host`.

use std::{rc::Rc, time::Duration};
use web_time::Instant;

use gpui::{
    AnyWindowHandle, App, Bounds, Context, FocusHandle, Hsla, KeyDownEvent, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Radians, Rgba, Role,
    SharedString, Task, TitlebarOptions, Transformation, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, img, point, prelude::*, px, rgb,
    rgba, size, svg,
};

use crate::{
    elevation::{Level as Elevation, Reach},
    shell::{MenuEntry, Shell, keystroke_label},
    text_input::FieldMenu,
};

pub use crate::tokens::menu::*;

/// A release this close to the press that opened the menu leaves it open, as a click.
const DRAG_SLOP: f32 = 4.;
/// The safe triangle ends after the pointer rests this long in it.
const SAFE_TIMEOUT: Duration = Duration::from_millis(300);
/// Type-to-select starts over after this long without a key.
const TYPE_RESET: Duration = Duration::from_millis(1000);

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
    rim: DARK_RIM,
    label: DARK_LABEL,
    secondary: DARK_SECONDARY,
    line: DARK_LINE,
    disabled: DARK_DISABLED,
    destructive: DARK_DESTRUCTIVE,
};

const LIGHT: Palette = Palette {
    fill: LIGHT_FILL,
    rim: LIGHT_RIM,
    label: LIGHT_LABEL,
    secondary: LIGHT_SECONDARY,
    line: LIGHT_LINE,
    disabled: LIGHT_DISABLED,
    destructive: LIGHT_DESTRUCTIVE,
};

/// The row under the pointer or the keys: the Space's accent, the accent pressed while the button
/// is down on it, and the label the palette solves on the accent, which the press moves the accent
/// away from, so it reads at least as well pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub fill: Rgba,
    pub pressed: Rgba,
    pub label: Rgba,
}

impl Highlight {
    pub fn of(palette: &crate::palette::Palette) -> Self {
        Highlight {
            fill: palette.accent,
            pressed: palette.accent_pressed().into(),
            label: palette.accent_label,
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

/// The menu's look: the panel's material, Ink in a dark window and Paper in a light one, and the
/// highlight in its Space's accent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub light: bool,
    pub highlight: Highlight,
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
            highlight: Highlight::of(&crate::palette::Palette::default()),
        }
    }

    pub fn palette(self) -> &'static Palette {
        if self.light { &LIGHT } else { &DARK }
    }
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
    /// Takes it away, and gives the keyboard back to what had it.
    fn hide(&self);
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
    /// With a press outside it, at this point in the browser window's content.
    PressedAt(Point<Pixels>),
    /// With the user gone to another window or app, or its browser window changed under it.
    Left,
    /// With its keys, a shortcut, a scroll outside it or another menu taking its place.
    Otherwise,
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
    /// Whether a column for checks comes before the titles.
    checks: bool,
}

impl Level {
    fn row_bounds(&self, at: usize, entries: &[MenuEntry]) -> Bounds<Pixels> {
        let height = if entries[self.items[at]].title.is_empty() {
            LINE
        } else {
            ROW
        };
        let origin = self.bounds.origin + point(px(PADDING), px(self.tops[at]));
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
    let chevron = if submenus { GAP + CHEVRON } else { 0. };
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

/// Lays `entries` out as a menu with its top left at `at`, flipping up or left where `area` has
/// no room below or to the right, as AppKit's does, with each submenu beside the row that opens
/// it, flipping left and sliding up to fit.
pub(crate) fn lay_out(
    entries: &[MenuEntry],
    at: Point<Pixels>,
    area: Bounds<Pixels>,
    width_of: &dyn Fn(&str) -> f32,
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
        let mut tops = Vec::with_capacity(items.len());
        let mut y = PADDING;
        for &item in &items {
            tops.push(y);
            y += if entries[item].title.is_empty() {
                LINE
            } else {
                ROW
            };
        }
        let checks = items.iter().any(|&item| entries[item].checked);
        let size = (panel_width(entries, &items, checks, width_of), y + PADDING);
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
        });
    }
    let frame = framed(&levels, Elevation::Popover.reach());
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
    let title = root.row_bounds(row, entries).origin + point(px(ROW_X + lead), px(0.));
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
    Close,
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
    typed: String,
    typed_at: Option<Instant>,
}

/// Whether `at` is inside the triangle `a`, `b`, `c`.
fn in_triangle(at: Point<Pixels>, a: Point<Pixels>, b: Point<Pixels>, c: Point<Pixels>) -> bool {
    let cross = |p: Point<Pixels>, q: Point<Pixels>, r: Point<Pixels>| {
        f32::from(q.x - p.x) * f32::from(r.y - p.y) - f32::from(q.y - p.y) * f32::from(r.x - p.x)
    };
    let (d1, d2, d3) = (cross(a, b, at), cross(b, c, at), cross(c, a, at));
    let negative = d1 < 0. || d2 < 0. || d3 < 0.;
    let positive = d1 > 0. || d2 > 0. || d3 > 0.;
    !(negative && positive)
}

/// `text` lowercased without its accents, for type-to-select.
fn folded(text: &str) -> String {
    const ACCENTED: [(&str, char); 8] = [
        ("àáâãäåā", 'a'),
        ("çćč", 'c'),
        ("èéêëēė", 'e'),
        ("ìíîïī", 'i'),
        ("ñń", 'n'),
        ("òóôõöøō", 'o'),
        ("ùúûüū", 'u'),
        ("ýÿ", 'y'),
    ];
    (text.chars().flat_map(char::to_lowercase))
        .map(|c| {
            ACCENTED
                .iter()
                .find(|(set, _)| set.contains(c))
                .map_or(c, |&(_, base)| base)
        })
        .collect()
}

impl Tracking {
    pub(crate) fn new(entries: Vec<MenuEntry>, layout: Layout, at: Point<Pixels>) -> Self {
        // A pop-up's menu opens with its current item highlighted.
        let over = (layout.levels[0].items.iter().copied()).find(|&item| {
            entries[item].over && entries[item].enabled && !entries[item].title.is_empty()
        });
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
            typed: String::new(),
            typed_at: None,
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
            let row = (0..level.items.len())
                .find(|&row| level.row_bounds(row, &self.entries).contains(&at));
            return match row {
                Some(row) if !self.entries[level.items[row]].title.is_empty() => {
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
    /// `rested` to be called once SAFE_TIMEOUT has passed.
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

    /// The pointer has rested since its last move: in the safe triangle for SAFE_TIMEOUT, what's
    /// under it takes the highlight. Returns how much longer it has to rest when that's not yet:
    /// a timer can fire a little early, as Windows' do by up to a clock tick (#2158).
    pub(crate) fn rested(&mut self, now: Instant) -> Option<Duration> {
        let rested = now.duration_since(self.resting?);
        if rested < SAFE_TIMEOUT {
            return Some(SAFE_TIMEOUT - rested);
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
            Hit::Outside if opening.is_some() => Outcome::Close,
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
                    return Outcome::Close;
                }
                self.open.pop();
            }
            Key::Char(_) => {}
        }
        Outcome::Stay
    }

    /// Type-to-select: the first item of the deepest level whose title starts with what's been
    /// typed in the last second, ignoring case and accents, or with the same letter again, the
    /// next one that starts with it.
    fn type_select(&mut self, c: char, now: Instant) {
        if self
            .typed_at
            .is_none_or(|at| now.duration_since(at) > TYPE_RESET)
        {
            self.typed.clear();
        }
        self.typed.push(c);
        self.typed_at = Some(now);
        let (k, items) = self.deepest();
        let typed = folded(&self.typed);
        let mut letters = typed.chars();
        let first = letters.next();
        let repeated = typed.chars().count() > 1 && letters.all(|each| Some(each) == first);
        let starts =
            |item: &usize, with: &str| folded(&self.entries[*item].title).starts_with(with);
        let found = if repeated {
            let with = first.map(String::from).unwrap_or_default();
            let after = self.open[k]
                .highlight
                .and_then(|item| items.iter().position(|&each| each == item));
            let start = after.map_or(0, |at| at + 1);
            (items[start..].iter().chain(&items[..start]))
                .find(|item| starts(item, &with))
                .copied()
        } else {
            items.iter().find(|item| starts(item, &typed)).copied()
        };
        if let Some(item) = found {
            self.open[k].highlight = Some(item);
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
            if entry.parent != parent || entry.title.is_empty() || entry.alternate {
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
        menu.close(Closed::Otherwise, cx);
        let system = window.text_system().clone();
        let width_of = |text: &str| {
            let text = SharedString::from(text.to_string());
            crate::tooltip::text_width(&text, TEXT, gpui::FontWeight::NORMAL, &system)
        };
        let layout = lay_out_over(&request.items, request.at, request.area, &width_of);
        let blends = menu.host.blends();
        let layout = if blends { layout } else { layout.opaque() };
        // check.sh waits for this line before it chooses.
        log!("browser: menu shown {}", described(&request.items));
        let frame = layout.frame;
        let origin = menu.fixed.unwrap_or(frame.origin);
        let shown = Shown {
            tracking: Tracking::new(request.items, layout, request.at),
            look: request.look,
            origin,
            chosen: Some(request.chosen),
            logged: None,
            logged_open: Vec::new(),
            alt,
            drawn: None,
            drawn_open: Vec::new(),
            blends,
        };
        let panels = shown.panels();
        menu.shown = Some(shown);
        window.focus(&menu.focus, cx);
        // After a hide that `close` asked for, so the new menu stays.
        let host = menu.host.clone();
        cx.defer(move |_| host.show(frame, &panels));
        cx.notify();
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
    update(cx, |menu, _, cx| menu.close(Closed::Left, cx));
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
            menu.close(Closed::Chose(item), cx);
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
    FieldMenu(Rc::new(move |items, at, from_keys, window, chosen, cx| {
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
            let items = (items.into_iter())
                .map(|(title, enabled)| MenuEntry::named(title, enabled))
                .collect();
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
    fn close(&mut self, closed: Closed, cx: &mut Context<Self>) {
        let Some(mut shown) = self.shown.take() else {
            return;
        };
        let closed = match closed {
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
        if let Some(answer) = shown.chosen.take() {
            cx.defer(move |cx| answer(closed, cx));
        }
        cx.notify();
    }

    fn outcome(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        match outcome {
            Outcome::Stay => cx.notify(),
            Outcome::Pick(item) => self.close(Closed::Chose(item), cx),
            Outcome::Close => self.close(Closed::Otherwise, cx),
        }
    }

    /// `at` in the window, in the browser window's content.
    fn in_content(&self, at: Point<Pixels>) -> Point<Pixels> {
        self.shown.as_ref().map_or(at, |shown| shown.origin + at)
    }

    fn pointer(&mut self, event: Pointer, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let at = self.in_content(at);
        let Some(shown) = self.shown.as_mut() else {
            return false;
        };
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
                // A click outside closes the menu and goes no further; a right-click goes on, to
                // open the menu of what it's on.
                self.close(Closed::PressedAt(at), cx);
                event == Pointer::Pressed
            }
            Pointer::Released => {
                let outcome = tracking.released(at);
                self.outcome(outcome, cx);
                true
            }
            Pointer::Scrolled => {
                if tracking.hit(at) != Hit::Outside {
                    return true;
                }
                self.close(Closed::Otherwise, cx);
                false
            }
        }
    }

    /// Once the pointer has rested in the safe triangle long enough, what's under it takes the
    /// highlight.
    fn wait_for_rest(&mut self, cx: &mut Context<Self>) {
        self._rest = Some(cx.spawn(async move |menu, cx| {
            let mut wait = SAFE_TIMEOUT;
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

    fn key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let modifiers = event.keystroke.modifiers;
        // A shortcut closes the menu, as the menu shows them but doesn't run them.
        if modifiers.platform || modifiers.control {
            return self.close(Closed::Otherwise, cx);
        }
        if let Some(key) = key_of(event) {
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
        if !shown.blends && open != shown.logged_open {
            let (host, frame, panels) = (self.host.clone(), tracking.layout.frame, shown.panels());
            cx.defer(move |_| host.show(frame, &panels));
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
        let top = bounds.origin.y - lay.bounds.origin.y;
        let placed = div()
            .absolute()
            .left(px(PADDING))
            .top(top)
            .w(bounds.size.width)
            .h(bounds.size.height);
        if entry.title.is_empty() {
            let line = div().mx(px(ROW_X)).h(px(1.)).bg(rgba(palette.line));
            return (placed.id(("menu-line", item)).flex().items_center())
                .when(self.a11y, |row| row.role(Role::Splitter))
                .child(div().flex_1().child(line))
                .into_any_element();
        }
        let lit = lit == Some(item);
        let highlight = shown.look.highlight;
        let (fill, on_fill): (Rgba, Rgba) = match entry.destructive {
            true => (rgba(DESTRUCTIVE_FILL), rgba(DESTRUCTIVE_LABEL)),
            false => (highlight.fill, highlight.label),
        };
        let label: Hsla = match (lit, entry.enabled, entry.destructive) {
            _ if entry.header => rgba(palette.secondary).into(),
            (true, ..) => on_fill.into(),
            (false, false, _) => rgba(palette.disabled).into(),
            (false, true, true) => rgba(palette.destructive).into(),
            (false, true, false) => rgba(palette.label).into(),
        };
        let secondary: Hsla = match (lit, entry.enabled) {
            (true, _) => on_fill.into(),
            (false, false) => rgba(palette.disabled).into(),
            (false, true) => rgba(palette.secondary).into(),
        };
        let submenu = tracking.layout.level_of(item).is_some();
        let shortcut = entry.shortcut.as_deref().map(keystroke_label);
        let pressed = lit && tracking.pressed;
        let check = lay.checks.then(|| {
            div().w(px(CHECK)).flex_none().when(entry.checked, |slot| {
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
            .rounded(px(ROW_RADIUS))
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
                        .rounded(px(ROW_RADIUS))
                        .bg(rgba(PRESSED)),
                )
            })
            .children(check)
            .children(swatch)
            .children(favicon)
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
        let palette = shown.look.palette();
        let at = level.bounds.origin - shown.origin;
        let next = tracking.open.get(depth + 1).map(|next| next.level);
        let rows = (0..level.items.len()).map(|row| {
            let opens = next
                .is_some_and(|next| tracking.layout.levels[next].parent == Some(level.items[row]));
            self.row(shown, open.level, row, open.highlight, opens)
        });
        let name = level.parent.map_or(SharedString::from("Menu"), |parent| {
            tracking.entries[parent].title.clone()
        });
        div()
            .id(("menu", open.level))
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(level.bounds.size.width)
            .h(level.bounds.size.height)
            // Where the window can't blend, a round corner or a shadow would show black.
            .when(shown.blends, |panel| {
                panel
                    .rounded(px(RADIUS))
                    .shadow(Elevation::Popover.shadow())
            })
            .bg(rgba(palette.fill))
            .border_1()
            .border_color(rgba(palette.rim))
            .text_size(px(TEXT))
            .children(rows)
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
        .children(panels)
    }
}

#[cfg(test)]
mod tests;
