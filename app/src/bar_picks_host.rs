//! Where the command bar's picks (browser::shell::bar_usage) are kept: in each profile's own folder,
//! which the core answers, so a deleted profile's go with it and a renamed one's stay with it.

use std::cell::Cell;
use std::path::PathBuf;

use browser::log;

use crate::{bc, name};

thread_local! {
    static MOVED: Cell<bool> = const { Cell::new(false) };
}

/// `profile`'s folder, or None for one the core doesn't know.
pub fn folder(profile: u64) -> Option<PathBuf> {
    let folder = name(bc::bc_profile_dir, profile);
    (!folder.is_empty()).then(|| PathBuf::from(folder.as_str()))
}

/// Once the profiles are all named, moves the picks from the file they all shared in the profile
/// directory into each one's folder, once a launch.
pub fn profiles_changed(profiles: &[(u64, String)]) {
    if MOVED.get() || profiles.is_empty() || profiles.iter().any(|(_, name)| name.is_empty()) {
        return;
    }
    MOVED.set(true);
    let old = PathBuf::from(crate::profile_dir()).join(browser::shell::PICKS_FILE);
    if !old.exists() {
        return;
    }
    let named: Vec<_> = (profiles.iter())
        .filter_map(|(id, name)| Some((name.clone(), folder(*id)?)))
        .collect();
    browser::shell::move_old_picks(&old, &named);
    // check.sh reads this line.
    log!(
        "browser: command bar picks moved into {} profiles' folders",
        named.len()
    );
}
