//! The chrome's face: SF on the Mac, which GPUI's `.SystemUIFont` names there, and Inter on Linux
//! and Windows, whose system faces differ by machine and mostly run wider than SF, so text that
//! fits on the Mac would clip there. Inter (OFL, its license in `assets/fonts`) is built in only
//! there: release 4.1's static instances at the four weights the chrome sets, subset to Latin,
//! Greek, Cyrillic and the symbols the chrome draws (NOTICE). Other scripts, such as Japanese,
//! fall back to the system's faces.

use gpui::App;

/// The family every surface's text is set in.
#[cfg(target_os = "macos")]
pub const UI: &str = ".SystemUIFont";
#[cfg(not(target_os = "macos"))]
pub const UI: &str = "Inter";

/// The monospaced faces, in the order one is taken: the first of them the computer has.
pub const MONO: [&str; 7] = [
    "SF Mono",
    "Menlo",
    "Cascadia Mono",
    "Consolas",
    "Noto Sans Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
];

/// The first of MONO among the computer's `fonts`, or the UI's face if it has none.
pub fn mono(fonts: &[String]) -> &'static str {
    (MONO.iter())
        .find(|name| fonts.iter().any(|font| font == *name))
        .map_or(UI, |name| name)
}

#[cfg(not(target_os = "macos"))]
const INTER: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Inter-Bold.ttf"),
];

/// Registers the face before the first window draws: registering one later makes GPUI shape every
/// line again.
#[cfg(not(target_os = "macos"))]
pub fn register(cx: &mut App) {
    let start = std::time::Instant::now();
    let faces = INTER
        .iter()
        .map(|face| std::borrow::Cow::Borrowed(*face))
        .collect();
    match cx.text_system().add_fonts(faces) {
        // What the face costs the launch.
        Ok(()) => log!(
            "browser: ui face in {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.
        ),
        Err(error) => log!("couldn't register the ui face: {error}"),
    }
}

/// SF is the system's own on the Mac.
#[cfg(target_os = "macos")]
pub fn register(_: &mut App) {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::UI;
    use gpui::{Font, FontWeight, PlatformTextSystem as _, font};

    /// On the Mac a text in medium draws in a face of its own, between the regular and the
    /// semibold: GPUI's match read the system face's medium as heavier than 500 and gave the
    /// regular one (patches/gpui-apple-atlas.patch). A family with no medium, as Menlo, still
    /// gives its regular.
    #[test]
    fn medium_is_a_face_of_its_own_on_the_mac() {
        let fonts = gpui_apple::AppleTextSystem::new();
        let face = |family: &'static str, weight| {
            let font = Font {
                weight,
                ..font(family)
            };
            fonts.font_id(&font).unwrap()
        };
        let medium = face(UI, FontWeight::MEDIUM);
        assert_ne!(medium, face(UI, FontWeight::NORMAL));
        assert_ne!(medium, face(UI, FontWeight::SEMIBOLD));
        assert_eq!(
            face("Menlo", FontWeight::MEDIUM),
            face("Menlo", FontWeight::NORMAL)
        );
    }
}
