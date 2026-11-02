//! An extension's side panel: a card right of the page, its header over the extension's page,
//! which drags from its left edge to resize (docs/ux-spec.md › Side panel).

use std::sync::Arc;

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, DispatchPhase, Image, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, SharedString, Size, canvas, div, img, point, prelude::*,
    px, size, svg,
};

use super::{
    CARD_INSET, MenuAction, MenuEntry, NAV, NAV_RADIUS, Shell, TITLE,
    URL_BUTTON_GLYPH, URL_BUTTON_INSET, URL_BUTTON_SIZE, card, panes::page_frame,
};

use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::side_panel::PANEL_WIDTH;
use crate::tokens::shell::side_panel::PANEL_MIN;
use crate::tokens::shell::side_panel::PANEL_MAX;
/// The header over the panel's page, holding a button's height.
const HEADER: f32 = URL_BUTTON_SIZE + 2. * URL_BUTTON_INSET;
use crate::tokens::shell::side_panel::HEADER_ICON;
use crate::tokens::shell::side_panel::HEADER_TEXT;

/// The side panel shown, as the core reports it.
#[derive(Clone)]
pub struct SidePanel {
    pub extension: SharedString,
    pub name: SharedString,
    pub icon: Option<Arc<Image>>,
    /// Its page's title.
    pub title: SharedString,
    /// The width the user last gave a panel in its profile, or 0 for none.
    pub width: f32,
}

impl Shell {
    /// Shows `panel` right of the page, or none.
    pub fn set_side_panel(&mut self, panel: Option<SidePanel>, cx: &mut Context<Self>) {
        // check.sh waits for these lines.
        match &panel {
            Some(panel) => log!("browser: side panel {} {}", panel.extension, panel.title),
            None => log!("browser: side panel none"),
        }
        if let Some(panel) = &panel
            && self.side_panel_resize.is_none()
        {
            self.side_panel_width = panel.width;
        }
        if panel.is_none() {
            self.side_panel_resize = None;
        }
        self.side_panel = panel;
        cx.notify();
    }

    /// Whether extension `id`'s side panel is the one shown.
    pub(super) fn side_panel_shown(&self, id: &str) -> bool {
        self.side_panel.as_ref().is_some_and(|panel| panel.extension == id)
    }

    /// The card in a window whose content is `viewport`, and its panel's card if one shows. A page
    /// full screen takes the window, and the panel hides.
    pub(super) fn card_frames(
        &self,
        viewport: Size<Pixels>,
    ) -> (Bounds<Pixels>, Option<Bounds<Pixels>>) {
        let card_left = self.card_left();
        let card = self.little_card(page_frame(viewport, card_left, self.card_push()));
        match self.side_panel.is_some() && card_left != 0. {
            true => {
                let (page, panel) = split_card(card, panel_width(self.side_panel_width));
                (page, Some(panel))
            }
            false => (card, None),
        }
    }

    /// Where the panel's page goes in a window whose content is `viewport`, or nowhere.
    pub fn side_panel_page(&self, viewport: Size<Pixels>) -> Option<Bounds<Pixels>> {
        self.card_frames(viewport).1.map(panel_page)
    }

    /// Lays the panel's page out under its header, unless it's there already.
    pub(super) fn lay_out_side_panel(&mut self, cx: &mut Context<Self>) {
        let frame = self.side_panel_page(self.viewport);
        if self.side_panel_laid != Some(frame) {
            self.side_panel_laid = Some(frame);
            self.browser.lay_out_side_panel(frame.unwrap_or_default(), cx);
            if let (_, Some(panel)) = self.card_frames(self.viewport) {
                // check.sh reads this line, to find the header's buttons and the edge.
                log!(
                    "browser: side panel at {} {} {} {}",
                    f32::from(panel.left()).round(),
                    f32::from(panel.top()).round(),
                    f32::from(panel.size.width).round(),
                    f32::from(panel.size.height).round()
                );
            }
        }
    }

    /// The panel's card with its header, and the edge left of it that drags to resize it.
    pub(super) fn side_panel(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (Some(panel), (_, Some(bounds))) = (&self.side_panel, self.card_frames(self.viewport))
        else {
            return Vec::new();
        };
        let button = |id: &'static str, icon: &'static str| {
            div()
                .id(id)
                .flex_none()
                .size(px(URL_BUTTON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(NAV_RADIUS))
                .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                .child(svg().path(icon).size(px(URL_BUTTON_GLYPH)).text_color(self.palette.glyph(NAV)))
        };
        let header = div()
            .h(px(HEADER))
            .flex()
            .items_center()
            .gap(px(URL_BUTTON_INSET))
            .pl(px(CARD_INSET))
            .pr(px(URL_BUTTON_INSET))
            .children(panel.icon.clone().map(|icon| {
                img(icon).flex_none().size(px(HEADER_ICON)).mr(px(CARD_INSET - URL_BUTTON_INSET))
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(HEADER_TEXT))
                    .text_color(self.palette.text(TITLE))
                    .child(panel.name.clone()),
            )
            .child(button("side-panel-menu", "icons/ellipsis.svg").on_click(cx.listener(
                move |shell, _, _, cx| {
                    let at = point(
                        bounds.right() - px(2. * URL_BUTTON_SIZE + URL_BUTTON_INSET),
                        bounds.top() + px(HEADER),
                    );
                    shell.side_panel_menu(at, cx)
                },
            )))
            .child(
                button("side-panel-close", "icons/x.svg")
                    .on_click(cx.listener(|shell, _, _, cx| shell.browser.close_side_panel(cx))),
            );
        let edge = div()
            .id("side-panel-edge")
            .absolute()
            .left(bounds.left() - px(CARD_INSET))
            .top(bounds.top())
            .w(px(CARD_INSET))
            .h(bounds.size.height)
            .cursor(CursorStyle::ResizeLeftRight)
            .block_mouse_except_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    shell.side_panel_resize = Some(f32::from(event.position.x - bounds.left()));
                    // From the width shown, which a narrow window can keep under the one kept.
                    shell.side_panel_width = f32::from(bounds.size.width).round();
                    cx.notify();
                }),
            );
        // The pointer's moves and release anywhere in the window, while the edge is dragged.
        let resizing = self.side_panel_resize.is_some();
        let right = f32::from(bounds.right());
        let shell = cx.weak_entity();
        let listener = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                if !resizing {
                    return;
                }
                window.set_window_cursor_style(CursorStyle::ResizeLeftRight);
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    // Only the drag's moves: off the Mac GPUI says the pointer left a window with
                    // a move to its far corner and no button held.
                    if phase == DispatchPhase::Capture
                        && event.pressed_button == Some(MouseButton::Left)
                    {
                        let at = f32::from(event.position.x);
                        moved.update(cx, |shell, cx| shell.drag_side_panel(at, right, cx)).ok();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        shell.update(cx, |shell, cx| shell.release_side_panel(cx)).ok();
                    }
                });
            },
        );
        vec![
            card(bounds, self.palette.page).child(header).into_any_element(),
            edge.into_any_element(),
            listener.absolute().size_0().into_any_element(),
        ]
    }

    /// Moves the panel's left edge with the pointer, `at` across the window, its right edge staying
    /// at `right`.
    fn drag_side_panel(&mut self, at: f32, right: f32, cx: &mut Context<Self>) {
        let (Some(grab), (page, Some(panel))) =
            (self.side_panel_resize, self.card_frames(self.viewport))
        else {
            return;
        };
        // No wider than it can show, so the width kept is the one the user sees.
        let card = f32::from(page.size.width + panel.size.width) + CARD_INSET;
        let width = panel_width(right - (at - grab)).min(widest(card)).round();
        if width != self.side_panel_width {
            self.side_panel_width = width;
            cx.notify();
        }
    }

    /// Keeps the width the panel was dragged to in its profile.
    fn release_side_panel(&mut self, cx: &mut Context<Self>) {
        if self.side_panel_resize.take().is_none() {
            return;
        }
        let width = self.side_panel_width;
        // check.sh waits for this line.
        log!("browser: side panel width {width}");
        self.browser.set_side_panel_width(width, cx);
        cx.notify();
    }

    /// The header's ⋯ menu: the extension's Options if it has them, Manage Extension, and Close
    /// Side Panel, as Chrome's panel header gives them.
    fn side_panel_menu(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(panel) = &self.side_panel else {
            return;
        };
        let profile = self.shown_space().map_or(0, |space| space.profile);
        let options = (self.extensions.get(&profile).into_iter().flatten())
            .find(|extension| extension.id == panel.extension)
            .map(|extension| extension.options.clone())
            .filter(|options| !options.is_empty());
        let manage = panel.extension.clone();
        let mut items: Vec<(MenuEntry, MenuAction)> = Vec::new();
        if let Some(options) = options {
            items.push((
                MenuEntry::new("Options", true),
                Box::new(move |shell, cx| shell.browser.open_extension_options(&options, cx)),
            ));
        }
        items.extend([
            (
                MenuEntry::new("Manage Extension", true),
                Box::new(move |shell: &mut Shell, cx: &mut Context<Shell>| {
                    shell.browser.manage_extensions(Some(&manage), cx)
                }) as MenuAction,
            ),
            (
                MenuEntry::new("Close Side Panel", true),
                Box::new(|shell, cx| shell.browser.close_side_panel(cx)),
            ),
        ]);
        // The panel is beside the page, in the window, while the sidebar is hidden too.
        self.show_menu_in(items, position, None, false, cx);
    }
}

/// The panel's width for the one the user last gave it, `saved`: Chrome's until they give one,
/// and in Ardia's range.
fn panel_width(saved: f32) -> f32 {
    match saved > 0. {
        true => saved.clamp(PANEL_MIN, PANEL_MAX),
        false => PANEL_WIDTH,
    }
}

/// The page's card and the panel's in `card`, the panel `width` wide at its right end and a gap
/// from the page. It takes no more than half, so the page always keeps as much as it.
fn split_card(card: Bounds<Pixels>, width: f32) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let width = width.min(widest(f32::from(card.size.width)));
    let page = Bounds::new(
        card.origin,
        size(card.size.width - px(width + CARD_INSET), card.size.height),
    );
    let panel = Bounds::new(
        point(card.right() - px(width), card.top()),
        size(px(width), card.size.height),
    );
    (page, panel)
}

/// The widest the panel shows in a card `card` wide: half of it, past the gap.
fn widest(card: f32) -> f32 {
    ((card - CARD_INSET) / 2.).max(0.)
}

/// Where the panel's page goes in its card `panel`: under the header.
fn panel_page(panel: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(panel.left(), panel.top() + px(HEADER)),
        size(panel.size.width, (panel.size.height - px(HEADER)).max(px(0.))),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, point, px, size};

    use super::{CARD_INSET, HEADER, PANEL_MAX, PANEL_MIN, PANEL_WIDTH, panel_page, panel_width};
    use super::split_card;

    /// The panel opens at Chrome's width until the user gives it one, and keeps theirs within
    /// 320 to 600.
    #[test]
    fn the_panel_keeps_the_width_given_within_its_range() {
        assert_eq!(panel_width(0.), PANEL_WIDTH);
        assert_eq!(panel_width(-5.), PANEL_WIDTH);
        assert_eq!(panel_width(450.), 450.);
        assert_eq!(panel_width(100.), PANEL_MIN);
        assert_eq!(panel_width(2000.), PANEL_MAX);
    }

    /// The page narrows by the panel and the gap, and the panel's page starts under its header.
    #[test]
    fn the_page_narrows_for_the_panel() {
        let card = Bounds::new(point(px(250.), px(10.)), size(px(1000.), px(700.)));
        let (page, panel) = split_card(card, 400.);
        assert_eq!(page.origin, card.origin);
        assert_eq!(page.size.width, px(1000. - 400. - CARD_INSET));
        assert_eq!(page.size.height, card.size.height);
        assert_eq!(panel.right(), card.right());
        assert_eq!(panel.size.width, px(400.));
        assert_eq!(panel.left() - page.right(), px(CARD_INSET));
        let under = panel_page(panel);
        assert_eq!(under.top(), panel.top() + px(HEADER));
        assert_eq!(under.bottom(), panel.bottom());
        assert_eq!(under.size.width, panel.size.width);
    }

    /// In a narrow window the panel gives way, and the page keeps at least as much as it.
    #[test]
    fn a_narrow_card_keeps_half_for_the_page() {
        let card = Bounds::new(point(px(10.), px(10.)), size(px(610.), px(500.)));
        let (page, panel) = split_card(card, 600.);
        assert_eq!(panel.size.width, px(300.));
        assert!(page.size.width >= panel.size.width);
        assert_eq!(panel.right(), card.right());
    }
}
