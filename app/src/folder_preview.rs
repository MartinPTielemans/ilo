//! The folder preview, Arc's (#690, #1405): resting the pointer on a closed folder's row shows a
//! list of the tabs in it beside the row, with those of the folders inside it, by the titles the
//! sidebar gives them, the one seen last first, each over how long ago that was, under a search.
//! The first is selected, so ↩ shows it. Typing narrows the list and selects the first match, ↑
//! and ↓ move the selection, and ↩ or a click shows the tab, which then peeks under the closed
//! folder, as the tab shown does, without opening it. It crosses the sidebar's edge onto the page,
//! which GPUI can't draw over, so it's a window of its own that the host shows over the browser
//! window, as its child, with the keyboard for the search. Until something is typed in it, it goes
//! once the pointer has left it and its row; after that, with escape, a choice or a click outside
//! it. What it asks of its window goes through `Host`.

use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight, Global,
    KeyBinding, MouseButton, ScrollStrategy, SharedString, Subscription, Task, TitlebarOptions,
    UniformListScrollHandle, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, actions, div, img, point, prelude::*, px, size, svg, uniform_list,
};

use crate::elevation::Level;
use crate::hover_card::{Anchor, Icon};
use crate::palette::{Palette, STATE_SELECTED};
use crate::shell::KEY_CONTEXT;
use crate::text_input::TextInput;
use crate::tokens::folder_preview::{
    AGE_LINE, AGE_SIZE, DIVIDER, DIVIDER_ALPHA, HINT, ICON, ICON_X, PADDING, RIM, ROW, ROW_FILL,
    ROW_RADIUS, SEARCH, SEARCH_GLYPH, TALLEST, TITLE, TITLE_LINE, TITLE_SIZE, TITLE_X,
};
pub use crate::tokens::folder_preview::{RADIUS, WIDTH};
use crate::tokens::shell::PLATE_INSET;

actions!(folder_preview, [Dismiss, Choose, Up, Down]);

/// Arc 1.171's timing (outside measurement, #1405): the preview starts 0.45 s after the pointer
/// comes onto the folder's row, and goes within 40 ms of it leaving both, fading out over 80 ms.
/// It comes in over 90 ms, fading in as it grows from 0.85 of its size about the middle of its left
/// edge. The host grows and fades its window, as the host draws its blur and shadow.
pub const DWELL: Duration = Duration::from_millis(450);
pub const LEAVE: Duration = Duration::from_millis(40);
pub const FROM: f32 = 0.85;
pub const IN: Duration = Duration::from_millis(90);
pub const OUT: Duration = Duration::from_millis(80);
/// It's 4 right of the sidebar, and kept 18 inside the window (Arc 1.171).
const BESIDE: f32 = 4.;
const INSIDE: f32 = 18.;

/// What the preview asks of its window.
pub trait Host {
    /// Shows the preview's window, `height` tall, with its top left at (`left`, `top`) in browser
    /// window `window`, the one whose row it's for, with the keyboard, or moves it there if it
    /// shows. It comes in over IN, growing from `from` of its size, and goes over OUT.
    fn show(&self, window: u64, left: f32, top: f32, height: f32, from: f32);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
}

/// A tab the preview lists.
#[derive(Clone)]
pub struct Item {
    pub tab: u64,
    /// What its row in the sidebar says, the name the user gave it if they did.
    pub title: SharedString,
    pub url: SharedString,
    pub icon: Icon,
    /// How long ago a window last showed it, in words, or empty when that isn't known.
    pub age: SharedString,
}

/// The pointer rests on a closed folder's row: in which browser window, what its preview lists,
/// where, and who hears which tab the user chose.
#[derive(Clone)]
pub struct Rest {
    /// The browser window whose sidebar has the row, which the preview shows over (#1432).
    pub window: u64,
    pub folder: u64,
    pub name: SharedString,
    pub items: Vec<Item>,
    pub anchor: Anchor,
    pub chosen: Rc<dyn Fn(u64, &mut App)>,
}

/// What the shell asks of the preview.
#[derive(Clone)]
pub enum Ask {
    /// The pointer has come onto a closed folder's row.
    Rest(Rest),
    /// It has left that folder's row.
    Left(u64),
    /// A press, a drag, a menu or a Space switch ends the preview at once.
    Close,
}

impl Ask {
    /// How the recorder's log names it.
    pub fn label(&self) -> String {
        match self {
            Ask::Rest(rest) => {
                let tabs: Vec<String> =
                    rest.items.iter().map(|item| item.tab.to_string()).collect();
                format!("folder preview rest {} {}", rest.folder, tabs.join(" "))
            }
            Ask::Left(folder) => format!("folder preview left {folder}"),
            Ask::Close => "folder preview close".into(),
        }
    }
}

/// Those of `items` whose title or address has `query` in it, ignoring case, by where they are
/// among them, in their order: all of them for an empty query.
pub fn matches(items: &[Item], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let has = |text: &SharedString| text.to_lowercase().contains(&query);
    (0..items.len())
        .filter(|&at| query.is_empty() || has(&items[at].title) || has(&items[at].url))
        .collect()
}

/// How tall the preview of `count` tabs is: the search and the line under it, then a row for each,
/// with the padding above the first's fill and under the last's, TALLEST at most, past which the
/// list scrolls. It keeps that height as a search narrows the list.
pub fn height(count: usize) -> f32 {
    let rows = count.max(1) as f32 * ROW - (ROW - ROW_FILL);
    (SEARCH + DIVIDER + 2. * PADDING + rows).min(TALLEST)
}

/// Where a preview `height` tall has its top left: BESIDE right of the sidebar, whose edge is
/// PLATE_INSET past its row's plate, or as far left of a sidebar on the right, centred on the row,
/// and INSIDE the window, the top and left first if it can't be both.
pub fn place(anchor: Anchor, height: f32) -> (f32, f32) {
    let (width, tall) = (
        f32::from(anchor.window.width),
        f32::from(anchor.window.height),
    );
    let left = match anchor.left {
        true => f32::from(anchor.row.left()) - PLATE_INSET - BESIDE - WIDTH,
        false => f32::from(anchor.row.right()) + PLATE_INSET + BESIDE,
    };
    let top = (f32::from(anchor.row.center().y) - height / 2.).round();
    (
        left.min(width - WIDTH - INSIDE).max(INSIDE),
        top.min(tall - height - INSIDE).max(INSIDE),
    )
}

/// The preview that shows.
struct Shown {
    rest: Rest,
    query: String,
    /// The items the search finds, by where they are among the rest's, and the one selected among
    /// those, which ↩ shows.
    found: Vec<usize>,
    selected: Option<usize>,
    /// Something was typed or a key moved the selection, so the pointer leaving doesn't end it.
    kept: bool,
    height: f32,
    _edit: Subscription,
}

pub struct FolderPreview {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    search: Entity<TextInput>,
    scroll: UniformListScrollHandle,
    shown: Option<Shown>,
    /// The folder the pointer rests on whose preview waits for its dwell.
    resting: Option<Rest>,
    /// The dwell's timer, or the leave's once the pointer has left the shown preview's row.
    /// Dropping it cancels it.
    wait: Option<Task<()>>,
    /// The pointer is on the preview, which keeps it.
    on_card: bool,
    /// The pointer is on the shown preview's row, which keeps it too.
    on_row: bool,
}

struct PreviewWindow(WindowHandle<FolderPreview>);

impl Global for PreviewWindow {}

/// Opens the preview's window, hidden, for `host` to show over the browser window, ahead of time
/// as the icon picker's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<FolderPreview>> {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("FolderPreview")),
        KeyBinding::new("enter", Choose, Some("FolderPreview")),
        KeyBinding::new("up", Up, Some("FolderPreview")),
        KeyBinding::new("down", Down, Some("FolderPreview")),
    ]);
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(TALLEST)));
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
        // The card is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        let search = cx.new(|cx| TextInput::new("Search", cx));
        cx.new(|cx| FolderPreview {
            host,
            focus: cx.focus_handle(),
            search,
            scroll: UniformListScrollHandle::new(),
            shown: None,
            resting: None,
            wait: None,
            on_card: false,
            on_row: false,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(PreviewWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the folder preview's window: {error}");
            None
        }
    }
}

fn update(
    cx: &mut App,
    f: impl FnOnce(&mut FolderPreview, &mut Window, &mut Context<FolderPreview>),
) {
    if let Some(window) = cx.try_global::<PreviewWindow>().map(|preview| preview.0) {
        window
            .update(cx, |preview, window, cx| f(preview, window, cx))
            .ok();
    }
}

/// What the shell asks: the preview of the closed folder the pointer rests on, after DWELL, or at
/// once while another shows; the pointer leaving that row, which ends it after LEAVE unless it's
/// onto the preview or something was typed in it; or its end.
pub fn ask(ask: Ask, cx: &mut App) {
    if matches!(ask, Ask::Rest(_)) {
        crate::lazy_window::ready::<FolderPreview>(cx);
    }
    update(cx, |preview, window, cx| match ask {
        Ask::Rest(rest) => preview.rest(rest, window, cx),
        Ask::Left(folder) => preview.left(folder, cx),
        Ask::Close => preview.close(cx),
    });
}

/// The user left the preview without choosing a tab, such as with a click outside it, and the
/// host has hidden it.
pub fn dismissed(cx: &mut App) {
    update(cx, |preview, _, cx| {
        preview.wait = None;
        preview.on_card = false;
        preview.on_row = false;
        if preview.shown.take().is_some() {
            log!("browser: folder preview dismissed");
            cx.notify();
        }
    });
}

impl FolderPreview {
    /// The folder whose preview shows, the tabs it lists by what the search finds, and the one
    /// selected.
    pub fn showing(&self) -> Option<(u64, Vec<u64>, Option<u64>)> {
        let shown = self.shown.as_ref()?;
        let tab = |at: usize| shown.rest.items[shown.found[at]].tab;
        let listed = (0..shown.found.len()).map(tab).collect();
        Some((shown.rest.folder, listed, shown.selected.map(tab)))
    }

    fn rest(&mut self, rest: Rest, window: &mut Window, cx: &mut Context<Self>) {
        if rest.items.is_empty() {
            return;
        }
        if let Some(shown) = &self.shown {
            if shown.rest.folder == rest.folder {
                self.wait = None;
                self.on_row = true;
                return;
            }
            self.resting = None;
            self.wait = None;
            return self.show(rest, window, cx);
        }
        let waiting = self
            .resting
            .as_ref()
            .is_some_and(|resting| resting.folder == rest.folder);
        self.resting = Some(rest);
        if waiting {
            return;
        }
        self.wait = Some(cx.spawn_in(window, async move |preview, cx| {
            cx.background_executor().timer(DWELL).await;
            preview
                .update_in(cx, |preview, window, cx| {
                    preview.wait = None;
                    if let Some(rest) = preview.resting.take() {
                        preview.show(rest, window, cx)
                    }
                })
                .ok();
        }));
    }

    fn left(&mut self, folder: u64, cx: &mut Context<Self>) {
        if self
            .resting
            .as_ref()
            .is_some_and(|rest| rest.folder == folder)
        {
            self.resting = None;
            self.wait = None;
        }
        let leaves = self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.rest.folder == folder);
        if leaves {
            self.on_row = false;
            if !self.on_card {
                self.leave_after(cx);
            }
        }
    }

    /// The preview goes after LEAVE, unless the pointer comes onto it or its row first, or
    /// something is typed in it.
    fn leave_after(&mut self, cx: &mut Context<Self>) {
        if self.shown.as_ref().is_some_and(|shown| shown.kept) {
            return;
        }
        self.wait = Some(cx.spawn(async move |preview, cx| {
            cx.background_executor().timer(LEAVE).await;
            preview
                .update(cx, |preview, cx| {
                    preview.wait = None;
                    preview.close(cx)
                })
                .ok();
        }));
    }

    /// The pointer came onto the preview or left it. Leaving it onto its row keeps it, as does
    /// the window, as it comes up, hearing the pointer come and go at the place it last had.
    fn on_card(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.on_card = hovered && self.shown.is_some();
        match hovered {
            true => self.wait = None,
            false if self.shown.is_some() && !self.on_row => self.leave_after(cx),
            false => {}
        }
    }

    fn show(&mut self, rest: Rest, window: &mut Window, cx: &mut Context<Self>) {
        let height = height(rest.items.len());
        let (left, top) = place(rest.anchor, height);
        let found = (0..rest.items.len()).collect();
        let placeholder: SharedString = format!("Search {}", rest.name).into();
        let palette = crate::palette::current(cx);
        let (hint, ink) = (palette.on_overlay(HINT), palette.on_overlay(TITLE));
        let selection = palette.plate(STATE_SELECTED);
        self.search.update(cx, |field, cx| {
            field.set_colors(hint, ink, selection);
            field.set_placeholder(placeholder, cx);
            field.reset(SharedString::default(), window, cx);
        });
        let edit = cx.observe_in(&self.search, window, |preview, field, _, cx| {
            let text = field.read(cx).text().to_string();
            preview.searched(&text, cx);
        });
        let (over, folder) = (rest.window, rest.folder);
        self.shown = Some(Shown {
            rest,
            query: String::new(),
            found,
            selected: Some(0),
            kept: false,
            height,
            _edit: edit,
        });
        // It shows for the row the pointer rests on.
        self.on_row = true;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        window.focus(&self.search.focus_handle(cx), cx);
        // check.sh reads this line, to see the preview is beside its row.
        log!("browser: folder preview shown {folder} at {left} {top} {WIDTH} {height}");
        // Under Reduce Motion nothing scales, and the fades stay (docs/ux-spec.md › Motion).
        let from = if cx.reduce_motion() { 1. } else { FROM };
        self.host.show(over, left, top, height, from);
        cx.notify();
    }

    /// The search has `text` in it: the list narrows to what matches, and the first is selected
    /// for ↩.
    fn searched(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        if shown.query == text {
            return;
        }
        shown.query = text.to_string();
        shown.found = matches(&shown.rest.items, text);
        shown.kept = true;
        shown.selected = (!shown.found.is_empty()).then_some(0);
        // check.sh waits for this line, to see what's typed reaches the search.
        log!(
            "browser: folder preview found {} for {text}",
            shown.found.len()
        );
        self.wait = None;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn go(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        if shown.found.is_empty() {
            return;
        }
        let last = shown.found.len() - 1;
        let moved = |at: usize| at.saturating_add_signed(by).min(last);
        shown.selected = Some(shown.selected.map_or(0, moved));
        shown.kept = true;
        self.wait = None;
        if let Some(at) = shown.selected {
            self.scroll.scroll_to_item(at, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.go(-1, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.go(1, cx);
    }

    /// ↩ shows the tab selected. While an input method composes, it's the method's.
    fn choose_selected(&mut self, _: &Choose, _: &mut Window, cx: &mut Context<Self>) {
        if self.search.read(cx).composing() {
            cx.propagate();
            return;
        }
        let Some(shown) = &self.shown else {
            return;
        };
        if let Some(at) = shown.selected {
            let tab = shown.rest.items[shown.found[at]].tab;
            self.choose(tab, cx);
        }
    }

    fn choose(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some(shown) = &self.shown else {
            return;
        };
        let chosen = shown.rest.chosen.clone();
        // check.sh reads this line.
        log!("browser: folder preview chose {tab}");
        self.close(cx);
        chosen(tab, cx);
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
    }

    /// The preview goes, and the browser window gets the keyboard back.
    fn close(&mut self, cx: &mut Context<Self>) {
        self.wait = None;
        self.resting = None;
        self.on_card = false;
        self.on_row = false;
        if self.shown.take().is_some() {
            self.host.hide();
            // check.sh reads this line.
            log!("browser: folder preview hidden");
            cx.notify();
        }
    }

    /// The rows `rows` of what the search found, those in view.
    fn rows(&self, rows: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(shown) = &self.shown else {
            return Vec::new();
        };
        let palette = crate::palette::current(cx);
        // The row under the pointer has the selected row's fill, both at once, and the pointer
        // doesn't move the selection (Arc 1.171).
        let fill = palette.plate(STATE_SELECTED);
        let line = |text: SharedString, size: f32, height: f32, alpha: f32| {
            div()
                .h(px(height))
                .flex()
                .items_center()
                .text_size(px(size))
                .text_color(palette.on_overlay(alpha))
                .child(div().truncate().child(text))
        };
        rows.filter_map(|at| Some((at, &shown.rest.items[*shown.found.get(at)?])))
            .map(|(at, item)| {
                let tab = item.tab;
                let selected = shown.selected == Some(at);
                let plate = div()
                    .id(("folder-preview-row", tab))
                    .h(px(ROW_FILL))
                    .w_full()
                    .relative()
                    .rounded(px(ROW_RADIUS))
                    .when(selected, |row| row.bg(fill))
                    .hover(|style| style.bg(fill))
                    .child(
                        div()
                            .absolute()
                            .left(px(ICON_X))
                            .top(px((ROW_FILL - ICON) / 2.))
                            .size(px(ICON))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(&palette, &item.icon)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(TITLE_X))
                            .right(px(ICON_X))
                            .top_0()
                            .h(px(ROW_FILL))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .child(line(item.title.clone(), TITLE_SIZE, TITLE_LINE, TITLE))
                            .when(!item.age.is_empty(), |text| {
                                text.child(line(item.age.clone(), AGE_SIZE, AGE_LINE, HINT))
                            }),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |preview, _, _, cx| preview.choose(tab, cx)));
                div().h(px(ROW)).w_full().child(plate).into_any_element()
            })
            .collect()
    }
}

/// A tab's icon, at the row's icon size: its emoji, else its favicon, else the globe.
fn icon(palette: &Palette, icon: &Icon) -> AnyElement {
    match icon {
        Icon::Favicon(Some(favicon)) => img(favicon.clone()).size(px(ICON)).into_any_element(),
        Icon::Favicon(None) => svg()
            .path("icons/globe.svg")
            .size(px(ICON))
            .text_color(palette.on_overlay_glyph(HINT))
            .into_any_element(),
        Icon::Emoji(emoji) => div()
            .text_size(px(ICON))
            .line_height(px(ICON))
            .child(emoji.clone())
            .into_any_element(),
    }
}

impl Focusable for FolderPreview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FolderPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI);
        let Some(shown) = &self.shown else {
            return root;
        };
        let palette = crate::palette::current(cx);
        let hint = palette.on_overlay(HINT);
        // The glyph is centred over the rows' icons, and the text starts where their titles do.
        let glyph_x = PADDING + ICON_X + (ICON - SEARCH_GLYPH) / 2.;
        let search = div()
            .flex_none()
            .h(px(SEARCH))
            .flex()
            .items_center()
            .gap(px(PADDING + TITLE_X - glyph_x - SEARCH_GLYPH))
            .pl(px(glyph_x))
            .pr(px(PADDING + ICON_X))
            .text_size(px(TITLE_SIZE))
            .text_color(palette.on_overlay(TITLE))
            .child(
                svg()
                    .path("icons/search.svg")
                    .flex_none()
                    .size(px(SEARCH_GLYPH))
                    .text_color(palette.on_overlay_glyph(HINT)),
            )
            .child(div().flex_1().child(self.search.clone()));
        let divider = div()
            .flex_none()
            .h(px(DIVIDER))
            .bg(palette.plate(DIVIDER_ALPHA));
        let list = match shown.found.is_empty() {
            // Arc's words, without its emoji.
            true => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(TITLE_SIZE))
                .text_color(hint)
                .child("No tabs matching that search")
                .into_any_element(),
            false => uniform_list(
                "folder-preview-rows",
                shown.found.len(),
                cx.processor(|preview, rows, _, cx| preview.rows(rows, cx)),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .into_any_element(),
        };
        // The last row's fill is as far from the bottom as the first's from the line, and as each
        // is from the sides.
        let list = div()
            .flex_1()
            .min_h_0()
            .px(px(PADDING))
            .pt(px(PADDING))
            .pb(px(PADDING - (ROW - ROW_FILL)))
            .child(list);
        // The rim is its own layer, so the border doesn't shift what the fill's box holds.
        let rim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(RADIUS))
            .border(px(RIM))
            .border_color(Level::Popover.rim_in(&palette));
        root.child(
            div()
                .id("folder-preview")
                .key_context("FolderPreview")
                .track_focus(&self.focus)
                .on_action(cx.listener(Self::dismiss))
                .on_action(cx.listener(Self::choose_selected))
                .on_action(cx.listener(Self::up))
                .on_action(cx.listener(Self::down))
                .on_hover(
                    cx.listener(|preview, hovered: &bool, _, cx| preview.on_card(*hovered, cx)),
                )
                // A click anywhere but a row keeps the keyboard in the search.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|preview, _, window, cx| {
                        window.focus(&preview.search.focus_handle(cx), cx)
                    }),
                )
                .relative()
                .w(px(WIDTH))
                .h(px(shown.height))
                .flex()
                .flex_col()
                .rounded(px(RADIUS))
                .bg(palette.overlay)
                .font_weight(FontWeight::NORMAL)
                .child(search)
                .child(divider)
                .child(list)
                .child(rim),
        )
    }
}

#[cfg(test)]
mod tests;
