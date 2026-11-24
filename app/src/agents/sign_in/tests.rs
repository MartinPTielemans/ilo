//! Signing in and out, against stand-ins only: no test here starts this computer's Claude Code or
//! Codex (`spawn` refuses a program outside the test's folder). Every value is made up.

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use serde_json::{Value, json};

use super::*;

/// What the stand-in Codex was asked, method and params, in order.
type Asked = mpsc::Receiver<(String, Value)>;

/// A Codex app-server on this computer's every system: a thread on two pipes, which answers
/// initialize as `version`, login/start as `start` answers its params, and then says each of
/// `after`'s lines. It ends as its input closes.
fn stand_in(
    version: &str,
    start: impl Fn(&Value) -> Value + Send + 'static,
    after: Vec<Value>,
) -> (Codex, Asked, UnboundedReceiver<Heard>) {
    let (to_codex_read, to_codex) = std::io::pipe().unwrap();
    let (from_codex, from_codex_write) = std::io::pipe().unwrap();
    let (asked_send, asked) = mpsc::channel();
    let agent = format!("codex_cli_rs/{version} (Made Up OS; x86_64) made-up-terminal");
    std::thread::spawn(move || {
        let mut out = from_codex_write;
        let mut answer = |message: Value| {
            _ = writeln!(out, "{message}");
        };
        for line in BufReader::new(to_codex_read).lines().map_while(Result::ok) {
            let message: Value = serde_json::from_str(&line).unwrap();
            let method = message["method"].as_str().unwrap_or_default().to_string();
            _ = asked_send.send((method.clone(), message["params"].clone()));
            match method.as_str() {
                "initialize" => {
                    answer(json!({ "id": message["id"], "result": { "userAgent": agent } }))
                }
                "account/login/start" => {
                    answer(json!({ "id": message["id"], "result": start(&message["params"]) }));
                    after.iter().cloned().for_each(&mut answer);
                }
                "account/login/cancel" => {
                    answer(json!({ "id": message["id"], "result": { "status": "canceled" } }))
                }
                _ => {}
            }
        }
    });
    let (heard_send, heard) = unbounded();
    let codex = Codex::over(from_codex, Box::new(to_codex), None, heard_send);
    (codex, asked, heard)
}

/// A made-up page, with the state a real one carries.
const PAGE: &str = "https://auth.example.test/oauth/authorize?state=made-up-state-0001";

fn started(_: &Value) -> Value {
    json!({ "type": "chatgpt", "loginId": "login-1", "authUrl": PAGE })
}

fn completed(login: &str, success: bool, error: Option<&str>) -> Value {
    json!({
        "method": "account/login/completed",
        "params": { "loginId": login, "success": success, "error": error },
    })
}

fn next(heard: &mut UnboundedReceiver<Heard>) -> Option<Heard> {
    futures::executor::block_on(heard.next())
}

/// What the stand-in was asked for `method`, the first time.
fn asked_for(asked: &Asked, method: &str) -> Value {
    loop {
        let (each, params) = asked.recv().expect("the stand-in was asked for it");
        if each == method {
            return params;
        }
    }
}

#[test]
fn codex_signs_in_on_its_page_asked_to_end_on_openais() {
    let done = completed("login-1", true, None);
    let (_codex, asked, mut heard) = stand_in("0.150.1", started, vec![done]);
    let params = asked_for(&asked, "account/login/start");
    assert_eq!(params["type"], "chatgpt");
    assert_eq!(params["useHostedLoginSuccessPage"], true);
    assert_eq!(params["appBrand"], "codex");
    assert_eq!(next(&mut heard), Some(Heard::Page(PAGE.into())));
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Done)));
}

#[test]
fn an_older_codex_signs_in_the_same_way() {
    let done = completed("login-1", true, None);
    let (_codex, _, mut heard) = stand_in("0.143.9", started, vec![done]);
    assert_eq!(next(&mut heard), Some(Heard::Page(PAGE.into())));
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Done)));
}

#[test]
fn another_sign_ins_end_is_not_this_ones() {
    let other = completed(
        "login-0",
        false,
        Some("Sign-in was replaced by a newer one"),
    );
    let done = completed("login-1", true, None);
    let (_codex, _, mut heard) = stand_in("0.150.1", started, vec![other, done]);
    assert!(matches!(next(&mut heard), Some(Heard::Page(_))));
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Done)));
}

#[test]
fn a_failed_sign_in_says_codexs_words_unless_they_might_hold_what_it_carries() {
    let failed = completed("login-1", false, Some("Your workspace doesn't allow Codex"));
    let (_codex, _, mut heard) = stand_in("0.150.1", started, vec![failed]);
    assert!(matches!(next(&mut heard), Some(Heard::Page(_))));
    let said = Some("Your workspace doesn't allow Codex".to_string());
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Failed(said))));

    let carrying = "Token exchange failed for code made-up-0f3c9a2b7d1e4f60a8b5c3d2e1f0a9b8";
    let failed = completed("login-1", false, Some(carrying));
    let (_codex, _, mut heard) = stand_in("0.150.1", started, vec![failed]);
    assert!(matches!(next(&mut heard), Some(Heard::Page(_))));
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Failed(None))));
}

#[test]
fn a_refused_start_ends_the_sign_in() {
    let refuse = |_: &Value| Value::Null;
    let (_codex, _, mut heard) = stand_in("0.150.1", refuse, vec![]);
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Failed(None))));
}

#[test]
fn cancel_asks_codex_to_cancel_then_closes_it() {
    let (mut codex, asked, mut heard) = stand_in("0.150.1", started, vec![]);
    assert!(matches!(next(&mut heard), Some(Heard::Page(_))));
    codex.cancel();
    let params = asked_for(&asked, "account/login/cancel");
    assert_eq!(params, json!({ "loginId": "login-1" }));
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Cancelled)));
}

#[test]
fn a_sign_in_ends_with_its_session() {
    let (codex, asked, mut heard) = stand_in("0.150.1", started, vec![]);
    assert!(matches!(next(&mut heard), Some(Heard::Page(_))));
    drop(codex);
    asked_for(&asked, "account/login/cancel");
    assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Cancelled)));
}

#[test]
fn sign_out_asks_codex_to_log_out() {
    let answers = |error: Option<&str>| {
        let (to_codex_read, to_codex) = std::io::pipe().unwrap();
        let (from_codex, mut from_codex_write) = std::io::pipe().unwrap();
        let error = error.map(String::from);
        let server = std::thread::spawn(move || {
            let mut methods = Vec::new();
            for line in BufReader::new(to_codex_read).lines().map_while(Result::ok) {
                let message: Value = serde_json::from_str(&line).unwrap();
                let method = message["method"].as_str().unwrap_or_default().to_string();
                let answer = match (method.as_str(), &error) {
                    ("initialize", _) => json!({ "id": message["id"], "result": {} }),
                    ("account/logout", None) => json!({ "id": message["id"], "result": {} }),
                    ("account/logout", Some(error)) => {
                        json!({ "id": message["id"], "error": { "code": -32603, "message": error } })
                    }
                    _ => Value::Null,
                };
                methods.push(method);
                if !answer.is_null() {
                    _ = writeln!(from_codex_write, "{answer}");
                }
            }
            methods
        });
        let answer = log_out_over(from_codex, Box::new(to_codex));
        (answer, server.join().unwrap())
    };
    let (answer, methods) = answers(None);
    assert_eq!(answer, Ok(()));
    assert_eq!(methods, ["initialize", "initialized", "account/logout"]);
    let (answer, _) = answers(Some("Couldn't remove the saved sign-in"));
    assert_eq!(
        answer,
        Err(Some("Couldn't remove the saved sign-in".into()))
    );
}

#[test]
fn said_keeps_plain_words_and_drops_what_a_sign_in_carries() {
    assert_eq!(
        said("  Error: couldn't reach the sign-in service\r"),
        Some("Error: couldn't reach the sign-in service".into())
    );
    // An address can carry a code or a sign-in's state.
    assert_eq!(
        said("Open https://auth.example.test/cb?code=made-up to finish"),
        None
    );
    assert_eq!(said("Go to www.example.test/device"), None);
    // A run of token characters, as an access token or a JWT.
    assert_eq!(
        said("Saved eyJhbGciOiJub25lIn0.eyJzdWIiOiJtYWRlLXVwIn0.c2ln"),
        None
    );
    // A word mixing letters and digits, as a one-time code is.
    assert_eq!(said("Paste this code where asked: K7Q2-9XZ4"), None);
    assert_eq!(said("Enter ABC123 to continue"), None);
    // Short ones, and plain words with numbers, stay.
    assert_eq!(
        said("Exited with status 1"),
        Some("Exited with status 1".into())
    );
    assert_eq!(said("   "), None);
    let long = "the sign-in didn't finish ".repeat(10);
    let kept = said(&long).unwrap();
    assert!(kept.ends_with('…') && kept.chars().count() <= 161);
}

#[test]
fn the_look_reads_codexs_plan_but_never_its_email() {
    use crate::agents::ready::{Ready, codex_account};
    let answer = json!({ "id": 2, "result": {
        "account": { "type": "chatgpt", "email": "someone@example.test", "planType": "plus", "workspaceName": null },
        "requiresOpenaiAuth": true,
    }});
    let read = codex_account(&answer);
    assert_eq!(read, (Ready::SignedIn, Some("Plus".into())));
    assert!(!format!("{read:?}").contains("example.test"));
    // An expired sign-in is out, though `codex login status` said in (#4099).
    let out = json!({ "id": 2, "result": { "account": null, "requiresOpenaiAuth": true } });
    assert_eq!(codex_account(&out), (Ready::SignedOut, None));
    // A provider that needs no OpenAI sign-in runs tasks without one.
    let other = json!({ "id": 2, "result": { "account": null, "requiresOpenaiAuth": false } });
    assert_eq!(codex_account(&other), (Ready::SignedIn, None));
    let key = json!({ "id": 2, "result": { "account": { "type": "apiKey" }, "requiresOpenaiAuth": true } });
    assert_eq!(codex_account(&key), (Ready::SignedIn, None));
    let refused = json!({ "id": 2, "error": { "code": -32601, "message": "unknown method" } });
    assert_eq!(codex_account(&refused), (Ready::SignedOut, None));
}

#[test]
#[should_panic(expected = "a test may start only a stand-in")]
fn a_test_cant_start_a_program_outside_its_folder() {
    let (heard, _) = unbounded();
    let program = std::env::current_dir().unwrap().join("claude");
    _ = start(Cli::Claude, &program, None, heard);
}

#[test]
fn a_tab_is_quiet_while_it_shows_a_sign_in() {
    set_quiet(77_001, true);
    assert!(quiet(77_001) && !quiet(77_002));
    set_quiet(77_001, false);
    assert!(!quiet(77_001));
}

/// Claude Code's sign-in, as a stand-in script in the test's own folder says it ends.
#[cfg(unix)]
mod claude {
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use super::*;

    /// A stand-in `claude` running `script`, written by a shell of its own. Had this process
    /// written it, another test's thread forking meanwhile would hand its child the open file
    /// until that child ran its own program, and the stand-in wouldn't run then: "Text file
    /// busy". Only the shell, which has ended, ever had it open for writing.
    fn stand_in(name: &str, script: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("ilo-sign-in-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let program = folder.join("claude");
        let mut writer = Command::new("/bin/sh")
            .args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"])
            .arg(&program)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = writer.stdin.take().unwrap();
        write!(input, "#!/bin/sh\n{script}\n").unwrap();
        drop(input);
        assert!(writer.wait().unwrap().success(), "the stand-in wasn't written");
        program
    }

    fn ended(program: &std::path::Path) -> (Box<dyn Session>, Option<Heard>) {
        let (send, mut heard) = unbounded();
        let session = start(Cli::Claude, program, None, send).unwrap();
        let end = next(&mut heard);
        (session, end)
    }

    #[test]
    fn claude_signs_in_with_its_own_command_and_no_input() {
        // It reads its arguments and its input, which ilo closes, so no code can be given.
        let program = stand_in(
            "done",
            r#"[ "$1 $2" = "auth login" ] || exit 9
               read code && exit 8
               exit 0"#,
        );
        assert_eq!(ended(&program).1, Some(Heard::Ended(Ended::Done)));
    }

    #[test]
    fn claude_failing_says_its_last_line_unless_it_might_hold_a_code() {
        let program = stand_in(
            "words",
            "echo 'Opening the sign-in page' >&2; echo 'Login failed: the request timed out' >&2; exit 1",
        );
        let said = Some("Login failed: the request timed out".to_string());
        assert_eq!(ended(&program).1, Some(Heard::Ended(Ended::Failed(said))));
        let program = stand_in(
            "code",
            "echo 'Paste code here if prompted > made-up-code-7Q2X9' >&2; exit 1",
        );
        assert_eq!(ended(&program).1, Some(Heard::Ended(Ended::Failed(None))));
    }

    #[test]
    fn cancel_ends_claudes_sign_in() {
        // It waits for nothing but its end.
        let program = stand_in("wait", "exec tail -f /dev/null");
        let (send, mut heard) = unbounded();
        let mut session = start(Cli::Claude, &program, None, send).unwrap();
        session.cancel();
        assert_eq!(next(&mut heard), Some(Heard::Ended(Ended::Cancelled)));
    }

    #[test]
    fn sign_out_runs_claudes_own_command() {
        let program = stand_in("out", r#"[ "$1 $2" = "auth logout" ] || exit 9"#);
        assert_eq!(sign_out(Cli::Claude, &program, None), Ok(()));
        let program = stand_in("out-failed", "echo 'Not signed in' >&2; exit 1");
        let failed = Err(Some("Not signed in".to_string()));
        assert_eq!(sign_out(Cli::Claude, &program, None), failed);
    }
}

/// The row in each state, the same wherever it's shown: a sign-in that failed or ran out reads as
/// signed out, never as signed in, and a failed Sign Out leaves it signed in.
#[test]
fn a_row_says_its_state_and_offers_what_comes_next() {
    use crate::agents::ready::Ready;
    let row = |cli, ready, plan, row: Row| shown(cli, ready, plan, &row);
    let says = |line: &str, offer| Shown {
        line: line.into(),
        offer,
    };
    let codex = Cli::Codex;
    assert_eq!(
        row(codex, Ready::Unknown, None, Row::Idle),
        says("Looking…", Offer::Nothing)
    );
    assert_eq!(
        row(codex, Ready::NotFound, None, Row::Idle),
        says("Not installed", Offer::Get)
    );
    let out = row(codex, Ready::SignedOut, Some("Plus"), Row::Idle);
    assert_eq!(out, says("Not signed in", Offer::SignIn));
    let signing = row(codex, Ready::SignedOut, None, Row::SigningIn);
    assert_eq!(
        signing,
        says("Finish in the page that opened", Offer::Cancel)
    );
    let signed = row(codex, Ready::SignedIn, Some("Plus"), Row::Idle);
    assert_eq!(signed, says("Signed in · Plus plan", Offer::SignedIn));
    let claude = row(Cli::Claude, Ready::SignedIn, None, Row::Idle);
    assert_eq!(claude, says("Signed in", Offer::SignedIn));
    let said = Row::Failed(Some("Your workspace doesn't allow Codex".into()));
    let failed = row(codex, Ready::SignedOut, None, said.clone());
    assert_eq!(
        failed,
        says("Your workspace doesn't allow Codex", Offer::TryAgain)
    );
    let failed = row(codex, Ready::SignedOut, None, Row::Failed(None));
    assert_eq!(failed, says("The sign-in didn't finish", Offer::TryAgain));
    // Claude Code's command asked for a code, which ilo never gives it.
    let code = row(Cli::Claude, Ready::SignedOut, None, Row::Failed(None));
    assert_eq!(code.line, "Finish with “claude auth login” in a terminal");
    assert_eq!(code.offer, Offer::TryAgain);
    let not_out = row(codex, Ready::SignedIn, None, Row::Failed(None));
    assert_eq!(not_out, says("Couldn't sign out", Offer::SignedIn));
    let out = row(codex, Ready::SignedIn, None, Row::SigningOut);
    assert_eq!(out, says("Signing out…", Offer::Nothing));
}

