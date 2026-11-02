//! Help › Restore Earlier Sidebar: the core backs up each profile's sidebar as it saves the Spaces,
//! and the menu lists the shown Space's profile's backups for the user to bring one back
//! (docs/ux-spec.md › Menus › Help), as the command bar's Restore Earlier Sidebar… does where
//! there's no menu bar.

use gpui::{Context, point, px};

use super::{MenuEntry, Shell, SidebarBackup, menus::MenuAction};
use crate::command_bar;

impl Shell {
    /// The core listed the profile's sidebar backups, newest first.
    pub fn set_sidebar_backups(
        &mut self,
        profile: u64,
        backups: Vec<SidebarBackup>,
        cx: &mut Context<Self>,
    ) {
        if let Some(newest) = backups.first() {
            // check.sh chooses it by this line.
            log!("browser: newest sidebar backup {profile} {}", newest.label);
        }
        self.sidebar_backups.insert(profile, backups);
        self.update_menus(cx);
    }

    /// The shown Space's profile's backups, newest first; none in an agent's Space.
    pub(super) fn shown_backups(&self) -> &[SidebarBackup] {
        let shown = self.shown_space().filter(|space| space.agent.is_empty());
        let backups = shown.and_then(|space| self.sidebar_backups.get(&space.profile));
        backups.map(Vec::as_slice).unwrap_or_default()
    }

    /// The command bar's Restore Earlier Sidebar…, for Linux and Windows, which show no menu bar:
    /// the backups in ilo's menu, newest first, where the bar was, each asking as Help's does.
    pub(super) fn list_sidebar_backups(&mut self, cx: &mut Context<Self>) {
        let items: Vec<(MenuEntry, MenuAction)> = (self.shown_backups().iter())
            .map(|backup| {
                let time = backup.time;
                let restore: MenuAction =
                    Box::new(move |shell, cx| shell.restore_sidebar(time, cx));
                (MenuEntry::named(backup.label.clone(), true), restore)
            })
            .collect();
        if items.is_empty() {
            return;
        }
        let left = (f32::from(self.viewport.width) - command_bar::WIDTH).max(0.) / 2.;
        let top = command_bar::centered_top(f32::from(self.viewport.height));
        self.show_menu(items, point(px(left), px(top)), None, cx);
    }

    /// Brings back the backup taken at `time` once the user confirms, as it replaces the whole
    /// sidebar, though the one it replaces stays in the same menu.
    pub(super) fn restore_sidebar(&mut self, time: u64, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space().filter(|space| space.agent.is_empty()) else {
            return;
        };
        let profile = space.profile;
        let Some(backup) = (self.sidebar_backups.get(&profile).into_iter().flatten())
            .find(|backup| backup.time == time)
        else {
            return;
        };
        // Named only where there's another profile it could be.
        let whose = match self.profiles.iter().find(|(id, _)| *id == profile) {
            Some((_, name)) if self.profiles.len() > 1 => format!("the “{name}” profile's"),
            _ => "this profile's".into(),
        };
        let shell = cx.weak_entity();
        self.browser.confirm(
            format!("Restore the sidebar from {}?", backup.label),
            format!(
                "Its Spaces, with their tabs, take the place of {whose}. The sidebar as it is now \
                 is kept as a backup, in the same menu."
            ),
            "Restore",
            Box::new(move |cx| {
                shell
                    .update(cx, |shell, cx| {
                        // check.sh waits for this line.
                        log!("browser: restore sidebar {time}");
                        shell.browser.restore_sidebar(time, cx);
                    })
                    .ok();
            }),
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, TestAppContext};

    use crate::shell::{
        Command, Folders, Shell, SidebarBackup, drag::detents::shell_with, recorder::Recorder,
    };

    fn backup(time: u64, label: &str) -> SidebarBackup {
        SidebarBackup { time, label: label.to_string().into() }
    }

    /// Help lists the shown Space's profile's backups, and one chosen is restored once the user
    /// confirms, never before.
    #[gpui::test]
    fn a_backup_chosen_from_help_is_restored_once_confirmed(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        assert!(host.menu_bar.borrow().sidebar_backups.is_empty());
        let backups = vec![backup(2000, "Thursday 29 October, 14:32"), backup(1000, "Wednesday")];
        shell.update(cx, |shell, cx| shell.set_sidebar_backups(0, backups.clone(), cx));
        assert_eq!(host.menu_bar.borrow().sidebar_backups, backups);
        // Another profile's aren't this Space's.
        shell.update(cx, |shell, cx| shell.set_sidebar_backups(7, vec![backup(5, "x")], cx));
        assert_eq!(host.menu_bar.borrow().sidebar_backups, backups);

        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::RestoreSidebar(2000), "ui", cx));
        let (message, detail, confirmed) = host.confirming.borrow_mut().take().unwrap();
        assert_eq!(message, "Restore the sidebar from Thursday 29 October, 14:32?");
        assert!(detail.contains("this profile's"), "{detail}");
        assert!(!host.take().iter().any(|call| call.starts_with("restore sidebar")));
        cx.update(confirmed);
        assert!(host.take().contains(&"restore sidebar 2000".to_string()));

        // One the core no longer lists asks nothing.
        shell.update(cx, |shell, cx| shell.run(Command::RestoreSidebar(3000), "ui", cx));
        assert!(host.confirming.borrow().is_none());
    }

    /// Where there's no menu bar, the command bar offers Restore Earlier Sidebar… once there's a
    /// backup, and lists the profile's by their labels, newest first. One chosen asks first.
    #[gpui::test]
    fn the_command_bar_lists_the_backups_and_restores_one_once_confirmed(
        cx: &mut TestAppContext,
    ) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        let offered = |cx: &mut TestAppContext| {
            shell.read_with(cx, |shell, _| shell.offers(Command::RestoreEarlierSidebar))
        };
        assert!(!offered(cx), "nothing to restore yet");
        let backups = vec![backup(2000, "Thursday 29 October, 14:32"), backup(1000, "Wednesday")];
        shell.update(cx, |shell, cx| shell.set_sidebar_backups(0, backups, cx));
        assert!(offered(cx));
        let keys = Default::default();
        let found = crate::shell::find_commands("restore earl", &keys, |name, _| Some(name));
        assert_eq!(found[0].command, Command::RestoreEarlierSidebar);
        assert_eq!(found[0].name.as_ref(), "Restore Earlier Sidebar…");

        host.take();
        let run = Command::RestoreEarlierSidebar;
        shell.update(cx, |shell, cx| shell.run(run, "command bar", cx));
        let listed = host.menus.borrow().last().cloned().unwrap();
        let titles: Vec<&str> = listed.iter().map(|(title, _)| title.as_ref()).collect();
        assert_eq!(titles, ["Thursday 29 October, 14:32", "Wednesday"]);
        cx.update(|cx| host.choose("Wednesday", cx));
        let (message, _, confirmed) = host.confirming.borrow_mut().take().unwrap();
        assert_eq!(message, "Restore the sidebar from Wednesday?");
        assert!(!host.take().iter().any(|call| call.starts_with("restore sidebar")));
        cx.update(confirmed);
        assert!(host.take().contains(&"restore sidebar 1000".to_string()));
    }

    /// An agent's Space, though in the profile, offers no restore: the restore replaces the user's
    /// Spaces, and the agent works in its own.
    #[gpui::test]
    fn an_agents_space_offers_no_restore(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![
                    (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                    (2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
                ],
                cx,
            );
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
            shell.set_sidebar_backups(0, vec![backup(2000, "Thursday")], cx);
            shell.set_space(2, cx);
        });
        let offered = |cx: &mut TestAppContext| {
            shell.read_with(cx, |shell, _| shell.offers(Command::RestoreEarlierSidebar))
        };
        assert!(!offered(cx));
        assert!(host.menu_bar.borrow().sidebar_backups.is_empty());
        let menus = host.menus.borrow().len();
        shell.update(cx, |shell, cx| shell.run(Command::RestoreEarlierSidebar, "ui", cx));
        shell.update(cx, |shell, cx| shell.run(Command::RestoreSidebar(2000), "ui", cx));
        assert_eq!(host.menus.borrow().len(), menus, "no list in the agent's Space");
        assert!(host.confirming.borrow().is_none());

        shell.update(cx, |shell, cx| shell.set_space(1, cx));
        assert!(offered(cx));
        assert_eq!(host.menu_bar.borrow().sidebar_backups.len(), 1);
    }
}
