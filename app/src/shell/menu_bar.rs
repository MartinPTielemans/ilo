//! What the menu bar shows that changes with the window, and the window's title, which the Window
//! menu lists it by (docs/ux-spec.md › Menus).

use gpui::{App, Context, SharedString};

use super::{Command, Extension, Shell, menus::space_menu_label, trace};
use crate::theme::Appearance;

/// What the key window's menu bar shows that changes with it.
#[derive(Clone, Default, PartialEq)]
pub struct MenuBar {
    /// The Spaces' names, each after its icon if it has one, for the Spaces menu.
    pub spaces: Vec<SharedString>,
    /// The shown Space's profile's extensions that run in the window, for the Extensions menu.
    pub extensions: Vec<Extension>,
    pub sidebar_hidden: bool,
    /// Whether the tab shown is pinned, or None with none shown.
    pub pinned: Option<bool>,
    /// Whether the Space shown has its pinned rows collapsed.
    pub pinned_shut: bool,
    /// Whether View's pinned tabs item would show or hide a row now, else it's dimmed.
    pub pinned_offered: bool,
    /// Whether the page shown is in Developer Mode, or None where it can't change: no web page, or
    /// a private window.
    pub developer_mode: Option<bool>,
    /// Whether a trace is recording, which Help › Troubleshooting's item stops.
    pub tracing: bool,
    /// Spaces › Change Profile: each profile by id and name, whether the Space shown can change to
    /// it and whether it's in it, then whether New Profile can be chosen. None in a private window.
    pub profiles: Option<(Vec<ProfileChoice>, bool)>,
    /// View › Appearance's choice, which it checks.
    pub appearance: Appearance,
}

/// A profile a Space can be in, as its menus list it.
#[derive(Clone, PartialEq, Debug)]
pub struct ProfileChoice {
    pub profile: u64,
    pub name: SharedString,
    pub enabled: bool,
    pub checked: bool,
}

impl MenuBar {
    /// View's item for ⌘S, by what it does now, as Arc's.
    pub fn sidebar_title(&self) -> &'static str {
        if self.sidebar_hidden { "Show Sidebar" } else { "Hide Sidebar" }
    }

    /// Tabs' item for ⌘D, by the tab shown, as Arc's, which names both with none shown.
    pub fn pin_title(&self) -> &'static str {
        match self.pinned {
            Some(true) => "Unpin Tab",
            Some(false) => "Pin Tab",
            None => "Pin/Unpin Tab",
        }
    }

    /// View's item that collapses the pinned rows or expands them, by what it does now, after the
    /// sidebar's, as Arc's.
    pub fn pinned_item(&self) -> (&'static str, Command) {
        match self.pinned_shut {
            true => ("Expand Pinned Tabs", Command::ExpandPinned),
            false => ("Collapse Pinned Tabs", Command::CollapsePinned),
        }
    }

    /// View › Developer's item for the page's Developer Mode, by what it would do, as the command
    /// bar names it. None leaves it out, as Arc's shows only with a page.
    pub fn developer_mode_title(&self) -> Option<&'static str> {
        self.developer_mode.map(super::commands::developer_mode_title)
    }

    /// Help › Troubleshooting's item for the trace, by what it would do, as the command bar names
    /// it.
    pub fn trace_title(&self) -> &'static str {
        trace::title(self.tracing)
    }
}

impl Shell {
    /// What the menu bar shows for this window now.
    pub(super) fn menu_bar(&self, cx: &App) -> MenuBar {
        let profile = self.shown_space().map_or(0, |space| space.profile);
        // A private window lists no Spaces, as its one Space isn't one of them, and only the
        // extensions the user allowed in Incognito, which Chrome runs in a private session.
        let extensions = (self.extensions.get(&profile).into_iter().flatten())
            .filter(|extension| extension.runs(self.private))
            .cloned()
            .collect();
        let spaces = (self.spaces.iter())
            .filter(|_| !self.private)
            .map(|space| space_menu_label(&space.icon, &space.name))
            .collect();
        let pinned = (self.selected != 0).then(|| self.pinned().contains(&self.selected));
        MenuBar {
            spaces,
            extensions,
            sidebar_hidden: self.sidebar_hidden,
            pinned,
            pinned_shut: self.shown_space().is_some_and(|space| space.folders.pinned_shut),
            pinned_offered: self.offers(Command::CollapsePinned)
                || self.offers(Command::ExpandPinned),
            developer_mode: self.developer_mode(self.selected),
            tracing: trace::recording(cx),
            profiles: self.profile_choices(self.space).filter(|_| !self.private),
            appearance: self.app_appearance,
        }
    }

    /// Gives the menu bar what it shows now, once that changes, so most tab switches and page
    /// updates leave the menus as they are.
    pub(super) fn update_menus(&mut self, cx: &mut Context<Self>) {
        let bar = self.menu_bar(cx);
        if bar == self.menus {
            return;
        }
        if bar.extensions != self.menus.extensions {
            // check.sh waits for this line.
            let ids: Vec<&str> = bar.extensions.iter().map(|each| each.id.as_ref()).collect();
            log!("browser: extensions menu {}", ids.join(" "));
        }
        self.browser.set_menus(&bar, cx);
        self.menus = bar;
    }

    /// Gives the menu bar this window's menus again, with the keys the commands have now.
    pub(super) fn refresh_menus(&mut self, cx: &mut Context<Self>) {
        // The menu bar is the key window's, so its Undo and Redo name this window's steps.
        self.send_edit_titles(true, cx);
        self.menus = self.menu_bar(cx);
        self.browser.set_menus(&self.menus, cx);
    }

    /// Titles the window by the tab shown, as the sidebar names it, or by its Space with none
    /// shown (Arc), once that changes. AppKit lists the window in the Window menu and Mission
    /// Control by it; the window doesn't draw it.
    pub fn update_title(&mut self, cx: &mut Context<Self>) {
        let title = match self.selected {
            0 => self.shown_space().map(|space| space.name.clone()).unwrap_or_default(),
            tab => self.title(tab),
        };
        if title != self.window_title {
            // check.sh reads this line.
            log!("browser: window title {title}");
            self.browser.set_window_title(&title, cx);
            self.window_title = title;
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::ProfileChoice;
    use crate::shell::{Command, Folders, Page, drag::detents::shell_with};

    /// Tabs › Pin Tab is named by the tab shown, and with none both ways; View's sidebar and pinned
    /// tabs items by what they do now.
    #[gpui::test]
    fn the_menu_bar_names_items_by_what_they_do_now(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        assert_eq!(host.menu_bar.borrow().pin_title(), "Pin/Unpin Tab");
        assert_eq!(host.menu_bar.borrow().sidebar_title(), "Hide Sidebar");
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        assert_eq!(host.menu_bar.borrow().pin_title(), "Pin Tab");
        shell.update(cx, |shell, cx| shell.set_selected(1, cx));
        assert_eq!(host.menu_bar.borrow().pin_title(), "Unpin Tab");
        shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
        assert_eq!(host.menu_bar.borrow().sidebar_title(), "Show Sidebar");
        assert_eq!(host.menu_bar.borrow().pinned_item().1, Command::CollapsePinned);
        let shut = Folders { pinned_shut: true, ..Folders::default() };
        shell.update(cx, |shell, cx| shell.set_tabs(1, vec![1], vec![10], Vec::new(), shut, cx));
        let item = host.menu_bar.borrow().pinned_item();
        assert_eq!(item, ("Expand Pinned Tabs", Command::ExpandPinned));
    }

    /// Tabs › Pin Tab pins the tab shown while its page loads, as the menu sends it to the shell
    /// whatever the page is doing (#1289).
    #[gpui::test]
    fn the_menu_bar_pins_a_loading_page(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        cx.update(crate::shell::bind_keys);
        let loading = Page { url: "https://example.com/".into(), loading: true, ..Page::default() };
        shell.update(cx, |shell, cx| {
            shell.set_selected(10, cx);
            shell.set_page(10, loading, cx);
        });
        assert_eq!(host.menu_bar.borrow().pin_title(), "Pin Tab");
        host.take_edits();
        cx.update(|cx| cx.dispatch_action(&crate::shell::Run(Command::TogglePin)));
        assert_eq!(host.take_edits(), ["pinned 10 true"]);
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![1, 10], Vec::new(), Vec::new(), Folders::default(), cx)
        });
        assert_eq!(host.menu_bar.borrow().pin_title(), "Unpin Tab");
        cx.update(|cx| cx.dispatch_action(&crate::shell::Run(Command::TogglePin)));
        assert_eq!(host.take_edits(), ["pinned 10 false"]);
    }

    /// View › Developer turns the page's Developer Mode the other way, named by what it would do,
    /// and is left out for a page that has none, as Arc's shows only with a page.
    #[gpui::test]
    fn developer_mode_s_item_follows_the_page(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10], Folders::default());
        let title = || host.menu_bar.borrow().developer_mode_title();
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        assert_eq!(title(), None);
        let page = |url: &str, developer_mode| Page {
            url: url.to_string().into(),
            developer_mode,
            ..Page::default()
        };
        shell.update(cx, |shell, cx| shell.set_page(10, page("http://localhost:3000/", true), cx));
        assert_eq!(title(), Some("Turn off Developer Mode for this site"));
        shell.update(cx, |shell, cx| shell.set_page(10, page("http://localhost:3000/", false), cx));
        assert_eq!(title(), Some("Turn on Developer Mode for this site"));
        shell.update(cx, |shell, cx| shell.set_page(10, page("chrome://settings/", false), cx));
        assert_eq!(title(), None);
    }

    /// The window is titled by the tab shown, as the sidebar names it, and by its Space with none.
    #[gpui::test]
    fn the_window_is_titled_by_the_tab_shown(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10], Folders::default());
        assert_eq!(*host.window_title.borrow(), "Work");
        shell.update(cx, |shell, cx| {
            shell.set_title(10, "Example Domain".into(), cx);
            shell.set_selected(10, cx);
        });
        assert_eq!(*host.window_title.borrow(), "Example Domain");
        shell.update(cx, |shell, cx| shell.set_title(10, "Example".into(), cx));
        assert_eq!(*host.window_title.borrow(), "Example");
    }

    /// Spaces › Change Profile checks the Space's profile and offers the others, and New Profile,
    /// only while the Space has no tabs, as its menu's Profile items do.
    #[gpui::test]
    fn change_profile_offers_what_the_spaces_menu_does(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10], Folders::default());
        shell.update(cx, |shell, cx| {
            shell.set_profiles(vec![(0, "Default".into()), (5, "Work".into())], cx)
        });
        let choice = |profile, name: &str, enabled, checked| ProfileChoice {
            profile,
            name: name.to_string().into(),
            enabled,
            checked,
        };
        let full = vec![choice(0, "Default", true, true), choice(5, "Work", false, false)];
        assert_eq!(host.menu_bar.borrow().profiles, Some((full, false)));
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, Vec::new(), Vec::new(), Vec::new(), Folders::default(), cx)
        });
        let empty = vec![choice(0, "Default", true, true), choice(5, "Work", true, false)];
        assert_eq!(host.menu_bar.borrow().profiles, Some((empty, true)));
    }
}
