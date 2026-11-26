//! The command (#3667), named as the product: `ilo-command`, a program beside the browser's,
//! which an agent in a terminal runs for one MCP call at a time, as one more client of the server.
//! Settings › Agents adds it as it connects Claude Code: a link named as the product in the user's
//! bin folder, and the command's agent file in the profile directory, which only the user can read
//! and which holds the server's address and the agent's token. Remove takes away exactly those,
//! never a line of the user's shell profile: the user pastes that line themselves.
//!
//! In a Flatpak, whose sandbox can't write the host's bin folder, Add writes the agent file alone,
//! and the row gives a line to run once in a terminal, which puts a script named as the command
//! there that runs it in the sandbox, as a client on the host runs the bridge.
//!
//! On Windows the program is `bin\<product>.exe`. In the installer's copy that's `bin` beside
//! chrome.exe, whose own folder on PATH would bring the engine's programs with it, and Add puts
//! `bin` on the user's PATH, which Remove and the uninstaller take back. In the Store's copy it's
//! the package's alias, on PATH from install, which does nothing but say it isn't added until Add
//! writes the agent file.

use crate::product::PRODUCT;
use std::io;
use std::path::{Path, PathBuf};

/// The program beside the browser's executable.
pub const PROGRAM: &str = if cfg!(windows) {
    "ilo-command.exe"
} else {
    "ilo-command"
};

/// What the agent types, the link's name in the bin folder: the product's name.
pub const LINK: &str = env!("PRODUCT_NAME");

/// The agent file's name in the profile directory.
pub const FILE: &str = concat!(env!("PRODUCT_NAME"), "-command");

/// The command's name from before it took the product's: its link's, and its agent file's with
/// "-command", which Add Again takes away when they're still the command's.
pub const OLD_LINK: &str = env!("APP_NAME");

/// Whether the command's name `new` is another than its `old` one. A build whose name and product
/// are one has nothing old to take away: its old link and file are the ones Add just wrote.
pub fn renamed(old: &str, new: &str) -> bool {
    old != new
}

/// The folder Add puts the link in, as the row names it.
pub const FOLDER_SHOWN: &str = "~/.local/bin";

/// The line the row gives the user to paste into their shell's profile, when their shell doesn't
/// look in the folder.
pub const EXPORT: &str = r#"export PATH="$HOME/.local/bin:$PATH""#;

/// The folder Add puts the link in: the one most distributions' shells, and Claude Code's
/// installer, put on PATH.
pub fn folder(home: &Path) -> PathBuf {
    home.join(".local/bin")
}

/// The command's agent file, in the profile directory `profile`.
pub fn agent_file(profile: &Path) -> PathBuf {
    profile.join(FILE)
}

/// The command's row in Settings › Agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Not added: Add.
    Off,
    /// Added, in a folder the user's shell looks in: Remove.
    On,
    /// Added, in a folder the user's shell doesn't look in: the line to paste, Copy Line and
    /// Remove.
    Unsearched,
    /// Added under the name from before the command took the product's: Add Again.
    Stale,
}

/// The row's line under its name: its words, the part of them set in the mono face, as a command
/// the agent types, and the line to paste, if any, under them, which Copy Line copies.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub line: String,
    pub mono: Option<std::ops::Range<usize>>,
    pub export: Option<String>,
}

/// What the row says in `state`. In a Flatpak, `flatpak` is the line that puts `ilo` in the bin
/// folder, which the row gives once it's added (`flatpak_line`). `windows` is the folder the
/// command is in on Windows, as the row names it, where Add links nothing and needs no line pasted.
pub fn detail(state: State, flatpak: Option<&str>, windows: Option<&str>) -> Detail {
    let what = "Lets an agent in a terminal use the browser.";
    // The stale row says what a row not added says; only its button changes.
    match (state, flatpak) {
        (State::Off | State::Stale, Some(_)) => Detail {
            line: format!(
                "{what} {PRODUCT}'s sandbox can't write to {FOLDER_SHOWN}, so Add gives a line to run once in a terminal."
            ),
            mono: None,
            export: None,
        },
        (_, Some(line)) => Detail {
            line: format!(
                "Added in {PRODUCT}. Run this once in a terminal to put {LINK} in {FOLDER_SHOWN}:"
            ),
            mono: None,
            export: Some(line.into()),
        },
        (State::Off | State::Stale, None) => Detail {
            line: match windows {
                Some(_) => what.into(),
                None => format!("{what} Add links {LINK} in {FOLDER_SHOWN}."),
            },
            mono: None,
            export: None,
        },
        (State::On, None) => {
            let folder = windows.unwrap_or(FOLDER_SHOWN);
            let start = format!("Added in {folder}. Agents can run ");
            let command = format!("{LINK} help");
            Detail {
                line: format!("{start}{command} to list commands."),
                mono: Some(start.len()..start.len() + command.len()),
                export: None,
            }
        }
        (State::Unsearched, None) => Detail {
            line: format!(
                "Added in {FOLDER_SHOWN}, which isn't on PATH. Add this to your shell profile:"
            ),
            mono: None,
            export: Some(EXPORT.into()),
        },
    }
}

/// In a Flatpak, whose bridge a client on the host runs as `bridge` gives it (bridge::launch), the
/// line that writes a script named as the command in the host's bin folder, which runs the command
/// beside the bridge the same way: in the sandbox, with its permissions, so it opens no new way in.
/// The script from before the command took the product's name runs the program the same way, so
/// the line takes it away first, if it's still exactly that script, which the sandbox can't.
pub fn flatpak_line(bridge: &[String]) -> Option<String> {
    let [flatpak, run, command, id] = bridge else {
        return None;
    };
    let bridge = command.strip_prefix("--command=")?;
    if flatpak != "flatpak" || run != "run" {
        return None;
    }
    let program = Path::new(bridge).with_file_name(PROGRAM);
    let script = format!(
        r#"#!/bin/sh\nexec flatpak run --command={} {id} "$@"\n"#,
        program.display()
    );
    let link = format!("{FOLDER_SHOWN}/{LINK}");
    let old = format!("{FOLDER_SHOWN}/{OLD_LINK}");
    Some(format!(
        "if [ \"$(cat {old} 2>/dev/null)\" = \"$(printf '{script}')\" ]; then rm {old}; fi; \
         mkdir -p {FOLDER_SHOWN} && printf '{script}' > {link} && chmod +x {link}"
    ))
}

/// The row's state: not added, or added and found or not by a terminal opened now. On Windows a
/// command that isn't found is one whose PATH entry was taken off since Add put it there, which
/// Add puts back, so the row says it isn't added: there's no line to paste there (#3783).
pub fn state(added: bool, searched: bool, windows: bool) -> State {
    match (added, searched) {
        (false, _) => State::Off,
        (true, true) => State::On,
        (true, false) if windows => State::Off,
        (true, false) => State::Unsearched,
    }
}

/// Whether `folder` is one of a PATH's, as a shell gives it, `:` between them.
pub fn searched(path: &str, folder: &Path) -> bool {
    path.split(':')
        .any(|each| !each.is_empty() && Path::new(each) == folder)
}

/// The agent file's lines: the server's address, the agent's token, the folders Add made, the
/// deepest first, which Remove then takes away too once they're empty, and on Windows the entry
/// Add put on the user's PATH, which Remove takes back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agent {
    pub address: PathBuf,
    pub token: String,
    pub made: Vec<PathBuf>,
    pub path: Option<String>,
}

impl Agent {
    fn text(&self) -> String {
        let mut text = format!("{}\n{}\n", self.address.display(), self.token);
        for folder in &self.made {
            text.push_str(&format!("made {}\n", folder.display()));
        }
        if let Some(entry) = &self.path {
            text.push_str(&format!("path {entry}\n"));
        }
        text
    }

    pub fn parse(text: &str) -> Option<Agent> {
        let mut lines = text.lines();
        let address = PathBuf::from(lines.next().filter(|line| !line.is_empty())?);
        let token = lines.next().filter(|line| !line.is_empty())?.to_string();
        let rest: Vec<&str> = lines.collect();
        let made = (rest.iter().filter_map(|line| line.strip_prefix("made ")))
            .map(PathBuf::from)
            .collect();
        let path = (rest.iter().find_map(|line| line.strip_prefix("path "))).map(String::from);
        Some(Agent {
            address,
            token,
            made,
            path,
        })
    }

    /// The agent file at `file`, as Add wrote it.
    pub fn read(file: &Path) -> Option<Agent> {
        Agent::parse(&std::fs::read_to_string(file).ok()?)
    }
}

/// Where Add puts the command, and what the link points at. Without `linked`, as in a Flatpak or
/// on Windows, Add makes no link: in a Flatpak it writes the agent file alone, and the user's
/// script stands for the link, and on Windows the folder holds the program itself. `store` says
/// it's the Store's alias, on PATH from install.
#[derive(Clone, Debug)]
pub struct Install {
    pub folder: PathBuf,
    pub program: PathBuf,
    pub file: PathBuf,
    pub linked: bool,
    pub store: bool,
    /// The command's name before it took the product's (OLD_LINK), for Add Again.
    pub old: String,
}

/// What Add says when the folder has an `ilo` of another program's, which it leaves.
pub fn taken(folder: &str) -> String {
    format!(
        "{folder}/{LINK} isn't {0}'s, so it wasn't replaced. Move or remove it, then Add again.",
        env!("PRODUCT_NAME")
    )
}

impl Install {
    /// The command on Windows, for the bridge at `bridge` and the agent file `file`: in the
    /// Store's copy, whose bridge is its alias, the package's alias beside it; in the installer's,
    /// `bin\<product>.exe` beside the bridge.
    pub fn windows(bridge: &Path, local_app_data: &str, file: PathBuf) -> Install {
        let store = bridge == super::bridge::store_alias(local_app_data);
        let (folder, program) = match store {
            true => {
                let folder = super::bridge::store_aliases(local_app_data);
                let program = format!(r"{folder}\{LINK}.exe");
                (folder.into(), program.into())
            }
            false => {
                let folder = bridge.with_file_name("bin");
                let program = folder.join(format!("{LINK}.exe"));
                (folder, program)
            }
        };
        Install {
            folder,
            program,
            file,
            linked: false,
            store,
            old: OLD_LINK.into(),
        }
    }

    fn link(&self) -> PathBuf {
        self.folder.join(LINK)
    }

    /// Whether the link is the command's: a link to a program named as it is, this copy's or
    /// another's, as a build in another folder makes.
    fn ours(&self) -> bool {
        is_ours(&self.link())
    }

    /// The agent file from before the command took the product's name, beside this one.
    fn old_file(&self) -> PathBuf {
        self.file.with_file_name(format!("{}-command", self.old))
    }

    /// The command had another name before, whose link and file are others than these.
    fn renamed(&self) -> bool {
        renamed(&self.old, LINK)
    }

    /// The agent file Add wrote, or the one from before the command took the product's name, whose
    /// folders and PATH entry Add Again carries over for Remove to take back.
    fn written(&self) -> Option<Agent> {
        Agent::read(&self.file)
            .or_else(|| (self.renamed()).then(|| Agent::read(&self.old_file()))?)
    }

    /// Takes away what Add made before the command took the product's name, once Add Again has
    /// written the new: the old link, only while it's still the command's, and the old agent file.
    fn take_old(&self) {
        if !self.renamed() {
            return;
        }
        let old = self.folder.join(&self.old);
        if self.linked && is_ours(&old) {
            _ = std::fs::remove_file(old);
        }
        _ = std::fs::remove_file(self.old_file());
    }

    /// Whether Add made the link, or without one the agent file, as Settings › Agents shows it.
    pub fn added(&self) -> bool {
        (!self.linked || self.ours()) && Agent::read(&self.file).is_some()
    }

    /// Makes the link to the program, and the agent file with the server's `address` and the
    /// agent's `token`, only the user's to read. A link of another copy's it replaces; anything
    /// else named as the command there it leaves, and says so.
    #[cfg(unix)]
    pub fn add(&self, address: &Path, token: &str) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        if !self.linked {
            let agent = Agent {
                address: address.into(),
                token: token.into(),
                made: Vec::new(),
                path: None,
            };
            self.write_agent(&agent).map_err(failed)?;
            self.take_old();
            return Ok(());
        }
        let link = self.link();
        // A home with no `~/.local` gets both folders, and Remove takes back both.
        let made_now: Vec<PathBuf> = (self.folder.ancestors())
            .take_while(|folder| !folder.exists())
            .map(Path::to_path_buf)
            .collect();
        let mut made = self.written().map_or(Vec::new(), |agent| agent.made);
        made.retain(|folder| !made_now.contains(folder));
        made.splice(0..0, made_now.iter().cloned());
        std::fs::create_dir_all(&self.folder).map_err(failed)?;
        match std::fs::symlink_metadata(&link) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) if self.ours() => std::fs::remove_file(&link).map_err(failed)?,
            _ => return Err(taken(FOLDER_SHOWN)),
        }
        std::os::unix::fs::symlink(&self.program, &link).map_err(failed)?;
        let agent = Agent {
            address: address.into(),
            token: token.into(),
            made,
            path: None,
        };
        if let Err(error) = self.write_agent(&agent) {
            _ = std::fs::remove_file(&link);
            for folder in &made_now {
                _ = std::fs::remove_dir(folder);
            }
            return Err(failed(error));
        }
        self.take_old();
        Ok(())
    }

    /// Writes the agent file, only the user's to read.
    #[cfg(unix)]
    fn write_agent(&self, agent: &Agent) -> io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        (std::fs::OpenOptions::new())
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&self.file)
            .and_then(|mut file| {
                // A file left from before keeps the mode it had, which open doesn't change.
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
                file.write_all(agent.text().as_bytes())
            })
    }

    #[cfg(windows)]
    pub fn add(&self, address: &Path, token: &str) -> Result<(), String> {
        self.add_on_windows(&Registry, address, token)
    }

    /// Add on Windows, with the user's PATH in `path`: the agent file, and in the installer's
    /// copy the folder on PATH, unless an entry names it already, which the agent file then
    /// records as the user's. A PATH that can't be read is never written.
    pub fn add_on_windows(
        &self,
        path: &dyn PathStore,
        address: &Path,
        token: &str,
    ) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        let mut entry = None;
        let mut put = false;
        if !self.store {
            // An entry for a folder with no command in it would only lengthen the user's PATH.
            std::fs::metadata(&self.program).map_err(failed)?;
            let folder = self.folder.to_string_lossy().into_owned();
            match path_with(path.read().map_err(failed)?.as_ref(), &folder) {
                Some(with) => {
                    path.write(&with).map_err(failed)?;
                    put = true;
                    entry = Some(folder);
                }
                // Add's own from before, as after Revoke and Add again, is still Add's to take back.
                None => {
                    entry =
                        (self.written().and_then(|agent| agent.path)).filter(|was| *was == folder)
                }
            }
        }
        let agent = Agent {
            address: address.into(),
            token: token.into(),
            made: Vec::new(),
            path: entry.clone(),
        };
        if let Err(error) = std::fs::write(&self.file, agent.text()) {
            if let Some(entry) = entry.filter(|_| put) {
                _ = take_off(path, &entry);
            }
            return Err(failed(error));
        }
        self.take_old();
        Ok(())
    }

    /// Remove on Windows: the entry Add put on the user's PATH, and the agent file.
    pub fn remove_on_windows(&self, path: &dyn PathStore) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        if let Some(entry) = Agent::read(&self.file).and_then(|agent| agent.path) {
            // The agent file stays when this fails, so a Remove after still knows the entry.
            take_off(path, &entry).map_err(failed)?;
        }
        match std::fs::remove_file(&self.file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(failed(error)),
            _ => Ok(()),
        }
    }

    #[cfg(windows)]
    pub fn remove(&self) -> Result<(), String> {
        self.remove_on_windows(&Registry)
    }

    /// Whether a terminal opened now finds the command, with the user's PATH in `path`. The entry
    /// Add put there may have been taken off since, by an uninstall or by the user, while the
    /// agent file stays in the profile (#3783). An entry of the user's, or the Store's alias, isn't
    /// Add's to watch, and a PATH that can't be read says nothing either way.
    pub fn on_path(&self, path: &dyn PathStore) -> bool {
        let Some(entry) = Agent::read(&self.file).and_then(|agent| agent.path) else {
            return true;
        };
        match path.read() {
            Ok(value) => value
                .is_some_and(|value| (entries(&value.text).iter()).any(|each| names(each, &entry))),
            Err(_) => true,
        }
    }

    /// Takes away what Add made: the link, if it's still the command's, the agent file, and the
    /// folders Add made, each if nothing else is in it now.
    #[cfg(not(windows))]
    pub fn remove(&self) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        let made = Agent::read(&self.file).map_or(Vec::new(), |agent| agent.made);
        if self.linked && self.ours() {
            std::fs::remove_file(self.link()).map_err(failed)?;
        }
        match std::fs::remove_file(&self.file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(failed(error)),
            _ => {}
        }
        for folder in made {
            // Only an empty one goes; a file the user put there since keeps it, and its parent.
            _ = std::fs::remove_dir(folder);
        }
        Ok(())
    }
}

/// Whether `link` is the command's: a link to a program named as it is, this copy's or another's,
/// as a build in another folder makes.
fn is_ours(link: &Path) -> bool {
    std::fs::symlink_metadata(link).is_ok_and(|meta| meta.file_type().is_symlink())
        && std::fs::read_link(link).is_ok_and(|to| to.file_name() == Some(PROGRAM.as_ref()))
}

/// `folder` as the row names it on Windows: under `%LOCALAPPDATA%`, as `~` stands for home on unix.
pub fn shown(folder: &Path, local_app_data: &str) -> String {
    let folder = folder.to_string_lossy();
    match folder.get(..local_app_data.len()) {
        Some(start) if !local_app_data.is_empty() && start.eq_ignore_ascii_case(local_app_data) => {
            format!("%LOCALAPPDATA%{}", &folder[local_app_data.len()..])
        }
        _ => folder.into_owned(),
    }
}

/// The user's PATH as Windows keeps it in HKCU\Environment: its text, and whether it's
/// REG_EXPAND_SZ, whose `%…%` Windows expands, or REG_SZ. Add and Remove keep the kind they find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPath {
    pub text: String,
    pub expand: bool,
}

/// Where the user's PATH is kept: the registry, or the tests' stand-in.
pub trait PathStore {
    /// The value, or None when there's none.
    fn read(&self) -> io::Result<Option<UserPath>>;
    /// Writes `value`, or with no text deletes it, and tells the running programs, so a terminal
    /// opened after has it.
    fn write(&self, value: &UserPath) -> io::Result<()>;
}

/// A PATH's entries as written: split at each `;` outside quotes, as Windows reads it, so joining
/// them with `;` gives back the value byte for byte.
fn entries(text: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (at, char) in text.char_indices() {
        match char {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                entries.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    entries.push(&text[start..]);
    entries
}

/// Whether a PATH entry names `folder` as Windows reads it: without its quotes or a last `\`, in
/// any case.
fn names(entry: &str, folder: &str) -> bool {
    let bare = |text: &str| {
        let text = text.replace('"', "");
        text.trim().trim_end_matches('\\').to_lowercase()
    };
    bare(entry) == bare(folder)
}

/// The user's PATH with `folder` last, or None when an entry names it already, as one of the
/// user's may. A value ending in `;` keeps its `;` last, and one there's none of is REG_EXPAND_SZ,
/// as Windows makes the user's PATH.
pub fn path_with(value: Option<&UserPath>, folder: &str) -> Option<UserPath> {
    let Some(value) = value.filter(|value| !value.text.is_empty()) else {
        return Some(UserPath {
            text: folder.into(),
            expand: value.is_none_or(|value| value.expand),
        });
    };
    let mut entries = entries(&value.text);
    if entries.iter().any(|entry| names(entry, folder)) {
        return None;
    }
    let last = entries.len() - usize::from(entries.last() == Some(&""));
    entries.insert(last, folder);
    Some(UserPath {
        text: entries.join(";"),
        expand: value.expand,
    })
}

/// The user's PATH without the entry `path_with` wrote, byte for byte as it wrote it, or None when
/// it isn't there. Every other entry, and the `;` between them, stays as it was.
pub fn path_without(value: &UserPath, folder: &str) -> Option<UserPath> {
    let mut entries = entries(&value.text);
    let at = entries.iter().rposition(|entry| *entry == folder)?;
    entries.remove(at);
    Some(UserPath {
        text: entries.join(";"),
        expand: value.expand,
    })
}

/// Takes `entry` off the user's PATH in `path`, if it's there.
fn take_off(path: &dyn PathStore, entry: &str) -> io::Result<()> {
    match path.read()?.and_then(|value| path_without(&value, entry)) {
        Some(without) => path.write(&without),
        None => Ok(()),
    }
}

/// The user's PATH in the registry, as HKCU\Environment\Path.
#[cfg(windows)]
pub struct Registry;

#[cfg(windows)]
impl PathStore for Registry {
    fn read(&self) -> io::Result<Option<UserPath>> {
        use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, REG_EXPAND_SZ, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
            RegGetValueW,
        };
        let (key, name) = (wide("Environment"), wide("Path"));
        let mut kind = 0;
        let mut read = |data: *mut core::ffi::c_void, size: &mut u32| unsafe {
            // Unexpanded, so what's written back keeps the user's %…% as they are.
            let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
            let kind = &mut kind as *mut u32;
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                flags,
                kind,
                data,
                size,
            )
        };
        let mut size = 0;
        match read(std::ptr::null_mut(), &mut size) {
            0 => {}
            ERROR_FILE_NOT_FOUND => return Ok(None),
            error => return Err(io::Error::from_raw_os_error(error as i32)),
        }
        let mut text = vec![0u16; (size as usize).div_ceil(2)];
        match read(text.as_mut_ptr().cast(), &mut size) {
            0 => {}
            error => return Err(io::Error::from_raw_os_error(error as i32)),
        }
        text.truncate(size as usize / 2);
        while text.last() == Some(&0) {
            text.pop();
        }
        // A value that isn't whole UTF-16 would come back changed, so it's left as it is.
        let text = String::from_utf16(&text).map_err(|_| io::ErrorKind::InvalidData)?;
        Ok(Some(UserPath {
            text,
            expand: kind == REG_EXPAND_SZ,
        }))
    }

    fn write(&self, value: &UserPath) -> io::Result<()> {
        use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, REG_EXPAND_SZ, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };
        let (key, name) = (wide("Environment"), wide("Path"));
        let error = match value.text.is_empty() {
            true => {
                match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) }
                {
                    ERROR_FILE_NOT_FOUND => 0,
                    error => error,
                }
            }
            false => {
                let data = wide(&value.text);
                let kind = if value.expand { REG_EXPAND_SZ } else { REG_SZ };
                unsafe {
                    RegSetKeyValueW(
                        HKEY_CURRENT_USER,
                        key.as_ptr(),
                        name.as_ptr(),
                        kind,
                        data.as_ptr().cast(),
                        (data.len() * 2) as u32,
                    )
                }
            }
        };
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        // Explorer reads the environment again, so a terminal opened from it after has the entry.
        let environment = wide("Environment");
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                environment.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            )
        };
        Ok(())
    }
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod windows_tests {
    use super::*;
    use std::cell::RefCell;

    /// The user's PATH, as the tests keep it, and what was written to it.
    struct Store {
        value: RefCell<Option<UserPath>>,
        unreadable: bool,
        writes: RefCell<usize>,
    }

    impl Store {
        fn new(value: Option<UserPath>) -> Store {
            Store {
                value: RefCell::new(value),
                unreadable: false,
                writes: RefCell::new(0),
            }
        }
    }

    impl PathStore for Store {
        fn read(&self) -> io::Result<Option<UserPath>> {
            match self.unreadable {
                true => Err(io::ErrorKind::PermissionDenied.into()),
                false => Ok(self.value.borrow().clone()),
            }
        }

        fn write(&self, value: &UserPath) -> io::Result<()> {
            *self.writes.borrow_mut() += 1;
            *self.value.borrow_mut() = (!value.text.is_empty()).then(|| value.clone());
            Ok(())
        }
    }

    const BIN: &str = r"C:\Users\someone\AppData\Local\Programs\ilo\bin";

    fn value(text: &str, expand: bool) -> Option<UserPath> {
        Some(UserPath {
            text: text.into(),
            expand,
        })
    }

    /// Add then Remove on each PATH a user may have: the entry goes last, and after Remove every
    /// other entry and `;` is as it was, in the kind of value it was.
    #[test]
    fn add_then_remove_leaves_the_users_path_as_it_was() {
        let cases = [
            (r"C:\Tools;%USERPROFILE%\go\bin", true),
            (r"C:\Tools;%USERPROFILE%\go\bin", false),
            (r"C:\Tools;", true),
            (";", false),
            (r#""C:\Odd; Folder\bin";C:\Tools"#, true),
            (r"C:\Tools;;D:\bin", true),
        ];
        for (text, expand) in cases {
            let before = value(text, expand).unwrap();
            let with = path_with(Some(&before), BIN).unwrap();
            assert_eq!(with.expand, expand, "{text}");
            assert!(entries(&with.text).contains(&BIN), "{text}: {}", with.text);
            assert!(
                with.text.ends_with(BIN) || with.text.ends_with(&format!("{BIN};")),
                "{text}: {}",
                with.text
            );
            assert_eq!(path_without(&with, BIN), Some(before), "{text}");
        }
        // A quoted folder holding `;` is one entry, which Add leaves whole.
        let quoted = path_with(value(r#""C:\Odd; Folder\bin""#, true).as_ref(), BIN).unwrap();
        assert_eq!(quoted.text, format!(r#""C:\Odd; Folder\bin";{BIN}"#));
    }

    /// With no PATH value, or an empty one, Add makes one of its entry alone, REG_EXPAND_SZ as
    /// Windows makes it unless the empty one was REG_SZ, and Remove leaves none.
    #[test]
    fn add_makes_the_path_it_finds_none_of() {
        let made = path_with(None, BIN).unwrap();
        assert_eq!(made, value(BIN, true).unwrap());
        assert_eq!(path_without(&made, BIN).unwrap().text, "");
        assert_eq!(path_with(value("", false).as_ref(), BIN), value(BIN, false));
    }

    /// An entry of the user's that names the folder already, in another case, quoted or with a
    /// last `\`, means Add adds nothing, and Remove takes nothing of the user's.
    #[test]
    fn the_users_own_entry_stays_theirs() {
        for theirs in [
            BIN.to_uppercase(),
            format!("\"{BIN}\""),
            format!(r"{BIN}\"),
            format!(" {}", BIN.to_lowercase()),
        ] {
            let text = format!(r"C:\Tools;{theirs}");
            assert_eq!(path_with(value(&text, true).as_ref(), BIN), None, "{text}");
        }
        assert_eq!(
            path_without(&value(&BIN.to_uppercase(), true).unwrap(), BIN),
            None
        );
    }

    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Temp {
            let dir = std::env::temp_dir().join(format!(
                "ilo-cli-windows-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("app/bin")).unwrap();
            std::fs::write(dir.join(format!("app/bin/{LINK}.exe")), "").unwrap();
            Temp(dir)
        }

        fn install(&self) -> Install {
            Install::windows(
                &self.0.join("app").join(super::super::bridge::NAME),
                r"C:\Users\someone\AppData\Local",
                agent_file(&self.0),
            )
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The installer's copy: Add puts `bin` on the user's PATH and records it, Revoke and Add
    /// again keep it Add's, and Remove takes back that entry alone and the agent file.
    #[test]
    fn add_and_remove_put_bin_on_the_path_and_take_it_back() {
        let temp = Temp::new("bin");
        let install = temp.install();
        assert!(!install.store);
        assert_eq!(install.program, install.folder.join(format!("{LINK}.exe")));
        let folder = install.folder.to_string_lossy().into_owned();
        let before = value(r"C:\Tools", true);
        let path = Store::new(before.clone());
        install
            .add_on_windows(&path, Path::new(r"\\.\pipe\ilo"), "t")
            .unwrap();
        assert_eq!(
            path.value.borrow().as_ref().unwrap().text,
            format!(r"C:\Tools;{folder}")
        );
        assert_eq!(
            Agent::read(&install.file).unwrap().path,
            Some(folder.clone())
        );
        install
            .add_on_windows(&path, Path::new(r"\\.\pipe\ilo"), "u")
            .unwrap();
        assert_eq!(*path.writes.borrow(), 1);
        assert_eq!(Agent::read(&install.file).unwrap().path, Some(folder));
        install.remove_on_windows(&path).unwrap();
        assert_eq!(*path.value.borrow(), before);
        assert!(!install.file.exists());

        // The user's own entry for the folder: Add writes nothing, and Remove leaves it.
        let theirs = value(&install.folder.to_string_lossy().to_uppercase(), false);
        let path = Store::new(theirs.clone());
        install.add_on_windows(&path, Path::new("/s"), "t").unwrap();
        assert_eq!(Agent::read(&install.file).unwrap().path, None);
        install.remove_on_windows(&path).unwrap();
        assert_eq!(*path.value.borrow(), theirs);
        assert_eq!(*path.writes.borrow(), 0);
    }

    /// A PATH that can't be read is never written: Add fails with no agent file, and Remove fails
    /// keeping the agent file, so the next Remove still knows its entry.
    #[test]
    fn an_unreadable_path_is_never_written() {
        let temp = Temp::new("unreadable");
        let install = temp.install();
        let mut path = Store::new(value(r"C:\Tools", true));
        path.unreadable = true;
        assert!(install.add_on_windows(&path, Path::new("/s"), "t").is_err());
        assert!(!install.file.exists());
        path.unreadable = false;
        install.add_on_windows(&path, Path::new("/s"), "t").unwrap();
        path.unreadable = true;
        assert!(install.remove_on_windows(&path).is_err());
        assert!(Agent::read(&install.file).unwrap().path.is_some());
        assert_eq!(*path.writes.borrow(), 1);
    }

    /// The Store's copy: the alias is on PATH from install, so Add writes the agent file alone,
    /// and the PATH is never read or written.
    #[test]
    fn the_stores_copy_adds_the_agent_file_alone() {
        let temp = Temp::new("store");
        let local = r"C:\Users\someone\AppData\Local";
        let alias = super::super::bridge::store_alias(local);
        let install = Install::windows(&alias, local, agent_file(&temp.0));
        assert!(install.store);
        assert_eq!(
            install.program,
            PathBuf::from(format!(r"{local}\Microsoft\WindowsApps\{LINK}.exe"))
        );
        let mut path = Store::new(value(r"C:\Tools", true));
        path.unreadable = true;
        install.add_on_windows(&path, Path::new("/s"), "t").unwrap();
        assert_eq!(Agent::read(&install.file).unwrap().path, None);
        install.remove_on_windows(&path).unwrap();
        assert!(!install.file.exists());
        assert_eq!(*path.writes.borrow(), 0);
    }

    /// After an uninstall takes Add's entry off the user's PATH, and a reinstall, the agent file
    /// still records it: the command isn't found, the row says it isn't added, and Add puts the
    /// entry back. Nothing else on the PATH counts, and an entry of the user's never goes stale.
    #[test]
    fn an_entry_taken_off_the_path_is_added_again() {
        let temp = Temp::new("taken-off");
        let install = temp.install();
        let path = Store::new(value(r"C:\Tools", true));
        assert!(install.on_path(&path), "nothing added, nothing to find");
        install.add_on_windows(&path, Path::new("/s"), "t").unwrap();
        assert!(install.on_path(&path));
        let folder = install.folder.to_string_lossy().into_owned();
        // The uninstaller takes back exactly Add's entry, and leaves the profile.
        let uninstalled = path_without(path.value.borrow().as_ref().unwrap(), &folder).unwrap();
        *path.value.borrow_mut() = Some(uninstalled);
        assert!(Agent::read(&install.file).unwrap().path.is_some());
        assert!(!install.on_path(&path));
        assert_eq!(state(true, install.on_path(&path), true), State::Off);
        install.add_on_windows(&path, Path::new("/s"), "u").unwrap();
        assert!(install.on_path(&path));
        assert_eq!(
            path.value.borrow().as_ref().unwrap().text,
            format!(r"C:\Tools;{folder}")
        );
        assert_eq!(
            Agent::read(&install.file).unwrap().path,
            Some(folder.clone())
        );
        // The entry back in another case, as the user may write it, is found.
        *path.value.borrow_mut() = value(&folder.to_uppercase(), true);
        assert!(install.on_path(&path));
        // With no PATH at all it isn't, and a PATH that can't be read says nothing.
        *path.value.borrow_mut() = None;
        assert!(!install.on_path(&path));
        let mut unreadable = Store::new(None);
        unreadable.unreadable = true;
        assert!(install.on_path(&unreadable));
        // The user's own entry, which Add recorded nothing for, isn't Add's to watch.
        install.remove_on_windows(&Store::new(None)).unwrap();
        let theirs = Store::new(value(&folder, true));
        install
            .add_on_windows(&theirs, Path::new("/s"), "t")
            .unwrap();
        *theirs.value.borrow_mut() = None;
        assert!(install.on_path(&theirs));
    }

    /// The row's states: on Windows a command added but not found is Add again, as there's no line
    /// to paste there; elsewhere it's the line to paste.
    #[test]
    fn a_command_not_found_is_add_on_windows() {
        assert_eq!(state(false, true, true), State::Off);
        assert_eq!(state(true, true, true), State::On);
        assert_eq!(state(true, false, true), State::Off);
        assert_eq!(state(true, false, false), State::Unsearched);
        assert_eq!(state(false, false, false), State::Off);
    }

    /// On Windows the row says neither that Add links nor what to paste, and names the folder
    /// under %LOCALAPPDATA%.
    #[test]
    fn the_row_on_windows_names_the_folder() {
        let local = r"C:\Users\someone\AppData\Local";
        let shown = super::shown(Path::new(&format!(r"{local}\Programs\ilo\bin")), local);
        assert_eq!(shown, r"%LOCALAPPDATA%\Programs\ilo\bin");
        let on = detail(State::On, None, Some(&shown));
        assert!(on.line.starts_with(&format!("Added in {shown}.")));
        assert_eq!(&on.line[on.mono.clone().unwrap()], format!("{LINK} help"));
        let off = detail(State::Off, None, Some(&shown));
        assert!(!off.line.contains("~/.local/bin"));
        assert_eq!(super::shown(Path::new(r"D:\ilo\bin"), local), r"D:\ilo\bin");
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A folder of its own under the system's temporary one, gone after the test.
    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Temp {
            let dir = std::env::temp_dir().join(format!(
                "ilo-cli-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Every file under `dir`, with what it holds or where it links to, and its mode.
    fn contents(dir: &Path) -> BTreeMap<PathBuf, String> {
        use std::os::unix::fs::PermissionsExt;
        let mut found = BTreeMap::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let what = if meta.file_type().is_symlink() {
                format!("-> {}", std::fs::read_link(&path).unwrap().display())
            } else if meta.is_dir() {
                found.extend(contents(&path));
                "folder".into()
            } else {
                std::fs::read_to_string(&path).unwrap_or_default()
            };
            found.insert(
                path,
                format!("{what} {:o}", meta.permissions().mode() & 0o777),
            );
        }
        found
    }

    fn install(temp: &Temp) -> Install {
        Install {
            folder: folder(&temp.0.join("home")),
            program: temp.0.join("app").join(PROGRAM),
            file: agent_file(&temp.0.join("profile")),
            linked: true,
            store: false,
            old: OLD_LINK.into(),
        }
    }

    /// Add then Remove leaves the user's folders as they were: a file of theirs beside the link
    /// stays, and nothing of the command's is left.
    #[test]
    fn add_then_remove_leaves_the_folders_as_they_were() {
        let temp = Temp::new("leaves");
        let install = install(&temp);
        std::fs::create_dir_all(&install.folder).unwrap();
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        std::fs::write(install.folder.join("tool"), "the user's").unwrap();
        std::fs::write(install.file.with_file_name("Agents"), "the browser's").unwrap();
        let before = contents(&temp.0);
        assert!(!install.added());
        install
            .add(Path::new("/profile/Agents.sock"), "a-token")
            .unwrap();
        assert!(install.added());
        assert_eq!(
            std::fs::read_link(install.folder.join(LINK)).unwrap(),
            install.program
        );
        assert_eq!(
            Agent::read(&install.file),
            Some(Agent {
                address: "/profile/Agents.sock".into(),
                token: "a-token".into(),
                made: Vec::new(),
                path: None,
            })
        );
        install.remove().unwrap();
        assert!(!install.added());
        assert_eq!(contents(&temp.0), before);

        // A home with no `~/.local` gets `~/.local/bin`, and Remove takes both back.
        let temp = Temp::new("bare-home");
        let install = self::install(&temp);
        let local = install.folder.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        let before = contents(&temp.0);
        install.add(Path::new("/s"), "t").unwrap();
        assert!(install.added());
        let made = Agent::read(&install.file).unwrap().made;
        assert_eq!(made, [install.folder.clone(), local.clone()]);
        install.remove().unwrap();
        assert_eq!(contents(&temp.0), before, "~/.local is left behind");

        // A file the user put in `~/.local` since keeps it, and the bin folder still goes.
        install.add(Path::new("/s"), "t").unwrap();
        std::fs::write(local.join("share"), "the user's").unwrap();
        install.remove().unwrap();
        assert!(!install.folder.exists());
        assert_eq!(
            std::fs::read_to_string(local.join("share")).unwrap(),
            "the user's"
        );
    }

    /// Add Again, after the command took the product's name: the old link goes while it's still
    /// the command's, the old agent file goes, and the folders the first Add made pass to the new
    /// one for Remove to take back. An old name that's another program's stays.
    #[test]
    fn add_again_takes_away_only_what_the_first_add_made() {
        let temp = Temp::new("add-again");
        let install = install(&temp);
        let local = install.folder.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        let before = contents(&temp.0);
        std::fs::create_dir_all(&install.folder).unwrap();
        let old_link = install.folder.join(OLD_LINK);
        std::os::unix::fs::symlink(&install.program, &old_link).unwrap();
        let first = Agent {
            address: "/s".into(),
            token: "before".into(),
            made: vec![install.folder.clone(), local.clone()],
            path: None,
        };
        std::fs::write(install.old_file(), first.text()).unwrap();
        assert!(!install.added());

        install.add(Path::new("/s"), "t").unwrap();
        assert!(install.added());
        assert!(
            std::fs::symlink_metadata(&old_link).is_err(),
            "the old link stayed"
        );
        assert!(!install.old_file().exists(), "the old agent file stayed");
        let made = Agent::read(&install.file).unwrap().made;
        assert_eq!(made, [install.folder.clone(), local.clone()]);
        install.remove().unwrap();
        assert_eq!(contents(&temp.0), before);

        // A file, or a link to another program, under the old name is the user's.
        for theirs in [None, Some(temp.0.join("their-tool"))] {
            std::fs::create_dir_all(&install.folder).unwrap();
            match &theirs {
                None => std::fs::write(&old_link, "the user's").unwrap(),
                Some(to) => std::os::unix::fs::symlink(to, &old_link).unwrap(),
            }
            std::fs::write(install.old_file(), first.text()).unwrap();
            let kept = contents(&install.folder);
            install.add(Path::new("/s"), "t").unwrap();
            assert!(!install.old_file().exists());
            assert_eq!(
                contents(&install.folder).get(&old_link),
                kept.get(&old_link)
            );
            install.remove().unwrap();
            std::fs::remove_file(&old_link).unwrap();
        }
    }

    /// In a build whose name and product are one, the old link and file are the ones Add writes:
    /// Add leaves them, Add again too, and Remove takes back exactly what Add made.
    #[test]
    fn with_one_name_add_takes_nothing_old() {
        assert!(renamed(OLD_LINK, "another"));
        assert!(!renamed(LINK, LINK));
        let temp = Temp::new("one-name");
        let install = Install {
            old: LINK.into(),
            ..install(&temp)
        };
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        let before = contents(&temp.0);
        install.add(Path::new("/s"), "t").unwrap();
        assert!(install.added(), "the link and the agent file stayed");
        assert_eq!(install.old_file(), install.file);
        let made = contents(&temp.0);
        install.add(Path::new("/s"), "t").unwrap();
        assert!(install.added(), "and stay as Add runs again");
        assert_eq!(contents(&temp.0), made);
        install.remove().unwrap();
        assert_eq!(contents(&temp.0), before);
    }

    /// The agent file holds the token, so only the user may read it, though a file left there
    /// before let others.
    #[test]
    fn only_the_user_reads_the_agent_file() {
        use std::os::unix::fs::PermissionsExt;
        let temp = Temp::new("mode");
        let install = install(&temp);
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        std::fs::write(&install.file, "old").unwrap();
        std::fs::set_permissions(&install.file, std::fs::Permissions::from_mode(0o644)).unwrap();
        install.add(Path::new("/s"), "t").unwrap();
        let mode = std::fs::metadata(&install.file)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    /// A folder Add made goes with Remove, but not one the user has put a file in since.
    #[test]
    fn a_folder_add_made_goes_with_remove_once_empty() {
        let temp = Temp::new("made");
        let install = install(&temp);
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        install.add(Path::new("/s"), "t").unwrap();
        // A second Add, as after Revoke and Add again, still knows the folder was its.
        install.add(Path::new("/s"), "u").unwrap();
        install.remove().unwrap();
        assert!(!install.folder.exists());
        install.add(Path::new("/s"), "t").unwrap();
        std::fs::write(install.folder.join("tool"), "the user's").unwrap();
        install.remove().unwrap();
        assert_eq!(
            std::fs::read_to_string(install.folder.join("tool")).unwrap(),
            "the user's"
        );
        assert!(!install.folder.join(LINK).exists());
    }

    /// An `ilo` of another program's stays as it was, and Add says so; another copy's link is
    /// replaced.
    #[test]
    fn add_leaves_another_programs_ilo() {
        let temp = Temp::new("taken");
        let install = install(&temp);
        std::fs::create_dir_all(&install.folder).unwrap();
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        let link = install.folder.join(LINK);
        std::fs::write(&link, "#!/bin/sh\n").unwrap();
        assert_eq!(install.add(Path::new("/s"), "t"), Err(taken(FOLDER_SHOWN)));
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "#!/bin/sh\n");
        assert!(!install.file.exists());
        install.remove().unwrap();
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "#!/bin/sh\n");
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("/elsewhere/vi", &link).unwrap();
        assert!(install.add(Path::new("/s"), "t").is_err());
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(Path::new("/other/copy").join(PROGRAM), &link).unwrap();
        install.add(Path::new("/s"), "t").unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), install.program);
    }

    /// The row says where the link is, sets the command the agent types in the mono face, and
    /// gives the line to paste only when the shell doesn't look in the folder.
    #[test]
    fn the_row_says_where_and_what_to_run() {
        let on = detail(State::On, None, None);
        assert_eq!(&on.line[on.mono.clone().unwrap()], format!("{LINK} help"));
        assert_eq!(on.export, None);
        assert_eq!(
            detail(State::Unsearched, None, None).export.as_deref(),
            Some(EXPORT)
        );
        assert!(EXPORT.contains("$HOME/.local/bin"));
        let home = Path::new("/home/someone");
        assert!(searched("/usr/bin:/home/someone/.local/bin", &folder(home)));
        assert!(searched(
            "/home/someone/.local/bin/:/usr/bin",
            &folder(home)
        ));
        assert!(!searched("/usr/bin:/home/other/.local/bin", &folder(home)));
        assert!(!searched("", &folder(home)));
    }

    /// In a Flatpak, Add writes the agent file alone, since the sandbox can't write the bin
    /// folder, and Remove takes it back: nothing else is made or touched, though a link of a copy
    /// outside the sandbox is there.
    #[test]
    fn a_flatpaks_add_writes_the_agent_file_alone() {
        let temp = Temp::new("flatpak");
        let install = Install {
            linked: false,
            ..install(&temp)
        };
        std::fs::create_dir_all(install.file.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&install.folder).unwrap();
        std::os::unix::fs::symlink(Path::new("/opt/ilo").join(PROGRAM), install.link()).unwrap();
        let before = contents(&temp.0);
        assert!(!install.added());
        install.add(Path::new("/s"), "t").unwrap();
        assert!(install.added());
        let mut after = before.clone();
        after.insert(install.file.clone(), "/s\nt\n 600".into());
        assert_eq!(contents(&temp.0), after);
        install.remove().unwrap();
        assert!(!install.added());
        assert_eq!(contents(&temp.0), before);
    }

    /// A Flatpak's row, before Add and after it, gives the line to run on the host, which Copy
    /// Line copies; run there, the line makes `ilo` a script that runs the command where Flatpak
    /// runs the bridge, the words the agent typed after it.
    #[test]
    fn a_flatpaks_line_makes_ilo_run_the_command_in_the_sandbox() {
        let bridge =
            super::super::bridge::launch(Path::new("/app/ilo/mcp-bridge"), Some("app.ilo.browser"));
        let line = flatpak_line(&bridge).unwrap();
        assert_eq!(flatpak_line(&["/opt/ilo/mcp-bridge".into()]), None);
        assert_eq!(detail(State::Off, Some(&line), None).export, None);
        assert_eq!(
            detail(State::On, Some(&line), None).export,
            Some(line.clone())
        );

        let temp = Temp::new("flatpak-line");
        let ran = crate::child::command("/bin/sh")
            .args(["-c", &line])
            .env("HOME", &temp.0)
            .status()
            .unwrap();
        assert!(ran.success());
        let script = folder(&temp.0).join(LINK);
        assert_eq!(
            std::fs::read_to_string(&script).unwrap(),
            "#!/bin/sh\nexec flatpak run --command=/app/ilo/ilo-command app.ilo.browser \"$@\"\n"
        );
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, 0o100, "the script isn't executable");

        // Run again, as after an update, it writes the same script.
        let ran = crate::child::command("/bin/sh")
            .args(["-c", &line])
            .env("HOME", &temp.0)
            .status()
            .unwrap();
        assert!(ran.success());
        assert!(
            std::fs::read_to_string(&script)
                .unwrap()
                .contains("app.ilo.browser")
        );
    }
}
