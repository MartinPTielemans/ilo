//! First run's frame (spec-first-run.md): its window holds the step in a column on the left, under
//! the row the window's buttons sit in, and the picture of the browser in a panel on the right.

use gpui::{
    AnyElement, App, Context, FontWeight, MouseButton, Rgba, SharedString, Stateful, div,
    prelude::*, px, rgb,
};

use super::{Colors, Shell, Step, display_text, loops, skip_frame, with_alpha};
use crate::import::Corner;
use crate::settings_rows::{named_button, ring};
use crate::theme_picker::SWATCHES;
use crate::tokens::palette::INK;
pub(super) use crate::tokens::shell::onboarding::sheet::{SWATCH, SWATCH_PITCH};
use crate::tokens::shell::onboarding::window as t;

/// The frame's inks in the window's appearance: the drawing's in Dark, light-values.md's in Light.
#[derive(Clone, Copy)]
pub(super) struct Inks {
    text: u32,
    pub quiet: f32,
    pub faint: f32,
    pub on_primary: u32,
    fill: (u32, f32),
    page: (u32, f32),
    line: (u32, f32),
}

impl Inks {
    pub(super) fn of(colors: Colors) -> Inks {
        match colors.ink == INK {
            true => Inks {
                text: t::LIGHT_TEXT,
                quiet: t::LIGHT_QUIET,
                faint: t::LIGHT_FAINT,
                on_primary: t::LIGHT_ON_PRIMARY,
                fill: (t::LIGHT_FILL, t::LIGHT_FILL_ALPHA),
                page: (t::LIGHT_OWN_PAGE, t::LIGHT_OWN_PAGE_ALPHA),
                line: (t::LIGHT_PANEL_LINE, t::LIGHT_PANEL_LINE_ALPHA),
            },
            false => Inks {
                text: t::TEXT,
                quiet: t::QUIET,
                faint: t::FAINT,
                on_primary: t::ON_PRIMARY,
                fill: (t::FILL, t::FILL_ALPHA),
                page: (t::OWN_PAGE, t::OWN_PAGE_ALPHA),
                line: (t::PANEL_LINE, t::PANEL_LINE_ALPHA),
            },
        }
    }

    /// The text's ink at `alpha`.
    pub(super) fn text(&self, alpha: f32) -> Rgba {
        with_alpha(self.text, alpha)
    }

    /// The window's ground: over its glass where it has some, else opaque.
    pub(super) fn ground(&self, glass: bool) -> Rgba {
        let (fill, alpha) = self.fill;
        with_alpha(fill, if glass { alpha } else { 1. })
    }
}

/// The step's own choices, such as the import's view in its states or the favorites' tiles: as
/// wide as the column, scrolling in it above the foot when taller than the room there.
pub(super) fn choice(view: impl IntoElement) -> Stateful<gpui::Div> {
    div()
        .id("onboarding-choice")
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_y_scroll()
        .text_size(px(crate::tokens::text::BODY))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(view)
}

/// The window's whole frame: `left` the step's column, `art` the picture's panel. `dialog` makes
/// the frame the step's dialog, as `id` names it.
pub(super) fn frame(
    colors: Colors,
    id: (&'static str, usize),
    dialog: impl FnOnce(Stateful<gpui::Div>) -> Stateful<gpui::Div>,
    left: impl IntoElement,
    art: impl IntoElement,
) -> gpui::Div {
    let inks = Inks::of(colors);
    let [top, right, bottom, start] = t::PADDING;
    let pane = div()
        .id(id)
        .size_full()
        .flex()
        .child(
            div()
                .flex_none()
                .w(px(t::LEFT))
                .h_full()
                .flex()
                .flex_col()
                // The row the window's buttons sit in moves the window, as a title bar does.
                .child(
                    div()
                        .flex_none()
                        .h(px(t::BUTTONS_ROW))
                        .on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.start_window_move()
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .pt(px(top))
                        .pr(px(right))
                        .pb(px(bottom))
                        .pl(px(start))
                        .child(left),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .py(px(t::PANEL_INSET))
                .pr(px(t::PANEL_INSET))
                .child(
                    div()
                        .id("onboarding-slot")
                        .flex_1()
                        .relative()
                        .overflow_hidden()
                        .rounded(px(t::PANEL_RADIUS))
                        .bg(with_alpha(inks.page.0, inks.page.1))
                        .border_1()
                        .border_color(with_alpha(inks.line.0, inks.line.1))
                        .child(art),
                ),
        );
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(dialog(pane))
}

/// Where the step is in the flow: a track filled as far as `at` of `count`, and "n of m", the
/// welcome counting as the first.
fn indicator(count: usize, at: usize, inks: Inks) -> gpui::Div {
    let count = count.max(1);
    let n = (at + 1).min(count);
    let filled = t::TRACK_WIDTH * n as f32 / count as f32;
    div()
        .flex_none()
        .h(px(t::INDICATOR_HEIGHT))
        .flex()
        .items_center()
        .gap(px(t::INDICATOR_GAP))
        .child(
            div()
                .w(px(t::TRACK_WIDTH))
                .h(px(t::TRACK_HEIGHT))
                .rounded(px(t::TRACK_RADIUS))
                .bg(inks.text(t::TRACK))
                .flex()
                .child(
                    div()
                        .h_full()
                        .w(px(filled))
                        .rounded(px(t::TRACK_RADIUS))
                        .bg(inks.text(1.)),
                ),
        )
        .child(
            div()
                .text_size(px(t::COUNT_SIZE))
                .line_height(px(t::COUNT_LINE))
                .text_color(inks.text(inks.faint))
                .child(format!("{n} of {count}")),
        )
}

/// A quiet button in the foot: its words in the quiet ink, the text's under the pointer.
fn quiet(id: &'static str, label: &'static str, inks: Inks) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(t::BUTTON_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(t::BUTTON_RADIUS))
        .text_size(px(t::BUTTON_SIZE))
        .line_height(px(t::BUTTON_LINE))
        .text_color(inks.text(inks.quiet))
        .hover(move |style| style.text_color(inks.text(1.)))
        .focus_visible(ring(inks.text(1.)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// The step's main button, `button` with its label in it: the text's ink, its label on it, and the
/// key that presses it, `key`, at its end.
pub(super) fn main_button(
    button: Stateful<gpui::Div>,
    key: SharedString,
    inks: Inks,
) -> Stateful<gpui::Div> {
    let chip = div()
        .h(px(t::KEY_HEIGHT))
        .px(px(t::KEY_INSET))
        .flex()
        .items_center()
        .rounded(px(t::KEY_RADIUS))
        .bg(with_alpha(inks.on_primary, t::KEY))
        .text_size(px(t::KEY_SIZE))
        .line_height(px(t::KEY_LINE))
        .font_weight(FontWeight::NORMAL)
        .child(key);
    button
        .flex_none()
        .relative()
        .h(px(t::BUTTON_HEIGHT))
        .pl(px(t::MAIN_START))
        .pr(px(t::MAIN_END))
        .flex()
        .items_center()
        .gap(px(t::MAIN_GAP))
        .rounded(px(t::BUTTON_RADIUS))
        .bg(inks.text(1.))
        .text_color(rgb(inks.on_primary))
        .text_size(px(t::BUTTON_SIZE))
        .line_height(px(t::BUTTON_LINE))
        .font_weight(FontWeight(t::MAIN_WEIGHT))
        .hover(|style| style.opacity(0.9))
        .focus_visible(ring(inks.text(1.)))
        .child(chip)
}

impl Shell {
    /// The column above the step's choices: where the step is in the flow, or at the welcome the
    /// lockup's place, then the step's heading and its line.
    pub(super) fn frame_left(
        &self,
        step: Step,
        (title, line): (SharedString, SharedString),
        colors: Colors,
        cx: &mut App,
    ) -> gpui::Div {
        let inks = Inks::of(colors);
        let (count, at) = self.onboarding.as_ref().map_or((1, 0), |onboarding| {
            let at = onboarding.steps.iter().position(|&each| each == step);
            (onboarding.steps.len(), at.unwrap_or_default())
        });
        let welcome = step == Step::Intro;
        let (top, size, height) = match welcome {
            true => (t::WELCOME_TOP, t::WELCOME_SIZE, t::WELCOME_LINE),
            false => (t::HEADING_TOP, t::HEADING_SIZE, t::HEADING_LINE),
        };
        let heading = display_text(title, size, height, cx)
            .font_weight(FontWeight(t::HEADING_WEIGHT))
            .text_color(inks.text(1.));
        let (line_top, line_size, line_height) = match welcome {
            true => (
                t::WELCOME_LINE_TOP,
                t::WELCOME_LINE_SIZE,
                t::WELCOME_LINE_LINE,
            ),
            false => (t::LINE_TOP, t::LINE_SIZE, t::LINE_LINE),
        };
        let line = (!line.is_empty()).then(|| {
            div()
                .pt(px(line_top))
                .pr(px(t::HEADING_RIGHT))
                .text_size(px(line_size))
                .line_height(px(line_height))
                .text_color(inks.text(inks.quiet))
                .child(line)
        });
        // The lockup's place at the welcome: the mark and the name are P9's, so it stays empty
        // until they come, and the heading sits where the drawing has it.
        let lockup = welcome.then(|| div().flex_none().h(px(t::LOCKUP)));
        let indicator = (!welcome).then(|| indicator(count, at, inks));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .children(indicator)
            .children(lockup)
            .child(div().pt(px(top)).pr(px(t::HEADING_RIGHT)).child(heading))
            .children(line)
            .child(div().flex_none().h(px(t::BODY_TOP)))
    }

    /// The foot: at the welcome Skip Setup, which ends the flow, and its button; at every other
    /// step Back at its start, then Skip and the step's own button. At the import they follow the
    /// state its view shows: Back goes back in the view where it has somewhere to go back to, and
    /// Skip goes while Cancel is the view's own way out.
    pub(super) fn frame_foot(
        &self,
        step: Step,
        button: Option<Stateful<gpui::Div>>,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let inks = Inks::of(colors);
        let stops = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| (onboarding.back_stop.clone(), onboarding.skip_stop.clone()));
        let welcome = step == Step::Intro;
        let corner = match step {
            Step::Import => (self.import_guide(cx)).map_or(Corner::Skip, |guide| guide.corner),
            _ => Corner::Skip,
        };
        let back = (!welcome).then(|| {
            quiet("onboarding-back", "Back", inks)
                .when(self.a11y, |control| named_button(control, "Back"))
                .when_some(stops.clone(), |control, (stop, _)| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_click(cx.listener(move |shell, _, _, cx| match corner {
                    Corner::Back => shell.import_escape("back", cx),
                    _ => shell.onboarding_back("back", cx),
                }))
                .relative()
                .children(self.place(step, "back"))
                .into_any_element()
        });
        let label = if welcome { "Skip Setup" } else { "Skip" };
        // check.sh clicks it where it's logged, once the step has come.
        let frame = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| skip_frame(onboarding.skip.clone(), onboarding.settled));
        let skip = (corner != Corner::None).then(|| {
            quiet("onboarding-skip", label, inks)
                // At the welcome it starts at the column's edge.
                .map(|control| match welcome {
                    true => control.pr(px(t::QUIET_INSET)),
                    false => control.px(px(t::QUIET_INSET)),
                })
                .when(self.a11y, |control| named_button(control, label))
                .when_some(stops, |control, (_, stop)| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_click(cx.listener(move |shell, _, _, cx| match welcome {
                    true => shell.onboarding_skip_setup("skip", cx),
                    false => shell.onboarding_next("skip", cx),
                }))
                .relative()
                .children(self.place(step, "skip"))
                .children(frame)
                .into_any_element()
        });
        let (lead, trail) = match welcome {
            true => (skip, None),
            false => (back, skip),
        };
        div()
            .mt_auto()
            .flex_none()
            .h(px(t::BUTTON_HEIGHT))
            .flex()
            .items_center()
            .gap(px(t::BUTTON_GAP))
            .children(lead)
            .child(div().flex_1())
            .children(trail)
            .children(button)
    }

    /// The panel's loop for `step`, `looped` seconds into it: at the color, the Space in the
    /// color picked, or in none, swiping to a Space in another.
    pub(super) fn frame_art(&self, step: Step, looped: f32, colors: Colors) -> AnyElement {
        let place = |name| (self.place(step, name)).map(IntoElement::into_any_element);
        match step {
            Step::Intro => loops::welcome(looped, colors, &place).into_any_element(),
            Step::Import => loops::import(looped, colors, &place).into_any_element(),
            Step::Favorites => loops::favorites(looped, colors, &place).into_any_element(),
            Step::DefaultBrowser => {
                loops::default_browser(looped, colors, &place).into_any_element()
            }
            Step::SearchEngine => loops::search(looped, colors, &place).into_any_element(),
            Step::Account => loops::sync(looped, colors, &place).into_any_element(),
            Step::Assistant => loops::assistant(looped, colors, &place).into_any_element(),
            Step::OptIns => loops::count(looped, colors, &place).into_any_element(),
            Step::Color => {
                let picked = (self.onboarding.as_ref()).and_then(|onboarding| onboarding.picked);
                let other = (picked.unwrap_or_default() + SWATCHES.len() / 2) % SWATCHES.len();
                let (picked, other) = (picked.map(|index| SWATCHES[index]), SWATCHES[other]);
                loops::colour(looped, picked, other, colors, &place).into_any_element()
            }
            Step::Transparency => div().into_any_element(),
        }
    }
}
