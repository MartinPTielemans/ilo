//! Zen's own sidebar, its workspaces, pinned tabs, folders and Essentials, which come as Spaces
//! (#4051). Not read yet: until it is, Zen comes as its profiles' bookmarks and history alone.

use std::path::PathBuf;

use super::found::Found;
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
