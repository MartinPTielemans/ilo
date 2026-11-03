//! The tab's history from the back and forward buttons, as Arc's: a right-click or a long press
//! lists the pages that way in ilo's menu, nearest first, with their favicons, and ⌘-click or a
//! middle click on back, forward or reload opens where it goes in a background tab.

use std::{sync::Arc, time::Duration};

use gpui::{Context, Image, Modifiers, Pixels, Point, SharedString};

use super::{Command, MenuEntry, Shell, menus::MenuAction};

/// How long back or forward is held before its menu opens: Chrome's, *interim* until Arc's is
/// measured.
pub(super) const HOLD: Duration = Duration::from_millis(500);

/// A page in a tab's history, as the core lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryPage {
    /// How far it is from the tab's page: -1 is where back goes, 1 where forward goes.
    pub offset: i32,
    pub url: SharedString,
    pub title: SharedString,
    pub favicon: Option<Arc<Image>>,
}

/// Where a ⌘-click on `command`'s button goes: the page that far from the tab's, 0 for reload's.
fn offset_of(command: Command) -> Option<i32> {
    match command {
        Command::Back => Some(-1),
        Command::Forward => Some(1),
        Command::Reload => Some(0),
        _ => None,
    }
}

impl Shell {
    /// Asks for the pages back from the tab shown, or forward of it, for a menu at `position`.
    pub(super) fn ask_history(
        &mut self,
        forward: bool,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let command = if forward {
            Command::Forward
        } else {
            Command::Back
        };
        if self.selected == 0 || !self.offers(command) {
            return;
        }
        self.history_asked = Some((self.selected, forward, position));
        self.browser.tab_history(self.selected, forward, cx);
    }

    /// The pages `ask_history` asked for, which its menu lists while their tab is shown. Picking
    /// one goes there.
    pub fn show_history(
        &mut self,
        tab: u64,
        forward: bool,
        pages: Vec<HistoryPage>,
        cx: &mut Context<Self>,
    ) {
        let Some((asked, way, position)) = self.history_asked else {
            return;
        };
        if (asked, way) != (tab, forward) {
            return;
        }
        self.history_asked = None;
        if pages.is_empty() || self.selected != tab {
            return;
        }
        let items = (pages.into_iter())
            .map(|page| {
                // As Chrome's menu names a page without a title.
                let title = if page.title.is_empty() {
                    page.url
                } else {
                    page.title
                };
                let offset = page.offset;
                let go: MenuAction = Box::new(move |shell, cx| {
                    log!("browser: command history {offset} via sidebar");
                    shell.browser.go_to_history(tab, offset, cx)
                });
                (MenuEntry::named(title, true).page(page.favicon), go)
            })
            .collect();
        self.show_menu(items, position, None, cx);
    }

    /// Opens where `command`'s button goes in a background tab, as ⌘-click or a middle click on
    /// back, forward or reload does.
    pub(super) fn open_history(&mut self, command: Command, cx: &mut Context<Self>) {
        let Some(offset) = offset_of(command) else {
            return;
        };
        if self.selected == 0 || !self.offers(command) {
            return;
        }
        // check.sh waits for this line.
        log!(
            "browser: command {} in the background via sidebar",
            command.name()
        );
        self.browser.open_history(self.selected, offset, cx);
    }

    /// The left button went down on `command`'s button: back or forward held for HOLD lists the
    /// history that way at `position`.
    pub(super) fn press_nav(
        &mut self,
        command: Command,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.nav_held = false;
        let forward = match command {
            Command::Back => false,
            Command::Forward => true,
            _ => return,
        };
        self.nav_hold = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(HOLD).await;
            shell
                .update(cx, |shell, cx| {
                    shell.nav_hold = None;
                    shell.nav_held = true;
                    // The menu takes the press over, so the button grows back as it opens.
                    shell.let_go_nav(cx);
                    shell.ask_history(forward, position, cx);
                })
                .ok();
        }));
    }

    /// The left button came up, before HOLD a click.
    pub(super) fn release_nav(&mut self) {
        self.nav_hold = None;
    }

    /// A click on `command`'s button, unless it was held for its menu.
    pub(super) fn click_nav(
        &mut self,
        command: Command,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.nav_hold = None;
        if std::mem::take(&mut self.nav_held) {
            return;
        }
        match modifiers.secondary(cx) && offset_of(command).is_some() {
            true => self.open_history(command, cx),
            false => self.run(command, "sidebar", cx),
        }
    }
}

#[cfg(test)]
#[path = "history/tests.rs"]
mod tests;
