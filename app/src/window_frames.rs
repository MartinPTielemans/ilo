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

/// `point`, a window's origin, at the whole point nearest it. The core makes a frame the whole
/// points around it, so a window placed at half a point would open a point bigger than it's meant
/// to be, as the first window did centred on a screen of odd height (#1568).
pub fn whole_point((x, y): (f64, f64)) -> (f64, f64) {
    (x.round(), y.round())
}

/// Where a Little ilo window opens from `centered`, centred on its screen: a little above the
/// middle, four tenths of the room over it rather than half, on the screen holding its centre
/// (ux-spec.md › Little Arc). Unmoved with no screens, as under Wayland, or when the host didn't
/// centre it, leaving the window manager to place it, as on X11.
pub fn raised(centered: Frame, screens: &[Frame]) -> Frame {
    let (x, y) = (
        centered.x + centered.width / 2.,
        centered.y + centered.height / 2.,
    );
    let Some(screen) = screens.iter().find(|screen| screen.holds(x, y)) else {
        return centered;
    };
    if (screen.x + screen.width / 2. - x).abs() > 1. {
        return centered;
    }
    let room = (screen.height - centered.height).max(0.);
    // At whole points, or each window after it would open bigger again, at the last one's size.
    let (x, y) = whole_point((centered.x, screen.y + room * 0.4));
    Frame { x, y, ..centered }
}

/// Where a Little ilo window of `size` opens on the screen holding `pointer`, as Arc's opens on the
/// pointer's screen: across its middle, and as `raised` puts it. None off every screen.
pub fn on_screen_of(pointer: (f64, f64), size: (f64, f64), screens: &[Frame]) -> Option<Frame> {
    let screen = screens
        .iter()
        .find(|screen| screen.holds(pointer.0, pointer.1))?;
    let (width, height) = size;
    let x = screen.x + (screen.width - width) / 2.;
    let y = screen.y + (screen.height - height) / 2.;
    Some(raised(
        Frame {
            x,
            y,
            width,
            height,
        },
        screens,
    ))
}

/// Where a page's pop-up opens from a Little ilo window at `opener`, as a window of its own (#1919):
/// its page gets the `content` size the page asked for, under a Little ilo bar, a side it didn't
/// ask for the opener's, at least 100 as Blink's least. At `asked`, the screen point the page asked
/// for its content's top-left, else centred on the opener and 10 under its top, as Chrome puts
/// one near the window that opened it. Kept on a screen, as `place` keeps one.
pub fn popup(
    opener: Frame,
    content: (f64, f64),
    asked: Option<(f64, f64)>,
    screens: &[Frame],
) -> Frame {
    use crate::tokens::shell::{CARD_INSET, LITTLE_BAR};
    let (bar, inset) = (f64::from(LITTLE_BAR), f64::from(CARD_INSET));
    let side = |asked: f64, opener: f64| match asked > 0. {
        true => asked.max(MINIMUM),
        false => opener,
    };
    let width = side(content.0, opener.width - 2. * inset) + 2. * inset;
    let height = side(content.1, opener.height - bar - inset) + bar + inset;
    let (x, y) = match asked {
        Some((x, y)) => (x - inset, y - bar),
        None => (
            opener.x + (opener.width - width) / 2.,
            opener.y + POPUP_DROP,
        ),
    };
    let (x, y) = whole_point((x, y));
    let frame = Frame {
        x,
        y,
        width,
        height,
    };
    place(frame, screens).unwrap_or(frame)
}

/// The least a page's pop-up is on a side, as Blink makes a window.open() asking for less.
const MINIMUM: f64 = 100.;
/// How far under its opener's top a page's pop-up opens, Chrome's cascade (kWindowTilePixels).
const POPUP_DROP: f64 = 10.;

/// The saved window a browser window opens as, at its frame, which comes back in the state it quit
/// in: with no window it opens from (`first`), the launch's first, the front one, and `saved`, one
/// behind it. None for a window that isn't `regular`: a private or Little ilo window, or one at a
/// frame of its own.
pub fn saved_at(first: bool, regular: bool, saved: Option<usize>) -> Option<usize> {
    match (regular, saved) {
        (false, _) => None,
        (true, Some(index)) => Some(index),
        (true, None) => first.then_some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launch's first window is the front one saved, and those it opens behind it the others,
    /// each by its index, so each comes back maximized if it quit so (#1454). A window opened from
    /// another, a private or Little ilo window, or one at a frame of its own is none of them.
    #[test]
    fn each_window_a_launch_brings_back_is_the_one_saved_at_its_place() {
        assert_eq!(saved_at(true, true, None), Some(0));
        assert_eq!(saved_at(false, true, Some(1)), Some(1));
        assert_eq!(saved_at(false, true, Some(3)), Some(3));
        assert_eq!(saved_at(false, true, None), None);
        assert_eq!(saved_at(true, false, None), None);
        assert_eq!(saved_at(false, false, Some(1)), None);
    }

    fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    // A 1512 by 982 laptop screen under a 25-point menu bar, with a 2560 by 1440 display to its
    // right, its top level with the laptop's.
    const LAPTOP: Frame = Frame {
        x: 0.,
        y: 25.,
        width: 1512.,
        height: 957.,
    };
    const DISPLAY: Frame = Frame {
        x: 1512.,
        y: 25.,
        width: 2560.,
        height: 1415.,
    };

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
        assert_eq!(
            place(saved, &[LAPTOP, DISPLAY]),
            Some(frame(2872., 640., 1200., 800.))
        );
        // Above the top of every screen, as AppKit never puts a window.
        let saved = frame(100., -400., 800., 600.);
        assert_eq!(
            place(saved, &[LAPTOP, DISPLAY]),
            Some(frame(100., 25., 800., 600.))
        );
    }

    #[test]
    fn a_frame_across_two_screens_goes_to_the_one_with_more_of_it() {
        let saved = frame(1312., 100., 1000., 600.);
        assert_eq!(
            place(saved, &[LAPTOP, DISPLAY]),
            Some(frame(1512., 100., 1000., 600.))
        );
        let saved = frame(812., 100., 1000., 600.);
        assert_eq!(
            place(saved, &[LAPTOP, DISPLAY]),
            Some(frame(512., 100., 1000., 600.))
        );
    }

    /// A maximized window's frame as Windows gives it, 8 pixels past each edge of its work area, is
    /// placed filling its screen, not dropped for the app's own place (#1422). On a 150% system,
    /// host_screens' points are pixels over 1.5: a 2880 by 1800 primary and a 3840 by 2160
    /// monitor to its left, each over a 48-pixel taskbar.
    #[test]
    fn a_maximized_frame_past_its_work_area_fills_its_screen() {
        const PRIMARY: Frame = Frame {
            x: 0.,
            y: 0.,
            width: 1920.,
            height: 1168.,
        };
        const LEFT: Frame = Frame {
            x: -2560.,
            y: 0.,
            width: 2560.,
            height: 1408.,
        };
        let maximized = frame(-3848. / 1.5, -8. / 1.5, 3856. / 1.5, 2128. / 1.5);
        assert_eq!(place(maximized, &[PRIMARY, LEFT]), Some(LEFT));
        // Its monitor unplugged since, on the primary.
        assert_eq!(place(maximized, &[PRIMARY]), Some(PRIMARY));
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
        assert_eq!(on_laptop, Some(frame(306., 152., 900., 640.)));
        assert_eq!(
            on_screen_of((-50., 700.), (900., 640.), &[LAPTOP, DISPLAY]),
            None
        );
        assert_eq!(on_screen_of((10., 900.), (900., 640.), &[]), None);
    }

    #[test]
    fn with_no_screens_a_frame_stays_as_saved() {
        let saved = frame(10., 10., 800., 600.);
        assert_eq!(place(saved, &[]), Some(saved));
    }

    #[test]
    fn a_little_window_opens_a_little_above_the_middle() {
        // 900 by 640, centred on the laptop: 158.5 of room above and below it, 126.8 above it
        // raised, to the nearest point.
        let centered = frame(306., 183.5, 900., 640.);
        assert_eq!(
            raised(centered, &[LAPTOP, DISPLAY]),
            frame(306., 152., 900., 640.)
        );
        let centered = frame(2342., 412.5, 900., 640.);
        assert_eq!(raised(centered, &[LAPTOP, DISPLAY]).y, 25. + 775. * 0.4);
        assert_eq!(raised(centered, &[]), centered);
        // At the screen's corner, where a host leaves it for the window manager to place.
        let placed = frame(0., 0., 900., 640.);
        assert_eq!(raised(placed, &[LAPTOP, DISPLAY]), placed);
    }

    /// The hosted Mac's 1920 by 971 visible frame centres a 1280 by 800 window at 116.5 down,
    /// which the core made 116 and 801 tall; Windows' work area over its scale can be any fraction.
    /// The origin goes to the nearest whole point, and one already whole stays (#1568).
    #[test]
    fn a_centred_window_opens_at_whole_points() {
        assert_eq!(whole_point((320., 116.5)), (320., 117.));
        assert_eq!(whole_point((319.6, 85.25)), (320., 85.));
        assert_eq!(whole_point((-1280.5, 0.)), (-1281., 0.));
        assert_eq!(whole_point((0., 0.)), (0., 0.));
    }

    /// A screen 674 tall under a 31-point menu bar, as the hosted Mac's, where a Little ilo window
    /// raised to 44.6 down opened 900 by 641, and an odd 1023 wide, which centres it at 61.5 across.
    #[test]
    fn a_little_window_opens_at_whole_points() {
        const SMALL: Frame = Frame {
            x: 0.,
            y: 31.,
            width: 1023.,
            height: 674.,
        };
        let opened = on_screen_of((500., 300.), (900., 640.), &[SMALL]);
        assert_eq!(opened, Some(frame(62., 45., 900., 640.)));
    }

    /// A sign-in's pop-up from a Little ilo window gets the size its page asked for, its bar
    /// over that, centred on the opener 10 under its top; where the page asked, there; a side it
    /// didn't ask, the opener's; and on a screen, however far off the page asked (#1919).
    #[test]
    fn a_popup_gets_its_pages_size_over_its_opener() {
        use crate::tokens::shell::{CARD_INSET, LITTLE_BAR};
        let (bar, inset) = (f64::from(LITTLE_BAR), f64::from(CARD_INSET));
        let opener = frame(300., 200., 900., 640.);
        let screens = [LAPTOP];
        let asked = popup(opener, (500., 600.), None, &screens);
        assert_eq!(
            (asked.width, asked.height),
            (500. + 2. * inset, 600. + bar + inset)
        );
        assert_eq!(asked.x + asked.width / 2., opener.x + opener.width / 2.);
        assert_eq!(asked.y, opener.y + 10.);

        let placed = popup(opener, (500., 600.), Some((100., 150.)), &screens);
        assert_eq!((placed.x, placed.y), (100. - inset, 150. - bar));

        let unasked = popup(opener, (0., 0.), None, &screens);
        assert_eq!(
            (unasked.width, unasked.height),
            (opener.width, opener.height)
        );
        let tiny = popup(opener, (20., 20.), None, &screens);
        assert_eq!(tiny.width, 100. + 2. * inset);

        let off = popup(opener, (500., 600.), Some((5000., -400.)), &screens);
        assert!(
            off.x + off.width <= LAPTOP.x + LAPTOP.width && off.y >= LAPTOP.y,
            "{off:?}"
        );
    }
}
