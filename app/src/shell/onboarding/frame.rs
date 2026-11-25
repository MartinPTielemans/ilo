//! First run's frame (spec-first-run.md): its window holds the step in a column on the left, under
//! the row the window's buttons sit in, and the picture of the browser in a panel on the right.

use gpui::{
    AnyElement, App, Context, FontWeight, MouseButton, Rgba, SharedString, Stateful, div,
    prelude::*, px, rgb,
};

use super::{Colors, Command, Shell, Step, display_text, picture, skip_frame, with_alpha};
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
        .font_weight(FontWeight::SEMIBOLD)
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
            .font_weight(FontWeight::SEMIBOLD)
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
        // As tall as the column, so the foot the step adds last sits at its bottom.
        div()
            .flex_1()
            .min_h_0()
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
            // The last step has nothing to skip: its button is the way in.
            Step::Keys => Corner::None,
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
        // The default browser's step asks, so its way on without is Not Now, while the system can
        // be asked; where it can't, the step's button is Continue.
        let asks = (self.onboarding.as_ref())
            .is_some_and(|onboarding| crate::default_browser::offers(onboarding.os, false));
        let label = match step {
            Step::Intro => "Skip Setup",
            Step::DefaultBrowser if asks => "Not Now",
            _ => "Skip",
        };
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

    /// The picture for `step`: the small browser in the Space's colour, the colour picked at the
    /// color step once one is, its assistant step's thread only with agents on.
    pub(super) fn frame_art(&self, step: Step, colors: Colors, cx: &gpui::App) -> AnyElement {
        let picked = (self.onboarding.as_ref()).and_then(|onboarding| onboarding.picked);
        let space = picked.map_or(picture::SPACE, |index| SWATCHES[index]);
        // Just the browser's sidebar is tabs only, with no thread at work or chip about one.
        let scene = match step == Step::Assistant && self.browser.agents(cx) != Some(true) {
            true => picture::Scene::default(),
            false => picture::Scene::of(step),
        };
        let rest = match scene.rest {
            true => picture::Rest {
                threads: self.threads_on(cx),
                key: keys_shown(Command::NewTab, cx).unwrap_or_default().into(),
            },
            false => picture::Rest::default(),
        };
        picture::picture(scene, rest, space, colors)
    }

    /// Whether ⌘T starts threads here: the agents' and their tasks' switches are on.
    fn threads_on(&self, cx: &gpui::App) -> bool {
        crate::agents::global(cx).is_some_and(|agents| agents.read(cx).tasks_on())
    }

    /// The keys step's rows, a key each with what it does, as the app's keys are now, and its
    /// foot line at the column's foot.
    pub(super) fn keys_step(&self, colors: Colors, cx: &gpui::App) -> gpui::Div {
        let inks = Inks::of(colors);
        let [height, start, end, gap, between] = t::KEYS_ROW;
        let [cap_width, cap_height, cap_padding] = t::KEY_CAP;
        let [foot, foot_size, foot_line] = t::FOOT;
        let new_tab = match self.threads_on(cx) {
            true => "A site, a search, or a thread",
            false => "A site or a search",
        };
        let rows = [
            (Command::NewTab, new_tab),
            (
                Command::ToggleSidebar,
                "Hide the sidebar, and bring it back",
            ),
            (Command::RecentTab, "Back to the tab you had before"),
        ];
        // A command the user left with no key has no row.
        let rows = rows.into_iter().filter_map(|(command, does)| {
            let keys = keys_shown(command, cx)?;
            Some(
                div()
                    .h(px(height))
                    .pl(px(start))
                    .pr(px(end))
                    .flex()
                    .items_center()
                    .gap(px(gap))
                    .rounded(px(t::KEYS_ROW_RADIUS))
                    .bg(inks.text(t::CARD))
                    .border_1()
                    .border_color(inks.text(t::CARD_LINE))
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(cap_width))
                            .px(px(cap_padding))
                            .h(px(cap_height))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(t::KEY_CAP_RADIUS))
                            .bg(inks.text(t::KEY_CAP_FILL))
                            .font_weight(FontWeight::MEDIUM)
                            .child(keys),
                    )
                    .child(div().flex_1().min_w_0().child(does)),
            )
        });
        let again = concat!("Help › Welcome to ", env!("APP_NAME"), " runs this again.");
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .text_size(px(t::BUTTON_SIZE))
            .line_height(px(t::BUTTON_LINE))
            .text_color(inks.text(1.))
            .child(
                div()
                    .pt(px(t::BODY_TOP))
                    .pr(px(t::QUIET_INSET))
                    .flex()
                    .flex_col()
                    .gap(px(between))
                    .children(rows),
            )
            .child(
                div()
                    .mt_auto()
                    .h(px(foot))
                    .flex()
                    .items_center()
                    .text_size(px(foot_size))
                    .line_height(px(foot_line))
                    .text_color(inks.text(inks.faint))
                    .child(again),
            )
    }
}

impl Shell {
    /// The tidy step: the row naming the time over the bar of Settings' times, its stops named
    /// under it, then Settings' Reopen your windows and tabs in its card. A click or a drag on the
    /// bar puts it at the stop nearest; the arrows move it a stop.
    pub(super) fn tidy_step(&self, colors: Colors, cx: &mut Context<Self>) -> gpui::Div {
        let inks = Inks::of(colors);
        let Some(tidy) = (self.onboarding.as_ref()).and_then(|onboarding| onboarding.tidy.clone())
        else {
            return div();
        };
        let [top, right, gap] = t::TIDY_BLOCK;
        let [width, height, track_top, track_height] = t::TIDY_BAR;
        let [label_width, label_height, label_size, label_line] = t::TIDY_LABELS;
        let [shadow_y, shadow_blur, shadow_alpha] = t::TIDY_THUMB_SHADOW;
        let thumb = t::TIDY_THUMB;
        let last = tidy.stops.len().saturating_sub(1).max(1);
        // Each stop's centre along the bar, the thumb's half in from either end.
        let x = move |at: usize| thumb / 2. + (width - thumb) * at as f32 / last as f32;
        let at = x(tidy.at);
        let words = tidy.stops[tidy.at].1;
        let row = div()
            .h(px(t::TIDY_ROW))
            .flex()
            .items_center()
            .justify_between()
            .child("Put unpinned tabs away after")
            .child(div().font_weight(FontWeight::SEMIBOLD).child(words));
        let dots = (0..tidy.stops.len()).map(move |each| {
            let dot = t::TIDY_TICK;
            div()
                .absolute()
                .left(px(x(each) - dot / 2.))
                .top(px((height - dot) / 2.))
                .size(px(dot))
                .rounded_full()
                .bg(inks.text(t::TIDY_DOT))
        });
        // Where the bar is drawn, which a click's place along it is read against.
        let drawn: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>> =
            Default::default();
        let stop_at = move |bounds: Option<gpui::Bounds<gpui::Pixels>>,
                            pointer: gpui::Point<gpui::Pixels>| {
            let along = bounds.map_or(0., |bounds| f32::from(pointer.x - bounds.origin.x));
            let along = ((along - thumb / 2.) / (width - thumb)).clamp(0., 1.);
            (along * last as f32).round() as usize
        };
        let (pressed, moved) = (drawn.clone(), drawn.clone());
        let a11y = self.a11y;
        let count = tidy.stops.len();
        let bar = div()
            .id("onboarding-tidy")
            .relative()
            .w(px(width))
            .h(px(height))
            .tab_index(0)
            .when_some(self.first_stop(Step::Tidy), |bar, stop| {
                bar.track_focus(&stop)
            })
            .focus_from_keys_only()
            .rounded(px(t::TIDY_TRACK_RADIUS))
            .focus_visible(ring(inks.text(1.)))
            .when(a11y, |bar| {
                bar.role(gpui::accesskit::Role::Slider)
                    .aria_label("Put unpinned tabs away after")
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value((count - 1) as f64)
                    .aria_numeric_value_step(1.)
                    .aria_numeric_value(tidy.at as f64)
                    .on_a11y_action(gpui::accesskit::Action::Increment, {
                        let shell = cx.weak_entity();
                        move |_, _, cx| {
                            shell
                                .update(cx, |shell, cx| shell.onboarding_tidy_by(1, cx))
                                .ok();
                        }
                    })
                    .on_a11y_action(gpui::accesskit::Action::Decrement, {
                        let shell = cx.weak_entity();
                        move |_, _, cx| {
                            shell
                                .update(cx, |shell, cx| shell.onboarding_tidy_by(-1, cx))
                                .ok();
                        }
                    })
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.onboarding_tidy(stop_at(pressed.get(), event.position), cx);
                }),
            )
            .on_mouse_move(
                cx.listener(move |shell, event: &gpui::MouseMoveEvent, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        shell.onboarding_tidy(stop_at(moved.get(), event.position), cx);
                    }
                }),
            )
            .child(
                gpui::canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(track_top))
                    .w(px(width))
                    .h(px(track_height))
                    .rounded(px(t::TIDY_TRACK_RADIUS))
                    .bg(inks.text(t::TIDY_TRACK))
                    .border_1()
                    .border_color(inks.text(t::TIDY_TRACK_LINE))
                    .child(
                        div()
                            .h_full()
                            .w(px(at))
                            .rounded(px(t::TIDY_TRACK_RADIUS))
                            .bg(inks.text(t::TIDY_FILLED)),
                    ),
            )
            .children(dots)
            .children(self.place(Step::Tidy, "tidy"))
            .child(
                div()
                    .absolute()
                    .left(px(at - thumb / 2.))
                    .top(px((height - thumb) / 2.))
                    .size(px(thumb))
                    .rounded_full()
                    .bg(rgb(t::TIDY_THUMB_FILL))
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0., 0., 0., shadow_alpha),
                        offset: gpui::point(px(0.), px(shadow_y)),
                        blur_radius: px(shadow_blur),
                        spread_radius: px(0.),
                        inset: false,
                    }]),
            );
        // The first stop's word starts at the bar's start and the last's ends at its end; the
        // others sit centred under their dots, the stop the bar is at in the text's ink.
        let labels = (tidy.stops.iter().enumerate()).map(move |(each, &(_, said))| {
            let left = match each {
                0 => 0.,
                _ if each == last => width - label_width,
                _ => x(each) - label_width / 2.,
            };
            let ink = match each == tidy.at {
                true => inks.text(1.),
                false => inks.text(inks.faint),
            };
            div()
                .absolute()
                .left(px(left))
                .w(px(label_width))
                .flex()
                .map(|label| match each {
                    0 => label.justify_start(),
                    _ if each == last => label.justify_end(),
                    _ => label.justify_center(),
                })
                .text_color(ink)
                .child(said)
        });
        let labels = div()
            .relative()
            .w(px(width))
            .h(px(label_height))
            .text_size(px(label_size))
            .line_height(px(label_line))
            .children(labels);
        div()
            .w_full()
            .flex()
            .flex_col()
            .text_size(px(t::BUTTON_SIZE))
            .line_height(px(t::BUTTON_LINE))
            .text_color(inks.text(1.))
            .child(
                div()
                    .pt(px(top))
                    .pr(px(right))
                    .flex()
                    .flex_col()
                    .gap(px(gap))
                    .child(row)
                    .child(bar)
                    .child(labels),
            )
            .child(
                div()
                    .pt(px(t::TIDY_CARD_TOP))
                    .pr(px(right))
                    .child(self.opt_in_rows(Step::Tidy, colors, cx)),
            )
    }
}

/// The keys that run `command`, as a tooltip's chips spell them, spaced: "⌘ T". None with no key.
pub(super) fn keys_shown(command: Command, cx: &gpui::App) -> Option<String> {
    let chips = crate::shell::user_keys(cx).chips(command);
    (!chips.is_empty()).then(|| chips.join(" "))
}
