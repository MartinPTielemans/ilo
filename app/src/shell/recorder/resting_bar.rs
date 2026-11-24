//! The core, as the resting bar of a window with no page sees it (#4140): what it was asked, in
//! order, which the tests read to see what reached the core and when.

use std::{cell::RefCell, rc::Rc};

use gpui::{App, SharedString};

use crate::command_bar::{self, TaskAgent, TaskAgents};
use crate::elevation::Level;
use crate::shell;
use crate::tooltip::{Anchor, Tip};

/// A resting bar's host: Claude is ready and a search goes to a search, and each call is a line
/// in `asked`.
pub(in crate::shell) struct RestingBar {
    pub(in crate::shell) asked: Rc<RefCell<Vec<String>>>,
}

impl RestingBar {
    fn ask(&self, call: String) {
        self.asked.borrow_mut().push(call);
    }
}

impl command_bar::Host for RestingBar {
    fn suggest(&self, _: Option<&str>, text: &str, _: bool) {
        self.ask(format!("suggest {text}"));
    }
    fn remove_suggestion(&self, url: &str, _: &mut App) {
        self.ask(format!("remove suggestion {url}"));
    }
    fn step_removal(&self, _: bool, _: &mut App) -> bool {
        false
    }
    fn typed_url(&self, text: &str) -> Option<String> {
        text.contains('.').then(|| format!("https://{text}/"))
    }
    fn search_url(&self, _: Option<&str>, text: &str) -> Option<String> {
        Some(format!("https://search.example/?q={text}"))
    }
    fn page_favicon(&self, url: &str) {
        self.ask(format!("favicon {url}"));
    }
    fn open_tab(&self, url: &str, _: &str, behind: bool) {
        self.ask(format!("open tab {url} behind {behind}"));
    }
    fn open_window(&self, url: &str, _: &str, _: &mut App) {
        self.ask(format!("open window {url}"));
    }
    fn navigate(&self, tab: u64, url: &str, _: &str) {
        self.ask(format!("navigate {tab} {url}"));
    }
    fn reload(&self, tab: u64) {
        self.ask(format!("reload {tab}"));
    }
    fn select(&self, tab: u64) {
        self.ask(format!("select {tab}"));
    }
    fn tab_window(&self, _: u64) -> Option<u64> {
        None
    }
    fn activate(&self, window: u64, tab: u64, _: &mut App) {
        self.ask(format!("activate {window} {tab}"));
    }
    fn split(&self, tab: u64, with: u64, _: &mut App) {
        self.ask(format!("split {tab} {with}"));
    }
    fn open_split(&self, url: &str, _: &str, with: u64, _: &mut App) {
        self.ask(format!("open split {url} {with}"));
    }
    fn copy(&self, text: &str, _: &mut App) {
        self.ask(format!("copy {text}"));
    }
    fn show(&self, _: bool, _: f32, _: f32, _: f32, _: f32, _: Level) {
        self.ask("show".into());
    }
    fn set_height(&self, _: f32) {
        self.ask("set height".into());
    }
    fn hide(&self) {
        self.ask("hide".into());
    }
    fn focus_page(&self) {
        self.ask("focus page".into());
    }
    fn agents_page(&self, _: &App) -> bool {
        false
    }
    fn dismiss(&self) {
        self.ask("dismiss".into());
    }
    fn take_keyboard(&self) {
        self.ask("take keyboard".into());
    }
    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
        shell::bar_commands(text, cx)
    }
    fn task_agents(&self, _: &App) -> Option<TaskAgents> {
        self.ask("task agents".into());
        let claude = TaskAgent {
            id: "claude".into(),
            name: "Claude".into(),
            glyph: "icons/terminal.svg",
            model: Some("Opus".into()),
            resets: None,
        };
        Some(TaskAgents {
            agents: vec![claude],
            known: true,
        })
    }
    fn over_private(&self) -> bool {
        false
    }
    fn asking(&self, shown: bool, _: &mut App) {
        self.ask(format!("asking {shown}"));
    }
    fn task_page(&self, _: &App) -> Option<(SharedString, SharedString)> {
        None
    }
    fn run_task(
        &self,
        agent: &str,
        text: &str,
        _: Option<(SharedString, SharedString)>,
        _: &mut App,
    ) {
        self.ask(format!("run task {agent} {text}"));
    }
    fn set_up_agents(&self, _: &mut App) {
        self.ask("set up agents".into());
    }
    fn tooltip(&self, _: Option<(Tip, Anchor)>, _: &mut App) {}
    fn archived(&self, _: &str, _: &App) -> Option<(u64, f64)> {
        None
    }
    fn restore_archived(&self, _: u64, _: &mut App) -> bool {
        false
    }
}
