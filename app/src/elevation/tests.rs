use super::*;

const LEVELS: [Level; 3] = [Level::Resting, Level::Popover, Level::Modal];

#[test]
fn each_level_floats_higher_than_the_last() {
    let host: Vec<HostShadow> = LEVELS.map(Level::host_shadow).to_vec();
    for pair in host.windows(2) {
        assert!(pair[0].shadow_opacity < pair[1].shadow_opacity);
        assert!(pair[0].shadow_radius < pair[1].shadow_radius);
        assert!(pair[0].shadow_drop < pair[1].shadow_drop);
    }
    let rims: Vec<f32> = LEVELS.iter().map(|level| level.rim().a).collect();
    assert!(rims.windows(2).all(|pair| pair[0] < pair[1]), "{rims:?}");
}

#[test]
fn every_level_has_the_same_outline() {
    let first = Level::Resting.host_shadow();
    for level in LEVELS {
        let shadow = level.host_shadow();
        assert_eq!(shadow.outline_opacity, first.outline_opacity);
        assert_eq!(shadow.outline_radius, first.outline_radius);
        // GPUI draws it as the shadow over the drop shadow, with no offset.
        let drawn = level.shadow();
        assert_eq!(drawn.len(), 2);
        assert_eq!(drawn[1].offset, point(px(0.), px(0.)));
        assert_eq!(drawn[1].blur_radius, px(0.5));
    }
}

#[test]
fn the_host_takes_a_blur_as_half_its_radius() {
    // The popover's blur is 24 and its drop 8, a black at 0x59 of 0xff.
    let popover = Level::Popover.host_shadow();
    assert_eq!(popover.shadow_radius, 12.);
    assert_eq!(popover.shadow_drop, 8.);
    assert!((popover.shadow_opacity - 0x59 as f32 / 255.).abs() < 1e-6);
    // The outline, a blur of 1 at 0xb3.
    assert_eq!(popover.outline_radius, 0.5);
    assert!((popover.outline_opacity - 0xb3 as f32 / 255.).abs() < 1e-6);
}

/// GPUI's shader reads a shadow's blur as the Gaussian's standard deviation, as the host's layers
/// do, so what GPUI draws and what the host draws at one level are the same size.
#[test]
fn gpui_and_the_host_draw_a_level_the_same_size() {
    for level in LEVELS {
        let (drawn, host) = (level.shadow(), level.host_shadow());
        assert_eq!(drawn[0].blur_radius, px(host.shadow_radius as f32));
        assert_eq!(drawn[1].blur_radius, px(host.outline_radius as f32));
        assert_eq!(drawn[0].offset, point(px(0.), px(host.shadow_drop as f32)));
    }
}

#[test]
fn a_higher_level_reaches_further() {
    let reach: Vec<Reach> = LEVELS.map(Level::reach).to_vec();
    for pair in reach.windows(2) {
        assert!(
            pair[0].sides < pair[1].sides && pair[0].below < pair[1].below,
            "{reach:?}"
        );
    }
    // The drop moves it down: further below, less above.
    for reach in reach {
        assert!(
            reach.below > reach.sides && reach.above < reach.sides,
            "{reach:?}"
        );
    }
}

/// A window with a transparent margin draws its own shadow, and a margin the shadow is still
/// visible at its end leaves a straight cut in it (the zoom HUD's, at the bottom, was 3 % dark).
#[test]
fn a_shadow_fades_out_inside_the_margin_of_the_window_that_draws_it() {
    use crate::{
        tokens::zoom_hud,
        zoom_hud::{MARGIN, WINDOW_HEIGHT, WINDOW_WIDTH},
    };

    let popover = Level::Popover.reach();
    // The tooltip's and the link target's windows are the plate and this much on every side.
    let margins = [
        ("tooltip and link target", [WINDOW_MARGIN; 4]),
        ("mini bar", [crate::mini_bar::MARGIN; 4]),
    ];
    // The zoom HUD's window holds its pill at the top right, and the window's width is the widest
    // pill with the margin on both sides.
    assert!(WINDOW_WIDTH >= 2. * MARGIN + 80., "room for \"500%\"");
    let hud = [
        MARGIN,
        MARGIN,
        WINDOW_HEIGHT - MARGIN - zoom_hud::HEIGHT,
        MARGIN,
    ];
    for (name, [top, right, bottom, left]) in margins.into_iter().chain([("zoom HUD", hud)]) {
        assert!(top >= popover.above, "{name}: {top} above, {popover:?}");
        assert!(right >= popover.sides, "{name}: {right} right, {popover:?}");
        assert!(
            bottom >= popover.below,
            "{name}: {bottom} below, {popover:?}"
        );
        assert!(left >= popover.sides, "{name}: {left} left, {popover:?}");
    }
}
