//! The server as an MCP client's bridge reaches it, over a real socket, or on Windows a real named
//! pipe.

use std::io::{BufReader, Cursor, Write};
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::sync::atomic::AtomicUsize;
use std::thread;

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
    let tokens: Tokens = Arc::default();
    tokens
        .lock()
        .unwrap()
        .insert(TOKEN.into(), "claude-code".into());
    let (events, handed) = futures::channel::mpsc::unbounded();
    (
        Server::start(&socket(), tokens.clone(), events).unwrap(),
        tokens,
        handed,
    )
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
            .contains("Space of your own")
    );
    match next(&mut handed) {
        Some(Event::Hello { agent, client }) => {
            assert_eq!(
                (agent.as_str(), client.as_str()),
                ("claude-code", "claude-code")
            )
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
        assert!(matches!(next(&mut handed), Some(Event::Bye { .. })));
    }
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
            "hand_off",
            "ask_for_tab",
            "point",
            "pin_tab",
            "unpin_tab",
            "remove_tab"
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

/// The agent's last connection closing tells the UI it's gone, and not before.
#[test]
fn the_agents_last_session_ending_says_bye() {
    let (server, _, mut handed) = server();
    let mut first = Client::connect(&server);
    first.initialize();
    let mut second = Client::connect(&server);
    second.initialize();
    assert!(matches!(next(&mut handed), Some(Event::Hello { .. })));
    assert!(matches!(next(&mut handed), Some(Event::Hello { .. })));
    drop(first);
    drop(second);
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
    send_call(&mut writer, 4, "click", json!({ "tab": 3, "ref": "e1" }));
    // The first ask and tab 3's call go at once, so either can reach the UI first.
    let first = next_call(&mut handed).expect("the first hand_off or tab 3's call");
    let second = next_call(&mut handed).expect("tab 3's call or the first hand_off waited");
    let (asking, click) = match first.tool.as_str() {
        "click" => (second, first),
        _ => (first, second),
    };
    assert_eq!(
        (asking.tool.as_str(), asking.arguments["tab"].clone()),
        ("hand_off", json!(1))
    );
    assert_eq!(click.tool, "click", "two asks at once");
    assert!(no_call(&mut handed), "two asks at once");
    click.reply.send(ToolResult::text("Clicked.")).unwrap();
    assert_eq!(answered(&answers), 4);
    asking.reply.send(ToolResult::text("Done.")).unwrap();
    assert_eq!(answered(&answers), 2);
    let next_ask = next_call(&mut handed).expect("the second hand_off");
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
