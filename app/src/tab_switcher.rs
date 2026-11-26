//! The switcher a held ⌃Tab shows (#2954): the Space's tabs, most recently shown first, as ⌘T's
//! panel in ⌘T's place, with its rows and no field. The shell decides what it lists, which row is
//! selected and what each key or click does; this draws them and tells it what the user did. It
//! floats over the page, which GPUI can't draw over, so it's a window of its own that the host
//! shows over the browser window with the keyboard, as the command bar's: the keys a walk takes go
//! to it, never to the page. What it asks of its window goes through `Host`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, Focusable, FontWeight, Global,
    HoverListenerMode, Hsla, KeyBinding, MouseButton, MouseDownEvent, ScrollHandle, SharedString,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, actions, div, img, linear_color_stop, linear_gradient, point, prelude::*, px,
    size, svg,
};

use crate::dismissal::{By, Close, Why};
use crate::elevation::Level;
use crate::hover_card::Icon;
use crate::palette::{self, Float, Palette};
use crate::shell::KEY_CONTEXT;
pub use crate::tokens::command_bar::{ABOVE_MIDDLE, RADIUS, WIDTH};
use crate::tokens::command_bar::{
    CHIP_RADIUS, DIM, FIELD_FADE, HOVERED, ICON_BOX, ICON_BOX_X, ICON_SIZE, LIST_X, PLATE_INSET,
    PLATE_RADIUS, ROW_END, ROW_GLYPH, ROW_HEIGHT, SELECTED_DIM, TITLE, TITLE_X,
};
use crate::tokens::tab_switcher::ROWS;
use crate::tokens::text;

actions!(tab_switcher, [Next, Back, Stay]);

/// How long ⌃ is held after ⌃Tab's press before the switcher shows (tokens.toml).
pub const DELAY: std::time::Duration =
    std::time::Duration::from_millis((crate::tokens::tab_switcher::motion::DELAY * 1000.) as u64);

/// Between a row's title and its host, as in ⌘T's rows.
const TITLE_GAP: f32 = 3.;

/// What the switcher asks of its window.
pub trait Host {
    /// Shows the switcher's window, `height` tall, over browser window `window`, where ⌘T's panel
    /// shows, with the keyboard; or makes it that tall if it shows.
    fn show(&self, window: u64, height: f32);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
}

/// A row the switcher lists: one tab, or a split view's, which counts once.
#[derive(Clone)]
pub struct Item {
    /// The tab it shows: a split's pane shown last.
    pub tab: u64,
    /// The tab's title, or a split's name, else its pages' titles joined by commas.
    pub title: SharedString,
    /// The page's host after the title, as ⌘T's tab rows have it, or empty.
    pub host: SharedString,
    pub icon: Icon,
    /// A split's glyph, in place of the icon when it has no emoji of its own.
    pub glyph: Option<&'static str>,
    /// Not loaded since launch: its favicon dims, as its row's does.
    pub sleeping: bool,
    /// The sidebar's speaker while its page plays sound or is muted.
    pub speaker: Option<&'static str>,
}

/// What the user did in the switcher, which the shell hears after the event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heard {
    /// Tab or →, else ⇧Tab or ←: the selection goes on or back, wrapping.
    Step(bool),
    /// A press on a row: its tab shows.
    Chose(usize),
    /// Escape, or a click outside it: it closes on the tab the walk started from.
    Stay,
    /// The user left the app: it closes as letting go of ⌃ does.
    LetGo,
}

/// The switcher's list over browser window `window`, and who hears what the user does in it.
#[derive(Clone)]
pub struct Show {
    pub window: u64,
    pub items: Vec<Item>,
    pub selected: usize,
    pub heard: Rc<dyn Fn(Heard, &mut App)>,
}

/// What the shell asks of the switcher.
#[derive(Clone)]
pub enum Ask {
    Show(Show),
    /// The selection moved to this row.
    Select(usize),
    Close,
}

impl Ask {
    /// How the recorder's log names it.
    pub fn label(&self) -> String {
        match self {
            Ask::Show(show) => {
                let tabs: Vec<String> =
                    show.items.iter().map(|item| item.tab.to_string()).collect();
                format!("tab switcher show {} at {}", tabs.join(" "), show.selected)
            }
            Ask::Select(at) => format!("tab switcher select {at}"),
            Ask::Close => "tab switcher close".into(),
        }
    }
}

/// How tall the switcher of `count` rows is: ROWS of them at most, inside the list's inset.
pub fn height(count: usize) -> f32 {
    count.clamp(1, ROWS as usize) as f32 * ROW_HEIGHT + 2. * LIST_X
}

/// How far down a list scrolled `scrolled` down goes to show row `at` whole, the least it can, with
/// `shown` rows in view.
pub fn scrolled_to(at: usize, scrolled: f32, shown: usize) -> f32 {
    let top = at as f32 * ROW_HEIGHT;
    let view = shown as f32 * ROW_HEIGHT;
    scrolled.min(top).max(top + ROW_HEIGHT - view)
}

pub struct TabSwitcher {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    shown: Option<Show>,
    /// The row under the pointer, which takes the hovered plate while the selection is elsewhere.
    hovered: Option<usize>,
    /// Whether a frame of this walk's switcher has been drawn on screen.
    drawn: bool,
}

struct SwitcherWindow(WindowHandle<TabSwitcher>);

impl Global for SwitcherWindow {}

/// Opens the switcher's window, hidden, for `host` to show over the browser window.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<TabSwitcher>> {
    // ⌃ is held while it shows, so the keys come with it, or without where a system takes them.
    cx.bind_keys([
        KeyBinding::new("ctrl-tab", Next, Some("TabSwitcher")),
        KeyBinding::new("tab", Next, Some("TabSwitcher")),
        KeyBinding::new("ctrl-right", Next, Some("TabSwitcher")),
        KeyBinding::new("right", Next, Some("TabSwitcher")),
        KeyBinding::new("ctrl-shift-tab", Back, Some("TabSwitcher")),
        KeyBinding::new("shift-tab", Back, Some("TabSwitcher")),
        KeyBinding::new("ctrl-left", Back, Some("TabSwitcher")),
        KeyBinding::new("left", Back, Some("TabSwitcher")),
        KeyBinding::new("ctrl-escape", Stay, Some("TabSwitcher")),
        KeyBinding::new("escape", Stay, Some("TabSwitcher")),
    ]);
    let window = Level::Modal
        .panel_margin()
        .around(size(px(WIDTH), px(height(1))));
    let bounds = Bounds::new(point(px(0.), px(0.)), window);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The panel is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| cx.new(|cx| TabSwitcher::new(host, cx)));
    match window {
        Ok(window) => {
            cx.set_global(SwitcherWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the tab switcher's window: {error}");
            None
        }
    }
}

fn update(cx: &mut App, f: impl FnOnce(&mut TabSwitcher, &mut Window, &mut Context<TabSwitcher>)) {
    if let Some(window) = cx.try_global::<SwitcherWindow>().map(|switcher| switcher.0) {
        window
            .update(cx, |switcher, window, cx| f(switcher, window, cx))
            .ok();
    }
}

/// What the shell asks. The window is made the first time it shows.
pub fn ask(ask: Ask, cx: &mut App) {
    if matches!(ask, Ask::Show(_)) {
        crate::lazy_window::ready::<TabSwitcher>(cx);
    }
    update(cx, |switcher, window, cx| switcher.ask(ask, window, cx));
}

/// The host hid the switcher as the user left it: a click outside it stays on the tab the walk
/// started from, and leaving the app ends the walk as letting go of ⌃ does.
pub fn dismissed(cx: &mut App) {
    let close = Close::left(cx.active_window().is_none());
    update(cx, |switcher, _, cx| {
        switcher.hovered = None;
        if let Some(show) = switcher.shown.take() {
            log!("browser: tab switcher dismissed");
            cx.notify();
            (show.heard)(Heard::closed(close), cx);
        }
    });
}

impl Heard {
    /// What the shell hears of a close: the walk ends where it started, unless the user left the
    /// app, which ends it as letting go of ⌃ does.
    fn closed(close: Close) -> Self {
        match close.why {
            Why::WindowLeft => Heard::LetGo,
            _ => Heard::Stay,
        }
    }
}

impl TabSwitcher {
    pub fn new(host: Rc<dyn Host>, cx: &mut Context<Self>) -> Self {
        TabSwitcher {
            host,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            shown: None,
            hovered: None,
            drawn: false,
        }
    }

    /// The tabs it lists and the one selected, while it shows.
    pub fn showing(&self) -> Option<(Vec<u64>, u64)> {
        let shown = self.shown.as_ref()?;
        let tabs: Vec<u64> = shown.items.iter().map(|item| item.tab).collect();
        Some((tabs.clone(), tabs[shown.selected]))
    }

    pub fn ask(&mut self, ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        match ask {
            Ask::Show(show) if show.items.is_empty() => self.close(cx),
            Ask::Show(show) => {
                let (over, count) = (show.window, show.items.len());
                let tabs: Vec<String> =
                    show.items.iter().map(|item| item.tab.to_string()).collect();
                // check.sh reads this line.
                log!(
                    "browser: tab switcher lists {} selected {}",
                    tabs.join(" "),
                    show.items[show.selected.min(count - 1)].tab
                );
                let selected = show.selected.min(count - 1);
                self.drawn &= self.shown.is_some();
                self.shown = Some(Show { selected, ..show });
                self.hovered = None;
                self.scroll.set_offset(point(px(0.), px(0.)));
                self.scroll_to(selected);
                window.focus(&self.focus, cx);
                self.host.show(over, height(count));
                cx.notify();
            }
            Ask::Select(at) => {
                let Some(shown) = &mut self.shown else {
                    return;
                };
                let at = at.min(shown.items.len() - 1);
                if shown.selected == at {
                    return;
                }
                shown.selected = at;
                // check.sh reads this line.
                log!("browser: tab switcher selected {}", shown.items[at].tab);
                self.scroll_to(at);
                cx.notify();
            }
            Ask::Close => self.close(cx),
        }
    }

    /// Scrolls the list the least that shows row `at` whole.
    fn scroll_to(&mut self, at: usize) {
        let Some(shown) = &self.shown else {
            return;
        };
        let rows = shown.items.len().min(ROWS as usize);
        let scrolled = -f32::from(self.scroll.offset().y);
        let to = scrolled_to(at, scrolled, rows);
        self.scroll.set_offset(point(px(0.), px(-to)));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.hovered = None;
        if self.shown.take().is_some() {
            self.host.hide();
            cx.notify();
        }
    }

    fn heard(&self, heard: Heard, cx: &mut Context<Self>) {
        if let Some(shown) = &self.shown {
            (shown.heard)(heard, cx);
        }
    }

    fn row(&self, at: usize, item: &Item, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(shown) = &self.shown else {
            return div().into_any_element();
        };
        let selected = at == shown.selected;
        // The selected row keeps its accent under the pointer, as ⌘T's does.
        let hovered = !selected && self.hovered == Some(at);
        let (accent, label) = (Hsla::from(palette.accent), Hsla::from(palette.accent_label));
        let (title, dim, glyph) = match selected {
            true => (label, label.opacity(SELECTED_DIM), label),
            false => (
                palette.on_overlay(TITLE),
                palette.on_overlay(DIM),
                palette.on_overlay(ROW_GLYPH),
            ),
        };
        // The icon sits on a tile of the accent's label in the selected row, as in ⌘T's.
        let icon = div()
            .size(px(ICON_BOX))
            .mr(px(TITLE_X - ICON_BOX_X - ICON_BOX))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .when(selected, |icon| icon.bg(label))
            .when(item.sleeping, |icon| {
                icon.opacity(crate::tokens::shell::SLEEPING_FAVICON)
            })
            .child(match item.glyph {
                Some(glyph) => svg()
                    .path(glyph)
                    .size(px(ICON_SIZE))
                    .text_color(if selected { accent } else { glyph_ink(palette) })
                    .into_any_element(),
                None => icon(palette, &item.icon),
            });
        let tab = at;
        // The plate sits in its row's padding, which the list's scroll counts, as margins aren't.
        let plate = div()
            .id(("tab-switcher-row", at))
            .size_full()
            .flex()
            .items_center()
            .pl(px(ICON_BOX_X - PLATE_INSET))
            .pr(px(ROW_END))
            .rounded(px(PLATE_RADIUS))
            .text_color(title)
            .when(selected, |line| line.bg(accent))
            .when(hovered, |line| line.bg(palette.plate(HOVERED)))
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(move |switcher, hovered: &bool, _, cx| {
                let was = switcher.hovered;
                switcher.hovered = match hovered {
                    true => Some(tab),
                    false => switcher.hovered.filter(|&at| at != tab),
                };
                if was != switcher.hovered {
                    cx.notify();
                }
            }))
            // On the press, not the click: on the Mac a press ends the walk too, and that's heard
            // after the press (host_watch_control).
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |switcher, _, _, cx| {
                    cx.stop_propagation();
                    switcher.heard(Heard::Chose(tab), cx);
                }),
            )
            // ⌃ is held while the switcher shows, and GPUI hands the Mac's ⌃-left press on as a
            // right press with ⌃ cleared, so that's the user's click too (#3360). A right press
            // has ⌃ set everywhere else.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |switcher, event: &MouseDownEvent, _, cx| {
                    if !event.modifiers.control {
                        cx.stop_propagation();
                        switcher.heard(Heard::Chose(tab), cx);
                    }
                }),
            )
            .child(icon)
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .gap(px(TITLE_GAP))
                    .child(div().min_w_0().truncate().child(item.title.clone()))
                    // The host gives way first, so a long one, a data: URL's, never hides the
                    // title, which shrank as much as it when both could.
                    .when(!item.host.is_empty(), |line| {
                        let host = format!("— {}", item.host);
                        let shrinks_first = div().min_w_0().flex_shrink(1000.).truncate();
                        line.child(shrinks_first.text_color(dim).child(host))
                    }),
            )
            .when_some(item.speaker, |line, speaker| {
                line.child(div().flex_1()).child(
                    svg()
                        .path(speaker)
                        .flex_none()
                        .size(px(ICON_SIZE))
                        .text_color(glyph),
                )
            });
        div()
            .h(px(ROW_HEIGHT))
            .flex_none()
            .p(px(PLATE_INSET))
            .child(plate)
            .into_any_element()
    }
}

/// The ink of a glyph standing for a tab's icon, as the sidebar's globe.
fn glyph_ink(palette: &Palette) -> Hsla {
    palette.on_overlay_glyph(ROW_GLYPH)
}

/// A tab's icon, at the row's icon size: its emoji, else its favicon, else the globe.
fn icon(palette: &Palette, icon: &Icon) -> AnyElement {
    match icon {
        Icon::Favicon(Some(favicon)) => img(favicon.clone()).size(px(ICON_SIZE)).into_any_element(),
        Icon::Favicon(None) => svg()
            .path("icons/globe.svg")
            .size(px(ICON_SIZE))
            .text_color(glyph_ink(palette))
            .into_any_element(),
        Icon::Emoji(emoji) => div()
            .text_size(px(ICON_SIZE))
            .line_height(px(ICON_SIZE))
            .child(emoji.clone())
            .into_any_element(),
    }
}

impl Focusable for TabSwitcher {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TabSwitcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI);
        let Some(shown) = &self.shown else {
            return root;
        };
        // The host puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, under Wayland after the host says it showed.
        if !self.drawn {
            if window.is_visible() {
                self.drawn = true;
                // check.sh waits for this line.
                log!("browser: tab switcher drawn");
            } else {
                window.request_animation_frame();
            }
        }
        let palette = palette::current(cx);
        let count = shown.items.len();
        let rows: Vec<AnyElement> = (shown.items.clone().iter().enumerate())
            .map(|(at, item)| self.row(at, item, &palette, cx))
            .collect();
        let shown_rows = count.min(ROWS as usize);
        // A cut edge fades into the panel, as ⌘T's field does.
        let scrolled = -f32::from(self.scroll.offset().y);
        let below = count as f32 * ROW_HEIGHT - shown_rows as f32 * ROW_HEIGHT - scrolled;
        let fade = |top: bool| {
            let panel = Hsla::from(Float::TabSwitcher.fill(palette.tone));
            let (from, to) = (panel, panel.opacity(0.));
            let (start, end) = if top { (from, to) } else { (to, from) };
            div()
                .absolute()
                .left_0()
                .right_0()
                .h(px(FIELD_FADE))
                .when(top, |fade| fade.top_0())
                .when(!top, |fade| fade.bottom_0())
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(start, 0.),
                    linear_color_stop(end, 1.),
                ))
        };
        let list = div()
            .relative()
            .mx(px(LIST_X))
            .my(px(LIST_X))
            .h(px(shown_rows as f32 * ROW_HEIGHT))
            .child(
                div()
                    .id("tab-switcher-rows")
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(rows),
            )
            .when(scrolled > 0.5, |list| list.child(fade(true)))
            .when(below > 0.5, |list| list.child(fade(false)));
        // The rim is its own layer, so the border doesn't shift what the fill's box holds.
        let rim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(RADIUS))
            .border(px(1.))
            .border_color(Level::Modal.rim_in(&palette));
        root.child(
            Level::Modal.in_window(
                div()
                    .id("tab-switcher")
                    .key_context("TabSwitcher")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|switcher, _: &Next, _, cx| {
                        switcher.heard(Heard::Step(true), cx)
                    }))
                    .on_action(cx.listener(|switcher, _: &Back, _, cx| {
                        switcher.heard(Heard::Step(false), cx)
                    }))
                    .on_action(cx.listener(|switcher, _: &Stay, _, cx| {
                        let escape = Close::new(Why::Escape, By::Keyboard);
                        switcher.heard(Heard::closed(escape), cx)
                    }))
                    .relative()
                    .w(px(WIDTH))
                    .h(px(height(count)))
                    .overflow_hidden()
                    .rounded(px(RADIUS))
                    .bg(Float::TabSwitcher.fill(palette.tone))
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(text::BODY))
                    .child(list)
                    .child(rim),
            ),
        )
    }
}

#[cfg(test)]
mod tests;
