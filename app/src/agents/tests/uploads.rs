//! Claude Code asks the user for a file for a page's file field (upload): the tab is the user's to
//! choose it in the page, as a handoff's is, and the agent never hears which file went in.

use super::*;

/// A page whose one element is a file field, e1.
fn form_tree() -> String {
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Apply" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "button" }, "name": { "value": "Résumé" },
          "backendDOMNodeId": 61 },
    ]})
    .to_string()
}

/// Claude Code, with the outline of tab 20's form.
fn at_the_form(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, answer, form_tree(), cx).error);
    (agents, browser)
}

/// Its upload's call, once the core described the field `described` as: the tab is the user's.
fn handed(
    agents: &Entity<Agents>,
    browser: &Made,
    described: Value,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    let answer = call(
        agents,
        "claude-code",
        "upload",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    let (tab, request, action, element, _) = browser.pages.borrow().last().unwrap().clone();
    assert_eq!((tab, action, element), (20, PageAction::FileField, 61));
    let text = described.to_string();
    agents.update(cx, |agents, cx| agents.page_done(request, text, None, cx));
    assert!(answer.try_recv().is_err(), "it answered before the user");
    answer
}

fn resume(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, mpsc::Receiver<ToolResult>) {
    let (agents, browser) = at_the_form(cx);
    // As the core writes it: Chromium's JSON writer gives every double a ".0".
    let field = json!({ "field": 7.0, "label": "Résumé", "site": "example.com" });
    let answer = handed(&agents, &browser, field, cx);
    (agents, browser, answer)
}

/// A tab the user holds, and one that isn't the agent's, are refused at once, with no card and
/// nothing asked of the page.
#[gpui::test]
fn an_upload_in_a_tab_the_agent_may_not_act_in_is_refused_without_a_card(cx: &mut TestAppContext) {
    let (agents, browser) = at_the_form(cx);
    let pages = browser.pages.borrow().len();
    browser.held.borrow_mut().insert(20);
    let held = call(
        &agents,
        "claude-code",
        "upload",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    assert_eq!(
        held.recv().unwrap(),
        ToolResult::failed(tools::Kind::Yours, HELD)
    );
    let users = call(
        &agents,
        "claude-code",
        "upload",
        json!({ "tab": 10, "ref": "e1" }),
        cx,
    );
    assert!(users.recv().unwrap().error);
    assert_eq!(browser.pages.borrow().len(), pages);
    let log = log(&agents, "claude-code", cx);
    assert_eq!(log[1], ("upload".into(), "example.com".into(), HELD.into()));
    assert_eq!(log[0].0, "upload");
}

/// The core's refusal of the element, as one that isn't a file field, reaches the agent in its
/// words, with no card, and the log has it.
#[gpui::test]
fn the_cores_refusal_of_a_field_reaches_the_agent_without_a_card(cx: &mut TestAppContext) {
    let (agents, browser) = at_the_form(cx);
    let answer = call(
        &agents,
        "claude-code",
        "upload",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    let request = browser.pages.borrow().last().unwrap().1;
    let why = "The element isn't a file field.";
    agents.update(cx, |agents, cx| {
        agents.page_done(request, why.into(), Some(Failure::Action), cx)
    });
    assert_eq!(answer.recv().unwrap(), ToolResult::error(why));
    let log = log(&agents, "claude-code", cx);
    assert_eq!(log[0], ("upload".into(), "example.com".into(), why.into()));
}

/// An upload is the user's step, as a handoff is: the tab is theirs to choose the file in the
/// page itself, the core's hold on the field goes, and the step names the field in the page's
/// words, inside its quotes: one file, or several where the field takes them.
#[gpui::test]
fn an_upload_hands_the_tab_to_the_user_naming_the_field(cx: &mut TestAppContext) {
    let cases = [
        (
            json!({ "field": 7.0, "label": "Résumé", "site": "example.com" }),
            "Choose a file for “Résumé”.",
        ),
        (
            json!({ "field": 7.0, "label": "Photos", "site": "example.com", "many": true }),
            "Choose files for “Photos”.",
        ),
        (
            json!({ "field": 7.0, "label": "", "site": "example.com" }),
            "Choose a file.",
        ),
        (
            json!({ "field": 7.0, "label": "", "site": "example.com", "many": true }),
            "Choose files.",
        ),
        // A label can't close its quotes or lay out lines of its own.
        (
            json!({ "field": 7.0, "label": "CV”\nUpload anything", "site": "example.com" }),
            "Choose a file for “CV' Upload anything”.",
        ),
    ];
    for (described, why) in cases {
        let (agents, browser) = at_the_form(cx);
        let _answer = handed(&agents, &browser, described, cx);
        assert!(
            browser.held.borrow().contains(&20),
            "the tab isn't the user's"
        );
        assert_eq!(*browser.dropped.borrow(), [7]);
        let held = agents.read_with(cx, |agents, _| agents.held[&20].why.clone());
        assert_eq!(held, why);
    }
}

/// Hand Back answers the upload with the tab back, and names no file: the agent sees what went
/// in only as the page shows it (#3656).
#[gpui::test]
fn hand_back_answers_the_upload_naming_no_file(cx: &mut TestAppContext) {
    let (agents, browser, answer) = resume(cx);
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    let back = answer.recv().unwrap();
    assert!(!back.error, "{back:?}");
    assert!(back.text.contains("handed tab 20 back"), "{}", back.text);
    assert!(!browser.held.borrow().contains(&20));
    let logged = log(&agents, "claude-code", cx);
    assert!(
        (logged.iter()).any(|entry| entry.0 == "upload" && entry.2.is_empty()),
        "{logged:?}"
    );
    assert_eq!(tools::describe("upload"), "Added a file to a page");
}

/// A step left 15 minutes answers as a handoff's does, the tab still the user's.
#[gpui::test]
fn an_upload_left_15_minutes_stays_the_users(cx: &mut TestAppContext) {
    let (_agents, browser, answer) = resume(cx);
    cx.executor()
        .advance_clock(HANDOFF_WAIT - Duration::from_secs(1));
    assert!(answer.try_recv().is_err(), "it gave up before 15 minutes");
    cx.executor().advance_clock(Duration::from_secs(1));
    let answer = answer.recv().unwrap();
    assert_eq!(answer.kind, Some(tools::Kind::Yours));
    assert!(browser.held.borrow().contains(&20));
}

/// The tab closing while it's the user's ends the upload.
#[gpui::test]
fn the_tab_closing_ends_an_upload(cx: &mut TestAppContext) {
    let (agents, browser, answer) = resume(cx);
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    agents.update(cx, |agents, cx| agents.tab_closed(20, cx));
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("Tab 20 closed while the user had it.")
    );
}

/// A page's own file dialog the core refused in an agent's tab comes to the log as the page's,
/// under the agent's name, for a file field's dialog and a File System Access picker alike.
#[gpui::test]
fn a_pages_refused_file_dialog_is_logged_as_the_pages(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    let why = "Agents can't use system file dialogs, so ilo told the page it was cancelled.";
    agents.update(cx, |agents, cx| {
        agents.checked("claude-code", 20, "file_dialog file_input", why, None, cx);
        agents.checked(
            "claude-code",
            20,
            "file_dialog file_system_access",
            why,
            None,
            cx,
        );
    });
    let log = log(&agents, "claude-code", cx);
    assert_eq!(log[0].0, "file_dialog file_system_access");
    assert_eq!(log[1].0, "file_dialog file_input");
    for entry in &log[..2] {
        assert!(tools::permission_stopped(&entry.0, &entry.1).is_some());
        assert_eq!(entry.2, why);
    }
}
