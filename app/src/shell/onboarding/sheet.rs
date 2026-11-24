//! First run's sheet (#4093): a pane over the browser's own window, its step on the left and the
//! step's loop in the slot on the right.

use gpui::{
    AnyElement, BoxShadow, Context, MouseButton, SharedString, Stateful, div, point, prelude::*,
    px, rgb, rgba,
};

use super::{Colors, Drawn, Shell, Step, loops, with_alpha, wordmark};
use crate::settings_rows::{named_button, ring};
use crate::theme_picker::SWATCHES;
use crate::tokens::palette::INK;
use crate::tokens::shell::onboarding::sheet as t;
pub(super) use crate::tokens::shell::onboarding::sheet::{
    SWATCH, SWATCH_PITCH, TITLE_LINE, TITLE_SIZE,
};

/// The steps the sheet shows, over the window, rather than the whole window's ground.
pub(super) fn on_sheet(step: Step) -> bool {
    matches!(step, Step::Intro | Step::Color)
}

/// The sheet, centred over the window: `left` its step, `art` the slot's loop. `dialog` makes the
/// pane itself the step's dialog, as `id` names it.
pub(super) fn sheet(
    colors: Colors,
    id: (&'static str, usize),
    dialog: impl FnOnce(Stateful<gpui::Div>) -> Stateful<gpui::Div>,
    left: impl IntoElement,
    art: impl IntoElement,
) -> gpui::Div {
    let light = colors.ink == INK;
    let fill = match light {
        true => with_alpha(t::LIGHT_FILL, t::LIGHT_FILL_ALPHA),
        false => with_alpha(t::FILL, t::FILL_ALPHA),
    };
    // A hairline lit from inside, a dark edge outside it where the ground is dark, and the lift.
    let mut shadows = vec![
        BoxShadow {
            color: colors.ink(t::RIM).into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(t::HAIRLINE),
            inset: true,
        },
        BoxShadow {
            color: black(t::SHADOW).into(),
            offset: point(px(0.), px(t::SHADOW_Y)),
            blur_radius: px(t::SHADOW_BLUR),
            spread_radius: px(0.),
            inset: false,
        },
    ];
    if !light {
        shadows.push(BoxShadow {
            color: black(t::EDGE).into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(t::HAIRLINE),
            inset: false,
        });
    }
    let [top, right, bottom, start] = t::PADDING;
    let pane = div()
        .id(id)
        .flex_none()
        .w(px(t::WIDTH))
        .h(px(t::HEIGHT))
        .flex()
        .rounded(px(t::RADIUS))
        .bg(fill)
        .shadow(shadows)
        .child(
            div()
                .flex_none()
                .w(px(t::LEFT))
                .h_full()
                .flex()
                .flex_col()
                .pt(px(top))
                .pr(px(right))
                .pb(px(bottom))
                .pl(px(start))
                .child(left),
        )
        .child(
            div()
                .id("onboarding-slot")
                .flex_1()
                .m(px(t::SLOT_INSET))
                .ml_0()
                .relative()
                .overflow_hidden()
                .rounded(px(t::SLOT_RADIUS))
                .bg(colors.ink(t::SLOT_GROUND))
                .border_1()
                .border_color(colors.ink(t::SLOT_RIM))
                .child(art),
        );
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(dialog(pane))
}

/// A dot for each of the flow's `count` steps, the one `at` drawn long and lit.
fn pager(count: usize, at: usize, colors: Colors) -> gpui::Div {
    div()
        .flex()
        .gap(px(t::PAGER_GAP))
        .mb(px(t::PAGER_UNDER))
        .children((0..count).map(|index| {
            let lit = index == at;
            div()
                .h(px(t::PAGER_DOT))
                .w(px(if lit { t::PAGER_ON } else { t::PAGER_DOT }))
                .rounded_full()
                .bg(colors.ink(if lit { t::PAGER_LIT } else { t::PAGER_OFF }))
        }))
}

/// A quiet button in the sheet's foot: its words in the ink, a step brighter under the pointer.
fn quiet(id: &'static str, label: &'static str, colors: Colors) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(t::BUTTON_HEIGHT))
        .px(px(t::BUTTON_INSET))
        .flex()
        .items_center()
        .rounded(px(super::BUTTON_RADIUS))
        .text_size(px(t::BUTTON_SIZE))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.ink(t::BUTTON_QUIET))
        .hover(move |style| style.text_color(colors.ink(1.)))
        .focus_visible(ring(colors.ink(1.)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// The step's own button, sized for the sheet's foot.
pub(super) fn go(button: Stateful<gpui::Div>) -> Stateful<gpui::Div> {
    button
        .mt_0()
        .h(px(t::BUTTON_HEIGHT))
        .px(px(t::BUTTON_INSET))
        .text_size(px(t::BUTTON_SIZE))
}

/// Black at `alpha`, for the sheet's edge and shadow.
fn black(alpha: f32) -> gpui::Rgba {
    rgba(((alpha * 255.).round() as u32).min(255))
}

impl Shell {
    /// The sheet's left half above its choice: where the step is in the flow, at the welcome the
    /// wordmark drawing in, and the step's words, which at the welcome come once it's drawn.
    pub(super) fn sheet_left(
        &self,
        step: Step,
        t_: f32,
        still: bool,
        (title, line): (gpui::Div, SharedString),
        colors: Colors,
    ) -> gpui::Div {
        let (count, at) = self.onboarding.as_ref().map_or((1, 0), |onboarding| {
            let at = onboarding.steps.iter().position(|&each| each == step);
            (onboarding.steps.len(), at.unwrap_or_default())
        });
        let drawn = Drawn::at(t_, still);
        let welcome = step == Step::Intro;
        let mark = welcome.then(|| {
            div()
                .mb(px(t::MARK_UNDER))
                .child(wordmark(drawn, colors, t::MARK_SCALE))
        });
        let words = div()
            .flex()
            .flex_col()
            .opacity(if welcome { drawn.tagline } else { 1. })
            .child(title.mb(px(t::TITLE_UNDER)).text_color(colors.ink(1.)))
            .child(
                div()
                    .mb(px(t::LINE_UNDER))
                    .text_size(px(t::LINE_SIZE))
                    .line_height(px(t::LINE_LINE))
                    .text_color(colors.ink(t::LINE_QUIET))
                    .child(line),
            );
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(pager(count, at, colors))
            .children(mark)
            .child(words)
    }

    /// The sheet's foot: Back at its start but at the welcome, then Skip, Skip Setup at the
    /// welcome, which ends the flow, and the step's own button at its end.
    pub(super) fn sheet_foot(
        &self,
        step: Step,
        button: Option<Stateful<gpui::Div>>,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let stops = (self.onboarding.as_ref())
            .filter(|onboarding| onboarding.step == step)
            .map(|onboarding| (onboarding.back_stop.clone(), onboarding.skip_stop.clone()));
        let welcome = step == Step::Intro;
        let back = (!welcome).then(|| {
            quiet("onboarding-back", "Back", colors)
                .when(self.a11y, |control| named_button(control, "Back"))
                .when_some(stops.clone(), |control, (stop, _)| {
                    control.track_focus(&stop).focus_from_keys_only()
                })
                .on_click(cx.listener(|shell, _, _, cx| shell.onboarding_back("back", cx)))
                .relative()
                .children(self.place(step, "back"))
                .into_any_element()
        });
        let label = if welcome { "Skip Setup" } else { "Skip" };
        let skip = quiet("onboarding-skip", label, colors)
            .when(self.a11y, |control| named_button(control, label))
            .when_some(stops, |control, (_, stop)| {
                control.track_focus(&stop).focus_from_keys_only()
            })
            .on_click(cx.listener(move |shell, _, _, cx| match welcome {
                true => shell.onboarding_skip_setup("skip", cx),
                false => shell.onboarding_next("skip", cx),
            }))
            .relative()
            .children(self.place(step, "skip"));
        div()
            .mt_auto()
            .flex()
            .items_center()
            .gap(px(t::BUTTON_GAP))
            .children(back)
            .child(div().flex_1())
            .child(skip)
            .children(button)
    }

    /// The slot's loop for `step`, `looped` seconds into it: at the colour, in the colour picked,
    /// else the first, paging to a Space in another.
    pub(super) fn sheet_art(&self, step: Step, looped: f32, colors: Colors) -> AnyElement {
        match step {
            Step::Intro => loops::welcome(looped, colors).into_any_element(),
            Step::Color => {
                let picked = (self.onboarding.as_ref())
                    .and_then(|onboarding| onboarding.picked)
                    .unwrap_or_default();
                let other = (picked + SWATCHES.len() / 2) % SWATCHES.len();
                let (picked, other) = (rgb(SWATCHES[picked]), rgb(SWATCHES[other]));
                loops::colour(looped, picked, other, colors).into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
