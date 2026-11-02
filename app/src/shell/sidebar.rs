//! The sidebar's width, which its edge drags; hiding and showing it, and while it's
//! hidden, its reveal over the page (docs/ux-spec.md › Sidebar reveal); its gradient in the
//! Space's colour; and the marker for the selected tab's row scrolled out of the list.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, ClickEvent, Context, CursorStyle, DispatchPhase, Hsla, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Rgba, canvas, div, linear_color_stop, linear_gradient, point,
    prelude::*, px, size,
};

use super::access::Accessible as _;
use crate::tokens::shell::{SHOWN_MARKER, SHOWN_MARKER_HIT};

use super::{
    PLATE_INSET, ROW_HEIGHT, Row, Slot,
    CARD_INSET, PRIVATE_SHADE, REVEAL_HOLD, REVEAL_INSET, REVEAL_IN_DAMPING, REVEAL_IN_RESPONSE, REVEAL_OUT_DAMPING,
    REVEAL_OUT_RESPONSE, REVEAL_OVERSHOOT, REVEAL_WAIT, SIDEBAR_EDGE, SIDEBAR_HIDE, SIDEBAR_MAX,
    SIDEBAR_MIN, SIDEBAR_SHOW, SIDEBAR_WIDTH, Settle, Shell, Space, settle_progress, spring,
};

/// What the toast says when a tab opens with the sidebar hidden, Arc's words, and for how long
/// (ours: Arc's isn't measured).
pub(super) const OPENED: &str = "New Tab Created";
const OPENED_TIME: Duration = Duration::from_secs(4);

impl Shell {
    /// The docked sidebar's width as drawn: with the pointer while its edge is dragged, then on its
    /// way to where it rests.
    pub(super) fn width(&self) -> f32 {
        if let Some((_, way)) = self.resize {
            return way.clamp(SIDEBAR_MIN, SIDEBAR_MAX).round();
        }
        let Some((from, start)) = self.width_motion else {
            return self.sidebar_width;
        };
        settle_progress(start)
            .map_or(self.sidebar_width, |progress| from + (self.sidebar_width - from) * progress)
    }

    /// The docked sidebar's right edge, which drags its width and puts it back on a double-click.
    pub(super) fn sidebar_edge(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let width = self.width();
        let cursor = match width {
            width if width <= SIDEBAR_MIN => CursorStyle::ResizeRight,
            width if width >= SIDEBAR_MAX => CursorStyle::ResizeLeft,
            _ => CursorStyle::ResizeLeftRight,
        };
        let resizing = self.resize.is_some();
        let shell = cx.weak_entity();
        div()
            .id("sidebar-edge")
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(width - SIDEBAR_EDGE))
            .w(px(SIDEBAR_EDGE))
            .cursor(cursor)
            .block_mouse_except_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseDownEvent, _, cx| shell.press_edge(event, cx)),
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
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                let x = event.position.x.into();
                                moved.update(cx, |shell, cx| shell.drag_edge(x, cx)).ok();
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
        self.resize = Some((f32::from(event.position.x) - width, width));
        cx.notify();
    }

    /// The pointer dragging the edge is at `x`. Past half the narrowest width, as AppKit's split
    /// view collapses a pane, the sidebar hides in one frame and keeps the width it had (Arc).
    pub(super) fn drag_edge(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((grab, _)) = self.resize else {
            return;
        };
        if x - grab >= SIDEBAR_MIN / 2. {
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
        }
    }

    /// Settles the docked sidebar at `to` wide from `from`, and keeps the width for the next
    /// launch.
    fn set_width(&mut self, from: f32, to: f32, cx: &mut Context<Self>) {
        let to = to.round();
        if to != self.sidebar_width {
            self.sidebar_width = to;
            self.browser.set_sidebar_width(to, cx);
        }
        self.width_motion = (from != to).then(|| (from, Instant::now()));
        if self.width_motion.is_none() {
            // check.sh waits for these lines, then reads where the page is.
            self.logged_once_laid.push(format!("browser: sidebar width {to}"));
        }
        cx.notify();
    }

    /// How far the sidebar has moved since `start`, or None once it's there.
    pub(super) fn sidebar_progress(&self, start: Instant) -> Option<f32> {
        let t = start.elapsed().as_secs_f32();
        let time = if self.sidebar_hidden { SIDEBAR_HIDE } else { SIDEBAR_SHOW };
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
        self.sidebar_motion = Some((from, Instant::now()));
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
        let shut: Vec<u64> = space.holders(row).filter(|&folder| !space.is_open(folder)).collect();
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
    /// says so, and a click on it reveals the top one's row (Arc's release notes).
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
        self.browser.toast_button(&text, (None, "show"), show, OPENED_TIME, cx);
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
        let made: Vec<u64> = made.into_iter().filter(|&space| Some(space) != form).collect();
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
                format!("Added the Space “{}”", name.map_or("", |space| space.name.as_ref()))
            }
            count => format!("Added {count} Spaces"),
        };
        let shell = cx.entity().downgrade();
        let show = Rc::new(move |cx: &mut App| {
            shell.update(cx, |shell, cx| shell.show_made_space(first, cx)).ok();
        });
        self.browser.toast_button(&text, (Some("Show"), "show"), show, OPENED_TIME, cx);
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
        let Row::Tab(row) = self.selected_row() else {
            return None;
        };
        let top = *self.layout().get(&Slot::Tab(row))? + self.tiles_room();
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
            let y = if top { list.top() } else { list.bottom() - px(SHOWN_MARKER_HIT) };
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
            .bg(self.palette.selected(0.));
        let marker = div()
            .id("shown-marker")
            .absolute()
            .left(px(PLATE_INSET))
            .right(px(PLATE_INSET))
            .h(px(SHOWN_MARKER_HIT))
            .flex()
            .flex_col()
            .map(|marker| if top { marker.top_0() } else { marker.bottom_0().justify_end() })
            .block_mouse_except_scroll()
            .when(self.a11y, |marker| marker.button_named("Scroll to the Selected Tab"))
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
        // A Little ilo window's are over its bar, where the window's own go.
        if self.little.is_some() {
            return WindowButtons { window: true, library: false, sidebar: false };
        }
        window_buttons_at(
            self.library_open,
            self.library_moving(),
            self.library_at_rest(),
            !self.sidebar_hidden && self.sidebar_motion.is_none(),
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
        self.reveal_step().unwrap_or(if self.revealed { 1. } else { 0. })
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

    /// The gradient down the window, at `y` from its top (0) to its bottom (1), where the sidebar
    /// is: the Space's colour, going to the same at HSL lightness + 0.25, or its theme's colours.
    /// While the sidebar pages between two Spaces, it cross-fades from one's to the other's as the
    /// pages move.
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
        Rgba { r, g, b, a: mix(from.a, to.a) }.into()
    }

    /// `space`'s colour `x` of the way across the window and `y` of the way down, as the gradient
    /// has it: its theme's, or nothing over the underlay without one.
    pub(super) fn space_color_at(&self, space: u64, x: f32, y: f32) -> Rgba {
        if self.private {
            return Rgba { r: 0., g: 0., b: 0., a: PRIVATE_SHADE };
        }
        match self.space_theme(space) {
            Some(theme) => self.theme_color_at(&theme, self.palette_of(space).layer_alpha, x, y),
            None => Rgba { r: 0., g: 0., b: 0., a: 0. },
        }
    }

    /// `space`'s gradient from top to bottom, for what is as tall as the window, such as its card
    /// in the Library.
    pub(super) fn space_gradient(&self, space: u64) -> gpui::Background {
        // Across the window's middle, for a theme of more than one colour.
        let stop = |y| linear_color_stop(Hsla::from(self.space_color_at(space, 0.5, y)), y);
        linear_gradient(180., stop(0.), stop(1.))
    }

    /// The gradient as a background, over `base` where the window's look has to be opaque.
    pub(super) fn gradient(&self, base: Option<Hsla>) -> gpui::Background {
        let stop = |y| {
            let color = self.gradient_color(y);
            linear_color_stop(base.map_or(color, |base| base.blend(color)), y)
        };
        linear_gradient(180., stop(0.), stop(1.))
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

    /// The pointer is at `x` in the browser window: kept at its left edge it brings the hidden
    /// sidebar out over the page, and more than the hold zone past the panel's edge, and off a
    /// row's hover card, it sends it away, and the card with it.
    pub fn pointer_at(&mut self, x: f32, cx: &mut Context<Self>) {
        self.reveal_x = x;
        if !self.can_reveal() {
            return;
        }
        let held = REVEAL_INSET + self.width() + REVEAL_HOLD;
        if self.revealed_by_option {
            if x > held {
                return;
            }
            self.revealed_by_option = false;
        }
        // The edge is the card's margin. Left of the window, the pointer has gone to another
        // window or display.
        if !(0. ..CARD_INSET).contains(&x) {
            self.reveal_wait = None;
            // A question keeps it out until it's answered, the Cast card and its menu until
            // they close.
            if x > held
                && self.revealed
                && !self.pointer_on_card
                && self.question().is_none()
                && self.agent_question.is_none()
                && self.cast.is_none()
                && !self.menu_open
            {
                self.set_revealed(false, cx);
                self.browser.hover_card(crate::hover_card::Hover::Close, cx);
            }
        } else if !self.revealed && self.reveal_wait.is_none() {
            // Arc waits a moment, so a pointer passing the edge on its way to another window or
            // display leaves the sidebar where it is.
            self.reveal_wait = Some(cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(REVEAL_WAIT).await;
                shell
                    .update(cx, |shell, cx| {
                        shell.reveal_wait = None;
                        if shell.can_reveal() {
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
        self.reveal_motion = Some((self.reveal_out(), Instant::now()));
        self.revealed = revealed;
        self.revealed_by_option = false;
        if revealed {
            self.browser.show_reveal(REVEAL_INSET + self.width() + REVEAL_OVERSHOOT, cx);
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
    let window = if moving { at_rest && (open || docked) } else { open || docked };
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
    use std::rc::Rc;

    use gpui::{
        Bounds, Context, Entity, InputEvent as _, Modifiers, MouseButton, MouseDownEvent, Render,
        ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, Window, WindowBounds,
        WindowOptions, point, prelude::*, px, size,
    };

    use super::super::{
        Folders, SIDEBAR_MAX, SIDEBAR_MIN, SIDEBAR_WIDTH, Shell, drag::detents::shell_with,
        recorder::Recorder,
    };

    /// A Space made while the sidebar is hidden gets a toast whose Show shows it with the sidebar
    /// back. Spaces made one after another are told of together. None for the window's first
    /// Spaces, one ⌘Z brings back, the form's own, a window behind another, or the sidebar shown.
    #[gpui::test]
    fn a_space_made_with_the_sidebar_hidden_is_toasted(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        let spaces = |ids: &'static [u64]| {
            move |shell: &mut Shell, cx: &mut Context<Shell>| {
                let entry = |&id: &u64| {
                    (id, format!("Space {id}").into(), None, 0, "".into(), "".into(), "".into())
                };
                shell.set_spaces(ids.iter().map(entry).collect(), cx)
            }
        };
        shell.update(cx, spaces(&[1, 2]));
        assert!(host.toasts.borrow().is_empty(), "a toast with the sidebar shown");
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        shell.update(cx, spaces(&[1]));
        shell.update(cx, spaces(&[1, 2]));
        assert!(host.toasts.borrow().is_empty(), "a toast for a Space brought back");
        host.behind.set(true);
        shell.update(cx, spaces(&[1, 2, 3]));
        assert!(host.toasts.borrow().is_empty(), "a toast from a window behind");
        host.behind.set(false);
        shell.update(cx, spaces(&[1, 2, 3, 4]));
        assert_eq!(*host.toasts.borrow(), ["Added the Space “Space 4”"]);
        shell.update(cx, spaces(&[1, 2, 3, 4, 5]));
        assert_eq!(host.toasts.borrow().last().unwrap(), "Added 2 Spaces");
        let show = host.toast_run.borrow_mut().take().expect("no Show");
        cx.update(|cx| show(cx));
        assert!(!shell.read_with(cx, |shell, _| shell.sidebar_hidden), "Show left it hidden");
        assert_eq!(host.shown.borrow().last(), Some(&4), "Show didn't show the first Space");
    }

    /// A tab opening while the sidebar is hidden gets a toast whose click brings the sidebar back at
    /// its row; tabs moving, or opening with the sidebar shown, get none.
    #[gpui::test]
    fn a_tab_opened_with_the_sidebar_hidden_is_toasted(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[1], Folders::default());
        let tabs = |today: Vec<u64>| {
            move |shell: &mut Shell, cx: &mut Context<Shell>| {
                shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx)
            }
        };
        shell.update(cx, tabs(vec![2, 1]));
        assert!(host.toasts.borrow().is_empty(), "a toast with the sidebar shown");
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        shell.update(cx, tabs(vec![1, 2]));
        assert!(host.toasts.borrow().is_empty(), "a toast for tabs moved");
        shell.update(cx, tabs(vec![3, 4, 1, 2]));
        assert_eq!(*host.toasts.borrow(), ["2 New Tabs Created"]);
        shell.update(cx, tabs(vec![5, 3, 4, 1, 2]));
        assert_eq!(host.toasts.borrow().last().unwrap(), super::OPENED);
        let click = host.toast_run.borrow_mut().take().expect("the toast can't be clicked");
        cx.update(|cx| click(cx));
        assert!(!shell.read_with(cx, |shell, _| shell.sidebar_hidden), "the click left it hidden");
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
            self.0.update(cx, |shell, cx| shell.render_reveal(window, cx))
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
            cx.open_window(options, |_, cx| cx.new(|_| Reveal(reveal))).unwrap();
        });
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        // Tab 4's row fits in the browser window but is past the bottom of the reveal's list.
        let offset = shell.update(cx, |shell, cx| {
            shell.set_tabs(1, Vec::new(), vec![2, 3, 4], Vec::new(), Folders::default(), cx);
            shell.set_selected(4, cx);
            shell.list_scroll.offset().y
        });
        assert_eq!(offset, px(0.));
    }

    /// Draws the shell again, as the app's next frame does.
    fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
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
            shell.set_tabs(1, Vec::new(), (10..70).collect(), Vec::new(), Folders::default(), cx);
            shell.set_selected(69, cx);
        });
        draw(&shell, cx);
        // Showing it scrolled the list to its row; the wheel takes the list back up.
        shell.update(cx, |shell, _| shell.list_scroll.set_offset(point(px(0.), px(0.))));
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        let (top, frame) = marker.expect("no marker for the row under the list's bottom");
        assert!(!top, "the marker for the row under the list's bottom is at its top");
        cx.simulate_click(frame.center(), Modifiers::none());
        draw(&shell, cx);
        let (marker, scrolled) = shell.read_with(cx, |shell, _| {
            (shell.marker_drawn.get(), shell.list_scroll.offset().y)
        });
        assert_eq!(marker, None, "the marker stayed once its row scrolled into the list");
        assert!(scrolled < px(0.), "a click on the marker didn't scroll to the row");
        // The top row selected there, past the list's top.
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        draw(&shell, cx);
        shell.update(cx, |shell, _| shell.list_scroll.set_offset(point(px(0.), scrolled)));
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        assert!(matches!(marker, Some((true, _))), "no marker at the top: {marker:?}");
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
            shell.set_tabs(1, Vec::new(), vec![11, 12], Vec::new(), Folders::default(), cx);
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
        let first = markers.iter().position(Option::is_some).expect("no marker on the way up");
        assert!(markers[first..].iter().all(|&past| past == Some(false)), "{markers:?}");
        draw(&shell, cx);
        let marker = shell.read_with(cx, |shell, _| shell.marker_drawn.get());
        let (top, frame) = marker.expect("the marker went once the list was drawn at its top");
        assert!(!top, "the marker for the row under the list's bottom is at its top");
        cx.simulate_click(frame.center(), Modifiers::none());
        draw(&shell, cx);
        let (marker, scrolled) = shell.read_with(cx, |shell, _| {
            (shell.marker_drawn.get(), shell.list_scroll.offset().y)
        });
        assert_eq!(marker, None, "the marker stayed once its row scrolled into the list");
        assert!(scrolled < px(0.), "a click on the marker didn't scroll to the row");
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
        assert_eq!(shell.read_with(cx, |shell, _| shell.sidebar_width), SIDEBAR_MIN);
    }

    /// Dragged on past half the narrowest width, the sidebar hides at once and keeps the width it
    /// had, which ⌘S brings back; letting go then changes nothing.
    #[gpui::test]
    fn dragged_past_its_narrowest_the_sidebar_hides(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1], Folders::default());
        drag_edge(&shell, cx, &[200., SIDEBAR_MIN, SIDEBAR_MIN / 2. + 1.]);
        assert!(!shell.read_with(cx, |shell, _| shell.sidebar_hidden));
        drag_edge(&shell, cx, &[SIDEBAR_MIN / 2. - 1.]);
        let (hidden, moving) =
            shell.read_with(cx, |shell, _| (shell.sidebar_hidden, shell.sidebar_motion.is_some()));
        assert_eq!((hidden, moving), (true, false));
        shell.update(cx, |shell, cx| shell.let_go_edge(cx));
        let (hidden, width) =
            shell.read_with(cx, |shell, _| (shell.sidebar_hidden, shell.sidebar_width));
        assert_eq!((hidden, width), (true, SIDEBAR_WIDTH));
    }
}
