//! The keyboard at a page's ends (#3220). Tab past a page's last control, or Shift+Tab before its
//! first, rings the shown tab's row, as Focus Sidebar does, and Tab or Shift+Tab from the ring
//! gives the page its first or last control, so the keyboard always has a place a screen reader
//! can say. With a task's page watched, the ring is the task's header at the sidebar's foot, whose
//! page shows, and the strip under it, in its own order: back to the conversation, the page, then
//! its chips. The Space's rows aren't stops then, as Return on one would swap in another page. The
//! foot's other stops, each task's header, its chip and its pages' rows, are the arrows', on from
//! the list's last row (task_cards.rs), so Tab's round stays as short as it was. An agent's page
//! the user doesn't hold never gets the keyboard from the ring: its stop is the strip's words,
//! which name the page as the agent's, and Take Over is its own stop. Where there's no row to ring
//! (Little ilo, a hidden sidebar, full screen, the Library, a page another window or
//! Peek shows) the app doesn't take the keyboard, and the page wraps.

use std::time::Duration;
use std::time::Instant;

use gpui::{Context, Role, SharedString, Stateful, div, prelude::*, px};

use super::{Shell, agent_bar::Bar, sidebar_keys::NEW_TAB};
use crate::tokens::shell::{KEYED, KEYED_RING};

/// A stop of the ring off the list's rows: at the sidebar's foot, or of the strip under a task's
/// page watched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum StripStop {
    /// A task's card in Today, by its task's number.
    Header(u64),
    /// The page: the panel's page row's words for an agent's page, the page itself for one the
    /// user holds.
    Page,
    TakeOver,
    HandBack,
    Stop,
    Open,
}

/// One key's press, shorter than a keyboard's repeat delay: a page with nothing to focus handing
/// straight back the Tab that went into it, which it answers with no key of its own between.
const SAME_PRESS: Duration = Duration::from_millis(200);

#[cfg(test)]
thread_local! {
    /// Where each stop was last drawn, for the tests: Take Over's arrival moves none of the others.
    pub(super) static STOPS_DRAWN: std::cell::RefCell<std::collections::HashMap<StripStop, gpui::Bounds<gpui::Pixels>>> =
        Default::default();
}

/// Records where `stop` is drawn, in the tests alone.
fn stop_drawn(stop: StripStop) -> Option<gpui::AnyElement> {
    #[cfg(test)]
    return Some(
        gpui::canvas(
            move |bounds, _, _| STOPS_DRAWN.with(|drawn| drawn.borrow_mut().insert(stop, bounds)),
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
        .into_any_element(),
    );
    #[cfg(not(test))]
    {
        let _ = stop;
        None
    }
}

/// Where the ring's Tab came from, kept until the page it went into answers.
#[derive(Default)]
pub(super) struct PageEnds {
    pub(super) entered: Option<(Instant, Option<StripStop>, u64)>,
}

impl StripStop {
    /// Whether it's a card's, in Today's list, which the arrows go over with the rows.
    pub(super) fn at_foot(self) -> bool {
        matches!(self, StripStop::Header(_))
    }
}

impl Shell {
    /// The core asks as Tab leaves the page of `tab` at its end, or its start if `reverse`:
    /// whether the app takes the keyboard, ringing the stop after or before the page.
    pub fn tab_focus_leaving(&mut self, tab: u64, reverse: bool, cx: &mut Context<Self>) -> bool {
        if !self.ring_reaches(tab) {
            log!("browser: tab {tab} focus left its page with no row to ring");
            return false;
        }
        self.take_keyboard(cx);
        self.focus_keys = true;
        // A page with nothing to focus hands the Tab that went in straight back: the ring stays
        // where it was, so one key never goes page, row, page.
        let entered = self.page_ends.entered.take();
        if let Some((_, strip, row)) = entered.filter(|(at, ..)| at.elapsed() < SAME_PRESS) {
            log!("browser: tab {tab} has nothing to focus");
            match strip {
                Some(stop) => self.key_strip(stop, cx),
                None => self.key_row_or_first(row, cx),
            }
            return true;
        }
        let stops = self.ring_stops(cx);
        let at = stops.iter().position(|&stop| stop == Some(StripStop::Page));
        let next = at.map(|at| match reverse {
            false => stops[(at + 1) % stops.len()],
            true => stops[(at + stops.len() - 1) % stops.len()],
        });
        match next.flatten() {
            Some(stop) => self.key_strip(stop, cx),
            None => self.key_row_or_first(self.row_of(tab), cx),
        }
        true
    }

    /// Tab or Shift+Tab with the ring on a row or the strip: the ring's next stop, which for the
    /// user's page is the page, at its first or last control.
    pub fn key_tab(&mut self, reverse: bool, cx: &mut Context<Self>) {
        let here = match (self.keyed, self.keyed_strip) {
            (Some(_), _) => None,
            (None, Some(stop)) => Some(stop),
            (None, None) => return cx.propagate(),
        };
        let stops = self.ring_stops(cx);
        // From a stop of the foot's that isn't the ring's own, Tab goes on from the header of the
        // task whose page is watched, and with none watched to the list's row.
        let header = || {
            let at_foot = here.is_some_and(StripStop::at_foot);
            let header = |stop: &Option<StripStop>| matches!(stop, Some(StripStop::Header(_)));
            stops.iter().position(header).filter(|_| at_foot)
        };
        let Some(at) = (stops.iter().position(|&stop| stop == here)).or_else(header) else {
            return self.key_ring_start(cx);
        };
        let next = match reverse {
            false => stops[(at + 1) % stops.len()],
            true => stops[(at + stops.len() - 1) % stops.len()],
        };
        match next {
            Some(StripStop::Page) if !self.agents_page_watched() => {
                self.enter_page(reverse, here, cx)
            }
            Some(stop) => self.key_strip(stop, cx),
            None => self.key_ring_start(cx),
        }
    }

    /// Tab with the window holding the keyboard while a task's page is watched: the strip's first
    /// stop, or with Shift+Tab its last, so the ring stays with the page watched. Says whether it
    /// rang one.
    pub fn tab_into_strip(&mut self, reverse: bool, cx: &mut Context<Self>) -> bool {
        let Some((_, tab)) = self.watched_task_page() else {
            return false;
        };
        if !self.ring_reaches(tab) {
            return false;
        }
        let stops = self.ring_stops(cx);
        let stop = match reverse {
            false => stops.first(),
            true => stops.last(),
        };
        let Some(&Some(stop)) = stop else {
            return false;
        };
        self.take_keyboard(cx);
        self.focus_keys = true;
        self.key_strip(stop, cx);
        true
    }

    /// Return on a strip's stop: what a click on it does. The agent's page's stop does nothing,
    /// as a press there would take the page over.
    pub(super) fn key_strip_show(&mut self, stop: StripStop, cx: &mut Context<Self>) {
        match stop {
            StripStop::Header(_) => self.key_card_show(stop, cx),
            StripStop::Page => {}
            StripStop::TakeOver | StripStop::HandBack => {
                let (Some(agent), Some((_, tab))) =
                    (self.task_strip_agent(), self.watched_task_page())
                else {
                    return;
                };
                log!("browser: command take-over-or-hand-back via strip keys");
                self.leave_strip("strip", cx);
                // What the page row's button does.
                let action = match stop {
                    StripStop::TakeOver => crate::agents::task::TaskAction::TakeOver(tab),
                    _ => crate::agents::task::TaskAction::HandBack(tab),
                };
                self.browser.task(agent, action, cx);
            }
            StripStop::Stop => {
                if let Some(agent) = self.task_strip_agent() {
                    log!("browser: command stop-task via strip keys");
                    self.browser
                        .task(agent, crate::agents::task::TaskAction::Stop, cx);
                }
            }
            StripStop::Open => {
                if let (Some(agent), Some((_, tab))) =
                    (self.task_strip_agent(), self.watched_task_page())
                {
                    self.leave_strip("open", cx);
                    log!("browser: command open-task-pages via strip keys");
                    let from = self.task_list.strip_open.get();
                    self.open_task_pages(agent, Some(tab), from, cx);
                }
            }
        }
    }

    /// The ring's stops in order, a row's as None: with a task's page watched, the task's header,
    /// then the page where the row's words stand for it or the user holds it, the row's button
    /// and an outside app's Stop; otherwise the row and the page.
    pub(super) fn ring_stops(&self, cx: &gpui::App) -> Vec<Option<StripStop>> {
        let Some((_, tab)) = self.watched_task_page() else {
            return vec![None, Some(StripStop::Page)];
        };
        let row = self.page_row_shown(tab, cx);
        // The page is a stop where the row's words stand for it, and wherever the user holds it,
        // as a page handed to them is with no words on the row, so the ring goes into it.
        let said = (row.case.as_ref()).and_then(|case| case.said()).is_some();
        let page = (said || self.strip_held()).then_some(StripStop::Page);
        let button = row.case.as_ref().map(|case| case.button());
        let stop = row.stop.then_some(StripStop::Stop);
        // The card while the list draws it, so the ring never sits where it can't be seen.
        let header = self
            .task_strip_agent()
            .filter(|agent| self.card_listed(agent))
            .map(|agent| StripStop::Header(super::task_cards::number(&agent)));
        header
            .into_iter()
            .chain(page)
            .chain(button)
            .chain(stop)
            .map(Some)
            .collect()
    }

    /// Whether the page watched is an agent's the user doesn't hold, which the ring never enters.
    pub(super) fn agents_page_watched(&self) -> bool {
        self.watched_task_page().is_some() && !self.strip_held()
    }

    /// Whether the user holds the page watched, as the strip's Hand Back says.
    fn strip_held(&self) -> bool {
        matches!(self.agent_bar(), Some(Bar::Held { .. }))
    }

    /// `element` as the strip's `stop`: named `name` for a screen reader, which says it as the
    /// ring's while the ring is on it, and ringed as a row is.
    pub(super) fn strip_stop(
        &self,
        element: Stateful<gpui::Div>,
        stop: StripStop,
        name: SharedString,
        radius: f32,
    ) -> Stateful<gpui::Div> {
        let keyed = self.keyed_strip == Some(stop);
        let ring = keyed.then(|| {
            div()
                .absolute()
                .inset_0()
                .rounded(px(radius))
                .border(px(KEYED_RING))
                .border_color(self.palette.plate(KEYED))
        });
        element
            .relative()
            .children(stop_drawn(stop))
            .children(ring)
            .when(self.a11y, |element| {
                let role = match stop {
                    StripStop::Page => Role::Group,
                    _ => Role::Button,
                };
                element
                    .role(role)
                    .aria_label(name)
                    .when(keyed, |element| element.aria_active_descendant())
            })
    }

    /// Whether Tab leaving `tab`'s page has a stop to ring: the page is the one the window shows,
    /// or the task's watched, and the sidebar is there to ring a row in.
    fn ring_reaches(&self, tab: u64) -> bool {
        let shown = match self.task_area.is_some() {
            true => self.agent_page_shows(tab),
            false => {
                let asked = self.tab_asked();
                tab == asked
                    || self
                        .split_of(asked)
                        .is_some_and(|(tabs, _)| tabs.contains(&tab))
            }
        };
        let room = !self.sidebar_hidden
            && !self.page_fullscreen
            && self.little().is_none()
            && self.library.is_none()
            && self.onboarding.is_none();
        shown && room
    }

    fn enter_page(&mut self, reverse: bool, from: Option<StripStop>, cx: &mut Context<Self>) {
        let row = self.keyed.unwrap_or_else(|| self.row_of(self.selected));
        self.page_ends.entered = Some((Instant::now(), from, row));
        self.keyed_strip = None;
        self.leave_keys("tab", cx);
        // check.sh reads this line.
        log!(
            "browser: ring gave the page its {} control",
            if reverse { "last" } else { "first" }
        );
        self.browser.focus_page_end(reverse, cx);
        cx.notify();
    }

    /// Rings the strip's `stop`, the UI keeping the keyboard.
    pub(super) fn key_strip(&mut self, stop: StripStop, cx: &mut Context<Self>) {
        self.keyed = None;
        self.leave_menu_keys("strip", cx);
        self.keyed_strip = Some(stop);
        log!("browser: strip keyed {}", strip_stop_name(stop));
        cx.notify();
    }

    /// Takes the ring off the strip, for `why`.
    pub(super) fn leave_strip(&mut self, why: &str, cx: &mut Context<Self>) {
        if self.keyed_strip.take().is_some() {
            log!("browser: strip keys left by {why}");
            cx.notify();
        }
    }

    /// Rings `row` if the ring goes over it, else the first row.
    fn key_row_or_first(&mut self, row: u64, cx: &mut Context<Self>) {
        self.keyed_strip = None;
        let rows = self.key_rows();
        let row = Some(row).filter(|row| rows.contains(row));
        if let Some(row) = row.or_else(|| rows.first().copied()) {
            self.key_row(row, cx);
        }
    }

    /// The ring's row stop: the tab shown's row, else the first, as Focus Sidebar rings.
    fn key_ring_start(&mut self, cx: &mut Context<Self>) {
        let shown = Some(self.row_of(self.selected)).filter(|row| *row != NEW_TAB);
        self.key_row_or_first(shown.unwrap_or(NEW_TAB), cx);
    }
}

/// How the log names a strip's stop.
fn strip_stop_name(stop: StripStop) -> &'static str {
    match stop {
        StripStop::Header(_) => "header",
        StripStop::Page => "page",
        StripStop::TakeOver => "take-over",
        StripStop::HandBack => "hand-back",
        StripStop::Stop => "stop",
        StripStop::Open => "open",
    }
}
