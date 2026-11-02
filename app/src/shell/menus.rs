//! The sidebar's menus, a Space's, a tab's, a favorite's, a folder's and the plus
//! button's, and what their items do.

use std::sync::Arc;

use gpui::{App, Context, Hsla, Image, Pixels, Point, Rgba, SharedString, rgb};

use super::{
    AgentGrants, Command, MAX_PANES, ProfileChoice, Row, Shell, UserKeys, clean_link, files,
    commands::markdown_text,
    undo::{Prop, Value},
    user_keys,
};
use crate::{
    dialog::{Field, Question},
    menu::{Highlight, Look, Swatch},
    space_icon::{SpaceIcon, has_icon},
    palette::Tone,
};

/// ilo's nouns, which stay capitalized in a sentence, as Chrome's Incognito does.
const PROPER_NOUNS: [&str; 4] = ["Space", "Spaces", "Library", "Markdown"];

/// `title`, written in title case, in sentence case, as Chromium's menus and the Windows and GNOME
/// guidelines write theirs off the Mac, so ilo's menus read as the window's others do there.
fn sentence_case(title: &str) -> String {
    let word = |(i, word): (usize, &str)| {
        let noun = word.trim_end_matches('…');
        // An acronym's plural too, as Copy URLs.
        let stem = noun.strip_suffix('s').unwrap_or(noun);
        let acronym = stem.len() > 1 && stem.chars().all(|c| c.is_ascii_uppercase());
        match i == 0 || acronym || PROPER_NOUNS.contains(&noun) {
            true => word.to_string(),
            false => word.to_lowercase(),
        }
    };
    title.split(' ').enumerate().map(word).collect::<Vec<_>>().join(" ")
}

/// What an item of a menu does once chosen.
pub(super) type MenuAction = Box<dyn FnOnce(&mut Shell, &mut Context<Shell>)>;

/// An item in a menu the host shows. A line between items has no title.
#[derive(Clone)]
pub struct MenuEntry {
    pub title: SharedString,
    pub enabled: bool,
    /// The keystroke it shows, as SHORTCUTS writes it.
    pub shortcut: Option<String>,
    /// The item whose submenu it's in, by index.
    pub parent: Option<usize>,
    /// Whether it shows a check mark.
    pub checked: bool,
    /// Whether it deletes something, which ilo's menu shows in red.
    pub destructive: bool,
    /// Whether the menu opens with its title at the point it's shown at, highlighted, as a pop-up
    /// button's menu opens over the button's title.
    pub over: bool,
    /// The Space it stands for, whose swatch goes before its title.
    pub swatch: Option<Swatch>,
    /// Whether it's shown and chosen in place of the item before it while ⌥ is held, as AppKit's
    /// alternates are, taking no row of its own.
    pub alternate: bool,
    /// Whether it stands for a page, whose favicon goes before its title, or a globe without one.
    pub page: bool,
    pub favicon: Option<Arc<Image>>,
}

impl MenuEntry {
    /// An item ilo names, in the platform's case.
    pub fn new(title: &'static str, enabled: bool) -> Self {
        MenuEntry::named(MenuEntry::label(title), enabled)
    }

    /// `title`, an item ilo names, in the platform's case, also for a pop-up button showing it.
    pub fn label(title: &'static str) -> SharedString {
        match super::OFF_THE_MAC {
            true => sentence_case(title).into(),
            false => title.into(),
        }
    }

    /// An item named by what it stands for, such as a Space's name, a page's title or Chromium's
    /// item, as written.
    pub fn named(title: impl Into<SharedString>, enabled: bool) -> Self {
        MenuEntry {
            title: title.into(),
            enabled,
            shortcut: None,
            parent: None,
            checked: false,
            destructive: false,
            over: false,
            swatch: None,
            alternate: false,
            page: false,
            favicon: None,
        }
    }

    pub fn line() -> Self {
        MenuEntry::new("", false)
    }

    /// In the submenu of the item at `parent`.
    fn under(self, parent: usize) -> Self {
        MenuEntry { parent: Some(parent), ..self }
    }

    pub(super) fn checked(self, checked: bool) -> Self {
        MenuEntry { checked, ..self }
    }

    pub(crate) fn destructive(self) -> Self {
        MenuEntry { destructive: true, ..self }
    }

    /// The item before it's ⌥ alternate.
    fn alternate(self) -> Self {
        MenuEntry { alternate: true, ..self }
    }

    fn swatch(self, swatch: Swatch) -> Self {
        MenuEntry { swatch: Some(swatch), ..self }
    }

    /// For a page with `favicon`.
    pub(super) fn page(self, favicon: Option<Arc<Image>>) -> Self {
        MenuEntry { page: true, favicon, ..self }
    }

    /// Shows the shortcut that runs `command`.
    fn shortcut(self, command: Command, keys: &UserKeys) -> Self {
        let shortcut = keys.keys(command).first().map(|&key| key.to_string());
        MenuEntry { shortcut, ..self }
    }
}

/// A new profile's name as Chrome numbers them, "Profile 2" after the first, that `names` hasn't.
pub fn new_profile_name(names: &[SharedString]) -> String {
    (names.len() + 1..)
        .map(|number| format!("Profile {number}"))
        .find(|name| !names.iter().any(|each| each == name))
        .unwrap_or_default()
}

impl Shell {
    /// The menu a right-click on a Space's name or dot shows (Arc), with its icon (ours).
    pub(super) fn space_menu(
        &mut self,
        space: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // A private window's Space has nothing to change.
        if self.private {
            return;
        }
        let items = self.space_items(space, cx);
        self.show_menu(items, position, None, cx);
    }

    /// The menu a right-click on the sidebar's empty space in `space`'s page shows: New Tab, as
    /// ⌘T, New Folder…, then the Space's own menu (ours: Arc's isn't recorded, #433, #1088).
    pub(super) fn empty_space_menu(
        &mut self,
        space: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let new_tab = MenuEntry::new("New Tab", true).shortcut(Command::NewTab, user_keys(cx));
        let mut items: Vec<(MenuEntry, MenuAction)> = vec![(
            new_tab,
            // Into the Space it was asked in, which a swipe may not have shown yet.
            Box::new(move |shell, cx| {
                shell.show_space(space, cx);
                shell.run(Command::NewTab, "sidebar", cx)
            }),
        )];
        // A private window's Space has no folders, and nothing to change.
        if !self.private {
            let new_folder =
                MenuEntry::new("New Folder…", true).shortcut(Command::NewFolder, user_keys(cx));
            items.push((
                new_folder,
                Box::new(move |shell, cx| {
                    shell.show_space(space, cx);
                    shell.run(Command::NewFolder, "sidebar", cx)
                }),
            ));
            items.push((MenuEntry::line(), Box::new(|_, _| {})));
            // Its Profile submenu's items name their parent by where it is, now further down.
            let after = items.len();
            items.extend(self.space_items(space, cx).into_iter().map(|(entry, action)| {
                let parent = entry.parent.map(|parent| parent + after);
                (MenuEntry { parent, ..entry }, action)
            }));
        }
        self.show_menu(items, position, None, cx);
    }

    /// The Space's menu's items, for its name, its dot and the sidebar's empty space.
    fn space_items(&self, space: u64, cx: &App) -> Vec<(MenuEntry, MenuAction)> {
        let entry = self.spaces.iter().find(|each| each.id == space);
        let icon = entry.is_some_and(|each| has_icon(&each.icon));
        let theme = entry.is_some_and(|each| each.color.is_some() || each.theme.is_some());
        let mut items = space_menu_head(space, icon, theme);
        // An agent's Space leads to what the agent did in it, and lists what the user let it do
        // for good, to take back.
        if let Some(each) = entry.filter(|each| !each.agent.is_empty()) {
            let mut agent = self.agent_items(space, &each.agent, cx);
            agent.extend(items);
            items = agent;
        }
        let at = items.len();
        items.push((MenuEntry::new("Profile", true), Box::new(|_, _| {})));
        let mut profiles = self.profile_items(space);
        let deletes = self.delete_profile_items();
        if !deletes.is_empty() {
            profiles.push((MenuEntry::line(), Box::new(|_, _| {})));
        }
        profiles.extend(deletes);
        items.extend(profiles.into_iter().map(|(entry, action)| (entry.under(at), action)));
        // Ours: Arc exports every Space at once, from Settings.
        items.push((
            MenuEntry::new("Export as Markdown…", true),
            Box::new(move |shell, cx| {
                // check.sh waits for this line.
                log!("browser: command export-space via space menu");
                shell.export_space(space, cx)
            }),
        ));
        items.push((MenuEntry::line(), Box::new(|_, _| {})));
        items.push((
            MenuEntry::new("Delete Space", self.spaces.len() > 1).destructive(),
            Box::new(move |shell, cx| shell.delete_space(space, cx)),
        ));
        items
    }

    /// The head of an agent's Space's menu: its activity, Revoke Permission with what the user
    /// answered for good on each site, Take Back All Tabs and Disconnect.
    fn agent_items(&self, space: u64, name: &str, cx: &App) -> Vec<(MenuEntry, MenuAction)> {
        let AgentGrants { sites: grants, connected, tabs } =
            self.browser.agent_permissions(space, cx);
        let mut items: Vec<(MenuEntry, MenuAction)> = vec![
            (
                MenuEntry::new("Agent Activity…", true),
                Box::new(|shell, cx| shell.browser.agent_activity(cx)),
            ),
            (MenuEntry::new("Revoke Permission", !grants.is_empty()), Box::new(|_, _| {})),
        ];
        let several = grants.len() > 1;
        for (site, may) in grants {
            let title = MenuEntry::label(if may { "Always Allow" } else { "Never" });
            let title = format!("{title} on {site}");
            items.push((
                MenuEntry::named(title, true).under(1),
                Box::new(move |shell, cx| shell.browser.revoke_agent(space, Some(site), cx)),
            ));
        }
        if several {
            items.push((MenuEntry::line().under(1), Box::new(|_, _| {})));
            items.push((
                MenuEntry::new("All Sites", true).under(1),
                Box::new(move |shell, cx| shell.browser.revoke_agent(space, None, cx)),
            ));
        }
        items.push((
            MenuEntry::new("Take Back All Tabs", tabs),
            Box::new(move |shell, cx| shell.browser.take_back_agent_tabs(space, cx)),
        ));
        items.push((
            MenuEntry::named(format!("Disconnect {name}"), connected),
            Box::new(move |shell, cx| shell.browser.disconnect_agent(space, cx)),
        ));
        items.push((MenuEntry::line(), Box::new(|_, _| {})));
        items
    }

    /// Shows `entries` in ilo's own menu at `position` in the sidebar, in the window or the
    /// reveal's panel, in the Space's look, and calls `chosen` once it closes.
    pub(super) fn context_menu(
        &mut self,
        entries: Vec<MenuEntry>,
        position: Point<Pixels>,
        chosen: Box<dyn FnOnce(Option<usize>, &mut Shell, &mut Context<Shell>)>,
        cx: &mut Context<Self>,
    ) {
        self.context_menu_in(entries, position, self.sidebar_hidden, chosen, cx);
    }

    /// `context_menu` at `position` in the window, or in the reveal's panel if `in_reveal`.
    pub(super) fn context_menu_in(
        &mut self,
        entries: Vec<MenuEntry>,
        position: Point<Pixels>,
        in_reveal: bool,
        chosen: Box<dyn FnOnce(Option<usize>, &mut Shell, &mut Context<Shell>)>,
        cx: &mut Context<Self>,
    ) {
        let look = self.menu_look();
        self.menu_open = true;
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            in_reveal,
            look,
            Box::new(move |index, cx| {
                shell
                    .update(cx, |shell, cx| {
                        shell.menu_open = false;
                        chosen(index, shell, cx);
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// The look of the window's menus: the Space's window's, dark or light, with its accent under
    /// the row the pointer or the keys are on.
    pub fn menu_look(&self) -> Look {
        let palette = self.palette_of(self.space);
        Look { light: palette.tone == Tone::Light, highlight: Highlight::of(&palette) }
    }

    /// Shows `items` at `position` in ilo's menu, or with the one at `over` there, as a pop-up
    /// button's, and does what the one chosen does.
    pub(super) fn show_menu(
        &mut self,
        items: Vec<(MenuEntry, MenuAction)>,
        position: Point<Pixels>,
        over: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.show_menu_in(items, position, over, self.sidebar_hidden, cx);
    }

    /// `show_menu` in the window, or in the reveal's panel if `in_reveal`.
    pub(super) fn show_menu_in(
        &mut self,
        items: Vec<(MenuEntry, MenuAction)>,
        position: Point<Pixels>,
        over: Option<usize>,
        in_reveal: bool,
        cx: &mut Context<Self>,
    ) {
        let entries = (items.iter().enumerate())
            .map(|(at, (entry, _))| MenuEntry { over: Some(at) == over, ..entry.clone() })
            .collect();
        self.context_menu_in(
            entries,
            position,
            in_reveal,
            Box::new(move |chosen, shell, cx| {
                if let Some((_, action)) = chosen.and_then(|at| items.into_iter().nth(at)) {
                    action(shell, cx)
                }
            }),
            cx,
        );
    }

    /// The profiles a Space can be in, its own checked, which only an empty Space can change to, as
    /// its tabs' pages are in its own; and a new one.
    pub(super) fn profile_items(&self, space: u64) -> Vec<(MenuEntry, MenuAction)> {
        let Some((choices, new)) = self.profile_choices(space) else {
            return Vec::new();
        };
        let mut items: Vec<(MenuEntry, MenuAction)> = Vec::new();
        for choice in choices {
            let profile = choice.profile;
            items.push((
                MenuEntry::named(choice.name, choice.enabled).checked(choice.checked),
                Box::new(move |shell, cx| shell.browser.set_space_profile(space, profile, cx)),
            ));
        }
        items.push((MenuEntry::line(), Box::new(|_, _| {})));
        items.push((
            MenuEntry::new("New Profile", new),
            Box::new(move |shell, cx| shell.new_profile(space, cx)),
        ));
        items
    }

    /// The profiles the Space can be in, as its menu and Spaces › Change Profile list them, and
    /// whether it can go in a new one: only an empty Space changes profile.
    pub(super) fn profile_choices(&self, space: u64) -> Option<(Vec<ProfileChoice>, bool)> {
        let entry = self.spaces.iter().find(|each| each.id == space)?;
        let (current, empty) = (entry.profile, entry.pinned.is_empty() && entry.today.is_empty());
        let choices = (self.profiles.iter())
            .map(|(profile, name)| ProfileChoice {
                profile: *profile,
                name: name.clone(),
                enabled: empty || *profile == current,
                checked: *profile == current,
            })
            .collect();
        Some((choices, empty))
    }

    /// Spaces › Change Profile's choice: puts the Space shown in `profile`, or with none in a new
    /// one, as its menu's Profile items do. Agents reach the same through `bc_space_set_profile`.
    pub(super) fn change_profile(&mut self, profile: Option<u64>, cx: &mut Context<Self>) {
        let Some((choices, new)) = self.profile_choices(self.space) else {
            return;
        };
        match profile {
            Some(profile) if choices.iter().any(|each| each.profile == profile && each.enabled) => {
                self.browser.set_space_profile(self.space, profile, cx)
            }
            None if new => self.new_profile(self.space, cx),
            _ => {}
        }
    }

    /// A Delete item for each profile nothing is in.
    fn delete_profile_items(&self) -> Vec<(MenuEntry, MenuAction)> {
        // The one the browser started with stays, as the browser needs one.
        (self.profiles.iter().skip(1))
            .filter(|(profile, _)| {
                !self.spaces.iter().any(|each| each.profile == *profile)
                    && !self.favorite_holders.values().any(|each| each == profile)
            })
            .map(|(profile, name)| {
                let (profile, name) = (*profile, name.clone());
                let entry = MenuEntry::named(format!("Delete “{name}”"), true).destructive();
                let action: MenuAction =
                    Box::new(move |shell, cx| shell.delete_profile(profile, name, cx));
                (entry, action)
            })
            .collect()
    }

    /// Deletes the profile once the user confirms, as nothing brings it back.
    fn delete_profile(&mut self, profile: u64, name: SharedString, cx: &mut Context<Self>) {
        let shell = cx.weak_entity();
        self.browser.confirm(
            format!("Delete “{name}”?"),
            "Its history, cookies and logins go with it.".into(),
            "Delete",
            Box::new(move |cx| {
                shell.update(cx, |shell, cx| shell.browser.delete_profile(profile, cx)).ok();
            }),
            cx,
        );
    }

    /// Moves the tab, or the folder with what's in it, to the Space, warning the first time either
    /// goes to another profile's, where pages load again without this profile's cookies and logins.
    pub(super) fn move_to_space(
        &mut self,
        tab: u64,
        space: u64,
        pin: bool,
        cx: &mut Context<Self>,
    ) {
        let profile_of =
            |space| self.spaces.iter().find(|each| each.id == space).map(|each| each.profile);
        let to = profile_of(space);
        // The tab's own Space, which isn't the one shown once New Space… shows the new one.
        let from = (self.spaces.iter())
            .find(|each| each.pinned.contains(&tab) || each.today.contains(&tab))
            .map_or(self.space, |each| each.id);
        if profile_of(from) == to || !self.browser.warn_moving_profiles(cx) {
            return self.send_to_space(tab, space, pin, cx);
        }
        let name = (self.profiles.iter())
            .find(|(profile, _)| Some(*profile) == to)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let shell = cx.weak_entity();
        let (title, body) = match self.is_folder_anywhere(tab) {
            true => ("folder", "Its pages load"),
            false => ("tab", "Its page loads"),
        };
        self.browser.confirm(
            format!("Move this {title} to the “{name}” profile?"),
            format!(
                "{body} again there, without this profile's cookies and logins, so a site may ask \
                 you to sign in."
            ),
            "Move",
            Box::new(move |cx| {
                shell
                    .update(cx, |shell, cx| {
                        shell.browser.set_warn_moving_profiles(false, cx);
                        shell.send_to_space(tab, space, pin, cx);
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Makes a profile and puts the empty Space in it, named at once. In the New Space form it's
    /// made with the name Chrome would number it, which the Profile row then shows in a field, as
    /// Finder's New Folder does. Elsewhere the dialog asks for the name first, and Cancel makes
    /// none. Left empty, the name is the numbered one.
    fn new_profile(&mut self, space: u64, cx: &mut Context<Self>) {
        let names: Vec<SharedString> = self.profiles.iter().map(|(_, name)| name.clone()).collect();
        let numbered = new_profile_name(&names);
        if self.form().is_some_and(|form| form.space == space) {
            if let Some(profile) = self.make_profile(space, &numbered, cx) {
                self.name_profile(profile, numbered.into(), cx);
            }
            return;
        }
        let question = Question {
            icon: None,
            message: "New Profile".into(),
            detail: "A profile keeps its own logins, cookies, history and extensions.".into(),
            list: Vec::new(),
            action: "Create".into(),
            cancel: "Cancel".into(),
            tone: crate::dialog::Tone::Accent,
            also: None,
            fields: vec![Field {
                placeholder: numbered.clone().into(),
                text: numbered.clone().into(),
                secure: false,
            }],
            check: None,
            on_return: crate::dialog::Return::Action,
        };
        let shell = cx.weak_entity();
        let made = Box::new(move |texts: Vec<String>, cx: &mut App| {
            let typed = texts.first().map_or("", |text| text.trim());
            let name = if typed.is_empty() { numbered } else { typed.to_string() };
            shell.update(cx, |shell, cx| shell.make_profile(space, &name, cx)).ok();
        });
        self.browser.tell(question, made, cx);
    }

    /// Makes a profile named `name`, puts the empty Space in it, and returns it.
    fn make_profile(&mut self, space: u64, name: &str, cx: &mut Context<Self>) -> Option<u64> {
        let profile = self.browser.create_profile(name, cx);
        // check.sh waits for this line.
        log!("browser: create profile {profile} {name}");
        if profile == 0 {
            return None;
        }
        self.browser.set_space_profile(space, profile, cx);
        Some(profile)
    }

    /// The menu a right-click on a tab's row shows: Chrome's tab actions that Arc's sidebar has,
    /// with their shortcuts.
    pub(super) fn tab_menu(&mut self, tab: u64, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.end_hovers(cx);
        let row = Row::Tab(self.row_of(tab));
        // A right-click on a row not chosen lets the chosen ones go, as a plain click does.
        let chosen = self.chosen.contains(&self.row_of(tab));
        if !chosen {
            self.unchoose(cx);
        }
        let keys = user_keys(cx);
        let items = if chosen {
            self.chosen_menu(keys)
        } else if self.favorites.contains(&tab) {
            self.favorite_menu(keys, tab)
        } else if self.is_folder(tab) {
            self.folder_menu(tab)
        } else if self.pinned().contains(&tab) {
            self.pinned_tab_menu(keys, tab)
        } else {
            self.today_tab_menu(keys, tab, position)
        };
        // A tab the user holds from an agent says so first, and gives it back (#404).
        let mut held = self.held_items(tab);
        let shift = held.len();
        held.extend(items.into_iter().map(|(entry, name, action)| {
            let parent = entry.parent.map(|parent| parent + shift);
            (MenuEntry { parent, ..entry }, name, action)
        }));
        let items = held;
        let entries = items.iter().map(|(entry, ..)| entry.clone()).collect();
        self.context_menu(
            entries,
            position,
            Box::new(move |chosen, shell, cx| {
                // The fill goes as the pointer leaving the row takes it.
                shell.menu_row = None;
                if shell.hovered != Some(row) {
                    shell.unhover(row);
                }
                cx.notify();
                let item = chosen.and_then(|at| items.into_iter().nth(at));
                if let Some((_, name, action)) = item {
                    log!("browser: command {name} via tab menu");
                    action(shell, cx)
                }
            }),
            cx,
        );
        self.menu_row = Some(self.row_of(tab));
        cx.notify();
    }

    /// The head of the menu of a tab the user holds from the agent whose Space shows: that they
    /// took it back or the agent handed it to them, and Hand Back.
    fn held_items(&self, tab: u64) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let Some(space) = self.shown_space().filter(|space| !space.agent.is_empty()) else {
            return Vec::new();
        };
        let held = (self.agent_status.iter())
            .filter(|status| status.space == space.id)
            .flat_map(|status| &status.held)
            .find(|(id, _)| *id == tab);
        let Some((_, why)) = held else {
            return Vec::new();
        };
        let name = &space.agent;
        let state = match why.is_empty() {
            true => MenuEntry::label("Taken Back from"),
            false => MenuEntry::label("Handed to You by"),
        };
        vec![
            (MenuEntry::named(format!("{state} {name}"), false), "", Box::new(|_, _| {})),
            (
                MenuEntry::named(format!("{} {name}", MenuEntry::label("Hand Back to")), true),
                "hand-back",
                Box::new(move |shell, cx| shell.browser.hand_back(tab, cx)),
            ),
            (MenuEntry::line(), "", Box::new(|_, _| {})),
        ]
    }

    /// A favorite's menu.
    fn favorite_menu(
        &self,
        keys: &UserKeys,
        tab: u64,
    ) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let page = self.page(tab);
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = vec![
            (
                MenuEntry::new("Back to Favorite URL", page.away_from_pinned_url)
                    .shortcut(Command::BackToPinnedUrl, keys),
                "back-to-pinned-url",
                Box::new(move |shell, cx| shell.browser.back_to_pinned_url(tab, cx)),
            ),
            replace_pinned_url(tab, "Replace Favorite URL with Current", page.away_from_pinned_url),
            (
                MenuEntry::new("Duplicate Tab", true),
                "duplicate-tab",
                Box::new(move |shell, cx| {
                    shell.browser.duplicate(tab, cx);
                }),
            ),
            (
                MenuEntry::new("Remove from Favorites", !self.agents_space_shown()),
                "remove-from-favorites",
                // To the top of Today.
                Box::new(move |shell, cx| shell.move_tab_to(tab, false, 0, 0, cx)),
            ),
        ];
        items.push((
            MenuEntry::new("Change Icon…", true),
            "change-icon",
            Box::new(move |shell, cx| shell.change_icon(tab, cx)),
        ));
        if has_icon(&page.icon) {
            items.push(remove_icon(tab));
        }
        items.push((MenuEntry::line(), "", Box::new(|_, _| {})));
        // One closed already has nothing to close.
        items.push((
            MenuEntry::new("Close Tab", !page.unloaded).shortcut(Command::CloseTab, keys),
            "close-tab",
            Box::new(move |shell, cx| shell.close_tab(tab, false, cx)),
        ));
        items
    }

    /// The chosen rows' menu (ours, *interim* until Arc's is captured): what's done to them
    /// together, each as one step to undo.
    fn chosen_menu(&self, keys: &UserKeys) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        // A chosen folder isn't a tab to close or unpin.
        let tabs = !self.chosen_tabs().is_empty();
        let close: (MenuEntry, &'static str, MenuAction) = (
            MenuEntry::new("Close Tabs", tabs).shortcut(Command::CloseTab, keys),
            "close-tab",
            Box::new(|shell, cx| shell.close_chosen(cx)),
        );
        // ⇧⌘C does it too, as Arc's Copy URLs does (owner, 2026-10-29).
        let copy: (MenuEntry, &'static str, MenuAction) = (
            MenuEntry::new("Copy URLs", true).shortcut(Command::CopyUrl, keys),
            "copy-url",
            Box::new(|shell, cx| shell.copy_chosen(cx)),
        );
        let line = || -> (MenuEntry, &'static str, MenuAction) {
            (MenuEntry::line(), "", Box::new(|_, _| {}))
        };
        // A private window's tabs are Today's alone, and stay in it.
        if self.private {
            return vec![copy, line(), close];
        }
        let pin = if self.chosen_pin() { "Pin Tabs" } else { "Unpin Tabs" };
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = vec![
            (
                MenuEntry::new(pin, tabs).shortcut(Command::TogglePin, keys),
                "toggle-pin",
                Box::new(|shell, cx| shell.pin_chosen(cx)),
            ),
        ];
        self.push_add_to_folder(self.chosen_in_order(), &mut items);
        let others = self.spaces_for_chosen();
        let move_to = items.len();
        items.push((MenuEntry::new("Move to Space", !others.is_empty()), "", Box::new(|_, _| {})));
        for (space, name, _) in others {
            items.push((
                MenuEntry::named(name, true).swatch(self.space_swatch(space)).under(move_to),
                "move-to-space",
                Box::new(move |shell, cx| shell.move_chosen_to_space(space, cx)),
            ));
        }
        items.extend([line(), copy, line(), close]);
        items
    }

    /// A folder's menu, as Arc's.
    fn folder_menu(&self, folder: u64) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let links = !self.folder_links(folder).is_empty();
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = vec![
            (
                MenuEntry::new("Rename Folder", true),
                "rename-folder",
                Box::new(move |shell, cx| shell.rename_folder(folder, cx)),
            ),
            (
                MenuEntry::new("Change Icon…", true),
                "change-folder-icon",
                Box::new(move |shell, cx| shell.change_folder_icon(folder, cx)),
            ),
        ];
        if has_icon(&self.folder_icon(folder)) {
            items.push((
                MenuEntry::new("Remove Icon", true),
                "remove-folder-icon",
                Box::new(move |shell, cx| {
                    shell.change(Prop::FolderIcon(folder), Value::Text(String::new()), cx)
                }),
            ));
        }
        items.push((
            MenuEntry::new("Duplicate", true),
            "duplicate-folder",
            Box::new(move |shell, cx| shell.duplicate_folder(folder, cx)),
        ));
        // Each other Space, as a pinned tab's Move to Space lists them (owner, #1353).
        let others = self.spaces_to_move_to(false);
        let move_to = items.len();
        items.push((MenuEntry::new("Move to Space", !others.is_empty()), "", Box::new(|_, _| {})));
        for (space, name, _) in others {
            items.push((
                MenuEntry::named(name, true).swatch(self.space_swatch(space)).under(move_to),
                "move-to-space",
                Box::new(move |shell, cx| shell.move_to_space(folder, space, false, cx)),
            ));
        }
        let rest: [(MenuEntry, &'static str, MenuAction); 6] = [
            (MenuEntry::line(), "", Box::new(|_, _| {})),
            (
                MenuEntry::new("Copy All Links", links),
                "copy-all-links",
                Box::new(move |shell, cx| shell.copy_folder_links(folder, false, cx)),
            ),
            (
                MenuEntry::new("Copy All Links as Markdown", links).alternate(),
                "copy-all-links-markdown",
                Box::new(move |shell, cx| shell.copy_folder_links(folder, true, cx)),
            ),
            (
                MenuEntry::new("Paste URL as New Tab", true),
                "paste-url-as-tab",
                Box::new(move |shell, cx| shell.paste_url_into(folder, cx)),
            ),
            (MenuEntry::line(), "", Box::new(|_, _| {})),
            (
                MenuEntry::new("Delete Folder", true).destructive(),
                "delete-folder",
                Box::new(move |shell, cx| shell.delete_folder_now(folder, cx)),
            ),
        ];
        items.extend(rest);
        items
    }

    /// A folder's Duplicate, as Arc's: a copy under it of what it holds, the tabs unloaded until
    /// shown. As a tab's Duplicate, it's no step to undo; the copy is deleted as any folder is.
    fn duplicate_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        self.browser.duplicate_folder(folder, cx);
    }

    /// The tabs in `folder` and the folders in it, in the sidebar's order.
    pub(super) fn folder_tabs(&self, folder: u64) -> Vec<u64> {
        let Some(space) = self.spaces.iter().find(|space| space.is_folder(folder)) else {
            return Vec::new();
        };
        let within = |row: u64| {
            let mut parent = space.folder_of(row);
            while parent != 0 && parent != folder {
                parent = space.folder_of(parent);
            }
            parent == folder
        };
        let tabs = space.pinned.iter().filter(|&&row| !space.is_folder(row) && within(row));
        tabs.copied().collect()
    }

    /// The tabs in `folder` and the folders in it, in the sidebar's order, with their links,
    /// without their tracking parameters. Those with none are left out.
    fn folder_links(&self, folder: u64) -> Vec<(u64, String)> {
        let tabs = self.folder_tabs(folder).into_iter();
        let links = tabs.map(|tab| (tab, clean_link(&self.url(tab))));
        links.filter(|(_, link)| !link.is_empty()).collect()
    }

    /// A folder's Copy All Links, as Arc's: each link on a line, or each a Markdown link, which
    /// Arc's menu offers with ⌥ held.
    fn copy_folder_links(&mut self, folder: u64, markdown: bool, cx: &mut Context<Self>) {
        let lines: Vec<String> = (self.folder_links(folder).into_iter())
            .map(|(tab, link)| match markdown {
                false => link,
                true => format!("[{}]({link})", markdown_text(&self.title(tab))),
            })
            .collect();
        let toast = if markdown { "Copied Links as Markdown" } else { "Copied All Links" };
        self.copy(&lines.join("\n"), toast, cx);
    }

    /// A folder's Paste URL as New Tab: the clipboard's web link as a tab at the folder's top,
    /// opened and shown, as ⌥⌘V opens one at the top of Today.
    fn paste_url_into(&mut self, folder: u64, cx: &mut Context<Self>) {
        let Some(urls) = self.pasted_urls(cx) else {
            return;
        };
        self.browser.set_folder_open(folder, true, cx);
        self.browser.open_at(&urls, true, folder, 0, cx);
    }

    /// A pinned tab's menu (ours: Arc's isn't recorded).
    fn pinned_tab_menu(
        &self,
        keys: &UserKeys,
        tab: u64,
    ) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let page = self.page(tab);
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = vec![
            (
                MenuEntry::new("Unpin Tab", true).shortcut(Command::TogglePin, keys),
                "toggle-pin",
                Box::new(move |shell, cx| shell.pin_tab(tab, false, cx)),
            ),
            (
                MenuEntry::new("Add to Favorites", self.has_room_for(tab)),
                "add-to-favorites",
                Box::new(move |shell, cx| shell.favorite_tab(tab, usize::MAX, cx)),
            ),
            (
                MenuEntry::new("Back to Pinned URL", page.away_from_pinned_url)
                    .shortcut(Command::BackToPinnedUrl, keys),
                "back-to-pinned-url",
                Box::new(move |shell, cx| shell.browser.back_to_pinned_url(tab, cx)),
            ),
        ];
        // Arc's Pinned URL › Replace Pinned URL with Current and Edit….
        let pinned_url = items.len();
        items.push((MenuEntry::new("Pinned URL", true), "", Box::new(|_, _| {})));
        let (entry, name, action) =
            replace_pinned_url(tab, "Replace Pinned URL with Current", page.away_from_pinned_url);
        items.push((entry.under(pinned_url), name, action));
        items.push((
            MenuEntry::new("Edit…", true).under(pinned_url),
            "edit-pinned-url",
            Box::new(move |shell, cx| shell.edit_pinned_url(tab, cx)),
        ));
        items.push((
            MenuEntry::new("Rename Tab", true),
            "rename-tab",
            Box::new(move |shell, cx| shell.rename_tab(tab, cx)),
        ));
        items.push((
            MenuEntry::new("Change Icon…", true),
            "change-icon",
            Box::new(move |shell, cx| shell.change_icon(tab, cx)),
        ));
        if !page.icon.is_empty() {
            items.push(remove_icon(tab));
        }
        items.push((
            MenuEntry::new("Duplicate Tab", true),
            "duplicate-tab",
            Box::new(move |shell, cx| shell.duplicate_row(tab, cx)),
        ));
        if self.panes(tab).len() > 1 {
            items.push(separate(tab));
        }
        self.push_add_to_folder(vec![tab], &mut items);
        // Each other Space, in the switcher's order, then the agents' it can be given to.
        let others = self.spaces_to_move_to(false);
        let agents = self.agents_to_give(tab);
        let move_to = items.len();
        let any = !others.is_empty() || !agents.is_empty();
        items.push((MenuEntry::new("Move to Space", any), "", Box::new(|_, _| {})));
        for (space, name, _) in others {
            items.push((
                MenuEntry::named(name, true).swatch(self.space_swatch(space)).under(move_to),
                "move-to-space",
                Box::new(move |shell, cx| shell.move_to_space(tab, space, false, cx)),
            ));
        }
        items.extend(self.give_items(tab, agents, move_to));
        items.push((MenuEntry::line(), "", Box::new(|_, _| {})));
        // One closed already has nothing to close.
        items.push((
            MenuEntry::new("Close Tab", !page.unloaded).shortcut(Command::CloseTab, keys),
            "close-tab",
            Box::new(move |shell, cx| shell.close_tab(tab, false, cx)),
        ));
        items
    }

    /// Share as the system can: a submenu of its services where it lists them, as the Mac does;
    /// else one Share… that opens its share sheet, as on Windows, which lists none; else nothing,
    /// as on Linux, which has neither, so the menu never holds a Share that leads nowhere. Dimmed
    /// for a page with no link.
    fn push_share(
        &self,
        items: &mut Vec<(MenuEntry, &'static str, MenuAction)>,
        tab: u64,
        link: &str,
        at: Point<Pixels>,
    ) {
        let services = match link.is_empty() {
            true => Vec::new(),
            false => self.browser.share_services(link),
        };
        if services.is_empty() {
            if self.browser.has_share_sheet() {
                let (link, title) = (link.to_string(), self.title(tab));
                items.push((
                    MenuEntry::new("Share…", !link.is_empty()),
                    "share",
                    Box::new(move |shell, cx| {
                        shell.browser.show_share_sheet(tab, &link, &title, at, cx)
                    }),
                ));
            }
            return;
        }
        let share = items.len();
        items.push((MenuEntry::new("Share", true), "", Box::new(|_, _| {})));
        for (at, service) in services.into_iter().enumerate() {
            let (link, title) = (link.to_string(), self.title(tab));
            items.push((
                MenuEntry::named(service, true).under(share),
                "share",
                Box::new(move |shell, cx| shell.browser.share(&link, &title, at, cx)),
            ));
        }
    }

    /// A Today tab's menu, as Arc's (docs/ux-spec.md › Tab list), with Remove Icon once it has
    /// one and Separate from Split View in a split, ours. Arc's Customize Page… is Boosts, which
    /// the browser hasn't. Each item's agent path is the core call it makes, but Copy Link's and
    /// Share's: the clipboard and the system's share services are the user's, outside the
    /// browser, so agents get neither.
    fn today_tab_menu(
        &self,
        keys: &UserKeys,
        tab: u64,
        position: Point<Pixels>,
    ) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let page = self.page(tab);
        let panes = self.panes(tab);
        let last = panes[panes.len() - 1];
        let below = self.today().iter().skip_while(|&&id| id != last).count() > 1;
        let link = clean_link(&page.url);
        let markdown = MenuEntry::new("Copy Link as Markdown", !link.is_empty())
            .shortcut(Command::CopyMarkdownLink, keys)
            .alternate();
        let mut items: Vec<(MenuEntry, &'static str, MenuAction)> = vec![
            (
                MenuEntry::new("Copy Link", !link.is_empty()).shortcut(Command::CopyUrl, keys),
                "copy-link",
                Box::new(move |shell, cx| shell.copy_link(tab, false, cx)),
            ),
            // Holding ⌥ makes Copy Link copy it as Markdown, as ⌥⇧⌘C does (owner, 2026-10-29).
            (
                markdown,
                "copy-markdown-link",
                Box::new(move |shell, cx| shell.copy_link(tab, true, cx)),
            ),
        ];
        self.push_share(&mut items, tab, &link, position);
        items.push((MenuEntry::line(), "", Box::new(|_, _| {})));
        items.push((
            MenuEntry::new("Change Icon…", true),
            "change-icon",
            Box::new(move |shell, cx| shell.change_icon(tab, cx)),
        ));
        if has_icon(&page.icon) {
            items.push(remove_icon(tab));
        }
        items.push((
            MenuEntry::new("Rename…", true),
            "rename-tab",
            Box::new(move |shell, cx| shell.rename_tab(tab, cx)),
        ));
        // Dimmed while the tab is silent, as Arc's; a muted one unmutes. A split's offers it under
        // each of its tabs' names, as Arc's does.
        for &pane in &panes {
            let page = self.page(pane);
            let mute = if page.muted { "Unmute" } else { "Mute" };
            let mut entry = MenuEntry::new(mute, page.audible || page.muted);
            if panes.len() > 1 {
                let name = items.len();
                let title = self.tabs.get(&pane).map(|tab| tab.title.clone()).unwrap_or_default();
                items.push((MenuEntry::named(title, true), "", Box::new(|_, _| {})));
                entry = entry.under(name);
            }
            items.push((entry, "toggle-mute", Box::new(move |shell, cx| shell.toggle_mute(pane, cx))));
        }
        // While a page shares another tab, as Chrome's infobar offers it on the rest.
        if page.share_instead {
            items.push((
                MenuEntry::new("Share This Tab Instead", true),
                "share-instead",
                Box::new(move |shell, cx| shell.browser.share_instead(tab, cx)),
            ));
        }
        items.push((MenuEntry::line(), "", Box::new(|_, _| {})));
        items.push((
            MenuEntry::new("Add Split View", panes.len() < MAX_PANES),
            "add-split",
            Box::new(move |shell, cx| shell.add_split_to(tab, cx)),
        ));
        if panes.len() > 1 {
            items.push(separate(tab));
        }
        items.push((
            MenuEntry::new("Duplicate", true),
            "duplicate-tab",
            Box::new(move |shell, cx| shell.duplicate_row(tab, cx)),
        ));
        // A private window's tabs stay in it. The others: every one of the user's Spaces, the
        // tab's checked, then a new one.
        if !self.private {
            self.push_add_to_folder(vec![tab], &mut items);
            let move_to = items.len();
            items.push((MenuEntry::new("Move to", true), "", Box::new(|_, _| {})));
            for (space, name, shown) in self.spaces_to_move_to(true) {
                let swatch = self.space_swatch(space);
                let folders = self.folders_to_move_to(tab, space);
                let entry = MenuEntry::named(name, true).checked(shown).swatch(swatch);
                let to_space: MenuAction = Box::new(move |shell, cx| {
                    if !shown {
                        shell.move_to_space(tab, space, false, cx)
                    }
                });
                if folders.is_empty() {
                    items.push((entry.under(move_to), "move-to-space", to_space));
                    continue;
                }
                // A Space with folders is a submenu of its Today tabs and its folders (Arc).
                let parent = items.len();
                items.push((entry.under(move_to), "", Box::new(|_, _| {})));
                let today = MenuEntry::new("Today", true).checked(shown).under(parent);
                items.push((today, "move-to-space", to_space));
                items.push((MenuEntry::line().under(parent), "", Box::new(|_, _| {})));
                for (folder, path) in folders {
                    items.push((
                        MenuEntry::named(path, true).under(parent),
                        "move-to-folder",
                        Box::new(move |shell, cx| shell.move_into_folder(tab, space, folder, cx)),
                    ));
                }
            }
            items.extend(self.give_items(tab, self.agents_to_give(tab), move_to));
            items.push((MenuEntry::line().under(move_to), "", Box::new(|_, _| {})));
            items.push((
                MenuEntry::new("New Space…", true).under(move_to),
                "new-space",
                Box::new(move |shell, cx| shell.new_space_with(Some(tab), cx)),
            ));
        }
        items.push((MenuEntry::line(), "", Box::new(|_, _| {})));
        // Closing a Today tab archives it.
        items.push((
            MenuEntry::new("Archive Tab", true),
            "close-tab",
            Box::new(move |shell, cx| shell.close_tab(tab, false, cx)),
        ));
        items.push((
            MenuEntry::new("Archive Tabs Below", below),
            "close-tabs-below",
            Box::new(move |shell, cx| shell.close_tabs_below(tab, cx)),
        ));
        items
    }

    /// The user's Spaces a tab can move to, by id and name in the switcher's order, and whether
    /// each is the one shown, which `with_shown` lists too. None moves into an agent's.
    fn spaces_to_move_to(&self, with_shown: bool) -> Vec<(u64, SharedString, bool)> {
        (self.spaces.iter())
            .filter(|space| space.agent.is_empty() && (with_shown || space.id != self.space))
            .map(|space| (space.id, space.name.clone(), space.id == self.space))
            .collect()
    }

    /// The other Spaces the chosen rows can move to: the profile's, where every row keeps its
    /// place to undo.
    pub(super) fn spaces_for_chosen(&self) -> Vec<(u64, SharedString, bool)> {
        let profile = self.shown_space().map(|space| space.profile);
        (self.spaces_to_move_to(false).into_iter())
            .filter(|&(space, ..)| {
                self.spaces.iter().any(|each| each.id == space && Some(each.profile) == profile)
            })
            .collect()
    }

    /// The folders of `space` that `tab` can move into, in the sidebar's order, each named by its
    /// path, "Docs / Old". None in another profile's Space, where the tab goes as a new tab.
    fn folders_to_move_to(&self, tab: u64, space: u64) -> Vec<(u64, String)> {
        let profile_of = |space: Option<&super::Space>| space.map(|space| space.profile);
        let from = self.spaces.iter().find(|each| each.today.contains(&tab));
        let Some(to) = self.spaces.iter().find(|each| each.id == space) else {
            return Vec::new();
        };
        if profile_of(from) != Some(to.profile) {
            return Vec::new();
        }
        folder_paths(to)
    }

    /// Add to Folder ›, as Chrome's Add Tab to Group (#1088): New Folder, then the shown Space's
    /// folders, nested ones after their parent, each named by its path. Each moves `rows` in.
    fn push_add_to_folder(
        &self,
        rows: Vec<u64>,
        items: &mut Vec<(MenuEntry, &'static str, MenuAction)>,
    ) {
        let parent = items.len();
        items.push((MenuEntry::new("Add to Folder", true), "", Box::new(|_, _| {})));
        let new = rows.clone();
        items.push((
            MenuEntry::new("New Folder", true).under(parent),
            "new-folder",
            Box::new(move |shell, cx| shell.add_to_new_folder(new, cx)),
        ));
        let folders = self.shown_space().map(folder_paths).unwrap_or_default();
        if !folders.is_empty() {
            items.push((MenuEntry::line().under(parent), "", Box::new(|_, _| {})));
        }
        for (folder, path) in folders {
            let rows = rows.clone();
            items.push((
                MenuEntry::named(path, true).under(parent),
                "add-to-folder",
                Box::new(move |shell, cx| shell.add_to_folder(rows, folder, cx)),
            ));
        }
    }

    /// Move to › a folder: the tab goes to the folder's top, from its Space's Today tabs or
    /// another Space's, as one step to undo.
    fn move_into_folder(&mut self, tab: u64, space: u64, folder: u64, cx: &mut Context<Self>) {
        if self.spaces.iter().any(|each| each.id == space && each.today.contains(&tab)) {
            return self.move_tab_to(tab, true, folder, 0, cx);
        }
        self.send_to_space(tab, space, false, cx);
        self.browser.move_tab(tab, true, folder, 0, cx);
    }

    /// The agents' Spaces the user can give `tab` to, by id and name: a tab of the user's own,
    /// to an agent whose Space is in its profile, as the page goes as it is (bc_tab_give_to_agent).
    fn agents_to_give(&self, tab: u64) -> Vec<(u64, SharedString)> {
        let Some(from) = (self.spaces.iter())
            .find(|space| space.pinned.contains(&tab) || space.today.contains(&tab))
            .filter(|space| space.agent.is_empty())
        else {
            return Vec::new();
        };
        (self.spaces.iter())
            .filter(|space| !space.agent.is_empty() && space.profile == from.profile)
            .map(|space| (space.id, space.name.clone()))
            .collect()
    }

    /// Move to's items that give `tab` to an agent, under the item at `move_to`.
    fn give_items(
        &self,
        tab: u64,
        agents: Vec<(u64, SharedString)>,
        move_to: usize,
    ) -> Vec<(MenuEntry, &'static str, MenuAction)> {
        let item = |(space, name)| -> (MenuEntry, &'static str, MenuAction) {
            (
                MenuEntry::named(name, true).swatch(self.space_swatch(space)).under(move_to),
                "give-to-agent",
                Box::new(move |shell: &mut Shell, cx: &mut Context<Shell>| {
                    shell.browser.give_to_agent(tab, space, cx)
                }),
            )
        };
        agents.into_iter().map(item).collect()
    }

    /// `space`'s swatch: its theme's first two colours, or its one colour going lighter down as
    /// the window's gradient does, with the untinted palette's accent for a Space without one.
    pub(super) fn space_swatch(&self, space: u64) -> Swatch {
        let theme = self.theme_of(space).map(|theme| theme.shown(self.tone_of(space) == Tone::Dark));
        if let Some(theme) = theme {
            let colors = theme.colors();
            if let [from, to, ..] = colors[..] {
                return Swatch { from, to };
            }
        }
        let entry = self.spaces.iter().find(|each| each.id == space);
        let color = theme.map(|theme| theme.main_color());
        let untinted = || crate::theme::packed(crate::palette::Palette::of(None, self.tone_of(space)).accent);
        let color = color.or(entry.and_then(|each| each.color)).unwrap_or_else(untinted);
        let hsla = Hsla::from(rgb(color));
        let lighter = crate::theme::packed(Rgba::from(Hsla { l: (hsla.l + 0.25).min(1.), ..hsla }));
        Swatch { from: color, to: lighter }
    }

    /// Copy Link: copies `tab`'s URL without its tracking parameters, as ⇧⌘C does the tab
    /// shown's, or as a Markdown link, as ⌥⇧⌘C does.
    fn copy_link(&mut self, tab: u64, markdown: bool, cx: &mut Context<Self>) {
        if let Some((text, toast)) = self.link_to_copy(tab, markdown) {
            self.copy(&text, toast, cx);
        }
    }

    /// The + button's menu, by Arc's menus' names for what it makes (ours: Arc's + menu isn't
    /// recorded).
    pub(super) fn plus_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let items = [
            ("New Space…", Command::NewSpace),
            ("New Folder…", Command::NewFolder),
            ("New Split View", Command::AddSplit),
            ("New Note", Command::NewNote),
        ];
        let split = self.selected != 0 && self.panes(self.selected).len() < MAX_PANES;
        let enabled = |command| command != Command::AddSplit || split;
        let mut entries: Vec<MenuEntry> =
            items.iter().map(|&(title, command)| MenuEntry::new(title, enabled(command))).collect();
        // From the desktop, after a line: the Mac's panel picks files and folders together, the
        // others' one or the other (ours, as Arc adds both from its +).
        let disk: &[(&str, bool, bool)] = match files::PANEL_PICKS_BOTH {
            true => &[("Add File or Folder…", true, true)],
            false => &[("Add Files…", true, false), ("Add Folder…", false, true)],
        };
        if !self.private {
            entries.push(MenuEntry::line());
            entries.extend(disk.iter().map(|&(title, ..)| MenuEntry::new(title, true)));
        }
        self.context_menu(
            entries,
            position,
            Box::new(move |chosen, shell, cx| {
                let Some(at) = chosen else {
                    return;
                };
                if let Some(&(_, command)) = items.get(at) {
                    return shell.run(command, "footer", cx);
                }
                // After the line.
                if let Some(&(title, files, folders)) = disk.get(at - items.len() - 1) {
                    // check.sh waits for this line.
                    log!("browser: footer {title}");
                    shell.add_from_disk(files, folders, cx);
                }
            }),
            cx,
        );
    }
}

/// The top of a Space's menu, above its profiles: Rename Space, Change Icon…, Remove Icon once it
/// has one, Edit Theme Color…, and Remove Theme Color once it has a colour.
/// `space`'s folders, in the sidebar's order, each named by its path, "Docs / Old".
fn folder_paths(space: &super::Space) -> Vec<(u64, String)> {
    let name = |folder: u64| space.folders.folders.get(&folder).map(|(name, _)| name.clone());
    let path = |folder: u64| {
        let mut path = vec![name(folder).unwrap_or_default()];
        path.extend(space.holders(folder).map(|parent| name(parent).unwrap_or_default()));
        path.reverse();
        path.join(" / ")
    };
    let folders = space.pinned.iter().filter(|&&row| space.is_folder(row));
    folders.map(|&folder| (folder, path(folder))).collect()
}

fn space_menu_head(space: u64, has_icon: bool, has_theme: bool) -> Vec<(MenuEntry, MenuAction)> {
    let mut items: Vec<(MenuEntry, MenuAction)> = vec![
        (
            MenuEntry::new("Rename Space", true),
            Box::new(move |shell, cx| shell.rename_space(space, cx)),
        ),
        (
            MenuEntry::new("Change Icon…", true),
            Box::new(move |shell, cx| shell.change_space_icon(space, cx)),
        ),
    ];
    if has_icon {
        items.push((
            MenuEntry::new("Remove Icon", true),
            Box::new(move |shell, cx| {
                shell.change(Prop::SpaceIcon(space), Value::Text(String::new()), cx)
            }),
        ));
    }
    // After the icon and the name, as in Arc's.
    items.push((
        MenuEntry::new("Edit Theme Color…", true),
        Box::new(move |shell, cx| shell.edit_space_theme(space, cx)),
    ));
    // The way back to no theme, and ilo green, which the picker has none of (ours: Arc's isn't
    // recorded).
    if has_theme {
        items.push((
            MenuEntry::new("Remove Theme Color", true),
            Box::new(move |shell, cx| shell.remove_space_theme(space, cx)),
        ));
    }
    items
}

/// A Space's item in the menu bar: its name, after its emoji if it has one. GPUI's menu items
/// have no image, so a glyph isn't shown.
pub(super) fn space_menu_label(icon: &str, name: &str) -> SharedString {
    match SpaceIcon::parse(icon) {
        SpaceIcon::Emoji(emoji) => format!("{emoji} {name}").into(),
        SpaceIcon::Dot | SpaceIcon::Glyph(_) => name.to_string().into(),
    }
}

/// The menu item that takes a tab out of its split view, to show alone.
fn separate(tab: u64) -> (MenuEntry, &'static str, MenuAction) {
    (
        MenuEntry::new("Separate from Split View", true),
        "separate-pane",
        Box::new(move |shell, cx| shell.separate_tab(tab, cx)),
    )
}

/// The menu item that makes the page a pinned tab or a favorite is at the one it goes back to,
/// dimmed while it's there.
fn replace_pinned_url(
    tab: u64,
    title: &'static str,
    away: bool,
) -> (MenuEntry, &'static str, MenuAction) {
    (
        MenuEntry::new(title, away),
        "replace-pinned-url",
        Box::new(move |shell, cx| {
            let url = shell.page(tab).url.to_string();
            shell.change(Prop::PinnedUrl(tab), Value::Text(url), cx)
        }),
    )
}

/// The menu item that gives a tab back its favicon.
pub(super) fn remove_icon(tab: u64) -> (MenuEntry, &'static str, MenuAction) {
    (
        MenuEntry::new("Remove Icon", true),
        "remove-icon",
        Box::new(move |shell, cx| shell.change(Prop::TabIcon(tab), Value::Text(String::new()), cx)),
    )
}

#[cfg(test)]
mod empty_space_tests;

#[cfg(test)]
mod tests {
    use gpui::{AppContext, Entity, TestAppContext, point, px};

    use super::{MenuEntry, sentence_case, space_menu_head, space_menu_label};
    use crate::shell::{
        Command, Folders, Page, Shell, drag::detents::shell_with, recorder::Recorder,
    };

    /// Off the Mac, ilo's items are in sentence case, keeping its nouns Space and Library and
    /// acronyms capitalized.
    #[test]
    fn items_off_the_mac_are_in_sentence_case() {
        let cases = [
            ("Add Split View", "Add split view"),
            ("Archive Tabs Below", "Archive tabs below"),
            ("Copy Link", "Copy link"),
            ("Change Icon…", "Change icon…"),
            ("New Space…", "New Space…"),
            ("Move to Space", "Move to Space"),
            ("Open Library…", "Open Library…"),
            ("Back to Pinned URL", "Back to pinned URL"),
            ("In Each Profile's Spaces", "In each profile's Spaces"),
            ("Duplicate", "Duplicate"),
            ("Show in Folder", "Show in folder"),
            ("Copy URLs", "Copy URLs"),
            ("Close Tabs", "Close tabs"),
        ];
        for (title, sentence) in cases {
            assert_eq!(sentence_case(title), sentence);
        }
    }

    fn titles(has_icon: bool, has_theme: bool) -> Vec<String> {
        let items = space_menu_head(1, has_icon, has_theme);
        items.into_iter().map(|(entry, _)| entry.title.to_string()).collect()
    }

    /// A Space's menu offers Change Icon… always and Remove Icon only once it has an icon, after
    /// Rename, as a tab's menu does.
    #[test]
    fn remove_icon_is_offered_once_the_space_has_one() {
        let theme = "Edit Theme Color…";
        assert_eq!(titles(false, false), ["Rename Space", "Change Icon…", theme]);
        assert_eq!(titles(true, false), ["Rename Space", "Change Icon…", "Remove Icon", theme]);
    }

    /// Remove Theme Color follows Edit Theme Color… only once the Space has a colour, as Remove
    /// Icon follows Change Icon….
    #[test]
    fn remove_theme_color_is_offered_once_the_space_has_one() {
        let titles = titles(false, true);
        assert_eq!(titles[2..], ["Edit Theme Color…", "Remove Theme Color"]);
    }

    /// The menu bar lists a Space by its name, after its emoji if it has one.
    #[test]
    fn the_menu_bar_names_a_space_after_its_emoji() {
        assert_eq!(space_menu_label("", "Work"), "Work");
        assert_eq!(space_menu_label("🙂", "Work"), "🙂 Work");
        assert_eq!(space_menu_label("glyph:star", "Work"), "Work");
    }

    /// A menu as the host logs it: lines left out, a submenu's items after their parent's title,
    /// and the dimmed and checked ones marked.
    fn logged(entries: &[MenuEntry]) -> Vec<String> {
        let parents: Vec<usize> = entries.iter().filter_map(|entry| entry.parent).collect();
        let mark = |entry: &MenuEntry, title: String| match (entry.enabled, entry.checked) {
            (false, _) => format!("{title} (disabled)"),
            (true, true) => format!("{title} (checked)"),
            (true, false) => title,
        };
        (entries.iter().enumerate())
            .filter(|(at, entry)| {
                !entry.title.is_empty() && !parents.contains(at) && !entry.alternate
            })
            .map(|(_, entry)| {
                let title = match entry.parent {
                    Some(parent) => format!("{} › {}", entries[parent].title, entry.title),
                    None => entry.title.to_string(),
                };
                mark(entry, title)
            })
            .collect()
    }

    /// What a right-click on `tab`'s row shows.
    fn tab_menu(
        shell: &Entity<Shell>,
        host: &Recorder,
        cx: &mut TestAppContext,
        tab: u64,
    ) -> Vec<String> {
        shell.update(cx, |shell, cx| shell.tab_menu(tab, point(px(100.), px(200.)), cx));
        logged(&host.entries.borrow())
    }

    fn page(url: &str, audible: bool, muted: bool) -> Page {
        Page { url: url.into(), audible, muted, ..Page::default() }
    }

    /// A Today tab's menu is Arc's: its link, its look, its place, then archiving, with Move to
    /// listing the Space it's in checked and New Space… under the rest.
    #[gpui::test]
    fn a_today_tabs_menu_is_arcs(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        *host.share_services.borrow_mut() = vec!["Mail".into(), "Messages".into()];
        let link = page("https://example.com/", false, false);
        shell.update(cx, |shell, cx| shell.set_page(10, link, cx));
        assert_eq!(
            tab_menu(&shell, &host, cx, 10),
            [
                "Copy Link",
                "Share › Mail",
                "Share › Messages",
                "Change Icon…",
                "Rename…",
                "Mute (disabled)",
                "Add Split View",
                "Duplicate",
                "Add to Folder › New Folder",
                "Move to › Work (checked)",
                "Move to › New Space…",
                "Archive Tab",
                "Archive Tabs Below",
            ]
        );
        // Copy Link alone shows its key, as in Arc's, and so does its ⌥ alternate.
        let entries = host.entries.borrow().clone();
        let keyed: Vec<&str> = (entries.iter())
            .filter(|entry| entry.shortcut.is_some())
            .map(|entry| entry.title.as_ref())
            .collect();
        assert_eq!(keyed, ["Copy Link", "Copy Link as Markdown"]);
        assert!(entries[1].alternate && !entries[0].alternate);
        host.take();
        cx.update(|cx| host.choose("Copy Link as Markdown", cx));
        let title = shell.read_with(cx, |shell, _| shell.title(10));
        let markdown = format!("copy [{title}](https://example.com/)");
        assert!(host.take().contains(&markdown), "{markdown}");
    }

    /// Move to › lists another Space of the profile with folders as a submenu of its Today and its
    /// folders, and the folder moves the tab into it.
    #[gpui::test]
    fn move_to_lists_another_spaces_folders(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10], Folders::default());
        shell.update(cx, |shell, cx| {
            let work = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
            let reading = (2, "Reading".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![work, reading], cx);
            let mut folders = Folders::default();
            folders.folders.insert(20, ("Shelf".into(), false));
            shell.set_tabs(2, vec![20], vec![21], Vec::new(), folders, cx);
        });
        tab_menu(&shell, &host, cx, 10);
        // As check.sh reads it, each submenu's items after its path.
        let shown = crate::menu::described(&host.entries.borrow());
        let moves = "Move to › Work (checked), Move to › Reading › Today, \
                     Move to › Reading › Shelf, Move to › New Space…";
        assert!(shown.contains(moves), "{shown}");
        host.take();
        cx.update(|cx| host.choose("Shelf", cx));
        let (calls, edits) = (host.take(), host.take_edits());
        assert!(edits.iter().any(|edit| edit.starts_with("move 10 ")), "{calls:?} {edits:?}");
    }

    /// A split's menu offers Mute under each of its tabs' names, and its Duplicate copies the whole
    /// split, joined again in its order and orientation, as Arc's does.
    #[gpui::test]
    fn a_splits_menu_mutes_each_tab_and_duplicates_the_split(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11, 12], Folders::default());
        shell.update(cx, |shell, cx| {
            let split = crate::shell::Split { tabs: vec![10, 11], stacked: true, ..Default::default() };
            shell.set_tabs(1, Vec::new(), vec![10, 11, 12], vec![split], Folders::default(), cx);
            shell.set_title(10, "Ten".into(), cx);
            shell.set_title(11, "Eleven".into(), cx);
            shell.set_page(11, page("https://example.com/", true, false), cx);
        });
        let menu = tab_menu(&shell, &host, cx, 11);
        let mutes: Vec<&str> = (menu.iter().map(String::as_str))
            .filter(|title| title.contains("Mute"))
            .collect();
        assert_eq!(mutes, ["Ten › Mute (disabled)", "Eleven › Mute"]);
        host.take();
        host.take_edits();
        shell.update(cx, |shell, cx| shell.duplicate_row(11, cx));
        let calls = host.take();
        assert_eq!(calls[..2], ["duplicate 10", "duplicate 11"]);
        let edits = host.take_edits();
        assert_eq!(edits.len(), 1, "{edits:?}");
        let copies: Vec<&str> = edits[0].split(' ').skip(1).collect();
        assert_eq!(copies[2], "false", "the second copy joins the first on its right");
        assert!(calls.contains(&format!("split stacked {} true", copies[1])), "{calls:?}");
    }

    /// The last tab has none below to archive, and a tab with no page has no link to copy or share.
    /// Move to marks each Space with its swatch, and New Space… with none (#320).
    #[gpui::test]
    fn move_to_shows_each_spaces_swatch(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        tab_menu(&shell, &host, cx, 10);
        let entries = host.entries.borrow();
        let move_to = entries.iter().position(|entry| entry.title == "Move to").unwrap();
        let swatches: Vec<(&str, bool)> = (entries.iter())
            .filter(|entry| entry.parent == Some(move_to) && !entry.title.is_empty())
            .map(|entry| (entry.title.as_ref(), entry.swatch.is_some()))
            .collect();
        assert_eq!(swatches, [("Work", true), ("New Space…", false)]);
        let work = shell.read_with(cx, |shell, _| shell.space_swatch(shell.space));
        assert_eq!(entries[move_to + 1].swatch, Some(work));
    }

    #[gpui::test]
    fn the_last_tabs_menu_dims_what_it_cant_do(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10, 11], Folders::default());
        host.share_sheet.set(true);
        let menu = tab_menu(&shell, &host, cx, 11);
        assert_eq!(menu[..2], ["Copy Link (disabled)", "Share… (disabled)"]);
        assert_eq!(menu.last().unwrap(), "Archive Tabs Below (disabled)");
    }

    /// Where the system lists its share services, as the Mac's does, Share is a submenu of them,
    /// and each shares the page with its service.
    #[gpui::test]
    fn share_lists_the_systems_services(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        *host.share_services.borrow_mut() = vec!["Mail".into(), "Messages".into()];
        host.share_sheet.set(true);
        let link = page("https://example.com/", false, false);
        shell.update(cx, |shell, cx| shell.set_page(10, link, cx));
        let menu = tab_menu(&shell, &host, cx, 10);
        assert_eq!(menu[1..3], ["Share › Mail", "Share › Messages"]);
        host.take();
        cx.update(|cx| host.choose("Messages", cx));
        assert_eq!(host.take(), ["share https://example.com/ with 1"]);
    }

    /// Where the system lists none but has a share sheet, as Windows does, Share… is one item that
    /// opens the sheet for the page, rather than an empty submenu.
    #[gpui::test]
    fn share_opens_the_systems_sheet_when_it_lists_no_services(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        host.share_sheet.set(true);
        let link = page("https://example.com/", false, false);
        shell.update(cx, |shell, cx| shell.set_page(10, link, cx));
        let menu = tab_menu(&shell, &host, cx, 10);
        assert_eq!(menu[..3], ["Copy Link", "Share…", "Change Icon…"]);
        let title = shell.read_with(cx, |shell, _| shell.title(10));
        host.take();
        cx.update(|cx| host.choose("Share…", cx));
        assert_eq!(host.take(), [format!("share sheet 10 https://example.com/ {title}")]);
    }

    /// A folder's menu is Arc's: its name and icon, a copy of it, its links, then its deletion,
    /// with Remove Icon once it has one.
    #[gpui::test]
    fn a_folders_menu_is_arcs(cx: &mut TestAppContext) {
        let folders = Folders {
            folders: [(20, ("Docs".into(), true)), (22, ("Old".into(), true))].into(),
            parents: [(21, 20)].into(),
            icons: [(22, "📚".into())].into(),
            ..Default::default()
        };
        let (shell, host) = shell_with(cx, &[20, 21, 22], &[24], folders);
        let menu = tab_menu(&shell, &host, cx, 20);
        // Its tab has no address here, so Copy All Links is greyed out, and the window has no other
        // Space to move it to.
        let head = [
            "Rename Folder",
            "Change Icon…",
            "Duplicate",
            "Move to Space (disabled)",
            "Copy All Links (disabled)",
        ];
        assert_eq!(menu[..5], head, "{menu:?}");
        assert_eq!(menu.last().unwrap(), "Delete Folder");
        let menu = tab_menu(&shell, &host, cx, 22);
        assert_eq!(menu[..4], ["Rename Folder", "Change Icon…", "Remove Icon", "Duplicate"]);
        host.take();
        host.take_edits();
        cx.update(|cx| host.choose("Remove Icon", cx));
        assert_eq!(host.take_edits(), ["folder icon 22 "]);
        tab_menu(&shell, &host, cx, 20);
        cx.update(|cx| host.choose("Duplicate", cx));
        let edits = host.take_edits();
        assert!(edits.len() == 1 && edits[0].starts_with("duplicate folder 20 "), "{edits:?}");
    }

    /// A folder's Move to Space lists the user's other Spaces, another profile's too, as a pinned
    /// tab's does, never an agent's, and moves the folder whole (owner, #1353).
    #[gpui::test]
    fn a_folder_moves_to_the_users_other_spaces_but_never_an_agents(cx: &mut TestAppContext) {
        let host = std::rc::Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        let folders = Folders {
            folders: [(20, ("Docs".into(), true))].into(),
            parents: [(21, 20)].into(),
            ..Default::default()
        };
        shell.update(cx, |shell, cx| {
            let space = |id, name: &str, profile, agent: &str| {
                (id, name.into(), None, profile, "".into(), agent.into(), "".into())
            };
            let spaces = vec![
                space(1, "Work", 0, ""),
                space(2, "Home", 0, ""),
                space(3, "Other", 1, ""),
                space(9, "Agent", 0, "claude"),
            ];
            shell.set_spaces(spaces, cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, vec![20, 21], vec![24], Vec::new(), folders, cx);
        });
        let menu = tab_menu(&shell, &host, cx, 20);
        let moves: Vec<&String> = menu.iter().filter(|title| title.contains("Space")).collect();
        assert_eq!(moves, ["Move to Space › Home", "Move to Space › Other"], "{menu:?}");
        host.take_edits();
        cx.update(|cx| host.choose("Home", cx));
        assert_eq!(host.take_edits(), ["space folder 20 2"]);
    }

    /// A folder's menu copies the links of every tab in it, nested folders' too, each on a line or
    /// as Markdown, and pastes the clipboard's link as a tab at its top, as Arc's does.
    #[gpui::test]
    fn a_folders_menu_copies_its_links_and_pastes_one_in(cx: &mut TestAppContext) {
        let folders = Folders {
            folders: [(20, ("Docs".into(), true)), (22, ("Old".into(), false))].into(),
            parents: [(21, 20), (22, 20), (23, 22)].into(),
            ..Default::default()
        };
        let (shell, host) = shell_with(cx, &[20, 21, 22, 23], &[24], folders);
        shell.update(cx, |shell, cx| {
            shell.set_page(21, page("https://a.example/?utm_source=x", false, false), cx);
            shell.set_page(23, page("https://b.example/", false, false), cx);
            shell.set_page(24, page("https://c.example/", false, false), cx);
            shell.set_title(21, "A [1]".into(), cx);
        });
        let menu = tab_menu(&shell, &host, cx, 20);
        let at = menu.iter().position(|title| title == "Copy All Links");
        let at = at.unwrap_or_else(|| panic!("{menu:?}"));
        assert_eq!(menu[at..at + 2], ["Copy All Links", "Paste URL as New Tab"]);
        // As Markdown, it's Copy All Links' ⌥ alternate, as in Arc's.
        let entries = host.entries.borrow().clone();
        let markdown = entries.iter().position(|entry| entry.title == "Copy All Links as Markdown");
        let markdown = markdown.unwrap_or_else(|| panic!("{menu:?}"));
        assert!(entries[markdown].alternate && entries[markdown - 1].title == "Copy All Links");
        host.take();
        cx.update(|cx| host.choose("Copy All Links", cx));
        assert!(host.take().contains(&"copy https://a.example/\nhttps://b.example/".to_string()));
        tab_menu(&shell, &host, cx, 20);
        host.take();
        cx.update(|cx| host.choose("Copy All Links as Markdown", cx));
        let title = shell.read_with(cx, |shell, _| shell.title(23));
        let (a, b) = ("[A \\[1\\]](https://a.example/)", "https://b.example/");
        let markdown = format!("copy {a}\n[{title}]({b})");
        assert!(host.take().contains(&markdown), "{markdown}");

        host.copied.borrow_mut().push("https://d.example/".into());
        tab_menu(&shell, &host, cx, 22);
        host.take();
        cx.update(|cx| host.choose("Paste URL as New Tab", cx));
        let calls = host.take();
        assert!(calls.contains(&"folder open 22 true".to_string()), "{calls:?}");
        assert!(calls.contains(&"open https://d.example/ true 22 0".to_string()), "{calls:?}");
    }

    /// Move to lists each Space's folders under it, by their paths, after its Today tabs, and a
    /// folder chosen takes the tab at its top, in its Space or another of the same profile. Another
    /// profile's Space lists none, as the tab goes there as a new tab.
    #[gpui::test]
    fn move_to_reaches_a_folder_in_any_space_of_the_profile(cx: &mut TestAppContext) {
        let folders = Folders {
            folders: [(20, ("Docs".into(), true))].into(),
            parents: Default::default(),
            ..Default::default()
        };
        let (shell, host) = shell_with(cx, &[20], &[10], folders);
        shell.update(cx, |shell, cx| {
            let space = |id, name: &str, profile| {
                (id, name.to_string().into(), None, profile, "".into(), "".into(), "".into())
            };
            let spaces = vec![space(1, "Work", 0), space(2, "Home", 0), space(3, "Other", 7)];
            shell.set_spaces(spaces, cx);
            let home = Folders {
                folders: [(30, ("Old".into(), false)), (31, ("Older".into(), true))].into(),
                parents: [(31, 30)].into(),
                ..Default::default()
            };
            shell.set_tabs(2, vec![30, 31], vec![40], Vec::new(), home, cx);
            let theirs = [(50, ("Theirs".into(), true))].into();
            let other = Folders { folders: theirs, ..Default::default() };
            shell.set_tabs(3, vec![50], Vec::new(), Vec::new(), other, cx);
        });
        let menu = tab_menu(&shell, &host, cx, 10);
        let start = menu.iter().position(|title| title.starts_with("Work › Today"));
        let end = menu.iter().position(|title| title == "Move to › New Space…");
        let (Some(start), Some(end)) = (start, end) else {
            panic!("{menu:?}");
        };
        assert_eq!(
            menu[start..=end],
            [
                "Work › Today (checked)",
                "Work › Docs",
                "Home › Today",
                "Home › Old",
                "Home › Old / Older",
                "Move to › Other",
                "Move to › New Space…",
            ]
        );
        host.take_edits();
        cx.update(|cx| host.choose_last("Docs", cx));
        assert_eq!(host.take_edits(), ["move 10 true 20 0"]);
        tab_menu(&shell, &host, cx, 10);
        cx.update(|cx| host.choose("Old / Older", cx));
        assert_eq!(host.take_edits(), ["space 10 2", "move 10 true 31 0"]);
    }

    /// The chosen rows' menu pins, folders or moves them together, each as one step that ⌘Z puts
    /// back in place and ⇧⌘Z does again, and a right-click on a row not chosen lets them go.
    #[gpui::test]
    fn the_chosen_rows_menu_pins_folders_and_moves_them_together(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[2], &[3, 4, 5], Folders::default());
        fn tabs(
            shell: &Entity<Shell>,
            cx: &mut TestAppContext,
            space: u64,
            pinned: Vec<u64>,
            today: Vec<u64>,
            folders: Folders,
        ) {
            shell.update(cx, |shell, cx| {
                shell.set_tabs(space, pinned, today, Vec::new(), folders, cx)
            })
        }
        let choose = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| {
                shell.choose_row(3, cx);
                shell.choose_row(5, cx);
            })
        };
        shell.update(cx, |shell, cx| {
            let space = |id, name: &str, profile| {
                (id, name.to_string().into(), None, profile, "".into(), "".into(), "".into())
            };
            let spaces = vec![space(1, "Work", 0), space(2, "Home", 0), space(3, "Other", 7)];
            shell.set_spaces(spaces, cx);
        });
        choose(&shell, cx);
        host.take();
        assert_eq!(
            tab_menu(&shell, &host, cx, 5),
            [
                "Pin Tabs",
                "Add to Folder › New Folder",
                "Move to Space › Home",
                "Copy URLs",
                "Close Tabs"
            ]
        );
        host.take_edits();
        cx.update(|cx| host.choose("Pin Tabs", cx));
        assert_eq!(host.take_edits(), ["pinned 3 true", "pinned 5 true"]);
        // The core reports them pinned once the calls return.
        tabs(&shell, cx, 1, vec![2, 3, 5], vec![4], Folders::default());
        shell.update(cx, |shell, cx| shell.undo(cx));
        assert_eq!(host.take_edits(), ["move 3 false 0 0", "move 5 false 0 2"]);
        shell.update(cx, |shell, cx| shell.redo(cx));
        assert_eq!(host.take_edits(), ["move 3 true 0 1", "move 5 true 0 2"]);
        host.take();
        assert_eq!(tab_menu(&shell, &host, cx, 3)[0], "Unpin Tabs");
        cx.update(|cx| host.dismiss(cx));

        tabs(&shell, cx, 1, vec![2], vec![3, 4, 5], Folders::default());
        host.take();
        tab_menu(&shell, &host, cx, 3);
        host.take_edits();
        cx.update(|cx| host.choose("New Folder", cx));
        let edits = host.take_edits();
        let folder: u64 = edits[0].split(' ').nth(1).unwrap().parse().unwrap();
        let into = |tab, index| format!("move {tab} true {folder} {index}");
        assert_eq!(edits[1..], [into(3, 0), into(5, 1)]);
        shell.read_with(cx, |shell, _| {
            assert!(shell.chosen.is_empty() && shell.renaming.is_some())
        });
        let made = Folders {
            folders: [(folder, ("New Folder".into(), true))].into(),
            parents: [(3, folder), (5, folder)].into(),
            ..Default::default()
        };
        tabs(&shell, cx, 1, vec![2, folder, 3, 5], vec![4], made);
        shell.update(cx, |shell, cx| shell.undo(cx));
        let back = ["move 3 false 0 0", "move 5 false 0 2", &format!("delete folder {folder}")];
        assert_eq!(host.take_edits(), back);

        tabs(&shell, cx, 1, vec![2], vec![3, 4, 5], Folders::default());
        choose(&shell, cx);
        host.take();
        tab_menu(&shell, &host, cx, 3);
        host.take_edits();
        cx.update(|cx| host.choose("Home", cx));
        assert_eq!(host.take_edits(), ["space 5 2", "space 3 2"]);
        tabs(&shell, cx, 1, vec![2], vec![4], Folders::default());
        tabs(&shell, cx, 2, Vec::new(), vec![3, 5], Folders::default());
        shell.update(cx, |shell, cx| shell.undo(cx));
        let back = ["space 3 1", "move 3 false 0 0", "space 5 1", "move 5 false 0 2"];
        assert_eq!(host.take_edits(), back);

        tabs(&shell, cx, 1, vec![2], vec![3, 4, 5], Folders::default());
        tabs(&shell, cx, 2, Vec::new(), Vec::new(), Folders::default());
        choose(&shell, cx);
        host.take();
        assert!(tab_menu(&shell, &host, cx, 4)[0].starts_with("Copy Link"));
        shell.read_with(cx, |shell, _| assert!(shell.chosen.is_empty()));
    }

    /// File › Share… and the command bar's Share… open the same sheet for the page shown, and
    /// aren't offered where the system has none, as on Linux, or for a page with no link.
    #[gpui::test]
    fn the_share_command_opens_the_sheet_for_the_page_shown(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        shell.update(cx, |shell, cx| {
            shell.set_page(10, page("https://example.com/?utm_source=x", false, false), cx);
            shell.set_selected(10, cx);
        });
        let offered =
            |cx: &mut TestAppContext| shell.read_with(cx, |shell, _| shell.offers(Command::Share));
        assert!(!offered(cx), "offered with no share sheet");
        host.share_sheet.set(true);
        assert!(offered(cx));
        let title = shell.read_with(cx, |shell, _| shell.title(10));
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::Share, "ui", cx));
        assert_eq!(host.take(), [format!("share sheet 10 https://example.com/ {title}")]);
        shell.update(cx, |shell, cx| shell.set_page(10, page("", false, false), cx));
        assert!(!offered(cx), "offered for a page with no link");
    }

    /// Where the system has neither, as Linux, the menu has no Share at all.
    #[gpui::test]
    fn share_is_left_out_where_the_system_cant_share(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10, 11], Folders::default());
        let link = page("https://example.com/", false, false);
        shell.update(cx, |shell, cx| shell.set_page(10, link, cx));
        let menu = tab_menu(&shell, &host, cx, 10);
        assert_eq!(menu[..2], ["Copy Link", "Change Icon…"]);
        assert!(!menu.iter().any(|item| item.starts_with("Share")));
    }

    /// Mute is dimmed while the tab is silent, and a muted tab's menu unmutes it.
    #[gpui::test]
    fn mute_follows_the_tabs_sound(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[], &[10], Folders::default());
        let url = "https://example.com/";
        shell.update(cx, |shell, cx| shell.set_page(10, page(url, true, false), cx));
        assert!(tab_menu(&shell, &host, cx, 10).contains(&"Mute".to_string()));
        shell.update(cx, |shell, cx| shell.set_page(10, page(url, false, true), cx));
        assert!(tab_menu(&shell, &host, cx, 10).contains(&"Unmute".to_string()));
    }

    /// A pinned tab's menu keeps its own items: it unpins, goes back to its URL and closes.
    #[gpui::test]
    fn a_pinned_tabs_menu_keeps_its_items(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        assert_eq!(
            tab_menu(&shell, &host, cx, 1),
            [
                "Unpin Tab",
                "Add to Favorites",
                "Back to Pinned URL (disabled)",
                "Pinned URL › Replace Pinned URL with Current (disabled)",
                "Pinned URL › Edit…",
                "Rename Tab",
                "Change Icon…",
                "Duplicate Tab",
                "Add to Folder › New Folder",
                "Move to Space (disabled)",
                "Close Tab",
            ]
        );
    }

    /// A tab's Add to Folder › lists New Folder, then the Space's folders. A folder takes the tab
    /// at its bottom, and New Folder makes one in the tab's place, named for its site, each one
    /// step for ⌘Z (#1088).
    #[gpui::test]
    fn add_to_folder_moves_a_tab_into_a_folder_or_a_new_one_in_its_place(cx: &mut TestAppContext) {
        let docs = Folders {
            folders: [(20, ("Docs".into(), true))].into(),
            parents: [(2, 20)].into(),
            ..Default::default()
        };
        let (shell, host) = shell_with(cx, &[1, 20, 2, 3], &[10], docs);
        let menu = tab_menu(&shell, &host, cx, 1);
        let folders: Vec<&String> =
            menu.iter().filter(|item| item.starts_with("Add to Folder")).collect();
        assert_eq!(folders, ["Add to Folder › New Folder", "Add to Folder › Docs"]);
        host.take_edits();
        cx.update(|cx| host.choose("Docs", cx));
        assert_eq!(host.take_edits(), ["move 1 true 20 1"]);
        shell.update(cx, |shell, cx| shell.undo(cx));
        assert_eq!(host.take_edits(), ["move 1 true 0 0"]);

        let site = page("https://www.example.com/a", false, false);
        shell.update(cx, |shell, cx| shell.set_page(3, site, cx));
        tab_menu(&shell, &host, cx, 3);
        cx.update(|cx| host.choose("New Folder", cx));
        let edits = host.take_edits();
        let folder: u64 = edits[0].split(' ').nth(1).unwrap().parse().unwrap();
        let named = format!("folder {folder} 1 example.com 0 2");
        assert_eq!(edits, [named, format!("move 3 true {folder} 0")]);
        shell.read_with(cx, |shell, _| assert!(shell.renaming.is_some(), "no name field"));
        // The core reports the folder made once the calls return.
        let made = Folders {
            folders: [(20, ("Docs".into(), true)), (folder, ("example.com".into(), true))].into(),
            parents: [(2, 20), (3, folder)].into(),
            ..Default::default()
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![1, 20, 2, folder, 3], vec![10], Vec::new(), made, cx)
        });
        shell.update(cx, |shell, cx| shell.undo(cx));
        let back = ["move 3 true 0 2".to_string(), format!("delete folder {folder}")];
        assert_eq!(host.take_edits(), back);
    }

    /// Away from its pinned URL, a pinned tab's Replace Pinned URL with Current makes the page it's
    /// at the one it goes back to, and a favorite's Replace Favorite URL with Current does the same.
    #[gpui::test]
    fn replace_with_current_keeps_the_page_its_at(cx: &mut TestAppContext) {
        let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
        let away = Page { away_from_pinned_url: true, ..page("https://away.test/", false, false) };
        shell.update(cx, |shell, cx| shell.set_page(1, away.clone(), cx));
        tab_menu(&shell, &host, cx, 1);
        host.take_edits();
        cx.update(|cx| host.choose("Replace Pinned URL with Current", cx));
        assert_eq!(host.take_edits(), ["pinned url 1 https://away.test/"]);
        shell.update(cx, |shell, cx| {
            shell.set_favorites(vec![(20, 0)], cx);
            shell.set_page(20, away, cx)
        });
        let menu = tab_menu(&shell, &host, cx, 20);
        assert_eq!(menu[..2], ["Back to Favorite URL", "Replace Favorite URL with Current"]);
        assert!(menu.contains(&"Change Icon…".to_string()), "{menu:?}");
        cx.update(|cx| host.choose("Replace Favorite URL with Current", cx));
        assert_eq!(host.take_edits(), ["pinned url 20 https://away.test/"]);
    }
}
