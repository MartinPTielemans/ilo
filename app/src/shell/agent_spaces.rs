//! An agent's Space in the UI, which shows and switches like the user's, and which the user's tabs
//! move into only as the user gives the agent one, on a host that records what the shell asks.

use std::{rc::Rc, time::Duration};

use gpui::{AppContext, Bounds, Entity, SharedString, TestAppContext, point, px, size};

use crate::shell::{AgentGrants, Command, Folders, Shell, recorder::Recorder};

/// The user's Space 1 with tab 10, shown, and Claude Code's Space 2 with tab 20, and favorite 30.
fn shell_with_agent(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
            ],
            cx,
        );
        shell.set_favorites(vec![(30, 0)], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    (shell, host)
}

/// The items of the menu a right-click on `tab`'s row shows.
fn tab_menu(
    shell: &Entity<Shell>,
    host: &Recorder,
    cx: &mut TestAppContext,
    tab: u64,
) -> Vec<(SharedString, bool)> {
    shell.update(cx, |shell, cx| {
        shell.tab_menu(tab, point(px(100.), px(200.)), cx)
    });
    host.menus.borrow().last().cloned().unwrap_or_default()
}

fn item(menu: &[(SharedString, bool)], title: &str) -> Option<bool> {
    menu.iter()
        .find(|(each, _)| each == title)
        .map(|&(_, enabled)| enabled)
}

/// The Spaces menu lists the agent's Space among the user's, ⌃2 shows it as it does any second
/// Space, and ⌃1 shows the user's again.
#[gpui::test]
fn an_agents_space_shows_and_switches_like_the_users(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_agent(cx);
    assert_eq!(host.menu_bar.borrow().spaces, ["Space 1", "Research"]);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(1), "ui", cx));
    assert_eq!(*host.shown.borrow(), [2]);
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.run(Command::SelectSpace(0), "ui", cx);
    });
    assert_eq!(*host.shown.borrow(), [2, 1]);
}

/// A tab of the user's Move to lists the agent's Space after theirs, which gives the agent the tab,
/// but not an agent's Space in another profile, where the page would load again without its
/// logins. A tab of the agent's can come to the user's and not go to another agent's, and a
/// favorite can't be taken out of the favorites into the agent's Space.
#[gpui::test]
fn the_users_tab_moves_into_an_agents_space_only_as_a_gift(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_agent(cx);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
                (
                    3,
                    "Elsewhere".into(),
                    None,
                    1,
                    "".into(),
                    "Codex".into(),
                    "".into(),
                ),
            ],
            cx,
        );
    });
    let menu = tab_menu(&shell, &host, cx, 10);
    assert_eq!(item(&menu, "Space 1"), Some(true));
    assert_eq!(item(&menu, "Research"), Some(true));
    assert_eq!(item(&menu, "Elsewhere"), None);
    host.take();
    cx.update(|cx| host.choose("Research", cx));
    assert_eq!(host.take(), ["give 10 to 2"]);
    assert_eq!(
        item(&tab_menu(&shell, &host, cx, 30), "Remove from Favorites"),
        Some(true)
    );
    assert_eq!(item(&tab_menu(&shell, &host, cx, 30), "Research"), None);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    let menu = tab_menu(&shell, &host, cx, 20);
    assert_eq!(item(&menu, "Move to"), Some(true));
    assert_eq!(item(&menu, "Research"), None);
    assert_eq!(item(&menu, "Elsewhere"), None);
    assert_eq!(item(&menu, "Space 1"), Some(true));
    assert_eq!(
        item(&tab_menu(&shell, &host, cx, 30), "Remove from Favorites"),
        Some(false)
    );
}

/// An agent is at work in the tab the activity log last let it act in, which its row's label
/// says while the agent's Space is shown, and not in the tab it acted in before. A tab the user
/// takes into a Space of theirs no longer shows it.
#[gpui::test]
fn an_agents_row_says_its_at_work_where_it_last_acted(cx: &mut TestAppContext) {
    let (shell, _) = shell_with_agent(cx);
    let at_work = |shell: &Entity<Shell>, cx: &mut TestAppContext, tab| {
        shell.update(cx, |shell, _| {
            shell.tab_label(tab).ends_with(", agent working")
        })
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            2,
            Vec::new(),
            vec![20, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(2, cx);
    });
    assert!(!at_work(&shell, cx, 20));
    shell.update(cx, |shell, cx| {
        shell.agent_acted("claude-code".into(), 20, cx)
    });
    assert!(at_work(&shell, cx, 20));
    shell.update(cx, |shell, cx| {
        shell.agent_acted("claude-code".into(), 21, cx)
    });
    assert!(!at_work(&shell, cx, 20));
    assert!(at_work(&shell, cx, 21));

    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
    });
    assert!(!at_work(&shell, cx, 21));
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    assert!(!at_work(&shell, cx, 21), "in the user's Space, it's theirs");
}

/// The core ending an agent's session with a tab, as its client disconnects, it releases the tab
/// or the user takes the tab back, takes that agent's mark off that tab and nowhere else: not
/// another agent's, nor the agent's own in a tab it has since moved on to. A tab that closes
/// still loses its mark without one, and an end that comes after changes nothing.
#[gpui::test]
fn an_agents_row_stops_saying_its_at_work_once_its_session_ends(cx: &mut TestAppContext) {
    let (shell, _) = shell_with_agent(cx);
    let at_work = |shell: &Entity<Shell>, cx: &mut TestAppContext, tab| {
        shell.update(cx, |shell, _| {
            shell.tab_label(tab).ends_with(", agent working")
        })
    };
    let acted = |shell: &Entity<Shell>, cx: &mut TestAppContext, agent: &'static str, tab| {
        shell.update(cx, |shell, cx| shell.agent_acted(agent.into(), tab, cx))
    };
    let ended = |shell: &Entity<Shell>, cx: &mut TestAppContext, agent: &str, tab| {
        shell.update(cx, |shell, cx| shell.agent_session_ended(agent, tab, cx))
    };
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
                (
                    3,
                    "Errands".into(),
                    None,
                    0,
                    "".into(),
                    "Codex".into(),
                    "".into(),
                ),
            ],
            cx,
        );
        shell.set_tabs(
            2,
            Vec::new(),
            vec![20, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_tabs(3, Vec::new(), vec![40], Vec::new(), Folders::default(), cx);
        shell.set_space(2, cx);
    });
    acted(&shell, cx, "claude-code", 20);
    acted(&shell, cx, "codex", 40);
    ended(&shell, cx, "codex", 20);
    assert!(
        at_work(&shell, cx, 20),
        "another agent's end isn't this one's"
    );
    ended(&shell, cx, "claude-code", 21);
    assert!(
        at_work(&shell, cx, 20),
        "an end in another tab isn't this one's"
    );
    ended(&shell, cx, "claude-code", 20);
    assert!(!at_work(&shell, cx, 20));
    shell.update(cx, |shell, cx| shell.set_space(3, cx));
    assert!(at_work(&shell, cx, 40), "the other agent is still at work");
    ended(&shell, cx, "codex", 40);
    assert!(!at_work(&shell, cx, 40));

    // The agent moved on to 21 before its session with 20 ended.
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    acted(&shell, cx, "claude-code", 20);
    acted(&shell, cx, "claude-code", 21);
    ended(&shell, cx, "claude-code", 20);
    assert!(at_work(&shell, cx, 21));
    // Acting again after an end puts the mark back.
    ended(&shell, cx, "claude-code", 21);
    acted(&shell, cx, "claude-code", 21);
    assert!(at_work(&shell, cx, 21));

    // 21 closes: its mark goes, and the end that follows is no news.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx)
    });
    assert!(!at_work(&shell, cx, 21));
    ended(&shell, cx, "claude-code", 21);
    acted(&shell, cx, "claude-code", 20);
    assert!(at_work(&shell, cx, 20));
}

/// An agent's Space's menu leads to what the agent did, in Settings › Agents, and the user's
/// Space's has no such item.
#[gpui::test]
fn an_agents_space_menu_shows_its_activity(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_agent(cx);
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(100.), px(200.)), cx)
    });
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    assert_eq!(item(&menu, "Agent Activity…"), None);
    shell.update(cx, |shell, cx| {
        shell.space_menu(2, point(px(100.), px(200.)), cx)
    });
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    assert_eq!(item(&menu, "Agent Activity…"), Some(true));
    host.take();
    cx.update(|cx| host.choose("Agent Activity…", cx));
    assert_eq!(host.take(), ["agent activity"]);
}

/// An agent's Space's menu lists what the user answered it for good on each site under Revoke
/// Permission, each to revoke, and all at once for several, Take Back All Tabs while it has a tab
/// the user doesn't hold, and Disconnect while it's connected.
#[gpui::test]
fn an_agents_space_menu_lists_its_permissions_to_revoke(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_agent(cx);
    let menu = |cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| {
            shell.space_menu(2, point(px(100.), px(200.)), cx)
        });
        host.menus.borrow().last().cloned().unwrap_or_default()
    };
    let menu_now = menu(cx);
    assert_eq!(item(&menu_now, "Revoke Permission"), Some(false));
    assert_eq!(item(&menu_now, "Disconnect Claude Code"), Some(false));
    cx.update(|cx| host.dismiss(cx));

    assert_eq!(item(&menu_now, "Take Back All Tabs"), Some(false));
    let sites = vec![
        ("bank.example".into(), false),
        ("shop.example".into(), true),
    ];
    *host.agent_permissions.borrow_mut() = AgentGrants {
        sites,
        connected: true,
        tabs: true,
    };
    let menu_now = menu(cx);
    assert_eq!(item(&menu_now, "Revoke Permission"), Some(true));
    assert_eq!(item(&menu_now, "Never on bank.example"), Some(true));
    assert_eq!(item(&menu_now, "Always Allow on shop.example"), Some(true));
    assert_eq!(item(&menu_now, "All Sites"), Some(true));
    host.take();
    cx.update(|cx| host.choose("Always Allow on shop.example", cx));
    assert_eq!(host.take(), ["revoke agent 2 shop.example"]);
    menu(cx);
    cx.update(|cx| host.choose("All Sites", cx));
    assert_eq!(host.take(), ["revoke agent 2 all"]);
    menu(cx);
    cx.update(|cx| host.choose("Disconnect Claude Code", cx));
    assert_eq!(host.take(), ["disconnect agent 2"]);
    menu(cx);
    cx.update(|cx| host.choose("Take Back All Tabs", cx));
    assert_eq!(host.take(), ["take back agent 2"]);

    let sites = vec![("shop.example".into(), true)];
    *host.agent_permissions.borrow_mut() = AgentGrants {
        sites,
        connected: true,
        tabs: true,
    };
    assert_eq!(item(&menu(cx), "All Sites"), None, "one site had All Sites");
}

/// The agent's pointer comes to an element it acts on in the page the window shows in its Space,
/// and not in a tab of its the window doesn't show or while the user's Space is shown. It goes a
/// moment after the agent's last action.
#[gpui::test]
fn the_agents_pointer_shows_where_it_acts_in_the_page_shown(cx: &mut TestAppContext) {
    let (shell, _) = shell_with_agent(cx);
    let element = Bounds::new(point(px(40.), px(60.)), size(px(120.), px(30.)));
    let point_at = |shell: &Entity<Shell>, cx: &mut TestAppContext, tab| {
        shell.update(cx, |shell, cx| {
            shell.agent_pointing(tab, "e3".into(), element, cx);
            shell.agent_pointer_shown()
        })
    };
    assert!(!point_at(&shell, cx, 20), "the user's Space is shown");
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            2,
            Vec::new(),
            vec![20, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(2, cx);
        shell.set_selected(20, cx);
    });
    assert!(!point_at(&shell, cx, 21), "the window doesn't show tab 21");
    assert!(point_at(&shell, cx, 20));
    cx.executor().advance_clock(Duration::from_secs(1));
    assert!(shell.update(cx, |shell, _| shell.agent_pointer_shown()));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(!shell.update(cx, |shell, _| shell.agent_pointer_shown()));
}
