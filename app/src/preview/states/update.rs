//! The update banner's made-up states.

use crate::*;

/// An updater that does nothing, at the stage PREVIEW_UPDATE names.
pub(crate) fn preview_update(update: &str, cx: &mut App) {
    use browser::updates::{self, Stage, Track, Updater};
    struct Made;
    impl updates::Host for Made {
        fn check(&self, _: &mut App) {}
        fn show(&self, _: &mut App) {}
        fn restart(&self, _: &mut App) {}
        fn follow(&self, _: Track, _: &mut App) {}
    }
    updates::start(Updater::Own, std::rc::Rc::new(Made), cx);
    let off = states::var("PREVIEW_UPDATE_CHECKS").is_ok_and(|checks| checks == "off");
    updates::checks(Some(!off), cx);
    let version = Some("0.1.5".into());
    let stage = match update.trim_end_matches("-open") {
        "available" => Stage::Available(version),
        _ => Stage::Ready(version),
    };
    updates::changed(stage, cx);
}
