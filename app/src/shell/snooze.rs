//! Snoozing Today tabs (#2350): the tab's menu's Snooze and the chosen rows', Pick a Date and
//! Time…, the toast with Undo, and the dot on a tab back by itself.

use std::rc::Rc;

use chrono::NaiveDateTime;
use gpui::{App, Context, SharedString};

use super::{
    Shell,
    menus::{MenuAction, MenuEntry},
    undo::{Edit, What},
};
use crate::{
    dialog::{Field, Question},
    snooze::{self, Choice},
};

impl Shell {
    /// The core brought `tab` back from a snooze by itself, at the top of its Space's Today tabs:
    /// its row is marked unread until it's shown. Nothing else happens, and nothing takes focus.
    pub fn tab_unsnoozed(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.selected != tab && self.unsnoozed.insert(tab) {
            cx.notify();
        }
    }

    /// Whether `tab` can be snoozed: a Today tab of the user's own Space, in a regular window. A
    /// pinned tab or favorite stays, and a private, Little ilo or Blank window's tab and an
    /// agent's have no Space of the user's to come back to.
    pub(super) fn snoozable(&self, tab: u64) -> bool {
        !self.one_space()
            && self.little.is_none()
            && (self.shown_space())
                .is_some_and(|space| space.agent.is_empty() && space.today.contains(&tab))
    }

    /// Snooze's submenu, under the item at `parent`, for `tabs`: each time the short list has
    /// now, ending with when it means, then Pick a Date and Time….
    pub(super) fn snooze_items(
        &self,
        tabs: Vec<u64>,
        parent: usize,
    ) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let now = snooze::now();
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = Vec::new();
        for choice in Choice::ALL {
            let Some(at) = choice.at(now) else {
                continue;
            };
            let entry = MenuEntry {
                note: Some(snooze::short(at, now).into()),
                ..MenuEntry::new(choice.name(), true).under(parent)
            };
            let tabs = tabs.clone();
            items.push((
                entry,
                "snooze",
                Box::new(move |shell, cx| shell.snooze_tabs(tabs, at, cx)),
            ));
        }
        items.push((MenuEntry::line().under(parent), "", Box::new(|_, _| {})));
        items.push((
            MenuEntry::new("Pick a Date and Time…", true).under(parent),
            "snooze-pick",
            Box::new(move |shell, cx| shell.pick_snooze(tabs, cx)),
        ));
        items
    }

    /// Snoozes the Today tabs among `tabs` until `at`, as one step ⌘Z takes back, and says when
    /// they come back in a toast with Undo. `tabs` is as Close's step has them: they snooze last
    /// first, and come back first first.
    pub(super) fn snooze_tabs(
        &mut self,
        tabs: Vec<u64>,
        at: NaiveDateTime,
        cx: &mut Context<Self>,
    ) {
        let tabs: Vec<u64> = tabs
            .into_iter()
            .filter(|&tab| self.snoozable(tab))
            .collect();
        let what = match tabs.len() {
            0 => return,
            1 => What::SNOOZE_TAB,
            _ => What::SNOOZE_TABS,
        };
        let until = snooze::millis(at);
        // Chosen rows that snooze leave the choice, as Close Tabs clears it.
        if tabs.iter().any(|tab| self.chosen.contains(tab)) {
            self.chosen.clear();
            self.chosen_from = 0;
        }
        let back = Edit::Snooze { tabs, until }.apply(self, cx);
        if back.is_none() {
            return;
        }
        self.keep(what, back, cx);
        let words = snooze::words(at, snooze::now());
        self.offer_undo(&format!("Snoozed until {words}"), cx);
    }

    /// What the bar's Snooze Tab snoozes: the chosen rows' Today tabs, or the Today tab shown.
    pub(super) fn tabs_to_snooze(&self) -> Vec<u64> {
        match self.chosen.is_empty() {
            true => (self.snoozable(self.selected))
                .then(|| self.panes(self.selected).into_iter().rev().collect())
                .unwrap_or_default(),
            false => self.chosen_to_snooze(),
        }
    }

    /// The chosen rows' Today tabs, each split's pages too, as Close Tabs has them.
    pub(super) fn chosen_to_snooze(&self) -> Vec<u64> {
        let today = self.today();
        let rows = self.chosen_tabs();
        (rows.iter().filter(|row| today.contains(row)))
            .flat_map(|&row| self.panes(row).into_iter().rev())
            .collect()
    }

    /// Pick a Date and Time…: asks for a day and a time in the dialog New Profile asks for a name
    /// in, starting with Tomorrow Morning's, all selected. The line under the field says the time
    /// ilo reads as the user types, and Snooze waits while it reads none.
    pub(super) fn pick_snooze(&mut self, tabs: Vec<u64>, cx: &mut Context<Self>) {
        let now = snooze::now();
        let start = Choice::TomorrowMorning
            .at(now)
            .map(|at| snooze::full(at, now))
            .unwrap_or_default();
        let question = Question {
            icon: None,
            message: "Snooze Until".into(),
            detail: "A day and a time, such as “Friday 17:30” or “24 Nov 09:00”.".into(),
            list: Vec::new(),
            action: "Snooze".into(),
            cancel: "Cancel".into(),
            tone: crate::dialog::Tone::Accent,
            also: None,
            fields: vec![Field {
                placeholder: "Day and time".into(),
                text: start.into(),
                secure: false,
                reads: Some(Rc::new(|text: &str| {
                    let now = snooze::now();
                    snooze::read(text, now)
                        .map(|at| SharedString::from(snooze::full(at, now)))
                        .map_err(SharedString::from)
                })),
            }],
            check: None,
            on_return: crate::dialog::Return::Action,
        };
        let shell = cx.weak_entity();
        let picked = Box::new(move |texts: Vec<String>, cx: &mut App| {
            let typed = texts.first().map_or("", String::as_str);
            // Read again as it's answered, the clock having moved on since it was typed.
            if let Ok(at) = snooze::read(typed, snooze::now()) {
                shell
                    .update(cx, |shell, cx| shell.snooze_tabs(tabs, at, cx))
                    .ok();
            }
        });
        self.browser.tell(question, picked, cx);
    }
}
