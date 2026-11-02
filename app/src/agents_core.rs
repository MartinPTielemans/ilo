//! The agents' browser: the core, as the Agents in the lib ask it (decision 8), and the Agents made
//! once the first window's UI shows, so nothing of theirs is on the launch's path.

use std::cell::Cell;
use std::ffi::CString;
use std::rc::Rc;
use std::sync::Arc;

use browser::agents::{self, Agents, Browser, GlobalAgents, PageAction, Question, TabInfo};
use gpui::{App, AppContext};

use crate::{WINDOWS, bc, ids, name};

thread_local! {
    /// A call of the Agents' into the core is under way, whose check the Agents log themselves.
    static CALLING: Cell<bool> = const { Cell::new(false) };
}

/// Whether the core's report of an agent's call is of one the Agents made.
pub fn calling() -> bool {
    CALLING.get()
}

/// Calls the core for the Agents.
fn call<T>(core: impl FnOnce() -> T) -> T {
    CALLING.set(true);
    let answer = core();
    CALLING.set(false);
    answer
}

struct Core;

impl Browser for Core {
    fn agent_space(&self, agent: &str) -> u64 {
        let mut spaces = ids(bc::bc_spaces).into_iter();
        spaces
            .find(|&space| name(bc::bc_space_agent, space) == agent)
            .unwrap_or(0)
    }

    fn make_agent_space(&self, agent: &str, agent_name: &str, _: &mut App) -> u64 {
        let (Ok(id), Ok(named)) = (CString::new(agent), CString::new(agent_name)) else {
            return 0;
        };
        let space =
            unsafe { bc::bc_space_create_for_agent(named.as_ptr(), id.as_ptr(), named.as_ptr()) };
        // check.sh reads this line, as the driver's agentspace writes it.
        browser::log!("browser: made space {space} for agent {agent}");
        space
    }

    fn space_tabs(&self, space: u64, cx: &App) -> Vec<TabInfo> {
        (self.space_tab_ids(space, cx).into_iter()).map(|tab| self.tab(tab, cx)).collect()
    }

    fn space_tab_ids(&self, space: u64, _: &App) -> Vec<u64> {
        let pinned = tabs(bc::bc_space_pinned, space).into_iter();
        let pinned = pinned.filter(|&row| !unsafe { bc::bc_folder(row, &mut false) });
        pinned.chain(tabs(bc::bc_space_today, space)).collect()
    }

    fn agent_tabs(&self, agent: &str, cx: &App) -> Vec<TabInfo> {
        let Ok(agent) = CString::new(agent) else {
            return Vec::new();
        };
        let list = |tabs: *mut bc::bc_id, capacity| unsafe {
            bc::bc_agent_list_tabs(agent.as_ptr(), tabs, capacity)
        };
        let mut tabs = vec![0; list(std::ptr::null_mut(), 0)];
        let count = list(tabs.as_mut_ptr(), tabs.len());
        tabs.truncate(count);
        (tabs.into_iter()).map(|tab| self.tab(tab, cx)).collect()
    }

    fn tab(&self, tab: u64, cx: &App) -> TabInfo {
        // Every window's shell has every Space's tabs.
        let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
        let Some(shell) = shell.and_then(|shell| shell.upgrade()) else {
            return TabInfo {
                id: tab,
                ..TabInfo::default()
            };
        };
        let shell = shell.read(cx);
        TabInfo {
            id: tab,
            title: shell.title(tab),
            url: shell.page(tab).url,
        }
    }

    fn may_act(&self, agent: &str, tab: u64, action: &str, _: &mut App) -> Result<(), String> {
        let (Ok(agent), Ok(action)) = (CString::new(agent), CString::new(action)) else {
            return Err("The agent's name can't be used.".into());
        };
        let mut reason = [0u8; 512];
        let may = call(|| unsafe {
            let reason = reason.as_mut_ptr().cast();
            bc::bc_agent_may_act(agent.as_ptr(), tab, action.as_ptr(), reason, 512)
        });
        match may {
            true => Ok(()),
            false => {
                let end = reason
                    .iter()
                    .position(|&byte| byte == 0)
                    .unwrap_or(reason.len());
                Err(String::from_utf8_lossy(&reason[..end]).into_owned())
            }
        }
    }

    fn read_text(&self, tab: u64, request: u64) {
        unsafe { bc::bc_tab_read_text(tab, request) };
    }

    fn disconnected(&self, agent: &str, _: &mut App) {
        if let Ok(agent) = CString::new(agent) {
            call(|| unsafe { bc::bc_agent_disconnected(agent.as_ptr()) });
        }
    }

    fn open_tab(&self, agent: &str, url: &str, _: &mut App) -> u64 {
        let (Ok(agent), Ok(url)) = (CString::new(agent), CString::new(url)) else {
            return 0;
        };
        call(|| unsafe { bc::bc_agent_open_tab(agent.as_ptr(), url.as_ptr()) })
    }

    fn navigate(&self, agent: &str, tab: u64, url: &str, _: &mut App) -> bool {
        let (Ok(agent), Ok(url)) = (CString::new(agent), CString::new(url)) else {
            return false;
        };
        call(|| unsafe { bc::bc_agent_navigate(agent.as_ptr(), tab, url.as_ptr()) })
    }

    fn page(
        &self,
        agent: &str,
        tab: u64,
        request: u64,
        action: PageAction,
        element: i64,
        text: &str,
    ) {
        let action = match action {
            PageAction::Snapshot => bc::BC_AGENT_PAGE_SNAPSHOT,
            PageAction::Click => bc::BC_AGENT_PAGE_CLICK,
            PageAction::Type => bc::BC_AGENT_PAGE_TYPE,
            PageAction::TypeSubmit => bc::BC_AGENT_PAGE_TYPE_SUBMIT,
            PageAction::Key => bc::BC_AGENT_PAGE_KEY,
            PageAction::ScrollDown => bc::BC_AGENT_PAGE_SCROLL_DOWN,
            PageAction::ScrollUp => bc::BC_AGENT_PAGE_SCROLL_UP,
            PageAction::Back => bc::BC_AGENT_PAGE_BACK,
            PageAction::Forward => bc::BC_AGENT_PAGE_FORWARD,
            PageAction::Screenshot => bc::BC_AGENT_PAGE_SCREENSHOT,
        };
        // A NUL can't reach the page, so text with one goes without what follows it.
        let text = text.split('\0').next().unwrap_or_default();
        let (Ok(agent), Ok(text)) = (CString::new(agent), CString::new(text)) else {
            return;
        };
        unsafe {
            bc::bc_agent_page(agent.as_ptr(), tab, request, action as i32, element, text.as_ptr())
        };
    }

    fn url_for(&self, text: &str) -> Option<String> {
        // Searches with the engine of the profile the core started with, as no window is asked.
        crate::url_for(text, |text, url, capacity| unsafe { bc::bc_typed_url(0, text, url, capacity) })
    }

    fn ask(&self, question: Option<Question>, cx: &mut App) {
        crate::update_shells_in(cx, |shell, cx| shell.set_agent_question(question.clone(), cx));
    }

    fn hand_off(&self, agent: &str, tab: u64) -> bool {
        let Ok(agent) = CString::new(agent) else {
            return false;
        };
        call(|| unsafe { bc::bc_agent_hand_off(agent.as_ptr(), tab) })
    }

    fn take_back(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_take_back(tab) };
    }

    fn hand_back(&self, tab: u64) -> bool {
        unsafe { bc::bc_tab_hand_back(tab) }
    }

    fn close_tab(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_close(tab, false) };
    }

    fn give(&self, tab: u64, space: u64, _: &mut App) -> bool {
        unsafe { bc::bc_tab_give_to_agent(tab, space) }
    }

    fn make_task_space(&self, agent: &str, agent_name: &str, like: u64, cx: &mut App) -> u64 {
        let space = self.make_agent_space(agent, agent_name, cx);
        let profile = unsafe { bc::bc_space_profile(like) };
        if space != 0 && profile != 0 && !unsafe { bc::bc_space_set_profile(space, profile) } {
            browser::log!("browser: task space {space} couldn't take profile {profile}");
        }
        space
    }

    fn delete_space(&self, space: u64, _: &mut App) {
        unsafe { bc::bc_space_delete(space) };
    }

    fn task_spaces(&self) -> Vec<u64> {
        (ids(bc::bc_spaces).into_iter())
            .filter(|&space| agents::is_task(&name(bc::bc_space_agent, space)))
            .collect()
    }

    fn move_tab(&self, tab: u64, space: u64, _: &mut App) {
        unsafe { bc::bc_tab_move_to_space(tab, space) }
    }
}

/// The tabs `copy` copies of `space`, as bc_space_today does.
fn tabs(
    copy: unsafe extern "C" fn(bc::bc_id, *mut bc::bc_id, usize) -> usize,
    space: u64,
) -> Vec<u64> {
    let count = unsafe { copy(space, std::ptr::null_mut(), 0) };
    let mut tabs = vec![0; count];
    let count = unsafe { copy(space, tabs.as_mut_ptr(), tabs.len()) };
    tabs.truncate(count);
    tabs
}

/// Where the Agents' settings are kept: in the profile directory, beside the Shortcuts.
fn path() -> std::path::PathBuf {
    format!("{}/Agents", crate::profile_dir()).into()
}

/// Makes the Agents, off, and reads what was saved in the background, which turns the server on
/// if the user left it on. Once, as the first window's UI shows.
pub fn start(cx: &mut App) {
    if agents::global(cx).is_some() {
        return;
    }
    let socket = agents::address(std::path::Path::new(&crate::profile_dir()));
    // Beside the browser's executable: in the app bundle, or beside Chromium's on Linux.
    let exe = std::env::current_exe().unwrap_or_default();
    let bridge = agents::bridge::launch(
        &exe.with_file_name(agents::bridge::NAME),
        agents::bridge::flatpak_id().as_deref(),
    );
    let made = Agents::new(
        Rc::new(Core),
        Arc::new(agents::System),
        Some(path()),
        socket,
        bridge,
    );
    let made = cx.new(|cx| {
        let mut made = made;
        // A run before that ended without quitting left its tasks' Spaces.
        made.end_old_tasks(cx);
        made
    });
    cx.set_global(GlobalAgents(made.clone()));
    // The bar in an agent's Space follows what it does, and what the user holds from it.
    cx.observe(&made, |agents, cx| {
        let status = agents.read(cx).status();
        // Each card is compared with the last, so a window redraws only for a task that changed.
        let cards = agents.read(cx).shown_cards(cx);
        crate::update_shells_in(cx, |shell, cx| {
            shell.set_agent_status(status.clone(), cx);
            shell.set_task_cards(cards.clone(), cx);
            // Only a window that shows a task's conversation is sent it.
            if let Some(agent) = shell.talk_agent() {
                let conversation = agents.read(cx).conversation(&agent, cx);
                shell.set_conversation(conversation, cx);
            }
        });
        browser::command_bar::task_agents_changed(cx);
    })
    .detach();
    cx.spawn(async move |cx| {
        let read = cx.background_spawn(async { std::fs::read_to_string(path()) });
        if let Ok(saved) = read.await {
            made.update(cx, |agents, cx| agents.load(&saved, cx));
        }
    })
    .detach();
}

/// Tells the Agents what the core reported, once the update the report came from, if any, is over.
pub fn update(change: impl FnOnce(&mut Agents, &mut gpui::Context<Agents>) + 'static) {
    crate::update_app_later(move |cx| {
        if let Some(agents) = agents::global(cx) {
            agents.update(cx, change);
        }
    });
}
