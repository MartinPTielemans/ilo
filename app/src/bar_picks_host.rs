//! What the browser keeps of each profile on this computer only, in the profile directory, under
//! each profile's name, which follows a rename, and a deleted profile's goes with it: the command
//! bar's picks (browser::shell::bar_usage), as `Command bar picks`, and Library › Agents' past tasks
//! (browser::agent_history), as `Agent sessions`. Never synced or sent, and the user's alone, so no
//! agent path reads them.

use std::cell::{Cell, RefCell};

use browser::log;

use crate::{bc, name};

/// One file of what's kept per profile.
struct Kept {
    file: &'static str,
    /// Each profile's lines, by its name.
    texts: RefCell<Vec<(String, String)>>,
    read: Cell<bool>,
}

thread_local! {
    static PICKS: Kept = const { Kept::new("Command bar picks") };
    static SESSIONS: Kept = const { Kept::new("Agent sessions") };
    /// The profiles as they were last named, to tell a rename from a new profile.
    static NAMES: RefCell<Vec<(u64, String)>> = const { RefCell::new(Vec::new()) };
}

impl Kept {
    const fn new(file: &'static str) -> Kept {
        Kept {
            file,
            texts: RefCell::new(Vec::new()),
            read: Cell::new(false),
        }
    }

    fn path(&self) -> String {
        format!("{}/{}", crate::profile_dir(), self.file)
    }

    /// Reads the file the first time it's needed: a line each, the profile's name, a tab, then the
    /// line as its owner writes it.
    fn read(&self) {
        if self.read.replace(true) {
            return;
        }
        let text = std::fs::read_to_string(self.path()).unwrap_or_default();
        let mut texts = self.texts.borrow_mut();
        for (profile, line) in text.lines().filter_map(|line| line.split_once('\t')) {
            let at = texts.iter().position(|(each, _)| each == profile);
            let at = at.unwrap_or_else(|| {
                texts.push((profile.to_string(), String::new()));
                texts.len() - 1
            });
            texts[at].1 += &format!("{line}\n");
        }
    }

    fn save(&self) {
        let text: String = self
            .texts
            .borrow()
            .iter()
            .flat_map(|(profile, text)| {
                text.lines().map(move |line| format!("{profile}\t{line}\n"))
            })
            .collect();
        if let Err(error) = std::fs::write(self.path(), text) {
            log!("couldn't save {}: {error}", self.file);
        }
    }

    fn get(&self, profile: u64) -> String {
        self.read();
        let profile = name(bc::bc_profile_name, profile);
        (self.texts.borrow().iter())
            .find(|(each, _)| *each == profile)
            .map(|(_, text)| text.clone())
            .unwrap_or_default()
    }

    fn set(&self, profile: u64, text: &str) {
        self.read();
        let profile = name(bc::bc_profile_name, profile);
        if profile.is_empty() {
            return;
        }
        {
            let mut texts = self.texts.borrow_mut();
            texts.retain(|(each, _)| *each != profile);
            if !text.is_empty() {
                texts.push((profile.to_string(), text.to_string()));
            }
        }
        self.save();
    }

    /// A renamed profile's lines follow it, and a deleted one's go, once the list was known before.
    fn profiles_changed(&self, before: &[(u64, String)], profiles: &[(u64, String)]) {
        self.read();
        let mut changed = false;
        {
            let mut texts = self.texts.borrow_mut();
            for (id, now) in profiles.iter().filter(|(_, now)| !now.is_empty()) {
                if let Some((_, was)) = before.iter().find(|(each, was)| each == id && was != now) {
                    for (profile, _) in texts.iter_mut().filter(|(profile, _)| profile == was) {
                        *profile = now.clone();
                        changed = true;
                    }
                }
            }
            // Only once the list was known before, so a launch's first, partial list forgets nothing.
            if !before.is_empty() && profiles.iter().all(|(_, name)| !name.is_empty()) {
                let count = texts.len();
                texts.retain(|(profile, _)| profiles.iter().any(|(_, name)| name == profile));
                changed |= texts.len() != count;
            }
        }
        if changed {
            self.save();
        }
    }
}

/// `profile`'s command bar picks.
pub fn picks(profile: u64) -> String {
    PICKS.with(|kept| kept.get(profile))
}

/// Keeps `text` as `profile`'s picks.
pub fn set_picks(profile: u64, text: &str) {
    PICKS.with(|kept| kept.set(profile, text));
}

/// `profile`'s past tasks, as Library › Agents lists them.
pub fn sessions(profile: u64) -> String {
    SESSIONS.with(|kept| kept.get(profile))
}

/// Keeps `text` as `profile`'s past tasks.
pub fn set_sessions(profile: u64, text: &str) {
    SESSIONS.with(|kept| kept.set(profile, text));
}

/// The profiles changed: a renamed one's picks and past tasks follow it, and a deleted one's are
/// forgotten.
pub fn profiles_changed(profiles: &[(u64, String)]) {
    let before = NAMES.replace(profiles.to_vec());
    PICKS.with(|kept| kept.profiles_changed(&before, profiles));
    SESSIONS.with(|kept| kept.profiles_changed(&before, profiles));
}
