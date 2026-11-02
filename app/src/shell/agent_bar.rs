//! The bar in an agent's Space (#365): while the agent is at work there, what it did last, with
//! Take Over, which makes the tab the user's, and Stop; on a tab the user holds, Hand Back, which
//! gives it back. ⌘↩ is Take Over or Hand Back, and ⌘. Stop while the bar has it, with focus in
//! the page too, and each button's tooltip shows its key. It's a card at the sidebar's foot, which
//! GPUI draws, as the window's question cards are.

use gpui::{
    App, Context, Div, FontWeight, MouseButton, SharedString, Stateful, div, prelude::*, px, rgba,
};

use super::{
    Command, Shell, TITLE,
    commands::{ShellHandle, keystroke_keys},
    extensions::keystroke_text,
    keys::user_keys,
    permission::{BUTTONS_TOP, card_button, card_listener},
};
use crate::agents::Status;
use crate::tooltip::{Anchor, Tip};
use crate::tokens::shell::permission::{BUTTON_GAP, DETAIL, GAP};

/// What the bar shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Bar {
    /// The agent with the id `agent` is at work in `tab`, having last done `doing`.
    Working { agent: SharedString, name: SharedString, doing: SharedString, tab: u64 },
    /// The user holds the tab shown from the agent: handed to them for `why`, or taken over.
    Held { name: SharedString, why: SharedString, tab: u64 },
}

/// The bar's Stop, which is Stop Loading's key elsewhere, so the bar takes it only while it shows
/// Stop. Escape, Stop Loading's other key, stays the page's, as an agent stopped by a stray escape
/// would lose its work.
const STOP_KEY: &str = "cmd-.";

/// The bar's buttons, by the ids their frames are kept under.
pub(super) const STOP: &str = "agent-stop";
pub(super) const PRIMARY: &str = "agent-primary";

/// ⌘. for Stop while the UI has the keyboard, before it's Stop Loading. The page hands it over
/// before it sees it (`run_agent_key`).
pub(super) fn bind(cx: &mut App) {
    cx.intercept_keystrokes(|event, _, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        let Some(shell) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) else {
            return;
        };
        let keystroke = keystroke_text(&event.keystroke);
        if shell.update(cx, |shell, cx| shell.run_agent_key(&keystroke, cx)).unwrap_or(false) {
            cx.stop_propagation();
        }
    })
    .detach();
}

impl Shell {
    /// Stops the agent at work in the Space shown if `keystroke` is the bar's Stop, and says
    /// whether it did.
    pub fn run_agent_key(&mut self, keystroke: &str, cx: &mut Context<Self>) -> bool {
        let Some(Bar::Working { agent, .. }) = self.agent_bar().filter(|_| keystroke == STOP_KEY)
        else {
            return false;
        };
        self.browser.stop_agent(agent, cx);
        true
    }

    /// What the agents do and what the user holds from them, as the Agents tell every window.
    pub fn set_agent_status(&mut self, status: Vec<Status>, cx: &mut Context<Self>) {
        if self.agent_status != status {
            self.agent_status = status;
            cx.notify();
        }
    }

    /// The bar of the agent's page the window shows (`agent_page`), if the agent is at work in its
    /// Space or the user holds that page.
    pub(super) fn agent_bar(&self) -> Option<Bar> {
        let (space, shown) = self.agent_page()?;
        let status = self.agent_status.iter().find(|status| status.space == space.id)?;
        let name = space.agent.clone();
        if let Some((tab, why)) = status.held.iter().find(|(tab, _)| *tab == shown) {
            return Some(Bar::Held { name, why: why.clone(), tab: *tab });
        }
        let tab = self.agents_at.get(&status.agent).copied()?;
        let doing = status.doing.clone();
        Some(Bar::Working { agent: status.agent.clone(), name, doing, tab })
    }

    /// Whether the user holds `tab` from an agent, which its row's mark shows in the agent's
    /// Space.
    pub(super) fn agent_holds(&self, tab: u64) -> bool {
        self.agents_space_shown()
            && self.agent_status.iter().any(|status| status.held.iter().any(|held| held.0 == tab))
    }

    /// Take Over, or Hand Back: the bar's first button and ⌘↩.
    pub(super) fn agent_bar_primary(&mut self, cx: &mut Context<Self>) {
        match self.agent_bar() {
            Some(Bar::Working { agent, tab, .. }) => {
                self.browser.take_over(agent, tab, cx);
                self.select(tab, cx);
            }
            Some(Bar::Held { tab, .. }) => {
                self.browser.hand_back(tab, cx);
                // A task's page watched is the agent's again, so the keys the user types next
                // aren't its.
                if self.watched_task_page().is_some() {
                    self.browser.focus_ui(cx);
                }
            }
            None => {}
        }
    }

    /// The pointer has come onto or left the bar's button `key`, whose tooltip shows its key:
    /// ⌘. for Stop, and Back to Pinned URL's, ⌘↩ unless the user moved it, for the other.
    pub(super) fn hover_agent_button(
        &self,
        key: &'static str,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self.button_frames.borrow().get(key).copied();
        let tip = (self.agent_bar().zip(frame).filter(|_| hovered && !busy)).map(|(bar, frame)| {
            let (text, keys) = match (key, bar) {
                (STOP, _) => ("Stop", keystroke_keys(STOP_KEY)),
                (_, Bar::Working { .. }) => ("Take Over", user_keys(cx).chips(Command::BackToPinnedUrl)),
                (_, Bar::Held { .. }) => ("Hand Back", user_keys(cx).chips(Command::BackToPinnedUrl)),
            };
            let keys = keys.into_iter().map(Into::into).collect();
            (Tip { text: text.into(), keys }, Anchor::under(frame, self.viewport))
        });
        let working = matches!(self.agent_bar(), Some(Bar::Working { .. }));
        self.agent_tip.set(tip.is_some().then_some(working));
        self.hover_tip(tip, cx);
    }

    /// A click on a button ends its tooltip, as other buttons' do.
    pub(super) fn end_agent_tip(&self, cx: &mut Context<Self>) {
        self.agent_tip.set(None);
        self.browser.tooltip(None, cx);
    }

    pub(super) fn agent_bar_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        // A button that goes from under the pointer, as Hand Back does once it's clicked, sends no
        // hover-out, so its tooltip ends here, or it shows later hanging from nothing.
        let working = self.agent_bar().map(|bar| matches!(bar, Bar::Working { .. }));
        if self.agent_tip.get().is_some_and(|was| Some(was) != working) {
            self.agent_tip.set(None);
            let browser = self.browser.clone();
            cx.defer(move |cx| browser.tooltip(None, cx));
        }
        // A task's page watched has the bar laid under it instead (task_area.rs).
        let Some(bar) = self.agent_bar().filter(|_| self.task_page_drawn().is_none()) else {
            // Drawn again, it's logged again.
            self.agent_bar_drawn.set(None);
            return None;
        };
        let (title, detail, tab) = bar_words(&bar);
        let buttons = self.agent_bar_buttons(&bar, cx);
        let listener = card_listener("agent bar", &self.agent_bar_drawn, tab);
        Some(
            self.plate("agent-bar")
                .top(gpui::auto())
                .bottom(px(self.list_bottom() + GAP))
                .child(listener)
                .child(
                    div()
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(div().truncate().text_color(rgba(DETAIL)).child(detail))
                .child(div().mt(px(BUTTONS_TOP)).flex().gap(px(BUTTON_GAP)).children(buttons)),
        )
    }

    /// The bar's buttons: Stop while the agent is at work, then Take Over or Hand Back, each with
    /// its tooltip.
    pub(super) fn agent_bar_buttons(
        &self,
        bar: &Bar,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let primary = match bar {
            Bar::Working { .. } => card_button(&self.palette, "agent-take-over", "Take Over"),
            Bar::Held { .. } => card_button(&self.palette, "agent-hand-back", "Hand Back"),
        };
        let primary = (primary.relative().child(self.button_frame(PRIMARY)))
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                shell.hover_agent_button(PRIMARY, *hovered, cx)
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|shell, _, _, cx| shell.end_agent_tip(cx)))
            .on_click(cx.listener(|shell, _, _, cx| shell.agent_bar_primary(cx)));
        let stop = match bar {
            Bar::Working { agent, .. } => {
                let agent = agent.clone();
                let stop = card_button(&self.palette, STOP, "Stop").relative();
                Some(stop.child(self.button_frame(STOP)).on_hover(cx.listener(
                    |shell, hovered: &bool, _, cx| shell.hover_agent_button(STOP, *hovered, cx),
                )).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| shell.end_agent_tip(cx)),
                ).on_click(cx.listener(move |shell, _, _, cx| {
                    shell.browser.stop_agent(agent.clone(), cx)
                })))
            }
            Bar::Held { .. } => None,
        };
        stop.into_iter().chain([primary]).collect()
    }
}

/// What the bar says, its title and the line under it, and the tab it's about.
pub(super) fn bar_words(bar: &Bar) -> (String, String, u64) {
    match bar {
        Bar::Working { name, doing, tab, .. } => {
            (format!("{name} is at work"), doing.to_string(), *tab)
        }
        Bar::Held { name, why, tab } if why.is_empty() => (
            "You have this tab".to_string(),
            format!("{name} can't act in it until you hand it back."),
            *tab,
        ),
        Bar::Held { name, why, tab } => (format!("{name} needs you"), why.to_string(), *tab),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, Entity, Modifiers, SharedString, TestAppContext, point, px};

    use super::{Bar, PRIMARY, STOP};
    use crate::shell::commands::keystroke_keys;
    use crate::agents::Status;
    use crate::shell::{Command, Folders, Shell, recorder::Recorder};

    /// The user's Space 1 with tab 10, and Claude Code's Space 2 with tabs 20 and 21, shown.
    fn shell_in_agents_space(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
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
            shell.set_tabs(2, Vec::new(), vec![20, 21], Vec::new(), Folders::default(), cx);
            shell.set_space(2, cx);
            shell.set_selected(21, cx);
        });
        (shell, host)
    }

    fn status(held: Vec<(u64, SharedString)>) -> Vec<Status> {
        let doing = "Clicked · example.com".into();
        vec![Status { agent: "claude-code".into(), space: 2, doing, held }]
    }

    /// While the agent is at work in its Space the bar says what it did, and Take Over, or ⌘↩,
    /// takes its tab and shows it; Stop stops it. With no agent at work there's no bar.
    #[gpui::test]
    fn the_bar_shows_the_agent_at_work_and_takes_over(cx: &mut TestAppContext) {
        let (shell, host) = shell_in_agents_space(cx);
        shell.update(cx, |shell, cx| shell.set_agent_status(status(Vec::new()), cx));
        assert_eq!(shell.read_with(cx, |shell, _| shell.agent_bar()), None);
        shell.update(cx, |shell, cx| shell.agent_acted("claude-code".into(), 20, cx));
        let bar = shell.read_with(cx, |shell, _| shell.agent_bar());
        let working = Bar::Working {
            agent: "claude-code".into(),
            name: "Claude Code".into(),
            doing: "Clicked · example.com".into(),
            tab: 20,
        };
        assert_eq!(bar, Some(working));
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::BackToPinnedUrl, "page", cx));
        assert_eq!(host.take(), ["take over claude-code 20", "select 20"]);
        shell.update(cx, |shell, cx| shell.set_space(1, cx));
        assert_eq!(shell.read_with(cx, |shell, _| shell.agent_bar()), None, "the user's Space");
    }

    /// ⌘. is the bar's Stop while it shows one, and the page hands it over too. Escape, and ⌘. on a
    /// tab the user holds, where the bar has no Stop, stay Stop Loading's.
    #[gpui::test]
    fn command_period_stops_the_agent_at_work(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(crate::shell::bind_keys);
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![(2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into())],
                cx,
            );
            shell.set_tabs(2, Vec::new(), vec![20, 21], Vec::new(), Folders::default(), cx);
            shell.set_space(2, cx);
            shell.set_selected(21, cx);
            shell.set_agent_status(status(Vec::new()), cx);
            shell.agent_acted("claude-code".into(), 20, cx);
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        host.take();
        cx.simulate_keystrokes("escape");
        assert!(!host.take().contains(&"stop agent claude-code".to_string()));
        cx.simulate_keystrokes("cmd-.");
        assert_eq!(host.take(), ["stop agent claude-code"]);
        assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)), "from the page");
        assert_eq!(host.take(), ["stop agent claude-code"]);

        let held = vec![(21, "Sign in to example.com".into())];
        shell.update(cx, |shell, cx| shell.set_agent_status(status(held), cx));
        cx.simulate_keystrokes("cmd-.");
        assert!(!host.take().contains(&"stop agent claude-code".to_string()));
        assert!(!shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    }

    /// Each button's tooltip shows its key, ⌘. for Stop and ⌘↩ for Take Over and Hand Back, so
    /// the keys are found where the buttons are.
    #[gpui::test]
    fn the_bars_tooltips_show_its_keys(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![(2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into())],
                cx,
            );
            shell.set_tabs(2, Vec::new(), vec![20, 21], Vec::new(), Folders::default(), cx);
            shell.set_space(2, cx);
            shell.set_selected(21, cx);
            shell.set_agent_status(status(Vec::new()), cx);
            shell.agent_acted("claude-code".into(), 20, cx);
        });
        cx.run_until_parked();
        let tip = |cx: &mut gpui::VisualTestContext, key: &'static str| {
            let at = shell.read_with(cx, |shell, _| shell.button_frames.borrow()[key].center());
            cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
            cx.run_until_parked();
            host.take();
            cx.simulate_mouse_move(at, None, Modifiers::none());
            cx.run_until_parked();
            host.take().into_iter().find(|line| line.starts_with("tooltip"))
        };
        let back = keystroke_keys("cmd-enter").concat();
        let stop = format!("tooltip Stop {}", keystroke_keys("cmd-.").concat());
        assert_eq!(tip(cx, STOP), Some(stop));
        assert_eq!(tip(cx, PRIMARY), Some(format!("tooltip Take Over {back}")));

        let held = vec![(21, "Sign in to example.com".into())];
        shell.update(cx, |shell, cx| shell.set_agent_status(status(held), cx));
        cx.run_until_parked();
        assert_eq!(tip(cx, PRIMARY), Some(format!("tooltip Hand Back {back}")));

        // The click hands the tab back, and the bar goes from under the pointer with no hover-out,
        // so the click itself ends the tooltip.
        let at = shell.read_with(cx, |shell, _| shell.button_frames.borrow()[PRIMARY].center());
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let asked = host.take();
        let ended = asked.iter().position(|line| line == "tooltip none");
        let handed = asked.iter().position(|line| line == "hand back 21");
        assert!(ended.is_some() && handed.is_some(), "{asked:?}");

        // A bar that changes under the pointer with no click, as when the agent hands the tab
        // back itself, ends the tooltip of the button that went too.
        let held = vec![(21, "Sign in to example.com".into())];
        shell.update(cx, |shell, cx| shell.set_agent_status(status(held), cx));
        cx.run_until_parked();
        assert_eq!(tip(cx, PRIMARY), Some(format!("tooltip Hand Back {back}")));
        shell.update(cx, |shell, cx| shell.set_agent_status(status(Vec::new()), cx));
        cx.run_until_parked();
        assert_eq!(host.take(), ["tooltip none"]);
    }

    /// On a tab the user holds the bar says so, the row's mark is a hollow ring, and Hand Back, or
    /// ⌘↩, gives the tab back.
    #[gpui::test]
    fn a_tab_the_user_holds_has_hand_back(cx: &mut TestAppContext) {
        let (shell, host) = shell_in_agents_space(cx);
        let held = vec![(21, "Sign in to example.com".into())];
        shell.update(cx, |shell, cx| shell.set_agent_status(status(held), cx));
        let bar = shell.read_with(cx, |shell, _| shell.agent_bar());
        let why = "Sign in to example.com".into();
        assert_eq!(bar, Some(Bar::Held { name: "Claude Code".into(), why, tab: 21 }));
        assert!(shell.read_with(cx, |shell, _| shell.agent_holds(21)));
        assert!(!shell.read_with(cx, |shell, _| shell.agent_holds(20)));
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::BackToPinnedUrl, "page", cx));
        assert_eq!(host.take(), ["hand back 21"]);
    }

    /// The menu of a tab the user took back says so first, and Hand Back gives it back; the rest
    /// of the menu keeps its submenus. A tab the agent still has says nothing of it.
    #[gpui::test]
    fn a_taken_back_tabs_menu_hands_it_back(cx: &mut TestAppContext) {
        let (shell, host) = shell_in_agents_space(cx);
        shell.update(cx, |shell, cx| shell.set_agent_status(status(vec![(21, "".into())]), cx));
        let at = point(px(100.), px(200.));
        shell.update(cx, |shell, cx| shell.tab_menu(21, at, cx));
        let menu = host.menus.borrow().last().cloned().unwrap();
        let head: Vec<_> = menu.iter().take(2).map(|(title, on)| (title.as_ref(), *on)).collect();
        let hand_back = ("Hand Back to Claude Code", true);
        assert_eq!(head, [("Taken Back from Claude Code", false), hand_back]);
        let entries = host.entries.borrow().clone();
        let move_to = entries.iter().position(|entry| entry.title == "Move to").unwrap();
        let new_space = entries.iter().find(|entry| entry.title == "New Space…").unwrap();
        assert_eq!(new_space.parent, Some(move_to), "the submenu lost its place");
        host.take();
        cx.update(|cx| host.choose("Hand Back to Claude Code", cx));
        assert!(host.take().contains(&"hand back 21".to_string()));

        shell.update(cx, |shell, cx| shell.tab_menu(20, at, cx));
        let menu = host.menus.borrow().last().cloned().unwrap();
        assert!(!menu.iter().any(|(title, _)| title.starts_with("Hand Back")));
    }
}
