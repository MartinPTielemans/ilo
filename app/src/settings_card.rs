//! Settings' card: the section's page on the window's card, scrolling, with the section's name and
//! the heading scrolled past held at its top, so the user keeps their place in a long section, and
//! its foot fading into its fill where rows go on.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    Bounds, BoxShadow, FocusHandle, FontWeight, Pixels, Rgba, ScrollHandle, SharedString,
    WeakFocusHandle, canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, rgb,
    rgba,
};

use crate::palette::OVERLAY_DARK;
use crate::tokens::palette::DIALOG_LIGHT;
use crate::tokens::settings::{
    CARD_RIM, CARD_RING, CARD_SHADOW, FADE, HELD_TITLE, PAGE_BOTTOM, PAGE_TOP, PAGE_X, TEXT_X,
};
use crate::tokens::shell::CARD_INSET;

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
    /// Keeps the control the keys move to in sight.
    follow: Follow,
}

/// A pane that scrolls keeps the control that takes the keyboard in sight, as a browser's page
/// does: GPUI moves the keys from control to control without scrolling to them, so ⇥ to a control
/// under a short window's edge would leave it, and its ring, out of sight (#2506). Its `before`
/// is drawn before the pane's controls and its `after` once they're drawn.
#[derive(Default)]
pub struct Follow {
    /// Where the control with the keyboard was drawn by the time the pane's began.
    before: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The control in the pane last scrolled to, kept to only as it takes the keyboard, so a wheel
    /// can take it away again.
    followed: Rc<RefCell<Option<WeakFocusHandle>>>,
    /// A row scrolled to this frame by other means, as `reveal` does, isn't followed as well.
    placed: Rc<Cell<bool>>,
}

impl Follow {
    /// Put in before the pane.
    pub fn before(&self) -> gpui::Canvas<()> {
        let before = self.before.clone();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| before.set(window.painted_focus_bounds()),
        )
        .absolute()
        .inset_0()
    }

    /// Put in after the pane, on the bounds of its scroll view: a control in it that took the
    /// keyboard is scrolled in, by `scroll`, as little as shows it whole between `top` and `bottom`
    /// points in from the view's edges, where the pane's held title and fade cover it.
    pub fn after(&self, scroll: ScrollHandle, top: f32, bottom: f32) -> gpui::Canvas<()> {
        let (before, followed) = (self.before.clone(), self.followed.clone());
        let placed = self.placed.clone();
        canvas(
            |_, _, _| {},
            move |view, _, window, cx| {
                let placed = placed.replace(false);
                let focus = window.painted_focus_bounds();
                let Some(control) = focus.filter(|_| focus != before.get()) else {
                    followed.replace(None);
                    return;
                };
                let focused = window.focused(cx);
                let kept = followed.replace(focused.as_ref().map(FocusHandle::downgrade));
                let same = match (&kept, &focused) {
                    (Some(kept), Some(focused)) => kept == focused,
                    (kept, focused) => kept.is_none() && focused.is_none(),
                };
                if same || placed {
                    return;
                }
                let by = into_sight(view, control, top, bottom);
                if by != 0. {
                    let most = f32::from(scroll.max_offset().y);
                    let mut offset = (f32::from(scroll.offset().y) + by).clamp(-most, 0.);
                    // Within the title's or the fade's height of an end, where neither is drawn,
                    // the page goes to the end rather than stop a few points short of it.
                    if by > 0. && offset > -top {
                        offset = 0.;
                    } else if by < 0. && offset < bottom - most {
                        offset = -most;
                    }
                    scroll.set_offset(point(px(0.), px(offset)));
                    // Not refresh, which does nothing while the frame draws.
                    window.request_animation_frame();
                }
            },
        )
        .absolute()
        .inset_0()
    }
}

/// How far to move a pane's page, `view` in the window, so `control` shows whole between `top`
/// and `bottom` points in from its edges: as little as that takes, its top first where it can't.
/// One that covers all it shows already shows all it can, as the pane itself does, whose controls
/// leave it the keyboard as a press takes it, and is left where it is (#2522).
fn into_sight(view: Bounds<Pixels>, control: Bounds<Pixels>, top: f32, bottom: f32) -> f32 {
    let (from, to) = (
        f32::from(view.origin.y) + top,
        f32::from(view.bottom()) - bottom,
    );
    let (y, end) = (f32::from(control.origin.y), f32::from(control.bottom()));
    if y <= from && end >= to {
        0.
    } else if y < from {
        from - y
    } else if end > to {
        -(end - to).min(y - from)
    } else {
        0.
    }
}

/// Where Page Down, Page Up, Home and End move a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paging {
    Down,
    Up,
    Top,
    Bottom,
}

impl Paging {
    /// Space and ⇧Space, as a page has them: Page Down and Page Up. Only while the window itself has
    /// the keyboard: with a control, Space is its press, and with a field, a space typed.
    pub fn space(keys: &gpui::Keystroke) -> Option<Paging> {
        let held = keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt;
        (keys.key == "space" && !held).then_some(match keys.modifiers.shift {
            true => Paging::Up,
            false => Paging::Down,
        })
    }
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

    /// Page Down and Page Up move the page by what the card shows between its held title and its
    /// fade, so the row the fade covered shows under the title; Home and End go to its top and its
    /// foot. False where it's already there.
    pub fn page(&self, to: Paging) -> bool {
        let (offset, most) = (
            f32::from(self.scroll.offset().y),
            f32::from(self.scroll.max_offset().y),
        );
        let sight = (self.height.get() - HELD_TITLE - FADE).max(0.);
        let next = match to {
            Paging::Down => offset - sight,
            Paging::Up => offset + sight,
            Paging::Top => 0.,
            Paging::Bottom => -most,
        }
        .clamp(-most, 0.);
        if next == offset {
            return false;
        }
        self.scroll.set_offset(point(px(0.), px(next)));
        true
    }

    /// Put in a row's element: once it's drawn after `reveal_next`, the card scrolls it to just
    /// under the held title, unless it's already in sight above the fade.
    pub fn reveal(&self) -> gpui::Canvas<()> {
        let (scroll, top, height) = (self.scroll.clone(), self.top.clone(), self.height.clone());
        let (revealing, placed) = (self.revealing.clone(), self.follow.placed.clone());
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
                placed.set(true);
                window.refresh();
            },
        )
        .absolute()
        .inset_0()
    }

    /// What the card's top holds for `section`: nothing at the page's top; once scrolled, the
    /// section's name, and after it the last heading gone under the held title.
    pub fn held(&self, section: &str) -> Option<SharedString> {
        // As numbers, not Pixels, which order -0 under 0: GPUI holds a page that fits at -0.
        if f32::from(self.scroll.offset().y) >= 0. {
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
        // A wheel moves the offset as far as it's turned, and GPUI holds it to the page only as it
        // lays the page out, after this: held to the page here, a page that fits never reads as
        // scrolled in the wheel's frame, which is drawn last while nothing moves (#2102).
        let offset = self.scroll.offset();
        let most = f32::from(self.scroll.max_offset().y);
        let kept = px(f32::from(offset.y).clamp(-most, 0.));
        if kept != offset.y {
            self.scroll.set_offset(point(offset.x, kept));
        }
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
        // What this frame shows of the scroll, from the page's last layout: a page whose layout
        // changes under it, as a resize does, is held to its new one only after, and is drawn
        // again then if that changes what shows.
        let shows = (held.is_some(), -offset < max - px(1.));
        let scroll = self.scroll.clone();
        let after = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let (offset, max) = (scroll.offset().y, scroll.max_offset().y);
                // Not refresh, which does nothing while the frame draws.
                if (f32::from(offset) < 0., -offset < max - px(1.)) != shows {
                    window.request_animation_frame();
                }
            },
        )
        .absolute()
        .inset_0();
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
        let fade = shows.1.then(|| {
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
            // A window of its own, whose corner it follows as the browser window's card does.
            .rounded(px(crate::shell::card_radius(crate::shell::window_radius())))
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
            .child(self.follow.before())
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
            .child(after)
            .child(self.follow.after(self.scroll.clone(), HELD_TITLE, FADE))
            .children(held)
            .children(fade)
    }
}
