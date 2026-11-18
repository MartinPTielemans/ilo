//! A page's own dialogs, such as its alert(), which the core reports for each tab in place of
//! Chrome's: the tab shown asks with the browser's dialog over its card, or its pane in a split
//! view, where the pane the user isn't in keeps its page's at rest, and the others' wait until
//! their tab shows again (docs/ux-spec.md › Page dialogs).

use std::fmt;

use gpui::{Bounds, Context, Pixels, SharedString};

use super::Shell;

/// What a page's dialog asks, as the core reports it, in Chrome's words.
#[derive(Clone, PartialEq, Debug)]
pub struct PageDialog {
    pub id: u64,
    pub kind: DialogKind,
    /// Such as "example.com says".
    pub title: SharedString,
    pub message: SharedString,
    /// What a prompt's field starts with.
    pub text: SharedString,
    pub accept: SharedString,
    /// Empty for none, as an alert has only OK.
    pub cancel: SharedString,
    /// A checkbox's label, or empty for none.
    pub check: SharedString,
    /// A sign-in's labels for its fields, else empty.
    pub username: SharedString,
    pub password: SharedString,
}

/// What a page's dialog is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt,
    /// Whether to leave the page, which its beforeunload handler asks.
    Leave,
    /// The user's name and password for the site's HTTP sign-in.
    SignIn,
    /// Whether the page may open another app for a link, such as Mail for mailto:.
    OpenApp,
}

/// The user's answer to a page's dialog. What was typed is for the page alone, so it's left out
/// of what Debug shows, and so of logs and test output.
pub struct PageReply {
    pub accept: bool,
    pub text: String,
    pub password: String,
    pub checked: bool,
}

impl fmt::Debug for PageReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let answer = if self.accept { "accept" } else { "cancel" };
        write!(f, "{answer}")?;
        if self.checked {
            write!(f, ", checked")?;
        }
        Ok(())
    }
}

/// The dialog shown over the card or a pane, by its tab and id, where, and whether it rests, over
/// a split view's pane the user isn't in, without the keyboard (#3461).
pub type ShownPageDialog = (u64, PageDialog, (Bounds<Pixels>, f32), bool);

impl Shell {
    /// The core reports the dialog `tab`'s page asks, or that it asks none.
    pub fn set_page_dialog(
        &mut self,
        tab: u64,
        dialog: Option<PageDialog>,
        cx: &mut Context<Self>,
    ) {
        match dialog {
            Some(dialog) => self.page_dialogs.insert(tab, dialog),
            None => self.page_dialogs.remove(&tab),
        };
        cx.notify();
    }

    /// The user answered the dialog `id` of `tab`'s page, once the dialog has gone.
    pub fn answer_page_dialog(
        &mut self,
        tab: u64,
        id: u64,
        reply: PageReply,
        cx: &mut Context<Self>,
    ) {
        if self
            .page_dialogs
            .get(&tab)
            .is_none_or(|dialog| dialog.id != id)
        {
            return;
        }
        let stayed = self
            .page_dialogs
            .remove(&tab)
            .is_some_and(|dialog| dialog.kind == DialogKind::Leave && !reply.accept);
        self.page_dialog_shown = Some(None);
        // check.sh reads this line, which never has what was typed.
        log!("browser: page {tab} dialog answered {reply:?}");
        // A page that asks to stay stops a quit, and so a restart into an update.
        if stayed {
            crate::updates::restart_stopped(cx);
        }
        self.browser.answer_page_dialog(tab, id, reply, cx);
        cx.notify();
    }

    /// Shows the dialog of the tab shown over its card, from the window's draw, or takes back the
    /// one shown once its tab no longer shows or its page no longer asks. In a split view the
    /// focused pane's page asks first, and while it asks nothing another pane's rests over its
    /// pane, so the user sees what that page waits on (#3461). A tab showing its page in Safari's
    /// engine keeps Chromium's dialog back, as its page isn't the one that asks, until the tab is
    /// back in Chromium.
    pub(super) fn show_page_dialog(&mut self, cx: &mut Context<Self>) {
        let panes = self.panes(self.selected);
        let asked = std::iter::once(self.selected)
            .chain(panes.iter().copied().filter(|&tab| tab != self.selected))
            .find_map(|tab| {
                let dialog = self.page_dialogs.get(&tab)?;
                (!self.page(tab).system_engine).then_some((tab, dialog))
            });
        // The user's page is out of the window while a task has the page area, and its dialog
        // waits for it.
        let asked = asked.filter(|_| self.task_area.is_none());
        // A Little ilo window's own page first, then what a tab it sent behind asks to open.
        let asked = asked.or_else(|| self.little_sent_asks());
        let wanted = match (asked, self.page_laid) {
            (Some((tab, dialog)), Some(laid)) => Some((
                tab,
                dialog.clone(),
                self.pane_laid(tab, laid),
                self.dialog_rests(tab),
            )),
            _ => None,
        };
        let key = wanted
            .as_ref()
            .map(|(tab, dialog, over, rests)| (*tab, dialog.id, *over, *rests));
        if self.page_dialog_shown == Some(key) {
            return;
        }
        self.page_dialog_shown = Some(key);
        self.browser.show_page_dialog(wanted, cx);
    }

    /// Whether `tab`'s dialog rests: it's in a split view's pane the user isn't in, so the page
    /// beside keeps the keyboard and the window's shortcuts.
    fn dialog_rests(&self, tab: u64) -> bool {
        tab != self.selected && self.panes(self.selected).contains(&tab)
    }

    /// Focuses `tab`'s pane, for a click on its dialog at rest, which then goes live with it.
    pub fn focus_dialog_pane(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.dialog_rests(tab) {
            self.select(tab, cx);
        }
    }

    /// Where `tab`'s page is in the page area `laid`: its pane in a split view, as Chrome shows a
    /// tab's dialog over its own page, so the page beside stays usable (#3231), else all of it.
    fn pane_laid(&self, tab: u64, laid: (Bounds<Pixels>, f32)) -> (Bounds<Pixels>, f32) {
        let panes = self.panes(self.selected);
        let Some(index) = panes.iter().position(|&pane| pane == tab) else {
            return laid;
        };
        if panes.len() < 2 {
            return laid;
        }
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        (
            super::panes::pane_frames(laid.0, panes.len(), stacked, &shares)[index],
            laid.1,
        )
    }
}

#[cfg(test)]
mod tests;
