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
                let name = read_name(&folder);
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

/// The name a Board's row was given, else the product's.
fn read_name(folder: &Path) -> String {
    std::fs::read_to_string(folder.join("name"))
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Board".to_owned())
}

/// Whether `name` is one the core gives a file it stored on a Board: its bytes' SHA-256 in
/// lowercase hex and its picture's extension (core/board_store.cc's FileName). A file half stored
/// has a temporary name, so it isn't one.
fn is_stored(name: &str) -> bool {
    name.split_once('.').is_some_and(|(hash, extension)| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && ["png", "jpg", "gif", "webp"].contains(&extension)
    })
}

/// A copy of the Board named `name`: "<name> copy", or "<name> copy 2", "<name> copy 3" and on,
/// the first no Board of `taken` has, as Finder names a duplicate. A copy's copy counts on from
/// the same "<name> copy" rather than being "copy copy".
pub fn copy_name(name: &str, taken: &[String]) -> String {
    let root = match name.rsplit_once(" copy ") {
        Some((root, number))
            if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            root
        }
        _ => name.strip_suffix(" copy").unwrap_or(name),
    };
    let first = format!("{root} copy");
    (1..)
        .map(|n| match n {
            1 => first.clone(),
            n => format!("{first} {n}"),
        })
        .find(|each| !taken.contains(each))
        .unwrap_or(first)
}

/// Copies Board `from` of `profile` as Board `to`, named by `copy_name`: its file, its picture and
/// the files it stored, but not the captures still to land on it (they land once, on the Board
/// they were sent to) or a file half stored. The copy is made in a hidden folder the list skips
/// and renamed to `to` once whole, so a half copy is never listed or opened. One that fails stays
/// in its hidden folder: removing it is the owner's call (#3406). Returns the copy's name. Off the
/// main thread.
pub fn duplicate(profile: &Path, from: &str, to: &str) -> io::Result<String> {
    if !is_id(from) || !is_id(to) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a Board's id",
        ));
    }
    let source = dir(profile).join(from);
    let taken: Vec<String> = list(profile).into_iter().map(|board| board.name).collect();
    let name = copy_name(&read_name(&source), &taken);
    let hidden = dir(profile).join(format!(".copy-{to}"));
    std::fs::create_dir(&hidden)?;
    std::fs::copy(source.join("board.json"), hidden.join("board.json"))?;
    match std::fs::copy(source.join("thumb.png"), hidden.join("thumb.png")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        copied => {
            copied?;
        }
    }
    match std::fs::read_dir(source.join("files")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
        Ok(entries) => {
            std::fs::create_dir(hidden.join("files"))?;
            for entry in entries {
                let entry = entry?;
                let stored = entry.file_name().to_str().is_some_and(is_stored);
                if stored && entry.file_type()?.is_file() {
                    std::fs::copy(entry.path(), hidden.join("files").join(entry.file_name()))?;
                }
            }
        }
    }
    std::fs::write(hidden.join("name"), &name)?;
    std::fs::rename(&hidden, dir(profile).join(to))?;
    Ok(name)
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

    #[test]
    fn a_copy_is_named_by_the_next_free_number_and_a_copys_copy_counts_on() {
        let taken = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(copy_name("Trip", &taken(&["Trip"])), "Trip copy");
        assert_eq!(
            copy_name("Trip", &taken(&["Trip", "Trip copy"])),
            "Trip copy 2"
        );
        assert_eq!(
            copy_name("Trip", &taken(&["Trip", "Trip copy", "Trip copy 2"])),
            "Trip copy 3"
        );
        assert_eq!(
            copy_name("Trip copy", &taken(&["Trip", "Trip copy"])),
            "Trip copy 2"
        );
        assert_eq!(
            copy_name("Trip copy 2", &taken(&["Trip copy", "Trip copy 2"])),
            "Trip copy 3"
        );
        // A number that's the name's own isn't counted on.
        assert_eq!(copy_name("Room 101", &[]), "Room 101 copy");
        assert_eq!(copy_name("Board", &taken(&["Board"])), "Board copy");
    }

    #[test]
    fn a_duplicate_has_the_boards_file_picture_name_and_stored_files_and_nothing_else() {
        let profile = std::env::temp_dir().join(format!("ilo-boards-{}", new_id()));
        let (from, to) = (new_id(), new_id());
        let folder = make(&profile, &from).unwrap();
        std::fs::write(folder.join("board.json"), r#"{"version":1,"cards":[1]}"#).unwrap();
        std::fs::write(folder.join("thumb.png"), b"\x89PNG").unwrap();
        write_name(&profile, &from, "Trip").unwrap();
        std::fs::write(folder.join("arrived"), "waiting").unwrap();
        let files = folder.join("files");
        std::fs::create_dir(&files).unwrap();
        let stored = format!("{}.png", "c".repeat(64));
        std::fs::write(files.join(&stored), b"picture").unwrap();
        std::fs::write(files.join(".org.chromium.Chromium.XyZ123"), b"half").unwrap();
        std::fs::write(files.join(format!("{}.exe", "d".repeat(64))), b"not").unwrap();

        assert_eq!(duplicate(&profile, &from, &to).unwrap(), "Trip copy");
        let copy = dir(&profile).join(&to);
        let read = |path: PathBuf| std::fs::read(path).unwrap();
        assert_eq!(
            read(copy.join("board.json")),
            read(folder.join("board.json"))
        );
        assert_eq!(read(copy.join("thumb.png")), b"\x89PNG");
        let mut copied: Vec<String> = std::fs::read_dir(copy.join("files"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        copied.sort();
        assert_eq!(copied, [stored]);
        assert!(
            !copy.join("arrived").exists(),
            "the captures waiting were copied"
        );
        assert!(!dir(&profile).join(format!(".copy-{to}")).exists());
        let names: Vec<String> = list(&profile).into_iter().map(|board| board.name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Trip copy".to_owned()), "{names:?}");
        // The source is as it was.
        assert!(
            folder.join("arrived").is_file()
                && files.join(".org.chromium.Chromium.XyZ123").is_file()
        );

        // A second copy takes the next number; an unnamed Board's is "Board copy".
        let again = new_id();
        assert_eq!(duplicate(&profile, &from, &again).unwrap(), "Trip copy 2");
        let unnamed = new_id();
        make(&profile, &unnamed).unwrap();
        assert_eq!(
            duplicate(&profile, &unnamed, &new_id()).unwrap(),
            "Board copy"
        );

        // A Board that isn't there leaves its hidden folder, out of the list, and nothing else.
        let (gone, lost) = (new_id(), new_id());
        assert!(duplicate(&profile, &gone, &lost).is_err());
        assert!(dir(&profile).join(format!(".copy-{lost}")).is_dir());
        assert!(!dir(&profile).join(&lost).exists());
        assert_eq!(list(&profile).len(), 5);
        assert!(duplicate(&profile, "../x", &new_id()).is_err());
        std::fs::remove_dir_all(&profile).unwrap();
    }
}
