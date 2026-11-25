//! What the shell asks of a sidebar tab's hover card (crate::hover_card): which tab it's for, what
//! it shows of it and which buttons it has, the calls they make, and when the tabs' pictures are
//! taken, which is after a run of switches is over, never on the switch.

use std::rc::Rc;

use gpui::{App, Bounds, Context, Pixels, Point, point, px, size};

use std::time::Duration;

use crate::folder_preview::{self, Ask, Item};
use crate::hover_card::{Action, Anchor, Card, Event, Hover, Icon, Opening, Rest};
use crate::recent::{self, Answer};
use crate::space_icon::has_icon;
use crate::{icon_picker, theme_picker};

use super::recent_downloads::{age, now};
use super::{PLATE_INSET, REVEAL_INSET, Row, Shell, host};

/// How long the switches are quiet before a page they hid is pictured: past a Space switch's
/// slide, which has settled by 300 ms, and long enough to take quick switches as one. A page the
/// core still keeps on screen then, as a switch does until the new one draws, it holds as
/// captured until the copy answers (core/capture.cc), so the picture needn't wait it out (#1658).
pub(super) const PICTURE_HIDDEN: Duration = Duration::from_millis(400);
impl Shell {
    /// The pointer came onto `row` or left it: a tab's row or tile rests for its card, and leaving
    /// it lets the card go. Folders, split views' rows and New Tab have none, and none shows while
    /// a row is dragged or has its menu up, a name field is open, with its menu or not (#1452), or
    /// the theme or icon picker shows over the sidebar.
    pub(super) fn hover_card(&self, row: Row, hovered: bool, cx: &mut Context<Self>) {
        let Row::Tab(tab) = row else {
            return;
        };
        if !hovered {
            return self.browser.hover_card(Hover::Left(tab), cx);
        }
        let picker = theme_picker::is_open(cx)
            || icon_picker::is_open(cx)
            || crate::boost_editor::is_open(cx);
        let naming = self.renaming.is_some();
        let busy = self.dragging().is_some() || self.menu_row.is_some() || naming || picker;
        let Some(card) = self.card(tab).filter(|_| !busy) else {
            return;
        };
        let card = self.with_recent(card, cx);
        let Some(frame) = self.tip_frame(tab) else {
            return;
        };
        // A tile's card starts where a row's would, past the sidebar's rows, not over the tiles
        // beside it: on their right, or with the sidebar on the right, on their left.
        let origin = self.sidebar_origin.get().x + self.reveal_offset();
        let row = match self.right() {
            true => {
                let left = (origin + px(PLATE_INSET)).min(frame.left());
                Bounds::from_corners(point(left, frame.top()), frame.bottom_right())
            }
            false => {
                let right = origin + px(self.rows_width() - PLATE_INSET);
                Bounds::new(
                    frame.origin,
                    size(right.max(frame.right()) - frame.origin.x, frame.size.height),
                )
            }
        };
        let shell = cx.weak_entity();
        // It may tell of the card inside this update, so what it tells waits for the update's end.
        let events = Rc::new(move |tab: u64, event: Event, cx: &mut App| {
            let shell = shell.clone();
            cx.defer(move |cx| {
                shell
                    .update(cx, |shell, cx| shell.card_event(tab, event, cx))
                    .ok();
            });
        });
        let anchor = Anchor {
            row,
            window: self.viewport,
            left: self.right(),
        };
        let window = self.browser.window();
        self.browser.hover_card(
            Hover::Rest(Rest {
                window,
                card,
                anchor,
                events,
            }),
            cx,
        );
    }

    /// The pointer came onto `row` or left it: a closed folder's row rests for its preview of the
    /// tabs in it, and leaving it lets the preview go. None shows while a row is dragged, has its
    /// menu up or is being named, or a folder is opening or closing.
    pub(super) fn folder_preview(&mut self, row: Row, hovered: bool, cx: &mut Context<Self>) {
        let Row::Tab(folder) = row else {
            return;
        };
        // A Live Folder's tooltip says how old its rows are in place of a preview.
        if !self.is_folder(folder) || self.live_source(folder).is_some() {
            return;
        }
        if !hovered {
            // The hover lost with the pointer last moved onto the row isn't the pointer leaving:
            // the browser window loses it with no move as the preview takes the keyboard. A move
            // off the row tells the preview instead.
            if self.on_row(folder, self.pointer_at) {
                self.preview_row = Some(folder);
                return;
            }
            return self.browser.folder_preview(Ask::Left(folder), cx);
        }
        self.preview_row = None;
        // Nor while a press opens or closes a folder, as the pointer that came with the press is
        // heard after it, with the folder still closed until the core says.
        let busy = self.dragging().is_some()
            || self.menu_row.is_some()
            || self.renaming.is_some()
            || self.folding.is_some();
        let items = self.previewed(folder);
        if busy || items.is_empty() {
            return;
        }
        let Some(row) = self.tip_frame(folder) else {
            return;
        };
        let shell = cx.weak_entity();
        // It tells of the choice inside its own update, so the shell's waits for the update's end.
        let chosen = Rc::new(move |tab: u64, cx: &mut App| {
            let shell = shell.clone();
            cx.defer(move |cx| {
                shell
                    .update(cx, |shell, cx| shell.show_previewed(tab, cx))
                    .ok();
            });
        });
        let rest = folder_preview::Rest {
            window: self.browser.window(),
            folder,
            name: self.folder_name(folder),
            items,
            anchor: Anchor {
                row,
                window: self.viewport,
                left: self.right(),
            },
            chosen,
        };
        self.browser.folder_preview(Ask::Rest(rest), cx);
    }

    /// The pointer moved or pressed at `at`: off the row whose preview hasn't heard it left, it now
    /// hears.
    pub(super) fn pointer_to(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.pointer_at = at;
        if let Some(folder) = self.preview_row.filter(|&folder| !self.on_row(folder, at)) {
            self.preview_row = None;
            self.browser.folder_preview(Ask::Left(folder), cx);
        }
    }

    /// Whether `at` is on the row of `folder`, as last drawn.
    fn on_row(&self, folder: u64, at: Point<Pixels>) -> bool {
        self.tip_frames
            .borrow()
            .get(&folder)
            .is_some_and(|row| row.contains(&at))
    }

    /// What a closed `folder`'s preview lists: the tabs in it and in the folders in it, as their
    /// rows say them, the one a window showed last first, each with how long ago that was, as
    /// Arc's does, or nothing for an open folder, whose rows show.
    pub(super) fn previewed(&self, folder: u64) -> Vec<Item> {
        if self.shown_space().is_none_or(|space| space.is_open(folder)) {
            return Vec::new();
        }
        let (now, today) = (now(), self.browser.day_starts()[0]);
        let mut tabs: Vec<(u64, f64)> = (self.folder_tabs(folder).into_iter())
            .map(|tab| (tab, self.browser.last_seen(tab)))
            .collect();
        // Those seen at the same time, or never, keep the sidebar's order.
        tabs.sort_by(|a, b| b.1.total_cmp(&a.1));
        let item = |(tab, seen): (u64, f64)| {
            let each = self.tabs.get(&tab);
            let icon = match each.filter(|each| has_icon(&each.page.icon)) {
                Some(each) => Icon::Emoji(each.page.icon.clone()),
                None => Icon::Favicon(each.and_then(|each| each.favicon.clone())),
            };
            let age = if seen > 0. {
                age(seen, now, today).into()
            } else {
                Default::default()
            };
            Item {
                tab,
                title: self.title(tab),
                url: self.url(tab),
                icon,
                age,
            }
        };
        tabs.into_iter().map(item).collect()
    }

    /// Shows the tab chosen in a folder's preview, which peeks under its closed folder, with the
    /// page focused, as a click on its row would.
    fn show_previewed(&mut self, tab: u64, cx: &mut Context<Self>) {
        if !self.tabs.contains_key(&tab) {
            return;
        }
        log!("browser: command select via folder preview");
        self.select(tab, cx);
        self.browser.focus_page(cx);
    }

    /// What `tab`'s card shows changed, as its name does when it's renamed: a card that shows or
    /// waits for it shows it as it is now (#1452).
    pub(super) fn card_changed(&self, tab: u64, cx: &mut App) {
        if let Some(card) = self.card(tab) {
            let card = self.with_recent(card, cx);
            self.browser.hover_card(Hover::Changed(card), cx);
        }
    }

    /// `card` with what's recent on its tab's site, where it has rows.
    fn with_recent(&self, card: Card, cx: &App) -> Card {
        let recent = self.recent_profile(card.tab, cx).map(|profile| {
            let (now, today) = (now(), self.browser.day_starts()[0]);
            recent::shown(profile, |at| age(at, now, today), cx)
        });
        Card { recent, ..card }
    }

    /// The profile whose answers `tab`'s card shows, for a tab whose card has what's recent on its
    /// site (#3074): a pinned tab or a favorite alone, not a split, whose page is on a site with
    /// rows that neither switch turned off, in a Space of the user's in a window that keeps its
    /// tabs, while the core can ask. The core asks the same of the tab itself.
    fn recent_profile(&self, tab: u64, cx: &App) -> Option<u64> {
        let space = self.shown_space()?;
        let kept = self.pinned().contains(&tab) || self.favorites.contains(&tab);
        let alone = self.panes(tab).len() <= 1 && self.favorite_split(tab).is_none();
        let site = recent::source(&self.page(tab).url)?;
        let ours = space.agent.is_empty() && !self.one_space();
        (self.browser.recent_reached() && kept && alone && ours && recent::shows(site, cx))
            .then_some(space.profile)
    }

    /// The site whose cards' rows `tab`'s page is on, for a pinned tab or favorite of the user's.
    pub fn recent_site(&self, tab: u64) -> Option<&'static str> {
        let kept = self.pinned().contains(&tab) || self.favorites.contains(&tab);
        let ours = self.shown_space()?.agent.is_empty() && !self.one_space();
        (self.browser.recent_reached() && kept && ours)
            .then(|| recent::source(&self.page(tab).url))
            .flatten()
    }

    /// Site Controls' switch for `tab`'s site: on or not, none while Settings' is off.
    pub fn recent_switch(&self, tab: u64, cx: &App) -> Option<bool> {
        let site = self.recent_site(tab).filter(|_| recent::on(cx))?;
        Some(recent::shows(site, cx))
    }

    /// The core's answer for `tab`'s card: kept for its profile, and the card shows it if it's up.
    pub fn recent_answered(&mut self, tab: u64, answer: Answer, cx: &mut Context<Self>) {
        let Some(profile) = self.recent_profile(tab, cx) else {
            return;
        };
        // check.sh reads this line, which says nothing of the rows.
        let said = match &answer {
            Answer::Rows { rows, .. } => format!("{} rows", rows.len()),
            Answer::SignedOut => "signed out".into(),
            Answer::Failed => "failed".into(),
        };
        log!("browser: tab {tab} recent {said}");
        recent::keep(profile, answer, cx);
        self.card_changed(tab, cx);
    }

    /// A row of what's recent on `tab`'s card was clicked: it opens in the tab, which leaves its
    /// pinned address and shows its way back, behind it, or beside the page.
    fn open_recent(
        &mut self,
        tab: u64,
        row: recent::Row,
        opening: Opening,
        cx: &mut Context<Self>,
    ) {
        log!("browser: command recent-{} via hover card", opening.label());
        if let Some(profile) = self.recent_profile(tab, cx) {
            recent::opened(profile, &row, cx);
        }
        let url = row.url();
        match opening {
            Opening::Behind => self.browser.open_behind(&[url], false, 0, 0, cx),
            Opening::Beside if self.selected != 0 && self.selected != tab => self
                .browser
                .open_split_at(&url, self.selected, false, false, cx),
            _ => self.follow_shown(tab, &url, cx),
        }
    }

    /// Loads `url` in `tab`, and shows it with the page focused, as a click on its row would.
    fn follow_shown(&mut self, tab: u64, url: &str, cx: &mut Context<Self>) {
        self.browser.follow(tab, url, cx);
        self.select(tab, cx);
        self.browser.focus_page(cx);
    }

    /// What `tab`'s card shows, if it has one. A favorite split's tile has one too, its pages
    /// side by side, the title the page focused last's and the hosts every page's in their order;
    /// a split's row has none.
    pub(super) fn card(&self, tab: u64) -> Option<Card> {
        let split = self
            .favorite_split(tab)
            .filter(|split| split.tabs[0] == tab);
        if tab == 0
            || self.is_folder(tab)
            || (self.panes(tab).len() > 1 && split.is_none())
            || !self.tabs.contains_key(&tab)
        {
            return None;
        }
        let icon_of = |tab: u64| match self.tabs.get(&tab) {
            Some(each) if has_icon(&each.page.icon) => Icon::Emoji(each.page.icon.clone()),
            each => Icon::Favicon(each.and_then(|each| each.favicon.clone())),
        };
        let host_of = |tab: u64| host(&self.page(tab).url).to_string();
        let (panes, focused) = match split {
            Some(split) => (split.tabs.clone(), self.favorite_focused(tab)),
            None => (Vec::new(), tab),
        };
        let host = match panes.is_empty() {
            true => host_of(tab),
            false => (panes.iter().map(|&pane| host_of(pane)))
                .collect::<Vec<_>>()
                .join(" · "),
        };
        Some(Card {
            tab,
            title: self.title(focused),
            host: host.into(),
            icon: icon_of(focused),
            // The page shown is on screen beside it.
            pictured: !self.on_screen(tab) && !panes.iter().any(|&pane| self.on_screen(pane)),
            paused: self.frozen(tab),
            actions: self.card_actions(tab),
            panes: panes.iter().map(|&pane| (pane, icon_of(pane))).collect(),
            focused,
            ring: self.palette.accent.into(),
            recent: None,
        })
    }

    /// The buttons on `tab`'s card, each the call the row's menu or its drag makes: a Today tab
    /// pins and a pinned one unpins, either becomes a favorite, and a favorite leaves the
    /// favorites, unless the Space shown is an agent's, as its menu has it, or there's no room for
    /// it among them (MOST_FAVORITES), where its menu dims Add to Favorites. A private window
    /// keeps no tab, so it has none of those. Split puts the tab beside the page shown, where its
    /// drag onto the page could, which a favorite's can't.
    pub(super) fn card_actions(&self, tab: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        let favorite = self.favorites.contains(&tab);
        if !self.one_space() {
            match favorite {
                true => actions.push(Action::Unfavorite),
                false if self.pinned().contains(&tab) => actions.push(Action::Unpin),
                false => actions.push(Action::Pin),
            }
            if !favorite && self.has_room_for(tab) {
                actions.push(Action::Favorite);
            }
        }
        if !favorite && self.split_partner(tab).is_some() {
            actions.push(Action::Split);
        }
        // Chrome offers it on every tab but those the share is from and of.
        if self.page(tab).share_instead {
            actions.push(Action::ShareInstead);
        }
        actions
    }

    /// Whether `tab`'s page is on screen in this window, alone or in the split view shown.
    pub(super) fn on_screen(&self, tab: u64) -> bool {
        self.selected != 0 && !self.new_tab && self.panes(self.selected).contains(&tab)
    }

    pub(super) fn card_event(&mut self, tab: u64, event: Event, cx: &mut Context<Self>) {
        match event {
            // A loaded tab's picture is taken again, as the page is now, and each of a favorite
            // split's. A page Battery Saver froze keeps the picture it had: taking one again would
            // thaw it.
            Event::Shown => {
                // Only the user's rest asks the site, and the core floors how often.
                if self.recent_profile(tab, cx).is_some() {
                    self.browser.fetch_recent(tab, cx);
                }
                for tab in self.panes(tab) {
                    if self.tabs.contains_key(&tab)
                        && !self.on_screen(tab)
                        && !self.page(tab).unloaded
                        && !self.frozen(tab)
                    {
                        self.browser.card_picture(tab, self.scale, cx);
                    }
                }
            }
            // Off the card, the pointer may be past a hidden sidebar's panel too, which the reveal
            // knows once the move that took it off is through.
            Event::Pointer(on) => {
                self.pointer_on_card = on;
                if !on {
                    cx.spawn(async move |shell, cx| {
                        shell
                            .update(cx, |shell, cx| {
                                if !shell.pointer_on_card {
                                    shell.pointer_at(shell.reveal_x, cx);
                                }
                            })
                            .ok();
                    })
                    .detach();
                }
            }
            Event::Act(action) => {
                log!("browser: command {} via hover card", action.label());
                match action {
                    Action::Pin => self.pin_tab(tab, true, cx),
                    Action::Unpin => self.pin_tab(tab, false, cx),
                    Action::Favorite => self.favorite_tab(tab, usize::MAX, cx),
                    Action::Unfavorite => self.move_tab_to(tab, false, 0, 0, cx),
                    Action::Split => {
                        if let Some(with) = self.split_partner(tab) {
                            self.join_split(tab, with, false, false, cx);
                        }
                    }
                    Action::ShareInstead => self.browser.share_instead(tab, cx),
                }
            }
            Event::Open(row, opening) => self.open_recent(tab, row, opening, cx),
            Event::SignIn => {
                log!("browser: command recent-sign-in via hover card");
                self.follow_shown(tab, "https://github.com/login", cx);
            }
        }
    }

    /// A press, a scroll, a menu or a Space switch ends the tooltip and the card.
    pub(super) fn end_hovers(&self, cx: &mut App) {
        self.browser.tooltip(None, cx);
        self.browser.hover_card(Hover::Close, cx);
        self.browser.folder_preview(Ask::Close, cx);
    }

    /// The page the popovers point into is going, as a swipe or a wheel turns it, which sends no
    /// click outside them that would close them: they all go, hovers and menus and pickers alike.
    pub(super) fn leave_popovers(&self, cx: &mut App) {
        self.end_hovers(cx);
        crate::menu::dismissed(cx);
        crate::icon_picker::leave(cx);
        crate::theme_picker::leave(cx);
        crate::boost_editor::leave(cx);
        crate::site_controls::dismissed(cx);
    }

    /// The tab shown was `before`'s pages and is now another's: those it hid are pictured once
    /// the switches have been quiet for `PICTURE_HIDDEN`, so nothing lands on the switch, and a
    /// run of switches pictures only what it left hidden.
    pub(super) fn hid_for_cards(&mut self, before: Vec<u64>, cx: &mut Context<Self>) {
        let shown = self.panes(self.selected);
        self.card_hidden.extend(
            before
                .into_iter()
                .filter(|tab| *tab != 0 && !shown.contains(tab)),
        );
        if self.card_hidden.is_empty() {
            return;
        }
        self.card_wait = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(PICTURE_HIDDEN).await;
            shell.update(cx, |shell, cx| shell.picture_hidden(cx)).ok();
        }));
    }

    fn picture_hidden(&mut self, cx: &mut Context<Self>) {
        self.card_wait = None;
        let mut hidden = std::mem::take(&mut self.card_hidden);
        hidden.dedup();
        for tab in hidden {
            let open = self.tabs.get(&tab).is_some_and(|each| !each.page.unloaded);
            if open && !self.on_screen(tab) {
                self.browser.card_picture(tab, self.scale, cx);
            }
        }
    }
}

#[cfg(test)]
mod tests;

impl Shell {
    /// Where `id`'s tile or folder row was last drawn, in the browser window. The reveal's window
    /// draws the hidden sidebar, at the browser window's left edge, or with the sidebar on the
    /// right, at its right edge, where its points are `reveal_offset` from the browser window's.
    fn tip_frame(&self, id: u64) -> Option<Bounds<Pixels>> {
        let frame = self.tip_frames.borrow().get(&id).copied()?;
        Some(frame + point(self.reveal_offset(), px(0.)))
    }

    /// How far across the browser window the reveal's window starts while it shows the sidebar at
    /// rest: 0 on the left, past the page on the right.
    pub(super) fn reveal_offset(&self) -> Pixels {
        match self.right() && self.sidebar_hidden && self.reveal_shown() {
            true => self.viewport.width - px(REVEAL_INSET + self.width()),
            false => px(0.),
        }
    }
}
