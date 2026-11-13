//! Library › Agents: the profile's past tasks, kept 30 days, each one deleted with its ✕, all with
//! Clear or with the profile's history of their time, and never in a private window (#1080).

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::Item;
use crate::agent_history::{Past, Step, read, write};
use crate::shell::commands::ShellHandle;
use crate::shell::recorder::Recorder;
use crate::shell::{Command, Folders, HistoryDeleted, Section, Shell, history_deleted};

const DAY: u64 = 24 * 60 * 60;

fn past(id: &str, ended: u64) -> Past {
    Past {
        id: id.into(),
        title: format!("Task {id}"),
        agent: "Claude Code".into(),
        started: ended - 60,
        ended,
        pages: vec![("Booking".into(), "https://taberna.example/booking".into())],
        orders: 1,
        messages: 0,
        steps: vec![Step {
            at: ended - 30,
            action: "click".into(),
            site: "taberna.example".into(),
            refused: String::new(),
            ..Default::default()
        }],
        answers: "claude-code".into(),
    }
}

/// A window on Space 1 of profile 0, whose file keeps task 2, a minute old, task 1, a day old,
/// and task 0, 30 days old, with the Library open on Agents.
fn agents(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let now = crate::usage::now();
    let kept = [
        past("2", now - 60),
        past("1", now - DAY),
        past("0", now - 30 * DAY),
    ];
    host.agent_history.borrow_mut().insert(0, write(&kept));
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (
            1,
            "Space 1".into(),
            None,
            0,
            "".into(),
            "".into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.section = Section::Agents;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    (shell, host, cx)
}

fn listed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let items = &shell.library.as_ref().unwrap().items;
        (items.iter())
            .filter_map(|item| match item {
                Item::Past(index) => Some(shell.agent_history[*index].id.clone()),
                Item::Clear => Some("clear".into()),
                _ => None,
            })
            .collect()
    })
}

fn file(host: &Recorder) -> Vec<String> {
    let text = host
        .agent_history
        .borrow()
        .get(&0)
        .cloned()
        .unwrap_or_default();
    read(&text).into_iter().map(|past| past.id).collect()
}

/// It lists what the file keeps, newest first, under Clear, and a task that ended 30 days ago goes,
/// from the list and from the file.
#[gpui::test]
fn agents_lists_the_profiles_past_tasks_for_30_days(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    assert_eq!(listed(&shell, cx), ["clear", "2", "1"]);
    assert_eq!(
        file(&host),
        ["2", "1"],
        "the 30-day-old task stayed in the file"
    );
}

/// A row's ✕ deletes it alone, and Clear the rest.
#[gpui::test]
fn one_deletes_alone_and_clear_takes_them_all(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    shell.update(cx, |shell, cx| shell.delete_past_task("2", cx));
    assert_eq!(listed(&shell, cx), ["clear", "1"]);
    assert_eq!(file(&host), ["1"]);
    shell.update(cx, |shell, cx| shell.clear_agent_history(cx));
    assert!(listed(&shell, cx).is_empty());
    assert!(file(&host).is_empty());
}

/// Deleting the profile's history takes the past tasks that ran in the time it deleted, and all
/// of them with all of it. Opening Clear Browsing Data deletes nothing: the user may cancel it.
#[gpui::test]
fn deleting_history_deletes_them(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
    shell.update(cx, |shell, cx| {
        shell.run(Command::ClearBrowsingData, "ui", cx)
    });
    assert_eq!(file(&host), ["2", "1"]);
    let now = crate::usage::now();
    let hour = HistoryDeleted::Some {
        begin: now - 3600,
        end: u64::MAX,
        urls: Vec::new(),
    };
    cx.update(|_, cx| history_deleted(0, hour, cx));
    assert_eq!(file(&host), ["1"]);
    // Another profile's history going leaves this one's.
    cx.update(|_, cx| history_deleted(1, HistoryDeleted::All, cx));
    assert_eq!(file(&host), ["1"]);
    cx.update(|_, cx| history_deleted(0, HistoryDeleted::All, cx));
    assert!(file(&host).is_empty());
}

/// A private window lists none, and reads none.
#[gpui::test]
fn a_private_window_lists_no_past_tasks(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    host.agent_history
        .borrow_mut()
        .insert(0, write(&[past("1", crate::usage::now())]));
    let browser = host.clone();
    let (shell, cx) =
        cx.add_window_view(|_, cx| Shell::private_window(browser, 1, "Private".into(), 0, cx));
    shell.update(cx, |shell, cx| {
        shell.section = Section::Agents;
        shell.set_library(true, cx);
    });
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.agent_history.is_empty()));
}

fn page(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let library = shell.library.as_ref().unwrap();
        (library.items.iter())
            .map(|item| match *item {
                Item::PastHead(index) => format!("head {}", shell.agent_history[index].id),
                Item::Heading(heading) => heading.to_string(),
                Item::PastSite(index) => {
                    let (site, answer) = &library.sites[index];
                    format!("{site} {answer:?}")
                }
                Item::PastStep(index, step) => {
                    format!("step {}", shell.agent_history[index].steps[step].action)
                }
                _ => "list".to_string(),
            })
            .collect()
    })
}

/// A row opens its task's page in the list's place: its sites, each with its agent's answer there
/// for good, then its steps, newest first. Revoke takes an answer, as Settings › Agents' does, and
/// Back, or a search, goes back to the list (#1080).
#[gpui::test]
fn a_past_tasks_page_lists_its_steps_and_sites_with_revoke(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    host.agent_answers.borrow_mut().insert(
        "claude-code".into(),
        vec![
            ("taberna.example".into(), true),
            ("elsewhere.example".into(), false),
        ],
    );
    shell.update(cx, |shell, cx| {
        let past = &mut shell.agent_history[0];
        past.steps.push(Step {
            at: past.ended - 10,
            action: "read_page".into(),
            site: "menu.example".into(),
            refused: String::new(),
            ..Default::default()
        });
        let entries = shell.agent_history.clone();
        shell.browser.set_agent_history(0, &write(&entries));
        shell.open_past_task("2", cx);
    });
    assert_eq!(
        page(&shell, cx),
        [
            "head 2",
            "Sites",
            "taberna.example Some(true)",
            "menu.example None",
            "Steps",
            "step read_page",
            "step click",
        ],
        "a site it didn't go to was listed, or its steps weren't newest first"
    );
    shell.update(cx, |shell, cx| shell.revoke_past_site(0, cx));
    assert_eq!(
        host.agent_answers.borrow()["claude-code"],
        [("elsewhere.example".to_string(), false)]
    );
    assert_eq!(page(&shell, cx)[2], "taberna.example None");
    // Revoke on a site that asks first does nothing.
    shell.update(cx, |shell, cx| shell.revoke_past_site(1, cx));
    assert_eq!(host.agent_answers.borrow()["claude-code"].len(), 1);
    shell.update(cx, |shell, cx| shell.close_past_task(cx));
    assert_eq!(listed(&shell, cx), ["clear", "2", "1"]);
    // A search from the page goes back to the list, to find in it.
    shell.update(cx, |shell, cx| shell.open_past_task("1", cx));
    assert_eq!(page(&shell, cx)[0], "head 1");
    let search = shell.read_with(cx, |shell, _| {
        shell.library.as_ref().unwrap().search.clone()
    });
    cx.update(|window, cx| {
        search.update(cx, |search, cx| {
            search.set_text("Task".into(), 4, window, cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(listed(&shell, cx), ["clear", "2", "1"]);
}

/// The node with `role` named `label` in what the window last told VoiceOver, if there's one.
fn named(
    cx: &mut VisualTestContext,
    role: gpui::accesskit::Role,
    label: &str,
) -> Option<(gpui::accesskit::NodeId, gpui::accesskit::Node)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    (tree.nodes.into_iter()).find(|(_, node)| node.role() == role && node.label() == Some(label))
}

/// A request among a past task's steps opens, as in Settings › Agents, to show where it went and
/// what it sent, eight lines at most until Show all, and closes again; a step that sent nothing
/// doesn't open (#2326).
#[gpui::test]
fn a_past_request_opens_to_what_it_sent(cx: &mut TestAppContext) {
    use crate::agents::request::Sent;
    use gpui::accesskit::{Action, Role};
    let (shell, _, cx) = agents(cx);
    let sent = (1..=10).map(|n| Sent {
        depth: 0,
        name: format!("field{n}"),
        value: n.to_string(),
    });
    shell.update(cx, |shell, cx| {
        let past = &mut shell.agent_history[0];
        past.steps.push(Step {
            at: past.ended - 10,
            action: "request".into(),
            site: "httpbin.org".into(),
            target: "POST /anything".into(),
            url: "https://httpbin.org/anything".into(),
            sent: sent.collect(),
            ..Default::default()
        });
        let entries = shell.agent_history.clone();
        shell.browser.set_agent_history(0, &write(&entries));
        shell.open_past_task("2", cx);
    });
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    let title = "Sent POST /anything to httpbin.org";
    let (request, node) = named(cx, Role::Button, title).expect("the request is a button");
    assert_eq!(node.is_expanded(), Some(false));
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let opens = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.is_expanded().is_some());
    assert_eq!(opens.count(), 1, "a step that sent nothing opens");
    cx.simulate_a11y_action(request, Action::Click);
    cx.run_until_parked();
    let (_, node) = named(cx, Role::Button, title).expect("the request is a button");
    assert_eq!(node.is_expanded(), Some(true));
    let (all, _) = named(cx, Role::Button, "Show all 10 lines").expect("ten lines take Show all");
    cx.simulate_a11y_action(all, Action::Click);
    cx.run_until_parked();
    assert!(named(cx, Role::Button, "Show all 10 lines").is_none());
    let (request, _) = named(cx, Role::Button, title).expect("the request is a button");
    cx.simulate_a11y_action(request, Action::Click);
    cx.run_until_parked();
    let (_, node) = named(cx, Role::Button, title).expect("the request is a button");
    assert_eq!(node.is_expanded(), Some(false));
}
