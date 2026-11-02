//! A page's own dialogs, such as its alert(), which the core reports for each tab in place of
//! Chrome's: the tab shown asks with the browser's dialog over its card, and the others' wait
//! until their tab shows again (docs/ux-spec.md › Page dialogs).

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

/// The dialog shown over the card, by its tab and id, and where.
pub type ShownPageDialog = (u64, PageDialog, (Bounds<Pixels>, f32));

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
        if self.page_dialogs.get(&tab).is_none_or(|dialog| dialog.id != id) {
            return;
        }
        self.page_dialogs.remove(&tab);
        self.page_dialog_shown = None;
        // check.sh reads this line, which never has what was typed.
        log!("browser: page {tab} dialog answered {reply:?}");
        self.browser.answer_page_dialog(tab, id, reply, cx);
        cx.notify();
    }

    /// Shows the dialog of the tab shown over its card, from the window's draw, or takes back the
    /// one shown once its tab no longer shows or its page no longer asks. A tab showing its page in
    /// Safari's engine keeps Chromium's dialog back, as its page isn't the one that asks, until the
    /// tab is back in Chromium.
    pub(super) fn show_page_dialog(&mut self, cx: &mut Context<Self>) {
        let asked = self.page_dialogs.get(&self.selected);
        let asked = asked.filter(|_| !self.page(self.selected).system_engine);
        // The user's page is out of the window while a task has the page area, and its dialog
        // waits for it.
        let asked = asked.filter(|_| self.task_area.is_none());
        let wanted = match (asked, self.page_laid) {
            (Some(dialog), Some(over)) => Some((self.selected, dialog.clone(), over)),
            _ => None,
        };
        let key = wanted.as_ref().map(|(tab, dialog, over)| (*tab, dialog.id, *over));
        if self.page_dialog_shown == key {
            return;
        }
        self.page_dialog_shown = key;
        self.browser.show_page_dialog(wanted, cx);
    }
}

#[cfg(test)]
mod tests;
