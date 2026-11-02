//! The glyphs the UI draws, built into the app.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// The shell's glyphs, built in: Lucide's outlines (NOTICE) at one stroke weight, and the nav
/// buttons, the Library's cards, the loading ring, the window buttons' glyphs, the close button's
/// bars, the baseball and the planet, which are ours.
pub struct Assets;

// The close button's bars, as Arc's: 11.4 pt between the middles of their round ends, 2 wide,
// crossing in a 14 pt box (Arc 1.170 rec at 2×).
const BARS: &str = "bars/";
use crate::tokens::shell::assets::BAR_LENGTH;
use crate::tokens::shell::assets::BAR_WIDTH;
use crate::tokens::shell::assets::BARS_BOX;

/// The close button's bars, crossing `angle` degrees from level either way, to the nearest
/// degree: 45 for ✕ and 0 for −.
pub(super) fn bars_path(angle: f32) -> SharedString {
    format!("{BARS}{}.svg", angle.round() as i32).into()
}

/// The bars at `angle` as one path, which the glyph's mask draws once where they cross.
fn bars_svg(angle: f32) -> String {
    let (sin, cos) = angle.to_radians().sin_cos();
    let (x, y) = (BAR_LENGTH / 2. * cos, BAR_LENGTH / 2. * sin);
    let mid = BARS_BOX / 2.;
    let (left, right, up, down) = (mid - x, mid + x, mid - y, mid + y);
    let bar = |from: f32, to: f32| format!("M{left:.2} {from:.2} L{right:.2} {to:.2}");
    let d = format!("{} {}", bar(up, down), bar(down, up));
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{BARS_BOX}\" height=\"{BARS_BOX}\" \
         viewBox=\"0 0 {BARS_BOX} {BARS_BOX}\" fill=\"none\" stroke=\"currentColor\" \
         stroke-width=\"{BAR_WIDTH}\" stroke-linecap=\"round\"><path d=\"{d}\"/></svg>"
    )
}

const ICONS: &[(&str, &[u8])] = &[
    ("icons/activity.svg", include_bytes!("../../assets/icons/activity.svg")),
    ("icons/app-window.svg", include_bytes!("../../assets/icons/app-window.svg")),
    ("icons/archive.svg", include_bytes!("../../assets/icons/archive.svg")),
    ("icons/arrow-down.svg", include_bytes!("../../assets/icons/arrow-down.svg")),
    ("icons/arrow-down-to-line.svg", include_bytes!("../../assets/icons/arrow-down-to-line.svg")),
    ("icons/arrow-left.svg", include_bytes!("../../assets/icons/arrow-left.svg")),
    ("icons/arrow-left-right.svg", include_bytes!("../../assets/icons/arrow-left-right.svg")),
    ("icons/arrow-right.svg", include_bytes!("../../assets/icons/arrow-right.svg")),
    ("icons/arrow-up.svg", include_bytes!("../../assets/icons/arrow-up.svg")),
    ("icons/asterisk.svg", include_bytes!("../../assets/icons/asterisk.svg")),
    ("icons/bandage.svg", include_bytes!("../../assets/icons/bandage.svg")),
    ("icons/baseball.svg", include_bytes!("../../assets/icons/baseball.svg")),
    ("icons/bed.svg", include_bytes!("../../assets/icons/bed.svg")),
    ("icons/bell.svg", include_bytes!("../../assets/icons/bell.svg")),
    ("icons/bluetooth.svg", include_bytes!("../../assets/icons/bluetooth.svg")),
    ("icons/book-open.svg", include_bytes!("../../assets/icons/book-open.svg")),
    ("icons/bookmark.svg", include_bytes!("../../assets/icons/bookmark.svg")),
    ("icons/brush-cleaning.svg", include_bytes!("../../assets/icons/brush-cleaning.svg")),
    ("icons/cable.svg", include_bytes!("../../assets/icons/cable.svg")),
    ("icons/calendar-days.svg", include_bytes!("../../assets/icons/calendar-days.svg")),
    ("icons/camera.svg", include_bytes!("../../assets/icons/camera.svg")),
    ("icons/cast.svg", include_bytes!("../../assets/icons/cast.svg")),
    ("icons/check.svg", include_bytes!("../../assets/icons/check.svg")),
    ("icons/chevron-down.svg", include_bytes!("../../assets/icons/chevron-down.svg")),
    ("icons/chevron-up.svg", include_bytes!("../../assets/icons/chevron-up.svg")),
    ("icons/chevrons-up-down.svg", include_bytes!("../../assets/icons/chevrons-up-down.svg")),
    ("icons/clipboard.svg", include_bytes!("../../assets/icons/clipboard.svg")),
    ("icons/circle-alert.svg", include_bytes!("../../assets/icons/circle-alert.svg")),
    ("icons/circle.svg", include_bytes!("../../assets/icons/circle.svg")),
    ("icons/clock.svg", include_bytes!("../../assets/icons/clock.svg")),
    ("icons/cloud.svg", include_bytes!("../../assets/icons/cloud.svg")),
    ("icons/columns-2.svg", include_bytes!("../../assets/icons/columns-2.svg")),
    ("icons/cloudy.svg", include_bytes!("../../assets/icons/cloudy.svg")),
    ("icons/code.svg", include_bytes!("../../assets/icons/code.svg")),
    ("icons/command.svg", include_bytes!("../../assets/icons/command.svg")),
    ("icons/compass.svg", include_bytes!("../../assets/icons/compass.svg")),
    ("icons/cookie.svg", include_bytes!("../../assets/icons/cookie.svg")),
    ("icons/copy.svg", include_bytes!("../../assets/icons/copy.svg")),
    ("icons/corner-down-left.svg", include_bytes!("../../assets/icons/corner-down-left.svg")),
    ("icons/credit-card.svg", include_bytes!("../../assets/icons/credit-card.svg")),
    ("icons/database.svg", include_bytes!("../../assets/icons/database.svg")),
    ("icons/download.svg", include_bytes!("../../assets/icons/download.svg")),
    ("icons/dumbbell.svg", include_bytes!("../../assets/icons/dumbbell.svg")),
    ("icons/egg.svg", include_bytes!("../../assets/icons/egg.svg")),
    ("icons/eighth-note.svg", include_bytes!("../../assets/icons/eighth-note.svg")),
    ("icons/ellipsis.svg", include_bytes!("../../assets/icons/ellipsis.svg")),
    ("icons/external-link.svg", include_bytes!("../../assets/icons/external-link.svg")),
    ("icons/file.svg", include_bytes!("../../assets/icons/file.svg")),
    ("icons/flag.svg", include_bytes!("../../assets/icons/flag.svg")),
    ("icons/flame.svg", include_bytes!("../../assets/icons/flame.svg")),
    ("icons/fold-vertical.svg", include_bytes!("../../assets/icons/fold-vertical.svg")),
    ("icons/folder.svg", include_bytes!("../../assets/icons/folder.svg")),
    ("icons/folder-open.svg", include_bytes!("../../assets/icons/folder-open.svg")),
    ("icons/folder-plus.svg", include_bytes!("../../assets/icons/folder-plus.svg")),
    (
        "icons/gallery-vertical-end.svg",
        include_bytes!("../../assets/icons/gallery-vertical-end.svg"),
    ),
    ("icons/gamepad-2.svg", include_bytes!("../../assets/icons/gamepad-2.svg")),
    ("icons/gift.svg", include_bytes!("../../assets/icons/gift.svg")),
    ("icons/glasses.svg", include_bytes!("../../assets/icons/glasses.svg")),
    ("icons/globe.svg", include_bytes!("../../assets/icons/globe.svg")),
    ("icons/grid-2x2.svg", include_bytes!("../../assets/icons/grid-2x2.svg")),
    ("icons/grip.svg", include_bytes!("../../assets/icons/grip.svg")),
    ("icons/hand.svg", include_bytes!("../../assets/icons/hand.svg")),
    ("icons/heart.svg", include_bytes!("../../assets/icons/heart.svg")),
    ("icons/image.svg", include_bytes!("../../assets/icons/image.svg")),
    ("icons/inbox.svg", include_bytes!("../../assets/icons/inbox.svg")),
    ("icons/info.svg", include_bytes!("../../assets/icons/info.svg")),
    ("icons/key-round.svg", include_bytes!("../../assets/icons/key-round.svg")),
    ("icons/keyboard.svg", include_bytes!("../../assets/icons/keyboard.svg")),
    ("icons/layers.svg", include_bytes!("../../assets/icons/layers.svg")),
    ("icons/leaf.svg", include_bytes!("../../assets/icons/leaf.svg")),
    ("icons/library.svg", include_bytes!("../../assets/icons/library.svg")),
    ("icons/lightbulb.svg", include_bytes!("../../assets/icons/lightbulb.svg")),
    ("icons/link.svg", include_bytes!("../../assets/icons/link.svg")),
    ("icons/loading.svg", include_bytes!("../../assets/icons/loading.svg")),
    ("icons/lock-open.svg", include_bytes!("../../assets/icons/lock-open.svg")),
    ("icons/lock.svg", include_bytes!("../../assets/icons/lock.svg")),
    ("icons/mail.svg", include_bytes!("../../assets/icons/mail.svg")),
    ("icons/map-pin.svg", include_bytes!("../../assets/icons/map-pin.svg")),
    ("icons/map.svg", include_bytes!("../../assets/icons/map.svg")),
    ("icons/maximize.svg", include_bytes!("../../assets/icons/maximize.svg")),
    ("icons/megaphone.svg", include_bytes!("../../assets/icons/megaphone.svg")),
    ("icons/message-circle-more.svg", include_bytes!("../../assets/icons/message-circle-more.svg")),
    ("icons/message-circle.svg", include_bytes!("../../assets/icons/message-circle.svg")),
    ("icons/mic.svg", include_bytes!("../../assets/icons/mic.svg")),
    ("icons/minus.svg", include_bytes!("../../assets/icons/minus.svg")),
    ("icons/moon-star.svg", include_bytes!("../../assets/icons/moon-star.svg")),
    ("icons/moon.svg", include_bytes!("../../assets/icons/moon.svg")),
    ("icons/mouse-pointer-2.svg", include_bytes!("../../assets/icons/mouse-pointer-2.svg")),
    ("icons/nav-back.svg", include_bytes!("../../assets/icons/nav-back.svg")),
    ("icons/nav-forward.svg", include_bytes!("../../assets/icons/nav-forward.svg")),
    ("icons/nav-plus.svg", include_bytes!("../../assets/icons/nav-plus.svg")),
    ("icons/nav-reload.svg", include_bytes!("../../assets/icons/nav-reload.svg")),
    ("icons/nav-sidebar.svg", include_bytes!("../../assets/icons/nav-sidebar.svg")),
    ("icons/nav-stop.svg", include_bytes!("../../assets/icons/nav-stop.svg")),
    ("icons/network.svg", include_bytes!("../../assets/icons/network.svg")),
    ("icons/nfc.svg", include_bytes!("../../assets/icons/nfc.svg")),
    ("icons/paintbrush.svg", include_bytes!("../../assets/icons/paintbrush.svg")),
    ("icons/palette.svg", include_bytes!("../../assets/icons/palette.svg")),
    ("icons/panel-left.svg", include_bytes!("../../assets/icons/panel-left.svg")),
    ("icons/paw-print.svg", include_bytes!("../../assets/icons/paw-print.svg")),
    ("icons/pause.svg", include_bytes!("../../assets/icons/pause.svg")),
    ("icons/pencil.svg", include_bytes!("../../assets/icons/pencil.svg")),
    ("icons/piano.svg", include_bytes!("../../assets/icons/piano.svg")),
    (
        "icons/picture-in-picture-2.svg",
        include_bytes!("../../assets/icons/picture-in-picture-2.svg"),
    ),
    ("icons/pin.svg", include_bytes!("../../assets/icons/pin.svg")),
    ("icons/pizza.svg", include_bytes!("../../assets/icons/pizza.svg")),
    ("icons/plane.svg", include_bytes!("../../assets/icons/plane.svg")),
    ("icons/planet.svg", include_bytes!("../../assets/icons/planet.svg")),
    ("icons/play.svg", include_bytes!("../../assets/icons/play.svg")),
    ("icons/plus.svg", include_bytes!("../../assets/icons/plus.svg")),
    ("icons/printer.svg", include_bytes!("../../assets/icons/printer.svg")),
    ("icons/puzzle.svg", include_bytes!("../../assets/icons/puzzle.svg")),
    ("icons/receipt.svg", include_bytes!("../../assets/icons/receipt.svg")),
    ("icons/rotate-ccw-clock.svg", include_bytes!("../../assets/icons/rotate-ccw-clock.svg")),
    ("icons/rotate-ccw.svg", include_bytes!("../../assets/icons/rotate-ccw.svg")),
    ("icons/rotate-cw.svg", include_bytes!("../../assets/icons/rotate-cw.svg")),
    ("icons/save.svg", include_bytes!("../../assets/icons/save.svg")),
    ("icons/screen-share.svg", include_bytes!("../../assets/icons/screen-share.svg")),
    ("icons/search.svg", include_bytes!("../../assets/icons/search.svg")),
    ("icons/settings.svg", include_bytes!("../../assets/icons/settings.svg")),
    ("icons/shapes.svg", include_bytes!("../../assets/icons/shapes.svg")),
    ("icons/share.svg", include_bytes!("../../assets/icons/share.svg")),
    ("icons/share.svg", include_bytes!("../../assets/icons/share.svg")),
    ("icons/shield.svg", include_bytes!("../../assets/icons/shield.svg")),
    ("icons/shopping-basket.svg", include_bytes!("../../assets/icons/shopping-basket.svg")),
    ("icons/skip-forward.svg", include_bytes!("../../assets/icons/skip-forward.svg")),
    ("icons/skull.svg", include_bytes!("../../assets/icons/skull.svg")),
    ("icons/spaces.svg", include_bytes!("../../assets/icons/spaces.svg")),
    ("icons/sliders-horizontal.svg", include_bytes!("../../assets/icons/sliders-horizontal.svg")),
    ("icons/sparkles.svg", include_bytes!("../../assets/icons/sparkles.svg")),
    ("icons/speaker.svg", include_bytes!("../../assets/icons/speaker.svg")),
    ("icons/square-check.svg", include_bytes!("../../assets/icons/square-check.svg")),
    ("icons/sun.svg", include_bytes!("../../assets/icons/sun.svg")),
    (
        "icons/square-mouse-pointer.svg",
        include_bytes!("../../assets/icons/square-mouse-pointer.svg"),
    ),
    ("icons/square-code.svg", include_bytes!("../../assets/icons/square-code.svg")),
    ("icons/square-terminal.svg", include_bytes!("../../assets/icons/square-terminal.svg")),
    ("icons/square.svg", include_bytes!("../../assets/icons/square.svg")),
    ("icons/star.svg", include_bytes!("../../assets/icons/star.svg")),
    ("icons/terminal.svg", include_bytes!("../../assets/icons/terminal.svg")),
    ("icons/text-search.svg", include_bytes!("../../assets/icons/text-search.svg")),
    ("icons/thumbs-up.svg", include_bytes!("../../assets/icons/thumbs-up.svg")),
    ("icons/train-front.svg", include_bytes!("../../assets/icons/train-front.svg")),
    ("icons/trash.svg", include_bytes!("../../assets/icons/trash.svg")),
    ("icons/triangle.svg", include_bytes!("../../assets/icons/triangle.svg")),
    ("icons/type.svg", include_bytes!("../../assets/icons/type.svg")),
    ("icons/undo-2.svg", include_bytes!("../../assets/icons/undo-2.svg")),
    ("icons/usb.svg", include_bytes!("../../assets/icons/usb.svg")),
    ("icons/user.svg", include_bytes!("../../assets/icons/user.svg")),
    ("icons/users.svg", include_bytes!("../../assets/icons/users.svg")),
    ("icons/utensils.svg", include_bytes!("../../assets/icons/utensils.svg")),
    ("icons/vibrate.svg", include_bytes!("../../assets/icons/vibrate.svg")),
    ("icons/video.svg", include_bytes!("../../assets/icons/video.svg")),
    ("icons/volume-2.svg", include_bytes!("../../assets/icons/volume-2.svg")),
    ("icons/volume-x.svg", include_bytes!("../../assets/icons/volume-x.svg")),
    ("icons/window-close.svg", include_bytes!("../../assets/icons/window-close.svg")),
    ("icons/window-full-screen.svg", include_bytes!("../../assets/icons/window-full-screen.svg")),
    ("icons/window-maximize.svg", include_bytes!("../../assets/icons/window-maximize.svg")),
    ("icons/window-minimize.svg", include_bytes!("../../assets/icons/window-minimize.svg")),
    ("icons/window-restore.svg", include_bytes!("../../assets/icons/window-restore.svg")),
    (
        "icons/window-windows-close.svg",
        include_bytes!("../../assets/icons/window-windows-close.svg"),
    ),
    (
        "icons/window-windows-maximize.svg",
        include_bytes!("../../assets/icons/window-windows-maximize.svg"),
    ),
    (
        "icons/window-windows-minimize.svg",
        include_bytes!("../../assets/icons/window-windows-minimize.svg"),
    ),
    (
        "icons/window-windows-restore.svg",
        include_bytes!("../../assets/icons/window-windows-restore.svg"),
    ),
    ("icons/wrench.svg", include_bytes!("../../assets/icons/wrench.svg")),
    ("icons/x.svg", include_bytes!("../../assets/icons/x.svg")),
    ("icons/zap.svg", include_bytes!("../../assets/icons/zap.svg")),
    ("icons/zoom-in.svg", include_bytes!("../../assets/icons/zoom-in.svg")),
    ("icons/zoom-out.svg", include_bytes!("../../assets/icons/zoom-out.svg")),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let angle = path.strip_prefix(BARS).and_then(|name| name.strip_suffix(".svg"));
        if let Some(angle) = angle.and_then(|angle| angle.parse::<f32>().ok()) {
            return Ok(Some(Cow::Owned(bars_svg(angle).into_bytes())));
        }
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|&(_, icon)| Cow::Borrowed(icon)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|&(name, _)| name.into())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, bars_path};
    use gpui::AssetSource;

    /// The ✕ crosses its bars, the − lays them on one line, and both are the glyph one path draws.
    #[test]
    fn the_close_bars_cross_for_x_and_lie_level_for_minus() {
        let load = |angle: f32| {
            let svg = Assets.load(&bars_path(angle)).unwrap().unwrap();
            String::from_utf8(svg.into_owned()).unwrap()
        };
        assert_eq!(bars_path(44.6), "bars/45.svg");
        let cross = load(45.);
        assert!(cross.contains("d=\"M2.97 2.97 L11.03 11.03 M2.97 11.03"), "{cross}");
        assert_eq!(cross.matches("<path").count(), 1);
        let line = load(0.);
        assert!(line.contains("M1.30 7.00 L12.70 7.00 M1.30 7.00 L12.70 7.00"), "{line}");
    }

    /// The glyphs that keep their own stroke: Arc's measured nav glyphs, the window buttons' and
    /// the loading ring, and the Library's cards, which has one solid.
    fn keeps_its_own_stroke(name: &str) -> bool {
        name.starts_with("nav-")
            || name.starts_with("window-")
            || ["loading.svg", "spaces.svg"].contains(&name)
    }

    /// One family: every other glyph is an outline on Lucide's grid at the one stroke, with round
    /// ends and no fill, so a footer, a menu and the picker don't mix weights. And a glyph's file
    /// that nothing builds in would be missing from the app without a word.
    #[test]
    fn every_icon_is_a_round_outline_at_the_one_stroke() {
        use crate::tokens::shell::assets::{ICON_GRID, ICON_STROKE};
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons");
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            assert!(path.is_file(), "{name}: icons sit flat, one family");
            assert!(Assets.load(&format!("icons/{name}")).unwrap().is_some(), "{name}: not built in");
            if keeps_its_own_stroke(&name) {
                continue;
            }
            let svg = std::fs::read_to_string(&path).unwrap();
            let viewbox = format!("viewBox=\"0 0 {ICON_GRID} {ICON_GRID}\"");
            let stroke = format!("stroke-width=\"{ICON_STROKE}\"");
            for want in [
                viewbox.as_str(),
                stroke.as_str(),
                "stroke-linecap=\"round\"",
                "stroke-linejoin=\"round\"",
                "fill=\"none\"",
            ] {
                assert!(svg.contains(want), "{name}: needs {want}");
            }
            assert_eq!(svg.matches("fill=").count(), 1, "{name}: an outline has no fill");
            checked += 1;
        }
        assert!(checked > 90, "{checked}");
    }

    /// A glyph that isn't built in draws nothing, with no error to say so.
    #[test]
    fn every_icon_the_ui_names_is_built_in() {
        let mut paths = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        let mut named = 0;
        while let Some(path) = paths.pop() {
            if path.is_dir() {
                let entries = std::fs::read_dir(&path).unwrap();
                paths.extend(entries.map(|entry| entry.unwrap().path()));
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (at, _) in text.match_indices("\"icons/") {
                let icon = text[at + 1..].split('"').next().unwrap_or_default();
                // Not this test's own pattern.
                if !icon.ends_with(".svg") {
                    continue;
                }
                assert!(Assets.load(icon).unwrap().is_some(), "{icon}, in {}", path.display());
                named += 1;
            }
        }
        assert!(named > 0);
    }
}
