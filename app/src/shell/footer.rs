//! The sidebar's footer: the Space switcher, the plus button beside it, and off the Mac the menu
//! control before the plus.

use gpui::{
    AnyElement, Context, DragMoveEvent, MouseButton, MouseDownEvent, Role, canvas, div, prelude::*,
    px, svg,
};

use super::{
    FOOTER_HEIGHT, NAV_GLYPH, NAV_HOVERED, NAV_PITCH, NAV_RADIUS, NAV_SIZE, PLUS_INSET,
    SPACE_CLEAR, SPACE_DRAGGED, SPACE_GLYPH, SPACE_HEIGHT, SPACE_HOVERED, SPACE_ICON_SWITCHER,
    SPACE_LEAST, SPACE_OTHER, SPACE_SHOWN_LEAST, SPACE_SWATCH, SPACE_SWATCH_HOVERED,
    SPACE_SWATCH_SHOWN, SWITCHER_INSET, Shell, Space,
    access::Accessible,
    space_order::{Along, DraggedSpace},
};
use crate::space_icon::{GLYPHS, SpaceIcon};
use crate::tokens::shell::{FOOT_GLYPH, FOOT_PLATE, FOOT_PLATE_RADIUS, ICON_INK, PLUS_PLATE};
use crate::tooltip::{Anchor, Tip};

/// How much of a glyph's box its drawing fills, so that it's squeezed by its ink.
const GLYPH_INK: f32 = 20. / 24.;

/// How far the +'s box is from the sidebar's side: centred where the first row's size of button
/// would be, so the glyph stays where it was drawn.
pub(super) fn plus_right() -> f32 {
    PLUS_INSET + (NAV_SIZE - FOOT_PLATE) / 2.
}

/// How wide each Space's cell is in a switcher `span` wide, from its own pitch in `pitches`, a
/// dot's or an icon's, with the `shown`th shown. While they don't all fit, the ones whose pitch is
/// under an even share of what the rest leave keep theirs and the others take that share, in half
/// points; once that share is under the least the Space shown keeps, it keeps that and the
/// others share the rest evenly (Arc 1.171). The cells can come to a fraction more than the span,
/// which the switcher centres.
pub(super) fn space_cells(span: f32, pitches: &[f32], shown: usize) -> Vec<f32> {
    let half = |width: f32| (width * 2.).round() / 2.;
    let count = pitches.len();
    if count < 2 || pitches.iter().sum::<f32>() <= span {
        return pitches.iter().map(|pitch| pitch.min(span)).collect();
    }
    let mut narrowest = pitches.to_vec();
    narrowest.sort_by(f32::total_cmp);
    let (mut left, mut share) = (span, 0.);
    for (index, pitch) in narrowest.into_iter().enumerate() {
        share = left / (count - index) as f32;
        if pitch > share {
            break;
        }
        left -= pitch;
    }
    let share = half(share);
    if share >= SPACE_SHOWN_LEAST {
        return pitches.iter().map(|pitch| pitch.min(share)).collect();
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
        (false, false) => SPACE_OTHER,
        (false, true) => SPACE_HOVERED,
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
                .child(self.space_mark(space, shown, cell))
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

    /// A Space's dot, glyph or emoji in its switcher cell `cell` wide.
    fn space_mark(&self, space: &Space, shown: bool, cell: f32) -> AnyElement {
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
                // The Space shown's is the only dot in colour, a tone of its first colour toward
                // the ink that reads on its own window (`Palette::space_dot`), or the ink without
                // one; the others' are the quietest ink, a step firmer under the pointer.
                match (shown, self.space_theme(id).is_some()) {
                    (true, true) => dot(squeezed(size, cell), self.palette_of(id).space_dot.into()),
                    (true, false) => dot(squeezed(size, cell), self.palette.glyph(1.)),
                    (false, _) => dot(
                        squeezed(size, cell),
                        self.palette.glyph(self.palette.tertiary()),
                    )
                    .group_hover("space", |style| {
                        style
                            .size(px(hovered))
                            .bg(self.palette.glyph(self.palette.secondary()))
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
                    (false, _) => self.palette.glyph(self.palette.tertiary()),
                })
                .when(!shown, |glyph| {
                    glyph.group_hover("space", |style| {
                        style.text_color(self.palette.glyph(self.palette.secondary()))
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
        let mark = self.space_mark(space, space.id == self.space, cell);
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

    /// Whether the + wears its plate: while the menu up is its own.
    pub(super) fn plus_lit(&self) -> bool {
        self.plus_menu && self.menu_open
    }

    /// The footer's + at its right (Arc), whose menu makes a Space or a folder. Its box is the
    /// plate it wears while that menu is up.
    pub(super) fn plus_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("plus")
            .when(self.a11y, |plus| {
                plus.button_named("New Space, Folder or Note")
                    .pressed_by_mouse_down(true, false)
            })
            .absolute()
            .right(px(plus_right()))
            .bottom(px((FOOTER_HEIGHT - FOOT_PLATE) / 2.))
            .size(px(FOOT_PLATE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(FOOT_PLATE_RADIUS))
            .block_mouse_except_scroll()
            // One plate, under the pointer and while its menu is up.
            .when(self.plus_lit(), |plus| {
                plus.bg(self.palette.plate(PLUS_PLATE))
            })
            .hover(|style| style.bg(self.palette.plate(PLUS_PLATE)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                    shell.plus_menu(event.position, cx)
                }),
            )
            .child(
                svg()
                    .path("icons/plus.svg")
                    .size(px(FOOT_GLYPH))
                    .text_color(self.palette.glyph(ICON_INK)),
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
                    .text_color(self.palette.glyph(ICON_INK)),
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
    use std::rc::Rc;

    use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

    use super::{icon_opacity, plus_right, space_cells, squeezed};
    use crate::palette::{TEXT_SECONDARY, TEXT_TERTIARY};
    use crate::shell::{FOOTER_HEIGHT, Shell, library::LIBRARY_BACK_INSET, recorder::Recorder};
    use crate::tokens::shell::{FOOT_PLATE, LIBRARY_BUTTON};

    /// A window with two Spaces, so its foot has the +.
    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| {
            let spaces = [1, 2]
                .map(|id| {
                    (
                        id,
                        format!("Space {id}").into(),
                        None,
                        1,
                        "".into(),
                        "".into(),
                        "".into(),
                    )
                })
                .to_vec();
            shell.set_spaces(spaces, cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (shell, cx, host)
    }

    /// The middle of the foot's row, this far from the sidebar's far side.
    fn foot(shell: &Entity<Shell>, cx: &mut VisualTestContext, from_right: f32) -> Point<Pixels> {
        shell.read_with(cx, |shell, _| {
            let y = f32::from(shell.viewport.height) - FOOTER_HEIGHT / 2.;
            point(px(shell.width() - from_right), px(y))
        })
    }

    /// The + wears its plate from the click that opens its menu until the menu closes, chosen
    /// from or not, and another menu doesn't give it one.
    #[gpui::test]
    fn the_plus_wears_its_plate_while_its_menu_is_up(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        assert!(!shell.read_with(cx, |shell, _| shell.plus_lit()));
        let middle = foot(&shell, cx, plus_right() + FOOT_PLATE / 2.);
        cx.simulate_click(middle, Modifiers::none());
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        assert!(
            titles.iter().any(|(title, _)| title == "New Space…"),
            "the click opened no menu"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.plus_lit()),
            "no plate while its menu is up"
        );
        cx.update(|_, cx| host.dismiss(cx));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.plus_lit()),
            "the plate outlived the menu"
        );

        // A Space's own menu is not the +'s.
        shell.update(cx, |shell, cx| shell.space_menu(1, middle, cx));
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.menu_open && !shell.plus_lit()));
        cx.update(|_, cx| host.dismiss(cx));
    }

    /// The + takes the pointer in the box its plate fills, 28 across about the glyph, and not
    /// past it.
    #[gpui::test]
    fn the_plus_takes_the_pointer_in_its_plates_box(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let middle = plus_right() + FOOT_PLATE / 2.;
        let past = foot(&shell, cx, middle + FOOT_PLATE / 2. + 1.);
        cx.simulate_click(past, Modifiers::none());
        cx.run_until_parked();
        assert!(
            host.menus.borrow().is_empty(),
            "a click past the box opened it"
        );
        let inside = foot(&shell, cx, middle + FOOT_PLATE / 2. - 1.);
        cx.simulate_click(inside, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            host.menus.borrow().len(),
            1,
            "a click in the box opened none"
        );
        cx.update(|_, cx| host.dismiss(cx));
    }

    /// The Library button wears its plate while the Library is open, from the click that opens it,
    /// and not once it's closed.
    #[gpui::test]
    fn the_library_button_wears_its_plate_while_the_library_is_open(cx: &mut TestAppContext) {
        let (shell, cx, _host) = window(cx);
        assert!(!shell.read_with(cx, |shell, _| shell.library_lit()));
        let y = shell.read_with(cx, |shell, _| {
            f32::from(shell.viewport.height) - FOOTER_HEIGHT / 2.
        });
        let button = point(px(LIBRARY_BACK_INSET + LIBRARY_BUTTON / 2.), px(y));
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.library_open && shell.library_lit()),
            "no plate with the Library open"
        );
        shell.update(cx, |shell, cx| shell.set_library(false, cx));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_lit()),
            "the plate outlived the Library"
        );
    }

    /// The switcher's cells in a sidebar 300 wide, 212 between the insets: a dot's pitch of 17 while
    /// they fit, and once an even share is under that the Space shown keeps it and the others
    /// share the rest in half points (the rule is Arc 1.171's): 16.5, 13 and 9.5 with 13, 16 and
    /// 22 Spaces.
    #[test]
    fn the_switcher_shares_its_span_as_arcs() {
        let span = 300. - 2. * 44.;
        assert_eq!(space_cells(span, &[17.; 5], 0), vec![17.; 5]);
        assert_eq!(space_cells(span, &[17.; 12], 3), vec![17.; 12]);
        for (count, others) in [(13, 16.5), (16, 13.), (22, 9.5)] {
            let cells = space_cells(span, &vec![17.; count], 4);
            assert_eq!(cells.len(), count);
            assert_eq!(cells[4], 17., "the Space shown with {count}");
            let shared = cells
                .iter()
                .enumerate()
                .all(|(i, &cell)| i == 4 || cell == others);
            assert!(shared, "{count}: {cells:?}");
            // Never more than the rounding's quarter point a Space over the span, which the
            // switcher centres.
            let over = cells.iter().sum::<f32>() - span;
            assert!(over <= 0.25 * count as f32, "{count}: {cells:?}");
        }
        // Without the Space shown among them, as while a new Space's form shows, all share alike.
        assert!(
            space_cells(span, &[17.; 22], usize::MAX)
                .iter()
                .all(|&cell| cell == 9.5)
        );
        assert!(space_cells(span, &[], 0).is_empty());
        assert_eq!(space_cells(span, &[17.], 0), vec![17.]);
    }

    /// A Space with an icon keeps its wider pitch beside the dots while all fit. Once they don't,
    /// the dots keep theirs and the icons share what's left, until that's under what the Space
    /// shown keeps and all share as dots do.
    #[test]
    fn an_icon_keeps_its_own_pitch_beside_the_dots() {
        let span = 228. - 2. * 44.;
        assert_eq!(space_cells(span, &[34., 17., 34.], 1), [34., 17., 34.]);
        assert_eq!(space_cells(span, &[34.; 5], 0), [28.; 5]);
        assert_eq!(
            space_cells(span, &[34., 17., 34., 17., 34., 17.], 0),
            [29.5, 17., 29.5, 17., 29.5, 17.]
        );
        let cells = space_cells(span, &[34.; 9], 2);
        assert_eq!((cells[2], cells[0], cells[8]), (17., 15.5, 15.5));
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
    /// at the quietest text's, and one under the pointer at the quiet text's.
    #[test]
    fn an_icon_dims_as_the_dot_does() {
        assert_eq!(icon_opacity(true, false), 1.);
        assert_eq!(icon_opacity(true, true), 1.);
        let others = icon_opacity(false, false);
        assert_eq!(others, TEXT_TERTIARY);
        let hovered = icon_opacity(false, true);
        assert_eq!(hovered, TEXT_SECONDARY);
        assert!(others < hovered && hovered < 1.);
    }
}

/// A plain round dot `size` across in `color`.
fn dot(size: f32, color: gpui::Hsla) -> gpui::Div {
    div().size(px(size)).flex_none().rounded_full().bg(color)
}
