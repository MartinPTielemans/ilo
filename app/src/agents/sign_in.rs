//! Signing in to a task CLI and out of it, from Settings › Agents and first run (#4100). ilo starts
//! the CLI's own sign-in, as its maker ships it, and waits for the CLI to say it's done: it never
//! reads, receives or keeps a password, a token or a code. Claude Code's `claude auth login` opens
//! its page in the default browser itself, and gets no input, so it can't be handed a code. Codex's
//! app-server gives the page's address, which the Agents open in a tab and never log, and keeps what
//! the sign-in brings itself. A failure's words show in the row only where `said` keeps them, and
//! no line either CLI writes is kept whole or logged.
//!
//! Each ends on its own: Claude's command as it exits, which a thread waits on, and Codex's as its
//! app-server says the sign-in completed. Nothing here runs on a timer.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc::UnboundedSender;
use gpui::{AppContext, Context};
use serde_json::{Value, json};

use super::Agents;
use super::runner::Cli;
use crate::child::command;

/// What a sign-in under way tells the Agents.
#[derive(Debug, PartialEq)]
pub enum Heard {
    /// Codex's page, for a tab of a private window of its own, which keeps nothing of it. Its
    /// address is the CLI's: never logged, and not kept once opened.
    Page(String),
    Ended(Ended),
}

/// How a sign-in ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Ended {
    /// The CLI says it's done; whether it's signed in is asked again.
    Done,
    /// It ended without, in its words where `said` keeps them.
    Failed(Option<String>),
    /// The user cancelled it.
    Cancelled,
}

/// A sign-in under way. Dropping it ends it, as quitting does.
pub trait Session: Send {
    fn cancel(&mut self);
}

/// Starts `cli`'s own sign-in with `program` on `path`, which tells `heard` what it hears.
pub fn start(
    cli: Cli,
    program: &Path,
    path: Option<&str>,
    heard: UnboundedSender<Heard>,
) -> std::io::Result<Box<dyn Session>> {
    match cli {
        Cli::Claude => Ok(Box::new(Claude::start(program, path, heard)?)),
        Cli::Codex => Ok(Box::new(Codex::start(program, path, heard)?)),
    }
}

/// Signs `cli` out, everywhere it's used, as its own command does. Blocks: runs the CLI.
pub fn sign_out(cli: Cli, program: &Path, path: Option<&str>) -> Result<(), Option<String>> {
    match cli {
        Cli::Claude => {
            let mut logout = cli_command(program, path);
            logout
                .args(["auth", "logout"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            let output = spawn(&mut logout)
                .and_then(|child| child.wait_with_output())
                .map_err(|_| None)?;
            match output.status.success() {
                true => Ok(()),
                false => Err(last_said(&output.stderr[..])),
            }
        }
        Cli::Codex => codex_sign_out(program, path),
    }
}

/// The words of a CLI's line, for the row, or None where they might hold what a sign-in carries:
/// an address, which can hold a code or a sign-in's state, anything token-shaped, or a word mixing
/// letters and digits, as a one-time code is. Never logged, and kept only while the row shows it.
pub fn said(line: &str) -> Option<String> {
    let line: String = line.chars().filter(|c| !c.is_control()).collect();
    let line = line.trim();
    if line.is_empty() || line.contains("://") || line.contains("www.") {
        return None;
    }
    let token_char = |c: char| c.is_ascii_alphanumeric() || "-_.~+/=".contains(c);
    let mut run = 0;
    for c in line.chars() {
        run = if token_char(c) { run + 1 } else { 0 };
        if run >= 24 {
            return None;
        }
    }
    let code_shaped = line.split_whitespace().any(|word| {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        word.len() >= 6
            && word.chars().any(|c| c.is_ascii_digit())
            && word.chars().any(|c| c.is_ascii_alphabetic())
    });
    if code_shaped {
        return None;
    }
    const MOST: usize = 160;
    Some(match line.char_indices().nth(MOST) {
        Some((cut, _)) => format!("{}…", line[..cut].trim_end()),
        None => line.to_string(),
    })
}

/// The last line of `output` that `said` keeps.
fn last_said(output: impl Read) -> Option<String> {
    let lines = BufReader::new(output).lines().map_while(Result::ok);
    lines
        .filter(|line| !line.trim().is_empty())
        .last()
        .and_then(|line| said(&line))
}

/// The tabs a sign-in's page is in, whose addresses no log line names (`quiet`).
static QUIET: Mutex<Option<HashSet<u64>>> = Mutex::new(None);

/// Whether `tab` shows a sign-in's page: what it loads isn't logged, as a sign-in's addresses can
/// hold its code or a token.
pub fn quiet(tab: u64) -> bool {
    (QUIET.lock().unwrap().as_ref()).is_some_and(|tabs| tabs.contains(&tab))
}

pub(super) fn set_quiet(tab: u64, on: bool) {
    let mut tabs = QUIET.lock().unwrap();
    let tabs = tabs.get_or_insert_default();
    match on {
        true => tabs.insert(tab),
        false => tabs.remove(&tab),
    };
}

/// `program`, on `path` if given, as the look runs it.
fn cli_command(program: &Path, path: Option<&str>) -> Command {
    let mut cli = command(program);
    if let Some(path) = path {
        cli.env("PATH", path);
    }
    cli
}

/// Starts `cli`. In the tests, only a stand-in in the test's own folder: no test may start this
/// computer's Claude Code or Codex, whose sign-in the computer's own agents run on.
fn spawn(cli: &mut Command) -> std::io::Result<Child> {
    #[cfg(test)]
    {
        let program = Path::new(cli.get_program());
        assert!(
            program.starts_with(std::env::temp_dir()),
            "a test may start only a stand-in in its folder, not {}",
            program.display()
        );
        // A stand-in written a moment ago can still be open in a child another test forked and
        // hasn't yet exec'd, and Linux won't run a file open for writing (#4180).
        for _ in 0..100 {
            match cli.spawn() {
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                spawned => return spawned,
            }
        }
    }
    cli.spawn()
}

/// Claude Code's sign-in: `claude auth login`, its input closed, its output unread, and its errors'
/// last line kept, through `said`, for the row if it fails.
struct Claude {
    pid: u32,
    /// Set as the command ends, after which its id may be another program's.
    ended: Arc<Mutex<bool>>,
    cancelled: Arc<AtomicBool>,
    end: Report,
}

/// Tells the Agents how a sign-in ended, once: the first to know does.
#[derive(Clone)]
struct Report {
    heard: UnboundedSender<Heard>,
    told: Arc<AtomicBool>,
}

impl Report {
    fn new(heard: UnboundedSender<Heard>) -> Report {
        let told = Arc::new(AtomicBool::new(false));
        Report { heard, told }
    }

    fn ended(&self, end: Ended) {
        if !self.told.swap(true, Ordering::SeqCst) {
            _ = self.heard.unbounded_send(Heard::Ended(end));
        }
    }
}

impl Claude {
    fn start(
        program: &Path,
        path: Option<&str>,
        heard: UnboundedSender<Heard>,
    ) -> std::io::Result<Claude> {
        let mut login = cli_command(program, path);
        login
            .args(["auth", "login"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = spawn(&mut login)?;
        let pid = child.id();
        let errors = child.stderr.take().expect("piped");
        // The last kept line, read as it comes, and the end of its errors, which a program the
        // command started, such as a browser, may hold open past the command's own end.
        let last = Arc::new(Mutex::new(None));
        let (read, all_read) = std::sync::mpsc::channel::<()>();
        std::thread::spawn({
            let last = last.clone();
            move || {
                let _read = read;
                for line in BufReader::new(errors).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        *last.lock().unwrap() = said(&line);
                    }
                }
            }
        });
        let ended = Arc::new(Mutex::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let end = Report::new(heard);
        std::thread::spawn({
            let (ended, cancelled, end) = (ended.clone(), cancelled.clone(), end.clone());
            move || {
                let status = child.wait();
                *ended.lock().unwrap() = true;
                match status {
                    _ if cancelled.load(Ordering::SeqCst) => end.ended(Ended::Cancelled),
                    Ok(status) if status.success() => end.ended(Ended::Done),
                    _ => {
                        // Its words come once its errors are all read. Cancel ends the wait.
                        _ = all_read.recv();
                        end.ended(Ended::Failed(last.lock().unwrap().take()));
                    }
                }
            }
        });
        Ok(Claude {
            pid,
            ended,
            cancelled,
            end,
        })
    }

    fn end(&self) {
        let ended = self.ended.lock().unwrap();
        match *ended {
            false => {
                self.cancelled.store(true, Ordering::SeqCst);
                end_process(self.pid);
            }
            // It failed, and its words are still coming.
            true => self.end.ended(Ended::Cancelled),
        }
    }
}

impl Session for Claude {
    fn cancel(&mut self) {
        self.end();
    }
}

impl Drop for Claude {
    fn drop(&mut self) {
        self.end();
    }
}

/// Ends the program `pid`, which hasn't been waited for: on Windows with the programs an npm shim
/// started for it, but not theirs, as the browser the sign-in opened may be one.
fn end_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(windows)]
    windows_end(pid);
}

#[cfg(windows)]
fn windows_end(pid: u32) {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    let terminate = |pid: u32| unsafe {
        let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !process.is_null() {
            TerminateProcess(process, 1);
            CloseHandle(process);
        }
    };
    let mut children = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot != INVALID_HANDLE_VALUE {
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                if entry.th32ParentProcessID == pid {
                    children.push(entry.th32ProcessID);
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
    }
    terminate(pid);
    children.into_iter().for_each(terminate);
}

/// The ids of Codex's requests.
const INITIALIZE: u64 = 1;
const LOGIN: u64 = 2;
const CANCEL: u64 = 3;
const LOGOUT: u64 = 4;

/// Codex's input, which closing ends it.
type Input = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

/// Codex's sign-in, on an app-server of its own: initialize, then `account/login/start`, whose
/// answer has the page's address, then `account/login/completed`. Closing its input ends the
/// server, and with it the sign-in and the listener its page comes back to.
struct Codex {
    input: Input,
    login: Arc<Mutex<Option<String>>>,
    cancelled: Arc<AtomicBool>,
    /// The server's id, while it may still run.
    pid: Option<u32>,
    ended: Arc<Mutex<bool>>,
}

/// Writes `message` to Codex, unless its input has closed.
fn say(input: &Mutex<Option<Box<dyn Write + Send>>>, message: &Value) -> bool {
    let mut input = input.lock().unwrap();
    let Some(input) = input.as_mut() else {
        return false;
    };
    writeln!(input, "{message}")
        .and_then(|_| input.flush())
        .is_ok()
}

fn initialize() -> Value {
    json!({
        "id": INITIALIZE,
        "method": "initialize",
        "params": {
            "clientInfo": { "name": env!("APP_NAME").to_lowercase(), "title": env!("APP_NAME"), "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": true },
        },
    })
}

/// Codex's app-server with its input and output piped and its errors dropped.
fn app_server(program: &Path, path: Option<&str>) -> std::io::Result<Child> {
    let mut server = cli_command(program, path);
    server
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    spawn(&mut server)
}

/// What one line from Codex says to a sign-in: the line is read for these and dropped.
#[derive(Debug, PartialEq)]
enum Line {
    Initialized,
    /// login/start's answer: its id and the page.
    Started {
        login: String,
        page: String,
    },
    /// login/start refused, in its words.
    Refused(Option<String>),
    Completed {
        login: Option<String>,
        ok: bool,
        said: Option<String>,
    },
    Other,
}

fn read_line(line: &str) -> Line {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Line::Other;
    };
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(String::from);
    let id = message.get("id").and_then(Value::as_u64);
    match (id, message.get("method").and_then(Value::as_str)) {
        (Some(INITIALIZE), None) => Line::Initialized,
        (Some(LOGIN), None) => match (
            text(message.pointer("/result/loginId")),
            text(message.pointer("/result/authUrl")),
        ) {
            (Some(login), Some(page)) => Line::Started { login, page },
            _ => Line::Refused(text(message.pointer("/error/message")).and_then(|m| said(&m))),
        },
        (None, Some("account/login/completed")) => Line::Completed {
            login: text(message.pointer("/params/loginId")),
            ok: message.pointer("/params/success").and_then(Value::as_bool) == Some(true),
            said: text(message.pointer("/params/error")).and_then(|error| said(&error)),
        },
        _ => Line::Other,
    }
}

impl Codex {
    fn start(
        program: &Path,
        path: Option<&str>,
        heard: UnboundedSender<Heard>,
    ) -> std::io::Result<Codex> {
        let mut child = app_server(program, path)?;
        let output = child.stdout.take().expect("piped");
        let input: Box<dyn Write + Send> = Box::new(child.stdin.take().expect("piped"));
        Ok(Codex::over(output, input, Some(child), heard))
    }

    /// The sign-in on the server whose output is `output` and input `input`: `child`, or in the
    /// tests, a stand-in on this computer's every system.
    fn over(
        output: impl Read + Send + 'static,
        input: Box<dyn Write + Send>,
        child: Option<Child>,
        heard: UnboundedSender<Heard>,
    ) -> Codex {
        let pid = child.as_ref().map(Child::id);
        let input: Input = Arc::new(Mutex::new(Some(input)));
        let login = Arc::new(Mutex::new(None::<String>));
        let cancelled = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(Mutex::new(false));
        say(&input, &initialize());
        std::thread::spawn({
            let (input, login, cancelled, ended) = (
                input.clone(),
                login.clone(),
                cancelled.clone(),
                ended.clone(),
            );
            move || {
                let mut end = None;
                for line in BufReader::new(output).lines().map_while(Result::ok) {
                    match read_line(&line) {
                        Line::Initialized => {
                            say(&input, &json!({ "method": "initialized" }));
                            say(&input, &login_start());
                        }
                        Line::Started { login: id, page } => {
                            *login.lock().unwrap() = Some(id);
                            _ = heard.unbounded_send(Heard::Page(page));
                        }
                        Line::Refused(said) => {
                            end = Some(Ended::Failed(said));
                            break;
                        }
                        Line::Completed {
                            login: id,
                            ok,
                            said,
                        } => {
                            if id.is_some() && id != *login.lock().unwrap() {
                                continue;
                            }
                            end = Some(match ok {
                                true => Ended::Done,
                                false => Ended::Failed(said),
                            });
                            break;
                        }
                        Line::Other => {}
                    }
                }
                // Its input closing ends it, and its listener with it.
                input.lock().unwrap().take();
                if let Some(mut child) = child {
                    _ = child.wait();
                }
                *ended.lock().unwrap() = true;
                let end = match end {
                    _ if cancelled.load(Ordering::SeqCst) => Ended::Cancelled,
                    Some(end) => end,
                    None => Ended::Failed(None),
                };
                _ = heard.unbounded_send(Heard::Ended(end));
            }
        });
        Codex {
            input,
            login,
            cancelled,
            pid,
            ended,
        }
    }
}

impl Session for Codex {
    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(login) = self.login.lock().unwrap().clone() {
            say(
                &self.input,
                &json!({ "id": CANCEL, "method": "account/login/cancel", "params": { "loginId": login } }),
            );
        }
        self.input.lock().unwrap().take();
    }
}

impl Drop for Codex {
    fn drop(&mut self) {
        self.cancel();
        // A server that doesn't end as its input closes ends with ilo all the same.
        let ended = self.ended.lock().unwrap();
        if let Some(pid) = self.pid
            && !*ended
        {
            end_process(pid);
        }
    }
}

/// Codex's `account/login/start`, asking it to end on OpenAI's page, whose address holds no token,
/// where its own would hold the account's identity token. Codex since 0.144.0 takes it; an older
/// one leaves it out, and its page, in a private window, is kept nowhere either way (#4100).
fn login_start() -> Value {
    json!({
        "id": LOGIN,
        "method": "account/login/start",
        "params": { "type": "chatgpt", "useHostedLoginSuccessPage": true, "appBrand": "codex" },
    })
}

/// Codex's `account/logout`, on an app-server of its own, which ends once it answers.
fn codex_sign_out(program: &Path, path: Option<&str>) -> Result<(), Option<String>> {
    let mut child = app_server(program, path).map_err(|_| None)?;
    let output = child.stdout.take().expect("piped");
    let input: Box<dyn Write + Send> = Box::new(child.stdin.take().expect("piped"));
    let answer = log_out_over(output, input);
    _ = child.wait();
    answer
}

/// `account/logout` on the server whose output is `output` and input `input`, whose input closes
/// once it answers.
fn log_out_over(output: impl Read, input: Box<dyn Write + Send>) -> Result<(), Option<String>> {
    let input = Mutex::new(Some(input));
    say(&input, &initialize());
    let mut answer = Err(None);
    for line in BufReader::new(output).lines().map_while(Result::ok) {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message.get("id").and_then(Value::as_u64) {
            Some(INITIALIZE) => {
                say(&input, &json!({ "method": "initialized" }));
                say(&input, &json!({ "id": LOGOUT, "method": "account/logout" }));
            }
            Some(LOGOUT) => {
                answer = match message.get("error") {
                    None => Ok(()),
                    Some(error) => {
                        Err((error.get("message").and_then(Value::as_str)).and_then(said))
                    }
                };
                break;
            }
            _ => {}
        }
    }
    input.lock().unwrap().take();
    answer
}

/// What an assistant's row says of its sign-in, beside what was last found of it (`Agents::found`).
/// Held for the window only: never saved, sent or logged.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Row {
    #[default]
    Idle,
    SigningIn,
    SigningOut,
    /// The last sign-in or sign-out ended without, in the CLI's words where `said` kept them.
    Failed(Option<String>),
}

/// A sign-in under way.
struct Running {
    session: Box<dyn Session>,
    /// Codex's page's tab and window, once open.
    page: Option<(u64, u64)>,
    _heard: gpui::Task<()>,
}

/// The sign-ins under way and what each row says, by CLI.
#[derive(Default)]
pub struct SignIns {
    running: HashMap<Cli, Running>,
    rows: HashMap<Cli, Row>,
}

impl Agents {
    /// What `cli`'s row says of its sign-in.
    pub fn sign_in_row(&self, cli: Cli) -> Row {
        let rows = &self.tasks.sign_ins.rows;
        rows.get(&cli).cloned().unwrap_or_default()
    }

    /// Starts `cli`'s own sign-in, from its row: Claude Code's opens its page in the default
    /// browser itself, and Codex's in a private window here. One at a time for each.
    pub fn sign_in(&mut self, cli: Cli, cx: &mut Context<Self>) {
        let sign_ins = &self.tasks.sign_ins;
        if sign_ins.running.contains_key(&cli) {
            return;
        }
        let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
        let (heard, mut hearing) = futures::channel::mpsc::unbounded();
        let row = match self.clients.start_sign_in(cli, &found, heard) {
            Ok(session) => {
                let listen = cx.spawn(async move |agents, cx| {
                    while let Some(heard) = hearing.next().await {
                        let hear = |agents: &mut Agents, cx: &mut Context<Agents>| {
                            agents.sign_in_heard(cli, heard, cx)
                        };
                        if agents.update(cx, hear).is_err() {
                            break;
                        }
                    }
                });
                let running = Running {
                    session,
                    page: None,
                    _heard: listen,
                };
                self.tasks.sign_ins.running.insert(cli, running);
                log!("browser: task agent {} sign-in started", cli.id());
                Row::SigningIn
            }
            Err(_) => {
                log!("browser: task agent {} sign-in couldn't start", cli.id());
                Row::Failed(None)
            }
        };
        self.tasks.sign_ins.rows.insert(cli, row);
        cx.notify();
    }

    /// Ends `cli`'s sign-in under way, as its row's Cancel does. Its end comes as any other.
    pub fn cancel_sign_in(&mut self, cli: Cli) {
        if let Some(running) = self.tasks.sign_ins.running.get_mut(&cli) {
            running.session.cancel();
        }
    }

    /// Signs `cli` out, everywhere it's used, as its row's Sign Out does once the user confirms.
    pub fn sign_out(&mut self, cli: Cli, cx: &mut Context<Self>) {
        if self.sign_in_row(cli) == Row::SigningOut
            || self.tasks.sign_ins.running.contains_key(&cli)
        {
            return;
        }
        let found = self.tasks.found.get(&cli).cloned().unwrap_or_default();
        let clients = self.clients.clone();
        let out = cx.background_spawn(async move { clients.sign_out(cli, &found) });
        self.tasks.sign_ins.rows.insert(cli, Row::SigningOut);
        cx.notify();
        cx.spawn(async move |agents, cx| {
            let out = out.await;
            _ = agents.update(cx, |agents, cx| {
                let row = match out {
                    Ok(()) => Row::Idle,
                    Err(said) => Row::Failed(said),
                };
                log!(
                    "browser: task agent {} sign-out {}",
                    cli.id(),
                    if row == Row::Idle { "ended" } else { "failed" },
                );
                agents.tasks.sign_ins.rows.insert(cli, row);
                agents.look_for_task_agents(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn sign_in_heard(&mut self, cli: Cli, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Page(url) => {
                let Some(running) = self.tasks.sign_ins.running.get_mut(&cli) else {
                    return;
                };
                if running.page.is_some() {
                    return;
                }
                running.page = self.browser.open_sign_in(&url, cx);
                match running.page {
                    Some((tab, _)) => set_quiet(tab, true),
                    // With nowhere to finish it, it ends.
                    None => running.session.cancel(),
                }
            }
            Heard::Ended(end) => {
                let Some(running) = self.tasks.sign_ins.running.remove(&cli) else {
                    return;
                };
                if let Some((tab, window)) = running.page {
                    set_quiet(tab, false);
                    self.browser.close_sign_in(tab, window, cx);
                }
                let (row, ended) = match end {
                    Ended::Done => (Row::Idle, "ended"),
                    Ended::Cancelled => (Row::Idle, "cancelled"),
                    Ended::Failed(said) => (Row::Failed(said), "failed"),
                };
                log!("browser: task agent {} sign-in {ended}", cli.id());
                self.tasks.sign_ins.rows.insert(cli, row);
                self.look_for_task_agents(cx);
                cx.notify();
            }
        }
    }

    /// `tab` closed: a sign-in whose page it showed can't finish there, so it ends.
    pub(super) fn sign_in_tab_closed(&mut self, tab: u64, _: &mut Context<Self>) {
        let running = self.tasks.sign_ins.running.values_mut();
        for running in running.filter(|running| running.page.is_some_and(|(at, _)| at == tab)) {
            // Closed already.
            running.page = None;
            set_quiet(tab, false);
            running.session.cancel();
        }
    }
}

/// What an assistant's row offers, at its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offer {
    /// Nothing, while it's looked for or signing out.
    Nothing,
    /// Get Claude Code or Get Codex: its maker's install page.
    Get,
    /// Sign In to Claude Code or Sign In to Codex.
    SignIn,
    /// Cancel, while it signs in.
    Cancel,
    /// Try Again, once a sign-in failed.
    TryAgain,
    /// Its ⋯ menu, with Model and Sign Out, and the mark that has tasks run on it.
    SignedIn,
}

impl Offer {
    /// Its one button's words, on the assistant's row and a task's card; None where it has none.
    pub fn label(self, cli: Cli) -> Option<String> {
        let name = cli.assistant();
        match self {
            Offer::Get => Some(format!("Get {name}")),
            Offer::SignIn => Some(format!("Sign In to {name}")),
            Offer::TryAgain => Some("Try Again".into()),
            Offer::Cancel => Some("Cancel".into()),
            Offer::Nothing | Offer::SignedIn => None,
        }
    }
}

/// An assistant's row, the same in Settings › Agents and first run: its line and what it offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub line: String,
    pub offer: Offer,
}

/// What Claude Code's line says to run where its sign-in needs a terminal.
pub const CLAUDE_SIGN_IN: &str = "claude auth login";

impl Shown {
    /// Where in its line a command to type is, set in the face a terminal shows it in.
    pub fn command(&self) -> Option<std::ops::Range<usize>> {
        let at = self.line.find(CLAUDE_SIGN_IN)?;
        Some(at..at + CLAUDE_SIGN_IN.len())
    }
}

impl Cli {
    /// Its maker's name for it, as its row says it, in text only.
    pub fn assistant(self) -> &'static str {
        match self {
            Cli::Claude => "Claude Code",
            Cli::Codex => "Codex",
        }
    }

    /// Its maker's page that says how to install it, which Get opens.
    pub fn install_page(self) -> &'static str {
        match self {
            Cli::Claude => "https://code.claude.com/docs/en/setup",
            Cli::Codex => "https://developers.openai.com/codex/cli",
        }
    }
}

/// What `cli`'s row shows, found `ready` with `plan` and its sign-in at `row`.
pub fn shown(cli: Cli, ready: super::ready::Ready, plan: Option<&str>, row: &Row) -> Shown {
    use super::ready::Ready;
    let shown = |line: &str, offer| Shown {
        line: line.to_string(),
        offer,
    };
    match (row, ready) {
        (Row::SigningIn, _) => shown("Finish in the page that opened", Offer::Cancel),
        (Row::SigningOut, _) => shown("Signing out…", Offer::Nothing),
        // Its Sign Out failed: still signed in, and the ⋯ menu has Sign Out again.
        (Row::Failed(said), Ready::SignedIn) => Shown {
            line: said.clone().unwrap_or_else(|| "Couldn't sign out".into()),
            offer: Offer::SignedIn,
        },
        (Row::Failed(Some(said)), _) => shown(said, Offer::TryAgain),
        // Claude Code's command ends this way when it asks for a code in a terminal, which ilo
        // never gives it.
        (Row::Failed(None), _) if cli == Cli::Claude => shown(
            &format!("Run {CLAUDE_SIGN_IN} in a terminal"),
            Offer::TryAgain,
        ),
        (Row::Failed(None), _) => shown("The sign-in didn't finish", Offer::TryAgain),
        (Row::Idle, Ready::Unknown) => shown("Checking…", Offer::Nothing),
        (Row::Idle, Ready::NotFound) => shown("Not installed", Offer::Get),
        (Row::Idle, Ready::SignedOut) => shown("Not signed in", Offer::SignIn),
        (Row::Idle, Ready::SignedIn) => match plan {
            Some(plan) => shown(&format!("Signed in · {plan} plan"), Offer::SignedIn),
            None => shown("Signed in", Offer::SignedIn),
        },
    }
}

impl Agents {
    /// What `cli`'s row shows now.
    pub fn assistant_row(&self, cli: Cli) -> Shown {
        let found = self.tasks.found.get(&cli);
        let plan = found.and_then(|found| found.plan.as_deref());
        shown(cli, self.found(cli), plan, &self.sign_in_row(cli))
    }

    /// Settings › Models' assistants: every one the app can run, in its state, whether agents
    /// and tasks are on or not, so the page is never only its title; each with whether its model
    /// for a new thread is drawn under it, which is once it's signed in.
    pub fn providers(&self) -> Vec<(Cli, Shown, bool)> {
        Cli::ALL
            .into_iter()
            .map(|cli| {
                let row = self.assistant_row(cli);
                let signed_in = row.offer == Offer::SignedIn;
                (cli, row, signed_in)
            })
            .collect()
    }

    /// What the card of a task that failed for want of `cli`'s sign-in or install offers: its
    /// row's own button, so the two never differ. Try Again reads as Sign in there, as on a card it
    /// would read as trying the task again.
    pub(super) fn card_offer(&self, cli: Cli) -> Option<Offer> {
        match self.assistant_row(cli).offer {
            Offer::TryAgain => Some(Offer::SignIn),
            offer @ (Offer::Get | Offer::SignIn | Offer::Cancel) => Some(offer),
            Offer::Nothing | Offer::SignedIn => None,
        }
    }

    /// The Sign in to … on the card of `agent`'s task, which starts its assistant's own sign-in,
    /// as the row's does. The user's alone: only a card's press sends it.
    pub(super) fn sign_in_for_task(&mut self, agent: &str, cx: &mut Context<Self>) {
        if let Some(cli) = self.task(agent).and_then(|task| task.cli) {
            log!("browser: assistant sign-in from task {agent}");
            self.sign_in(cli, cx);
        }
    }

    /// The Cancel on the card of `agent`'s task while its assistant signs in, as the row's.
    pub(super) fn cancel_sign_in_for_task(&mut self, agent: &str) {
        if let Some(cli) = self.task(agent).and_then(|task| task.cli) {
            self.cancel_sign_in(cli);
        }
    }
}

#[cfg(test)]
mod tests;
