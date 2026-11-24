//! Finished tasks the user can reply to (#4105). The assistant keeps a task's conversation in its
//! own store, as it keeps one started in a terminal, and ilo holds the conversation's id beside
//! the task and nothing else of it: no transcript, no page text. This is the one place that decides
//! whether a run is kept (`decide`), what the runner starts it with (`claude_args`), the access a
//! task comes back at (`comes_back`) and how a kept conversation is removed (`remove`).
//!
//! The id is ilo's own making and goes to the assistant on its command line, into the file below
//! and nowhere else: no log line, no crash key, not Library › Agents' file, and nothing an agent
//! or an outside client is told. Its `Debug` prints none of it, so a line written by mistake holds
//! none either.
//!
//! A task that has ended is there to reply to after a quit too, so the file says of it what
//! finds it again (`Past`, the owner's call): its title, how and when it ended, and its profile.
//! Never what was asked, a page's address, a step, a file's name or the access it had. A row
//! with an id alone is a conversation held only to be removed.
//!
//! A kept task runs in a folder of its own, `Kept tasks/<id>` under the profile's, which the
//! assistant files the conversation by. Nothing is ever put in it: it outlives the task's run only
//! so the conversation can be found again, and removed. Removing is the assistant's own
//! `purge <folder> --yes`, which takes out what its run in that folder left: the conversation, the
//! folder's lines in the assistant's prompt history and its entry in the assistant's config. ilo
//! itself reads and writes nothing of the assistant's.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::runner::Cli;
use super::tier::Tier;

/// The folder under the profile's that holds each kept task's own, and the file of their ids.
pub const FOLDER: &str = "Kept tasks";
/// The file of ids, in `FOLDER`: each kept conversation's id and its assistant.
const INDEX: &str = "index.json";
/// What starts a Claude Code run that keeps nothing, which a kept run's id takes the place of.
const KEEP_NOTHING: &str = "--no-session-persistence";
/// How long the assistant gets to remove one conversation before it counts as not removed.
const REMOVING: Duration = Duration::from_secs(30);

/// A kept conversation's id: a UUID ilo makes, which the assistant files the conversation under.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Id(String);

impl Id {
    /// A new one: a random UUID (version 4), as Claude Code's `--session-id` takes.
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).expect("the system has no random numbers");
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        Id(format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        ))
    }

    /// `text` as an id, if it has a UUID's shape: nothing else names a folder or reaches a command
    /// line, so a file someone changed can't point either elsewhere.
    pub fn parse(text: &str) -> Option<Self> {
        let shaped = text.len() == 36
            && text.bytes().enumerate().all(|(at, byte)| match at {
                8 | 13 | 18 | 23 => byte == b'-',
                _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
            });
        shaped.then(|| Id(text.to_string()))
    }

    /// The id itself, for the assistant's command line and the file of ids alone.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

/// None of it, so nothing that prints a value prints an id.
impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Id(…)")
    }
}

/// What a run keeps of its conversation, which the runner starts it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Nothing: the conversation lives as long as the run.
    Nothing,
    /// The assistant keeps it under this id, new with this run.
    New(Id),
    /// The run goes on with the conversation the assistant kept under this id.
    Resume(Id),
}

/// Why a task a run before left has nowhere to work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nowhere {
    /// Its profile was deleted.
    Deleted,
    /// Its profile is there, with no Space of the user's left in it.
    NoSpace,
}

/// Whether an assistant's runs can be kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keeps {
    /// Never: each run keeps nothing, whatever is wanted.
    Never,
    /// When a run is wanted kept.
    WhenWanted,
}

/// The table: one row for each assistant.
pub const fn keeps(cli: Cli) -> Keeps {
    match cli {
        // Its print-mode conversations stay out of its own history in a terminal, in a folder of
        // ilo's that no terminal is in.
        Cli::Claude => Keeps::WhenWanted,
        // Until a run shows its app-server's threads stay out of its own history in a terminal.
        Cli::Codex => Keeps::Never,
    }
}

/// What a new run of `cli` keeps. `wanted` is why it would be kept at all: today Settings ›
/// Agents' switch alone. The runner is told only what comes of it, never the reason.
pub fn decide(cli: Cli, wanted: bool) -> Keep {
    match (keeps(cli), wanted) {
        (Keeps::WhenWanted, true) => Keep::New(Id::new()),
        (Keeps::WhenWanted, false) | (Keeps::Never, _) => Keep::Nothing,
    }
}

/// Claude Code's arguments for a run that keeps what `keep` says, from a run's that keeps nothing.
/// A run going on with a conversation takes every other argument again, as Claude Code restores
/// none of them itself: its tools, its permissions and its settings are this run's own.
pub fn claude_args(mut args: Vec<String>, keep: &Keep) -> Vec<String> {
    let (flag, id) = match keep {
        Keep::Nothing => return args,
        Keep::New(id) => ("--session-id", id),
        Keep::Resume(id) => ("--resume", id),
    };
    let kept = [flag.to_string(), id.as_str().to_string()];
    match args.iter().position(|arg| arg == KEEP_NOTHING) {
        Some(at) => _ = args.splice(at..=at, kept),
        None => args.extend(kept),
    }
    args
}

/// The access a task comes back at when the user replies after it ended at `ended`: where every
/// task starts, or lower if the user had put it lower. Full access is never brought back. Only the
/// app decides it, from what it kept itself.
pub fn comes_back(ended: Tier) -> Tier {
    ended.min(Tier::default())
}

/// Whether what a run that went said, on its way out, is Claude Code finding no conversation
/// under the id it was to go on with: the assistant cleared it, was signed out or reinstalled.
pub fn gone(said: &str) -> bool {
    said.contains("No conversation found")
}

/// `said` without `id`, for a log line: a run's last words may name the conversation.
pub fn without(said: &str, id: &Id) -> String {
    said.replace(id.as_str(), "…")
}

/// What a task says when `agent` no longer has the conversation it kept: ilo's line in its reply
/// field's place, and why a task that was to go on with it couldn't.
pub fn gone_words(agent: &str) -> String {
    format!("{agent} no longer has this conversation.")
}

/// How a task ended, as the file says it: the four its word tells apart (`task::Word`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Done,
    /// The user stopped it, or quit while it worked.
    Stopped,
    Failed,
    /// At its assistant's limit. When that resets isn't kept.
    Limit,
}

impl End {
    const ALL: [End; 4] = [End::Done, End::Stopped, End::Failed, End::Limit];

    fn id(self) -> &'static str {
        match self {
            End::Done => "done",
            End::Stopped => "stopped",
            End::Failed => "failed",
            End::Limit => "limit",
        }
    }
}

/// The most of a title the file gives back, in characters: a name an agent gave is shorter, so
/// only a file someone changed has more.
const TITLE: usize = 80;

/// What the file says of a task that has ended, beside its conversation's id: all a task is
/// after a quit, until a reply takes it up.
#[derive(Clone, Debug, PartialEq)]
pub struct Past {
    /// The name its agent gave it (#3690), or empty if it gave none: what the user asked, which
    /// names a task until then, is never kept.
    pub title: String,
    pub end: End,
    /// When it ended, in seconds since 1970.
    pub at: u64,
}

/// A kept conversation, beside its task.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Its task, or None once the task went and the conversation couldn't be removed with it, or
    /// for one an earlier run of the browser left, until `give` makes it a task again.
    pub task: Option<String>,
    pub cli: Cli,
    pub id: Id,
    /// What the file says of its task, once it has ended and while it's listed: none for one
    /// deleted from the Archive, which a quit then leaves to be removed.
    pub past: Option<Past>,
    /// The profile its task was asked in, by the name of the profile's own folder, which names
    /// it from one run to the next where the core's number for it doesn't. Its task works in
    /// that profile's sessions and comes back in no other.
    pub profile: String,
    /// Why its task has nowhere to work, if it hasn't, so it takes no reply.
    pub nowhere: Option<Nowhere>,
    /// The access the task last ended at.
    pub tier: Tier,
    /// The assistant no longer has it: a reply found none.
    pub gone: bool,
    /// The user's replies that took the task up again, each by how many messages of theirs came
    /// before it and the access it came back at: ilo's line goes over each.
    pub resumed: Vec<(usize, Tier)>,
    /// Its removal is under way.
    removing: bool,
    /// Its removal failed: it's still there, for the user to try again.
    stuck: bool,
}

/// The conversations ilo holds an id for.
#[derive(Debug, Default)]
pub struct Kept {
    records: Vec<Record>,
    /// Settings › Agents' Keep finished tasks, off.
    pub off: bool,
    /// The tasks whose conversation was removed from under them, as Remove All does: each had
    /// one, where a task that started with the switch off never did, and the two say so apart.
    removed_from: Vec<String>,
}

impl Kept {
    /// How many it holds an id for: not a promise each still takes a reply.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// How many of them the last try couldn't remove.
    pub fn stuck(&self) -> usize {
        (self.records.iter()).filter(|record| record.stuck).count()
    }

    /// `task`'s, while it isn't being removed.
    pub fn of(&self, task: &str) -> Option<&Record> {
        (self.records.iter())
            .find(|record| record.task.as_deref() == Some(task) && !record.removing)
    }

    /// Whether `task` had a conversation kept that has since been removed from under it.
    pub fn had(&self, task: &str) -> bool {
        self.removed_from.iter().any(|each| each == task)
    }

    /// `task` goes for good, so nothing is said of it again.
    pub fn forget(&mut self, task: &str) {
        self.removed_from.retain(|each| each != task);
    }

    pub fn of_mut(&mut self, task: &str) -> Option<&mut Record> {
        (self.records.iter_mut())
            .find(|record| record.task.as_deref() == Some(task) && !record.removing)
    }

    /// Holds `id` for `task`, new, or with no task for one an earlier run left.
    fn hold_for(&mut self, task: Option<&str>, cli: Cli, id: Id, profile: &str) {
        self.records.push(Record {
            task: task.map(String::from),
            cli,
            id,
            past: None,
            profile: profile.to_string(),
            nowhere: None,
            tier: Tier::default(),
            gone: false,
            resumed: Vec::new(),
            removing: false,
            stuck: false,
        });
    }

    /// Holds `id` for `task`, new, asked in `profile`.
    pub fn hold(&mut self, task: &str, cli: Cli, id: Id, profile: &str) {
        self.hold_for(Some(task), cli, id, profile);
    }

    /// The tasks an earlier run of the browser left that ended, each by its conversation's id,
    /// the last to end first: those `read` took that the file says more of than an id.
    pub fn left(&self) -> Vec<(Id, Cli, Past)> {
        // The last held first, so of two that ended in one second the later is the newer.
        let mut left: Vec<_> = (self.records.iter().rev())
            .filter(|record| record.task.is_none() && !record.removing)
            .filter_map(|record| Some((record.id.clone(), record.cli, record.past.clone()?)))
            .collect();
        left.sort_by_key(|(.., past)| std::cmp::Reverse(past.at));
        left
    }

    /// The conversation under `id`, which an earlier run left, is `task`'s from now: a task of
    /// this run's.
    pub fn give(&mut self, id: &Id, task: &str) {
        for record in self.records.iter_mut().filter(|record| record.id == *id) {
            record.task = Some(task.to_string());
        }
    }

    /// The tasks whose conversation takes a reply: each one's that has ended and is listed.
    pub fn listed(&self) -> impl Iterator<Item = &str> {
        (self.records.iter())
            .filter(|record| record.past.is_some() && !record.gone && !record.removing)
            .filter_map(|record| record.task.as_deref())
    }

    /// The ids to remove of those `which` picks, each with its assistant, now marked as under way
    /// so none is taken up or removed twice meanwhile.
    pub fn removing(&mut self, which: impl Fn(&Record) -> bool) -> Vec<(Id, Cli)> {
        let removed_from = &mut self.removed_from;
        (self.records.iter_mut())
            .filter(|record| !record.removing && which(record))
            .map(|record| {
                record.removing = true;
                record.stuck = false;
                // Its task has gone, or is left with none.
                removed_from.extend(record.task.take());
                (record.id.clone(), record.cli)
            })
            .collect()
    }

    /// A removal ended: the id goes if the conversation did, else it stays, marked.
    pub fn removed(&mut self, id: &Id, removed: bool) {
        match removed {
            true => self.records.retain(|record| record.id != *id),
            false => {
                for record in self.records.iter_mut().filter(|record| record.id == *id) {
                    record.removing = false;
                    record.stuck = true;
                }
            }
        }
    }

    /// Takes what an earlier run of the browser left in `root`'s file, each with no task: one
    /// that ended, with what the file says of it, for `left` and `give` to make a task of again,
    /// and one with an id alone, held only to be removed. Whether there was a file: with none,
    /// the look for it is all this does.
    pub fn read(&mut self, root: &Path) -> bool {
        let Ok(saved) = std::fs::read(root.join(INDEX)) else {
            return false;
        };
        let saved: Value = serde_json::from_slice(&saved).unwrap_or_default();
        for each in saved.as_array().into_iter().flatten() {
            let text = |key: &str| each.get(key).and_then(Value::as_str);
            let id = text("id").and_then(Id::parse);
            let cli =
                text("cli").and_then(|cli| Cli::ALL.into_iter().find(|each| each.id() == cli));
            let (Some(id), Some(cli)) = (id, cli) else {
                continue;
            };
            if self.records.iter().any(|record| record.id == id) {
                continue;
            }
            // A folder's own name and nothing more: one with a path in it names no profile.
            let profile = text("profile").filter(|name| !name.contains(['/', '\\']));
            self.hold_for(None, cli, id, profile.unwrap_or_default());
            let end =
                text("state").and_then(|end| End::ALL.into_iter().find(|each| each.id() == end));
            let at = each.get("ended").and_then(Value::as_u64);
            if let (Some(end), Some(at), Some(record)) = (end, at, self.records.last_mut()) {
                // One line of a title's length, as the app wrote it.
                let title: String = (text("title").unwrap_or_default().chars())
                    .filter(|each| !each.is_control())
                    .take(TITLE)
                    .collect();
                record.past = Some(Past { title, end, at });
            }
        }
        true
    }

    /// Writes what's held to `root`'s file, which only its owner can read, or removes the file
    /// and the folder with none left: each conversation's id and assistant, and of a task that
    /// has ended and is listed, what `Past` holds and its profile.
    pub fn write(&self, root: &Path) {
        if self.records.is_empty() {
            _ = std::fs::remove_file(root.join(INDEX));
            // Only if it's empty.
            _ = std::fs::remove_dir(root);
            return;
        }
        let saved: Vec<Value> = (self.records.iter())
            .map(|record| {
                let mut row = json!({ "id": record.id.as_str(), "cli": record.cli.id() });
                let listed = record.task.is_some() && !record.gone && !record.removing;
                if let Some(past) = record.past.as_ref().filter(|_| listed) {
                    row["title"] = json!(past.title);
                    row["state"] = json!(past.end.id());
                    row["ended"] = json!(past.at);
                    row["profile"] = json!(record.profile);
                }
                row
            })
            .collect();
        _ = std::fs::create_dir_all(root);
        _ = super::clients::write_private(
            &root.join(INDEX),
            Value::from(saved).to_string().as_bytes(),
        );
    }
}

/// The folder a kept task runs in, in `root`: its id's, which is a UUID and so no path elsewhere.
pub fn folder(root: &Path, id: &Id) -> PathBuf {
    root.join(id.as_str())
}

/// `id`'s folder as the system resolves it, if it's one of `root`'s own: directly inside it, by
/// its real path, whatever links stand in the way. Made if it's missing, as the assistant files
/// the conversation by its path whether or not it's still there.
fn own_folder(root: &Path, id: &Id) -> Option<PathBuf> {
    let id = Id::parse(id.as_str())?;
    let folder = folder(root, &id);
    _ = std::fs::create_dir_all(&folder);
    let (root, folder) = (root.canonicalize().ok()?, folder.canonicalize().ok()?);
    (folder.parent() == Some(root.as_path()) && folder.is_dir()).then_some(folder)
}

/// Removes the conversation kept under `id` in `root`: the assistant `program` removes what its
/// run in the task's folder left, then the folder goes. True once it's gone, or was already;
/// false if it may still be there, when nothing of ilo's is removed either, so it can be tried
/// again. Only a folder ilo made is ever named to the assistant, checked right before it's asked.
/// It waits on the assistant, so it's never run on the UI's thread.
pub fn remove(program: &Path, path: Option<&str>, root: &Path, id: &Id) -> bool {
    let Some(folder) = own_folder(root, id) else {
        return false;
    };
    let mut command = crate::child::command(program);
    // The whole command: remove what's kept for this one folder, without asking.
    command
        .arg("purge")
        .arg(&folder)
        .arg("--yes")
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    // The standard library waits with no limit, so this looks again until it ends or its time is
    // up: on a thread of the removal's own, for as long as the removal takes.
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < REMOVING => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                _ = child.kill();
                _ = child.wait();
                return false;
            }
        }
    };
    // Removed, or 1: it had nothing for the folder, which is as good as removed.
    if !matches!(status.code(), Some(0 | 1)) {
        return false;
    }
    std::fs::remove_dir_all(&folder).is_ok() || !folder.exists()
}

#[cfg(test)]
mod tests;
