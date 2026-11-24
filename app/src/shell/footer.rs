//! The sidebar's footer: the Space switcher, the plus button beside it, and off the Mac the menu
//! control before the plus.

use gpui::{
    AnyElement, Context, DragMoveEvent, MouseButton, MouseDownEvent, Rgba, Role, canvas, div,
    prelude::*, px, svg,
};

use super::{
    FOOTER_HEIGHT, NAV, NAV_GLYPH, NAV_HOVERED, NAV_PITCH, NAV_RADIUS, NAV_SIZE, PLUS_INSET,
    SPACE_CLEAR, SPACE_DRAGGED, SPACE_GLYPH, SPACE_HEIGHT, SPACE_HOVERED, SPACE_ICON_SWITCHER,
    SPACE_LEAST, SPACE_OTHER, SPACE_PITCH, SPACE_SHOWN, SPACE_SHOWN_LEAST, SPACE_SWATCH,
    SPACE_SWATCH_HOVERED, SPACE_SWATCH_SHOWN, SWITCHER_INSET, Shell, Space,
    access::Accessible,
    space_order::{Along, DraggedSpace},
};
use crate::space_icon::{GLYPHS, SpaceIcon};
use crate::tooltip::{Anchor, Tip};

/// How much of a glyph's box its drawing fills, so that it's squeezed by its ink.
const GLYPH_INK: f32 = 20. / 24.;

/// How wide each of `count` Spaces' cells is in a switcher `span` wide, with the `shown`th shown:
/// an even share in half points, at most the pitch, and once that's under the least the Space
/// shown keeps, it keeps that and the others share the rest (Arc 1.171). The cells can come to a
/// fraction more than the span, which the switcher centres.
pub(super) fn space_cells(span: f32, count: usize, shown: usize) -> Vec<f32> {
    let half = |width: f32| (width * 2.).round() / 2.;
    if count == 0 {
        return Vec::new();
    }
    let even = half(span / count as f32);
    if even >= SPACE_SHOWN_LEAST || count == 1 {
        return vec![even.min(SPACE_PITCH); count];
    }
    let others = half((span - SPACE_SHOWN_LEAST).max(0.) / (count - 1) as f32);
    (0..count)
        .map(|index| {
            if index == shown {
                SPACE_SHOWN_LEAST
            } else {
                others
            }
        })
        .collect()
}

/// How big an item's ink `size` across draws in a cell `cell` wide: its own size, or less to keep
/// clear of the items beside it, but never under the least, or the cell less a point below that.
pub(super) fn squeezed(size: f32, cell: f32) -> f32 {
    size.min(cell - SPACE_CLEAR)
        .max(SPACE_LEAST.min(cell - 1.))
        .max(0.)
}

/// How strong a Space's icon draws in the switcher, as its dot would: the Space shown at full
/// strength, the others dimmed by the dot's own ratio, and a hovered one by the dot's hovered
/// ratio.
pub(super) fn icon_opacity(shown: bool, hovered: bool) -> f32 {
    match (shown, hovered) {
        (true, _) => 1.,
        (false, false) => SPACE_OTHER / SPACE_SHOWN,
        (false, true) => SPACE_HOVERED / SPACE_SHOWN,
    }
}

impl Shell {
    /// A dot for each Space, in the footer. Arc draws none while there's only one.
    pub(super) fn switcher(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.spaces.len() < 2 {
            return None;
        }
        let span = self.switcher_span();
        let cells = self.switcher_cells(span);
        let behind = self.switcher_behind();
        let shell = cx.weak_entity();
        let dots = self.spaces.iter().zip(cells).map(|(space, cell)| {
            let (id, shown) = (space.id, space.id == self.space);
            let shell = shell.clone();
            div()
                .id(("space", id))
                .when(self.a11y, |dot| {
                    dot.role(Role::Tab)
                        .accessibility_id(format!("space-{id}"))
                        .aria_label(space.name.clone())
                        .aria_selected(shown)
                        .pressed_by_mouse_down(false, true)
                })
                .group("space")
                .flex_none()
                .relative()
                .left(px(self.space_slide(Along::Switcher, id)))
                .w(px(cell))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .when(self.space_dragged(id), |dot| dot.opacity(SPACE_DRAGGED))
                .block_mouse_except_scroll()
                .on_click(cx.listener(move |shell, _, _, cx| {
                    log!("browser: command select-space via sidebar");
                    shell.show_space(id, cx)
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                        shell.space_menu(id, event.position, cx)
                    }),
                )
                // Along the switcher it moves the Space (#1580).
                .on_drag(DraggedSpace, move |_, grab, _, cx| {
                    let grab = f32::from(grab.x);
                    Shell::start_space_drag(&shell, id, Along::Switcher, cell, grab, cx)
                })
                .child(self.space_mark(space, shown, cell, &behind))
                .child(self.dot_drawn(id))
        });
        Some(
            div()
                .id("switcher")
                .when(self.a11y, |switcher| {
                    switcher.role(Role::TabList).aria_label("Spaces")
                })
                .absolute()
                .left(px(SWITCHER_INSET))
                .w(px(span))
                .bottom(px((FOOTER_HEIGHT - SPACE_HEIGHT) / 2.))
                .h(px(SPACE_HEIGHT))
                .flex()
                .justify_center()
                .on_drag_move(
                    cx.listener(|shell, event: &DragMoveEvent<DraggedSpace>, _, cx| {
                        shell.drag_space_over(Along::Switcher, event, cx)
                    }),
                )
                .children(dots)
                .children(self.space_drop_line(Along::Switcher))
                .child(self.space_drop_listener(Along::Switcher, cx)),
        )
    }

    /// Logs where Space `id`'s dot is drawn, its centre in the window, as "browser: switcher dot
    /// `id` in window `window` at x y", each time that moves, for check.sh to click it there
    /// whatever else the footer holds (#3175).
    fn dot_drawn(&self, id: u64) -> impl IntoElement + use<> {
        let (drawn, window) = (self.space_dots_drawn.clone(), self.browser.window());
        canvas(
            |_, _, _| {},
            move |bounds, _, _, _| {
                let centre = bounds.center();
                let at = (f32::from(centre.x).round(), f32::from(centre.y).round());
                let (x, y) = (at.0 as i32, at.1 as i32);
                if drawn.borrow_mut().insert(id, (x, y)) != Some((x, y)) {
                    // check.sh reads this line.
                    log!("browser: switcher dot {id} in window {window} at {x} {y}");
                }
            },
        )
        .absolute()
        .inset_0()
    }

    /// How wide the switcher is: the footer between its insets, less the menu control's pitch where
    /// it has one, so the dots stay centred between the Library and the control (#3148).
    pub(super) fn switcher_span(&self) -> f32 {
        let control = if self.menu_control { NAV_PITCH } else { 0. };
        self.width() - 2. * SWITCHER_INSET - control
    }

    /// What the switcher's swatches lie on: the window at the footer, over whatever is behind it.
    fn switcher_behind(&self) -> [Rgba; 2] {
        let window = f32::from(self.viewport.height).max(1.);
        let at = self.gradient_color((window - FOOTER_HEIGHT / 2.) / window);
        self.palette.over_backdrops(at)
    }

    /// A Space's dot, glyph or emoji in its switcher cell `cell` wide.
    fn space_mark(&self, space: &Space, shown: bool, cell: f32, behind: &[Rgba; 2]) -> AnyElement {
        let id = space.id;
        match SpaceIcon::parse(&space.icon) {
            SpaceIcon::Dot => {
                let size = if shown {
                    SPACE_SWATCH_SHOWN
                } else {
                    SPACE_SWATCH
                };
                // Under the pointer it grows as far again past its squeezed size.
                let room = cell + SPACE_SWATCH_HOVERED - SPACE_SWATCH;
                let hovered = squeezed(SPACE_SWATCH_HOVERED, room);
                // The Space shown's is the only dot in colour, its swatch, or the ink without one;
                // the others' are the ink, quiet, firmer under the pointer.
                match (shown, self.space_theme(id).is_some()) {
                    (true, true) => self
                        .palette_of(id)
                        .swatch
                        .draw(squeezed(size, cell), behind),
                    (true, false) => dot(squeezed(size, cell), self.palette.glyph(1.)),
                    (false, _) => dot(squeezed(size, cell), self.palette.glyph(SPACE_OTHER))
                        .group_hover("space", |style| {
                            style
                                .size(px(hovered))
                                .bg(self.palette.glyph(SPACE_HOVERED))
                        }),
                }
                .into_any_element()
            }
            // The Space shown's in its accent, as its dot is in its colour, or the ink without
            // one; the others' in the dots' greys.
            SpaceIcon::Glyph(index) => svg()
                .path(GLYPHS[index].1)
                .size(px(squeezed(SPACE_GLYPH * GLYPH_INK, cell) / GLYPH_INK))
                .text_color(match (shown, self.space_theme(id).is_some()) {
                    (true, true) => self.palette_of(id).accent.into(),
                    (true, false) => self.palette.glyph(1.),
                    (false, _) => self.palette.glyph(SPACE_OTHER),
                })
                .when(!shown, |glyph| {
                    glyph.group_hover("space", |style| {
                        style.text_color(self.palette.glyph(SPACE_HOVERED))
                    })
                })
                .into_any_element(),
            // The item's box is the dot's, so the strip doesn't move with an icon.
            SpaceIcon::Emoji(emoji) => div()
                .text_size(px(squeezed(SPACE_ICON_SWITCHER, cell)))
                .opacity(icon_opacity(shown, false))
                .when(!shown, |icon| {
                    icon.group_hover("space", |style| style.opacity(icon_opacity(false, true)))
                })
                .child(emoji.to_string())
                .into_any_element(),
        }
    }

    /// The copy of a dragged dot that follows the pointer: its cell `cell` wide, as it's drawn in
    /// the switcher.
    pub(super) fn dot_ghost(&self, space: u64, cell: f32) -> Option<AnyElement> {
        let space = self.spaces.iter().find(|each| each.id == space)?;
        let mark = self.space_mark(space, space.id == self.space, cell, &self.switcher_behind());
        Some(
            div()
                .w(px(cell))
                .h(px(SPACE_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .child(mark)
                .into_any_element(),
        )
    }

    /// The footer's + at its right (Arc), whose menu makes a Space or a folder.
    pub(super) fn plus_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("plus")
            .when(self.a11y, |plus| {
                plus.button_named("New Space, Folder or Note")
                    .pressed_by_mouse_down(true, false)
            })
            .absolute()
            .right(px(PLUS_INSET))
            .bottom(px((FOOTER_HEIGHT - NAV_SIZE) / 2.))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                    shell.plus_menu(event.position, cx)
                }),
            )
            .child(
                svg()
                    .path("icons/nav-plus.svg")
                    .size(px(NAV_GLYPH))
                    .text_color(self.palette.glyph(NAV)),
            )
    }

    /// Off the Mac, the control before the + that opens ilo's menu, which the Mac's menu bar holds
    /// (#3148). It keeps its plate while its menu is up, as the name row keeps its fill.
    pub(super) fn menu_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let lit = self.app_menu && self.menu_open;
        div()
            .id(MENU_CONTROL)
            .when(self.a11y, |button| button.button_named(MENU_CONTROL_NAME))
            // A screen reader says the control as the keys' while the ring is on it.
            .when(self.a11y && self.keyed_menu.is_some(), |button| {
                button.aria_active_descendant()
            })
            .absolute()
            .right(px(PLUS_INSET + NAV_PITCH))
            .bottom(px((FOOTER_HEIGHT - NAV_SIZE) / 2.))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .when(lit, |button| button.bg(self.palette.plate(NAV_HOVERED)))
            .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
            .on_hover(
                cx.listener(|shell, hovered: &bool, _, cx| shell.hover_menu_control(*hovered, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.browser.tooltip(None, cx)),
            )
            // On the click, as the actions' overflow opens its menu: the menu opens above the
            // control, away from the pointer, so a release after it opened would read as a drag
            // that let go outside it and close it, as Windows' did.
            .on_click(cx.listener(|shell, _, _, cx| shell.app_menu(cx)))
            .child(self.button_frame(MENU_CONTROL))
            .children(self.menu_ring())
            .child(
                svg()
                    .path("icons/nav-menu.svg")
                    .size(px(NAV_GLYPH))
                    .text_color(self.palette.glyph(NAV)),
            )
    }

    /// The pointer has come onto or left the menu control, whose tooltip names it as the header's
    /// buttons' name theirs, with no keys, as the menu has none.
    fn hover_menu_control(&self, hovered: bool, cx: &mut Context<Self>) {
        let busy = self.dragging().is_some() || self.menu_row.is_some() || self.menu_open;
        let frame = self.button_frames.borrow().get(MENU_CONTROL).copied();
        let tip = frame.filter(|_| hovered && !busy).map(|frame| {
            let tip = Tip::named(MENU_CONTROL_NAME.into());
            (tip, Anchor::under(frame, self.viewport))
        });
        self.hover_tip(tip, cx);
    }
}

/// The menu control's key among the buttons' frames, and its id.
pub(super) const MENU_CONTROL: &str = "app-menu";
/// What the menu control's tooltip and the screen reader call it: the menu the Mac's menu bar names
/// after the app.
const MENU_CONTROL_NAME: &str = concat!(env!("APP_NAME"), " menu");

#[cfg(test)]
mod tests {
    use super::{icon_opacity, space_cells, squeezed};

    /// The switcher's cells in Arc's sidebar 300 wide, 216 between the insets: 34 apart while they
    /// fit, an even share in half points past that, and once that's under 22 the Space shown keeps
    /// 22 and the others share the rest, as Arc 1.171's 17.5, 13 and 9 with 12, 16 and 22 Spaces.
    #[test]
    fn the_switcher_shares_its_span_as_arcs() {
        let span = 300. - 2. * 42.;
        assert_eq!(space_cells(span, 5, 0), vec![34.; 5]);
        assert_eq!(space_cells(span, 8, 3), vec![27.; 8]);
        for (count, others) in [(12, 17.5), (16, 13.), (22, 9.)] {
            let cells = space_cells(span, count, 4);
            assert_eq!(cells.len(), count);
            assert_eq!(cells[4], 22., "the Space shown with {count}");
            let shared = cells
                .iter()
                .enumerate()
                .all(|(i, &cell)| i == 4 || cell == others);
            assert!(shared, "{count}: {cells:?}");
            // Never more than a point over the span, which the switcher centres.
            assert!(cells.iter().sum::<f32>() <= span + 1., "{count}: {cells:?}");
        }
        // Without the Space shown among them, as while a new Space's form shows, all share alike.
        assert!(
            space_cells(span, 22, usize::MAX)
                .iter()
                .all(|&cell| cell == 9.)
        );
        assert!(space_cells(span, 0, 0).is_empty());
        assert_eq!(space_cells(span, 1, 0), vec![34.]);
    }

    /// An item keeps its size while it has 4 clear of its neighbours, shrinks to keep that, and
    /// never goes under 4, or under the cell less a point.
    #[test]
    fn a_squeezed_item_keeps_clear_of_its_neighbours() {
        assert_eq!(squeezed(10., 34.), 10.);
        assert_eq!(squeezed(10., 17.5), 10.);
        assert_eq!(squeezed(10., 13.), 9.);
        assert_eq!(squeezed(10., 9.), 5.);
        assert_eq!(squeezed(10., 7.), 4.);
        assert_eq!(squeezed(10., 3.), 2.);
        assert_eq!(squeezed(14., 22.), 14.);
    }

    /// An icon is dimmed as the dot it replaces is: the Space shown at full strength, the others
    /// by the dots' ratio of 0.35 to 0.6, and a hovered one by the dots' 0.5 to 0.6 (#320's
    /// ladder).
    #[test]
    fn an_icon_dims_as_the_dot_does() {
        assert_eq!(icon_opacity(true, false), 1.);
        assert_eq!(icon_opacity(true, true), 1.);
        let others = icon_opacity(false, false);
        assert!(
            (others - 0.35 / 0.6).abs() < 0.02,
            "the others are at {others}"
        );
        let hovered = icon_opacity(false, true);
        assert!(
            (hovered - 0.5 / 0.6).abs() < 0.02,
            "a hovered one is at {hovered}"
        );
        assert!(others < hovered && hovered < 1.);
    }
}

/// A plain round dot `size` across in `color`.
fn dot(size: f32, color: gpui::Hsla) -> gpui::Div {
    div().size(px(size)).flex_none().rounded_full().bg(color)
}
