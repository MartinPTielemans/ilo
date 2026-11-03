//! The profile Little ilo opens each site in (browser::little_sites), kept in the profile directory
//! as `Little ilo profiles`, by the profile's name, which follows a rename. The user's alone, so no
//! agent path reads or sets it.

use std::cell::{Cell, RefCell};

use browser::little_sites::Sites;
use browser::log;

use crate::{bc, ids, name};

thread_local! {
    static SITES: RefCell<Sites> = RefCell::new(Sites::default());
    static READ: Cell<bool> = const { Cell::new(false) };
    /// The profiles as they were last named, to tell a rename from a new profile.
    static NAMES: RefCell<Vec<(u64, String)>> = const { RefCell::new(Vec::new()) };
}

fn path() -> String {
    format!("{}/Little ilo profiles", crate::profile_dir())
}

/// Reads the file the first time it's needed.
fn read() {
    if !READ.replace(true) {
        SITES.set(Sites::parse(&std::fs::read_to_string(path()).unwrap_or_default()));
    }
}

fn save() {
    let text = SITES.with_borrow(Sites::to_text);
    if let Err(error) = std::fs::write(path(), text) {
        log!("couldn't save Little ilo's profiles: {error}");
    }
}

/// Keeps `profile` as the one `url`'s site opens in.
pub fn remember(url: &str, profile: u64) {
    read();
    let profile_name = name(bc::bc_profile_name, profile);
    if !profile_name.is_empty() && SITES.with_borrow_mut(|sites| sites.remember(url, &profile_name)) {
        save();
    }
}

/// The profile picked for `url`'s site, if one was and it's still there.
pub fn profile(url: &str) -> Option<u64> {
    read();
    let kept = SITES.with_borrow(|sites| sites.profile(url).map(str::to_string))?;
    ids(bc::bc_profiles).into_iter().find(|&profile| name(bc::bc_profile_name, profile) == kept)
}

/// The profiles changed: a renamed one's sites follow it, and a deleted one's are forgotten.
pub fn profiles_changed(profiles: &[(u64, String)]) {
    read();
    let before = NAMES.replace(profiles.to_vec());
    let mut changed = false;
    SITES.with_borrow_mut(|sites| {
        for (id, now) in profiles.iter().filter(|(_, now)| !now.is_empty()) {
            if let Some((_, was)) = before.iter().find(|(each, was)| each == id && was != now) {
                changed |= sites.rename(was, now);
            }
        }
        // Only once the list was known before, so a launch's first, partial list forgets nothing.
        if !before.is_empty() && profiles.iter().all(|(_, name)| !name.is_empty()) {
            let names: Vec<&str> = profiles.iter().map(|(_, name)| name.as_str()).collect();
            changed |= sites.keep_profiles(&names);
        }
    });
    if changed {
        save();
    }
}
