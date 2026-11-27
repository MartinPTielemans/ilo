//! The recents under the bar an empty Space rests on (#4140): up to five of the tabs closed and
//! the threads left in the profile of the Space shown, newest first, read from the Archive the
//! window already holds. ↓ from the bar with nothing typed moves into them, ↑ from the first or
//! Esc goes back to the field, and ↩ or a click opens a row: a tab as the Library's Reopen brings
//! it, a thread with its conversation open. A private window lists none.

use gpui::{
    App, Context, KeyBinding, SharedString, Window, accesskit::Role, actions, div, prelude::*, px,
};

use crate::agents::task::ArchivedTask;
use crate::tokens::command_bar::{DIM, LINE_COLOR, TITLE};
use crate::tokens::shell::composer::*;
use crate::tokens::shell::{FAVICON_DRAWN, HAIRLINE};
use crate::tokens::text;

use super::super::{Shell, favicon, knot::knot, library::Archived, library::history_when};

/// The recents' key context while they have the keyboard, under the Browser one, so their keys win
/// over the shortcuts on the same keys.
const CONTEXT: &str = "Recents";

actions!(recents, [RecentUp, RecentDown, RecentOpen, RecentLeave]);

pub(in crate::shell) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", RecentUp, Some(CONTEXT)),
        KeyBinding::new("down", RecentDown, Some(CONTEXT)),
        KeyBinding::new("enter", RecentOpen, Some(CONTEXT)),
        KeyBinding::new("escape", RecentLeave, Some(CONTEXT)),
    ]);
}

/// A row of the recents.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::shell) enum Recent {
    Tab {
        id: u64,
        url: SharedString,
        title: SharedString,
        at: f64,
    },
    Thread {
        agent: SharedString,
        title: SharedString,
        at: f64,
    },
}

impl Recent {
    /// When it closed or last worked, in milliseconds since the Unix epoch, or 0 if nothing says.
    fn at(&self) -> f64 {
        match self {
            Recent::Tab { at, .. } | Recent::Thread { at, .. } => *at,
        }
    }
}

/// The recents of `profile`, newest first, at most `RECENTS`: the tabs of `archive` closed in one
/// of its Spaces, never a Little Window's, and with `threads`, the tasks of `tasks` asked in one,
/// each Space's profile by `profile_of`. A tab the core's timer archived is listed by when it went,
/// as one the user closed is: the Archive doesn't say how a tab left until #4408.
pub(in crate::shell) fn recents(
    archive: &[Archived],
    tasks: &[ArchivedTask],
    profile_of: impl Fn(u64) -> Option<u64>,
    profile: u64,
    threads: bool,
) -> Vec<Recent> {
    let ours = |space: u64| space != 0 && profile_of(space) == Some(profile);
    let tabs = (archive.iter())
        .filter(|tab| !tab.little && ours(tab.space))
        .map(|tab| Recent::Tab {
            id: tab.id,
            url: tab.url.clone(),
            title: match tab.title.is_empty() {
                true => tab.url.clone(),
                false => tab.title.clone(),
            },
            at: tab.archived,
        });
    let tasks = (tasks.iter())
        .filter(|task| threads && ours(task.home))
        .map(|task| Recent::Thread {
            agent: task.agent.clone(),
            title: match task.title.is_empty() {
                true => task.asked.clone(),
                false => task.title.clone(),
            },
            at: task.ended,
        });
    let mut recents: Vec<Recent> = tabs.chain(tasks).collect();
    // Stable, so rows of one time keep the Archive's order.
    recents.sort_by(|a, b| b.at().total_cmp(&a.at()));
    recents.truncate(RECENTS as usize);
    recents
}

impl Shell {
    /// The recents the window lists under its resting bar: none in a private window, or one with
    /// no Space of the user's, and no threads while they're off.
    pub(in crate::shell) fn recents(&self, cx: &App) -> Vec<Recent> {
        let Some(profile) = self.learning_profile() else {
            return Vec::new();
        };
        let profile_of = |space: u64| {
            (self.spaces.iter())
                .find(|each| each.id == space)
                .map(|each| each.profile)
        };
        let threads = self.browser.threads(cx) == Some(true);
        recents(
            &self.archive,
            &self.archived_tasks,
            profile_of,
            profile,
            threads,
        )
    }

    /// ↓ in the resting bar with nothing to select: the recents take the keyboard at their first
    /// row, if there are any.
    pub(in crate::shell) fn into_recents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recents(cx).is_empty() {
            return;
        }
        let Some(resting) = &mut self.resting else {
            return;
        };
        resting.recent = 0;
        window.focus(&resting.recents, cx);
        log!("browser: recents have the keyboard");
        cx.notify();
    }

    /// ↑ or ↓ in the recents: the next row or the one before, and from the first, up to the field.
    fn step_recents(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.recents(cx).len();
        let Some(resting) = &mut self.resting else {
            return;
        };
        match (down, resting.recent) {
            (false, 0) => return self.focus_resting(window, cx),
            (false, at) => resting.recent = at - 1,
            (true, at) => resting.recent = (at + 1).min(count.saturating_sub(1)),
        }
        cx.notify();
    }

    /// ↩ or a click on the row at `index`: a tab comes back as the Library's Reopen brings it,
    /// and a thread with its conversation open.
    fn open_recent_row(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.recents(cx).into_iter().nth(index) {
            Some(Recent::Tab { id, .. }) => {
                self.reopen_archived(id, cx);
            }
            Some(Recent::Thread { agent, .. }) => self.restore_archived_task(agent, cx),
            None => {}
        }
    }

    /// The recents' plate, under the bar `width` wide, or None with nothing recent, as then the
    /// field stands alone with no heading. The row the keys are on is flat on the chosen plate
    /// while the recents have the keyboard.
    pub(in crate::shell) fn recents_plate(
        &mut self,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let recents = self.recents(cx);
        if recents.is_empty() {
            return None;
        }
        let resting = self.resting.as_ref()?;
        let focus = resting.recents.clone();
        let keyed = focus.is_focused(window).then_some(resting.recent);
        let palette = self.palette;
        let starts = self.browser.day_starts();
        let twelve = crate::clock::twelve_hour();
        let mut rows = Vec::new();
        for (index, recent) in recents.into_iter().enumerate() {
            if index > 0 {
                rows.push((div().h(px(HAIRLINE)).bg(palette.faint(LINE_COLOR))).into_any_element());
            }
            let (icon, title, site) = match &recent {
                Recent::Tab { url, title, .. } => {
                    // Asked for once a row first shows, as the Library's rows ask.
                    if !self.page_favicons.contains_key(url) {
                        self.page_favicons.insert(url.clone(), None);
                        self.browser.page_favicon(url, cx);
                    }
                    let icon = favicon(&palette, self.page_favicons.get(url).cloned().flatten());
                    (icon, title.clone(), crate::agents::site(url))
                }
                // The knot at rest: a thread in the Archive isn't at work.
                Recent::Thread { title, .. } => (
                    knot(FAVICON_DRAWN, &palette, None).into_any_element(),
                    title.clone(),
                    String::new(),
                ),
            };
            let when = match recent.at() {
                at if at > 0. => history_when(at, starts, twelve),
                _ => String::new(),
            };
            let row = div()
                .id(("recent", index))
                .debug_selector(move || format!("recent-{index}"))
                .when(self.a11y, |row| {
                    row.role(Role::Button).aria_label(title.clone())
                })
                .h(px(RECENT_HEIGHT))
                .px(px(RECENT_INSET))
                .flex()
                .items_center()
                .gap(px(RECENT_GAP))
                .text_size(px(text::BODY))
                .map(|row| match keyed == Some(index) {
                    true => row.bg(palette.flat_plate()),
                    false => row.hover(|style| style.bg(palette.plate(RECENT_HOVERED))),
                })
                .on_click(cx.listener(move |shell, _, _, cx| shell.open_recent_row(index, cx)))
                .child(div().flex_none().size(px(FAVICON_DRAWN)).child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap(px(RECENT_SITE_GAP))
                        .child(
                            div()
                                .min_w_0()
                                .whitespace_nowrap()
                                .truncate()
                                .text_color(palette.text(TITLE))
                                .child(title),
                        )
                        .when(!site.is_empty(), |line| {
                            line.child(
                                div()
                                    .flex_none()
                                    .text_size(px(text::SMALL))
                                    .text_color(palette.gray(DIM))
                                    .child(site),
                            )
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(text::SMALL))
                        .text_color(palette.gray(DIM))
                        .child(when),
                );
            rows.push(row.into_any_element());
        }
        Some(
            div()
                .id("recents")
                .debug_selector(|| "resting-recents".into())
                .track_focus(&focus)
                .key_context(CONTEXT)
                .when(self.a11y, |plate| plate.role(Role::List))
                .on_action(cx.listener(|shell, _: &RecentDown, window, cx| {
                    shell.step_recents(true, window, cx)
                }))
                .on_action(cx.listener(|shell, _: &RecentUp, window, cx| {
                    shell.step_recents(false, window, cx)
                }))
                .on_action(cx.listener(|shell, _: &RecentOpen, _, cx| {
                    let at = shell.resting.as_ref().map_or(0, |resting| resting.recent);
                    shell.open_recent_row(at, cx)
                }))
                .on_action(
                    cx.listener(|shell, _: &RecentLeave, window, cx| {
                        shell.focus_resting(window, cx)
                    }),
                )
                .w(px(width))
                .mt(px(RECENTS_GAP))
                .flex()
                .flex_col()
                .rounded(px(RECENTS_RADIUS))
                .bg(palette.plate(RECENTS_FILL))
                .border_1()
                .border_color(super::composer::rim(&palette, RECENTS_RIM))
                .overflow_hidden()
                .children(rows),
        )
    }
}
