//! The `ilo` command (#3667): `ilo-command`, a program beside the browser's, which an agent in a
//! terminal runs for one MCP call at a time, as one more client of the server. Settings › Agents
//! adds it as it connects Claude Code: a link named `ilo` in the user's bin folder, and the
//! command's agent file in the profile directory, which only the user can read and which holds
//! the server's address and the agent's token. Remove takes away exactly those, never a line of
//! the user's shell profile: the user pastes that line themselves.

use std::io;
use std::path::{Path, PathBuf};

/// The program beside the browser's executable.
pub const PROGRAM: &str = if cfg!(windows) {
    "ilo-command.exe"
} else {
    "ilo-command"
};

/// What the agent types, the link's name in the bin folder.
pub const LINK: &str = "ilo";

/// The agent file's name in the profile directory.
pub const FILE: &str = "ilo-command";

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
}

/// The row's line under its name: its words, the part of them set in the mono face, as a command
/// the agent types, and the line to paste, if any, under them.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub line: String,
    pub mono: Option<std::ops::Range<usize>>,
    pub export: Option<&'static str>,
}

/// What the row says in `state`.
pub fn detail(state: State) -> Detail {
    match state {
        State::Off => Detail {
            line: format!(
                "Lets an agent in a terminal use the browser. Add links ilo in {FOLDER_SHOWN}."
            ),
            mono: None,
            export: None,
        },
        State::On => {
            let start = format!("Added in {FOLDER_SHOWN}. Agents can run ");
            let command = "ilo help";
            Detail {
                line: format!("{start}{command} to list commands."),
                mono: Some(start.len()..start.len() + command.len()),
                export: None,
            }
        }
        State::Unsearched => Detail {
            line: format!(
                "Added in {FOLDER_SHOWN}, which isn't on PATH. Add this to your shell profile:"
            ),
            mono: None,
            export: Some(EXPORT),
        },
    }
}

/// Whether `folder` is one of a PATH's, as a shell gives it, `:` between them.
pub fn searched(path: &str, folder: &Path) -> bool {
    path.split(':')
        .any(|each| !each.is_empty() && Path::new(each) == folder)
}

/// The agent file's lines: the server's address, the agent's token, and whether Add made the
/// folder, which Remove then takes away too once it's empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub address: PathBuf,
    pub token: String,
    pub made_folder: bool,
}

impl Agent {
    fn text(&self) -> String {
        let made = if self.made_folder {
            "made-folder\n"
        } else {
            ""
        };
        format!("{}\n{}\n{made}", self.address.display(), self.token)
    }

    pub fn parse(text: &str) -> Option<Agent> {
        let mut lines = text.lines();
        let address = PathBuf::from(lines.next().filter(|line| !line.is_empty())?);
        let token = lines.next().filter(|line| !line.is_empty())?.to_string();
        let made_folder = lines.next() == Some("made-folder");
        Some(Agent {
            address,
            token,
            made_folder,
        })
    }

    /// The agent file at `file`, as Add wrote it.
    pub fn read(file: &Path) -> Option<Agent> {
        Agent::parse(&std::fs::read_to_string(file).ok()?)
    }
}

/// Where Add puts the command, and what the link points at.
#[derive(Clone, Debug)]
pub struct Install {
    pub folder: PathBuf,
    pub program: PathBuf,
    pub file: PathBuf,
}

/// What Add says when the folder has an `ilo` of another program's, which it leaves.
pub fn taken(folder: &str) -> String {
    format!(
        "{folder}/{LINK} is another program's, so {} left it.",
        env!("APP_NAME")
    )
}

impl Install {
    fn link(&self) -> PathBuf {
        self.folder.join(LINK)
    }

    /// Whether the link is the command's: a link to a program named as it is, this copy's or
    /// another's, as a build in another folder makes.
    fn ours(&self) -> bool {
        std::fs::symlink_metadata(self.link()).is_ok_and(|meta| meta.file_type().is_symlink())
            && std::fs::read_link(self.link())
                .is_ok_and(|to| to.file_name() == Some(PROGRAM.as_ref()))
    }

    /// Whether Add made the link, as Settings › Agents shows the row.
    pub fn added(&self) -> bool {
        self.ours() && Agent::read(&self.file).is_some()
    }

    /// Makes the link to the program, and the agent file with the server's `address` and the
    /// agent's `token`, only the user's to read. A link of another copy's it replaces; anything
    /// else named `ilo` there it leaves, and says so.
    #[cfg(unix)]
    pub fn add(&self, address: &Path, token: &str) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let failed = |error: io::Error| error.to_string();
        let link = self.link();
        let made_folder = !self.folder.exists();
        let made_before = Agent::read(&self.file).is_some_and(|agent| agent.made_folder);
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
            made_folder: made_folder || made_before,
        };
        let wrote = (std::fs::OpenOptions::new())
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&self.file)
            .and_then(|mut file| {
                // A file left from before keeps the mode it had, which open doesn't change.
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
                file.write_all(agent.text().as_bytes())
            });
        if let Err(error) = wrote {
            _ = std::fs::remove_file(&link);
            if made_folder {
                _ = std::fs::remove_dir(&self.folder);
            }
            return Err(failed(error));
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn add(&self, _: &Path, _: &str) -> Result<(), String> {
        Err(io::Error::from(io::ErrorKind::Unsupported).to_string())
    }

    /// Takes away what Add made: the link, if it's still the command's, the agent file, and the
    /// folder if Add made it and nothing else is in it now.
    pub fn remove(&self) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        let made_folder = Agent::read(&self.file).is_some_and(|agent| agent.made_folder);
        if self.ours() {
            std::fs::remove_file(self.link()).map_err(failed)?;
        }
        match std::fs::remove_file(&self.file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(failed(error)),
            _ => {}
        }
        if made_folder {
            // Only an empty one goes; a file the user put there since keeps it.
            _ = std::fs::remove_dir(&self.folder);
        }
        Ok(())
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
                made_folder: false,
            })
        );
        install.remove().unwrap();
        assert!(!install.added());
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
        let on = detail(State::On);
        assert_eq!(&on.line[on.mono.clone().unwrap()], "ilo help");
        assert_eq!(on.export, None);
        assert_eq!(detail(State::Unsearched).export, Some(EXPORT));
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
}
