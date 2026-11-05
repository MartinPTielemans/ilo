//! What AppleScript reads of a window and asks of it (#704, scripting_host.rs in the app): the
//! user's Spaces and their tabs as the switcher and the sidebar list them, never an agent's, and a
//! Space or a tab shown as a click on it shows it.

use gpui::{Context, SharedString};

use super::{Shell, commands::space_tabs};

impl Shell {
    /// The user's Spaces, as the switcher lists them.
    pub fn script_spaces(&self) -> Vec<u64> {
        (self.spaces.iter())
            .filter(|space| space.agent.is_empty())
            .map(|space| space.id)
            .collect()
    }

    /// The name of the user's Space `space`, or None for any other.
    pub fn script_space_name(&self, space: u64) -> Option<SharedString> {
        (self.spaces.iter())
            .find(|each| each.id == space && each.agent.is_empty())
            .map(|space| space.name.clone())
    }

    /// The Space shown, or 0 unless it's one of the user's.
    pub fn script_space(&self) -> u64 {
        let shown = self.space;
        match self.script_space_name(shown) {
            Some(_) => shown,
            None => 0,
        }
    }

    /// The tabs of the user's Space `space`, or with 0 of the Space shown, pinned then Today, as
    /// the sidebar lists them; none for an agent's.
    pub fn script_tabs(&self, space: u64) -> Vec<u64> {
        let space = if space == 0 { self.space } else { space };
        (self.spaces.iter())
            .find(|each| each.id == space && each.agent.is_empty())
            .map_or_else(Vec::new, |space| space_tabs(space).collect())
    }

    /// Shows the user's Space `space`, as a click in the switcher does. False for any other.
    pub fn script_show_space(&mut self, space: u64, cx: &mut Context<Self>) -> bool {
        if self.script_space_name(space).is_none() {
            return false;
        }
        self.show_space(space, cx);
        true
    }

    /// Shows `tab`, in one of the user's Spaces, with its Space, as a click on its row does. False
    /// for a tab of none of them.
    pub fn script_select(&mut self, tab: u64, cx: &mut Context<Self>) -> bool {
        let owner = (self.spaces.iter())
            .find(|space| space.agent.is_empty() && space_tabs(space).any(|each| each == tab))
            .map(|space| space.id);
        let Some(owner) = owner else {
            return false;
        };
        self.show_space(owner, cx);
        self.select(tab, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext, TestAppContext};

    use crate::shell::{Folders, Shell, recorder::Recorder};

    /// The user's Work and Home, and an agent's Space, each with tabs.
    fn shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, std::rc::Rc<Recorder>) {
        let host = std::rc::Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64, name: &str, agent: &str| {
                let (name, agent) = (name.to_string().into(), agent.to_string().into());
                (id, name, None, 0, "".into(), agent, "".into())
            };
            shell.set_spaces(
                vec![
                    space(1, "Work", ""),
                    space(2, "Home", ""),
                    space(3, "Claude Code", "claude-code"),
                ],
                cx,
            );
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                vec![10],
                vec![11, 12],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
            shell.set_tabs(3, Vec::new(), vec![30], Vec::new(), Folders::default(), cx);
        });
        (shell, host)
    }

    /// A script sees the user's Spaces and their tabs, pinned then Today, and none of an agent's.
    #[gpui::test]
    fn a_script_reads_the_users_spaces_and_tabs_and_no_agents(cx: &mut TestAppContext) {
        let (shell, _) = shell(cx);
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.script_spaces(), vec![1, 2]);
            assert_eq!(shell.script_space(), 1);
            assert_eq!(shell.script_tabs(0), vec![10, 11, 12]);
            assert_eq!(shell.script_tabs(2), vec![20]);
            assert!(
                shell.script_tabs(3).is_empty(),
                "an agent's tabs were listed"
            );
            assert_eq!(shell.script_space_name(2).as_deref(), Some("Home"));
            assert!(shell.script_space_name(3).is_none());
        });
    }

    /// A script's focus and select show the Space or the tab as a click does, a tab of another
    /// Space with its Space, and are refused an agent's.
    #[gpui::test]
    fn a_script_shows_a_users_space_or_tab_and_never_an_agents(cx: &mut TestAppContext) {
        let (shell, host) = shell(cx);
        host.take();
        assert!(shell.update(cx, |shell, cx| shell.script_show_space(2, cx)));
        assert_eq!(*host.shown.borrow(), vec![2]);
        assert!(!shell.update(cx, |shell, cx| shell.script_show_space(3, cx)));
        assert!(!shell.update(cx, |shell, cx| shell.script_select(30, cx)));
        assert_eq!(*host.shown.borrow(), vec![2], "an agent's Space was shown");
        assert!(!host.take().contains(&"select 30".to_string()));

        shell.update(cx, |shell, cx| shell.set_space(1, cx));
        host.shown.borrow_mut().clear();
        assert!(shell.update(cx, |shell, cx| shell.script_select(20, cx)));
        assert_eq!(
            *host.shown.borrow(),
            vec![2],
            "the tab's Space didn't show first"
        );
        assert!(host.take().contains(&"select 20".to_string()));
    }
}
