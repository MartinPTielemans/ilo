//! A Space's theme in the window: opening the theme picker for it, showing what the picker makes
//! as it's made and keeping it, and the window's colours and grain from it (docs/ux-spec.md ›
//! Themes).

use std::rc::Rc;

use gpui::{
    App, ColorSpace, Context, Global, Hsla, Rgba, canvas, div, linear_color_stop, linear_gradient,
    point, prelude::*, px, rgb, size,
};

use super::{
    Shell,
    undo::{Prop, Value},
};
use crate::{
    palette::{self, Palette, SECOND_AT, Tone},
    theme::{self, Appearance, GRAIN_TILE, INACTIVE_TEXTURE, Texture, Theme},
    theme_picker::{self, Changed},
};

/// The appearance View › Appearance chose for every window, which stands in for the system's
/// where it isn't automatic. The user's preference: agents have no call to change it.
#[derive(Clone, Copy, Default)]
pub struct AppAppearance(pub Appearance);

impl Global for AppAppearance {}

/// The appearance View › Appearance has chosen.
pub fn app_appearance(cx: &App) -> Appearance {
    cx.try_global::<AppAppearance>()
        .map_or(Appearance::Automatic, |chosen| chosen.0)
}

/// How many themes' palettes a window keeps, the least recently drawn going first: more than
/// the Spaces whose swatches a frame draws, and the one the picker is making.
const PALETTES: usize = 64;

impl Shell {
    /// The theme `space` shows: the one the picker is making for it, else its own.
    pub(super) fn theme_of(&self, space: u64) -> Option<Theme> {
        match self.theme_preview {
            Some((previewed, theme)) if previewed == space => Some(theme),
            _ => self
                .spaces
                .iter()
                .find(|each| each.id == space)
                .and_then(|each| each.theme),
        }
    }

    /// The theme `space`'s window shows: its own, in the look its tone draws, or a theme of its
    /// colour alone for a Space whose colour came before themes did; none in a private window.
    pub(super) fn space_theme(&self, space: u64) -> Option<Theme> {
        if self.private {
            return None;
        }
        let dark = self.tone_of(space) == Tone::Dark;
        self.theme_of(space)
            .map(|theme| theme.shown(dark))
            .or_else(|| {
                let entry = self.spaces.iter().find(|each| each.id == space)?;
                entry.color.map(Theme::of_color)
            })
    }

    /// Whether `space`'s window is dark or light: as its theme's appearance forces it, else as
    /// the system's is, as Arc's follows it. A private window is dark, as Arc's is.
    pub(super) fn tone_of(&self, space: u64) -> Tone {
        self.tone_with(self.theme_of(space).as_ref())
    }

    /// The tone a Space with `theme` draws in: the theme's, else the system's; dark in a private
    /// window.
    pub(super) fn tone_with(&self, theme: Option<&Theme>) -> Tone {
        if self.private {
            return Tone::Dark;
        }
        match theme.map(|theme| theme.appearance) {
            Some(Appearance::Light) => Tone::Light,
            Some(Appearance::Dark) => Tone::Dark,
            Some(Appearance::Automatic) | None => self.system_tone,
        }
    }

    /// Reads the system's appearance and whether the window is active as the window draws, and
    /// draws the window again once either changes, which GPUI doesn't by itself.
    pub(super) fn watch_appearance(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let system = match window.appearance() {
            gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => Tone::Light,
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => Tone::Dark,
        };
        self.app_appearance = app_appearance(cx);
        let chosen = match self.app_appearance {
            Appearance::Light => Some(Tone::Light),
            Appearance::Dark => Some(Tone::Dark),
            Appearance::Automatic => None,
        };
        self.system_tone = self.forced_tone.or(chosen).unwrap_or(system);
        self.window_active = window.is_window_active();
        if self.window_watches.is_empty() {
            self.window_watches = vec![
                cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
                cx.observe_window_activation(window, |_, _, cx| cx.notify()),
            ];
        }
    }

    /// Gives every window `appearance` in place of the system's, and keeps it for the next launch.
    pub(super) fn set_app_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: appearance {}", appearance.name());
        cx.set_global(AppAppearance(appearance));
        self.app_appearance = appearance;
        self.browser.set_appearance(appearance, cx);
        self.update_menus(cx);
        cx.refresh_windows();
    }

    /// View › Appearance and Show Space Header as they came from the account (settings_synced):
    /// the window draws with them and its menus say so, as when the user chose them here, but
    /// nothing is kept again, as the core has them.
    pub fn look_followed(&mut self, appearance: Appearance, header: bool, cx: &mut Context<Self>) {
        cx.set_global(AppAppearance(appearance));
        cx.set_global(super::LoneSpaceHeader(header));
        if self.app_appearance == appearance && self.lone_header == header {
            return;
        }
        self.app_appearance = appearance;
        // check.sh waits for the outline the rows are drawn with next.
        self.outline_changed |= self.lone_header != header;
        self.lone_header = header;
        self.update_menus(cx);
        cx.notify();
    }

    /// Draws the window as if the system's appearance were `tone`, as the preview does to show
    /// either without changing the system's.
    pub fn force_tone(&mut self, tone: Option<Tone>) {
        self.forced_tone = tone;
    }

    /// `space`'s palette, worked out once for each theme and tone it shows rather than while
    /// drawing.
    pub(super) fn palette_of(&self, space: u64) -> Palette {
        let key = (self.space_theme(space), self.tone_of(space));
        let mut palettes = self.palettes.borrow_mut();
        if let Some(at) = palettes.iter().position(|(each, _)| *each == key) {
            let kept = palettes.remove(at);
            palettes.push(kept);
            return kept.1;
        }
        let palette = Palette::of(key.0.as_ref(), key.1);
        if palettes.len() == PALETTES {
            palettes.remove(0);
        }
        palettes.push((key, palette));
        palette
    }

    /// The palette the chrome draws with: the shown Space's, or while the sidebar pages between
    /// two, going from one's to the other's as the window's colour does. The key window's is the
    /// one what floats over the page takes, and its accent the focused page's ring.
    pub(super) fn update_palette(&mut self, cx: &mut Context<Self>) {
        let (space, beside) = self.colored_pages();
        let mut palette = self.palette_of(space);
        if let Some((beside, t)) = beside {
            palette = palette.toward(&self.palette_of(beside), t);
        }
        self.palette = palette;
        let light = palette.tone == Tone::Light;
        if self.window_light != Some(light) {
            self.window_light = Some(light);
            self.browser.set_window_light(light, cx);
            log!("browser: window light {light}");
        }
        let look = self.menu_look();
        if self.capture_look != Some(look) {
            self.capture_look = Some(look);
            self.browser.capture_look(self.selected, look, cx);
        }
        if super::commands::key_shell(cx).is_some_and(|key| key == cx.weak_entity()) {
            palette::set_current(palette, cx);
        }
    }

    /// Shows the theme picker for `space` `at` its place in the window, to its left if `left`, with
    /// the Space's theme, or its colour alone if it has no theme.
    pub(super) fn show_theme_picker(
        &mut self,
        space: u64,
        at: theme_picker::At,
        left: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.spaces.iter().find(|each| each.id == space) else {
            return;
        };
        // A theme of two looks opens as the one shown, and what the picker makes is one look.
        let dark = self.tone_of(space) == Tone::Dark;
        let theme = entry
            .theme
            .map(|theme| theme.shown(dark))
            .or_else(|| entry.color.map(Theme::of_color));
        let shell = cx.weak_entity();
        let changed: Changed = Rc::new(move |theme, done, cx| {
            shell
                .update(cx, |shell, cx| shell.theme_changed(space, theme, done, cx))
                .ok();
        });
        let height = f32::from(self.viewport.height);
        // A card showing would stay under the picker, as rows' cards wait while it shows.
        self.browser.hover_card(crate::hover_card::Hover::Close, cx);
        theme_picker::show(
            self.browser.window(),
            space,
            theme,
            at,
            left,
            height,
            changed,
            cx,
        );
    }

    /// The picker made `theme` for `space`: the window shows it at once, and once the user is
    /// `done` the core keeps it, with its main colour as the Space's colour, as one step for ⌘Z:
    /// a drag is one, however many frames it took.
    pub(super) fn theme_changed(
        &mut self,
        space: u64,
        theme: Theme,
        done: bool,
        cx: &mut Context<Self>,
    ) {
        self.theme_preview = Some((space, theme));
        if done {
            let color = Value::Color {
                color: Some(theme.main_color()),
                theme: theme.to_text(),
            };
            self.change(Prop::SpaceColor(space), color, cx);
        }
        cx.notify();
    }

    /// Takes `space`'s theme and colour away, so it shows the underlay alone and ilo's accent.
    pub(super) fn remove_space_theme(&mut self, space: u64, cx: &mut Context<Self>) {
        let none = Value::Color {
            color: None,
            theme: String::new(),
        };
        self.change(Prop::SpaceColor(space), none, cx);
    }

    /// The Spaces came from the core: the theme being made gives way to the Space's own once
    /// the core has it, or the Space has gone.
    pub(super) fn settle_theme_preview(&mut self) {
        let Some((space, theme)) = self.theme_preview else {
            return;
        };
        let kept = self
            .spaces
            .iter()
            .find(|each| each.id == space)
            .map(|each| each.theme);
        if kept.is_none_or(|kept| kept.map(|kept| kept.to_text()) == Some(theme.to_text())) {
            self.theme_preview = None;
        }
    }

    /// `theme`'s colour `x` of the way across the window and `y` of the way down, at `alpha`: one
    /// colour going to the same at HSL lightness + 0.25 down the window, as a Space's colour does;
    /// two from the main one at the top left to the second along the diagonal; and a third coming
    /// in toward the bottom.
    pub(super) fn theme_color_at(&self, theme: &Theme, alpha: f32, x: f32, y: f32) -> Rgba {
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let main = Hsla::from(colors[0]);
        let color = match colors.len() {
            1 => Rgba::from(Hsla {
                l: (main.l + 0.25 * y).min(1.),
                ..main
            }),
            count => {
                let (width, height) = (self.viewport.width.into(), self.viewport.height.into());
                let along = (theme::diagonal(x, y, width, height) / SECOND_AT).min(1.);
                let two = theme::mix_oklab(colors[0], colors[1], along);
                if count > 2 {
                    theme::mix_oklab(two, colors[2], y)
                } else {
                    two
                }
            }
        };
        Rgba { a: alpha, ..color }
    }

    /// Where the sidebar's middle is across the window, 0 to 1, where the window's colour is taken
    /// for what covers it.
    pub(super) fn sidebar_x(&self) -> f32 {
        let x = self.width() / 2. / f32::from(self.viewport.width).max(1.);
        match self.right() {
            true => 1. - x,
            false => x,
        }
    }

    /// The window's colour behind everything: the gradient down it, or with a Space of more than
    /// one colour shown, its colours across it, one Space's over the other's as the sidebar pages
    /// between them; and each one's texture over them, fainter while the window isn't active.
    pub(super) fn window_background(&self) -> gpui::Div {
        let (space, beside) = self.colored_pages();
        // Each Space shown, and how much of the window's colour is its.
        let pages = match beside {
            Some((beside, t)) => vec![(space, 1. - t), (beside, t)],
            None => vec![(space, 1.)],
        };
        let themes: Vec<_> = pages
            .into_iter()
            .map(|(space, weight)| {
                let theme = self.space_theme(space);
                (space, theme, weight)
            })
            .collect();
        let layered = themes
            .iter()
            .any(|(_, theme, _)| theme.is_some_and(|theme| theme.count > 1));
        let background = div().absolute().size_full();
        let background = if layered {
            background.children(
                themes
                    .iter()
                    .map(|&(space, theme, weight)| self.theme_layer(theme, space).opacity(weight)),
            )
        } else {
            background.bg(self.gradient(None))
        };
        let dimmed = if self.window_active {
            1.
        } else {
            INACTIVE_TEXTURE
        };
        background.children(themes.iter().filter_map(|&(_, theme, weight)| {
            let theme = theme.filter(|theme| theme.grain > 0.)?;
            Some(texture_layer(
                theme.texture,
                theme.grain_opacity() * weight * dimmed,
            ))
        }))
    }

    /// `space`'s colours over the whole window, with `theme`.
    fn theme_layer(&self, theme: Option<Theme>, space: u64) -> gpui::Div {
        let layer = div().absolute().top_0().left_0();
        let Some(theme) = theme.filter(|theme| theme.count > 1) else {
            return layer.size_full().bg(self.space_gradient(space));
        };
        let colors: Vec<Rgba> = theme.colors().into_iter().map(rgb).collect();
        let alpha = self.palette_of(space).layer_alpha;
        let stop = |color: Rgba, at: f32| linear_color_stop(Rgba { a: alpha, ..color }, at);
        let (width, height) = (
            f32::from(self.viewport.width),
            f32::from(self.viewport.height),
        );
        let (side, start, end) = diagonal_stops(width, height);
        let diagonal = linear_gradient(135., stop(colors[0], start), stop(colors[1], end))
            .color_space(ColorSpace::Oklab);
        let layer = layer.size(px(side)).bg(diagonal);
        match colors.get(2) {
            Some(&third) => layer.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(width))
                    .h(px(height))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(Rgba { a: 0., ..third }, 0.),
                        stop(third, 1.),
                    )
                    .color_space(ColorSpace::Oklab)),
            ),
            None => layer,
        }
    }
}

/// The square the diagonal is drawn on, from the window's top left, and its stops, for a window
/// `width` by `height`. GPUI's 135° gradient on a square runs along its diagonal from -0.207 at
/// its top left corner (0.5 - √2 / 2) to 1.207 at its bottom right, so on one as big as the
/// window's longer side, these stops put the main colour at the window's top left and the second
/// SECOND_AT of the way along the window's own 135° line, as CSS draws it.
fn diagonal_stops(width: f32, height: f32) -> (f32, f32, f32) {
    let side = width.max(height).max(1.);
    let start = 0.5 - std::f32::consts::FRAC_1_SQRT_2;
    let end = start + std::f32::consts::FRAC_1_SQRT_2 * SECOND_AT * (width + height) / side;
    (side, start, end)
}

/// `texture` over the whole window at `opacity`, in tiles at one device pixel to a pixel.
fn texture_layer(texture: Texture, opacity: f32) -> impl IntoElement {
    div().absolute().size_full().opacity(opacity).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let image = texture.image();
                let tile = GRAIN_TILE as f32 / window.scale_factor();
                let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let (columns, rows) = ((width / tile).ceil() as i32, (height / tile).ceil() as i32);
                for row in 0..rows {
                    for column in 0..columns {
                        let origin =
                            bounds.origin + point(px(column as f32 * tile), px(row as f32 * tile));
                        let tile = gpui::Bounds::new(origin, size(px(tile), px(tile)));
                        window
                            .paint_image(tile, tile, px(0.).into(), image.clone(), 0, false)
                            .ok();
                    }
                }
            },
        )
        .size_full(),
    )
}

#[cfg(test)]
mod tests;
