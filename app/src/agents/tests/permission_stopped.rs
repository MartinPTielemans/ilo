//! A permission the browser refused a page a thread holds is one entry in the activity log, in the
//! kind's own sentence with the site as the one stopped. The core's entry names the kind by a code
//! word and gives a code word for a reason; the user reads neither.

use super::*;
use crate::product::PRODUCT;

/// The reason the core gives with every such entry: a code word, the same for every kind.
const REASON: &str = "agent_space";

/// Each kind with a sentence of its own, by the core's code word, and what its site was stopped
/// from.
const KINDS: [(&str, &str); 19] = [
    ("camera", "using the camera"),
    ("camera_pan_tilt_zoom", "using the camera"),
    ("microphone", "using the microphone"),
    ("location", "seeing your location"),
    ("clipboard_read", "reading the clipboard"),
    ("clipboard_write", "copying to the clipboard"),
    ("screen_capture", "recording the screen"),
    ("notifications", "sending notifications"),
    ("file_picker", "opening a file picker"),
    ("file_handle", "using your files"),
    ("color_picker", "picking a color from the screen"),
    ("print", "printing"),
    ("share", "opening the share sheet"),
    (
        "federated_sign_in",
        "signing you in with another site's account",
    ),
    ("payment", "asking for a payment"),
    ("client_certificate", "asking for a certificate"),
    ("cast", "casting to a device"),
    ("extension_install", "installing an extension"),
    ("app_install", "installing itself as an app"),
];

/// Kinds with no sentence of their own.
const OTHERS: [&str; 4] = ["midi", "local_fonts", "idle_detection", "a_kind_made_later"];

/// The entry's row as the activity log draws it: its title, and what it says under it of why.
fn row(action: &str, site: &str) -> (String, String) {
    let did = tools::Did {
        tool: action.into(),
        site: site.into(),
        ..Default::default()
    };
    (
        tools::step_title(&did, tools::Tense::Failed),
        tools::refusal_shown(action, REASON).to_string(),
    )
}

#[test]
fn each_kind_has_its_own_sentence() {
    for (kind, from) in KINDS {
        let (title, why) = row(&format!("request_permission {kind}"), "example.com");
        assert_eq!(title, format!("{PRODUCT} stopped example.com from {from}"));
        assert_eq!(why, "", "{kind}");
    }
}

#[test]
fn every_other_kind_takes_the_one_sentence() {
    for kind in OTHERS {
        let (title, why) = row(&format!("request_permission {kind}"), "example.com");
        assert_eq!(
            title,
            format!("{PRODUCT} stopped a permission request from example.com")
        );
        assert_eq!(why, "", "{kind}");
    }
}

/// A page with no host to give, as a blank one, is `a page` in the site's place, from one place, in
/// each kind's whole sentence.
#[test]
fn a_page_with_no_host_is_a_page_in_every_sentence() {
    assert_eq!(tools::stopped_site(""), "a page");
    assert_eq!(tools::stopped_site("example.com"), "example.com");
    for (kind, from) in KINDS {
        let (title, why) = row(&format!("request_permission {kind}"), "");
        assert_eq!(title, format!("{PRODUCT} stopped a page from {from}"));
        assert_eq!(why, "", "{kind}");
    }
    for kind in OTHERS {
        let (title, _) = row(&format!("request_permission {kind}"), "");
        assert_eq!(
            title,
            format!("{PRODUCT} stopped a permission request from a page")
        );
    }
}

/// A page's file dialog the core cancelled is the file picker's entry, whichever action it came
/// under, and its row says the one sentence and nothing of why under it.
#[test]
fn a_cancelled_file_dialog_is_the_file_pickers_sentence() {
    let why = "Agents can't use system file dialogs, so the page was told it was cancelled.";
    let picker = format!("{PRODUCT} stopped example.com from opening a file picker");
    for action in [
        "file_dialog file_input",
        "file_dialog file_system_access",
        "request_permission file_picker",
    ] {
        let did = tools::Did {
            tool: action.into(),
            site: "example.com".into(),
            ..Default::default()
        };
        assert_eq!(tools::step_title(&did, tools::Tense::Failed), picker);
        assert_eq!(tools::refusal_shown(action, why), "", "{action}");
    }
}

#[test]
fn the_sentence_is_the_same_done_or_refused() {
    let did = tools::Did {
        tool: "request_permission camera".into(),
        site: "example.com".into(),
        ..Default::default()
    };
    let failed = tools::step_title(&did, tools::Tense::Failed);
    assert_eq!(tools::step_title(&did, tools::Tense::Done), failed);
    assert_eq!(tools::step_title(&did, tools::Tense::Now), failed);
}

/// The core's entry, as it comes for the page in tab 20 of Claude Code's Space, is in the log with
/// the page's host alone, and its row never says the entry's code word or its reason. Nor the
/// kind's, where the kind has no sentence of its own: it stands in the entry only.
#[gpui::test]
fn the_row_never_says_the_cores_code_words(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    let kinds = (KINDS.iter().map(|(kind, _)| *kind)).chain(OTHERS);
    for kind in kinds {
        let action = format!("request_permission {kind}");
        agents.update(cx, |agents, cx| {
            agents.checked("claude-code", 20, &action, REASON, None, cx)
        });
        let (logged, site, refused) = log(&agents, "claude-code", cx).remove(0);
        assert_eq!(
            (logged.as_str(), refused.as_str()),
            (action.as_str(), REASON)
        );
        assert!(!site.is_empty() && !site.contains('/'), "{site}");

        let (title, why) = row(&logged, &site);
        assert!(title.contains(&site), "{title}");
        let others = OTHERS.contains(&kind).then_some(kind);
        for code in ["request_permission", REASON].into_iter().chain(others) {
            assert!(!title.contains(code), "{title} says {code}");
            assert!(!why.contains(code), "{why} says {code}");
        }
    }
}

/// A device's refusal keeps the chooser's sentence under its row, as any other refusal keeps its.
#[test]
fn another_refusal_keeps_its_sentence() {
    let chooser = "The page asked for a device, which only the user can give.";
    assert_eq!(tools::refusal_shown("request_device usb", chooser), chooser);
    assert_eq!(tools::refusal_shown("click", "Not a tab."), "Not a tab.");
}

/// The reason the core gives for a question it cancelled in a handed tab the user themself took
/// over is matched whole and kept as every other entry's. Anything that is not that code whole is
/// kept as it came.
#[test]
fn the_users_hold_reason_is_kept_as_every_other_entrys() {
    assert_eq!(tools::kept_reason(tools::USERS_HOLD), REASON);
    assert_eq!(tools::AGENT_SPACE, REASON);
    for other in [
        "",
        REASON,
        "agent_space_users_hold ",
        " agent_space_users_hold",
        "Agent_Space_Users_Hold",
        "agent_space_users_hold: example.com",
        "Not a tab.",
    ] {
        assert_eq!(tools::kept_reason(other), other);
    }
}

/// The core's entry with that reason, for the camera and for the microphone, is in the activity
/// log by the reason every other has, and so are the line for the core's entry, the line for the
/// log's own and the thread's file once it is over: none of them ever holds the code.
#[gpui::test]
fn neither_log_line_nor_the_past_threads_file_holds_the_users_hold_code(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    crate::heard::listen();
    for kind in ["camera", "microphone"] {
        let action = format!("request_permission {kind}");
        tools::log_activity("claude-code", 20, &action, "", tools::USERS_HOLD);
        agents.update(cx, |agents, cx| {
            agents.checked("claude-code", 20, &action, tools::USERS_HOLD, None, cx)
        });
        let (logged, _, refused) = log(&agents, "claude-code", cx).remove(0);
        assert_eq!(
            (logged.as_str(), refused.as_str()),
            (action.as_str(), REASON)
        );
    }
    let lines = crate::heard::take();
    for start in ["browser: agent activity ", "browser: agent logged "] {
        let of_it: Vec<_> = (lines.iter())
            .filter(|line| line.starts_with(start) && line.contains("request_permission "))
            .collect();
        assert_eq!(of_it.len(), 2, "{lines:#?}");
        for line in of_it {
            assert!(line.ends_with(&format!(" refused: {REASON}")), "{line}");
        }
    }
    let held = lines.iter().find(|line| line.contains(tools::USERS_HOLD));
    assert_eq!(held, None);

    for event in [
        Event::Closed {
            agent: "claude-code".into(),
            connection: 1,
        },
        Event::Bye {
            agent: "claude-code".into(),
        },
    ] {
        agents.update(cx, |agents, cx| agents.handle(event, cx));
    }
    let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
    assert_eq!(kept.len(), 1);
    let file = crate::agent_history::write(&[kept[0].1.clone()]);
    assert!(file.contains("request_permission camera"), "{file}");
    assert!(file.contains("request_permission microphone"), "{file}");
    assert!(!file.contains(tools::USERS_HOLD), "{file}");
}

/// No entry an agent's call can cause has that code for its reason: not by a tool of its name, by
/// the code as any argument's words, or by any road an entry is made on. Only the core's own entry
/// comes with it, and the log keeps that one by the other reason too.
#[gpui::test]
fn no_entry_an_agents_call_can_cause_has_the_users_hold_code_for_its_reason(
    cx: &mut TestAppContext,
) {
    let (agents, _, _) = connected(cx);
    let code = tools::USERS_HOLD;
    let mut names = tools::names("claude-code");
    names.extend(tools::names("task-1"));
    names.push(code.to_string());
    crate::heard::listen();
    for agent in ["claude-code", "task-1"] {
        for name in &names {
            let said = json!({
                "tab": 20, "tabs": [20], "url": code, "text": code, "ref": code, "why": code,
                "title": code, "key": code, "option": code, "name": code, "method": code,
                "what": code, "reason": code, "refused": code, "action": code,
                "steps": [{ "tool": code, "ref": code, "text": code, "refused": code }],
            });
            let _ = call(&agents, agent, name, said, cx);
            cx.run_until_parked();
        }
    }
    agents.update(cx, |agents, _| {
        agents.note("claude-code", 1, code, code, code);
        agents.note_page("claude-code", (1, 20), code, code, code);
        agents.note_on("claude-code", 1, code, code, code, code);
        let nothing = ("", Vec::new(), "");
        agents.note_entry("claude-code", (1, 20), code, code, code, code, nothing);
    });
    let lines = crate::heard::take();
    let reasons: Vec<String> = agents.read_with(cx, |agents, _| {
        (agents.recent())
            .flat_map(|entry| std::iter::once(entry).chain(&entry.steps))
            .map(|entry| entry.refused.to_string())
            .collect()
    });
    assert!(reasons.len() > 4, "{reasons:?}");
    assert_eq!(reasons.iter().find(|reason| *reason == code), None);
    let ends = format!(" refused: {code}");
    assert_eq!(lines.iter().find(|line| line.ends_with(&ends)), None);
}
