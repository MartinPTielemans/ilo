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
