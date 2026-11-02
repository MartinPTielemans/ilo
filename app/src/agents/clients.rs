//! Connect and Disconnect: the browser's bridge in an agent client's own config, as a stdio MCP
//! server with the agent's token, and out of it again, and nothing else of the config changed. Claude Code's is in ~/.claude.json, which the
//! running client rewrites, so its own `claude mcp` command changes it; Codex's is
//! ~/.codex/config.toml, which Codex only reads, so the browser edits its one table. Any other
//! MCP client gets the bridge's command and its token to paste.

use std::path::{Path, PathBuf};
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
}

/// The configs in the user's home.
pub struct System;

impl Clients for System {
    fn connect(&self, client: Client, entry: &Entry) -> Result<(), String> {
        match client {
            Client::ClaudeCode => {
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
            Client::ClaudeCode => claude(&["mcp", "remove", "--scope", "user", name]),
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

fn home() -> PathBuf {
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

/// Runs Claude Code's `claude` with `arguments`.
fn claude(arguments: &[&str]) -> Result<(), String> {
    let output = claude_command()?
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    match output.status.success() {
        true => Ok(()),
        false if output.status.code() == Some(127) => Err(NO_CLAUDE.into()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// The app may not have the user's PATH, so `claude` runs in their login shell, which does, with
/// the arguments as the shell's own rather than in its command line.
#[cfg(not(windows))]
fn claude_command() -> Result<Command, String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut command = Command::new(shell);
    command.args(["-l", "-c", r#"exec claude "$@""#, "claude"]);
    Ok(command)
}

/// `claude` where a terminal opened now would find it, or where an installer put it (#1420),
/// with that terminal's PATH, where npm's claude.cmd finds node. Rust runs a .cmd through cmd.exe
/// with each argument quoted for it, the JSON's quotes doubled, which node reads back whole.
#[cfg(windows)]
fn claude_command() -> Result<Command, String> {
    use std::os::windows::process::CommandExt;
    let var = |name: &str| std::env::var(name).ok();
    let path = windows_path(&var, registry_path(false), registry_path(true));
    let folders = windows_folders(&var, &path);
    let program = windows_claude(&folders, var("PATHEXT")).ok_or(NO_CLAUDE)?;
    let mut command = Command::new(program);
    command.env("PATH", path.join(";"));
    // A console program the windowed browser starts would get a console window of its own.
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    Ok(command)
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

/// Where Windows looks for `claude`: the folders on `path`, first to last, as a terminal does,
/// then where Claude Code's own installer, winget and npm put it, which may not be on PATH.
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

/// The first `claude` in `folders`, trying in each the extensions in `pathext` that CreateProcess
/// runs, in its order, as a terminal tries all of PATHEXT's.
#[cfg(any(windows, test))]
fn windows_claude(folders: &[PathBuf], pathext: Option<String>) -> Option<PathBuf> {
    let pathext = pathext.unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let extensions: Vec<String> = (pathext.split(';'))
        .map(|extension| extension.trim().to_ascii_lowercase())
        .filter(|extension| [".com", ".exe", ".bat", ".cmd"].contains(&extension.as_str()))
        .collect();
    (folders.iter())
        .flat_map(|folder| (extensions.iter()).map(move |extension| folder.join(format!("claude{extension}"))))
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
        RegGetValueW(key, folder.as_ptr(), name.as_ptr(), flags, std::ptr::null_mut(), data, size)
    };
    let mut size = 0;
    (read(std::ptr::null_mut(), &mut size) == 0).then_some(())?;
    let mut text = vec![0u16; (size as usize).div_ceil(2)];
    (read(text.as_mut_ptr().cast(), &mut size) == 0).then_some(())?;
    text.truncate(size as usize / 2);
    let text = String::from_utf16_lossy(&text);
    Some(text.trim_end_matches('\0').to_string())
}

fn codex_config() -> PathBuf {
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
        entry.args.iter().map(|arg| toml_string(arg)).collect::<Vec<_>>().join(", "),
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

    /// A home of its own, with `programs` in it, each a file at a path under it.
    fn home_with(programs: &[&[&str]]) -> PathBuf {
        static HOMES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = HOMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("claude-home-{}-{number}", std::process::id()));
        _ = std::fs::remove_dir_all(&home);
        for program in programs {
            let program = program.iter().fold(home.clone(), |path, part| path.join(part));
            std::fs::create_dir_all(program.parent().unwrap()).unwrap();
            std::fs::write(program, "").unwrap();
        }
        home
    }

    const NATIVE: &[&str] = &[".local", "bin", "claude.exe"];
    const WINGET: &[&str] = &["AppData", "Local", "Microsoft", "WinGet", "Links", "claude.exe"];
    const NPM: &[&str] = &["AppData", "Roaming", "npm", "claude.cmd"];

    /// Where Connect on Windows finds `claude` in `home`, with `path` the browser's PATH and
    /// `user` the user's in the registry.
    fn found_in(home: &Path, path: &str, user: Option<&str>) -> Option<PathBuf> {
        let var = |name: &str| match name {
            "USERPROFILE" => Some(home.to_string_lossy().into_owned()),
            "LOCALAPPDATA" => Some(home.join("AppData").join("Local").to_string_lossy().into_owned()),
            "APPDATA" => Some(home.join("AppData").join("Roaming").to_string_lossy().into_owned()),
            "PATH" => Some(path.into()),
            _ => None,
        };
        let path = windows_path(&var, None, user.map(String::from));
        let found = windows_claude(&windows_folders(&var, &path), Some(".COM;.EXE;.BAT;.CMD;.VBS;.JS".into()));
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
            parts.iter().fold(both.clone(), |path, part| path.join(part)).to_string_lossy().into_owned()
        };
        let (native, winget) = (folder(NATIVE), folder(WINGET));
        assert_eq!(found_in(&both, &format!("{winget};{native}"), None), at(WINGET));
        assert_eq!(found_in(&both, &format!("\"{native}\";{winget}"), None), at(NATIVE));
        // The user's PATH as the registry keeps it, added to since the browser started.
        let separator = std::path::MAIN_SEPARATOR;
        let user = format!("%NOT_SET%;%LOCALAPPDATA%{separator}Microsoft{separator}WinGet{separator}Links");
        assert_eq!(found_in(&both, "", Some(&user)), at(WINGET));
    }

    /// In one folder, PATHEXT's order picks, among what CreateProcess runs: a .js isn't one.
    #[test]
    fn windows_takes_pathexts_order_in_a_folder() {
        let home = home_with(&[&["bin", "claude.js"], &["bin", "claude.cmd"], &["bin", "claude.exe"]]);
        let path = home.join("bin").to_string_lossy().into_owned();
        assert_eq!(found_in(&home, &path, None), at(&["bin", "claude.exe"]));
        std::fs::remove_file(home.join("bin").join("claude.exe")).unwrap();
        assert_eq!(found_in(&home, &path, None), at(&["bin", "claude.cmd"]));
        std::fs::remove_file(home.join("bin").join("claude.cmd")).unwrap();
        assert_eq!(found_in(&home, &path, None), None);
    }

    /// The registry's PATH has its variables filled in, and one that isn't set is kept as it is.
    #[test]
    fn windows_expands_the_registrys_path() {
        let var = |name: &str| (name == "USERPROFILE").then(|| r"C:\Users\Me Too".to_string());
        assert_eq!(expand(r"%USERPROFILE%\.local\bin", &var), r"C:\Users\Me Too\.local\bin");
        assert_eq!(expand(r"%NOPE%\x;100%", &var), r"%NOPE%\x;100%");
        assert_eq!(
            windows_path(&var, Some(r"C:\Windows;;".into()), Some(r#""%USERPROFILE%\bin""#.into())),
            [r"C:\Windows", r"C:\Users\Me Too\bin"]
        );
    }
}
