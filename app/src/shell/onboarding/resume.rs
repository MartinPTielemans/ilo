//! ⌘Q during first run keeps what is set and the next launch resumes at the same step (v9's keys):
//! one number in the profile's folder, the step's place in the design (1 to 7), and nothing else,
//! never a pick or anything typed, as what was set comes back from the settings themselves. It
//! isn't synced, it's in no log line or crash report, and it goes as first run ends. A launch after
//! first run reads only whether it's there.

use gpui::App;

/// The file the step is kept in, in the profile's own folder, as Settings › Appearance keeps the
/// transparency.
const FILE: &str = "first-run-step";

/// The folder it's kept in, where a test says.
struct Kept(String);

impl gpui::Global for Kept {}

/// Keeps it in `dir` for a test, never the app's own folder.
#[cfg(test)]
pub(crate) fn kept_in(dir: String, cx: &mut App) {
    cx.set_global(Kept(dir))
}

/// The file, in the profile's folder; in a test only where it said, so no test writes into a real
/// home.
fn path(cx: &App) -> Option<std::path::PathBuf> {
    let dir = match cx.try_global::<Kept>() {
        Some(kept) => kept.0.clone(),
        None if cfg!(test) => return None,
        None => crate::profile_dir(),
    };
    Some(std::path::Path::new(&dir).join(FILE))
}

/// First run was left under way in the profile at `dir`, so this launch plays it again.
pub fn waiting(dir: &str) -> bool {
    std::path::Path::new(dir).join(FILE).exists()
}

/// The step's place kept, if one is and it's a number.
pub(super) fn read(cx: &App) -> Option<u8> {
    std::fs::read_to_string(path(cx)?).ok()?.trim().parse().ok()
}

/// Keeps `place` as the step to resume at.
pub(super) fn keep(place: u8, cx: &App) {
    if let Some(path) = path(cx) {
        _ = std::fs::write(path, place.to_string());
    }
}

/// First run has ended: nothing to resume.
pub(super) fn clear(cx: &App) {
    if let Some(path) = path(cx) {
        _ = std::fs::remove_file(path);
    }
}
