//! A permission the browser refused a page a thread holds is one entry in the activity log, in the
//! kind's own sentence with the site as the one stopped. The core's entry names the kind by a code
//! word and gives a code word for a reason; the user reads neither.

use super::*;
use crate::product::PRODUCT;

/// The reason the core gives with every such entry: a code word, the same for every kind.
const REASON: &str = "agent_space";

/// Each kind with a sentence of its own, by the core's code word, and what its site was stopped
/// from.
const KINDS: [(&str, &str); 9] = [
    ("camera", "using the camera"),
    ("camera_pan_tilt_zoom", "using the camera"),
    ("microphone", "using the microphone"),
    ("location", "seeing your location"),
    ("clipboard_read", "reading the clipboard"),
    ("clipboard_write", "copying to the clipboard"),
    ("screen_capture", "recording the screen"),
    ("notifications", "sending notifications"),
    ("file_picker", "opening a file picker"),
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
    let why = "Agents can't use system file dialogs, so ilo told the page it was cancelled.";
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
            agents.checked("claude-code", 20, &action, REASON, cx)
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
