//! The bridge an MCP client launches, `mcp-bridge <socket>`, with the agent's token in its
//! environment, as Connect writes it into the client's config. It hands the token to the browser's
//! server, then carries the client's messages from its stdin to the socket and the browser's
//! answers back to its stdout, as MCP's stdio transport, which every client speaks. It's a program
//! of its own, beside the browser's, so it never starts Chromium.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

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

/// The bridge in a copy from the Microsoft Store: the alias its manifest declares
/// (platform/windows/package/AppxManifest.xml), which Windows keeps in the user's WindowsApps
/// folder under `local_app_data`. The package's own folder is named for its version, so a client
/// given the bridge's path there broke with each Store update (#1442).
pub fn store_alias(local_app_data: &str) -> PathBuf {
    format!(
        r"{local_app_data}\Microsoft\WindowsApps\{}-mcp-bridge.exe",
        env!("APP_NAME")
    )
    .into()
}

/// What a client runs to reach the server through the bridge at `bridge`, before the socket. In
/// a Flatpak, the app with id `flatpak`, the bridge is a path in the sandbox, which a client on the
/// host can't run, so Flatpak runs it there.
pub fn launch(bridge: &Path, flatpak: Option<&str>) -> Vec<String> {
    let bridge = bridge.to_string_lossy().into_owned();
    match flatpak {
        Some(id) => vec![
            "flatpak".into(),
            "run".into(),
            format!("--command={bridge}"),
            id.into(),
        ],
        None => vec![bridge],
    }
}

/// The app's Flatpak id, if it runs in a Flatpak's sandbox, which has /.flatpak-info: the
/// FLATPAK_ID `flatpak run` sets, or the name the file gives.
pub fn flatpak_id() -> Option<String> {
    let info = std::fs::read_to_string("/.flatpak-info").ok()?;
    std::env::var("FLATPAK_ID")
        .ok()
        .or_else(|| flatpak_name(&info))
        .filter(|id| !id.is_empty())
}

/// The app's id in a /.flatpak-info's `info`: its [Application] group's name.
fn flatpak_name(info: &str) -> Option<String> {
    let application = info.split("[Application]").nth(1)?;
    let group = application.split("\n[").next()?;
    group
        .lines()
        .find_map(|line| line.trim().strip_prefix("name="))
        .map(|name| name.trim().into())
}

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

#[cfg(test)]
mod launch_tests {
    use super::*;

    /// Outside a Flatpak a client runs the bridge itself, and in one Flatpak runs it in the app's
    /// sandbox, by the id the sandbox's info gives.
    #[test]
    fn a_client_runs_the_bridge_or_flatpak_runs_it() {
        let bridge = Path::new("/opt/ilo/mcp-bridge");
        assert_eq!(launch(bridge, None), ["/opt/ilo/mcp-bridge"]);
        assert_eq!(
            launch(Path::new("/app/ilo/mcp-bridge"), Some("app.ilo.browser")),
            [
                "flatpak",
                "run",
                "--command=/app/ilo/mcp-bridge",
                "app.ilo.browser"
            ]
        );
        let info = "[Application]\nname=app.ilo.browser\nruntime=runtime/org.freedesktop.Platform/x86_64/25.08\n\n[Instance]\nname=other\n";
        assert_eq!(flatpak_name(info).as_deref(), Some("app.ilo.browser"));
        assert_eq!(flatpak_name("[Instance]\nname=other\n"), None);
    }

    /// A Store copy's clients run the alias the MSIX's manifest declares for the bridge, in the
    /// folder Windows keeps aliases in, not a path in the package's versioned folder.
    #[test]
    fn a_store_copy_names_the_alias_its_manifest_declares() {
        let manifest = include_str!("../../platform/windows/package/AppxManifest.xml");
        let declared = manifest
            .split("<uap5:ExecutionAlias Alias=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the manifest declares an alias for the bridge")
            .replace("@NAME@", env!("APP_NAME"));
        assert!(manifest.contains(r#"Executable="mcp-bridge.exe""#));
        assert_eq!(
            store_alias(r"C:\Users\someone\AppData\Local"),
            PathBuf::from(format!(
                r"C:\Users\someone\AppData\Local\Microsoft\WindowsApps\{declared}"
            ))
        );
    }
}
