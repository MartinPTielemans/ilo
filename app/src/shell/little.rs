//! Little ilo, Arc's Little Arc (docs/ux-spec.md › Little Arc): one page in a small window with no
//! sidebar, under a bar in its Space's colour with the page's favicon, host and title, a click on
//! which opens the command bar on its URL, and Copy Link and Open in <Space> ⌘O | ⌄ at its end,
//! after the profile picker while there's more than one profile.
//!
//! The window is the main window with its sidebar hidden: the same material and colours, the card
//! at the same inset and radius. The bar's row is the main window's header row, its buttons the
//! header's, and the favicon stands where the header's first button does.

use std::cell::Cell;

use gpui::{
    App, Bounds, Context, Entity, FontWeight, MouseButton, Pixels, Point, SharedString, Window,
    div, point, prelude::*, px, relative, size, svg,
};

use super::{
    BarSpace, CARD_INSET, Command, DialogKind, MenuEntry, NAV, NAV_HOVERED, NAV_RADIUS, PageDialog,
    Shell, Space, SpaceEntry, Swapped, TITLE, URL_BUTTON_INSET, URL_BUTTON_SIZE, WINDOW_BUTTONS,
    access::Accessible,
    browser::Browser,
    favicon,
    header::TipSide,
    host,
    swap::{CHECK, LINK},
};
use crate::theme::Theme;
use crate::tokens::command_bar::{LITTLE_DOWN, LITTLE_WIDTH, ROW_HEIGHT};
use crate::tokens::shell::{
    FAVICON_DRAWN, LITTLE_BAR, LITTLE_CHEVRON, LITTLE_CHEVRON_GLYPH, LITTLE_COPY_GAP, LITTLE_GLYPH,
    LITTLE_OPEN, LITTLE_OPEN_GAP, LITTLE_OPEN_KEY, LITTLE_OPEN_MAX, LITTLE_OPEN_PAD,
    LITTLE_SPACES_BELOW, LITTLE_SPACES_EDGE, LITTLE_SPACES_WIDTH, LITTLE_SPLIT, LITTLE_SPLIT_LINE,
    NAV_SIZE, NAV_TOP, SIDEBAR_BUTTON_X,
};
use crate::tokens::text;
use crate::tooltip::text_width;

/// The gap between the bar's title and its buttons, and between the title's parts (ours).
const GAP: f32 = 6.;

/// The bar's buttons are the header's pitch apart, back to forward.
const PITCH: f32 = super::NAV_PITCH;

/// A Little ilo window's own: the user's Space it's for, whose look it takes and which ⌘O moves
/// its page to, the user's Spaces, in the sidebar's order, which its Spaces menu offers, and
/// whether it has drawn on screen yet, and where it last logged its profile picker.
pub(super) struct Little {
    pub(super) home: u64,
    spaces: Vec<Elsewhere>,
    drawn: bool,
    picker_at: Cell<Option<Point<Pixels>>>,
    /// The tabs its page opened, which went behind: their pages' questions to open another app
    /// are asked here, where the user is, as a sign-in's pop-up's hand-back is (#1914).
    pub(super) sent: Vec<u64>,
    /// Where its bar's title went in its last draw, at whose end its extensions' buttons are.
    pub(super) title: Cell<Option<Bounds<Pixels>>>,
    /// Whether it's about to close by itself, as a sign-in's page another app opened ends (#2327).
    closing: bool,
}

/// One of the user's Spaces, as a Little ilo window knows it: enough to name and mark it in its
/// Spaces menu and to know whether its page would go to another profile there.
struct Elsewhere {
    id: u64,
    name: SharedString,
    profile: u64,
    color: Option<u32>,
    theme: Option<Theme>,
}

/// Where the bar's end lies, from the window's right edge: Open in <Space>'s chevron half, the line
/// between its halves, its words' half, and the middles of Copy Link and the profile picker.
pub(super) struct BarEnd {
    pub(super) chevron: Bounds<Pixels>,
    pub(super) line: Pixels,
    pub(super) words: Bounds<Pixels>,
    pub(super) copy: Point<Pixels>,
    pub(super) profile: Point<Pixels>,
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
        shell.space = space;
        shell.sidebar_hidden = true;
        // The profiles too, for the bar's picker, which the core reports again only as they change.
        let mut spaces = Vec::new();
        let look = from.and_then(|from| {
            let from = from.read(cx);
            shell.profiles = from.profiles.clone();
            spaces = (from.spaces.iter())
                .filter(|each| each.agent.is_empty())
                .map(|each| Elsewhere {
                    id: each.id,
                    name: each.name.clone(),
                    profile: each.profile,
                    color: each.color,
                    theme: each.theme,
                })
                .collect();
            from.spaces.iter().find(|each| each.id == home).cloned()
        });
        shell.little = Some(Little {
            home,
            spaces,
            drawn: false,
            picker_at: Cell::new(None),
            sent: Vec::new(),
            title: Cell::new(None),
            closing: false,
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
            peeked: Vec::new(),
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
        let users = (spaces.iter())
            .filter(|(.., agent, _)| agent.is_empty())
            .map(|(id, name, color, profile, _, _, theme)| Elsewhere {
                id: *id,
                name: name.clone(),
                profile: *profile,
                color: *color,
                theme: Theme::parse(theme),
            });
        if let Some(little) = self.little.as_mut() {
            little.spaces = users.collect();
        }
        let Some((_, name, color, _, icon, _, theme)) =
            spaces.into_iter().find(|&(id, ..)| id == home)
        else {
            return;
        };
        let theme = Theme::parse(&theme);
        if self
            .spaces
            .first()
            .is_some_and(|own| (own.color, own.theme) != (color, theme))
        {
            // check.sh reads this line, as the main window's "themed space".
            let text = theme.map_or("none".to_string(), |theme| theme.to_text());
            log!("browser: little window for {home} looks {text}");
        }
        self.take_look(name, color, theme, icon);
        self.update_title(cx);
        cx.notify();
    }

    /// Settings › Features' Little ilo switch: off, File, the command bar and ⌥⌘N offer no Little
    /// ilo window, and ⌥⌘-click on a row is a click. The windows open stay.
    pub fn set_little_windows(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.little_windows == on {
            return;
        }
        self.little_windows = on;
        self.update_menus(cx);
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
        let users = self
            .shown_space()
            .is_some_and(|space| space.agent.is_empty());
        // ⌘ is Control off the Mac, where the platform key is Super.
        let takes = event.modifiers.alt && event.modifiers.secondary(cx) && !event.modifiers.shift;
        // With Little ilo off it's a click, as on any row.
        if !takes
            || !users
            || self.one_space()
            || !self.little_windows
            || self.little.is_some()
            || self.is_folder(tab)
        {
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
        if let Some(home) = self.little() {
            self.open_in_space(home, cx);
        }
    }

    /// Its page goes to the user's Space `space` in the main window, and the Little ilo window
    /// closes: live in its own profile, and in another, warned the first time as Move to Space
    /// warns, as a page loaded again there.
    pub(super) fn open_in_space(&mut self, space: u64, cx: &mut Context<Self>) {
        let (Some(little), tab) = (self.little.as_ref(), self.selected) else {
            return;
        };
        let Some(to) = little.spaces.iter().find(|each| each.id == space) else {
            return;
        };
        let own = self.spaces.first().map(|own| own.profile);
        if tab == 0 || own == Some(to.profile) || !self.browser.warn_moving_profiles(cx) {
            if tab != 0 {
                self.browser.open_in_main_window(tab, space, cx);
            }
            return;
        }
        let name = (self.profiles.iter())
            .find(|(profile, _)| *profile == to.profile)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let shell = cx.weak_entity();
        self.browser.confirm(
            format!("Move this tab to the “{name}” profile?"),
            "Its page loads again there, without this profile's cookies and logins, so a site may \
             ask you to sign in."
                .into(),
            "Move",
            Box::new(move |cx| {
                shell
                    .update(cx, |shell, cx| {
                        shell.browser.set_warn_moving_profiles(false, cx);
                        shell.browser.open_in_main_window(tab, space, cx);
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// The chevron's Spaces menu: the compact command bar hanging under Open in <Space>, its
    /// right edge the button's, listing the user's Spaces to send the page to (Arc).
    /// Its opening grows from its first frame on screen, which AppKit gives some frames after it's
    /// asked to, so the motion's start is seen (Arc's Little Arc).
    pub(super) fn step_little(&mut self, window: &Window, cx: &mut App) {
        let Some(little) = self.little.as_mut().filter(|little| !little.drawn) else {
            return;
        };
        if window.is_visible() {
            little.drawn = true;
            self.browser.little_drawn(cx);
        }
    }

    fn little_spaces_menu(&mut self, cx: &mut Context<Self>) {
        let Some(little) = self.little.as_ref() else {
            return;
        };
        let spaces = (little.spaces.iter())
            .map(|each| BarSpace {
                space: each.id,
                name: each.name.clone(),
                swatch: self.swatch_of(each.theme, each.color),
            })
            .collect();
        let end = self.little_end(cx);
        let right = end
            .chevron
            .right()
            .min(self.viewport.width - px(LITTLE_SPACES_EDGE));
        let width = px(LITTLE_SPACES_WIDTH).min(right);
        let top = end.chevron.bottom() + px(LITTLE_SPACES_BELOW);
        let at = Bounds::new(point(right - width, top), size(width, px(ROW_HEIGHT)));
        self.browser.spaces_menu(spaces, at, cx);
    }

    /// Open in <Space>'s words: Open in and its home Space's name, and the key ⌘O is now.
    pub(super) fn little_open_words(&self, cx: &App) -> (SharedString, Option<SharedString>) {
        let name = self
            .spaces
            .first()
            .map(|own| own.name.clone())
            .unwrap_or_default();
        let key = super::user_keys(cx)
            .label(Command::OpenInMainWindow)
            .map(Into::into);
        (format!("Open in {name}").into(), key)
    }

    /// Where the bar's end lies in the window: the chevron's half, its plate ending where a header
    /// button's would whose glyph is as far in from the window's right edge as the close button's
    /// is from its left, so the row's ends mirror each other; the line; then the words' half as
    /// wide as its words, at most LITTLE_OPEN_MAX; and Copy Link's plate and the profile picker's,
    /// a pitch apart, before it.
    pub(super) fn little_end(&self, cx: &App) -> BarEnd {
        let right = self.viewport.width - px(WINDOW_BUTTONS[0].0 - NAV_SIZE / 2.);
        let top = px(NAV_TOP);
        let plate =
            |left: Pixels, width: f32| Bounds::new(point(left, top), size(px(width), px(NAV_SIZE)));
        let chevron = plate(right - px(LITTLE_CHEVRON), LITTLE_CHEVRON);
        let line = chevron.left() - px(1.);
        let (words, key) = self.little_open_words(cx);
        let system = gpui::WindowTextSystem::new(cx.text_system().clone());
        let measured = text_width(&words, text::BODY, FontWeight::MEDIUM, &system)
            + key.map_or(0., |key| {
                LITTLE_OPEN_GAP + text_width(&key, text::BODY, FontWeight::NORMAL, &system)
            });
        let width = (2. * LITTLE_OPEN_PAD + measured).min(LITTLE_OPEN_MAX);
        let words = plate(line - px(width), width);
        let middle = px(NAV_TOP + NAV_SIZE / 2.);
        let copy = point(words.left() - px(LITTLE_COPY_GAP + NAV_SIZE / 2.), middle);
        let profile = point(copy.x - px(PITCH), middle);
        BarEnd {
            chevron,
            line,
            words,
            copy,
            profile,
        }
    }

    /// The profiles the bar's picker offers, each with whether the window's page is in it, or
    /// None with only one, when there's nothing to pick.
    fn little_profiles(&self) -> Option<Vec<(u64, SharedString, bool)>> {
        let own = self.spaces.first()?.profile;
        let profiles = self
            .profiles
            .iter()
            .map(|(id, name)| (*id, name.clone(), *id == own));
        Some(profiles.collect()).filter(|profiles: &Vec<_>| profiles.len() > 1)
    }

    /// The profile picker's menu, hanging from its button: the profiles, the page's checked. One
    /// picked opens the page in it and is kept for its site (Arc's Little Arc).
    fn little_profile_menu(&mut self, cx: &mut Context<Self>) {
        let Some(profiles) = self.little_profiles() else {
            return;
        };
        let entries = (profiles.iter())
            .map(|(_, name, own)| MenuEntry::named(name.clone(), true).checked(*own))
            .collect();
        // From its plate's left edge, 4 under it.
        let plate = self.little_end(cx).profile;
        let at = point(
            plate.x - px(NAV_SIZE / 2.),
            plate.y + px(NAV_SIZE / 2. + 4.),
        );
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            at,
            false,
            self.menu_look(),
            Box::new(move |chosen, cx| {
                let Some(&(profile, ..)) = chosen.and_then(|index| profiles.get(index)) else {
                    return;
                };
                // check.sh reads this line.
                log!("browser: little profile {profile} via bar");
                shell
                    .update(cx, |shell, cx| {
                        let url = shell.url(shell.selected);
                        shell.browser.little_profile(profile, &url, cx)
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Whether the window is about to close by itself, its sign-in done: its bar says so in place
    /// of the page's title, with the key that brings it back, so the window going reads as meant.
    pub fn set_little_closing(&mut self, closing: bool, cx: &mut Context<Self>) {
        if let Some(little) = &mut self.little
            && little.closing != closing
        {
            little.closing = closing;
            // check.sh reads this line.
            log!("browser: little bar closing {closing}");
            cx.notify();
        }
    }

    /// The bar's title after the page's host: the page's own, unless the window is about to close
    /// by itself, when it says so and names the key that reopens it, as a message one can undo
    /// names its undo's key.
    pub(super) fn little_bar_title(
        &self,
        tab: u64,
        site: &str,
        url: &str,
        cx: &App,
    ) -> SharedString {
        if self.little.as_ref().is_some_and(|little| little.closing) {
            return match super::user_keys(cx).label(Command::ReopenClosedTab) {
                Some(key) => format!("Signed in, closing · {key} reopens").into(),
                None => "Signed in, closing".into(),
            };
        }
        // A page with no title of its own is named by its host, which the bar shows already, as
        // part of the whole URL in Developer Mode.
        match tab {
            0 => SharedString::default(),
            _ => Some(self.title(tab))
                .filter(|title| title.as_ref() != site && title.as_ref() != host(url))
                .unwrap_or_default(),
        }
    }

    /// `tab`, which its page opened, went behind: while no window shows it, its question to open
    /// another app is asked here.
    pub fn little_sent(&mut self, tab: u64, cx: &mut Context<Self>) {
        if let Some(little) = &mut self.little {
            little.sent.push(tab);
            cx.notify();
        }
    }

    /// The tab its page opened and sent behind whose page asks to open another app, with what it
    /// asks, if one does.
    pub(super) fn little_sent_asks(&self) -> Option<(u64, &PageDialog)> {
        let little = self.little.as_ref()?;
        (little.sent.iter()).find_map(|tab| {
            let dialog = self.page_dialogs.get(tab)?;
            (dialog.kind == DialogKind::OpenApp).then_some((*tab, dialog))
        })
    }

    /// The card in a Little ilo window, under its bar, or `card` in any other. While its page asks
    /// a question, the card goes under the question's, which hangs from the bar as it would from
    /// the URL field, since nothing can be drawn over the page.
    pub(super) fn little_card(&self, card: Bounds<Pixels>) -> Bounds<Pixels> {
        if self.little.is_none() || self.page_fullscreen {
            return card;
        }
        let asked = (self.question().is_some())
            .then(|| self.question_drawn.get())
            .flatten();
        let top = match asked {
            Some(question) => question.bottom() + px(CARD_INSET),
            None => px(LITTLE_BAR),
        };
        Bounds::new(
            point(card.origin.x, top),
            size(card.size.width, card.bottom() - top),
        )
    }

    /// Where the bar's title goes, which a click opens the command bar over, as the URL field's
    /// does in the sidebar: from where the header's first button is, the sidebar's, to the bar's
    /// buttons, a plate's height on the header's line.
    fn little_title_bounds(&self, cx: &App) -> Bounds<Pixels> {
        let end = self.little_end(cx);
        let first = if self.little_profiles().is_some() {
            end.profile
        } else {
            end.copy
        };
        let right = first.x - px(NAV_SIZE / 2. + GAP);
        Bounds::new(
            point(px(SIDEBAR_BUTTON_X), px(NAV_TOP)),
            size(right - px(SIDEBAR_BUTTON_X), px(NAV_SIZE)),
        )
    }

    /// Where a click on its bar's title opens the compact command bar in a Little ilo window: over
    /// the title, as a click on the URL field opens it over the field.
    pub(super) fn little_field_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.little.as_ref().map(|_| self.little_title_bounds(cx))
    }

    /// Where ⌘L and ⌘T open the command bar in a Little ilo window: inside it, centred across it,
    /// its top a third of the way down (Arc), where a key's surfaces are in this window.
    pub(super) fn little_bar_bounds(&self) -> Bounds<Pixels> {
        let width = px(LITTLE_WIDTH).min(self.viewport.width);
        let left = (self.viewport.width - width) / 2.;
        let top = self.viewport.height * LITTLE_DOWN;
        Bounds::new(
            point(left, top),
            size(width, px(crate::tokens::command_bar::HEIGHT)),
        )
    }

    /// The bar, over the window's tint: the page's favicon, host and title, then the profile
    /// picker, Copy Link and Open in <Space> ⌘O | ⌄. None while the page is full screen.
    pub(super) fn little_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.little.is_none() || self.page_fullscreen {
            return None;
        }
        let tab = self.selected;
        let url = self.url(tab);
        let row = self.little_title_bounds(cx);
        if let Some(little) = &self.little
            && little.title.replace(Some(row)) != Some(row)
        {
            // check.sh reads this line, to click the extensions' buttons at the title's end.
            log!(
                "browser: little bar title ends at {:.0} {:.0}",
                f32::from(row.right()),
                f32::from(row.center().y)
            );
        }
        // The pinned extensions' buttons end the title, as they end the URL field in the sidebar,
        // the title giving up their room.
        let (shown, overflow) = self.shown_actions();
        let actions = px((shown + overflow as usize) as f32 * URL_BUTTON_SIZE);
        let bounds = Bounds::new(row.origin, size(row.size.width - actions, row.size.height));
        let end = self.little_end(cx);
        // The page's host, or with no page the URL field's placeholder, which says what a click on
        // the bar does, in the URL field's colours.
        let (site, site_color) = self.url_words(tab, &url, 1.);
        let icon = (tab != 0).then(|| self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()));
        let title = self.little_bar_title(tab, &site, &url, cx);
        let closing = self.little.as_ref().is_some_and(|little| little.closing);
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
            // a long one leaves the title room, and the title gives up its room first, but for the
            // line that the window's closing, whose key to reopen it is what it's there to say.
            .child(
                div()
                    .when(!closing, |site| site.flex_shrink_0())
                    .when(closing, |site| site.min_w(px(0.)))
                    .max_w(relative(0.5))
                    .truncate()
                    .text_color(site_color)
                    .child(site),
            )
            .child(
                div()
                    .when(!closing, |title| title.flex_1().min_w(px(0.)))
                    .when(closing, |title| title.flex_none().max_w(relative(1.)))
                    .truncate()
                    .text_color(self.palette.text(TITLE))
                    .child(title),
            );
        let glyph = self.palette.glyph(NAV);
        // The header's buttons: its plates, hover and tooltips, with the key they share.
        let button = |id: &'static str,
                      middle: Point<Pixels>,
                      command: Option<Command>,
                      name: &'static str,
                      cx: &mut Context<Self>| {
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
            match command {
                Some(command) => self.with_tooltip(button, command, TipSide::Under, cx),
                None => button,
            }
        };
        let copy = button(
            "little-copy",
            end.copy,
            Some(Command::CopyUrl),
            "Copy URL",
            cx,
        )
        .on_click(cx.listener(|shell, _, _, cx| shell.copy_url_from_button(cx)))
        .child(self.swaps.glyph(
            Swapped::CopyUrl,
            if self.url_copied.is_some() {
                CHECK
            } else {
                LINK
            },
            LITTLE_GLYPH,
            glyph,
            cx,
        ));
        // Open in <Space> ⌘O | ⌄: one plate of the header's, split by a line, each half its own
        // hover. Its words name the Space and the key, so it needs no tooltip.
        let half = |id: &'static str, bounds: Bounds<Pixels>| {
            div()
                .id(id)
                .absolute()
                .left(bounds.left())
                .top(bounds.top())
                .w(bounds.size.width)
                .h(bounds.size.height)
                .flex()
                .items_center()
                .justify_center()
                .block_mouse_except_scroll()
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
        };
        let (words, key) = self.little_open_words(cx);
        let open =
            half("little-open", end.words)
                .when(self.a11y, |button| button.button_named(words.clone()))
                .px(px(LITTLE_OPEN_PAD))
                .gap(px(LITTLE_OPEN_GAP))
                .rounded_l(px(NAV_RADIUS))
                .text_size(px(text::BODY))
                .on_click(cx.listener(|shell, _, _, cx| {
                    shell.run(Command::OpenInMainWindow, "little bar", cx)
                }))
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(self.palette.text(LITTLE_OPEN))
                        .child(words),
                )
                .children(key.map(|key| {
                    div()
                        .flex_none()
                        .text_color(self.palette.text(LITTLE_OPEN_KEY))
                        .child(key)
                }));
        let line = div()
            .absolute()
            .left(end.line)
            .top(px(NAV_TOP + (NAV_SIZE - LITTLE_SPLIT) / 2.))
            .w(px(1.))
            .h(px(LITTLE_SPLIT))
            .bg(self.palette.plate(LITTLE_SPLIT_LINE));
        let spaces = half("little-spaces", end.chevron)
            .when(self.a11y, |button| {
                button.button_named("Open in Another Space")
            })
            .rounded_r(px(NAV_RADIUS))
            .on_click(cx.listener(|shell, _, _, cx| shell.little_spaces_menu(cx)))
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .size(px(LITTLE_CHEVRON_GLYPH))
                    .text_color(glyph),
            );
        // Profiles' glyph, as Settings' Profiles tab has it, and nothing with only one profile. It
        // runs no command, so it has no tooltip: its menu names the profiles.
        let profile = self.little_profiles().map(|_| {
            // check.sh clicks it where it's logged, as the words before it make its place vary.
            let at = Some(end.profile);
            if let Some(little) = self
                .little
                .as_ref()
                .filter(|little| little.picker_at.get() != at)
            {
                little.picker_at.set(at);
                let (x, y) = (f32::from(end.profile.x), f32::from(end.profile.y));
                log!("browser: little profile picker at {x:.0} {y:.0}");
            }
            button("little-profile", end.profile, None, "Profile", cx)
                .on_click(cx.listener(|shell, _, _, cx| shell.little_profile_menu(cx)))
                .child(
                    svg()
                        .path("icons/user.svg")
                        .size(px(LITTLE_GLYPH))
                        .text_color(glyph),
                )
        });
        let bar = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(LITTLE_BAR));
        // In a strip at the title's end, where the URL field's buttons place themselves from its
        // right, with its inset.
        let extensions = (actions > px(0.)).then(|| {
            let top = row.top() + (row.size.height - px(URL_BUTTON_SIZE)) / 2.;
            div()
                .absolute()
                .left(row.right() - actions - px(URL_BUTTON_INSET))
                .top(top - px(URL_BUTTON_INSET))
                .w(actions + px(2. * URL_BUTTON_INSET))
                .h(px(URL_BUTTON_SIZE + 2. * URL_BUTTON_INSET))
                .children(self.action_buttons(cx))
        });
        Some(
            bar.child(title)
                .children(extensions)
                .children(profile)
                .child(copy)
                .child(open)
                .child(line)
                .child(spaces),
        )
    }
}

/// Whether a Little ilo window runs `command`: it has one page and no sidebar, so what works on the
/// Spaces, the sidebar, the Library or other tabs isn't its, as it isn't a private window's. Its
/// page is in a profile's site settings, as a private window's isn't, so it has Developer Mode,
/// which ⌘O takes to the main window with it (Arc).
pub(super) fn little_runs(command: Command) -> bool {
    matches!(
        command,
        Command::NewLittleWindow | Command::ToggleDeveloperMode
    ) || (super::commands::private_runs(command)
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
                    | Command::MoveSidebar
                    | Command::AddSplit
                    // A private window's Library has its session's downloads; a Little ilo
                    // window has no Library at all.
                    | Command::ViewLibrary
                    | Command::ViewDownloads
                    | Command::ViewMedia
        ))
}
