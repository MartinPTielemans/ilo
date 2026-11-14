//! The sidebar's header: the buttons to hide the sidebar and go back, forward and
//! reload, and the URL field under them.

use std::{cell::RefCell, collections::HashMap, rc::Rc};
use web_time::Instant;

use crate::command_bar::PLACEHOLDER_TEXT;
use crate::site_controls::Place;

use gpui::{
    App, Bounds, ClickEvent, Context, DispatchPhase, Div, Hsla, MouseButton, MouseDownEvent,
    MouseUpEvent, NavigationDirection, Pixels, SharedString, Stateful, Window, canvas, div, point,
    prelude::*, px, size, svg,
};

use super::{
    Command, FULL_SCREEN_SIDEBAR_BUTTON_X, HeldNav, NAV, NAV_AFTER_SIDEBAR, NAV_DISABLED,
    NAV_GLYPH, NAV_HOVERED, NAV_INACTIVE, NAV_PITCH, NAV_PRESS_SCALE, NAV_RADIUS, NAV_SIZE,
    NAV_TOP, NAV_TRAIL, PLATE_INSET, PLATE_RADIUS, SECONDARY, SIDEBAR_BUTTON_X, Shell, Swapped,
    TILE_HOVERED, TOOLS_FADE, Translation, URL_BUTTON_GLYPH, URL_BUTTON_INSET, URL_BUTTON_SIZE,
    URL_FIELD, URL_FIELD_HEIGHT, URL_FIELD_TEXT_X, URL_FIELD_TOP,
    access::Accessible,
    beside::AtRest,
    commands::BAR_COMMANDS,
    drag::held_depth,
    fade_at, host,
    scaled::scaled_from_middle,
    swap::{CHECK, LINK},
    user_keys,
};
use crate::tooltip::{Anchor, Tip};

/// Where a sidebar button's tooltip hangs: under it, as a tile's does, or right of it, as Arc's
/// does beside the footer's Library button.
#[derive(Clone, Copy)]
pub(super) enum TipSide {
    Under,
    Beside,
}

use crate::passwords::Key;
use crate::tokens::shell::header::KEY_GAP;
use crate::tokens::shell::header::KEY_GLYPH;
use crate::tokens::shell::header::KEY_REACH;
use crate::tokens::shell::header::SITE_CONTROLS_END;
use crate::tokens::shell::header::TOOL_PITCH;
use crate::tokens::shell::header::URL_PLACEHOLDER;
use crate::tokens::text;

impl Shell {
    /// The tab whose page the header is about, the one in the page area: a task's page watched in
    /// place, none while a task's conversation is there, else the tab selected. The URL field
    /// shows its address, and its buttons act on it.
    pub(super) fn field_tab(&self) -> u64 {
        match self.task_area {
            Some(_) => self.task_page_live().unwrap_or(0),
            None => self.selected,
        }
    }

    /// A header button at `x`, with the glyph `icon`, that runs `command`. One that can't dims, as
    /// back does on a page with no history.
    fn nav_button(
        &self,
        x: f32,
        icon: &'static str,
        command: Command,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let color = self.nav_glyph(enabled);
        // Reload and stop are one button, which changes with the tab shown's page.
        let glyph = match matches!(command, Command::Reload | Command::Stop) {
            true => self.swaps.glyph(
                Swapped::Reload(self.field_tab()),
                icon,
                NAV_GLYPH,
                color,
                cx,
            ),
            false => svg()
                .path(icon)
                .size(px(NAV_GLYPH))
                .text_color(color)
                .into_any_element(),
        };
        let button = div()
            .id(command.name())
            .absolute()
            .left(px(x))
            .top(px(NAV_TOP))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll();
        let name = self.button_name(command).unwrap_or(command.name());
        let button = button.when(self.a11y, |button| {
            button
                .button_named(name)
                .a11y_synthetic_children(move |builder| {
                    if !enabled {
                        builder.parent_node().set_disabled();
                    }
                })
        });
        // Back and forward list the tab's history that way, held or right-clicked (history.rs).
        let lists = matches!(command, Command::Back | Command::Forward);
        // A dimmed one names itself too, as the system's tooltips do.
        let button = self
            .with_tooltip(button, command, TipSide::Under, cx)
            .when(enabled, |button| {
                button
                    .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                    .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        shell.click_nav(command, event.modifiers(), cx)
                    }))
                    .on_aux_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        if let ClickEvent::Mouse(click) = event
                            && click.up.button == MouseButton::Middle
                        {
                            shell.open_history(command, cx)
                        }
                    }))
            })
            // Back, forward and reload shrink a little while they're held.
            .when(enabled && command != Command::ToggleSidebar, |button| {
                let release = |shell: &mut Shell,
                               _: &MouseUpEvent,
                               _: &mut Window,
                               cx: &mut Context<Shell>| {
                    shell.release_nav();
                    shell.let_go_nav(cx)
                };
                button
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.hold_nav(x, command, cx);
                            shell.press_nav(command, event.position, cx)
                        }),
                    )
                    .on_mouse_up(MouseButton::Left, cx.listener(release))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(release))
            })
            // Reload's lists its kinds of reload, right-clicked, also while it's stop (menus.rs).
            .when(
                matches!(command, Command::Reload | Command::Stop),
                |button| {
                    button.on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.reload_menu(event.position, cx)
                        }),
                    )
                },
            )
            .when(enabled && lists, |button| {
                button.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                        shell.ask_history(command == Command::Forward, event.position, cx)
                    }),
                )
            })
            .child(glyph);
        scaled_from_middle(button, self.nav_scale(x))
    }

    /// The pointer pressed the header button at `x`, which runs `command`.
    fn hold_nav(&mut self, x: f32, command: Command, cx: &mut Context<Self>) {
        let pressed = Instant::now();
        self.held_nav = Some(HeldNav {
            x,
            command,
            pressed,
            let_go: None,
            drawn_in: false,
        });
        cx.notify();
    }

    /// The pointer let go of the header button it pressed, or its menu took the press over.
    pub(super) fn let_go_nav(&mut self, cx: &mut Context<Self>) {
        if let Some(held) = self.held_nav.as_mut().filter(|held| held.let_go.is_none()) {
            held.let_go = Some(Instant::now());
            cx.notify();
        }
    }

    /// How small the header button at `x` is drawn, plate and glyph: smaller while it's held,
    /// growing back from where it is once it's let go, as a row's plate does.
    pub(super) fn nav_scale(&self, x: f32) -> f32 {
        let Some(held) = self.held_nav.as_ref().filter(|held| held.x == x) else {
            return 1.;
        };
        1. - (1. - NAV_PRESS_SCALE) * held_depth(held.pressed, held.let_go, Instant::now())
    }

    /// Lets the held header button go once it has grown back, and asks for the next frame only
    /// while it's shrinking or growing, so a button at rest costs no frames.
    pub(super) fn step_held_nav(&mut self, window: &mut Window) {
        let Some(held) = &mut self.held_nav else {
            return;
        };
        let name = held.command.name();
        match (
            held.let_go,
            held_depth(held.pressed, held.let_go, Instant::now()),
        ) {
            (Some(_), 0.) => {
                // check.sh waits for these lines.
                log!("browser: nav {name} at rest drawn");
                self.held_nav = None;
            }
            (None, 1.) if !held.drawn_in => {
                held.drawn_in = true;
                log!("browser: nav {name} pressed drawn");
            }
            (None, 1.) => {}
            _ => window.request_animation_frame(),
        }
    }

    /// Gives `button`, a sidebar button's plate that runs `command`, a tooltip with the command's
    /// name and the keys of the shortcut the menus show for it, as Arc's Library button has, hung
    /// `side` of the plate. A press ends it.
    pub(super) fn with_tooltip(
        &self,
        button: Stateful<Div>,
        command: Command,
        side: TipSide,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        button
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                shell.hover_button(command, side, *hovered, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.browser.tooltip(None, cx)),
            )
            .child(self.button_frame(command.name()))
    }

    /// Keeps the frame a button is drawn in as `key`, the name of the command it runs if it runs
    /// one, for its tooltip to hang from, since GPUI gives a hover handler no bounds.
    pub(super) fn button_frame(&self, key: &'static str) -> impl IntoElement + use<> {
        let frames = self.button_frames.clone();
        canvas(
            move |bounds, _, _| {
                frames.borrow_mut().insert(key, bounds);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    }

    /// The pointer has come onto or left the button that runs `command`.
    pub(super) fn hover_button(
        &self,
        command: Command,
        side: TipSide,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self.button_frames.borrow().get(command.name()).copied();
        let tip = (hovered && !busy)
            .then(|| self.button_name(command))
            .flatten()
            .zip(frame)
            .map(|(name, frame)| {
                let keys = user_keys(cx)
                    .chips(command)
                    .into_iter()
                    .map(Into::into)
                    .collect();
                let anchor = match side {
                    TipSide::Under => Anchor::under(frame, self.viewport),
                    TipSide::Beside => Anchor::beside(frame, self.viewport, self.right),
                };
                (
                    Tip {
                        text: name.into(),
                        keys,
                    },
                    anchor,
                )
            });
        self.hover_tip(tip, cx);
    }

    /// What a sidebar button's tooltip calls `command`: the menus' name, as Arc's Library button
    /// says "View Library" where its command bar says "Open Library…", else the bar's. The sidebar
    /// button's is what Arc labels it, what a click does now.
    pub(super) fn button_name(&self, command: Command) -> Option<&'static str> {
        match command {
            Command::ToggleSidebar if self.sidebar_hidden => Some("Lock Sidebar"),
            Command::ToggleSidebar => Some("Hide Sidebar"),
            Command::ViewLibrary => Some("View Library"),
            Command::ViewDownloads => Some("View Downloads"),
            // A translated page's mark opens the card that takes it back, so it says what it shows.
            Command::TranslatePage
                if self.translations.get(&self.field_tab()) == Some(&Translation::Translated) =>
            {
                Some("Translated")
            }
            _ => (BAR_COMMANDS.iter())
                .find(|&&(_, each, _)| each == command)
                .map(|&(name, ..)| name),
        }
    }

    /// The window became the one the user is in, or stopped being, as the host says: its header's
    /// glyphs take the disabled alpha while it isn't, as Arc's do.
    pub fn set_window_main(&mut self, main: bool, cx: &mut Context<Self>) {
        if self.window_main != main {
            self.window_main = main;
            cx.notify();
        }
    }

    /// A header button's glyph colour: the disabled alpha while it's disabled, raised to the glyph
    /// floor, and the same alpha as it is, under the floor, while the window isn't the user's.
    pub(super) fn nav_glyph(&self, enabled: bool) -> Hsla {
        match (self.window_main, enabled) {
            (false, _) => self.palette.faint(NAV_INACTIVE),
            (true, true) => self.palette.glyph(NAV),
            (true, false) => self.palette.glyph(NAV_DISABLED),
        }
    }

    /// The sidebar button, back, forward, and reload, which is stop while the page loads, as many
    /// as fit.
    pub(super) fn nav_buttons(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let dimmed = !self.window_main;
        if self.nav_dimmed_drawn.replace(Some(dimmed)) != Some(dimmed) {
            // check.sh reads this line.
            log!("browser: nav glyphs dimmed {dimmed}");
        }
        let page = self.page(self.field_tab());
        let [sidebar, back, forward, reload_x] =
            nav_places(self.width(), self.full_screen, self.right);
        let reload = match page.loading {
            true => (reload_x, reload_glyph(true), Command::Stop, true),
            false => (
                reload_x,
                reload_glyph(false),
                Command::Reload,
                self.field_tab() != 0,
            ),
        };
        if self.reload_drawn.replace(Some(reload.2)) != Some(reload.2) {
            // check.sh reads this line: a press hits the button the window drew last, which
            // follows the page's loading a frame behind.
            log!("browser: nav shows {}", reload.2.name());
        }
        if self.back_drawn.replace(Some(page.can_go_back)) != Some(page.can_go_back) {
            // check.sh reads this line too, before a click on back.
            log!(
                "browser: nav back drawn {}",
                if page.can_go_back { "on" } else { "dimmed" }
            );
        }
        let buttons = [
            (
                sidebar,
                match self.right {
                    true => "icons/nav-sidebar-right.svg",
                    false => "icons/nav-sidebar.svg",
                },
                Command::ToggleSidebar,
                true,
            ),
            (back, "icons/nav-back.svg", Command::Back, page.can_go_back),
            (
                forward,
                "icons/nav-forward.svg",
                Command::Forward,
                page.can_go_forward,
            ),
            reload,
        ];
        buttons
            .into_iter()
            .filter_map(|(x, icon, command, enabled)| Some((x?, icon, command, enabled)))
            .map(|(x, icon, command, enabled)| {
                self.nav_button(x, icon, command, enabled, cx)
                    .into_any_element()
            })
            .collect()
    }

    /// A mouse's back and forward buttons anywhere over the UI walk the row a swipe pages, the
    /// Library, the Spaces and a new Space, a step a press (the owner's call, #1852); over a page,
    /// the page takes them itself and goes back and forward. The window hears them, as the
    /// sidebar keeps its presses from what's under it.
    pub(super) fn navigate_listener(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    let MouseButton::Navigate(direction) = event.button else {
                        return;
                    };
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let back = direction == NavigationDirection::Back;
                    shell
                        .update(cx, |shell, cx| shell.navigate(back, window, cx))
                        .ok();
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// The URL field's box in the window, while the docked sidebar shows it.
    pub(super) fn url_field_bounds(&self) -> Option<Bounds<Pixels>> {
        let docked = !self.page_fullscreen && !self.sidebar_hidden && self.sidebar_motion.is_none();
        (docked && self.form().is_none() && self.library.is_none()).then(|| {
            Bounds::new(
                point(px(self.column_x() + PLATE_INSET), px(URL_FIELD_TOP)),
                size(px(self.width() - 2. * PLATE_INSET), px(URL_FIELD_HEIGHT)),
            )
        })
    }

    /// Where the Site Controls button ends, in from the URL field's end: Arc's place, or before
    /// the pinned extensions' buttons.
    fn site_controls_end(&self) -> f32 {
        let (shown, overflow) = self.shown_actions();
        match shown + overflow as usize {
            0 => SITE_CONTROLS_END,
            places => URL_BUTTON_INSET + places as f32 * URL_BUTTON_SIZE,
        }
    }

    /// Opens the Site Controls under the URL field, hanging from their button, or with the field
    /// hidden at the page's top left corner, as an extension's popup is; or closes them. They list
    /// every turned-on extension, the pinned ones first, which the core reports once they show.
    /// With a request to show notifications held, they open at Notifications, its row ringed, so
    /// the bell's press and the keyboard's way in, the command bar's Site Controls, answer it alike
    /// (#2323).
    pub(super) fn site_controls(&mut self, cx: &mut Context<Self>) {
        if self.field_tab() == 0 {
            return;
        }
        if self.held_notifications(self.field_tab()).is_some() {
            crate::site_controls::open_at_permission("notifications", cx);
        }
        let place = match self.url_field_bounds() {
            Some(field) => {
                let left = field.right() - px(self.site_controls_end() + URL_BUTTON_SIZE);
                let button = Bounds::new(
                    point(left, field.top() + px(URL_BUTTON_INSET)),
                    size(px(URL_BUTTON_SIZE), px(URL_BUTTON_SIZE)),
                );
                Place::Button { field, button }
            }
            None => Place::Corner(point(px(self.card_corner() + PLATE_INSET), px(PLATE_INSET))),
        };
        let extensions = self.site_controls_extensions();
        let profiles = self.site_controls_profiles();
        self.browser.site_controls(
            self.field_tab(),
            place,
            self.viewport,
            extensions,
            profiles,
            cx,
        );
    }

    /// The tab's profile and the user's others, for the Site Controls' chip, while there's more
    /// than one (#2339). A window with a Space of its own has none: a private one's session shares
    /// none of the profile's logins, and Little ilo's bar has its own picker.
    pub(super) fn site_controls_profiles(&self) -> Option<crate::site_controls::Profiles> {
        if self.private || self.blank || self.little.is_some() || self.profiles.len() < 2 {
            return None;
        }
        let current = self.shown_space()?.profile;
        let name = (self.profiles.iter())
            .find(|(id, _)| *id == current)?
            .1
            .clone();
        let others = (self.profiles.iter())
            .filter(|(id, _)| *id != current)
            .map(|(id, name)| (*id, name.clone(), self.users_space_in(*id).is_some()))
            .collect();
        Some(crate::site_controls::Profiles { name, others })
    }

    /// The first of the user's Spaces in `profile`, in the sidebar's order: agents' aren't theirs.
    fn users_space_in(&self, profile: u64) -> Option<u64> {
        (self.spaces.iter())
            .find(|space| space.profile == profile && space.agent.is_empty())
            .map(|space| space.id)
    }

    /// Site Controls' Open in <profile> (#2339): the tab's page in a new tab of the first of the
    /// user's Spaces in `profile`, shown, as Little ilo's picker goes to that profile's first
    /// Space. It loads fresh there, as the profile's cookies are its own (decision 5).
    pub fn open_in_profile(&mut self, tab: u64, profile: u64, cx: &mut Context<Self>) {
        let url = self.page(tab).url;
        let Some(space) = self.users_space_in(profile) else {
            return;
        };
        self.show_space(space, cx);
        self.browser.open_page(&url, cx);
    }

    /// Site Controls' New Space in <profile>…, for a profile none of the user's Spaces is in: New
    /// Space's form, its Space in `profile`.
    pub fn new_space_in_profile(&mut self, profile: u64, cx: &mut Context<Self>) {
        self.new_space_from(None, Some(profile), cx);
    }

    /// A button at the URL field's end, `end` in from it, shown while the pointer is over the
    /// sidebar and taking clicks only then.
    fn field_tool(&self, id: &'static str, end: f32) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .opacity(self.tools_opacity())
            .absolute()
            .top(px(URL_BUTTON_INSET))
            .right(px(end))
            .size(px(URL_BUTTON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
    }

    /// What the URL field says, in its colour: the page's host, or with no page shown, the
    /// command bar's placeholder, there from the frame the card's mark is, or at once with a
    /// task's conversation, which shows at once.
    pub(super) fn url_field_words(&self, url: &str, cx: &App) -> (SharedString, Hsla) {
        let shown = match self.task_area {
            Some(_) => 1.,
            None => self.empty_opacity(cx.background_executor().now()),
        };
        self.url_words(self.field_tab(), url, shown)
    }

    /// What a URL field showing `tab`, at `url`, says, in its colour: the page's host, its whole
    /// URL in Developer Mode, as Chrome's Always Show Full URLs has it, or with no tab, the command
    /// bar's placeholder, `shown` of the way in.
    pub(super) fn url_words(&self, tab: u64, url: &str, shown: f32) -> (SharedString, Hsla) {
        if tab == 0 {
            let mut color = self.palette.text(URL_PLACEHOLDER);
            color.a *= shown;
            return (PLACEHOLDER_TEXT.into(), color);
        }
        let words = match self.extension_name(url) {
            Some(name) => name,
            None if self.page(tab).developer_mode => url.to_string().into(),
            None => host(url).to_string().into(),
        };
        (words, self.palette.text(SECONDARY).into())
    }

    pub(super) fn url_field(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let url = self.url(self.field_tab());
        let (words, color) = self.url_field_words(&url, cx);
        if *self.url_field_drawn.borrow() != words {
            // check.sh reads this line.
            log!("browser: url field {words}");
            self.url_field_drawn.replace(words.clone());
        }
        // The pinned extensions' actions, at the field's end, and Copy URL and Site Controls
        // before them, where the host stops short of them.
        let actions = self.action_buttons(cx);
        let buttons_width = actions.len() as f32 * URL_BUTTON_SIZE;
        let live = self.tools_live();
        let tools = self.tools_shown() && !url.is_empty();
        let end = self.site_controls_end();
        let copy = tools.then(|| {
            let button = self.field_tool("copy-url", end + TOOL_PITCH);
            let button = button.when(self.a11y, |button| button.button_named("Copy URL"));
            self.with_tooltip(button, Command::CopyUrl, TipSide::Under, cx)
                // Fading out, it takes no click.
                .when(live, |button| {
                    button
                        .block_mouse_except_scroll()
                        .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                        .on_click(cx.listener(|shell, _, _, cx| shell.copy_url_from_button(cx)))
                })
                .child(self.swaps.glyph(
                    Swapped::CopyUrl,
                    if self.url_copied.is_some() {
                        CHECK
                    } else {
                        LINK
                    },
                    URL_BUTTON_GLYPH,
                    self.palette.glyph(NAV).into(),
                    cx,
                ))
        });
        // While a request to show notifications is held quietly, the Site Controls button is its
        // bell, at rest as well as under the pointer: a press opens them at Notifications, whose
        // Allow or Block answers it (#2323). One button, so nothing moves as the tools fade in.
        let held = self.held_notifications(self.field_tab());
        let site_controls = (tools || held.is_some()).then(|| {
            let tab = self.field_tab();
            let from = if held.is_some() { "bell" } else { "sidebar" };
            let name: SharedString = match &held {
                Some(site) => format!("{site} wants to send notifications").into(),
                None => "Site Controls".into(),
            };
            self.field_tool("site-controls", end)
                .when(held.is_some(), |button| button.opacity(1.))
                .when(self.a11y, |button| {
                    button
                        .button_named(name.clone())
                        .pressed_by_mouse_down(true, false)
                })
                .when(live || held.is_some(), |button| {
                    button
                        .block_mouse_except_scroll()
                        .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                        // On the press, as a pop-up button opens: a click while the panel shows
                        // closes it first, and its press then toggles rather than opens it again.
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |shell, _, _, cx| {
                                cx.stop_propagation();
                                shell.browser.tooltip(None, cx);
                                shell.run(Command::SiteControls, from, cx);
                            }),
                        )
                })
                .when(held.is_some(), |button| {
                    button
                        .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                            let frame = shell.bell_drawn.get();
                            let tip = (*hovered && shell.dragging().is_none())
                                .then(|| {
                                    frame.map(|frame| {
                                        (
                                            Tip::named(name.clone()),
                                            Anchor::under(frame, shell.viewport),
                                        )
                                    })
                                })
                                .flatten();
                            shell.hover_tip(tip, cx);
                        }))
                        .child(super::permission::card_listener(
                            "notification bell",
                            &self.bell_drawn,
                            tab,
                        ))
                })
                .child(
                    svg()
                        .path(match held {
                            Some(_) => "icons/bell-off.svg",
                            None => "icons/sliders-horizontal.svg",
                        })
                        .size(px(URL_BUTTON_GLYPH))
                        .text_color(self.palette.glyph(NAV)),
                )
        });
        // In Developer Mode, DevTools before them, as ⌥⌘I opens them (ours, as Arc's buttons
        // aren't recorded). Agents get no DevTools, so it's the user's alone.
        let devtools = (tools && self.page(self.field_tab()).developer_mode).then(|| {
            let button = self.field_tool("devtools", end + 2. * TOOL_PITCH);
            let button = button.when(self.a11y, |button| button.button_named("Developer Tools"));
            self.with_tooltip(button, Command::DevTools, TipSide::Under, cx)
                .when(live, |button| {
                    button
                        .block_mouse_except_scroll()
                        .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.run(Command::DevTools, "sidebar", cx)
                        }))
                })
                .child(
                    svg()
                        .path(DEVTOOLS_GLYPH)
                        .size(px(URL_BUTTON_GLYPH))
                        .text_color(self.palette.glyph(NAV)),
                )
        });
        // While Battery Saver is on, its leaf, before the tools and in that place while they're
        // gone, so it says so whether or not the pointer's over the sidebar. A press shows its
        // toast again, with Turn Off (battery.rs).
        let slots = if self.page(self.field_tab()).developer_mode {
            3.
        } else {
            2.
        };
        let leaf_end = end + slots * TOOL_PITCH;
        let leaf = self.battery_saver.map(|_| {
            let drawn = self.battery_leaf_drawn.clone();
            let frame = canvas(
                move |bounds, _, _| {
                    if drawn.replace(Some(bounds)) != Some(bounds) {
                        let (origin, size) = (bounds.origin, bounds.size);
                        // check.sh reads this line.
                        log!(
                            "browser: battery saver leaf at {} {} {} {}",
                            f32::from(origin.x),
                            f32::from(origin.y),
                            f32::from(size.width),
                            f32::from(size.height),
                        );
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full();
            self.field_tool("battery-saver", leaf_end)
                .opacity(1.)
                .when(self.a11y, |button| button.button_named("Battery Saver"))
                .block_mouse_except_scroll()
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.show_battery_notice(cx);
                    }),
                )
                .child(
                    svg()
                        .path("icons/leaf.svg")
                        .size(px(URL_BUTTON_GLYPH))
                        .text_color(self.palette.accent),
                )
                .child(frame)
        });
        if held.is_none() {
            self.bell_drawn.set(None);
        }
        let engine = self.engine_mark(self.field_tab());
        // The password manager's key, after the host (ours, for Chrome's key icon): a click brings
        // back the offer showing another tab closed, or opens the Site Controls, which list the
        // site's logins. On the press, as Site Controls' button opens them.
        let key = (self.field_tab() != 0)
            .then(|| self.keys.get(&self.field_tab()).copied())
            .flatten();
        let key = key.map(|key| {
            div()
                .id("key")
                .flex_none()
                .ml(px(KEY_GAP - KEY_REACH))
                .p(px(KEY_REACH))
                .when(self.a11y, |button| button.button_named("Passwords"))
                // It lights under the pointer as the field's other buttons do (#2112).
                .rounded(px(NAV_RADIUS))
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, _, _, cx| {
                        cx.stop_propagation();
                        shell.key_clicked(key, cx);
                    }),
                )
                .relative()
                .child(self.key_glyph())
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(key_frame(self.button_frames.clone())),
                )
        });
        // A page in another language offers to translate it, after the key (translate.rs).
        let translate = self.translate_mark(cx);
        let text_end = match tools {
            true if devtools.is_some() => end + 2. * TOOL_PITCH + URL_BUTTON_SIZE,
            true => end + TOOL_PITCH + URL_BUTTON_SIZE,
            false if actions.is_empty() => URL_FIELD_TEXT_X - PLATE_INSET,
            false => URL_BUTTON_INSET + buttons_width,
        };
        let text_end = match leaf {
            Some(_) => text_end.max(leaf_end + URL_BUTTON_SIZE),
            None => text_end,
        };
        let text_end = match held {
            Some(_) => text_end.max(end + URL_BUTTON_SIZE),
            None => text_end,
        };
        // A button, as it is: a press opens the command bar on the URL, where it's edited. It's
        // called what it shows, the host.
        let field = div().id("url-field").when(self.a11y, |field| {
            field
                .button_named(words.clone())
                .a11y_synthetic_children(|builder| {
                    builder.parent_node().set_role_description("address")
                })
        });
        field
            .absolute()
            .top(px(URL_FIELD_TOP))
            .left(px(PLATE_INSET))
            .w(px(self.width() - 2. * PLATE_INSET))
            .h(px(URL_FIELD_HEIGHT))
            .flex()
            .items_center()
            .pl(px(URL_FIELD_TEXT_X - PLATE_INSET))
            .pr(px(text_end))
            .rounded(px(PLATE_RADIUS))
            .block_mouse_except_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.pressed_bar_target(cx)),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.clicked_bar_target(Command::EditUrl, "sidebar", cx)
            }))
            .bg(self.palette.plate(URL_FIELD))
            // It brightens under the pointer as a tile does, from the same white 0.10 (ours).
            .hover(|style| style.bg(self.palette.plate(TILE_HOVERED)))
            .text_size(px(text::BODY))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(color)
                    .child(words),
            )
            .children(engine)
            .children(key)
            .children(translate)
            .children(leaf)
            .children(devtools)
            .children(copy)
            .children(site_controls)
            .children(actions)
    }

    /// What of the URL field rests `x` in from its left, as `url_field` lays it out: its Copy URL,
    /// Site Controls or DevTools button while they take clicks, a pinned extension's action, or
    /// the field.
    pub(super) fn field_part_at(&self, x: f32) -> AtRest {
        let width = self.width() - 2. * PLATE_INSET;
        let (shown, overflow) = self.shown_actions();
        let actions = (shown + overflow as usize) as f32 * URL_BUTTON_SIZE;
        if actions > 0. && x >= width - URL_BUTTON_INSET - actions {
            return AtRest::Action;
        }
        let tools =
            self.tools_shown() && self.tools_live() && !self.url(self.field_tab()).is_empty();
        let button = |end: f32| (width - end - URL_BUTTON_SIZE..width - end).contains(&x);
        let developer = self.page(self.field_tab()).developer_mode;
        match tools {
            true if button(self.site_controls_end()) => AtRest::SiteControls,
            true if button(self.site_controls_end() + TOOL_PITCH) => AtRest::CopyUrl,
            true if developer && button(self.site_controls_end() + 2. * TOOL_PITCH) => {
                AtRest::DevTools
            }
            _ => AtRest::Field,
        }
    }

    /// The password manager's key for `tab`'s page, or none for a page it has nothing for.
    pub fn set_key(&mut self, tab: u64, key: Option<Key>, cx: &mut Context<Self>) {
        let changed = match key {
            Some(key) => self.keys.insert(tab, key) != Some(key),
            None => self.keys.remove(&tab).is_some(),
        };
        if changed && tab == self.field_tab() {
            cx.notify();
        }
    }

    fn key_clicked(&mut self, key: Key, cx: &mut Context<Self>) {
        let tab = self.field_tab();
        // check.sh reads this line.
        log!("browser: key {tab} clicked");
        match key {
            Key::Offer => self.browser.show_offer(tab),
            Key::Saved => self.run(Command::SiteControls, "key", cx),
        }
    }

    /// The key's glyph, quiet as the engine's mark.
    fn key_glyph(&self) -> gpui::Svg {
        let mut quiet = self.palette.text(SECONDARY);
        quiet.a *= 0.6;
        svg()
            .path("icons/key-round.svg")
            .size(px(KEY_GLYPH))
            .text_color(quiet)
    }

    /// A tab showing its page in Safari's engine says so after the host, quietly (ours): the Site
    /// Controls name it and turn it off.
    fn engine_mark(&self, tab: u64) -> Option<gpui::Svg> {
        (tab != 0 && self.page(tab).system_engine).then(|| {
            let mut quiet = self.palette.text(SECONDARY);
            quiet.a *= 0.6;
            svg()
                .path("icons/compass.svg")
                .flex_none()
                .ml(px(5.))
                .size(px(12.))
                .text_color(quiet)
        })
    }

    /// The URL field on the page of a Space beside the shown one's, as it shows `tab` once that
    /// Space is shown, with Copy URL and the Site Controls while the shown field has them, but not
    /// the pinned extensions' actions, which are its profile's. It takes no input.
    pub(super) fn still_url_field(&self, tab: u64) -> Div {
        let url = self.url(tab);
        let (words, color) = self.url_words(tab, &url, 1.);
        let tools = self.tools_shown() && !url.is_empty();
        let tool = |end: f32, glyph: &'static str| {
            div()
                .opacity(self.tools_opacity())
                .absolute()
                .top(px(URL_BUTTON_INSET))
                .right(px(end))
                .size(px(URL_BUTTON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(glyph)
                        .size(px(URL_BUTTON_GLYPH))
                        .text_color(self.palette.glyph(NAV)),
                )
        };
        let devtools = tools && self.page(tab).developer_mode;
        // A held request's bell, in the Site Controls' place at rest too (#2323).
        let held = self.held_notifications(tab).is_some();
        let text_end = match tools {
            true if devtools => SITE_CONTROLS_END + 2. * TOOL_PITCH + URL_BUTTON_SIZE,
            true => SITE_CONTROLS_END + TOOL_PITCH + URL_BUTTON_SIZE,
            false if held => SITE_CONTROLS_END + URL_BUTTON_SIZE,
            false => URL_FIELD_TEXT_X - PLATE_INSET,
        };
        let field = div()
            .absolute()
            .top(px(URL_FIELD_TOP))
            .left(px(PLATE_INSET))
            .w(px(self.width() - 2. * PLATE_INSET))
            .h(px(URL_FIELD_HEIGHT))
            .flex()
            .items_center()
            .pl(px(URL_FIELD_TEXT_X - PLATE_INSET))
            .pr(px(text_end))
            .rounded(px(PLATE_RADIUS))
            .bg(self.palette.plate(URL_FIELD))
            .text_size(px(text::BODY))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(color)
                    .child(words),
            )
            .children(self.engine_mark(tab))
            .children(self.keys.contains_key(&tab).then(|| {
                let reach = div()
                    .flex_none()
                    .ml(px(KEY_GAP - KEY_REACH))
                    .p(px(KEY_REACH));
                reach.child(self.key_glyph())
            }))
            .when(tools, |field| {
                field.child(tool(SITE_CONTROLS_END + TOOL_PITCH, LINK))
            })
            .when(tools && !held, |field| {
                field.child(tool(SITE_CONTROLS_END, "icons/sliders-horizontal.svg"))
            })
            .when(held, |field| {
                field.child(tool(SITE_CONTROLS_END, "icons/bell-off.svg").opacity(1.))
            })
            .when(devtools, |field| {
                field.child(tool(SITE_CONTROLS_END + 2. * TOOL_PITCH, DEVTOOLS_GLYPH))
            });
        #[cfg(test)]
        let field = field.child(self.beside_framed(0, 0.));
        field
    }

    /// Whether the sidebar draws Clear and the URL field's button, which it does from the pointer
    /// coming over it until they've faded out again (Arc).
    pub(super) fn tools_shown(&self) -> bool {
        self.tools_opacity() > 0.
    }

    /// Whether they take clicks: while the pointer is over the sidebar, and not while they fade
    /// out. The reveal's panel shows only under the pointer. VoiceOver's cursor is no pointer,
    /// so they stay for it.
    pub(super) fn tools_live(&self) -> bool {
        self.sidebar_hovered || self.sidebar_hidden || self.a11y
    }

    /// How far they've faded in. The reveal's panel shows them at once, as the sidebar does while
    /// an assistive app reads it.
    pub(super) fn tools_opacity(&self) -> f32 {
        if self.sidebar_hidden || self.a11y {
            return 1.;
        }
        let to = if self.tools_on { 1. } else { 0. };
        self.tools_fade.map_or(to, |(start, from)| {
            fade_at(from, to, start.elapsed().as_secs_f32(), TOOLS_FADE)
        })
    }

    /// Starts the fade as the pointer comes or goes, from where it is, and runs frames only while
    /// it does.
    pub(super) fn step_tools(&mut self, window: &mut Window) {
        if self.sidebar_hidden {
            self.tools_fade = None;
            self.tools_on = self.sidebar_hovered;
        } else if self.sidebar_hovered != self.tools_on {
            self.tools_fade = Some((Instant::now(), self.tools_opacity()));
            self.tools_on = self.sidebar_hovered;
        }
        if let Some((start, _)) = self.tools_fade {
            if start.elapsed().as_secs_f32() < TOOLS_FADE {
                window.request_animation_frame();
                return;
            }
            self.tools_fade = None;
        }
        let shown = self.tools_shown();
        if shown != self.drawn_tools {
            self.drawn_tools = shown;
            // check.sh waits for these lines, so a capture after one shows that state.
            log!(
                "browser: sidebar tools {}",
                if shown { "shown" } else { "hidden" }
            );
        }
    }
}

/// Keeps the frame the key is drawn in, as button_frame does, and says where it is when it moves,
/// for check.sh to click.
fn key_frame(frames: Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if frames.borrow_mut().insert("key", bounds) != Some(bounds) {
                let at = bounds.center();
                log!(
                    "browser: key drawn at {:.0} {:.0}",
                    f32::from(at.x),
                    f32::from(at.y)
                );
            }
        },
        |_, _, _, _| {},
    )
    .size_full()
}

/// The URL field's DevTools button's glyph in Developer Mode: Lucide's code, the command bar's
/// View Source glyph, beside Developer Mode's own square-code.
const DEVTOOLS_GLYPH: &str = "icons/code.svg";

/// The header's reload, which is stop while the page loads.
pub(super) fn reload_glyph(loading: bool) -> &'static str {
    if loading {
        "icons/nav-stop.svg"
    } else {
        "icons/nav-reload.svg"
    }
}

/// Where the header's buttons' plates start in a sidebar `width` wide: the sidebar button, then
/// back, forward and reload, each None where it doesn't fit. The sidebar button follows the traffic
/// lights, or starts near the sidebar's edge in full screen, where they're gone (Arc 1.171). The
/// others keep to the sidebar's right edge, NAV_TRAIL in and NAV_PITCH apart (Arc 1.170), and a
/// narrow sidebar loses reload, then forward, while back would come nearer the sidebar button than
/// NAV_AFTER_SIDEBAR. With the sidebar on the `right` the row is mirrored, the sidebar button by the
/// window's buttons at the right edge, but back, forward and reload keep their order, back NAV_TRAIL
/// from the page (#2375).
pub(super) fn nav_places(width: f32, full_screen: bool, right: bool) -> [Option<f32>; 4] {
    let sidebar = if full_screen {
        FULL_SCREEN_SIDEBAR_BUTTON_X
    } else {
        SIDEBAR_BUTTON_X
    };
    let first = sidebar + NAV_SIZE + NAV_AFTER_SIDEBAR;
    let last = width - NAV_TRAIL - NAV_SIZE;
    let count = (0..=3)
        .rev()
        .find(|&count| last - (count as f32 - 1.) * NAV_PITCH >= first);
    let mut places = [Some(sidebar), None, None, None];
    if let Some(count) = count.filter(|&count| count > 0) {
        for index in 0..count {
            places[1 + index] = Some(match right {
                true => NAV_TRAIL + index as f32 * NAV_PITCH,
                false => last - (count - 1 - index) as f32 * NAV_PITCH,
            });
        }
    }
    if right {
        places[0] = Some(width - sidebar - NAV_SIZE);
    }
    places
}

#[cfg(test)]
mod tests;
