//! The import's calls (browser::import::Host), made with the core's calls a user's actions make:
//! a tab comes in by opening behind in the front window, unloaded, and moving to its Space and
//! its place there, so it loads only once it's first shown.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::CString;
use std::path::{Path, PathBuf};

use browser::import::{Exported, Passwords};
use browser::import::found::{Address, Page, Tab};
use browser::import::plan::{Existing, ExistingSpace, Row};
use browser::import::sources::{Os, Roots};
use browser::link_rules::{Host as _, Rule};
use browser::theme::Theme;
use gpui::{App, PathPromptOptions};

use crate::{bc, front, ids, name, open_tab_behind, platform};

thread_local! {
    /// What waits for the Spaces to change, such as an import for its new profiles to load.
    static WAITING: RefCell<Vec<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(Vec::new()) };
    /// What waits for history_added, in the order the history went in, which is the order the core
    /// answers in.
    static HISTORY: RefCell<VecDeque<Box<dyn FnOnce(usize, &mut App)>>> =
        const { RefCell::new(VecDeque::new()) };
    /// What waits for addresses_added, as HISTORY waits for history_added.
    static ADDRESSES: RefCell<VecDeque<Box<dyn FnOnce(usize, &mut App)>>> =
        const { RefCell::new(VecDeque::new()) };
    /// What waits for the app to come back to the front, such as an import for Full Disk Access.
    static ACTIVE: RefCell<Vec<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(Vec::new()) };
    /// What waits for passwords_imported, with the profile the passwords went to.
    static PASSWORDS: RefCell<Vec<(u64, Box<dyn FnOnce(Passwords, &mut App)>)>> =
        const { RefCell::new(Vec::new()) };
}

/// Runs what waited for the app to become the active one.
pub fn became_active(cx: &mut App) {
    for then in ACTIVE.take() {
        then(cx);
    }
}

/// Runs what waited for the Spaces to change.
pub fn spaces_changed(cx: &mut App) {
    for then in WAITING.take() {
        then(cx);
    }
}

/// Tells what waited for the history it added that it's in, with how many pages were new.
pub fn history_added(added: usize, cx: &mut App) {
    if let Some(then) = HISTORY.with_borrow_mut(VecDeque::pop_front) {
        then(added, cx);
    }
}

/// Tells what waited for the addresses it added that they're in, with how many were new.
pub fn addresses_added(added: usize, cx: &mut App) {
    if let Some(then) = ADDRESSES.with_borrow_mut(VecDeque::pop_front) {
        then(added, cx);
    }
}

/// Tells what waited for the passwords imported into `profile` what came of it.
pub fn passwords_imported(profile: u64, passwords: Passwords, cx: &mut App) {
    let at = PASSWORDS.with_borrow(|waiting| waiting.iter().position(|(each, _)| *each == profile));
    if let Some(at) = at {
        let (_, then) = PASSWORDS.with_borrow_mut(|waiting| waiting.remove(at));
        then(passwords, cx);
    }
}

/// The profile passwords go into: the one of the Space the front window shows, unless that's an
/// agent's, whose sessions aren't the user's to fill, else the one the browser started with.
fn passwords_profile() -> u64 {
    let space = unsafe { bc::bc_window_space(front().0) };
    if space != 0 && name(bc::bc_space_agent, space).is_empty() {
        return unsafe { bc::bc_space_profile(space) };
    }
    ids(bc::bc_profiles).first().copied().unwrap_or(0)
}

pub struct CoreImport;

/// The ids `copy` copies for `space`.
fn of_space(
    copy: unsafe extern "C" fn(bc::bc_id, *mut bc::bc_id, usize) -> usize,
    space: u64,
) -> Vec<u64> {
    let count = unsafe { copy(space, std::ptr::null_mut(), 0) };
    let mut tabs = vec![0; count];
    let count = unsafe { copy(space, tabs.as_mut_ptr(), tabs.len()) };
    tabs.truncate(count);
    tabs
}

/// The Space's pinned rows in `folder`, else at its top level.
fn rows(pinned: &[(u64, u64)], folder: u64) -> Vec<Row> {
    let inside = pinned.iter().filter(|&&(_, holder)| holder == folder);
    inside
        .map(|&(id, _)| {
            if unsafe { bc::bc_folder(id, std::ptr::null_mut()) } {
                let name = name(bc::bc_folder_name, id).to_string();
                Row::Folder {
                    id,
                    name,
                    rows: rows(pinned, id),
                }
            } else {
                Row::Tab(name(bc::bc_tab_pinned_url, id).to_string())
            }
        })
        .collect()
}

/// Opens a tab for `url` that loads once shown, in `space`, at the top of its Today tabs, and
/// returns it, or 0.
fn open_in(space: u64, url: &str) -> u64 {
    let window = front().0;
    let tab = open_tab_behind(window, url, true);
    let shown = unsafe { bc::bc_window_space(window) };
    if tab == 0 || shown == space {
        return tab;
    }
    let same_profile = unsafe { bc::bc_space_profile(shown) == bc::bc_space_profile(space) };
    unsafe { bc::bc_tab_move_to_space(tab, space) };
    if same_profile {
        return tab;
    }
    // Into another profile's Space it opens again as a new tab, at the top of its Today tabs.
    of_space(bc::bc_space_today, space)
        .first()
        .copied()
        .unwrap_or(0)
}

/// Pins `tab` at `tab`'s URL, which the core would take from the page it hasn't loaded, with its
/// name.
fn keep(id: u64, tab: &Tab) {
    let url = CString::new(tab.url.as_str()).unwrap_or_default();
    unsafe { bc::bc_tab_set_pinned_url(id, url.as_ptr()) };
    if !tab.name.is_empty() {
        let name = CString::new(tab.name.as_str()).unwrap_or_default();
        unsafe { bc::bc_tab_rename(id, name.as_ptr()) };
    }
}

impl browser::import::Host for CoreImport {
    fn roots(&self) -> Roots {
        // A check run gives a made-up home folder, so it never reads the user's browsers.
        match std::env::var_os("BROWSER_IMPORT_HOME") {
            Some(home) => Roots::under(Path::new(&home), Os::current()),
            None => Roots::system(),
        }
    }

    fn existing(&self, cx: &App) -> Existing {
        // Every window's shell has every tab's page, as the core tells them all.
        let shell = crate::shell_with_view(front().1).upgrade();
        let url = |tab: u64| shell.as_ref().map(|shell| shell.read(cx).page(tab).url.to_string());
        let profiles = ids(bc::bc_profiles)
            .into_iter()
            .map(|profile| (profile, name(bc::bc_profile_name, profile).to_string()))
            .collect();
        let spaces = ids(bc::bc_spaces)
            .into_iter()
            // An agent's Space is never the user's to import into.
            .filter(|&space| name(bc::bc_space_agent, space).is_empty())
            .map(|space| {
                let pinned: Vec<(u64, u64)> = of_space(bc::bc_space_pinned, space)
                    .into_iter()
                    .map(|id| (id, unsafe { bc::bc_folder_of(id) }))
                    .collect();
                ExistingSpace {
                    id: space,
                    name: name(bc::bc_space_name, space).to_string(),
                    profile: unsafe { bc::bc_space_profile(space) },
                    pinned: rows(&pinned, 0),
                    today: of_space(bc::bc_space_today, space)
                        .into_iter()
                        .filter_map(&url)
                        .collect(),
                }
            })
            .collect();
        let favorites = ids(bc::bc_favorites)
            .into_iter()
            .map(|tab| {
                (
                    unsafe { bc::bc_tab_profile(tab) },
                    name(bc::bc_tab_pinned_url, tab).to_string(),
                )
            })
            .collect();
        Existing {
            profiles,
            spaces,
            favorites,
            link_rules: crate::link_rules_host::CoreLinkRules.rules().rules,
        }
    }

    fn create_profile(&self, name: &str) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_profile_create(name.as_ptr()) }
    }

    fn create_space(&self, name: &str, icon: &str, theme: Option<&Theme>) -> u64 {
        let space = crate::create_space(name, 0);
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_space_set_icon(space, icon.as_ptr()) };
        if let Some(theme) = theme {
            let text = CString::new(theme.to_text()).unwrap_or_default();
            unsafe {
                bc::bc_space_set_theme(space, text.as_ptr());
                bc::bc_space_set_color(space, true, theme.main_color());
            }
        }
        space
    }

    fn set_space_profile(&self, space: u64, profile: u64) {
        unsafe { bc::bc_space_set_profile(space, profile) };
    }

    fn space_profile(&self, space: u64) -> u64 {
        unsafe { bc::bc_space_profile(space) }
    }

    fn create_folder(&self, space: u64, folder: u64, name: &str) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_folder_create(space, name.as_ptr(), folder, usize::MAX) }
    }

    fn add_pinned(&self, space: u64, folder: u64, tab: &Tab) {
        let id = open_in(space, &tab.url);
        if id != 0 {
            unsafe { bc::bc_tab_move(id, true, folder, usize::MAX) };
            keep(id, tab);
        }
    }

    fn add_favorite(&self, space: u64, tab: &Tab) {
        let id = open_in(space, &tab.url);
        if id != 0 {
            unsafe { bc::bc_tab_move_to_favorites(id, usize::MAX) };
            keep(id, tab);
        }
    }

    fn add_today(&self, space: u64, tab: &Tab) {
        open_in(space, &tab.url);
    }

    fn add_link_rules(&self, rules: &[Rule]) {
        let host = crate::link_rules_host::CoreLinkRules;
        let mut all = host.rules();
        all.rules.extend_from_slice(rules);
        host.set_rules(&all);
    }

    fn profile_loaded(&self, profile: u64) -> bool {
        ids(bc::bc_profiles).contains(&profile)
    }

    fn add_history(
        &self,
        profile: u64,
        pages: &[Page],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool {
        // A page whose text holds a nul can't cross; it's dropped rather than cut short.
        let text: Vec<(CString, CString, &Page)> = pages
            .iter()
            .filter_map(|page| {
                let url = CString::new(page.url.as_str()).ok()?;
                let title = CString::new(page.title.as_str()).unwrap_or_default();
                Some((url, title, page))
            })
            .collect();
        let rows: Vec<bc::bc_history_page> = text
            .iter()
            .map(|(url, title, page)| bc::bc_history_page {
                url: url.as_ptr(),
                title: title.as_ptr(),
                last_visit: page.last_visit,
                visits: page.visits.min(i32::MAX as u32) as i32,
                typed: page.typed.min(i32::MAX as u32) as i32,
            })
            .collect();
        let started = unsafe { bc::bc_profile_add_history(profile, rows.as_ptr(), rows.len()) };
        if started {
            HISTORY.with_borrow_mut(|waiting| waiting.push_back(then));
        }
        started
    }

    fn add_addresses(
        &self,
        profile: u64,
        addresses: &[Address],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool {
        // A value holding a nul can't cross; that field is dropped rather than cut short.
        let values: Vec<Vec<(i32, CString, i32)>> = (addresses.iter())
            .map(|address| {
                (address.fields.iter())
                    .filter_map(|field| {
                        let value = CString::new(field.value.as_str()).ok()?;
                        Some((field.kind, value, field.status))
                    })
                    .collect()
            })
            .collect();
        let fields: Vec<Vec<bc::bc_address_field>> = (values.iter())
            .map(|address| {
                (address.iter())
                    .map(|(kind, value, status)| bc::bc_address_field {
                        type_: *kind,
                        value: value.as_ptr(),
                        status: *status,
                    })
                    .collect()
            })
            .collect();
        let rows: Vec<bc::bc_address> = (fields.iter())
            .map(|fields| bc::bc_address {
                fields: fields.as_ptr(),
                count: fields.len(),
            })
            .collect();
        let started = unsafe { bc::bc_profile_add_addresses(profile, rows.as_ptr(), rows.len()) };
        if started {
            ADDRESSES.with_borrow_mut(|waiting| waiting.push_back(then));
        }
        started
    }

    fn when_spaces_change(&self, then: Box<dyn FnOnce(&mut App)>) {
        WAITING.with_borrow_mut(|waiting| waiting.push(then));
    }

    fn open_full_disk_access(&self) {
        unsafe { platform::host_open_full_disk_access() };
    }

    fn when_active(&self, then: Box<dyn FnOnce(&mut App)>) {
        ACTIVE.with_borrow_mut(|waiting| waiting.push(then));
    }

    fn choose_file(
        &self,
        exported: Exported,
        then: Box<dyn FnOnce(Option<PathBuf>, &mut App)>,
        cx: &mut App,
    ) {
        // A check run names its made-up file, as it can't drive the system's panel.
        let named = match exported {
            Exported::Passwords => "BROWSER_IMPORT_PASSWORDS",
            Exported::Bookmarks => "BROWSER_IMPORT_BOOKMARKS",
        };
        if let Some(file) = std::env::var_os(named) {
            return cx.defer(move |cx| then(Some(file.into()), cx));
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |cx| {
            let chosen = chosen.await.ok().and_then(Result::ok).flatten();
            let file = chosen.and_then(|files| files.into_iter().next());
            cx.update(|cx| then(file, cx));
        })
        .detach();
    }

    fn import_passwords(&self, file: &Path, then: Box<dyn FnOnce(Passwords, &mut App)>) -> bool {
        let profile = passwords_profile();
        let Ok(path) = CString::new(file.to_string_lossy().as_bytes()) else {
            return false;
        };
        let started = unsafe { bc::bc_profile_import_passwords(profile, path.as_ptr()) };
        if started {
            PASSWORDS.with_borrow_mut(|waiting| waiting.push((profile, then)));
        }
        started
    }

    fn passwords_profile(&self) -> Option<String> {
        let profiles = ids(bc::bc_profiles);
        (profiles.len() > 1).then(|| name(bc::bc_profile_name, passwords_profile()).to_string())
    }

    fn open_store_page(&self, url: &str) {
        open_tab_behind(front().0, url, false);
    }
}
