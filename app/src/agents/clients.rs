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

/// Runs Claude Code's `claude` with `arguments`. The app may not have the user's PATH, so it runs
/// in their login shell, which does, with the arguments as the shell's own rather than in its
/// command line.
fn claude(arguments: &[&str]) -> Result<(), String> {
    let mut command = match cfg!(windows) {
        true => Command::new("claude.cmd"),
        false => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            let mut command = Command::new(shell);
            command.args(["-l", "-c", r#"exec claude "$@""#, "claude"]);
            command
        }
    };
    let output = command
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    match output.status.success() {
        true => Ok(()),
        false if output.status.code() == Some(127) => {
            Err("Claude Code's claude command isn't installed.".into())
        }
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
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
}
