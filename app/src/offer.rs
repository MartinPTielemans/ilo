//! The offer to save a login's password, or to update the one saved, after a page submits it, or
//! to save the address or the card a page's form submitted (docs/ux-spec.md › Passwords and
//! autofill): a card at the top right of the page, in Chrome's words. GPUI can't draw over the
//! page's native view, so as the find bar is, each browser window's offer is a window of its own
//! that the host shows over it, as its child. The core keeps the password; the card shows the
//! username, which the user can edit or pick from the page's other fields, and the password masked.
//! An address's or a card's shows what it saves in the same two rows, to read. Showing another tab
//! closes the offer, as it closes Chrome's bubble, and coming back doesn't bring it back. What it
//! asks of its window and the core goes through `Host`.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight, Global, Hsla, KeyBinding,
    Pixels, Point, SharedString, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, actions, div, point, prelude::*, px, size,
    svg,
};

use crate::{
    palette::{Palette, STATE_HOVERED, STATE_PRESSED},
    settling::Settling,
    shell,
    text_input::{FieldMenu, TextInput},
};

actions!(offer, [Accept, NotNow]);

// Ours, as neither Arc's nor Dia's was measured: the panel grid (docs/ux-spec.md › Overlays), 8
// from the card's side and 8 below the page's top, its content 16 in, its title and site at 13/16
// and 11/14, and 32 pt rows.
pub use crate::tokens::offer::WIDTH;
pub const HEIGHT: f32 = INSET * 2.
    + TITLE_LINE
    + SITE_LINE
    + HEADER_GAP
    + ROW * 2.
    + ROW_GAP
    + BUTTONS_TOP
    + BUTTON_HEIGHT;
pub const RIGHT: f32 = shell::CARD_INSET + 8.;
pub const TOP: f32 = shell::CARD_INSET + 8.;
use crate::menu::Closed;
use crate::tokens::offer::INSET;
use crate::tokens::offer::RADIUS;
use crate::tokens::offer::TITLE_LINE;
use crate::tokens::offer::SITE_LINE;
use crate::tokens::offer::HEADER_GAP;
use crate::tokens::offer::ROW;
use crate::tokens::offer::ROW_GAP;
use crate::tokens::offer::FIELD_X;
use crate::tokens::offer::CHEVRON;
use crate::tokens::offer::GLYPH;
use crate::tokens::offer::BUTTONS_TOP;
use crate::tokens::offer::BUTTON_HEIGHT;
use crate::tokens::offer::BUTTON_X;
use crate::tokens::offer::BUTTON_GAP;
use crate::tokens::offer::PLATE_RADIUS;
use crate::tokens::offer::TITLE;
use crate::tokens::offer::DETAIL;
use crate::elevation::Level;
use crate::tokens::offer::PLATE;
use crate::tokens::offer::PLACEHOLDER;
use crate::tokens::text;

/// What an offer offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// To save the password of the login the page submitted.
    Save,
    /// To update the password saved for it.
    Update,
    /// To save the address the page's form submitted.
    Address,
    /// To save the card the page's form submitted, to this device.
    Card,
}

/// An offer from Chrome's password manager, as the core reports it, in Chrome's words.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub kind: Kind,
    /// Such as "Save password?".
    pub title: SharedString,
    /// Whose password it is, as the user should see it.
    pub site: SharedString,
    /// What it saves unless the user edits it.
    pub username: SharedString,
    /// The username, then the values the page's other fields had that might be it instead.
    pub usernames: Vec<SharedString>,
    /// How many characters the password has.
    pub password_length: usize,
    pub accept: SharedString,
    pub decline: SharedString,
    /// None where there's no Never, as for an update.
    pub never: Option<SharedString>,
    /// An address's or a card's: what it saves, a line each, as Chrome's bubble lists it.
    pub details: Vec<SharedString>,
}

impl Offer {
    /// It saves a password, which the user names in the field.
    fn of_login(&self) -> bool {
        matches!(self.kind, Kind::Save | Kind::Update)
    }
}

/// The user's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Save, or update.
    Accept,
    /// Never save one for the site.
    Never,
    /// Not now.
    NotNow,
}

/// What a browser window's offer asks of its window and the core.
pub trait Host {
    /// Shows the offer's window over the top right of the browser window's page.
    fn show(&self);
    /// Hides it.
    fn hide(&self);
    /// Answers `tab`'s offer with the username as the user left it.
    fn answer(&self, tab: u64, answer: Answer, username: &str);
}

thread_local! {
    // Every tab's open offer. Kept out of GPUI's entities, so a tab closing inside a GPUI update
    // drops its own.
    static OFFERS: RefCell<HashMap<u64, Offer>> = RefCell::new(HashMap::new());
}

pub struct OfferPrompt {
    host: Rc<dyn Host>,
    /// The window's colour, which the card takes, and the palette its text is drawn in.
    look: Box<dyn Fn(&App) -> (Hsla, Palette)>,
    input: Entity<TextInput>,
    /// The card's own, which has the keyboard for an address's or a card's offer, with no field.
    focus: FocusHandle,
    /// The tab the window shows, whose offer this is.
    tab: u64,
    /// The tab's page shows in another window, which shows the offer, and this one a picture of it.
    elsewhere: bool,
    /// Its moment on screen before Save, Update and Never work.
    settling: Settling,
}

/// Each browser window's offer, by the core's id of the window.
#[derive(Default)]
struct OfferWindows(HashMap<u64, WindowHandle<OfferPrompt>>);

impl Global for OfferWindows {}

/// Opens the offer's window for the browser window `browser`, hidden, for `host` to show over it,
/// ahead of time as the find bar's is. The card takes `fill`'s colour.
pub fn open(
    browser: u64,
    host: Rc<dyn Host>,
    look: impl Fn(&App) -> (Hsla, Palette) + 'static,
    cx: &mut App,
) -> Option<WindowHandle<OfferPrompt>> {
    // The keys are bound once, for every window's offer.
    if !cx.has_global::<OfferWindows>() {
        cx.bind_keys([
            KeyBinding::new("enter", Accept, Some("Offer")),
            KeyBinding::new("escape", NotNow, Some("Offer")),
        ]);
        cx.set_global(OfferWindows::default());
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WIDTH), px(HEIGHT)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The card's rounded shape is drawn, and the shadow takes it.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let input = cx.new(|cx| {
            TextInput::new("Username", cx)
                .placeholder_color(look(cx).1.text(PLACEHOLDER))
                .caret_color(look(cx).1.accent)
                .selection_color(look(cx).1.selection())
        });
        // The field has the keyboard whenever the window does, unless the offer has none.
        window.focus(&input.focus_handle(cx), cx);
        cx.new(|cx| OfferPrompt {
            host,
            look: Box::new(look),
            input,
            focus: cx.focus_handle(),
            tab: 0,
            elsewhere: false,
            settling: Settling::default(),
        })
    });
    match window {
        Ok(window) => {
            cx.global_mut::<OfferWindows>().0.insert(browser, window);
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the password offer's window: {error}");
            None
        }
    }
}

/// Forgets the offer of the browser window `browser`, which closed.
pub fn window_closed(browser: u64, cx: &mut App) {
    if cx.has_global::<OfferWindows>() {
        cx.global_mut::<OfferWindows>().0.remove(&browser);
    }
}

fn windows(cx: &App) -> Vec<WindowHandle<OfferPrompt>> {
    let windows = cx.try_global::<OfferWindows>();
    windows.map_or(Vec::new(), |windows| windows.0.values().copied().collect())
}

/// The core reports `tab`'s offer, or that it has none.
pub fn changed(tab: u64, offer: Option<Offer>, cx: &mut App) {
    // check.sh reads these lines. They count the page's other fields' values rather than log them,
    // as a check keeps its log, and leave out the password's length, since prod logs to stderr too.
    // An address's or a card's logs how many lines it has, not what they say.
    match &offer {
        Some(offer) => match offer.kind {
            Kind::Save | Kind::Update => log!(
                "browser: offer {tab} {} {} {}",
                if offer.kind == Kind::Save { "save" } else { "update" },
                offer.username,
                offer.usernames.len()
            ),
            Kind::Address | Kind::Card => log!(
                "browser: offer {tab} {} {}",
                if offer.kind == Kind::Address { "address" } else { "card" },
                offer.details.len()
            ),
        },
        None => log!("browser: offer {tab} gone"),
    }
    OFFERS.with_borrow_mut(|offers| match offer {
        Some(offer) => offers.insert(tab, offer),
        None => offers.remove(&tab),
    });
    for window in windows(cx) {
        if window.read(cx).is_ok_and(|prompt| prompt.tab == tab) {
            show_offer(window, cx);
        }
    }
}

/// The browser window `browser` shows `tab`, or no page (0). The offer of the tab it showed
/// closes, unless another window shows that tab.
pub fn tab_shown(browser: u64, tab: u64, cx: &mut App) {
    let Some(window) = window_of(browser, cx) else {
        return;
    };
    let left = window
        .update(cx, |prompt, _, _| {
            prompt.elsewhere = false;
            (prompt.tab != tab).then(|| std::mem::replace(&mut prompt.tab, tab))
        })
        .ok()
        .flatten();
    if let Some(left) = left {
        let still_shown = windows(cx)
            .into_iter()
            .any(|window| window.read(cx).is_ok_and(|prompt| prompt.tab == left));
        if !still_shown {
            OFFERS.with_borrow_mut(|offers| offers.remove(&left));
        }
        show_offer(window, cx);
    }
}

/// The page of the tab the browser window `browser` shows went to another window (`elsewhere`),
/// which shows its offer, or came back. The tab stays selected, so its offer stays open.
pub fn page_elsewhere(browser: u64, elsewhere: bool, cx: &mut App) {
    let Some(window) = window_of(browser, cx) else {
        return;
    };
    window.update(cx, |prompt, _, _| prompt.elsewhere = elsewhere).ok();
    show_offer(window, cx);
}

fn window_of(browser: u64, cx: &App) -> Option<WindowHandle<OfferPrompt>> {
    cx.try_global::<OfferWindows>().and_then(|windows| windows.0.get(&browser)).copied()
}

/// `tab` closed.
pub fn forget(tab: u64) {
    OFFERS.with_borrow_mut(|offers| offers.remove(&tab));
}

/// Shows the offer of the tab `window` shows, with its username in the field, or hides the window
/// if it has none or shows the tab's page elsewhere.
fn show_offer(window: WindowHandle<OfferPrompt>, cx: &mut App) {
    window
        .update(cx, |prompt, window, cx| {
            match prompt.offer().filter(|_| !prompt.elsewhere) {
                Some(offer) => {
                    match offer.of_login() {
                        true => window.focus(&prompt.input.focus_handle(cx), cx),
                        false => window.focus(&prompt.focus, cx),
                    }
                    prompt.input.update(cx, |input, cx| input.reset(offer.username, window, cx));
                    prompt.host.show();
                    prompt.settling.start(format!("offer {}", prompt.tab), cx);
                }
                None => prompt.host.hide(),
            }
            cx.notify();
        })
        .ok();
}

impl OfferPrompt {
    fn offer(&self) -> Option<Offer> {
        OFFERS.with_borrow(|offers| offers.get(&self.tab).cloned())
    }

    fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        let tab = self.tab;
        if answer != Answer::NotNow && !self.settling.settled(cx) {
            return;
        }
        if OFFERS.with_borrow_mut(|offers| offers.remove(&tab)).is_none() {
            return;
        }
        let username = self.input.read(cx).text().to_string();
        // check.sh reads this line.
        log!("browser: offer {tab} answered {answer:?} {username}");
        self.host.answer(tab, answer, &username);
        self.host.hide();
        cx.notify();
    }

    /// Return saves, once an input method has committed what it was composing.
    fn accept(&mut self, _: &Accept, _: &mut Window, cx: &mut Context<Self>) {
        if !self.input.read(cx).composing() {
            self.answer(Answer::Accept, cx);
        }
    }

    fn not_now(&mut self, _: &NotNow, _: &mut Window, cx: &mut Context<Self>) {
        self.answer(Answer::NotNow, cx);
    }

    /// Shows the page's usernames under the field, and puts the one chosen in it.
    fn pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(offer), Some(menu)) =
            (self.offer(), cx.try_global::<FieldMenu>().map(|menu| menu.0.clone()))
        else {
            return;
        };
        let items = offer.usernames.iter().map(|username| (username.clone(), true)).collect();
        let input = self.input.clone();
        let usernames = offer.usernames;
        let chosen = Box::new(move |closed: Closed, window: &mut Window, cx: &mut App| {
            if let Some(username) = closed.chosen().and_then(|index| usernames.get(index)) {
                input.update(cx, |input, cx| input.reset(username.clone(), window, cx));
            }
        });
        menu(items, field_bottom_left(), false, window, chosen, cx);
    }
}

/// Where the username field's bottom left corner is in the window, where its menu shows.
fn field_bottom_left() -> Point<Pixels> {
    point(px(INSET), px(INSET + TITLE_LINE + SITE_LINE + HEADER_GAP + ROW))
}

impl Render for OfferPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The browser's shortcuts work here too, below the offer's own keys.
        let root = div().key_context(shell::KEY_CONTEXT).size_full().font_family(crate::face::UI);
        let Some(offer) = self.offer() else {
            return root;
        };
        let (fill, palette) = (self.look)(cx);
        // The placeholder and the accent follow the Space shown, which may have changed since it
        // opened.
        self.input.update(cx, |input, _| {
            input.set_colors(palette.text(PLACEHOLDER), palette.accent.into(), palette.selection())
        });
        // Save or Update stands out in the Space's accent, as a dialog's action does.
        let button = |id: &'static str, label: SharedString, answer: Answer, default: bool| {
            let (rest, hovered, pressed) = if default {
                (palette.accent.into(), palette.accent_hovered(), palette.accent_pressed())
            } else {
                (palette.plate(PLATE), palette.state(PLATE, STATE_HOVERED), palette.state(PLATE, STATE_PRESSED))
            };
            div()
                .id(id)
                .flex_none()
                .h(px(BUTTON_HEIGHT))
                .px(px(BUTTON_X))
                .flex()
                .items_center()
                .rounded(px(PLATE_RADIUS))
                .bg(rest)
                .when(default, |button| button.text_color(palette.accent_label))
                .hover(move |style| style.bg(hovered))
                .active(move |style| style.bg(pressed))
                .font_weight(FontWeight::MEDIUM)
                .child(label)
                .on_click(cx.listener(move |prompt, _, _, cx| prompt.answer(answer, cx)))
        };
        let row = || {
            div()
                .flex_none()
                .h(px(ROW))
                .flex()
                .items_center()
                .pl(px(FIELD_X))
                .rounded(px(PLATE_RADIUS))
                .bg(palette.plate(PLATE))
        };
        let chevron = div()
            .id("usernames")
            .flex_none()
            .mx(px((ROW - CHEVRON) / 2.))
            .size(px(CHEVRON))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(PLATE_RADIUS))
            .hover(move |style| style.bg(palette.plate(STATE_HOVERED)))
            .active(move |style| style.bg(palette.plate(STATE_PRESSED)))
            .on_click(cx.listener(|prompt, _, window, cx| prompt.pick(window, cx)))
            .child(
                svg().path("icons/chevron-down.svg").size(px(GLYPH)).text_color(palette.glyph(DETAIL)),
            );
        let username = row()
            .pr(px(FIELD_X))
            .child(div().flex_1().min_w_0().overflow_hidden().child(self.input.clone()))
            .when(offer.usernames.len() > 1, |row| row.pr_0().child(chevron));
        let password = row()
            .overflow_hidden()
            .text_color(palette.text(DETAIL))
            .child("•".repeat(offer.password_length));
        // An address's first line, its name, then the rest on one; a card's name and last digits,
        // then its expiry.
        let first = offer.details.first().cloned().unwrap_or_default();
        let rest: Vec<&str> = offer.details.iter().skip(1).map(|line| line.as_ref()).collect();
        let line = |text: SharedString| {
            row().pr(px(FIELD_X)).child(div().flex_1().min_w_0().truncate().child(text))
        };
        let (first, second) = match offer.of_login() {
            true => (username, password),
            false => (line(first), line(rest.join(", ").into()).text_color(palette.text(DETAIL))),
        };
        let buttons = div()
            .mt(px(BUTTONS_TOP))
            .flex()
            .gap(px(BUTTON_GAP))
            .children(offer.never.map(|never| button("never", never, Answer::Never, false)))
            .child(div().flex_1())
            .child(button("decline", offer.decline, Answer::NotNow, false))
            .child(button("accept", offer.accept, Answer::Accept, true));
        root.child(
            div()
                .key_context("Offer")
                .track_focus(&self.focus)
                .on_action(cx.listener(Self::accept))
                .on_action(cx.listener(Self::not_now))
                .size_full()
                .p(px(INSET))
                .flex()
                .flex_col()
                .rounded(px(RADIUS))
                .bg(fill)
                .border_1()
                .border_color(Level::Popover.rim_in(&palette))
                .text_color(palette.text(TITLE))
                .text_size(px(text::BODY))
                .line_height(px(18.))
                .child(
                    div()
                        .line_height(px(TITLE_LINE))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(offer.title),
                )
                .child(
                    div()
                        .text_size(px(text::SMALL))
                        .line_height(px(SITE_LINE))
                        .text_color(palette.text(DETAIL))
                        .truncate()
                        .child(offer.site),
                )
                .child(first.mt(px(HEADER_GAP)))
                .child(second.mt(px(ROW_GAP)))
                .child(buttons),
        )
    }
}

#[cfg(test)]
mod tests;
