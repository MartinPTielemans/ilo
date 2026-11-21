//! Where a profile keeps its saved reads: in its own folder as `FILE`, as Library › Agents keeps
//! its past tasks (agent_history). Kept on this computer only and never synced. A private window
//! has no folder, so it has no saved reads and keeps none. Each read keeps its definition, when
//! it was saved and who asked for it, when it was last read, and what that read came to; never
//! its rows.

use std::path::Path;

use serde_json::{Value, json};

use super::definition::Definition;

/// The file in a profile's folder its saved reads are kept in.
pub const FILE: &str = "Saved reads";

/// The most saved reads a profile keeps.
pub const MOST: usize = 200;

/// What a saved read's last read came to. Paused isn't kept: it's whether its site is on Always
/// ask first at the time it's asked about.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Ready,
    NeedsSignIn,
    /// Why, as the shape check said it.
    Broken(String),
}

impl State {
    pub fn id(&self) -> &'static str {
        match self {
            State::Ready => "ready",
            State::NeedsSignIn => "needs_sign_in",
            State::Broken(_) => "broken",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub read: Definition,
    /// When it was saved, in seconds since 1970.
    pub saved: u64,
    /// The agent that asked for it, such as "Claude Code".
    pub asked_by: String,
    /// When it was last read, and what that came to since when.
    pub last_read: u64,
    pub state: State,
    pub since: u64,
}

impl Saved {
    fn to_json(&self) -> Value {
        let mut value = json!({
            "read": self.read.to_json(),
            "saved": self.saved,
            "asked_by": self.asked_by,
            "last_read": self.last_read,
            "state": self.state.id(),
            "since": self.since,
        });
        if let State::Broken(why) = &self.state {
            value["why"] = json!(why);
        }
        value
    }

    fn from_json(value: &Value) -> Option<Saved> {
        let number = |key: &str| value.get(key).and_then(Value::as_u64);
        let text = |key: &str| value.get(key).and_then(Value::as_str);
        let state = match text("state")? {
            "ready" => State::Ready,
            "needs_sign_in" => State::NeedsSignIn,
            "broken" => State::Broken(super::text(
                text("why").unwrap_or_default(),
                super::VALUE_KEPT,
            )),
            _ => return None,
        };
        Some(Saved {
            // A definition kept before a rule tightened is checked again, and left out if it fails.
            read: Definition::parse(value.get("read")?).ok()?,
            saved: number("saved")?,
            asked_by: super::text(text("asked_by")?, super::definition::NAME_MOST),
            last_read: number("last_read")?,
            state,
            since: number("since")?,
        })
    }
}

/// Why a read can't be saved or changed.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// A read already has its name, which agents call it by.
    Taken,
    /// No read has the name.
    NotFound,
    /// The profile keeps `MOST`.
    Full,
}

/// A profile's saved reads, in the order they were saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    reads: Vec<Saved>,
}

impl Store {
    /// The reads `text` keeps, as `write` wrote it; a read it can't read is left out.
    pub fn read(text: &str) -> Store {
        let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let mut store = Store::default();
        for saved in value
            .get("reads")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(saved) = Saved::from_json(saved) else {
                continue;
            };
            if store.reads.len() < MOST && store.get(&saved.read.name).is_none() {
                store.reads.push(saved);
            }
        }
        store
    }

    /// The reads as the profile's file has them, or "" for none, which leaves no file.
    pub fn write(&self) -> String {
        if self.reads.is_empty() {
            return String::new();
        }
        let reads: Vec<Value> = self.reads.iter().map(Saved::to_json).collect();
        format!("{}\n", json!({ "version": 1, "reads": reads }))
    }

    /// The reads `folder` keeps, or none for a private window's, which has no folder.
    pub fn load(folder: Option<&Path>) -> Store {
        let text = folder.and_then(|folder| std::fs::read_to_string(folder.join(FILE)).ok());
        text.map_or_else(Store::default, |text| Store::read(&text))
    }

    /// Keeps the reads in `folder`, or nowhere for a private window. None leaves no file.
    pub fn keep(&self, folder: Option<&Path>) -> std::io::Result<()> {
        let Some(folder) = folder else { return Ok(()) };
        let path = folder.join(FILE);
        let text = self.write();
        if text.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        // Written beside it and moved into place, so a crash mid-write leaves the last one whole.
        let next = folder.join(format!("{FILE}.new"));
        std::fs::write(&next, text)?;
        std::fs::rename(&next, &path)
    }

    pub fn list(&self) -> &[Saved] {
        &self.reads
    }

    /// The read named `name`, in any case, as an agent may write it.
    pub fn get(&self, name: &str) -> Option<&Saved> {
        self.reads
            .iter()
            .find(|saved| saved.read.name.eq_ignore_ascii_case(name))
    }

    /// Saves `read`, which `asked_by` asked for, as read at `now` with `state`.
    pub fn save(
        &mut self,
        read: Definition,
        asked_by: &str,
        state: State,
        now: u64,
    ) -> Result<(), Refusal> {
        if self.get(&read.name).is_some() {
            return Err(Refusal::Taken);
        }
        if self.reads.len() >= MOST {
            return Err(Refusal::Full);
        }
        let asked_by = super::text(asked_by, super::definition::NAME_MOST);
        self.reads.push(Saved {
            read,
            saved: now,
            asked_by,
            last_read: now,
            state,
            since: now,
        });
        Ok(())
    }

    /// Puts `read` in place of the one named `name`, keeping when and by whom it was saved. Its
    /// state is what reading the new one at `now` came to.
    pub fn change(
        &mut self,
        name: &str,
        read: Definition,
        state: State,
        now: u64,
    ) -> Result<(), Refusal> {
        let at = self.at(name).ok_or(Refusal::NotFound)?;
        if !read.name.eq_ignore_ascii_case(name) && self.get(&read.name).is_some() {
            return Err(Refusal::Taken);
        }
        let saved = &mut self.reads[at];
        saved.read = read;
        saved.last_read = now;
        saved.state = state;
        saved.since = now;
        Ok(())
    }

    /// Removes the read named `name`, giving it back for an undo.
    pub fn remove(&mut self, name: &str) -> Option<Saved> {
        self.at(name).map(|at| self.reads.remove(at))
    }

    /// Puts back a read `remove` gave, where it was if it can.
    pub fn restore(&mut self, saved: Saved, at: usize) -> Result<(), Refusal> {
        if self.get(&saved.read.name).is_some() {
            return Err(Refusal::Taken);
        }
        if self.reads.len() >= MOST {
            return Err(Refusal::Full);
        }
        let at = at.min(self.reads.len());
        self.reads.insert(at, saved);
        Ok(())
    }

    /// Notes that the read named `name` was read at `now` and came to `state`; `since` moves only
    /// when the state does.
    pub fn read_at(&mut self, name: &str, state: State, now: u64) -> Result<(), Refusal> {
        let at = self.at(name).ok_or(Refusal::NotFound)?;
        let saved = &mut self.reads[at];
        if saved.state.id() != state.id() {
            saved.since = now;
        }
        saved.last_read = now;
        saved.state = state;
        Ok(())
    }

    /// Where the read named `name` is in the list.
    pub fn at(&self, name: &str) -> Option<usize> {
        self.reads
            .iter()
            .position(|saved| saved.read.name.eq_ignore_ascii_case(name))
    }
}
