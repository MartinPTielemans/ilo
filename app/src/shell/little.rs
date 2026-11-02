//! Little ilo, Arc's Little Arc (docs/ux-spec.md › Little Arc): one page in a small window with no
//! sidebar, under a bar in its Space's colour with the page's favicon, host and title, a click on
//! which opens the command bar on its URL, and Copy Link and Open in Main Window at its end.
//!
//! The window is the main window with its sidebar hidden: the same material and colours, the card
//! at the same inset and radius. The bar's row is the main window's header row, its buttons the
//! header's, and the favicon stands where the header's first button does.

use gpui::{
    Bounds, Context, Entity, MouseButton, Pixels, Point, SharedString, div, point, prelude::*, px,
    relative, size, svg,
};

use super::{
    Command, NAV, NAV_HOVERED, NAV_RADIUS, Shell, Space, SpaceEntry, Swapped, TITLE,
    WINDOW_BUTTONS,
    access::Accessible,
    host,
    browser::Browser,
    favicon,
    header::TipSide,
    swap::{CHECK, LINK},
};
use crate::theme::Theme;
use crate::tokens::shell::{
    BACK_X, FAVICON_DRAWN, FORWARD_X, LITTLE_BAR, LITTLE_GLYPH, NAV_SIZE, NAV_TOP,
    SIDEBAR_BUTTON_X,
};
use crate::tokens::text;

/// The gap between the bar's title and its buttons, and between the title's parts (ours).
const GAP: f32 = 6.;

/// The bar's buttons are the header's pitch apart, back to forward.
const PITCH: f32 = FORWARD_X - BACK_X;

/// The middle of the bar's button `index` from its right end, in a window `width` wide. The last
/// one's glyph is as far in from the window's right edge as the close button's is from its left,
/// so the row's ends mirror each other and its glyphs end where the window buttons start.
pub(super) fn little_button_middle(width: Pixels, index: usize) -> Point<Pixels> {
    let x = width - px(WINDOW_BUTTONS[0].0 + index as f32 * PITCH);
    point(x, px(NAV_TOP + NAV_SIZE / 2.))
}

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
        let theme = Theme::parse(&theme);
        if self.spaces.first().is_some_and(|own| (own.color, own.theme) != (color, theme)) {
            // check.sh reads this line, as the main window's "themed space".
            let text = theme.map_or("none".to_string(), |theme| theme.to_text());
            log!("browser: little window for {home} looks {text}");
        }
        self.take_look(name, color, theme, icon);
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

    /// ⌥⌘-click on a Today tab's row in the sidebar opens the tab in a Little ilo window, its page
    /// live (Arc), and says whether it did. Not a pinned tab or a favorite, which keep their place,
    /// a split view's, a folder, an agent's or a private window's: those take ⌥-click's split.
    pub(super) fn little_press_row(
        &mut self,
        tab: u64,
        event: &gpui::MouseDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let users = self.shown_space().is_some_and(|space| space.agent.is_empty());
        let takes = event.modifiers.alt && event.modifiers.platform && !event.modifiers.shift;
        if !takes || !users || self.private || self.little.is_some() || self.is_folder(tab) {
            return false;
        }
        if !self.today().contains(&tab) || self.panes(tab).len() > 1 {
            return false;
        }
        self.end_hovers(cx);
        // check.sh reads this line.
        log!("browser: command little via sidebar {tab}");
        self.browser.open_tab_in_little(tab, cx);
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

    /// How many buttons the bar has at its end: Copy Link and Open in Main Window.
    fn little_buttons(&self) -> usize {
        2
    }

    /// Where the bar's title goes, which a click opens the command bar over, as the URL field's
    /// does in the sidebar: from where the header's first button is, the sidebar's, to the bar's
    /// buttons, a plate's height on the header's line.
    fn little_title_bounds(&self) -> Bounds<Pixels> {
        let first = little_button_middle(self.viewport.width, self.little_buttons() - 1);
        let right = first.x - px(NAV_SIZE / 2. + GAP);
        Bounds::new(
            point(px(SIDEBAR_BUTTON_X), px(NAV_TOP)),
            size(right - px(SIDEBAR_BUTTON_X), px(NAV_SIZE)),
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
        // The page's host, or with no page the URL field's placeholder, which says what a click on
        // the bar does, in the URL field's colours.
        let (site, site_color) = self.url_words(tab, &url, 1.);
        let icon = (tab != 0).then(|| self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()));
        // A page with no title of its own is named by its host, which the bar shows already, as
        // part of the whole URL in Developer Mode.
        let title = match tab {
            0 => SharedString::default(),
            _ => Some(self.title(tab))
                .filter(|title| *title != site && title.as_ref() != host(&url))
                .unwrap_or_default(),
        };
        let title = div()
            .id("little-title")
            .when(self.a11y, |title| title.button_named(site.clone()))
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            // The favicon in the plate's first square, as a header button's glyph is in its own.
            .pl(px((NAV_SIZE - FAVICON_DRAWN) / 2.))
            .pr(px(GAP))
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
            .children(icon.map(|icon| div().flex_none().child(favicon(&self.palette, icon))))
            // Each ends in an ellipsis rather than a cut. The host keeps at most half the row, so
            // a long one leaves the title room, and the title gives up its room first.
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(relative(0.5))
                    .truncate()
                    .text_color(site_color)
                    .child(site),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(self.palette.text(TITLE))
                    .child(title),
            );
        let glyph = self.palette.glyph(NAV);
        // The header's buttons: its plates, hover and tooltips, with the key they share.
        let button = |id: &'static str,
                      index: usize,
                      command: Command,
                      name: &'static str,
                      cx: &mut Context<Self>| {
            let middle = little_button_middle(self.viewport.width, index);
            let button = div()
                .id(id)
                .when(self.a11y, |button| button.button_named(name))
                .absolute()
                .left(middle.x - px(NAV_SIZE / 2.))
                .top(middle.y - px(NAV_SIZE / 2.))
                .size(px(NAV_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(NAV_RADIUS))
                .block_mouse_except_scroll()
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)));
            self.with_tooltip(button, command, TipSide::Under, cx)
        };
        let copy = button("little-copy", 1, Command::CopyUrl, "Copy URL", cx)
            .on_click(cx.listener(|shell, _, _, cx| shell.copy_url_from_button(cx)))
            .child(self.swaps.glyph(
                Swapped::CopyUrl,
                if self.url_copied.is_some() { CHECK } else { LINK },
                LITTLE_GLYPH,
                glyph,
            ));
        let main = button("little-main", 0, Command::OpenInMainWindow, "Open in Main Window", cx)
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.run(Command::OpenInMainWindow, "little bar", cx)
            }))
            .child(svg().path("icons/external-link.svg").size(px(LITTLE_GLYPH)).text_color(glyph));
        let bar = div().absolute().top_0().left_0().right_0().h(px(LITTLE_BAR));
        Some(bar.child(title).child(copy).child(main))
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
