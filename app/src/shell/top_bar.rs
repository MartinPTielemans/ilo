//! Top bar (#3160), Settings › Tabs & Sidebar's, off by default: the header's row widened across
//! the window's top, its buttons where they are in the sidebar, and the URL field moved into it
//! from the sidebar's edge to the card's right edge. The card starts under it and the sidebar's
//! list rises into the field's place. Off, none of it is in the tree, and layout reads one bool.

use gpui::{
    Bounds, Context, MouseButton, MouseDownEvent, Pixels, Point, div, point, prelude::*, px, size,
};

use crate::tokens::shell::{TOP_BAR, URL_FIELD_HEIGHT, WINDOWS_BUTTON_PLATE};

use super::{CARD_INSET, LIST_TOP, PLATE_INSET, Shell, URL_FIELD_TOP, WINDOW_BUTTONS};

/// Windows has the window's buttons at its right corner, where the bar puts them; the Mac's and
/// Linux's stay with the header's (#3160).
const BUTTONS_AT_BAR_END: bool = cfg!(target_os = "windows");

impl Shell {
    /// Follows Top bar: the window lays itself out with the bar, or without.
    pub(super) fn set_top_bar(&mut self, on: bool, cx: &mut Context<Self>) {
        self.top_bar = on;
        // check.sh waits for this line.
        log!("browser: top bar {}", if on { "on" } else { "off" });
        self.update_menus(cx);
        cx.notify();
    }

    /// Whether this window draws the bar: Top bar on, and no page full screen, which takes the
    /// window with the sidebar's row as it does today. A Little ilo window has its own bar.
    pub(super) fn bar_on(&self) -> bool {
        self.top_bar && !self.page_fullscreen
    }

    /// `card` under the bar while it's on: its top at the bar's bottom, its other edges as they
    /// are.
    pub(super) fn under_bar(&self, card: Bounds<Pixels>) -> Bounds<Pixels> {
        if !self.bar_on() || self.little.is_some() {
            return card;
        }
        let top = px(TOP_BAR);
        Bounds::new(
            point(card.origin.x, top),
            size(card.size.width, (card.bottom() - top).max(px(0.))),
        )
    }

    /// How much lower the card's top is for the bar: what the find bar, the zoom HUD and the
    /// toast, placed from the window's top by the card's inset, go down by to stay on the page.
    pub(super) fn bar_drop(&self) -> f32 {
        match self.bar_on() && self.little.is_none() {
            true => TOP_BAR - CARD_INSET,
            false => 0.,
        }
    }

    /// Whether the window's buttons are at the bar's right end, away from the sidebar on the left.
    /// With the sidebar on the right they're there already, in its column, and full screen has
    /// none.
    pub(super) fn buttons_at_bar_end(&self) -> bool {
        // Mock only: PREVIEW_BUTTONS_AT_END puts Linux's at the bar's end as Windows has them.
        (BUTTONS_AT_BAR_END || std::env::var("PREVIEW_BUTTONS_AT_END").is_ok())
            && self.bar_on()
            && self.little.is_none()
            && !self.right
            && !self.full_screen
    }

    /// Where the sidebar's list starts: under the URL field, or in its place with the bar on.
    pub(super) fn list_start(&self) -> f32 {
        match self.bar_on() {
            true => URL_FIELD_TOP,
            false => LIST_TOP,
        }
    }

    /// The URL field's box in the bar, across the window: from the docked sidebar's edge to the
    /// card's right edge, or the window's buttons there, mirrored with the sidebar on the right,
    /// centred on the header's line. The sidebar hidden, it stays where it is, as do the header's
    /// buttons.
    pub(super) fn bar_field_bounds(&self) -> Bounds<Pixels> {
        let window = f32::from(self.viewport.width);
        // The buttons' far edge from the corner, and the card's inset past it.
        let buttons = match self.buttons_at_bar_end() {
            true if BUTTONS_AT_BAR_END => WINDOW_BUTTONS[2].0 + WINDOWS_BUTTON_PLATE / 2.,
            true => f32::from(self.window_button_frames()[2].right()),
            false => 0.,
        };
        let (left, right) = match self.right {
            true => (CARD_INSET, window - self.width()),
            false => (self.width(), window - buttons - CARD_INSET),
        };
        Bounds::new(
            point(px(left), px((TOP_BAR - URL_FIELD_HEIGHT) / 2.)),
            size(px((right - left).max(0.)), px(URL_FIELD_HEIGHT)),
        )
    }

    /// The URL field's box in the window, wherever it's drawn: in the bar, or in the docked
    /// sidebar's column.
    pub(super) fn field_frame(&self) -> Bounds<Pixels> {
        match self.bar_on() {
            true => self.bar_field_bounds(),
            false => Bounds::new(
                point(px(self.column_x() + PLATE_INSET), px(URL_FIELD_TOP)),
                size(px(self.width() - 2. * PLATE_INSET), px(URL_FIELD_HEIGHT)),
            ),
        }
    }

    /// The URL field's width, as `url_field` lays it out and `field_part_at` reads it.
    pub(super) fn field_width(&self) -> f32 {
        f32::from(self.field_frame().size.width)
    }

    /// The bar's own menu, at `position`: Hide Top Bar, the way out where the bar is seen.
    pub(super) fn top_bar_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let items = vec![self.top_bar_item(cx)];
        self.show_menu(items, position, None, cx);
    }

    /// The bar: the header's buttons over the sidebar's column, the URL field beside them, and a
    /// right-click anywhere on it for Hide Top Bar, the way out where the bar is seen. Its empty
    /// space moves the window, as the sidebar's header does. None while it's off.
    pub(super) fn top_bar_row(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if !self.bar_on() || self.little.is_some() {
            return None;
        }
        let buttons = div()
            .absolute()
            .top_0()
            .left(px(self.column_x()))
            .w(px(self.width()))
            .h(px(TOP_BAR))
            .children(self.nav_buttons(cx));
        // url_field places itself in the sidebar's column, so its column goes where the field is.
        let field = self.bar_field_bounds();
        let column = div()
            .absolute()
            .left(field.left() - px(PLATE_INSET))
            .top(field.top() - px(URL_FIELD_TOP))
            .w(field.size.width + px(2. * PLATE_INSET))
            .h(px(URL_FIELD_TOP + URL_FIELD_HEIGHT))
            .child(self.url_field(cx));
        Some(
            div()
                .id("top-bar")
                .absolute()
                .top_0()
                .left_0()
                .w(self.viewport.width)
                .h(px(TOP_BAR))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                        shell.top_bar_menu(event.position, cx)
                    }),
                )
                .child(buttons)
                .child(column),
        )
    }
}
