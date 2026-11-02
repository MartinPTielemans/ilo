//! Little ilo, Arc's Little Arc (docs/ux-spec.md › Little Arc): one page in a small window with no
//! sidebar, under a bar in its Space's colour with the page's favicon, host and title, a click on
//! which opens the command bar on its URL, and Copy Link and Open in Main Window at its end.

use gpui::{
    Bounds, Context, Entity, MouseButton, Pixels, SharedString, div, point, prelude::*, px, size,
    svg,
};

use super::{
    Command, NAV, NAV_HOVERED, NAV_RADIUS, SECONDARY, Shell, Space, SpaceEntry, Swapped, TITLE,
    URL_BUTTON_GLYPH, URL_BUTTON_SIZE, browser::Browser, favicon, host,
    swap::{CHECK, LINK},
};
use crate::theme::Theme;
use crate::tokens::shell::{CARD_INSET, LITTLE_BAR, LITTLE_TITLE_X, WINDOW_BUTTON_Y};
use crate::tokens::text;

/// The gap between the bar's parts (ours).
const GAP: f32 = 6.;

/// A Little ilo window's own: the user's Space it's for, whose look it takes and which ⌘O moves
/// its page to.
pub(super) struct Little {
    pub(super) home: u64,
}

impl Shell {
    /// A Little ilo window's shell: it shows `space`, the window's own, holding its page in
    /// `profile`, for the user's Space `home`, whose look it takes from what `from` knows of it.
    /// It hears nothing of the other Spaces' tabs, the favorites or the Library.
    pub fn little_window(
        browser: std::rc::Rc<dyn Browser>,
        space: u64,
        home: u64,
        profile: u64,
        from: Option<&Entity<Shell>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut shell = Self::new(browser, cx);
        shell.little = Some(Little { home });
        shell.space = space;
        shell.sidebar_hidden = true;
        let look = from.and_then(|from| {
            let from = from.read(cx);
            from.spaces.iter().find(|each| each.id == home).cloned()
        });
        shell.spaces = vec![Space {
            id: space,
            name: SharedString::default(),
            color: None,
            theme: None,
            icon: SharedString::default(),
            agent: SharedString::default(),
            profile,
            pinned: Vec::new(),
            today: Vec::new(),
            splits: Vec::new(),
            folders: Default::default(),
            shown: 0,
        }];
        if let Some(look) = look {
            shell.take_look(look.name, look.color, look.theme, look.icon);
        }
        shell
    }

    /// The user's Space this Little ilo window is for, or None for any other window.
    pub fn little(&self) -> Option<u64> {
        self.little.as_ref().map(|little| little.home)
    }

    /// The window's own Space shows its home's name, colour, theme and icon.
    fn take_look(
        &mut self,
        name: SharedString,
        color: Option<u32>,
        theme: Option<Theme>,
        icon: SharedString,
    ) {
        if let Some(own) = self.spaces.first_mut() {
            own.name = name;
            own.color = color;
            own.theme = theme;
            own.icon = icon;
        }
    }

    /// The Spaces, as `set_spaces` has them, for a Little ilo window: only its home's look, which
    /// its own Space takes.
    pub(super) fn set_little_spaces(&mut self, spaces: Vec<SpaceEntry>, cx: &mut Context<Self>) {
        let Some(home) = self.little() else {
            return;
        };
        let Some((_, name, color, _, icon, _, theme)) =
            spaces.into_iter().find(|&(id, ..)| id == home)
        else {
            return;
        };
        self.take_look(name, color, Theme::parse(&theme), icon);
        self.update_title(cx);
        cx.notify();
    }

    /// An Esc the page didn't use closes a Little ilo window, once there's no load for it to stop
    /// and no page full screen for it to leave (Arc), and says whether it did.
    pub fn escape_unused(&mut self, cx: &mut Context<Self>) -> bool {
        if self.little.is_none() || self.page(self.selected).loading || self.page_fullscreen {
            return false;
        }
        // check.sh reads this line.
        log!("browser: little window closed by escape");
        self.browser.close_window(cx);
        true
    }

    /// ⌘O: its page goes, live, to its Space in the main window, and the Little ilo window closes.
    pub(super) fn open_in_main_window(&mut self, cx: &mut Context<Self>) {
        if let Some(home) = self.little()
            && self.selected != 0
        {
            self.browser.open_in_main_window(self.selected, home, cx);
        }
    }

    /// The card in a Little ilo window, under its bar, or `card` in any other.
    pub(super) fn little_card(&self, card: Bounds<Pixels>) -> Bounds<Pixels> {
        if self.little.is_none() || self.page_fullscreen {
            return card;
        }
        let top = px(LITTLE_BAR);
        Bounds::new(point(card.origin.x, top), size(card.size.width, card.bottom() - top))
    }

    /// Where the bar's title goes, which a click opens the command bar over, as the URL field's
    /// does in the sidebar.
    fn little_title_bounds(&self) -> Bounds<Pixels> {
        let buttons = 2. * URL_BUTTON_SIZE + GAP;
        let right = self.viewport.width - px(buttons + CARD_INSET);
        Bounds::new(
            point(px(LITTLE_TITLE_X), px(WINDOW_BUTTON_Y - URL_BUTTON_SIZE / 2.)),
            size(right - px(LITTLE_TITLE_X + GAP), px(URL_BUTTON_SIZE)),
        )
    }

    /// Where ⌘L's compact command bar opens in a Little ilo window: over its title.
    pub(super) fn little_field_bounds(&self) -> Option<Bounds<Pixels>> {
        self.little.as_ref().map(|_| self.little_title_bounds())
    }

    /// The bar, over the window's tint: the page's favicon, host and title, then Copy Link and
    /// Open in Main Window. None while the page is full screen.
    pub(super) fn little_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.little.is_none() || self.page_fullscreen {
            return None;
        }
        let tab = self.selected;
        let url = self.url(tab);
        let bounds = self.little_title_bounds();
        let icon = self.tabs.get(&tab).and_then(|tab| tab.favicon.clone());
        let site: SharedString = host(&url).to_string().into();
        // A page with no title of its own is named by its host, which the bar shows already.
        let title = match tab {
            0 => SharedString::default(),
            _ => Some(self.title(tab)).filter(|title| *title != site).unwrap_or_default(),
        };
        let title = div()
            .id("little-title")
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .px(px(GAP))
            .flex()
            .items_center()
            .gap(px(GAP))
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
            .text_size(px(text::BODY))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.pressed_bar_target(cx)),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.clicked_bar_target(Command::EditUrl, "little bar", cx)
            }))
            .child(div().flex_none().child(favicon(&self.palette, icon)))
            .child(div().flex_none().text_color(self.palette.text(SECONDARY)).child(site))
            .child(
                div().min_w(px(0.)).truncate().text_color(self.palette.text(TITLE)).child(title),
            );
        let button = |id: &'static str| {
            div()
                .id(id)
                .flex_none()
                .size(px(URL_BUTTON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(NAV_RADIUS))
                .block_mouse_except_scroll()
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
        };
        let copy = button("little-copy")
            .on_click(cx.listener(|shell, _, _, cx| shell.copy_url_from_button(cx)))
            .child(self.swaps.glyph(
                Swapped::CopyUrl,
                if self.url_copied.is_some() { CHECK } else { LINK },
                URL_BUTTON_GLYPH,
                self.palette.glyph(NAV),
            ));
        let main = button("little-main")
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.run(Command::OpenInMainWindow, "little bar", cx)
            }))
            .child(
                svg()
                    .path("icons/external-link.svg")
                    .size(px(URL_BUTTON_GLYPH))
                    .text_color(self.palette.glyph(NAV)),
            );
        let buttons = div()
            .absolute()
            .right(px(CARD_INSET))
            .top(px(WINDOW_BUTTON_Y - URL_BUTTON_SIZE / 2.))
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(copy)
            .child(main);
        let bar = div().absolute().top_0().left_0().right_0().h(px(LITTLE_BAR));
        Some(bar.child(title).child(buttons))
    }
}

/// Whether a Little ilo window runs `command`: it has one page and no sidebar, so what works on the
/// Spaces, the sidebar, the Library or other tabs isn't its, as it isn't a private window's.
pub(super) fn little_runs(command: Command) -> bool {
    matches!(command, Command::NewLittleWindow)
        || (super::commands::private_runs(command)
            && !matches!(
                command,
                Command::NewTab
                    | Command::SearchTabs
                    | Command::NextTab
                    | Command::PreviousTab
                    | Command::SelectTab(_)
                    | Command::SelectLastTab
                    | Command::DuplicateTab
                    | Command::RenameTab
                    | Command::RevealTab
                    | Command::Clear
                    | Command::ToggleSidebar
                    | Command::AddSplit
            ))
}
