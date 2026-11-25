//! The page an empty Space rests on (#4140): ilo's mark over a field that searches or, after ⇥,
//! asks an assistant, the line Ask adds under it, and the user's recent tasks. The field is the
//! command bar at rest (`command_bar::in_place`), whose input and rows this page places and whose
//! keys it takes. Nothing of the tasks is read for the page's first frame, and the bar asks the
//! core nothing until it's typed in.

use std::{cell::Cell, path::PathBuf, rc::Rc};

use gpui::{
    Bounds, BoxShadow, Context, Corners, Entity, FontWeight, Hsla, Pixels, SharedString,
    Subscription, Window, canvas, div, point, prelude::*, px, rgb, rgba, svg,
};

use crate::agents::files::{self, Picked, words as files_words};
use crate::agents::task::{Card, Dot, Word};
use crate::agents::task::{Start, TaskAction, added_address, model_menu};
use crate::agents::tier::{ANY_SITE, ONLY_ADDED, Tier};
use crate::agents::wall::Granted;
use crate::command_bar::{self, CommandBar, Given, InPlaceKey};
use crate::palette::{Palette, Tone};
use crate::shell::ShowStarted;
use crate::tokens::shell::composer::*;
use crate::tokens::text;

use super::super::{
    STATUS_AGENT, Shell, URL_FIELD,
    commands::{key_name, space_tabs},
    keys::OFF_THE_MAC,
    menus::{MenuAction, MenuEntry},
    task_area::covering,
};

/// What the field says in Search.
pub(in crate::shell) const SEARCH_WORDS: &str = crate::command_bar::PLACEHOLDER_TEXT;
/// The switch's two sides.
const SEARCH: &str = "Search";
const ASK: &str = "Ask";
/// Over the cards.
const LATEST: &str = "Latest tasks";

/// What the field says in Ask, naming the assistant it runs on.
fn ask_words(assistant: Option<&str>) -> SharedString {
    format!(
        "Ask {} to do something",
        assistant.unwrap_or("an assistant")
    )
    .into()
}

/// The bar the page rests on, while the Space shows no page.
pub(in crate::shell) struct Resting {
    pub(in crate::shell) bar: Entity<CommandBar>,
    _keys: Subscription,
    /// The page has drawn its first frame, from when it draws the tasks.
    painted: bool,
    /// Whether the field is owed the keyboard, which it takes once, at the first frame that may
    /// give it (`give_resting_keys`), so whatever the user moves the keyboard to after keeps it.
    wants_keys: Owed,
    /// The level the next task starts at: the user's pick on Ask's line, in this view's memory
    /// alone. It's Work on pages again after a run, out of Ask, and in another Space.
    tier: Tier,
    /// The Space the pick was made in.
    space: u64,
    /// Where the level's chip was last drawn, which its menu hangs from.
    pub(in crate::shell) tier_chip: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The model the next task starts on, picked on the assistant's chip for the assistant it
    /// names, by id, empty for its own default. It's in this view's memory alone, never Settings',
    /// and goes as the level does.
    pub(in crate::shell) model: Option<(SharedString, String)>,
    /// Where the assistant's chip was last drawn, which its menu hangs from.
    pub(in crate::shell) model_chip: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The pages the user added on the `+` menu, in the order added, whose addresses alone the
    /// next task gets; and whether it may go only to their sites. Both go as the level does.
    pub(in crate::shell) pages: Vec<Added>,
    pub(in crate::shell) only_added: bool,
    /// Where the `+` chip was last drawn, which its menu hangs from.
    pub(in crate::shell) plus_chip: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The files the user attached and the folder they granted on the `+` menu, for the next
    /// task alone, and the line saying the panel gave a folder ilo never gives. They go as the
    /// level does.
    pub(in crate::shell) files: Vec<PathBuf>,
    pub(in crate::shell) folder: Option<Granted>,
    pub(in crate::shell) refused: Option<SharedString>,
}

/// A page the user added: its tab, which its row in the menu is checked by, its address as it was
/// when added, which is what the task gets, and its title, which only the user sees.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::shell) struct Added {
    pub(in crate::shell) tab: u64,
    pub(in crate::shell) url: String,
    title: SharedString,
}

/// Where a chip's menu hangs: 4 under the chip, its names under the chip's own word, or at the
/// click before the chip is drawn.
fn hung_under(chip: Option<Bounds<Pixels>>, clicked: gpui::Point<Pixels>) -> gpui::Point<Pixels> {
    let Some(chip) = chip else {
        return clicked;
    };
    let names = crate::menu::PADDING + crate::menu::ROW_X + crate::menu::CHECK + crate::menu::GAP;
    chip.origin + point(px(CHIP_INSET - names), chip.size.height + px(MENU_DROP))
}

pub(super) use crate::command_bar::{model_words, next_model};

impl Resting {
    /// The picks on Ask's line, which hold for the next task alone: its level, model, pages,
    /// files and folder.
    fn forget_picks(&mut self) {
        self.tier = Tier::default();
        self.model = None;
        self.pages.clear();
        self.only_added = false;
        self.files.clear();
        self.folder = None;
        self.refused = None;
    }

    /// The files' chip and the folder's, in the order the field draws them after the pages.
    fn given(&self) -> Vec<Given> {
        let files = (!self.files.is_empty())
            .then(|| Given::Files(files_words::files(self.files.len()).into()));
        let folder = (self.folder.as_ref())
            .map(|folder| Given::Folder(files::shown_name(folder.root()).into()));
        files.into_iter().chain(folder).collect()
    }
}

/// The level's menu's last row, which opens Settings › Agents at Access.
const SETTINGS: &str = "Settings…";

/// Records where the element it's put in is drawn, for a menu to hang from it.
fn drawn_at(at: &Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    let at = at.clone();
    canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
        .absolute()
        .size_full()
}

/// Why the field is owed the keyboard, if it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owed {
    No,
    /// The Space came to show no page: the field takes the keyboard only from the window itself,
    /// never from a field or form the user is in.
    Shown,
    /// ⌘T or ⌘L asked for it.
    Asked,
}

impl Shell {
    /// Makes the bar the page rests on as the Space comes to show no page, and drops it as a page
    /// shows, or the welcome takes the card.
    pub(in crate::shell) fn step_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != 0 || self.welcome {
            if self.resting.take().is_some() {
                log!("browser: resting field dropped");
            }
            return;
        }
        let space = self.space();
        if let Some(resting) = &mut self.resting {
            // Another Space empty too: its field is owed the keyboard, once the swipe settles.
            if resting.space != space {
                (resting.tier, resting.space) = (Tier::default(), space);
                resting.forget_picks();
                resting.wants_keys = Owed::Shown;
            }
            return self.give_resting_keys(window, cx);
        }
        let bar = self.browser.command_bar_in_place(window, cx);
        let keys = cx.subscribe_in(&bar, window, |shell: &mut Shell, _, key, window, cx| {
            shell.resting_key(*key, window, cx)
        });
        bar.update(cx, |bar, cx| {
            bar.set_on_ask(false, SEARCH_WORDS.into(), window, cx)
        });
        self.resting = Some(Resting {
            bar,
            _keys: keys,
            painted: false,
            wants_keys: Owed::Shown,
            tier: Tier::default(),
            space,
            tier_chip: Rc::default(),
            model: None,
            model_chip: Rc::default(),
            pages: Vec::new(),
            only_added: false,
            plus_chip: Rc::default(),
            files: Vec::new(),
            folder: None,
            refused: None,
        });
        cx.on_next_frame(window, |shell, _, cx| {
            if let Some(resting) = &mut shell.resting {
                resting.painted = true;
                cx.notify();
            }
        });
        log!("browser: resting field made");
        self.give_resting_keys(window, cx);
    }

    /// ⌘T and ⌘L while the Space shows no page: the field takes the keyboard, and no bar opens
    /// over it (#4140). Whether it did.
    pub(in crate::shell) fn keys_to_resting(&mut self, cx: &mut Context<Self>) -> bool {
        // The field stays until the next frame drops it, so a page shown since isn't one.
        if self.selected != 0 || self.welcome {
            return false;
        }
        let Some(resting) = &mut self.resting else {
            return false;
        };
        // The core shows a page the shell hasn't heard of yet, as at launch with a page to open:
        // the keys are that page's, and open the bar over it (#4268).
        if self.browser.shown_now() != 0 {
            return false;
        }
        resting.wants_keys = Owed::Asked;
        cx.notify();
        true
    }

    /// Gives the field the keyboard it's owed once the window is key, no overlay or Library is
    /// over it and no swipe is moving the pages: an overlay keeps the keys it has, and the field
    /// takes them only when the swipe to its Space has settled. Owed for being shown, it leaves
    /// the keyboard where the user put it, as in a Space's name or its form.
    fn give_resting_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still = self.swipe.is_none() && self.settle.is_none();
        let free = window.is_window_active()
            && !self.browser.command_bar_open(cx)
            && self.library.is_none();
        let root = self.focus_handle.clone();
        let Some(resting) =
            (self.resting.as_mut()).filter(|resting| resting.wants_keys != Owed::No)
        else {
            return;
        };
        if !(still && free) {
            return;
        }
        let focus = resting.bar.read(cx).field_focus(cx);
        let elsewhere = window
            .focused(cx)
            .is_some_and(|at| at != root && at != focus);
        let owed = std::mem::replace(&mut resting.wants_keys, Owed::No);
        if owed == Owed::Shown && elsewhere {
            return;
        }
        window.focus(&focus, cx);
        log!("browser: resting field has the keyboard");
    }

    /// Gives the resting field the keyboard, as a click on the page does.
    pub(in crate::shell) fn focus_resting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(resting) = &self.resting {
            let focus = resting.bar.read(cx).field_focus(cx);
            window.focus(&focus, cx);
        }
    }

    /// The keys the resting bar hands the page: ⇥ flips the switch, Esc in Ask goes back to
    /// Search and in Search does nothing, and ↩ in Ask runs the task.
    fn resting_key(&mut self, key: InPlaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resting) = &self.resting else {
            return;
        };
        let on_ask = resting.bar.read(cx).on_ask();
        match key {
            InPlaceKey::Flip => self.set_on_ask(!on_ask, window, cx),
            InPlaceKey::Escape if on_ask => self.set_on_ask(false, window, cx),
            InPlaceKey::Escape => {}
            InPlaceKey::Run => self.run_composed(window, cx),
            // The last chip in the field goes: the folder, then the files one by one, then the
            // pages.
            InPlaceKey::DropChip => {
                let Some(resting) = &self.resting else {
                    return;
                };
                if resting.folder.is_some() {
                    self.revoke_composer_folder(cx);
                } else if let Some(file) = resting.files.last().cloned() {
                    self.drop_file(file, cx);
                } else if let Some(page) = resting.pages.last() {
                    self.toggle_page(page.tab, cx);
                }
            }
            InPlaceKey::Given(at) => {
                let Some(resting) = &self.resting else {
                    return;
                };
                let clicked = window.mouse_position();
                match resting.given().get(at) {
                    Some(Given::Files(_)) => self.composer_files_menu(clicked, cx),
                    Some(Given::Folder(_)) => self.composer_folder_menu(clicked, cx),
                    None => {}
                }
            }
        }
    }

    /// The page's switch goes to Ask, or back to Search.
    pub(in crate::shell) fn set_on_ask(
        &mut self,
        on_ask: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if on_ask && !self.offers_tasks() {
            return;
        }
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !on_ask {
            resting.forget_picks();
        }
        let bar = resting.bar.clone();
        bar.update(cx, |bar, cx| {
            let words = match on_ask {
                true => ask_words(bar.assistant(cx).as_ref().map(|agent| agent.name.as_ref())),
                false => SEARCH_WORDS.into(),
            };
            bar.set_on_ask(on_ask, words, window, cx)
        });
        log!("browser: composer on {}", if on_ask { ASK } else { SEARCH });
        cx.notify();
    }

    /// What the browser knows of the assistants changed, as once it has looked for them: the
    /// page's bar learns them again, and in Ask the field names the one it found.
    pub(in crate::shell) fn assistants_learned(&mut self, cx: &mut Context<Self>) {
        let Some(resting) = &self.resting else {
            return;
        };
        let changed = resting.bar.update(cx, |bar, cx| {
            let changed = bar.agents_changed(cx);
            if changed && bar.on_ask() {
                let assistant = bar.assistant(cx);
                let name = assistant.as_ref().map(|agent| agent.name.as_ref());
                bar.set_placeholder(ask_words(name), cx);
            }
            changed
        });
        if changed {
            cx.notify();
        }
    }

    /// ↩ in Ask: what was typed runs as a task on the assistant, from this Space.
    fn run_composed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        let bar = resting.bar.clone();
        let Some((agent, asked)) = bar.update(cx, |bar, cx| bar.take_task(window, cx)) else {
            return;
        };
        let model = resting
            .model
            .take()
            .filter(|(on, _)| *on == agent)
            .map(|(_, id)| id);
        let start = Start {
            tier: resting.tier,
            model,
            pages: resting.pages.iter().map(|page| page.url.clone()).collect(),
            only_added: resting.only_added,
            files: Picked {
                files: std::mem::take(&mut resting.files),
                folder: resting.folder.take(),
            },
        };
        resting.forget_picks();
        log!("browser: composer runs a task on {agent}");
        let home = self.space();
        self.show_started = Some(ShowStarted::Asked {
            home,
            over: self.selected,
        });
        self.browser.start_task(&agent, &asked, home, start, cx);
        cx.notify();
    }

    /// ⌘T's ↩ is starting a task from this window's Space, over the tab it shows: as the
    /// composer's does, its conversation takes that tab's place as its card comes in.
    pub fn bar_starting(&mut self) {
        self.show_started = Some(ShowStarted::Asked {
            home: self.space(),
            over: self.selected,
        });
    }

    /// The Agents' answer to the composer's or ⌘T's last ↩: the task it started, whose view alone
    /// opens as its card comes in, or None, when it started none and no other task's card opens a
    /// view.
    pub fn composed_started(&mut self, agent: Option<SharedString>) {
        let Some(ShowStarted::Asked { over, .. }) = self.show_started else {
            return;
        };
        self.show_started = agent.map(|agent| ShowStarted::Task { agent, over });
    }

    /// The composer's and ⌘T's ↩ show the task they started in the place of the page they were
    /// asked over, the empty page or a tab, while it still shows. The card comes before the window
    /// hears of the task's Space, which the core tells after the call that made it, so until then
    /// the start waits for that Space, by its task. `was`, the cards before these, tells the new
    /// ones while the Agents haven't said which task it started.
    pub(in crate::shell) fn show_started_task(
        &mut self,
        was: Option<&[SharedString]>,
        cx: &mut Context<Self>,
    ) {
        let started = |card: &&Card| match &self.show_started {
            Some(ShowStarted::Asked { home, .. }) => {
                card.home == *home && was.is_some_and(|was| !was.contains(&card.agent))
            }
            Some(ShowStarted::Task { agent, .. }) => card.agent == *agent,
            None => false,
        };
        let Some(card) = self.task_cards.iter().find(started) else {
            return;
        };
        let over = self.show_started.as_ref().map_or(0, ShowStarted::over);
        let (agent, home, space) = (card.agent.clone(), card.home, card.space);
        if space != 0 && self.task_space(space).is_none() {
            self.show_started = Some(ShowStarted::Task { agent, over });
            return;
        }
        self.show_started = None;
        if self.selected == over && self.space() == home && self.talk_agent().is_none() {
            self.show_task(agent, cx);
        }
    }

    /// Puts the composer's level back to Work on pages, and its model back to Settings'.
    pub(in crate::shell) fn forget_composer_tier(&mut self) {
        if let Some(resting) = &mut self.resting {
            resting.forget_picks();
        }
    }

    /// The assistant's chip's menu: its own default first, then the models it offers, from the
    /// list Settings' Model row is built from, the one the next task starts on checked. A pick
    /// holds for the next run only, and Settings keeps its own.
    pub(in crate::shell) fn model_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(resting) = &self.resting else {
            return;
        };
        let (bar, chip) = (resting.bar.clone(), resting.model_chip.get());
        let Some(agent) = bar.update(cx, |bar, cx| bar.assistant(cx)) else {
            return;
        };
        let chosen = next_model(resting.model.as_ref(), &agent);
        let default = agent.default_model.as_deref();
        let (ids, entries) = model_menu(&chosen, &agent.default_row, default, agent.models.clone());
        let items: Vec<(MenuEntry, MenuAction)> = (ids.into_iter().zip(entries))
            .map(|((id, _), entry)| {
                let on = agent.id.clone();
                let pick: MenuAction = Box::new(move |shell, cx| shell.pick_model(on, id, cx));
                (entry, pick)
            })
            .collect();
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The user's pick of where the next task may go: only the sites of the pages added, which
    /// needs one, or any site.
    pub(in crate::shell) fn pick_reach(&mut self, only: bool, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() || (only && resting.pages.is_empty()) {
            return;
        }
        resting.only_added = only;
        cx.notify();
    }

    /// The Space's pages the `+` menu offers: its pinned and Today tabs with an http(s) page whose
    /// address isn't too long to give a task, by title, those shown most recently first.
    fn pages_to_add(&self) -> Vec<(u64, SharedString)> {
        let Some(space) = self.shown_space() else {
            return Vec::new();
        };
        let tabs = self.recent_first(space_tabs(space).collect());
        (tabs.into_iter())
            .filter(|&tab| added_address(&self.url(tab)).is_some())
            .map(|tab| (tab, self.title(tab)))
            .collect()
    }

    /// Whether the `+` has anything to offer: files and folders where this system can wall a
    /// task in with them, or else a page of the Space's to add. Without either it's disabled in
    /// its place.
    pub(in crate::shell) fn plus_offers(&self, cx: &gpui::App) -> bool {
        self.browser.files_available(cx) || !self.pages_to_add().is_empty()
    }

    /// The `+` chip's menu. Where this system can wall a task in with its files: Add Page, whose
    /// submenu is the Space's pages, Add Files… and Add Folder…. Till then, the pages alone.
    pub(in crate::shell) fn composer_plus_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.browser.files_available(cx) {
            return self.pages_menu(clicked, cx);
        }
        let Some(resting) = &self.resting else {
            return;
        };
        let chip = resting.plus_chip.get();
        let added: Vec<u64> = resting.pages.iter().map(|page| page.tab).collect();
        let pages = self.pages_to_add();
        let none: MenuAction = Box::new(|_, _| {});
        let mut items: Vec<(MenuEntry, MenuAction)> = vec![(
            MenuEntry::new(files_words::ADD_PAGE, !pages.is_empty()),
            none,
        )];
        for (tab, title) in pages {
            let entry = MenuEntry {
                page: true,
                favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
                ..MenuEntry::named(title, true).checked(added.contains(&tab))
            };
            let pick: MenuAction = Box::new(move |shell, cx| shell.toggle_page(tab, cx));
            items.push((entry.under(0), pick));
        }
        let add_files: MenuAction = Box::new(|shell, cx| shell.add_files(cx));
        let add_folder: MenuAction = Box::new(|shell, cx| shell.add_folder(cx));
        items.push((MenuEntry::new(files_words::ADD_FILES, true), add_files));
        items.push((MenuEntry::new(files_words::ADD_FOLDER, true), add_folder));
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The attached files' chip's menu: each file by its name, checked, as a pick drops it.
    pub(in crate::shell) fn composer_files_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(resting) = &self.resting else {
            return;
        };
        let items: Vec<(MenuEntry, MenuAction)> = (resting.files.iter().cloned())
            .map(|file| {
                let entry = MenuEntry::named(files::shown_name(&file), true).checked(true);
                let drop: MenuAction = Box::new(move |shell, cx| shell.drop_file(file, cx));
                (entry, drop)
            })
            .collect();
        self.show_menu(items, clicked, None, cx);
    }

    /// The folder's chip's menu: its one act, Revoke.
    pub(in crate::shell) fn composer_folder_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !(self.resting.as_ref()).is_some_and(|resting| resting.folder.is_some()) {
            return;
        }
        let revoke: MenuAction = Box::new(|shell, cx| shell.revoke_composer_folder(cx));
        let items = vec![(MenuEntry::new(files_words::REVOKE, true), revoke)];
        self.show_menu(items, clicked, None, cx);
    }

    /// The `+` chip's menu where files and folders aren't available: the Space's pages, those
    /// added checked, as a menu of one row (Add Page) would be no menu. With none, the `+` is
    /// disabled.
    pub(in crate::shell) fn pages_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(resting) = &self.resting else {
            return;
        };
        let chip = resting.plus_chip.get();
        let added: Vec<u64> = resting.pages.iter().map(|page| page.tab).collect();
        let pages = self.pages_to_add();
        if pages.is_empty() {
            return;
        }
        let mut items: Vec<(MenuEntry, MenuAction)> = Vec::new();
        for (tab, title) in pages {
            let entry = MenuEntry {
                page: true,
                favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
                ..MenuEntry::named(title, true).checked(added.contains(&tab))
            };
            let pick: MenuAction = Box::new(move |shell, cx| shell.toggle_page(tab, cx));
            items.push((entry, pick));
        }
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The user's pick of a page on the `+` menu: added, its address as it is now, or taken off
    /// again if it was. With none left, the task may go to any site.
    pub(in crate::shell) fn toggle_page(&mut self, tab: u64, cx: &mut Context<Self>) {
        let (Some(url), title) = (added_address(&self.url(tab)), self.title(tab)) else {
            return;
        };
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        match resting.pages.iter().position(|page| page.tab == tab) {
            Some(at) => _ = resting.pages.remove(at),
            None => resting.pages.push(Added { tab, url, title }),
        }
        resting.only_added &= !resting.pages.is_empty();
        log!("browser: composer pages {}", resting.pages.len());
        cx.notify();
    }

    /// The user's pick on the assistant's menu, while the field is in Ask: for the next task on
    /// `agent` alone.
    pub(in crate::shell) fn pick_model(
        &mut self,
        agent: SharedString,
        id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        log!("browser: composer model picked");
        resting.model = Some((agent, id));
        cx.notify();
    }

    /// The level the next ↩ in Ask starts a task at.
    pub(in crate::shell) fn composer_tier(&self) -> Tier {
        self.resting
            .as_ref()
            .map(|resting| resting.tier)
            .unwrap_or_default()
    }

    /// The level chip's menu, names only: the three levels, the one the next task starts at
    /// checked; where it may go, which is any site until the composer holds a page the user added;
    /// and Settings, the one place the levels are explained. A pick holds for the next run only. It
    /// hangs under the chip, its names under the chip's own word.
    pub(in crate::shell) fn tier_menu(
        &mut self,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let current = self.composer_tier();
        let mut items: Vec<(MenuEntry, MenuAction)> = (Tier::ALL.into_iter())
            .map(|tier| {
                let pick: MenuAction = Box::new(move |shell, cx| shell.pick_tier(tier, cx));
                (
                    MenuEntry::named(tier.label(), true).checked(tier == current),
                    pick,
                )
            })
            .collect();
        let none: fn() -> MenuAction = || Box::new(|_, _| {});
        let (added, only) = (self.resting.as_ref()).map_or((false, false), |resting| {
            (!resting.pages.is_empty(), resting.only_added)
        });
        let reach =
            |only: bool| -> MenuAction { Box::new(move |shell, cx| shell.pick_reach(only, cx)) };
        items.push((MenuEntry::line(), none()));
        items.push((
            MenuEntry::named(ANY_SITE, true).checked(!only),
            reach(false),
        ));
        items.push((
            MenuEntry::named(ONLY_ADDED, added).checked(only),
            reach(true),
        ));
        items.push((MenuEntry::line(), none()));
        let settings: MenuAction = Box::new(|shell, cx| shell.browser.agent_access(cx));
        items.push((MenuEntry::new(SETTINGS, true), settings));
        let chip = (self.resting.as_ref()).and_then(|resting| resting.tier_chip.get());
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The level chip's menu under a task's reply field, as the composer's: names only, the
    /// task's level checked, where it may go, and Settings. Only the user's pick here changes the
    /// task's level, through the press its refusal's button makes; there's no agent path to it. A
    /// running task can widen to any site but not narrow to the sites added, which it never had.
    pub(in crate::shell) fn task_tier_menu(
        &mut self,
        agent: SharedString,
        current: Tier,
        only: bool,
        chip: Option<Bounds<Pixels>>,
        clicked: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let press = |action: TaskAction| -> MenuAction {
            let agent = agent.clone();
            Box::new(move |shell, cx| shell.browser.task(agent, action, cx))
        };
        let mut items: Vec<(MenuEntry, MenuAction)> = (Tier::ALL.into_iter())
            .map(|tier| {
                (
                    MenuEntry::named(tier.label(), true).checked(tier == current),
                    press(TaskAction::PickTier(tier)),
                )
            })
            .collect();
        let none: fn() -> MenuAction = || Box::new(|_, _| {});
        items.push((MenuEntry::line(), none()));
        items.push((
            MenuEntry::named(ANY_SITE, true).checked(!only),
            press(TaskAction::PickAnySite),
        ));
        items.push((MenuEntry::named(ONLY_ADDED, only).checked(only), none()));
        items.push((MenuEntry::line(), none()));
        let settings: MenuAction = Box::new(|shell, cx| shell.browser.agent_access(cx));
        items.push((MenuEntry::new(SETTINGS, true), settings));
        self.show_menu(items, hung_under(chip, clicked), None, cx);
    }

    /// The user's pick on the level menu, while the field is in Ask.
    pub(in crate::shell) fn pick_tier(&mut self, tier: Tier, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        resting.tier = tier;
        log!("browser: composer level {}", tier.label());
        cx.notify();
    }

    /// Whether the page offers Ask, its switch and its key. A private window starts no task, as
    /// agents get no path into one, so it shows Search alone; it's sent no cards (`one_space`).
    pub(in crate::shell) fn offers_tasks(&self) -> bool {
        !self.private
    }

    /// The page in the card at `frame`: the mark, the field, Ask's line and the recent tasks.
    pub(in crate::shell) fn composer_page(
        &mut self,
        frame: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let width = f32::from(frame.size.width);
        let resting = self.resting.as_ref()?;
        let (bar, painted) = (resting.bar.clone(), resting.painted);
        let palette = self.palette;
        let light = palette.tone == Tone::Light;
        let (fill, near, far) = match light {
            true => (
                Hsla::from(palette.underlay).blend(palette.plate(URL_FIELD)),
                LIGHT_FIELD_SHADOW_NEAR,
                LIGHT_FIELD_SHADOW_FAR,
            ),
            false => (
                palette.plate(FIELD_FILL),
                FIELD_SHADOW_NEAR,
                FIELD_SHADOW_FAR,
            ),
        };
        // The pages added are chips in the field, before what's typed, as ⌘T's ask has its page.
        let added: Vec<_> = (self.resting.iter())
            .flat_map(|resting| &resting.pages)
            .map(|page| (page.url.clone().into(), page.title.clone()))
            .collect();
        let given = (self.resting.as_ref()).map_or_else(Vec::new, Resting::given);
        let field = bar.update(cx, |bar, cx| {
            bar.set_added(added, cx);
            bar.set_given(given, cx);
            bar.field(fill, window, cx)
        });
        let on_ask = bar.read(cx).on_ask();
        // A narrow window keeps the field inside the card's gutters.
        let field_width = FIELD_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let line_width = LINE_WIDTH.min(field_width - (FIELD_WIDTH - LINE_WIDTH));
        let tab = div()
            .flex_none()
            .mr(px(KEY_GAP))
            .opacity(KEY_OPACITY)
            .h(px(KEY_HEIGHT))
            .px(px(KEY_INSET))
            .flex()
            .items_center()
            .rounded(px(KEY_RADIUS))
            .bg(palette.plate(KEY_FILL))
            .text_size(px(KEY_SIZE))
            .text_color(palette.text(KEY_INK))
            .child(key_name("tab", OFF_THE_MAC));
        let field = div()
            .id("composer-field")
            .flex_none()
            .w(px(field_width))
            .h(px(FIELD_HEIGHT))
            .pl(px(FIELD_LEFT))
            .pr(px(FIELD_RIGHT))
            .flex()
            .items_center()
            .rounded(px(FIELD_RADIUS))
            .bg(fill)
            .border_1()
            .border_color(rim(&palette, FIELD_RIM))
            .shadow(vec![
                shadow(rgba(near).into(), 1., 3.),
                shadow(rgba(far).into(), 14., 36.),
            ])
            .text_size(px(FIELD_SIZE))
            .text_color(palette.text(1.))
            .child(div().flex_1().min_w_0().flex().child(field));
        let field = match self.offers_tasks() {
            true => field.child(tab).child(self.switch(on_ask, cx)),
            false => field,
        };
        let cards: Vec<Card> = match painted {
            true => self.latest_tasks(CARDS as usize),
            false => Vec::new(),
        };
        let row = command_bar::centered_row_middle(f32::from(self.viewport.height))
            - f32::from(frame.top());
        let plates = cards.iter().any(|card| self.plate_of(card).is_some());
        let top = group_top(row, f32::from(frame.size.height), !cards.is_empty(), plates);
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(top))
            .child(super::mark(1., &palette).mb(px(MARK_GAP)))
            .child(field)
            .child(self.ask_line(on_ask, line_width, cx));
        if cards.is_empty() {
            return Some(page);
        }
        let label_width = LABEL_WIDTH.min(width - 2. * FIELD_GUTTER).max(0.);
        let cards: Vec<_> = (cards.into_iter().enumerate())
            .map(|(index, card)| self.recent_card(index, card, cx))
            .collect();
        Some(
            page.child(
                div()
                    .w(px(label_width))
                    .line_height(px(LABEL_LINE))
                    .mt(px(LABEL_TOP))
                    .mb(px(LABEL_BOTTOM))
                    .pl(px(LABEL_INSET))
                    .text_size(px(LABEL_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text(1.))
                    .child(LATEST),
            )
            .child(
                div()
                    .w(px(label_width))
                    .flex()
                    .gap(px(CARD_GAP))
                    .overflow_hidden()
                    .children(cards),
            ),
        )
    }

    /// Search | Ask, at the field's end: a click on a side flips to it.
    fn switch(&self, on_ask: bool, cx: &mut Context<Self>) -> gpui::Div {
        let palette = self.palette;
        let part = |words: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(words)
                .h(px(SWITCH_HEIGHT))
                .px(px(SWITCH_PART_INSET))
                .flex()
                .items_center()
                .rounded(px(SWITCH_PART_RADIUS))
                .when(on, |part| {
                    part.bg(palette.plate(SWITCH_ON))
                        .border_1()
                        .border_color(rgba(SWITCH_ON_RIM))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text(1.))
                })
                .when(!on, |part| part.text_color(palette.text(SWITCH_INK)))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.set_on_ask(words == ASK, window, cx)
                }))
                .child(words)
        };
        div()
            .flex_none()
            .p(px(SWITCH_INSET))
            .gap(px(SWITCH_INSET))
            .flex()
            .rounded(px(SWITCH_RADIUS))
            .bg(palette.plate(SWITCH_FILL))
            .text_size(px(SWITCH_SIZE))
            .child(part(SEARCH, !on_ask, cx))
            .child(part(ASK, on_ask, cx))
    }

    /// Ask's line under the field: add a page, the task's access, and the assistant and its model,
    /// which a page added leaves where they are, as its chip is in the field.
    /// It keeps its room in Search, so nothing moves as the switch flips.
    fn ask_line(&mut self, on_ask: bool, width: f32, cx: &mut Context<Self>) -> gpui::Div {
        let palette = self.palette;
        let line = div()
            .relative()
            .w(px(width))
            .mt(px(LINE_TOP))
            .h(px(CHIP_HEIGHT))
            .flex()
            .items_center()
            .text_size(px(CHIP_SIZE))
            .text_color(palette.text(CHIP_INK));
        if !on_ask {
            return line;
        }
        let chip = || {
            div()
                .flex_none()
                .h(px(CHIP_HEIGHT))
                .px(px(CHIP_INSET))
                .flex()
                .items_center()
                .gap(px(CHIP_GAP))
                .rounded(px(CHIP_RADIUS))
        };
        let glyph = |path: &'static str, size: f32| {
            svg()
                .flex_none()
                .path(path)
                .size(px(size))
                .text_color(palette.glyph(CHIP_INK))
        };
        let tier_chip = (self.resting.as_ref()).map(|resting| resting.tier_chip.clone());
        let model_chip = (self.resting.as_ref()).map(|resting| resting.model_chip.clone());
        let plus_chip = (self.resting.as_ref()).map(|resting| resting.plus_chip.clone());
        let refused = (self.resting.as_ref()).and_then(|resting| resting.refused.clone());
        let assistant = (self.resting.as_ref()).and_then(|resting| {
            let agent = resting.bar.update(cx, |bar, cx| bar.assistant(cx))?;
            Some(model_words(
                &agent,
                &next_model(resting.model.as_ref(), &agent),
            ))
        });
        // With nothing to add, the + stays in its place, disabled: no click that shows nothing.
        let offers = self.plus_offers(cx);
        line.child(
            chip()
                .id("composer-plus")
                .relative()
                .when(offers, |chip| {
                    chip.on_click(cx.listener(|shell, event: &gpui::ClickEvent, _, cx| {
                        shell.composer_plus_menu(event.position(), cx)
                    }))
                })
                .children(plus_chip.as_ref().map(drawn_at))
                .child(glyph("icons/plus.svg", CHIP_GLYPH).when(!offers, |glyph| {
                    glyph.text_color(palette.glyph(CHIP_DISABLED))
                })),
        )
        .child(
            chip()
                .id("composer-tier")
                .relative()
                .on_click(cx.listener(|shell, event: &gpui::ClickEvent, _, cx| {
                    shell.tier_menu(event.position(), cx)
                }))
                .children(tier_chip.as_ref().map(drawn_at))
                .child(self.composer_tier().label())
                .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON)),
        )
        .child(div().flex_1())
        .children(assistant.map(|assistant| {
            chip()
                .id("composer-model")
                .relative()
                .on_click(cx.listener(|shell, event: &gpui::ClickEvent, _, cx| {
                    shell.model_menu(event.position(), cx)
                }))
                .children(model_chip.as_ref().map(drawn_at))
                .child(assistant)
                .child(glyph("icons/chevron-down.svg", CHIP_CHEVRON))
        }))
        // Under the line, so nothing on it moves.
        .children(refused.map(|refused| {
            div()
                .absolute()
                .top(px(CHIP_HEIGHT + LINE_TOP))
                .left(px(CHIP_INSET))
                .right(px(CHIP_INSET))
                .child(refused)
        }))
    }

    /// A recent task's card: its state, its title and its last steps, the one under way in full
    /// ink (or its status, with no steps this turn), over the picture its last page's tile ended
    /// on. Once over, its one line is a done task's answer's first or, quiet, a stopped one's
    /// last finished step, if it has one; a task a run before left says when it ended and has
    /// none. A card with no picture ends after its words. A click opens it.
    fn recent_card(
        &self,
        index: usize,
        card: Card,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = self.palette;
        let state = match &card.ended_at {
            Some(at) => format!("{} · {at}", card.word.alone(&card.name)).into(),
            None => card.word.alone(&card.name),
        };
        let dot = (card.dot == Dot::Working).then(|| {
            div()
                .flex_none()
                .size(px(DOT))
                .rounded_full()
                .bg(rgb(STATUS_AGENT))
        });
        let working = card.dot == Dot::Working;
        let lines: Vec<(SharedString, bool)> = match card.word {
            _ if card.ended_at.is_some() => Vec::new(),
            // A done task says its answer's first line; a stopped one only its last finished
            // step, quiet, or nothing after its title.
            Word::Done | Word::Stopped => (card.line.clone().into_iter())
                .map(|line| (line, card.word == Word::Done))
                .collect(),
            _ if card.steps.is_empty() => vec![(card.status.clone(), true)],
            _ => {
                let last = card.steps.len() - 1;
                (card.steps.iter().cloned().enumerate())
                    .map(|(at, step)| (step, working && at == last))
                    .collect()
            }
        };
        let lines = lines.into_iter().map(|(line, now)| {
            div()
                .whitespace_nowrap()
                .truncate()
                .text_color(match now {
                    true => palette.text(1.),
                    false => palette.text(STEP_INK),
                })
                .child(line)
        });
        let plate = self.plate_of(&card).map(|picture| {
            let corners = Corners::all(px(PAGE_RADIUS));
            div()
                .h(px(PAGE_HEIGHT))
                .mx(px(PAGE_INSET))
                .mb(px(PAGE_INSET))
                .rounded(px(PAGE_RADIUS))
                .bg(palette.plate(PAGE_FILL))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let drawn = covering(bounds, picture.size());
                            (window.paint_image(bounds, drawn, corners, picture.image(), 0, false))
                                .ok();
                        },
                    )
                    .size_full(),
                )
        });
        let agent = card.agent.clone();
        div()
            .id(("composer-task", index))
            .flex_none()
            .w(px(CARD_WIDTH))
            .flex()
            .flex_col()
            .rounded(px(CARD_RADIUS))
            .bg(palette.plate(CARD_FILL))
            .border_1()
            .border_color(rim(&palette, CARD_RIM))
            .overflow_hidden()
            .text_size(px(text::BODY))
            .on_click(cx.listener(move |shell, _, _, cx| shell.open_latest(agent.clone(), cx)))
            .child(
                div()
                    .h(px(CARD_WORDS))
                    .pt(px(CARD_TOP))
                    .px(px(CARD_INSET))
                    .pb(px(CARD_BOTTOM))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(STATE_HEIGHT))
                            .flex()
                            .items_center()
                            .gap(px(STATE_GAP))
                            .text_size(px(STATE_SIZE))
                            .text_color(match card.dot {
                                Dot::Waiting => palette.text(1.),
                                _ => palette.text(STATE_INK),
                            })
                            .children(dot)
                            .child(state),
                    )
                    .child(
                        div()
                            .my(px(TITLE_GAP))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text(1.))
                            .whitespace_nowrap()
                            .truncate()
                            .child(card.title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .text_size(px(STEP_SIZE))
                            .line_height(px(STEP_LINE))
                            .children(lines),
                    ),
            )
            .children(plate)
    }
}

/// How far under a card `height` tall the page's group starts, with its field's row's middle
/// `row` under the card's top as ⌘T's bar's is: there, or as far up as the group needs to fit,
/// through its cards when it has any (`plates` if one has its site's plate), never above `TOP`.
pub(in crate::shell) fn group_top(row: f32, height: f32, cards: bool, plates: bool) -> f32 {
    let card = match plates {
        // A card is its words and its plate inside its rim, 1 on each side.
        true => 2. + CARD_WORDS + PAGE_HEIGHT + PAGE_INSET,
        false => 2. + CARD_WORDS,
    };
    let below = match cards {
        true => LABEL_TOP + LABEL_LINE + LABEL_BOTTOM + card,
        false => 0.,
    };
    let above_field = super::MARK_HEIGHT + MARK_GAP;
    let group = above_field + FIELD_HEIGHT + LINE_TOP + CHIP_HEIGHT + below;
    let level = row - FIELD_HEIGHT / 2. - above_field;
    level.min(height - TOP - group).max(TOP)
}

/// A plate's rim: white `dark` on a dark window, the ink on a light one, where white doesn't show.
fn rim(palette: &Palette, dark: u32) -> Hsla {
    match palette.tone {
        Tone::Dark => rgba(dark).into(),
        Tone::Light => palette.plate(LIGHT_RIM),
    }
}

/// A shadow straight down, `y` under and `blur` wide.
fn shadow(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}
