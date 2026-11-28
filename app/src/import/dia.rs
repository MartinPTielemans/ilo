//! Dia's own sidebar, its Spaces, pinned tabs, folders and favorites, which come as Spaces
//! (#4051). Not read yet: until it is, Dia comes as its profiles' bookmarks and history alone.

use std::path::PathBuf;

use super::found::{Action, Found, Note};
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

/// Why Dia's Spaces can't come here, if they can't: none until its reader is in, so its Spaces
/// read as none at all.
pub fn note(_source: &Source) -> Option<Note> {
    None
}

/// Said in place of Dia's Spaces, which it keeps encrypted, with the way to bring them from its
/// export. Only PREVIEW_IMPORT shows it until Choose File… does something: the change that makes
/// `note` give it is the one that reads the export the user chooses.
pub const ENCRYPTED: Note = Note {
    said: "Dia encrypts its Spaces, so ilo can't import them directly. Export a Markdown file \
        from Dia, then choose it here.",
    action: Some(Action::ChooseFile),
};
