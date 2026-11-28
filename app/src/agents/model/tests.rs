//! The model layer against a stand-in maker on a local port: its listing, its stream, how each
//! status and a broken stream fail, and a maker ilo has no adapter for, run through a whole
//! conversation by adding one Wire.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use futures::channel::mpsc;
use serde_json::{Value, json};

use super::compat::Compat;
use super::http::{self, Endpoint, Key, Remote};
use super::sse::Sse;
use super::*;
use crate::agents::runner::Out;
use crate::agents::turns::{Step, Turns};

const KEY: &str = "sk-test-secret-1234";

enum Answer {
    /// A whole response, after which the connection closes.
    Send(String),
    /// No response for longer than any patience here.
    Hang,
}

/// What the stand-in was sent: its request line and headers, and its body.
struct Seen {
    head: String,
    body: String,
}

struct Stand {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

/// A maker on a local port that gives each connection the next answer.
fn stand(answers: Vec<Answer>) -> Stand {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for answer in answers {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line.to_ascii_lowercase());
            }
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|length| length.trim().parse().ok())
                .unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            log.lock().unwrap().push(Seen {
                head,
                body: String::from_utf8_lossy(&body).into(),
            });
            match answer {
                Answer::Send(raw) => {
                    let _ = stream.write_all(raw.as_bytes());
                }
                Answer::Hang => std::thread::sleep(Duration::from_secs(2)),
            }
        }
    });
    Stand { base, seen }
}

/// A stream of these events, each a JSON `data:` line.
fn events(events: &[Value]) -> Answer {
    let body: String = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
    Answer::Send(format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{body}"
    ))
}

fn status(code: u16, headers: &str, body: &str) -> Answer {
    Answer::Send(format!(
        "HTTP/1.1 {code} Status\r\ncontent-length: {}\r\n{headers}connection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn endpoint(stand: &Stand) -> Endpoint {
    Endpoint {
        base: stand.base.clone(),
        key: Some(Key::new(KEY.into())),
        patience: Duration::from_millis(200),
    }
}

fn ask(text: &str) -> Request {
    Request {
        instructions: "Work in the browser.".into(),
        items: vec![Item::User(vec![Part::Text(text.into())])],
        tools: Vec::new(),
        effort: None,
        schema: None,
        thinking: false,
        max_out: 1000,
    }
}

/// One turn of `wire` at the stand-in, as its deltas.
fn turn(wire: &dyn Wire, stand: &Stand) -> Vec<Delta> {
    let mut deltas = Vec::new();
    let cancel = AtomicBool::new(false);
    http::stream(
        wire,
        &endpoint(stand),
        "m",
        &ask("Hi"),
        &cancel,
        &mut |delta| deltas.push(delta),
    );
    deltas
}

/// A maker ilo has no adapter for, with a listing and a stream of its own shapes.
struct Fake;
static FAKE: Fake = Fake;

impl Wire for Fake {
    fn maker(&self) -> &'static str {
        "fake"
    }

    fn listing(&self) -> &'static str {
        "/catalog"
    }

    fn auth(&self, key: &str) -> Vec<(&'static str, String)> {
        vec![("x-fake-key", key.into())]
    }

    fn models(&self, listing: &Value) -> Vec<Listed> {
        let models = listing["models"].as_array().cloned().unwrap_or_default();
        models
            .iter()
            .map(|model| Listed {
                id: model["key"].as_str().unwrap_or_default().into(),
                name: model["title"].as_str().unwrap_or_default().into(),
                caps: Caps {
                    efforts: serde_json::from_value(model["levels"].clone()).unwrap_or_default(),
                    ..Caps::plain(model["window"].as_u64().unwrap_or(0) as u32)
                },
            })
            .collect()
    }

    fn call(&self, model: &str, request: &Request) -> (String, Value) {
        let log: Vec<Value> = request
            .items
            .iter()
            .filter_map(|item| match item {
                Item::User(parts) => Some(json!({ "user": text_of(parts) })),
                Item::Said(text) => Some(json!({ "model": text })),
                Item::Call { id, name, args } => {
                    Some(json!({ "call": { "id": id, "name": name, "args": args } }))
                }
                Item::Result { id, parts, .. } => {
                    Some(json!({ "result": { "id": id, "text": text_of(parts) } }))
                }
                Item::Thought(_) => None,
            })
            .collect();
        let body = json!({ "log": log, "effort": request.effort, "tools": request.tools.len() });
        (format!("/talk/{model}"), body)
    }

    fn reader(&self) -> Box<dyn Reader> {
        struct Talk;
        impl Reader for Talk {
            fn event(&mut self, _: Option<&str>, data: &str) -> Vec<Delta> {
                let event: Value = serde_json::from_str(data).unwrap_or_default();
                if let Some(text) = event["say"].as_str() {
                    return vec![Delta::Text(text.into())];
                }
                if let Some(call) = event.get("use") {
                    return vec![Delta::Call {
                        id: call["id"].as_str().unwrap_or_default().into(),
                        name: call["tool"].as_str().unwrap_or_default().into(),
                        args: call["with"].clone(),
                    }];
                }
                match event["done"].as_str() {
                    Some("calls") => vec![Delta::Ended(Stop::Calls)],
                    Some(_) => vec![Delta::Ended(Stop::Done)],
                    None => Vec::new(),
                }
            }
        }
        Box::new(Talk)
    }
}

#[test]
fn a_maker_ilo_has_no_adapter_for_is_one_wire_and_runs_a_whole_conversation() {
    let stand = stand(vec![
        Answer::Send({
            let listing = json!({ "models": [
                { "key": "fake-1", "title": "Fake One", "window": 64000, "levels": ["quick", "deep"] },
            ] })
            .to_string();
            format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{listing}",
                listing.len()
            )
        }),
        events(&[
            json!({ "say": "Reading the page." }),
            json!({ "use": { "id": "u1", "tool": "read_page", "with": { "tab": 3 } } }),
            json!({ "done": "calls" }),
        ]),
        events(&[
            json!({ "say": "It says hello." }),
            json!({ "done": "stop" }),
        ]),
    ]);
    let endpoint = Arc::new(endpoint(&stand));
    let listed = http::list(&FAKE, &endpoint).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id.as_str(), listed[0].name.as_str()),
        ("fake-1", "Fake One")
    );
    assert_eq!(listed[0].caps.efforts, ["quick", "deep"]);
    assert_eq!(listed[0].caps.context, 64000);

    let remote = Remote::new(&FAKE, endpoint, listed[0].clone());
    let tool = Tool {
        name: "read_page".into(),
        description: "Reads a tab.".into(),
        schema: json!({ "type": "object" }),
    };
    let mut turns = Turns::new(
        remote.caps().clone(),
        "Work in the browser.".into(),
        vec![tool],
        Some("deep".into()),
        1000,
    );
    let (sender, mut deltas) = mpsc::unbounded();
    let (mut outs, mut ran) = (Vec::new(), Vec::new());
    let mut steps: VecDeque<Step> = turns
        .say(vec![Part::Text("What does tab 3 say?".into())], false)
        .into();
    let ended = loop {
        let Some(step) = steps.pop_front() else {
            let (turn, delta) = futures::executor::block_on(deltas.next()).unwrap();
            steps.extend(turns.delta(turn, delta, Instant::now()));
            continue;
        };
        match step {
            Step::Model { turn, request } => remote.turn(turn, request, sender.clone()),
            // The test is the gate, and the tab says hello.
            Step::Run { id, name, args } => {
                ran.push((name, args));
                steps.extend(turns.result(&id, vec![Part::Text("hello".into())], false));
            }
            Step::Out(out) => outs.push(out),
            Step::Cancel(turn) => panic!("cancelled turn {turn}"),
            Step::Ended(why) => break why,
        }
    };
    assert_eq!(ended, None);
    assert_eq!(ran, [("read_page".to_string(), json!({ "tab": 3 }))]);
    assert_eq!(
        outs,
        [
            Out::Writing(String::new()),
            Out::Writing("Reading the page.".into()),
            Out::Said("Reading the page.".into()),
            Out::Calling,
            Out::Writing(String::new()),
            Out::Writing("It says hello.".into()),
            Out::Said("It says hello.".into()),
        ]
    );
    let seen = stand.seen.lock().unwrap();
    assert!(
        seen[0].head.starts_with("get /catalog "),
        "{}",
        seen[0].head
    );
    assert!(
        seen[1].head.starts_with("post /talk/fake-1 "),
        "{}",
        seen[1].head
    );
    assert!(seen[1].head.contains(&format!("x-fake-key: {KEY}")));
    let second: Value = serde_json::from_str(&seen[2].body).unwrap();
    assert_eq!(second["effort"], "deep");
    assert_eq!(
        second["log"].as_array().unwrap()[1..],
        [
            json!({ "model": "Reading the page." }),
            json!({ "call": { "id": "u1", "name": "read_page", "args": { "tab": 3 } } }),
            json!({ "result": { "id": "u1", "text": "hello" } }),
        ]
    );
}

#[test]
fn each_status_says_why_a_turn_failed() {
    let message = json!({ "error": { "message": "Unknown tool" } }).to_string();
    for (answer, failure) in [
        (status(401, "", ""), Failure::Refused),
        (status(403, "", ""), Failure::Refused),
        (status(402, "", ""), Failure::Credit),
        (status(404, "", ""), Failure::Withdrawn),
        (
            status(400, "", &message),
            Failure::Other("Unknown tool".into()),
        ),
    ] {
        let stand = stand(vec![answer]);
        assert_eq!(turn(&FAKE, &stand), [Delta::Failed(failure)]);
        assert_eq!(
            stand.seen.lock().unwrap().len(),
            1,
            "a refusal isn't tried again"
        );
    }
}

#[test]
fn a_limit_says_until_when_its_maker_says() {
    let stand = stand(vec![status(429, "retry-after: 30\r\n", "")]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let [Delta::Failed(Failure::Limit(Some(until)))] = turn(&FAKE, &stand)[..] else {
        panic!("not a limit with its time");
    };
    assert!((now + 29..=now + 31).contains(&until));
}

#[test]
fn a_busy_maker_is_tried_twice_more_quietly_before_its_turn_fails() {
    let stand = stand(vec![
        status(529, "", ""),
        status(503, "", ""),
        status(500, "", ""),
    ]);
    assert_eq!(turn(&FAKE, &stand), [Delta::Failed(Failure::Busy)]);
    assert_eq!(stand.seen.lock().unwrap().len(), 3);
}

#[test]
fn a_maker_busy_once_streams_on_its_retry_as_if_it_never_was() {
    let stand = stand(vec![
        status(529, "", ""),
        events(&[json!({ "say": "Hi" }), json!({ "done": "stop" })]),
    ]);
    assert_eq!(
        turn(&FAKE, &stand),
        [Delta::Text("Hi".into()), Delta::Ended(Stop::Done)]
    );
}

#[test]
fn no_answer_in_time_is_unreachable() {
    let stand = stand(vec![Answer::Hang]);
    let start = Instant::now();
    assert_eq!(turn(&FAKE, &stand), [Delta::Failed(Failure::Unreachable)]);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_stream_that_breaks_off_fails_after_what_it_said_and_isnt_tried_again() {
    let stand = stand(vec![events(&[json!({ "say": "Half" })])]);
    assert_eq!(
        turn(&FAKE, &stand),
        [
            Delta::Text("Half".into()),
            Delta::Failed(Failure::Unreachable)
        ]
    );
    assert_eq!(stand.seen.lock().unwrap().len(), 1);
}

#[test]
fn a_redirect_is_never_followed_with_the_key() {
    let elsewhere = stand(vec![events(&[json!({ "done": "stop" })])]);
    let location = format!("location: {}/talk/m\r\n", elsewhere.base);
    let stand = stand(vec![status(307, &location, "")]);
    assert_eq!(
        turn(&FAKE, &stand),
        [Delta::Failed(Failure::Other(String::new()))]
    );
    assert!(elsewhere.seen.lock().unwrap().is_empty());
}

#[test]
fn a_key_is_never_printed_even_where_its_maker_repeats_it() {
    let message = json!({ "error": { "message": format!("Bad key {KEY}") } }).to_string();
    let stand = stand(vec![status(400, "", &message)]);
    assert_eq!(
        turn(&FAKE, &stand),
        [Delta::Failed(Failure::Other("Bad key …".into()))]
    );
    let key = Key::new(KEY.into());
    assert!(!format!("{key:?} {:?}", endpoint(&stand)).contains("secret"));
    assert_eq!(key.last4(), "1234");
}

#[test]
fn server_sent_events_end_at_a_blank_line_or_the_close() {
    let mut sse = Sse::default();
    let mut read = |line: &str| sse.line(line);
    assert_eq!(read(": a comment\r\n"), None);
    assert_eq!(read("event: delta\r\n"), None);
    assert_eq!(read("data: {\"a\":\n"), None);
    assert_eq!(read("data:1}\n"), None);
    assert_eq!(
        read("\r\n"),
        Some((Some("delta".into()), "{\"a\":\n1}".into()))
    );
    assert_eq!(read("\n"), None);
    assert_eq!(read("data: last"), None);
    assert_eq!(sse.finish(), Some((None, "last".into())));
}

#[test]
fn compat_lists_what_a_gateway_says_of_each_model_and_assumes_the_rest() {
    let listing = json!({ "data": [
        { "id": "vendor/big", "name": "Big", "context_length": 200000,
          "supported_parameters": ["tools", "reasoning", "structured_outputs"],
          "architecture": { "input_modalities": ["text", "image"] } },
        { "id": "vendor/chat", "supported_parameters": ["temperature"] },
        { "id": "llama3.2:3b", "object": "model" },
    ] });
    let listed = Compat.models(&listing);
    assert_eq!(listed[0].name, "Big");
    assert_eq!(
        listed[0].caps,
        Caps {
            images_in: true,
            structured: true,
            thinking: Thinking::Summaries,
            ..Caps::plain(200000)
        }
    );
    assert!(
        !listed[1].caps.tools,
        "a model whose listing names no tools is chat only"
    );
    assert_eq!(listed[2].name, "llama3.2:3b");
    assert_eq!(listed[2].caps, Caps::plain(8192));
}

#[test]
fn compat_sends_calls_with_what_was_said_before_them_and_pictures_after_the_results() {
    let mut request = ask("Compare");
    request.effort = Some("low".into());
    request.tools = vec![Tool {
        name: "read".into(),
        description: "Reads a tab.".into(),
        schema: json!({ "type": "object" }),
    }];
    request.items.extend([
        Item::Thought(Signed {
            maker: "anthropic".into(),
            text: "t".into(),
            signature: "s".into(),
        }),
        Item::Said("Reading both.".into()),
        Item::Call {
            id: "a".into(),
            name: "read".into(),
            args: json!({ "tab": 1 }),
        },
        Item::Call {
            id: "b".into(),
            name: "read".into(),
            args: json!({ "tab": 2 }),
        },
        Item::Result {
            id: "a".into(),
            parts: vec![Part::Text("one".into())],
            error: false,
        },
        Item::Result {
            id: "b".into(),
            parts: vec![
                Part::Text("two".into()),
                Part::Image {
                    media: "image/png".into(),
                    base64: "AAAA".into(),
                },
            ],
            error: false,
        },
    ]);
    let (path, body) = Compat.call("local", &request);
    assert_eq!(path, "/chat/completions");
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["tools"][0]["function"]["name"], "read");
    assert_eq!(
        body["messages"],
        json!([
            { "role": "system", "content": "Work in the browser." },
            { "role": "user", "content": "Compare" },
            { "role": "assistant", "content": "Reading both.", "tool_calls": [
                { "id": "a", "type": "function", "function": { "name": "read", "arguments": "{\"tab\":1}" } },
                { "id": "b", "type": "function", "function": { "name": "read", "arguments": "{\"tab\":2}" } },
            ] },
            { "role": "tool", "tool_call_id": "a", "content": "one" },
            { "role": "tool", "tool_call_id": "b", "content": "two" },
            { "role": "user", "content": [
                { "type": "image_url", "image_url": { "url": "data:image/png;base64,AAAA" } },
            ] },
        ])
    );
}

#[test]
fn compat_streams_text_thinking_and_calls_whole_at_the_end() {
    let stand = stand(vec![Answer::Send(format!(
        "HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n{}",
        [
            json!({ "choices": [{ "delta": { "reasoning_content": "Hm." } }] }),
            json!({ "choices": [{ "delta": { "content": "Open" } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [
                { "index": 0, "id": "c0", "function": { "name": "open", "arguments": "{\"url\":" } },
            ] } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [
                { "index": 0, "function": { "arguments": "\"a.com\"}" } },
                { "index": 1, "function": { "name": "read", "arguments": "" } },
            ] } }] }),
            json!({ "choices": [{ "delta": {}, "finish_reason": "tool_calls" }] }),
            json!({ "choices": [], "usage": { "prompt_tokens": 12, "completion_tokens": 3 } }),
        ]
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>()
            + "data: [DONE]\n\n"
    ))]);
    let deltas = turn(&Compat, &stand);
    // The server named the second call nothing, so it's given an id of its own.
    let unnamed = match &deltas[4] {
        Delta::Call { id, .. } => id.clone(),
        other => panic!("not a call: {other:?}"),
    };
    assert_eq!(
        deltas,
        [
            Delta::Thinking("Hm.".into()),
            Delta::Text("Open".into()),
            Delta::Usage {
                input: 12,
                output: 3
            },
            Delta::Call {
                id: "c0".into(),
                name: "open".into(),
                args: json!({ "url": "a.com" })
            },
            Delta::Call {
                id: unnamed.clone(),
                name: "read".into(),
                args: json!({})
            },
            Delta::Ended(Stop::Calls),
        ]
    );
    assert!(unnamed.starts_with("call-"), "{unnamed}");
}

#[test]
fn compat_ends_at_the_close_where_a_server_sends_no_done() {
    let stand = stand(vec![events(&[
        json!({ "choices": [{ "delta": { "content": "Hi" } }] }),
        json!({ "choices": [{ "delta": {}, "finish_reason": "stop" }] }),
    ])]);
    assert_eq!(
        turn(&Compat, &stand),
        [Delta::Text("Hi".into()), Delta::Ended(Stop::Done)]
    );
}
