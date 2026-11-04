//! The command bar's picks (browser::shell::bar_usage), kept on this computer only, in the profile
//! directory as `Command bar picks`, under each profile's name, which follows a rename. A deleted
//! profile's go with it. Never synced or sent, and the user's alone, so no agent path reads them.

use std::cell::{Cell, RefCell};

use browser::log;

use crate::{bc, name};

thread_local! {
    /// Each profile's picks, by its name, as `usage::Usage` writes them.
    static PICKS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static READ: Cell<bool> = const { Cell::new(false) };
    /// The profiles as they were last named, to tell a rename from a new profile.
    static NAMES: RefCell<Vec<(u64, String)>> = const { RefCell::new(Vec::new()) };
}

fn path() -> String {
    format!("{}/Command bar picks", crate::profile_dir())
}

/// Reads the file the first time it's needed: a line a pick, the profile's name, a tab, then the
/// pick's line as `usage::Usage` writes it.
fn read() {
    if READ.replace(true) {
        return;
    }
    let text = std::fs::read_to_string(path()).unwrap_or_default();
    PICKS.with_borrow_mut(|picks| {
        for (profile, line) in text.lines().filter_map(|line| line.split_once('\t')) {
            let at = picks.iter().position(|(each, _)| each == profile);
            let at = at.unwrap_or_else(|| {
                picks.push((profile.to_string(), String::new()));
                picks.len() - 1
            });
            picks[at].1 += &format!("{line}\n");
        }
    });
}

fn save() {
    let text: String = PICKS.with_borrow(|picks| {
        let lines = picks.iter().flat_map(|(profile, text)| {
            text.lines().map(move |line| format!("{profile}\t{line}\n"))
        });
        lines.collect()
    });
    if let Err(error) = std::fs::write(path(), text) {
        log!("couldn't save the command bar's picks: {error}");
    }
}

/// `profile`'s picks.
pub fn picks(profile: u64) -> String {
    read();
    let profile = name(bc::bc_profile_name, profile);
    PICKS.with_borrow(|picks| {
        (picks.iter())
            .find(|(each, _)| *each == profile)
            .map(|(_, text)| text.clone())
            .unwrap_or_default()
    })
}

/// Keeps `text` as `profile`'s picks.
pub fn set_picks(profile: u64, text: &str) {
    read();
    let profile = name(bc::bc_profile_name, profile);
    if profile.is_empty() {
        return;
    }
    PICKS.with_borrow_mut(|picks| {
        picks.retain(|(each, _)| *each != profile);
        if !text.is_empty() {
            picks.push((profile.to_string(), text.to_string()));
        }
    });
    save();
}

/// The profiles changed: a renamed one's picks follow it, and a deleted one's are forgotten.
pub fn profiles_changed(profiles: &[(u64, String)]) {
    read();
    let before = NAMES.replace(profiles.to_vec());
    let mut changed = false;
    PICKS.with_borrow_mut(|picks| {
        for (id, now) in profiles.iter().filter(|(_, now)| !now.is_empty()) {
            if let Some((_, was)) = before.iter().find(|(each, was)| each == id && was != now) {
                for (profile, _) in picks.iter_mut().filter(|(profile, _)| profile == was) {
                    *profile = now.clone();
                    changed = true;
                }
            }
        }
        // Only once the list was known before, so a launch's first, partial list forgets nothing.
        if !before.is_empty() && profiles.iter().all(|(_, name)| !name.is_empty()) {
            let count = picks.len();
            picks.retain(|(profile, _)| profiles.iter().any(|(_, name)| name == profile));
            changed |= picks.len() != count;
        }
    });
    if changed {
        save();
    }
}
