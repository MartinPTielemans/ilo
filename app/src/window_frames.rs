//! Where a window a relaunch brings back opens: at the frame it had, on the screen it was on, and
//! never off screen, whatever displays the Mac has now.

/// A frame in the core's terms: points from the main display's top-left corner, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    pub fn is_empty(&self) -> bool {
        self.width <= 0. || self.height <= 0.
    }

    fn holds(&self, x: f64, y: f64) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }

    fn overlap(&self, other: &Frame) -> f64 {
        let width = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let height = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        width.max(0.) * height.max(0.)
    }
}

/// Where a window saved at `saved` opens on `screens`, their visible frames with the main one
/// first: on the screen it overlaps most, else, its display gone, on the main one, made no bigger
/// than that screen and moved inside it. None for a frame saved empty, for the app's own placing.
/// With no screens, as under Wayland, which shows a client none, the frame as saved, of which the
/// compositor keeps the size.
pub fn place(saved: Frame, screens: &[Frame]) -> Option<Frame> {
    if saved.is_empty() {
        return None;
    }
    if screens.is_empty() {
        return Some(saved);
    }
    let screen = screens
        .iter()
        .map(|screen| (screen, saved.overlap(screen)))
        .filter(|&(_, overlap)| overlap > 0.)
        // The first of equals, so the main screen wins a tie.
        .fold(None, |best: Option<(&Frame, f64)>, each| match best {
            Some(best) if best.1 >= each.1 => Some(best),
            _ => Some(each),
        })
        .map(|(screen, _)| screen)
        .or(screens.first())?;
    let width = saved.width.min(screen.width);
    let height = saved.height.min(screen.height);
    Some(Frame {
        x: saved.x.clamp(screen.x, screen.x + screen.width - width),
        y: saved.y.clamp(screen.y, screen.y + screen.height - height),
        width,
        height,
    })
}

/// Where a Little ilo window opens from `centered`, centred on its screen: a little above the
/// middle, four tenths of the room over it rather than half, on the screen holding its centre
/// (ux-spec.md › Little Arc). Unmoved with no screens, as under Wayland, or when the host didn't
/// centre it, leaving the window manager to place it, as on X11.
pub fn raised(centered: Frame, screens: &[Frame]) -> Frame {
    let (x, y) = (centered.x + centered.width / 2., centered.y + centered.height / 2.);
    let Some(screen) = screens.iter().find(|screen| screen.holds(x, y)) else {
        return centered;
    };
    if (screen.x + screen.width / 2. - x).abs() > 1. {
        return centered;
    }
    let room = (screen.height - centered.height).max(0.);
    Frame { y: screen.y + room * 0.4, ..centered }
}

/// Where a Little ilo window of `size` opens on the screen holding `pointer`, as Arc's opens on the
/// pointer's screen: across its middle, and as `raised` puts it. None off every screen.
pub fn on_screen_of(pointer: (f64, f64), size: (f64, f64), screens: &[Frame]) -> Option<Frame> {
    let screen = screens.iter().find(|screen| screen.holds(pointer.0, pointer.1))?;
    let (width, height) = size;
    let x = screen.x + (screen.width - width) / 2.;
    let y = screen.y + (screen.height - height) / 2.;
    Some(raised(Frame { x, y, width, height }, screens))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame {
        Frame { x, y, width, height }
    }

    // A 1512 by 982 laptop screen under a 25-point menu bar, with a 2560 by 1440 display to its
    // right, its top level with the laptop's.
    const LAPTOP: Frame = Frame { x: 0., y: 25., width: 1512., height: 957. };
    const DISPLAY: Frame = Frame { x: 1512., y: 25., width: 2560., height: 1415. };

    #[test]
    fn a_frame_on_its_screen_stays_where_it_was() {
        let saved = frame(1700., 100., 1400., 900.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(saved));
        let saved = frame(40., 60., 1000., 700.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(saved));
    }

    #[test]
    fn a_frame_whose_display_is_gone_moves_onto_the_main_screen_and_fits_it() {
        let saved = frame(1700., 100., 1400., 1200.);
        assert_eq!(place(saved, &[LAPTOP]), Some(frame(112., 25., 1400., 957.)));
    }

    #[test]
    fn a_frame_bigger_than_its_screen_now_shrinks_to_it() {
        let saved = frame(1600., 30., 3000., 1600.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(DISPLAY));
    }

    #[test]
    fn a_frame_hanging_off_its_screen_comes_back_on_it() {
        // Mostly on the display, partly below it.
        let saved = frame(3000., 1000., 1200., 800.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(frame(2872., 640., 1200., 800.)));
        // Above the top of every screen, as AppKit never puts a window.
        let saved = frame(100., -400., 800., 600.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(frame(100., 25., 800., 600.)));
    }

    #[test]
    fn a_frame_across_two_screens_goes_to_the_one_with_more_of_it() {
        let saved = frame(1312., 100., 1000., 600.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(frame(1512., 100., 1000., 600.)));
        let saved = frame(812., 100., 1000., 600.);
        assert_eq!(place(saved, &[LAPTOP, DISPLAY]), Some(frame(512., 100., 1000., 600.)));
    }

    #[test]
    fn a_frame_saved_empty_is_the_apps_to_place() {
        assert_eq!(place(frame(0., 0., 0., 0.), &[LAPTOP]), None);
        assert_eq!(place(frame(0., 0., 0., 0.), &[]), None);
    }

    /// Little ilo opens on the screen the pointer is on, across its middle and four tenths of the
    /// room down, and off every screen it's left to the screen the windows open on.
    #[test]
    fn little_ilo_opens_on_the_pointers_screen() {
        let on_display = on_screen_of((3000., 700.), (900., 640.), &[LAPTOP, DISPLAY]);
        assert_eq!(on_display, Some(frame(2342., 25. + 775. * 0.4, 900., 640.)));
        let on_laptop = on_screen_of((10., 900.), (900., 640.), &[LAPTOP, DISPLAY]);
        assert_eq!(on_laptop, Some(frame(306., 25. + 317. * 0.4, 900., 640.)));
        assert_eq!(on_screen_of((-50., 700.), (900., 640.), &[LAPTOP, DISPLAY]), None);
        assert_eq!(on_screen_of((10., 900.), (900., 640.), &[]), None);
    }

    #[test]
    fn with_no_screens_a_frame_stays_as_saved() {
        let saved = frame(10., 10., 800., 600.);
        assert_eq!(place(saved, &[]), Some(saved));
    }

    #[test]
    fn a_little_window_opens_a_little_above_the_middle() {
        // 900 by 640, centred on the laptop: 158.5 of room above and below it.
        let centered = frame(306., 183.5, 900., 640.);
        assert_eq!(raised(centered, &[LAPTOP, DISPLAY]), frame(306., 25. + 317. * 0.4, 900., 640.));
        let centered = frame(2342., 412.5, 900., 640.);
        assert_eq!(raised(centered, &[LAPTOP, DISPLAY]).y, 25. + 775. * 0.4);
        assert_eq!(raised(centered, &[]), centered);
        // At the screen's corner, where a host leaves it for the window manager to place.
        let placed = frame(0., 0., 900., 640.);
        assert_eq!(raised(placed, &[LAPTOP, DISPLAY]), placed);
    }
}
