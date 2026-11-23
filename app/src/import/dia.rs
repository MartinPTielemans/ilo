//! Dia's own sidebar, its Spaces, pinned tabs, folders and favorites, which come as Spaces
//! (#4051). Not read yet: until it is, Dia comes as its profiles' bookmarks and history alone.

use std::path::PathBuf;

use super::found::Found;
use super::read::ReadError;
use super::sources::Source;

/// The files Dia keeps its sidebar in: none yet.
pub fn files(_source: &Source) -> Vec<PathBuf> {
    Vec::new()
}

/// Dia's Spaces from its sidebar: none yet.
pub fn read(_source: &Source) -> Result<Option<Found>, ReadError> {
    Ok(None)
}
