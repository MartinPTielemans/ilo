//! What VoiceOver and other assistive apps read of the sidebar: a role, a name and a state for
//! each thing in it, and the presses and menus the mouse has. GPUI builds the tree only while an
//! assistive app asks for it, and the shell names things only then (`Shell::a11y`), so with none
//! running a frame costs what it did.

use gpui::{
    A11ySubtreeBuilder, SharedString, StatefulInteractiveElement,
    accesskit::{Action, Node, NodeId, Rect, Role},
};

use super::Shell;

/// The `accessibility_id`s the list's rows go by, which say which group each belongs in.
pub(super) const NEW_TAB: &str = "new-tab";
const TAB: &str = "tab-";
const FOLDER: &str = "folder-";

pub(super) trait Accessible: StatefulInteractiveElement + Sized {
    /// Names it a button, as the tooltip does.
    fn button_named(self, name: impl Into<SharedString>) -> Self {
        self.role(Role::Button).aria_label(name)
    }

    /// Lists the press and, with `menu`, the menu its mouse-down listeners answer, which GPUI
    /// lists only for a click listener. GPUI answers them with a click or a right-click at its
    /// middle.
    fn pressed_by_mouse_down(self, press: bool, menu: bool) -> Self {
        self.a11y_synthetic_children(move |builder| {
            let node = builder.parent_node();
            if press {
                node.add_action(Action::Click);
            }
            if menu {
                node.add_action(Action::ShowContextMenu);
            }
        })
    }
}

impl<E: StatefulInteractiveElement> Accessible for E {}

impl Shell {
    /// What a tab is called: its title, then what it's doing, as Chrome's tab strip says it.
    pub(super) fn tab_label(&self, tab: u64) -> String {
        let page = self.page(tab);
        let states = [
            (page.audible && !page.muted, "playing audio"),
            (page.muted, "muted"),
            (page.camera, "using the camera"),
            (page.microphone, "using the microphone"),
            (page.sharing, "sharing"),
            (page.shared, "being shared"),
            (self.agent_at_work(tab), "agent working"),
            (page.loading, "loading"),
            (page.unloaded, "unloaded"),
        ];
        let mut label = self.title(tab).to_string();
        for (_, state) in states.iter().filter(|(on, _)| *on) {
            label.push_str(", ");
            label.push_str(state);
        }
        label
    }

    /// The `accessibility_id` of a tab's row, or a split view's segment for it.
    pub(super) fn tab_access_id(tab: u64) -> String {
        format!("{TAB}{tab}")
    }

    pub(super) fn folder_access_id(folder: u64) -> String {
        format!("{FOLDER}{folder}")
    }
}

/// Puts the list's rows in two groups VoiceOver reads as tab groups, Pinned before New Tab and
/// Today after it, each saying how many tabs it has and where each stands. The list draws its
/// rows flat, so that they move as one, and leaves itself out of the tree.
pub(super) fn group_rows(builder: &mut A11ySubtreeBuilder) {
    let children = builder.parent_node().children().to_vec();
    let mut order = Vec::new();
    let mut groups: [(&str, Vec<NodeId>); 2] = [("Pinned", Vec::new()), ("Today", Vec::new())];
    let mut after_new_tab = false;
    for child in children {
        let id = builder.built_node(child).and_then(|node| node.author_id().map(str::to_owned));
        let id = id.unwrap_or_default();
        if id == NEW_TAB {
            after_new_tab = true;
        }
        if !(id.starts_with(TAB) || id.starts_with(FOLDER)) {
            order.push(Err(child));
            continue;
        }
        let group = after_new_tab as usize;
        if groups[group].1.is_empty() {
            order.push(Ok(group));
        }
        groups[group].1.push(child);
    }
    let mut ids = [None, None];
    for (group, (name, members)) in groups.iter().enumerate() {
        if members.is_empty() {
            continue;
        }
        let tabs: Vec<_> = members
            .iter()
            .copied()
            .filter(|&member| builder.built_node(member).is_some_and(|n| n.role() == Role::Tab))
            .collect();
        for (index, &tab) in tabs.iter().enumerate() {
            let node = builder.built_node(tab).expect("listed above");
            node.set_position_in_set(index + 1);
            node.set_size_of_set(tabs.len());
        }
        let bounds = members
            .iter()
            .filter_map(|&member| builder.built_node(member).and_then(|node| node.bounds()))
            .reduce(|a, b| Rect {
                x0: a.x0.min(b.x0),
                y0: a.y0.min(b.y0),
                x1: a.x1.max(b.x1),
                y1: a.y1.max(b.y1),
            });
        let mut node = Node::new(Role::TabList);
        node.set_label(*name);
        node.set_children(members.clone());
        if let Some(bounds) = bounds {
            node.set_bounds(bounds);
        }
        let id = builder.synthetic_node_id(name);
        builder.push_child(id, node);
        ids[group] = Some(id);
    }
    let order: Vec<_> = order
        .into_iter()
        .filter_map(|each| match each {
            Ok(group) => ids[group],
            Err(child) => Some(child),
        })
        .collect();
    let list = builder.parent_node();
    list.set_children(order);
    // A container VoiceOver passes through, so its rows read as the sidebar's own.
    list.set_role(Role::GenericContainer);
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{
        Entity, TestAppContext, VisualTestContext,
        accesskit::{Action, Node, NodeId, Role, TreeUpdate},
    };

    use crate::shell::{Folders, Page, Shell, recorder::Recorder};

    /// The tree GPUI last gave the system, as an assistive app reads it.
    struct Tree(TreeUpdate);

    impl Tree {
        fn read(cx: &mut VisualTestContext) -> Self {
            Tree(cx.a11y_tree().expect("GPUI built a tree"))
        }

        fn node(&self, id: NodeId) -> &Node {
            let found = self.0.nodes.iter().find(|(each, _)| *each == id);
            &found.expect("every child is in the tree").1
        }

        fn named(&self, role: Role, label: &str) -> (NodeId, &Node) {
            let found = self.0.nodes.iter().find(|(_, node)| {
                node.role() == role && node.label() == Some(label)
            });
            let (id, node) = found.unwrap_or_else(|| panic!("no {role:?} called {label}"));
            (*id, node)
        }

        fn children(&self, id: NodeId) -> Vec<(Role, String)> {
            let children = self.node(id).children().iter().map(|&child| self.node(child));
            children.map(|node| (node.role(), node.label().unwrap_or("").to_owned())).collect()
        }

        /// The labels of the nodes under `id` and the groups among them, in order, as VoiceOver
        /// steps through them, which skips what holds them only.
        fn walk(&self, id: NodeId) -> Vec<String> {
            let mut out = Vec::new();
            for &child in self.node(id).children() {
                let node = self.node(child);
                if node.role() != Role::GenericContainer
                    && let Some(label) = node.label()
                {
                    out.push(label.to_owned());
                }
                if node.role() != Role::TabList {
                    out.extend(self.walk(child));
                }
            }
            out
        }
    }

    /// A shell with Docs, Mail and Notes pinned, Wiki in a closed folder Work after them, and
    /// News, Video and Search in Today, Video playing sound as it loads, and `shown` selected.
    fn shell(
        cx: &mut TestAppContext,
        shown: u64,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let folders = Folders {
            folders: [(9, ("Work".into(), false))].into(),
            parents: [(10, 9)].into(),
            ..Default::default()
        };
        shell.update(cx, |shell, cx| {
            let space = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, vec![1, 2, 3, 9, 10], vec![4, 5, 6], Vec::new(), folders, cx);
            for (tab, title) in
                [(1, "Docs"), (2, "Mail"), (3, "Notes"), (10, "Wiki"), (4, "News"), (5, "Video")]
            {
                shell.set_title(tab, title.into(), cx);
            }
            shell.set_title(6, "Search".into(), cx);
            let playing = Page { audible: true, loading: true, ..Page::default() };
            shell.set_page(5, playing, cx);
            shell.set_selected(shown, cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    /// Without an assistive app, the shell names nothing and GPUI sends no tree, so a frame costs
    /// what it did.
    #[gpui::test]
    fn no_tree_is_built_until_an_assistive_app_asks(cx: &mut TestAppContext) {
        let (shell, cx, _) = shell(cx, 2);
        assert!(!shell.read_with(cx, |shell, _| shell.a11y));
        assert!(cx.a11y_tree().is_none());
        cx.simulate_a11y_activation();
        assert!(shell.read_with(cx, |shell, _| shell.a11y));
        assert!(cx.a11y_tree().is_some());
    }

    /// VoiceOver reads the list as the user sees it: the Space's name, Pinned, New Tab and Today,
    /// each group a list of tabs that says which is selected and where each stands, the mock's
    /// "Mail, tab, selected, 2 of 3, Pinned". A tab says what it's doing after its title.
    #[gpui::test]
    fn the_rows_read_as_pinned_and_today_tab_groups(cx: &mut TestAppContext) {
        let (_, cx, _) = shell(cx, 2);
        cx.simulate_a11y_activation();
        let tree = Tree::read(cx);

        let (_, heading) = tree.named(Role::Heading, "Home");
        assert_eq!(heading.author_id(), Some("space-name"));
        let (pinned, _) = tree.named(Role::TabList, "Pinned");
        assert_eq!(
            tree.children(pinned),
            [
                (Role::Tab, "Docs".into()),
                (Role::Tab, "Mail".into()),
                (Role::Tab, "Notes".into()),
                (Role::Button, "Work".into()),
            ]
        );
        let (today, _) = tree.named(Role::TabList, "Today");
        assert_eq!(
            tree.children(today),
            [
                (Role::Tab, "News".into()),
                (Role::Tab, "Video, playing audio, loading".into()),
                (Role::Tab, "Search".into()),
            ]
        );

        let (_, mail) = tree.named(Role::Tab, "Mail");
        assert_eq!(mail.is_selected(), Some(true));
        assert_eq!((mail.position_in_set(), mail.size_of_set()), (Some(2), Some(3)));
        assert!(mail.supports_action(Action::Click));
        assert!(mail.supports_action(Action::ShowContextMenu));
        let (_, news) = tree.named(Role::Tab, "News");
        assert_eq!(news.is_selected(), Some(false));
        assert_eq!((news.position_in_set(), news.size_of_set()), (Some(1), Some(3)));

        let (_, work) = tree.named(Role::Button, "Work");
        assert_eq!(work.is_expanded(), Some(false));
        assert_eq!(work.role_description(), Some("folder"));

        let root = tree.0.tree.as_ref().expect("a first update has the root").root;
        let order = tree.walk(root);
        let at = |label: &str| order.iter().position(|each| each == label).unwrap();
        assert!(at("Home") < at("Pinned") && at("Pinned") < at("New Tab"));
        assert!(at("New Tab") < at("Today"), "{order:?}");
    }

    /// A tab in a call says what it's capturing, as its row's mark shows it, and the mark it
    /// shows is the share's while it shares, the camera's over the microphone's.
    #[gpui::test]
    fn a_tab_in_a_call_says_what_it_captures(cx: &mut TestAppContext) {
        let (shell, cx, _) = shell(cx, 2);
        let call = Page { camera: true, microphone: true, ..Page::default() };
        shell.update(cx, |shell, cx| shell.set_page(6, call.clone(), cx));
        cx.run_until_parked();
        cx.simulate_a11y_activation();
        let tree = Tree::read(cx);
        tree.named(Role::Tab, "Search, using the camera, using the microphone");

        let glyph = |page: Page| super::super::rows::capture_glyph(&page);
        assert_eq!(glyph(Page::default()), None);
        assert_eq!(glyph(Page { microphone: true, ..Page::default() }), Some("icons/mic.svg"));
        assert_eq!(glyph(call.clone()), Some("icons/video.svg"));
        let sharing = Page { sharing: true, ..call };
        assert_eq!(glyph(sharing), Some("icons/screen-share.svg"));
    }

    /// VoiceOver's press shows a tab, opens a folder, and VO-Shift-M shows a row's menu, as the
    /// mouse's do.
    #[gpui::test]
    fn a_rows_press_and_menu_answer_voiceover(cx: &mut TestAppContext) {
        let (shell, cx, host) = shell(cx, 2);
        cx.simulate_a11y_activation();
        let tree = Tree::read(cx);
        host.take();

        cx.simulate_a11y_action(tree.named(Role::Tab, "News").0, Action::Click);
        assert!(host.take().contains(&"select 4".to_owned()));

        cx.simulate_a11y_action(tree.named(Role::Button, "Work").0, Action::Click);
        assert_eq!(shell.read_with(cx, |shell, _| shell.folding), Some(true));

        cx.simulate_a11y_action(tree.named(Role::Tab, "Mail").0, Action::ShowContextMenu);
        let menus = host.menus.borrow();
        let titles: Vec<_> = menus.last().expect("a menu").iter().map(|(t, _)| t).collect();
        assert!(titles.iter().any(|title| title.contains("Unpin")), "{titles:?}");
    }

    /// The header's buttons say what they do, and back says it can't while there's no page to
    /// go back to.
    #[gpui::test]
    fn the_header_and_footer_buttons_are_named(cx: &mut TestAppContext) {
        let (_, cx, _) = shell(cx, 2);
        cx.simulate_a11y_activation();
        let tree = Tree::read(cx);
        let (_, back) = tree.named(Role::Button, "Go Back");
        assert!(back.is_disabled());
        let (_, sidebar) = tree.named(Role::Button, "Hide Sidebar");
        assert!(!sidebar.is_disabled() && sidebar.supports_action(Action::Click));
        let (_, plus) = tree.named(Role::Button, "New Space, Folder or Note");
        assert!(plus.supports_action(Action::Click));
        tree.named(Role::Button, "View Library");
        tree.named(Role::Button, "New Tab");
        tree.named(Role::Button, "Clear Today");
    }
}
