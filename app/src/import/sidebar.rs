//! A browser's own sidebar, where it keeps its Spaces, pinned tabs, folders and favorites apart
//! from its profiles' bookmarks: Arc's, Dia's, Zen's and Ardia's, each read by its own module, so
//! a source added here touches only its own (#4051).

use std::path::PathBuf;

use super::found::{Found, Note};
use super::read::ReadError;
use super::sources::Source;
use super::{arc, ardia, dia, zen};

/// The files `source` keeps its own sidebar in, which date when it was used last; none if it
/// keeps none, or none here.
pub fn files(source: &Source) -> Vec<PathBuf> {
    match source.browser.name {
        "Arc" => arc::files(source),
        "Dia" => dia::files(source),
        "Zen" => zen::files(source),
        "Ardia" => ardia::files(source),
        _ => Vec::new(),
    }
}

/// Whether `source` keeps a sidebar of its own here, whose Spaces come as Spaces.
pub fn has(source: &Source) -> bool {
    !files(source).is_empty()
}

/// `source`'s Spaces with their pinned tabs and folders, its favorites, and the profiles and
/// things with no place here they name, read from a copy of its sidebar; none if it keeps none.
pub fn read(source: &Source) -> Result<Option<Found>, ReadError> {
    match source.browser.name {
        "Arc" => arc::read(source),
        "Dia" => dia::read(source),
        "Zen" => zen::read(source),
        "Ardia" => ardia::read(source),
        _ => Ok(None),
    }
}

/// Why `source`'s Spaces can't come here, said in their place: a browser that keeps them where
/// ilo can't read them may keep no sidebar file it can, so this asks whether there's none.
pub fn note(source: &Source) -> Option<Note> {
    match source.browser.name {
        "Dia" => dia::note(source),
        "Zen" => zen::note(source),
        _ => None,
    }
}
