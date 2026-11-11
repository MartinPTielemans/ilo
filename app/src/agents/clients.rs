//! Connect and Disconnect: the browser's bridge in an agent client's own config, as a stdio MCP
//! server with the agent's token, and out of it again, and nothing else of the config changed. Claude Code's is in ~/.claude.json, which the
//! running client rewrites, so its own `claude mcp` command changes it; Codex's is
//! ~/.codex/config.toml, which Codex only reads, so the browser edits its one table. Any other
//! MCP client gets the bridge's command and its token to paste.

use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::Command;

use serde_json::{Value, json};

use super::bridge;

/// The agent clients Settings › Agents connects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Client {
    ClaudeCode,
    Codex,
    /// Any other MCP client, which the user sets up with what Copy Setup copies.
    Other,
}

impl Client {
    pub const ALL: [Client; 3] = [Client::ClaudeCode, Client::Codex, Client::Other];

    /// The id of its agent, which its Space and the core know it by.
    pub fn id(self) -> &'static str {
        match self {
            Client::ClaudeCode => "claude-code",
            Client::Codex => "codex",
            Client::Other => "mcp-client",
        }
    }

    pub fn from_id(id: &str) -> Option<Client> {
        Client::ALL.into_iter().find(|client| client.id() == id)
    }

    /// Its name, for the user, and its Space's.
    pub fn name(self) -> &'static str {
        match self {
            Client::ClaudeCode => "Claude Code",
            Client::Codex => "Codex",
            Client::Other => "MCP client",
        }
    }

    /// Where its config is, as the user would write it.
    pub fn config(self) -> &'static str {
        match self {
            Client::ClaudeCode => "~/.claude.json",
            Client::Codex => "~/.codex/config.toml",
            Client::Other => "",
        }
    }
}

/// The server as a client's config names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The name the client lists it by: the browser's.
    pub name: String,
    /// What the client runs to reach the server: the bridge, or what runs it, and its arguments,
    /// the server's socket last.
    pub command: String,
    pub args: Vec<String>,
    pub token: String,
}

impl Entry {
    /// The server as most clients' JSON names a stdio one.
    pub fn json(&self) -> Value {
        json!({
            "type": "stdio",
            "command": self.command,
            "args": self.args,
            "env": { bridge::TOKEN: self.token },
        })
    }
}

/// What writes the clients' configs: the system's, or the tests' stand-in.
pub trait Clients: Send + Sync {
    fn connect(&self, client: Client, entry: &Entry) -> Result<(), String>;
    fn disconnect(&self, client: Client, name: &str) -> Result<(), String>;
    /// Whether the client is on this computer, for Settings › Agents to list it. Other is always.
    fn found(&self, client: Client) -> bool;
    /// Whether a CLI that runs agent tasks is on this computer and signed in, which runs it.
    fn look_for_task_cli(&self, cli: super::runner::Cli) -> super::ready::Found {
        super::ready::look(cli)
    }
}

/// The configs in the user's home.
pub struct System;

impl Clients for System {
    fn connect(&self, client: Client, entry: &Entry) -> Result<(), String> {
        match client {
            Client::ClaudeCode => {
                let claude = claude()?;
                // add-json refuses a name it has, as after a Connect the user undid by hand.
                _ = claude(&["mcp", "remove", "--scope", "user", &entry.name]);
                claude(
                    &claude_add(entry)
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                )
            }
            Client::Codex => {
                let path = codex_config();
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                write(&path, &codex_with(&text, entry))
            }
            Client::Other => Ok(()),
        }
    }

    fn disconnect(&self, client: Client, name: &str) -> Result<(), String> {
        match client {
            Client::ClaudeCode => claude()?(&["mcp", "remove", "--scope", "user", name]),
            Client::Codex => {
                let path = codex_config();
                match std::fs::read_to_string(&path) {
                    Ok(text) => write(&path, &codex_without(&text, name)),
                    Err(_) => Ok(()),
                }
            }
            Client::Other => Ok(()),
        }
    }

    /// By the folder or file each makes in the user's home as it first runs, which a stat finds at
    /// once: its command may be on a PATH only the user's login shell has.
    fn found(&self, client: Client) -> bool {
        match client {
            Client::ClaudeCode => {
                home().join(".claude.json").exists() || home().join(".claude").is_dir()
            }
            Client::Codex => codex_config().parent().is_some_and(Path::is_dir),
            Client::Other => true,
        }
    }
}

pub fn home() -> PathBuf {
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    PathBuf::from(home.unwrap_or_default())
}

/// `claude`'s arguments that add the server for every project, as the user's.
pub fn claude_add(entry: &Entry) -> Vec<String> {
    let server = entry.json().to_string();
    ["mcp", "add-json", "--scope", "user", &entry.name, &server]
        .map(String::from)
        .into()
}

/// What Connect says when there's no `claude` to run.
const NO_CLAUDE: &str = "Claude Code's claude command isn't installed.";

/// Claude Code's `claude`, found once for a Connect's two runs, to run with each one's arguments.
fn claude() -> Result<impl Fn(&[&str]) -> Result<(), String>, String> {
    let (program, path) = find_claude().ok_or(NO_CLAUDE)?;
    Ok(move |arguments: &[&str]| {
        let mut command = crate::child::command(&program);
        command.env("PATH", &path).args(arguments);
        let output = command.output().map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => NO_CLAUDE.into(),
            _ => error.to_string(),
        })?;
        match output.status.success() {
            true => Ok(()),
            false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        }
    })
}

/// `claude` where a terminal opened now would find it, or where an installer put it (#1420).
/// Rust runs a .cmd through cmd.exe with each argument quoted for it, the JSON's quotes doubled,
/// which node reads back whole.
#[cfg(windows)]
fn find_claude() -> Option<(PathBuf, String)> {
    windows_find("claude")
}

/// `name`, as `claude` or `codex`, where a terminal opened now would find it, or where an
/// installer put it, and that terminal's PATH, where an npm .cmd shim finds node. Connect and the
/// task agents both look for their CLI so (#1676).
#[cfg(windows)]
pub fn windows_find(name: &str) -> Option<(PathBuf, String)> {
    let var = |name: &str| std::env::var(name).ok();
    let path = windows_path(&var, registry_path(false), registry_path(true));
    let folders = windows_folders(&var, &path);
    Some((
        windows_program(name, &folders, var("PATHEXT"))?,
        path.join(";"),
    ))
}

/// `claude` where a terminal the user opens would find it, or where an installer put it, which
/// may be on no PATH the browser has (#1441), and those folders as its PATH, where npm's `claude`
/// finds node. A browser the Dock or Finder started has only the system's PATH.
#[cfg(not(windows))]
fn find_claude() -> Option<(PathBuf, String)> {
    let var = |name: &str| std::env::var(name).ok();
    let home = home();
    let npmrc = std::fs::read_to_string(home.join(".npmrc")).ok();
    let npm = npm_bin(&var, &home, npmrc.as_deref());
    let folders = unix_folders(&[var("PATH"), shell_path()], &home, npm, Path::new("/"));
    let path = folders
        .iter()
        .map(|folder| folder.to_string_lossy())
        .collect::<Vec<_>>();
    Some((unix_claude(&folders)?, path.join(":")))
}

/// Where a terminal finds `claude`: the folders on `paths`, the browser's PATH and then the user's
/// shell's, first to last, then where Claude Code's installer, npm and Homebrew put it, and the
/// folder of the install Claude Code once made in ~/.claude, which the user reached by an alias.
/// Each folder once, and only from its root, `root`, as the app's working folder means nothing.
#[cfg(not(windows))]
fn unix_folders(
    paths: &[Option<String>],
    home: &Path,
    npm: Option<PathBuf>,
    root: &Path,
) -> Vec<PathBuf> {
    let path = paths.iter().flatten().flat_map(|path| path.split(':'));
    let path = path
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute());
    let installed = [
        Some(home.join(".local/bin")),
        npm,
        Some(root.join("opt/homebrew/bin")),
        Some(root.join("usr/local/bin")),
        Some(home.join(".claude/local")),
    ];
    let mut folders: Vec<PathBuf> = Vec::new();
    for folder in path.chain(installed.into_iter().flatten()) {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders
}

/// The first `claude` in `folders` that's a program, as a shell runs one.
#[cfg(unix)]
fn unix_claude(folders: &[PathBuf]) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let runs = |program: &PathBuf| {
        (program.metadata())
            .is_ok_and(|file| file.is_file() && file.permissions().mode() & 0o111 != 0)
    };
    folders
        .iter()
        .map(|folder| folder.join("claude"))
        .find(runs)
}

/// The folder npm installs global packages' commands in when the user moved its prefix, as npm's
/// docs have them do to install without sudo: in npm_config_prefix, or `npmrc`, ~/.npmrc. npm's
/// own prefix is node's folder, which the PATH that has node has too.
#[cfg(not(windows))]
fn npm_bin(
    var: &dyn Fn(&str) -> Option<String>,
    home: &Path,
    npmrc: Option<&str>,
) -> Option<PathBuf> {
    let set = npmrc.unwrap_or_default().lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "prefix").then(|| value.trim().trim_matches('"').to_string())
    });
    let prefix = (var("npm_config_prefix")
        .or_else(|| var("NPM_CONFIG_PREFIX"))
        .or(set))
    .filter(|prefix| !prefix.is_empty())?;
    let home = home.to_string_lossy();
    let prefix = prefix.replace("${HOME}", &home);
    let prefix = match prefix.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => prefix,
    };
    Some(Path::new(&prefix).join("bin"))
}

/// What starts the line the user's shell prints its PATH on, after anything their rc files print.
#[cfg(unix)]
const SHELL_PATH: &str = "ilo-shell-path=";

/// The shell command that prints the PATH, on a line of its own.
#[cfg(not(windows))]
const PRINT_PATH: &str = r#"printf '\nilo-shell-path=%s\n' "$PATH""#;

/// How long the user's shell may take to start before Connect does without its PATH.
#[cfg(not(windows))]
const SHELL_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// The PATH a terminal the user opens has: their shell's, started as a terminal starts it, an
/// interactive login shell, which reads ~/.zshrc or ~/.bashrc, where Claude Code's installer has
/// the user put ~/.local/bin on PATH, as well as the login files (#1441).
#[cfg(not(windows))]
fn shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut command = crate::child::command(shell);
    command.args(["-i", "-l", "-c", PRINT_PATH]);
    printed_path(command, SHELL_WAIT)
}

/// The PATH `command` prints with PRINT_PATH, within `wait`. It starts in a session of its own,
/// without the terminal the browser may have been started from, and with nothing to read, so an
/// rc file that asks something goes on without an answer; one still going after `wait` is
/// given up on, with all it started.
#[cfg(unix)]
fn printed_path(mut command: Command, wait: std::time::Duration) -> Option<String> {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // setsid is async-signal-safe, as pre_exec asks: an interactive shell with the browser's
    // terminal would try to take it over, and stop until it could.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut shell = command.spawn().ok()?;
    let output = shell.stdout.take()?;
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(output).lines().map_while(Result::ok);
        _ = send.send(lines.find_map(|line| line.strip_prefix(SHELL_PATH).map(String::from)));
    });
    let path = receive.recv_timeout(wait);
    if path.is_err() {
        // The session's leader is its process group's: the group is the shell and all it started.
        unsafe { libc::kill(-(shell.id() as libc::pid_t), libc::SIGKILL) };
    }
    // Reaped once it exits, which may be after it printed the PATH.
    std::thread::spawn(move || shell.wait());
    path.ok().flatten()
}

/// The site's demo, on the web, has no programs to run, so it finds no Claude Code.
#[cfg(not(any(unix, windows)))]
fn unix_claude(_: &[PathBuf]) -> Option<PathBuf> {
    None
}

#[cfg(not(any(unix, windows)))]
fn printed_path(_: Command, _: std::time::Duration) -> Option<String> {
    None
}

/// The folders on PATH as a terminal opened now has them: the browser's own PATH, from when
/// Explorer started it, then the system's and the user's as the registry has them now, which an
/// install since may have added to, each `%VARIABLE%` in them filled in from `var`.
#[cfg(any(windows, test))]
fn windows_path(
    var: &dyn Fn(&str) -> Option<String>,
    system: Option<String>,
    user: Option<String>,
) -> Vec<String> {
    [var("PATH"), system, user]
        .into_iter()
        .flatten()
        .flat_map(|path| {
            path.split(';')
                .map(|folder| expand(folder.trim().trim_matches('"'), var))
                .filter(|folder| !folder.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Where Windows looks for a CLI: the folders on `path`, first to last, as a terminal does, then
/// where Claude Code's own installer, winget and npm put one, which may not be on PATH.
#[cfg(any(windows, test))]
fn windows_folders(var: &dyn Fn(&str) -> Option<String>, path: &[String]) -> Vec<PathBuf> {
    let installed: [(&str, &[&str]); 3] = [
        ("USERPROFILE", &[".local", "bin"]),
        ("LOCALAPPDATA", &["Microsoft", "WinGet", "Links"]),
        ("APPDATA", &["npm"]),
    ];
    let installed = installed.into_iter().filter_map(|(root, folder)| {
        let root = PathBuf::from(var(root).filter(|root| !root.is_empty())?);
        Some(folder.iter().fold(root, |path, part| path.join(part)))
    });
    path.iter().map(PathBuf::from).chain(installed).collect()
}

/// The first `name` in `folders`, trying in each the extensions in `pathext` that CreateProcess
/// runs, in its order, as a terminal tries all of PATHEXT's. A .ps1 npm writes beside its .cmd
/// isn't one.
#[cfg(any(windows, test))]
fn windows_program(name: &str, folders: &[PathBuf], pathext: Option<String>) -> Option<PathBuf> {
    let pathext = pathext.unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let extensions: Vec<String> = (pathext.split(';'))
        .map(|extension| extension.trim().to_ascii_lowercase())
        .filter(|extension| [".com", ".exe", ".bat", ".cmd"].contains(&extension.as_str()))
        .collect();
    (folders.iter())
        .flat_map(|folder| {
            (extensions.iter()).map(move |extension| folder.join(format!("{name}{extension}")))
        })
        .find(|program| program.is_file())
}

/// `text` with each `%VARIABLE%` `var` has replaced by its value, as Windows expands a PATH the
/// registry keeps; one it lacks stays as it is.
#[cfg(any(windows, test))]
fn expand(text: &str, var: &dyn Fn(&str) -> Option<String>) -> String {
    let mut expanded = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let Some(length) = rest[start + 1..].find('%') else {
            break;
        };
        let end = start + 1 + length;
        expanded += &rest[..start];
        match var(&rest[start + 1..end]) {
            Some(value) => expanded += &value,
            None => expanded += &rest[start..=end],
        }
        rest = &rest[end + 1..];
    }
    expanded + rest
}

/// The system's PATH, or with `user` the user's, as the registry keeps it now, unexpanded.
#[cfg(windows)]
fn registry_path(user: bool) -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
        RegGetValueW,
    };
    let (key, folder) = match user {
        true => (HKEY_CURRENT_USER, "Environment"),
        false => (
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
    };
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (folder, name) = (wide(folder), wide("Path"));
    let read = |data: *mut core::ffi::c_void, size: &mut u32| unsafe {
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
        RegGetValueW(
            key,
            folder.as_ptr(),
            name.as_ptr(),
            flags,
            std::ptr::null_mut(),
            data,
            size,
        )
    };
    let mut size = 0;
    (read(std::ptr::null_mut(), &mut size) == 0).then_some(())?;
    let mut text = vec![0u16; (size as usize).div_ceil(2)];
    (read(text.as_mut_ptr().cast(), &mut size) == 0).then_some(())?;
    text.truncate(size as usize / 2);
    let text = String::from_utf16_lossy(&text);
    Some(text.trim_end_matches('\0').to_string())
}

pub fn codex_config() -> PathBuf {
    let home = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".codex"));
    home.join("config.toml")
}

/// The table that names the server in Codex's config.
fn codex_table(name: &str) -> String {
    format!("[mcp_servers.{}]", toml_key(name))
}

fn toml_key(name: &str) -> String {
    match name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        true => name.into(),
        false => toml_string(name),
    }
}

fn toml_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Codex's config `text` with the server's table, in place of any it had.
pub fn codex_with(text: &str, entry: &Entry) -> String {
    let mut text = codex_without(text, &entry.name);
    if !text.is_empty() && !text.ends_with("\n\n") {
        text += if text.ends_with('\n') { "\n" } else { "\n\n" };
    }
    text += &format!(
        "{}\ncommand = {}\nargs = [{}]\nenv = {{ {} = {} }}\n",
        codex_table(&entry.name),
        toml_string(&entry.command),
        entry
            .args
            .iter()
            .map(|arg| toml_string(arg))
            .collect::<Vec<_>>()
            .join(", "),
        toml_string(bridge::TOKEN),
        toml_string(&entry.token),
    );
    text
}

/// Codex's config `text` without the server's table, or its subtables.
pub fn codex_without(text: &str, name: &str) -> String {
    let table = codex_table(name);
    let subtable = format!("{}.", &table[..table.len() - 1]);
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == table || trimmed.starts_with(&subtable);
        }
        if !inside {
            kept.push(line);
        }
    }
    while kept.last().is_some_and(|line| line.trim().is_empty()) {
        kept.pop();
    }
    match kept.is_empty() {
        true => String::new(),
        false => kept.join("\n") + "\n",
    }
}

/// Writes `text` to `path` in one step, as a reader never sees half of it, readable by the user
/// alone, as it holds a token.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let error = |error: std::io::Error| format!("{}: {error}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    let temporary = path.with_extension("toml.new");
    write_private(&temporary, text.as_bytes()).map_err(error)?;
    std::fs::rename(&temporary, path).map_err(error)
}

/// Writes a file only its owner can read.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            name: "ilo".into(),
            command: "/Applications/ilo.app/Contents/MacOS/mcp-bridge".into(),
            args: vec!["/Users/me/Library/Application Support/ilo/Agents.sock".into()],
            token: "t0k".into(),
        }
    }

    /// Connect adds one table after the user's own, and a second Connect replaces it rather than
    /// adding another.
    #[test]
    fn codex_gets_one_table_for_the_browser() {
        let mine = "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let once = codex_with(mine, &entry());
        assert_eq!(
            once,
            "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n\n[mcp_servers.ilo]\n\
             command = \"/Applications/ilo.app/Contents/MacOS/mcp-bridge\"\n\
             args = [\"/Users/me/Library/Application Support/ilo/Agents.sock\"]\n\
             env = { \"BROWSER_AGENT_TOKEN\" = \"t0k\" }\n"
        );
        let new_token = Entry {
            token: "n3w".into(),
            ..entry()
        };
        let twice = codex_with(&once, &new_token);
        assert_eq!(twice.matches("[mcp_servers.ilo]").count(), 1);
        assert!(twice.contains("\"n3w\"") && !twice.contains("\"t0k\""));
        assert_eq!(
            codex_with("", &entry()).lines().next(),
            Some("[mcp_servers.ilo]")
        );
    }

    /// Disconnect takes out the browser's table and its subtables, and leaves the rest as it was.
    #[test]
    fn codex_loses_only_the_browsers_table() {
        let mine = "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let connected = codex_with(mine, &entry()) + "[mcp_servers.ilo.env]\nA = \"b\"\n";
        assert_eq!(codex_without(&connected, "ilo"), mine);
        assert_eq!(codex_without(mine, "ilo"), mine);
        assert_eq!(codex_without(&codex_with("", &entry()), "ilo"), "");
        let dev = codex_with(
            mine,
            &Entry {
                name: "ilo-dev".into(),
                ..entry()
            },
        );
        assert!(codex_without(&dev, "ilo").contains("[mcp_servers.ilo-dev]"));
    }

    /// Claude Code adds the bridge for the user, as a stdio server, with the token in its environment.
    #[test]
    fn claude_code_adds_the_bridge_with_the_token() {
        let arguments = claude_add(&entry());
        assert_eq!(
            arguments[..5],
            ["mcp", "add-json", "--scope", "user", "ilo"]
        );
        let server: serde_json::Value = serde_json::from_str(&arguments[5]).unwrap();
        assert_eq!(server["type"], "stdio");
        assert_eq!(
            server["command"],
            "/Applications/ilo.app/Contents/MacOS/mcp-bridge"
        );
        assert_eq!(
            server["args"][0],
            "/Users/me/Library/Application Support/ilo/Agents.sock"
        );
        assert_eq!(server["env"]["BROWSER_AGENT_TOKEN"], "t0k");
    }

    /// A home of its own, with `programs` in it, each a file at a path under it that may run.
    fn home_with(programs: &[&[&str]]) -> PathBuf {
        static HOMES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = HOMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let home =
            std::env::temp_dir().join(format!("claude-home-{}-{number}", std::process::id()));
        _ = std::fs::remove_dir_all(&home);
        for program in programs {
            let program = program
                .iter()
                .fold(home.clone(), |path, part| path.join(part));
            std::fs::create_dir_all(program.parent().unwrap()).unwrap();
            std::fs::write(&program, "").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let runs = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&program, runs).unwrap();
            }
        }
        home
    }

    const NATIVE: &[&str] = &[".local", "bin", "claude.exe"];
    const WINGET: &[&str] = &[
        "AppData",
        "Local",
        "Microsoft",
        "WinGet",
        "Links",
        "claude.exe",
    ];
    const NPM: &[&str] = &["AppData", "Roaming", "npm", "claude.cmd"];

    /// Where Connect on Windows finds `claude` in `home`, with `path` the browser's PATH and
    /// `user` the user's in the registry.
    fn found_in(home: &Path, path: &str, user: Option<&str>) -> Option<PathBuf> {
        cli_in("claude", home, path, user)
    }

    /// Where the browser on Windows finds the CLI `name` in `home`, as `found_in` finds `claude`.
    fn cli_in(name: &str, home: &Path, path: &str, user: Option<&str>) -> Option<PathBuf> {
        let var = |name: &str| match name {
            "USERPROFILE" => Some(home.to_string_lossy().into_owned()),
            "LOCALAPPDATA" => Some(
                home.join("AppData")
                    .join("Local")
                    .to_string_lossy()
                    .into_owned(),
            ),
            "APPDATA" => Some(
                home.join("AppData")
                    .join("Roaming")
                    .to_string_lossy()
                    .into_owned(),
            ),
            "PATH" => Some(path.into()),
            _ => None,
        };
        let path = windows_path(&var, None, user.map(String::from));
        let pathext = Some(".COM;.EXE;.BAT;.CMD;.VBS;.JS".into());
        let found = windows_program(name, &windows_folders(&var, &path), pathext);
        found.map(|program| program.strip_prefix(home).unwrap().to_path_buf())
    }

    fn at(parts: &[&str]) -> Option<PathBuf> {
        Some(parts.iter().collect())
    }

    /// On Windows, Connect finds Claude Code however it was installed, though its folder isn't on
    /// the browser's PATH: by its own installer, winget or npm. When there are two, the first on
    /// PATH runs, as in a terminal; with neither on PATH, the native installer's.
    #[test]
    fn windows_finds_claude_however_it_was_installed() {
        assert_eq!(found_in(&home_with(&[]), "", None), None);
        assert_eq!(found_in(&home_with(&[NATIVE]), "", None), at(NATIVE));
        assert_eq!(found_in(&home_with(&[WINGET]), "", None), at(WINGET));
        assert_eq!(found_in(&home_with(&[NPM]), "", None), at(NPM));
        assert_eq!(found_in(&home_with(&[NATIVE, NPM]), "", None), at(NATIVE));

        let both = home_with(&[NATIVE, WINGET]);
        assert_eq!(found_in(&both, "", None), at(NATIVE));
        let folder = |parts: &[&str]| {
            let parts = &parts[..parts.len() - 1];
            parts
                .iter()
                .fold(both.clone(), |path, part| path.join(part))
                .to_string_lossy()
                .into_owned()
        };
        let (native, winget) = (folder(NATIVE), folder(WINGET));
        assert_eq!(
            found_in(&both, &format!("{winget};{native}"), None),
            at(WINGET)
        );
        assert_eq!(
            found_in(&both, &format!("\"{native}\";{winget}"), None),
            at(NATIVE)
        );
        // The user's PATH as the registry keeps it, added to since the browser started.
        let separator = std::path::MAIN_SEPARATOR;
        let user = format!(
            "%NOT_SET%;%LOCALAPPDATA%{separator}Microsoft{separator}WinGet{separator}Links"
        );
        assert_eq!(found_in(&both, "", Some(&user)), at(WINGET));
    }

    /// In one folder, PATHEXT's order picks, among what CreateProcess runs: a .js isn't one.
    #[test]
    fn windows_takes_pathexts_order_in_a_folder() {
        let home = home_with(&[
            &["bin", "claude.js"],
            &["bin", "claude.cmd"],
            &["bin", "claude.exe"],
        ]);
        let path = home.join("bin").to_string_lossy().into_owned();
        assert_eq!(found_in(&home, &path, None), at(&["bin", "claude.exe"]));
        std::fs::remove_file(home.join("bin").join("claude.exe")).unwrap();
        assert_eq!(found_in(&home, &path, None), at(&["bin", "claude.cmd"]));
        std::fs::remove_file(home.join("bin").join("claude.cmd")).unwrap();
        assert_eq!(found_in(&home, &path, None), None);
    }

    /// The task agents find Codex on Windows as Connect finds Claude Code (#1676): npm's codex.cmd,
    /// with its codex.ps1 beside it, which CreateProcess can't run, or a codex.exe, though the
    /// browser's PATH doesn't have its folder. A folder on PATH that isn't there is passed over.
    #[test]
    fn windows_finds_codex_however_it_was_installed() {
        const NPM_CODEX: &[&str] = &["AppData", "Roaming", "npm", "codex.cmd"];
        const NPM_PS1: &[&str] = &["AppData", "Roaming", "npm", "codex.ps1"];
        const WINGET_CODEX: &[&str] = &[
            "AppData",
            "Local",
            "Microsoft",
            "WinGet",
            "Links",
            "codex.exe",
        ];
        const TOOLS_CODEX: &[&str] = &["tools", "codex.exe"];
        assert_eq!(cli_in("codex", &home_with(&[NPM_PS1]), "", None), None);
        assert_eq!(
            cli_in("codex", &home_with(&[NPM_CODEX, NPM_PS1]), "", None),
            at(NPM_CODEX)
        );
        assert_eq!(
            cli_in("codex", &home_with(&[WINGET_CODEX]), "", None),
            at(WINGET_CODEX)
        );
        // Claude Code's own isn't taken for Codex.
        assert_eq!(cli_in("codex", &home_with(&[NATIVE, NPM]), "", None), None);

        let home = home_with(&[TOOLS_CODEX, NPM_CODEX]);
        let tools = home.join("tools").to_string_lossy().into_owned();
        let gone = home.join("gone").to_string_lossy().into_owned();
        // Off the browser's PATH, npm's; on it, past a folder that isn't there, the first on PATH.
        assert_eq!(cli_in("codex", &home, &gone, None), at(NPM_CODEX));
        assert_eq!(
            cli_in("codex", &home, &format!("{gone};{tools}"), None),
            at(TOOLS_CODEX)
        );
        // Put on the user's PATH by an install since the browser started.
        assert_eq!(cli_in("codex", &home, &gone, Some(&tools)), at(TOOLS_CODEX));
    }

    /// The registry's PATH has its variables filled in, and one that isn't set is kept as it is.
    #[test]
    fn windows_expands_the_registrys_path() {
        let var = |name: &str| (name == "USERPROFILE").then(|| r"C:\Users\Me Too".to_string());
        assert_eq!(
            expand(r"%USERPROFILE%\.local\bin", &var),
            r"C:\Users\Me Too\.local\bin"
        );
        assert_eq!(expand(r"%NOPE%\x;100%", &var), r"%NOPE%\x;100%");
        assert_eq!(
            windows_path(
                &var,
                Some(r"C:\Windows;;".into()),
                Some(r#""%USERPROFILE%\bin""#.into())
            ),
            [r"C:\Windows", r"C:\Users\Me Too\bin"]
        );
    }

    #[cfg(not(windows))]
    const LOCAL: &[&str] = &[".local", "bin", "claude"];
    #[cfg(not(windows))]
    const NPM_PREFIX: &[&str] = &[".npm-global", "bin", "claude"];
    #[cfg(not(windows))]
    const HOMEBREW: &[&str] = &["opt", "homebrew", "bin", "claude"];
    #[cfg(not(windows))]
    const USR_LOCAL: &[&str] = &["usr", "local", "bin", "claude"];
    #[cfg(not(windows))]
    const CLAUDE_LOCAL: &[&str] = &[".claude", "local", "claude"];
    #[cfg(not(windows))]
    const TOOLS: &[&str] = &["tools", "bin", "claude"];

    /// Where Connect on the Mac or Linux finds `claude` in `home`, with `path` the browser's PATH,
    /// `shell` the user's shell's, and `home` the system's root too, for Homebrew's folders.
    #[cfg(not(windows))]
    fn unix_found_in(home: &Path, path: &str, shell: Option<&str>) -> Option<PathBuf> {
        let npmrc = std::fs::read_to_string(home.join(".npmrc")).ok();
        let npm = npm_bin(&|_| None, home, npmrc.as_deref());
        let folders = unix_folders(
            &[Some(path.into()), shell.map(String::from)],
            home,
            npm,
            home,
        );
        let found = unix_claude(&folders);
        found.map(|program| program.strip_prefix(home).unwrap().to_path_buf())
    }

    /// On the Mac and Linux, Connect finds Claude Code wherever a terminal would, though the
    /// browser's PATH has none of it: on the PATH the user's shell gives, as ~/.zshrc sets it, or
    /// where Claude Code's installer, npm or Homebrew put it. Of two, the first on the browser's
    /// PATH runs, then the first on the shell's, as in a terminal, then the installers' in order.
    #[cfg(not(windows))]
    #[test]
    fn finds_claude_however_it_was_installed() {
        assert_eq!(unix_found_in(&home_with(&[]), "/usr/bin:/bin", None), None);
        assert_eq!(
            unix_found_in(&home_with(&[LOCAL]), "/usr/bin:/bin", None),
            at(LOCAL)
        );
        assert_eq!(
            unix_found_in(&home_with(&[HOMEBREW]), "", None),
            at(HOMEBREW)
        );
        assert_eq!(
            unix_found_in(&home_with(&[USR_LOCAL]), "", None),
            at(USR_LOCAL)
        );
        assert_eq!(
            unix_found_in(&home_with(&[CLAUDE_LOCAL]), "", None),
            at(CLAUDE_LOCAL)
        );
        // npm's folder, once ~/.npmrc moves npm's prefix there.
        let npm = home_with(&[NPM_PREFIX]);
        assert_eq!(unix_found_in(&npm, "", None), None);
        std::fs::write(npm.join(".npmrc"), "; mine\nprefix = ~/.npm-global\n").unwrap();
        assert_eq!(unix_found_in(&npm, "", None), at(NPM_PREFIX));
        // A folder only the shell's PATH has.
        let tools = home_with(&[TOOLS]);
        let folder = tools.join("tools/bin");
        assert_eq!(unix_found_in(&tools, "/usr/bin", None), None);
        assert_eq!(
            unix_found_in(&tools, "/usr/bin", folder.to_str()),
            at(TOOLS)
        );

        let two = home_with(&[TOOLS, LOCAL, HOMEBREW]);
        let (tools, local) = (two.join("tools/bin"), two.join(".local/bin"));
        let (tools, local) = (tools.to_str().unwrap(), local.to_str().unwrap());
        assert_eq!(unix_found_in(&two, "", None), at(LOCAL));
        assert_eq!(unix_found_in(&two, "", Some(tools)), at(TOOLS));
        assert_eq!(
            unix_found_in(&two, "", Some(&format!("{local}:{tools}"))),
            at(LOCAL)
        );
        assert_eq!(unix_found_in(&two, local, Some(tools)), at(LOCAL));
    }

    /// A `claude` that isn't a program, a folder of that name or a file that may not run, is passed
    /// over, as a shell passes it over.
    #[cfg(not(windows))]
    #[test]
    fn passes_over_a_claude_that_doesnt_run() {
        use std::os::unix::fs::PermissionsExt;
        let home = home_with(&[TOOLS, HOMEBREW]);
        let tools = home.join("tools/bin");
        std::fs::set_permissions(tools.join("claude"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        std::fs::create_dir_all(home.join(".local/bin/claude")).unwrap();
        assert_eq!(
            unix_found_in(&home, tools.to_str().unwrap(), None),
            at(HOMEBREW)
        );
    }

    /// claude runs with the browser's PATH, the shell's and the installers' folders, each once,
    /// without the folders the shell's names relative to wherever it runs.
    #[cfg(not(windows))]
    #[test]
    fn claude_gets_each_folder_once() {
        let paths = [
            Some("/usr/bin:/bin".into()),
            Some("/Users/me/.local/bin:bin:/usr/bin::/opt/homebrew/bin".into()),
        ];
        let folders = unix_folders(&paths, Path::new("/Users/me"), None, Path::new("/"));
        let path = [
            "/usr/bin",
            "/bin",
            "/Users/me/.local/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/Users/me/.claude/local",
        ];
        assert_eq!(folders, path.map(PathBuf::from));
    }

    /// npm's folder where the user moved its prefix, as npm's docs and its config have it.
    #[cfg(not(windows))]
    #[test]
    fn finds_npms_folder_where_the_user_moved_it() {
        let home = Path::new("/Users/me");
        let unset = |_: &str| None;
        let bin = |path: &str| Some(PathBuf::from(path));
        assert_eq!(npm_bin(&unset, home, None), None);
        assert_eq!(
            npm_bin(&unset, home, Some("registry=https://registry.npmjs.org/\n")),
            None
        );
        let packages = Some("prefix=${HOME}/.npm-packages");
        assert_eq!(
            npm_bin(&unset, home, packages),
            bin("/Users/me/.npm-packages/bin")
        );
        assert_eq!(
            npm_bin(&unset, home, Some("prefix = \"/opt/npm\"")),
            bin("/opt/npm/bin")
        );
        let set = |name: &str| (name == "npm_config_prefix").then(|| "~/npm".to_string());
        assert_eq!(
            npm_bin(&set, home, Some("prefix=/opt/npm")),
            bin("/Users/me/npm/bin")
        );
    }

    #[cfg(not(windows))]
    fn sh(script: &str) -> Command {
        let mut command = crate::child::command("/bin/sh");
        command.args(["-c", script]);
        command
    }

    /// The shell's PATH is read past what its rc files print, and an rc file that asks something
    /// gets no answer and goes on.
    #[cfg(not(windows))]
    #[test]
    fn reads_the_path_the_shell_prints() {
        let wait = std::time::Duration::from_secs(10);
        let rc = format!("echo Welcome; read answer; PATH=/tools/bin:/usr/bin; {PRINT_PATH}");
        assert_eq!(
            printed_path(sh(&rc), wait).as_deref(),
            Some("/tools/bin:/usr/bin")
        );
        assert_eq!(printed_path(sh("echo Welcome"), wait), None);
        assert_eq!(
            printed_path(crate::child::command("/no/such/shell"), wait),
            None
        );
    }

    /// A shell still starting after the wait, as on an rc file that waits on the network, is given
    /// up on then.
    #[cfg(not(windows))]
    #[test]
    fn gives_up_on_a_shell_that_hangs() {
        let start = std::time::Instant::now();
        let hangs = sh(&format!("sleep 30; {PRINT_PATH}"));
        assert_eq!(
            printed_path(hangs, std::time::Duration::from_millis(200)),
            None
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
    }
}
