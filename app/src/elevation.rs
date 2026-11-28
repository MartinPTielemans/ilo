//! The three elevation levels every overlay takes (#317). Each is one recipe: a rim inside the
//! surface's edge, the outline round the outside and a drop shadow, with the values in
//! `design/tokens.toml` under `[elevation]`. A surface GPUI draws takes `shadow()` and `rim()`;
//! one the host draws a shadow window for takes `host_shadow()`, and on Windows draws its shadow
//! in its own window with `in_window()`.

use gpui::{
    AnyElement, BoxShadow, Hsla, IntoElement, ParentElement as _, Pixels, Rgba, Size, Styled, div,
    point, px, rgba,
};

use crate::palette::Palette;
use crate::tokens::elevation as t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Sits on the window: favorite tiles, library cards, pictures of pages.
    Resting,
    /// Opens from something and closes on a click outside: menus, the Site Controls, the toast,
    /// the zoom HUD, tooltips, the link target, the find bar, the command bar over the URL field.
    Popover,
    /// Takes the keyboard: the centred command bar, the dialog, the icon and theme pickers.
    Modal,
}

/// How far a window that draws a popover's shadow itself reaches past what it holds.
pub const WINDOW_MARGIN: f32 = t::WINDOW_MARGIN;

/// Whether a panel the host floats in a window of its own size, the command bar, the tab switcher,
/// the find bar and the offer, draws its level's shadow itself, in a margin round it in that window,
/// as the menus do: on Windows, whose own shadow comes only with its own corner, rounder or squarer
/// than the panel's, which left a sliver of the window round it (#2988). The Mac's and Linux's
/// hosts draw it in a window under the panel.
pub const PANEL_SHADOW_IN_WINDOW: bool = cfg!(target_os = "windows");

/// What the host's shadow window draws, in Core Animation's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostShadow {
    pub outline_opacity: f32,
    pub outline_radius: f64,
    pub shadow_opacity: f32,
    pub shadow_radius: f64,
    pub shadow_drop: f64,
}

/// The black's alpha of an `rgba` colour.
fn alpha(color: u32) -> f32 {
    (color & 0xff) as f32 / 255.
}

/// The Gaussian's standard deviation of a CSS blur, which is half of it. GPUI's shader and Core
/// Animation's shadow layers both take this, not the blur.
fn sigma(blur: f32) -> f32 {
    blur / 2.
}

/// How far past a shadow's edge it can still be seen, in points. A shadow is a Gaussian's tail
/// there, and an 8-bit channel can't show less than half a step.
fn invisible_past(color: u32, blur: f32) -> f32 {
    let sigma = sigma(blur);
    // The tail of the normal distribution at `z` deviations, from the error function GPUI's shader
    // approximates the same way.
    let tail = |z: f32| {
        let x = z / std::f32::consts::SQRT_2;
        0.5 / (1. + (0.278393 + (0.230389 + (0.000972 + 0.078108 * x) * x) * x) * x).powi(4)
    };
    let mut distance = 0.;
    while alpha(color) * tail(distance / sigma) >= 0.5 / 255. {
        distance += 0.25;
    }
    distance
}

/// How far past what it shadows a drop shadow of `color`, falling `y` with `blur`, can be seen.
fn reach_of(color: u32, y: f32, blur: f32) -> Reach {
    let sides = invisible_past(color, blur);
    Reach {
        above: (sides - y).max(0.),
        below: sides + y,
        sides,
    }
}

/// How far past what a level shadows its drop shadow can be seen, on each side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub above: f32,
    pub below: f32,
    pub sides: f32,
}

/// The level as the core draws it in a page, as Arrange lifts an element off it: colours as
/// 0xRRGGBBAA and blurs as CSS's, as the tokens have them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lift {
    pub rim: u32,
    pub outline: u32,
    pub shadow: u32,
    pub outline_blur: f32,
    pub shadow_y: f32,
    pub shadow_blur: f32,
}

impl Level {
    /// The white line inside the surface's edge.
    pub fn rim(self) -> Rgba {
        rgba(self.rim_token())
    }

    fn rim_token(self) -> u32 {
        match self {
            Level::Resting => t::RESTING_RIM,
            Level::Popover => t::POPOVER_RIM,
            Level::Modal => t::MODAL_RIM,
        }
    }

    /// For a surface the core draws in a page.
    pub fn lift(self) -> Lift {
        let (shadow, shadow_y, shadow_blur) = self.drop();
        Lift {
            rim: self.rim_token(),
            outline: t::OUTLINE,
            shadow,
            outline_blur: t::OUTLINE_BLUR,
            shadow_y,
            shadow_blur,
        }
    }

    /// The rim in the palette's ink, Paper or Ink as the Space's theme has it, at the level's
    /// alpha, for a surface the theme colours.
    pub fn rim_in(self, palette: &Palette) -> Hsla {
        palette.faint(self.rim().a)
    }

    /// The drop shadow's colour, how far it falls and its blur.
    fn drop(self) -> (u32, f32, f32) {
        match self {
            Level::Resting => (
                t::RESTING_SHADOW,
                t::RESTING_SHADOW_Y,
                t::RESTING_SHADOW_BLUR,
            ),
            Level::Popover => (
                t::POPOVER_SHADOW,
                t::POPOVER_SHADOW_Y,
                t::POPOVER_SHADOW_BLUR,
            ),
            Level::Modal => (t::MODAL_SHADOW, t::MODAL_SHADOW_Y, t::MODAL_SHADOW_BLUR),
        }
    }

    /// For a surface GPUI draws, which is wholly inside its window: the drop shadow, and over it
    /// the outline.
    pub fn shadow(self) -> Vec<BoxShadow> {
        let (color, y, blur) = self.drop();
        let shadow = |color: u32, y: f32, blur: f32| BoxShadow {
            color: rgba(color).into(),
            offset: point(px(0.), px(y)),
            blur_radius: px(sigma(blur)),
            spread_radius: px(0.),
            inset: false,
        };
        vec![
            shadow(color, y, blur),
            shadow(t::OUTLINE, 0., t::OUTLINE_BLUR),
        ]
    }

    /// For a surface the host puts a shadow window under.
    pub fn host_shadow(self) -> HostShadow {
        let (color, y, blur) = self.drop();
        HostShadow {
            outline_opacity: alpha(t::OUTLINE),
            outline_radius: f64::from(sigma(t::OUTLINE_BLUR)),
            shadow_opacity: alpha(color),
            shadow_radius: f64::from(sigma(blur)),
            shadow_drop: f64::from(y),
        }
    }

    /// How much room a window that draws this level's shadow itself needs round what it holds.
    pub fn reach(self) -> Reach {
        let (color, y, blur) = self.drop();
        reach_of(color, y, blur)
    }

    /// The margin a panel at this level leaves round itself in its window for its shadow, in whole
    /// points, which the host leaves room for (host_set_shadow_margin): its reach where it draws its
    /// own shadow (`PANEL_SHADOW_IN_WINDOW`), else none.
    pub fn panel_margin(self) -> Reach {
        self.margin_if(PANEL_SHADOW_IN_WINDOW)
    }

    fn margin_if(self, in_window: bool) -> Reach {
        let reach = self.reach();
        match in_window {
            true => Reach {
                above: reach.above.ceil(),
                below: reach.below.ceil(),
                sides: reach.sides.ceil(),
            },
            false => Reach {
                above: 0.,
                below: 0.,
                sides: 0.,
            },
        }
    }

    /// `panel`, a surface at this level that fills what its window holds, in the margin its shadow
    /// draws in, with that shadow, where it draws its own (`PANEL_SHADOW_IN_WINDOW`).
    pub fn in_window(self, panel: impl IntoElement + Styled) -> AnyElement {
        self.in_room_of(self, panel)
    }

    /// As `in_window`, in the margin `room` leaves, a level whose shadow reaches as far as this
    /// one's at least: a panel that shows at two levels keeps the larger's room in one window.
    pub fn in_room_of(self, room: Level, panel: impl IntoElement + Styled) -> AnyElement {
        self.in_window_if(PANEL_SHADOW_IN_WINDOW, room, panel)
    }

    fn in_window_if(
        self,
        in_window: bool,
        room: Level,
        panel: impl IntoElement + Styled,
    ) -> AnyElement {
        if !in_window {
            return panel.into_any_element();
        }
        let margin = room.margin_if(true);
        div()
            .size_full()
            .pt(px(margin.above))
            .px(px(margin.sides))
            .pb(px(margin.below))
            .child(panel.shadow(self.shadow()))
            .into_any_element()
    }
}

/// A menu's glass (#4090): a popover with two shadows of its own, close and wide, fainter in the
/// `light` look, under the levels' outline.
pub fn menu_shadow(light: bool) -> Vec<BoxShadow> {
    let (contact, wide) = match light {
        true => (t::LIGHT_MENU_CONTACT, t::LIGHT_MENU_SHADOW),
        false => (t::MENU_CONTACT, t::MENU_SHADOW),
    };
    let shadow = |color: u32, y: f32, blur: f32| BoxShadow {
        color: rgba(color).into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(sigma(blur)),
        spread_radius: px(0.),
        inset: false,
    };
    vec![
        shadow(wide, t::MENU_SHADOW_Y, t::MENU_SHADOW_BLUR),
        shadow(contact, t::MENU_CONTACT_Y, t::MENU_CONTACT_BLUR),
        shadow(t::OUTLINE, 0., t::OUTLINE_BLUR),
    ]
}

/// How much room a menu's window leaves round its panels for `menu_shadow`, in either look.
pub fn menu_reach() -> Reach {
    let wide = reach_of(t::MENU_SHADOW, t::MENU_SHADOW_Y, t::MENU_SHADOW_BLUR);
    let contact = reach_of(t::MENU_CONTACT, t::MENU_CONTACT_Y, t::MENU_CONTACT_BLUR);
    Reach {
        above: wide.above.max(contact.above),
        below: wide.below.max(contact.below),
        sides: wide.sides.max(contact.sides),
    }
}

impl Reach {
    /// The size of a window that holds a panel `size` big with this margin round it.
    pub fn around(self, size: Size<Pixels>) -> Size<Pixels> {
        gpui::size(
            size.width + px(2. * self.sides),
            size.height + px(self.above + self.below),
        )
    }
}

#[cfg(test)]
mod tests;
