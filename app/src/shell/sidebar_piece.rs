//! A piece of a Space's sidebar, as if seen through a window onto the desktop: its address field,
//! two favourites, the Space's name, a pinned row, three Today tabs and the foot, over a stand-in
//! desktop and drawn from the Space's palette as the theme editor makes it (#4450). First run's
//! Space step shows it beside its editor, so each control's effect is seen as it moves.

use gpui::{FontWeight, SharedString, div, point, prelude::*, px, rgb, svg};

use super::onboarding::Step;
use super::{
    FOOTER_HEIGHT, ICON_INK, NAV, SECONDARY, Shell, URL_FIELD, divider_box, name_box, plate,
    row_box, selected_plate, title,
};
use crate::elevation::Level;
use crate::palette::{Ground, PAPER};
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::shell::{
    BLOCK_GAP, BLOCK_TOP, CARD_INSET, CARD_RADIUS, FAVICON_DRAWN, FOOT_GLYPH, FOOT_PLATE, FOOT_ROW,
    HAIRLINE, LIBRARY_BUTTON, PLATE_INSET, PLATE_RADIUS, SIDEBAR_WIDTH, SPACE_BUTTON_CHEVRON,
    SPACE_BUTTON_GAP, SPACE_BUTTON_PAD, TILE_GAP, TILE_HEIGHT, TILE_RADIUS, URL_FIELD_HEIGHT,
    URL_FIELD_TEXT_X,
};

/// The made-up sites: two favourites, the pinned row, then Today's three, the first shown.
const FAVOURITES: [&str; 2] = ["M", "P"];
const PINNED: &str = "Reading list";
const TODAY: [&str; 3] = ["Island Ferries", "Recipe notes", "Weekly plan"];
/// The shown tab's address, as the field says it.
const HOST: &str = "islandferries.example";

/// What the piece last drew, for a test to read its colours changing.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Drawn {
    pub name: SharedString,
    /// What lies over the desktop under the Space's colour: the tint over the material, or the
    /// opaque ground where nothing blurs.
    pub ground: gpui::Rgba,
    pub space_tint: gpui::Rgba,
    pub selected: gpui::Hsla,
    pub grain: f32,
}

#[cfg(test)]
thread_local! {
    pub(super) static DRAWN: std::cell::RefCell<Option<Drawn>> = const { std::cell::RefCell::new(None) };
}

impl Shell {
    /// `space`'s sidebar called `name`, in the look its window shows now, filling its parent with
    /// the plane's corners.
    pub(super) fn sidebar_piece(&self, space: u64, name: SharedString) -> gpui::Div {
        let palette = self.palette_of(space);
        let grain = (self.space_theme(space))
            .filter(|theme| theme.grain > 0.)
            .map(|theme| (theme.texture, theme.grain_opacity()));
        // The window's ground as the real one lays it (`window_tint`): the material and its tint at
        // the Space's Transparency where the window's material blurs what's behind it, else the
        // opaque stand-in; the Space's flat colour (#4402); then its grain, as the window's
        // background has them.
        let dark = palette.tone == crate::palette::Tone::Dark;
        let ground = match self.blurred {
            true => vec![crate::transparency::material(dark), palette.tint],
            false => vec![palette.underlay],
        };
        #[cfg(test)]
        DRAWN.set(Some(Drawn {
            name: name.clone(),
            ground: ground[ground.len() - 1],
            space_tint: palette.space_tint,
            selected: palette.shown(0.),
            grain: grain.map_or(0., |(_, opacity)| opacity),
        }));
        let [inset, window_radius, field_top] = t::SPACE_PIECE;
        let ink = palette.glyph(ICON_INK);
        let field = div()
            .relative()
            .flex_none()
            .mt(px(field_top))
            .mx(px(PLATE_INSET))
            .h(px(URL_FIELD_HEIGHT))
            .flex()
            .items_center()
            .pl(px(URL_FIELD_TEXT_X - PLATE_INSET))
            .rounded(px(PLATE_RADIUS))
            .bg(palette.ground_plate(Ground::Field, URL_FIELD))
            .text_size(px(crate::tokens::text::BODY))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(PLATE_RADIUS))
                    .border(px(HAIRLINE))
                    .border_color(Level::Resting.rim_in(&palette)),
            )
            .child(
                div()
                    .truncate()
                    .text_color(palette.text(SECONDARY))
                    .child(HOST),
            );
        let [first, second, pinned_icon, today_icons @ ..] = t::SPACE_ICONS;
        let tiles = div()
            .flex_none()
            .mt(px(BLOCK_TOP))
            .px(px(PLATE_INSET))
            .flex()
            .gap(px(TILE_GAP))
            .children(
                FAVOURITES
                    .iter()
                    .zip([first, second])
                    .map(|(letter, color)| {
                        div()
                            .flex_1()
                            .h(px(TILE_HEIGHT))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(TILE_RADIUS))
                            .border(px(HAIRLINE))
                            .border_color(Level::Resting.rim_in(&palette))
                            .bg(super::favorites::tile_fill(&palette, false, 0.))
                            .child(letter_icon(color, letter))
                    }),
            );
        let row = |color: u32, label: &'static str, shown: bool| {
            let words = title(&palette, label.into(), shown)
                .when(shown, |words| words.font_weight(FontWeight::MEDIUM));
            let plate = plate(letter_icon(color, &label[..1]), words).relative();
            let plate = match shown {
                true => selected_plate(&palette, plate, 1., 0.)
                    .children(self.place(Step::Space, "piece-shown")),
                false => plate,
            };
            row_box().child(plate)
        };
        let today = TODAY
            .iter()
            .zip(today_icons)
            .enumerate()
            .map(|(index, (label, color))| row(color, label, index == 0));
        let foot = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(FOOTER_HEIGHT))
            .child(
                div()
                    .absolute()
                    .left(px(super::library::LIBRARY_BACK_INSET))
                    .bottom(px((FOOTER_HEIGHT - FOOT_ROW) / 2.))
                    .w(px(LIBRARY_BUTTON))
                    .h(px(FOOT_ROW))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path("icons/library.svg")
                            .size(px(FOOT_GLYPH))
                            .text_color(palette.glyph(NAV)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(super::footer::space_button_right()))
                    .bottom(px((FOOTER_HEIGHT - FOOT_PLATE) / 2.))
                    .w(px(super::footer::space_button_width()))
                    .h(px(FOOT_PLATE))
                    .px(px(SPACE_BUTTON_PAD))
                    .flex()
                    .items_center()
                    .gap(px(SPACE_BUTTON_GAP))
                    .child(crate::space_icon::mark("", &name, FOOT_GLYPH, ink))
                    .child(
                        svg()
                            .path("icons/chevrons-up-down.svg")
                            .size(px(SPACE_BUTTON_CHEVRON))
                            .flex_none()
                            .text_color(ink),
                    ),
            );
        let sidebar = div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(SIDEBAR_WIDTH))
            .flex()
            .flex_col()
            .child(field)
            .child(tiles)
            .child(
                name_box(&palette, false, false)
                    .mt(px(BLOCK_GAP))
                    .child(div().truncate().child(name)),
            )
            .child(row(pinned_icon, PINNED, false))
            .child(divider_box(&palette, self.hairline(), None::<gpui::Div>))
            .children(today)
            .child(foot);
        // The page beside the sidebar, cut off by the plane's edges, with its corner where they
        // meet.
        let page = div()
            .absolute()
            .left(px(SIDEBAR_WIDTH))
            .top(px(CARD_INSET))
            .right_0()
            .bottom_0()
            .rounded_tl(px(CARD_RADIUS))
            .rounded_br(px(t::PLANE_RADIUS))
            .bg(palette.page);
        let layer = || {
            div()
                .absolute()
                .inset_0()
                .rounded_tl(px(window_radius))
                .rounded_br(px(t::PLANE_RADIUS))
        };
        let window = div()
            .absolute()
            .left(px(inset))
            .top(px(inset))
            .right_0()
            .bottom_0()
            .children(ground.into_iter().map(|fill| layer().bg(fill)))
            .child(layer().bg(palette.space_tint))
            .children(grain.map(|(texture, opacity)| {
                layer()
                    .overflow_hidden()
                    .child(super::themes::texture_layer(texture, opacity))
            }))
            .child(sidebar)
            .child(page);
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .rounded(px(t::PLANE_RADIUS))
            .child(desktop())
            .child(window)
    }
}

/// The stand-in desktop: the Transparency pictures' with soft lights on it, as what's behind a
/// window comes through its material blurred.
fn desktop() -> gpui::Div {
    let [size, blur] = t::SPACE_LIGHT;
    let lights = t::SPACE_LIGHT_COLORS
        .iter()
        .enumerate()
        .map(|(index, &color)| {
            let (x, y) = (t::SPACE_LIGHTS[2 * index], t::SPACE_LIGHTS[2 * index + 1]);
            div()
                .absolute()
                .left(px(x - size / 2.))
                .top(px(y - size / 2.))
                .size(px(size))
                .rounded_full()
                .shadow(vec![gpui::BoxShadow {
                    color: rgb(color).into(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(blur),
                    spread_radius: px(0.),
                    inset: false,
                }])
        });
    crate::transparency::desktop().children(lights)
}

/// A made-up site's icon: its `letter` on a tile of `color`, as big as a favicon is drawn.
fn letter_icon(color: u32, letter: &str) -> impl IntoElement + use<> {
    let [radius, size] = t::SPACE_ICON;
    div()
        .flex_none()
        .size(px(FAVICON_DRAWN))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .bg(rgb(color))
        .text_size(px(size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(PAPER))
        .child(SharedString::from(letter.to_string()))
}
