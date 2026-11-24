//! A walled task's rules (#4101), in the order the system reads them: Codex's base (base.sb),
//! which refuses what nothing after it allows; the system's own reads (system.rs); the task's own
//! folders and files; the assistant's program and its sign-in; the folders above each of those,
//! for their names alone; the writes; the network; and last the mark. A folder's or a file's path
//! is only ever a parameter, never in the text, so the text holds no name of the user's.
//!
//! In the system's rules a later line wins over an earlier one, so each deny below comes after the
//! allows it narrows.

use std::io;
use std::path::{Path, PathBuf};

use super::launch::{PARAMETERS_MAX, Rules};
use super::leftovers::{Mark, PREFIX};
use crate::agents::files::{self, Op, Place};
use crate::agents::runner::Cli;
use crate::agents::tier::Tier;

pub mod system;

/// The assistant's own home, `CODEX_HOME` or `~/.claude`.
pub const CLI_HOME: &str = "CLI_HOME";
/// Claude Code's `~/.claude.json`, beside its home and not in it.
pub const CLAUDE_CONFIG: &str = "CLAUDE_CONFIG";
pub const MARK: &str = "MARK";
/// The folder the user granted, and ilo's copies of the files they attached.
pub const FOLDER: &str = "FOLDER";
pub const ATTACHED: &str = "ATTACHED";
/// The task's own of ilo's: its folder, its MCP config and the instructions beside it (ilo_reads).
pub const TASK_DIR: &str = "TASK_DIR";
pub const CONFIG: &str = "CONFIG";
pub const CONFIG_MD: &str = "CONFIG_MD";
/// The task's own temporary folder, and ilo's MCP socket.
pub const TMP: &str = "TMP";
pub const SOCKET: &str = "SOCKET";
/// The folder the assistant's program is in.
pub const PROGRAM_DIR: &str = "PROGRAM_DIR";

/// The parameters naming the folders above every path the text names, one each.
const UP: [&str; PARAMETERS_MAX] = [
    "UP_0", "UP_1", "UP_2", "UP_3", "UP_4", "UP_5", "UP_6", "UP_7", "UP_8", "UP_9", "UP_10",
    "UP_11", "UP_12", "UP_13", "UP_14", "UP_15", "UP_16", "UP_17", "UP_18", "UP_19", "UP_20",
    "UP_21", "UP_22", "UP_23", "UP_24", "UP_25", "UP_26", "UP_27", "UP_28", "UP_29", "UP_30",
    "UP_31",
];

const BASE: &str = include_str!("base.sb");

/// What the assistant writes in its home to keep its sign-in: Codex replaces `auth.json` by a
/// rename from a `.auth-` file beside it and takes the two locks; Claude Code keeps its sign-in in
/// the Keychain and falls back to the one file. A refused sign-in save signs the user out.
pub(super) fn sign_in(cli: Cli) -> &'static [&'static str] {
    match cli {
        Cli::Codex => &["auth.json", "auth.lock", "auth.json.lock"],
        Cli::Claude => &[".credentials.json"],
    }
}

/// The start of the temporary file a sign-in save renames into place, if the assistant has one.
fn sign_in_temp(cli: Cli) -> Option<&'static str> {
    match cli {
        Cli::Codex => Some(".auth-"),
        Cli::Claude => None,
    }
}

/// What a later start reads or runs, refused by name after every allow, so no allow added later
/// reaches one. A name ending in `/` is a folder, one ending in `*` every name it starts.
pub fn kept_out(cli: Cli) -> &'static [&'static str] {
    match cli {
        Cli::Codex => &[
            "config.toml",
            "managed_config.toml",
            "work.config.toml",
            ".env",
            "AGENTS.md",
            "rules/",
            "skills/",
            "prompts/",
            "plugins/",
            "hooks/",
            "hook_outputs/",
            "bin/",
            "codex-path/",
            "codex-resources/",
            "memories/",
            "shell_snapshots/",
            "tmp/",
            ".tmp/",
            "sessions/",
            "archived_sessions/",
            "state_5.sqlite*",
            "secrets/",
            "editor/",
            "worktrees/",
        ],
        Cli::Claude => &[
            "CLAUDE.md",
            "settings.json",
            "settings.local.json",
            "keybindings.json",
            "themes/",
            "rules/",
            "skills/",
            "commands/",
            "output-styles/",
            "agents/",
            "workflows/",
            "agent-memory/",
            "plugins/",
            "projects/",
            "remote-settings.json",
            "policy-limits.json",
            "jobs/",
            "daemon/",
            "shell-snapshots/",
            "backups/",
        ],
    }
}

/// A path under the home, as the text names it: the home's parameter and a name joined to it.
fn under_home(name: &str) -> String {
    format!("(string-append (param \"{CLI_HOME}\") \"/{name}\")")
}

/// A regex on a path under the home: the home's parameter quoted, as a regex can't take a
/// parameter's value as it is without reading its dots and brackets.
fn home_regex(tail: &str) -> String {
    format!("(regex (string-append \"^\" (regex-quote (param \"{CLI_HOME}\")) \"{tail}\"))")
}

/// Escapes `name` for a regex inside the text's string, where the backslash is itself escaped.
fn regex_escaped(name: &str) -> String {
    let mut escaped = String::new();
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && !matches!(c, '_' | '-' | '/') {
            escaped.push_str("\\\\");
        }
        escaped.push(c);
    }
    escaped
}

/// The filters of the home's write allows: the sign-in's names, and its temporary files as direct
/// children of the home.
fn sign_in_filters(cli: Cli) -> Vec<String> {
    let temp =
        sign_in_temp(cli).map(|start| home_regex(&format!("/{}[^/]+$", regex_escaped(start))));
    let names = sign_in(cli)
        .iter()
        .map(|name| format!("(literal {})", under_home(name)));
    temp.into_iter().chain(names).collect()
}

fn kept_out_filter(name: &str) -> String {
    if let Some(folder) = name.strip_suffix('/') {
        format!("(subpath {})", under_home(folder))
    } else if let Some(start) = name.strip_suffix('*') {
        home_regex(&format!("/{}", regex_escaped(start)))
    } else {
        format!("(literal {})", under_home(name))
    }
}

/// What `cli` may write in its own home, in order: the sign-in's allows, the names a later start
/// reads denied, then nothing but a plain file made under an allowed name, so no link, folder or
/// pipe there leads the user's own later start somewhere else.
pub fn home(cli: Cli) -> String {
    let allows = sign_in_filters(cli);
    let mut text = String::new();
    for filter in &allows {
        text.push_str(&format!("(allow file-write* {filter})\n"));
    }
    let denied: Vec<String> = kept_out(cli)
        .iter()
        .map(|name| kept_out_filter(name))
        .collect();
    text.push_str(&format!("(deny file-write* {})\n", denied.join(" ")));
    if cli == Cli::Claude {
        text.push_str(&format!(
            "(deny file-write* (literal (param \"{CLAUDE_CONFIG}\")))\n"
        ));
    }
    text.push_str(&format!(
        "(deny file-write-create (require-all (require-any {}) (require-not (vnode-type REGULAR-FILE))))\n",
        allows.join(" ")
    ));
    text
}

/// The wall's mark, which the system answers is allowed for what is inside and no other wall's:
/// the last of the text's lookup rules, every name of ilo's refused and then this wall's own
/// allowed, so the mark holds whatever lookup a later line allows above it. The control is one of
/// the names refused.
pub fn mark() -> String {
    format!(
        "(deny mach-lookup (global-name-prefix \"{PREFIX}\"))\n\
         (allow mach-lookup (global-name (param \"{MARK}\")))\n"
    )
}

/// The parameters the text names, each path resolved, since the system matches a rule on where a
/// path leads and the launcher resolves nothing. The mark is a name, not a path.
pub fn params(
    paths: Vec<(&'static str, PathBuf)>,
    mark: &Mark,
) -> io::Result<Vec<(&'static str, PathBuf)>> {
    let mut params = Vec::with_capacity(paths.len() + 1);
    for (key, path) in paths {
        params.push((key, resolved(&path)?));
    }
    let name = mark.as_cstr().to_str().expect("a mark is hex");
    params.push((MARK, PathBuf::from(name)));
    Ok(params)
}

/// `path` with every link in it followed. A file not made yet, such as a sign-in a first save
/// makes, is its folder resolved and its own name.
fn resolved(path: &std::path::Path) -> io::Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Err(why) if why.kind() == io::ErrorKind::NotFound => {
            let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
                return Err(why);
            };
            Ok(std::fs::canonicalize(folder)?.join(name))
        }
        answer => answer,
    }
}

/// What the system does for an assistant that verifies its own service's certificates, from
/// Codex's seatbelt_network_policy.sbpl at the commit base.sb names, none of its network allows:
/// the platform's own sockets and the lookups for trust and the network's configuration.
const TRUST: &str = "\
(allow system-socket (require-all (socket-domain AF_SYSTEM) (socket-protocol 2)))
(allow mach-lookup (global-name \"com.apple.bsd.dirhelper\") \
(global-name \"com.apple.system.opendirectoryd.membership\") \
(global-name \"com.apple.SecurityServer\") (global-name \"com.apple.networkd\") \
(global-name \"com.apple.ocspd\") (global-name \"com.apple.trustd.agent\") \
(global-name \"com.apple.SystemConfiguration.DNSConfiguration\") \
(global-name \"com.apple.SystemConfiguration.configd\"))
";

/// The ioctl that types into a terminal's input, `TIOCSTI` on the Mac.
const TIOCSTI: u64 = 0x8001_7472;

/// Codex's own hardening at the commit base.sb names (seatbelt.rs): no service looked up by its
/// XPC name, no change to a file through a handle opened to read it (F_MAKECOMPRESSED and
/// F_TRANSFEREXTENTS), and nothing typed into a terminal the assistant has.
fn hardening() -> String {
    format!(
        "(deny mach-lookup (xpc-service-name-prefix \"\"))\n\
         (deny system-fcntl (fcntl-command 80 110))\n\
         (deny file-ioctl (ioctl-command {TIOCSTI}))\n"
    )
}

/// What the system is asked for each operation of the table (files.rs). A move is a new name and
/// the old one gone; replacing a file is the same over the other's name.
fn seatbelt(op: Op) -> &'static [&'static str] {
    match op {
        Op::Read | Op::Search => &["file-read*", "file-test-existence"],
        Op::Make => &["file-write-create", "file-write-data"],
        Op::Change => &[
            "file-write-data",
            "file-write-mode",
            "file-write-times",
            "file-write-xattr",
        ],
        Op::Delete => &["file-write-unlink"],
        Op::Move | Op::Replace => &["file-write-create", "file-write-unlink"],
    }
}

/// Everything `level` may do in `place`, each operation once, in the table's order.
fn place_ops(level: Tier, place: Place) -> Vec<&'static str> {
    let mut ops = Vec::new();
    for op in Op::ALL {
        if files::allows(level, place, op) {
            for each in seatbelt(op) {
                if !ops.contains(each) {
                    ops.push(*each);
                }
            }
        }
    }
    ops
}

/// What a walled task's text depends on: never a path, so it holds no name of the user's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub cli: Cli,
    pub level: Tier,
    pub folder: bool,
    pub attached: bool,
    /// The task's listener's port.
    pub port: u16,
    /// How many folders above the named paths have a parameter.
    pub ancestors: usize,
}

/// The whole text of a walled task's rules.
pub fn text(shape: &Shape) -> String {
    let mut text = String::from(BASE);
    text.push_str(&system::text());
    for (key, given, place) in [
        (FOLDER, shape.folder, Place::Folder),
        (ATTACHED, shape.attached, Place::Attached),
    ] {
        if given {
            let ops = place_ops(shape.level, place).join(" ");
            text.push_str(&format!("(allow {ops} (subpath (param \"{key}\")))\n"));
        }
    }
    text.push_str(&format!(
        "(allow file-read* file-test-existence (subpath (param \"{TASK_DIR}\")))\n\
         (allow file-read* file-test-existence (literal (param \"{CONFIG}\")) \
         (literal (param \"{CONFIG_MD}\")))\n\
         (allow file-read* file-test-existence file-write* (subpath (param \"{TMP}\")))\n\
         (allow file-read* file-test-existence file-map-executable \
         (subpath (param \"{PROGRAM_DIR}\")))\n\
         (allow file-read* file-test-existence {})\n",
        sign_in_filters(shape.cli).join(" ")
    ));
    if shape.cli == Cli::Claude {
        text.push_str(&format!(
            "(allow file-read* file-test-existence (literal (param \"{CLAUDE_CONFIG}\")))\n"
        ));
    }
    for up in &UP[..shape.ancestors] {
        text.push_str(&format!(
            "(allow file-read-metadata file-test-existence (literal (param \"{up}\")))\n"
        ));
    }
    // After every other write allow, so its denies win over a folder that holds the home.
    text.push_str(&home(shape.cli));
    text.push_str(&format!(
        "(deny network*)\n\
         (allow network-outbound (remote ip \"localhost:{}\"))\n\
         (allow system-socket (socket-domain AF_UNIX))\n\
         (allow network-outbound (remote unix-socket (path-literal (param \"{SOCKET}\"))))\n\
         (allow network-bind (local unix-socket (subpath (param \"{TMP}\"))))\n\
         (allow network-outbound (remote unix-socket (subpath (param \"{TMP}\"))))\n",
        shape.port
    ));
    text.push_str(TRUST);
    text.push_str(&hardening());
    text.push_str(&mark());
    text
}

/// The paths a walled task's rules name, as put_up has them.
pub struct Paths<'a> {
    pub folder: Option<&'a Path>,
    pub attached: Option<&'a Path>,
    pub task_dir: &'a Path,
    pub config: &'a Path,
    pub tmp: &'a Path,
    pub socket: &'a Path,
    pub program_dir: &'a Path,
    pub cli_home: &'a Path,
    /// Claude Code's `~/.claude.json`, which Codex has none of.
    pub claude_config: Option<&'a Path>,
}

/// Why rules weren't made for paths this deep, a sentence that names none of them.
pub const TOO_DEEP: &str = "the task's folders are too deep for the launcher's parameters";

/// The rules for a walled task: its text and every parameter it names, each resolved.
pub fn rules(cli: Cli, level: Tier, port: u16, paths: &Paths, mark: &Mark) -> io::Result<Rules> {
    let mut named = Vec::new();
    if let Some(folder) = paths.folder {
        named.push((FOLDER, folder.to_path_buf()));
    }
    if let Some(attached) = paths.attached {
        named.push((ATTACHED, attached.to_path_buf()));
    }
    named.extend([
        (TASK_DIR, paths.task_dir.to_path_buf()),
        (CONFIG, paths.config.to_path_buf()),
        (CONFIG_MD, paths.config.with_extension("md")),
        (TMP, paths.tmp.to_path_buf()),
        (SOCKET, paths.socket.to_path_buf()),
        (PROGRAM_DIR, paths.program_dir.to_path_buf()),
        (CLI_HOME, paths.cli_home.to_path_buf()),
    ]);
    if let Some(config) = paths.claude_config {
        named.push((CLAUDE_CONFIG, config.to_path_buf()));
    }
    let mut params = params(named, mark)?;
    let ups = ancestors(
        (params.iter())
            .filter(|(key, _)| *key != MARK)
            .map(|(_, path)| path.as_path()),
    );
    if params.len() + ups.len() > PARAMETERS_MAX {
        return Err(io::Error::other(TOO_DEEP));
    }
    let shape = Shape {
        cli,
        level,
        folder: paths.folder.is_some(),
        attached: paths.attached.is_some(),
        port,
        ancestors: ups.len(),
    };
    params.extend(UP.iter().copied().zip(ups));
    Ok(Rules {
        text: text(&shape),
        params,
    })
}

/// Every folder above `paths`, each once and in the order met, but the root, which the system's
/// part reads, and those paths themselves.
fn ancestors<'a>(paths: impl Iterator<Item = &'a Path>) -> Vec<PathBuf> {
    let mut ups: Vec<PathBuf> = Vec::new();
    for path in paths {
        for up in path.ancestors().skip(1) {
            if up.parent().is_some() && !ups.iter().any(|known| known == up) {
                ups.push(up.to_path_buf());
            }
        }
    }
    ups
}

#[cfg(test)]
mod tests;
