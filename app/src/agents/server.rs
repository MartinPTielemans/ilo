//! The MCP server: JSON-RPC a message a line, as MCP's stdio transport has it, on a Unix socket in
//! the profile directory that only the user can open (decision 8), or on Windows a named pipe only
//! they can open (pipe.rs). An MCP client launches the bridge (`bridge.rs`), which hands over the
//! agent's token as its first line, then carries the client's stdio to the socket and back. A
//! thread waits in `accept`, and one for each connection waits in `read`, so an idle server wakes
//! nothing. The threads answer what needs no browser themselves, and send the rest to the UI
//! thread as events. A connection's calls on different tabs are answered at once and a tab's in
//! order, so a turn's step across ten tabs takes about as long as one (#1476).
//!
//! There's no TCP port, so no web page and no other user's process can reach it. The token says
//! which agent a connection is, and once the user disconnects the agent its connections close.
//! The site's demo, on the web, has neither, and never starts one.
#![cfg_attr(not(any(unix, windows)), allow(dead_code))]

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use web_time::Instant;

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
/// The site's demo, on the web, has neither: no connection is ever made there.
#[cfg(not(any(unix, windows)))]
pub(super) enum Stream {}

#[cfg(not(any(unix, windows)))]
impl Stream {
    pub(super) fn try_clone(&self) -> io::Result<Stream> {
        match *self {}
    }
}

#[cfg(not(any(unix, windows)))]
impl Read for Stream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        match *self {}
    }
}

#[cfg(not(any(unix, windows)))]
impl Write for Stream {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        match *self {}
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {}
    }
}

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
    /// Messages being answered, and whether the user stopped its agent: once the last of those has
    /// its answer written, the connection closes.
    answering: usize,
    stopping: bool,
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

    /// On the web there's nothing for a client to connect to, so the server never starts.
    #[cfg(not(any(unix, windows)))]
    pub fn start(_: &Path, _: Tokens, _: UnboundedSender<Event>) -> io::Result<Server> {
        Err(io::ErrorKind::Unsupported.into())
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
        Ok(Server {
            shared,
            accepting: Some(accepting),
        })
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

    /// The user stopped the agent: each of its connections closes once what it's answering has
    /// its answer, the UI's that the user stopped it, and calls from then on are refused. The
    /// client shows the browser's server as gone until the user reconnects it, a session of its
    /// own (#1080).
    pub fn stop(&self, agent: &str) {
        let mut connections = self.shared.connections.lock().unwrap();
        for (number, connection) in connections
            .iter_mut()
            .filter(|(_, each)| each.agent == agent)
        {
            connection.stopping = true;
            if connection.answering == 0 {
                connection.close_stopped(*number);
            }
        }
    }
}

impl Connection {
    fn close_stopped(&self, number: u64) {
        // check.sh reads this line.
        log!(
            "browser: agent {} connection {number} closed, as the user stopped it",
            self.agent
        );
        self.close();
    }

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
#[cfg(not(any(unix, windows)))]
pub fn address(folder: &Path) -> PathBuf {
    folder.join("Agents")
}

/// Connects to the server at `path`, as the bridge does.
#[cfg(unix)]
pub(super) fn connect(path: &Path) -> io::Result<Stream> {
    UnixStream::connect(path)
}
#[cfg(windows)]
pub(super) fn connect(path: &Path) -> io::Result<Stream> {
    pipe::Pipe::connect(path)
}
#[cfg(not(any(unix, windows)))]
pub(super) fn connect(_: &Path) -> io::Result<Stream> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Tells the server the client closed its end, which it ends the session at, then closes the
/// connection once it has answered.
#[cfg(unix)]
pub(super) fn close_write(stream: &mut Stream) -> io::Result<()> {
    stream.shutdown(std::net::Shutdown::Write)
}
#[cfg(not(any(unix, windows)))]
pub(super) fn close_write(stream: &mut Stream) -> io::Result<()> {
    match *stream {}
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
            answering: 0,
            stopping: false,
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
        _ = writeln!(writer, "{}", json!({ "error": refused })).and_then(|()| writer.flush());
        return;
    };
    if let Some(connection) = shared.connections.lock().unwrap().get_mut(&number) {
        connection.agent = agent.clone();
    }
    if writeln!(writer, "{}", json!({ "ok": true })).is_err() {
        return;
    }
    // Its messages are answered on threads of their own, so this one reads on while calls wait on
    // pages or the user, and a client that closes its end then ends its session, which drops what
    // waits for it (`Event::Bye`). The last answering thread's end closes the connection.
    let answering = Answering {
        shared,
        number,
        agent: agent.clone(),
        writer: Mutex::new(writer),
        queue: Mutex::default(),
        changed: Condvar::new(),
    };
    thread::scope(|scope| {
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
            if !line.trim().is_empty() && !answering.queue_up(line, scope) {
                break;
            }
        }
        answering.close();
        end(shared, number);
    });
}

/// Messages a connection answers at once, at most: a turn's calls across ten tabs, with room.
const MAX_AT_ONCE: usize = 16;

/// What a message waits for before it's answered. Calls on one tab keep their order, and those on
/// different tabs go at once, so a page that's slow to load holds only its own tab's calls. A
/// turn's tabs open at once. What asks the user asks one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Lane {
    /// Messages with no tab of their own, in the order they came: initialize, tools/list, a task's
    /// title and finish, and an action with no tab, which acts in the tab used last.
    InOrder,
    /// One tab's calls, in the order they came.
    Tab(u64),
    /// ask, hand_off and ask_for_tab: one prompt at a time, the others waiting their turn.
    Asks,
}

impl Lane {
    /// Whether two messages in these lanes wait for each other. One with no tab acts in the tab
    /// used last, which may be any, so it waits for every tab's calls before it, and they for it.
    fn meets(self, other: Lane) -> bool {
        self == other
            || matches!(
                (self, other),
                (Lane::InOrder, Lane::Tab(_)) | (Lane::Tab(_), Lane::InOrder)
            )
    }
}

/// A message read and not yet answered, with the lanes it holds while it's answered: none for
/// open_tab, which waits only for room.
struct Waiting {
    line: String,
    lanes: Vec<Lane>,
}

/// A connection's messages between reading and answering.
#[derive(Default)]
struct Queue {
    waiting: std::collections::VecDeque<Waiting>,
    /// The lanes of the messages being answered.
    busy: HashSet<Lane>,
    answering: usize,
    /// The most answered at once since the connection last had none, for the log.
    most: usize,
    /// The threads answering, and those waiting for a message they can take.
    threads: usize,
    idle: usize,
    closed: bool,
}

impl Queue {
    /// The first message whose lanes are free, its lanes taken. A message waiting holds its lanes
    /// for those behind it, so each lane keeps its order.
    fn take(&mut self) -> Option<Waiting> {
        let mut held: Vec<Lane> = self.busy.iter().copied().collect();
        let at = self.waiting.iter().position(|waiting| {
            let meets = |lane: &Lane| held.iter().any(|each| lane.meets(*each));
            let free = !waiting.lanes.iter().any(meets);
            held.extend(waiting.lanes.iter().copied());
            free
        })?;
        let taken = self.waiting.remove(at)?;
        self.busy.extend(taken.lanes.iter().copied());
        self.answering += 1;
        self.most = self.most.max(self.answering);
        Some(taken)
    }
}

/// A connection's answering: its messages, the threads that answer them, and its writer, which
/// each answer has whole.
struct Answering<'a> {
    shared: &'a Shared,
    number: u64,
    agent: String,
    writer: Mutex<Stream>,
    queue: Mutex<Queue>,
    changed: Condvar,
}

impl<'a> Answering<'a> {
    /// Queues a message, with a thread for it while there's room for one and the free ones have
    /// messages enough. Whether the connection can go on.
    fn queue_up<'scope, 'env>(
        &'scope self,
        line: String,
        scope: &'scope thread::Scope<'scope, 'env>,
    ) -> bool {
        let mut queue = self.queue.lock().unwrap();
        queue.waiting.push_back(Waiting {
            lanes: lanes(&line),
            line,
        });
        if queue.waiting.len() > queue.idle && queue.threads < MAX_AT_ONCE {
            let spawned = thread::Builder::new()
                .name("agents-answers".into())
                .spawn_scoped(scope, || self.answer());
            match spawned {
                Ok(_) => queue.threads += 1,
                Err(_) if queue.threads == 0 => return false,
                Err(_) => {}
            }
        }
        self.changed.notify_all();
        true
    }

    /// Answers what it can take until the connection has closed and nothing waits.
    fn answer(&self) {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if let Some(taken) = queue.take() {
                drop(queue);
                self.shared.answering(self.number, true);
                if let Some(answer) = message(self.shared, self.number, &self.agent, &taken.line) {
                    let mut writer = self.writer.lock().unwrap();
                    _ = writeln!(writer, "{answer}").and_then(|()| writer.flush());
                }
                self.shared.answering(self.number, false);
                queue = self.queue.lock().unwrap();
                for lane in &taken.lanes {
                    queue.busy.remove(lane);
                }
                queue.answering -= 1;
                if queue.answering == 0 {
                    if queue.most > 1 {
                        // check.sh reads this line.
                        let (agent, most) = (&self.agent, queue.most);
                        log!("browser: agent {agent} answered {most} calls at once");
                    }
                    queue.most = 0;
                }
                self.changed.notify_all();
                continue;
            }
            if queue.closed && queue.waiting.is_empty() {
                return;
            }
            queue.idle += 1;
            queue = self.changed.wait(queue).unwrap();
            queue.idle -= 1;
        }
    }

    /// The connection read its last: what it read is still answered, the calls among it failing
    /// as the connection has closed.
    fn close(&self) {
        self.queue.lock().unwrap().closed = true;
        self.changed.notify_all();
    }
}

/// The lanes a message holds while it's answered.
fn lanes(line: &str) -> Vec<Lane> {
    let message = serde_json::from_str::<Value>(line).unwrap_or_default();
    if message.get("method").and_then(Value::as_str) != Some("tools/call") {
        return vec![Lane::InOrder];
    }
    let tool = (message.pointer("/params/name").and_then(Value::as_str)).unwrap_or_default();
    let tab = (message
        .pointer("/params/arguments/tab")
        .and_then(Value::as_u64))
    .filter(|tab| *tab != 0);
    let own = match (tool, tab) {
        ("open_tab", _) => None,
        (_, Some(tab)) => Some(Lane::Tab(tab)),
        (_, None) => Some(Lane::InOrder),
    };
    let asks = matches!(tool, "ask" | "hand_off" | "ask_for_tab" | "point").then_some(Lane::Asks);
    own.into_iter().chain(asks).collect()
}

impl Shared {
    fn agent(&self, token: &str) -> Option<String> {
        self.tokens.lock().unwrap().get(token).cloned()
    }

    /// Connection `number` began answering a message, or wrote its answer: a stopped one closes
    /// once it has written its last.
    fn answering(&self, number: u64, began: bool) {
        let mut connections = self.connections.lock().unwrap();
        let Some(connection) = connections.get_mut(&number) else {
            return;
        };
        match began {
            true => connection.answering += 1,
            false => connection.answering -= 1,
        }
        if !began && connection.stopping && connection.answering == 0 {
            connection.close_stopped(number);
        }
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
    // The name its card shows: its title, which MCP gives for display, else its name as it is.
    let named = |pointer: &str| {
        (params.pointer(pointer).and_then(Value::as_str))
            .map(str::trim)
            .filter(|name| !name.is_empty())
    };
    let client = named("/clientInfo/title")
        .or_else(|| named("/clientInfo/name"))
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

/// Each call's number, for the UI to match its answer to it.
static NEXT_CALL: AtomicU64 = AtomicU64::new(1);

/// Runs a tool on the UI thread, and waits for its answer, which the UI then hears too: a task's
/// trace times each step by when its call came and was answered here.
fn call(shared: &Shared, number: u64, agent: &str, tool: &str, arguments: Value) -> ToolResult {
    let (reply, answer) = mpsc::channel();
    let id = NEXT_CALL.fetch_add(1, Ordering::Relaxed);
    let call = Call {
        id,
        agent: agent.into(),
        tool: tool.into(),
        arguments,
        reply,
        at: Instant::now(),
    };
    // Sent while the connection is open, under the lock `end` closes it with, so that the UI
    // hears no call of a session after its agent's bye.
    {
        let connections = shared.connections.lock().unwrap();
        match connections.get(&number) {
            None => return ToolResult::error("The connection closed."),
            // Stopped, it closes once it has answered: no call reaches the UI from then on.
            Some(connection) if connection.stopping => return ToolResult::error(super::STOPPED),
            Some(_) => {}
        }
        if shared.events.unbounded_send(Event::Call(call)).is_err() {
            return ToolResult::error("The browser has turned agents off.");
        }
    }
    // The UI drops the call unanswered when the user turns agents off or disconnects the agent.
    let result = answer
        .recv()
        .unwrap_or_else(|_| ToolResult::error("The browser has turned agents off."));
    _ = shared.events.unbounded_send(Event::Answered {
        agent: agent.into(),
        id,
        at: Instant::now(),
        error: result.error.then(|| result.text.clone()),
        image: result.image.clone(),
    });
    result
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
