//! The card's corners follow the window's own where the OS rounds it: outer = inner + inset, as the
//! chrome's own corners are (#361).

use super::{CARD_INSET, CARD_RADIUS, card_radius, framed_radius};
use crate::tokens::shell::{WINDOW_RADIUS_LINUX, WINDOW_RADIUS_MAC, WINDOW_RADIUS_WINDOWS};

#[test]
fn the_card_is_concentric_with_the_window_corner() {
    // The Mac's: the frame is as thick round the curve as along the sides.
    assert_eq!(
        card_radius(WINDOW_RADIUS_MAC) + CARD_INSET,
        WINDOW_RADIUS_MAC
    );
    // Windows 11's curve is narrower than the frame, so the card's corner is square.
    assert_eq!(card_radius(WINDOW_RADIUS_WINDOWS), 0.);
}

#[test]
fn a_square_window_corner_keeps_ilo_own() {
    // Full screen, maximized, Windows 10 and Linux, whose compositors don't say.
    assert_eq!(card_radius(0.), CARD_RADIUS);
    assert_eq!(card_radius(WINDOW_RADIUS_LINUX), CARD_RADIUS);
    // So a Mac window going full screen keeps its card's corners as they were.
    assert_eq!(card_radius(WINDOW_RADIUS_MAC), card_radius(0.));
}

#[test]
fn a_framed_panel_takes_windows_corner_where_windows_rounds_it() {
    #[cfg(target_os = "windows")]
    let corner = match crate::build_info::windows_11() {
        true => WINDOW_RADIUS_WINDOWS,
        false => 20.,
    };
    #[cfg(not(target_os = "windows"))]
    let corner = 20.;
    assert_eq!(framed_radius(20.), corner);
    assert_eq!(framed_radius(6.), if corner == 20. { 6. } else { corner });
}
