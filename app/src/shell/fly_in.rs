//! A new download's icon flying into the footer's Library button, as in Arc and Dia
//! (docs/ux-spec.md › Downloads; ours until Arc's is recorded).

use std::{collections::HashSet, time::Instant};

use gpui::{Context, Window, div, img, prelude::*, px, svg};

use super::{
    Download, DownloadState, FOOTER_HEIGHT, LIST_TOP, NAV_SIZE, SECONDARY, Shell, cubic_bezier,
    library::LIBRARY_BACK_INSET,
};

/// Ours: the flight takes 0.45 s, the last 30 % of it fading out, along a curve that starts slow
/// and arrives at 4× the mean speed, as a magnet pulls.
const FLIGHT: f32 = 0.45;
const FLIGHT_FADE: f32 = 0.3;
const FLIGHT_CURVE: (f32, f32, f32, f32) = (0.55, 0., 0.85, 0.4);
/// The icon's size as it leaves, under the URL field, and as it lands, the button's glyph's.
const FLIGHT_FROM: f32 = 32.;
const FLIGHT_TO: f32 = 16.;

/// A download's icon on its way to the Library button: which download, and when its first frame
/// was drawn.
pub(super) struct Flight {
    pub(super) id: u64,
    start: Option<Instant>,
}

/// Where the flying icon is `t` seconds into its flight, in a sidebar `width` by `height`.
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
        // A later download takes the place of one still flying.
        self.flight = Some(Flight { id, start: None });
        if !self.download_icons.contains_key(&id) {
            self.download_icons.insert(id, None);
            self.browser.download_icon(id, cx);
        }
    }

    /// Whether a flight shows: the sidebar is docked, with the Library closed.
    fn flight_shown(&self) -> bool {
        !self.sidebar_hidden && self.library.is_none() && !self.page_fullscreen
    }

    /// Steps the flight for a frame, and once it's done, says so and lets it go.
    /// Under Reduce Motion the icon lands as it leaves, flying nowhere.
    pub(super) fn step_flight(&mut self, window: &mut Window, cx: &gpui::App) {
        let shown = self.flight_shown();
        let Some(flight) = &mut self.flight else {
            return;
        };
        let start = match flight.start {
            Some(start) => start,
            None if !shown => {
                self.flight = None;
                return;
            }
            None => {
                // check.sh waits for this line, logged with the frame that draws the icon first.
                log!("browser: download flying {}", flight.id);
                *flight.start.insert(crate::motion::start(cx))
            }
        };
        if start.elapsed().as_secs_f32() < FLIGHT {
            window.request_animation_frame();
        } else {
            // check.sh waits for this line, logged with the first frame without the icon.
            log!("browser: download landed {}", flight.id);
            self.flight = None;
        }
    }

    /// The flying icon, over the sidebar's rows, which keep the pointer.
    pub(super) fn flying_icon(&self) -> Option<impl IntoElement + use<>> {
        let flight = self.flight.as_ref()?;
        let t = flight
            .start
            .map_or(0., |start| start.elapsed().as_secs_f32());
        let place = flight_place(t, self.width(), f32::from(self.viewport.height))?;
        // The generic file glyph, as the Library's rows show, until the core sends the icon.
        let icon = match self.download_icons.get(&flight.id).cloned().flatten() {
            Some(icon) => img(icon).size_full().into_any_element(),
            None => svg()
                .path("icons/file.svg")
                .size_full()
                .text_color(self.palette.text(SECONDARY))
                .into_any_element(),
        };
        Some(
            div()
                .absolute()
                .left(px(place.x - place.size / 2.))
                .top(px(place.y - place.size / 2.))
                .size(px(place.size))
                .opacity(place.opacity)
                .child(icon),
        )
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

/// Where the icon is `t` seconds in: from the sidebar's middle just under the URL field, down and
/// left on a slight arc into the Library button's glyph. None once it's landed.
fn flight_place(t: f32, width: f32, height: f32) -> Option<Place> {
    if t >= FLIGHT {
        return None;
    }
    let (x1, y1, x2, y2) = FLIGHT_CURVE;
    let p = cubic_bezier(x1, y1, x2, y2, t / FLIGHT);
    let (from_x, from_y) = (width / 2., LIST_TOP + FLIGHT_FROM / 2.);
    let (to_x, to_y) = (
        LIBRARY_BACK_INSET + NAV_SIZE / 2.,
        height - FOOTER_HEIGHT / 2.,
    );
    // A quadratic curve whose middle point is straight under the start, so the icon drops first
    // and swings left into the button.
    let (mid_x, mid_y) = (from_x, (from_y + to_y) / 2.);
    let along = |from: f32, mid: f32, to: f32| {
        (1. - p) * (1. - p) * from + 2. * (1. - p) * p * mid + p * p * to
    };
    Some(Place {
        x: along(from_x, mid_x, to_x),
        y: along(from_y, mid_y, to_y),
        size: FLIGHT_FROM + (FLIGHT_TO - FLIGHT_FROM) * p,
        opacity: ((1. - t / FLIGHT) / FLIGHT_FADE).min(1.),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{FLIGHT, flight_place, new_download};
    use crate::shell::{Download, DownloadState};

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

    /// The icon leaves under the URL field at full size, keeps inside the sidebar on its way, is
    /// still less than halfway as it's halfway through its time, and lands on the button's glyph at
    /// its size, gone.
    #[test]
    fn the_icon_speeds_up_into_the_library_button() {
        let (width, height) = (240., 800.);
        let start = flight_place(0., width, height).unwrap();
        assert_eq!(
            (start.x, start.y, start.size, start.opacity),
            (120., 106., 32., 1.)
        );
        let half = flight_place(FLIGHT / 2., width, height).unwrap();
        assert!(half.y < (106. + 775.) / 2. && half.y > 106.);
        assert!(half.size > 24. && half.opacity == 1.);
        let mut last_y = 0.;
        for step in 0..45 {
            let place = flight_place(step as f32 / 100., width, height).unwrap();
            assert!(place.x >= 24. - 0.5 && place.x <= width / 2. + 0.5);
            assert!(place.y >= last_y);
            last_y = place.y;
        }
        let landing = flight_place(FLIGHT - 1e-5, width, height).unwrap();
        assert!((landing.x - 24.).abs() < 0.5 && (landing.y - 775.).abs() < 0.5);
        assert!((landing.size - 16.).abs() < 0.5 && landing.opacity < 0.05);
        assert_eq!(flight_place(FLIGHT, width, height), None);
    }
}
