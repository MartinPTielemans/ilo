//! What a task's assistant may do with files on this computer, by its level (#4101). One table, so
//! nothing else decides: the wall's rules and each assistant's start are made from it.

use std::path::{Component, Path, PathBuf, Prefix};

use super::tier::Tier;
use super::wall::Granted;

#[cfg(unix)]
pub mod give;
pub mod read;
#[cfg(unix)]
pub mod resolve;

/// Where a file is: one the user attached, which the assistant gets as ilo's copy, in a folder
/// the user granted, or outside both, where only the product's own file tools reach (road B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Attached,
    Folder,
    Outside,
}

impl Place {
    pub const ALL: [Place; 3] = [Place::Attached, Place::Folder, Place::Outside];
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

/// Whether a task at `tier` may do `op` to a file in `place` without asking: Look only reads, Work
/// on pages also makes and changes, Full access also deletes and moves, and attached files are
/// only read. Outside the folders only Full access goes ahead; `decide` says what Work asks.
pub fn allows(tier: Tier, place: Place, op: Op) -> bool {
    match (place, op) {
        (Place::Outside, _) => tier == Tier::Full,
        (_, Op::Read | Op::Search) => true,
        (Place::Attached, _) => false,
        (Place::Folder, Op::Make | Op::Change) => tier >= Tier::Work,
        (Place::Folder, Op::Delete | Op::Move | Op::Replace) => tier == Tier::Full,
    }
}

/// How a folder of the thread's is listed: one it may change as its level allows, or one it only
/// reads (Settings › Folders' "Read" row).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Change,
    Read,
}

/// A folder of the thread's, as the user picked it in the system's panel: `Granted` is made by
/// the panel alone, so a folder reaches this list by the user's pick and no other road.
#[derive(Clone, PartialEq, Eq)]
pub struct Listed {
    root: PathBuf,
    pub row: Row,
}

impl std::fmt::Debug for Listed {
    /// Its row and never its path (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listed").field("row", &self.row).finish()
    }
}

impl Listed {
    pub fn of(picked: &Granted, row: Row) -> Listed {
        Listed {
            root: picked.root().to_path_buf(),
            row,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(test)]
    pub(crate) fn for_test(root: PathBuf, row: Row) -> Listed {
        Listed { root, row }
    }
}

/// Where a real path is, as the path floor found it, before the level is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// On the never list: no level reaches it.
    Never,
    /// A place the system guards with a prompt of its own, outside every folder the user picked,
    /// whose refusal asks for its folder. Never reached, so the system never asks.
    Guarded,
    Attached,
    Folder(Row),
    Outside,
}

/// Why a file act wasn't done, one for each refusal the words table has a line for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Never,
    Guarded,
    /// The thread's level doesn't allow it there; the lowest that would allow or ask it.
    Needs(Tier),
    /// A change to one of ilo's copies of the attached files, which are only read.
    Attached,
    /// A change, delete or move of a file with another name, which would reach it there too.
    OtherNames,
    /// The file isn't the one the user was asked about, found at the act.
    Changed,
    /// The path didn't resolve: a folder on the way is gone, or a file stands for one.
    Gone,
    /// A pipe, device or socket, or too many links in a row, never opened.
    NotOpened,
    /// A file only in the cloud, which a read never downloads.
    Cloud,
    /// A write over the cap.
    TooLarge,
    /// read_file on a file that isn't text.
    NotText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Only the user's press on the ask goes ahead (Work on pages, outside its folders).
    Ask,
    Refuse(Refused),
}

/// What a thread at `tier` does when its assistant asks for `op` at `at`, the one place a file act
/// is decided: the never list, then the place, then the level, then the folder's row. A file with
/// another name (`other_names`) is read, never changed, deleted or moved, at any level.
pub fn decide(tier: Tier, at: At, op: Op, other_names: bool) -> Decision {
    let reads = matches!(op, Op::Read | Op::Search);
    match at {
        At::Never => return Decision::Refuse(Refused::Never),
        At::Guarded => return Decision::Refuse(Refused::Guarded),
        At::Attached if !reads => return Decision::Refuse(Refused::Attached),
        _ => {}
    }
    let Some(decided) = by_level(tier, at, op) else {
        let needs = (Tier::ALL.into_iter())
            .find(|&above| above > tier && by_level(above, at, op).is_some())
            .unwrap_or(Tier::Full);
        return Decision::Refuse(Refused::Needs(needs));
    };
    match other_names && !reads && op != Op::Make {
        true => Decision::Refuse(Refused::OtherNames),
        false => decided,
    }
}

/// Whether `tier` goes ahead or asks for `op` at `at`, or None where the level refuses it.
fn by_level(tier: Tier, at: At, op: Op) -> Option<Decision> {
    let table = |place| allows(tier, place, op).then_some(Decision::Allow);
    match at {
        At::Never | At::Guarded => None,
        At::Attached => table(Place::Attached),
        At::Folder(Row::Change) => table(Place::Folder),
        At::Folder(Row::Read) => match (op, tier) {
            (Op::Read | Op::Search, _) => Some(Decision::Allow),
            (_, Tier::Look) => None,
            // A change in a folder it only reads is asked as one outside it would be.
            (_, Tier::Work) => outside(tier, op),
            (_, Tier::Full) => table(Place::Folder),
        },
        At::Outside => outside(tier, op),
    }
}

/// Outside the folders: Full access goes ahead, Work on pages asks to read, make or change and
/// refuses the rest, and Look only refuses all.
fn outside(tier: Tier, op: Op) -> Option<Decision> {
    match (tier, op) {
        (Tier::Full, _) => Some(Decision::Allow),
        (Tier::Work, Op::Read | Op::Search | Op::Make | Op::Change) => Some(Decision::Ask),
        _ => None,
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
}

/// What ilo never gives a task as its folder: the folders whose contents are the user's secrets,
/// their sign-ins or the system's, which `never_grantable` keeps a granted folder clear of.
pub struct Never {
    home: PathBuf,
    /// Its own folders, a profile's included.
    ilo: Vec<PathBuf>,
    /// The task's own, where its run keeps what ilo gives it.
    task: Option<PathBuf>,
    /// The cloud drives' roots where the system names them, as Windows does OneDrive's, whose
    /// name may be a business's.
    clouds: Vec<PathBuf>,
    /// The places the system guards with a prompt of its own (the Mac's Desktop, Documents,
    /// Downloads, cloud drives and volumes), reached only under a folder the user picked.
    guarded: Vec<PathBuf>,
    /// Whether the system's names are the same whatever their case, as the Mac's and Windows'
    /// are by default, so the never list is read so too. Refusing a name a case-sensitive disk
    /// holds apart only refuses more.
    folded: bool,
}

impl Never {
    /// For this user and this build, with the tasks' folder where there is one.
    pub fn here(task: Option<PathBuf>) -> Never {
        let named = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home = PathBuf::from(std::env::var_os(named).unwrap_or_default());
        let name = include_str!("../../branding/name").trim();
        let ilo = [
            PathBuf::from(crate::profile_dir()),
            home.join("Library/Application Support").join(name),
            home.join("Library/Caches/browser-app"),
            home.join(".config").join(name),
            home.join(".cache").join(name),
            home.join(".cache/browser-app"),
        ];
        let mut never = Never::of(home, ilo.to_vec(), task);
        never.clouds = ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"]
            .into_iter()
            .filter_map(std::env::var_os)
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from)
            .collect();
        if cfg!(target_os = "macos") {
            never.guarded = never.mac_guarded();
        }
        never
    }

    pub fn of(home: PathBuf, ilo: Vec<PathBuf>, task: Option<PathBuf>) -> Never {
        Never {
            home,
            ilo,
            task,
            clouds: Vec::new(),
            guarded: Vec::new(),
            folded: cfg!(any(windows, target_os = "macos")),
        }
    }

    /// A path as the never list compares it.
    fn fold(&self, path: &Path) -> PathBuf {
        match self.folded {
            true => PathBuf::from(path.to_string_lossy().to_lowercase()),
            false => key(path),
        }
    }

    /// The Mac's places that only a system prompt opens to an app the user didn't pick them in.
    fn mac_guarded(&self) -> Vec<PathBuf> {
        let mut guarded: Vec<PathBuf> = [
            "Desktop",
            "Documents",
            "Downloads",
            "Library/Mobile Documents",
            "Library/CloudStorage",
        ]
        .map(|folder| self.home.join(folder))
        .to_vec();
        guarded.push(PathBuf::from("/Volumes"));
        guarded
    }

    /// The never roots the path floor compares by identity as it walks, so a second name for one
    /// (its case changed, a firmlink, a bind mount) is refused as the first is: every whole but the
    /// two a path goes on through, the folder of homes, which holds the user's own, and the
    /// system's /run, which holds Linux's volumes.
    pub fn by_identity(&self) -> Vec<PathBuf> {
        let homes = self.home.parent().filter(|homes| *homes != Path::new("/"));
        (self.whole().into_iter())
            .filter(|whole| Some(whole.as_path()) != homes && whole != Path::new("/run"))
            .collect()
    }

    fn library(&self) -> PathBuf {
        self.home.join("Library")
    }

    /// Where `real`, a path the path floor resolved, is for a thread with `folders` and ilo's
    /// copies of its attached files at `attached`. The never list wins over every folder, and
    /// `~/Library` is never reached but inside a folder the user picked there.
    pub fn at(&self, real: &Path, folders: &[Listed], attached: Option<&Path>) -> At {
        let folded = &self.fold(real);
        let real = &key(real);
        let inside = |root: &Path| real.starts_with(key(root));
        let folded_in = |root: &Path| folded.starts_with(self.fold(root));
        if attached.is_some_and(inside) {
            return At::Attached;
        }
        let never = !real.is_absolute() || real.components().any(|part| part.as_os_str() == "..");
        if never || self.in_whole(real) {
            return At::Never;
        }
        // The deepest folder holding it says its row.
        let folder = (folders.iter().filter(|folder| inside(folder.root())))
            .max_by_key(|folder| folder.root().components().count());
        match folder {
            Some(folder) => At::Folder(folder.row),
            // iCloud Drive and the cloud drives are in `~/Library`, guarded by the system, and
            // reached by a folder the user adds there, so the rest of it is never.
            None if self.guarded.iter().any(|place| folded_in(place)) => At::Guarded,
            None if folded_in(&self.library()) => At::Never,
            None => At::Outside,
        }
    }

    /// The folder the ask's middle button would allow for a file in `parent`, if it's one ilo
    /// would grant: never inside `~/Library`, nor one `never_grantable` refuses.
    pub fn ask_folder(&self, parent: &Path) -> Option<PathBuf> {
        let library = self.fold(parent).starts_with(self.fold(&self.library()));
        let refused = library || never_grantable(parent, self);
        (!refused).then(|| parent.to_path_buf())
    }

    /// Whether `path`, in `key`'s form, is inside one of the wholes: another user's home is, the
    /// user's own isn't, and a volume Linux mounts under the system's /run isn't.
    fn in_whole(&self, path: &Path) -> bool {
        if self.in_photos(path) {
            return true;
        }
        let (path, home) = (&self.fold(path), &self.fold(&self.home));
        let on_a_volume = MOUNTS.iter().any(|(mounts, _)| path.starts_with(mounts));
        self.whole().iter().any(|whole| {
            let whole = &self.fold(whole);
            let system_mount = on_a_volume && whole == Path::new("/run");
            let inside = path.starts_with(whole) && !system_mount;
            match home.parent() == Some(whole.as_path()) {
                true => inside && !path.starts_with(home),
                false => inside,
            }
        })
    }

    /// Whether `path` is a photo library under `~/Pictures` or inside one: a bundle the Photos app
    /// keeps, under whatever name the user gave it.
    fn in_photos(&self, path: &Path) -> bool {
        let pictures = self.fold(&self.home.join("Pictures"));
        let Ok(under) = self
            .fold(path)
            .strip_prefix(&pictures)
            .map(Path::to_path_buf)
        else {
            return false;
        };
        under.components().any(|part| {
            let name = part.as_os_str().to_string_lossy();
            name.to_lowercase().ends_with(".photoslibrary")
        })
    }

    /// Folders refused as themselves only, as each holds the user's whole work: the home folder
    /// and a cloud drive's root (a disk's root is refused by `never_grantable`). One inside them
    /// may be granted.
    fn themselves(&self) -> Vec<PathBuf> {
        let home = &self.home;
        let mut folders = vec![home.clone()];
        folders.extend(
            [
                "Dropbox",
                "Google Drive",
                "OneDrive",
                "iCloud Drive",
                "pCloud Drive",
                "Box",
                // The Mac's: every app's cloud folders, iCloud Drive's root, and where the cloud
                // drives keep theirs.
                "Library/Mobile Documents",
                "Library/Mobile Documents/com~apple~CloudDocs",
                "Library/CloudStorage",
            ]
            .map(|drive| home.join(drive)),
        );
        folders.extend(self.clouds.iter().cloned());
        folders
    }

    /// Folders refused with all inside them, and any folder holding one: the system's, other users'
    /// homes, the user's keys and sign-ins, the assistants' own, every browser's profiles, every
    /// app's settings, ilo's and the task's own. `~/Library` isn't one, as a folder the user
    /// picked inside it is reached; `at` keeps the rest of it out.
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
                // The Mac's, inside `~/Library`, refused even in a folder the user picked there:
                // keychains and sign-ins, the system's own stores, what runs at login, other
                // apps' walled data, password managers', every browser's profiles and the
                // assistants' own.
                "Library/Keychains",
                "Library/Preferences",
                "Library/Cookies",
                "Library/HTTPStorages",
                "Library/Accounts",
                "Library/Mail",
                "Library/Messages",
                "Library/Safari",
                "Library/Calendars",
                "Library/Suggestions",
                "Library/Biome",
                "Library/LaunchAgents",
                "Library/Containers",
                "Library/Group Containers",
                // Each named as well, so each is refused by its own name and not only as part of
                // other apps' walled data.
                "Library/Containers/com.apple.Notes",
                "Library/Group Containers/group.com.apple.notes",
                "Library/Containers/com.apple.mail",
                "Library/Containers/com.apple.iChat",
                "Library/Containers/com.apple.AddressBook",
                "Library/Containers/com.apple.Safari",
                "Library/Application Support/com.apple.TCC",
                "Library/Application Support/AddressBook",
                "Library/Application Support/CallHistoryDB",
                "Library/Application Support/Knowledge",
                "Library/Application Support/MobileSync",
                "Library/Application Support/1Password",
                "Library/Application Support/Bitwarden",
                "Library/Application Support/Google",
                "Library/Application Support/Chromium",
                "Library/Application Support/Firefox",
                "Library/Application Support/Arc",
                "Library/Application Support/BraveSoftware",
                "Library/Application Support/Microsoft Edge",
                "Library/Application Support/Vivaldi",
                "Library/Application Support/com.operasoftware.Opera",
                "Library/Application Support/Claude",
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
                ".docker/config.json",
                // Shells' and tools' histories, which hold what was typed, passwords among it.
                ".zsh_history",
                ".zhistory",
                ".zsh_sessions",
                ".bash_history",
                ".bash_sessions",
                ".sh_history",
                ".history",
                ".local/share/fish/fish_history",
                ".python_history",
                ".node_repl_history",
                ".psql_history",
                ".mysql_history",
                ".sqlite_history",
                ".lesshst",
                ".irb_history",
                ".kube",
                ".config/gcloud",
                ".azure",
                ".config/gh",
                // The assistants' own.
                ".claude",
                ".claude.json",
                ".codex",
                ".config/claude",
                // Every app's settings and data on Linux, sign-ins among them.
                ".config",
                ".local/share",
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
        #[cfg(windows)]
        folders.extend(windows(home));
        folders
    }
}

/// Windows's own: its tree, the programs for every user and their data, and the user's app data,
/// which holds every app's settings and sign-ins, every browser's profiles and ilo's own. Each
/// where the system says it is, and where it is by default.
#[cfg(windows)]
fn windows(home: &Path) -> Vec<PathBuf> {
    let said = [
        "SystemRoot",
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "ALLUSERSPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
    ];
    let mut folders: Vec<PathBuf> = said
        .into_iter()
        .filter_map(std::env::var_os)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .collect();
    folders.extend(
        [
            r"C:\Windows",
            r"C:\Program Files",
            r"C:\Program Files (x86)",
            r"C:\ProgramData",
        ]
        .map(PathBuf::from),
    );
    folders.push(home.join("AppData"));
    folders
}

/// A path as `never_grantable` compares it: as it is, but on Windows, whose names are the same
/// whatever their case, in lower case.
fn key(path: &Path) -> PathBuf {
    match cfg!(windows) {
        true => PathBuf::from(path.to_string_lossy().to_lowercase()),
        false => path.to_path_buf(),
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
    // A disk's root: `/`, and on Windows a drive's or a network share's.
    if folder.parent().is_none() {
        return true;
    }
    // On Windows, a disk named other than as a drive or a share (a volume, a device, the verbatim
    // forms) can reach any folder by a name none of `never`'s match.
    if let Some(Component::Prefix(prefix)) = folder.components().next()
        && !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::UNC(..))
    {
        return true;
    }
    let folder = &key(folder);
    if never.themselves().iter().any(|own| key(own) == *folder) {
        return true;
    }
    // Each cloud drive the system keeps in its own folder has its root there.
    let drives = key(&never.library().join("CloudStorage"));
    if folder.parent() == Some(drives.as_path()) {
        return true;
    }
    // A volume's root, and the folder volumes are mounted in, are refused.
    let volume_root = MOUNTS.iter().any(|(mounts, depth)| {
        (folder.strip_prefix(mounts)).is_ok_and(|under| under.components().count() <= *depth)
    });
    volume_root
        || never.in_whole(folder)
        || (never.whole().iter()).any(|whole| key(whole).starts_with(folder))
}

/// The path the panel gave, its links resolved, as the folder is checked and granted by it.
pub fn resolved(picked: &Path) -> Option<PathBuf> {
    let resolved = plain(std::fs::canonicalize(picked).ok()?);
    resolved.is_dir().then_some(resolved)
}

/// A resolved path in the form the user and `Never` know it. Windows resolves to its `\\?\` form,
/// which names a drive as `\\?\C:\` and a share as `\\?\UNC\server\share`; elsewhere it's as it is.
fn plain(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let mut parts = path.components();
        let head = match parts.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(drive) => format!("{}:\\", drive as char),
                Prefix::VerbatimUNC(server, share) => format!(
                    "\\\\{}\\{}\\",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                ),
                _ => return path,
            },
            _ => return path,
        };
        let mut plain = PathBuf::from(head);
        plain.extend(parts.filter(|part| matches!(part, Component::Normal(_))));
        return plain;
    }
    #[cfg(not(windows))]
    path
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

    /// The attached files' chip on the quiet line.
    pub fn files(count: usize) -> String {
        match count {
            1 => "1 file".into(),
            count => format!("{count} files"),
        }
    }

    /// The folder's one act, the same word as Settings'.
    pub const REVOKE: &str = "Revoke";

    /// Settings › Agents' heading for files and folders, which pairs with the level's "On the web".
    pub const ON_THIS_COMPUTER: &str = "On this computer";
    /// Under it on a system that can't wall a task in with them.
    pub const NOT_AVAILABLE: &str = "Files and folders aren't available on this system.";
    /// Settings › Agents' group of every folder a live task holds.
    pub const FOLDERS_IN_USE: &str = "Folders in use";

    use super::{Op, Refused};
    use crate::product::PRODUCT;

    /// A file's name as a step names it: in quotes, cut in the middle past 32 characters.
    pub fn quoted(name: &str) -> String {
        const MOST: usize = 32;
        let chars: Vec<char> = name.chars().collect();
        let name: String = match chars.len() > MOST {
            true => {
                let head = (MOST - 1) / 2;
                let tail = MOST - 1 - head;
                let (start, end) = (&chars[..head], &chars[chars.len() - tail..]);
                start.iter().chain(['…'].iter()).chain(end).collect()
            }
            false => name.into(),
        };
        format!("\u{201c}{name}\u{201d}")
    }

    /// A refused step's line in the thread's trace, its failed line and the reason. Only the
    /// line asking for the file's folder has a button, `ADD_FOLDER`, which opens the system's
    /// folder panel at that folder.
    pub fn refused(step: Step, why: Refused) -> String {
        let makes = matches!(step, Step::MakeFile(_) | Step::MakeFolder(_));
        let reason = match why {
            Refused::Never => format!("{PRODUCT} keeps it from threads"),
            Refused::Changed => format!("it changed after {PRODUCT} asked you"),
            Refused::Guarded => "add its folder first".into(),
            Refused::Needs(level) => format!("needs {}", level.label()),
            Refused::Attached => "attached files can only be read".into(),
            Refused::OtherNames => "it's linked to another file".into(),
            // What's made isn't there yet, so what's missing is the folder it goes in.
            Refused::Gone if makes => "its folder isn't there".into(),
            Refused::Gone => "it isn't there".into(),
            Refused::NotOpened => format!("{PRODUCT} doesn't open this kind of file"),
            Refused::Cloud => "it isn't downloaded".into(),
            Refused::TooLarge => "it's over 1 MB".into(),
            Refused::NotText => "it isn't text".into(),
        };
        format!("{}: {reason}", failed(step))
    }

    /// A step's failed line, with no reason the table names.
    pub fn failed(step: Step) -> String {
        let q = quoted;
        match step {
            Step::Read(file) => format!("Couldn't read {}", q(file)),
            Step::List(folder) => format!("Couldn't look in {}", q(folder)),
            Step::Search(folder) => format!("Couldn't search in {}", q(folder)),
            Step::MakeFile(name) | Step::MakeFolder(name) => format!("Couldn't make {}", q(name)),
            Step::Change(file) => format!("Couldn't change {}", q(file)),
            Step::Move { file, .. } => format!("Couldn't move {}", q(file)),
            Step::Rename { old, .. } => format!("Couldn't rename {}", q(old)),
            Step::Trash(file) => format!("Couldn't delete {}", q(file)),
        }
    }

    /// What the assistant is told of a refused act. It never names a level: only the user
    /// changes it.
    pub fn told(op: Op, why: Refused) -> String {
        match why {
            Refused::Never => {
                format!("Not done: {PRODUCT} never gives this path to a task. Go on without it.")
            }
            Refused::Changed if matches!(op, Op::Read | Op::Search) => {
                "Not done: the file changed after the user was asked. Try reading it again if \
                 you still need it."
                    .into()
            }
            Refused::Changed => "Not done: the file changed after the user was asked. Read it \
                                 again before trying again."
                .into(),
            Refused::Guarded => "Not done: the user has to add this file's folder to the task \
                                 first. Go on without it, or ask them to add it."
                .into(),
            Refused::Needs(_) => "Not done: this needs more access than this task has, and only \
                                  the user can change it. Say what you couldn't do and carry on \
                                  without it."
                .into(),
            Refused::Attached => {
                "Not done: an attached file is only for reading. Go on without changing it.".into()
            }
            Refused::OtherNames => "Not done: this file also exists under another name \
                                    elsewhere, so it can only be read. Go on without changing it."
                .into(),
            Refused::Gone => {
                "Not done: there's nothing at this path. List its folder to find the right name."
                    .into()
            }
            Refused::NotOpened => format!(
                "Not done: this isn't a regular file or folder, and {PRODUCT} doesn't open it."
            ),
            Refused::Cloud => format!(
                "Not done: this file isn't downloaded to this computer, and {PRODUCT} doesn't \
                 download it. Ask the user to download it, or go on without it."
            ),
            Refused::TooLarge => "Not done: one write can be at most 1 MB.".into(),
            Refused::NotText => {
                "Not done: read_file reads only text, and this file isn't text.".into()
            }
        }
    }

    /// A list cut at its 500 entries: the done line's tail, and what's added to the answer.
    pub fn listed_first(folder: &str) -> String {
        format!("Looked in {} \u{b7} first 500", quoted(folder))
    }
    pub const LISTED_FIRST_TOLD: &str =
        "Only the first 500 entries are listed. Search the folder to find something specific.";

    /// A search stopped at its limit.
    pub fn searched_early(folder: &str) -> String {
        format!("Searched in {} \u{b7} stopped early", quoted(folder))
    }
    pub const SEARCHED_EARLY_TOLD: &str = "The search stopped at its limit before it covered the \
                                           whole folder. Search a smaller folder or a narrower \
                                           pattern.";

    /// What a file tool's step did, by the names its lines take.
    #[derive(Clone, Copy, Debug)]
    pub enum Step<'a> {
        Read(&'a str),
        List(&'a str),
        Search(&'a str),
        MakeFile(&'a str),
        /// A file written over or edited.
        Change(&'a str),
        MakeFolder(&'a str),
        /// To another folder, `to`.
        Move {
            file: &'a str,
            to: &'a str,
        },
        /// In the same folder.
        Rename {
            old: &'a str,
            new: &'a str,
        },
        Trash(&'a str),
    }

    /// A file tool's step line while it's doing and once it's done; a failed one is `failed` or
    /// `refused`.
    pub fn step(step: Step, done: bool) -> String {
        let q = quoted;
        match (step, done) {
            (Step::Read(file), false) => format!("Reading {}", q(file)),
            (Step::Read(file), true) => format!("Read {}", q(file)),
            (Step::List(folder), false) => format!("Looking in {}", q(folder)),
            (Step::List(folder), true) => format!("Looked in {}", q(folder)),
            (Step::Search(folder), false) => format!("Searching in {}", q(folder)),
            (Step::Search(folder), true) => format!("Searched in {}", q(folder)),
            (Step::MakeFile(name) | Step::MakeFolder(name), false) => format!("Making {}", q(name)),
            (Step::MakeFile(name) | Step::MakeFolder(name), true) => format!("Made {}", q(name)),
            (Step::Change(file), false) => format!("Changing {}", q(file)),
            (Step::Change(file), true) => format!("Changed {}", q(file)),
            (Step::Move { file, .. }, false) => format!("Moving {}", q(file)),
            (Step::Move { file, to }, true) => format!("Moved {} to {}", q(file), q(to)),
            (Step::Rename { old, .. }, false) => format!("Renaming {}", q(old)),
            (Step::Rename { old, new }, true) => format!("Renamed {} to {}", q(old), q(new)),
            (Step::Trash(file), false) => format!("Moving {} to the {TRASH}", q(file)),
            (Step::Trash(file), true) => format!("Moved {} to the {TRASH}", q(file)),
        }
    }

    /// Where a delete goes, as each system names it.
    const TRASH: &str = match cfg!(windows) {
        true => "Recycle Bin",
        false => "Trash",
    };

    /// A change past the undo's cap: its done line says it can't be undone. A delete has none,
    /// since it goes to the Trash.
    pub fn past_undo(step: Step) -> String {
        format!("{} \u{b7} can't be undone", self::step(step, true))
    }

    /// Where the panel gave a folder ilo never gives.
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

    // Each system's own paths, so each list is read as that system reads it: on Windows, /Users/sam
    // is a path on whichever drive is current, and refused as one.
    #[cfg(not(windows))]
    fn never() -> Never {
        Never::of(
            PathBuf::from("/Users/sam"),
            vec![PathBuf::from("/Users/sam/Library/Application Support/ilo")],
            Some(PathBuf::from("/srv/ilo/Tasks")),
        )
    }

    #[cfg(windows)]
    fn never() -> Never {
        let mut never = Never::of(
            PathBuf::from(r"C:\Users\sam"),
            vec![PathBuf::from(r"C:\Users\sam\AppData\Local\ilo\User Data")],
            Some(PathBuf::from(r"D:\ilo\Tasks")),
        );
        never.clouds = vec![PathBuf::from(r"C:\Users\sam\OneDrive - Lakeside Studio")];
        never
    }

    #[cfg(not(windows))]
    const REFUSED: &[&str] = &[
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
        "/Users/sam/Library/Application Support",
        "/Users/sam/Library/LaunchAgents",
        "/Users/sam/Library/Group Containers/2BUA8C4S2C.com.1password",
        "/Users/sam/Library/Containers/com.apple.mail",
        "/Users/sam/Library/Mobile Documents",
        "/Users/sam/Library/Mobile Documents/com~apple~CloudDocs",
        "/Users/sam/Library/CloudStorage",
        "/Users/sam/Library/CloudStorage/GoogleDrive-sam",
        "/Users/sam/.config",
        "/Users/sam/.config/fish",
        "/Users/sam/.local/share",
        "/Users/sam/.local/share/fonts",
    ];

    #[cfg(windows)]
    const REFUSED: &[&str] = &[
        r"C:\",
        r"D:\",
        r"\\nas\share",
        r"\\nas\share\",
        r"\Users\sam\Documents",
        "/Users/sam/Documents",
        r"C:Users\sam\Documents",
        r"C:\Windows",
        r"C:\WINDOWS\System32",
        r"c:\windows\Temp",
        r"C:\Program Files",
        r"C:\Program Files\Git",
        r"C:\Program Files (x86)",
        r"C:\ProgramData",
        r"C:\ProgramData\Microsoft",
        r"C:\Users",
        r"C:\Users\kim",
        r"C:\Users\kim\Documents",
        r"C:\Users\sam",
        r"c:\users\SAM",
        r"C:\Users\sam\AppData",
        r"C:\Users\sam\AppData\Roaming",
        r"C:\Users\sam\AppData\Local\Google\Chrome\User Data",
        r"C:\Users\sam\appdata\local\ilo",
        r"C:\Users\sam\.ssh",
        r"C:\Users\sam\.claude",
        r"C:\Users\sam\.codex",
        r"C:\Users\sam\OneDrive",
        r"C:\Users\sam\Dropbox",
        r"D:\ilo",
        r"D:\ilo\Tasks\task-1",
        r"d:\ILO\tasks",
        "work",
        r"C:\Users\sam\Documents\..\.ssh",
        r"C:\Users\sam\OneDrive - Lakeside Studio",
        r"c:\users\sam\onedrive - lakeside studio",
        r"\\?\Volume{0c5a3f4e-1d2b-4c6d-8e9f-0a1b2c3d4e5f}\Photos",
        r"\\.\C:\Users\sam\Documents",
        r"\\?\C:\Users\sam\Documents",
        r"\\?\UNC\nas\share\Photos",
        r"\\?\Documents",
    ];

    #[cfg(not(windows))]
    const GIVEN: &[&str] = &[
        "/Users/sam/Documents",
        "/Users/sam/Documents/Trip",
        "/Users/sam/code/site",
        "/Users/sam/Dropbox/Trip",
        "/Volumes/Backup/Photos",
        "/media/sam/Backup/Photos",
        "/run/media/sam/Backup/Photos",
        "/srv/site",
        "/Users/sam/Library/Mobile Documents/com~apple~CloudDocs/Trip",
        "/Users/sam/Library/CloudStorage/Dropbox/Trip",
        "/Users/sam/Library/Fonts",
        "/Users/sam/.local/bin",
    ];

    #[cfg(windows)]
    const GIVEN: &[&str] = &[
        r"C:\Users\sam\Documents",
        r"C:\Users\sam\Documents\Trip",
        r"c:\users\sam\Documents",
        r"C:\Users\sam\code\site",
        r"C:\Users\sam\OneDrive\Trip",
        r"C:\Users\sam\OneDrive - Lakeside Studio\Plans",
        r"D:\Photos",
        r"\\nas\share\Photos",
    ];

    #[test]
    fn ilo_never_gives_a_folder_of_secrets_the_system_or_others() {
        let never = never();
        for folder in REFUSED {
            assert!(never_grantable(Path::new(folder), &never), "{folder}");
        }
    }

    #[test]
    fn a_folder_of_the_users_own_work_may_be_given() {
        let never = never();
        for folder in GIVEN {
            assert!(!never_grantable(Path::new(folder), &never), "{folder}");
        }
    }

    /// A kept thread's card folder, under the profile's, is never given, nor a folder holding it:
    /// as this build names it, and on the made-up profile.
    #[test]
    fn the_kept_threads_folder_is_never_given() {
        use crate::agents::kept::cards::FOLDER;
        let profile = PathBuf::from(crate::profile_dir());
        let here = Never::here(None);
        for folder in [profile.join(FOLDER), profile.clone()] {
            assert!(never_grantable(&folder, &here), "{}", folder.display());
        }
        let never = never();
        let profile = never.ilo[0].clone();
        let holding = profile.parent().unwrap().to_path_buf();
        for folder in [
            profile.join(FOLDER).join("0123"),
            profile.join(FOLDER),
            holding,
        ] {
            assert!(never_grantable(&folder, &never), "{}", folder.display());
        }
    }

    #[test]
    fn a_disks_root_resolves_to_a_root_and_is_refused() {
        let root = resolved(Path::new("/")).unwrap();
        assert_eq!(root.parent(), None, "{}", root.display());
        assert!(never_grantable(&root, &never()));
    }

    #[test]
    fn a_resolved_folder_is_named_as_the_user_knows_it() {
        let here = resolved(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(here.is_absolute(), "{}", here.display());
        assert!(
            !here.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            here.display()
        );
        assert_eq!(resolved(&here).as_deref(), Some(here.as_path()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_resolved_forms_are_plain() {
        let plain = |path: &str| super::plain(PathBuf::from(path));
        assert_eq!(plain(r"\\?\C:\"), PathBuf::from(r"C:\"));
        assert_eq!(plain(r"\\?\C:\Users\sam"), PathBuf::from(r"C:\Users\sam"));
        assert_eq!(plain(r"\\?\UNC\nas\share"), PathBuf::from(r"\\nas\share\"));
        assert_eq!(
            plain(r"\\?\UNC\nas\share\Photos"),
            PathBuf::from(r"\\nas\share\Photos")
        );
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
            for place in [Place::Attached, Place::Folder] {
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

    const OPS: [Op; 7] = Op::ALL;

    /// Each cell of `decide` for a file with one name, as one letter per op in `Op::ALL`'s order:
    /// A goes ahead, ? asks; refused, N by the never list, G as the system's, a as an attached
    /// file, and w or f as needing Work on pages or Full access.
    #[test]
    fn decide_reads_never_place_level_and_row_in_turn() {
        let read = At::Folder(Row::Read);
        let folder = At::Folder(Row::Change);
        let cells = [
            (At::Never, ["NNNNNNN", "NNNNNNN", "NNNNNNN"]),
            (At::Guarded, ["GGGGGGG", "GGGGGGG", "GGGGGGG"]),
            (At::Attached, ["AAaaaaa", "AAaaaaa", "AAaaaaa"]),
            (folder, ["AAwwfff", "AAAAfff", "AAAAAAA"]),
            (read, ["AAwwfff", "AA??fff", "AAAAAAA"]),
            (At::Outside, ["wwwwfff", "????fff", "AAAAAAA"]),
        ];
        for (at, by_tier) in cells {
            for (tier, row) in Tier::ALL.into_iter().zip(by_tier) {
                let got: String = (OPS.iter())
                    .map(|&op| match decide(tier, at, op, false) {
                        Decision::Allow => 'A',
                        Decision::Ask => '?',
                        Decision::Refuse(Refused::Needs(Tier::Work)) => 'w',
                        Decision::Refuse(Refused::Needs(Tier::Full)) => 'f',
                        Decision::Refuse(Refused::Never) => 'N',
                        Decision::Refuse(Refused::Guarded) => 'G',
                        Decision::Refuse(Refused::Attached) => 'a',
                        Decision::Refuse(other) => panic!("{other:?}"),
                    })
                    .collect();
                assert_eq!(got, row, "{at:?} at {tier:?}");
            }
        }
    }

    /// A refusal by the level names the lowest level that would go ahead or ask, so going to it
    /// does what the line says.
    #[test]
    fn a_level_refusal_names_the_lowest_level_that_would_do_it() {
        let places = [At::Folder(Row::Change), At::Folder(Row::Read), At::Outside];
        for tier in Tier::ALL {
            for at in places {
                for op in OPS {
                    let Decision::Refuse(Refused::Needs(needs)) = decide(tier, at, op, false)
                    else {
                        continue;
                    };
                    assert!(needs > tier, "{at:?} {tier:?} {op:?}");
                    assert!(
                        !matches!(decide(needs, at, op, false), Decision::Refuse(_)),
                        "{at:?} {tier:?} {op:?}"
                    );
                    for between in Tier::ALL.into_iter().filter(|t| *t > tier && *t < needs) {
                        assert!(matches!(
                            decide(between, at, op, false),
                            Decision::Refuse(_)
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn a_file_with_another_name_is_read_and_never_changed_deleted_or_moved() {
        let places = [At::Folder(Row::Change), At::Folder(Row::Read), At::Outside];
        for tier in Tier::ALL {
            for at in places {
                for op in OPS {
                    let one = decide(tier, at, op, false);
                    let two = decide(tier, at, op, true);
                    let writes = !matches!(op, Op::Read | Op::Search | Op::Make);
                    match (writes, one) {
                        (true, Decision::Allow | Decision::Ask) => {
                            assert_eq!(
                                two,
                                Decision::Refuse(Refused::OtherNames),
                                "{at:?} {tier:?}"
                            )
                        }
                        _ => assert_eq!(two, one, "{at:?} {tier:?} {op:?}"),
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    fn listed(root: &str, row: Row) -> Listed {
        Listed {
            root: PathBuf::from(root),
            row,
        }
    }

    #[cfg(not(windows))]
    fn on_a_mac() -> Never {
        let mut never = never();
        never.guarded = never.mac_guarded();
        never.folded = true;
        never
    }

    #[cfg(not(windows))]
    #[test]
    fn the_library_is_never_reached_but_in_a_folder_picked_there() {
        let never = on_a_mac();
        let fonts = [listed("/Users/sam/Library/Fonts", Row::Change)];
        let at = |path: &str, folders: &[Listed]| never.at(Path::new(path), folders, None);
        for path in [
            "/Users/sam/Library",
            "/Users/sam/Library/Preferences/com.example.plist",
            "/Users/sam/library/keychains/login.keychain-db",
        ] {
            assert_ne!(at(path, &fonts), At::Outside, "{path}");
            assert_ne!(at(path, &[]), At::Outside, "{path}");
        }
        assert_eq!(
            at("/Users/sam/Library/Preferences/a.plist", &fonts),
            At::Never
        );
        assert_eq!(
            at("/Users/sam/Library/Fonts/Inter.ttf", &fonts),
            At::Folder(Row::Change)
        );
        // Listed for its own sake: never, even inside a folder the user picked.
        for (picked, path) in [
            (
                "/Users/sam/Library",
                "/Users/sam/Library/Keychains/login.keychain-db",
            ),
            (
                "/Users/sam/Library/Keychains",
                "/Users/sam/Library/Keychains/login.keychain-db",
            ),
            (
                "/Users/sam/Library",
                "/Users/sam/Library/LaunchAgents/run.plist",
            ),
            (
                "/Users/sam/Library/Application Support",
                "/Users/sam/Library/Application Support/Google/Chrome/Default/Cookies",
            ),
            (
                "/Users/sam/Library/Application Support",
                "/Users/sam/Library/Application Support/ilo/Default",
            ),
        ] {
            let folders = [listed(picked, Row::Change)];
            assert_eq!(at(path, &folders), At::Never, "{path} in {picked}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn a_guarded_place_is_reached_only_under_a_folder_picked_there() {
        let never = on_a_mac();
        let trip = [listed("/Users/sam/Documents/Trip", Row::Change)];
        let at = |path: &str| never.at(Path::new(path), &trip, None);
        assert_eq!(
            at("/Users/sam/Documents/Trip/plan.md"),
            At::Folder(Row::Change)
        );
        for path in [
            "/Users/sam/Documents/taxes.pdf",
            "/Users/sam/Desktop/note.txt",
            "/Users/sam/Downloads/a.zip",
            "/Users/sam/Library/Mobile Documents/com~apple~CloudDocs/a.txt",
            "/Users/sam/Library/CloudStorage/Dropbox/a.txt",
            "/Volumes/Backup/a.txt",
        ] {
            assert_eq!(at(path), At::Guarded, "{path}");
        }
        assert_eq!(at("/Users/sam/code/site/index.html"), At::Outside);
        // Elsewhere nothing is guarded by the system, so outside is outside.
        let elsewhere = super::tests::never();
        let at = |path: &str| elsewhere.at(Path::new(path), &[], None);
        assert_eq!(at("/Users/sam/Documents/taxes.pdf"), At::Outside);
    }

    #[cfg(not(windows))]
    #[test]
    fn where_a_path_is_from_the_never_list_its_folders_and_its_copies() {
        let never = on_a_mac();
        let folders = [
            listed("/Users/sam/code", Row::Read),
            listed("/Users/sam/code/site", Row::Change),
        ];
        let attached = Path::new("/srv/ilo/Tasks/task-1/Attached");
        let at = |path: &str| never.at(Path::new(path), &folders, Some(attached));
        assert_eq!(at("/Users/sam/code/notes.md"), At::Folder(Row::Read));
        assert_eq!(
            at("/Users/sam/code/site/index.html"),
            At::Folder(Row::Change)
        );
        assert_eq!(at("/srv/ilo/Tasks/task-1/Attached/a.txt"), At::Attached);
        assert_eq!(at("/Users/sam/notes.md"), At::Outside);
        assert_eq!(at("/run/media/sam/Backup/Photos/a.jpg"), At::Outside);
        for path in [
            "/srv/ilo/Tasks/task-1/config.json",
            "/srv/ilo/Tasks/task-2/Attached/a.txt",
            "/Users/sam/.ssh/id_ed25519",
            "/Users/sam/.config/fish/config.fish",
            "/Users/sam/.local/share/keyrings/login.keyring",
            "/Users/kim/notes.md",
            "/etc/hosts",
            "/proc/self/environ",
            "/Users/sam/code/../.ssh/id_ed25519",
            "code/notes.md",
        ] {
            assert_eq!(at(path), At::Never, "{path}");
        }
        // A folder picked on the never list doesn't open it.
        let ssh = [listed("/Users/sam/.ssh", Row::Change)];
        let ssh_at = never.at(Path::new("/Users/sam/.ssh/id_ed25519"), &ssh, None);
        assert_eq!(ssh_at, At::Never);
    }

    /// woven's own store of the user's keys and sign-ins is in its profile (a 0600 file in a dev
    /// build) or the Keychain (prod), and the vendors' programs keep theirs in their homes: never
    /// reached, whatever folder the user picked around them.
    #[cfg(not(windows))]
    #[test]
    fn the_account_store_keychains_and_vendor_homes_are_never_reached() {
        let here = Never::here(None);
        let profile = PathBuf::from(crate::profile_dir());
        let around = [Listed::for_test(
            profile.parent().unwrap().into(),
            Row::Change,
        )];
        for store in [profile.join("accounts"), profile.clone()] {
            assert_eq!(
                here.at(&store, &around, None),
                At::Never,
                "{}",
                store.display()
            );
        }
        let never = on_a_mac();
        let home = [
            listed("/Users/sam", Row::Change),
            listed("/Users/sam/Library", Row::Change),
        ];
        for path in [
            "/Users/sam/Library/Keychains/login.keychain-db",
            "/Users/sam/Library/Application Support/ilo/Default/accounts",
            "/Users/sam/.claude/.credentials.json",
            "/Users/sam/.claude.json",
            "/Users/sam/.codex/auth.json",
            "/Users/sam/.config/claude/settings.json",
            "/Users/sam/.aws/credentials",
            "/Users/sam/.config/gcloud/credentials.db",
        ] {
            assert_eq!(never.at(Path::new(path), &home, None), At::Never, "{path}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn the_identity_set_holds_every_whole_but_those_a_path_goes_on_through() {
        let never = never();
        let roots = never.by_identity();
        for whole in [
            "/Users/sam/.ssh",
            "/Users/sam/.config",
            "/Users/sam/Library/Keychains",
            "/Users/sam/Library/Application Support/ilo",
            "/srv/ilo/Tasks",
            "/System",
        ] {
            assert!(roots.contains(&PathBuf::from(whole)), "{whole}");
        }
        for through in ["/Users", "/run", "/Users/sam", "/Users/sam/Library", "/"] {
            assert!(!roots.contains(&PathBuf::from(through)), "{through}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn the_asks_folder_is_never_in_the_library_nor_one_never_given() {
        let never = on_a_mac();
        let ask = |parent: &str| never.ask_folder(Path::new(parent));
        assert_eq!(
            ask("/Users/sam/code/site").as_deref(),
            Some(Path::new("/Users/sam/code/site"))
        );
        for parent in [
            "/Users/sam/Library/Fonts",
            "/Users/sam/Library",
            "/Users/sam",
            "/Users/sam/Dropbox",
            "/",
            "/Volumes/Backup",
        ] {
            assert_eq!(ask(parent), None, "{parent}");
        }
    }

    #[test]
    fn file_words_name_the_product_and_cut_a_long_name_in_the_middle() {
        use super::words::*;
        let long = "Island Ferries timetable for the whole summer.pdf";
        let cut = quoted(long);
        assert_eq!(cut.chars().count(), 32 + 2, "{cut}");
        assert!(
            cut.starts_with("\u{201c}Island") && cut.ends_with("summer.pdf\u{201d}"),
            "{cut}"
        );
        assert_eq!(quoted("plan.md"), "\u{201c}plan.md\u{201d}");
        let product = crate::product::PRODUCT;
        for (why, reason) in [
            (Refused::Never, format!("{product} keeps it from threads")),
            (
                Refused::Changed,
                format!("it changed after {product} asked you"),
            ),
            (Refused::Guarded, "add its folder first".into()),
            (Refused::Needs(Tier::Work), "needs Work on pages".into()),
            (Refused::Needs(Tier::Full), "needs Full access".into()),
            (Refused::Attached, "attached files can only be read".into()),
            (Refused::OtherNames, "it's linked to another file".into()),
            (Refused::Gone, "it isn't there".into()),
            (
                Refused::NotOpened,
                format!("{product} doesn't open this kind of file"),
            ),
            (Refused::Cloud, "it isn't downloaded".into()),
            (Refused::TooLarge, "it's over 1 MB".into()),
            (Refused::NotText, "it isn't text".into()),
        ] {
            let line = refused(Step::Change("plan.md"), why);
            assert_eq!(
                line,
                format!("Couldn't change \u{201c}plan.md\u{201d}: {reason}")
            );
            for op in [Op::Read, Op::Change, Op::Make] {
                let told = told(op, why);
                assert!(told.starts_with("Not done: "), "{told}");
                // Only the user changes the level, so the assistant is never told one.
                for level in Tier::ALL {
                    assert!(!told.contains(level.label()), "{told}");
                }
            }
        }
        assert_eq!(
            refused(Step::MakeFile("plan.md"), Refused::Gone),
            "Couldn't make \u{201c}plan.md\u{201d}: its folder isn't there"
        );
        assert_eq!(
            refused(Step::Search("Trip"), Refused::Guarded),
            "Couldn't search in \u{201c}Trip\u{201d}: add its folder first"
        );
        let changed = |op| told(op, Refused::Changed);
        assert!(changed(Op::Read).contains("Try reading it again"));
        assert!(changed(Op::Change).contains("Read it again before trying again"));
        assert_eq!(changed(Op::Make), changed(Op::Change));
        assert_eq!(
            past_undo(Step::Move {
                file: "plan.md",
                to: "Trip"
            }),
            "Moved \u{201c}plan.md\u{201d} to \u{201c}Trip\u{201d} \u{b7} can't be undone"
        );
        assert_eq!(
            listed_first("Trip"),
            "Looked in \u{201c}Trip\u{201d} \u{b7} first 500"
        );
        assert_eq!(
            searched_early("Trip"),
            "Searched in \u{201c}Trip\u{201d} \u{b7} stopped early"
        );
    }

    /// Each tool's line while doing, done and failed, a search kept apart from the web's by `in`.
    #[test]
    fn each_file_tools_step_says_what_it_does_and_did() {
        use super::words::{Step, failed, step};
        let q = |name: &str| format!("\u{201c}{name}\u{201d}");
        let (plan, trip) = (q("plan.md"), q("Trip"));
        for (each, doing, done) in [
            (
                Step::Read("plan.md"),
                format!("Reading {plan}"),
                format!("Read {plan}"),
            ),
            (
                Step::List("Trip"),
                format!("Looking in {trip}"),
                format!("Looked in {trip}"),
            ),
            (
                Step::Search("Trip"),
                format!("Searching in {trip}"),
                format!("Searched in {trip}"),
            ),
            (
                Step::MakeFile("plan.md"),
                format!("Making {plan}"),
                format!("Made {plan}"),
            ),
            (
                Step::Change("plan.md"),
                format!("Changing {plan}"),
                format!("Changed {plan}"),
            ),
            (
                Step::MakeFolder("Trip"),
                format!("Making {trip}"),
                format!("Made {trip}"),
            ),
            (
                Step::Move {
                    file: "plan.md",
                    to: "Trip",
                },
                format!("Moving {plan}"),
                format!("Moved {plan} to {trip}"),
            ),
            (
                Step::Rename {
                    old: "plan.md",
                    new: "old.md",
                },
                format!("Renaming {plan}"),
                format!("Renamed {plan} to {}", q("old.md")),
            ),
        ] {
            assert_eq!(step(each, false), doing);
            assert_eq!(step(each, true), done);
        }
        for (each, line) in [
            (Step::Read("plan.md"), format!("Couldn't read {plan}")),
            (Step::List("Trip"), format!("Couldn't look in {trip}")),
            (Step::Search("Trip"), format!("Couldn't search in {trip}")),
            (Step::MakeFolder("Trip"), format!("Couldn't make {trip}")),
            (
                Step::Rename {
                    old: "plan.md",
                    new: "old.md",
                },
                format!("Couldn't rename {plan}"),
            ),
            (Step::Trash("plan.md"), format!("Couldn't delete {plan}")),
        ] {
            assert_eq!(failed(each), line);
        }
        #[cfg(not(windows))]
        {
            let trash = Step::Trash("plan.md");
            assert_eq!(step(trash, false), format!("Moving {plan} to the Trash"));
            assert_eq!(step(trash, true), format!("Moved {plan} to the Trash"));
        }
    }

    /// The lead's list (26 Nov): each is refused by its own name, inside a folder the user picked
    /// around it, and not only as part of a wider rule.
    #[cfg(not(windows))]
    #[test]
    fn each_named_secret_is_refused_by_name_inside_a_picked_folder() {
        let never = on_a_mac();
        let picked = [
            listed("/Users/sam", Row::Change),
            listed("/Users/sam/Library", Row::Change),
            listed("/Users/sam/Pictures", Row::Change),
        ];
        let named = never.whole();
        for (path, by_name) in [
            ("Library/Mail/V10/INBOX.mbox", "Library/Mail"),
            ("Library/Messages/chat.db", "Library/Messages"),
            ("Library/Calendars/Calendar.sqlitedb", "Library/Calendars"),
            (
                "Library/Application Support/AddressBook/AddressBook-v22.abcddb",
                "Library/Application Support/AddressBook",
            ),
            ("Library/Safari/History.db", "Library/Safari"),
            (
                "Library/Group Containers/group.com.apple.notes/NoteStore.sqlite",
                "Library/Group Containers/group.com.apple.notes",
            ),
            (
                "Library/Containers/com.apple.Notes/Data",
                "Library/Containers/com.apple.Notes",
            ),
            (
                "Library/Containers/com.example.App/Data",
                "Library/Containers",
            ),
            (
                "Library/Group Containers/2BUA8C4S2C.com.1password/x",
                "Library/Group Containers",
            ),
            ("Library/Accounts/Accounts4.sqlite", "Library/Accounts"),
            ("Library/Cookies/Cookies.binarycookies", "Library/Cookies"),
            ("Library/Keychains/login.keychain-db", "Library/Keychains"),
            (
                "Library/LaunchAgents/com.example.plist",
                "Library/LaunchAgents",
            ),
            (
                "Library/Preferences/com.apple.finder.plist",
                "Library/Preferences",
            ),
            (".ssh/id_ed25519", ".ssh"),
            (".gnupg/private-keys-v1.d/a.key", ".gnupg"),
            (".aws/credentials", ".aws"),
            (".config/gh/hosts.yml", ".config/gh"),
            (".netrc", ".netrc"),
            (".docker/config.json", ".docker/config.json"),
            (".zsh_history", ".zsh_history"),
            (".bash_history", ".bash_history"),
            (".zsh_sessions/A1.history", ".zsh_sessions"),
            (".python_history", ".python_history"),
            (".psql_history", ".psql_history"),
        ] {
            let path = Path::new("/Users/sam").join(path);
            assert_eq!(
                never.at(&path, &picked, None),
                At::Never,
                "{}",
                path.display()
            );
            let root = Path::new("/Users/sam").join(by_name);
            assert!(named.contains(&root), "{} listed by name", root.display());
        }
        for library in [
            "Pictures/Photos Library.photoslibrary/database/Photos.sqlite",
            "Pictures/Photos Library.photoslibrary",
            "Pictures/Old/Trip 2019.photoslibrary/originals/a.heic",
        ] {
            let path = Path::new("/Users/sam").join(library);
            assert_eq!(never.at(&path, &picked, None), At::Never, "{library}");
            assert!(never_grantable(&path, &never), "{library}");
        }
        let photo = Path::new("/Users/sam/Pictures/Trip/a.heic");
        assert_eq!(never.at(photo, &picked, None), At::Folder(Row::Change));
        // And what the lead's change opens: iCloud Drive and the cloud drives, by a folder
        // picked inside them.
        for (folder, file) in [
            (
                "/Users/sam/Library/Mobile Documents/com~apple~CloudDocs/Trip",
                "/Users/sam/Library/Mobile Documents/com~apple~CloudDocs/Trip/plan.md",
            ),
            (
                "/Users/sam/Library/CloudStorage/Dropbox/Trip",
                "/Users/sam/Library/CloudStorage/Dropbox/Trip/plan.md",
            ),
        ] {
            assert!(!never_grantable(Path::new(folder), &never), "{folder}");
            let trip = [listed(folder, Row::Change)];
            assert_eq!(
                never.at(Path::new(file), &trip, None),
                At::Folder(Row::Change)
            );
            assert_eq!(never.at(Path::new(file), &[], None), At::Guarded, "{file}");
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
