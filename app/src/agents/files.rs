//! What a task's assistant may do with files on this computer, by its level (#4101). One table, so
//! nothing else decides: the wall's rules and each assistant's start are made from it.

use std::path::{Path, PathBuf};

use super::tier::Tier;
use super::wall::Granted;

/// Where a file is: one the user attached, which the assistant gets as ilo's copy, or in the folder
/// the user granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Attached,
    Folder,
}

impl Place {
    pub const ALL: [Place; 2] = [Place::Attached, Place::Folder];
}

/// What is done to a file or a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Read,
    Search,
    /// A new file, folder or link, a second name for a file in the folder included.
    Make,
    /// A file's contents, written over in whole or in part.
    Change,
    Delete,
    /// A name taken to another place or another name.
    Move,
    /// One file moved over another, which loses the other's name.
    Replace,
}

impl Op {
    pub const ALL: [Op; 7] = [
        Op::Read,
        Op::Search,
        Op::Make,
        Op::Change,
        Op::Delete,
        Op::Move,
        Op::Replace,
    ];
}

/// Whether a task at `tier` may do `op` to a file in `place`: Look only reads, Work on pages also
/// makes and changes, Full access also deletes and moves, and attached files are only read.
pub fn allows(tier: Tier, place: Place, op: Op) -> bool {
    match (place, op) {
        (_, Op::Read | Op::Search) => true,
        (Place::Attached, _) => false,
        (Place::Folder, Op::Make | Op::Change) => tier >= Tier::Work,
        (Place::Folder, Op::Delete | Op::Move | Op::Replace) => tier == Tier::Full,
    }
}

/// The files the user gave a task: a folder they granted, ilo's copies of the files they attached,
/// both or neither. A task with any has its assistant run inside a wall (wall.rs).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Given {
    pub folder: Option<Granted>,
    /// Where ilo's copies of the attached files are.
    pub attached: Option<PathBuf>,
}

impl std::fmt::Debug for Given {
    /// Whether it has each and never where, as what holds one may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Given")
            .field("folder", &self.folder.is_some())
            .field("attached", &self.attached.is_some())
            .finish()
    }
}

/// What the user picked on the composer's `+` for the next task: files to attach, as the system's
/// panel gave them, and a folder they granted. Never more than the start's: a reply carries none.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Picked {
    pub files: Vec<PathBuf>,
    pub folder: Option<Granted>,
}

impl std::fmt::Debug for Picked {
    /// How many and never which, as a start's picks may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picked")
            .field("files", &self.files.len())
            .field("folder", &self.folder.is_some())
            .finish()
    }
}

/// The names a task's files are shown by in its conversation, and nowhere else: never a log, the
/// 30-day file, a notification or an agent's answer (#3656). Set and cleared where `Given` is.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Names {
    pub folder: Option<String>,
    pub files: Vec<String>,
}

impl std::fmt::Debug for Names {
    /// How many alone.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Names")
            .field("folder", &self.folder.is_some())
            .field("files", &self.files.len())
            .finish()
    }
}

impl Names {
    /// The names of what `picked` gives, as the system's panel shows each.
    pub fn of(picked: &Picked) -> Names {
        Names {
            folder: (picked.folder.as_ref()).map(|folder| shown_name(folder.root())),
            files: picked.files.iter().map(|file| shown_name(file)).collect(),
        }
    }

    /// The names left once `given` is all the task still has.
    pub fn kept_for(&self, given: &Given) -> Names {
        Names {
            folder: self.folder.clone().filter(|_| given.folder.is_some()),
            files: match given.attached.is_some() {
                true => self.files.clone(),
                false => Vec::new(),
            },
        }
    }
}

impl Given {
    pub fn any(&self) -> bool {
        self.folder.is_some() || self.attached.is_some()
    }

    /// Whether a task with these files is allowed the same at `from` and at `to`: a wall made
    /// for one is then the wall for the other. True of a task with none, and of one whose files
    /// are attached copies alone, which every level only reads.
    pub fn same_rules(&self, from: Tier, to: Tier) -> bool {
        let has = |place| match place {
            Place::Attached => self.attached.is_some(),
            Place::Folder => self.folder.is_some(),
        };
        (Place::ALL.into_iter().filter(|&place| has(place))).all(|place| {
            Op::ALL
                .iter()
                .all(|&op| allows(from, place, op) == allows(to, place, op))
        })
    }
}

/// What ilo never gives a task as its folder: the folders whose contents are the user's secrets,
/// their sign-ins or the system's, which `never_grantable` keeps a granted folder clear of.
pub struct Never {
    home: PathBuf,
    /// Its own folders, a profile's included.
    ilo: Vec<PathBuf>,
    /// The task's own, where its run keeps what ilo gives it.
    task: Option<PathBuf>,
}

impl Never {
    /// For this user and this build, with the tasks' folder where there is one.
    pub fn here(task: Option<PathBuf>) -> Never {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let name = include_str!("../../branding/name").trim();
        let ilo = [
            PathBuf::from(crate::profile_dir()),
            home.join("Library/Application Support").join(name),
            home.join("Library/Caches/browser-app"),
            home.join(".config").join(name),
            home.join(".cache").join(name),
            home.join(".cache/browser-app"),
        ];
        Never::of(home, ilo.to_vec(), task)
    }

    pub fn of(home: PathBuf, ilo: Vec<PathBuf>, task: Option<PathBuf>) -> Never {
        Never { home, ilo, task }
    }

    /// Folders refused as themselves only, as each holds the user's whole work: the disk's root,
    /// the home folder and a cloud drive's root. One inside them may be granted.
    fn themselves(&self) -> Vec<PathBuf> {
        let home = &self.home;
        let mut folders = vec![PathBuf::from("/"), home.clone()];
        folders.extend(
            [
                "Dropbox",
                "Google Drive",
                "OneDrive",
                "iCloud Drive",
                "pCloud Drive",
                "Box",
            ]
            .map(|drive| home.join(drive)),
        );
        folders
    }

    /// Folders refused with all inside them, and any folder holding one: the system's, other users'
    /// homes, the user's keys and sign-ins, the assistants' own, every
    /// browser's profiles, ilo's and the task's own.
    fn whole(&self) -> Vec<PathBuf> {
        let home = &self.home;
        let mut folders: Vec<PathBuf> = [
            // The Mac's system.
            "/System",
            "/Library",
            "/private",
            "/etc",
            "/usr",
            "/bin",
            "/sbin",
            "/var",
            "/cores",
            "/Network",
            // What these hold runs outside any wall: the Mac's apps, and what's added beside the
            // system.
            "/Applications",
            "/opt",
            // Linux's.
            "/proc",
            "/sys",
            "/dev",
            "/boot",
            "/root",
            "/run",
            "/lib",
            "/lib32",
            "/lib64",
            "/libx32",
            "/snap",
            "/lost+found",
        ]
        .map(PathBuf::from)
        .to_vec();
        folders.extend(
            [
                // The Mac's: its keychains, cloud drives, apps' data and every browser's profiles.
                "Library",
                // The user's own apps, which run outside any wall.
                "Applications",
                // Keys and sign-ins.
                ".ssh",
                ".aws",
                ".gnupg",
                ".password-store",
                ".local/share/keyrings",
                ".pki",
                ".netrc",
                ".docker",
                ".kube",
                ".config/gcloud",
                ".azure",
                ".config/gh",
                // The assistants' own.
                ".claude",
                ".claude.json",
                ".codex",
                ".config/claude",
                // Browsers' profiles on Linux.
                ".mozilla",
                ".config/google-chrome",
                ".config/chromium",
                ".config/BraveSoftware",
                ".config/microsoft-edge",
                ".config/vivaldi",
                ".config/opera",
                ".var/app",
            ]
            .map(|folder| home.join(folder)),
        );
        // Other users' homes: every folder beside this one's.
        if let Some(homes) = home.parent().filter(|homes| *homes != Path::new("/")) {
            folders.push(homes.to_path_buf());
        }
        folders.extend(self.ilo.iter().cloned());
        folders.extend(self.task.iter().cloned());
        folders
    }
}

/// Where volumes are mounted, and how deep a volume's root is under it.
const MOUNTS: [(&str, usize); 4] = [
    ("/Volumes", 1),
    ("/mnt", 1),
    ("/media", 2),
    ("/run/media", 2),
];

/// Whether ilo refuses `folder`, as the panel gave it, links resolved: one of `never`'s own, inside
/// one of its wholes, or holding one. A folder under the home folder that holds none of them, and
/// one on another disk, may be granted. The folder of homes is a whole but for the user's own.
pub fn never_grantable(folder: &Path, never: &Never) -> bool {
    if !folder.is_absolute() || folder.components().any(|part| part.as_os_str() == "..") {
        return true;
    }
    if never.themselves().iter().any(|own| own == folder) {
        return true;
    }
    // A volume's root, and the folder volumes are mounted in, are refused; a folder on the volume
    // isn't, though Linux mounts some under the system's /run.
    let mut on_a_volume = false;
    for (mounts, depth) in MOUNTS {
        if let Ok(under) = folder.strip_prefix(mounts) {
            if under.components().count() <= depth {
                return true;
            }
            on_a_volume = true;
        }
    }
    let home = &never.home;
    never.whole().iter().any(|whole| {
        let system_mount = on_a_volume && whole == Path::new("/run");
        let inside = folder.starts_with(whole) && !system_mount;
        let holds = whole.starts_with(folder);
        match home.parent() == Some(whole.as_path()) {
            true => holds || (inside && !folder.starts_with(home)),
            false => holds || inside,
        }
    })
}

/// The path the panel gave, its links resolved, as the folder is checked and granted by it.
pub fn resolved(picked: &Path) -> Option<PathBuf> {
    let resolved = std::fs::canonicalize(picked).ok()?;
    resolved.is_dir().then_some(resolved)
}

/// ilo's copies of the files the user attached, read only to the task: each into `into`, a folder
/// only this user may open, made for them inside the task's own. A name met twice gets a number.
/// Off the UI thread, as a file may be large. On a failure, what was copied is removed.
pub fn copy_attached(files: &[PathBuf], into: &Path) -> std::io::Result<()> {
    let copied = (|| {
        let mut folder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut folder, 0o700);
        folder.recursive(true).create(into)?;
        let mut names = std::collections::HashSet::new();
        for file in files {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let (stem, dot) = match name.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
                _ => (name.to_string(), String::new()),
            };
            let named = (1..)
                .map(|n| match n {
                    1 => format!("{stem}{dot}"),
                    n => format!("{stem} {n}{dot}"),
                })
                .find(|named| names.insert(named.clone()))
                .unwrap_or_default();
            let to = into.join(named);
            std::fs::copy(file, &to)?;
            #[cfg(unix)]
            std::fs::set_permissions(&to, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        }
        Ok(())
    })();
    if copied.is_err() {
        _ = std::fs::remove_dir_all(into);
    }
    copied
}

/// The name a folder or file is shown by: its last part, as the system's panel shows it.
pub fn shown_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

/// The files half's words (#4101), the lead's (rework-words-final.md), one constant each, here
/// alone.
pub mod words {
    /// The `+` menu's rows once this system can wall a task in with its files.
    pub const ADD_PAGE: &str = "Add Page";
    pub const ADD_FILES: &str = "Add Files…";
    pub const ADD_FOLDER: &str = "Add Folder…";

    /// X2, the attached files' chip on the quiet line.
    pub fn files(count: usize) -> String {
        match count {
            1 => "1 file".into(),
            count => format!("{count} files"),
        }
    }

    /// X3, the folder's one act, the same word as Settings'.
    pub const REVOKE: &str = "Revoke";

    /// Settings › Agents' heading for files and folders, which pairs with the level's "On the web".
    pub const ON_THIS_COMPUTER: &str = "On this computer";
    /// X9, under it on a system that can't wall a task in with them.
    pub const NOT_AVAILABLE: &str = "Files and folders aren't available on this system.";
    /// Settings › Agents' group of every folder a live task holds.
    pub const FOLDERS_IN_USE: &str = "Folders in use";

    /// X4, where the panel gave a folder ilo never gives.
    pub fn never_given(folder: &str) -> String {
        format!(
            "ilo doesn't give \u{201c}{folder}\u{201d} to a task. Choose a folder that holds only \
             what this task needs."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never() -> Never {
        Never::of(
            PathBuf::from("/Users/sam"),
            vec![PathBuf::from("/Users/sam/Library/Application Support/ilo")],
            Some(PathBuf::from("/srv/ilo/Tasks")),
        )
    }

    #[test]
    fn ilo_never_gives_a_folder_of_secrets_the_system_or_others() {
        let never = never();
        for folder in [
            "/",
            "/Users",
            "/Users/sam",
            "/Users/kim",
            "/Users/kim/Documents",
            "/Users/sam/.ssh",
            "/Users/sam/.ssh/keys",
            "/Users/sam/.aws",
            "/Users/sam/.gnupg",
            "/Users/sam/.claude",
            "/Users/sam/.codex",
            "/Users/sam/Library",
            "/Users/sam/Library/Keychains",
            "/Users/sam/Library/CloudStorage/Dropbox",
            "/Users/sam/Library/Application Support/Google/Chrome",
            "/Users/sam/.config/google-chrome",
            "/Users/sam/.mozilla/firefox",
            "/Users/sam/Dropbox",
            "/System/Library",
            "/private/tmp/work",
            "/etc",
            "/usr/local",
            "/var/log",
            "/proc/1",
            "/Volumes",
            "/Volumes/Backup",
            "/media/sam/Backup",
            "/run/media/sam/Backup",
            "/run/user/501",
            "/srv/ilo",
            "/srv/ilo/Tasks/task-1",
            "work",
            "/Users/sam/Documents/../.ssh",
            "/Applications",
            "/Applications/Utilities",
            "/Users/sam/Applications",
            "/opt",
            "/opt/homebrew",
        ] {
            assert!(never_grantable(Path::new(folder), &never), "{folder}");
        }
    }

    #[test]
    fn a_folder_of_the_users_own_work_may_be_given() {
        let never = never();
        for folder in [
            "/Users/sam/Documents",
            "/Users/sam/Documents/Trip",
            "/Users/sam/code/site",
            "/Users/sam/Dropbox/Trip",
            "/Volumes/Backup/Photos",
            "/media/sam/Backup/Photos",
            "/run/media/sam/Backup/Photos",
            "/srv/site",
        ] {
            assert!(!never_grantable(Path::new(folder), &never), "{folder}");
        }
    }

    #[test]
    fn attached_copies_are_the_users_alone_and_keep_each_name() {
        let dir = std::env::temp_dir().join(format!("ilo-attached-{}", std::process::id()));
        let picked = dir.join("picked");
        std::fs::create_dir_all(picked.join("b")).unwrap();
        std::fs::write(picked.join("notes.txt"), "one").unwrap();
        std::fs::write(picked.join("b/notes.txt"), "two").unwrap();
        let into = dir.join("task-1/Attached");
        let files = [picked.join("notes.txt"), picked.join("b/notes.txt")];
        copy_attached(&files, &into).unwrap();
        assert_eq!(
            std::fs::read_to_string(into.join("notes.txt")).unwrap(),
            "one"
        );
        assert_eq!(
            std::fs::read_to_string(into.join("notes 2.txt")).unwrap(),
            "two"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&into), 0o700);
            assert_eq!(mode(&into.join("notes.txt")), 0o600);
        }
        let gone = [picked.join("missing.txt")];
        assert!(copy_attached(&gone, &dir.join("task-2/Attached")).is_err());
        assert!(
            !dir.join("task-2/Attached").exists(),
            "a failed copy leaves nothing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_level_reads_and_searches_both_places() {
        for tier in Tier::ALL {
            for place in Place::ALL {
                assert!(allows(tier, place, Op::Read) && allows(tier, place, Op::Search));
            }
        }
    }

    #[test]
    fn attached_files_are_only_read_at_every_level() {
        for tier in Tier::ALL {
            for op in [Op::Make, Op::Change, Op::Delete, Op::Move, Op::Replace] {
                assert!(!allows(tier, Place::Attached, op), "{tier:?} {op:?}");
            }
        }
    }

    #[test]
    fn each_level_adds_to_the_one_below_in_the_folder() {
        let folder = |tier| {
            Op::ALL
                .into_iter()
                .filter(move |&op| allows(tier, Place::Folder, op))
        };
        let look: Vec<Op> = folder(Tier::Look).collect();
        let work: Vec<Op> = folder(Tier::Work).collect();
        let full: Vec<Op> = folder(Tier::Full).collect();
        assert_eq!(look, [Op::Read, Op::Search]);
        assert_eq!(work, [Op::Read, Op::Search, Op::Make, Op::Change]);
        assert_eq!(full, Op::ALL);
    }

    #[test]
    fn a_level_changes_the_rules_only_of_a_task_with_a_folder() {
        let folder = Given {
            folder: Some(Granted::for_test()),
            attached: Some(PathBuf::from("/nonexistent/attached")),
        };
        let attached = Given {
            folder: None,
            ..folder.clone()
        };
        for from in Tier::ALL {
            for to in Tier::ALL {
                assert!(Given::default().same_rules(from, to));
                assert!(attached.same_rules(from, to));
                assert_eq!(folder.same_rules(from, to), from == to, "{from:?} {to:?}");
            }
        }
    }

    #[test]
    fn files_printed_name_no_place() {
        let given = Given {
            folder: Some(Granted::for_test()),
            attached: Some(PathBuf::from("/nonexistent/Private Letters")),
        };
        let printed = format!("{given:?}");
        assert_eq!(printed, "Given { folder: true, attached: true }");
    }
}
