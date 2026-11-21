//! The `ilo` command (#3667): `ilo-command`, a program beside the browser's, which an agent in a
//! terminal runs for one MCP call at a time, as one more client of the server. Settings › Agents
//! adds it as it connects Claude Code: a link named `ilo` in the user's bin folder, and the
//! command's agent file in the profile directory, which only the user can read and which holds
//! the server's address and the agent's token. Remove takes away exactly those, never a line of
//! the user's shell profile: the user pastes that line themselves.
//!
//! In a Flatpak, whose sandbox can't write the host's bin folder, Add writes the agent file alone,
//! and the row gives a line to run once in a terminal, which puts a script named `ilo` there that
//! runs the command in the sandbox, as a client on the host runs the bridge.

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
/// the agent types, and the line to paste, if any, under them, which Copy Line copies.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub line: String,
    pub mono: Option<std::ops::Range<usize>>,
    pub export: Option<String>,
}

/// What the row says in `state`. In a Flatpak, `flatpak` is the line that puts `ilo` in the bin
/// folder, which the row gives once it's added (`flatpak_line`).
pub fn detail(state: State, flatpak: Option<&str>) -> Detail {
    let what = "Lets an agent in a terminal use the browser.";
    match (state, flatpak) {
        (State::Off, Some(_)) => Detail {
            line: format!(
                "{what} ilo's sandbox can't write to {FOLDER_SHOWN}, so Add gives a line to run once in a terminal."
            ),
            mono: None,
            export: None,
        },
        (_, Some(line)) => Detail {
            line: format!(
                "Added in ilo. Run this once in a terminal to put ilo in {FOLDER_SHOWN}:"
            ),
            mono: None,
            export: Some(line.into()),
        },
        (State::Off, None) => Detail {
            line: format!("{what} Add links ilo in {FOLDER_SHOWN}."),
            mono: None,
            export: None,
        },
        (State::On, None) => {
            let start = format!("Added in {FOLDER_SHOWN}. Agents can run ");
            let command = "ilo help";
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
/// line that writes a script named `ilo` in the host's bin folder, which runs the command beside
/// the bridge the same way: in the sandbox, with its permissions, so it opens no new way in.
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
    Some(format!(
        "mkdir -p {FOLDER_SHOWN} && printf '{script}' > {link} && chmod +x {link}"
    ))
}

/// Whether `folder` is one of a PATH's, as a shell gives it, `:` between them.
pub fn searched(path: &str, folder: &Path) -> bool {
    path.split(':')
        .any(|each| !each.is_empty() && Path::new(each) == folder)
}

/// The agent file's lines: the server's address, the agent's token, and the folders Add made,
/// the deepest first, which Remove then takes away too once they're empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub address: PathBuf,
    pub token: String,
    pub made: Vec<PathBuf>,
}

impl Agent {
    fn text(&self) -> String {
        let mut text = format!("{}\n{}\n", self.address.display(), self.token);
        for folder in &self.made {
            text.push_str(&format!("made {}\n", folder.display()));
        }
        text
    }

    pub fn parse(text: &str) -> Option<Agent> {
        let mut lines = text.lines();
        let address = PathBuf::from(lines.next().filter(|line| !line.is_empty())?);
        let token = lines.next().filter(|line| !line.is_empty())?.to_string();
        let made = (lines.filter_map(|line| line.strip_prefix("made ")))
            .map(PathBuf::from)
            .collect();
        Some(Agent {
            address,
            token,
            made,
        })
    }

    /// The agent file at `file`, as Add wrote it.
    pub fn read(file: &Path) -> Option<Agent> {
        Agent::parse(&std::fs::read_to_string(file).ok()?)
    }
}

/// Where Add puts the command, and what the link points at. Without `linked`, as in a Flatpak,
/// Add writes the agent file alone, and the user's script stands for the link.
#[derive(Clone, Debug)]
pub struct Install {
    pub folder: PathBuf,
    pub program: PathBuf,
    pub file: PathBuf,
    pub linked: bool,
}

/// What Add says when the folder has an `ilo` of another program's, which it leaves.
pub fn taken(folder: &str) -> String {
    format!(
        "{folder}/{LINK} isn't {0}'s, so it wasn't replaced. Move or remove it, then Add again.",
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
        (!self.linked || self.ours()) && Agent::read(&self.file).is_some()
    }

    /// Makes the link to the program, and the agent file with the server's `address` and the
    /// agent's `token`, only the user's to read. A link of another copy's it replaces; anything
    /// else named `ilo` there it leaves, and says so.
    #[cfg(unix)]
    pub fn add(&self, address: &Path, token: &str) -> Result<(), String> {
        let failed = |error: io::Error| error.to_string();
        if !self.linked {
            let agent = Agent {
                address: address.into(),
                token: token.into(),
                made: Vec::new(),
            };
            return self.write_agent(&agent).map_err(failed);
        }
        let link = self.link();
        // A home with no `~/.local` gets both folders, and Remove takes back both.
        let made_now: Vec<PathBuf> = (self.folder.ancestors())
            .take_while(|folder| !folder.exists())
            .map(Path::to_path_buf)
            .collect();
        let mut made = Agent::read(&self.file).map_or(Vec::new(), |agent| agent.made);
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
        };
        if let Err(error) = self.write_agent(&agent) {
            _ = std::fs::remove_file(&link);
            for folder in &made_now {
                _ = std::fs::remove_dir(folder);
            }
            return Err(failed(error));
        }
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

    #[cfg(not(unix))]
    pub fn add(&self, _: &Path, _: &str) -> Result<(), String> {
        Err(io::Error::from(io::ErrorKind::Unsupported).to_string())
    }

    /// Takes away what Add made: the link, if it's still the command's, the agent file, and the
    /// folders Add made, each if nothing else is in it now.
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
        let on = detail(State::On, None);
        assert_eq!(&on.line[on.mono.clone().unwrap()], "ilo help");
        assert_eq!(on.export, None);
        assert_eq!(
            detail(State::Unsearched, None).export.as_deref(),
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
        assert_eq!(detail(State::Off, Some(&line)).export, None);
        assert_eq!(detail(State::On, Some(&line)).export, Some(line.clone()));

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
