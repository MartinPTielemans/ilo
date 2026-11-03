//! What the core says of agents' sessions and actions in tabs.

use super::*;

/// An agent's action as the core judged it, for the activity log in Settings › Agents, which
/// check.sh reads in the log too. One it let the agent do puts the agent's dot on the tab's row,
/// from the same entry. It comes from inside the call that asked, so the app hears it after.
pub(crate) unsafe extern "C" fn agent_activity(
    _: *mut c_void,
    entry: *const bc::bc_agent_activity,
) {
    let entry = unsafe { &*entry };
    let text = |text| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    };
    let (agent, action, refused) = (text(entry.agent), text(entry.action), text(entry.refused));
    let tab = entry.tab;
    match refused.is_empty() {
        true => log!("browser: agent activity {agent} {tab} {action}"),
        false => log!("browser: agent activity {agent} {tab} {action} refused: {refused}"),
    }
    if refused.is_empty() {
        let agent = SharedString::from(agent.clone());
        update_app_later(move |cx| {
            update_shells_in(cx, |shell, cx| shell.agent_acted(agent.clone(), tab, cx))
        });
    }
    // The Agents' own calls log themselves.
    if !agents_core::calling() {
        agents_core::update(move |agents, cx| agents.checked(&agent, tab, &action, &refused, cx));
    }
}

/// The core ended an agent's session with a tab: its client disconnected, it released the tab, the
/// user took the tab back or its page asked for the user's passkey. The activity log has it beside
/// what the agent did, and the agent is no longer at work in the tab.
pub(crate) unsafe extern "C" fn agent_session_ended(
    _: *mut c_void,
    agent: *const c_char,
    tab: bc::bc_id,
    why: c_int,
) {
    let agent = unsafe { CStr::from_ptr(agent) }
        .to_string_lossy()
        .into_owned();
    let why = match why as u32 {
        bc::BC_AGENT_SESSION_DISCONNECTED => "disconnected",
        bc::BC_AGENT_SESSION_RELEASED => "released",
        bc::BC_AGENT_SESSION_TAKEN_BACK => "taken back",
        bc::BC_AGENT_SESSION_PASSKEY => "passkey",
        _ => "ended",
    };
    // check.sh reads this line.
    log!("browser: agent session ended {agent} {tab} {why}");
    // As agent_activity, from inside the call that ended it.
    update_app_later(move |cx| {
        update_shells_in(cx, |shell, cx| shell.agent_session_ended(&agent, tab, cx));
        if let Some(agents) = browser::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.session_ended(&agent, tab, why, cx));
        }
    });
}

/// The core's answer to an agent's action in a tab (bc_agent_page): why it couldn't, a
/// screenshot, or the page's tree.
pub(crate) unsafe extern "C" fn agent_page_done(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    text: *const c_char,
    error: bool,
) {
    let text = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    // check.sh reads this line.
    match error {
        true => log!("browser: agent page {tab} {request} error: {text}"),
        false => log!("browser: agent page {tab} {request} {}", text.len()),
    }
    agents_core::update(move |agents, cx| agents.page_done(request, text, error, cx));
}

/// The element an agent's action in a tab (bc_agent_page) is about to click or type in, for the
/// agent's pointer over the page.
pub(crate) unsafe extern "C" fn agent_page_element(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    b: bc::bc_rect,
) {
    // check.sh reads this line.
    log!(
        "browser: agent element {tab} {request} at {} {} {} {}",
        b.x,
        b.y,
        b.width,
        b.height
    );
    let element = Bounds::new(
        point(px(b.x as f32), px(b.y as f32)),
        size(px(b.width as f32), px(b.height as f32)),
    );
    agents_core::update(move |agents, cx| {
        let Some(tag) = agents.ref_of(request) else {
            return;
        };
        update_shells_in(cx, |shell, cx| {
            shell.agent_pointing(tab, tag.clone().into(), element, cx)
        });
    });
}

/// The text of a tab's page that bc_tab_read_text asked for, or null if it had none.
pub(crate) unsafe extern "C" fn tab_text(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    text: *const c_char,
) {
    let text = (!text.is_null()).then(|| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    });
    // check.sh reads this line.
    log!(
        "browser: tab text {tab} {request} {}",
        text.as_ref().map_or(0, String::len)
    );
    agents_core::update(move |agents, cx| agents.text_read(request, text, cx));
}
