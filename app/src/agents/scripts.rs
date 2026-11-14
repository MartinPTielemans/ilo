//! Scripts (#704): AppleScript reads the windows, the user's Spaces and their tabs, and gives the
//! commands a click gives, once Settings › Agents' "Let scripts control the browser" lets them
//! in, which is off at first. macOS asks the sending app once, but a terminal it said yes for
//! lets every process under it in, agents too, which the agents' rules keep from the user's tabs,
//! so off refuses reads too. Each Apple Event is one entry of the activity log, refused or not,
//! under the sender's name, and none reads a page or opens one that isn't http or https.

use web_time::SystemTime;

use gpui::{Context, SharedString};

use super::{Activity, Agents, LOG_LENGTH};

/// The id scripts' entries go under in the activity log, no agent's.
pub const AGENT: &str = "scripting";

/// What a script did, as its entry names it.
pub const READ: &str = "read";
pub const SHOW_SPACE: &str = "show_space";
pub const SHOW_TAB: &str = "show_tab";
pub const NEW_WINDOW: &str = "new_window";
pub const NEW_TAB: &str = "new_tab";

/// Why a script is refused while the switch is off.
pub const OFF: &str = "scripts are off";

/// Whether a script may open `url`: an http or https page, never a script's own JavaScript, a
/// file or the browser's own pages.
pub fn page_allowed(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// An entry's line in Settings › Agents, in the activity log's words: what it did, and on what.
pub fn title(entry: &Activity) -> String {
    let site = entry.site.as_ref();
    match entry.action.as_ref() {
        SHOW_SPACE => format!("Showed the Space {site}"),
        SHOW_TAB => format!("Showed the tab {site}"),
        NEW_WINDOW => "Opened a new window".into(),
        NEW_TAB if site.is_empty() => "Opened a page in a new tab".into(),
        NEW_TAB => format!("Opened {site} in a new tab"),
        _ => "Read the windows, Spaces and tabs".into(),
    }
}

/// The line under it: who sent it, and why it was refused.
pub fn detail(entry: &Activity) -> String {
    match entry.refused.is_empty() {
        true => entry.by.to_string(),
        false => format!("Refused: {} · {}", entry.refused, entry.by),
    }
}

impl Agents {
    /// Settings › Agents' "Let scripts control the browser".
    pub fn scripts_on(&self) -> bool {
        self.scripts
    }

    pub fn set_scripts_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on == self.scripts {
            return;
        }
        self.scripts = on;
        self.save();
        cx.notify();
    }

    /// The Apple Event `event`, from `by`, did `action` on `site`, refused for `refused` if that
    /// isn't empty, or for the switch being off. Returns whether it may go ahead. An event is one
    /// entry: its reads, as the objects its command names are found, go under the command.
    pub fn script(
        &mut self,
        event: u64,
        by: &str,
        action: &str,
        site: &str,
        refused: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let refused = match self.scripts {
            false => OFF,
            true => refused,
        };
        let entry = Activity {
            at: SystemTime::now(),
            agent: AGENT.into(),
            action: SharedString::from(action.to_string()),
            site: SharedString::from(site.to_string()),
            target: SharedString::default(),
            refused: SharedString::from(refused.to_string()),
            steps: Vec::new(),
            by: SharedString::from(by.to_string()),
            id: super::entry_id(),
            url: SharedString::default(),
            sent: Vec::new(),
        };
        let same = event != 0 && self.script_event == event;
        match self.log.iter().rposition(|each| each.agent == AGENT) {
            // A read under a command already logged, or another read of it, is that entry.
            Some(_) if same && action == READ => return refused.is_empty(),
            Some(at) if same => self.log[at] = entry,
            _ => {
                if self.log.len() >= LOG_LENGTH {
                    self.log.remove(0);
                }
                self.log.push(entry);
            }
        }
        self.script_event = event;
        // check.sh reads this line.
        let why = match refused.is_empty() {
            true => String::new(),
            false => format!(" refused: {refused}"),
        };
        log!("browser: script {action} {site} by {by}{why}");
        cx.notify();
        refused.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script opens only http and https pages: never its own JavaScript, a file or the
    /// browser's pages.
    #[test]
    fn a_script_opens_only_web_pages() {
        assert!(page_allowed("https://example.com/"));
        assert!(page_allowed("HTTP://example.com"));
        for url in [
            "javascript:alert(document.cookie)",
            "file:///etc/passwd",
            "chrome://settings",
            "data:text/html,hi",
            "about:blank",
            "example.com",
        ] {
            assert!(!page_allowed(url), "{url} was allowed");
        }
    }
}
