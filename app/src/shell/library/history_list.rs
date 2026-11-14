//! History's visited pages (#2337): the profile's own history, beside the closed tabs, asked for a
//! page at a time once the Library has drawn, and again as its list nears its end. The app keeps
//! nothing of them past the open Library: they're read from the profile's history each time, and a
//! visit an agent's tab made is told by the past tasks Library › Agents keeps (crate::agent_history).

use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use chrono::{Local, NaiveDate, TimeZone};
use gpui::{
    AnyElement, ClickEvent, Context, Modifiers, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, div, prelude::*, px, svg,
};

use super::{
    ARCHIVED_FAVICON_X, ARCHIVED_LEFT, ARCHIVED_PITCH, ARCHIVED_PLATE, ARCHIVED_RADIUS,
    ARCHIVED_RIGHT, ARCHIVED_TEXT_X, MenuEntry, Only, SECONDARY, STATE_HOVERED, STATE_PRESSED,
    Section, Shell, UNSELECTED_TITLE, favicon, host, text, text_line,
};
use crate::agent_history::{self, Past};
use crate::shell::access::Accessible;
use crate::tokens::shell::{CLOSE_GLYPH, CLOSE_HOVERED, CLOSE_RIGHT, CLOSE_SIZE, NAV_RADIUS};

/// How many pages History asks for at a time: a few screens of its rows.
const PAGE: usize = 60;
/// It asks for older pages once a row this near its list's end draws.
const NEAR_END: usize = 10;

/// Each window's requests have their own ids, so a window hears only its own answers.
static REQUESTS: AtomicU64 = AtomicU64::new(1);

/// A page the profile's history has, as the browser lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Visited {
    pub url: SharedString,
    pub title: SharedString,
    /// Its last visit that day, in milliseconds since the Unix epoch.
    pub visited: f64,
}

/// A visited page History lists: once a day, on its calendar day, and with the agent whose task's
/// tab visited it.
pub(super) struct Visit {
    pub(super) page: Visited,
    pub(super) day: Option<NaiveDate>,
    pub(super) by: Option<SharedString>,
}

/// The History row under the pointer, which shows its trash: a closed tab's by its id, or a visited
/// page's by its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Pointed {
    Closed(u64),
    Visited(usize),
}

/// What a request asked for.
struct Asked {
    request: u64,
    /// It's for the pages older than those listed, not the list from its top.
    older: bool,
    text: String,
}

/// The open Library's visited pages.
#[derive(Default)]
pub(super) struct History {
    /// The profile they're of: the Space shown's, none in a window without one.
    profile: Option<u64>,
    /// Newest first, each once a day.
    pub(super) pages: Vec<Visit>,
    listed: HashSet<(SharedString, Option<NaiveDate>)>,
    /// When the Library came, until its first frame, after which it asks.
    pub(super) opening: Option<Instant>,
    drawn: bool,
    asked: Option<Asked>,
    /// The search the pages are of, once answered.
    text: Option<String>,
    /// Older pages are left to ask for.
    more: bool,
    /// Some of what's listed was deleted, so the list is asked for again from its top.
    stale: bool,
    /// The profile has visited pages, which the Filter button picks out: kept through a search
    /// that finds none.
    pub(super) any: bool,
    /// The profile's agents' past tasks of the last 30 days, read once.
    pasts: Option<Vec<Past>>,
    /// The row under the pointer.
    pub(super) pointed: Option<Pointed>,
}

impl History {
    pub(super) fn new(profile: Option<u64>) -> Self {
        Self {
            profile,
            opening: Some(Instant::now()),
            ..Self::default()
        }
    }

    /// Whether the pages are the search `query`'s, as the browser found them.
    pub(super) fn found(&self, query: &str) -> bool {
        self.text.as_deref() == Some(query)
    }

    /// The time before which History lists nothing yet, while older pages are left: what's older
    /// comes with them, so each page only adds to the list's end.
    pub(super) fn horizon(&self) -> Option<f64> {
        match self.more && self.text.is_some() {
            true => self.pages.last().map(|visit| visit.page.visited),
            false => None,
        }
    }
}

/// The calendar day of `at`, in milliseconds since the Unix epoch, where the user is.
fn day(at: f64) -> Option<NaiveDate> {
    Local
        .timestamp_millis_opt(at as i64)
        .single()
        .map(|time| time.date_naive())
}

/// `day` from its start to the next's, in microseconds since the Unix epoch, as the core removes
/// a page's visits by.
pub(super) fn day_span(day: NaiveDate) -> Option<(i64, i64)> {
    // Where a clock change skips midnight, the day starts when the clock comes back.
    let start = |day: NaiveDate| {
        (0..3)
            .find_map(|hour| {
                Local
                    .from_local_datetime(&day.and_hms_opt(hour, 0, 0)?)
                    .earliest()
            })
            .map(|time| time.timestamp_micros())
    };
    Some((start(day)?, start(day.succ_opt()?)?))
}

/// When a History row's tab closed or its page was visited, under its day's heading: the time
/// today and yesterday, with the weekday this week and last, and the date before. Times are on the
/// system's clock, 12- or 24-hour (`twelve`).
pub(super) fn history_when(at: f64, starts: [f64; 4], twelve: bool) -> String {
    let Some(time) = Local.timestamp_millis_opt(at as i64).single() else {
        return String::new();
    };
    let hour = crate::clock::hour_minute(twelve);
    let format = match starts.iter().position(|&start| at >= start) {
        Some(0 | 1) => hour.to_string(),
        Some(_) => format!("%a {hour}"),
        None => "%-d %b".to_string(),
    };
    time.format(&format).to_string()
}

/// A History row's title over what it is and when, which truncate before the row's trash while it
/// shows, and otherwise end 12 from the row's end.
pub(super) fn history_text(
    title: SharedString,
    about: SharedString,
    at_rest: bool,
    palette: &crate::palette::Palette,
) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .justify_center()
        .pl(px(ARCHIVED_TEXT_X))
        .when(at_rest, |text| text.pr(px(12.)))
        .child(text_line(
            title,
            text::BODY,
            17.,
            palette.text(UNSELECTED_TITLE),
        ))
        .child(text_line(about, text::SMALL, 15., palette.text(SECONDARY)))
}

/// The agent whose past task's tab visited `page`: one that recorded the page, and ran when it was
/// last visited that day. A task's pages are the pages its tabs ended at, so a page a tab went
/// through on its way shows unmarked, as does a visit after its task went from Library › Agents,
/// 30 days after it ended.
fn visited_by(pasts: &[Past], page: &Visited) -> Option<SharedString> {
    let at = page.visited;
    (pasts.iter())
        .find(|past| {
            let (started, ended) = (past.started as f64 * 1000., (past.ended + 1) as f64 * 1000.);
            started <= at && at < ended && past.pages.iter().any(|(_, url)| *url == *page.url)
        })
        .map(|past| past.agent.clone().into())
}

impl Shell {
    /// The pointer came onto a History row, or left it: only the row under it shows its trash.
    pub(super) fn point_history(&mut self, row: Pointed, hovered: bool, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        let pointed = &mut library.history.pointed;
        // Off it, only its own: the pointer may be on another's already.
        let now = match hovered {
            true => Some(row),
            false => pointed.filter(|on| *on != row),
        };
        if *pointed != now {
            *pointed = now;
            // check.sh waits for this line, which tells it which kind of row is under the pointer.
            match now {
                Some(Pointed::Closed(id)) => log!("browser: history pointed closed {id}"),
                Some(Pointed::Visited(line)) => log!("browser: history pointed visited {line}"),
                None => log!("browser: history pointed none"),
            }
            cx.notify();
        }
    }

    /// A History row's trash: the sidebar's close button, the same control in the same place, 5
    /// inside the plate's end, with History's glyph, as the owner's "trash removes" has it
    /// (11-02). It shows only under the pointer, and the title truncates before it then, as a tab
    /// row's does before its ✕, so at rest a title has the row's whole width.
    pub(super) fn history_trash(
        &self,
        id: impl Into<gpui::ElementId>,
        name: &'static str,
        remove: impl Fn(&mut Shell, &mut Context<Shell>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .when(self.a11y, |button| button.button_named(name))
            .flex_none()
            .ml(px(4.))
            .mr(px(CLOSE_RIGHT))
            .size(px(CLOSE_SIZE))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path("icons/trash.svg")
                    .size(px(CLOSE_GLYPH))
                    .text_color(self.palette.text(SECONDARY)),
            )
            // Not the row's press, which would open what it lists.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                remove(shell, cx)
            }))
    }

    /// The Library drew its first frame: History asks for its pages now, off the way the Library
    /// opens.
    pub(super) fn library_drawn(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        library.history.drawn = true;
        if self.section == Section::Archive {
            self.filter(true, cx);
        }
    }

    /// Asks for History's pages from the top, when the list hasn't them for the search typed, or
    /// some of them were deleted: once the Library has drawn, for the profile of the Space shown.
    /// A browser whose history hasn't loaded gets asked again at the next change.
    pub(super) fn ask_visited(&mut self, cx: &mut Context<Self>) {
        let profile = self.learning_profile();
        let Some(library) = &mut self.library else {
            return;
        };
        let query = library.search.read(cx).text().trim().to_lowercase();
        let history = &mut library.history;
        // Another profile's Space shows: its own history, from nothing.
        if history.profile != profile {
            let drawn = history.drawn;
            *history = History {
                drawn,
                ..History::new(profile)
            };
            history.opening = None;
        }
        let Some(profile) = profile.filter(|_| history.drawn) else {
            return;
        };
        let asking = history.asked.as_ref().filter(|asked| !asked.older);
        let current = match asking {
            Some(asked) => asked.text == query,
            None => history.found(&query) && !history.stale,
        };
        if current {
            return;
        }
        let max = match history.stale {
            true => PAGE.max(history.pages.len()),
            false => PAGE,
        };
        let request = REQUESTS.fetch_add(1, Ordering::Relaxed);
        if self
            .browser
            .list_history(profile, request, 0, &query, max, cx)
        {
            history.stale = false;
            history.asked = Some(Asked {
                request,
                older: false,
                text: query,
            });
        }
    }

    /// Asks for the pages older than History lists, as its list nears its end, unless it has them
    /// all or is asking already.
    fn ask_older_visited(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        let history = &mut library.history;
        let (Some(profile), Some(text)) = (history.profile, history.text.clone()) else {
            return;
        };
        if !history.more || history.asked.is_some() || history.stale || text != library.query {
            return;
        }
        let Some(oldest) = history.pages.last() else {
            return;
        };
        // Back to the core's microseconds, which the milliseconds hold exactly.
        let before = (oldest.page.visited * 1000.).round() as i64;
        let request = REQUESTS.fetch_add(1, Ordering::Relaxed);
        if self
            .browser
            .list_history(profile, request, before, &text, PAGE, cx)
        {
            history.asked = Some(Asked {
                request,
                older: true,
                text,
            });
        }
    }

    /// The pages History asked for with `request`, newest first, and whether older ones are left.
    /// Another window's answer, or one a later request replaced, is left.
    pub fn history_listed(
        &mut self,
        profile: u64,
        request: u64,
        pages: Vec<Visited>,
        more: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(library) = &mut self.library else {
            return;
        };
        let history = &mut library.history;
        if history.profile != Some(profile)
            || history
                .asked
                .as_ref()
                .is_none_or(|asked| asked.request != request)
        {
            return;
        }
        let Some(asked) = history.asked.take() else {
            return;
        };
        let browser = self.browser.clone();
        let pasts = history.pasts.get_or_insert_with(|| {
            let pasts = agent_history::read(&browser.agent_history(profile));
            agent_history::living(pasts, crate::usage::now())
        });
        if !asked.older {
            history.pages.clear();
            history.listed.clear();
        }
        for page in pages {
            let day = day(page.visited);
            // A page the last page listed that day, from before its visit there.
            if !history.listed.insert((page.url.clone(), day)) {
                continue;
            }
            let by = visited_by(pasts, &page);
            history.pages.push(Visit { page, day, by });
        }
        history.any |= !history.pages.is_empty();
        history.more = more;
        history.text = Some(asked.text);
        self.filter(true, cx);
    }

    /// Some of `profile`'s history was deleted: History lists what's left.
    pub fn history_forgotten(&mut self, profile: u64, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        if library.history.profile != Some(profile) {
            return;
        }
        library.history.stale = true;
        if self.section == Section::Archive {
            self.filter(true, cx);
        }
    }

    /// The Space shown changed, and History lists its profile's pages, if it's another's.
    pub(in crate::shell) fn history_space_shown(&mut self, cx: &mut Context<Self>) {
        let profile = self.learning_profile();
        if self.section == Section::Archive
            && (self.library.as_ref()).is_some_and(|library| library.history.profile != profile)
        {
            self.filter(true, cx);
        }
    }

    /// History's line `index` drew: near the list's end, while it lists visited pages, it asks for
    /// the older ones.
    pub(super) fn history_row_drawn(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(library) = &self.library else {
            return;
        };
        if self.section == Section::Archive
            && index + NEAR_END >= library.items.len()
            && matches!(self.archive_only(), Only::All | Only::Visited)
        {
            self.ask_older_visited(cx);
        }
    }

    /// A visited page's line in History, as a closed tab's: its favicon, its title over when it
    /// was visited, by an agent's tab if one did, and its host. A click opens it in Peek, and a
    /// right-click shows its menu.
    pub(super) fn visited_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(library) = &self.library else {
            return div().into_any_element();
        };
        let Some(visit) = library.history.pages.get(index) else {
            return div().into_any_element();
        };
        let url = visit.page.url.clone();
        let title = match visit.page.title.is_empty() {
            true => url.clone(),
            false => visit.page.title.clone(),
        };
        let when = history_when(
            visit.page.visited,
            library.starts,
            crate::clock::twelve_hour(),
        );
        let about = match &visit.by {
            Some(agent) => format!("Visited {when} by {agent} · {}", host(&url)),
            None => format!("Visited {when} · {}", host(&url)),
        };
        let day = visit.day;
        let pointed = library.history.pointed == Some(Pointed::Visited(index));
        if !self.page_favicons.contains_key(&url) {
            self.page_favicons.insert(url.clone(), None);
            self.browser.page_favicon(&url, cx);
        }
        let icon = favicon(
            &self.palette,
            self.page_favicons.get(&url).cloned().flatten(),
        );
        let (open, menu, removed) = (url.clone(), url.clone(), url);
        let trash = pointed.then(|| {
            let remove = move |shell: &mut Shell, cx: &mut Context<Shell>| {
                shell.remove_visited(&removed, day, cx)
            };
            self.history_trash(("visited-trash", index), "Remove from History", remove, cx)
        });
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("visited", index))
                    .size_full()
                    .relative()
                    .flex()
                    .items_center()
                    .rounded(px(ARCHIVED_RADIUS))
                    .block_mouse_except_scroll()
                    .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                        shell.point_history(Pointed::Visited(index), *hovered, cx)
                    }))
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                    .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        shell.open_visited(open.clone(), event.position(), event.modifiers(), cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.visited_menu(menu.clone(), day, event.position, cx)
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(icon),
                    )
                    .child(history_text(
                        title,
                        about.into(),
                        trash.is_none(),
                        &self.palette,
                    ))
                    .children(trash),
            )
            .into_any_element()
    }

    /// A click on a visited page's row opens it in Peek, growing from the row at `at`, as a closed
    /// tab's does, so a click means one thing down the list; Open as Tab keeps it. ⌘-click, or a
    /// page Peek can't show, opens it at once.
    fn open_visited(
        &mut self,
        url: SharedString,
        at: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let web = url.starts_with("https://") || url.starts_with("http://");
        let page = self.preview_page().filter(|_| {
            web && self.selected != 0 && !modifiers.secondary(cx) && self.browser.link_previews()
        });
        let Some(page) = page else {
            return self.open_visited_tab(&url, cx);
        };
        // check.sh waits for this line. The page isn't logged: it's the user's.
        log!("browser: history page in preview");
        self.browser.open_preview(self.selected, url, at, page, cx);
    }

    /// Opens a visited page in a new tab of the Space shown, and closes the Library.
    fn open_visited_tab(&mut self, url: &str, cx: &mut Context<Self>) {
        log!("browser: history open");
        self.browser.open_at(&[url.to_string()], false, 0, 0, cx);
        self.set_library(false, cx);
    }

    /// The menu a right-click on a visited page's row shows: open it in a new tab, copy its URL,
    /// or take that day's visits to it out of the profile's history.
    fn visited_menu(
        &mut self,
        url: SharedString,
        day: Option<NaiveDate>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let entries = vec![
            MenuEntry::new("Open in New Tab", true),
            MenuEntry::new("Copy URL", true),
            MenuEntry::line(),
            MenuEntry::new("Remove from History", true).destructive(),
        ];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            self.menu_look(),
            Box::new(move |chosen, cx| {
                shell
                    .update(cx, |shell, cx| match chosen {
                        Some(0) => shell.open_visited_tab(&url, cx),
                        Some(1) => {
                            let link = shell.copied_link(&url);
                            shell.copy(&link, "Copied URL", cx)
                        }
                        Some(3) => shell.remove_visited(&url, day, cx),
                        _ => {}
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Deletes the profile's visits to `url` on `day`, the row's, and the row goes at once.
    fn remove_visited(&mut self, url: &str, day: Option<NaiveDate>, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        let history = &mut library.history;
        let (Some(profile), Some((begin, end))) = (history.profile, day.and_then(day_span)) else {
            return;
        };
        if !(self.browser).remove_history(profile, url, begin, end, cx) {
            return;
        }
        history
            .pages
            .retain(|visit| !(visit.page.url == url && visit.day == day));
        log!("browser: history removed a page");
        self.filter(true, cx);
    }
}
