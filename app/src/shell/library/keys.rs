//! The Library with the keyboard (#3238). Its key or command, or a section's, gives the section's
//! search field the keyboard as it opens, so a screen reader, which follows the keyboard, says
//! where it is, as the command bar's field does. Spaces has no field: a ring goes on its first
//! card instead, as Focus Sidebar's does on a row, left and right move it along the cards and
//! return shows the ringed card's Space, as a click on it does. Escape still closes the Library,
//! as neither binds it. A swipe or a click on the Library moves no keyboard. Closed by its key, a
//! section's or Escape, the Library gives the keyboard back to whoever had it as it opened, the
//! page or the UI (#3313), unless the page shown since is an agent's; closed any other way, to the
//! window.
//!
//! From the field, down and up ring the list's rows as the command bar's keys go over its
//! suggestions, wrapping at the ends, and return does what a click on the ringed row does. The
//! field keeps the keyboard, to go on typing, so the ringed row is said as the bar's selected one
//! is: an option of the list, selected, and a polite live region with its name and where it
//! stands. Media's tiles are each a stop, in reading order, as left and right are the field's.
//! Headings, Agents' Clear and a past task's sites aren't rows to open, so the ring passes them.

use gpui::{
    App, Bounds, Context, Focusable, KeyBinding, Modifiers, Pixels, Role, SharedString, Stateful,
    Window, accesskit::Live, actions, canvas, div, point, prelude::*, px,
};

use super::{DownloadAction, DownloadState, Item, Medium, Section, Shell};
use crate::dismissal::{By, Close, FocusReturn, Returned, Why};
use crate::tokens::shell::{KEYED, KEYED_RING, library::CARD_RADIUS};

/// Spaces' key context while its cards have the keyboard.
const CARDS: &str = "LibraryCards";
/// The search field's, whose keys go over the list's rows.
pub(super) const ROWS: &str = "LibraryRows";
/// The Library's own, round all of it, where Tab goes over its controls.
const PANEL: &str = "Library";

actions!(
    library_keys,
    [
        CardLeft,
        CardRight,
        CardShow,
        RowDown,
        RowUp,
        RowOpen,
        NextStop,
        PreviousStop
    ]
);

/// The Library's keyboard, between the command that opens it and its close.
#[derive(Default)]
pub(in crate::shell) struct LibraryKeys {
    /// A command opened the Library or showed a section: its field or first card takes the
    /// keyboard on the next frame.
    asked: bool,
    /// It opened the Library, and whether its key came from the page.
    opened: Option<bool>,
    /// The keyboard's way back to whoever had it as the Library took it.
    back: FocusReturn,
    /// How it closed, said by its key, a section's or Escape; any other close is a choice in it
    /// or another surface taking over.
    closed: Option<Close>,
}

pub(in crate::shell) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", CardLeft, Some(CARDS)),
        KeyBinding::new("right", CardRight, Some(CARDS)),
        KeyBinding::new("enter", CardShow, Some(CARDS)),
        KeyBinding::new("down", RowDown, Some(ROWS)),
        KeyBinding::new("up", RowUp, Some(ROWS)),
        KeyBinding::new("enter", RowOpen, Some(ROWS)),
        KeyBinding::new("tab", NextStop, Some(PANEL)),
        KeyBinding::new("shift-tab", PreviousStop, Some(PANEL)),
    ]);
}

/// Who ran a command, by where it came from (`Shell::run`): a key in the UI or the page, a click in
/// the sidebar, or the app.
fn by(from: &str) -> By {
    match from {
        "ui" | "page" => By::Keyboard,
        "sidebar" => By::Pointer,
        _ => By::Code,
    }
}

impl Shell {
    /// Runs a command that opens the Library or shows one of its sections, whose field or first
    /// card then takes the keyboard, or closes it. `from` is where its key came from (`run`).
    pub(in crate::shell) fn library_command(
        &mut self,
        from: &str,
        cx: &mut Context<Self>,
        run: impl FnOnce(&mut Self, &mut Context<Self>),
    ) {
        let open = self.library_open;
        run(self, cx);
        let keys = &mut self.library_keys;
        keys.asked = self.library_open;
        if !open && self.library_open {
            keys.opened = Some(from == "page");
        }
        keys.closed = (open && !self.library_open).then(|| Close::new(Why::Toggled, by(from)));
    }

    /// Escape closes the Library, giving the keyboard back as its key does, unless it was an input
    /// method's, composing in its field (dismissal's rule).
    pub(in crate::shell) fn escape_library(&mut self, cx: &mut Context<Self>) {
        let search = (self.library.as_ref()).map(|library| library.search.clone());
        let Some(close) = Close::escape_from(&search, cx) else {
            return;
        };
        self.library_keys.closed = Some(close);
        self.set_library(false, cx);
    }

    /// Before the frame's drawn: gives the section shown the keyboard a command asked for, ends
    /// Spaces' ring once something else has the keyboard, and gives the keyboard back as the
    /// Library closes.
    pub(super) fn step_library_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.spaces.first().map(|space| space.id);
        let Some(library) = &self.library else {
            return;
        };
        let (search, cards) = (library.search.focus_handle(cx), library.cards.clone());
        let keys = &mut self.library_keys;
        let asked = std::mem::take(&mut keys.asked) && self.library_open;
        // Who has the keyboard as the Library takes it: the page, if the key came from it, as
        // GPUI's focus doesn't see the page's, else the UI's focus.
        if let Some(from_page) = keys.opened.take().filter(|_| asked) {
            keys.back = FocusReturn::remember(from_page, window, cx);
        }
        if asked {
            // The page may have the keyboard.
            self.take_keyboard(cx);
        }
        let Some(library) = &mut self.library else {
            return;
        };
        if asked {
            match self.section {
                Section::Spaces => {
                    library.keyed_card = first;
                    window.focus(&cards, cx);
                }
                _ => window.focus(&search, cx),
            }
        } else if library.keyed_card.is_some() && !cards.is_focused(window) {
            library.keyed_card = None;
        }
        // A click elsewhere ends the rows' ring, as the field's keys move it.
        if library.keyed_row.is_some() && !search.is_focused(window) {
            library.keyed_row = None;
            // check/screen-reader.sh reads this line.
            log!("browser: library rows ring ended");
        }
        if let Some(card) = library.keyed_card.filter(|_| asked) {
            log!("browser: library card keyed {card}");
        }
        if self.library_open {
            return;
        }
        // The field and cards go with the Library. Its key or Escape gives the keyboard back to
        // whoever had it. Otherwise the window takes it, where Tab rings the sidebar, and not the
        // page: a click on a row may be opening another, and a click elsewhere took it already.
        let LibraryKeys { back, closed, .. } = std::mem::take(&mut self.library_keys);
        let close = closed.unwrap_or(Close::new(Why::Chosen, By::Code));
        let fallback = self.focus_handle.clone();
        match back.give_back(close, &[&search, &cards], &fallback, window, cx) {
            // The tab shown may have changed since, by a key for another tab or Space: an agent's
            // page the user doesn't hold never gets the keyboard from a key in the UI.
            Returned::Page if self.shows_agents_page() => {
                log!("browser: library kept the keyboard from an agent's page");
            }
            Returned::Page => {
                self.browser.focus_page(cx);
                // check.sh reads this line.
                log!("browser: library gave the keyboard back to the page");
            }
            Returned::Left | Returned::Opener | Returned::Fallback => {}
        }
    }

    /// Whether the core shows an agent's page, or a task's watched is still drawn: agents' Spaces
    /// are never the window's own, so only those are theirs. The Library's key closes the task's
    /// area as it closes, while the core still shows the task's page, so the window keeps the
    /// keyboard, a page the user holds from the agent too.
    fn shows_agents_page(&self) -> bool {
        self.task_core != 0 || self.agent_page().is_some()
    }

    /// Spaces' cards, taking their keys while they have the keyboard. An assistive app reads them
    /// as Spaces, and, as they have the keyboard, the ringed card.
    pub(super) fn with_card_keys(
        &self,
        cards: Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        let Some(library) = &self.library else {
            return cards;
        };
        cards
            .when(self.a11y, |cards| {
                cards.role(Role::Group).aria_label("Spaces")
            })
            .key_context(CARDS)
            .track_focus(&library.cards)
            .on_action(cx.listener(|shell, _: &CardLeft, _, cx| shell.card_step(false, cx)))
            .on_action(cx.listener(|shell, _: &CardRight, _, cx| shell.card_step(true, cx)))
            .on_action(cx.listener(|shell, _: &CardShow, _, cx| shell.card_show(cx)))
    }

    /// A Space's card as an assistive app reads it, by the Space's name, and with the ring while
    /// the keys are on it.
    pub(super) fn card_keyed(
        &self,
        card: Stateful<gpui::Div>,
        space: u64,
        name: gpui::SharedString,
    ) -> Stateful<gpui::Div> {
        let keyed = self.library.as_ref().and_then(|library| library.keyed_card) == Some(space);
        card.when(self.a11y, |card| {
            card.role(Role::Button)
                .aria_label(name)
                .when(keyed, |card| card.aria_active_descendant())
        })
        .when(keyed, |card| {
            card.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(CARD_RADIUS))
                    .border(px(KEYED_RING))
                    .border_color(self.palette.plate(KEYED)),
            )
        })
    }

    /// The Library, where Tab and Shift+Tab go over its controls in reading order and round
    /// again: the section buttons, Back, the search field or Spaces' cards, Filter and the list's
    /// buttons. The rows are the field's, on ↓ and ↑, as the command bar's suggestions are.
    pub(super) fn with_stops(&self, panel: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        panel
            .key_context(PANEL)
            .on_action(cx.listener(|_, _: &NextStop, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &PreviousStop, window, cx| window.focus_prev(cx)))
    }

    /// One of the Library's controls as a Tab stop, read by `name`, ringed while the keys have
    /// it. Return and Space press it, as a click does.
    pub(super) fn stop(
        &self,
        control: Stateful<gpui::Div>,
        name: SharedString,
    ) -> Stateful<gpui::Div> {
        control
            .when(self.a11y, |control| {
                crate::settings_rows::named_button(control, name)
            })
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(self.stop_ring())
    }

    /// The ring round a control the keys have, the rows' and cards' colour, inside its edge.
    pub(super) fn stop_ring(&self) -> impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement {
        let color = self.palette.plate(KEYED);
        move |style| {
            style.shadow(vec![gpui::BoxShadow {
                color,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(KEYED_RING),
                inset: true,
            }])
        }
    }

    /// Tab or Shift+Tab from the window while the Library is open, as after a click on it: the
    /// keyboard goes where the Library's key puts it, its field or Spaces' first card.
    pub(in crate::shell) fn tab_into_library(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.library_open || self.library.is_none() {
            return false;
        }
        self.library_keys.asked = true;
        cx.notify();
        true
    }

    /// The search field, taking the keys that go over the list's rows.
    pub(super) fn with_row_keys(
        &self,
        field: Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        field
            .key_context(ROWS)
            .on_action(cx.listener(|shell, _: &RowDown, _, cx| shell.row_step(true, cx)))
            .on_action(cx.listener(|shell, _: &RowUp, _, cx| shell.row_step(false, cx)))
            .on_action(cx.listener(|shell, _: &RowOpen, _, cx| shell.row_open(cx)))
            // The field's own menu keys, ⇧F10 and the menu key, as the sidebar's rows have them,
            // are the ringed row's while one is ringed.
            .capture_action(
                cx.listener(|shell, _: &crate::text_input::OpenMenu, _, cx| shell.row_menu(cx)),
            )
    }

    /// The rows the ring goes over, in order: each by its line in the list and, in Media, its
    /// tile's column.
    fn row_stops(&self) -> Vec<(usize, usize)> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        let mut stops = Vec::new();
        for (line, item) in library.items.iter().enumerate() {
            match item {
                Item::Heading(_) | Item::Clear | Item::PastSite(_) => {}
                Item::PastStep(past, step) => {
                    if self.past_step_opens(*past, *step) {
                        stops.push((line, 0));
                    }
                }
                Item::Media(tiles) => stops.extend(
                    (tiles.iter().enumerate())
                        .filter(|(_, tile)| tile.is_some())
                        .map(|(column, _)| (line, column)),
                ),
                _ => stops.push((line, 0)),
            }
        }
        stops
    }

    /// Whether the step at `step` of the past task at `past` opens to show what it did.
    pub(super) fn past_step_opens(&self, past: usize, step: usize) -> bool {
        (self.agent_history.get(past))
            .and_then(|past| past.steps.get(step))
            .is_some_and(|each| !each.url.is_empty())
    }

    /// Down or up from the field: rings the next row or the one before, wrapping at the ends as the
    /// command bar's keys do, from the first or the last while none is ringed.
    fn row_step(&mut self, down: bool, cx: &mut Context<Self>) {
        let stops = self.row_stops();
        let Some(library) = &mut self.library else {
            return;
        };
        if stops.is_empty() {
            return cx.propagate();
        }
        let count = stops.len();
        let at = (library.keyed_row).and_then(|keyed| stops.iter().position(|&stop| stop == keyed));
        let next = match (at, down) {
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        let (line, column) = stops[next];
        library.keyed_row = Some((line, column));
        library.list.scroll_to_reveal_item(line);
        // check.sh waits for this line.
        log!("browser: library row keyed {line} {column}");
        cx.notify();
    }

    /// Rings the list's `rows`th row as ↓ from the field does, for the preview, once the list
    /// has rows: whether it had.
    pub fn preview_ring_library_row(&mut self, rows: usize, cx: &mut Context<Self>) -> bool {
        if self.row_stops().is_empty() {
            return false;
        }
        for _ in 0..rows {
            self.row_step(true, cx);
        }
        true
    }

    /// Whether the ring is on `item`'s row, or on its tile at `column` in Media.
    pub(super) fn row_keyed(&self, item: Item, column: usize) -> bool {
        self.library.as_ref().is_some_and(|library| {
            library
                .keyed_row
                .is_some_and(|(line, at)| at == column && library.items.get(line) == Some(&item))
        })
    }

    /// A row of the list, or a tile of Media's, as an assistive app reads it, an option of the
    /// list named as the live region names it, selected while the ring is on it, which then draws.
    pub(super) fn library_row(
        &self,
        row: Stateful<gpui::Div>,
        item: Item,
        column: usize,
        radius: f32,
    ) -> Stateful<gpui::Div> {
        let keyed = self.row_keyed(item, column);
        let name = self.a11y.then(|| self.row_name(item, column));
        let drawn = (self.library.as_ref()).map(|library| library.row_drawn.clone());
        row.when_some(name, |row, name| {
            row.role(Role::ListBoxOption)
                .aria_label(name)
                .aria_selected(keyed)
        })
        .when(keyed, |row| {
            let log_drawn = canvas(
                |_, _, _| {},
                move |bounds: Bounds<Pixels>, _, _, _| {
                    let Some(drawn) = &drawn else {
                        return;
                    };
                    if drawn.replace(Some(bounds)) != Some(bounds) {
                        // tools/parity.sh waits for this line.
                        log!(
                            "browser: library row ring at {:.0} {:.0} {:.0} {:.0}",
                            f32::from(bounds.origin.x),
                            f32::from(bounds.origin.y),
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height)
                        );
                    }
                },
            )
            .size_full();
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius))
                    .border(px(KEYED_RING))
                    .border_color(self.palette.plate(KEYED))
                    .child(log_drawn),
            )
        })
    }

    /// What a screen reader says for the row at `item`, or its tile at `column`: its title, or
    /// its address or name where it has none.
    fn row_name(&self, item: Item, column: usize) -> SharedString {
        let titled = |title: &SharedString, or: &SharedString| match title.is_empty() {
            true => or.clone(),
            false => title.clone(),
        };
        let library = self.library.as_ref();
        let name = match item {
            Item::Tab(tab) => self
                .archive
                .get(tab)
                .map(|tab| titled(&tab.title, &tab.url)),
            Item::Visit(page) => (library.and_then(|library| library.history.pages.get(page)))
                .map(|visit| titled(&visit.page.title, &visit.page.url)),
            Item::Task(task) => {
                (self.archived_tasks.get(task)).map(|task| titled(&task.title, &task.asked))
            }
            Item::Past(past) => {
                (self.agent_history.get(past)).map(|past| past.title.clone().into())
            }
            Item::PastHead(_) => Some("Back".into()),
            Item::PastStep(past, step) => self.past_step_title(past, step).map(Into::into),
            Item::Boost(boost) => self.boost_list.get(boost).map(|boost| boost.site.clone()),
            Item::Board(board) => {
                (self.board_list.get(board)).map(|board| board.name.clone().into())
            }
            Item::Download(download) => {
                (self.downloads.get(download)).map(|each| each.name.clone())
            }
            Item::Media(tiles) => match tiles.get(column).copied().flatten() {
                Some(Medium::Capture(index)) => {
                    self.captures.get(index).map(|each| each.name.clone())
                }
                Some(Medium::Download(index)) => {
                    self.downloads.get(index).map(|each| each.name.clone())
                }
                None => None,
            },
            Item::Heading(_) | Item::Clear | Item::PastSite(_) => None,
        };
        name.unwrap_or_default()
    }

    /// What the live region says as the ring moves: the ringed row's name and where it stands
    /// among the rows, as the command bar's says its selected suggestion.
    pub(super) fn row_said(&self) -> Option<String> {
        let keyed = self.library.as_ref()?.keyed_row?;
        let stops = self.row_stops();
        let at = stops.iter().position(|&stop| stop == keyed)?;
        let library = self.library.as_ref()?;
        let item = *library.items.get(keyed.0)?;
        Some(format!(
            "{}, {} of {}",
            self.row_name(item, keyed.1),
            at + 1,
            stops.len()
        ))
    }

    /// A live region that says the ringed row as the ring moves, a polite one, as the command
    /// bar's says its selected suggestion: the field keeps the keyboard, so a screen reader would
    /// say nothing of the row otherwise.
    pub(super) fn rows_announcer(&self) -> impl IntoElement + use<> {
        let said = self.row_said();
        div()
            .id("library-row-said")
            .absolute()
            .size_0()
            .role(Role::Status)
            .when_some(said, |announcer, said| {
                announcer.aria_label(said.clone()).aria_value(said)
            })
            .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
    }

    /// Return from the field: does what a click on the ringed row does, and ends the ring.
    fn row_open(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        let Some((line, column)) = library.keyed_row.take() else {
            return cx.propagate();
        };
        let Some(item) = library.items.get(line).copied() else {
            return;
        };
        let bounds = library.row_drawn.take();
        // Where a click on it would be: Peek grows from there, and a menu opens under it.
        let at = bounds.map(|bounds| bounds.center()).unwrap_or_default();
        let under = bounds
            .map(|bounds| point(bounds.left(), bounds.bottom()))
            .unwrap_or_default();
        // check.sh waits for this line.
        log!("browser: library row opened {line} {column}");
        cx.notify();
        match item {
            Item::Tab(tab) => {
                if let Some(id) = self.archive.get(tab).map(|tab| tab.id) {
                    self.open_archived(id, at, Modifiers::none(), cx);
                }
            }
            Item::Visit(page) => {
                let url = (self.library.as_ref())
                    .and_then(|library| library.history.pages.get(page))
                    .map(|visit| visit.page.url.clone());
                if let Some(url) = url {
                    self.open_visited(url, at, Modifiers::none(), cx);
                }
            }
            Item::Task(task) => {
                if let Some(agent) = self.archived_tasks.get(task).map(|task| task.agent.clone()) {
                    self.restore_archived_task(agent, cx);
                }
            }
            Item::Past(past) => {
                if let Some(id) = self.agent_history.get(past).map(|past| past.id.clone()) {
                    self.open_past_task(&id, cx);
                }
            }
            Item::PastHead(_) => self.close_past_task(cx),
            Item::PastStep(_, step) => self.open_past_step(step, false, cx),
            Item::Boost(boost) => {
                if let Some(site) = self.boost_list.get(boost).map(|boost| boost.site.clone()) {
                    self.boost_menu(site, under, cx);
                }
            }
            Item::Board(board) => {
                if let Some(id) = self.board_list.get(board).map(|board| board.id.clone()) {
                    self.open_listed_board(&id, cx);
                }
            }
            Item::Download(download) => {
                let Some(each) = self.downloads.get(download) else {
                    return;
                };
                let (id, held, state) = (each.id, each.held, each.state);
                // As a click on its row: a held file's choices are in its menu.
                if held {
                    self.download_menu(id, under, cx);
                } else if state == DownloadState::Done {
                    self.act_on_download(id, DownloadAction::Open, "row", cx);
                    self.hide_recent_downloads(cx);
                }
            }
            Item::Media(tiles) => {
                let path = match tiles.get(column).copied().flatten() {
                    Some(Medium::Capture(index)) => {
                        self.captures.get(index).map(|each| each.path.clone())
                    }
                    Some(Medium::Download(index)) => {
                        self.downloads.get(index).map(|each| each.path.clone())
                    }
                    None => None,
                };
                if let Some(path) = path {
                    self.open_media(&path, cx);
                }
            }
            Item::Heading(_) | Item::Clear | Item::PastSite(_) => {}
        }
    }

    /// ⇧F10 or the menu key from the field with a row ringed: the row's menu under it, as a
    /// right-click on it shows, with the keyboard on its first item, as the sidebar's keys do. The
    /// ring stays, so Escape from the menu leaves it on the row. A row with no menu shows none, as
    /// a right-click on it does; with no row ringed, the field shows its own.
    fn row_menu(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &self.library else {
            return;
        };
        let Some((line, column)) = library.keyed_row else {
            return;
        };
        cx.stop_propagation();
        let Some(item) = library.items.get(line).copied() else {
            return;
        };
        let under = (library.row_drawn.get())
            .map(|bounds| point(bounds.left(), bounds.bottom()))
            .unwrap_or_default();
        let visit = match item {
            Item::Visit(page) => {
                (library.history.pages.get(page)).map(|visit| (visit.page.url.clone(), visit.day))
            }
            _ => None,
        };
        let shown = match item {
            Item::Tab(tab) => (self.archive.get(tab).map(|tab| tab.id))
                .map(|id| self.archived_menu(id, under, cx)),
            Item::Visit(_) => visit.map(|(url, day)| self.visited_menu(url, day, under, cx)),
            Item::Task(task) => (self.archived_tasks.get(task).map(|task| task.agent.clone()))
                .map(|agent| self.archived_task_menu(agent, under, cx)),
            Item::Boost(boost) => (self.boost_list.get(boost).map(|boost| boost.site.clone()))
                .map(|site| self.boost_menu(site, under, cx)),
            Item::Board(board) => (self.board_list.get(board).map(|board| board.id.clone()))
                .map(|id| self.board_menu(id, under, cx)),
            Item::Download(download) => (self.downloads.get(download).map(|each| each.id))
                .map(|id| self.download_menu(id, under, cx)),
            Item::Media(tiles) => match tiles.get(column).copied().flatten() {
                Some(Medium::Capture(index)) => (self.captures.get(index))
                    .map(|each| each.path.clone())
                    .map(|path| self.capture_menu(path, under, cx)),
                Some(Medium::Download(index)) => (self.downloads.get(index).map(|each| each.id))
                    .map(|id| self.download_menu(id, under, cx)),
                None => None,
            },
            Item::Past(_) | Item::PastHead(_) | Item::PastStep(..) => None,
            Item::Heading(_) | Item::Clear | Item::PastSite(_) => None,
        };
        if shown.is_some() {
            // check.sh waits for this line.
            log!("browser: library row menu {line} {column}");
            // After the host's own deferred show, where overlays open for a shortcut.
            cx.defer(crate::menu::highlight_first);
        }
    }

    /// Moves the ring to the next card along or back, stopping at the ends.
    fn card_step(&mut self, along: bool, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.spaces.iter().map(|space| space.id).collect();
        let Some(library) = &mut self.library else {
            return;
        };
        let Some(at) = (library.keyed_card).and_then(|card| ids.iter().position(|&id| id == card))
        else {
            return cx.propagate();
        };
        let next = match along {
            true => ids.get(at + 1),
            false => at.checked_sub(1).and_then(|at| ids.get(at)),
        };
        if let Some(&next) = next {
            library.keyed_card = Some(next);
            log!("browser: library card keyed {next}");
            cx.notify();
        }
    }

    /// Return: shows the ringed card's Space, as a click on the card does.
    fn card_show(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self
            .library
            .as_mut()
            .and_then(|library| library.keyed_card.take())
        else {
            return cx.propagate();
        };
        self.pick_space(card, cx);
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, Role, TestAppContext, VisualTestContext};

    use crate::shell::{Command, Folders, Section, Shell, commands::bind_keys, recorder::Recorder};

    /// A window on Space 1 of three, a tab showing, the Library to open on `section`.
    fn window(
        section: Section,
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(|cx| {
            bind_keys(cx);
            crate::text_input::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        cx.simulate_a11y_activation();
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1), space(2), space(3)], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
            shell.section = section;
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.run_until_parked();
        host.take();
        (shell, host, cx)
    }

    fn run(shell: &Entity<Shell>, command: Command, cx: &mut VisualTestContext) {
        run_from(shell, command, "test", cx);
    }

    /// Runs `command` for a key from `from`, "page" for one the page handed back.
    fn run_from(shell: &Entity<Shell>, command: Command, from: &str, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.run(command, from, cx);
            settle(shell);
        });
        cx.run_until_parked();
    }

    /// The Library's motion and the sidebar's fade at their ends.
    fn settle(shell: &mut Shell) {
        shell.library_motion = None;
        shell.sidebar_fade = None;
    }

    fn root_keyed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| shell.read(cx).focus_handle.is_focused(window))
    }

    /// What has the keyboard, as a screen reader reads it: its role and name, or, inside it, the
    /// active one's.
    fn keyed(cx: &mut VisualTestContext) -> (Role, String) {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, node) = (tree.nodes.iter())
            .find(|(id, _)| *id == tree.focus)
            .expect("something has the keyboard");
        (node.role(), node.label().unwrap_or_default().to_string())
    }

    /// The ring goes over each row that opens, in the list's order, and over Media's tiles in
    /// reading order, past headings, Agents' Clear and a past task's sites, wrapping at the ends.
    #[gpui::test]
    fn the_rows_ring_goes_in_reading_order(cx: &mut TestAppContext) {
        use crate::shell::library::{Item, Medium};
        let (shell, _, cx) = window(Section::Media, cx);
        run(&shell, Command::ViewLibrary, cx);
        let went = shell.update(cx, |shell, cx| {
            let library = shell.library.as_mut().unwrap();
            let tile = |index| Some(Medium::Capture(index));
            library.items = vec![
                Item::Heading("Today"),
                Item::Clear,
                Item::Media([tile(0), tile(1), tile(2)]),
                Item::PastSite(0),
                Item::Media([tile(3), None, None]),
            ];
            library.keyed_row = None;
            let mut went = Vec::new();
            for down in [true, true, true, true, true, false] {
                shell.row_step(down, cx);
                went.push(shell.library.as_ref().unwrap().keyed_row.unwrap());
            }
            went
        });
        assert_eq!(went, [(2, 0), (2, 1), (2, 2), (4, 0), (2, 0), (4, 0)]);
    }

    /// The Library's key gives History's search field the keyboard, taking it from the page, and
    /// the window takes it back as the Library closes. A section's own command gives its field it.
    #[gpui::test]
    fn the_librarys_key_gives_its_search_field_the_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(host.take().contains(&"focus ui".to_owned()));
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        run(&shell, Command::ViewDownloads, cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search Downloads".into()));
        run(&shell, Command::ViewLibrary, cx);
        assert!(
            root_keyed(&shell, cx),
            "the window didn't take the keyboard back"
        );
    }

    /// Opened by its key from the page, the Library gives the page the keyboard back as its key or
    /// a section's closes it, or Escape does, and the window's UI lets it go (#3313).
    #[gpui::test]
    fn the_page_gets_the_keyboard_back_as_a_key_closes_the_library(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        let focused_page = |host: &Recorder| host.take().contains(&"focus page".to_owned());
        run_from(&shell, Command::ViewDownloads, "page", cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search Downloads".into()));
        // Another section's key keeps the Library open, and the page's turn for the keyboard.
        run(&shell, Command::ViewArchive, cx);
        assert!(
            !focused_page(&host),
            "the page took it while the Library was open"
        );
        run(&shell, Command::ViewArchive, cx);
        assert!(
            focused_page(&host),
            "a section's key didn't give the page it back"
        );
        assert!(root_keyed(&shell, cx), "the field kept the UI's keyboard");

        run_from(&shell, Command::ViewLibrary, "page", cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(
            focused_page(&host),
            "the Library's key didn't give the page it back"
        );

        run_from(&shell, Command::ViewLibrary, "page", cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape left it open"
        );
        assert!(focused_page(&host), "escape didn't give the page it back");
    }

    /// Escape while an input method composes in the search field is the composition's: the Library
    /// stays open, its field keeping the keyboard and the marked text, and once the method has
    /// dropped it, the next Escape closes the Library.
    #[gpui::test]
    fn escape_while_composing_keeps_the_library(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler as _;
        let (shell, _, cx) = window(Section::Archive, cx);
        run(&shell, Command::ViewLibrary, cx);
        let search = shell.read_with(cx, |shell, _| {
            shell.library.as_ref().unwrap().search.clone()
        });
        cx.update(|window, cx| {
            search.update(cx, |search, cx| {
                search.replace_and_mark_text_in_range(None, "ぴ", None, window, cx)
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.library_open),
            "escape closed it"
        );
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        search.read_with(cx, |search, _| {
            assert!(search.composing(), "escape ended the composition");
            assert_eq!(search.text(), "ぴ");
        });
        cx.update(|window, cx| {
            search.update(cx, |search, cx| {
                search.replace_text_in_range(None, "", window, cx)
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape left it open"
        );
        assert!(root_keyed(&shell, cx));
    }

    /// Tab goes over the Library's controls in reading order and round again: from the field to
    /// the section buttons, each a tab of the Sections list, and on to Back and the field, and
    /// Shift+Tab goes the other way. ↓ moves along the sections, and Return shows the one the keys
    /// are on, which keeps them (#3238).
    #[gpui::test]
    fn tab_goes_over_the_librarys_controls(cx: &mut TestAppContext) {
        let (shell, _, cx) = window(Section::Archive, cx);
        run(&shell, Command::ViewLibrary, cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(keyed(cx), (Role::Button, "Back".into()));
        cx.simulate_keystrokes("tab");
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        cx.simulate_keystrokes("tab");
        assert_eq!(keyed(cx), (Role::Tab, "Media".into()));
        cx.simulate_keystrokes("up");
        assert_eq!(
            keyed(cx),
            (Role::Tab, "Media".into()),
            "↑ left the first section"
        );
        cx.simulate_keystrokes("down");
        assert_eq!(keyed(cx), (Role::Tab, "Downloads".into()));
        // Pressed and let go, as a keyboard sends it: Return clicks on the release.
        cx.simulate_keystrokes("enter");
        let keystroke = gpui::Keystroke::parse("enter").unwrap();
        cx.update(|window, cx| {
            use gpui::InputEvent as _;
            window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
        });
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.section),
            Section::Downloads
        );
        assert_eq!(keyed(cx), (Role::Tab, "Downloads".into()));
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(keyed(cx), (Role::Tab, "Media".into()));
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(keyed(cx), (Role::TextInput, "Search Downloads".into()));
        // Escape closes the Library from any of its stops, not only the field.
        cx.simulate_keystrokes("tab");
        assert_eq!(keyed(cx), (Role::Tab, "Media".into()));
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape from a section left the Library open"
        );
    }

    /// Opened by a click, the Library leaves the keyboard with the window, and Tab or Shift+Tab
    /// from there goes to its field, where its key puts it, not to the sidebar under it.
    #[gpui::test]
    fn tab_from_the_window_goes_into_the_open_library(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        shell.update(cx, |shell, cx| {
            shell.set_library(true, cx);
            settle(shell);
        });
        cx.run_until_parked();
        assert!(root_keyed(&shell, cx));
        cx.simulate_keystrokes("tab");
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        assert!(
            !host.take().iter().any(|call| call.contains("sidebar")),
            "Tab rang the sidebar under the Library"
        );
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
    }

    /// Closed as a click closes it, the Library opened from the page gives the window the
    /// keyboard: the click went where the user wants it. Opened from the UI, its key gives the UI
    /// the keyboard back, never the page.
    #[gpui::test]
    fn a_click_or_the_uis_key_never_gives_the_page_the_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        run_from(&shell, Command::ViewLibrary, "page", cx);
        shell.update(cx, |shell, cx| {
            shell.set_library(false, cx);
            settle(shell);
        });
        cx.run_until_parked();
        assert!(!host.take().contains(&"focus page".to_owned()));
        assert!(root_keyed(&shell, cx));

        run_from(&shell, Command::ViewLibrary, "ui", cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(!host.take().contains(&"focus page".to_owned()));
        assert!(root_keyed(&shell, cx));
    }

    /// The archive's tabs `ids`, for a Library opened on History.
    fn archive(shell: &Entity<Shell>, ids: &[u64], cx: &mut VisualTestContext) {
        let tab = |id: u64| crate::shell::Archived {
            id,
            url: format!("https://{id}.example/").into(),
            title: "".into(),
            archived: 0.,
            little: false,
        };
        let tabs = ids.iter().map(|id| tab(*id)).collect();
        shell.update(cx, |shell, cx| shell.set_archive(tabs, cx));
        cx.run_until_parked();
    }

    /// The menu keys from the field, ⇧F10 and the menu key, open the ringed row's menu under it,
    /// as a right-click on the row does. Its close leaves the ring on the row and the keyboard in
    /// the field, where ↓ goes on. With no row ringed, they open the field's own menu, as before.
    #[gpui::test]
    fn the_menu_keys_open_the_ringed_rows_menu(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        let field_menus = Rc::new(std::cell::Cell::new(0));
        let shown = field_menus.clone();
        cx.update(|_, cx| {
            cx.set_global(crate::text_input::FieldMenu(Rc::new(
                move |_, _, _, _, _, _| shown.set(shown.get() + 1),
            )))
        });
        archive(&shell, &[7, 8], cx);
        cx.simulate_keystrokes("cmd-shift-l");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        cx.simulate_keystrokes("shift-f10");
        assert_eq!(field_menus.get(), 1, "no row ringed, and no field menu");
        assert!(host.menus.borrow().is_empty(), "{:?}", host.menus.borrow());
        let ring = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                let library = shell.library.as_ref().expect("the Library");
                (library.keyed_row, library.row_drawn.get())
            })
        };
        for (key, row) in [("shift-f10", (1, 0)), ("menu", (2, 0))] {
            cx.simulate_keystrokes("down");
            cx.run_until_parked();
            let (ringed, drawn) = ring(cx);
            assert_eq!(ringed, Some(row), "↓ didn't ring the next row before {key}");
            let drawn = drawn.expect("the ring drawn");
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            let menu =
                (host.menus.borrow_mut().pop()).unwrap_or_else(|| panic!("no menu for {key}"));
            let titles: Vec<&str> = menu.iter().map(|(title, _)| title.as_ref()).collect();
            assert_eq!(
                titles,
                ["Reopen Tab", "Copy URL", "", "Delete from Archive"]
            );
            let under = gpui::point(drawn.left(), drawn.bottom());
            assert_eq!(
                host.menu_at.get(),
                Some(under),
                "{key}'s menu isn't under the row"
            );
            cx.update(|_, cx| host.dismiss(cx));
            cx.run_until_parked();
            assert_eq!(
                ring(cx).0,
                Some(row),
                "the menu's close took the ring off the row"
            );
            assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        }
        assert_eq!(
            field_menus.get(),
            1,
            "a ringed row's key opened the field's menu too"
        );
    }

    /// Opened by its key, the Library's empty search field has the keyboard, and ⌘Z after Clear
    /// Archive brings the tabs back, as its toast offers, and ⇧⌘Z clears them again (#3306).
    #[gpui::test]
    fn z_in_the_empty_field_undoes_clear_archive(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        archive(&shell, &[7, 8], cx);
        cx.simulate_keystrokes("cmd-shift-l");
        shell.update(cx, |shell, _| {
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        shell.update(cx, |shell, cx| shell.clear_archive(cx));
        let (_, _, confirmed) = host
            .confirming
            .borrow_mut()
            .take()
            .expect("Clear Archive asked");
        cx.update(|_, cx| confirmed(cx));
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["archive clear as 9001"]);
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        cx.simulate_keystrokes("cmd-z");
        assert_eq!(host.take_edits(), ["undo archive remove 9001"]);
        cx.simulate_keystrokes("cmd-shift-z");
        assert_eq!(host.take_edits(), ["archive clear as 9002"]);
    }

    /// With text in the Library's search field and no toast offering it, ⌘Z stops at the field,
    /// so typing never undoes what's out of sight, here a tab deleted from the archive before the
    /// search, once the toast that offered its undo has gone.
    #[gpui::test]
    fn z_with_text_in_the_field_undoes_nothing(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        archive(&shell, &[7, 8], cx);
        run(&shell, Command::ViewLibrary, cx);
        shell.update(cx, |shell, cx| shell.delete_archived(7, cx));
        assert_eq!(host.take_edits(), ["archive remove 7 as 9001"]);
        cx.simulate_input("ex");
        cx.run_until_parked();
        // The toast's time is up.
        host.undo_toast.borrow_mut().take();
        cx.simulate_keystrokes("cmd-z");
        assert!(
            host.take_edits().is_empty(),
            "⌘Z with text in the field undid the delete"
        );
        let text = shell.read_with(cx, |shell, cx| {
            shell
                .library
                .as_ref()
                .map(|library| library.search.read(cx).text().to_string())
        });
        assert_eq!(text.as_deref(), Some("ex"));
    }

    /// A key for another tab can show another of the user's pages while the Library is open, and
    /// the Library's key then gives that page the keyboard. An agent's page watched is task_area's
    /// test.
    #[gpui::test]
    fn the_page_shown_since_gets_the_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 11],
                Vec::new(),
                Folders::default(),
                cx,
            );
        });
        run_from(&shell, Command::ViewLibrary, "page", cx);
        shell.update(cx, |shell, cx| shell.set_selected(11, cx));
        cx.run_until_parked();
        host.take();
        run(&shell, Command::ViewLibrary, cx);
        assert!(host.take().contains(&"focus page".to_owned()));
    }

    /// Searched for and deleted from the archive, a tab comes back with ⌘Z while the toast that
    /// offers it shows, with the search's text still in the field: the toast is in sight and
    /// promises the key (#3307). ⇧⌘Z, which no toast offers, still stops at the field.
    #[gpui::test]
    fn z_with_text_in_the_field_undoes_what_the_toast_offers(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        archive(&shell, &[7, 8], cx);
        run(&shell, Command::ViewLibrary, cx);
        cx.simulate_input("ex");
        cx.run_until_parked();
        shell.update(cx, |shell, cx| shell.delete_archived(7, cx));
        assert_eq!(host.take_edits(), ["archive remove 7 as 9001"]);
        assert!(
            host.undo_toast.borrow().is_some(),
            "no toast offered the undo"
        );
        cx.simulate_keystrokes("cmd-z");
        assert_eq!(host.take_edits(), ["undo archive remove 9001"]);
        cx.simulate_keystrokes("cmd-shift-z");
        assert!(
            host.take_edits().is_empty(),
            "⇧⌘Z with text in the field did the delete again"
        );
        let text = shell.read_with(cx, |shell, cx| {
            shell
                .library
                .as_ref()
                .map(|library| library.search.read(cx).text().to_string())
        });
        assert_eq!(text.as_deref(), Some("ex"));
    }

    /// Opened by a swipe, or set open as the gesture does, the Library leaves the keyboard where
    /// it was.
    #[gpui::test]
    fn a_swipe_moves_no_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        shell.update(cx, |shell, cx| shell.set_library(true, cx));
        cx.run_until_parked();
        assert!(!host.take().contains(&"focus ui".to_owned()));
    }

    /// In Spaces, which has no field, the ring goes on the first card, read by its Space's name;
    /// left and right move it along the cards, stopping at the ends, and return shows its Space.
    /// Escape closes the Library.
    #[gpui::test]
    fn spaces_rings_its_first_card(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Spaces, cx);
        run(&shell, Command::ViewLibrary, cx);
        let card = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.library.as_ref()?.keyed_card)
        };
        assert_eq!(card(&shell, cx), Some(1));
        assert_eq!(keyed(cx), (Role::Button, "Space 1".into()));
        cx.simulate_keystrokes("left");
        assert_eq!(
            card(&shell, cx),
            Some(1),
            "the ring went past the first card"
        );
        cx.simulate_keystrokes("right right right");
        assert_eq!(
            card(&shell, cx),
            Some(3),
            "the ring went past the last card"
        );
        cx.run_until_parked();
        assert_eq!(keyed(cx), (Role::Button, "Space 3".into()));
        cx.simulate_keystrokes("left");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            host.shown.borrow().last(),
            Some(&2),
            "return showed no Space"
        );
        run(&shell, Command::ViewLibrary, cx);
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| {
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape left it open"
        );
    }

    /// Pressed and let go, as a keyboard sends it: a button clicks on the release.
    fn press(key: &'static str, cx: &mut VisualTestContext) {
        cx.simulate_keystrokes(key);
        let keystroke = gpui::Keystroke::parse(key).unwrap();
        cx.update(|window, cx| {
            use gpui::InputEvent as _;
            window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
        });
        cx.run_until_parked();
    }

    /// Whether a screen reader finds a button named `name`.
    fn has_button(name: &str, cx: &mut VisualTestContext) -> bool {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        (tree.nodes.iter())
            .any(|(_, node)| node.role() == Role::Button && node.label().as_deref() == Some(name))
    }

    /// With no Boards, the list offers New Board (#3575): Tab from the search field reaches it,
    /// read as "New Board" without its key, and Space, Return or a click asks the core for a Board,
    /// as New Board's key does, closing the Library so the new row's name field takes the keyboard.
    #[gpui::test]
    fn the_empty_boards_list_s_new_board_makes_one_from_the_keys(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Boards, cx);
        // A click leaves the keyboard in the search field, where the Library put it.
        for (made, key) in [(1, "space"), (2, "enter"), (3, "click")] {
            run(&shell, Command::ViewLibrary, cx);
            assert_eq!(keyed(cx), (Role::TextInput, "Search Boards".into()));
            if key == "click" {
                let drawn = shell.read_with(cx, |shell, _| {
                    shell
                        .library
                        .as_ref()
                        .and_then(|library| library.new_board_drawn.get())
                });
                let at = drawn.expect("New Board wasn't drawn").center();
                cx.simulate_click(at, gpui::Modifiers::none());
                cx.run_until_parked();
            } else {
                cx.simulate_keystrokes("tab");
                assert_eq!(keyed(cx), (Role::Button, "New Board".into()));
                press(key, cx);
            }
            shell.update(cx, |shell, cx| {
                settle(shell);
                cx.notify();
            });
            cx.run_until_parked();
            assert_eq!(host.boards.borrow().len(), made, "{key} made no Board");
            assert!(
                !shell.read_with(cx, |shell, _| shell.library_open),
                "the Library stayed over the new Board's name, after {key}"
            );
            let named = cx.update(|window, cx| {
                let shell = shell.read(cx);
                let renaming = shell
                    .renaming
                    .as_ref()
                    .expect("no field for the Board's name");
                assert!(matches!(renaming.naming, crate::shell::Naming::Tab(_)));
                gpui::Focusable::focus_handle(renaming.name.read(cx), cx).is_focused(window)
            });
            assert!(
                named,
                "the Board's name field hasn't the keyboard, after {key}"
            );
            shell.update(cx, |shell, _| shell.renaming = None);
        }
    }

    /// A search that matches no Board keeps "No Boards match." and offers no New Board: the list
    /// isn't empty, the search is.
    #[gpui::test]
    fn a_search_matching_no_board_offers_no_new_board(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Boards, cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(
            has_button("New Board", cx),
            "the empty list offers no New Board"
        );
        cx.simulate_input("porch");
        cx.run_until_parked();
        assert!(
            !has_button("New Board", cx),
            "a search matching nothing offered New Board"
        );
        let line = shell.read_with(cx, |shell, _| {
            let query = &shell.library.as_ref().unwrap().query;
            shell.empty_line(Section::Boards, query.is_empty())
        });
        assert_eq!(line, "No Boards match.");
        cx.simulate_keystrokes("tab");
        assert_ne!(keyed(cx), (Role::Button, "New Board".into()));
        assert!(host.boards.borrow().is_empty());
    }
}
