//! What `ilo-command` does with its arguments (agents::cli): each run is one connection to the
//! server as the `ilo` command's agent, one tool call, and the answer printed. Its words map onto
//! the server's tools, whose list it reads each time, so help and the arguments it takes can't
//! drift from them. It decides nothing: every refusal, question and handoff is the browser's.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Map, Value, json};

use super::cli::LINK;
use crate::product::PRODUCT;

/// The variable that names the agent for its card, as `--as` does: the command's name, upper
/// case, then `_AS` (build.rs).
pub const NAME: &str = env!("PRODUCT_AS");

/// The card's name for an agent that gives none.
pub const UNNAMED: &str = "Shell agent";

/// How a run ends, as a script reads it.
pub mod exit {
    /// The browser answered with an error, or refused.
    pub const REFUSED: i32 = 1;
    /// The command line isn't one the command takes.
    pub const USAGE: i32 = 2;
    /// The browser isn't running, or agents are off.
    pub const NOT_RUNNING: i32 = 3;
    /// The browser doesn't know the command's agent: Remove or Revoke took it away.
    pub const UNKNOWN: i32 = 4;
}

/// A word the agent types, the tool it calls, and the arguments it takes in order, as help lists
/// them.
struct Word {
    word: &'static str,
    tool: &'static str,
    args: &'static [&'static str],
}

const fn word(word: &'static str, tool: &'static str, args: &'static [&'static str]) -> Word {
    Word { word, tool, args }
}

/// The words, in help's order. `wait`, `run` and `request` are run_steps' and request's with
/// arguments of their own (call).
const WORDS: &[Word] = &[
    word("tabs", "list_tabs", &[]),
    word("open", "open_tab", &["url"]),
    word("go", "navigate", &["url"]),
    word("look", "snapshot", &[]),
    word("read", "read_page", &[]),
    word("click", "click", &["ref"]),
    word("type", "type", &["ref", "text"]),
    word("select", "select", &["ref", "option"]),
    word("key", "press_key", &["key"]),
    word("scroll", "scroll", &["up|down|ref"]),
    word("wait", "run_steps", &["text"]),
    word("run", "run_steps", &[]),
    word("shot", "screenshot", &[]),
    word("request", "request", &["method", "path"]),
    word("close", "close_tab", &[]),
    word("pin", "pin_tab", &[]),
    word("unpin", "unpin_tab", &[]),
    word("remove", "remove_tab", &[]),
    word("sort", "sort_tabs_by_site", &[]),
    word("split", "split_tabs", &["tabs"]),
    word("add-to-split", "add_to_split", &["split"]),
    word("separate", "separate_tab", &[]),
    word("splits", "list_split", &[]),
    word("upload", "upload", &["ref"]),
    word("hand-off", "hand_off", &["reason"]),
    word("ask-for", "ask_for_tab", &["reason"]),
    word("done", "done", &[]),
];

/// A run's command line, read.
#[derive(Debug, Default, PartialEq)]
pub struct Line {
    pub words: Vec<String>,
    /// --json: the answer as JSON, for a script.
    pub json: bool,
    /// --as: the agent's name, for its card.
    pub name: Option<String>,
    /// -o: where a screenshot goes.
    pub output: Option<PathBuf>,
}

/// Takes the command's own options out of `args`, wherever they are; the rest are the tool's.
pub fn line(args: impl IntoIterator<Item = String>) -> Result<Line, String> {
    let mut line = Line::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => line.json = true,
            "--as" => line.name = Some(args.next().ok_or("--as takes the agent's name.")?),
            "-o" | "--output" => {
                line.output = Some(args.next().ok_or("-o takes a file's path.")?.into())
            }
            _ => line.words.push(arg),
        }
    }
    Ok(line)
}

/// The tool `words` call, with its arguments, as the server's `tools` list types them. Text read
/// from stdin is `input`, for `run` and `request`.
pub fn call(
    words: &[String],
    tools: &[Value],
    input: impl FnOnce() -> String,
) -> Result<(String, Value), String> {
    let Some((first, rest)) = words.split_first() else {
        return Err(usage());
    };
    let known = WORDS.iter().find(|word| word.word == first);
    let tool = known.map_or(first.as_str(), |word| word.tool);
    let schema = schema(tools, tool).ok_or_else(|| {
        format!("There's no command “{first}”. Run {LINK} help for the commands.")
    })?;
    let (positional, mut arguments) = options(rest, schema, first)?;
    let positional = positional.as_slice();
    match (known.map(|word| word.word), positional) {
        (Some("scroll"), [where_]) => {
            let key = match where_.as_str() {
                "up" | "down" => "direction",
                _ => "ref",
            };
            arguments.insert(key.into(), json!(where_));
        }
        (Some("wait"), [text @ ..]) if !text.is_empty() => {
            let mut step = Map::new();
            step.insert("action".into(), json!("wait"));
            let gone = arguments.remove("gone").is_some();
            step.insert(
                if gone { "gone" } else { "text" }.into(),
                json!(text.join(" ")),
            );
            if let Some(seconds) = arguments.remove("for") {
                step.insert("seconds".into(), seconds);
            }
            arguments.insert("steps".into(), json!([step]));
        }
        (Some("run"), []) => {
            let steps: Result<Vec<Value>, String> = input()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(step)
                .collect();
            arguments.insert("steps".into(), json!(steps?));
        }
        (Some("request"), [method, path]) => {
            let mut request = Map::new();
            request.insert("method".into(), json!(method.to_uppercase()));
            request.insert("path".into(), json!(path));
            for key in ["body", "what", "headers"] {
                if let Some(value) = arguments.remove(key) {
                    request.insert(key.into(), value);
                }
            }
            arguments.insert("requests".into(), json!([request]));
        }
        (Some("request"), []) => {
            let requests: Value = serde_json::from_str(&input())
                .map_err(|_| "request takes a method and a path, or JSON on stdin.")?;
            let requests = match requests {
                Value::Array(_) => requests,
                one => json!([one]),
            };
            arguments.insert("requests".into(), requests);
        }
        (Some("split"), tabs) if !tabs.is_empty() => {
            let tabs: Result<Vec<u64>, _> = tabs.iter().map(|tab| tab.parse::<u64>()).collect();
            let tabs = tabs.map_err(|_| "split takes the tabs' ids.")?;
            arguments.insert("tabs".into(), json!(tabs));
        }
        (Some("scroll" | "wait" | "run" | "request" | "split"), _) => {
            return Err(format!("Usage: {}", synopsis(known.unwrap())));
        }
        _ => {
            let names: Vec<&str> = match known {
                Some(word) => word.args.to_vec(),
                None => required(schema),
            };
            if positional.len() > names.len() && names.is_empty() {
                return Err(format!(
                    "{first} takes no words but options. Run {LINK} help {first}."
                ));
            }
            for (index, name) in names.iter().enumerate() {
                let Some(value) = positional.get(index) else {
                    break;
                };
                // The last takes the words left, as `type e12 Lisbon museums` types both.
                let value = match index + 1 == names.len() {
                    true => positional[index..].join(" "),
                    false => value.clone(),
                };
                arguments.insert((*name).into(), typed(schema, name, &value)?);
            }
        }
    }
    for name in required(schema) {
        if !arguments.contains_key(name) {
            return Err(format!("{first} needs {name}. Run {LINK} help {first}."));
        }
    }
    Ok((tool.into(), Value::Object(arguments)))
}

/// A run_steps step from a line of `run`'s input, in the commands' own words: `type e12 "Lisbon"`.
fn step(line: &str) -> Result<Value, String> {
    let words = split(line)?;
    let Some((first, rest)) = words.split_first() else {
        return Err("A step is empty.".into());
    };
    // wait is a step of its own, not run_steps as its command is.
    let known = WORDS
        .iter()
        .find(|word| word.word == first && word.word != "wait");
    let action = known.map_or(first.as_str(), |word| word.tool);
    let mut step = Map::new();
    step.insert("action".into(), json!(action));
    let names: &[&str] = match action {
        "click" => &["ref"],
        "type" => &["ref", "text"],
        "select" => &["ref", "option"],
        "press_key" => &["key"],
        "navigate" => &["url"],
        "wait" => &["text"],
        "scroll" => &[],
        "snapshot" => &[],
        _ => return Err(format!("{first} can't be a step.")),
    };
    let mut rest = rest.iter();
    let mut index = 0;
    while let Some(word) = rest.next() {
        if let Some(option) = word.strip_prefix("--") {
            match option {
                "submit" => _ = step.insert("submit".into(), json!(true)),
                "gone" => _ = step.insert("gone".into(), Value::Null),
                "for" => {
                    let seconds = rest.next().and_then(|seconds| seconds.parse::<f64>().ok());
                    step.insert(
                        "seconds".into(),
                        json!(seconds.ok_or("--for takes seconds.")?),
                    );
                }
                _ => return Err(format!("There's no option --{option} in a step.")),
            }
            continue;
        }
        if action == "scroll" {
            let key = if matches!(word.as_str(), "up" | "down") {
                "direction"
            } else {
                "ref"
            };
            step.insert(key.into(), json!(word));
            continue;
        }
        let Some(name) = names.get(index) else {
            return Err(format!("{first} takes {} words.", names.len()));
        };
        step.insert((*name).into(), json!(word));
        index += 1;
    }
    // `wait --gone "Loading"`: the text is what's to go.
    if step.get("gone") == Some(&Value::Null) {
        let text = step.remove("text").unwrap_or(json!(""));
        step.insert("gone".into(), text);
    }
    Ok(Value::Object(step))
}

/// A line's words, as a shell splits them: spaces between, and quotes around words with spaces.
fn split(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote = None;
    for character in line.chars() {
        match (quote, character) {
            (None, '"' | '\'') => {
                quote = Some(character);
                word.get_or_insert_default();
            }
            (Some(open), _) if open == character => quote = None,
            (None, ' ' | '\t') => words.extend(word.take()),
            _ => word.get_or_insert_default().push(character),
        }
    }
    if quote.is_some() {
        return Err(format!("A quote isn't closed: {line}"));
    }
    words.extend(word);
    Ok(words)
}

/// The words before the options, and the options as the tool's arguments: `-t N` its tab, and
/// `--name value`, or `--name` alone for a boolean, any property the tool takes.
fn options(
    words: &[String],
    schema: &Value,
    command: &str,
) -> Result<(Vec<String>, Map<String, Value>), String> {
    let mut positional = Vec::new();
    let mut arguments = Map::new();
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let name = match word.as_str() {
            "-t" | "--tab" => "tab".to_string(),
            option if option.starts_with("--") && option.len() > 2 => option[2..].replace('-', "_"),
            _ => {
                positional.push(word.clone());
                continue;
            }
        };
        // wait's own options, which become its step's.
        let kind = match name.as_str() {
            "gone" => Some("boolean"),
            "for" => Some("number"),
            _ => property(schema, &name).map(|property| kind(property)),
        };
        let Some(kind) = kind else {
            return Err(format!(
                "There's no option {word} here. Run {LINK} help {command} for its options."
            ));
        };
        let value = match kind {
            "boolean" => json!(true),
            _ => {
                let value = words
                    .next()
                    .ok_or_else(|| format!("{word} takes a value."))?;
                typed_as(kind, &name, value)?
            }
        };
        arguments.insert(name, value);
    }
    Ok((positional, arguments))
}

/// The tool's schema in the server's list.
fn schema<'a>(tools: &'a [Value], tool: &str) -> Option<&'a Value> {
    tools
        .iter()
        .find(|each| each.get("name").and_then(Value::as_str) == Some(tool))
        .and_then(|tool| tool.get("inputSchema"))
}

/// A property the tool takes, or one of each item of a list it takes, as each request's body
/// is, for the command that makes one item.
fn property<'a>(schema: &'a Value, name: &str) -> Option<&'a Value> {
    let properties = schema.get("properties")?.as_object()?;
    properties.get(name).or_else(|| {
        (properties.values()).find_map(|list| list.pointer("/items/properties")?.get(name))
    })
}

fn required(schema: &Value) -> Vec<&str> {
    (schema.get("required").and_then(Value::as_array))
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn kind(property: &Value) -> &str {
    property
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("any")
}

fn typed(schema: &Value, name: &str, value: &str) -> Result<Value, String> {
    let kind = property(schema, name).map_or("string", kind);
    typed_as(kind, name, value)
}

/// `value` as the tool's schema types it: a number as a number, JSON for anything that isn't text.
fn typed_as(kind: &str, name: &str, value: &str) -> Result<Value, String> {
    let wrong = || format!("{name} takes {kind}, not {value}.");
    Ok(match kind {
        "integer" => json!(value.parse::<i64>().map_err(|_| wrong())?),
        "number" => json!(value.parse::<f64>().map_err(|_| wrong())?),
        "boolean" => json!(value.parse::<bool>().map_err(|_| wrong())?),
        "string" => json!(value),
        _ => serde_json::from_str(value).unwrap_or_else(|_| json!(value)),
    })
}

/// How a word is written, for help and a usage error.
fn synopsis(word: &Word) -> String {
    let args = word.args.iter().map(|arg| format!(" <{arg}>"));
    let extra = match word.word {
        "wait" => " [--gone] [--for <seconds>]",
        "run" => ", one step a line on stdin",
        "shot" => " [-o <file.png>]",
        "request" => " [--body <json>] [--what <sentence>], or JSON on stdin",
        "split" => " ...",
        _ => "",
    };
    format!("{LINK} {}{}{extra}", word.word, args.collect::<String>())
}

fn usage() -> String {
    format!(
        "Usage: {LINK} <command> [-t <tab>] [--json] [--as <name>]. Run {LINK} help for the commands."
    )
}

/// What `ilo help` prints: the server's instructions, then each command the server has a tool
/// for, with its tool's first sentence; or with a command, its tool's whole description and
/// options.
pub fn help(about: Option<&str>, instructions: &str, tools: &[Value]) -> Result<String, String> {
    let description = |tool: &Value| {
        tool.get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let find = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
    };
    let Some(about) = about else {
        let mut text = format!(
            "{instructions}\n\nCommands (each accepts -t <tab> to target a tab, --json for JSON):\n"
        );
        for word in WORDS {
            let Some(tool) = find(word.tool) else {
                continue;
            };
            let first = description(tool);
            let first = first
                .split_inclusive(". ")
                .next()
                .unwrap_or_default()
                .trim();
            text += &format!("  {}\n      {first}\n", synopsis(word));
        }
        text += &format!("Run {LINK} help <command> for one command's options.\n");
        return Ok(text);
    };
    let word = WORDS.iter().find(|word| word.word == about);
    let name = word.map_or(about, |word| word.tool);
    let tool = find(name).ok_or_else(|| {
        format!("There's no command “{about}”. Run {LINK} help for the commands.")
    })?;
    let mut text = match word {
        Some(word) => format!("{}\n{}\n", synopsis(word), description(tool)),
        None => format!("{LINK} {about}\n{}\n", description(tool)),
    };
    let properties = (tool
        .pointer("/inputSchema/properties")
        .and_then(Value::as_object))
    .cloned()
    .unwrap_or_default();
    for (name, property) in properties {
        let option = match name.as_str() {
            "tab" => "-t <tab>".to_string(),
            _ => format!("--{}", name.replace('_', "-")),
        };
        let about = property
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        text += &format!("  {option}  {}  {about}\n", kind(&property));
    }
    Ok(text)
}

/// What a run prints, and how it ends.
#[derive(Debug, PartialEq)]
pub struct Printed {
    pub out: String,
    pub err: String,
    pub code: i32,
}

/// A picture of a page, which may be a signed-in one, is the user's alone to read.
#[cfg(unix)]
fn private(options: &mut std::fs::OpenOptions) -> &mut std::fs::OpenOptions {
    std::os::unix::fs::OpenOptionsExt::mode(options, 0o600)
}

#[cfg(not(unix))]
fn private(options: &mut std::fs::OpenOptions) -> &mut std::fs::OpenOptions {
    options
}

/// Writes `bytes` to a file at `path`, which the caller chose with -o, only the user's to read
/// when the run makes it.
fn save_at(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    private(options.write(true).create(true).truncate(true))
        .open(path)?
        .write_all(bytes)
}

/// Writes `bytes` to a new file of its own in `dir`, a shared one such as /tmp: under a name no
/// one can guess, made by this run, so nothing already there, a file or a link, is written
/// through, and only the user's to read.
fn save_new(dir: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let mut tries = 0;
    loop {
        let mut random = [0u8; 12];
        getrandom::fill(&mut random).map_err(std::io::Error::other)?;
        let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = dir.join(format!("{LINK}-shot-{name}.png"));
        match write_new(&path, bytes) {
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && tries < 8 => {
                tries += 1;
            }
            written => return written.map(|()| path),
        }
    }
}

/// Writes `bytes` to a file at `path` that this call makes, never one that's there.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    private(options.write(true).create_new(true))
        .open(path)?
        .write_all(bytes)
}

/// The tool's `result` as the run prints it: its text on stdout, or on stderr if it's an error;
/// a picture in `output`, or a file of its own, whose path it prints.
/// Where a screenshot without `-o` goes: the system's temporary folder, or in a Flatpak, whose
/// /tmp the host can't see, the user's Downloads folder, which the sandbox shares with the host at
/// the same path, so the path printed is right there.
fn shots() -> PathBuf {
    if super::bridge::flatpak_id().is_none() {
        return std::env::temp_dir();
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let config = (std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .unwrap_or_else(|| home.join(".config"));
    let user_dirs = std::fs::read_to_string(config.join("user-dirs.dirs")).ok();
    downloads(&home, user_dirs.as_deref())
}

/// The Downloads folder a user-dirs.dirs file's `user_dirs` names, `$HOME` being `home`, or
/// `~/Downloads` without one.
pub fn downloads(home: &Path, user_dirs: Option<&str>) -> PathBuf {
    let named = (user_dirs.unwrap_or_default().lines())
        .filter_map(|line| line.trim().strip_prefix("XDG_DOWNLOAD_DIR="))
        .filter_map(|value| value.strip_prefix('"')?.strip_suffix('"'))
        .find_map(|value| match value.strip_prefix("$HOME") {
            Some(rest) => Some(home.join(rest.trim_start_matches('/'))),
            None => value.starts_with('/').then(|| PathBuf::from(value)),
        });
    // The spec's "$HOME/" names the home itself, which is no Downloads folder.
    named
        .filter(|folder| folder != home)
        .unwrap_or_else(|| home.join("Downloads"))
}

pub fn printed(result: &Value, json: bool, output: Option<&Path>) -> Printed {
    let error = result.get("isError").and_then(Value::as_bool) == Some(true);
    let content = (result.get("content").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();
    let mut text: Vec<String> = Vec::new();
    let mut image = None;
    for item in &content {
        match item.get("type").and_then(Value::as_str) {
            Some("text") => {
                text.extend(item.get("text").and_then(Value::as_str).map(String::from));
            }
            Some("image") => {
                let data = item.get("data").and_then(Value::as_str).unwrap_or_default();
                let written = (base64::engine::general_purpose::STANDARD.decode(data))
                    .map_err(|error| error.to_string())
                    .and_then(|bytes| {
                        let saved = match output {
                            Some(path) => save_at(path, &bytes).map(|()| path.to_path_buf()),
                            None => save_new(&shots(), &bytes),
                        };
                        saved.map_err(|error| error.to_string())
                    });
                match written {
                    Ok(path) => image = Some(path),
                    Err(why) => text.push(format!("The screenshot couldn't be saved: {why}")),
                }
            }
            _ => {}
        }
    }
    let mut text = text.join("\n");
    if !json && let Some(image) = &image {
        if !text.is_empty() {
            text += "\n";
        }
        text += &format!("{}", image.display());
    }
    let code = if error { exit::REFUSED } else { 0 };
    if json {
        let image = image.map(|image| image.display().to_string());
        let out = json!({ "text": text, "error": error, "image": image }).to_string();
        return Printed {
            out: out + "\n",
            err: String::new(),
            code,
        };
    }
    let text = text + "\n";
    match error {
        true => Printed {
            out: String::new(),
            err: text,
            code,
        },
        false => Printed {
            out: text,
            err: String::new(),
            code,
        },
    }
}

/// What the agent file says, for a run to connect with.
pub struct Agent {
    pub address: PathBuf,
    pub token: String,
}

/// The agent the agent file in the profile directory `profile` names, which Add writes: none
/// before, as in the Store's copy, whose `ilo` is on PATH from install.
pub fn added(profile: &Path) -> Option<Agent> {
    let agent = super::cli::Agent::read(&super::cli::agent_file(profile))?;
    Some(Agent {
        address: agent.address,
        token: agent.token,
    })
}

/// What the program prints and exits with when run under `name`, the name it was run as, if that
/// isn't the command's: a link or script from before the command took the product's name, which
/// Install Again takes away. The command's own link, and the program run by its own name, as the
/// Flatpak's script and the Mac's bundle run it, go on to `run`.
pub fn renamed(name: &std::ffi::OsStr) -> Option<Printed> {
    let stem = Path::new(name).file_stem()?;
    let own = [
        Some(LINK.as_ref()),
        Path::new(super::cli::PROGRAM).file_stem(),
    ];
    if own.contains(&Some(stem)) {
        return None;
    }
    Some(Printed {
        out: String::new(),
        err: format!(
            "This command is now {LINK}. Install it again in {PRODUCT} › Settings › Other apps.\n"
        ),
        code: exit::REFUSED,
    })
}

/// Runs the command line `args` as the agent the agent file names, with `input` for stdin.
pub fn run(
    args: Vec<String>,
    named: Option<String>,
    agent: Option<Agent>,
    input: impl FnOnce() -> String,
) -> Printed {
    let fail = |code: i32, why: String| Printed {
        out: String::new(),
        err: why + "\n",
        code,
    };
    let line = match line(args) {
        Ok(line) => line,
        Err(why) => return fail(exit::USAGE, why),
    };
    let Some(agent) = agent else {
        return fail(
            exit::UNKNOWN,
            format!(
                "The {LINK} command isn't installed. Install it in {PRODUCT} › Settings › Other apps."
            ),
        );
    };
    let name = (line.name.clone())
        .or(named)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| UNNAMED.into());
    let mut session = match Session::open(&agent, &name) {
        Ok(session) => session,
        Err((code, why)) => return fail(code, why),
    };
    let tools = match session.request("tools/list", json!({})) {
        Ok(listed) => (listed.get("tools").and_then(Value::as_array))
            .cloned()
            .unwrap_or_default(),
        Err(why) => return fail(exit::NOT_RUNNING, why),
    };
    if line.words.first().map(String::as_str) == Some("help") {
        let about = line.words.get(1).map(String::as_str);
        return match help(about, &session.instructions, &tools) {
            Ok(text) => Printed {
                out: text,
                err: String::new(),
                code: 0,
            },
            Err(why) => fail(exit::USAGE, why),
        };
    }
    let (tool, arguments) = match call(&line.words, &tools, input) {
        Ok(call) => call,
        Err(why) => return fail(exit::USAGE, why),
    };
    let params = json!({ "name": tool, "arguments": arguments });
    match session.request("tools/call", params) {
        Ok(result) => printed(&result, line.json, line.output.as_deref()),
        Err(why) => fail(exit::REFUSED, why),
    }
}

/// A connection to the server, as the command's agent, initialized.
struct Session {
    writer: super::server::Stream,
    reader: BufReader<super::server::Stream>,
    next: u64,
    instructions: String,
}

impl Session {
    fn open(agent: &Agent, name: &str) -> Result<Session, (i32, String)> {
        let not_running = || (exit::NOT_RUNNING, super::bridge::off());
        let stream = super::server::connect(&agent.address).map_err(|_| not_running())?;
        let mut writer = stream.try_clone().map_err(|_| not_running())?;
        writeln!(writer, "{}", json!({ "token": agent.token })).map_err(|_| not_running())?;
        let mut reader = BufReader::new(stream);
        let answer = super::server::read_line(&mut reader).ok_or_else(not_running)?;
        let answer: Value = serde_json::from_str(&answer).unwrap_or_default();
        // The server refuses with agents' off answer while other apps' agents are off, which a
        // script reads as it does agents off.
        match answer.get("error").and_then(Value::as_str) {
            Some(refused) if refused == super::bridge::off() => return Err(not_running()),
            Some(refused) => return Err((exit::UNKNOWN, refused.into())),
            None => {}
        }
        let mut session = Session {
            writer,
            reader,
            next: 1,
            instructions: String::new(),
        };
        let initialized = session
            .request(
                "initialize",
                json!({
                    "protocolVersion": super::server::VERSIONS[0],
                    "capabilities": {},
                    "clientInfo": { "name": name, "version": env!("CARGO_PKG_VERSION") },
                }),
            )
            .map_err(|why| (exit::NOT_RUNNING, why))?;
        session.instructions = (initialized.get("instructions").and_then(Value::as_str))
            .unwrap_or_default()
            .into();
        let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        writeln!(session.writer, "{initialized}").map_err(|_| not_running())?;
        Ok(session)
    }

    /// Sends a request, and waits for its answer's result.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let gone = || "The browser closed the connection.".to_string();
        let id = self.next;
        self.next += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.writer, "{request}").map_err(|_| gone())?;
        loop {
            let line = super::server::read_line(&mut self.reader).ok_or_else(gone)?;
            let Ok(answer) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if answer.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = answer.get("error") {
                let message = error.get("message").and_then(Value::as_str);
                return Err(message.unwrap_or("The browser refused the command.").into());
            }
            return Ok(answer.get("result").cloned().unwrap_or_default());
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        _ = super::server::close_write(&mut self.writer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Value> {
        super::super::tools::list("ilo-command")
            .as_array()
            .cloned()
            .unwrap()
    }

    fn called(line: &str) -> Result<(String, Value), String> {
        called_with(line, "")
    }

    fn called_with(line: &str, input: &str) -> Result<(String, Value), String> {
        let words = split(line).unwrap();
        call(&words, &tools(), || input.to_string())
    }

    /// Each word calls its tool, the words after it its arguments in order, the last taking what's
    /// left, and -t the tab, typed as the server's list types them.
    #[test]
    fn words_call_the_servers_tools() {
        assert_eq!(called("tabs").unwrap(), ("list_tabs".into(), json!({})));
        assert_eq!(
            called("open https://example.com/").unwrap(),
            ("open_tab".into(), json!({ "url": "https://example.com/" }))
        );
        assert_eq!(
            called("click e2 -t 3").unwrap(),
            ("click".into(), json!({ "ref": "e2", "tab": 3 }))
        );
        assert_eq!(
            called("type e12 Lisbon museums --submit").unwrap(),
            (
                "type".into(),
                json!({ "ref": "e12", "text": "Lisbon museums", "submit": true })
            )
        );
        assert_eq!(
            called("scroll down").unwrap().1,
            json!({ "direction": "down" })
        );
        assert_eq!(called("scroll e4").unwrap().1, json!({ "ref": "e4" }));
        assert_eq!(
            called("close --discard -t 9").unwrap(),
            ("close_tab".into(), json!({ "discard": true, "tab": 9 }))
        );
        assert_eq!(
            called("split 3 4").unwrap(),
            ("split_tabs".into(), json!({ "tabs": [3, 4] }))
        );
        assert_eq!(
            called("hand-off 'Sign in to your bank' -t 5").unwrap(),
            (
                "hand_off".into(),
                json!({ "reason": "Sign in to your bank", "tab": 5 })
            )
        );
        // A tool with no word of its own is called by its name.
        assert_eq!(called("list_split").unwrap().0, "list_split");
    }

    /// wait and run make run_steps' steps, run's from stdin in the commands' own words.
    #[test]
    fn wait_and_run_make_steps() {
        assert_eq!(
            called("wait Results --for 10").unwrap(),
            (
                "run_steps".into(),
                json!({ "steps": [{ "action": "wait", "text": "Results", "seconds": 10.0 }] })
            )
        );
        assert_eq!(
            called("wait Loading --gone").unwrap().1,
            json!({ "steps": [{ "action": "wait", "gone": "Loading" }] })
        );
        let steps = "type e12 \"Lisbon\"\nclick e14\n\nwait \"Results\"\nkey Enter\n";
        assert_eq!(
            called_with("run -t 4", steps).unwrap(),
            (
                "run_steps".into(),
                json!({ "tab": 4, "steps": [
                    { "action": "type", "ref": "e12", "text": "Lisbon" },
                    { "action": "click", "ref": "e14" },
                    { "action": "wait", "text": "Results" },
                    { "action": "press_key", "key": "Enter" },
                ] })
            )
        );
        assert!(called_with("run", "shot\n").is_err());
        assert!(called_with("run", "type \"open\n").is_err());
    }

    /// request takes a method and a path with a JSON body, or the requests as JSON on stdin.
    #[test]
    fn request_sends_one_or_what_stdin_gives() {
        assert_eq!(
            called(r#"request post /api/items --body '{"title":"Milk"}' -t 4"#).unwrap(),
            (
                "request".into(),
                json!({ "tab": 4, "requests": [
                    { "method": "POST", "path": "/api/items", "body": { "title": "Milk" } }
                ] })
            )
        );
        let input = r#"{"method":"GET","path":"/api/items"}"#;
        assert_eq!(
            called_with("request", input).unwrap().1,
            json!({ "requests": [{ "method": "GET", "path": "/api/items" }] })
        );
    }

    /// A word the server has no tool for, a missing argument, an option the tool doesn't take
    /// and a tab that isn't a number are usage errors, sent nowhere.
    #[test]
    fn a_wrong_line_is_a_usage_error() {
        assert!(
            called("fly")
                .unwrap_err()
                .contains("There's no command “fly”")
        );
        assert!(called("click").unwrap_err().contains("click needs ref"));
        assert!(called("click e2 --fast").is_err());
        assert!(called("click e2 -t three").is_err());
        assert!(called("tabs extra").is_err());
        assert!(call(&[], &tools(), String::new).is_err());
    }

    /// The command's own options come out wherever they are.
    #[test]
    fn its_own_options_come_out() {
        let read = line(
            [
                "shot",
                "--json",
                "-t",
                "3",
                "-o",
                "page.png",
                "--as",
                "Researcher",
            ]
            .map(String::from),
        )
        .unwrap();
        assert_eq!(read.words, ["shot", "-t", "3"]);
        assert!(read.json);
        assert_eq!(read.output, Some(PathBuf::from("page.png")));
        assert_eq!(read.name.as_deref(), Some("Researcher"));
    }

    /// Help lists each command the server has a tool for, from its description, and one command's
    /// options from its schema, so it can't drift from the tools.
    #[test]
    fn help_reads_the_tools() {
        let text = help(None, "The instructions.", &tools()).unwrap();
        assert!(text.starts_with("The instructions."));
        assert!(text.contains(&format!("{LINK} click <ref>")));
        assert!(text.contains(&format!("{LINK} hand-off <reason>")));
        // Not on this server: no done tool, so no done command.
        let has_done = tools().iter().any(|tool| tool["name"] == "done");
        assert_eq!(text.contains(&format!("{LINK} done")), has_done);
        let click = help(Some("click"), "", &tools()).unwrap();
        assert!(click.contains("-t <tab>"));
        assert!(click.contains("--outline"));
        assert!(help(Some("fly"), "", &tools()).is_err());
    }

    /// An answer's text goes to stdout, an error's to stderr with exit 1, a picture to a file whose
    /// path it prints, and with --json all of it as one object.
    #[test]
    fn answers_print_as_scripts_read_them() {
        let text = json!({ "content": [{ "type": "text", "text": "tab 3 · example.com" }] });
        assert_eq!(
            printed(&text, false, None),
            Printed {
                out: "tab 3 · example.com\n".into(),
                err: String::new(),
                code: 0
            }
        );
        let refused = json!({ "isError": true, "content": [{ "type": "text", "text": "No." }] });
        assert_eq!(
            printed(&refused, false, None),
            Printed {
                out: String::new(),
                err: "No.\n".into(),
                code: exit::REFUSED
            }
        );
        let file =
            std::env::temp_dir().join(format!("ilo-command-test-{}.png", std::process::id()));
        let picture = json!({ "content": [
            { "type": "text", "text": "Took it." },
            { "type": "image", "data": "aWxv", "mimeType": "image/png" },
        ] });
        let shown = printed(&picture, true, Some(&file));
        let shown: Value = serde_json::from_str(&shown.out).unwrap();
        assert_eq!(shown["text"], "Took it.");
        assert_eq!(shown["error"], false);
        assert_eq!(shown["image"], file.display().to_string());
        assert_eq!(std::fs::read(&file).unwrap(), b"ilo");
        _ = std::fs::remove_file(&file);
    }

    /// In a Flatpak a screenshot without -o goes in the Downloads folder the user's user-dirs.dirs
    /// names, which the host shares at the same path, or ~/Downloads without one.
    #[test]
    fn a_flatpaks_screenshot_goes_in_downloads() {
        let home = Path::new("/home/someone");
        let named = |text: &str| downloads(home, Some(text));
        assert_eq!(downloads(home, None), home.join("Downloads"));
        assert_eq!(
            named(
                "# written by xdg-user-dirs-update\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/Téléchargements\"\n"
            ),
            home.join("Téléchargements")
        );
        assert_eq!(
            named("XDG_DOWNLOAD_DIR=\"/data/downloads\""),
            Path::new("/data/downloads")
        );
        assert_eq!(named("XDG_DOWNLOAD_DIR=\"$HOME/\""), home.join("Downloads"));
        assert_eq!(named("XDG_DOWNLOAD_DIR=relative"), home.join("Downloads"));
    }

    /// A screenshot without -o is a new file only the user can read, under a name of its own: a
    /// file or a link already at a path it would take isn't written through.
    #[cfg(unix)]
    #[test]
    fn a_screenshot_is_a_new_file_only_the_user_reads() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ilo-shot-test-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;

        let first = save_new(&dir, b"ilo").unwrap();
        let second = save_new(&dir, b"ilo").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(dir.as_path()));
        assert_eq!(std::fs::read(&first).unwrap(), b"ilo");
        assert_eq!(mode(&first), 0o600);

        let there = dir.join("there.png");
        std::fs::write(&there, b"theirs").unwrap();
        assert!(write_new(&there, b"ilo").is_err());
        assert_eq!(std::fs::read(&there).unwrap(), b"theirs");
        let target = dir.join("target");
        std::fs::write(&target, b"theirs").unwrap();
        let link = dir.join("link.png");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(write_new(&link, b"ilo").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"theirs");

        let chosen = dir.join("chosen.png");
        save_at(&chosen, b"ilo").unwrap();
        assert_eq!(mode(&chosen), 0o600);
        _ = std::fs::remove_dir_all(&dir);
    }

    /// Run under the name it had before it took the product's, as an old link or a script names it,
    /// the command says what it's called now and refuses, before it reads or calls anything. Under
    /// its own name, or its program's, it runs.
    #[test]
    fn under_its_old_name_the_command_says_its_new_one() {
        let said = format!(
            "This command is now {LINK}. Install it again in {PRODUCT} › Settings › Other apps.\n"
        );
        let old = super::super::cli::OLD_LINK;
        for name in [
            old.to_string(),
            format!("/home/someone/.local/bin/{old}"),
            format!("{old}.exe"),
        ] {
            let printed = renamed(name.as_ref()).expect(&name);
            assert_eq!(
                (printed.out.as_str(), printed.err.as_str()),
                ("", said.as_str())
            );
            assert_eq!(printed.code, exit::REFUSED);
        }
        let program = super::super::cli::PROGRAM;
        for name in [
            LINK.to_string(),
            format!("/usr/bin/{LINK}"),
            format!("{LINK}.exe"),
            program.into(),
        ] {
            assert!(renamed(name.as_ref()).is_none(), "{name} refused");
        }
    }

    /// Without an agent file there's nothing to connect as, and the run says where to install it.
    #[test]
    fn a_run_without_the_agent_file_says_where_to_install_it() {
        let printed = run(vec!["tabs".into()], None, None, String::new);
        assert_eq!(printed.code, exit::UNKNOWN);
        let said = format!(
            "The {LINK} command isn't installed. Install it in {PRODUCT} › Settings › Other apps."
        );
        assert!(printed.err.contains(&said), "{}", printed.err);
    }

    /// The command before Add, as the Store's alias is from install: the profile's server hears
    /// nothing of it, and it ends not added.
    #[cfg(unix)]
    #[test]
    fn before_add_nothing_reaches_the_server() {
        let profile = std::env::temp_dir().join(format!("ilo-inert-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&profile);
        std::fs::create_dir_all(&profile).unwrap();
        let listener =
            std::os::unix::net::UnixListener::bind(super::super::server::address(&profile))
                .unwrap();
        listener.set_nonblocking(true).unwrap();
        let printed = run(vec!["tabs".into()], None, added(&profile), String::new);
        assert_eq!(printed.code, exit::UNKNOWN);
        let heard = listener.accept().map(|_| ()).map_err(|error| error.kind());
        assert_eq!(heard, Err(std::io::ErrorKind::WouldBlock));
        _ = std::fs::remove_dir_all(&profile);
    }
}
