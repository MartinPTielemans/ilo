//! What the shell asks of the browser, and what the browser tells it about a tab's page,
//! a split view and a Space's folders. The engine answers in main.rs, the preview with
//! made-up tabs in preview.rs.

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use gpui::{App, Bounds, Pixels, Point, RenderImage, SharedString, Size, point, px, size};

use crate::theme::Appearance;
use crate::tokens::shell::{WINDOW_BUTTON_SIZE, WINDOW_BUTTON_Y};

use super::{Answer, BarTab, DownloadAction, MenuBar, MenuEntry, PageReply, ShownPageDialog};

/// What the shell asks of the browser. The browser answers by updating the shell afterwards, never
/// during the call.
pub trait Browser {
    fn select(&self, tab: u64, cx: &mut App);
    /// Closes a Today tab. A pinned tab stays, and only its page goes. A tab in a split view
    /// leaves it, unless `split` closes the rest of the split with it.
    fn close(&self, tab: u64, split: bool, cx: &mut App);
    /// Closes the Today tabs below `tab`, showing `tab` if one of them showed.
    fn close_below(&self, tab: u64, cx: &mut App);
    /// Opens a copy of `tab` with its history, under it in Today, and shows it. Returns its id, or
    /// 0.
    fn duplicate(&self, tab: u64, cx: &mut App) -> u64;
    /// Moves `tab` to `space`. The window goes with it if it shows it. Into another profile's Space
    /// it goes as a new tab, whose page loads there again.
    fn move_to_space(&self, tab: u64, space: u64, cx: &mut App);
    /// Whether to warn before moving a tab to another profile's Space, which the UI does once.
    fn warn_moving_profiles(&self, cx: &mut App) -> bool;
    fn set_warn_moving_profiles(&self, warn: bool, cx: &mut App);
    /// Pins `tab` at the bottom of its Space's pinned tabs, or unpins it to the top of Today.
    fn set_pinned(&self, tab: u64, pinned: bool, cx: &mut App);
    /// Makes `tab` the favorite at `index` of the favorites without it, else the last, out of its
    /// Space, or moves it there among them.
    fn move_to_favorites(&self, tab: u64, index: usize, cx: &mut App);
    /// Moves `tab` to `index` in its Space's Today tabs, or in its pinned tabs, inside `folder` if
    /// it isn't 0, pinning or unpinning it, with the rest of its split view. `index` counts tabs,
    /// not rows, without the split's, and folders directly in the pinned tabs or `folder` count
    /// one. A favorite goes to the Space shown.
    fn move_tab(&self, tab: u64, pinned: bool, folder: u64, index: usize, cx: &mut App);
    /// Opens tabs loading `urls`, in their order, at `index` in the Space shown's Today tabs, or
    /// in its pinned tabs, inside `folder` if it isn't 0, counted as `move_tab` counts. Shows the
    /// first and returns it, or 0 if none opened.
    fn open_at(&self, urls: &[String], pinned: bool, folder: u64, index: usize, cx: &mut App)
    -> u64;
    /// Opens tabs for `urls` where `open_at` would, each loading once it's shown, and shows none,
    /// as a folder from the desktop's files come in.
    fn open_behind(&self, urls: &[String], pinned: bool, folder: u64, index: usize, cx: &mut App);
    /// Makes an open folder named `name` at `index` in `folder`, else in the pinned tabs of
    /// `space`, counted as `move_tab` counts, else at the bottom, and returns it.
    fn create_folder(&self, space: u64, name: &str, folder: u64, index: usize, cx: &mut App)
    -> u64;
    fn rename_folder(&self, folder: u64, name: &str, cx: &mut App);
    /// Gives `folder` `icon`, an emoji, or takes its icon away with an empty one.
    fn set_folder_icon(&self, folder: u64, icon: &str, cx: &mut App);
    /// Makes a copy of `folder` under it, with a copy of each tab and folder in it, the tabs
    /// unloaded, and returns it, or 0.
    fn duplicate_folder(&self, folder: u64, cx: &mut App) -> u64;
    fn set_folder_open(&self, folder: u64, open: bool, cx: &mut App);
    /// Collapses `space`'s pinned rows, or expands them.
    fn set_pinned_open(&self, space: u64, open: bool, cx: &mut App);
    /// Moves `folder`, with what's in it, to `index` in `to`, else in the pinned tabs, counted as
    /// `move_tab` counts.
    fn move_folder(&self, folder: u64, to: u64, index: usize, cx: &mut App);
    /// Deletes `folder`, leaving what was in it where it was.
    fn delete_folder(&self, folder: u64, cx: &mut App);
    /// Takes `tab` out of its split view.
    fn separate(&self, tab: u64, cx: &mut App);
    /// Moves `tab` to `index` in its split view, counted from the left or the top.
    fn move_in_split(&self, tab: u64, index: usize, cx: &mut App);
    /// Stacks the pages of `tab`'s split view top to bottom, or puts them side by side.
    fn set_split_stacked(&self, tab: u64, stacked: bool, cx: &mut App);
    /// Gives the pages of `tab`'s split view `shares` of it, in their order, or even shares.
    fn set_split_shares(&self, tab: u64, shares: Option<&[f64]>, cx: &mut App);
    /// Gives `tab`'s page its split's card, the other pages kept at 220 pt however wide it is, or
    /// ends that and evens them.
    fn set_split_expanded(&self, tab: u64, expanded: bool, cx: &mut App);
    /// Puts `tab` in a split view with `with`, at its left end if `left`, else its right end, and
    /// shows it there.
    fn split_with(&self, tab: u64, with: u64, left: bool, cx: &mut App);
    /// Opens a tab loading `url` in a split view with `with`, at its left end if `left`, else its
    /// right end, stacking a new split if `stacked`, and shows it there.
    fn open_split_at(&self, url: &str, with: u64, left: bool, stacked: bool, cx: &mut App);
    /// Shows the window over the page that draws `Shell::over_page`, drawing it again, or hides
    /// it.
    fn show_drop_targets(&self, shown: bool, cx: &mut App);
    /// Loads a pinned tab's pinned URL, unless it's there already.
    fn back_to_pinned_url(&self, tab: u64, cx: &mut App);
    /// Mutes `tab`, or unmutes it.
    fn set_muted(&self, tab: u64, muted: bool, cx: &mut App);
    /// Has the page sharing a tab share `tab` in its place.
    fn share_instead(&self, tab: u64, cx: &mut App);
    /// Plays, pauses or skips `tab`'s media.
    fn media(&self, tab: u64, action: MediaAction, cx: &mut App);
    /// Opens `tab`'s Cast dialog, which the core reports with `Shell::set_cast`.
    fn cast(&self, tab: u64, cx: &mut App);
    /// Closes the Cast dialog.
    fn cast_close(&self, cx: &mut App);
    /// A click on the device `id` in `tab`'s Cast dialog: casts to it, or stops it casting.
    fn cast_to(&self, tab: u64, id: SharedString, cx: &mut App);
    /// Closes the Today tabs of the Space shown, but the one shown, unless it shows them all, and
    /// returns the id of the clear, or 0 if it closed none.
    fn clear(&self, cx: &mut App) -> u64;
    /// Brings back the tabs the window's clear `clear` closed, showing the one shown it closed,
    /// and returns how many, 0 if that clear is gone.
    fn undo_clear(&self, clear: u64, cx: &mut App) -> usize;
    /// Brings back the Space `space`, which was deleted, and shows it. Returns whether it did.
    fn undo_delete_space(&self, space: u64, cx: &mut App) -> bool;
    /// Reopens the Today tab closed last, and shows it.
    fn reopen_closed_tab(&self, cx: &mut App);
    /// Opens the link preview the user closed again, at its page, over its tab, whose window's
    /// page is at `page` with corners of that radius, growing from its link.
    fn reopen_preview(
        &self,
        closed: &crate::link_preview::Closed,
        page: (Bounds<Pixels>, f32),
        cx: &mut App,
    );
    /// Closes the link preview over `over`, if one shows, as Redo does. Returns whether it did.
    fn close_preview(&self, over: u64, cx: &mut App) -> bool;
    /// Reopens the Today tab that had the id `closed` when it closed, at its place, and returns its
    /// new id, or 0 if it isn't among the closed tabs.
    fn reopen_tab(&self, closed: u64, cx: &mut App) -> u64;
    /// The URL a pinned tab or a favorite goes back to, or empty for another tab.
    fn pinned_url(&self, tab: u64) -> String;
    /// Gives a pinned tab or a favorite the URL it goes back to, which unpinning forgets.
    fn set_pinned_url(&self, tab: u64, url: &str, cx: &mut App);
    /// The URL the address bar goes to for `text` typed in it, or None if it names nothing.
    fn typed_url(&self, text: &str) -> Option<String>;
    /// The name the user gave the tab, or empty while it shows its page's title.
    fn tab_name(&self, tab: u64) -> String;
    /// Puts back what the archive removal `removal` took out, and returns how many tabs, 0 if it
    /// is gone. The removals are the core's, shared by its windows, so each asks for its own.
    fn undo_archive_remove(&self, removal: u64, cx: &mut App) -> usize;
    /// Reopens the archived tab `archived` at the top of the Today tabs of the Space shown, and
    /// shows it.
    fn restore_archived(&self, archived: u64, cx: &mut App);
    /// Takes the archived tab `archived` out of the archive, and returns the removal's id, or 0.
    fn remove_archived(&self, archived: u64, cx: &mut App) -> u64;
    /// Empties the archive, and returns the removal's id, or 0 if it was empty.
    fn clear_archive(&self, cx: &mut App) -> u64;
    /// Does `action` to the download.
    fn act_on_download(&self, download: u64, action: DownloadAction, cx: &mut App);
    /// Takes the downloads that aren't running or paused off the list, keeping their files.
    fn clear_downloads(&self, cx: &mut App);
    /// Looks up the icon the system shows for the download's file, and calls
    /// `Shell::set_download_icon`.
    fn download_icon(&self, download: u64, cx: &mut App);
    /// Looks up a preview of the download's file at `path`, a thumbnail of an image, else the
    /// system's icon for it, and calls `Shell::set_download_preview`.
    fn file_preview(&self, download: u64, path: &str, cx: &mut App);
    /// Plays the video at `path` for Media's tile `id`, `side` pixels square, without sound and
    /// from its start again at its end. False where the platform plays none.
    fn open_video(&self, id: u64, path: &str, side: u32, cx: &mut App) -> bool;
    /// The frame tile `id`'s video shows now, if it's new since the last.
    fn video_frame(&self, id: u64, cx: &mut App) -> Option<Arc<RenderImage>>;
    /// Stops the video tile `id` plays.
    fn close_video(&self, id: u64, cx: &mut App);
    /// Lists the captures saved to the Library, away from the main thread, and calls
    /// `Shell::set_captures`.
    fn list_captures(&self, cx: &mut App);
    /// What the Library button's hover lists, as `recent_downloads::HOVER_IMAGES`' and its
    /// fellows' bits, as the user picked them from its menu.
    fn hover_kinds(&self, cx: &mut App) -> u32;
    fn set_hover_kinds(&self, kinds: u32, cx: &mut App);
    /// Moves the file at `path` to the system's Trash, and says whether it's gone, as it is if it
    /// was gone already.
    fn trash(&self, path: &str, cx: &mut App) -> bool;
    /// Starts the system's drag of the file at `path` from the pointer the left button holds, and
    /// calls `Shell::file_drag_ended` once it's let go. False if it didn't start.
    fn drag_file(&self, path: &str, cx: &mut App) -> bool;
    /// Looks up the favicon the browser has for `url`, and calls `Shell::set_page_favicon`.
    fn page_favicon(&self, url: &str, cx: &mut App);
    /// The starts of today, yesterday, this week and last week in the user's calendar, in
    /// milliseconds since the Unix epoch.
    fn day_starts(&self) -> [f64; 4];
    fn go_back(&self, tab: u64, cx: &mut App);
    fn go_forward(&self, tab: u64, cx: &mut App);
    /// Lists the pages back from `tab`'s, or forward of it, and calls `Shell::show_history`.
    fn tab_history(&self, tab: u64, forward: bool, cx: &mut App);
    /// Goes to the page `offset` from `tab`'s in its history, as `tab_history` numbers them.
    fn go_to_history(&self, tab: u64, offset: i32, cx: &mut App);
    /// Opens a copy of `tab` at the page `offset` from its own, or 0 for its page again, under it
    /// and not shown.
    fn open_history(&self, tab: u64, offset: i32, cx: &mut App);
    fn reload(&self, tab: u64, bypass_cache: bool, cx: &mut App);
    /// Deletes the cookies, or else the cache, the profile keeps for the site of the tab's page,
    /// then reloads it.
    fn clear_site(&self, tab: u64, cookies: bool, cx: &mut App);
    /// Turns Developer Mode on or off for the site of the tab's page, in its profile. The user's
    /// view: agents get none.
    fn set_developer_mode(&self, tab: u64, on: bool, cx: &mut App);
    /// Answers what the tab's page asks (Shell::set_question), if it still asks.
    fn answer_permission(&self, tab: u64, answer: Answer, cx: &mut App);
    /// Answers what an agent asks (Shell::set_agent_question), if it still asks.
    fn answer_agent(&self, question: u64, answer: crate::agents::Answer, cx: &mut App);
    /// Asks the dialog a tab's page asks over the tab's card, with the corners of its radius, or
    /// takes back the one it asks (None).
    fn show_page_dialog(&self, shown: Option<ShownPageDialog>, cx: &mut App);
    /// Answers the dialog `id` of `tab`'s page (Shell::set_page_dialog), if it still asks.
    fn answer_page_dialog(&self, tab: u64, id: u64, reply: PageReply, cx: &mut App);
    /// Brings back the tab's offer to save or update its login, closed by showing another tab, as
    /// its key does. The user's alone: agents get no path to it.
    fn show_offer(&self, _tab: u64) {}
    fn stop(&self, tab: u64, cx: &mut App);
    /// Zooms the tab's page in (1), out (-1) or to 100 % (0), for its site.
    fn zoom(&self, tab: u64, step: i32, cx: &mut App);
    /// Opens the tab's source in a tab under it.
    fn view_source(&self, tab: u64, cx: &mut App);
    /// Asks where to save the tab's page, and saves it there.
    fn save_page(&self, tab: u64, cx: &mut App);
    /// Asks where to save `markdown`, which holds `links` links, offering `name`, and saves it
    /// there: a Space's from its menu, or every Space's from Settings.
    fn save_markdown(&self, name: &str, markdown: String, links: usize, cx: &mut App);
    /// Shows print preview for the tab's page.
    fn print(&self, tab: u64, cx: &mut App);
    /// Opens or closes the tab's DevTools (0), opens or closes them at the console (1), or opens
    /// them picking an element (2).
    fn devtools(&self, tab: u64, what: i32, cx: &mut App);
    /// Opens Chrome's Task Manager, with the page shown selected.
    fn task_manager(&self, cx: &mut App);
    /// Starts recording a trace of the whole browser, and returns false if one is recording
    /// already. The user's alone: agents get no tool for it.
    fn start_trace(&self) -> bool;
    /// Stops the trace, which comes back to `trace::recorded`.
    fn stop_trace(&self);
    /// Shows that a trace is recording in the toast, with Stop, which runs `stop`, until it's
    /// pressed or `None` takes it away. Another toast shows in its place only for its own time.
    fn trace_note(&self, stop: Option<Rc<dyn Fn(&mut App)>>, cx: &mut App);
    /// Makes the menu bar show `bar`, if the window is the key one.
    fn set_menus(&self, bar: &MenuBar, cx: &mut App);
    /// Titles the window `title`, which the Window menu and Mission Control list it by.
    fn set_window_title(&self, title: &str, cx: &mut App);
    /// Runs extension `id`'s action on the tab shown, its popup hanging from `anchor` in the
    /// window, and returns whether the extension is turned on in the tab's profile.
    fn run_extension(&self, id: &str, anchor: Bounds<Pixels>, cx: &mut App) -> bool;
    /// Runs extension `id`'s keyboard command `name` on the tab shown, and returns whether the
    /// extension is turned on in the tab's profile and listening.
    fn run_extension_command(&self, id: &str, name: &str, cx: &mut App) -> bool;
    /// Pins extension `id`'s action in `profile`, or unpins it.
    fn set_extension_pinned(&self, profile: u64, id: &str, pinned: bool, cx: &mut App);
    /// Removes extension `id` from `profile`, with the data it keeps, and returns whether it went.
    fn remove_extension(&self, profile: u64, id: &str, cx: &mut App) -> bool;
    /// Turns extension `id` on again in `profile`, and returns whether it's on.
    fn turn_on_extension(&self, profile: u64, id: &str, cx: &mut App) -> bool;
    /// Tells the core the user was told it turned extension `id` off in `profile` as it crashed.
    fn extension_crash_told(&self, profile: u64, id: &str, cx: &mut App);
    /// Opens an extension's options page, `url`, in a new tab, and shows it.
    fn open_extension_options(&self, url: &str, cx: &mut App);
    /// Opens Chrome's extensions page in a new tab, at extension `id`'s details if there's one,
    /// and shows it.
    fn manage_extensions(&self, id: Option<&str>, cx: &mut App);
    /// Opens the Web Store in a new tab, and shows it.
    fn add_extension(&self, cx: &mut App);
    /// Lays the side panel's page out at `frame` in the window, under its card's header, or hides
    /// it for an empty frame.
    fn lay_out_side_panel(&self, frame: Bounds<Pixels>, cx: &mut App);
    /// Closes the side panel shown.
    fn close_side_panel(&self, cx: &mut App);
    /// Whether extension `id` has a side panel for the tab shown.
    fn has_side_panel(&self, id: &str) -> bool;
    /// Opens extension `id`'s side panel for the tab shown, or closes it if it shows.
    fn toggle_side_panel(&self, id: &str, cx: &mut App);
    /// Keeps the side panel's width in the profile of the tab shown.
    fn set_side_panel_width(&self, width: f32, cx: &mut App);
    /// Opens Chrome's history page in a new tab, and shows it.
    fn view_history(&self, cx: &mut App);
    /// Opens `url`, one of the app's own web pages such as its help, in a new tab, and shows it.
    fn open_page(&self, url: &str, cx: &mut App);
    /// Opens a new document in the notes service `space`'s profile picks, in a tab at the top of
    /// its Today tabs, as a new tab opens, or in a split view with `beside`, on its right.
    fn new_note(&self, space: u64, beside: Option<u64>, cx: &mut App);
    /// Opens Chromium's search engines page in a tab of the Space shown, for its profile's site
    /// searches. The user's: agents open no pages outside http(s) (AGENTS.md).
    fn site_search_settings(&self, cx: &mut App);
    /// Opens Chrome's Clear browsing data dialog in a tab of the Space shown, for its profile. The
    /// user's, as site_search_settings is.
    fn clear_browsing_data(&self, cx: &mut App);
    /// Closes the window with the keyboard if it's another of the app's, such as Chrome's Task
    /// Manager, and says whether it did.
    fn close_other_window(&self, cx: &mut App) -> bool;
    fn show_space(&self, space: u64, cx: &mut App);
    /// The tab the window shows when it shows `space`, or the one it shows, else 0.
    fn space_tab(&self, space: u64) -> u64;
    /// Makes an empty Space named `name` right after `after`, else after the others, in `profile`
    /// if it isn't 0, and returns it.
    fn create_space(&self, name: &str, profile: u64, after: u64, cx: &mut App) -> u64;
    fn rename_space(&self, space: u64, name: &str, cx: &mut App);
    /// Names a tab, which shows `name` for its title from then on, or gives it back its page's
    /// title for an empty one.
    fn rename_tab(&self, tab: u64, name: &str, cx: &mut App);
    /// Gives a tab `icon`, an emoji, to show in place of its page's favicon, kept as its name is,
    /// or gives it back its favicon for an empty one.
    fn set_icon(&self, tab: u64, icon: &str, cx: &mut App);
    /// Deletes a Space, closing its tabs, and returns whether it will, as the last can't go.
    fn delete_space(&self, space: u64, cx: &mut App) -> bool;
    /// Brings back the sidebar of the profile of the Space the window shows from its backup taken
    /// at `time`, in milliseconds since 1970, keeping the one it has as a backup.
    fn restore_sidebar(&self, time: u64, cx: &mut App);
    /// Gives a Space a colour, 0xRRGGBB, or none, for the system's accent colour.
    fn set_space_color(&self, space: u64, color: Option<u32>, cx: &mut App);
    /// Gives a Space `icon`, an emoji or a glyph's `glyph:` name (`space_icon`), to show in place
    /// of its dot and before its name, or takes its icon away for an empty one.
    fn set_space_icon(&self, space: u64, icon: &str, cx: &mut App);
    /// Gives a Space `theme`, as `theme::Theme` writes it, which the core keeps for the UI, or
    /// takes it away for an empty one. Its main colour goes with `set_space_color`.
    fn set_space_theme(&self, space: u64, theme: &str, cx: &mut App);
    /// Puts an empty Space in `profile`.
    fn set_space_profile(&self, space: u64, profile: u64, cx: &mut App);
    /// Makes a profile named `name`, and returns it. It's listed once it's made.
    fn create_profile(&self, name: &str, cx: &mut App) -> u64;
    /// Names `profile` `name`.
    fn rename_profile(&self, profile: u64, name: &str, cx: &mut App);
    /// What the import view (crate::import) reads and adds through, for the first launch's step.
    fn import_host(&self) -> Rc<dyn crate::import::Host>;
    /// What Settings › Sync's view (crate::account) asks the core through, for the first launch's
    /// account step; none in a build that names no sync server, and in the preview.
    fn account_host(&self) -> Option<Rc<dyn crate::account::Host>> {
        None
    }
    /// Deletes a profile no Space or favorite is in, with everything the browser keeps in it.
    fn delete_profile(&self, profile: u64, cx: &mut App);
    /// Shows ilo's own menu of `items` at `position` in the window, or in the reveal's panel if
    /// `in_reveal`, in `look`, as a right-click there does, or with the item marked `over` on
    /// `position`, its title's top left there, as a pop-up button's menu does. Then calls `chosen`
    /// once it closes, with the index of the item chosen, if any.
    fn context_menu(
        &self,
        items: Vec<MenuEntry>,
        position: Point<Pixels>,
        in_reveal: bool,
        look: crate::menu::Look,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    );
    /// Closes the tab's page menu (`Shell::show_page_menu`), running its row with `command` first,
    /// or none.
    fn close_page_menu(&self, tab: u64, command: Option<i32>, cx: &mut App);
    /// Asks whether to do what `message` asks, with `detail` under it, and calls `confirmed` if the
    /// user chooses `action` over Cancel.
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        confirmed: Box<dyn FnOnce(&mut App)>,
        cx: &mut App,
    );
    /// Asks `question` in the browser's dialog, and calls `confirmed` with what its fields hold if
    /// the user chooses its action, once the dialog has gone. Returns whether it asks: not while it
    /// asks another.
    fn tell(
        &self,
        question: crate::dialog::Question,
        confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
        cx: &mut App,
    ) -> bool;
    /// Gives the UI the keyboard, from the page if the page has it.
    fn focus_ui(&self, cx: &mut App);
    /// With no page shown, gives the UI the keyboard until a page shows, which then has it, as the
    /// Mac's window holds the keys for the next page.
    fn hold_keyboard_for_page(&self, cx: &mut App) {
        self.focus_ui(cx)
    }
    /// Gives the page shown the keyboard, from the UI if it has it, now or once a page still
    /// loading has a view, unless the UI takes it back first.
    fn focus_page(&self, cx: &mut App);
    /// Lays the page out at `frame` in the window, with corners of `radius`
    /// (`Shell::page_frame`). A split view's pages share it.
    fn lay_out_page(&self, frame: Bounds<Pixels>, radius: f32, cx: &mut App);
    /// The colour of the ring inside a split view's focused page, 0xRRGGBB.
    fn set_focus_color(&self, color: u32, cx: &mut App);
    /// What the window's pages show until their first frame, 0xRRGGBB.
    fn set_page_color(&self, color: u32, cx: &mut App);
    /// Draws the window's material light, or dark, under the UI's tint.
    fn set_window_light(&self, light: bool, cx: &mut App);
    /// Keeps View › Appearance's choice for the next launch, and gives it the app's native parts
    /// and its pages, which follow it as they follow the system's.
    fn set_appearance(&self, appearance: Appearance, cx: &mut App);
    /// Whether the shell is moving the page's frame on its own, a step at a time. A page that
    /// hasn't drawn at its new size yet shows its last frame stretched to fill the view, rather
    /// than at its old size in the corner, so its right edge and scrollbar stay with the frame's.
    fn page_moving(&self, moving: bool, cx: &mut App);
    /// Keeps the find bar and the zoom HUD at the top right corner of a split view's focused page,
    /// `right` points further in from the window's right edge and `top` further down.
    fn set_pane_corner(&self, right: f32, top: f32, cx: &mut App);
    /// Shows the window's close, minimize and zoom buttons, or hides them.
    fn show_window_buttons(&self, shown: bool, cx: &mut App);
    /// Does what a click on the window's close, minimize or zoom button does.
    fn press_window_button(&self, button: WindowButton, cx: &mut App);
    /// The window's own buttons as the system draws them, for the copies the UI draws in their
    /// place, once it can say: the Mac's. Elsewhere the UI draws the only ones, at
    /// `WindowButtonGeometry::default`.
    fn window_button_geometry(&self, _cx: &mut App) -> Option<WindowButtonGeometry> {
        None
    }
    /// On Linux, has the browser window follow the pointer, on a drag's first move over the
    /// window's background, as GPUI's own move can't: its window is inside the browser window.
    fn start_window_move(&self, cx: &mut App);
    /// On Linux, whether the browser window is maximized, which the third window button the UI
    /// draws shows. False on the Mac, where AppKit draws that button.
    fn window_maximized(&self) -> bool;
    /// On Linux, whether the browser window is full screen, as the window manager or a page can
    /// put it. False on the Mac, where AppKit says when it goes and comes back.
    fn window_full_screen(&self) -> bool;
    /// Shows the sidebar reveal's window over the page, `width` points wide from the browser
    /// window's left edge and as tall as it. It calls `Shell::pointer_at` as the pointer moves
    /// anywhere in the browser's windows until it's hidden.
    fn show_reveal(&self, width: f32, cx: &mut App);
    /// The docked sidebar's width the last launch kept, if it kept one.
    fn saved_sidebar_width(&self) -> Option<f32>;
    /// Keeps the docked sidebar's width for the next launch.
    fn set_sidebar_width(&self, width: f32, cx: &mut App);
    fn hide_reveal(&self, cx: &mut App);
    /// Whether ⌥ pressed alone brings the hidden sidebar out, as the user chose.
    fn option_reveals_sidebar(&self) -> bool;
    /// A detent on the trackpad, as a swipe's nearest Space changes.
    fn detent(&self, cx: &mut App);
    /// Calls `Shell::control_released` once the user lets go of ⌃.
    fn watch_control(&self, cx: &mut App);
    /// Puts `text` on the clipboard.
    fn copy(&self, text: &str, cx: &mut App);
    /// The clipboard's text, from where `copy` puts it, or None if it holds none.
    fn pasted(&self, cx: &App) -> Option<String>;
    /// Whether the platform can open a page in Safari, which plays the protected video ilo can't.
    /// Where it can't, the UI offers no button for it.
    fn can_open_in_safari(&self) -> bool {
        false
    }
    /// Opens `url` in Safari, leaving the tab as it is.
    fn open_in_safari(&self, _url: &str, _cx: &mut App) {}
    /// Whether the platform has Safari's engine for a tab's page, which plays the protected video
    /// Chromium can't: macOS 14 or later. Where it hasn't, the note offers Safari instead.
    fn can_use_system_engine(&self) -> bool {
        false
    }
    /// Shows `tab`'s page in Safari's engine, in the tab, and has its profile keep the site for it.
    /// The user's: agents get none.
    fn use_system_engine(&self, _tab: u64, _cx: &mut App) {}
    /// Where Google's Widevine module is, which the core offers on Linux alone.
    fn widevine(&self) -> crate::widevine::Widevine {
        crate::widevine::Widevine::None
    }
    /// Downloads Google's Widevine module from Google. The user's yes: agents get none.
    fn download_widevine(&self, _cx: &mut App) {}
    /// Quits and starts again, bringing back the windows and their Spaces.
    fn restart(&self, _cx: &mut App) {}
    /// What PlayReady, which Windows plays protected video with, needs that the PC may lack.
    fn playready_needs(&self) -> super::PlayReady {
        super::PlayReady::None
    }
    /// The names of the system's share services for the page at `url`, as its share menu lists
    /// them.
    fn share_services(&self, url: &str) -> Vec<SharedString>;
    /// Shares the page at `url`, titled `title`, with the service at `service` in
    /// `share_services`.
    fn share(&self, url: &str, title: &str, service: usize, cx: &mut App);
    /// Whether the system has a share sheet of its own, which Windows' and the Mac's are, for
    /// when it lists no services the menu can show.
    fn has_share_sheet(&self) -> bool {
        false
    }
    /// Opens the system's share sheet for the page at `url`, titled `title`, in `tab`'s window,
    /// hanging from `at` where the system places it.
    fn show_share_sheet(
        &self,
        _tab: u64,
        _url: &str,
        _title: &str,
        _at: Point<Pixels>,
        _cx: &mut App,
    ) {
    }
    /// The user closed the tab's protected-media note: the profile keeps it from showing again at
    /// the site of its page, and Site Controls offer Safari's engine there instead.
    fn close_media_note(&self, _tab: u64, _cx: &mut App) {}
    /// Shows `text` in the toast at the window's top right for `time`, then `key`'s chip and the
    /// words after it if there's a key.
    fn toast(&self, text: &str, key: Option<(&str, &str)>, time: Duration, cx: &mut App);
    /// Shows `text` in the toast with an Undo button and ⌘Z's chip after it, which runs `undo` as
    /// the key does, for `time` and for as long as the pointer is over it.
    fn toast_undo(&self, text: &str, undo: Rc<dyn Fn(&mut App)>, time: Duration, cx: &mut App);
    /// Whether this window is the browser window in front, over which the toast shows.
    fn in_front(&self) -> bool;
    /// Shows `text` in the toast with a button after it, its label and its name in the logs, which
    /// runs `run`, for `time` and for as long as the pointer is over it. With no label, the toast
    /// itself is the button.
    fn toast_button(
        &self,
        text: &str,
        button: (Option<&'static str>, &'static str),
        run: Rc<dyn Fn(&mut App)>,
        time: Duration,
        cx: &mut App,
    );
    /// Takes the toast away if it has an Undo button, as the undo it offered has been done.
    fn hide_undo_toast(&self, cx: &mut App);
    /// Gives the Edit menu's Undo and Redo the titles of what they'd undo and redo, plain Undo and
    /// Redo for `None`, which a page's own undo and redo are.
    fn set_edit_titles(&self, undo: Option<&str>, redo: Option<&str>, cx: &mut App);
    /// Shows the note on how to leave full screen in the toast, or takes it away if it shows.
    fn full_screen_note(&self, shown: bool, cx: &mut App);
    /// Shows the note that `tab`'s page stopped answering in the toast, with Wait and Reload, until
    /// one is pressed or it's taken away (`shown` false).
    fn hang_note(&self, tab: u64, shown: bool, cx: &mut App);
    /// Shows `tip` in a tooltip after its delay, hung from `anchor` in the window, or at once if
    /// one shows; `None` ends it. The app does the same on the window losing the keyboard.
    fn tooltip(&self, tip: Option<(crate::tooltip::Tip, crate::tooltip::Anchor)>, cx: &mut App);
    /// Tells a tab's hover card the pointer rests on its row or tile, or left it, or ends it
    /// (crate::hover_card). The app ends it too on the window losing the keyboard.
    fn hover_card(&self, hover: crate::hover_card::Hover, cx: &mut App);
    /// Tells a closed folder's preview the pointer rests on its row, or left it, or ends it
    /// (crate::folder_preview).
    fn folder_preview(&self, ask: crate::folder_preview::Ask, cx: &mut App);
    /// Asks for a small picture of `tab`'s page, as it was last drawn, for its hover card, which
    /// the browser keeps for it. The user's: agents get none, as with the page's picture.
    fn card_picture(&self, tab: u64, cx: &mut App);
    /// Shows `plate`, the address of the link the pointer is on, at `frame` in the window, in
    /// `side` of the card, after its delay or at once if one shows or just went; or moves it there
    /// if it shows. `None` ends it.
    fn link_target(
        &self,
        plate: Option<(crate::link_target::Plate, Bounds<Pixels>, crate::link_target::Side)>,
        cx: &mut App,
    );
    /// Opens the command bar for a new tab or on `editing`'s URL, with `tabs` to list before
    /// anything is typed: over `field`, the URL field's box in the window, else centred over it.
    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        tabs: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        cx: &mut App,
    );
    /// Opens the command bar centred for Search Tabs, listing only `tabs` and filtering them by
    /// what's typed, to switch to one.
    fn search_tabs(&self, tabs: Vec<BarTab>, cx: &mut App);
    /// Whether the command bar shows, which stays true through a click outside it until the app
    /// calls `Shell::command_bar_dismissed`, so a click on what opens it can tell it has closed it.
    fn command_bar_open(&self, cx: &App) -> bool;
    /// The window shows another Space: the command bar, if it shows over this window, lists
    /// `tabs` from it, or `open_tabs` for Search Tabs, and its profile's suggestions (decision 5).
    /// Returns whether its pick opens a new tab, for New Tab to keep its plate.
    fn command_bar_follows(
        &self,
        tabs: Vec<BarTab>,
        open_tabs: Vec<BarTab>,
        cx: &mut App,
    ) -> bool;
    /// Opens the command bar over `pane`, a new pane in the card at `card`, for a page to show
    /// there in a split view with `with`.
    fn split_bar(&self, with: u64, pane: Bounds<Pixels>, card: Bounds<Pixels>, cx: &mut App);
    /// Opens the tab's find bar with the keyboard.
    fn find(&self, tab: u64, cx: &mut App);
    /// Opens the Site Controls for the site of the tab's page, or closes them if they show:
    /// hanging from `place` in a window of `window`'s size, listing `extensions`.
    fn site_controls(
        &self,
        tab: u64,
        place: crate::site_controls::Place,
        window: Size<Pixels>,
        extensions: Vec<crate::site_controls::Extension>,
        cx: &mut App,
    );
    /// The extensions the Site Controls list, if they show for the tab, as the core reported them.
    fn site_controls_extensions(
        &self,
        tab: u64,
        extensions: Vec<crate::site_controls::Extension>,
        cx: &mut App,
    );
    /// Starts capture mode over the tab's page, at `frame` in the window with corners of `radius`,
    /// or leaves it if it's on; or with `full_page` captures all of the page and copies it. Its
    /// menu offers Save to Library with `library`, as a private window has no Library, and it
    /// draws in the window's menus' `look`.
    fn capture(
        &self,
        tab: u64,
        frame: Bounds<Pixels>,
        radius: f32,
        full_page: bool,
        library: bool,
        look: crate::menu::Look,
        cx: &mut App,
    );
    /// The window's menus' `look` changed: capture mode over `tab`, if it's on, takes it, as it
    /// took the look it started in from `capture`.
    fn capture_look(&self, _tab: u64, _look: crate::menu::Look, _cx: &mut App) {}
    /// Asks for a picture of `tab`'s page, as it was last drawn, whole, which the browser answers
    /// with `Shell::page_pictured` and `request`. For the window whose page another window shows.
    fn page_picture(&self, tab: u64, request: u64, cx: &mut App);
    /// The browser's id for the window this shell draws, for the log. 0 where there is none.
    fn window(&self) -> u64 {
        0
    }
    /// Opens the Settings window, or brings it to the front.
    fn settings(&self, cx: &mut App);
    /// Opens the Settings window on its Shortcuts tab.
    fn shortcuts(&self, cx: &mut App);
    /// Opens the Settings window on its About tab.
    fn about(&self, cx: &mut App);
    /// Whether this build updates itself: a release with its updater, or on Linux the Flatpak.
    fn updates(&self) -> bool {
        false
    }
    /// Check for Updates…: asks the updater to look, or says where the update stands.
    fn check_for_updates(&self, _: &mut App) {}
    /// Opens the Settings window on its Import tab.
    fn import(&self, cx: &mut App);
    /// Opens the Settings window on its Passwords tab, with the logins of `space`'s profile.
    fn passwords(&self, space: u64, cx: &mut App);
    /// Opens the Settings window on its Agents tab, which lists what each agent did.
    fn agent_activity(&self, cx: &mut App);
    /// What the user let the agent whose Space is `space` do, for its Space's menu to take back.
    fn agent_permissions(&self, space: u64, cx: &App) -> AgentGrants;
    /// Forgets the user's answer for good to the agent whose Space is `space` on `site`, or on
    /// every site with None.
    fn revoke_agent(&self, space: u64, site: Option<SharedString>, cx: &mut App);
    /// Disconnects the agent whose Space is `space`, whose token then stops working.
    fn disconnect_agent(&self, space: u64, cx: &mut App);
    /// The user takes back each tab of the agent's Space `space` they don't hold, as Take Over
    /// takes one.
    fn take_back_agent_tabs(&self, space: u64, cx: &mut App);
    /// The user takes over `tab`, where the agent with the id `agent` is at work: it may not act
    /// there until they hand it back.
    fn take_over(&self, agent: SharedString, tab: u64, cx: &mut App);
    /// The user hands `tab` back to its agent.
    fn hand_back(&self, tab: u64, cx: &mut App);
    /// The user gives the agent whose Space is `space` their `tab`, which moves there.
    fn give_to_agent(&self, tab: u64, space: u64, cx: &mut App);
    /// The user stops the agent with the id `agent`: what it's doing and asking ends.
    fn stop_agent(&self, agent: SharedString, cx: &mut App);
    /// Selects the tab's next match, or the one before, opening its find bar if it's closed.
    fn find_next(&self, tab: u64, forward: bool, cx: &mut App);
    /// Scrolls the tab's page to put its selection in the middle.
    fn jump_to_selection(&self, tab: u64, cx: &mut App);
    fn quit(&self, cx: &mut App);
    /// Opens another window on the same Spaces, after this one's draw.
    fn new_window(&self, cx: &mut App);
    /// new_window, with a new profile's first launch playing in the new window, which shows no
    /// page.
    fn welcome_window(&self, _cx: &mut App) {}
    /// Whether the system opens http and https links with the browser, which the onboarding asks
    /// only as it starts, to leave out its step for it.
    fn is_default_browser(&self) -> bool {
        true
    }
    /// Asks the system to make the browser the default, which asks the user first.
    fn make_default_browser(&self, _cx: &mut App) {}
    /// The system it runs on, which sets the default browser its own way.
    fn os(&self) -> crate::default_browser::Os {
        crate::default_browser::Os::current()
    }

    /// Whether crash reports are sent, as Settings' switch has it, or None while there's no
    /// collector to send them to, when neither Settings nor the first launch offers it.
    fn crash_reports(&self, _cx: &App) -> Option<bool> {
        None
    }
    fn set_crash_reports(&self, _on: bool, _cx: &mut App) {}
    /// Whether agents may use the browser, as Settings' switch has it, or None in a build without
    /// the agents' server.
    fn agents(&self, _cx: &App) -> Option<bool> {
        None
    }
    fn set_agents(&self, _on: bool, _cx: &mut App) {}
    /// Opens a private window in the shown Space's profile's off-the-record session, after this
    /// one's draw.
    fn new_private_window(&self, cx: &mut App);
    /// Closes this window. Its tabs go to the window that was in front before it, live.
    fn close_window(&self, cx: &mut App);
    /// Opens an empty Little ilo window (docs/ux-spec.md › Little Arc) for the user's Space
    /// `home`, after this one's draw.
    fn new_little_window(&self, home: u64, cx: &mut App);
    /// Opens a Little ilo window for `tab`'s Space and moves `tab`, one of its Today tabs, into it,
    /// its page live with its history, as ⌥⌘-click on its row does (Arc).
    fn open_tab_in_little(&self, tab: u64, cx: &mut App);
    /// A Little ilo window's ⌘O: moves `tab`, live, to the user's Space `home` in a regular window,
    /// which shows it and comes to the front, and closes this window.
    fn open_in_main_window(&self, tab: u64, home: u64, cx: &mut App);
    /// A Little ilo window's profile picker: opens its page, at `url`, in `profile`, in a new Little ilo
    /// window for a Space of it, closing this one, and keeps `profile` for the page's site, which
    /// links from other apps then open in (Arc's Little Arc). With the window's own, only keeps it.
    fn little_profile(&self, profile: u64, url: &str, cx: &mut App);
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WindowButton {
    Close,
    Minimize,
    Zoom,
}

/// Where the window's close, minimize and zoom buttons are and how big: each one's centre across
/// from the window's left edge, their row's centre down from its top, and their diameter. macOS 26
/// draws the Mac's larger in an app built with its SDK than in one built with an older SDK, 14 on
/// a pitch of 23 against 12 on 20 (#1310), so a copy takes them from AppKit.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WindowButtonGeometry {
    pub centres: [f32; 3],
    pub y: f32,
    pub diameter: f32,
}

impl Default for WindowButtonGeometry {
    /// Arc's, which the core's frame gives AppKit's in an app built with macOS 26's SDK, and the
    /// UI's own on Linux and Windows.
    fn default() -> Self {
        let centres = super::WINDOW_BUTTONS.map(|(x, _)| x);
        Self { centres, y: WINDOW_BUTTON_Y, diameter: WINDOW_BUTTON_SIZE }
    }
}

impl WindowButtonGeometry {
    /// Each button's frame in the window.
    pub fn frames(&self) -> [Bounds<Pixels>; 3] {
        let (radius, side) = (self.diameter / 2., px(self.diameter));
        let top = px(self.y - radius);
        let frame = |x: f32| Bounds::new(point(px(x - radius), top), size(side, side));
        self.centres.map(frame)
    }
}

/// Where a tab's page is, as the browser reports it.
#[derive(Clone, Default, PartialEq)]
pub struct Page {
    pub url: SharedString,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub loading: bool,
    /// The tab has no page, as one put back and not yet shown, a pinned tab closed, or one unseen
    /// for hours. Its page loads once it's shown.
    pub unloaded: bool,
    /// The tab is pinned or a favorite, and its page isn't the one it was pinned or made a favorite
    /// at.
    pub away_from_pinned_url: bool,
    /// The page is playing sound, muted or not, or was in the last 2 s.
    pub audible: bool,
    pub muted: bool,
    /// The emoji the user gave the tab to show in place of its page's favicon, or empty.
    pub icon: SharedString,
    pub trouble: Trouble,
    /// Safari's engine shows the page, in place of Chromium.
    pub system_engine: bool,
    /// The page uses the camera, or the microphone.
    pub camera: bool,
    pub microphone: bool,
    /// The page shares a screen, a window or another tab, as a call does.
    pub sharing: bool,
    /// Another page shares this tab.
    pub shared: bool,
    /// Another page shares a tab, and `share_instead` would have it share this one.
    pub share_instead: bool,
    /// The page has media the browser can play, pause and skip, as Chrome's media controls do:
    /// whether it's playing, its title and artist, each empty without one, whether it has a next
    /// track, and whether it can float in Picture in Picture while nothing of the page floats.
    pub media: bool,
    pub media_playing: bool,
    pub media_title: SharedString,
    pub media_artist: SharedString,
    pub media_next: bool,
    pub media_picture_in_picture: bool,
    /// The tab shows its page in Developer Mode, for its site or as localhost.
    pub developer_mode: bool,
}

/// What the sidebar's player does to a page's media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    Next,
    PictureInPicture,
}

/// What's wrong with a tab's page, as the core says (BC_PAGE_ in browser_core.h). Agents read the
/// same.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum Trouble {
    #[default]
    Fine,
    /// Its renderer died while a window showed it: its pane shows the app's card, with Reload.
    Crashed,
    /// It hasn't answered input for Chrome's hang timeout: a note over it offers Wait and Reload.
    Hung,
    /// It's the core's error page for a load that failed, with Chromium's net error code.
    NetworkError(i32),
}

/// A split view: its tabs in their order, which sit together in their list as one row, shown side
/// by side, else stacked top to bottom.
/// What an agent's Space's menu lists, to take back (agent_permissions).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentGrants {
    /// The sites where the user answered the agent for good, by name: Always Allow (true) or
    /// Never (false).
    pub sites: Vec<(SharedString, bool)>,
    pub connected: bool,
    /// Whether the Space has a tab the user doesn't hold.
    pub tabs: bool,
}

#[derive(Clone, Default)]
pub struct Split {
    pub tabs: Vec<u64>,
    pub stacked: bool,
    /// Each tab's share of the split's length with the gap after it, adding up to 1, or none
    /// while they're even.
    pub shares: Vec<f64>,
    /// The tab whose page took the card with the others kept at 220 pt, else 0. The shares then
    /// stay even, as the core works the widths out from this at every layout.
    pub expanded: u64,
}

/// A Space's folders in its pinned tabs. What's in each follows it there.
#[derive(Clone, Default)]
pub struct Folders {
    /// Each folder's name, and whether it's open.
    pub folders: HashMap<u64, (SharedString, bool)>,
    /// The folder holding each pinned tab or folder in one.
    pub parents: HashMap<u64, u64>,
    /// Whether its pinned rows are collapsed, which hides them as a closed folder hides what's in
    /// it.
    pub pinned_shut: bool,
    /// Each folder's icon, an emoji, which the sidebar draws in place of the folder; those without
    /// one aren't here.
    pub icons: HashMap<u64, SharedString>,
}
