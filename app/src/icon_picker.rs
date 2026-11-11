//! The popover that picks a Space's icon, Arc's: it hangs under the Space's name, its arrow at the
//! icon, with Emoji | Icon at its top, and a trash button after it once there's an icon to remove,
//! and under them the glyphs, or a search and a grid of emoji (docs/ux-spec.md › Spaces › Icon).
//! A tab's hangs the same way from its favicon, and a folder's from its folder, each with the Emoji
//! tab only, as their icons are emoji. GPUI can't draw over the page's native view, so it's a
//! window of its own that the host shows over the browser window, as its child, with the keyboard,
//! translucent over a blur in the shape drawn here. What it asks of its window goes through `Host`.

use std::{ops::Range, rc::Rc};
use web_time::Instant;

use gpui::{
    AnyElement, App, Bounds, Context, Div, Entity, FocusHandle, Focusable, FontWeight, Global,
    Hsla, KeyBinding, MouseButton, PathBuilder, Pixels, Point, Rgba, ScrollStrategy, SharedString,
    Stateful, Subscription, TitlebarOptions, UniformListScrollHandle, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::Role, actions, canvas, div, point, prelude::*, px, rgb, rgba, size, svg,
    uniform_list,
};

use crate::{
    elevation::Level,
    palette::{INK, Palette, STATE_HOVERED, STATE_PRESSED},
    shell::{KEY_CONTEXT, cubic_bezier},
    space_icon::{self, GLYPHS, SpaceIcon, has_icon},
    text_input::{self, TextInput},
};

mod emoji;

pub use emoji::Recents;
use emoji::{Drawable, Grid, Row};
use emojis::SkinTone;

actions!(icon_picker, [Dismiss, Keep, Up, Down]);

use crate::tokens::icon_picker::ARROW;
use crate::tokens::icon_picker::ARROW_X;
use crate::tokens::icon_picker::CARD;
pub use crate::tokens::icon_picker::WIDTH;
/// The arrow's tip in the window, which the picker grows in from as it opens.
pub const ARROW_TIP: (f32, f32) = (ARROW_X, 0.);
use crate::tokens::icon_picker::ARROW_FOOT_RADIUS;
use crate::tokens::icon_picker::ARROW_TIP_RADIUS;
use crate::tokens::icon_picker::RADIUS;
pub const HEIGHT: f32 = ARROW + CARD;
use crate::tokens::icon_picker::BELOW_ICON;
use crate::tokens::icon_picker::LABEL;
use crate::tokens::icon_picker::SEGMENT_WIDTH;
use crate::tokens::icon_picker::SEGMENTS_FILL;
use crate::tokens::icon_picker::SEGMENTS_HEIGHT;
use crate::tokens::icon_picker::SEGMENTS_RADIUS;
use crate::tokens::icon_picker::SEGMENTS_TOP;
use crate::tokens::icon_picker::TRASH_GAP;
use crate::tokens::icon_picker::TRASH_GLYPH;
/// The glyphs, eight to a row on a 31 pt pitch, the first 12.25 in and 47.75 under the card's top.
const COLUMNS: usize = 8;
use crate::tokens::icon_picker::CARET;
use crate::tokens::icon_picker::CHOSEN;
use crate::tokens::icon_picker::GLYPH_SIZE;
use crate::tokens::icon_picker::GRID_TOP;
use crate::tokens::icon_picker::GRID_X;
use crate::tokens::icon_picker::HINT;
use crate::tokens::icon_picker::HOVERED;
use crate::tokens::icon_picker::PITCH;
use crate::tokens::icon_picker::PLATE;
use crate::tokens::icon_picker::PLATE_RADIUS;
use crate::tokens::text;
/// Arc's switch between Emoji and Icon slides the content a card's width over 0.2 s, easing out
/// (Arc 1.170 rec at 2×).
const SLIDE: f32 = 0.2;

/// What the picker asks of its window.
pub trait Host {
    /// Shows the picker's window with its top left at `left`, `top` in browser window `window`,
    /// the one that asked for it, with the keyboard.
    fn show(&self, window: u64, left: f32, top: f32);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
    /// The emoji picked last have changed: `recents` is what `Recents::to_text` makes of them, for
    /// the host to keep and give `open` again after a relaunch.
    fn keep_recents(&self, recents: &str);
    /// Whether the system's emoji font draws `emoji` as the one emoji it is. A system older than
    /// the list draws a newer emoji as a missing glyph and a newer sequence as its parts, so the
    /// grid leaves those out. A platform with no way to ask offers everything.
    fn draws(&self, _emoji: &str) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tab {
    Emoji,
    Icon,
}

/// What the picker picks an icon for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Subject {
    Space(u64),
    Tab(u64),
    Folder(u64),
}

impl Subject {
    /// What an assistive app calls the picker.
    fn picker_name(self) -> &'static str {
        match self {
            Subject::Space(_) => "Space Icon",
            Subject::Tab(_) => "Tab Icon",
            Subject::Folder(_) => "Folder Icon",
        }
    }

    /// The tabs it offers: a tab's custom icon takes the favicon's place, and a folder's the
    /// folder's, so each is a character the row draws, and the Icon tab's glyphs are a Space's.
    fn tabs(self) -> &'static [Tab] {
        match self {
            Subject::Space(_) => &[Tab::Emoji, Tab::Icon],
            Subject::Tab(_) | Subject::Folder(_) => &[Tab::Emoji],
        }
    }
}

impl std::fmt::Display for Subject {
    /// As check.sh reads it: a Space is its id, a tab "tab" and its id, a folder "folder" and its.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Subject::Space(space) => write!(f, "{space}"),
            Subject::Tab(tab) => write!(f, "tab {tab}"),
            Subject::Folder(folder) => write!(f, "folder {folder}"),
        }
    }
}

/// The icon a Space is to have: what the core keeps, `""` for none.
pub type Picked = Rc<dyn Fn(&str, &mut App)>;

struct Picking {
    subject: Subject,
    icon: SharedString,
    tab: Tab,
    picked: Picked,
    /// What the Emoji tab lists for what's in its search, and the one on the plate, which ↩ picks.
    query: String,
    grid: Grid,
    selected: Option<usize>,
    /// Whether the six skin tones are out along the search.
    tones: bool,
    /// While the content slides from one tab's to the other's: the tab it slides from, and when.
    slide: Option<(Tab, Instant)>,
    /// Whether it has drawn on screen since it was shown, before which a click finds nothing.
    drawn: bool,
    _edit: Option<Subscription>,
}

pub struct IconPicker {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    /// The Emoji tab's search.
    search: Entity<TextInput>,
    scroll: UniformListScrollHandle,
    recents: Recents,
    /// The emoji the system draws, asked of the host when the Emoji tab first shows, once a launch.
    drawable: Option<Drawable>,
    picking: Option<Picking>,
    /// An assistive app asks for the tree, so the picker names what it draws.
    a11y: bool,
}

struct IconPickerWindow(WindowHandle<IconPicker>);

impl Global for IconPickerWindow {}

/// Opens the picker's window, hidden, for `host` to show over the browser window, ahead of time as
/// the command bar's is. `recents` are the emoji picked before.
pub fn open(
    host: Rc<dyn Host>,
    recents: Recents,
    cx: &mut App,
) -> Option<WindowHandle<IconPicker>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    open_at(host, recents, bounds, false, cx)
}

/// Opens the picker's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    recents: Recents,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<IconPicker>> {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("IconPicker")),
        KeyBinding::new("enter", Keep, Some("IconPicker")),
        KeyBinding::new("up", Up, Some("IconPicker")),
        KeyBinding::new("down", Down, Some("IconPicker")),
    ]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // The preview's has none, so it hasn't the window buttons the host's takes off.
        titlebar: (!show).then_some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: show,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The card's shape is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        let search = cx.new(|cx| {
            TextInput::new("Search", cx)
                .placeholder_color(rgba(HINT))
                .caret_color(rgb(CARET))
                .selection_color(rgba(HOVERED))
        });
        cx.new(|cx| IconPicker {
            host,
            focus: cx.focus_handle(),
            search,
            scroll: UniformListScrollHandle::new(),
            recents,
            drawable: None,
            picking: None,
            a11y: false,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(IconPickerWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the icon picker's window: {error}");
            None
        }
    }
}

/// The card's corners, from the window's top left, clockwise from the arrow's tip, each with the
/// radius it's rounded by, for the host's blur and shadow and for the drawing here.
pub fn corners() -> [(f32, f32, f32); 6] {
    [
        (ARROW_X, 0., ARROW_TIP_RADIUS),
        (ARROW_X + ARROW, ARROW, ARROW_FOOT_RADIUS),
        (WIDTH, ARROW, RADIUS),
        (WIDTH, HEIGHT, RADIUS),
        (0., HEIGHT, RADIUS),
        (0., ARROW, 0.),
    ]
}

/// The tab the picker opens on for a Space with `icon`: the emoji's if it has one.
pub fn tab_for(icon: &str) -> Tab {
    match SpaceIcon::parse(icon) {
        SpaceIcon::Emoji(_) => Tab::Emoji,
        _ => Tab::Icon,
    }
}

/// Shows the picker for `subject`, which has `icon`, its arrow at `anchor`, the middle of the
/// icon's slot in browser window `over`, with the chosen segment in the Space's accent. `picked`
/// gets the icon the user picks. A context menu in a window behind another asks for it there.
pub fn show(
    over: u64,
    subject: Subject,
    icon: SharedString,
    anchor: Point<f32>,
    picked: Picked,
    cx: &mut App,
) {
    crate::lazy_window::ready::<IconPicker>(cx);
    let Some(window) = cx.try_global::<IconPickerWindow>().map(|picker| picker.0) else {
        return;
    };
    window
        .update(cx, |picker, window, cx| {
            let tab = match subject {
                Subject::Space(_) => tab_for(&icon),
                Subject::Tab(_) | Subject::Folder(_) => Tab::Emoji,
            };
            // check.sh reads this line.
            log!("browser: icon picker for {subject} on {}", tab.name());
            picker.picking = Some(Picking {
                subject,
                icon,
                tab,
                picked,
                query: String::new(),
                grid: Grid::default(),
                selected: None,
                tones: false,
                slide: None,
                drawn: false,
                _edit: None,
            });
            picker.show_tab(tab, window, cx);
            picker
                .host
                .show(over, anchor.x - ARROW_X, anchor.y + BELOW_ICON - ARROW);
            cx.notify();
        })
        .ok();
}

/// The picker, for the window it hangs over to watch whether it still shows.
pub fn entity(cx: &App) -> Option<Entity<IconPicker>> {
    cx.try_global::<IconPickerWindow>()?.0.entity(cx).ok()
}

/// The user left the picker without picking, such as with a click outside it.
/// The Space it edits is going from under it, as a swipe turns the page: it closes as Esc closes
/// it, where `dismissed` hears of a close the host has made.
pub fn leave(cx: &mut App) {
    if let Some(window) = cx.try_global::<IconPickerWindow>().map(|picker| picker.0) {
        window
            .update(cx, |picker, window, cx| {
                picker.dismiss(&Dismiss, window, cx)
            })
            .ok();
    }
}

pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<IconPickerWindow>().map(|picker| picker.0) {
        window
            .update(cx, |picker, _, cx| {
                picker.picking = None;
                cx.notify();
            })
            .ok();
    }
}

/// Whether the picker shows, for a click on the icon it points at to close it rather than open it
/// again.
pub fn is_open(cx: &App) -> bool {
    open_for(cx).is_some()
}

/// What the picker shows for, if it shows.
pub fn open_for(cx: &App) -> Option<Subject> {
    let picker = cx.try_global::<IconPickerWindow>()?.0.read(cx).ok()?;
    picker.picking.as_ref().map(|picking| picking.subject)
}

/// How far right the old tab's content and the new one's are, `progress` (0 to 1) into a switch
/// from `from` to `to`: the content moves the way the segment chosen is from the other, so Icon's
/// comes in from the right.
fn slid(from: Tab, to: Tab, progress: f32) -> (f32, f32) {
    let way = match (from, to) {
        (Tab::Emoji, Tab::Icon) => -1.,
        (Tab::Icon, Tab::Emoji) => 1.,
        _ => 0.,
    };
    let t = cubic_bezier(0., 0., 0.58, 1., progress.clamp(0., 1.));
    (way * WIDTH * t, way * WIDTH * (t - 1.))
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::Emoji => "emoji",
            Tab::Icon => "icon",
        }
    }
}

impl IconPicker {
    fn show_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picking) = &mut self.picking else {
            return;
        };
        if picking.tab != tab {
            picking.slide = Some((picking.tab, crate::motion::start(cx)));
        }
        picking.tab = tab;
        picking.tones = false;
        picking._edit = None;
        if tab == Tab::Icon {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        // The whole list, with the Space's emoji on the plate and in view. The table is static, so
        // this is the work of a pass over it, and only when this tab shows.
        picking.query.clear();
        let host = &self.host;
        let drawable = self
            .drawable
            .get_or_insert_with(|| Drawable::new(|emoji| host.draws(emoji)));
        drawable.ask_tone(self.recents.tone(), |emoji| host.draws(emoji));
        picking.grid = Grid::new("", &self.recents, drawable);
        picking.selected = current(&picking.icon, &picking.grid);
        self.search.update(cx, |field, cx| {
            field.reset(SharedString::default(), window, cx)
        });
        self.scroll_to_selected(ScrollStrategy::Center);
        if let Some(picking) = &mut self.picking {
            picking._edit = Some(cx.observe_in(&self.search, window, |picker, field, _, cx| {
                let text = field.read(cx).text().to_string();
                picker.searched(&text, cx);
            }));
        }
        window.focus(&self.search.focus_handle(cx), cx);
        cx.notify();
    }

    /// The search has `text` in it: the list narrows to what matches, the first of which takes the
    /// plate for ↩, and an empty search brings the whole list back at the Space's emoji.
    fn searched(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(picking) = &mut self.picking else {
            return;
        };
        if picking.tab != Tab::Emoji || picking.query == text {
            return;
        }
        // Asked for when the tab showed, which any search follows.
        if self.drawable.is_none() {
            return;
        }
        picking.query = text.to_string();
        self.regrid(cx);
    }

    /// Lists the emoji again for the search, as when it changes.
    fn regrid(&mut self, cx: &mut Context<Self>) {
        let (Some(picking), Some(drawable)) = (&mut self.picking, &self.drawable) else {
            return;
        };
        let text = picking.query.as_str();
        picking.grid = Grid::new(text, &self.recents, drawable);
        picking.selected = if text.trim().is_empty() {
            current(&picking.icon, &picking.grid)
        } else {
            (!picking.grid.items.is_empty()).then_some(0)
        };
        if picking.selected.is_some() {
            self.scroll_to_selected(ScrollStrategy::Center);
        } else {
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn scroll_to_selected(&self, strategy: ScrollStrategy) {
        let Some(picking) = &self.picking else {
            return;
        };
        if let Some(row) = picking.selected.and_then(|item| picking.grid.row_of(item)) {
            self.scroll.scroll_to_item(row, strategy);
        }
    }

    /// Moves the plate `across` items or `down` rows, and into view. Whether there was a list to
    /// move in.
    fn go(&mut self, across: isize, down: isize, cx: &mut Context<Self>) -> bool {
        let Some(picking) = self
            .picking
            .as_mut()
            .filter(|picking| picking.tab == Tab::Emoji)
        else {
            return false;
        };
        if self.search.read(cx).composing() {
            return false;
        }
        picking.selected = picking.grid.step(picking.selected, across, down);
        self.scroll_to_selected(ScrollStrategy::Nearest);
        cx.notify();
        true
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.go(0, -1, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.go(0, 1, cx);
    }

    /// ← and → move the caret in a search that has something in it, else the plate.
    fn left(&mut self, _: &text_input::Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.search.read(cx).text().is_empty() && self.go(-1, 0, cx) {
            cx.stop_propagation();
        }
    }

    fn right(&mut self, _: &text_input::Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.search.read(cx).text().is_empty() && self.go(1, 0, cx) {
            cx.stop_propagation();
        }
    }

    /// Shows the grid's emoji in `tone`, and keeps it with the recents for the next time.
    fn pick_tone(&mut self, tone: SkinTone, cx: &mut Context<Self>) {
        let Some(picking) = &mut self.picking else {
            return;
        };
        picking.tones = false;
        if tone != self.recents.tone() {
            let host = &self.host;
            if let Some(drawable) = &mut self.drawable {
                drawable.ask_tone(tone, |emoji| host.draws(emoji));
            }
            self.recents.set_tone(tone);
            self.host.keep_recents(&self.recents.to_text());
            self.regrid(cx);
        }
        cx.notify();
    }

    /// The tone button at the search's right end, or the six tones along it once it's clicked.
    fn tone_button(&self, picking: &Picking, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let a11y = self.a11y;
        let plate = |id: (&'static str, usize), tone: SkinTone| {
            div()
                .id(id)
                .size(px(emoji::TONE_PLATE))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(SEGMENTS_RADIUS))
                .hover(|plate| plate.bg(Palette::on_dark(STATE_HOVERED)))
                .active(|plate| plate.bg(Palette::on_dark(STATE_PRESSED)))
                .text_size(px(emoji::TONE_EMOJI))
                .child(emoji::hand(tone))
        };
        let shown = self.recents.tone();
        let tones = picking.tones.then(|| {
            emoji::TONES.iter().enumerate().map(|(index, &tone)| {
                plate(("icon-picker-tone", index), tone)
                    .when(a11y, |plate| {
                        let name = emoji::tone_name(tone);
                        plate
                            .role(Role::ListBoxOption)
                            .aria_label(name)
                            .aria_selected(tone == shown)
                    })
                    .when(tone == shown, |plate| plate.bg(rgba(CHOSEN)))
                    .on_click(cx.listener(move |picker, _, window, cx| {
                        picker.pick_tone(tone, cx);
                        window.focus(&picker.search.focus_handle(cx), cx);
                    }))
            })
        });
        let button = (!picking.tones).then(|| {
            let name = format!("Skin Tone, {}", emoji::tone_name(shown));
            let plate = plate(("icon-picker-tones", 0), shown);
            let plate = plate.when(a11y, |plate| {
                plate
                    .role(Role::Button)
                    .aria_label(name)
                    .aria_expanded(false)
            });
            plate.on_click(cx.listener(|picker, _, window, cx| {
                if let Some(picking) = &mut picker.picking {
                    picking.tones = true;
                    cx.notify();
                }
                // The picker takes the keyboard on the press, and typing goes on in the search.
                window.focus(&picker.search.focus_handle(cx), cx);
            }))
        });
        div()
            .id("icon-picker-tone-list")
            .when(a11y && picking.tones, |tones| {
                tones.role(Role::ListBox).aria_label("Skin Tone")
            })
            .flex()
            .flex_none()
            .children(tones.into_iter().flatten())
            .children(button)
    }

    /// Gives the Space the emoji at `item` in the list, and puts it first among the recents.
    fn pick_emoji(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(emoji) = self
            .picking
            .as_ref()
            .and_then(|picking| picking.grid.items.get(item).copied())
        else {
            return;
        };
        let emoji = emoji.as_str();
        self.recents.push(emoji);
        self.host.keep_recents(&self.recents.to_text());
        self.pick(emoji, cx);
    }

    /// Gives the Space `icon`, `""` for none, if it hasn't it already, and closes.
    fn pick(&mut self, icon: &str, cx: &mut Context<Self>) {
        let Some(picking) = self.picking.take() else {
            return;
        };
        self.host.hide();
        if icon != picking.icon.as_ref() {
            (picking.picked)(icon, cx);
        }
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.picking.take().is_some() {
            // check.sh reads this line.
            log!("browser: icon picker dismissed");
            self.host.hide();
            cx.notify();
        }
    }

    /// ↩ on the Emoji tab picks the emoji on the plate. While an input method composes, it's the
    /// method's.
    fn keep(&mut self, _: &Keep, _: &mut Window, cx: &mut Context<Self>) {
        let Some(picking) = self
            .picking
            .as_ref()
            .filter(|picking| picking.tab == Tab::Emoji)
        else {
            return;
        };
        if let Some(item) = picking
            .selected
            .filter(|_| !self.search.read(cx).composing())
        {
            self.pick_emoji(item, cx);
        }
    }

    /// Emoji | Icon, and the trash button after it if there's an icon to remove.
    fn top(&self, picking: &Picking, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let a11y = self.a11y;
        let segment = |tab: Tab, label: &'static str| {
            let chosen = picking.tab == tab;
            div()
                .id(label)
                .when(a11y, |segment| {
                    segment
                        .role(Role::Tab)
                        .aria_label(label)
                        .aria_selected(chosen)
                })
                .w(px(SEGMENT_WIDTH))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(SEGMENTS_RADIUS))
                .when(chosen, |segment| {
                    let palette = crate::palette::current(cx);
                    segment.bg(palette.accent).text_color(palette.accent_label)
                })
                // The other takes the ladder under the pointer (ours: Arc's isn't recorded), in Ink, away
                // from its white label, which reads on the track with no more room, as the
                // accent's states move away from theirs.
                .when(!chosen, |segment| {
                    segment
                        .hover(|style| style.bg(Hsla::from(rgb(INK)).opacity(STATE_HOVERED)))
                        .active(|style| style.bg(Hsla::from(rgb(INK)).opacity(STATE_PRESSED)))
                })
                .child(label)
                .on_click(cx.listener(move |picker, _, window, cx| {
                    if picker
                        .picking
                        .as_ref()
                        .is_some_and(|picking| picking.tab != tab)
                    {
                        // check.sh reads this line.
                        log!("browser: icon picker on {}", tab.name());
                        picker.show_tab(tab, window, cx);
                    }
                }))
        };
        let trash = has_icon(&picking.icon).then(|| {
            div()
                .id("icon-picker-trash")
                .when(a11y, |trash| {
                    trash.role(Role::Button).aria_label("Remove Icon")
                })
                .size(px(SEGMENTS_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(SEGMENTS_RADIUS))
                .hover(|trash| trash.bg(Palette::on_dark(STATE_HOVERED)))
                .active(|trash| trash.bg(Palette::on_dark(STATE_PRESSED)))
                .child(
                    svg()
                        .path("icons/trash.svg")
                        .size(px(TRASH_GLYPH))
                        .text_color(rgb(LABEL)),
                )
                .on_click(cx.listener(|picker, _, _, cx| picker.pick("", cx)))
        });
        div()
            .absolute()
            .top(px(ARROW + SEGMENTS_TOP))
            .left_0()
            .w(px(WIDTH))
            .h(px(SEGMENTS_HEIGHT))
            .flex()
            .justify_center()
            .gap(px(TRASH_GAP))
            .child(
                div()
                    .id("icon-picker-tabs")
                    .when(a11y, |tabs| {
                        tabs.role(Role::TabList).aria_label("Kind of Icon")
                    })
                    .h_full()
                    .flex()
                    .rounded(px(SEGMENTS_RADIUS))
                    .bg(rgba(SEGMENTS_FILL))
                    .children(picking.subject.tabs().iter().map(|&tab| match tab {
                        Tab::Emoji => segment(tab, "Emoji"),
                        Tab::Icon => segment(tab, "Icon"),
                    })),
            )
            .children(trash)
    }

    /// The glyphs, the Space's on a plate.
    fn glyphs(&self, picking: &Picking, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let chosen = match SpaceIcon::parse(&picking.icon) {
            SpaceIcon::Glyph(index) => Some(index),
            _ => None,
        };
        let a11y = self.a11y;
        let cells = GLYPHS.iter().enumerate().map(|(index, &(name, path))| {
            let glyph = svg().path(path).size(px(GLYPH_SIZE)).text_color(rgb(LABEL));
            let chosen = chosen == Some(index);
            cell(("glyph", index), chosen, glyph)
                .when(a11y, |cell| {
                    cell.role(Role::ListBoxOption)
                        .aria_label(name)
                        .aria_selected(chosen)
                })
                .on_click(
                    cx.listener(move |picker, _, _, cx| picker.pick(&space_icon::glyph(index), cx)),
                )
        });
        div()
            .id("icon-picker-glyphs")
            .when(a11y, |grid| grid.role(Role::ListBox).aria_label("Icons"))
            .absolute()
            .top(px(ARROW + GRID_TOP))
            .left(px(GRID_X))
            .w(px(PITCH * COLUMNS as f32))
            .flex()
            .flex_wrap()
            .children(cells)
    }

    /// The search, and under it the emoji in rows that scroll, of which only those in view are laid
    /// out.
    fn emoji_grid(&self, picking: &Picking, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let list = if picking.grid.items.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgba(HINT))
                .child("No emoji found")
                .into_any_element()
        } else {
            uniform_list(
                "icon-picker-emoji",
                picking.grid.rows.len(),
                cx.processor(|picker, rows, _, cx| picker.emoji_rows(rows, cx)),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .into_any_element()
        };
        div()
            .absolute()
            .top(px(ARROW + emoji::SEARCH_TOP))
            .left_0()
            .w(px(WIDTH))
            .bottom_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(GRID_X))
                    .w(px(PITCH * COLUMNS as f32))
                    .h(px(emoji::SEARCH_HEIGHT))
                    .pl(px(emoji::SEARCH_PADDING))
                    .pr(px((emoji::SEARCH_HEIGHT - emoji::TONE_PLATE) / 2.))
                    .flex()
                    .items_center()
                    .rounded(px(emoji::SEARCH_RADIUS))
                    .bg(rgba(SEGMENTS_FILL))
                    .child(div().flex_1().min_w_0().child(self.search.clone()))
                    .child(self.tone_button(picking, cx)),
            )
            .child(
                div()
                    .id("icon-picker-emoji-list")
                    .when(self.a11y, |list| {
                        list.role(Role::ListBox).aria_label("Emoji")
                    })
                    .absolute()
                    .top(px(emoji::LIST_TOP - emoji::SEARCH_TOP))
                    .left(px(GRID_X))
                    .w(px(PITCH * COLUMNS as f32))
                    .bottom(px(emoji::LIST_BOTTOM))
                    .child(list),
            )
            // A click anywhere else on the tab keeps the keyboard in the search.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|picker, _, window, cx| {
                    window.focus(&picker.search.focus_handle(cx), cx)
                }),
            )
    }

    /// The list's rows `rows` as the emoji tab shows them: a header, or up to eight emoji.
    fn emoji_rows(&self, rows: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(picking) = &self.picking else {
            return Vec::new();
        };
        rows.filter_map(|index| picking.grid.rows.get(index))
            .map(|row| match row {
                Row::Header(title) => div()
                    .h(px(PITCH))
                    .flex()
                    .items_end()
                    .pl(px(emoji::HEADER_LEFT))
                    .pb(px(emoji::HEADER_BOTTOM))
                    .text_size(px(emoji::HEADER_TEXT))
                    .text_color(rgba(HINT))
                    .child(*title)
                    .into_any_element(),
                Row::Cells(items) => div()
                    .h(px(PITCH))
                    .flex()
                    .children(items.clone().map(|item| {
                        let emoji = picking.grid.items[item].as_str();
                        let glyph = div().text_size(px(emoji::EMOJI_SIZE)).child(emoji);
                        let selected = picking.selected == Some(item);
                        // The system says an emoji by its name, as its own picker does.
                        cell(("emoji", item), selected, glyph)
                            .when(self.a11y, |cell| {
                                cell.role(Role::ListBoxOption)
                                    .aria_label(emoji)
                                    .aria_selected(selected)
                            })
                            .on_click(
                                cx.listener(move |picker, _, _, cx| picker.pick_emoji(item, cx)),
                            )
                    }))
                    .into_any_element(),
            })
            .collect()
    }
}

/// A grid cell: `content` centred on a plate that's chosen, or lights under the pointer.
fn cell(id: (&'static str, usize), chosen: bool, content: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(PITCH))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("plate")
                .size(px(PLATE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(PLATE_RADIUS))
                .when(chosen, |plate| plate.bg(rgba(CHOSEN)))
                .when(!chosen, |plate| {
                    plate
                        .hover(|plate| plate.bg(Palette::on_dark(STATE_HOVERED)))
                        .active(|plate| plate.bg(Palette::on_dark(STATE_PRESSED)))
                })
                .child(content),
        )
}

/// The place in `grid` of the emoji `icon` is, if it is one.
fn current(icon: &str, grid: &Grid) -> Option<usize> {
    match SpaceIcon::parse(icon) {
        SpaceIcon::Emoji(emoji) => grid.position(emoji),
        _ => None,
    }
}

/// The outline of the shape with `corners` (x, y and radius, clockwise) `inset` inside its edge:
/// each corner moved in along both its sides, and its radius smaller where it bulges out and
/// larger where it bends in.
fn inset_corners(corners: &[(f32, f32, f32)], inset: f32) -> Vec<(Point<f32>, f32, bool)> {
    let count = corners.len();
    let at = |index: usize| {
        let (x, y, _) = corners[index % count];
        point(x, y)
    };
    let unit = |from: Point<f32>, to: Point<f32>| {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let length = (dx * dx + dy * dy).sqrt();
        point(dx / length, dy / length)
    };
    (0..count)
        .map(|index| {
            let (before, here, after) = (at(index + count - 1), at(index), at(index + 1));
            let (along_in, along_out) = (unit(before, here), unit(here, after));
            // Clockwise on screen, each side's inside is to its right.
            let normal_in = point(-along_in.y, along_in.x);
            let normal_out = point(-along_out.y, along_out.x);
            let scale = inset / (1. + normal_in.x * normal_out.x + normal_in.y * normal_out.y);
            let moved = point(
                here.x + (normal_in.x + normal_out.x) * scale,
                here.y + (normal_in.y + normal_out.y) * scale,
            );
            let convex = along_in.x * along_out.y - along_in.y * along_out.x > 0.;
            let radius = corners[index].2;
            let radius = if radius == 0. {
                0.
            } else if convex {
                (radius - inset).max(0.)
            } else {
                radius + inset
            };
            (moved, radius, convex)
        })
        .collect()
}

/// The outline of the shape with `corners` `inset` inside its edge, at `origin`, into `path`, as
/// the host's blur and shadow take the shape.
pub(crate) fn outline(
    path: &mut PathBuilder,
    origin: Point<Pixels>,
    corners: &[(f32, f32, f32)],
    inset: f32,
) {
    let to = |at: Point<f32>| origin + point(px(at.x), px(at.y));
    trace(corners, inset, |step| match step {
        Step::Move(at) => path.move_to(to(at)),
        Step::Line(at) => path.line_to(to(at)),
        Step::Arc(radius, convex, at) => {
            path.arc_to(point(px(radius), px(radius)), px(0.), false, convex, to(at))
        }
        Step::Close => path.close(),
    });
}

/// The outline of the shape with `corners`, moved by `x` and `y`, as an SVG path's data, for a
/// host that draws its shadow from a picture (platform/linux/shadows.rs).
pub fn outline_svg(corners: &[(f32, f32, f32)], x: f32, y: f32) -> String {
    let mut data = String::new();
    trace(corners, 0., |step| {
        data += &match step {
            Step::Move(at) => format!("M{} {}", at.x + x, at.y + y),
            Step::Line(at) => format!("L{} {}", at.x + x, at.y + y),
            Step::Arc(radius, convex, at) => {
                format!(
                    "A{radius} {radius} 0 0 {} {} {}",
                    u8::from(convex),
                    at.x + x,
                    at.y + y
                )
            }
            Step::Close => "Z".into(),
        }
    });
    data
}

/// A step along an outline: to a point, a line to one, an arc of a radius to one, clockwise if
/// the corner bulges out, or back to the start.
enum Step {
    Move(Point<f32>),
    Line(Point<f32>),
    Arc(f32, bool, Point<f32>),
    Close,
}

/// The steps round the shape with `corners` `inset` inside its edge, each corner rounded by its
/// radius.
fn trace(corners: &[(f32, f32, f32)], inset: f32, mut step: impl FnMut(Step)) {
    let corners = inset_corners(corners, inset);
    let count = corners.len();
    for index in 0..count {
        let (here, radius, convex) = corners[index];
        let before = corners[(index + count - 1) % count].0;
        let after = corners[(index + 1) % count].0;
        let toward = |other: Point<f32>, by: f32| {
            let (dx, dy) = (other.x - here.x, other.y - here.y);
            let length = (dx * dx + dy * dy).sqrt();
            point(here.x + dx / length * by, here.y + dy / length * by)
        };
        // Where the corner's arc meets its sides: as far along each as the radius over the tangent
        // of half the angle between them.
        let (dx0, dy0) = (before.x - here.x, before.y - here.y);
        let (dx1, dy1) = (after.x - here.x, after.y - here.y);
        let lengths = ((dx0 * dx0 + dy0 * dy0) * (dx1 * dx1 + dy1 * dy1)).sqrt();
        let cos = (dx0 * dx1 + dy0 * dy1) / lengths;
        let half = cos.clamp(-1., 1.).acos() / 2.;
        let along = if radius > 0. { radius / half.tan() } else { 0. };
        let (start, end) = (toward(before, along), toward(after, along));
        step(if index == 0 {
            Step::Move(start)
        } else {
            Step::Line(start)
        });
        if radius > 0. {
            step(Step::Arc(radius, convex, end));
        }
    }
    step(Step::Close);
}

/// A popover's card through `corners` at `origin`: its fill, and the modal level's rim round it.
pub(crate) fn paint_card(
    origin: Point<Pixels>,
    corners: &[(f32, f32, f32)],
    color: Rgba,
    window: &mut Window,
) {
    let mut fill = PathBuilder::fill();
    outline(&mut fill, origin, corners, 0.);
    if let Ok(path) = fill.build() {
        window.paint_path(path, color);
    }
    // The modal level's rim, 1 pt inside the edge; its outline and shadow are the host's.
    let mut edge = PathBuilder::stroke(px(1.));
    outline(&mut edge, origin, corners, 0.5);
    if let Ok(path) = edge.build() {
        window.paint_path(path, Level::Modal.rim());
    }
}

impl Focusable for IconPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for IconPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        let root = div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI);
        // The slide's progress, while the content slides.
        let slide = self.picking.as_mut().and_then(|picking| {
            let (from, start) = picking.slide?;
            let progress = start.elapsed().as_secs_f32() / SLIDE;
            if progress >= 1. {
                picking.slide = None;
                return None;
            }
            window.request_animation_frame();
            Some((from, progress))
        });
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, as the theme picker's.
        if let Some(picking) = self.picking.as_mut().filter(|picking| !picking.drawn) {
            if window.is_visible() {
                picking.drawn = true;
                // check.sh waits for this line.
                log!("browser: icon picker drawn");
            } else {
                window.request_animation_frame();
            }
        }
        let Some(picking) = &self.picking else {
            return root;
        };
        let card_fill = crate::palette::current(cx).card_drawn();
        let card = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| paint_card(bounds.origin, &corners(), card_fill, window),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let mut content = |tab, x: f32| {
            let pane = match tab {
                Tab::Icon => self.glyphs(picking, cx).into_any_element(),
                Tab::Emoji => self.emoji_grid(picking, cx).into_any_element(),
            };
            div().absolute().top_0().left(px(x)).size_full().child(pane)
        };
        let content = match slide {
            Some((from, progress)) => {
                let (old, new) = slid(from, picking.tab, progress);
                vec![content(from, old), content(picking.tab, new)]
            }
            None => vec![content(picking.tab, 0.)],
        };
        let top = self.top(picking, cx);
        root.child(
            div()
                .id("icon-picker")
                .when(self.a11y, |picker| {
                    picker
                        .role(Role::Dialog)
                        .aria_label(picking.subject.picker_name())
                })
                .key_context("IconPicker")
                .track_focus(&self.focus)
                .on_action(cx.listener(Self::dismiss))
                .on_action(cx.listener(Self::keep))
                .on_action(cx.listener(Self::up))
                .on_action(cx.listener(Self::down))
                .capture_action(cx.listener(Self::left))
                .capture_action(cx.listener(Self::right))
                .relative()
                .size_full()
                .text_color(rgb(LABEL))
                .text_size(px(text::BODY))
                .font_weight(FontWeight::MEDIUM)
                .child(card)
                .child(top)
                .children(content),
        )
    }
}

impl IconPicker {
    /// What it's picking for, and the tab it shows, while it shows.
    pub fn picking(&self) -> Option<(Subject, Tab)> {
        self.picking
            .as_ref()
            .map(|picking| (picking.subject, picking.tab))
    }
}

#[cfg(test)]
mod tests;
