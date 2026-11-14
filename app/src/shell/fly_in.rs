//! A new download's icon flying from where it was clicked into the footer's Library button, and the
//! bin it rests in there for a while, as Arc's (docs/ux-spec.md › Downloads).

use std::{collections::HashSet, time::Duration};
use web_time::Instant;

use gpui::{AnyElement, App, Context, Task, Window, div, img, prelude::*, px, svg};

use super::{
    Download, DownloadState, FOOTER_HEIGHT, NAV_SIZE, SECONDARY, Shell, library::LIBRARY_BACK_INSET,
};
use crate::tokens::shell::library::{BIN, BIN_RIGHT, BIN_UP};

/// Arc's: 1.10 s from where it leaves to the bin, at a steady speed across, on a parabola whose top
/// is 90 pt over where it left.
const FLIGHT: f32 = 1.1;
const FLIGHT_APEX: f32 = 90.;
/// Arc's: 32 pt as it leaves and 48 at its top, then shrinking evenly to 10 as it lands. It comes
/// in from nothing by its top.
const FLIGHT_FROM: f32 = 32.;
const FLIGHT_TOP: f32 = 48.;
const FLIGHT_TO: f32 = 10.;
/// Arc's: the bin shows as the icon leaves, 6 pt over its place, drops to 12 under it by 0.15 s,
/// comes back to 2 over it and rests by 0.4 s, as (seconds, pt down).
const BIN_BOUNCE: [(f32, f32); 4] = [(0., -6.), (0.15, 12.), (0.28, -2.), (0.4, 0.)];
/// Arc's: the bin leaves 30 s after the icon lands, fading over 0.15 s.
const BIN_STAY: f32 = 30.;
const BIN_FADE: f32 = 0.15;

/// A download's icon on its way to the Library button, then the bin it rests in.
pub(super) struct Flight {
    pub(super) id: u64,
    /// Where it leaves from, in the window: where the pointer was as the download started.
    from: (f32, f32),
    /// When its first frame was drawn, on the executor's clock.
    start: Option<Instant>,
    /// It has landed, and its line is logged.
    landed: bool,
    /// When the bin starts to leave, if sooner than its stay.
    leave: Option<Instant>,
    /// Wakes the window as the bin's stay ends.
    wake: Option<Task<()>>,
}

/// Where the flying icon is, in the window.
#[derive(Debug, PartialEq)]
struct Place {
    /// Its centre.
    x: f32,
    y: f32,
    size: f32,
    opacity: f32,
}

impl Shell {
    /// Notes the report's downloads, and starts a flight for the newest that's new, if the sidebar
    /// is there to show it.
    pub(super) fn fly_new_download(&mut self, downloads: &[Download], cx: &mut Context<Self>) {
        let first = !std::mem::replace(&mut self.downloads_read, true);
        let new = new_download(&self.downloads_seen, first, downloads);
        self.downloads_seen
            .extend(downloads.iter().map(|each| each.id));
        let Some(id) = new.filter(|_| self.flight_shown()) else {
            return;
        };
        // A later download takes the place of one still flying, and of its bin.
        let from = self.flight_launch(cx);
        self.flight = Some(Flight {
            id,
            from,
            start: None,
            landed: false,
            leave: None,
            wake: None,
        });
        if !self.download_icons.contains_key(&id) {
            self.download_icons.insert(id, None);
            self.browser.download_icon(id, cx);
        }
    }

    /// Whether a flight and its bin show: the sidebar is docked, with the Library closed.
    fn flight_shown(&self) -> bool {
        !self.sidebar_hidden && self.library.is_none() && !self.page_fullscreen
    }

    /// Where a flight leaves from: the pointer, kept inside the window so the icon shows whole, as
    /// Arc's leaves from the window's edge with the pointer outside; or the page's middle where the
    /// host can't say where the pointer is.
    fn flight_launch(&self, cx: &mut App) -> (f32, f32) {
        let (width, height) = (
            f32::from(self.viewport.width),
            f32::from(self.viewport.height),
        );
        let edge = FLIGHT_FROM / 2.;
        let inside = |at: f32, length: f32| at.clamp(edge, (length - edge).max(edge));
        match self.browser.pointer(cx) {
            Some(at) => (
                inside(f32::from(at.x), width),
                inside(f32::from(at.y), height),
            ),
            None => {
                let middle = self.page_frame(self.viewport).0.center();
                (f32::from(middle.x), f32::from(middle.y))
            }
        }
    }

    /// The bin's centre in the window, right of and above the Library button's glyph.
    fn bin_center(&self) -> (f32, f32) {
        let height = f32::from(self.viewport.height);
        (
            self.column_x() + LIBRARY_BACK_INSET + NAV_SIZE / 2. + BIN_RIGHT,
            height - FOOTER_HEIGHT / 2. - BIN_UP,
        )
    }

    /// Whether the icon is on its way, which the window over the page draws.
    pub(super) fn download_flying(&self, cx: &App) -> bool {
        let start = self.flight.as_ref().and_then(|flight| flight.start);
        start.is_some_and(|start| (cx.background_executor().now() - start).as_secs_f32() < FLIGHT)
    }

    /// The bin leaves now, as the recent downloads open where it rests.
    pub(super) fn bin_leaves(&mut self, cx: &App) {
        let now = cx.background_executor().now();
        if let Some(flight) = &mut self.flight {
            flight.leave = Some(flight.leave.map_or(now, |leave| leave.min(now)));
        }
    }

    /// Steps the flight and the bin for a frame: says when the icon leaves and lands, wakes the
    /// window as the bin's stay ends, and lets it go once it has left. Under Reduce Motion the
    /// icon lands as it leaves, and the bin shows still.
    pub(super) fn step_flight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.flight_shown() {
            // The Library opening, the sidebar hiding and a full-screen page take it away.
            self.flight = None;
            return;
        }
        let Some(flight) = &mut self.flight else {
            return;
        };
        let now = cx.background_executor().now();
        // It lands as it leaves too where the window over the page would cover the page with it.
        let still = crate::motion::still(cx) || !self.browser.over_page_blends();
        let start = *flight.start.get_or_insert_with(|| {
            // check.sh waits for this line, logged with the frame that draws the icon first.
            log!("browser: download flying {}", flight.id);
            // Exactly one flight ago, not motion::start's long ago, as the bin's stay counts from it.
            let still = Duration::from_secs_f32(if still { FLIGHT } else { 0. });
            now.checked_sub(still).unwrap_or(now)
        });
        if (now - start).as_secs_f32() < FLIGHT {
            window.request_animation_frame();
            return;
        }
        let stayed = start + Duration::from_secs_f32(FLIGHT + BIN_STAY);
        if !std::mem::replace(&mut flight.landed, true) {
            // check.sh waits for this line, logged with the first frame without the icon.
            log!("browser: download landed {}", flight.id);
            let stay = stayed.saturating_duration_since(now);
            flight.wake = Some(cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(stay).await;
                shell.update(cx, |_, cx| cx.notify()).ok();
            }));
        }
        let leave = flight.leave.unwrap_or(stayed);
        if now < leave {
            return;
        }
        if (now - leave).as_secs_f32() < BIN_FADE {
            window.request_animation_frame();
        } else {
            self.flight = None;
        }
    }

    /// The icon on its way, drawn in the window over the page, so it crosses the page and the
    /// sidebar alike.
    pub(super) fn download_flight_drawn(
        &self,
        window: &mut Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let flight = self.flight.as_ref()?;
        let t = (cx.background_executor().now() - flight.start?).as_secs_f32();
        let place = flight_place(t, flight.from, self.bin_center())?;
        window.request_animation_frame();
        Some(
            div()
                .absolute()
                .left(px(place.x - place.size / 2.))
                .top(px(place.y - place.size / 2.))
                .size(px(place.size))
                .opacity(place.opacity)
                .child(self.file_icon(flight.id))
                .into_any_element(),
        )
    }

    /// The bin on the footer's Library button, holding the file's icon. It takes no pointer, so
    /// the button under it still shows its name and opens the Library.
    pub(super) fn download_bin(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let flight = self.flight.as_ref()?;
        let now = cx.background_executor().now();
        let start = flight.start?;
        let leave = flight
            .leave
            .unwrap_or(start + Duration::from_secs_f32(FLIGHT + BIN_STAY));
        let opacity = 1. - (now.saturating_duration_since(leave).as_secs_f32() / BIN_FADE).min(1.);
        let down = crate::motion::offset(bin_drop((now - start).as_secs_f32()), cx);
        let (x, y) = self.bin_center();
        Some(
            div()
                .absolute()
                .left(px(x - BIN / 2.))
                .top(px(y + down - BIN / 2.))
                .size(px(BIN))
                .opacity(opacity)
                .child(self.file_icon(flight.id)),
        )
    }

    /// A download's icon, or the generic file glyph, as the Library's rows show, until the core
    /// sends it.
    fn file_icon(&self, id: u64) -> AnyElement {
        match self.download_icons.get(&id).cloned().flatten() {
            Some(icon) => img(icon).size_full().into_any_element(),
            None => svg()
                .path("icons/file.svg")
                .size_full()
                .text_color(self.palette.text(SECONDARY))
                .into_any_element(),
        }
    }
}

/// The download a report starts a flight for: the newest one running that no report listed before.
/// None for the first report, which lists the downloads kept from the last launch.
fn new_download(seen: &HashSet<u64>, first: bool, downloads: &[Download]) -> Option<u64> {
    if first {
        return None;
    }
    let new = |each: &&Download| each.state == DownloadState::Running && !seen.contains(&each.id);
    downloads.iter().find(new).map(|each| each.id)
}

/// Where the icon is `t` seconds after leaving `from` for the bin at `to`: across at a steady
/// speed, up to its top 90 pt over `from` and down into the bin, growing up to its top and
/// shrinking after. None once it's landed.
fn flight_place(t: f32, from: (f32, f32), to: (f32, f32)) -> Option<Place> {
    if t >= FLIGHT {
        return None;
    }
    let s = (t / FLIGHT).max(0.);
    // y = from + a·s² − b·s, least at s = b / 2a, where it's the apex over `from`, and at `to` at
    // s = 1. A bin higher than the apex can't be reached on the way down, so the rest of the way
    // up is added evenly.
    let rise = (to.1 - from.1).max(-FLIGHT_APEX);
    let root = (4. * FLIGHT_APEX).sqrt();
    let u = (root + (root * root + 4. * rise).sqrt()) / 2.;
    let (a, b) = (u * u, root * u);
    let top = (root / (2. * u)).min(1.);
    let size = match s <= top {
        true => FLIGHT_FROM + (FLIGHT_TOP - FLIGHT_FROM) * s / top,
        false => FLIGHT_TOP + (FLIGHT_TO - FLIGHT_TOP) * (s - top) / (1. - top),
    };
    Some(Place {
        x: from.0 + (to.0 - from.0) * s,
        y: from.1 + a * s * s - b * s + (to.1 - from.1 - rise) * s,
        size,
        opacity: (s / top).min(1.),
    })
}

/// How far under its place the bin is `t` seconds after it shows, easing between the bounce's
/// turns.
fn bin_drop(t: f32) -> f32 {
    let mut last = BIN_BOUNCE[0];
    for next in &BIN_BOUNCE[1..] {
        if t < next.0 {
            let p = ((t - last.0) / (next.0 - last.0)).clamp(0., 1.);
            return last.1 + (next.1 - last.1) * p * p * (3. - 2. * p);
        }
        last = *next;
    }
    last.1
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, rc::Rc, time::Duration};

    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use super::{
        BIN_BOUNCE, BIN_FADE, BIN_STAY, FLIGHT, FLIGHT_APEX, Place, bin_drop, flight_place,
        new_download,
    };
    use crate::shell::{Download, DownloadState, Shell, recorder::Recorder};

    /// The icon's place at each hundredth of a second of its flight.
    fn path(from: (f32, f32), to: (f32, f32)) -> Vec<Place> {
        (0..110)
            .map(|step| flight_place(step as f32 / 100., from, to).unwrap())
            .collect()
    }

    fn download(id: u64, state: DownloadState) -> Download {
        Download {
            id,
            name: "file.zip".into(),
            url: "https://example.com/file.zip".into(),
            path: "/Users/someone/Downloads/file.zip".into(),
            received: 0,
            total: 0,
            state,
            open_when_done: false,
            held: false,
            started: 0.,
            profile: 0,
            private: false,
        }
    }

    /// A download flies when it first shows up running, and not when it's one the last launch
    /// kept, one already listed that changes state, or one that's already finished.
    #[test]
    fn only_a_new_running_download_flies() {
        use DownloadState::{Done, Paused, Running};
        let none = HashSet::new();
        assert_eq!(new_download(&none, true, &[download(1, Running)]), None);
        assert_eq!(new_download(&none, false, &[download(1, Running)]), Some(1));
        let seen = HashSet::from([1, 2]);
        let report = [download(2, Running), download(1, Done)];
        assert_eq!(new_download(&seen, false, &report), None);
        assert_eq!(
            new_download(&seen, false, &[download(3, Done), download(2, Paused)]),
            None
        );
        // Several new at once fly once, as the newest, which comes first.
        let report = [
            download(5, Running),
            download(4, Running),
            download(2, Running),
        ];
        assert_eq!(new_download(&seen, false, &report), Some(5));
    }

    /// The icon leaves the pointer unseen at 32 pt, crosses at a steady speed, tops out 90 pt over
    /// where it left at 48 pt and fully shown, and lands in the bin at 10 pt after 1.10 s.
    #[test]
    fn the_icon_arcs_from_the_pointer_into_the_bin() {
        let (from, to) = ((640., 250.), (33., 763.));
        let start = flight_place(0., from, to).unwrap();
        assert_eq!(
            (start.x, start.y, start.size, start.opacity),
            (640., 250., 32., 0.)
        );
        let places = path(from, to);
        let apex = places.iter().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
        assert!((apex.y - (250. - FLIGHT_APEX)).abs() < 0.5, "{apex:?}");
        assert!(
            (apex.size - 48.).abs() < 1.5 && apex.opacity > 0.95,
            "{apex:?}"
        );
        for pair in places.windows(2) {
            assert!(
                (pair[0].x - pair[1].x - (640. - 33.) / 110.).abs() < 0.01,
                "steady across"
            );
        }
        let landing = flight_place(FLIGHT - 1e-4, from, to).unwrap();
        assert!(
            (landing.x - 33.).abs() < 0.5 && (landing.y - 763.).abs() < 0.5,
            "{landing:?}"
        );
        assert!((landing.size - 10.).abs() < 0.1 && landing.opacity == 1.);
        assert_eq!(flight_place(FLIGHT, from, to), None);
    }

    /// From a pointer under the bin, as at the window's foot, it still tops out 90 pt over the
    /// pointer and lands in the bin.
    #[test]
    fn the_icon_lands_from_under_the_bin_too() {
        let (from, to) = ((300., 790.), (33., 763.));
        let apex = path(from, to)
            .iter()
            .map(|place| place.y)
            .fold(f32::MAX, f32::min);
        assert!((apex - 700.).abs() < 0.5, "{apex}");
        let landing = flight_place(FLIGHT - 1e-4, from, to).unwrap();
        assert!(
            (landing.x - 33.).abs() < 0.5 && (landing.y - 763.).abs() < 0.5,
            "{landing:?}"
        );
    }

    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            // The first report lists what the last launch kept, which never flies.
            shell.set_downloads(Vec::new(), cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    /// Moves the test's clock on by `seconds` and draws a frame, which steps the flight.
    fn after(seconds: f32, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        cx.executor()
            .advance_clock(Duration::from_secs_f32(seconds));
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    /// Whether the icon flies, and whether the bin shows.
    fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, bool) {
        cx.update(|_, cx| {
            let shell = shell.read(cx);
            (shell.download_flying(cx), shell.download_bin(cx).is_some())
        })
    }

    /// A new download's icon leaves from the pointer, kept inside the window when it's past the
    /// window's edge, flies for 1.10 s with the bin showing from the start, and the bin stays 30 s
    /// after it lands, then fades out.
    #[gpui::test]
    fn a_download_flies_from_the_pointer_and_rests_in_the_bin(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        host.pointer.set(Some(point(px(5000.), px(300.))));
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![download(7, DownloadState::Running)], cx)
        });
        after(0., &shell, cx);
        let (from, width) = shell.read_with(cx, |shell, _| {
            (
                shell.flight.as_ref().unwrap().from,
                f32::from(shell.viewport.width),
            )
        });
        assert_eq!(from, (width - 16., 300.));
        assert_eq!(shown(&shell, cx), (true, true));
        after(FLIGHT, &shell, cx);
        assert_eq!(shown(&shell, cx), (false, true), "landed in the bin");
        after(BIN_STAY - 0.01, &shell, cx);
        assert_eq!(shown(&shell, cx), (false, true));
        after(0.02 + BIN_FADE, &shell, cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.flight.is_none()),
            "the bin has left"
        );
    }

    /// With no pointer to leave from, the icon leaves from the page's middle. The recent downloads
    /// opening send the bin away, and the Library opening takes it at once.
    #[gpui::test]
    fn the_bin_leaves_as_the_list_or_the_library_opens(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![download(7, DownloadState::Running)], cx)
        });
        after(0., &shell, cx);
        let (from, middle) = shell.read_with(cx, |shell, _| {
            let middle = shell.page_frame(shell.viewport).0.center();
            (
                shell.flight.as_ref().unwrap().from,
                (f32::from(middle.x), f32::from(middle.y)),
            )
        });
        assert_eq!(from, middle);
        after(FLIGHT + 1., &shell, cx);
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        cx.simulate_mouse_move(point(px(24.), px(height - 25.)), None, Modifiers::none());
        after(0.3, &shell, cx);
        assert_eq!(shown(&shell, cx), (false, true), "fading as the list opens");
        after(BIN_FADE, &shell, cx);
        assert!(shell.read_with(cx, |shell, _| shell.flight.is_none()));

        cx.simulate_mouse_move(point(px(400.), px(300.)), None, Modifiers::none());
        let report = vec![
            download(8, DownloadState::Running),
            download(7, DownloadState::Done),
        ];
        shell.update(cx, |shell, cx| shell.set_downloads(report, cx));
        after(FLIGHT + 1., &shell, cx);
        assert_eq!(shown(&shell, cx), (false, true));
        cx.simulate_click(point(px(24.), px(height - 25.)), Modifiers::none());
        after(0., &shell, cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.flight.is_none()),
            "the Library takes it"
        );
    }

    /// Under Reduce Motion, and where the window over the page would cover the page, the icon
    /// lands as it leaves, with nothing drawn over the page, and the bin shows.
    #[gpui::test]
    fn the_icon_lands_at_once_without_motion_or_blending(cx: &mut TestAppContext) {
        for reduced in [true, false] {
            let (shell, cx, host) = window(cx);
            cx.update(|_, cx| cx.set_reduce_motion(reduced));
            host.over_page_opaque.set(!reduced);
            shell.update(cx, |shell, cx| {
                shell.set_downloads(vec![download(7, DownloadState::Running)], cx)
            });
            after(0., &shell, cx);
            assert_eq!(shown(&shell, cx), (false, true), "reduced {reduced}");
            let drawn =
                cx.update(|window, cx| shell.read(cx).download_flight_drawn(window, cx).is_none());
            assert!(drawn, "nothing over the page, reduced {reduced}");
        }
    }

    /// The bin shows 6 pt high, drops to 12 under its place, comes back to 2 over it and rests.
    #[test]
    fn the_bin_bounces_once_and_rests() {
        for (at, down) in BIN_BOUNCE {
            assert_eq!(bin_drop(at), down);
        }
        assert!(bin_drop(0.075) > -6. && bin_drop(0.075) < 12.);
        assert_eq!(bin_drop(0.5), 0.);
        assert_eq!(bin_drop(30.), 0.);
    }
}
