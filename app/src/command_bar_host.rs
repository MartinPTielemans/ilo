//! The command bar's host: its requests, as calls to the core and the host's overlay.

use super::*;

/// The command bar's requests, as calls to the core and to the host's overlay window, for the
/// window it shows over (`front`).
pub(crate) struct Bar;

impl command_bar::Host for Bar {
    fn suggest(&self, site: Option<&str>, text: &str, complete: bool) {
        let Ok(site) = site.map(CString::new).transpose() else {
            return;
        };
        let site = site.as_ref().map_or(std::ptr::null(), |site| site.as_ptr());
        if let Ok(text) = CString::new(text) {
            unsafe { bc::bc_window_suggest(front().0, site, text.as_ptr(), complete) };
        }
    }

    fn remove_suggestion(&self, url: &str, cx: &mut App) {
        let Ok(url) = CString::new(url) else {
            return;
        };
        let window = front().0;
        let removal = unsafe { bc::bc_window_remove_suggestion(window, url.as_ptr()) };
        // The bar shows over the window in front, whose ⌘Z puts it back.
        if let Some(shell) = shell_of(window) {
            shell
                .update(cx, |shell, cx| shell.suggestion_removed(removal, cx))
                .ok();
        }
    }

    fn step_removal(&self, redo: bool, cx: &mut App) -> bool {
        let Some(shell) = shell_of(front().0) else {
            return false;
        };
        shell
            .update(cx, |shell, cx| shell.step_suggestion_removal(!redo, cx))
            .unwrap_or(false)
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        let window = front().0;
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_typed_url(window, text, url, capacity)
        })
    }

    fn search_url(&self, site: Option<&str>, text: &str) -> Option<String> {
        let site = site.map(CString::new).transpose().ok()?;
        let site = site.as_ref().map_or(std::ptr::null(), |site| site.as_ptr());
        let window = front().0;
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_search_url(window, site, text, url, capacity)
        })
    }

    fn page_favicon(&self, url: &str) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_page_favicon(url.as_ptr()) };
        }
    }

    fn open_tab(&self, url: &str, typed: &str, behind: bool) {
        match behind {
            // ⌘↩'s tab loads once it's first shown, as a link opened behind does (Dia).
            true => _ = open_typed(front().0, url, typed, true),
            false => open_tab_typed(front().0, url, typed),
        }
    }

    fn open_window(&self, url: &str, typed: &str, cx: &mut App) {
        let (id, url, typed) = (front().0, url.to_string(), typed.to_string());
        // Outside the bar's update, as ⌘N's window opens outside the shell's.
        cx.defer(move |cx| {
            if let Some(window) = open_window_after(id, cx) {
                open_tab_typed(window, &url, &typed);
            }
        });
    }

    fn navigate(&self, tab: u64, url: &str, typed: &str) {
        if let (Ok(url), Ok(typed)) = (CString::new(url), CString::new(typed)) {
            unsafe { bc::bc_tab_navigate_typed(tab, url.as_ptr(), typed.as_ptr()) };
        }
    }

    fn reload(&self, tab: u64) {
        unsafe { bc::bc_tab_reload(tab, false) };
    }

    fn select(&self, tab: u64) {
        unsafe { bc::bc_window_select(front().0, tab) };
    }

    fn tab_window(&self, tab: u64) -> Option<u64> {
        let window = unsafe { bc::bc_tab_window(tab) };
        (window != 0 && window != front().0).then_some(window)
    }

    fn activate(&self, window: u64, tab: u64, cx: &mut App) {
        let Some(ui_view) = WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|open| open.id == window)
                .map(|open| open.ui_view)
        }) else {
            return;
        };
        // Outside any update: the window reports that it's key at once, to its shell.
        cx.spawn(async move |_| unsafe {
            bc::bc_window_select(window, tab);
            platform::host_activate(ui_view);
        })
        .detach();
    }

    fn split(&self, tab: u64, with: u64, cx: &mut App) {
        split_tab(front().0, tab, with, false, cx);
    }

    fn open_split(&self, url: &str, typed: &str, with: u64, cx: &mut App) {
        open_split(front().0, url, typed, with, cx);
    }

    fn copy(&self, text: &str, cx: &mut App) {
        shell::copy_from_bar(text, cx);
    }

    fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32, level: Level) {
        let (left, top, width) = (f64::from(left), f64::from(top), f64::from(width));
        elevate(COMMAND_BAR, level);
        unsafe {
            platform::host_set_overlay_radius(COMMAND_BAR, radius.into());
            platform::host_show_overlay(COMMAND_BAR, front().1, centered, left, top, width)
        };
    }

    fn set_height(&self, height: f32) {
        unsafe { platform::host_set_overlay_height(COMMAND_BAR, f64::from(height)) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(COMMAND_BAR) };
        crate::settings::bar_hidden();
    }

    fn focus_page(&self) {
        unsafe { bc::bc_window_focus_page(front().0) };
    }

    fn agents_page(&self, cx: &App) -> bool {
        shell::agents_page_shown(cx)
    }

    fn dismiss(&self) {
        unsafe { platform::host_dismiss_overlay(COMMAND_BAR) };
        crate::settings::bar_hidden();
    }

    fn take_keyboard(&self) {
        // Linux's and Windows' hosts close the bar once its keyboard goes elsewhere.
        #[cfg(target_os = "macos")]
        unsafe {
            platform::host_take_overlay_keyboard(COMMAND_BAR)
        };
    }

    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
        shell::bar_commands(text, cx)
    }

    fn settings_index(&self, _: &App) -> Option<std::rc::Rc<Vec<browser::settings_search::Entry>>> {
        crate::settings::index()
    }

    fn show_setting(
        &self,
        section: browser::settings_column::Section,
        name: SharedString,
        cx: &mut App,
    ) {
        crate::settings::show_row(section, name, cx);
    }

    fn task_agents(&self, cx: &App) -> Option<command_bar::TaskAgents> {
        let agents = browser::agents::global(cx)?;
        let agents = agents.read(cx);
        agents.threads().then(|| task_agents_of(agents))
    }

    fn over_private(&self) -> bool {
        WINDOWS.with_borrow(|windows| windows.first().is_some_and(|window| window.private))
    }

    fn other_tabs(&self, _: &App) -> Rc<Vec<command_bar::OtherTab>> {
        // A private window's pages stay apart from the profile's, as its history does.
        match self.over_private() {
            true => Rc::default(),
            false => other_tabs_over(front().0),
        }
    }

    fn archived(&self, url: &str, cx: &App) -> Option<(u64, f64)> {
        // Only a window of the user's Spaces reopens a tab from the archive, as only it has the
        // Library: a private, Little ilo or Blank Window shows a Space of its own.
        let front = front().0;
        let shell = WINDOWS.with_borrow(|windows| {
            (windows.iter())
                .find(|window| window.id == front && window.regular())
                .map(|window| window.shell.clone())
        })?;
        shell.upgrade()?.read(cx).archived_at(url)
    }

    fn restore_archived(&self, archived: u64, cx: &mut App) -> bool {
        update_window_in(front().0, cx, |shell, cx| {
            shell.reopen_archived(archived, cx)
        })
        .unwrap_or(false)
    }

    fn look_for_agents(&self, cx: &mut App) {
        agents_core::update(|agents, cx| agents.task_ask_shown(cx));
    }

    fn asking(&self, shown: bool, cx: &mut App) {
        if shown {
            agents_core::update(|agents, cx| agents.task_ask_shown(cx));
            // Only the window the bar shows over opens one.
            let front = front().0;
            let shells: Vec<_> = WINDOWS.with_borrow(|windows| {
                windows
                    .iter()
                    .map(|window| (window.id == front, window.shell.clone()))
                    .collect()
            });
            for (shown, shell) in shells {
                shell
                    .update(cx, |shell, cx| shell.set_ask_slot(shown, cx))
                    .ok();
            }
        } else {
            update_shells_in(cx, |shell, cx| shell.set_ask_slot(false, cx));
        }
    }

    fn task_page(&self, cx: &App) -> Option<(SharedString, SharedString)> {
        let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
        let shell = shell?.upgrade()?;
        let shell = shell.read(cx);
        let tab = shell::shown_tab(cx);
        let url = shell.page(tab).url;
        let web = url.starts_with("https://") || url.starts_with("http://");
        web.then(|| (url.into(), shell.title(tab).into()))
    }

    fn run_task(
        &self,
        agent: &str,
        asked: &str,
        page: Option<(SharedString, SharedString)>,
        tier: browser::agents::tier::Tier,
        model: Option<String>,
        cx: &mut App,
    ) {
        let Some(cli) = browser::agents::runner::Cli::from_id(agent) else {
            return;
        };
        let page = page.map(|(url, title)| browser::agents::task::AskedOn { url, title });
        // The bar folds into the slot at the foot that its task's card takes, where the window
        // shows one; elsewhere it hides, and the card fades in.
        let window = front().0;
        let still = browser::motion::still(cx);
        let from = rgba_u32(browser::palette::current(cx).overlay);
        let folded = match update_window_in(window, cx, |shell, _| shell.fold_target()).flatten() {
            Some((to, radius, fill)) => {
                use browser::tokens::shell::task::motion::{CROSSFADE, FOLD, FOLD_FADE};
                let fade = if still { CROSSFADE } else { FOLD_FADE };
                unsafe {
                    platform::host_fold_overlay(
                        COMMAND_BAR,
                        f32::from(to.origin.x).into(),
                        f32::from(to.origin.y).into(),
                        f32::from(to.size.width).into(),
                        f32::from(to.size.height).into(),
                        radius.into(),
                        from,
                        rgba_u32(fill.into()),
                        FOLD.into(),
                        fade.into(),
                        still,
                    )
                }
            }
            None => {
                unsafe { platform::host_hide_overlay(COMMAND_BAR) };
                false
            }
        };
        // The bar has closed on no tab, so New Tab gives back the plate ⌘T gave it.
        update_window_in(window, cx, |shell, cx| {
            shell.command_bar_dismissed(cx);
            shell.fold_started(folded, cx);
            shell.bar_starting();
        });
        start_thread(cli, asked, page, tier, model, window, cx);
    }

    fn run_task_at_rest(
        &self,
        agent: &str,
        asked: &str,
        tier: browser::agents::tier::Tier,
        model: Option<String>,
        cx: &mut App,
    ) {
        let Some(cli) = browser::agents::runner::Cli::from_id(agent) else {
            return;
        };
        let window = front().0;
        update_window_in(window, cx, |shell, _| shell.bar_starting());
        start_thread(cli, asked, None, tier, model, window, cx);
    }

    fn set_up_agents(&self, cx: &mut App) {
        settings::show_agents(cx);
    }

    fn composer_last(
        &self,
        cx: &App,
    ) -> (browser::agents::tier::Tier, Option<(SharedString, String)>) {
        let Some(agents) = browser::agents::global(cx) else {
            return Default::default();
        };
        let last = agents.read(cx).composer_last(front_space(cx));
        let model = last.model.map(|(cli, model)| (cli.id().into(), model));
        (last.tier, model)
    }

    fn sign_in(&self, agent: &str, _: &mut App) {
        let Some(cli) = browser::agents::runner::Cli::from_id(agent) else {
            return;
        };
        agents_core::update(move |agents, cx| agents.sign_in(cli, cx));
    }

    fn tooltip(&self, tip: Option<(tooltip::Tip, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            // Its buttons are in the bar's window, which the tooltip goes over.
            Some((tip, anchor)) => {
                tooltip::hover_over(tooltip::Over::Overlay(COMMAND_BAR), tip, anchor, cx)
            }
            None => tooltip::dismiss(cx),
        }
    }
}

/// What the composer and Settings' `A new thread starts with` offer: the assistants signed in, the
/// default first, and those installed but not signed in.
pub(crate) fn task_agents_of(agents: &browser::agents::Agents) -> command_bar::TaskAgents {
    let listed = (agents.ask_agents().into_iter()).map(|agent| command_bar::TaskAgent {
        id: agent.cli.id().into(),
        name: agent.cli.name().into(),
        glyph: agent.cli.glyph(),
        model: agents.task_model_name(agent.cli),
        model_id: agents.task_model(agent.cli).to_string(),
        default_row: browser::agents::task::model_default(agent.cli).into(),
        default_model: (agents.task_default_model(agent.cli)).map(|name| name.to_string().into()),
        models: agents.task_models(agent.cli),
        resets: agent.resets,
    });
    let signed_out = (agents.signed_out_agents().into_iter())
        .map(|cli| (cli.id().into(), cli.name().into()))
        .collect();
    command_bar::TaskAgents {
        agents: listed.collect(),
        known: agents.task_agents_known(),
        signed_out,
    }
}

/// The Space shown in the front window, whose profile the composer remembers what it last started
/// a thread with in.
pub(crate) fn front_space(cx: &App) -> u64 {
    let space = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
    (space.and_then(|shell| shell.upgrade())).map_or(0, |shell| shell.read(cx).space())
}

/// Starts the composer's thread, `asked` on `cli` with `page`, from the Space shown in the front
/// window, at `tier` on `model`, which the profile remembers as the composer's last once it starts.
/// Its conversation then takes the place of what `window` shows: ⌘T's slot, or a resting bar's
/// empty page.
fn start_thread(
    cli: browser::agents::runner::Cli,
    asked: &str,
    page: Option<browser::agents::task::AskedOn>,
    tier: browser::agents::tier::Tier,
    model: Option<String>,
    window: bc::bc_id,
    cx: &mut App,
) {
    let home = front_space(cx);
    let asked = asked.to_string();
    agents_core::update(move |agents, cx| {
        let start = browser::agents::task::Start {
            tier,
            model: model.clone(),
            pages: Vec::new(),
            only_added: false,
            // The bar's composer has no `+` yet: a path reaches a task only from the panel.
            files: Default::default(),
        };
        let started = agents.start_task(cli, &asked, page, home, start, cx);
        if started.is_some() {
            agents.remember_composer(home, tier, model.map(|model| (cli, model)));
        }
        // Its conversation takes the page's place as its card comes, which takes the slot; one
        // that didn't start leaves the slot empty and the page as it was.
        update_window_in(window, cx, |shell, _| {
            shell.composed_started(started.clone().map(Into::into))
        });
        if started.is_none() {
            update_shells_in(cx, |shell, cx| shell.set_ask_slot(false, cx));
        }
    });
}

/// `color` as the host takes a colour, 0xRRGGBBAA.
pub(crate) fn rgba_u32(color: gpui::Rgba) -> u32 {
    let byte = |value: f32| (value.clamp(0., 1.) * 255.).round() as u32;
    byte(color.r) << 24 | byte(color.g) << 16 | byte(color.b) << 8 | byte(color.a)
}

/// The user left the command bar without choosing anything in it.
pub(crate) extern "C" fn command_bar_dismissed() {
    update_app(command_bar::dismissed);
    crate::settings::bar_hidden();
    update_window(front().0, |shell, cx| shell.command_bar_dismissed(cx));
}
