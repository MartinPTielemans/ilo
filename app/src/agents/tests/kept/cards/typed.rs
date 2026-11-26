//! What an agent typed or chose in a field is seen again only where the core said the field may
//! show it (`page::typed_shown`): in a step's line, the kept log, a refusal's line, the activity
//! log, the app's log and a kept card's picture alike, by the tool or a step of run_steps. A field
//! the core says nothing of, as today's core says nothing, shows none of it.

use super::*;
use crate::agents::tools::{self, Tense};

/// What the agent puts in each field, none of it a word of the stand-in's answer.
const SEAT: &str = "Aisle 12B";
const CARD: &str = "4242 4242 4242 4242";
const COACH: &str = "Q-7731";
const EXPIRY: &str = "12/29";
const CLASS: &str = "First class";
const SECRET: &str = "hunter2-swordfish";
const CODE: &str = "882910";

/// The page's tree as the core answers each act, a field in each kind: one the core says may show
/// (e1, e5), one it says is a secret's (e2, e4), one it says nothing of (e3), a password field (e6)
/// and one whose name says it's a secret (e7). `focused` is the node with the keyboard, if any.
fn tree(focused: Option<&str>) -> Value {
    let field = |id: &str, role: &str, name: &str, backend: i64, facts: Value| {
        let mut node = json!({
            "nodeId": id, "role": { "value": role }, "name": { "value": name },
            "backendDOMNodeId": backend,
        });
        for (key, value) in facts.as_object().unwrap() {
            node[key] = value.clone();
        }
        if focused == Some(id) {
            node["properties"] = json!([{ "name": "focused", "value": { "value": true } }]);
        }
        node
    };
    let shows = json!({ "secret": false, "autofilled": false });
    let secret = json!({ "secret": true, "autofilled": false });
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Seats" },
          "childIds": ["2", "3", "4", "5", "6", "7", "8"] },
        field("2", "textbox", "Seat", 21, shows.clone()),
        field("3", "textbox", "Card number", 31, secret.clone()),
        field("4", "textbox", "Coach", 41, json!({})),
        field("5", "combobox", "Expiry", 51, secret),
        field("6", "combobox", "Class", 61, shows.clone()),
        field("7", "textbox", "Password", 71, json!({ "password": true })),
        field("8", "textbox", "Passcode", 81, shows),
    ]})
}

/// A task at work on a page of the tree's, listening to the app's log.
struct Typing {
    run: Run,
    agent: String,
    tab: u64,
    /// How many page requests the core was asked, all answered.
    answered: usize,
}

impl Typing {
    fn new(name: &str, cx: &mut TestAppContext) -> Self {
        crate::heard::listen();
        let run = Run::new(name, cx);
        let (agent, tab) = at_work(&run, 1, cx);
        let mut typing = Typing {
            run,
            agent,
            tab,
            answered: 0,
        };
        typing.answer(None, cx);
        typing.act("snapshot", json!({ "tab": tab }), cx);
        typing
    }

    /// The core answers each page request it hasn't yet with the tree.
    fn answer(&mut self, focused: Option<&str>, cx: &mut TestAppContext) {
        loop {
            let request = {
                let pages = self.run.browser.pages.borrow();
                match pages.get(self.answered) {
                    Some(page) => page.1,
                    None => break,
                }
            };
            self.answered += 1;
            let tree = tree(focused).to_string();
            (self.run.agents).update(cx, |agents, cx| agents.page_done(request, tree, None, cx));
            cx.run_until_parked();
        }
    }

    /// The agent calls `tool`, and the core answers.
    fn act(&mut self, tool: &str, arguments: Value, cx: &mut TestAppContext) -> Option<ToolResult> {
        self.act_on(tool, arguments, None, cx)
    }

    fn act_on(
        &mut self,
        tool: &str,
        arguments: Value,
        focused: Option<&str>,
        cx: &mut TestAppContext,
    ) -> Option<ToolResult> {
        let answer = call(&self.run.agents, &self.agent, tool, arguments, cx);
        cx.run_until_parked();
        self.answer(focused, cx);
        answer.try_recv().ok()
    }

    fn keeps(&self, cx: &mut TestAppContext) -> bool {
        let tab = self.tab;
        (self.run.agents).read_with(cx, |agents, cx| agents.keeps_picture(tab, cx))
    }

    fn new_document(&self, url: &str, cx: &mut TestAppContext) {
        let tab = self.tab;
        (self.run.agents).update(cx, |agents, cx| {
            agents.tab_state(tab, url, true, true, cx);
            agents.tab_loaded(tab);
            agents.tab_state(tab, url, false, true, cx);
        });
    }

    /// Everything of the task the user or a file could see again: its steps' lines, its
    /// refusals, its activity entries and the page requests' texts aside, which are the core's.
    fn seen(&self, cx: &mut TestAppContext) -> String {
        let agent = self.agent.as_str();
        (self.run.agents).read_with(cx, |agents, _| {
            let task = agents.task(agent).expect("the task");
            let steps = (task.trace.iter())
                .flat_map(|traced| {
                    [Tense::Now, Tense::Done].map(|tense| tools::step_title(&traced.did, tense))
                })
                .collect::<Vec<_>>();
            let stops = agents.stops(agent);
            let activity = agents.activity(agent).collect::<Vec<_>>();
            format!("{steps:?}\n{stops:?}\n{activity:?}")
        })
    }

    /// The task finishes: what it shows of what was typed, the kept log and the app's log included.
    fn finished(self, cx: &mut TestAppContext) -> String {
        let seen = self.seen(cx);
        self.run.finish(&self.agent, cx);
        let log = kept_log(&self.run, &self.agent, cx).expect("a log");
        let heard = crate::heard::take();
        format!("{seen}\n{log:?}\n{heard:?}")
    }
}

/// A type shows its text only in a field the core says may show it: a secret's field and one the
/// core says nothing of show none of it, in any line or log, and keep the page's picture off a
/// kept card until a new document.
#[gpui::test]
fn typed_text_shows_only_where_the_core_says_the_field_may(cx: &mut TestAppContext) {
    let mut typing = Typing::new("typed", cx);
    let tab = typing.tab;
    typing.act("type", json!({ "tab": tab, "ref": "e1", "text": SEAT }), cx);
    assert!(typing.seen(cx).contains(SEAT), "{}", typing.seen(cx));
    assert!(typing.keeps(cx), "what may show keeps the picture");

    typing.act("type", json!({ "tab": tab, "ref": "e2", "text": CARD }), cx);
    assert!(!typing.keeps(cx));
    typing.new_document("https://www.example.com/coach", cx);
    assert!(typing.keeps(cx));
    typing.act(
        "type",
        json!({ "tab": tab, "ref": "e3", "text": COACH }),
        cx,
    );
    assert!(!typing.keeps(cx), "a field the core says nothing of");

    let typed = typing.run.browser.pages.borrow().clone();
    assert!(
        typed.iter().any(|page| page.3 == 31 && page.4 == CARD),
        "{typed:?}"
    );
    let seen = typing.finished(cx);
    assert!(seen.contains(SEAT), "{seen}");
    for text in [CARD, COACH] {
        assert!(!seen.contains(text), "{text} in {seen}");
    }
}

/// A choice is held as typing is: an option chosen in a secret's list shows nowhere and keeps the
/// picture off a kept card; one in a list the core says may show does both.
#[gpui::test]
fn a_choice_shows_only_where_the_core_says_the_list_may(cx: &mut TestAppContext) {
    let mut typing = Typing::new("chosen", cx);
    let tab = typing.tab;
    typing.act(
        "select",
        json!({ "tab": tab, "ref": "e5", "option": CLASS }),
        cx,
    );
    assert!(typing.keeps(cx), "what may show keeps the picture");
    typing.act(
        "select",
        json!({ "tab": tab, "ref": "e4", "option": EXPIRY }),
        cx,
    );
    assert!(!typing.keeps(cx));

    let seen = typing.finished(cx);
    assert!(seen.contains(CLASS), "{seen}");
    assert!(!seen.contains(EXPIRY), "{seen}");
}

/// A key pressed in a field names only the key, and moves nothing of the picture's keeping: the
/// text in the field came by typing, which held it.
#[gpui::test]
fn a_key_on_a_field_shows_only_the_key(cx: &mut TestAppContext) {
    let mut typing = Typing::new("key", cx);
    let tab = typing.tab;
    let typed = json!({ "tab": tab, "ref": "e2", "text": CARD });
    typing.act_on("type", typed, Some("3"), cx);
    for key in ["Backspace", "Enter"] {
        typing.act_on(
            "press_key",
            json!({ "tab": tab, "key": key }),
            Some("3"),
            cx,
        );
        assert!(!typing.keeps(cx), "{key} kept the picture");
    }
    typing.new_document("https://www.example.com/again", cx);
    typing.act_on(
        "press_key",
        json!({ "tab": tab, "key": "Tab" }),
        Some("3"),
        cx,
    );
    assert!(typing.keeps(cx), "a key alone");

    let seen = typing.finished(cx);
    assert!(!seen.contains(CARD), "{seen}");
}

/// A step of run_steps is held as the tool alone is, its line and its entry in the run's.
#[gpui::test]
fn a_run_s_steps_show_what_the_tool_alone_would(cx: &mut TestAppContext) {
    let mut typing = Typing::new("run-typed", cx);
    let tab = typing.tab;
    let steps = json!([
        { "action": "type", "ref": "e1", "text": SEAT },
        { "action": "type", "ref": "e2", "text": CARD },
        { "action": "select", "ref": "e4", "option": EXPIRY },
    ]);
    typing.act("run_steps", json!({ "tab": tab, "steps": steps }), cx);
    let typed = typing.run.browser.pages.borrow().clone();
    assert!(
        typed.iter().any(|page| page.3 == 51 && page.4 == EXPIRY),
        "{typed:?}"
    );
    assert!(!typing.keeps(cx));

    let seen = typing.finished(cx);
    assert!(seen.contains(SEAT), "{seen}");
    for text in [CARD, EXPIRY] {
        assert!(!seen.contains(text), "{text} in {seen}");
    }
}

/// A refused type or choice names the site, not what it would have put in a secret's field: in
/// the refusal's line, the kept log and the app's log. One that may show is named as before.
#[gpui::test]
fn a_refusal_names_the_site_not_what_was_typed(cx: &mut TestAppContext) {
    let mut typing = Typing::new("refused-typed", cx);
    let (agent, tab) = (typing.agent.clone(), typing.tab);
    (typing.run.agents).update(cx, |agents, cx| agents.set_tier(&agent, Tier::Look, cx));
    let refused = [
        (
            "type",
            json!({ "tab": tab, "ref": "e2", "text": CARD, "submit": true }),
        ),
        (
            "select",
            json!({ "tab": tab, "ref": "e4", "option": EXPIRY }),
        ),
        ("type", json!({ "tab": tab, "ref": "e1", "text": SEAT })),
    ];
    for (tool, arguments) in refused {
        let answer = typing.act(tool, arguments, cx).expect("an answer");
        assert!(answer.error, "{tool} wasn't refused: {}", answer.text);
    }
    let stops = (typing.run.agents).read_with(cx, |agents, _| agents.stops(&agent));
    assert!(stops.iter().any(|stop| stop.at == stop.site), "{stops:?}");
    assert!(stops.iter().any(|stop| stop.at.contains(SEAT)), "{stops:?}");

    let seen = typing.finished(cx);
    assert!(seen.contains("stopped"), "{seen}");
    for text in [CARD, EXPIRY] {
        assert!(!seen.contains(text), "{text} in {seen}");
    }
}

/// A password field takes nothing, and one whose name says it's a secret shows nothing of what
/// it takes, whatever the core says of it.
#[gpui::test]
fn a_password_field_takes_nothing_and_a_secret_s_name_shows_nothing(cx: &mut TestAppContext) {
    let mut typing = Typing::new("password", cx);
    let tab = typing.tab;
    let typed = json!({ "tab": tab, "ref": "e6", "text": SECRET });
    let answer = typing.act("type", typed, cx).expect("an answer");
    assert!(answer.error, "{}", answer.text);
    assert!(typing.keeps(cx), "nothing was typed");
    typing.act("type", json!({ "tab": tab, "ref": "e7", "text": CODE }), cx);
    assert!(!typing.keeps(cx));

    let typed = typing.run.browser.pages.borrow().clone();
    assert!(!typed.iter().any(|page| page.4 == SECRET), "{typed:?}");
    let seen = typing.finished(cx);
    for text in [SECRET, CODE] {
        assert!(!seen.contains(text), "{text} in {seen}");
    }
}

/// A private window's tab takes nothing an agent types: the call is refused before the core is
/// asked, and nothing of it is seen again.
#[gpui::test]
fn a_private_window_s_tab_takes_nothing_typed(cx: &mut TestAppContext) {
    const PRIVATE: (u64, u64) = (90, 9);
    let mut typing = Typing::new("private-typed", cx);
    let browser = &typing.run.browser;
    browser.private.borrow_mut().push(PRIVATE.1);
    (browser.tabs.borrow_mut()).push((PRIVATE.0, PRIVATE.1, "Seats", "https://www.example.com/"));
    let asked = browser.pages.borrow().len();
    let typed = json!({ "tab": PRIVATE.0, "ref": "e2", "text": CARD });
    let answer = typing.act("type", typed, cx).expect("an answer");
    assert!(answer.error, "{}", answer.text);
    assert_eq!(
        typing.run.browser.pages.borrow().len(),
        asked,
        "the core was asked"
    );

    let seen = typing.finished(cx);
    assert!(!seen.contains(CARD), "{seen}");
}
