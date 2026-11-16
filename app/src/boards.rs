//! Boards (#3073): a Board is a folder of the profile's, `Boards/<id>/`, which the core's host
//! serves its page from and writes (core/board.cc). The app makes the folder and opens the page;
//! the page's own writes go through the host, never through the app.

use std::io;
use std::path::{Path, PathBuf};

/// The page's address for Board `id`.
pub fn url(id: &str) -> String {
    format!("chrome://board/{id}")
}

/// The Board `url` names, as the host reads it: 32 lowercase hex digits after chrome://board/,
/// and nothing after them but the page's own query or fragment.
pub fn id_in(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("chrome://board/")?;
    let id = rest.split(['?', '#']).next().unwrap_or_default();
    is_id(id).then_some(id)
}

/// Whether `id` is a Board's: 32 lowercase hex digits, so it can't name a path.
pub fn is_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A new Board's id, from the system's random numbers.
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the system has no random numbers");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The folder each of `profile`'s Boards' folders is in.
pub fn dir(profile: &Path) -> PathBuf {
    profile.join("Boards")
}

/// Makes Board `id`'s folder in `profile`'s, which the host reads as the Board being there. It
/// blocks, so it runs off the main thread.
pub fn make(profile: &Path, id: &str) -> io::Result<PathBuf> {
    if !is_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    let folder = dir(profile).join(id);
    std::fs::create_dir_all(&folder)?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_id_names_a_board_and_its_address_gives_it_back() {
        let id = new_id();
        assert!(is_id(&id), "{id}");
        assert_ne!(id, new_id(), "two Boards had one id");
        assert_eq!(id_in(&url(&id)), Some(id.as_str()));
        assert_eq!(id_in(&format!("{}#here", url(&id))), Some(id.as_str()));
    }

    /// The id is the folder's name, so anything that could step out of `Boards/` isn't one.
    #[test]
    fn only_32_lowercase_hex_digits_are_an_id() {
        let id = "0123456789abcdef0123456789abcdef";
        assert!(is_id(id));
        for not in [
            "",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "../456789abcdef0123456789abcdef0",
            "0123456789abcdef/123456789abcdef",
        ] {
            assert!(!is_id(not), "{not}");
            assert_eq!(id_in(&url(not)), None, "{not}");
        }
        assert_eq!(id_in(&format!("chrome://boards/{id}")), None);
        assert_eq!(id_in(&format!("https://board/{id}")), None);
        assert_eq!(id_in(&format!("chrome://board/{id}/board.json")), None);
    }

    #[test]
    fn make_puts_the_folder_in_the_profiles_boards_and_refuses_what_isnt_an_id() {
        let profile = std::env::temp_dir().join(format!("ilo-boards-{}", new_id()));
        let id = new_id();
        let made = make(&profile, &id).unwrap();
        assert_eq!(made, profile.join("Boards").join(&id));
        assert!(made.is_dir());
        // Again is the same folder, as a second New Board never meets one.
        assert_eq!(make(&profile, &id).unwrap(), made);
        assert!(make(&profile, "../outside").is_err());
        std::fs::remove_dir_all(&profile).unwrap();
    }
}
