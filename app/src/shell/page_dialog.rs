//! A page's own dialogs, such as its alert(), which the core reports for each tab in place of
//! Chrome's: the tab shown asks with the browser's dialog over its card, and the others' wait
//! until their tab shows again (docs/ux-spec.md › Page dialogs).

use std::fmt;

use gpui::{App, Bounds, Context, Pixels, SharedString, Window};

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
        self.page_dialog_up = None;
        // check.sh reads this line, which never has what was typed.
        log!("browser: page {tab} dialog answered {reply:?}");
        // A page that asks to stay stops a quit, and so a restart into an update.
        if stayed {
            crate::updates::restart_stopped(cx);
        }
        self.browser.answer_page_dialog(tab, id, reply, cx);
        cx.notify();
    }

    /// Shows the dialog of the tab shown over its card, once the window has drawn, or takes back
    /// the one shown once its tab no longer shows or its page no longer asks. A tab showing its
    /// page in Safari's engine keeps Chromium's dialog back, as its page isn't the one that asks,
    /// until the tab is back in Chromium. A dialog not yet up waits while the user types in a
    /// field of ilo's, the command bar's and find's among them, and goes up on the first draw
    /// after the field lets go; one up stays. A Leave doesn't wait: it answers the user's own
    /// close, quit or navigation.
    pub(super) fn show_page_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The user's page is out of the window while a task has the page area, and its dialog
        // waits for it. A task's page watched there shows its sign-in, which only the user may
        // answer (#3662); its other dialogs pause the agent until it closes the tab (#3536).
        let shown = match &self.task_area {
            None => Some(self.selected),
            Some(_) => self.watched_task_page().map(|(_, tab)| tab),
        };
        let asked = shown.and_then(|tab| Some((tab, self.page_dialogs.get(&tab)?)));
        let asked = asked
            .filter(|(_, dialog)| self.task_area.is_none() || dialog.kind == DialogKind::SignIn);
        let asked = asked.filter(|(tab, _)| !self.page(*tab).system_engine);
        // A Little ilo window's own page first, then what a tab it sent behind asks to open.
        let asked = asked.or_else(|| self.little_sent_asks());
        let asked = asked.map(|(tab, dialog)| (tab, dialog.clone()));
        let waits = asked.as_ref().is_some_and(|(tab, dialog)| {
            dialog.kind != DialogKind::Leave
                && self.page_dialog_up != Some((*tab, dialog.id))
                && self.typing(window, cx)
        });
        let waiting = asked
            .as_ref()
            .filter(|_| waits)
            .map(|(tab, dialog)| (*tab, dialog.id));
        if waiting.is_some() && waiting != self.page_dialog_waits {
            // check.sh waits for this line.
            let (tab, id) = waiting.unwrap_or_default();
            log!("browser: page {tab} dialog {id} waits for the field");
        }
        self.page_dialog_waits = waiting;
        let asked = asked.filter(|_| !waits);
        let wanted = match (asked, self.page_laid) {
            (Some((tab, dialog)), Some(over)) => Some((tab, dialog, over)),
            _ => None,
        };
        self.page_dialog_up = wanted.as_ref().map(|(tab, dialog, _)| (*tab, dialog.id));
        let key = wanted
            .as_ref()
            .map(|(tab, dialog, over)| (*tab, dialog.id, *over));
        if self.page_dialog_shown == Some(key) {
            return;
        }
        self.page_dialog_shown = Some(key);
        self.browser.show_page_dialog(wanted, cx);
    }

    /// A field of ilo's over the window let the keyboard go, which the window's own focus doesn't
    /// see, as the command bar's and find's are windows of their own: a page's dialog waiting for
    /// it goes up on the draw this asks for.
    pub fn field_let_go(&mut self, cx: &mut Context<Self>) {
        if self.page_dialog_waits.is_some() {
            cx.notify();
        }
    }

    /// The host said the page took the keyboard from the UI, which GPUI's focus doesn't see: a
    /// field still drawn with the focus no longer has the keys.
    pub(super) fn page_took_keyboard(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.page_keyboard = Some(window.focused(cx).map(|focused| focused.downgrade()));
        self.field_let_go(cx);
    }

    /// Gives the UI the keyboard, from the page if the page has it. Every way the shell takes it
    /// comes here, so the note that the page has the keys never outlives them.
    pub(crate) fn take_keyboard(&mut self, cx: &mut App) {
        self.page_keyboard = None;
        self.browser.focus_ui(cx);
    }

    /// With no page shown, gives the UI the keyboard until a page shows, as `take_keyboard`.
    pub(super) fn hold_keyboard_for_page(&mut self, cx: &mut App) {
        self.page_keyboard = None;
        self.browser.hold_keyboard_for_page(cx);
    }

    /// The app is the active one again. Windows' word that the page took the keyboard also comes
    /// as the app goes to the background, and its keys come back to the UI with it unsaid, so the
    /// word no longer holds: a field still drawn with the focus counts until the next press in the
    /// page, which the host tells again.
    pub fn app_came_back(&mut self) {
        self.page_keyboard = None;
    }

    /// Whether the page has the keyboard: as the core says where it keeps that, else since the
    /// host said the page took it, unless GPUI's focus has moved since, as the UI giving a field
    /// the keyboard moves it.
    fn page_has_keyboard(&self, window: &Window, cx: &App) -> bool {
        if let Some(page) = self.browser.page_has_keyboard(cx) {
            return page;
        }
        self.page_keyboard
            .as_ref()
            .is_some_and(|then| match (then, window.focused(cx)) {
                (Some(then), Some(now)) => *then == now,
                (None, None) => true,
                _ => false,
            })
    }

    /// Whether the user types in a field of ilo's in this window: one of the shell's while the UI
    /// has the keys, the command bar while it's open, or the find bar while it has the keyboard.
    fn typing(&self, window: &Window, cx: &mut Context<Self>) -> bool {
        (crate::text_input::typing_in(window, cx) && !self.page_has_keyboard(window, cx))
            || self.browser.command_bar_open(cx)
            || self.browser.field_window_typing(cx)
    }
}

#[cfg(test)]
mod tests;
