//! Zen's own sidebar, its workspaces, pinned tabs, folders and Essentials, which come as Spaces
//! (#4051). Not read yet: until it is, Zen comes as its profiles' bookmarks and history alone.

use std::path::PathBuf;

use super::found::{Found, Note};
use super::read::ReadError;
use super::sources::Source;

/// The files Zen keeps its sidebar in: none yet.
pub fn files(_source: &Source) -> Vec<PathBuf> {
    Vec::new()
}

/// Zen's Spaces from its sidebar: none yet.
pub fn read(_source: &Source) -> Result<Option<Found>, ReadError> {
    Ok(None)
}

/// Why Zen's Spaces can't come here, if they can't: none until its reader is in, so its Spaces
/// read as none at all.
pub fn note(_source: &Source) -> Option<Note> {
    None
}

/// Said in place of Zen's Spaces when its version keeps them where ilo can't read them.
pub const TOO_OLD: Note = Note {
    said: "This version of Zen is older than 1.18, so ilo can't import Spaces.",
    action: None,
};
