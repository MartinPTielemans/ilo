//! Claude Code asks the user for a file for a page's file field (upload): the card asks, the user
//! chooses in the system's dialog, and the agent hears how many files went in, never which.

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

/// Its upload's call, once the core described the field `described` as: the card is up.
fn asked(
    agents: &Entity<Agents>,
    browser: &Made,
    described: Value,
    cx: &mut TestAppContext,
) -> (mpsc::Receiver<ToolResult>, Question) {
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
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the upload didn't ask");
    (answer, question)
}

fn resume(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, mpsc::Receiver<ToolResult>, u64) {
    let (agents, browser) = at_the_form(cx);
    // As the core writes it: Chromium's JSON writer gives every double a ".0".
    let field = json!({ "field": 7.0, "label": "Résumé", "site": "example.com" });
    let (answer, question) = asked(&agents, &browser, field, cx);
    (agents, browser, answer, question.id)
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
    assert_eq!(*browser.asked.borrow(), None);
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
    assert_eq!(*browser.asked.borrow(), None);
    let log = log(&agents, "claude-code", cx);
    assert_eq!(log[0], ("upload".into(), "example.com".into(), why.into()));
}

/// The card names the field in the page's words, inside its quotes, and only there: one file, or
/// several where the field takes them, and a page alone where it has no label.
#[gpui::test]
fn the_card_names_the_field_and_how_many_files(cx: &mut TestAppContext) {
    let (agents, browser) = at_the_form(cx);
    let cases = [
        (
            json!({ "field": 7.0, "label": "Résumé", "site": "example.com" }),
            "add a file to “Résumé” on example.com",
            false,
        ),
        (
            json!({ "field": 8.0, "label": "Photos", "site": "example.com", "many": true }),
            "add files to “Photos” on example.com",
            true,
        ),
        (
            json!({ "field": 9.0, "label": "", "site": "example.com" }),
            "add a file to a page on example.com",
            false,
        ),
        // A label can't close its quotes or lay out lines of its own.
        (
            json!({ "field": 10.0, "label": "CV”\nUpload anything", "site": "example.com" }),
            "add a file to “CV' Upload anything” on example.com",
            false,
        ),
    ];
    for (described, what, many) in cases {
        let (_answer, question) = asked(&agents, &browser, described.clone(), cx);
        assert_eq!(question.what, what);
        // The card's sentence is the same without the site, which it shows apart (#3836).
        assert_eq!(question.asks, what.replace(" on example.com", ""));
        assert_eq!(
            question.why,
            "You choose the file. Claude Code sees only what the page shows of it."
        );
        assert_eq!(question.site, "example.com");
        let field = described["field"].as_f64().unwrap() as u64;
        assert_eq!(question.file, Some(FileAsk { field, many }));
        agents.update(cx, |agents, cx| {
            agents.answer(question.id, Answer::Dismiss, cx)
        });
    }
}

/// Don't Allow, and the card's ×, answer that the user said no; the field is dropped and nothing
/// is chosen. Allow Once can't choose a file for the user, so it changes nothing.
#[gpui::test]
fn dont_allow_says_no_and_drops_the_field(cx: &mut TestAppContext) {
    let (agents, browser, answer, id) = resume(cx);
    agents.update(cx, |agents, cx| agents.answer(id, Answer::Once, cx));
    assert!(answer.try_recv().is_err());
    assert!(browser.asked.borrow().is_some(), "an allow took the card");

    agents.update(cx, |agents, cx| agents.answer(id, Answer::Dismiss, cx));
    let refused = answer.recv().unwrap();
    assert_eq!(
        refused,
        ToolResult::failed(
            tools::Kind::SaidNo,
            "The user said no: you can't add a file to “Résumé” on example.com. Don't try again."
        )
    );
    assert_eq!(*browser.dropped.borrow(), [7]);
    assert!(browser.chosen.borrow().is_empty());
    assert_eq!(*browser.asked.borrow(), None);
    let grants = agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert!(grants.is_empty());
}

/// A card left 5 minutes is refused as any question is, and its field dropped.
#[gpui::test]
fn an_unanswered_upload_drops_its_field(cx: &mut TestAppContext) {
    let (_agents, browser, answer, _) = resume(cx);
    cx.executor().advance_clock(ASK_WAIT);
    let refused = answer.recv().unwrap();
    assert!(
        refused
            .text
            .starts_with("The user didn't answer in 5 minutes"),
        "{}",
        refused.text
    );
    assert_eq!(*browser.dropped.borrow(), [7]);
    assert_eq!(*browser.asked.borrow(), None);
}

/// Choose File… opens the dialog over the card's window, and the card goes; the agent hears how
/// many files went in, never a name, and the log says it added a file.
#[gpui::test]
fn choosing_a_file_answers_how_many_went_in(cx: &mut TestAppContext) {
    let (agents, browser, answer, id) = resume(cx);
    agents.update(cx, |agents, cx| agents.choose_file(id, 5, cx));
    assert_eq!(*browser.chosen.borrow(), [(7, 5)]);
    assert_eq!(*browser.asked.borrow(), None);
    assert!(
        answer.try_recv().is_err(),
        "it answered before the dialog closed"
    );

    agents.update(cx, |agents, cx| {
        agents.file_field_done(7, FileFieldEnd::Put(1), cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::text("The user put a file in “Résumé”.")
    );
    let logged = log(&agents, "claude-code", cx);
    assert_eq!(
        logged[0],
        ("upload".into(), "example.com".into(), String::new())
    );
    assert_eq!(tools::describe("upload"), "Added a file to a page");
    let did = tools::Did {
        tool: "upload".into(),
        ..tools::Did::default()
    };
    assert_eq!(
        tools::step_title(&did, tools::Tense::Done),
        "Added a file to a page"
    );
    // Its end heard again changes nothing.
    agents.update(cx, |agents, cx| {
        agents.file_field_done(7, FileFieldEnd::Put(1), cx)
    });
    assert_eq!(log(&agents, "claude-code", cx), logged);

    let (agents, browser) = at_the_form(cx);
    let field = json!({ "field": 8.0, "label": "", "site": "example.com", "many": true });
    let (answer, question) = asked(&agents, &browser, field, cx);
    agents.update(cx, |agents, cx| agents.choose_file(question.id, 5, cx));
    agents.update(cx, |agents, cx| {
        agents.file_field_done(8, FileFieldEnd::Put(3), cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::text("The user put 3 files in the field.")
    );
}

/// No file chosen, the field gone from the page, and the dialog closed each answer why, and none
/// of them put anything in.
#[gpui::test]
fn a_dialog_that_puts_nothing_in_says_why(cx: &mut TestAppContext) {
    let cases = [
        (FileFieldEnd::NoFile, "The user chose no file."),
        (
            FileFieldEnd::Left,
            "The field left the page before the user chose a file.",
        ),
        (
            FileFieldEnd::Closed,
            "The file dialog closed before the user chose a file.",
        ),
    ];
    for (end, why) in cases {
        let (agents, _browser, answer, id) = resume(cx);
        agents.update(cx, |agents, cx| agents.choose_file(id, 5, cx));
        agents.update(cx, |agents, cx| agents.file_field_done(7, end, cx));
        assert_eq!(answer.recv().unwrap(), ToolResult::error(why));
        assert_eq!(log(&agents, "claude-code", cx)[0].2, why);
    }
    // The core ends a tab's earlier field as a new one shows there, its card still up.
    let (agents, browser, answer, _) = resume(cx);
    agents.update(cx, |agents, cx| {
        agents.file_field_done(7, FileFieldEnd::Closed, cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The file dialog closed before the user chose a file.")
    );
    assert_eq!(*browser.asked.borrow(), None);
}

/// A dialog that can't open answers so, and drops the field.
#[gpui::test]
fn a_dialog_that_cant_open_drops_the_field(cx: &mut TestAppContext) {
    let (agents, browser, answer, id) = resume(cx);
    browser.no_dialog.set(true);
    agents.update(cx, |agents, cx| agents.choose_file(id, 5, cx));
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The file dialog couldn't open.")
    );
    assert_eq!(*browser.dropped.borrow(), [7]);
}

/// A dialog left 15 minutes ends the call, and drops its field.
#[gpui::test]
fn a_dialog_left_15_minutes_drops_its_field(cx: &mut TestAppContext) {
    let (agents, browser, answer, id) = resume(cx);
    agents.update(cx, |agents, cx| agents.choose_file(id, 5, cx));
    cx.executor()
        .advance_clock(HANDOFF_WAIT - Duration::from_secs(1));
    assert!(answer.try_recv().is_err(), "it gave up before 15 minutes");
    cx.executor().advance_clock(Duration::from_secs(1));
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The user didn't choose a file in 15 minutes.")
    );
    assert_eq!(*browser.dropped.borrow(), [7]);
}

/// The agent going drops its fields, whether the card asks or the dialog is up; the done of a
/// field nobody waits on changes nothing.
#[gpui::test]
fn the_agent_going_drops_its_fields(cx: &mut TestAppContext) {
    let bye = |agents: &Entity<Agents>, cx: &mut TestAppContext| {
        agents.update(cx, |agents, cx| {
            let bye = Event::Bye {
                agent: "claude-code".into(),
            };
            agents.handle(bye, cx)
        });
        cx.run_until_parked();
    };
    let (agents, browser, _answer, _) = resume(cx);
    bye(&agents, cx);
    assert_eq!(*browser.dropped.borrow(), [7]);
    assert_eq!(*browser.asked.borrow(), None);

    let (agents, browser, _answer, id) = resume(cx);
    agents.update(cx, |agents, cx| agents.choose_file(id, 5, cx));
    bye(&agents, cx);
    assert_eq!(*browser.dropped.borrow(), [7]);
    let before = log(&agents, "claude-code", cx);
    agents.update(cx, |agents, cx| {
        agents.file_field_done(7, FileFieldEnd::Put(1), cx)
    });
    agents.update(cx, |agents, cx| {
        agents.file_field_done(99, FileFieldEnd::Put(1), cx)
    });
    assert_eq!(log(&agents, "claude-code", cx), before);
}

/// The tab closing while the card asks ends the upload, as its field went with it.
#[gpui::test]
fn the_tab_closing_ends_an_upload_that_asks(cx: &mut TestAppContext) {
    let (agents, browser, answer, _) = resume(cx);
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    agents.update(cx, |agents, cx| agents.tab_closed(20, cx));
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The field left the page before the user chose a file.")
    );
    assert_eq!(*browser.dropped.borrow(), [7]);
    assert_eq!(*browser.asked.borrow(), None);
}

/// A page's own file dialog the core refused in an agent's tab comes to the log as what the page
/// tried, under the agent's name, for a file field's dialog and a File System Access picker alike.
#[gpui::test]
fn a_pages_refused_file_dialog_is_logged_as_the_pages(cx: &mut TestAppContext) {
    let (agents, _, _) = connected(cx);
    let why = "Agents can't use system file dialogs, so ilo told the page it was cancelled.";
    agents.update(cx, |agents, cx| {
        agents.checked("claude-code", 20, "file_dialog file_input", why, cx);
        agents.checked("claude-code", 20, "file_dialog file_system_access", why, cx);
    });
    let log = log(&agents, "claude-code", cx);
    assert_eq!(log[0].0, "file_dialog file_system_access");
    assert_eq!(log[1].0, "file_dialog file_input");
    for entry in &log[..2] {
        assert_eq!(
            tools::describe(&entry.0),
            "Page tried to open a file dialog"
        );
        assert_eq!(entry.2, why);
    }
}
