//! Settings' card: the section's page on the window's card, scrolling, with the section's name and
//! the heading scrolled past held at its top, so the user keeps their place in a long section, and
//! its foot fading into its fill where rows go on.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    BoxShadow, FontWeight, Rgba, ScrollHandle, SharedString, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, rgb, rgba,
};

use crate::palette::OVERLAY_DARK;
use crate::tokens::palette::DIALOG_LIGHT;
use crate::tokens::settings::{
    CARD_RIM, CARD_RING, CARD_SHADOW, FADE, HELD_TITLE, PAGE_BOTTOM, PAGE_TOP, PAGE_X, TEXT_X,
};
use crate::tokens::shell::{CARD_INSET, CARD_RADIUS};

#[cfg(test)]
mod tests;

/// What the card holds between frames: its scroll, and where its headings and its top were drawn.
#[derive(Default)]
pub struct Card {
    pub scroll: ScrollHandle,
    /// Each heading the page marked, with its top in the window, in the page's order.
    headings: Rc<RefCell<Vec<(SharedString, f32)>>>,
    /// The scroll view's top in the window, and its height.
    top: Rc<Cell<f32>>,
    height: Rc<Cell<f32>>,
    /// The row put in with `reveal` is to be scrolled to once it's drawn.
    revealing: Rc<Cell<bool>>,
    /// What the top held when last drawn, logged once a change.
    logged: RefCell<Option<SharedString>>,
    /// Where the card shows a row whole, as `reveal` has it, logged once a change.
    sight_logged: Rc<Cell<Option<(i32, i32)>>>,
}

/// The card's colours: its fill, the held title's hairline, its name's and its heading's ink.
pub struct Colors {
    pub line: Rgba,
    pub name: Rgba,
    pub heading: Rgba,
}

impl Card {
    /// Back at the top, as a section shows from its start.
    pub fn reset(&self) {
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.headings.borrow_mut().clear();
    }

    /// Put in a heading's element: it notes where the heading is drawn.
    pub fn mark(&self, heading: SharedString) -> gpui::Canvas<()> {
        let headings = self.headings.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                headings
                    .borrow_mut()
                    .push((heading.clone(), f32::from(bounds.origin.y)));
            },
        )
        .absolute()
        .inset_0()
    }

    /// The next row drawn with `reveal` in it is scrolled into sight.
    pub fn reveal_next(&self) {
        self.revealing.set(true);
    }

    /// Put in a row's element: once it's drawn after `reveal_next`, the card scrolls it to just
    /// under the held title, unless it's already in sight above the fade.
    pub fn reveal(&self) -> gpui::Canvas<()> {
        let (scroll, top, height) = (self.scroll.clone(), self.top.clone(), self.height.clone());
        let revealing = self.revealing.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if !revealing.replace(false) {
                    return;
                }
                let (row, card) = (f32::from(bounds.origin.y), top.get());
                let bottom = row + f32::from(bounds.size.height);
                if row >= card + HELD_TITLE && bottom <= card + height.get() - FADE {
                    return;
                }
                let offset = f32::from(scroll.offset().y) - (row - card - HELD_TITLE - PAGE_TOP);
                let most = f32::from(scroll.max_offset().y);
                scroll.set_offset(point(px(0.), px(offset.clamp(-most, 0.))));
                window.refresh();
            },
        )
        .absolute()
        .inset_0()
    }

    /// What the card's top holds for `section`: nothing at the page's top; once scrolled, the
    /// section's name, and after it the last heading gone under the held title.
    pub fn held(&self, section: &str) -> Option<SharedString> {
        if self.scroll.offset().y >= px(0.) {
            return None;
        }
        let under = self.top.get() + HELD_TITLE;
        let headings = self.headings.borrow();
        let passed = headings.iter().filter(|(_, y)| *y < under).next_back();
        Some(match passed {
            Some((heading, _)) => format!("{section} › {heading}").into(),
            None => section.to_string().into(),
        })
    }

    /// The card around `page`, light or dark.
    pub fn draw(
        &self,
        section: &str,
        page: impl IntoElement,
        dark: bool,
        colors: Colors,
    ) -> gpui::Div {
        let held = self.held(section);
        if *self.logged.borrow() != held {
            // check.sh waits for this line.
            crate::log!(
                "browser: settings held {}",
                held.as_deref().unwrap_or("none")
            );
            self.logged.replace(held.clone());
        }
        // Drawn again below, in the page's order.
        self.headings.borrow_mut().clear();
        let fill = if dark {
            rgb(OVERLAY_DARK)
        } else {
            rgb(DIALOG_LIGHT)
        };
        let (offset, max) = (self.scroll.offset().y, self.scroll.max_offset().y);
        let held = held.map(|held| {
            let (name, heading) = match held.split_once(" › ") {
                Some((name, heading)) => (name.to_string(), Some(format!(" › {heading}"))),
                None => (held.to_string(), None),
            };
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(HELD_TITLE))
                .px(px(PAGE_X + TEXT_X))
                .flex()
                .items_center()
                .bg(fill)
                .border_b_1()
                .border_color(colors.line)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.name)
                        .child(name),
                )
                .children(heading.map(|heading| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(colors.heading)
                        .child(heading)
                }))
        });
        let fade = (-offset < max - px(1.)).then(|| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(FADE))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(fill.opacity(0.), 0.),
                    linear_color_stop(fill, 1.),
                ))
        });
        let (top, height) = (self.top.clone(), self.height.clone());
        let sight_logged = self.sight_logged.clone();
        let at = canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                top.set(f32::from(bounds.origin.y));
                height.set(f32::from(bounds.size.height));
                // Under the held title and over the fade, in the window's points, where a press
                // on a control lands: one drawn past them is under the card's edge, or behind
                // them. check.sh reads this line, to scroll a control into sight first (#1950).
                let card = f32::from(bounds.origin.y);
                let sight = (
                    (card + HELD_TITLE).round() as i32,
                    (card + f32::from(bounds.size.height) - FADE).round() as i32,
                );
                if sight_logged.replace(Some(sight)) != Some(sight) {
                    crate::log!("browser: settings card shows {} to {}", sight.0, sight.1);
                }
            },
        )
        .absolute()
        .inset_0();
        div()
            .flex_1()
            .min_w_0()
            .mt(px(CARD_INSET))
            .mr(px(CARD_INSET))
            .mb(px(CARD_INSET))
            .relative()
            .rounded(px(CARD_RADIUS))
            .bg(fill)
            .when(dark, |card| card.border_1().border_color(rgba(CARD_RIM)))
            .when(!dark, |card| {
                card.border_1()
                    .border_color(rgba(CARD_RING))
                    .shadow(vec![BoxShadow {
                        color: rgba(CARD_SHADOW).into(),
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(3.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
            })
            .overflow_hidden()
            .child(at)
            .child(
                div()
                    .id("card")
                    .tab_group()
                    .tab_index(2)
                    // Its controls are the stops, not the scrolling page they're on.
                    .tab_stop(false)
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .pt(px(PAGE_TOP))
                    .pb(px(PAGE_BOTTOM))
                    .px(px(PAGE_X))
                    .child(page),
            )
            .children(held)
            .children(fade)
    }
}
