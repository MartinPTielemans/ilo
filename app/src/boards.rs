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

/// An empty Board's file, as its page writes one.
const EMPTY: &str = r#"{"version":1,"camera":{"x":0,"y":0,"zoom":1},"cards":[]}"#;

/// Makes Board `id`'s folder in `profile`'s, which the host reads as the Board being there, with an
/// empty Board's file in it. Every Board has its file from the start, so a page that can't read one
/// knows the file is there and never writes over it. It blocks, so it runs off the main thread.
pub fn make(profile: &Path, id: &str) -> io::Result<PathBuf> {
    if !is_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    let folder = dir(profile).join(id);
    std::fs::create_dir_all(&folder)?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join("board.json"));
    match file {
        Ok(mut file) => io::Write::write_all(&mut file, EMPTY.as_bytes())?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    Ok(folder)
}

/// A Board as the Library lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: String,
    /// The name its row was given, else the product's.
    pub name: String,
    /// Its picture, which the page saves as it's left, if it has one yet.
    pub thumb: Option<PathBuf>,
    /// When its page last saved it, in milliseconds since 1970, for the Library's days.
    pub changed: f64,
}

/// Where a removed Board waits for ⌘Z, out of the list. The next launch's list empties it.
const REMOVED: &str = ".removed";

/// `profile`'s Boards, the last changed first. It reads the disk, so it runs off the main thread.
/// Its first read of a profile in a launch deletes the Boards removed before it, which no ⌘Z of
/// this launch's can bring back, and none can have been removed yet, as removing needs the list.
pub fn list(profile: &Path) -> Vec<Listed> {
    static FORGOTTEN: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
    if let Ok(mut forgotten) = FORGOTTEN.lock()
        && !forgotten.iter().any(|each| each == profile)
    {
        forgotten.push(profile.to_owned());
        if let Err(error) = forget_removed(profile) {
            crate::log!("browser: removed boards stay: {error}");
        }
    }
    let Ok(entries) = std::fs::read_dir(dir(profile)) else {
        return Vec::new();
    };
    let mut boards: Vec<Listed> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().into_string().ok()?;
            let folder = entry.path();
            (is_id(&id) && folder.is_dir()).then(|| {
                let name = std::fs::read_to_string(folder.join("name"))
                    .ok()
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "Board".to_owned());
                let thumb = Some(folder.join("thumb.png")).filter(|thumb| thumb.is_file());
                let changed = (std::fs::metadata(folder.join("board.json")))
                    .or_else(|_| std::fs::metadata(&folder))
                    .and_then(|data| data.modified())
                    .ok()
                    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0., |since| since.as_millis() as f64);
                Listed {
                    id,
                    name,
                    thumb,
                    changed,
                }
            })
        })
        .collect();
    boards.sort_by(|a, b| {
        b.changed
            .total_cmp(&a.changed)
            .then_with(|| a.id.cmp(&b.id))
    });
    boards
}

/// Keeps `name` as Board `id`'s, for the Library, as its row was named. Off the main thread.
pub fn write_name(profile: &Path, id: &str, name: &str) -> io::Result<()> {
    if !is_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    std::fs::write(dir(profile).join(id).join("name"), name.trim())
}

/// Takes Board `id` out of the list, aside where `restore` brings it back from. Off the main
/// thread.
pub fn remove(profile: &Path, id: &str) -> io::Result<()> {
    if !is_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    let aside = dir(profile).join(REMOVED);
    std::fs::create_dir_all(&aside)?;
    std::fs::rename(dir(profile).join(id), aside.join(id))
}

/// Brings Board `id` back from where `remove` put it.
pub fn restore(profile: &Path, id: &str) -> io::Result<()> {
    if !is_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    std::fs::rename(dir(profile).join(REMOVED).join(id), dir(profile).join(id))
}

/// Deletes the Boards removed before this launch, which ⌘Z can no longer bring back.
pub fn forget_removed(profile: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir(profile).join(REMOVED)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        done => done,
    }
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
        let file = made.join("board.json");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), EMPTY);
        // Again is the same folder, as a second New Board never meets one, and its file isn't
        // written over.
        std::fs::write(&file, "kept").unwrap();
        assert_eq!(make(&profile, &id).unwrap(), made);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "kept");
        assert!(make(&profile, "../outside").is_err());
        std::fs::remove_dir_all(&profile).unwrap();
    }

    #[test]
    fn the_list_is_the_last_changed_first_with_each_name_and_picture_and_a_removed_one_comes_back()
    {
        let profile = std::env::temp_dir().join(format!("ilo-boards-{}", new_id()));
        let (old, new) = ("a".repeat(32), "b".repeat(32));
        make(&profile, &old).unwrap();
        std::fs::write(dir(&profile).join(&old).join("board.json"), "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let folder = make(&profile, &new).unwrap();
        std::fs::write(folder.join("board.json"), "{}").unwrap();
        std::fs::write(folder.join("thumb.png"), b"\x89PNG").unwrap();
        write_name(&profile, &new, "  Trip  ").unwrap();
        // Not a Board's: the list leaves it out.
        std::fs::create_dir_all(dir(&profile).join("notes")).unwrap();

        let listed = list(&profile);
        let ids: Vec<&str> = listed.iter().map(|board| board.id.as_str()).collect();
        assert_eq!(ids, [new.as_str(), old.as_str()]);
        assert_eq!(
            (listed[0].name.as_str(), listed[1].name.as_str()),
            ("Trip", "Board")
        );
        assert_eq!(listed[0].thumb, Some(folder.join("thumb.png")));
        assert_eq!(listed[1].thumb, None);

        remove(&profile, &new).unwrap();
        assert_eq!(list(&profile).len(), 1, "the removed Board is still listed");
        restore(&profile, &new).unwrap();
        assert_eq!(
            list(&profile)[0].name,
            "Trip",
            "⌘Z didn't bring the Board back whole"
        );
        remove(&profile, &new).unwrap();
        forget_removed(&profile).unwrap();
        assert!(
            restore(&profile, &new).is_err(),
            "a Board removed before came back"
        );
        forget_removed(&profile).unwrap();
        assert!(remove(&profile, "../x").is_err() && write_name(&profile, "x", "y").is_err());
        std::fs::remove_dir_all(&profile).unwrap();
    }
}
