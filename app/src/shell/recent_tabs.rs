//! ⌃Tab, as in Arc: the Space's tab shown before this one, at once, and while ⌃ stays held the
//! switcher of its tabs, most recently shown first (#2954). A quick ⌃Tab is one switch with nothing
//! shown. Held past the switcher's delay, or pressed again before it, the switcher shows with the
//! tab now shown selected, and from then on the page doesn't change: Tab and → go on, ⇧Tab and ←
//! back, letting go of ⌃ or leaving the app switches once more to the row selected, a press on a
//! row to that one, and Escape or a click outside goes back to the tab the walk started from. So a
//! walk is two switches at most.

use std::rc::Rc;

use gpui::{App, Context, Task};

use crate::hover_card::Icon;
use crate::space_icon::has_icon;
use crate::tab_switcher::{Ask, DELAY, Heard, Item, Show};

use super::rows::speaker_glyph;
use super::{Shell, host};

/// A ⌃Tab walk under way, from the press until ⌃ goes up.
pub(super) struct Held {
    /// The tab shown as the walk started, which Escape goes back to.
    start: u64,
    /// The rows the switcher lists, most recently shown first, each by its first tab.
    rows: Vec<u64>,
    /// The row selected among them.
    at: usize,
    /// Whether the switcher shows, else it waits for DELAY.
    shown: bool,
    /// The delay's timer; dropping it cancels it.
    _delay: Option<Task<()>>,
}

impl Shell {
    /// ⌃Tab with `rows`, the sidebar's, or ⌃⇧Tab back (`forward` false). A split's row counts once.
    pub(super) fn walk(&mut self, rows: Vec<u64>, forward: bool, cx: &mut Context<Self>) {
        if let Some(held) = &mut self.held {
            let count = held.rows.len();
            held.at = step(held.at, forward, count);
            match held.shown {
                true => self.browser.tab_switcher(Ask::Select(held.at), cx),
                // A second ⌃Tab before the delay is intent: it shows now, and the page stays.
                false => self.show_switcher(cx),
            }
            return;
        }
        // The rows shown most recently first, then those not shown yet, in the sidebar's order.
        let mut order: Vec<u64> = Vec::new();
        for row in self
            .recent
            .iter()
            .map(|&tab| self.row_of(tab))
            .chain(rows.iter().copied())
        {
            if rows.contains(&row) && !order.contains(&row) {
                order.push(row);
            }
        }
        let count = order.len();
        let next = match order
            .iter()
            .position(|&row| row == self.row_of(self.selected))
        {
            Some(at) => step(at, forward, count),
            // With no tab shown, the tab shown last (Arc).
            None if forward && count > 0 => 0,
            None => match count.checked_sub(1) {
                Some(last) => last,
                None => return,
            },
        };
        let tab = self.row_tab(order[next]);
        if tab == self.selected {
            return;
        }
        if self.walked.is_none() {
            self.browser.watch_control(cx);
        }
        let start = self.selected;
        self.walked = Some(tab);
        self.select(tab, cx);
        let delay = cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(DELAY).await;
            shell.update(cx, |shell, cx| shell.show_switcher(cx)).ok();
        });
        self.held = Some(Held {
            start,
            rows: order,
            at: next,
            shown: false,
            _delay: Some(delay),
        });
    }

    /// ⌃ went up, which ends a ⌃Tab walk on the tab it shows. A click ends it too, and the core
    /// may report the tab the click showed before this runs, so the walk's tab counts first.
    pub fn control_released(&mut self, cx: &mut Context<Self>) {
        if self.held.as_ref().is_some_and(|held| held.shown) {
            // On the Mac a click outside the switcher ends the walk too, and the switcher's
            // dismissal, which stays, is posted beside this in either order: a turn later,
            // whichever is left decides.
            cx.spawn(async move |shell, cx| {
                shell.update(cx, |shell, cx| shell.let_go(cx)).ok();
            })
            .detach();
            return;
        }
        // A quick ⌃Tab: nothing showed, and its delay goes.
        self.held = None;
        if let Some(walked) = self.walked.take().filter(|tab| self.tabs.contains_key(tab)) {
            self.shown(walked, cx);
        }
        self.shown(self.selected, cx);
    }

    /// The walk's switcher, listing its rows with the one selected.
    fn show_switcher(&mut self, cx: &mut Context<Self>) {
        let Some(held) = &mut self.held else {
            return;
        };
        held._delay = None;
        if held.shown {
            return;
        }
        let (rows, at) = (held.rows.clone(), held.at);
        // Those that closed since the walk started are left out, the selection staying on its row.
        let open: Vec<usize> = (0..rows.len())
            .filter(|&each| self.tabs.contains_key(&self.row_tab(rows[each])))
            .collect();
        let Some(selected) = open
            .iter()
            .position(|&each| each >= at)
            .or(open.len().checked_sub(1))
        else {
            return self.end_walk(None, cx);
        };
        let rows: Vec<u64> = open.iter().map(|&each| rows[each]).collect();
        let items = rows.iter().map(|&row| self.switcher_item(row)).collect();
        if let Some(held) = &mut self.held {
            (held.rows, held.at, held.shown) = (rows, selected, true);
        }
        let shell = cx.weak_entity();
        // It tells inside its own update, so the shell hears after the update's end.
        let heard = Rc::new(move |heard: Heard, cx: &mut App| {
            let shell = shell.clone();
            cx.defer(move |cx| {
                shell.update(cx, |shell, cx| shell.heard(heard, cx)).ok();
            });
        });
        let show = Show {
            window: self.browser.window(),
            items,
            selected,
            heard,
        };
        self.browser.tab_switcher(Ask::Show(show), cx);
    }

    /// What the user did in the switcher.
    fn heard(&mut self, heard: Heard, cx: &mut Context<Self>) {
        let Some(held) = &mut self.held else {
            return;
        };
        match heard {
            Heard::Step(forward) => {
                held.at = step(held.at, forward, held.rows.len());
                let at = held.at;
                self.browser.tab_switcher(Ask::Select(at), cx);
            }
            Heard::Chose(at) => {
                let row = held.rows.get(at).copied();
                // check.sh reads this line.
                log!("browser: tab switcher chose {}", row.unwrap_or_default());
                let tab = row.map(|row| self.row_tab(row));
                self.end_walk(tab, cx);
            }
            Heard::Stay => {
                let (start, asked) = (held.start, self.tab_asked());
                // A click outside that showed a tab, on its sidebar row, is the user's choice, and
                // it's heard before the dismissal that comes with it.
                let start = match Some(asked) == self.walked {
                    true => start,
                    false => asked,
                };
                // check.sh reads this line.
                log!("browser: tab switcher stays on {start}");
                self.end_walk(Some(start), cx);
            }
            Heard::LetGo => self.let_go(cx),
        }
    }

    /// ⌃ went up, or the user left the app, with the switcher shown: its row selected shows.
    fn let_go(&mut self, cx: &mut Context<Self>) {
        let Some(held) = &self.held else {
            return;
        };
        let tab = held.rows.get(held.at).map(|&row| self.row_tab(row));
        self.end_walk(tab, cx);
    }

    /// Ends the walk on `to`: switched to unless it's shown or switching in, then counted as shown.
    fn end_walk(&mut self, to: Option<u64>, cx: &mut Context<Self>) {
        let Some(held) = self.held.take() else {
            return;
        };
        if held.shown {
            self.browser.tab_switcher(Ask::Close, cx);
        }
        self.walked = None;
        let asked = self.tab_asked();
        let to = to
            .filter(|tab| self.tabs.contains_key(tab))
            .unwrap_or(asked);
        if to != asked {
            // It counts as shown once the core shows it, as no walk is under way.
            self.select(to, cx);
        } else if to == self.selected {
            self.shown(to, cx);
        }
    }

    /// What the switcher's row for `row` shows: its tab, or its split as one.
    fn switcher_item(&self, row: u64) -> Item {
        let tab = self.row_tab(row);
        let panes = self.panes(row);
        let icon_of = |tab: u64| match self.tabs.get(&tab) {
            Some(each) if has_icon(&each.page.icon) => Icon::Emoji(each.page.icon.clone()),
            each => Icon::Favicon(each.and_then(|each| each.favicon.clone())),
        };
        let sleeping = panes.iter().all(|&pane| self.page(pane).unloaded);
        let speaker = (panes.iter().map(|&pane| self.page(pane)))
            .find(|page| page.audible || page.muted)
            .map(|page| speaker_glyph(page.muted));
        if panes.len() < 2 {
            return Item {
                tab,
                title: self.title(tab),
                host: host(&self.page(tab).url).to_string().into(),
                icon: icon_of(tab),
                glyph: None,
                sleeping,
                speaker,
            };
        }
        // A split is one row, as its plate with a name or an icon is in the sidebar: the icon, else
        // Lucide's columns-2 or rows-2, and the name, else its pages' titles joined by commas.
        let split = self.split(row).or_else(|| self.favorite_split(row));
        let (name, icon) = split.map_or_else(Default::default, |split| {
            (split.name.clone(), split.icon.clone())
        });
        let title = match name.is_empty() {
            true => (panes.iter().map(|&pane| self.title(pane)))
                .collect::<Vec<_>>()
                .join(", ")
                .into(),
            false => name,
        };
        let glyph = match self.stacked(row) {
            true => "icons/rows-2.svg",
            false => "icons/columns-2.svg",
        };
        Item {
            tab,
            title,
            host: Default::default(),
            icon: Icon::Emoji(icon.clone()),
            glyph: (!has_icon(&icon)).then_some(glyph),
            sleeping,
            speaker,
        }
    }
}

/// Row `at` of `count`, one on or back, wrapping.
fn step(at: usize, forward: bool, count: usize) -> usize {
    match forward {
        true => (at + 1) % count,
        false => (at + count - 1) % count,
    }
}
