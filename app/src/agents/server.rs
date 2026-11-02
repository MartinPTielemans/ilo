//! The MCP server: JSON-RPC a message a line, as MCP's stdio transport has it, on a Unix socket in
//! the profile directory that only the user can open (decision 8), or on Windows a named pipe only
//! they can open (pipe.rs). An MCP client launches the bridge (`bridge.rs`), which hands over the
//! agent's token as its first line, then carries the client's stdio to the socket and back. A thread waits in `accept`, and one for each connection
//! waits in `read`, so an idle server wakes nothing. The threads answer what needs no browser
//! themselves, and send the rest to the UI thread as events.
//!
//! There's no TCP port, so no web page and no other user's process can reach it. The token says
//! which agent a connection is, and once the user disconnects the agent its connections close.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};

use super::{Call, Event, ToolResult, tools};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(windows)]
use super::pipe;

/// One end of a connection to the server.
#[cfg(unix)]
pub(super) type Stream = UnixStream;
#[cfg(windows)]
pub(super) type Stream = pipe::Pipe;

/// The line the bridge sends where it can't close its end for writing alone, as a named pipe
/// can't, once its client has closed its end: the server ends the session as it would at the end
/// of the stream, then closes the connection once it has answered what it read.
pub(super) const END: &str = r#"{"end":true}"#;

/// Agents' tokens, each to the id of the agent it's for, which the UI thread changes as the user
/// connects and disconnects agents.
pub type Tokens = Arc<Mutex<HashMap<String, String>>>;

/// The protocol versions the server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// A message may be no longer, which none an agent sends comes near.
const MAX_LINE: usize = 4 * 1024 * 1024;
/// Connections at once: a client keeps one.
const MAX_CONNECTIONS: usize = 32;

/// The server, running until it's dropped.
pub struct Server {
    shared: Arc<Shared>,
    /// The thread waiting for connections, which a stopping server waits for, so its pipe is gone
    /// once it has stopped, as the socket file is.
    #[cfg(windows)]
    accepting: Option<thread::JoinHandle<()>>,
}

struct Shared {
    path: PathBuf,
    stopped: AtomicBool,
    tokens: Tokens,
    /// The connections open, by a number of their own.
    connections: Mutex<HashMap<u64, Connection>>,
    next_connection: AtomicU64,
    events: UnboundedSender<Event>,
}

struct Connection {
    stream: Stream,
    /// Its agent's id, once its token has said, and whether it began a session with initialize.
    agent: String,
    initialized: bool,
}

impl Server {
    /// Listens on a socket at `path`, readable and writable by the user alone, in place of any
    /// socket a browser that didn't stop left there.
    #[cfg(unix)]
    pub fn start(
        path: &Path,
        tokens: Tokens,
        events: UnboundedSender<Event>,
    ) -> io::Result<Server> {
        use std::os::unix::fs::PermissionsExt;
        // Made under another name and moved into place once it's the user's alone, so no one
        // else can ever open it at `path`.
        let made = PathBuf::from(format!("{}.new", path.display()));
        _ = std::fs::remove_file(&made);
        let listener = UnixListener::bind(&made)?;
        let placed = std::fs::set_permissions(&made, std::fs::Permissions::from_mode(0o600))
            .and_then(|()| std::fs::rename(&made, path));
        if let Err(error) = placed {
            _ = std::fs::remove_file(&made);
            return Err(error);
        }
        let shared = Arc::new(Shared {
            path: path.into(),
            stopped: AtomicBool::new(false),
            tokens,
            connections: Mutex::default(),
            next_connection: AtomicU64::new(0),
            events,
        });
        let accepting = shared.clone();
        thread::Builder::new()
            .name("agents-server".into())
            .spawn(move || accept(listener, accepting))?;
        Ok(Server { shared })
    }

    /// Makes the named pipe at `path`, which only the user can open, unless it's there already.
    #[cfg(windows)]
    pub fn start(
        path: &Path,
        tokens: Tokens,
        events: UnboundedSender<Event>,
    ) -> io::Result<Server> {
        let listener = pipe::Listener::bind(path)?;
        let shared = Arc::new(Shared {
            path: path.into(),
            stopped: AtomicBool::new(false),
            tokens,
            connections: Mutex::default(),
            next_connection: AtomicU64::new(0),
            events,
        });
        let accepting = shared.clone();
        let accepting = thread::Builder::new()
            .name("agents-server".into())
            .spawn(move || accept(listener, accepting))?;
        Ok(Server { shared, accepting: Some(accepting) })
    }

    pub fn path(&self) -> &Path {
        &self.shared.path
    }

    /// Closes the agent's connections, as the user disconnected it.
    pub fn disconnect(&self, agent: &str) {
        let connections = self.shared.connections.lock().unwrap();
        for connection in connections.values().filter(|each| each.agent == agent) {
            connection.close();
        }
    }
}

impl Connection {
    fn close(&self) {
        #[cfg(unix)]
        {
            _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
        #[cfg(windows)]
        self.stream.close();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        // Closed here, on the UI thread, which its task's wake then is, not on the last server
        // thread to let go of it.
        self.shared.events.close_channel();
        // `accept` returns for a connection, which then finds the server stopped. Held until the
        // accept thread is gone, since a pipe client that left before ConnectNamedPipe is lost.
        let _wake = connect(&self.shared.path);
        #[cfg(windows)]
        if let Some(accepting) = self.accepting.take() {
            pipe::wait_for(accepting);
        }
        for (_, connection) in self.shared.connections.lock().unwrap().drain() {
            connection.close();
        }
        #[cfg(unix)]
        {
            _ = std::fs::remove_file(&self.shared.path);
        }
    }
}

/// Where the server for the profile in `folder` listens: its socket, or on Windows its pipe.
#[cfg(unix)]
pub fn address(folder: &Path) -> PathBuf {
    folder.join("Agents.sock")
}
#[cfg(windows)]
pub use pipe::address;

/// Connects to the server at `path`, as the bridge does.
#[cfg(unix)]
pub(super) fn connect(path: &Path) -> io::Result<Stream> {
    UnixStream::connect(path)
}
#[cfg(windows)]
pub(super) fn connect(path: &Path) -> io::Result<Stream> {
    pipe::Pipe::connect(path)
}

/// Tells the server the client closed its end, which it ends the session at, then closes the
/// connection once it has answered.
#[cfg(unix)]
pub(super) fn close_write(stream: &mut Stream) -> io::Result<()> {
    stream.shutdown(std::net::Shutdown::Write)
}
#[cfg(windows)]
pub(super) fn close_write(stream: &mut Stream) -> io::Result<()> {
    // After a line of its own, in case the client's last didn't end.
    write!(stream, "\n{END}\n")
}

#[cfg(unix)]
fn accept(listener: UnixListener, shared: Arc<Shared>) {
    for stream in listener.incoming() {
        if shared.stopped.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(stream) = stream {
            welcome(stream, &shared);
        }
    }
}

#[cfg(windows)]
fn accept(mut listener: pipe::Listener, shared: Arc<Shared>) {
    while let Ok(stream) = listener.accept() {
        if shared.stopped.load(Ordering::SeqCst) {
            return;
        }
        welcome(stream, &shared);
    }
}

/// Keeps a new connection and serves it on a thread of its own, unless there are too many.
fn welcome(stream: Stream, shared: &Arc<Shared>) {
    let number = shared.next_connection.fetch_add(1, Ordering::SeqCst);
    {
        let mut connections = shared.connections.lock().unwrap();
        if connections.len() >= MAX_CONNECTIONS {
            return;
        }
        let Ok(kept) = stream.try_clone() else {
            return;
        };
        let connection = Connection {
            stream: kept,
            agent: String::new(),
            initialized: false,
        };
        connections.insert(number, connection);
    }
    let serving = shared.clone();
    let spawned = thread::Builder::new()
        .name("agents-connection".into())
        .spawn(move || {
            serve(stream, number, &serving);
            end(&serving, number);
        });
    if spawned.is_err() {
        shared.connections.lock().unwrap().remove(&number);
    }
}

/// Takes the connection's token, then answers its messages in turn until it closes.
fn serve(stream: Stream, number: u64, shared: &Shared) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let Some(first) = read_line(&mut reader) else {
        return;
    };
    let token = serde_json::from_str::<Value>(&first)
        .ok()
        .and_then(|first| Some(first.get("token")?.as_str()?.to_string()))
        .unwrap_or_default();
    let Some(agent) = shared.agent(&token) else {
        let refused =
            "The token isn't one the browser gave. Connect the client again in Settings › Agents.";
        _ = writeln!(writer, "{}", json!({ "error": refused }));
        return;
    };
    if let Some(connection) = shared.connections.lock().unwrap().get_mut(&number) {
        connection.agent = agent.clone();
    }
    if writeln!(writer, "{}", json!({ "ok": true })).is_err() {
        return;
    }
    // Its messages are answered on a thread of their own, so this one reads on while a call waits
    // on the user, and a client that closes its end then ends its session, which drops what waits
    // for it (`Event::Bye`). The answering thread's end closes the connection.
    thread::scope(|scope| {
        let (requests, next) = mpsc::channel::<String>();
        let answering = thread::Builder::new()
            .name("agents-answers".into())
            .spawn_scoped(scope, {
                let agent = agent.clone();
                move || {
                    for line in next {
                        let Some(answer) = message(shared, number, &agent, &line) else {
                            continue;
                        };
                        if writeln!(writer, "{answer}")
                            .and_then(|()| writer.flush())
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            });
        if answering.is_err() {
            return;
        }
        while let Some(line) = read_line(&mut reader) {
            // A token the user has since disconnected, or a browser that stopped, answers no more.
            if shared.stopped.load(Ordering::SeqCst)
                || shared.agent(&token).as_ref() != Some(&agent)
            {
                break;
            }
            if line == END {
                break;
            }
            if !line.trim().is_empty() && requests.send(line).is_err() {
                break;
            }
        }
        drop(requests);
        end(shared, number);
    });
}

impl Shared {
    fn agent(&self, token: &str) -> Option<String> {
        self.tokens.lock().unwrap().get(token).cloned()
    }
}

/// The next line, without its end, or None once the connection closes or sends a line too long.
pub fn read_line(reader: &mut impl BufRead) -> Option<String> {
    let mut line = Vec::new();
    let read = reader
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', &mut line)
        .ok()?;
    if read == 0 || (line.len() > MAX_LINE && line.last() != Some(&b'\n')) {
        return None;
    }
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    Some(String::from_utf8_lossy(&line).into_owned())
}

/// The answer to a message, if it needs one: a request does, and a notification or a response to
/// the server, which sends no requests, doesn't.
fn message(shared: &Shared, number: u64, agent: &str, line: &str) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(error(Value::Null, -32700, "The message isn't JSON."));
    };
    if message.is_array() {
        return Some(error(Value::Null, -32600, "Send one message at a time."));
    }
    let method = message.get("method").and_then(Value::as_str)?;
    let id = message.get("id").cloned()?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => initialize(shared, number, agent, &params),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::list(agent) }),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            call(shared, number, agent, name, arguments).to_json()
        }
        _ => return Some(error(id, -32601, "There's no such method.")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn initialize(shared: &Shared, number: u64, agent: &str, params: &Value) -> Value {
    let asked = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = VERSIONS
        .iter()
        .find(|each| **each == asked)
        .unwrap_or(&VERSIONS[0]);
    let client = params
        .pointer("/clientInfo/name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(connection) = shared.connections.lock().unwrap().get_mut(&number) {
        connection.initialized = true;
    }
    _ = shared.events.unbounded_send(Event::Hello {
        agent: agent.into(),
        client: client.into(),
    });
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": env!("APP_NAME"), "version": env!("CARGO_PKG_VERSION") },
        "instructions": tools::INSTRUCTIONS,
    })
}

/// Runs a tool on the UI thread, and waits for its answer.
fn call(shared: &Shared, number: u64, agent: &str, tool: &str, arguments: Value) -> ToolResult {
    let (reply, answer) = mpsc::channel();
    let call = Call {
        agent: agent.into(),
        tool: tool.into(),
        arguments,
        reply,
    };
    // Sent while the connection is open, under the lock `end` closes it with, so that the UI
    // hears no call of a session after its agent's bye.
    {
        let connections = shared.connections.lock().unwrap();
        if !connections.contains_key(&number) {
            return ToolResult::error("The connection closed.");
        }
        if shared.events.unbounded_send(Event::Call(call)).is_err() {
            return ToolResult::error("The browser has turned agents off.");
        }
    }
    // The UI drops the call unanswered when the user turns agents off or disconnects the agent.
    answer
        .recv()
        .unwrap_or_else(|_| ToolResult::error("The browser has turned agents off."))
}

/// The connection closed. The agent's last session ending tells the UI it's gone.
fn end(shared: &Shared, number: u64) {
    let mut connections = shared.connections.lock().unwrap();
    let Some(ended) = connections.remove(&number) else {
        return;
    };
    let others = connections
        .values()
        .any(|each| each.agent == ended.agent && each.initialized);
    if ended.initialized && !others {
        _ = shared
            .events
            .unbounded_send(Event::Bye { agent: ended.agent });
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests;
