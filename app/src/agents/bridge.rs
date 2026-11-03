//! The bridge an MCP client launches, `mcp-bridge <socket>`, with the agent's token in its
//! environment, as Connect writes it into the client's config. It hands the token to the browser's
//! server, then carries the client's messages from its stdin to the socket and the browser's
//! answers back to its stdout, as MCP's stdio transport, which every client speaks. It's a program
//! of its own, beside the browser's, so it never starts Chromium.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

use serde_json::{Value, json};

/// The variable the client's config gives the bridge the agent's token in. An environment
/// variable, not an argument, so it's never in the list of processes.
pub const TOKEN: &str = "BROWSER_AGENT_TOKEN";

/// The bridge's name beside the browser's executable.
pub const NAME: &str = if cfg!(windows) {
    "mcp-bridge.exe"
} else {
    "mcp-bridge"
};

/// Connects to the server at `socket`, a Unix socket or a Windows named pipe, as the agent with
/// `token`, then carries `input` to it and its answers to `output` until either side closes. An
/// error is why it couldn't, for the client's log.
pub fn run(
    socket: &Path,
    token: &str,
    input: impl Read + Send + 'static,
    mut output: impl Write,
) -> Result<(), String> {
    use super::server;
    let stream = server::connect(socket).map_err(|_| {
        format!(
            "{} isn't running with agents on. Turn them on in Settings › Agents.",
            env!("APP_NAME")
        )
    })?;
    let failed = |error: io::Error| format!("The browser closed the connection: {error}");
    let mut writer = stream.try_clone().map_err(failed)?;
    writeln!(writer, "{}", json!({ "token": token })).map_err(failed)?;
    let mut reader = BufReader::new(stream);
    let answer = super::server::read_line(&mut reader).unwrap_or_default();
    let answer: Value = serde_json::from_str(&answer).unwrap_or_default();
    if let Some(refused) = answer.get("error").and_then(Value::as_str) {
        return Err(refused.into());
    }
    if answer.get("ok").is_none() {
        return Err("The browser didn't answer.".into());
    }
    // The client's messages, on a thread of their own, until it closes its end.
    std::thread::spawn(move || {
        _ = io::copy(&mut BufReader::new(input), &mut writer);
        // For the client's log, which has the bridge's stderr: the session ends from here.
        eprintln!("{NAME}: the client closed its end");
        _ = server::close_write(&mut writer);
    });
    // The browser's answers, a line at a time, as the client reads them.
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => return Ok(()),
            Ok(_) => output
                .write_all(&line)
                .and_then(|()| output.flush())
                .map_err(failed)?,
            Err(error) => return Err(failed(error)),
        }
    }
}
