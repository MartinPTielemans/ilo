//! A window's shell's requests, as calls to the core, and the helpers that read the core's
//! answers.

use super::*;

/// A window's shell's requests, as calls to the core. The core reports back once each call has
/// returned.
pub(crate) struct Engine {
    /// The window's, in the core.
    pub(crate) window: bc::bc_id,
    /// The view the UI draws in, for the host.
    pub(crate) ui_view: *mut c_void,
    /// GPUI's view, which has the keyboard when the UI does.
    pub(crate) view: *mut c_void,
    /// The videos Media's tiles play, by tile.
    pub(crate) videos: RefCell<HashMap<u64, Video>>,
}

/// A video a Media tile plays, from the host, with the pixels its frames are copied into.
pub(crate) struct Video {
    pub(crate) player: *mut c_void,
    pub(crate) side: u32,
    pub(crate) pixels: Vec<u8>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        for (_, video) in self.videos.take() {
            unsafe { platform::host_video_close(video.player) };
        }
    }
}

impl Browser for Engine {
    fn select(&self, tab: u64, _: &mut App) {
        browser::handover_clicked();
        unsafe { bc::bc_window_select(self.window, tab) };
    }

    fn close(&self, tab: u64, split: bool, _: &mut App) {
        unsafe { bc::bc_tab_close(tab, split) };
    }

    fn close_below(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_close_below(tab) };
    }

    fn duplicate(&self, tab: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_tab_duplicate(tab) }
    }

    fn move_to_space(&self, tab: u64, space: u64, _: &mut App) {
        unsafe { bc::bc_tab_move_to_space(tab, space) };
    }

    fn move_folder_to_space(&self, folder: u64, space: u64, _: &mut App) {
        unsafe { bc::bc_folder_move_to_space(folder, space) };
    }

    fn warn_moving_profiles(&self, _: &mut App) -> bool {
        unsafe { bc::bc_warn_moving_profiles() }
    }

    fn set_warn_moving_profiles(&self, warn: bool, _: &mut App) {
        unsafe { bc::bc_set_warn_moving_profiles(warn) };
    }

    fn set_pinned(&self, tab: u64, pinned: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_pinned(tab, pinned) };
    }

    fn move_to_favorites(&self, tab: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move_to_favorites(tab, index) };
    }

    fn move_split_to_favorites(&self, tab: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move_split_to_favorites(tab, index) };
    }

    fn move_tab(&self, tab: u64, pinned: bool, folder: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move(tab, pinned, folder, index) };
    }

    // The ones after the first load once shown, as links opened behind do.
    fn open_at(
        &self,
        urls: &[String],
        pinned: bool,
        folder: u64,
        index: usize,
        _: &mut App,
    ) -> u64 {
        let tabs = urls
            .iter()
            .enumerate()
            .map(|(at, url)| (at, open_tab_behind(self.window, url, at > 0)));
        let tabs: Vec<u64> = tabs
            .filter(|&(_, tab)| tab != 0)
            .map(|(_, tab)| tab)
            .collect();
        for (at, &tab) in tabs.iter().enumerate() {
            unsafe { bc::bc_tab_move(tab, pinned, folder, index + at) };
        }
        let first = tabs.first().copied().unwrap_or(0);
        if first != 0 {
            unsafe { bc::bc_window_select(self.window, first) };
        }
        first
    }

    fn open_behind(&self, urls: &[String], pinned: bool, folder: u64, index: usize, _: &mut App) {
        let tabs = urls
            .iter()
            .map(|url| open_tab_behind(self.window, url, true));
        for (at, tab) in tabs.filter(|&tab| tab != 0).enumerate() {
            unsafe { bc::bc_tab_move(tab, pinned, folder, index + at) };
        }
    }

    fn create_folder(&self, space: u64, name: &str, folder: u64, index: usize, _: &mut App) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_folder_create(space, name.as_ptr(), folder, index) }
    }

    fn rename_folder(&self, folder: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_folder_rename(folder, name.as_ptr()) };
    }

    fn set_folder_icon(&self, folder: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_folder_set_icon(folder, icon.as_ptr()) };
    }

    fn duplicate_folder(&self, folder: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_folder_duplicate(folder) }
    }

    fn set_folder_open(&self, folder: u64, open: bool, _: &mut App) {
        unsafe { bc::bc_folder_set_open(folder, open) };
    }

    fn set_pinned_open(&self, space: u64, open: bool, _: &mut App) {
        unsafe { bc::bc_space_set_pinned_open(space, open) };
    }

    fn move_folder(&self, folder: u64, to: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_folder_move(folder, to, index) };
    }

    fn delete_folder(&self, folder: u64, _: &mut App) {
        unsafe { bc::bc_folder_delete(folder) };
    }

    fn separate(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_separate(tab) };
    }

    fn move_in_split(&self, tab: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move_in_split(tab, index) };
    }

    fn set_split_stacked(&self, tab: u64, stacked: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_split_stacked(tab, stacked) };
    }

    fn set_split_shares(&self, tab: u64, shares: Option<&[f64]>, _: &mut App) {
        let (shares, count) = shares.map_or((std::ptr::null(), 0), |shares| {
            (shares.as_ptr(), shares.len())
        });
        unsafe { bc::bc_tab_set_split_shares(tab, shares, count) };
    }

    fn set_split_expanded(&self, tab: u64, expanded: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_split_expanded(tab, expanded) };
    }

    fn split_with(&self, tab: u64, with: u64, left: bool, _: &mut App) {
        unsafe {
            bc::bc_tab_split_with(tab, with, left);
            bc::bc_window_select(self.window, tab);
        }
    }

    fn open_split_at(&self, url: &str, with: u64, left: bool, stacked: bool, _: &mut App) {
        let (window, url) = (self.window, url.to_string());
        // After the drop that asked, which Chromium is still handling.
        update_app_later(move |cx| {
            let tab = open_tab_behind(window, &url, false);
            if tab == 0 {
                return;
            }
            let joined = unsafe { bc::bc_tab_split_with(tab, with, left) };
            unsafe {
                if stacked {
                    bc::bc_tab_set_split_stacked(with, true);
                }
                bc::bc_window_select(window, tab);
            }
            if joined {
                split_opened(window, tab, cx);
            }
        });
    }

    fn show_drop_targets(&self, shown: bool, cx: &mut App) {
        let ui_view = self.ui_view;
        // After the shell's draw, which asks for it.
        cx.defer(move |cx| drop_targets::show(shown, ui_view, cx));
    }

    fn pointer(&self, _: &mut App) -> Option<Point<Pixels>> {
        let (mut x, mut y, mut left, mut top, mut width, mut height) = (0., 0., 0., 0., 0., 0.);
        // The window's frame stands in for its content's, near enough for where a download's
        // flight leaves from: the browser window draws its own title bar.
        let known = unsafe {
            platform::host_pointer(&mut x, &mut y)
                && platform::host_window_frame(
                    self.ui_view,
                    &mut left,
                    &mut top,
                    &mut width,
                    &mut height,
                )
        };
        known.then(|| point(px((x - left) as f32), px((y - top) as f32)))
    }

    fn over_page_blends(&self) -> bool {
        #[cfg(target_os = "linux")]
        return gpui_linux::overlays_blend();
        #[cfg(not(target_os = "linux"))]
        true
    }

    fn back_to_pinned_url(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_back_to_pinned_url(tab) };
    }

    fn set_muted(&self, tab: u64, muted: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_muted(tab, muted) };
    }

    fn share_instead(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_share_instead(tab) };
    }

    fn media(&self, tab: u64, action: MediaAction, _: &mut App) {
        let action = match action {
            MediaAction::Play => bc::BC_MEDIA_PLAY,
            MediaAction::Pause => bc::BC_MEDIA_PAUSE,
            MediaAction::Next => bc::BC_MEDIA_NEXT,
            MediaAction::PictureInPicture => bc::BC_MEDIA_PICTURE_IN_PICTURE,
        };
        unsafe { bc::bc_tab_media(tab, action as i32) };
    }

    fn cast(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_cast(tab) };
    }

    fn cast_close(&self, _: &mut App) {
        unsafe { bc::bc_cast_close() };
    }

    fn cast_to(&self, tab: u64, id: SharedString, _: &mut App) {
        let id = CString::new(id.as_ref()).unwrap_or_default();
        unsafe { bc::bc_tab_cast_to(tab, id.as_ptr()) };
    }

    fn clear(&self, _: &mut App) -> u64 {
        unsafe { bc::bc_window_clear(self.window) }
    }

    fn undo_clear(&self, clear: u64, _: &mut App) -> usize {
        unsafe { bc::bc_window_undo_clear(self.window, clear) }
    }

    fn undo_delete_space(&self, space: u64, _: &mut App) -> bool {
        unsafe { bc::bc_window_undo_delete_space(self.window, space) != 0 }
    }

    fn reopen_closed_tab(&self, _: &mut App) {
        unsafe { bc::bc_window_reopen_closed_tab(self.window) };
    }

    fn reopen_tab(&self, closed: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_window_reopen_tab(self.window, closed) }
    }

    fn open_preview(
        &self,
        over: u64,
        url: SharedString,
        link: Point<Pixels>,
        page: (Bounds<Pixels>, f32),
        _: &mut App,
    ) {
        let window = self.window;
        // Outside the shell's update, as the core tells the shell of the tab it opens.
        update_app_later(move |cx| {
            let Ok(text) = CString::new(url.as_ref()) else {
                return;
            };
            let preview = unsafe { bc::bc_tab_open_preview(over, text.as_ptr()) };
            if preview == 0 {
                log!("browser: link preview over {over} not opened");
                return;
            }
            link_preview::show(window, preview, over, url, page, link, cx);
        });
    }

    fn link_previews(&self) -> bool {
        LINK_PREVIEWS.get() && unsafe { bc::bc_link_previews() }
    }

    fn close_preview(&self, over: u64, cx: &mut App) -> bool {
        link_preview::close_over(self.window, over, cx)
    }

    fn pinned_url(&self, tab: u64) -> String {
        name(bc::bc_tab_pinned_url, tab).to_string()
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        let window = self.window;
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_typed_url(window, text, url, capacity)
        })
    }

    fn set_pinned_url(&self, tab: u64, url: &str, _: &mut App) {
        let url = CString::new(url).unwrap_or_default();
        unsafe { bc::bc_tab_set_pinned_url(tab, url.as_ptr()) };
    }

    fn tab_name(&self, tab: u64) -> String {
        name(bc::bc_tab_name, tab).to_string()
    }

    fn undo_archive_remove(&self, removal: u64, _: &mut App) -> usize {
        unsafe { bc::bc_archive_undo_remove(removal) }
    }

    fn restore_archived(&self, archived: u64, _: &mut App) {
        unsafe { bc::bc_window_restore_archived(self.window, archived) };
    }

    fn act_on_download(&self, download: u64, action: DownloadAction, _: &mut App) {
        let action = match action {
            DownloadAction::Open => bc::BC_DOWNLOAD_OPEN,
            DownloadAction::Show => bc::BC_DOWNLOAD_SHOW,
            DownloadAction::Pause => bc::BC_DOWNLOAD_PAUSE,
            DownloadAction::Resume => bc::BC_DOWNLOAD_RESUME,
            DownloadAction::Cancel => bc::BC_DOWNLOAD_CANCEL,
            DownloadAction::Remove => bc::BC_DOWNLOAD_REMOVE,
            DownloadAction::Keep => bc::BC_DOWNLOAD_KEEP,
        };
        unsafe { bc::bc_download_act(download, action as i32) };
    }

    fn clear_downloads(&self, _: &mut App) {
        unsafe { bc::bc_downloads_clear() };
    }

    fn download_icon(&self, download: u64, _: &mut App) {
        unsafe { bc::bc_download_icon(download) };
    }

    fn file_preview(&self, download: u64, path: &str, _: &mut App) {
        if let Ok(path) = CString::new(path) {
            unsafe { platform::host_file_preview(path.as_ptr(), download, file_preview) };
        }
    }

    fn open_video(&self, id: u64, path: &str, side: u32, _: &mut App) -> bool {
        let Ok(path) = CString::new(path) else {
            return false;
        };
        let player = unsafe { platform::host_video_open(path.as_ptr(), side as c_int) };
        if player.is_null() {
            return false;
        }
        let pixels = vec![0; side as usize * side as usize * 4];
        self.videos.borrow_mut().insert(
            id,
            Video {
                player,
                side,
                pixels,
            },
        );
        true
    }

    fn video_frame(&self, id: u64, _: &mut App) -> Option<Arc<gpui::RenderImage>> {
        let mut videos = self.videos.borrow_mut();
        let video = videos.get_mut(&id)?;
        if !unsafe { platform::host_video_frame(video.player, video.pixels.as_mut_ptr()) } {
            return None;
        }
        // BGRA, as GPUI draws a RenderImage's pixels.
        let frame = image::RgbaImage::from_raw(video.side, video.side, video.pixels.clone())?;
        Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
            frame,
        )])))
    }

    fn close_video(&self, id: u64, _: &mut App) {
        if let Some(video) = self.videos.borrow_mut().remove(&id) {
            unsafe { platform::host_video_close(video.player) };
        }
    }

    fn list_captures(&self, cx: &mut App) {
        list_captures(cx);
    }

    fn hover_kinds(&self, _: &mut App) -> u32 {
        unsafe { bc::bc_hover_kinds() as u32 }
    }

    fn set_hover_kinds(&self, kinds: u32, _: &mut App) {
        unsafe { bc::bc_set_hover_kinds(kinds as c_int) };
    }

    fn trash(&self, path: &str, _: &mut App) -> bool {
        CString::new(path).is_ok_and(|path| unsafe { platform::host_trash(path.as_ptr()) })
    }

    fn drag_file(&self, path: &str, _: &mut App) -> bool {
        CString::new(path)
            .is_ok_and(|path| unsafe { platform::host_drag_file(path.as_ptr(), file_drag_ended) })
    }

    fn remove_archived(&self, archived: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_archive_remove(archived) }
    }

    fn clear_archive(&self, _: &mut App) -> u64 {
        unsafe { bc::bc_archive_clear() }
    }

    fn page_favicon(&self, url: &str, _: &mut App) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_page_favicon(url.as_ptr()) };
        }
    }

    fn day_starts(&self) -> [f64; 4] {
        let mut starts = [0.; 4];
        unsafe { platform::host_day_starts(starts.as_mut_ptr()) };
        starts
    }

    fn go_back(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_go_back(tab) };
    }

    fn go_forward(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_go_forward(tab) };
    }

    fn tab_history(&self, tab: u64, forward: bool, _: &mut App) {
        unsafe { bc::bc_tab_history(tab, forward) };
    }

    fn go_to_history(&self, tab: u64, offset: i32, _: &mut App) {
        unsafe { bc::bc_tab_go_to_history(tab, offset) };
    }

    fn open_history(&self, tab: u64, offset: i32, _: &mut App) {
        unsafe { bc::bc_tab_open_history(tab, offset) };
    }

    fn reload(&self, tab: u64, bypass_cache: bool, _: &mut App) {
        unsafe { bc::bc_tab_reload(tab, bypass_cache) };
    }

    fn clear_site(&self, tab: u64, cookies: bool, _: &mut App) {
        unsafe { bc::bc_tab_clear_site(tab, cookies) };
    }

    fn set_developer_mode(&self, tab: u64, on: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_developer_mode(tab, on) };
    }

    fn answer_permission(&self, tab: u64, answer: shell::Answer, _: &mut App) {
        let answer = match answer {
            shell::Answer::Allow => bc::BC_PERMISSION_ALLOW,
            shell::Answer::Block => bc::BC_PERMISSION_BLOCK,
            shell::Answer::Dismiss => bc::BC_PERMISSION_DISMISS,
        };
        unsafe { bc::bc_tab_answer_permission(tab, answer as i32) };
    }

    fn answer_agent(&self, question: u64, answer: browser::agents::Answer, _: &mut App) {
        // After the shell's update, as the answer tells every window's shell the next question.
        agents_core::update(move |agents, cx| agents.answer(question, answer, cx));
    }

    fn take_over(&self, agent: SharedString, tab: u64, _: &mut App) {
        agents_core::update(move |agents, cx| agents.take_over(&agent, tab, cx));
    }

    fn hand_back(&self, tab: u64, _: &mut App) {
        agents_core::update(move |agents, cx| agents.hand_back(tab, cx));
    }

    fn stop_agent(&self, agent: SharedString, _: &mut App) {
        // A task's Stop, from its page's strip too, ends its turn as well as what it's doing.
        agents_core::update(move |agents, cx| match browser::agents::is_task(&agent) {
            true => agents.stop_task(&agent, cx),
            false => agents.stop_agent(&agent, cx),
        });
    }

    fn task(&self, agent: SharedString, action: browser::agents::task::TaskAction, _: &mut App) {
        let window = self.window;
        agents_core::update(move |agents, cx| {
            let (opened, _) = agents.task_action(&agent, action, cx);
            // Open shows the tab it moved into the user's Space, in the window it was opened from.
            if let Some(tab) = opened {
                unsafe { bc::bc_window_select(window, tab) };
            }
        });
    }

    fn show_task(&self, _: SharedString, _: &mut App) {
        // The Agents send a window its conversation as it changes; this sends it now.
        agents_core::update(|_, cx| cx.notify());
    }

    fn task_picture(&self, tab: u64, request: u64, _: &mut App) {
        // At twice the picture's size, for a Retina screen.
        let width = (2. * browser::tokens::shell::conversation::PICTURE_WIDTH) as i32;
        let height = (2. * browser::tokens::shell::conversation::PICTURE_HEIGHT) as i32;
        unsafe { bc::bc_tab_picture(tab, request, width, height) };
    }

    fn show_page_dialog(&self, shown: Option<shell::ShownPageDialog>, cx: &mut App) {
        // The dialog shows over the window last key, as the others do; another's waits for it.
        if self.window != front().0 {
            return;
        }
        let Some((tab, asked, over)) = shown else {
            dialog::withdraw_page(cx);
            return;
        };
        let field = |placeholder: &SharedString, text: &SharedString, secure| dialog::Field {
            placeholder: placeholder.clone(),
            text: text.clone(),
            secure,
        };
        let none = SharedString::default();
        let fields = match asked.kind {
            shell::DialogKind::Prompt => vec![field(&none, &asked.text, false)],
            shell::DialogKind::SignIn => {
                vec![
                    field(&asked.username, &none, false),
                    field(&asked.password, &none, true),
                ]
            }
            _ => Vec::new(),
        };
        let question = dialog::Question {
            icon: None,
            message: asked.title.clone(),
            detail: asked.message.clone(),
            list: Vec::new(),
            action: asked.accept.clone(),
            cancel: asked.cancel.clone(),
            tone: match asked.kind {
                // Leaving loses what the user changed in the page.
                shell::DialogKind::Leave => dialog::Tone::Destructive,
                // Another answer to a page takes nothing back.
                _ => dialog::Tone::Accent,
            },
            also: None,
            fields,
            check: (!asked.check.is_empty()).then(|| asked.check.clone()),
            on_return: match asked.kind == shell::DialogKind::OpenApp {
                true => dialog::Return::Cancel,
                false => dialog::Return::Action,
            },
        };
        let window = self.window;
        let kind = asked.kind;
        let answered = Box::new(move |reply: dialog::Reply, cx: &mut App| {
            let mut texts = reply.texts.into_iter();
            let (text, password) = match kind {
                shell::DialogKind::Prompt => (texts.next().unwrap_or_default(), String::new()),
                shell::DialogKind::SignIn => (
                    texts.next().unwrap_or_default(),
                    texts.next().unwrap_or_default(),
                ),
                _ => (String::new(), String::new()),
            };
            let reply = shell::PageReply {
                accept: reply.accept,
                text,
                password,
                checked: reply.checked,
            };
            let shell = WINDOWS.with_borrow(|windows| {
                windows
                    .iter()
                    .find(|w| w.id == window)
                    .map(|w| w.shell.clone())
            });
            match shell.and_then(|shell| shell.upgrade()) {
                Some(shell) => shell.update(cx, |shell, cx| {
                    shell.answer_page_dialog(tab, asked.id, reply, cx)
                }),
                // Its window closed while the dialog left, so the page isn't left waiting.
                None => answer_page_dialog(tab, asked.id, reply),
            }
        });
        dialog::ask_page((tab, asked.id), question, over, answered, cx);
    }

    fn answer_page_dialog(&self, tab: u64, id: u64, reply: shell::PageReply, _: &mut App) {
        answer_page_dialog(tab, id, reply);
    }

    fn show_offer(&self, tab: u64) {
        unsafe { bc::bc_tab_show_offer(tab) };
    }

    fn stop(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_stop(tab) };
    }

    fn zoom(&self, tab: u64, step: i32, cx: &mut App) {
        if tab == 0 {
            return;
        }
        let percent = unsafe { bc::bc_tab_zoom(tab, step) };
        // check.sh reads this line.
        log!("browser: zoom {tab} {percent}%");
        let shell = shell_with_view(self.ui_view);
        with_overlays(move |cx| zoom_hud::show(tab, percent, shell, cx), cx);
    }

    fn view_source(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_view_source(tab) };
        }
    }

    fn save_page(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_save_page(tab) };
        }
    }

    fn save_markdown(&self, name: &str, markdown: String, links: usize, cx: &mut App) {
        let save = move |path: std::path::PathBuf| match std::fs::write(&path, markdown) {
            // check.sh reads this line.
            Ok(()) => log!("browser: exported {links} links to {}", path.display()),
            Err(error) => log!("couldn't export to {}: {error}", path.display()),
        };
        // A check names its file, as it can't drive the system's panel.
        if !cfg!(feature = "prod")
            && let Some(file) = std::env::var_os("BROWSER_EXPORT_FILE")
        {
            return save(file.into());
        }
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&home, Some(name));
        cx.foreground_executor()
            .spawn(async move {
                if let Ok(Ok(Some(path))) = chosen.await {
                    save(path);
                }
            })
            .detach();
    }

    fn close_page_menu(&self, tab: u64, command: Option<i32>, _: &mut App) {
        unsafe { bc::bc_tab_close_page_menu(tab, command.unwrap_or(0)) };
    }

    fn print(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_print(tab) };
        }
    }

    fn devtools(&self, tab: u64, what: i32, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_devtools(tab, what) };
        }
    }

    fn task_manager(&self, _: &mut App) {
        unsafe { bc::bc_window_task_manager(self.window) };
    }

    fn start_trace(&self) -> bool {
        unsafe { bc::bc_trace_start() }
    }

    fn stop_trace(&self) {
        unsafe { bc::bc_trace_stop() };
    }

    fn trace_note(&self, stop: Option<Rc<dyn Fn(&mut App)>>, cx: &mut App) {
        let Some(stop) = stop else {
            return toast::hide_resting(cx);
        };
        let button = toast::Button {
            label: toast::Label::Text(shell::STOP.into()),
            name: "stop",
            run: Rc::new(move |_, cx| stop(cx)),
        };
        with_overlays(
            move |cx| toast::show_resting(shell::RECORDING, vec![button], cx),
            cx,
        );
    }

    fn set_menus(&self, bar: &shell::MenuBar, cx: &mut App) {
        MENUS.with_borrow_mut(|menus| menus.bar = bar.clone());
        // The menu bar is the key window's, which lists its own Space's extensions.
        if front().0 == self.window {
            show_menus(cx);
        }
    }

    fn set_edit_titles(&self, undo: Option<&str>, redo: Option<&str>, cx: &mut App) {
        MENUS.with_borrow_mut(|menus| {
            menus.undo = undo.map(Into::into);
            menus.redo = redo.map(Into::into);
        });
        if front().0 == self.window {
            show_menus(cx);
        }
    }

    fn set_window_title(&self, title: &str, _: &mut App) {
        let title = CString::new(title).unwrap_or_default();
        unsafe { platform::host_set_window_title(self.ui_view, title.as_ptr()) };
    }

    fn run_extension(&self, id: &str, anchor: Bounds<Pixels>, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_window_run_extension(self.window, id.as_ptr(), rect(anchor)) }
    }

    fn run_extension_command(&self, id: &str, name: &str, _: &mut App) -> bool {
        let (Ok(id), Ok(name)) = (CString::new(id), CString::new(name)) else {
            return false;
        };
        unsafe { bc::bc_window_run_extension_command(self.window, id.as_ptr(), name.as_ptr()) }
    }

    fn set_extension_pinned(&self, profile: u64, id: &str, pinned: bool, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_extension_set_pinned(profile, id.as_ptr(), pinned) };
        }
    }

    fn remove_extension(&self, profile: u64, id: &str, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_extension_remove(profile, id.as_ptr()) }
    }

    fn turn_on_extension(&self, profile: u64, id: &str, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_extension_turn_on(profile, id.as_ptr()) }
    }

    fn extension_crash_told(&self, profile: u64, id: &str, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_extension_crash_told(profile, id.as_ptr()) };
        }
    }

    fn open_extension_options(&self, url: &str, _: &mut App) {
        open_tab(self.window, url);
    }

    fn manage_extensions(&self, id: Option<&str>, _: &mut App) {
        match id {
            Some(id) => open_tab(self.window, &format!("chrome://extensions/?id={id}")),
            None => open_tab(self.window, "chrome://extensions"),
        }
    }

    fn add_extension(&self, _: &mut App) {
        open_tab(self.window, WEB_STORE);
    }

    fn lay_out_side_panel(&self, frame: Bounds<Pixels>, _: &mut App) {
        unsafe { bc::bc_window_set_side_panel_frame(self.window, rect(frame)) };
    }

    fn close_side_panel(&self, _: &mut App) {
        unsafe { bc::bc_window_close_side_panel(self.window) };
    }

    fn has_side_panel(&self, id: &str) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_window_has_side_panel(self.window, id.as_ptr()) }
    }

    fn toggle_side_panel(&self, id: &str, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_window_toggle_side_panel(self.window, id.as_ptr()) };
        }
    }

    fn set_side_panel_width(&self, width: f32, _: &mut App) {
        unsafe { bc::bc_window_set_side_panel_width(self.window, width.into()) };
    }

    fn view_history(&self, _: &mut App) {
        open_tab(self.window, "chrome://history");
    }

    fn open_page(&self, url: &str, _: &mut App) {
        open_tab(self.window, url);
    }

    fn new_note(&self, space: u64, beside: Option<u64>, cx: &mut App) {
        let profile = unsafe { bc::bc_space_profile(space) };
        let url = browser::notes::new_document(&name(bc::bc_profile_notes_service, profile));
        // check.sh waits for this line.
        log!(
            "browser: new note {url}{}",
            if beside.is_some() { " split" } else { "" }
        );
        match beside {
            // Outside the shell's update, which the split's layout updates.
            Some(with) => {
                let window = self.window;
                cx.defer(move |cx| open_split(window, url, with, cx));
            }
            None => open_tab(self.window, url),
        }
    }

    fn site_search_settings(&self, _: &mut App) {
        open_tab(self.window, "chrome://settings/searchEngines");
    }

    fn clear_browsing_data(&self, _: &mut App) {
        open_tab(self.window, CLEAR_BROWSING_DATA);
    }

    fn close_other_window(&self, _: &mut App) -> bool {
        unsafe { platform::host_close_other_window() }
    }

    fn show_space(&self, space: u64, _: &mut App) {
        unsafe { bc::bc_window_show_space(self.window, space) };
    }

    fn space_tab(&self, space: u64) -> u64 {
        unsafe { bc::bc_window_space_tab(self.window, space) }
    }

    fn space_agent(&self, space: u64) -> SharedString {
        name(bc::bc_space_agent, space)
    }

    fn create_space(&self, name: &str, profile: u64, after: u64, _: &mut App) -> u64 {
        let space = create_space(name, after);
        if profile != 0 {
            unsafe { bc::bc_space_set_profile(space, profile) };
        }
        space
    }

    fn rename_space(&self, space: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_space_set_name(space, name.as_ptr()) };
    }

    fn rename_tab(&self, tab: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_tab_rename(tab, name.as_ptr()) };
    }

    fn set_icon(&self, tab: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_tab_set_icon(tab, icon.as_ptr()) };
    }

    fn delete_space(&self, space: u64, _: &mut App) -> bool {
        unsafe { bc::bc_space_delete(space) }
    }

    fn restore_sidebar(&self, time: u64, _: &mut App) {
        unsafe { bc::bc_window_restore_sidebar(self.window, time as f64) };
    }

    fn set_space_color(&self, space: u64, color: Option<u32>, _: &mut App) {
        unsafe { bc::bc_space_set_color(space, color.is_some(), color.unwrap_or(0)) };
    }

    fn set_space_icon(&self, space: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_space_set_icon(space, icon.as_ptr()) };
    }

    fn set_space_theme(&self, space: u64, theme: &str, _: &mut App) {
        let theme = CString::new(theme).unwrap_or_default();
        unsafe { bc::bc_space_set_theme(space, theme.as_ptr()) };
    }

    fn set_space_profile(&self, space: u64, profile: u64, _: &mut App) {
        unsafe { bc::bc_space_set_profile(space, profile) };
    }

    fn create_profile(&self, name: &str, _: &mut App) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_profile_create(name.as_ptr()) }
    }

    fn rename_profile(&self, profile: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_profile_rename(profile, name.as_ptr()) };
    }

    fn import_host(&self) -> Rc<dyn browser::import::Host> {
        Rc::new(import_host::CoreImport)
    }

    fn account_host(&self) -> Option<Rc<dyn browser::account::Host>> {
        // A build that names no server leaves the first launch's step out, as the address is for
        // Settings › Sync to ask (main's --sync-url).
        (!env!("SYNC_SERVER").is_empty()).then(settings::browser_account)
    }

    fn delete_profile(&self, profile: u64, _: &mut App) {
        unsafe { bc::bc_profile_delete(profile) };
    }

    fn context_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        in_reveal: bool,
        look: menu::Look,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        show_menu(
            self.ui_view,
            items,
            at,
            in_reveal,
            look,
            menu::picked(chosen),
            cx,
        );
    }

    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        confirmed: Box<dyn FnOnce(&mut App)>,
        cx: &mut App,
    ) {
        let window = Some(self.window);
        with_overlays(
            move |cx| dialog::ask(window, message, detail, action, confirmed, cx),
            cx,
        );
    }

    fn tell(
        &self,
        question: dialog::Question,
        confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
        cx: &mut App,
    ) -> bool {
        if !dialog::can_pose(cx) {
            return false;
        }
        let window = Some(self.window);
        with_overlays(move |cx| dialog::pose(window, question, confirmed, cx), cx);
        true
    }

    fn focus_ui(&self, _: &mut App) {
        focus_ui(self.window, self.view, false);
    }

    fn hold_keyboard_for_page(&self, _: &mut App) {
        focus_ui(self.window, self.view, true);
    }

    fn focus_page(&self, _: &mut App) {
        unsafe { bc::bc_window_focus_page(self.window) };
    }

    fn lay_out_page(&self, frame: Bounds<Pixels>, radius: f32, cx: &mut App) {
        set_page_frame(self.window, (frame, radius));
        // After the shell's update, which may be drawing its window.
        let window = self.window;
        cx.defer(move |cx| link_preview::page_moved(window, (frame, radius), cx));
    }

    fn set_focus_color(&self, color: u32, _: &mut App) {
        unsafe { bc::bc_window_set_focus_color(self.window, color) };
    }

    fn set_page_color(&self, color: u32, _: &mut App) {
        unsafe { bc::bc_window_set_page_color(self.window, color) };
    }

    fn set_window_light(&self, light: bool, _: &mut App) {
        unsafe { platform::host_set_window_light(self.ui_view, light) };
    }

    fn capture_look(&self, tab: u64, look: menu::Look, cx: &mut App) {
        capture::set_look(tab, look, cx);
    }

    fn set_appearance(&self, appearance: Appearance, _: &mut App) {
        settings::save_appearance(appearance);
        give_host_appearance(appearance);
    }

    fn page_moving(&self, moving: bool, _: &mut App) {
        unsafe { bc::bc_window_page_moving(self.window, moving) };
    }

    fn set_pane_corner(&self, left: f32, right: f32, top: f32, _: &mut App) {
        let (left, right, top) = (f64::from(left), f64::from(right), f64::from(top));
        unsafe { platform::host_set_pane_corner(self.ui_view, left, right, top) };
    }

    fn show_window_buttons(&self, shown: bool, _: &mut App) {
        unsafe { platform::host_show_window_buttons(self.ui_view, shown) };
    }

    #[cfg(target_os = "macos")]
    fn window_button_geometry(&self, _: &mut App) -> Option<browser::shell::WindowButtonGeometry> {
        let (mut centres, mut y, mut diameter) = ([0f64; 3], 0f64, 0f64);
        let read = unsafe {
            platform::host_window_buttons(self.ui_view, centres.as_mut_ptr(), &mut y, &mut diameter)
        };
        read.then(|| browser::shell::WindowButtonGeometry {
            centres: centres.map(|x| x as f32),
            y: y as f32,
            diameter: diameter as f32,
        })
    }

    fn press_window_button(&self, button: WindowButton, _: &mut App) {
        // Windows' own zoom button maximizes, and its window has no full screen of its own, so the
        // zoom Escape presses in full screen takes the window out through the core.
        if cfg!(target_os = "windows")
            && matches!(button, WindowButton::Zoom)
            && self.window_full_screen()
        {
            unsafe { bc::bc_window_toggle_full_screen(self.window) };
            return;
        }
        let button = match button {
            WindowButton::Close => 0,
            WindowButton::Minimize => 1,
            WindowButton::Zoom => 2,
        };
        unsafe { platform::host_press_window_button(self.ui_view, button) };
    }

    fn start_window_move(&self, _: &mut App) {
        // The core moves a window on Linux alone; on Windows the host does, from the app.
        #[cfg(target_os = "windows")]
        unsafe {
            platform::host_start_window_move(self.ui_view)
        };
        #[cfg(not(target_os = "windows"))]
        unsafe {
            bc::bc_window_start_move(self.window)
        };
    }

    fn window_maximized(&self) -> bool {
        #[cfg(target_os = "windows")]
        return unsafe { platform::host_window_maximized(self.ui_view) };
        #[cfg(not(target_os = "windows"))]
        return cfg!(target_os = "linux") && unsafe { bc::bc_window_is_maximized(self.window) };
    }

    fn window_full_screen(&self) -> bool {
        cfg!(any(target_os = "linux", target_os = "windows"))
            && unsafe { bc::bc_window_is_full_screen(self.window) }
    }

    fn show_reveal(&self, width: f32, _: &mut App) {
        unsafe { platform::host_show_reveal(self.ui_view, width.into()) };
    }

    fn hide_reveal(&self, _: &mut App) {
        unsafe { platform::host_hide_reveal(self.ui_view) };
    }

    fn option_reveals_sidebar(&self) -> bool {
        unsafe { bc::bc_option_reveals_sidebar() }
    }

    fn saved_sidebar_width(&self) -> Option<f32> {
        let width = unsafe { bc::bc_window_sidebar_width(self.window) };
        (width > 0.).then_some(width as f32)
    }

    fn set_sidebar_width(&self, width: f32, _: &mut App) {
        unsafe { bc::bc_window_set_sidebar_width(self.window, width.into()) };
    }

    fn detent(&self, _: &mut App) {
        if unsafe { bc::bc_haptics() } {
            unsafe { platform::host_detent() };
        }
    }

    fn watch_control(&self, _: &mut App) {
        unsafe { platform::host_watch_control(control_released) };
    }

    fn copy(&self, text: &str, cx: &mut App) {
        #[cfg(target_os = "linux")]
        system::copy(text, cx);
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            let text = CString::new(text.replace('\0', "")).unwrap_or_default();
            unsafe { platform::host_copy(text.as_ptr()) };
        }
    }

    fn pasted(&self, cx: &App) -> Option<String> {
        #[cfg(target_os = "linux")]
        return system::pasted(cx);
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            // Asked for its length first, then for the text.
            let length = unsafe { platform::host_paste(std::ptr::null_mut(), 0) };
            let mut text = vec![0u8; length.checked_add(1)?];
            unsafe { platform::host_paste(text.as_mut_ptr().cast(), text.len()) };
            text.truncate(length);
            Some(String::from_utf8_lossy(&text).into_owned()).filter(|text| !text.is_empty())
        }
    }

    fn can_open_in_safari(&self) -> bool {
        true
    }

    fn open_in_safari(&self, url: &str, _: &mut App) {
        if let Ok(url) = CString::new(url) {
            unsafe { platform::host_open_in_safari(url.as_ptr()) };
        }
    }

    fn can_use_system_engine(&self) -> bool {
        unsafe { bc::bc_system_engine_available() }
    }

    fn use_system_engine(&self, tab: u64, _: &mut App) {
        use_system_engine(tab);
    }

    fn close_media_note(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_close_media_note(tab) };
    }

    fn widevine(&self) -> Widevine {
        Widevine::from_core(unsafe { bc::bc_widevine() })
    }

    fn download_widevine(&self, _: &mut App) {
        unsafe { bc::bc_widevine_download() };
    }

    fn restart(&self, cx: &mut App) {
        restart_browser(cx);
    }

    fn playready_needs(&self) -> shell::PlayReady {
        shell::PlayReady::from_core(unsafe { bc::bc_playready_needs() })
    }

    fn share_services(&self, url: &str) -> Vec<SharedString> {
        unsafe extern "C" fn each(context: *mut c_void, name: *const c_char) {
            let names = unsafe { &mut *(context as *mut Vec<SharedString>) };
            names.push(
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned()
                    .into(),
            );
        }
        let mut names: Vec<SharedString> = Vec::new();
        if let Ok(url) = CString::new(url) {
            let context = &mut names as *mut Vec<SharedString> as *mut c_void;
            unsafe { platform::host_share_services(url.as_ptr(), each, context) };
        }
        names
    }

    fn share(&self, url: &str, title: &str, service: usize, _: &mut App) {
        if let (Ok(url), Ok(title)) = (CString::new(url), CString::new(title)) {
            unsafe { platform::host_share_with(url.as_ptr(), title.as_ptr(), service) };
        }
    }

    fn has_share_sheet(&self) -> bool {
        has_share_sheet()
    }

    fn show_share_sheet(&self, tab: u64, url: &str, title: &str, at: Point<Pixels>, cx: &mut App) {
        if let Some(window) = tab_window(tab, cx) {
            share_page(&window, url, title, Bounds::new(at, Size::default()));
        }
    }

    fn toast(&self, text: &str, key: Option<(&str, &str)>, time: Duration, cx: &mut App) {
        let text = text.to_string();
        let key = key.map(|(keys, what)| (keys.to_string(), what.to_string()));
        let show = move |cx: &mut App| {
            let key = key
                .as_ref()
                .map(|(keys, what)| (keys.as_str(), what.as_str()));
            toast::show(&text, key, time, cx)
        };
        with_overlays(show, cx);
    }

    fn toast_undo(&self, text: &str, undo: Rc<dyn Fn(&mut App)>, time: Duration, cx: &mut App) {
        let text = text.to_string();
        with_overlays(move |cx| toast::show_undo(&text, undo, time, cx), cx);
    }

    fn hide_undo_toast(&self, cx: &mut App) {
        toast::hide_undo(cx);
    }

    fn in_front(&self) -> bool {
        front().0 == self.window
    }

    fn toast_button(
        &self,
        text: &str,
        (label, name): (&'static str, &'static str),
        run: Rc<dyn Fn(&mut App)>,
        time: Duration,
        cx: &mut App,
    ) {
        let text = text.to_string();
        let button = toast::Button {
            label: toast::Label::Text(label.into()),
            name,
            run: Rc::new(move |_, cx| run(cx)),
        };
        with_overlays(
            move |cx| toast::show_with_buttons(&text, vec![button], time, cx),
            cx,
        );
    }

    fn full_screen_note(&self, shown: bool, cx: &mut App) {
        match shown {
            true => with_overlays(toast::show_full_screen, cx),
            false => toast::hide_full_screen(cx),
        }
    }

    fn hang_note(&self, tab: u64, shown: bool, cx: &mut App) {
        if !shown {
            toast::hide_held(tab, cx);
            return;
        }
        let button = |label: &str, name, run: fn(u64)| toast::Button {
            label: toast::Label::Text(label.to_owned().into()),
            name,
            run: Rc::new(move |_, _| run(tab)),
        };
        let buttons = vec![
            button(shell::WAIT, "wait", |tab| unsafe { bc::bc_tab_wait(tab) }),
            button(shell::RELOAD, "reload", |tab| unsafe {
                bc::bc_tab_reload(tab, false)
            }),
        ];
        with_overlays(
            move |cx| toast::show_held(shell::NOT_RESPONDING, buttons, tab, cx),
            cx,
        );
    }

    fn tooltip(&self, tip: Option<(tooltip::Tip, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            Some((tip, anchor)) => tooltip::hover(self.window, tip, anchor, cx),
            None => tooltip::dismiss(cx),
        }
    }

    fn link_target(
        &self,
        plate: Option<(link_target::Plate, Bounds<Pixels>, link_target::Side)>,
        cx: &mut App,
    ) {
        match plate {
            Some((plate, frame, side)) => link_target::show(self.window, plate, frame, side, cx),
            None => link_target::hide(cx),
        }
    }

    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        shown: Vec<SharedString>,
        tabs: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        cx: &mut App,
    ) {
        with_overlays(
            move |cx| command_bar::show(editing, shown, tabs, field, cx),
            cx,
        );
    }

    fn search_tabs(&self, tabs: Vec<BarTab>, cx: &mut App) {
        with_overlays(move |cx| command_bar::show_tabs(tabs, cx), cx);
    }

    fn command_bar_open(&self, cx: &App) -> bool {
        command_bar::is_open(cx)
    }

    fn command_bar_follows(&self, tabs: Vec<BarTab>, open_tabs: Vec<BarTab>, cx: &mut App) -> bool {
        // The bar shows over the window in front; another's Space, as an agent's, isn't its.
        self.window == front().0 && command_bar::space_shown(tabs, open_tabs, cx)
    }

    fn split_bar(&self, with: u64, pane: Bounds<Pixels>, card: Bounds<Pixels>, cx: &mut App) {
        with_overlays(move |cx| command_bar::show_split(with, pane, card, cx), cx);
    }

    fn find(&self, tab: u64, cx: &mut App) {
        let window = self.window;
        with_overlays(move |cx| find_bar::show(window, tab, cx), cx);
    }

    fn site_controls(
        &self,
        tab: u64,
        place: site_controls::Place,
        window: gpui::Size<Pixels>,
        extensions: Vec<site_controls::Extension>,
        cx: &mut App,
    ) {
        with_overlays(
            move |cx| site_controls::show(tab, place, window, extensions, cx),
            cx,
        );
    }

    fn site_controls_extensions(
        &self,
        tab: u64,
        extensions: Vec<site_controls::Extension>,
        cx: &mut App,
    ) {
        // The panel isn't open while the overlays wait to open.
        site_controls::set_extensions(tab, extensions, cx);
    }

    fn capture(
        &self,
        tab: u64,
        frame: Bounds<Pixels>,
        radius: f32,
        full_page: bool,
        library: bool,
        look: menu::Look,
        cx: &mut App,
    ) {
        with_overlays(
            move |cx| match full_page {
                true => capture::full_page(tab, cx),
                false => capture::start(tab, frame, radius, library, look, cx),
            },
            cx,
        );
    }

    fn page_picture(&self, tab: u64, request: u64, _: &mut App) {
        // The page's whole view, which is the shape it has where it's live; the card fits it.
        unsafe { bc::bc_tab_picture(tab, request, 0, 0) };
    }

    fn hover_card(&self, hover: hover_card::Hover, cx: &mut App) {
        hover_card::hover(hover, cx);
    }

    fn folder_preview(&self, ask: folder_preview::Ask, cx: &mut App) {
        folder_preview::ask(ask, cx);
    }

    fn card_picture(&self, tab: u64, cx: &mut App) {
        let request = hover_card::asking(tab, cx);
        // check.sh reads this line: a hidden page draws for its picture from here.
        log!("browser: tab {tab} picturing {request}");
        let (width, height) = hover_card::PICTURE_PIXELS;
        unsafe { bc::bc_tab_picture(tab, request, width, height) };
    }

    fn window(&self) -> u64 {
        self.window
    }

    fn find_next(&self, tab: u64, forward: bool, cx: &mut App) {
        let window = self.window;
        with_overlays(move |cx| find_bar::find_next(window, tab, forward, cx), cx);
    }

    fn jump_to_selection(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_jump_to_selection(tab) };
        }
    }

    fn settings(&self, cx: &mut App) {
        settings::show(cx);
    }

    fn shortcuts(&self, cx: &mut App) {
        settings::show_shortcuts(cx);
    }

    fn about(&self, cx: &mut App) {
        settings::show_about(cx);
    }

    fn updates(&self) -> bool {
        updates_available()
    }

    fn check_for_updates(&self, cx: &mut App) {
        check_for_updates(cx);
    }

    fn import(&self, cx: &mut App) {
        settings::show_import(cx);
    }

    fn passwords(&self, space: u64, cx: &mut App) {
        let profile = unsafe { bc::bc_space_profile(space) };
        settings::show_passwords(Some(profile).filter(|&profile| profile != 0), cx);
    }

    fn crash_reports(&self, _: &App) -> Option<bool> {
        let offered = !settings::crash_reports_collector().is_empty();
        offered.then(|| unsafe { bc::bc_send_crash_reports() })
    }

    fn set_crash_reports(&self, on: bool, cx: &mut App) {
        settings::set_send_crash_reports(on, cx);
    }

    // Every build has the agents' server. The first launch picks its steps before the first
    // frame, when agents_core makes the Agents, and a new profile's are off until then.
    fn agents(&self, cx: &App) -> Option<bool> {
        Some(browser::agents::global(cx).is_some_and(|agents| agents.read(cx).is_on()))
    }

    fn set_agents(&self, on: bool, cx: &mut App) {
        agents_core::start(cx);
        if let Some(agents) = browser::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.set_on(on, cx));
        }
    }

    fn new_window(&self, cx: &mut App) {
        let id = self.window;
        // Outside the shell's update, as the new window's shell reads this one's.
        cx.defer(move |cx| {
            // From a private window, the Spaces are the regular window's in front.
            let from = WINDOWS.with_borrow(|windows| {
                let this = windows.iter().find(|w| w.id == id && w.regular());
                this.or_else(|| windows.iter().find(|w| w.regular()))
                    .cloned()
            });
            if from.is_some() {
                open_window(from, None, cx);
            }
        });
    }

    fn is_default_browser(&self) -> bool {
        unsafe { platform::host_is_default_browser() }
    }

    fn make_default_browser(&self, _: &mut App) {
        unsafe { platform::host_make_default_browser(settings::default_browser_answered) };
    }

    fn welcome_window(&self, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| {
                let this = windows.iter().find(|w| w.id == id && w.regular());
                this.or_else(|| windows.iter().find(|w| w.regular()))
                    .cloned()
            });
            let Some(window) = from.and_then(|from| open_window(Some(from), None, cx)) else {
                return;
            };
            let shell = WINDOWS.with_borrow(|windows| {
                windows
                    .iter()
                    .find(|w| w.id == window)
                    .and_then(|w| w.shell.upgrade())
            });
            if let Some(shell) = shell {
                shell.update(cx, |shell, cx| shell.start_onboarding(None, cx));
            }
        });
    }

    fn new_private_window(&self, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned());
            // The profile of the Space this window shows, a private one's too.
            let profile = unsafe { bc::bc_space_profile(bc::bc_window_space(id)) };
            if from.is_some() && profile != 0 {
                open_window(from, Some(Own::Private(profile)), cx);
            }
        });
    }

    fn new_little_window(&self, home: u64, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned());
            if let Some(from) = from {
                open_little(from, home, cx);
            }
        });
    }

    fn open_tab_in_little(&self, tab: u64, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned());
            let Some(little) = from.and_then(|from| open_little(from, 0, cx)) else {
                return;
            };
            if unsafe { bc::bc_window_take_tab(little, tab) } == 0 {
                // check.sh reads this line.
                log!("browser: little window {little} refused {tab}");
                unsafe { bc::bc_window_close(little) };
                return;
            }
            // check.sh reads this line.
            log!("browser: little window {little} took {tab} from window {id}");
        });
    }

    fn open_in_main_window(&self, tab: u64, home: u64, cx: &mut App) {
        let id = self.window;
        cx.defer(move |_| {
            let Some(main) = main_window_for(home) else {
                // check.sh reads this line.
                log!("browser: little window {id} has no main window for {tab}");
                return;
            };
            let home = little_home(home, main);
            unsafe {
                if bc::bc_window_space(main) != home {
                    bc::bc_window_show_space(main, home);
                }
                bc::bc_tab_move_to_space(tab, home);
                bc::bc_window_select(main, tab);
                bc::bc_window_activate(main);
            }
            // check.sh reads this line.
            log!("browser: little window {id} put {tab} in window {main} space {home}");
            unsafe { bc::bc_window_close(id) };
        });
    }

    fn little_profile(&self, profile: u64, url: &str, cx: &mut App) {
        let (id, url) = (self.window, url.to_string());
        cx.defer(move |cx| {
            little_sites_host::remember(&url, profile);
            let own = unsafe { bc::bc_space_profile(bc::bc_window_space(id)) };
            if profile == own || url.is_empty() {
                // check.sh reads this line.
                log!("browser: little window {id} keeps profile {profile}");
                return;
            }
            let window =
                WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned());
            let Some(window) = window else {
                return;
            };
            let own_home = window.little.unwrap_or(0);
            // From a regular window, whose Spaces give the new one its home's look.
            let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.regular()).cloned());
            let home = home_in_profile(own_home, profile);
            let Some(opened) = home.and_then(|home| open_little(from.unwrap_or(window), home, cx))
            else {
                // check.sh reads this line.
                log!("browser: little window {id} has no Space in profile {profile}");
                return;
            };
            let tab = open_tab_behind(opened, &url, false);
            if tab != 0 {
                unsafe { bc::bc_window_select(opened, tab) };
            }
            // check.sh reads this line.
            log!("browser: little window {id} reopened {url} in profile {profile} as {opened}");
            unsafe { bc::bc_window_close(id) };
        });
    }

    fn close_window(&self, cx: &mut App) {
        let id = self.window;
        // Outside the shell's update: the core closes the NSWindow at once, and GPUI's handler for
        // that needs the App.
        cx.spawn(async move |_| {
            if WINDOWS.with_borrow(|windows| windows.iter().any(|w| w.id == id)) {
                unsafe { bc::bc_window_close(id) };
            }
        })
        .detach();
    }

    /// Asks first, as Arc does, unless the user said not to. Only the user's Quit asks: a quit from
    /// the Dock or at logout doesn't come here.
    fn quit(&self, cx: &mut App) {
        // Once the shell that asked is done, as it reads the shell. Chromium's quit is posted, so
        // comes after.
        cx.defer(save_launch_picture);
        // With no window to ask over, there's nothing open to lose.
        if !unsafe { bc::bc_warn_before_quitting() } || front().1.is_null() {
            return quit_because("the user's Quit, which had nothing to ask");
        }
        let question = dialog::Question {
            icon: None,
            message: concat!("Quit ", env!("APP_NAME"), "?").into(),
            detail: "".into(),
            list: Vec::new(),
            action: "Quit".into(),
            cancel: "Cancel".into(),
            tone: dialog::Tone::Accent,
            also: Some((
                "Quit, and don’t ask again".into(),
                Box::new(|_| {
                    // check.sh waits for this line.
                    log!("browser: warn before quitting off");
                    unsafe { bc::bc_set_warn_before_quitting(false) };
                    quit_because("the user's Quit, answered");
                }),
            )),
            fields: Vec::new(),
            check: None,
            on_return: dialog::Return::Action,
        };
        // A Quit as the window comes on screen, before its overlays are made, asks too (#905):
        // with no dialog to ask in, pose would drop it and the app would never quit.
        let quit =
            Box::new(|_: Vec<String>, _: &mut App| quit_because("the user's Quit, answered"));
        with_overlays(move |cx| dialog::pose(None, question, quit, cx), cx);
    }
}

/// A Space's or a folder's name, or a Space's icon, which `copy` copies as bc_space_name does.
pub(crate) fn name(
    copy: unsafe extern "C" fn(bc::bc_id, *mut c_char, usize) -> usize,
    id: u64,
) -> SharedString {
    let length = unsafe { copy(id, std::ptr::null_mut(), 0) };
    let mut name = vec![0u8; length + 1];
    unsafe { copy(id, name.as_mut_ptr().cast(), name.len()) };
    name.truncate(length);
    String::from_utf8_lossy(&name).into_owned().into()
}

/// The ids the core's `copy` copies.
pub(crate) fn ids(copy: unsafe extern "C" fn(*mut bc::bc_id, usize) -> usize) -> Vec<bc::bc_id> {
    let count = unsafe { copy(std::ptr::null_mut(), 0) };
    let mut ids = vec![0; count];
    let count = unsafe { copy(ids.as_mut_ptr(), ids.len()) };
    ids.truncate(count);
    ids
}

/// A string the core passed, for as long as the app keeps it.
pub(crate) fn core_text(text: *const c_char) -> SharedString {
    match text.is_null() {
        true => SharedString::default(),
        false => unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
            .into(),
    }
}
