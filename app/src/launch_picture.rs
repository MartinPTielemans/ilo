//! Whether the launch shows the sidebar's picture in a window of another size than the one it was
//! taken at, and where, on each OS that has a launch window (docs/ux-principles.md: the first
//! frame is the browser the user left). A Dock or a menu bar that grew since the quit clamps the
//! window a few points shorter (#1450).
//!
//! The column is laid out from both ends: the window's buttons, the URL field, the favorites and
//! the list from the window's top, and the foot, the footer with the players and the tasks over
//! it (`list_bottom`), from its bottom, with empty room under the list's last row between them.
//! So the picture's top stays at the window's top and its foot goes to the window's bottom: a
//! shorter window takes the difference out of the room, and a taller one repeats a band of it.
//! The shell says how far that leaves every control where it draws it ([`Taken::shorter`] and
//! [`Taken::taller`]). Nothing in the column moves with the window's width, as the sidebar keeps
//! its own and the page starts after it.
//!
//! What does move with the window's size is the colour behind the column, which runs down the
//! window and along its diagonal, so the picture shows only within [`TOLERANCE`] of its size.

/// How far the window's width and its height may each be from the picture's, as a share of the
/// picture's. The theme's colour at a point of the column is `y` of the way down the window and
/// `(x + y) / (width + height)` along its diagonal, reaching the second colour `SECOND_AT` (0.75)
/// along it, so within 1 % of each it moves by at most 1.3 % of a theme's range: 3.4 levels in 255
/// for a theme from black to white, under a fifth of the quietest fill the sidebar shows to be
/// seen (the header's hover plate, white 0.077), and none without a theme. 3 points of 677 is
/// 0.44 %.
pub const TOLERANCE: f64 = 0.01;

/// The picture as the app took it: the window's frame then, in the host's points, where the
/// foot starts, in points from the window's top, and how much shorter and taller the window may
/// get with the column drawn the same around the room under its rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Taken {
    pub width: f64,
    pub height: f64,
    pub foot_top: f64,
    pub shorter: f64,
    pub taller: f64,
}

/// Whether a window `width` by `height` shows the picture `taken`, with its top at the window's
/// top and its foot `height - taken.height` points lower than it was.
pub fn shows(taken: &Taken, width: f64, height: f64) -> bool {
    let near = |now: f64, then: f64| (now - then).abs() <= TOLERANCE * then;
    let grows = height - taken.height;
    near(width, taken.width)
        && near(height, taken.height)
        && -taken.shorter <= grows
        && grows <= taken.taller
}

/// `taken` as the app keeps it beside the picture.
pub fn line(taken: &Taken) -> String {
    let Taken {
        width,
        height,
        foot_top,
        shorter,
        taller,
    } = taken;
    format!("{width} {height} {foot_top} {shorter} {taller}\n")
}

/// The picture `line` wrote, if it's one.
pub fn read(line: &str) -> Option<Taken> {
    let numbers: Vec<f64> = line
        .split_whitespace()
        .map(|word| word.parse().ok())
        .collect::<Option<_>>()?;
    let &[width, height, foot_top, shorter, taller] = numbers.as_slice() else {
        return None;
    };
    let fits = width > 0. && (0. ..=height).contains(&foot_top) && shorter >= 0. && taller >= 0.;
    fits.then_some(Taken {
        width,
        height,
        foot_top,
        shorter,
        taller,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hosted Mac's `links` run that flaked (#1450): taken at 1024 by 677, the foot 50 high,
    /// with 120 points free under the last row.
    const TAKEN: Taken = Taken {
        width: 1024.,
        height: 677.,
        foot_top: 627.,
        shorter: 120.,
        taller: 120.,
    };

    #[test]
    fn at_its_own_size_the_picture_shows() {
        assert!(shows(&TAKEN, 1024., 677.));
        // However full the list was: nothing moves.
        let full = Taken {
            shorter: 0.,
            taller: 0.,
            ..TAKEN
        };
        assert!(shows(&full, 1024., 677.));
    }

    #[test]
    fn three_points_shorter_it_shows_while_the_room_under_the_rows_gives_them() {
        assert!(shows(&TAKEN, 1024., 674.));
        // Rows reaching under the foot would be cut 3 points higher.
        let full = Taken {
            shorter: 0.,
            ..TAKEN
        };
        assert!(!shows(&full, 1024., 674.));
        let tight = Taken {
            shorter: 2.,
            ..TAKEN
        };
        assert!(!shows(&tight, 1024., 674.));
    }

    #[test]
    fn taller_it_shows_while_the_room_can_grow() {
        assert!(shows(&TAKEN, 1024., 680.));
        // A taller list would show rows the picture doesn't have.
        let full = Taken {
            taller: 0.,
            ..TAKEN
        };
        assert!(!shows(&full, 1024., 680.));
    }

    #[test]
    fn past_the_tolerance_no_picture_shows_however_much_room() {
        // 1 % of 677 is 6.77 points.
        assert!(shows(&TAKEN, 1024., 671.));
        assert!(!shows(&TAKEN, 1024., 670.));
        assert!(shows(&TAKEN, 1024., 683.));
        assert!(!shows(&TAKEN, 1024., 684.));
    }

    #[test]
    fn a_different_width_shows_within_the_tolerance_only() {
        // The sidebar keeps its width and the page starts after it, so nothing in the column
        // moves; the colour behind it does, with the window's diagonal.
        assert!(shows(&TAKEN, 1014., 677.));
        assert!(shows(&TAKEN, 1034., 674.));
        assert!(!shows(&TAKEN, 1000., 677.));
        assert!(!shows(&TAKEN, 1280., 677.));
    }

    #[test]
    fn the_line_kept_beside_the_picture_reads_back() {
        assert_eq!(read(&line(&TAKEN)), Some(TAKEN));
        assert_eq!(read("1024 677 627 120"), None);
        assert_eq!(read("1024 677 700 0 0"), None);
        assert_eq!(read("1024 677 627 -1 0"), None);
        assert_eq!(read("not a picture"), None);
    }
}
