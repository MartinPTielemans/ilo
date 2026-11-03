//! The folder preview, Arc's (#690): resting the pointer on a closed folder's row shows a list of
//! the tabs in it beside the row, with those of the folders inside it, by the titles the sidebar
//! gives them, under a search. Typing narrows the list and selects the first match, ↑ and ↓ move
//! the selection, and ↩ or a click shows the tab, which then peeks under the closed folder, as the
//! tab shown does, without opening it. It crosses the sidebar's edge onto the page, which GPUI
//! can't draw over, so it's a window of its own that the host shows over the browser window, as
//! its child, with the keyboard for the search. Until something is typed in it, it goes as the
//! hover card does, once the pointer has left it and its row; after that, with escape, a choice or
//! a click outside it. What it asks of its window goes through `Host`.

use std::{ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight, Global,
    KeyBinding, MouseButton, ScrollStrategy, SharedString, Subscription, Task, TitlebarOptions,
    UniformListScrollHandle, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, actions, div, img, point, prelude::*, px, size, svg, uniform_list,
};

use crate::elevation::Level;
use crate::hover_card::{Anchor, DWELL, EDGE, GAP, Icon, LEAVE};
use crate::palette::{Palette, STATE_HOVERED, STATE_SELECTED};
use crate::shell::KEY_CONTEXT;
use crate::text_input::TextInput;
use crate::tokens::folder_preview::{
    HINT, ICON, ICON_X, PADDING, RIM, ROW, ROW_RADIUS, SEARCH, SEARCH_GLYPH, TITLE, TITLE_SIZE,
    TITLE_X,
};
pub use crate::tokens::folder_preview::{RADIUS, WIDTH};

actions!(folder_preview, [Dismiss, Choose, Up, Down]);

/// How many rows show before the list scrolls (ours): a long folder's preview stays beside the
/// rows rather than reaching down the window.
pub const MOST_ROWS: usize = 8;

/// What the preview asks of its window.
pub trait Host {
    /// Shows the preview's window, `height` tall, with its top left at (`left`, `top`) in browser
    /// window `window`, the one whose row it's for, with the keyboard, or moves it there if it
    /// shows.
    fn show(&self, window: u64, left: f32, top: f32, height: f32);
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

/// How tall the preview of `count` tabs is: the search, then a row for each, MOST_ROWS at most,
/// inside its padding. It keeps that height as a search narrows the list.
pub fn height(count: usize) -> f32 {
    2. * PADDING + SEARCH + count.clamp(1, MOST_ROWS) as f32 * ROW
}

/// Where a preview `height` tall has its top left: as the hover card's, GAP right of its row, its
/// top at the row's, and EDGE inside the window, the top and left first if it can't be both.
pub fn place(anchor: Anchor, height: f32) -> (f32, f32) {
    let (width, tall) = (
        f32::from(anchor.window.width),
        f32::from(anchor.window.height),
    );
    let left = f32::from(anchor.row.right()) + GAP;
    let top = f32::from(anchor.row.top());
    (
        left.min(width - WIDTH - EDGE).max(EDGE),
        top.min(tall - height - EDGE).max(EDGE),
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
    let bounds = Bounds::new(
        point(px(0.), px(0.)),
        size(px(WIDTH), px(height(MOST_ROWS))),
    );
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
        if preview.shown.take().is_some() {
            // check.sh reads this line.
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
        if leaves && !self.on_card {
            self.leave_after(cx);
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

    fn on_card(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.on_card = hovered && self.shown.is_some();
        match hovered {
            true => self.wait = None,
            false if self.shown.is_some() => self.leave_after(cx),
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
            selected: None,
            kept: false,
            height,
            _edit: edit,
        });
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        window.focus(&self.search.focus_handle(cx), cx);
        // check.sh reads this line, to see the preview is beside its row.
        log!("browser: folder preview shown {folder} at {left} {top} {WIDTH} {height}");
        self.host.show(over, left, top, height);
        cx.notify();
    }

    /// The search has `text` in it: the list narrows to what matches, and the first is selected
    /// for ↩, or none for an empty search.
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
        shown.selected = (!text.trim().is_empty() && !shown.found.is_empty()).then_some(0);
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
        shown.selected = Some(match shown.selected {
            None if by < 0 => last,
            None => 0,
            Some(at) => at.saturating_add_signed(by).min(last),
        });
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
        rows.filter_map(|at| Some((at, &shown.rest.items[*shown.found.get(at)?])))
            .map(|(at, item)| {
                let tab = item.tab;
                let selected = shown.selected == Some(at);
                div()
                    .id(("folder-preview-row", tab))
                    .h(px(ROW))
                    .w_full()
                    .relative()
                    .rounded(px(ROW_RADIUS))
                    .when(selected, |row| row.bg(palette.plate(STATE_SELECTED)))
                    .when(!selected, |row| {
                        row.hover(|style| style.bg(palette.plate(STATE_HOVERED)))
                    })
                    .child(
                        div()
                            .absolute()
                            .left(px(ICON_X))
                            .top(px((ROW - ICON) / 2.))
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
                            .h(px(ROW))
                            .flex()
                            .items_center()
                            .text_size(px(TITLE_SIZE))
                            .text_color(palette.on_overlay(TITLE))
                            .child(div().truncate().child(item.title.clone())),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |preview, _, _, cx| preview.choose(tab, cx)))
                    .into_any_element()
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
        let search = div()
            .h(px(SEARCH))
            .flex()
            .items_center()
            .gap(px(TITLE_X - ICON_X - SEARCH_GLYPH))
            .pl(px(ICON_X + (ICON - SEARCH_GLYPH) / 2.))
            .pr(px(ICON_X))
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
        let list = match shown.found.is_empty() {
            true => div()
                .h(px(ROW))
                .flex()
                .items_center()
                .pl(px(TITLE_X))
                .text_size(px(TITLE_SIZE))
                .text_color(hint)
                .child("No Matches")
                .into_any_element(),
            false => uniform_list(
                "folder-preview-rows",
                shown.found.len(),
                cx.processor(|preview, rows, _, cx| preview.rows(rows, cx)),
            )
            .track_scroll(&self.scroll)
            .flex_1()
            .into_any_element(),
        };
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
                .p(px(PADDING))
                .flex()
                .flex_col()
                .rounded(px(RADIUS))
                .bg(palette.overlay)
                .font_weight(FontWeight::NORMAL)
                .child(search)
                .child(list)
                .child(rim),
        )
    }
}

#[cfg(test)]
mod tests;
