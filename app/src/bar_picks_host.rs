//! Where the command bar's picks (browser::shell::bar_usage) and Library › Agents' past tasks
//! (browser::agent_history) are kept: in each profile's own folder, which the core answers, so a
//! deleted profile's go with it and a renamed one's stay with it.

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
    log!(
        "browser: command bar picks moved into {} profiles' folders",
        named.len()
    );
}

/// `profile`'s past tasks, as Library › Agents lists them, or none for a profile the core doesn't
/// know.
pub fn sessions(profile: u64) -> String {
    let path = folder(profile).map(|folder| folder.join(browser::agent_history::FILE));
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// Keeps `text` as `profile`'s past tasks; none leaves no file.
pub fn set_sessions(profile: u64, text: &str) {
    let Some(path) = folder(profile).map(|folder| folder.join(browser::agent_history::FILE)) else {
        return;
    };
    let saved = match text.is_empty() {
        true => std::fs::remove_file(&path).or_else(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        }),
        false => std::fs::write(&path, text),
    };
    if let Err(error) = saved {
        log!("couldn't save Library › Agents' past tasks: {error}");
    }
}
