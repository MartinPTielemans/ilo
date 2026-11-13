//! The zoom HUD: Arc's pill with the percent over the top right of the page for a while after a
//! zoom, its only sign of one (docs/ux-spec.md › Zoom). GPUI can't draw over the page's native
//! view, so as the toast is, it's a window of its own that the host shows over the browser window,
//! as its child, and the mouse goes through it to the page. What it asks of its window goes
//! through `Host`.

use std::{rc::Rc, time::Duration};
use web_time::Instant;

use gpui::{
    App, Bounds, Context, FontWeight, Global, Hsla, Task, TitlebarOptions, WeakEntity, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, rgb, size,
};

use crate::palette::Palette;
use crate::shell::{CARD_INSET, Shell, cubic_bezier, scaled::scaled_from_top};

use crate::elevation::Level;
use crate::tokens::zoom_hud::FILL;
use crate::tokens::zoom_hud::HEIGHT;
pub use crate::tokens::zoom_hud::MARGIN;
use crate::tokens::zoom_hud::PADDING;
use crate::tokens::zoom_hud::RADIUS;
use crate::tokens::zoom_hud::SHADE;
use crate::tokens::zoom_hud::TEXT_SIZE;
pub use crate::tokens::zoom_hud::WINDOW_HEIGHT;
pub use crate::tokens::zoom_hud::WINDOW_WIDTH;
/// Where the window goes, from the top right corner of the focused page's pane in the window.
pub const WINDOW_RIGHT: f32 = CARD_INSET + 8. - MARGIN;
pub const WINDOW_TOP: f32 = CARD_INSET + 8. - MARGIN;
/// Arc's stayed 4.9 s after the last zoom before a tab switch took it, so at least that long.
const SHOWN: Duration = Duration::from_secs(5);
/// Arc's motion: it grows from 0.4 about the middle of its top edge over 0.18 s, easing out, and
/// leaves the way it came over 0.07 s. A change shrinks it to 0.45 over 0.085 s, the old percent
/// going with it, and grows it back with the new one over 0.15 s.
const FROM: f32 = 0.4;
const GROW: f32 = 0.18;
const FADE_IN: f32 = 0.1;
const DIPPED: f32 = 0.45;
const DIP: f32 = 0.085;
const BACK: f32 = 0.15;
const LEAVE: f32 = 0.07;

/// What the HUD asks of its window.
pub trait Host {
    /// Shows the HUD's window over the browser window `shell` draws in, as the HUD draws now.
    fn show(&self, shell: &WeakEntity<Shell>);
    /// Hides it.
    fn hide(&self);
}

/// How big the pill is drawn, from 0.4 to its size (1), and how opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    scale: f32,
    opacity: f32,
}

fn ease_out(t: f32) -> f32 {
    cubic_bezier(0., 0., 0.58, 1., t.clamp(0., 1.))
}

impl Pose {
    /// `seconds` after its window was first on screen.
    fn arriving(seconds: f32) -> Self {
        Pose {
            scale: FROM + (1. - FROM) * ease_out(seconds / GROW),
            opacity: (seconds / FADE_IN).min(1.),
        }
    }

    /// `seconds` after the percent changed, from where the motion was then.
    fn changing(self, seconds: f32) -> Self {
        let opacity = self.opacity + (1. - self.opacity) * (seconds / DIP).min(1.);
        let scale = if seconds < DIP {
            self.scale + (DIPPED - self.scale) * cubic_bezier(0.42, 0., 0.58, 1., seconds / DIP)
        } else {
            DIPPED + (1. - DIPPED) * ease_out((seconds - DIP) / BACK)
        };
        Pose { scale, opacity }
    }

    /// `seconds` after it started leaving, from where the motion was then.
    fn leaving(self, seconds: f32) -> Self {
        let t = cubic_bezier(0.42, 0., 1., 1., (seconds / LEAVE).min(1.));
        Pose {
            scale: self.scale + (FROM - self.scale) * t,
            opacity: self.opacity * (1. - t),
        }
    }

    /// How opaque the percent is: it's gone by the time the pill has shrunk to 0.45.
    fn text_opacity(self) -> f32 {
        ((self.scale - DIPPED) / (1. - DIPPED)).clamp(0., 1.)
    }
}

struct Shown {
    /// The shell of the browser window it shows over, for the window's colour, which it takes.
    shell: WeakEntity<Shell>,
    /// The tab zoomed, which the HUD goes with.
    tab: u64,
    percent: i32,
    /// The percent before the last change, which shrinks away with the pill.
    was: i32,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    /// When the percent last changed, and where the motion was.
    changed: Option<(Instant, Pose)>,
    /// Once it's leaving: when, and where the motion was.
    leaving: Option<(Instant, Pose)>,
    _timer: Task<()>,
}

impl Shown {
    fn pose(&self) -> Pose {
        let mut pose = Pose::arriving(self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32()));
        if let Some((at, from)) = self.changed {
            pose = from.changing(at.elapsed().as_secs_f32());
        }
        match self.leaving {
            Some((at, from)) => from.leaving(at.elapsed().as_secs_f32()),
            None => pose,
        }
    }

    fn moving(&self) -> bool {
        let Some(shown) = self.shown else {
            return true;
        };
        self.leaving.is_some()
            || shown.elapsed().as_secs_f32() < GROW
            || self
                .changed
                .is_some_and(|(at, _)| at.elapsed().as_secs_f32() < DIP + BACK)
    }

    /// The percent the pill shows now: the old one until it has shrunk away.
    fn percent(&self) -> i32 {
        match self.changed {
            Some((at, _)) if at.elapsed().as_secs_f32() < DIP => self.was,
            _ => self.percent,
        }
    }
}

pub struct ZoomHud {
    host: Rc<dyn Host>,
    shown: Option<Shown>,
}

struct ZoomHudWindow(WindowHandle<ZoomHud>);

impl Global for ZoomHudWindow {}

/// Opens the HUD's window, hidden, for `host` to show over every browser window, ahead of time as
/// the find bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<ZoomHud>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The pill and its shadow are drawn.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| cx.new(|_| ZoomHud { host, shown: None }));
    match window {
        Ok(window) => {
            cx.set_global(ZoomHudWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the zoom HUD: {error}");
            None
        }
    }
}

/// Shows `tab`'s zoom, `percent`, over the browser window `shell` draws in, for a while from now.
pub fn show(tab: u64, percent: i32, shell: WeakEntity<Shell>, cx: &mut App) {
    crate::lazy_window::ready::<ZoomHud>(cx);
    let Some(window) = cx.try_global::<ZoomHudWindow>().map(|hud| hud.0) else {
        return;
    };
    window
        .update(cx, |hud, _, cx| {
            let timer = cx.spawn(async move |hud, cx| {
                cx.background_executor().timer(SHOWN).await;
                hud.update(cx, |hud, cx| hud.leave(cx)).ok();
            });
            // One in place over the same window shrinks and grows back with the new percent; any
            // other pops in.
            let shown = hud.shown.take().filter(|shown| {
                shown.leaving.is_none()
                    && shown.shown.is_some()
                    && shown.shell.entity_id() == shell.entity_id()
            });
            let changed = shown.as_ref().map(|shown| (Instant::now(), shown.pose()));
            hud.shown = Some(Shown {
                shell: shell.clone(),
                tab,
                percent,
                was: shown.as_ref().map_or(percent, |shown| shown.percent),
                shown: shown.and_then(|shown| shown.shown),
                changed,
                leaving: None,
                _timer: timer,
            });
            hud.host.show(&shell);
            cx.notify();
        })
        .ok();
}

/// The window shows `tab`, or no page (0): the HUD goes at once if it showed another tab's zoom.
pub fn tab_shown(tab: u64, cx: &mut App) {
    let Some(window) = cx.try_global::<ZoomHudWindow>().map(|hud| hud.0) else {
        return;
    };
    window
        .update(cx, |hud, _, cx| {
            if hud.shown.as_ref().is_some_and(|shown| shown.tab != tab) {
                hud.leave(cx);
            }
        })
        .ok();
}

impl ZoomHud {
    fn leave(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        if shown.leaving.is_some() {
            return;
        }
        shown.leaving = Some((Instant::now(), shown.pose()));
        shown._timer = cx.spawn(async move |hud, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(LEAVE))
                .await;
            hud.update(cx, |hud, cx| {
                hud.shown = None;
                hud.host.hide();
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }
}

impl Render for ZoomHud {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .size_full()
            .flex()
            .justify_end()
            .items_start()
            .p(px(MARGIN))
            .font_family(crate::face::UI);
        let Some(shown) = &mut self.shown else {
            return root;
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start.
        if shown.shown.is_none() && window.is_visible() {
            shown.shown = Some(Instant::now());
        }
        if shown.moving() {
            window.request_animation_frame();
        }
        let pose = shown.pose();
        let shell = shown.shell.upgrade();
        let look = shell.map(|shell| (shell.read(cx).window_fill(), shell.read(cx).palette()));
        let (fill, palette) = look.unwrap_or((rgb(FILL).into(), Palette::default()));
        let bottom = Hsla {
            l: fill.l * SHADE,
            ..fill
        };
        let pill = div()
            .h(px(HEIGHT))
            .px(px(PADDING))
            .flex()
            .items_center()
            .rounded(px(RADIUS))
            .border(px(0.5))
            .border_color(Level::Popover.rim())
            .bg(linear_gradient(
                180.,
                linear_color_stop(fill, 0.),
                linear_color_stop(bottom, 1.),
            ))
            .shadow(Level::Popover.shadow())
            .opacity(pose.opacity)
            .text_size(px(TEXT_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink)
            .child(
                div()
                    .opacity(pose.text_opacity())
                    .child(format!("{}%", shown.percent())),
            );
        root.child(scaled_from_top(pill, pose.scale))
    }
}

#[cfg(test)]
mod tests;
