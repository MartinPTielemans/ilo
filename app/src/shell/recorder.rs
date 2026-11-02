//! A host for the shell's tests, which records what the shell asks of the core and answers
//! nothing, so a test plays the core's reports itself.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{App, Bounds, Pixels, Point, RenderImage, SharedString, Size, WeakEntity};

use crate::theme::Appearance;

use super::{
    Answer, BarTab, Browser, DownloadAction, MediaAction, MenuBar, MenuEntry, PageReply,
    ShownPageDialog, browser::WindowButton,
};

/// What the core shares between its windows, as it keeps them: the Spaces deleted and the archive's
/// removals, 25 of each at most, the newest last. Clears are each window's own.
#[derive(Default)]
pub(super) struct Core {
    ticket: Cell<u64>,
    /// The Spaces deleted.
    pub(super) deleted: RefCell<Vec<u64>>,
    /// The archive removals, each by its id and the tabs it took.
    pub(super) removals: RefCell<Vec<(u64, usize)>>,
}

impl Core {
    /// As many as the core keeps.
    const KEPT: usize = 25;

    fn ticket(&self) -> u64 {
        self.ticket.set(self.ticket.get() + 1);
        9000 + self.ticket.get()
    }

    fn removed(&self, tabs: usize) -> u64 {
        let id = self.ticket();
        let mut removals = self.removals.borrow_mut();
        removals.push((id, tabs));
        if removals.len() > Self::KEPT {
            removals.remove(0);
        }
        id
    }
}

#[derive(Default)]
pub(super) struct Recorder {
    /// The core this window shares with the others a test makes with `Recorder::beside`.
    pub(super) core: Rc<Core>,
    /// This window's Clears, by id, the last ten.
    cleared: RefCell<Vec<u64>>,
    calls: RefCell<Vec<String>>,
    /// How many detents the trackpad was asked for.
    pub(super) detents: Cell<usize>,
    /// How many tabs and folders it was asked to move.
    pub(super) moves: Cell<usize>,
    /// Whether the system opens links with it, as `is_default_browser` answers.
    pub(super) default_browser: Cell<bool>,
    /// The system it says it runs on, the Mac unless a test says.
    pub(super) os: Cell<Option<crate::default_browser::Os>>,
    /// The build updates itself, as `updates` answers, and how often Check for Updates ran.
    pub(super) updates: Cell<bool>,
    pub(super) checked_for_updates: Cell<usize>,
    /// The import finds no browser on the machine.
    pub(super) no_browsers: Cell<bool>,
    /// The window is full screen, as Linux's window manager says, for `window_full_screen`.
    pub(super) full_screen: Cell<bool>,
    /// The opt-ins' switches, as `crash_reports` and `agents` answer, None for one not offered.
    pub(super) crash_reports: Cell<Option<bool>>,
    pub(super) agents: Cell<Option<bool>>,
    /// The user turned off ⌥ bringing the hidden sidebar out.
    pub(super) option_off: Cell<bool>,
    /// The window's own buttons as the system says it draws them, once their window shows them.
    pub(super) window_buttons: Cell<Option<super::WindowButtonGeometry>>,
    /// The Spaces it was asked to show, in order.
    pub(super) shown: RefCell<Vec<u64>>,
    /// The menus it was asked to show, each item by its title and whether it's enabled.
    pub(super) menus: RefCell<Vec<Vec<(SharedString, bool)>>>,
    /// Where the last menu was asked to show, in the window.
    pub(super) menu_at: Cell<Option<Point<Pixels>>>,
    /// The last menus it was asked to show, item by item.
    pub(super) entries: RefCell<Vec<MenuEntry>>,
    /// The look of each of ilo's own menus it was asked to show, as `menus` has them.
    pub(super) menu_looks: RefCell<Vec<crate::menu::Look>>,
    /// Whether each of ilo's own menus was to show in the reveal's panel, as `menus` has them.
    pub(super) menus_in_reveal: RefCell<Vec<bool>>,
    /// What the menu bar was last asked to show, and the window's title.
    pub(super) menu_bar: RefCell<MenuBar>,
    /// What `confirm` asked last, with what it does if the test confirms it.
    pub(super) confirming: RefCell<Option<(String, String, Box<dyn FnOnce(&mut App)>)>>,
    pub(super) window_title: RefCell<String>,
    /// Whether it was last asked to draw the window's material light.
    pub(super) window_light: Cell<Option<bool>>,
    /// The tab and the look capture mode over it was last given, and whether it was light.
    pub(super) capture_look: Cell<Option<(u64, bool)>>,
    /// The appearance View › Appearance last chose.
    pub(super) appearance: Cell<Option<Appearance>>,
    /// The share services it lists for any page.
    pub(super) share_services: RefCell<Vec<SharedString>>,
    /// The system has a share sheet of its own.
    pub(super) share_sheet: Cell<bool>,
    /// What the last menu does with the item chosen, as `choose` picks it.
    answer: RefCell<Option<Box<dyn FnOnce(Option<usize>, &mut App)>>>,
    /// The questions the dialog was asked through `tell`, each as its message, its buttons and
    /// its other answer's, and what the last one's action does.
    pub(super) told: RefCell<Vec<String>>,
    pub(super) told_action: RefCell<Option<Box<dyn FnOnce(Vec<String>, &mut App)>>>,
    /// The dialog asks something else, so `tell` doesn't ask.
    pub(super) dialog_busy: Cell<bool>,
    /// Each time the shell opened the Site Controls: the tab, and where they hang from.
    pub(super) site_controls: RefCell<Vec<(u64, crate::site_controls::Place)>>,
    /// The ids of the extensions the Site Controls were given to list, each time.
    pub(super) site_controls_extensions: RefCell<Vec<Vec<SharedString>>>,
    /// Each time the shell opened the command bar: the URL field's box it opened over, if any.
    pub(super) command_bars: RefCell<Vec<Option<Bounds<Pixels>>>>,
    /// And the tab whose URL it edits, if any.
    pub(super) bar_edits: RefCell<Vec<Option<u64>>>,
    /// Each time the shell opened Search Tabs: the tabs it lists, in order.
    pub(super) searched_tabs: RefCell<Vec<Vec<u64>>>,
    /// The command bar shows, until a test plays the app's report that it was dismissed.
    pub(super) bar_open: Cell<bool>,
    /// Each New Note: the Space whose profile's service it opens, and the tab it splits with.
    pub(super) notes: RefCell<Vec<(u64, Option<u64>)>>,
    /// How many times Site Search Settings opened.
    pub(super) site_search_settings: Cell<usize>,
    pub(super) clear_browsing_data: Cell<usize>,
    /// The pages `open_page` opened, in order.
    pub(super) opened_pages: RefCell<Vec<String>>,
    /// The text `copy` put on the clipboard, in order.
    pub(super) copied: RefCell<Vec<String>>,
    /// The tabs the command bar was given to list each time the window showed another Space.
    pub(super) followed_tabs: RefCell<Vec<Vec<u64>>>,
    /// What moved the pages, in order: "layout" for the page's frame, "shares" for a split's, and
    /// "moving" and "still" for what the shell told the core about the motion.
    pub(super) page_events: RefCell<Vec<&'static str>>,
    /// Where each tooltip it was asked for hangs from, in order.
    pub(super) anchors: RefCell<Vec<crate::tooltip::Anchor>>,
    /// The last hover card the shell asked for, for a test to press its buttons.
    pub(super) card: RefCell<Option<crate::hover_card::Rest>>,
    /// The folder preview asked for last, whose choice a test makes.
    pub(super) folder_preview: RefCell<Option<crate::folder_preview::Rest>>,
    /// Where the shell last put the link target's plate, in the window.
    pub(super) link_frame: Cell<Option<Bounds<Pixels>>>,
    /// What the shell did to tabs, folders and Spaces that ⌘Z puts back, in order, such as
    /// `close 5` and `reopen 5 as 1001`.
    edits: RefCell<Vec<String>>,
    /// The id the next tab or folder it makes gets.
    made: Cell<u64>,
    /// The URL each pinned tab or favorite goes back to, and the name each tab was given.
    pub(super) pinned_urls: RefCell<HashMap<u64, String>>,
    /// Each Markdown file saved: the name offered, the file and its links.
    pub(super) saved_markdown: RefCell<Vec<(String, String, usize)>>,
    pub(super) names: RefCell<HashMap<u64, String>>,
    /// The words of the toasts with no Undo button it was asked to show.
    pub(super) toasts: RefCell<Vec<String>>,
    /// The words of the toasts with an Undo button it was asked to show, and what the last one's
    /// button does.
    pub(super) undo_toasts: RefCell<Vec<String>>,
    pub(super) undo_toast: RefCell<Option<Rc<dyn Fn(&mut App)>>>,
    /// What the last toast with a button other than Undo runs, its words in `toasts`.
    pub(super) toast_run: RefCell<Option<Rc<dyn Fn(&mut App)>>>,
    /// Another browser window is in front of this one.
    pub(super) behind: Cell<bool>,
    /// The Edit menu's Undo and Redo titles it was last given.
    pub(super) edit_titles: RefCell<(Option<String>, Option<String>)>,
    /// The platform has Apple's engine for a tab's page.
    /// The platform has Safari's engine for a tab's page.
    pub(super) system_engine: Cell<bool>,
    /// Where it says Google's Widevine module is.
    pub(super) widevine: Cell<crate::widevine::Widevine>,
    /// What it says PlayReady needs.
    pub(super) playready: Cell<super::PlayReady>,
    /// The shell it answers each picture asked of it for with this picture, at once, inside the
    /// call that asked, as a core that answers inline would.
    pub(super) pictures: RefCell<Option<(WeakEntity<super::Shell>, super::PagePicture)>>,
    /// The answers to page dialogs it was given, with what was typed.
    pub(super) replies: RefCell<Vec<PageReply>>,
    /// What it says every agent's Space's menu lists.
    pub(super) agent_permissions: RefCell<super::AgentGrants>,
    /// What the Library button's hover leaves out, none at first, as the core's default.
    hover_kinds_off: Cell<u32>,
    /// The account's core, as the first launch's account step asks it, in a build that names a
    /// sync server, which a test says; one that names none at first.
    pub(super) account: Rc<Account>,
    pub(super) sync_server: Cell<bool>,
    /// The videos opened that haven't given their first frame yet.
    videos_starting: RefCell<Vec<u64>>,
}

impl Recorder {
    /// A host for another window of the same core.
    pub(super) fn beside(other: &Recorder) -> Self {
        Recorder {
            core: other.core.clone(),
            ..Default::default()
        }
    }

    fn record(&self, call: String) {
        self.calls.borrow_mut().push(call);
    }

    /// Records the menu it was asked to show, for `choose` to answer.
    fn menu(
        &self,
        entries: Vec<MenuEntry>,
        position: Point<Pixels>,
        _: Option<usize>,
        _: bool,
        answer: Box<dyn FnOnce(Option<usize>, &mut App)>,
        _: &mut App,
    ) {
        self.menu_at.set(Some(position));
        let items = entries.iter().map(|entry| (entry.title.clone(), entry.enabled)).collect();
        self.menus.borrow_mut().push(items);
        *self.entries.borrow_mut() = entries;
        *self.answer.borrow_mut() = Some(answer);
    }

    /// Chooses the item titled `title` in the last menu, as the user's click does.
    pub(super) fn choose(&self, title: &str, cx: &mut App) {
        let at = self.menus.borrow().last().and_then(|items| {
            items.iter().position(|(each, enabled)| each.as_ref() == title && *enabled)
        });
        let answer = self.answer.borrow_mut().take().expect("a menu to choose from");
        answer(Some(at.unwrap_or_else(|| panic!("no {title} in the menu"))), cx);
    }

    /// Chooses the last item titled `title`, for a title that's in two submenus.
    pub(super) fn choose_last(&self, title: &str, cx: &mut App) {
        let at = self.menus.borrow().last().and_then(|items| {
            items.iter().rposition(|(each, enabled)| each.as_ref() == title && *enabled)
        });
        let answer = self.answer.borrow_mut().take().expect("a menu to choose from");
        answer(Some(at.unwrap_or_else(|| panic!("no {title} in the menu"))), cx);
    }

    /// Closes the last menu without a choice, as Escape or a click outside it does.
    pub(super) fn dismiss(&self, cx: &mut App) {
        let answer = self.answer.borrow_mut().take().expect("a menu to close");
        answer(None, cx);
    }

    /// What the shell asked for since the last time, in order.
    pub(super) fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.calls.borrow_mut())
    }

    fn edit(&self, edit: String) {
        self.edits.borrow_mut().push(edit);
    }

    /// What the shell did that ⌘Z puts back since the last time, in order.
    pub(super) fn take_edits(&self) -> Vec<String> {
        std::mem::take(&mut self.edits.borrow_mut())
    }

    fn make(&self) -> u64 {
        self.made.set(self.made.get() + 1);
        1000 + self.made.get()
    }
}

impl Browser for Recorder {
    fn select(&self, tab: u64, _: &mut App) {
        self.record(format!("select {tab}"))
    }

    fn close(&self, tab: u64, _: bool, _: &mut App) {
        self.edit(format!("close {tab}"));
        self.record(format!("close {tab}"))
    }

    fn close_below(&self, tab: u64, _: &mut App) {
        self.edit(format!("close below {tab}"));
    }

    fn duplicate(&self, tab: u64, _: &mut App) -> u64 {
        self.record(format!("duplicate {tab}"));
        self.make()
    }

    fn move_to_space(&self, tab: u64, space: u64, _: &mut App) {
        self.edit(format!("space {tab} {space}"));
    }

    fn warn_moving_profiles(&self, _: &mut App) -> bool {
        Default::default()
    }

    fn set_warn_moving_profiles(&self, _: bool, _: &mut App) {}

    fn set_pinned(&self, tab: u64, pinned: bool, _: &mut App) {
        self.edit(format!("pinned {tab} {pinned}"));
    }

    fn move_to_favorites(&self, tab: u64, index: usize, _: &mut App) {
        self.edit(format!("favorite {tab} {index}"));
        self.moves.set(self.moves.get() + 1);
    }

    fn move_tab(&self, tab: u64, pinned: bool, folder: u64, index: usize, _: &mut App) {
        self.edit(format!("move {tab} {pinned} {folder} {index}"));
        self.moves.set(self.moves.get() + 1);
    }

    fn open_at(
        &self,
        urls: &[String],
        pinned: bool,
        folder: u64,
        index: usize,
        _: &mut App,
    ) -> u64 {
        self.record(format!("open {} {pinned} {folder} {index}", urls.join(" ")));
        Default::default()
    }

    fn open_behind(&self, urls: &[String], pinned: bool, folder: u64, index: usize, _: &mut App) {
        self.record(format!("open behind {} {pinned} {folder} {index}", urls.join(" ")));
    }

    fn create_folder(&self, space: u64, name: &str, folder: u64, index: usize, _: &mut App) -> u64 {
        let made = self.make();
        self.edit(format!("folder {made} {space} {name} {folder} {index}"));
        made
    }

    fn rename_folder(&self, folder: u64, name: &str, _: &mut App) {
        self.edit(format!("rename folder {folder} {name}"));
    }

    fn set_folder_icon(&self, folder: u64, icon: &str, _: &mut App) {
        self.edit(format!("folder icon {folder} {icon}"));
    }

    fn duplicate_folder(&self, folder: u64, _: &mut App) -> u64 {
        let made = self.make();
        self.edit(format!("duplicate folder {folder} {made}"));
        made
    }

    fn set_folder_open(&self, folder: u64, open: bool, _: &mut App) {
        self.record(format!("folder open {folder} {open}"))
    }

    fn set_pinned_open(&self, space: u64, open: bool, _: &mut App) {
        self.record(format!("pinned open {space} {open}"))
    }

    fn move_folder(&self, folder: u64, to: u64, index: usize, _: &mut App) {
        self.edit(format!("move folder {folder} {to} {index}"));
        self.moves.set(self.moves.get() + 1);
    }

    fn delete_folder(&self, folder: u64, _: &mut App) {
        self.edit(format!("delete folder {folder}"));
    }

    fn separate(&self, tab: u64, _: &mut App) {
        self.edit(format!("separate {tab}"));
    }

    fn move_in_split(&self, _: u64, _: usize, _: &mut App) {}

    fn set_split_stacked(&self, tab: u64, stacked: bool, _: &mut App) {
        self.record(format!("split stacked {tab} {stacked}"))
    }

    fn set_split_shares(&self, tab: u64, shares: Option<&[f64]>, _: &mut App) {
        self.page_events.borrow_mut().push("shares");
        let shares = shares.map_or_else(
            || "even".to_string(),
            |shares| shares.iter().map(f64::to_string).collect::<Vec<_>>().join(" "),
        );
        self.record(format!("split shares {tab} {shares}"))
    }

    fn set_split_expanded(&self, tab: u64, expanded: bool, _: &mut App) {
        self.record(format!("split expanded {tab} {expanded}"))
    }

    fn split_with(&self, tab: u64, with: u64, left: bool, _: &mut App) {
        self.edit(format!("split {tab} {with} {left}"));
    }

    fn open_split_at(&self, url: &str, with: u64, left: bool, stacked: bool, _: &mut App) {
        self.record(format!("open split {url} {with} {left} {stacked}"))
    }

    fn show_drop_targets(&self, _: bool, _: &mut App) {}

    fn back_to_pinned_url(&self, tab: u64, _: &mut App) {
        self.record(format!("back to pinned url {tab}"))
    }

    fn set_muted(&self, _: u64, _: bool, _: &mut App) {}

    fn share_instead(&self, tab: u64, _: &mut App) {
        self.record(format!("share instead {tab}"));
    }

    fn media(&self, tab: u64, action: MediaAction, _: &mut App) {
        self.record(format!("media {tab} {action:?}"));
    }

    fn cast(&self, tab: u64, _: &mut App) {
        self.record(format!("cast {tab}"));
    }

    fn cast_close(&self, _: &mut App) {
        self.record("cast close".to_owned());
    }

    fn cast_to(&self, tab: u64, id: SharedString, _: &mut App) {
        self.record(format!("cast to {tab} {id}"));
    }

    fn clear(&self, _: &mut App) -> u64 {
        let id = self.core.ticket();
        self.edit(format!("clear as {id}"));
        self.record("clear".into());
        let mut cleared = self.cleared.borrow_mut();
        cleared.push(id);
        if cleared.len() > 10 {
            cleared.remove(0);
        }
        id
    }

    fn undo_clear(&self, clear: u64, _: &mut App) -> usize {
        let mut cleared = self.cleared.borrow_mut();
        let Some(at) = cleared.iter().position(|&each| each == clear) else {
            return 0;
        };
        cleared.remove(at);
        self.edit(format!("undo clear {clear}"));
        self.record("undo clear".into());
        1
    }

    fn undo_delete_space(&self, space: u64, _: &mut App) -> bool {
        let mut deleted = self.core.deleted.borrow_mut();
        let Some(at) = deleted.iter().position(|&each| each == space) else {
            return false;
        };
        deleted.remove(at);
        self.edit(format!("undo delete space {space}"));
        true
    }

    fn reopen_closed_tab(&self, _: &mut App) {
        self.record("reopen closed tab".into());
    }

    fn reopen_preview(
        &self,
        closed: &crate::link_preview::Closed,
        _: (Bounds<Pixels>, f32),
        _: &mut App,
    ) {
        self.record(format!("reopen preview over {} {}", closed.over, closed.url));
    }

    fn close_preview(&self, over: u64, _: &mut App) -> bool {
        self.record(format!("close preview over {over}"));
        true
    }

    fn reopen_tab(&self, closed: u64, _: &mut App) -> u64 {
        let made = self.make();
        self.edit(format!("reopen {closed} as {made}"));
        made
    }

    fn pinned_url(&self, tab: u64) -> String {
        self.pinned_urls.borrow().get(&tab).cloned().unwrap_or_default()
    }

    fn set_pinned_url(&self, tab: u64, url: &str, _: &mut App) {
        self.edit(format!("pinned url {tab} {url}"));
        self.pinned_urls.borrow_mut().insert(tab, url.to_string());
    }

    /// An address as it's typed, else a test search.
    fn typed_url(&self, text: &str) -> Option<String> {
        Some(match text.contains('.') {
            true if text.contains("://") => text.to_string(),
            true => format!("https://{text}/"),
            false => format!("https://search.test/?q={text}"),
        })
    }

    fn tab_name(&self, tab: u64) -> String {
        self.names.borrow().get(&tab).cloned().unwrap_or_default()
    }

    fn undo_archive_remove(&self, removal: u64, _: &mut App) -> usize {
        let mut removals = self.core.removals.borrow_mut();
        let Some(at) = removals.iter().position(|&(id, _)| id == removal) else {
            return 0;
        };
        self.edit(format!("undo archive remove {removal}"));
        removals.remove(at).1
    }

    fn restore_archived(&self, _: u64, _: &mut App) {}

    fn remove_archived(&self, archived: u64, _: &mut App) -> u64 {
        let id = self.core.removed(1);
        self.edit(format!("archive remove {archived} as {id}"));
        id
    }

    fn clear_archive(&self, _: &mut App) -> u64 {
        let id = self.core.removed(2);
        self.edit(format!("archive clear as {id}"));
        id
    }

    fn act_on_download(&self, _: u64, _: DownloadAction, _: &mut App) {}

    fn clear_downloads(&self, _: &mut App) {}

    fn download_icon(&self, _: u64, _: &mut App) {}

    fn file_preview(&self, download: u64, _: &str, _: &mut App) {
        self.record(format!("file preview {download}"))
    }

    fn open_video(&self, id: u64, _: &str, side: u32, _: &mut App) -> bool {
        self.record(format!("open video {id} {side}"));
        self.videos_starting.borrow_mut().push(id);
        true
    }

    /// A pixel, the first time.
    fn video_frame(&self, id: u64, _: &mut App) -> Option<Arc<RenderImage>> {
        let mut starting = self.videos_starting.borrow_mut();
        let at = starting.iter().position(|&each| each == id)?;
        starting.remove(at);
        let pixel = image::RgbaImage::from_raw(1, 1, vec![0; 4])?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixel)])))
    }

    fn close_video(&self, id: u64, _: &mut App) {
        self.record(format!("close video {id}"))
    }

    fn list_captures(&self, _: &mut App) {
        self.record("list captures".into())
    }

    fn hover_kinds(&self, _: &mut App) -> u32 {
        super::recent_downloads::HOVER_ALL & !self.hover_kinds_off.get()
    }

    fn set_hover_kinds(&self, kinds: u32, _: &mut App) {
        self.hover_kinds_off.set(super::recent_downloads::HOVER_ALL & !kinds);
    }

    fn trash(&self, path: &str, _: &mut App) -> bool {
        self.record(format!("trash {path}"));
        true
    }

    fn drag_file(&self, _: &str, _: &mut App) -> bool {
        Default::default()
    }

    fn page_favicon(&self, _: &str, _: &mut App) {}

    fn day_starts(&self) -> [f64; 4] {
        Default::default()
    }

    fn go_back(&self, tab: u64, _: &mut App) {
        self.record(format!("back {tab}"))
    }

    fn go_forward(&self, tab: u64, _: &mut App) {
        self.record(format!("forward {tab}"))
    }

    fn tab_history(&self, tab: u64, forward: bool, _: &mut App) {
        self.record(format!("history {tab} {}", if forward { "forward" } else { "back" }))
    }

    fn go_to_history(&self, tab: u64, offset: i32, _: &mut App) {
        self.record(format!("go to {offset} in {tab}"))
    }

    fn open_history(&self, tab: u64, offset: i32, _: &mut App) {
        self.record(format!("open {offset} of {tab} in the background"))
    }

    fn reload(&self, tab: u64, bypass_cache: bool, _: &mut App) {
        self.record(format!("reload {tab}{}", if bypass_cache { " bypassing cache" } else { "" }))
    }

    fn clear_site(&self, _: u64, _: bool, _: &mut App) {}

    fn set_developer_mode(&self, tab: u64, on: bool, _: &mut App) {
        self.record(format!("developer mode {tab} {on}"))
    }

    fn answer_permission(&self, tab: u64, answer: Answer, _: &mut App) {
        self.record(format!("answer {tab} {answer:?}"))
    }

    fn answer_agent(&self, question: u64, answer: crate::agents::Answer, _: &mut App) {
        self.record(format!("answer agent {question} {answer:?}"))
    }

    fn can_open_in_safari(&self) -> bool {
        true
    }

    fn open_in_safari(&self, url: &str, _: &mut App) {
        self.record(format!("open in safari {url}"))
    }

    fn can_use_system_engine(&self) -> bool {
        self.system_engine.get()
    }

    fn playready_needs(&self) -> super::PlayReady {
        self.playready.get()
    }

    fn close_media_note(&self, tab: u64, _: &mut App) {
        self.record(format!("close media note {tab}"));
    }

    fn use_system_engine(&self, tab: u64, _: &mut App) {
        self.record(format!("system engine {tab}"))
    }

    fn widevine(&self) -> crate::widevine::Widevine {
        self.widevine.get()
    }

    fn download_widevine(&self, _: &mut App) {
        self.record("download widevine".into())
    }

    fn restart(&self, _: &mut App) {
        self.record("restart".into())
    }

    fn show_page_dialog(&self, shown: Option<ShownPageDialog>, _: &mut App) {
        match shown {
            Some((tab, dialog, _)) => self.record(format!("page dialog {tab} {}", dialog.id)),
            None => self.record("page dialog none".into()),
        }
    }

    fn show_offer(&self, tab: u64) {
        self.record(format!("show offer {tab}"))
    }

    fn answer_page_dialog(&self, tab: u64, id: u64, reply: PageReply, _: &mut App) {
        self.record(format!("answer dialog {tab} {id} {reply:?}"));
        self.replies.borrow_mut().push(reply);
    }

    fn stop(&self, _: u64, _: &mut App) {}

    fn zoom(&self, _: u64, _: i32, _: &mut App) {}

    fn view_source(&self, _: u64, _: &mut App) {}

    fn save_page(&self, _: u64, _: &mut App) {}

    fn save_markdown(&self, name: &str, markdown: String, links: usize, _: &mut App) {
        self.saved_markdown.borrow_mut().push((name.to_string(), markdown, links));
    }

    fn close_page_menu(&self, tab: u64, command: Option<i32>, _: &mut App) {
        match command {
            Some(command) => self.record(format!("page menu {tab} runs {command}")),
            None => self.record(format!("page menu {tab} closes")),
        }
    }

    fn print(&self, _: u64, _: &mut App) {}

    fn devtools(&self, tab: u64, what: i32, _: &mut App) {
        self.record(format!("devtools {tab} {what}"));
    }

    fn task_manager(&self, _: &mut App) {}

    fn start_trace(&self) -> bool {
        self.record("start trace".into());
        true
    }

    fn stop_trace(&self) {
        self.record("stop trace".into());
    }

    fn trace_note(&self, stop: Option<Rc<dyn Fn(&mut App)>>, _: &mut App) {
        self.record(format!("trace note {}", if stop.is_some() { "shown" } else { "hidden" }));
    }

    fn set_menus(&self, bar: &MenuBar, _: &mut App) {
        *self.menu_bar.borrow_mut() = bar.clone();
    }

    fn set_window_title(&self, title: &str, _: &mut App) {
        *self.window_title.borrow_mut() = title.to_string();
    }

    fn set_edit_titles(&self, undo: Option<&str>, redo: Option<&str>, _: &mut App) {
        *self.edit_titles.borrow_mut() = (undo.map(Into::into), redo.map(Into::into));
    }

    fn run_extension(&self, _: &str, _: Bounds<Pixels>, _: &mut App) -> bool {
        Default::default()
    }

    fn run_extension_command(&self, _: &str, _: &str, _: &mut App) -> bool {
        Default::default()
    }

    fn set_extension_pinned(&self, profile: u64, id: &str, pinned: bool, _: &mut App) {
        self.record(format!("pin {profile} {id} {pinned}"));
    }

    fn remove_extension(&self, _: u64, _: &str, _: &mut App) -> bool {
        Default::default()
    }

    fn turn_on_extension(&self, profile: u64, id: &str, _: &mut App) -> bool {
        self.record(format!("turn on {profile} {id}"));
        true
    }

    fn extension_crash_told(&self, profile: u64, id: &str, _: &mut App) {
        self.record(format!("crash told {profile} {id}"));
    }

    fn open_extension_options(&self, _: &str, _: &mut App) {}

    fn manage_extensions(&self, _: Option<&str>, _: &mut App) {}

    fn add_extension(&self, _: &mut App) {}

    fn lay_out_side_panel(&self, _: Bounds<Pixels>, _: &mut App) {}

    fn close_side_panel(&self, _: &mut App) {}

    fn has_side_panel(&self, _: &str) -> bool {
        false
    }

    fn toggle_side_panel(&self, _: &str, _: &mut App) {}

    fn set_side_panel_width(&self, _: f32, _: &mut App) {}

    fn view_history(&self, _: &mut App) {}
    fn new_note(&self, space: u64, beside: Option<u64>, _: &mut App) {
        self.notes.borrow_mut().push((space, beside));
    }

    fn open_page(&self, url: &str, _: &mut App) {
        self.opened_pages.borrow_mut().push(url.to_owned());
    }

    fn site_search_settings(&self, _: &mut App) {
        self.site_search_settings.set(self.site_search_settings.get() + 1);
    }

    fn clear_browsing_data(&self, _: &mut App) {
        self.clear_browsing_data.set(self.clear_browsing_data.get() + 1);
    }

    fn close_other_window(&self, _: &mut App) -> bool {
        Default::default()
    }

    fn show_space(&self, space: u64, _: &mut App) {
        self.shown.borrow_mut().push(space);
    }

    fn space_tab(&self, _: u64) -> u64 {
        Default::default()
    }

    // The Space it makes is always 7.
    fn create_space(&self, name: &str, _: u64, after: u64, _: &mut App) -> u64 {
        self.record(format!("create space {name} after {after}"));
        7
    }

    fn rename_space(&self, space: u64, name: &str, _: &mut App) {
        self.edit(format!("rename space {space} {name}"));
    }

    fn rename_tab(&self, tab: u64, name: &str, _: &mut App) {
        self.edit(format!("rename tab {tab} {name}"));
        self.names.borrow_mut().insert(tab, name.into());
    }

    fn set_icon(&self, tab: u64, icon: &str, _: &mut App) {
        self.edit(format!("icon {tab} {icon}"));
    }

    fn restore_sidebar(&self, time: u64, _: &mut App) {
        self.record(format!("restore sidebar {time}"));
    }

    fn delete_space(&self, space: u64, _: &mut App) -> bool {
        self.edit(format!("delete space {space}"));
        let mut deleted = self.core.deleted.borrow_mut();
        deleted.push(space);
        if deleted.len() > Core::KEPT {
            deleted.remove(0);
        }
        true
    }

    fn set_space_color(&self, space: u64, color: Option<u32>, _: &mut App) {
        self.edit(format!("color {space} {color:?}"));
        let color = color.map_or("none".to_string(), |color| format!("{color:06x}"));
        self.record(format!("space color {space} {color}"))
    }

    fn set_space_icon(&self, space: u64, icon: &str, _: &mut App) {
        self.edit(format!("space icon {space} {icon}"));
    }

    fn set_space_theme(&self, space: u64, theme: &str, _: &mut App) {
        let theme = if theme.is_empty() { "none" } else { theme };
        self.edit(format!("theme {space} {theme}"));
        self.record(format!("space theme {space} {theme}"))
    }

    fn set_space_profile(&self, space: u64, profile: u64, _: &mut App) {
        self.edit(format!("space {space} profile {profile}"));
    }

    // The profile it makes is always 9.
    fn create_profile(&self, name: &str, _: &mut App) -> u64 {
        self.edit(format!("create profile {name}"));
        9
    }

    fn rename_profile(&self, profile: u64, name: &str, _: &mut App) {
        self.edit(format!("rename profile {profile} {name}"));
    }

    fn account_host(&self) -> Option<Rc<dyn crate::account::Host>> {
        let account: Rc<dyn crate::account::Host> = self.account.clone();
        self.sync_server.get().then_some(account)
    }

    // The import's made-up Arc, so the first launch has a browser to import from, unless a test
    // says there's none.
    fn import_host(&self) -> std::rc::Rc<dyn crate::import::Host> {
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/src/import/fixture");
        let home = if self.no_browsers.get() { "/nonexistent" } else { fixture };
        let os = crate::import::sources::Os::Mac;
        let roots = crate::import::sources::Roots::under(std::path::Path::new(home), os);
        std::rc::Rc::new(crate::import::Detached(roots))
    }

    fn delete_profile(&self, _: u64, _: &mut App) {}

    fn context_menu(
        &self,
        entries: Vec<MenuEntry>,
        position: Point<Pixels>,
        in_reveal: bool,
        look: crate::menu::Look,
        answer: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        self.menu_looks.borrow_mut().push(look);
        self.menus_in_reveal.borrow_mut().push(in_reveal);
        self.menu(entries, position, None, in_reveal, answer, cx);
    }

    fn confirm(
        &self,
        message: String,
        detail: String,
        _: &'static str,
        confirmed: Box<dyn FnOnce(&mut App)>,
        _: &mut App,
    ) {
        *self.confirming.borrow_mut() = Some((message, detail, confirmed));
    }

    fn tell(
        &self,
        question: crate::dialog::Question,
        confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
        _: &mut App,
    ) -> bool {
        if self.dialog_busy.get() {
            return false;
        }
        let also = question.also.as_ref().map_or("", |(title, _)| title.as_ref());
        let (message, action, cancel) = (question.message, question.action, question.cancel);
        self.told.borrow_mut().push(format!("{message}: {action} / {cancel} / {also}"));
        *self.told_action.borrow_mut() = Some(confirmed);
        true
    }

    fn focus_ui(&self, _: &mut App) {
        self.record("focus ui".into())
    }

    fn hold_keyboard_for_page(&self, _: &mut App) {
        self.record("hold keyboard for page".into())
    }

    fn focus_page(&self, _: &mut App) {
        self.record("focus page".into())
    }

    fn lay_out_page(&self, _: Bounds<Pixels>, _: f32, _: &mut App) {
        self.page_events.borrow_mut().push("layout");
    }

    fn set_focus_color(&self, _: u32, _: &mut App) {}

    fn set_page_color(&self, _: u32, _: &mut App) {}

    fn set_window_light(&self, light: bool, _: &mut App) {
        self.window_light.set(Some(light));
    }

    fn capture_look(&self, tab: u64, look: crate::menu::Look, _: &mut App) {
        self.capture_look.set(Some((tab, look.light)));
    }

    fn set_appearance(&self, appearance: Appearance, _: &mut App) {
        self.appearance.set(Some(appearance));
    }

    fn page_moving(&self, moving: bool, _: &mut App) {
        self.page_events.borrow_mut().push(if moving { "moving" } else { "still" });
    }

    fn set_pane_corner(&self, _: f32, _: f32, _: &mut App) {}

    fn show_window_buttons(&self, _: bool, _: &mut App) {}

    fn window_button_geometry(&self, _: &mut App) -> Option<super::WindowButtonGeometry> {
        self.window_buttons.get()
    }

    fn press_window_button(&self, button: WindowButton, _: &mut App) {
        if let WindowButton::Zoom = button {
            self.record("window button zoom".into())
        }
    }

    fn start_window_move(&self, _: &mut App) {}

    fn window_maximized(&self) -> bool {
        false
    }

    fn window_full_screen(&self) -> bool {
        self.full_screen.get()
    }

    fn show_reveal(&self, _: f32, _: &mut App) {}

    fn saved_sidebar_width(&self) -> Option<f32> {
        Default::default()
    }

    fn set_sidebar_width(&self, _: f32, _: &mut App) {}

    fn hide_reveal(&self, _: &mut App) {}

    fn option_reveals_sidebar(&self) -> bool {
        !self.option_off.get()
    }

    fn detent(&self, _: &mut App) {
        self.detents.set(self.detents.get() + 1);
    }

    fn watch_control(&self, _: &mut App) {}

    fn copy(&self, text: &str, _: &mut App) {
        self.copied.borrow_mut().push(text.to_owned());
        self.record(format!("copy {text}"))
    }

    /// The host's clipboard holds what was copied last.
    fn pasted(&self, _: &App) -> Option<String> {
        self.copied.borrow().last().cloned()
    }

    fn share_services(&self, _: &str) -> Vec<SharedString> {
        self.share_services.borrow().clone()
    }

    fn share(&self, url: &str, _: &str, service: usize, _: &mut App) {
        self.record(format!("share {url} with {service}"))
    }

    fn has_share_sheet(&self) -> bool {
        self.share_sheet.get()
    }

    fn show_share_sheet(&self, tab: u64, url: &str, title: &str, _: Point<Pixels>, _: &mut App) {
        self.record(format!("share sheet {tab} {url} {title}"))
    }

    fn toast(&self, text: &str, _: Option<(&str, &str)>, _: Duration, _: &mut App) {
        self.toasts.borrow_mut().push(text.into());
    }

    fn toast_undo(&self, text: &str, undo: Rc<dyn Fn(&mut App)>, _: Duration, _: &mut App) {
        self.undo_toasts.borrow_mut().push(text.into());
        *self.undo_toast.borrow_mut() = Some(undo);
    }

    fn hide_undo_toast(&self, _: &mut App) {
        self.undo_toast.borrow_mut().take();
    }

    fn in_front(&self) -> bool {
        !self.behind.get()
    }

    fn toast_button(
        &self,
        text: &str,
        _: (Option<&'static str>, &'static str),
        run: Rc<dyn Fn(&mut App)>,
        _: Duration,
        _: &mut App,
    ) {
        self.toasts.borrow_mut().push(text.into());
        *self.toast_run.borrow_mut() = Some(run);
    }

    fn full_screen_note(&self, shown: bool, _: &mut App) {
        self.record(format!("full screen note {}", if shown { "shown" } else { "hidden" }))
    }

    fn hang_note(&self, tab: u64, shown: bool, _: &mut App) {
        self.record(format!("hang note {tab} {}", if shown { "shown" } else { "hidden" }))
    }

    fn tooltip(&self, tip: Option<(crate::tooltip::Tip, crate::tooltip::Anchor)>, _: &mut App) {
        match tip {
            Some((tip, anchor)) => {
                self.anchors.borrow_mut().push(anchor);
                self.record(format!("tooltip {} {}", tip.text, tip.keys.concat()))
            }
            None => self.record("tooltip none".into()),
        }
    }

    fn folder_preview(&self, ask: crate::folder_preview::Ask, _: &mut App) {
        self.record(ask.label());
        if let crate::folder_preview::Ask::Rest(rest) = ask {
            *self.folder_preview.borrow_mut() = Some(rest);
        }
    }

    fn hover_card(&self, hover: crate::hover_card::Hover, _: &mut App) {
        self.record(hover.label());
        if let crate::hover_card::Hover::Rest(rest) = hover {
            *self.card.borrow_mut() = Some(rest);
        }
    }

    fn card_picture(&self, tab: u64, _: &mut App) {
        self.record(format!("card picture {tab}"))
    }

    fn link_target(
        &self,
        plate: Option<(crate::link_target::Plate, Bounds<Pixels>, crate::link_target::Side)>,
        _: &mut App,
    ) {
        match plate {
            Some((plate, frame, side)) => {
                self.link_frame.set(Some(frame));
                let side = match side {
                    crate::link_target::Side::Left => "left",
                    crate::link_target::Side::Right => "right",
                };
                self.record(format!("link target {} {side}", plate.text()))
            }
            None => self.record("link target none".into()),
        }
    }

    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        _: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        _: &mut App,
    ) {
        self.command_bars.borrow_mut().push(field);
        self.bar_edits.borrow_mut().push(editing.map(|(tab, _)| tab));
        self.bar_open.set(true);
    }

    fn search_tabs(&self, tabs: Vec<BarTab>, _: &mut App) {
        self.searched_tabs.borrow_mut().push(tabs.iter().map(|tab| tab.tab).collect());
        self.bar_open.set(true);
    }

    fn command_bar_open(&self, _: &App) -> bool {
        self.bar_open.get()
    }

    /// A bar that follows opens a new tab, as one ⌘T opened.
    fn command_bar_follows(&self, tabs: Vec<BarTab>, _: Vec<BarTab>, _: &mut App) -> bool {
        let open = self.bar_open.get();
        if open {
            self.followed_tabs.borrow_mut().push(tabs.iter().map(|tab| tab.tab).collect());
        }
        open
    }

    fn split_bar(&self, _: u64, _: Bounds<Pixels>, _: Bounds<Pixels>, _: &mut App) {
        Default::default()
    }

    fn find(&self, _: u64, _: &mut App) {}

    fn site_controls(
        &self,
        tab: u64,
        place: crate::site_controls::Place,
        _: Size<Pixels>,
        extensions: Vec<crate::site_controls::Extension>,
        cx: &mut App,
    ) {
        self.site_controls.borrow_mut().push((tab, place));
        self.site_controls_extensions(tab, extensions, cx);
    }

    fn site_controls_extensions(
        &self,
        _: u64,
        extensions: Vec<crate::site_controls::Extension>,
        _: &mut App,
    ) {
        let ids = extensions.into_iter().map(|extension| extension.id).collect();
        self.site_controls_extensions.borrow_mut().push(ids);
    }

    fn capture(
        &self,
        tab: u64,
        frame: Bounds<Pixels>,
        _: f32,
        full_page: bool,
        _: bool,
        look: crate::menu::Look,
        _: &mut App,
    ) {
        let (x, y) = (f32::from(frame.origin.x), f32::from(frame.origin.y));
        let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
        let what = if full_page { "full page" } else { "portion" };
        let tone = if look.light { "light" } else { "dark" };
        self.record(format!("capture {what} {tab} at {x} {y} {width} {height} {tone}"));
    }

    fn page_picture(&self, tab: u64, request: u64, cx: &mut App) {
        self.record(format!("picture {tab}"));
        if let Some((shell, picture)) = self.pictures.borrow().clone() {
            super::picture_answered(vec![shell], request, Some(picture), &cx.to_async());
        }
    }

    fn settings(&self, _: &mut App) {}

    fn shortcuts(&self, _: &mut App) {}

    fn about(&self, _: &mut App) {}

    fn updates(&self) -> bool {
        self.updates.get()
    }

    fn check_for_updates(&self, _: &mut App) {
        self.checked_for_updates.set(self.checked_for_updates.get() + 1);
    }

    fn import(&self, _: &mut App) {}

    fn passwords(&self, space: u64, _: &mut App) {
        self.record(format!("passwords {space}"));
    }

    fn agent_activity(&self, _: &mut App) {
        self.record("agent activity".into());
    }

    fn agent_permissions(&self, _: u64, _: &App) -> super::AgentGrants {
        self.agent_permissions.borrow().clone()
    }

    fn revoke_agent(&self, space: u64, site: Option<SharedString>, _: &mut App) {
        let site = site.map_or("all".into(), |site| site.to_string());
        self.record(format!("revoke agent {space} {site}"));
    }

    fn disconnect_agent(&self, space: u64, _: &mut App) {
        self.record(format!("disconnect agent {space}"));
    }

    fn take_back_agent_tabs(&self, space: u64, _: &mut App) {
        self.record(format!("take back agent {space}"));
    }

    fn take_over(&self, agent: SharedString, tab: u64, _: &mut App) {
        self.record(format!("take over {agent} {tab}"));
    }

    fn hand_back(&self, tab: u64, _: &mut App) {
        self.record(format!("hand back {tab}"));
    }

    fn give_to_agent(&self, tab: u64, space: u64, _: &mut App) {
        self.record(format!("give {tab} to {space}"));
    }

    fn stop_agent(&self, agent: SharedString, _: &mut App) {
        self.record(format!("stop agent {agent}"));
    }

    fn find_next(&self, _: u64, _: bool, _: &mut App) {}

    fn jump_to_selection(&self, _: u64, _: &mut App) {}

    fn quit(&self, _: &mut App) {}

    fn new_window(&self, _: &mut App) {
        self.record("new window".into())
    }

    fn welcome_window(&self, _: &mut App) {
        self.record("welcome window".into())
    }

    fn is_default_browser(&self) -> bool {
        self.default_browser.get()
    }

    fn make_default_browser(&self, _: &mut App) {
        self.record("make default browser".into())
    }

    fn os(&self) -> crate::default_browser::Os {
        self.os.get().unwrap_or(crate::default_browser::Os::Mac)
    }

    fn crash_reports(&self, _: &App) -> Option<bool> {
        self.crash_reports.get()
    }

    fn set_crash_reports(&self, on: bool, _: &mut App) {
        self.crash_reports.set(Some(on));
        self.record(format!("crash reports {on}"))
    }

    fn agents(&self, _: &App) -> Option<bool> {
        self.agents.get()
    }

    fn set_agents(&self, on: bool, _: &mut App) {
        self.agents.set(Some(on));
        self.record(format!("agents {on}"))
    }

    fn close_window(&self, _: &mut App) {
        self.record("close window".into())
    }

    fn new_private_window(&self, _: &mut App) {}

    fn new_little_window(&self, home: u64, _: &mut App) {
        self.record(format!("new little window {home}"))
    }

    fn open_in_main_window(&self, tab: u64, home: u64, _: &mut App) {
        self.record(format!("open in main window {tab} {home}"))
    }

    fn open_tab_in_little(&self, tab: u64, _: &mut App) {
        self.record(format!("open tab in little {tab}"))
    }

    fn little_profile(&self, profile: u64, url: &str, _: &mut App) {
        self.record(format!("little profile {profile} {url}"))
    }
}

/// The account's core, recording what the account step asks of it.
#[derive(Default)]
pub(super) struct Account {
    pub(super) calls: RefCell<Vec<String>>,
}

impl crate::account::Host for Account {
    fn set_server(&self, server: &str) -> bool {
        self.calls.borrow_mut().push(format!("server {server}"));
        true
    }

    fn sign_in(&self) {
        self.calls.borrow_mut().push("sign in".into());
    }

    fn cancel(&self) {
        self.calls.borrow_mut().push("cancel".into());
    }

    fn sign_out(&self) {
        self.calls.borrow_mut().push("sign out".into());
    }

    fn recovery_words(&self) -> String {
        self.calls.borrow_mut().push("recovery words".into());
        "abandon ability able about above absent absorb abstract absurd abuse access accident \
         account accuse achieve acid acoustic acquire across act action actor actress actual"
            .into()
    }

    fn enter_recovery_words(&self, _: &str) -> bool {
        self.calls.borrow_mut().push("words".into());
        true
    }

    fn start_over(&self) {
        self.calls.borrow_mut().push("start over".into());
    }

    fn request_pairing(&self) {
        self.calls.borrow_mut().push("request pairing".into());
    }

    fn read_devices(&self) {
        self.calls.borrow_mut().push("read devices".into());
    }

    fn remove_device(&self, id: &str) {
        self.calls.borrow_mut().push(format!("remove {id}"));
    }

    fn copy_secret(&self, _: &str, _: Duration, _: &mut App) {
        self.calls.borrow_mut().push("copy secret".into());
    }

    fn confirm(
        &self,
        message: String,
        _: String,
        _: &'static str,
        _: crate::dialog::Return,
        _: Box<dyn FnOnce(&mut App)>,
        _: &mut gpui::Window,
        _: &mut App,
    ) {
        self.calls.borrow_mut().push(format!("ask {message}"));
    }
}
