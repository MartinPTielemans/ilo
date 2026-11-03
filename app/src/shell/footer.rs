//! The sidebar's footer: the Space switcher and the plus button beside it.

use gpui::{Context, MouseButton, MouseDownEvent, Role, div, prelude::*, px, svg};

use super::{
    FOOTER_HEIGHT, NAV, NAV_GLYPH, NAV_HOVERED, NAV_RADIUS, NAV_SIZE, PLUS_INSET, SPACE_GLYPH,
    SPACE_HEIGHT, SPACE_HOVERED, SPACE_ICON_SWITCHER, SPACE_OTHER, SPACE_PITCH, SPACE_SHOWN,
    SPACE_SWATCH, SPACE_SWATCH_HOVERED, SPACE_SWATCH_SHOWN, SWITCHER_INSET, Shell,
    access::Accessible,
};
use crate::space_icon::{GLYPHS, SpaceIcon};

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
        let span = self.width() - 2. * SWITCHER_INSET;
        let pitch = SPACE_PITCH.min(span / self.spaces.len() as f32);
        // What the swatches lie on: the window at the footer, over whatever is behind it.
        let window = f32::from(self.viewport.height).max(1.);
        let at = self.gradient_color((window - FOOTER_HEIGHT / 2.) / window);
        let behind = self.palette.over_backdrops(at);
        let dots = self.spaces.iter().map(|space| {
            let (id, shown) = (space.id, space.id == self.space);
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
                .w(px(pitch))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
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
                .child(match SpaceIcon::parse(&space.icon) {
                    SpaceIcon::Dot => {
                        let size = if shown {
                            SPACE_SWATCH_SHOWN
                        } else {
                            SPACE_SWATCH
                        };
                        (self.palette_of(id).swatch.draw(size, &behind))
                            .when(!shown, |swatch| {
                                swatch.group_hover("space", |style| {
                                    style.size(px(SPACE_SWATCH_HOVERED))
                                })
                            })
                            .into_any_element()
                    }
                    // The Space shown's in its accent, as its dot is in its colour, or the ink
                    // without one; the others' in the dots' greys.
                    SpaceIcon::Glyph(index) => svg()
                        .path(GLYPHS[index].1)
                        .size(px(SPACE_GLYPH))
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
                        .text_size(px(SPACE_ICON_SWITCHER))
                        .opacity(icon_opacity(shown, false))
                        .when(!shown, |icon| {
                            icon.group_hover("space", |style| {
                                style.opacity(icon_opacity(false, true))
                            })
                        })
                        .child(emoji.to_string())
                        .into_any_element(),
                })
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
                .children(dots),
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
}

#[cfg(test)]
mod tests {
    use super::icon_opacity;

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
