//! The server as an MCP client's bridge reaches it, over a real socket, or on Windows a real named
//! pipe.

use std::io::{BufReader, Cursor, ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::sync::atomic::AtomicUsize;
use std::thread;
use std::time::Instant;

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;

use super::*;

const TOKEN: &str = "s3cret";

/// A socket or pipe of the test's own, for a folder of its own, so tests run beside each other.
pub fn socket() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    let folder = std::env::temp_dir().join(format!("agents-{}-{number}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    address(&folder)
}

/// A server with Claude Code's token.
fn server() -> (Server, Tokens, UnboundedReceiver<Event>) {
    server_waiting(TOKEN_WAIT)
}

/// A server with Claude Code's token that gives a connection `token_wait` to send one.
fn server_waiting(token_wait: Duration) -> (Server, Tokens, UnboundedReceiver<Event>) {
    let tokens: Tokens = Arc::default();
    tokens
        .lock()
        .unwrap()
        .insert(TOKEN.into(), "claude-code".into());
    let (events, handed) = futures::channel::mpsc::unbounded();
    (
        Server::listen(&socket(), tokens.clone(), events, token_wait).unwrap(),
        tokens,
        handed,
    )
}

/// How long a bounded client's read waits for the server.
const BOUND: Duration = Duration::from_secs(10);

/// One byte's read, again when a signal interrupts it, as `read_exact` reads, while BOUND hasn't
/// passed: the tests run beside others that start and reap processes, whose signals interrupt a
/// read on Linux (#3734).
fn read_byte(reader: &mut impl Read) -> std::io::Result<usize> {
    let start = Instant::now();
    loop {
        match reader.read(&mut [0]) {
            Err(error) if error.kind() == ErrorKind::Interrupted && start.elapsed() < BOUND => {}
            read => return read,
        }
    }
}

/// A client's connection, as its bridge makes it.
struct Client {
    writer: Stream,
    reader: BufReader<Stream>,
}

impl Client {
    fn open(server: &Server) -> Client {
        let stream = connect(server.path()).unwrap();
        Client {
            writer: stream.try_clone().unwrap(),
            reader: BufReader::new(stream),
        }
    }

    /// Opens one whose reads give up after BOUND, set as it connects: the Mac refuses a socket's
    /// timeout once the server has closed the connection (EINVAL).
    fn open_bounded(server: &Server) -> Client {
        let client = Client::open(server);
        client
            .reader
            .get_ref()
            .set_read_timeout(Some(BOUND))
            .unwrap();
        client
    }

    /// Asserts the server has closed this connection, which a bounded client sees within BOUND.
    fn closed(mut self) {
        match read_byte(&mut self.reader) {
            Ok(0) => {}
            Err(error) if error.kind() == ErrorKind::Interrupted => {
                panic!("a connection's read was interrupted again and again for {BOUND:?}")
            }
            read => panic!("a connection that sent no token was still open {BOUND:?} on: {read:?}"),
        }
    }

    /// Connects and sends `token` first, and gives the server's answer to it.
    fn with(server: &Server, token: &str) -> (Client, Value) {
        let mut client = Client::open(server);
        let answer = client.ask(json!({ "token": token }));
        (client, answer)
    }

    fn connect(server: &Server) -> Client {
        let (client, answer) = Client::with(server, TOKEN);
        assert_eq!(answer, json!({ "ok": true }));
        client
    }

    fn send(&mut self, message: Value) {
        writeln!(self.writer, "{message}").unwrap();
    }

    /// The server's next line, or None once it has closed the connection.
    fn read(&mut self) -> Option<Value> {
        read_line(&mut self.reader).map(|line| serde_json::from_str(&line).unwrap())
    }

    fn ask(&mut self, message: Value) -> Value {
        self.send(message);
        self.read().expect("an answer")
    }

    fn initialize(&mut self) -> Value {
        self.ask(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "claude-code", "version": "2.0.0" },
            },
        }))
    }
}

fn next(handed: &mut UnboundedReceiver<Event>) -> Option<Event> {
    futures::executor::block_on(handed.next())
}

/// The socket is the user's alone, a socket where the server said, and gone once the server is.
#[cfg(unix)]
#[test]
fn the_socket_is_the_users_alone() {
    let (server, _, _handed) = server();
    let path = server.path().to_path_buf();
    let metadata = std::fs::metadata(&path).unwrap();
    assert!(metadata.file_type().is_socket());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert!(!PathBuf::from(format!("{}.new", path.display())).exists());
    drop(server);
    assert!(!path.exists(), "the socket stayed once the server stopped");
}

/// A socket a browser that didn't stop left behind gives way to the new server's.
#[cfg(unix)]
#[test]
fn a_socket_left_behind_gives_way() {
    let path = socket();
    std::fs::write(&path, "left").unwrap();
    let (events, _handed) = futures::channel::mpsc::unbounded();
    let server = Server::start(&path, Tokens::default(), events).unwrap();
    assert!(
        std::fs::metadata(server.path())
            .unwrap()
            .file_type()
            .is_socket()
    );
}

/// The pipe is the browser's own: a second server can't make it while the first has it, a client
/// reaches it no more once the server has stopped, and the next server makes it again.
#[cfg(windows)]
#[test]
fn the_pipe_is_the_servers_alone() {
    let (server, _, _handed) = server();
    let path = server.path().to_path_buf();
    assert!(path.to_str().unwrap().starts_with(r"\\.\pipe\"));
    let (events, _second) = futures::channel::mpsc::unbounded();
    assert!(Server::start(&path, Tokens::default(), events).is_err());
    drop(server);
    assert!(
        connect(&path).is_err(),
        "the pipe stayed once the server stopped"
    );
    let (events, _again) = futures::channel::mpsc::unbounded();
    let again = Server::start(&path, Tokens::default(), events).unwrap();
    assert!(connect(again.path()).is_ok());
}

/// A connection without a token the browser gave, or one that starts with a message instead,
/// is told why and closed, and the UI hears nothing of it.
#[test]
fn a_connection_without_a_valid_token_is_refused() {
    let (server, _, mut handed) = server();
    let (mut guess, answer) = Client::with(&server, "guess");
    assert!(
        answer["error"]
            .as_str()
            .unwrap()
            .contains("Settings › Agents")
    );
    assert_eq!(guess.read(), None, "a refused connection stayed open");
    let mut early = Client::open(&server);
    let answer = early.ask(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }));
    assert!(answer.get("error").is_some());
    assert_eq!(early.read(), None);
    drop(server);
    assert!(
        next(&mut handed).is_none(),
        "a refused connection reached the UI"
    );
}

/// A client that finds every place taken is told so, through its bridge, rather than turned away
/// without a word (#3700).
#[test]
fn a_full_server_says_so() {
    use super::super::bridge;
    // Long enough that no connection gives way while the test runs.
    let (server, _, _handed) = server_waiting(Duration::from_secs(60));
    let _silent: Vec<Client> = (0..MAX_CONNECTIONS)
        .map(|_| Client::open(&server))
        .collect();
    let refused = bridge::run(
        server.path(),
        TOKEN,
        Cursor::new(Vec::new()),
        Output::default(),
    );
    assert_eq!(refused.unwrap_err(), FULL);
}

/// Connections that come while the server is full and neither send nor read hold up no one: the
/// server says it's full to each and goes on, and a client once places are free is served. Each
/// still reads the line late, after the server closed its end.
#[test]
fn connections_that_never_read_hold_up_no_one() {
    let (server, _, _handed) = server_waiting(Duration::from_secs(1));
    // Bounded where the server keeps them: its close is what each waits for.
    let silent: Vec<Client> = (0..MAX_CONNECTIONS)
        .map(|_| Client::open_bounded(&server))
        .collect();
    let deaf: Vec<Client> = (0..8).map(|_| Client::open(&server)).collect();
    for client in silent {
        client.closed();
    }
    // Were the line to the connections that don't read wait on them, the server would take no
    // other: the connection would fail, or its answer wouldn't come.
    let stream = connect(server.path())
        .expect("no connection once places were free, behind connections that never read");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut client = Client {
        writer: stream.try_clone().unwrap(),
        reader: BufReader::new(stream),
    };
    client.send(json!({ "token": TOKEN }));
    let answer = read_line(&mut client.reader)
        .expect("no answer to the token 10 s on, behind connections that never read");
    assert_eq!(
        serde_json::from_str::<Value>(&answer).unwrap(),
        json!({ "ok": true })
    );
    for mut client in deaf {
        assert_eq!(client.read(), Some(json!({ "error": FULL })));
        assert_eq!(client.read(), None);
    }
}

/// A read a signal interrupts is read again, so a test sees the server's close and not the signal
/// (#3734).
#[test]
fn an_interrupted_read_is_read_again() {
    struct Interrupted(usize);
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            match self.0 {
                0 => Ok(0),
                _ => {
                    self.0 -= 1;
                    Err(ErrorKind::Interrupted.into())
                }
            }
        }
    }
    assert_eq!(read_byte(&mut Interrupted(3)).unwrap(), 0);
}

/// A connection that sends no token is closed once its time is up, so connections that send
/// nothing can't keep a client out (#3700).
#[test]
fn connections_that_send_no_token_give_way() {
    let (server, _, _handed) = server_waiting(Duration::from_millis(200));
    // Bounded where the server keeps them: its close is what each waits for.
    let silent: Vec<Client> = (0..MAX_CONNECTIONS)
        .map(|_| Client::open_bounded(&server))
        .collect();
    for client in silent {
        client.closed();
    }
    let mut client = Client::connect(&server);
    let answer = client.ask(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }));
    assert_eq!(answer, json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));
}

/// initialize begins a session, in the version the client asked for, says what the server is and
/// how to use it, and tells the UI which agent said hello.
#[test]
fn initialize_begins_a_session() {
    let (server, _, mut handed) = server();
    let mut client = Client::connect(&server);
    let answer = client.initialize();
    assert_eq!(answer["id"], 1);
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert_eq!(result["serverInfo"]["name"], env!("APP_NAME"));
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("tabs of your own")
    );
    match next(&mut handed) {
        Some(Event::Hello {
            agent,
            client,
            connection,
        }) => {
            assert_eq!(
                (agent.as_str(), client.as_str()),
                ("claude-code", "claude-code")
            );
            assert_ne!(connection, 0, "0 is the UI's for no connection");
        }
        _ => panic!("the UI didn't hear the agent's hello"),
    }
    let newest = client.ask(json!({
        "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" },
    }));
    assert_eq!(newest["result"]["protocolVersion"], VERSIONS[0]);
}

/// The name a client's card shows is the title MCP gives it for display, else its name, trimmed.
#[test]
fn hello_gives_the_clients_title_else_its_name() {
    let (server, _, mut handed) = server();
    for (info, named) in [
        (
            json!({ "name": "zed", "title": " Zed ", "version": "1" }),
            "Zed",
        ),
        (json!({ "name": "zed", "title": "", "version": "1" }), "zed"),
        (json!({ "version": "1" }), ""),
    ] {
        let mut client = Client::connect(&server);
        client.ask(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": info },
        }));
        match next(&mut handed) {
            Some(Event::Hello { client, .. }) => assert_eq!(client, named),
            _ => panic!("the UI didn't hear the agent's hello"),
        }
        drop(client);
        assert!(matches!(next(&mut handed), Some(Event::Closed { .. })));
        assert!(matches!(next(&mut handed), Some(Event::Bye { .. })));
    }
}

/// A connection's hello and its calls carry its number, which tells the UI's sessions apart: two
/// connections of one agent have two.
#[test]
fn hello_and_calls_carry_their_connection() {
    let (server, _, mut handed) = server();
    let (mut first, _first_answers) = session(&server, &mut handed);
    let (mut second, _second_answers) = session(&server, &mut handed);
    send_call(&mut first, 2, "read_page", json!({ "tab": 1 }));
    let on_first = next_call(&mut handed).expect("the first connection's call");
    send_call(&mut second, 2, "read_page", json!({ "tab": 2 }));
    let on_second = next_call(&mut handed).expect("the second connection's call");
    assert_ne!(on_first.connection, on_second.connection);

    let mut third = Client::connect(&server);
    third.initialize();
    let Some(Event::Hello { connection, .. }) = next(&mut handed) else {
        panic!("the UI didn't hear the agent's hello");
    };
    third.send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "read_page", "arguments": { "tab": 3 } },
    }));
    let on_third = next_call(&mut handed).expect("the third connection's call");
    assert_eq!(on_third.connection, connection);
    assert!(![on_first.connection, on_second.connection].contains(&connection));
}

/// The tools list says what each takes; a notification is answered with nothing; an unknown
/// method, a line that isn't JSON and a batch are errors, and the connection goes on.
#[test]
fn the_server_answers_what_needs_no_browser() {
    let (server, _, _handed) = server();
    let mut client = Client::connect(&server);
    let list = client.ask(json!({ "jsonrpc": "2.0", "id": "a", "method": "tools/list" }));
    let names: Vec<_> = (list["result"]["tools"].as_array().unwrap().iter())
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "list_tabs",
            "open_tab",
            "close_tab",
            "split_tabs",
            "add_to_split",
            "separate_tab",
            "list_split",
            "navigate",
            "snapshot",
            "read_page",
            "click",
            "type",
            "select",
            "press_key",
            "scroll",
            "screenshot",
            "request",
            "run_steps",
            "upload",
            "hand_off",
            "sign_in",
            "ask_for_tab",
            "pin_tab",
            "unpin_tab",
            "remove_tab",
            "sort_tabs_by_site",
            "title",
            "done",
        ]
    );
    assert_eq!(list["id"], "a");
    client.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    // The next line is the ping's, so the notification had none.
    let ping = client.ask(json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }));
    assert_eq!(
        (ping["id"].clone(), ping["result"].clone()),
        (json!(2), json!({}))
    );
    let unknown = client.ask(json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" }));
    assert_eq!(unknown["error"]["code"], -32601);
    writeln!(client.writer, "{{").unwrap();
    assert_eq!(client.read().unwrap()["error"]["code"], -32700);
    let batch = client.ask(json!([{ "jsonrpc": "2.0", "id": 4, "method": "ping" }]));
    assert_eq!(batch["error"]["code"], -32600);
    assert_eq!(
        client.ask(json!({ "jsonrpc": "2.0", "id": 5, "method": "ping" }))["id"],
        5
    );
}

/// A tool call waits for the UI thread's answer, and gives it as MCP's result.
#[test]
fn a_tool_call_is_the_ui_threads_to_answer() {
    let (server, _, mut handed) = server();
    let mut client = Client::connect(&server);
    let calling = thread::spawn(move || {
        client.ask(json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "read_page", "arguments": { "tab": 20 } },
        }))
    });
    match next(&mut handed) {
        Some(Event::Call(call)) => {
            assert_eq!(
                (call.agent.as_str(), call.tool.as_str()),
                ("claude-code", "read_page")
            );
            assert_eq!(call.arguments, json!({ "tab": 20 }));
            call.reply.send(ToolResult::text("Example Domain")).unwrap();
        }
        _ => panic!("the call didn't reach the UI thread"),
    }
    let answer = calling.join().unwrap();
    assert_eq!(answer["id"], 7);
    assert_eq!(
        answer["result"]["content"][0],
        json!({ "type": "text", "text": "Example Domain" })
    );
    assert_eq!(answer["result"]["isError"], false);
}

/// A call the UI drops, as when the user turns agents off, is answered with an error.
#[test]
fn a_dropped_call_is_an_error() {
    let (server, _, mut handed) = server();
    let mut client = Client::connect(&server);
    let calling = thread::spawn(move || {
        let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "list_tabs" } });
        client.ask(call)
    });
    drop(next(&mut handed));
    assert_eq!(calling.join().unwrap()["result"]["isError"], true);
}

/// Each connection closing tells the UI which closed, and the agent's last tells it after that
/// the agent's gone, and not before.
#[test]
fn each_connection_closing_says_so_and_the_last_says_bye() {
    let (server, _, mut handed) = server();
    let mut first = Client::connect(&server);
    first.initialize();
    let Some(Event::Hello {
        connection: one, ..
    }) = next(&mut handed)
    else {
        panic!("the UI didn't hear the first hello");
    };
    let mut second = Client::connect(&server);
    second.initialize();
    let Some(Event::Hello {
        connection: two, ..
    }) = next(&mut handed)
    else {
        panic!("the UI didn't hear the second hello");
    };
    drop(first);
    match next(&mut handed) {
        Some(Event::Closed { agent, connection }) => {
            assert_eq!((agent.as_str(), connection), ("claude-code", one))
        }
        _ => panic!("the UI didn't hear the first connection close"),
    }
    drop(second);
    match next(&mut handed) {
        Some(Event::Closed { agent, connection }) => {
            assert_eq!((agent.as_str(), connection), ("claude-code", two))
        }
        _ => panic!("the agent said bye with a connection left"),
    }
    match next(&mut handed) {
        Some(Event::Bye { agent }) => assert_eq!(agent, "claude-code"),
        _ => panic!("the UI didn't hear the agent go"),
    }
    drop(server);
    assert!(next(&mut handed).is_none(), "the agent said bye twice");
}

/// A client that closes its end while its call waits on the user, as on an ask before opening a
/// bank's site, ends its session: the UI hears the agent go and drops the call, as it drops what
/// waits for an agent that went, and the server closes the connection, which the bridge waits for.
#[test]
fn a_client_closing_while_a_call_waits_ends_its_session() {
    let (server, _, mut handed) = server();
    let mut client = Client::connect(&server);
    client.initialize();
    assert!(matches!(next(&mut handed), Some(Event::Hello { .. })));
    client.send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "open_tab", "arguments": { "url": "bank.example" } },
    }));
    let Some(Event::Call(waiting)) = next(&mut handed) else {
        panic!("the call didn't reach the UI thread");
    };
    close_write(&mut client.writer).unwrap();
    assert!(matches!(next(&mut handed), Some(Event::Closed { .. })));
    match next(&mut handed) {
        Some(Event::Bye { agent }) => assert_eq!(agent, "claude-code"),
        _ => panic!("the UI didn't hear the agent go while its call waited"),
    }
    drop(waiting);
    assert_eq!(client.read().unwrap()["result"]["isError"], true);
    assert_eq!(client.read(), None, "the server kept the connection open");
}

/// Once the user disconnects the agent, its connection answers no more: the server closes it,
/// and a token taken away stops the next message too.
#[test]
fn a_disconnected_agents_connection_closes() {
    let (server, tokens, _handed) = server();
    let mut closed = Client::connect(&server);
    server.disconnect("claude-code");
    assert_eq!(closed.read(), None);

    let mut later = Client::connect(&server);
    tokens.lock().unwrap().clear();
    later.send(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }));
    assert_eq!(later.read(), None);
}

/// A line is read up to its end, and one too long ends the connection.
#[test]
fn lines_are_read_one_after_another() {
    let mut reader = Cursor::new(&b"{\"a\":1}\r\n{\"b\":2}\nlast"[..]);
    assert_eq!(read_line(&mut reader).as_deref(), Some("{\"a\":1}"));
    assert_eq!(read_line(&mut reader).as_deref(), Some("{\"b\":2}"));
    assert_eq!(read_line(&mut reader).as_deref(), Some("last"));
    assert_eq!(read_line(&mut reader), None);
    let endless = "a".repeat(MAX_LINE + 10);
    assert_eq!(read_line(&mut Cursor::new(endless.as_bytes())), None);
}

/// What the bridge writes, for the test to read once it's done.
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The bridge carries a client's stdio to the server and back until the client closes its end,
/// and with a token the browser didn't give it fails, saying why.
#[test]
fn the_bridge_carries_a_clients_stdio() {
    use super::super::bridge;
    let (server, _, _handed) = server();
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec();
    let output = Output::default();
    bridge::run(server.path(), TOKEN, Cursor::new(input), output.clone()).unwrap();
    let written = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(written.ends_with('\n') && written.lines().count() == 1);
    let answer: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(answer, json!({ "jsonrpc": "2.0", "id": 1, "result": {} }));

    let refused = bridge::run(
        server.path(),
        "guess",
        Cursor::new(Vec::new()),
        Output::default(),
    );
    assert!(refused.unwrap_err().contains("Settings › Agents"));
    let path = server.path().to_path_buf();
    drop(server);
    let off = bridge::run(&path, TOKEN, Cursor::new(Vec::new()), Output::default());
    assert!(off.unwrap_err().contains("agents on"));
}

/// The UI's next call, past what else it hears, or None if none comes in a second.
fn next_call(handed: &mut UnboundedReceiver<Event>) -> Option<Call> {
    call_within(handed, 1000)
}

/// Whether no call reaches the UI in a while, as one that waits its turn doesn't.
fn no_call(handed: &mut UnboundedReceiver<Event>) -> bool {
    call_within(handed, 100).is_none()
}

fn call_within(handed: &mut UnboundedReceiver<Event>, milliseconds: u64) -> Option<Call> {
    let until = Instant::now() + std::time::Duration::from_millis(milliseconds);
    while Instant::now() < until {
        match handed.try_recv() {
            Ok(Event::Call(call)) => return Some(call),
            Ok(_) => {}
            Err(_) => thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
    None
}

/// A connection's session, its client sending calls and reading answers on threads of its own.
fn session(
    server: &Server,
    handed: &mut UnboundedReceiver<Event>,
) -> (Stream, mpsc::Receiver<Value>) {
    let mut client = Client::connect(server);
    client.initialize();
    assert!(matches!(next(handed), Some(Event::Hello { .. })));
    let (answered, answers) = mpsc::channel();
    let writer = client.writer.try_clone().unwrap();
    thread::spawn(move || {
        while let Some(answer) = client.read() {
            _ = answered.send(answer);
        }
    });
    (writer, answers)
}

fn send_call(writer: &mut Stream, id: u64, tool: &str, arguments: Value) {
    let call = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments },
    });
    writeln!(writer, "{call}").unwrap();
}

fn answered(answers: &mpsc::Receiver<Value>) -> Value {
    answers
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("an answer")["id"]
        .clone()
}

/// A page slow to answer holds only its own tab: the call on the next tab reaches the UI and is
/// answered while the first still waits, each answer by its call's id (#1476).
#[test]
fn a_slow_call_on_one_tab_holds_no_other_tab() {
    let (server, _, mut handed) = server();
    let (mut writer, answers) = session(&server, &mut handed);
    send_call(&mut writer, 2, "click", json!({ "tab": 1, "ref": "e1" }));
    send_call(&mut writer, 3, "click", json!({ "tab": 2, "ref": "e1" }));
    // The two tabs' calls go at once, so either can reach the UI first.
    let mut calls = [
        next_call(&mut handed).expect("a tab's call"),
        next_call(&mut handed).expect("one tab's call waited for the other's"),
    ];
    calls.sort_by_key(|call| call.arguments["tab"].as_u64());
    let [slow, quick] = calls;
    let tabs = (
        slow.arguments["tab"].clone(),
        quick.arguments["tab"].clone(),
    );
    assert_eq!(tabs, (json!(1), json!(2)));
    quick.reply.send(ToolResult::text("Clicked.")).unwrap();
    assert_eq!(answered(&answers), 3);
    slow.reply.send(ToolResult::text("Clicked.")).unwrap();
    assert_eq!(answered(&answers), 2);
}

/// One tab's calls go in the order they came, the next waiting for the one before it, and a call
/// with no tab, which acts in the tab used last, waits for the tabs' calls before it.
#[test]
fn a_tabs_calls_keep_their_order() {
    let (server, _, mut handed) = server();
    let (mut writer, answers) = session(&server, &mut handed);
    send_call(&mut writer, 2, "click", json!({ "tab": 1, "ref": "e1" }));
    send_call(
        &mut writer,
        3,
        "type",
        json!({ "tab": 1, "ref": "e2", "text": "Closed" }),
    );
    send_call(&mut writer, 4, "press_key", json!({ "key": "Enter" }));
    let first = next_call(&mut handed).expect("the first call");
    assert_eq!(first.tool, "click");
    assert!(
        no_call(&mut handed),
        "a call went before the one ahead of it"
    );
    first.reply.send(ToolResult::text("Clicked.")).unwrap();
    assert_eq!(answered(&answers), 2);
    let second = next_call(&mut handed).expect("the second call");
    assert_eq!(second.tool, "type");
    assert!(
        no_call(&mut handed),
        "the call with no tab went before tab 1's"
    );
    second.reply.send(ToolResult::text("Typed.")).unwrap();
    assert_eq!(answered(&answers), 3);
    let last = next_call(&mut handed).expect("the call with no tab");
    assert_eq!(last.tool, "press_key");
}

/// What asks the user asks one at a time, the next waiting its turn, while calls on other tabs go
/// on; and a turn's tabs open at once.
#[test]
fn asks_come_one_at_a_time() {
    let (server, _, mut handed) = server();
    let (mut writer, answers) = session(&server, &mut handed);
    send_call(&mut writer, 2, "ask_for_tab", json!({ "tab": 1 }));
    send_call(&mut writer, 3, "ask_for_tab", json!({ "tab": 2 }));
    send_call(&mut writer, 4, "click", json!({ "tab": 3, "ref": "e1" }));
    // The first ask and tab 3's call go at once, so either can reach the UI first.
    let first = next_call(&mut handed).expect("the first ask_for_tab or tab 3's call");
    let second = next_call(&mut handed).expect("tab 3's call or the first ask_for_tab waited");
    let (asking, click) = match first.tool.as_str() {
        "click" => (second, first),
        _ => (first, second),
    };
    assert_eq!(
        (asking.tool.as_str(), asking.arguments["tab"].clone()),
        ("ask_for_tab", json!(1))
    );
    assert_eq!(click.tool, "click", "two asks at once");
    assert!(no_call(&mut handed), "two asks at once");
    click.reply.send(ToolResult::text("Clicked.")).unwrap();
    assert_eq!(answered(&answers), 4);
    asking.reply.send(ToolResult::text("Done.")).unwrap();
    assert_eq!(answered(&answers), 2);
    let next_ask = next_call(&mut handed).expect("the second ask_for_tab");
    assert_eq!(next_ask.arguments["tab"], 2);
    next_ask.reply.send(ToolResult::text("Done.")).unwrap();
    assert_eq!(answered(&answers), 3);

    for id in 5..15 {
        let url = format!("case-{id}.example");
        send_call(&mut writer, id, "open_tab", json!({ "url": url }));
    }
    let opening: Vec<Call> = (5..15).filter_map(|_| next_call(&mut handed)).collect();
    assert_eq!(opening.len(), 10, "a turn's tabs opened one after another");
}

/// A hand-off is its tab's alone: two tabs handed off at once both reach the UI, neither waiting
/// for the other, and each is answered when its own tab is handed back, in whatever order.
#[test]
fn hand_offs_on_two_tabs_wait_for_neither() {
    let (server, _, mut handed) = server();
    let (mut writer, answers) = session(&server, &mut handed);
    send_call(
        &mut writer,
        2,
        "hand_off",
        json!({ "tab": 1, "reason": "Sign in" }),
    );
    send_call(
        &mut writer,
        3,
        "hand_off",
        json!({ "tab": 2, "reason": "Sign in" }),
    );
    let mut calls: Vec<Call> = (0..2).filter_map(|_| next_call(&mut handed)).collect();
    assert_eq!(
        calls.len(),
        2,
        "the second tab's hand_off waited for the first's"
    );
    calls.sort_by_key(|call| call.arguments["tab"].as_u64());
    let [one, two] = <[Call; 2]>::try_from(calls).ok().unwrap();
    two.reply.send(ToolResult::text("Handed back.")).unwrap();
    assert_eq!(answered(&answers), 3, "tab 2's hand-off waited for tab 1's");
    one.reply.send(ToolResult::text("Handed back.")).unwrap();
    assert_eq!(answered(&answers), 2);
    // A call on a handed-off tab still waits for its tab.
    send_call(
        &mut writer,
        4,
        "hand_off",
        json!({ "tab": 1, "reason": "Sign in" }),
    );
    send_call(&mut writer, 5, "click", json!({ "tab": 1, "ref": "e1" }));
    let again = next_call(&mut handed).expect("tab 1's hand_off");
    assert_eq!(again.tool, "hand_off");
    assert!(
        no_call(&mut handed),
        "a click on a tab the user has went before it was handed back"
    );
    again.reply.send(ToolResult::text("Handed back.")).unwrap();
    assert_eq!(answered(&answers), 4);
    assert_eq!(next_call(&mut handed).expect("tab 1's click").tool, "click");
}

/// Stopped by the user, a connection answers the call under way, refuses the calls after it
/// without the UI hearing them, then closes, and the UI hears the agent's bye (#1080).
#[test]
fn a_stopped_agents_connection_closes_once_it_has_answered() {
    let (server, _, mut handed) = server();
    let (mut writer, answers) = session(&server, &mut handed);
    send_call(&mut writer, 2, "click", json!({ "tab": 1, "ref": "e1" }));
    let under_way = next_call(&mut handed).expect("the call under way");
    server.stop("claude-code");
    send_call(&mut writer, 3, "click", json!({ "tab": 2, "ref": "e1" }));
    let refused = answers
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("an answer to the call after Stop");
    assert_eq!(refused["id"], 3);
    assert_eq!(refused["result"]["isError"], true);
    assert_eq!(
        refused["result"]["content"][0]["text"],
        super::super::STOPPED
    );
    assert!(no_call(&mut handed), "a call after Stop reached the UI");
    under_way
        .reply
        .send(ToolResult::error(super::super::STOPPED))
        .unwrap();
    assert_eq!(answered(&answers), 2);
    assert!(
        matches!(
            answers.recv_timeout(std::time::Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ),
        "the connection stayed open once it had answered"
    );
    let until = Instant::now() + std::time::Duration::from_secs(1);
    let mut bye = false;
    while !bye && Instant::now() < until {
        match handed.try_recv() {
            Ok(Event::Bye { agent }) => bye = agent == "claude-code",
            Ok(_) => {}
            Err(_) => thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
    assert!(bye, "the UI didn't hear the stopped agent's bye");
}

/// done has no tab, so on its connection it waits for the calls before it, as a task's finish does:
/// sent while a hand_off waits on the user, it's answered once that is, never beside a question
/// (#3652).
#[test]
fn done_waits_for_the_calls_before_it() {
    let line = |tool: &str, arguments: &str| {
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{arguments}}}}}"#
        )
    };
    let done = lanes(&line("done", "{}"));
    for (tool, arguments) in [
        ("hand_off", r#"{"tab":7,"reason":"Sign in"}"#),
        ("ask", r#"{"question":"Which one?"}"#),
        ("click", r#"{"tab":7,"ref":"e1"}"#),
    ] {
        let before = lanes(&line(tool, arguments));
        assert!(
            done.iter()
                .any(|lane| before.iter().any(|other| lane.meets(*other))),
            "done didn't wait for {tool}"
        );
    }
}
