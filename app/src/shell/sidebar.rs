//! The sidebar's width, which its edge drags; hiding and showing it, and while it's
//! hidden, its reveal over the page (docs/ux-spec.md › Sidebar reveal); its gradient in the
//! Space's colour; and the marker for the selected tab's row scrolled out of the list.

use std::time::Instant;
use std::{rc::Rc, time::Duration};

use gpui::{
    App, Bounds, ClickEvent, Context, CursorStyle, DispatchPhase, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba, Window, canvas, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, size,
};

use super::access::Accessible as _;
use crate::glow;
use crate::tokens::shell::{SHOWN_MARKER, SHOWN_MARKER_HIT};

use super::{
    CARD_INSET, PLATE_INSET, PRIVATE_SHADE, REVEAL_BACK, REVEAL_HOLD, REVEAL_IN_DAMPING,
    REVEAL_IN_RESPONSE, REVEAL_INSET, REVEAL_MOVE, REVEAL_OUT_DAMPING, REVEAL_OUT_RESPONSE,
    REVEAL_OVERSHOOT, REVEAL_WAIT, ROW_HEIGHT, Row, SIDEBAR_EDGE, SIDEBAR_HIDE, SIDEBAR_MAX,
    SIDEBAR_MIN, SIDEBAR_SHOW, SIDEBAR_WIDTH, Settle, Shell, Slot, Space, settle_progress, spring,
};
use crate::tokens::shell::{FOOTER_HEIGHT, NAV_SIZE, NAV_TOP, NAV_TRAIL, PLUS_INSET};

/// What the toast says when a tab opens with the sidebar hidden, Arc's words, and for how long,
/// counted again each time it's asked for (Arc 1.171, measured by an outside reverse-engineering,
/// *interim*, #1404). The Space toast takes the same time (ours).
pub(super) const OPENED: &str = "New Tab Created";
const OPENED_TIME: Duration = Duration::from_millis(4800);

/// Settings › Tabs & Sidebar's "Show the hidden sidebar at the window's edge", on unless the user
/// turned it off: a display to the left, or a scrollbar against that edge, can set it off by
/// accident. ⌘S and ⌥ still bring the sidebar out. The user's preference: agents have no call to
/// change it.
#[derive(Clone, Copy)]
pub struct SidebarEdge(pub bool);

impl gpui::Global for SidebarEdge {}

/// Whether the pointer at the window's edge brings the hidden sidebar out.
pub fn edge_reveals(cx: &gpui::App) -> bool {
    cx.try_global::<SidebarEdge>().is_none_or(|edge| edge.0)
}

/// Settings › Tabs & Sidebar's Sidebar position, Right rather than the default Left (#2375): the
/// sidebar, the window's buttons and the reveal go to the window's right, and what opens toward the
/// page opens to their left. This device's, not synced: a laptop and a desk with a second screen
/// can want different sides. Every window follows at once. The user's preference: agents have no
/// call to change it.
#[derive(Clone, Copy)]
pub struct SidebarRight(pub bool);

impl gpui::Global for SidebarRight {}

/// Whether Sidebar position is Right.
pub fn sidebar_right(cx: &gpui::App) -> bool {
    cx.try_global::<SidebarRight>().is_some_and(|right| right.0)
}

/// Settings › Tabs & Sidebar's Top bar, off by default (#3160): the header's row widens across the
/// window's top and the URL field moves into it, the card under it. This device's, not synced, as
/// Sidebar position is, and every window follows at once. A view preference: agents have no call
/// to change it.
#[derive(Clone, Copy)]
pub struct TopBar(pub bool);

impl gpui::Global for TopBar {}

/// Whether Top bar is on.
pub fn top_bar(cx: &gpui::App) -> bool {
    cx.try_global::<TopBar>().is_some_and(|on| on.0)
}

/// `x` across a window `width` wide, measured from the sidebar's side: itself on the left, from the
/// right edge on the right. Its own inverse.
pub(super) fn from_side(right: bool, width: f32, x: f32) -> f32 {
    match right {
        true => width - x,
        false => x,
    }
}

/// `frame` in a window `width` wide, mirrored when the sidebar is on the right.
pub(super) fn mirrored(right: bool, width: Pixels, frame: Bounds<Pixels>) -> Bounds<Pixels> {
    match right {
        true => Bounds::new(
            point(width - frame.origin.x - frame.size.width, frame.origin.y),
            frame.size,
        ),
        false => frame,
    }
}

impl Shell {
    /// Whether this window's sidebar is on the right (`SidebarRight`). A Little ilo window has
    /// none.
    pub(super) fn right(&self) -> bool {
        self.right
    }

    /// `x` across the window from the sidebar's side (`from_side`).
    pub(super) fn from_side(&self, x: f32) -> f32 {
        from_side(self.right, f32::from(self.viewport.width), x)
    }

    /// Where the docked sidebar's column starts across the window: 0, or its width in from the
    /// right edge.
    pub(super) fn column_x(&self) -> f32 {
        match self.right {
            true => f32::from(self.viewport.width) - self.width(),
            false => 0.,
        }
    }

    /// The docked sidebar's edge toward the page, across the window, where what it opens beside
    /// it points from.
    pub(super) fn page_edge_x(&self) -> f32 {
        self.column_x() + if self.right { 0. } else { self.width() }
    }

    /// Follows Sidebar position: the window lays itself out on the other side, and its host moves
    /// the window's buttons and the reveal's edge.
    pub fn set_sidebar_right(&mut self, right: bool, cx: &mut Context<Self>) {
        let right = right && self.little.is_none();
        if right == self.right {
            return;
        }
        self.right = right;
        self.conceal_now(cx);
        self.browser.set_sidebar_right(right, cx);
        // check.sh waits for this line.
        log!(
            "browser: sidebar on the {}",
            if right { "right" } else { "left" }
        );
        self.update_menus(cx);
        cx.notify();
    }

    /// The docked sidebar's width as drawn: with the pointer while its edge is dragged, then on its
    /// way to where it rests.
    pub(super) fn width(&self) -> f32 {
        if let Some((_, way)) = self.resize {
            return way.clamp(SIDEBAR_MIN, SIDEBAR_MAX).round();
        }
        let Some((from, start)) = self.width_motion else {
            return self.sidebar_width;
        };
        settle_progress(start).map_or(self.sidebar_width, |progress| {
            from + (self.sidebar_width - from) * progress
        })
    }

    /// The sidebar's edge toward the page, which drags its width and puts it back on a
    /// double-click: the docked sidebar's, or with `revealed`, the reveal's panel's, inside it.
    /// Buttons keep their plates: beside the header's it's the gap past reload, beside the
    /// footer's + the gap past that, and down the rows it takes 2 of their plates too.
    pub(super) fn sidebar_edge(
        &self,
        revealed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let width = self.width();
        // Toward the page widens it, so the arrows turn with the side.
        let (wider, narrower) = match self.right {
            true => (CursorStyle::ResizeLeft, CursorStyle::ResizeRight),
            false => (CursorStyle::ResizeRight, CursorStyle::ResizeLeft),
        };
        let cursor = match width {
            width if width <= SIDEBAR_MIN => wider,
            width if width >= SIDEBAR_MAX => narrower,
            _ => CursorStyle::ResizeLeftRight,
        };
        let resizing = self.resize.is_some();
        let shell = cx.weak_entity();
        let right = self.right;
        // Across the browser window. The reveal's window is a strip at the sidebar's side, which
        // widens while its edge is dragged.
        let across = move |shell: &Shell, x: Pixels, window: &Window| -> f32 {
            match revealed {
                true => shell.from_reveal(x.into(), window.viewport_size().width.into()),
                false => x.into(),
            }
        };
        let part = move |id: &'static str, wide: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
                .absolute()
                .when(right, |part| part.left_0())
                .when(!right, |part| part.right_0())
                .w(px(wide))
                .cursor(cursor)
                .block_mouse_except_scroll()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, event: &MouseDownEvent, window, cx| {
                        let mut event = event.clone();
                        event.position.x = px(across(shell, event.position.x, window));
                        shell.press_edge(&event, cx)
                    }),
                )
        };
        let head = NAV_TOP + NAV_SIZE;
        // The reveal's panel is the edge's container there, from its own left.
        let left = match (revealed, self.right) {
            (false, true) => self.column_x(),
            (true, true) => 0.,
            (_, false) => width - SIDEBAR_EDGE,
        };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left))
            .w(px(SIDEBAR_EDGE))
            .child(part("sidebar-edge-head", NAV_TRAIL, cx).top_0().h(px(head)))
            .child(
                part("sidebar-edge", SIDEBAR_EDGE, cx)
                    .top(px(head))
                    .bottom(px(FOOTER_HEIGHT)),
            )
            .child(
                part("sidebar-edge-foot", PLUS_INSET, cx)
                    .bottom_0()
                    .h(px(FOOTER_HEIGHT)),
            )
            // The pointer's moves and release anywhere in the window, while the edge is dragged.
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        if resizing {
                            window.set_window_cursor_style(cursor);
                        }
                        let moved = shell.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                moved
                                    .update(cx, |shell, cx| {
                                        let x = across(shell, event.position.x, window);
                                        shell.drag_edge(x, cx)
                                    })
                                    .ok();
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                shell.update(cx, |shell, cx| shell.let_go_edge(cx)).ok();
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
    }

    pub(super) fn press_edge(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let width = self.width();
        if event.click_count == 2 {
            self.resize = None;
            self.set_width(width, SIDEBAR_WIDTH, cx);
            return;
        }
        self.width_motion = None;
        self.resize = Some((self.from_side(f32::from(event.position.x)) - width, width));
        cx.notify();
    }

    /// The pointer dragging the edge is at `x` across the window. Past half the narrowest width, as
    /// AppKit's split view collapses a pane, the docked sidebar hides in one frame and keeps the
    /// width it had (Arc). The reveal's panel, over a sidebar already hidden, stops at the
    /// narrowest.
    pub(super) fn drag_edge(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((grab, _)) = self.resize else {
            return;
        };
        let x = self.from_side(x);
        if x - grab >= SIDEBAR_MIN / 2. || self.sidebar_hidden {
            self.resize = Some((grab, x - grab));
            return cx.notify();
        }
        self.resize = None;
        self.sidebar_hidden = true;
        self.sidebar_motion = None;
        self.sidebar_hover(false, cx);
        self.recent_downloads_gone(cx);
        self.update_menus(cx);
        // check.sh waits for this line, then reads where the page is.
        self.logged_once_laid.push("browser: sidebar hidden".into());
        cx.notify();
    }

    pub(super) fn let_go_edge(&mut self, cx: &mut Context<Self>) {
        if let Some((_, way)) = self.resize.take() {
            let width = way.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
            self.set_width(width, width, cx);
            // The reveal's panel stayed out through the drag: from where the pointer is now, it
            // goes as it would have.
            if self.sidebar_hidden {
                self.pointer_at(self.reveal_x, cx);
            }
        }
    }

    /// Makes the reveal's window as wide as its panel at rest needs: its edge, past which the page
    /// takes clicks again, or while the edge is dragged, as far as it can go, or while the width
    /// settles, the wider of the two ends.
    pub(super) fn size_reveal(&mut self, cx: &mut Context<Self>) {
        let wide = match (self.resize, self.width_motion) {
            (Some(_), _) => SIDEBAR_MAX,
            (None, Some((from, _))) => from.max(self.sidebar_width),
            (None, None) => self.width(),
        };
        let wide = REVEAL_INSET + wide;
        if self.revealed && wide != self.reveal_window {
            self.reveal_window = wide;
            self.browser.show_reveal(wide, cx);
        }
    }

    /// Settles the docked sidebar at `to` wide from `from`, and keeps the width for the next
    /// launch.
    fn set_width(&mut self, from: f32, to: f32, cx: &mut Context<Self>) {
        let to = to.round();
        if to != self.sidebar_width {
            self.sidebar_width = to;
            self.browser.set_sidebar_width(to, cx);
            // The tasks' lines and chips at the foot wrap to the new width.
            self.reshape_foot(cx);
        }
        self.width_motion = (from != to).then(|| (from, crate::motion::start(cx)));
        if self.width_motion.is_none() {
            // check.sh waits for these lines, then reads where the page is.
            self.logged_once_laid
                .push(format!("browser: sidebar width {to}"));
        }
        cx.notify();
    }

    /// How far the sidebar has moved since `start`, or None once it's there.
    pub(super) fn sidebar_progress(&self, start: Instant) -> Option<f32> {
        let t = start.elapsed().as_secs_f32();
        let time = if self.sidebar_hidden {
            SIDEBAR_HIDE
        } else {
            SIDEBAR_SHOW
        };
        (t < time).then(|| 1. - (1. - t / time).powi(3))
    }

    /// Hides the docked sidebar, or shows it, from wherever it is.
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        // A full-screen page has no sidebar to hide.
        if self.page_fullscreen {
            return;
        }
        // Ours: the sidebar comes back from the Library as it was.
        if self.library_open {
            return self.set_library(false, cx);
        }
        if self.dock(cx) {
            return;
        }
        let from = self.card_left();
        self.sidebar_hidden = !self.sidebar_hidden;
        self.sidebar_motion = Some((from, crate::motion::start(cx)));
        self.sidebar_hover(false, cx);
        self.recent_downloads_gone(cx);
        self.update_menus(cx);
        cx.notify();
    }

    /// Tabs › Reveal Tab in Sidebar: shows the hidden sidebar, opens the closed folders the tab
    /// shown is in and scrolls the list to its row, once more after the folders open. It moves
    /// only the user's view, so agents get none; they open folders with `bc_folder_set_open`.
    pub(super) fn reveal_tab(&mut self, cx: &mut Context<Self>) {
        self.reveal_row(self.selected, cx);
    }

    /// Reveal Tab in Sidebar for `tab`, if the Space shown still has it.
    fn reveal_row(&mut self, tab: u64, cx: &mut Context<Self>) {
        let listed = |space: &&Space| space.pinned.contains(&tab) || space.today.contains(&tab);
        let Some(space) = self.shown_space().filter(listed) else {
            return;
        };
        let row = self.row_of(tab);
        let shut: Vec<u64> = space
            .holders(row)
            .filter(|&folder| !space.is_open(folder))
            .collect();
        if self.sidebar_hidden && !self.reveal_shown() {
            self.toggle_sidebar(cx);
        }
        for &folder in &shut {
            self.browser.set_folder_open(folder, true, cx);
        }
        self.revealing = (!shut.is_empty()).then_some(tab);
        self.scroll_to_row(tab);
    }

    /// Tabs opened in the Space shown while the sidebar is hidden, whose rows aren't seen: a toast
    /// says so, and its Show reveals the top one's row (Arc's release notes), as the Spaces' does.
    pub(super) fn toast_opened(&mut self, tabs: &[u64], cx: &mut Context<Self>) {
        let Some(&tab) = tabs.first().filter(|_| self.unseen_sidebar()) else {
            return;
        };
        let text = match tabs.len() {
            1 => OPENED.to_owned(),
            count => format!("{count} New Tabs Created"),
        };
        let shell = cx.entity().downgrade();
        let show = Rc::new(move |cx: &mut App| {
            shell.update(cx, |shell, cx| shell.reveal_row(tab, cx)).ok();
        });
        self.browser
            .toast_button(&text, ("Show", "show"), show, OPENED_TIME, cx);
    }

    /// Whether what's added to the sidebar goes unseen: it's hidden, its reveal isn't out, and the
    /// window is the one in front, where the toast shows.
    fn unseen_sidebar(&self) -> bool {
        self.sidebar_hidden && !self.reveal_shown() && self.browser.in_front()
    }

    /// Spaces made while the sidebar is hidden, as by an import or an agent, whose icons in the
    /// switcher aren't seen: a toast says so, and its Show shows the first with the sidebar back
    /// (Arc's release notes; the words are ours). The form's own Space brings the sidebar back
    /// itself. Spaces made while the toast still tells of others are told of with them.
    pub(super) fn toast_spaces(&mut self, made: Vec<u64>, cx: &mut Context<Self>) {
        let form = self.form.as_ref().map(|form| form.space);
        let made: Vec<u64> = made
            .into_iter()
            .filter(|&space| Some(space) != form)
            .collect();
        if made.is_empty() || !self.unseen_sidebar() {
            return;
        }
        let (told, at) = &mut self.spaces_toasted;
        if at.is_none_or(|at| at.elapsed() >= OPENED_TIME) {
            told.clear();
        }
        told.extend(made);
        *at = Some(Instant::now());
        let first = told[0];
        let text = match told.len() {
            1 => {
                let name = self.spaces.iter().find(|space| space.id == first);
                format!(
                    "Added the Space “{}”",
                    name.map_or("", |space| space.name.as_ref())
                )
            }
            count => format!("Added {count} Spaces"),
        };
        let shell = cx.entity().downgrade();
        let show = Rc::new(move |cx: &mut App| {
            shell
                .update(cx, |shell, cx| shell.show_made_space(first, cx))
                .ok();
        });
        self.browser
            .toast_button(&text, ("Show", "show"), show, OPENED_TIME, cx);
    }

    /// The Space toast's Show: the Space, if it's still there, with the sidebar back.
    fn show_made_space(&mut self, space: u64, cx: &mut Context<Self>) {
        if !self.spaces.iter().any(|each| each.id == space) {
            return;
        }
        if self.sidebar_hidden && !self.reveal_shown() {
            self.toggle_sidebar(cx);
        }
        self.show_space(space, cx);
    }

    /// Whether the selected tab's row is scrolled wholly out past the list's top (true) or its
    /// bottom (false), as the list was last drawn. None while any of it shows.
    fn selected_row_past(&self) -> Option<bool> {
        let Some(Row::Tab(row)) = self.selected_row() else {
            return None;
        };
        let top = *self.layout().get(&Slot::Tab(row))? + self.tiles_room() - self.fixed_head();
        let height = f32::from(self.list_scroll.bounds().size.height);
        // Where the list will draw: GPUI clamps a wheel's move past either end in the layout
        // after this, and a marker drawn from the unclamped offset would flash up and stay.
        let most = f32::from(self.list_scroll.max_offset().y);
        let scrolled = (-f32::from(self.list_scroll.offset().y)).clamp(0., most.max(0.));
        past(top, scrolled, height)
    }

    /// While the selected tab's row is scrolled out of the list, a sliver of its plate at the
    /// edge it's past, over the list, which a click scrolls back to it (Arc; the look is ours).
    pub(super) fn shown_marker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let past = self.selected_row_past();
        let list = self.list_scroll.bounds();
        let drawn = past.map(|top| {
            let y = if top {
                list.top()
            } else {
                list.bottom() - px(SHOWN_MARKER_HIT)
            };
            let width = list.size.width - px(2. * PLATE_INSET);
            let origin = point(list.left() + px(PLATE_INSET), y);
            (top, Bounds::new(origin, size(width, px(SHOWN_MARKER_HIT))))
        });
        if self.marker_drawn.replace(drawn) != drawn {
            // check.sh waits for these lines, and clicks the marker where it says, in the window.
            match drawn {
                Some((top, frame)) => log!(
                    "browser: shown tab marker {} at {} {} {} {}",
                    if top { "top" } else { "bottom" },
                    f32::from(frame.origin.x),
                    f32::from(frame.origin.y),
                    f32::from(frame.size.width),
                    f32::from(frame.size.height)
                ),
                None => log!("browser: shown tab marker hidden"),
            }
        }
        let top = past?;
        let tab = self.selected;
        let sliver = div()
            .w_full()
            .h(px(SHOWN_MARKER))
            .rounded(px(SHOWN_MARKER / 2.))
            .bg(self.palette.shown(0.));
        let marker = div()
            .id("shown-marker")
            .absolute()
            .left(px(PLATE_INSET))
            .right(px(PLATE_INSET))
            .h(px(SHOWN_MARKER_HIT))
            .flex()
            .flex_col()
            // At the scrolled rows' edge, under the name row that stays put.
            .map(|marker| match top {
                true => marker.top(px(self.fixed_head())),
                false => marker.bottom_0().justify_end(),
            })
            .block_mouse_except_scroll()
            .when(self.a11y, |marker| {
                marker.button_named("Scroll to the Selected Tab")
            })
            .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                shell.scroll_to_row(tab);
                cx.notify();
            }))
            .child(sliver);
        Some(marker)
    }

    /// Whether the window's own buttons show (`window_buttons_at`).
    pub(super) fn window_buttons_shown(&self) -> bool {
        self.window_buttons_drawn().window
    }

    /// Whether the Library's panel draws the window's buttons, on its column.
    pub(super) fn library_buttons(&self) -> bool {
        self.window_buttons_drawn().library
    }

    /// Whether the sidebar draws the window's buttons, receding with it under the Library.
    pub(super) fn sidebar_buttons(&self) -> bool {
        self.window_buttons_drawn().sidebar
    }

    fn window_buttons_drawn(&self) -> WindowButtons {
        // A Little ilo window's are over its bar, where the window's own go, as are Windows' at the
        // top bar's end, which nothing covers.
        if self.little.is_some() || self.buttons_at_bar_end() {
            return WindowButtons {
                window: true,
                library: false,
                sidebar: false,
            };
        }
        window_buttons_at(
            self.library_open,
            self.library_moving(),
            self.library_at_rest(),
            // The bar keeps them while the sidebar is hidden, as it keeps the header's buttons.
            self.bar_on() || (!self.sidebar_hidden && self.sidebar_motion.is_none()),
        )
    }

    /// Docks the sidebar the reveal has out, in one frame (Arc), and returns whether it was out.
    pub(super) fn dock(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.reveal_shown() {
            return false;
        }
        self.conceal_now(cx);
        self.sidebar_hidden = false;
        self.sidebar_motion = None;
        // The pointer that kept the panel out is over the sidebar, so its tools stay as they were.
        self.sidebar_hover(true, cx);
        self.tools_on = true;
        self.tools_fade = None;
        self.update_menus(cx);
        // check.sh waits for this line.
        log!("browser: sidebar shown");
        cx.notify();
        true
    }

    /// Whether the reveal's panel is on screen, moving or not.
    pub fn reveal_shown(&self) -> bool {
        self.revealed || self.reveal_motion.is_some()
    }

    /// How far out the reveal's panel is while it moves: 0 off the window, 1 at rest over the page
    /// and past 1 as it overshoots. None once it's at rest.
    pub(super) fn reveal_step(&self) -> Option<f32> {
        let (from, start) = self.reveal_motion?;
        let t = start.elapsed().as_secs_f32();
        if self.revealed {
            spring(t, REVEAL_IN_RESPONSE, REVEAL_IN_DAMPING).map(|eased| from + (1. - from) * eased)
        } else {
            spring(t, REVEAL_OUT_RESPONSE, REVEAL_OUT_DAMPING).map(|eased| from * (1. - eased))
        }
    }

    pub(super) fn reveal_out(&self) -> f32 {
        self.reveal_step()
            .unwrap_or(if self.revealed { 1. } else { 0. })
    }

    /// The Space the window's colour is of, and while the sidebar pages between two, the one
    /// beside and how far the colour has gone to it, 0 to 1: a switch next door changes the
    /// colour ahead of the slide, anything else with the pages.
    pub(super) fn colored_pages(&self) -> (u64, Option<(u64, f32)>) {
        let pages = self.pages();
        let (space, x) = pages[0];
        let Some(&(beside, _)) = pages.get(1) else {
            return (space, None);
        };
        let settle = self.settle.as_ref().filter(|settle| {
            self.swipe.is_none() && settle.space == space && settle.beside == Some(beside)
        });
        let t = match settle.and_then(Settle::color_left) {
            Some(left) => left,
            None => (x.abs() / self.width()).min(1.),
        };
        (space, Some((beside, t)))
    }

    /// The Space's colour where the sidebar's middle is, `y` from the window's top (0) to its
    /// bottom (1): its glows there. While the sidebar pages between two Spaces, it cross-fades from
    /// one's to the other's as the pages move.
    pub(super) fn gradient_color(&self, y: f32) -> Hsla {
        let x = self.sidebar_x();
        let (space, beside) = self.colored_pages();
        let from = self.space_color_at(space, x, y);
        let Some((beside, t)) = beside else {
            return from.into();
        };
        let to = self.space_color_at(beside, x, y);
        let mix = |from: f32, to: f32| from + (to - from) * t;
        let (r, g, b) = (mix(from.r, to.r), mix(from.g, to.g), mix(from.b, to.b));
        Rgba {
            r,
            g,
            b,
            a: mix(from.a, to.a),
        }
        .into()
    }

    /// `space`'s colour `x` of the way across the window and `y` of the way down: its glows there,
    /// to lay over the window's ground; a private window's shade; nothing without a colour.
    pub(super) fn space_color_at(&self, space: u64, x: f32, y: f32) -> Rgba {
        if self.private {
            return Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: PRIVATE_SHADE,
            };
        }
        let (width, height) = (
            f32::from(self.viewport.width),
            f32::from(self.viewport.height),
        );
        let (foot, far) = glow::strengths(x * width, y * height, width, height, self.right());
        self.palette_of(space).glow_at(foot, far)
    }

    /// `space`'s glow at the foot of an opaque copy of its sidebar `width` by `height` with corners
    /// of `radius`, such as the panel the hidden sidebar comes out in and its card in the Library:
    /// the glow down the column's middle, as GPUI draws no radial gradient in a rounded box.
    pub(super) fn foot_glow(&self, space: u64, width: f32, height: f32, radius: f32) -> gpui::Div {
        let (_, reach) = glow::Corner::Foot.extent();
        let reach = reach.min(height);
        let palette = self.palette_of(space);
        let at = |up: f32| {
            let (foot, _) = glow::strengths(width / 2., height - up, width, height, false);
            Hsla::from(palette.glow_at(foot, 0.))
        };
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(reach))
            .rounded_b(px(radius))
            .bg(linear_gradient(
                180.,
                linear_color_stop(at(reach), 0.),
                linear_color_stop(at(0.), 1.),
            ))
    }

    /// The window's look, opaque, halfway down: what covers the rows it passes over, and the find
    /// bar's colour.
    pub fn window_fill(&self) -> Hsla {
        self.window_fill_at(0.5)
    }

    /// The colours the chrome draws with over the Space shown, for what floats over the page in
    /// the window's colour: a toast, the zoom HUD, a password offer.
    pub fn palette(&self) -> crate::palette::Palette {
        self.palette
    }

    /// The window's look, opaque, `y` of the way down.
    pub(super) fn window_fill_at(&self, y: f32) -> Hsla {
        Hsla::from(self.palette.underlay).blend(self.gradient_color(y))
    }

    /// The pointer moved to `at` over the browser window's own view, which has the strip at its
    /// left edge while the sidebar is hidden: it brings the sidebar out once it has moved a few
    /// points up or down in the strip, not as it comes in, so one flung at the edge leaves the
    /// sidebar hidden (Arc 1.171). Back in the strip soon after leaving the window across it, it
    /// has moved there, as a pointer kept in at a screen's edge would have (#3989).
    pub(super) fn pointer_over(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        let side = self.from_side(x);
        let mut back = false;
        if y < 0. {
            // Above the window, as GPUI's (-1, -1) before each leave on Linux and Windows: its
            // last move in the window says which way it went.
            self.left_by_last_move(cx);
        } else {
            let toward = self.reveal_heard.is_some_and(|(from, _)| side <= from);
            self.reveal_heard = Some((side, toward));
            // Its first move back over the window is its return.
            let left = self.reveal_left.take();
            let now = cx.background_executor().now();
            back = left.is_some_and(|left| now.duration_since(left) <= REVEAL_BACK);
        }
        let in_strip = (0. ..CARD_INSET).contains(&side) && y >= 0.;
        self.reveal_strip = in_strip.then(|| match self.reveal_strip {
            Some((from, moved)) => (from, moved || (y - from).abs() >= REVEAL_MOVE),
            None => (y, back),
        });
        self.pointer_at(x, cx);
    }

    /// The pointer left the browser window's own view, onto the page or another window, so a wait
    /// at the edge ends as one the pointer passed. On Linux and Windows the page is Chromium's
    /// window, whose moves the shell doesn't hear, so its leave is how the shell knows.
    pub(super) fn pointer_left(&mut self, cx: &mut Context<Self>) {
        self.left_by_last_move(cx);
        if !self.revealed {
            self.reveal_wait = None;
            self.reveal_strip = None;
        }
    }

    /// The pointer left the window's view. From the strip, a last move away from the edge took it
    /// onto the page, which starts where the strip ends, and one toward the edge or along it took
    /// it out across the edge. A first move in, with none heard before it to tell, counts as
    /// neither: a pointer jumping onto the page and one jumping out look the same.
    fn left_by_last_move(&mut self, cx: &mut Context<Self>) {
        if self.reveal_heard.take().is_some_and(|(_, toward)| toward) {
            self.left_across_edge(cx);
        }
    }

    /// The pointer left the window across its edge on the sidebar's side: from the strip, with the
    /// sidebar still to reveal, it's when its return counts.
    fn left_across_edge(&mut self, cx: &mut Context<Self>) {
        if self.can_reveal() && !self.revealed && self.reveal_strip.is_some() {
            // check.sh reads this line.
            log!("browser: reveal left across the edge");
            self.reveal_left = Some(cx.background_executor().now());
        }
    }

    /// `x` across the reveal's window, `width` wide, as across the browser window, whose edge on the
    /// sidebar's side the reveal's window is at.
    pub fn from_reveal(&self, x: f32, width: f32) -> f32 {
        match self.right {
            true => f32::from(self.viewport.width) - width + x,
            false => x,
        }
    }

    /// The pointer is at `x` in the browser window: kept at its edge on the sidebar's side, unless
    /// Settings › Tabs & Sidebar has the edge off, it brings the hidden sidebar out over the page,
    /// and more than the hold zone past the panel's edge, and off a row's hover card, it sends it
    /// away, and the card with it. The hosts say the pointer has gone with an `x` past any window.
    pub fn pointer_at(&mut self, x: f32, cx: &mut Context<Self>) {
        self.reveal_x = x;
        if !self.can_reveal() {
            return;
        }
        // From the sidebar's side, gone staying gone.
        let x = match x < f32::MAX {
            true => self.from_side(x),
            false => x,
        };
        let held = REVEAL_INSET + self.width() + REVEAL_HOLD;
        if self.revealed_by_option {
            if x > held {
                return;
            }
            self.revealed_by_option = false;
        }
        // The edge is the card's margin. Past it, out of the window, the pointer has gone to
        // another window or display.
        if !(0. ..CARD_INSET).contains(&x) {
            self.reveal_wait = None;
            self.reveal_strip = None;
            // A question or a chooser keeps it out until it's answered, the Cast card and its
            // menu until they close.
            // A drag of its edge keeps it out until it's let go.
            if x > held
                && self.revealed
                && self.resize.is_none()
                && !self.pointer_on_card
                && !self.page_asks()
                && self.cast.is_none()
                && !self.menu_open
            {
                self.set_revealed(false, cx);
                self.browser.hover_card(crate::hover_card::Hover::Close, cx);
            }
        } else if !self.revealed
            && self.reveal_wait.is_none()
            && self.reveal_strip.is_some_and(|(_, moved)| moved)
            && edge_reveals(cx)
            && self.is_key_window(cx)
        {
            // Arc waits a moment, so a pointer passing the edge on its way to another window or
            // display leaves the sidebar where it is. Only the key window's comes out, as Arc's:
            // a window behind leaves its edge to the pointer on its way to the one in front.
            // check.sh reads this line.
            log!("browser: reveal waits");
            self.reveal_wait = Some(cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(REVEAL_WAIT).await;
                shell
                    .update(cx, |shell, cx| {
                        shell.reveal_wait = None;
                        if shell.can_reveal() && shell.is_key_window(cx) {
                            shell.set_revealed(true, cx);
                        }
                    })
                    .ok();
            }));
        }
    }

    /// ⌥ was pressed and let go alone: it brings the hidden sidebar out over the page wherever the
    /// pointer is, and sends away the panel it brought out (Arc's release notes; ours: a tap, so
    /// ⌥ held for a shortcut, a typed character or an ⌥-click never does).
    pub fn option_tapped(&mut self, cx: &mut Context<Self>) {
        if !self.browser.option_reveals_sidebar() {
            return;
        }
        if self.revealed_by_option {
            log!("browser: reveal away by option");
            self.set_revealed(false, cx);
        } else if !self.revealed && self.can_reveal() {
            log!("browser: reveal by option");
            self.reveal_wait = None;
            self.set_revealed(true, cx);
            self.revealed_by_option = true;
        }
    }

    /// Whether this is the window the user last made key, which stays the one while the app is
    /// behind another's.
    fn is_key_window(&self, cx: &Context<Self>) -> bool {
        super::commands::key_shell(cx).is_none_or(|key| key == cx.weak_entity())
    }

    /// Whether the sidebar is hidden and still, so the reveal can bring it out.
    pub(super) fn can_reveal(&self) -> bool {
        let still = self.sidebar_motion.is_none() && self.library.is_none();
        self.sidebar_hidden && still && !self.page_fullscreen
    }

    /// Starts the reveal's panel moving out over the page, or away, from wherever it is.
    pub(super) fn set_revealed(&mut self, revealed: bool, cx: &mut Context<Self>) {
        // A Little ilo window has no sidebar to reveal.
        if revealed == self.revealed || (revealed && self.little.is_some()) {
            return;
        }
        self.reveal_motion = Some((self.reveal_out(), crate::motion::start(cx)));
        self.revealed = revealed;
        self.revealed_by_option = false;
        if revealed {
            self.reveal_window = REVEAL_INSET + self.width() + REVEAL_OVERSHOOT;
            self.browser.show_reveal(self.reveal_window, cx);
        } else {
            // The panel takes them away, and the pointer it had.
            self.recent_downloads_gone(cx);
        }
        cx.notify();
    }

    /// Takes the reveal's panel off screen at once.
    pub(super) fn conceal_now(&mut self, cx: &mut Context<Self>) {
        if self.reveal_shown() {
            self.revealed = false;
            self.revealed_by_option = false;
            self.reveal_motion = None;
            self.browser.hide_reveal(cx);
            self.recent_downloads_gone(cx);
        }
    }

    /// The pointer is over the docked sidebar, or not. A hidden one has no pointer over it, so
    /// ⌘S bringing it back shows its tools only once the pointer comes over it.
    pub(super) fn sidebar_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let hovered = hovered && !self.sidebar_hidden;
        if hovered != self.sidebar_hovered {
            self.sidebar_hovered = hovered;
            self.hover_changed = true;
            cx.notify();
        }
    }
}

/// Which of the window's buttons are drawn: its own, or a copy in the Library's panel or in the
/// sidebar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WindowButtons {
    pub window: bool,
    pub library: bool,
    pub sidebar: bool,
}

/// Where the window's buttons are drawn, given whether the Library is `open` or on its way there
/// and whether it's `moving`, whether its panel is `at_rest` where it opens, and whether the
/// sidebar is `docked`. Its own show over the docked sidebar at rest and over the open Library, as
/// in Arc. While the Library moves they show only once its panel is at rest, where the panel's
/// copy was, so the swap can't be seen. The sidebar hides them as it goes, and they come back
/// once it's in. Short of that, the sidebar's copy recedes with it and the panel's slides in.
pub(super) fn window_buttons_at(
    open: bool,
    moving: bool,
    at_rest: bool,
    docked: bool,
) -> WindowButtons {
    let window = if moving {
        at_rest && (open || docked)
    } else {
        open || docked
    };
    WindowButtons {
        window,
        library: (open || moving) && !window,
        sidebar: moving && !window,
    }
}

/// Whether a row `top` down the list is wholly past its top (true) or bottom (false), with the list
/// scrolled `scrolled` and `height` of it showing. None while any of it shows, and before the
/// list's first layout.
fn past(top: f32, scrolled: f32, height: f32) -> Option<bool> {
    match () {
        _ if height <= 0. => None,
        _ if top + ROW_HEIGHT <= scrolled => Some(true),
        _ if top >= scrolled + height => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{
        Bounds, Context, Entity, InputEvent as _, Modifiers, MouseButton, MouseDownEvent, Render,
        ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, Window, WindowBounds,
        WindowOptions, point, prelude::*, px, size,
    };

    use std::ops::DerefMut as _;

    use super::super::{
        Folders, NAME_HEIGHT, PLATE_HEIGHT, REVEAL_INSET, ROW_HEIGHT, SIDEBAR_MAX, SIDEBAR_MIN,
        SIDEBAR_WIDTH, Shell, SidebarRight, Slot,
        drag::detents::{row_y, shell_with},
        recorder::Recorder,
    };
    use crate::tokens::shell::{FOOTER_HEIGHT, NAV_SIZE, NAV_TOP};

    /// A Space made while the sidebar is hidden gets a toast whose Show shows it with the sidebar
    /// back. Spaces made one after another are told of together. None for the window's first
    /// Spaces, one ⌘Z brings back, the form's own, a window behind another, or the sidebar shown.
    #[gpui::test]
    fn a_space_made_with_the_sidebar_hidden_is_toasted(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        let spaces = |ids: &'static [u64]| {
            move |shell: &mut Shell, cx: &mut Context<Shell>| {
                let entry = |&id: &u64| {
                    (
                        id,
                        format!("Space {id}").into(),
                        None,
                        0,
                        "".into(),
                        "".into(),
                        "".into(),
                    )
                };
                shell.set_spaces(ids.iter().map(entry).collect(), cx)
            }
        };
        shell.update(cx, spaces(&[1, 2]));
        assert!(
            host.toasts.borrow().is_empty(),
            "a toast with the sidebar shown"
        );
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        shell.update(cx, spaces(&[1]));
        shell.update(cx, spaces(&[1, 2]));
        assert!(
            host.toasts.borrow().is_empty(),
            "a toast for a Space brought back"
        );
        host.behind.set(true);
        shell.update(cx, spaces(&[1, 2, 3]));
        assert!(
            host.toasts.borrow().is_empty(),
            "a toast from a window behind"
        );
        host.behind.set(false);
        shell.update(cx, spaces(&[1, 2, 3, 4]));
        assert_eq!(*host.toasts.borrow(), ["Added the Space “Space 4”"]);
        shell.update(cx, spaces(&[1, 2, 3, 4, 5]));
        assert_eq!(host.toasts.borrow().last().unwrap(), "Added 2 Spaces");
        let (label, show) = host.toast_run.borrow_mut().take().expect("no Show");
        assert_eq!(label, "Show");
        cx.update(|cx| show(cx));
        assert!(
            !shell.read_with(cx, |shell, _| shell.sidebar_hidden),
            "Show left it hidden"
        );
        assert_eq!(
            host.shown.borrow().last(),
            Some(&4),
            "Show didn't show the first Space"
        );
    }

    /// A tab opening while the sidebar is hidden gets a toast whose Show, as the Spaces' toast has,
    /// brings the sidebar back at its row; tabs moving, or opening with the sidebar shown, get
    /// none.
    #[gpui::test]
    fn a_tab_opened_with_the_sidebar_hidden_is_toasted(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        let tabs = |today: Vec<u64>| {
            move |shell: &mut Shell, cx: &mut Context<Shell>| {
                shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx)
            }
        };
        shell.update(cx, tabs(vec![2, 1]));
        assert!(
            host.toasts.borrow().is_empty(),
            "a toast with the sidebar shown"
        );
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        shell.update(cx, tabs(vec![1, 2]));
        assert!(host.toasts.borrow().is_empty(), "a toast for tabs moved");
        shell.update(cx, tabs(vec![3, 4, 1, 2]));
        assert_eq!(*host.toasts.borrow(), ["2 New Tabs Created"]);
        shell.update(cx, tabs(vec![5, 3, 4, 1, 2]));
        assert_eq!(host.toasts.borrow().last().unwrap(), super::OPENED);
        let (label, show) = host
            .toast_run
            .borrow_mut()
            .take()
            .expect("the toast has no Show");
        assert_eq!(label, "Show");
        cx.update(|cx| show(cx));
        assert!(
            !shell.read_with(cx, |shell, _| shell.sidebar_hidden),
            "Show left it hidden"
        );
    }

    /// ⌥ tapped alone brings the hidden sidebar out wherever the pointer is, which keeps it out
    /// until it has come over the panel and left, or ⌥ is tapped again; the switch turns it off,
    /// and with the sidebar docked it does nothing.
    #[gpui::test]
    fn option_tapped_brings_the_hidden_sidebar_out(cx: &mut TestAppContext) {
        use crate::shell::drag::detents::shell_with;
        let (shell, host) = shell_with(cx, &[], &[1], Default::default());
        let revealed = |cx: &mut TestAppContext| shell.read_with(cx, |shell, _| shell.revealed);
        shell.update(cx, |shell, cx| shell.option_tapped(cx));
        assert!(!revealed(cx), "the docked sidebar came out");
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.option_tapped(cx);
        });
        assert!(revealed(cx), "⌥ didn't bring the hidden sidebar out");
        shell.update(cx, |shell, cx| shell.pointer_at(600., cx));
        assert!(revealed(cx), "the pointer over the page sent it away");
        shell.update(cx, |shell, cx| shell.option_tapped(cx));
        assert!(!revealed(cx), "a second ⌥ didn't send it away");
        // Out again, the pointer comes over it, and leaving it then sends it away.
        shell.update(cx, |shell, cx| {
            shell.reveal_motion = None;
            shell.option_tapped(cx);
            shell.pointer_at(100., cx);
            shell.pointer_at(600., cx);
        });
        assert!(!revealed(cx), "the pointer leaving the panel left it out");
        host.option_off.set(true);
        shell.update(cx, |shell, cx| {
            shell.reveal_motion = None;
            shell.option_tapped(cx);
        });
        assert!(!revealed(cx), "⌥ brought it out with the switch off");
    }

    /// The reveal's window, which draws the shell's panel, as the app's does.
    struct Reveal(Entity<Shell>);

    impl Render for Reveal {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0
                .update(cx, |shell, cx| shell.render_reveal(window, cx))
        }
    }

    /// The hidden reveal's small window, drawing after the browser window's, leaves the docked
    /// list's scroll alone: a tab shown that fits in the browser window scrolls nothing.
    #[gpui::test]
    fn a_hidden_reveal_leaves_the_docked_list_unscrolled(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        let reveal = shell.clone();
        cx.update(|_, cx| {
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(SIDEBAR_WIDTH), px(228.)));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                show: false,
                ..Default::default()
            };
            cx.open_window(options, |_, cx| cx.new(|_| Reveal(reveal)))
                .unwrap();
        });
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        // Tab 4's row fits in the browser window but is past the bottom of the reveal's list.
        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3, 4],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(4, cx);
        });
        draw(&shell, cx);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.list_scroll.offset().y),
            px(0.)
        );
    }

    /// Draws the shell again, as the app's next frame does, by when the list's glide to a row has
    /// landed.
    fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.land_list_glide();
            cx.notify();
        });
        cx.run_until_parked();
    }

    /// A shortcut the page handed back hides the row under the pointer until it moves (#2133).
    /// With the sidebar hidden, its panel's window hears that move: ⌘S from the page, then ⌥, and
    /// the pointer over a row in the panel shows that row's hover.
    #[gpui::test]
    fn a_move_in_the_revealed_panel_shows_its_row_after_a_page_key(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        let reveal = shell.clone();
        let window = cx.update(|_, cx| {
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(SIDEBAR_WIDTH), px(600.)));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            cx.open_window(options, |_, cx| cx.new(|_| Reveal(reveal)))
                .unwrap()
        });
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(2, cx);
            shell.run_for_page(super::super::Command::ToggleSidebar, cx);
            shell.sidebar_motion = None;
            shell.option_tapped(cx);
            shell.reveal_motion = None;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.revealed),
            "⌥ left it in"
        );
        let mut panel = VisualTestContext::from_window(window.into(), cx.deref_mut());
        // Down the panel until the pointer is on a tab's row.
        let mut hovered = None;
        for y in (0..600).step_by(8) {
            panel.simulate_mouse_move(point(px(100.), px(y as f32)), None, Modifiers::none());
            hovered = shell.read_with(&panel, |shell, _| shell.hovered);
            if hovered.is_some() {
                break;
            }
        }
        assert!(hovered.is_some(), "no row in the panel heard the pointer");
        assert!(
            shell.read_with(&panel, |shell, _| shell.pointer_row() == hovered),
            "the panel's row under the pointer drew no hover"
        );
    }

    /// The selected tab's row scrolled out past the list's bottom, and then its top, puts a sliver
    /// of its plate at that edge, and a click on it scrolls the list back to the row, which takes
    /// it away.
    #[gpui::test]
    fn a_marker_shows_where_the_selected_row_scrolled_out_is(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                (10..70).collect(),
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(69, cx);
        });
        draw(&shell, cx);
        // Showing it scrolled the list to its row; the wheel takes the list back up.
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(0.)))
        });
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        let (top, frame) = marker.expect("no marker for the row under the list's bottom");
        assert!(
            !top,
            "the marker for the row under the list's bottom is at its top"
        );
        cx.simulate_click(frame.center(), Modifiers::none());
        draw(&shell, cx);
        let (marker, scrolled) = shell.read_with(cx, |shell, _| {
            (shell.marker_drawn.get(), shell.list_scroll.offset().y)
        });
        assert_eq!(
            marker, None,
            "the marker stayed once its row scrolled into the list"
        );
        assert!(
            scrolled < px(0.),
            "a click on the marker didn't scroll to the row"
        );
        // The top row selected there, past the list's top.
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        draw(&shell, cx);
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), scrolled))
        });
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        assert!(
            matches!(marker, Some((true, _))),
            "no marker at the top: {marker:?}"
        );
    }

    /// The wheel takes the list up, as favorites.sh's does: a marker comes only once the selected
    /// row is wholly under the list's bottom, also for a move past the list's top, which GPUI
    /// clamps only after the shell has drawn, and it stays through the rest of the scroll.
    #[gpui::test]
    fn the_wheel_brings_the_marker_only_for_a_row_wholly_scrolled_out(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        cx.simulate_resize(size(px(1000.), px(340.)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_favorites(vec![(10, 1)], cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![11, 12],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(12, cx);
        });
        draw(&shell, cx);
        // What the shell reads as it draws next, once the wheel has moved the list.
        let wheel = |cx: &mut VisualTestContext, dy: f32| {
            let past = cx.update(|window, cx| {
                let event = ScrollWheelEvent {
                    position: point(px(100.), px(200.)),
                    delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
                    ..Default::default()
                };
                window.dispatch_event(event.to_platform_input(), cx);
                shell.read(cx).selected_row_past()
            });
            cx.run_until_parked();
            past
        };
        // At the list's top the last row still shows its top: a move past it marks nothing.
        assert_eq!(wheel(cx, 50.), None, "a marker for a row that still shows");
        // Shorter, the row is wholly under the list's bottom at its top.
        cx.simulate_resize(size(px(1000.), px(300.)));
        shell.update(cx, |shell, cx| shell.set_selected(12, cx));
        draw(&shell, cx);
        let markers: Vec<_> = (0..8).map(|_| wheel(cx, 10.)).collect();
        let first = markers
            .iter()
            .position(Option::is_some)
            .expect("no marker on the way up");
        assert!(
            markers[first..].iter().all(|&past| past == Some(false)),
            "{markers:?}"
        );
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        let (top, frame) = marker.expect("the marker went once the list was drawn at its top");
        assert!(
            !top,
            "the marker for the row under the list's bottom is at its top"
        );
        cx.simulate_click(frame.center(), Modifiers::none());
        draw(&shell, cx);
        let (marker, scrolled) = shell.read_with(cx, |shell, _| {
            (shell.marker_drawn.get(), shell.list_scroll.offset().y)
        });
        assert_eq!(
            marker, None,
            "the marker stayed once its row scrolled into the list"
        );
        assert!(
            scrolled < px(0.),
            "a click on the marker didn't scroll to the row"
        );
    }

    /// Presses the sidebar's edge where it's drawn, and drags it to `to`, a list of x's.
    fn drag_edge(shell: &Entity<Shell>, cx: &mut TestAppContext, to: &[f32]) {
        shell.update(cx, |shell, cx| {
            let at = shell.width() - 3.;
            let event = MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(at), px(300.)),
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            };
            shell.press_edge(&event, cx);
            for &x in to {
                shell.drag_edge(x - 3., cx);
            }
        });
    }

    /// The edge drags the width between Arc's limits and stops at them, and let go, the width is
    /// where it stopped, with nothing left to settle.
    #[gpui::test]
    fn the_width_stops_at_arcs_limits(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Folders::default());
        drag_edge(&shell, cx, &[300., 900.]);
        assert_eq!(shell.read_with(cx, |shell, _| shell.width()), SIDEBAR_MAX);
        shell.update(cx, |shell, cx| shell.let_go_edge(cx));
        let (width, settling) = shell.read_with(cx, |shell, _| {
            (shell.sidebar_width, shell.width_motion.is_some())
        });
        assert_eq!((width, settling), (SIDEBAR_MAX, false));
        // Short of hiding, the narrowest it goes is the limit.
        drag_edge(&shell, cx, &[SIDEBAR_MIN - 20.]);
        assert_eq!(shell.read_with(cx, |shell, _| shell.width()), SIDEBAR_MIN);
        shell.update(cx, |shell, cx| shell.let_go_edge(cx));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.sidebar_width),
            SIDEBAR_MIN
        );
    }

    /// Dragged on past half the narrowest width, the sidebar hides at once and keeps the width it
    /// had, which ⌘S brings back; letting go then changes nothing.
    #[gpui::test]
    fn dragged_past_its_narrowest_the_sidebar_hides(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Folders::default());
        drag_edge(&shell, cx, &[200., SIDEBAR_MIN, SIDEBAR_MIN / 2. + 1.]);
        assert!(!shell.read_with(cx, |shell, _| shell.sidebar_hidden));
        drag_edge(&shell, cx, &[SIDEBAR_MIN / 2. - 1.]);
        let (hidden, moving) = shell.read_with(cx, |shell, _| {
            (shell.sidebar_hidden, shell.sidebar_motion.is_some())
        });
        assert_eq!((hidden, moving), (true, false));
        shell.update(cx, |shell, cx| shell.let_go_edge(cx));
        let (hidden, width) =
            shell.read_with(cx, |shell, _| (shell.sidebar_hidden, shell.sidebar_width));
        assert_eq!((hidden, width), (true, SIDEBAR_WIDTH));
    }

    /// A shell in a window 340 tall, showing a Space with `today` tabs, drawn.
    fn list_shell(
        today: Vec<u64>,
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        cx.simulate_resize(size(px(1000.), px(340.)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
        });
        draw(&shell, cx);
        (shell, cx)
    }

    /// Scrolled, the rows go under the Space's name row, which stays put over them as Arc 1.170's:
    /// the scrolled part starts under it, and a right-click on it still shows the Space's menu, not
    /// the menu of a row scrolled under it.
    #[gpui::test]
    fn the_rows_scroll_under_the_name_row(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.simulate_resize(size(px(1000.), px(340.)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                (10..40).collect(),
                Vec::new(),
                Folders::default(),
                cx,
            );
        });
        draw(&shell, cx);
        let name_top = shell.read_with(cx, |shell, _| {
            shell.sidebar_origin.get().y + px(shell.drawn_list_top())
        });
        let rows_top = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.list_scroll.bounds().top())
        };
        assert_eq!(
            rows_top(cx),
            name_top + px(NAME_HEIGHT),
            "the rows don't start under the name"
        );
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(-200.)))
        });
        draw(&shell, cx);
        assert_eq!(
            rows_top(cx),
            name_top + px(NAME_HEIGHT),
            "the name row scrolled with the rows"
        );
        let at = point(px(100.), name_top + px(NAME_HEIGHT / 2.));
        host.menus.borrow_mut().clear();
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let menu = host.menus.borrow().last().cloned().unwrap_or_default();
        let space_menu = menu
            .iter()
            .any(|(title, _)| title.as_ref() == "Rename Space…");
        assert!(space_menu, "a right-click on the name row showed {menu:?}");
    }

    /// Rows hidden under the list's foot bring its edge line, and the list scrolled all the way
    /// takes it away, though the room after the last row is still under it: there the last row's
    /// plate ends 121 above the foot, as Arc's.
    #[gpui::test]
    fn the_foot_edge_shows_only_while_rows_are_hidden_under_it(cx: &mut TestAppContext) {
        let (shell, cx) = list_shell((10..30).collect(), cx);
        let edges = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| (shell.top_edge.shown, shell.foot_edge.shown))
        };
        assert_eq!(
            edges(cx),
            (false, true),
            "the rows under the foot brought no edge"
        );
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(-1e4)))
        });
        draw(&shell, cx);
        draw(&shell, cx);
        assert_eq!(
            edges(cx),
            (true, false),
            "the list scrolled all the way kept its foot's edge"
        );
        let ends = shell.read_with(cx, |shell, _| {
            let last = shell.layout()[&Slot::Tab(29)] + shell.tiles_room() - shell.fixed_head();
            let plate_end = last + (ROW_HEIGHT + PLATE_HEIGHT) / 2.;
            let height = f32::from(shell.list_scroll.bounds().size.height);
            height - (plate_end + f32::from(shell.list_scroll.offset().y))
        });
        assert!(
            (ends - 121.).abs() < 0.5,
            "the last row's plate ends {ends} above the foot"
        );
        // Rows that all fit bring no edge at the foot.
        let (shell, cx) = list_shell(vec![10, 11], cx.deref_mut());
        let foot = shell.read_with(cx, |shell, _| shell.foot_edge.shown);
        assert!(!foot, "rows that fit brought the foot's edge");
    }

    /// Wheel ticks of a fraction of a line, as Windows' are, scrolled down and back leave the list
    /// a rounding error off its top, which is its top: the edge under the favorites goes.
    #[gpui::test]
    fn the_list_back_at_its_top_hides_its_edge_after_fractional_ticks(cx: &mut TestAppContext) {
        let (shell, cx) = list_shell((10..30).collect(), cx);
        let tick = 26. * (12. / 120. * 3.);
        let mut offset = 0f32;
        for _ in 0..5 {
            offset -= tick;
        }
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(offset)))
        });
        draw(&shell, cx);
        draw(&shell, cx);
        let top = shell.read_with(cx, |shell, _| shell.top_edge.shown);
        assert!(top, "the list scrolled down showed no edge");
        for _ in 0..5 {
            offset += tick;
        }
        assert!(offset < 0., "the ticks summed back to the top exactly");
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(offset)))
        });
        draw(&shell, cx);
        draw(&shell, cx);
        let top = shell.read_with(cx, |shell, _| shell.top_edge.shown);
        assert!(!top, "the list {offset} off its top kept its edge");
    }

    /// The list glides to the row of a tab shown under its foot, and a scroll of the user's on the
    /// way ends the glide where the user has it.
    #[gpui::test]
    fn a_scroll_on_the_way_to_a_row_ends_the_glide(cx: &mut TestAppContext) {
        let (shell, cx) = list_shell((10..70).collect(), cx);
        shell.update(cx, |shell, cx| shell.set_selected(69, cx));
        let to = shell.read_with(cx, |shell, _| shell.list_glide.get().map(|glide| glide.to));
        assert!(
            to.is_some_and(|to| to > 0.),
            "showing the last tab didn't glide to its row"
        );
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(-37.)))
        });
        draw(&shell, cx);
        let (glide, offset) = shell.read_with(cx, |shell, _| {
            (shell.list_glide.get(), shell.list_scroll.offset().y)
        });
        assert!(glide.is_none(), "the glide went on past the user's scroll");
        assert_eq!(
            offset,
            px(-37.),
            "the glide moved the list from where the user had it"
        );
    }

    /// ⌘T's command bar, with the list scrolled down to its last row, scrolls it back up to the
    /// New Tab row, which ends at the list's top, as Arc's.
    #[gpui::test]
    fn new_tab_scrolls_the_list_to_its_row(cx: &mut TestAppContext) {
        let (shell, cx) = list_shell((10..70).collect(), cx);
        shell.update(cx, |shell, cx| shell.set_selected(69, cx));
        draw(&shell, cx);
        shell.update(cx, |shell, cx| shell.set_new_tab(true, cx));
        draw(&shell, cx);
        let (top, offset) = shell.read_with(cx, |shell, _| {
            let top = shell.layout()[&Slot::NewTab] + shell.tiles_room() - shell.fixed_head();
            (top, -f32::from(shell.list_scroll.offset().y))
        });
        assert_eq!(offset, top, "the New Tab row isn't at the list's top");
    }

    /// With two windows, the pointer at the edge of the one behind leaves its sidebar hidden, and
    /// at the key window's brings it out.
    #[gpui::test]
    fn only_the_key_windows_edge_reveals(cx: &mut TestAppContext) {
        let (behind, _) = shell_with(cx, &[], &[1], Default::default());
        let (key, _) = shell_with(cx, &[], &[2], Default::default());
        for shell in [&behind, &key] {
            shell.update(cx, |shell, cx| {
                shell.sidebar_hidden = true;
                shell.pointer_over(point(px(1.), px(300.)), cx);
                shell.pointer_over(point(px(1.), px(304.)), cx);
            });
        }
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        let revealed = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
            shell.read_with(cx, |shell, _| shell.revealed)
        };
        assert!(
            !revealed(&behind, cx),
            "the window behind brought its sidebar out"
        );
        assert!(
            revealed(&key, cx),
            "the key window's edge didn't bring its sidebar out"
        );
    }

    /// With Settings' edge switch off, the pointer moving along the edge leaves the hidden sidebar
    /// hidden, and ⌘S still brings it back.
    #[gpui::test]
    fn off_the_edge_brings_nothing_out(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Default::default());
        cx.update(|cx| cx.set_global(super::SidebarEdge(false)));
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.pointer_over(point(px(1.), px(300.)), cx);
            shell.pointer_over(point(px(1.), px(304.)), cx);
        });
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert!(
                !shell.revealed,
                "the edge brought the sidebar out while off"
            );
            assert!(shell.reveal_wait.is_none());
        });
        shell.update(cx, |shell, cx| {
            shell.run(crate::shell::Command::ToggleSidebar, "key", cx)
        });
        assert!(
            shell.read_with(cx, |shell, _| !shell.sidebar_hidden),
            "⌘S didn't show it"
        );
    }

    /// A pointer that jumps into the strip at the window's edge and stays leaves the sidebar
    /// hidden, one moving only across it too, and moving a few points up or down in it brings the
    /// sidebar out, as Arc 1.171's does.
    #[gpui::test]
    fn the_pointer_has_to_move_in_the_edge_to_reveal(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Default::default());
        let to = |shell: &Entity<Shell>, x: f32, y: f32, cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| shell.pointer_over(point(px(x), px(y)), cx));
            cx.executor().advance_clock(super::REVEAL_WAIT * 2);
            shell.read_with(cx, |shell, _| shell.revealed)
        };
        shell.update(cx, |shell, _| shell.sidebar_hidden = true);
        assert!(
            !to(&shell, 400., 300., cx),
            "the pointer over the page brought it out"
        );
        assert!(
            !to(&shell, 5., 300., cx),
            "a jump into the edge brought it out"
        );
        assert!(
            !to(&shell, 1., 301., cx),
            "a move across the edge brought it out"
        );
        assert!(
            to(&shell, 1., 304., cx),
            "a few points down the edge didn't bring it out"
        );
        // Out of the strip and back, it's a new jump in.
        shell.update(cx, |shell, cx| {
            shell.set_revealed(false, cx);
            shell.reveal_motion = None;
        });
        assert!(
            !to(&shell, 400., 304., cx),
            "the pointer over the page brought it out"
        );
        assert!(
            !to(&shell, 2., 200., cx),
            "a jump back into the edge brought it out"
        );
        assert!(
            to(&shell, 2., 196., cx),
            "a few points up the edge didn't bring it out"
        );
    }

    /// The same as GPUI tells it, with the sidebar hidden: the window hears the pointer leave its
    /// own view for the page's, whose moves it doesn't hear, so the wait at the edge ends.
    #[gpui::test]
    fn the_hidden_sidebars_window_hears_the_pointer_leave_for_the_page(cx: &mut TestAppContext) {
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(Rc::new(Recorder::default()), cx));
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(1.), px(300.)), None, Modifiers::none());
        cx.simulate_mouse_move(point(px(1.), px(306.)), None, Modifiers::none());
        assert!(
            shell.read_with(cx, |shell, _| shell.reveal_wait.is_some()),
            "the pointer moving down the edge didn't start the wait"
        );
        cx.simulate_event(gpui::MouseExitEvent {
            position: point(px(-1.), px(-1.)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        });
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.revealed),
            "the pointer gone onto the page brought the sidebar out"
        );
    }

    /// A pointer that moves down the edge and goes on onto the page, which on Linux and Windows
    /// is a window the shell doesn't hear the pointer in, leaves the sidebar hidden.
    #[gpui::test]
    fn a_pointer_leaving_the_edge_for_the_page_leaves_the_sidebar_hidden(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Default::default());
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.pointer_over(point(px(1.), px(300.)), cx);
            shell.pointer_over(point(px(1.), px(306.)), cx);
            shell.pointer_left(cx);
        });
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        assert!(
            !shell.read_with(cx, |shell, _| shell.revealed),
            "the pointer gone onto the page brought the sidebar out"
        );
    }

    /// The pointer moving to `x`, `y` over a window, after `wait`.
    fn move_to(shell: &Entity<Shell>, wait: u64, x: f32, y: f32, cx: &mut TestAppContext) {
        cx.executor().advance_clock(Duration::from_millis(wait));
        shell.update(cx, |shell, cx| shell.pointer_over(point(px(x), px(y)), cx));
    }

    /// The pointer leaving the window's own view, after `wait`, as GPUI tells it on Linux and
    /// Windows: a move to (-1, -1), then the leave.
    fn leave(shell: &Entity<Shell>, wait: u64, cx: &mut TestAppContext) {
        cx.executor().advance_clock(Duration::from_millis(wait));
        shell.update(cx, |shell, cx| {
            shell.pointer_over(point(px(-1.), px(-1.)), cx);
            shell.pointer_left(cx);
        });
    }

    /// Whether the sidebar is out once the reveal's wait is over.
    fn out_after_the_wait(shell: &Entity<Shell>, cx: &mut TestAppContext) -> bool {
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| shell.revealed)
    }

    /// A window with its sidebar hidden.
    fn hidden_sidebar(cx: &mut TestAppContext) -> Entity<Shell> {
        let (shell, _) = shell_with(cx, &[], &[1], Default::default());
        shell.update(cx, |shell, _| shell.sidebar_hidden = true);
        shell
    }

    /// Puts the revealed sidebar away and the pointer over the page.
    fn put_away(shell: &Entity<Shell>, cx: &mut TestAppContext) {
        shell.update(cx, |shell, cx| {
            shell.set_revealed(false, cx);
            shell.reveal_motion = None;
        });
        move_to(shell, 0, 400., 300., cx);
    }

    /// A pointer flung at the edge of a window away from the screen's edge overshoots out of the
    /// window and comes back to the edge without moving along it, which brings the sidebar out, as
    /// a pointer the screen's edge stopped would have (#3989).
    #[gpui::test]
    fn a_pointer_back_at_the_edge_after_an_overshoot_reveals(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        move_to(&shell, 0, 400., 300., cx);
        move_to(&shell, 16, 4., 300., cx);
        leave(&shell, 16, cx);
        move_to(&shell, 200, 3., 300., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "back at the edge after an overshoot, the pointer didn't bring the sidebar out"
        );
        // The Mac tells the leave alone.
        put_away(&shell, cx);
        move_to(&shell, 16, 4., 300., cx);
        cx.executor().advance_clock(Duration::from_millis(16));
        shell.update(cx, |shell, cx| shell.pointer_left(cx));
        move_to(&shell, 200, 3., 300., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "back at the edge after a leave alone, the pointer didn't bring the sidebar out"
        );
    }

    /// On Linux and Windows the page is a window the shell doesn't hear the pointer in, so it hears
    /// a pointer coming from the page first in the strip: heard twice on its way to the edge before
    /// it overshoots, its return brings the sidebar out.
    #[gpui::test]
    fn a_pointer_heard_only_at_the_edge_reveals_after_an_overshoot(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        leave(&shell, 0, cx);
        move_to(&shell, 1500, 8., 300., cx);
        move_to(&shell, 8, 4., 300., cx);
        leave(&shell, 8, cx);
        move_to(&shell, 200, 3., 300., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "back at the edge after an overshoot from the page, the pointer didn't bring the \
             sidebar out"
        );
    }

    /// A pointer heard once in the strip before it leaves may have jumped onto the page as well as
    /// out across the edge: its jump back to the edge has to move along it, as one from the page.
    #[gpui::test]
    fn a_pointer_heard_once_at_the_edge_has_to_move_along_it_after_leaving(
        cx: &mut TestAppContext,
    ) {
        let shell = hidden_sidebar(cx);
        leave(&shell, 0, cx);
        move_to(&shell, 1500, 1., 300., cx);
        leave(&shell, 300, cx);
        move_to(&shell, 200, 0., 300., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a pointer jumping back to the edge from the page brought the sidebar out"
        );
    }

    /// A pointer brought slowly to the edge and stopped there reveals as it drifts along the edge,
    /// as before, and as it drifts out across the edge and back.
    #[gpui::test]
    fn a_pointer_stopped_at_the_edge_reveals_as_it_drifts(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        for x in [30., 9., 6., 3.] {
            move_to(&shell, 50, x, 300., cx);
        }
        assert!(
            !out_after_the_wait(&shell, cx),
            "a pointer coming straight in brought the sidebar out"
        );
        move_to(&shell, 300, 3., 304., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "a drift along the edge didn't bring the sidebar out"
        );
        put_away(&shell, cx);
        for x in [30., 9., 6., 3.] {
            move_to(&shell, 50, x, 300., cx);
        }
        move_to(&shell, 300, 1., 300., cx);
        leave(&shell, 100, cx);
        move_to(&shell, 400, 2., 300., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "a drift out across the edge and back didn't bring the sidebar out"
        );
    }

    /// A pointer passing out of the window across the edge, on its way to another window or
    /// display, brings nothing out.
    #[gpui::test]
    fn a_pointer_passing_out_across_the_edge_brings_nothing_out(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        move_to(&shell, 0, 400., 300., cx);
        move_to(&shell, 16, 4., 300., cx);
        leave(&shell, 16, cx);
        cx.executor().advance_clock(super::REVEAL_BACK);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a pointer gone out across the edge brought the sidebar out"
        );
        assert!(shell.read_with(cx, |shell, _| shell.reveal_wait.is_none()));
    }

    /// Back at the edge later than `REVEAL_BACK` after leaving, the pointer has come in afresh and
    /// has to move along the edge.
    #[gpui::test]
    fn a_late_return_has_to_move_along_the_edge(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        move_to(&shell, 0, 400., 300., cx);
        move_to(&shell, 16, 4., 300., cx);
        leave(&shell, 16, cx);
        move_to(&shell, 1100, 3., 300., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a return 1.1 s after leaving brought the sidebar out"
        );
        move_to(&shell, 16, 3., 304., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "a move along the edge after a late return didn't bring the sidebar out"
        );
    }

    /// A pointer that leaves the window's view on a move away from the edge has gone onto the
    /// page, which starts where the strip ends, so its return isn't one from past the edge.
    #[gpui::test]
    fn a_pointer_back_from_the_page_has_to_move_along_the_edge(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        move_to(&shell, 0, 2., 300., cx);
        move_to(&shell, 16, 8., 300., cx);
        leave(&shell, 16, cx);
        move_to(&shell, 50, 4., 300., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a pointer back from the page brought the sidebar out"
        );
    }

    /// A return that counts and goes on in, past the strip, before the wait is out brings
    /// nothing out: a pointer passing through the edge on its way in.
    #[gpui::test]
    fn a_return_passing_on_in_brings_nothing_out(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        move_to(&shell, 0, 400., 300., cx);
        move_to(&shell, 16, 4., 300., cx);
        leave(&shell, 16, cx);
        move_to(&shell, 200, 3., 300., cx);
        move_to(&shell, 50, 30., 300., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a pointer passing on in through the edge brought the sidebar out"
        );
    }

    /// A pointer leaving across the edge while the sidebar shows, docked or revealed, leaves
    /// nothing behind: hidden again, its return is a fresh one in.
    #[gpui::test]
    fn a_leave_with_the_sidebar_out_leaves_nothing_behind(cx: &mut TestAppContext) {
        let shell = hidden_sidebar(cx);
        shell.update(cx, |shell, _| shell.sidebar_hidden = false);
        move_to(&shell, 0, 400., 300., cx);
        move_to(&shell, 16, 4., 300., cx);
        leave(&shell, 16, cx);
        shell.update(cx, |shell, _| shell.sidebar_hidden = true);
        move_to(&shell, 200, 3., 300., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a leave while the sidebar was docked counted after it hid"
        );
        move_to(&shell, 16, 3., 304., cx);
        assert!(out_after_the_wait(&shell, cx), "the edge didn't reveal");
        // Out now: a leave and return leave it out, and once it's away count for nothing.
        move_to(&shell, 16, 4., 310., cx);
        leave(&shell, 16, cx);
        move_to(&shell, 200, 3., 310., cx);
        assert!(
            out_after_the_wait(&shell, cx),
            "the revealed sidebar went away"
        );
        put_away(&shell, cx);
        move_to(&shell, 16, 3., 310., cx);
        assert!(
            !out_after_the_wait(&shell, cx),
            "a leave while the sidebar was out counted after it went away"
        );
    }

    /// A press in the band at the window's edge, on Linux, resizes the window, the window manager
    /// taking the pointer from the strip: back in it as the drag ends, the pointer has come in
    /// afresh and the sidebar stays hidden.
    #[cfg(target_os = "linux")]
    #[gpui::test]
    fn a_pointer_back_from_a_band_drag_leaves_the_sidebar_hidden(cx: &mut TestAppContext) {
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(Rc::new(Recorder::default()), cx));
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(1.), px(300.)), None, Modifiers::none());
        cx.simulate_mouse_down(
            point(px(1.), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_event(gpui::MouseExitEvent {
            position: point(px(1.), px(300.)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
        cx.simulate_mouse_up(
            point(px(1.), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.simulate_mouse_move(point(px(2.), px(300.)), None, Modifiers::none());
        cx.executor().advance_clock(super::REVEAL_WAIT * 2);
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.revealed),
            "the pointer back from a drag of the window's edge brought the sidebar out"
        );
    }

    /// A window showing Today tabs 11 and 12, 12 selected, drawn.
    fn edge_window<'a>(
        cx: &'a mut TestAppContext,
    ) -> (Entity<Shell>, Rc<Recorder>, &'a mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let shell_host = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(shell_host, cx));
        cx.simulate_resize(size(px(1000.), px(600.)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![11, 12],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(12, cx);
        });
        draw(&shell, cx);
        (shell, host, cx)
    }

    /// Whether a press at `x`, `y` across the window takes the sidebar's edge, let go at once.
    fn takes_edge(shell: &Entity<Shell>, cx: &mut VisualTestContext, x: f32, y: f32) -> bool {
        let at = point(px(x), px(y));
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        let took = shell.read_with(cx, |shell, _| shell.resize.is_some());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        took
    }

    /// The edge is 10 wide down the rows, the margin right of them and 2 of their plates: a press
    /// 9 in on a row drags it rather than selecting the tab, and one 11 in selects it.
    #[gpui::test]
    fn the_edge_takes_10_of_a_rows_side(cx: &mut TestAppContext) {
        let (shell, host, cx) = edge_window(cx);
        let y = shell.read_with(cx, |shell, _| row_y(shell, 11));
        host.take();
        assert!(
            takes_edge(&shell, cx, SIDEBAR_WIDTH - 9., y),
            "a press 9 in from the sidebar's edge didn't take the edge"
        );
        assert!(
            !host.take().contains(&"select 11".to_string()),
            "a press 9 in from the sidebar's edge selected the row's tab"
        );
        assert_eq!(shell.read_with(cx, |shell, _| shell.width()), SIDEBAR_WIDTH);
        assert!(
            !takes_edge(&shell, cx, SIDEBAR_WIDTH - 11., y),
            "a press 11 in from the sidebar's edge took the edge"
        );
        assert!(
            host.take().contains(&"select 11".to_string()),
            "a click 11 in from the sidebar's edge didn't select the row's tab"
        );
    }

    /// The header's buttons and the footer's + keep their plates: beside reload the edge is the 6
    /// past it, and beside the + the 8 past that.
    #[gpui::test]
    fn the_edge_leaves_the_buttons_their_plates(cx: &mut TestAppContext) {
        let (shell, _, cx) = edge_window(cx);
        let header = NAV_TOP + NAV_SIZE / 2.;
        assert!(!takes_edge(&shell, cx, SIDEBAR_WIDTH - 7., header));
        assert!(takes_edge(&shell, cx, SIDEBAR_WIDTH - 5., header));
        let foot = 600. - FOOTER_HEIGHT / 2.;
        assert!(!takes_edge(&shell, cx, SIDEBAR_WIDTH - 9., foot));
        assert!(takes_edge(&shell, cx, SIDEBAR_WIDTH - 7., foot));
    }

    /// A drag from the edge that ends over a row's close button, the sidebar stopped at its
    /// narrowest, closes nothing.
    #[gpui::test]
    fn a_drag_from_the_edge_let_go_over_a_close_button_closes_nothing(cx: &mut TestAppContext) {
        let (shell, host, cx) = edge_window(cx);
        let y = shell.read_with(cx, |shell, _| row_y(shell, 12));
        cx.simulate_mouse_down(
            point(px(SIDEBAR_WIDTH - 5.), px(y)),
            MouseButton::Left,
            Modifiers::none(),
        );
        // 24 in from the narrowest sidebar's edge, inside the close button, 13 to 39 in.
        let over_close = point(px(SIDEBAR_MIN - 24.), px(y));
        cx.simulate_mouse_move(over_close, Some(MouseButton::Left), Modifiers::none());
        host.take();
        cx.simulate_mouse_up(over_close, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(
            !host.take().iter().any(|call| call.starts_with("close")),
            "a drag from the edge let go over a close button closed something"
        );
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.sidebar_width),
            SIDEBAR_MIN
        );
    }

    /// On the right the edge is the sidebar's left 10, mirrored.
    #[gpui::test]
    fn on_the_right_the_edge_is_the_sidebars_left_10(cx: &mut TestAppContext) {
        let (shell, host, cx) = edge_window(cx);
        cx.update(|_, cx| {
            cx.set_global(SidebarRight(true));
            cx.refresh_windows();
        });
        draw(&shell, cx);
        let (x, y) = shell.read_with(cx, |shell, _| (shell.column_x(), row_y(shell, 11)));
        host.take();
        assert!(
            takes_edge(&shell, cx, x + 9., y),
            "a press 9 in didn't take the edge"
        );
        assert!(
            !takes_edge(&shell, cx, x + 11., y),
            "a press 11 in took the edge"
        );
        assert!(host.take().contains(&"select 11".to_string()));
    }

    /// The reveal's panel out over the page as the window's edge brings it, in a window of its own
    /// at the browser window's left, with Today tabs 2 and 3, and that window's context.
    fn revealed_window<'a>(
        cx: &'a mut TestAppContext,
    ) -> (Entity<Shell>, &'a mut VisualTestContext, VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        cx.simulate_resize(size(px(1000.), px(600.)));
        let reveal = shell.clone();
        let window = cx.update(|_, cx| {
            let wide = REVEAL_INSET + SIDEBAR_WIDTH;
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(wide), px(600.)));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            cx.open_window(options, |_, cx| cx.new(|_| Reveal(reveal)))
                .unwrap()
        });
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(2, cx);
            shell.run_for_page(super::super::Command::ToggleSidebar, cx);
            shell.sidebar_motion = None;
            shell.option_tapped(cx);
            // Out as the window's edge brings it, which the pointer's leaving sends away.
            shell.revealed_by_option = false;
            shell.reveal_motion = None;
            cx.notify();
        });
        cx.run_until_parked();
        let panel = VisualTestContext::from_window(window.into(), cx.deref_mut());
        (shell, cx, panel)
    }

    /// The reveal's panel has the edge too, inside its own: a drag sizes the panel and the one
    /// width the docked sidebar keeps, stops at the narrowest rather than hiding anything, and
    /// moves the page not at all. Through the drag the panel stays out wherever the pointer goes,
    /// over the page or out of the window; let go, it goes from where the pointer is then.
    #[gpui::test]
    fn the_revealed_panels_edge_drags_the_width_and_keeps_it_out(cx: &mut TestAppContext) {
        let (shell, cx, mut panel) = revealed_window(cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.revealed),
            "⌥ left it in"
        );
        let page = shell.read_with(cx, |shell, _| shell.card_left());
        let edge = REVEAL_INSET + SIDEBAR_WIDTH - 5.;
        panel.simulate_mouse_down(
            point(px(edge), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        panel.run_until_parked();
        let (resizing, window) = shell.read_with(&panel, |shell, _| {
            (shell.resize.is_some(), shell.reveal_window)
        });
        assert!(
            resizing,
            "a press 5 in from the panel's edge didn't take its edge"
        );
        assert_eq!(
            window,
            REVEAL_INSET + SIDEBAR_MAX,
            "the panel's window didn't widen for the drag"
        );
        panel.simulate_mouse_move(
            point(px(edge + 100.), px(300.)),
            Some(MouseButton::Left),
            Modifiers::none(),
        );
        assert_eq!(
            shell.read_with(&panel, |shell, _| shell.width()),
            SIDEBAR_WIDTH + 100.
        );
        // Over the page, and then out of the window, the button still down.
        shell.update(&mut panel, |shell, cx| {
            shell.pointer_at(900., cx);
            shell.pointer_at(f32::MAX, cx);
        });
        panel.simulate_mouse_move(
            point(px(20.), px(300.)),
            Some(MouseButton::Left),
            Modifiers::none(),
        );
        panel.run_until_parked();
        let (out, hidden, width, moving, card) = shell.read_with(&panel, |shell, _| {
            (
                shell.revealed,
                shell.sidebar_hidden,
                shell.width(),
                shell.page_moving_told,
                shell.card_left(),
            )
        });
        assert!(out, "the panel went while its edge was dragged");
        assert!(hidden, "the drag docked the sidebar");
        assert_eq!(
            width, SIDEBAR_MIN,
            "past the narrowest the panel didn't stop there"
        );
        assert!(!moving, "the panel's drag told the core the page moves");
        assert_eq!(card, page, "the panel's drag moved the page");
        // Let go with the pointer last heard out of the window, it goes.
        panel.simulate_mouse_up(
            point(px(20.), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        panel.run_until_parked();
        let (out, width) =
            shell.read_with(&panel, |shell, _| (shell.revealed, shell.sidebar_width));
        assert_eq!(
            width, SIDEBAR_MIN,
            "the docked sidebar's width isn't the panel's"
        );
        assert!(!out, "let go with the pointer gone, the panel stayed");
    }

    /// Let go with the pointer still over it, the panel stays out, and its window ends at its edge
    /// again.
    #[gpui::test]
    fn let_go_over_the_panel_it_stays_out(cx: &mut TestAppContext) {
        let (shell, _, mut panel) = revealed_window(cx);
        let edge = REVEAL_INSET + SIDEBAR_WIDTH - 5.;
        panel.simulate_mouse_down(
            point(px(edge), px(300.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        let to = point(px(edge + 50.), px(300.));
        panel.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        shell.update(&mut panel, |shell, cx| shell.pointer_at(edge + 40., cx));
        panel.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        panel.run_until_parked();
        let (out, width, window) = shell.read_with(&panel, |shell, _| {
            (shell.revealed, shell.sidebar_width, shell.reveal_window)
        });
        assert!(out, "let go over the panel, it went");
        assert_eq!(width, SIDEBAR_WIDTH + 50.);
        assert_eq!(
            window,
            REVEAL_INSET + SIDEBAR_WIDTH + 50.,
            "its window didn't end at its edge"
        );
    }
}
