//! The find bar, over the top right of the page. GPUI can't draw over the page's native view, so
//! each browser window's bar is a window of its own that the host shows over it, as its child,
//! while the tab shown has find in page open. The core keeps each tab's find; the bar shows the
//! shown tab's.

use std::{cell::RefCell, collections::HashMap, ffi::CString, ffi::c_void, rc::Rc};

use browser::{
    elevation::Level,
    log,
    palette::{Palette, STATE_HOVERED, STATE_PRESSED},
    shell::{self, keystroke_keys},
    text_input::TextInput,
    tooltip::{self, Anchor, Tip},
};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, Focusable, FontWeight, Global, KeyBinding,
    Pixels, Point, SharedString, Size, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::{Live, Role},
    actions, canvas, div, point,
    prelude::*,
    px, rgb, rgba, size, svg,
};

use crate::{bc, platform};

actions!(find_bar, [FindNext, FindPrevious, SelectAll, Close]);

// Arc's bar (docs/ux-spec.md › Find in page): 300 × 32, radius 6, 10 in from the card's top right
// corner, so twice the card's inset from the window's.
use browser::tokens::find_bar::HEIGHT;
use browser::tokens::find_bar::RADIUS;
use browser::tokens::find_bar::WIDTH;
const RIGHT: f32 = shell::CARD_INSET * 2.;
// Arc's fill, a navy unlike its window's colour (one Space captured).
use browser::tokens::find_bar::FILL;
const TOP: f32 = shell::CARD_INSET * 2.;
use browser::tokens::find_bar::ARROW_OVERLAP;
use browser::tokens::find_bar::BUTTON;
use browser::tokens::find_bar::COUNT;
use browser::tokens::find_bar::COUNT_GAP;
use browser::tokens::find_bar::DIVIDER;
use browser::tokens::find_bar::DIVIDER_GAP;
use browser::tokens::find_bar::DIVIDER_HEIGHT;
use browser::tokens::find_bar::FIELD_GAP;
use browser::tokens::find_bar::GLYPH;
use browser::tokens::find_bar::GLYPH_COLOR;
use browser::tokens::find_bar::GLYPH_DISABLED;
use browser::tokens::find_bar::INSET;
use browser::tokens::find_bar::NOT_FOUND;
use browser::tokens::find_bar::PLACEHOLDER;
use browser::tokens::find_bar::TEXT;
use browser::tokens::text;

/// A tab's find in page, as the core reports it.
#[derive(Clone, Default)]
pub struct Find {
    pub open: bool,
    pub text: String,
    /// -1 until the page has looked.
    pub matches: i32,
    /// From 1, or 0 for none.
    pub selected: i32,
}

thread_local! {
    // Every tab's. Kept out of GPUI's entities, so a tab closing inside a GPUI update drops its own.
    static FINDS: RefCell<HashMap<u64, Find>> = RefCell::new(HashMap::new());
}

/// Where the bar's top left is in the browser window, and that window's size, as the shell lays
/// the page out.
type Place = Rc<dyn Fn(&App) -> Option<(Point<Pixels>, Size<Pixels>)>>;

/// A button that names itself and its key in the tooltip, as the header's buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Named {
    Previous,
    Next,
    Close,
}

impl Named {
    /// Its name and the key that does what it does, as the bar binds it.
    fn named(self) -> (&'static str, &'static str) {
        match self {
            Named::Previous => ("Previous", "shift-cmd-g"),
            Named::Next => ("Next", "cmd-g"),
            Named::Close => ("Close", "escape"),
        }
    }
}

pub struct FindBar {
    /// The core's id of the browser window the bar is over.
    browser: bc::bc_id,
    /// The UI's view of the browser window the bar is over.
    ui_view: *mut c_void,
    place: Place,
    /// Where each button that names itself was last drawn, in the bar.
    frames: Rc<RefCell<HashMap<Named, Bounds<Pixels>>>>,
    /// The button whose tooltip the bar asked for, until the pointer leaves it or something ends
    /// it.
    tipped: Option<Named>,
    input: Entity<TextInput>,
    /// The tab the window shows, whose find this bar is.
    tab: u64,
    /// What the page was last asked to find, so a change that leaves the text as it was doesn't
    /// search again.
    searched: String,
}

/// Each browser window's bar, by the core's id of the window.
#[derive(Default)]
struct FindBars(HashMap<bc::bc_id, WindowHandle<FindBar>>);

impl Global for FindBars {}

/// Opens the find bar's window for the browser window `browser`, whose UI draws in `ui_view`,
/// hidden, ahead of time as the command bar does, and returns its window. `corner` gives the
/// shell's `at_page_corner`, where the host puts the bar.
pub fn open(
    browser: bc::bc_id,
    ui_view: *mut c_void,
    corner: impl Fn(&App, f32, f32, f32) -> Option<(Point<Pixels>, Size<Pixels>)> + 'static,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let place: Place = Rc::new(move |cx| corner(cx, RIGHT, TOP, WIDTH));
    // The keys are bound once, for every window's bar.
    if !cx.has_global::<FindBars>() {
        // The browser's ⌘G, ⇧⌘G and ⌘F, for the bar while it has the keyboard.
        let key = |key, action| crate::shell::view_binding(key, action, "FindBar", cx);
        let mac_keys = [
            key("cmd-g", Box::new(FindNext)),
            key("shift-cmd-g", Box::new(FindPrevious)),
            key("cmd-f", Box::new(SelectAll)),
        ];
        cx.bind_keys(mac_keys);
        cx.bind_keys([
            KeyBinding::new("enter", FindNext, Some("FindBar")),
            KeyBinding::new("shift-enter", FindPrevious, Some("FindBar")),
            KeyBinding::new("escape", Close, Some("FindBar")),
        ]);
        cx.set_global(FindBars::default());
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
        // The bar's rounded shape is drawn, and the shadow takes it.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let input =
            cx.new(|cx| TextInput::new("Find in page", cx).placeholder_color(rgba(PLACEHOLDER)));
        window.focus(&input.focus_handle(cx), cx);
        cx.new(|cx| {
            cx.observe(&input, |bar: &mut FindBar, _, cx| bar.edited(cx))
                .detach();
            FindBar {
                browser,
                ui_view,
                place,
                frames: Rc::default(),
                tipped: None,
                input,
                tab: 0,
                searched: String::new(),
            }
        })
    });
    match window {
        Ok(window) => {
            if let Ok(view) = window.update(cx, |_, window, _| crate::ns_view(window)) {
                let (right, top) = (f64::from(RIGHT), f64::from(TOP));
                unsafe { platform::host_add_find_bar(ui_view, view, right, top) };
                cx.global_mut::<FindBars>().0.insert(browser, window);
            }
            Some(window.into())
        }
        Err(error) => {
            log!("couldn't open the find bar: {error}");
            None
        }
    }
}

/// Forgets the bar of the browser window `browser`, which closed.
pub fn window_closed(browser: bc::bc_id, cx: &mut App) {
    if cx.has_global::<FindBars>() {
        cx.global_mut::<FindBars>().0.remove(&browser);
    }
}

/// The bar over the browser window `browser`.
fn window(browser: bc::bc_id, cx: &App) -> Option<WindowHandle<FindBar>> {
    cx.try_global::<FindBars>()
        .and_then(|bars| bars.0.get(&browser).copied())
}

/// Opens `tab`'s find bar over the browser window `browser`, or selects its text if it's open, and
/// gives it the keyboard.
pub fn show(browser: bc::bc_id, tab: u64, cx: &mut App) {
    let Some(window) = window(browser, cx).filter(|_| tab != 0) else {
        return;
    };
    let text = open_find(tab);
    FINDS.with_borrow_mut(|finds| {
        let find = finds.entry(tab).or_default();
        find.open = true;
        find.text = text.clone();
    });
    let ui_view = window.update(cx, |bar, window, cx| {
        bar.tab = tab;
        bar.searched = text.clone();
        bar.input
            .update(cx, |input, cx| input.reset(text.into(), window, cx));
        bar.ui_view
    });
    let Ok(ui_view) = ui_view else {
        return;
    };
    // On Windows the field takes a character only once the bar has drawn with it, which a bar the
    // host shows for the first time hasn't: the keys held since the shortcut, given as the bar
    // takes the keyboard, came before its first frame, and the first was lost (#1917). So the bar
    // shows first and takes the keyboard, and with it the held keys, once it has drawn: from a task
    // its next frame starts, outside the frame, as taking the keyboard tells its window at once.
    // Until then the keys are held, as only a shortcut the page gets first holds them: from the UI,
    // keys typed before that frame went to the browser window and were lost.
    #[cfg(target_os = "windows")]
    {
        unsafe { platform::host_hold_keys() };
        unsafe { platform::host_show_find_bar(ui_view, false) };
        let shown = window.update(cx, |_, window, _| {
            window.on_next_frame(move |_, cx| {
                cx.spawn(async move |_| unsafe { platform::host_show_find_bar(ui_view, true) })
                    .detach();
            });
        });
        if shown.is_ok() {
            return;
        }
    }
    unsafe { platform::host_show_find_bar(ui_view, true) };
}

/// Goes to `tab`'s next match, or the one before, opening its find bar over the browser window
/// `browser` if it's closed, as Chrome's ⌘G does.
pub fn find_next(browser: bc::bc_id, tab: u64, forward: bool, cx: &mut App) {
    let open = FINDS.with_borrow(|finds| finds.get(&tab).is_some_and(|find| find.open));
    if !open {
        show(browser, tab, cx);
    }
    if let Some(bar) = window(browser, cx).and_then(|window| window.read(cx).ok()) {
        let text = bar.input.read(cx).text();
        if !text.is_empty() {
            find(tab, text, forward);
        }
    }
}

/// The browser window `browser` shows `tab`, or no page (0): its bar shows the tab's find if
/// that's open.
pub fn tab_shown(browser: bc::bc_id, tab: u64, cx: &mut App) {
    let Some(window) = window(browser, cx) else {
        return;
    };
    let find = FINDS.with_borrow(|finds| finds.get(&tab).filter(|find| find.open).cloned());
    let changed = window
        .update(cx, |bar, window, cx| {
            if bar.tab == tab {
                return None;
            }
            bar.tab = tab;
            bar.end_tip(cx);
            if let Some(find) = &find {
                bar.searched = find.text.clone();
                bar.input.update(cx, |input, cx| {
                    input.reset(find.text.clone().into(), window, cx)
                });
            }
            cx.notify();
            Some(bar.ui_view)
        })
        .ok()
        .flatten();
    if let Some(ui_view) = changed {
        match find {
            Some(_) => unsafe { platform::host_show_find_bar(ui_view, false) },
            None => unsafe { platform::host_hide_find_bar(ui_view) },
        }
    }
}

/// The core reports `tab`'s find.
pub fn changed(tab: u64, find: Find, cx: &mut App) {
    // check.sh reads these lines.
    if find.open {
        log!(
            "browser: find {tab} open {} of {} {}",
            find.selected,
            find.matches,
            find.text
        );
    } else {
        log!("browser: find {tab} closed");
    }
    let open = find.open;
    FINDS.with_borrow_mut(|finds| finds.insert(tab, find));
    let windows: Vec<_> = match cx.try_global::<FindBars>() {
        Some(bars) => bars.0.values().copied().collect(),
        None => Vec::new(),
    };
    for window in windows {
        let shown = window
            .update(cx, |bar, _, cx| {
                cx.notify();
                if bar.tab == tab && !open {
                    bar.end_tip(cx);
                }
                (bar.tab == tab).then_some(bar.ui_view)
            })
            .ok()
            .flatten();
        // A tab going to another page closes its find bar.
        if let Some(ui_view) = shown
            && !open
        {
            unsafe { platform::host_hide_find_bar(ui_view) };
        }
    }
}

/// `tab` closed.
pub fn forget(tab: u64) {
    FINDS.with_borrow_mut(|finds| finds.remove(&tab));
}

/// Opens `tab`'s find in the core and returns its text.
fn open_find(tab: u64) -> String {
    // Long enough for most, so the core opens it once.
    let mut text = vec![0u8; 256];
    let length = unsafe { bc::bc_tab_find_open(tab, text.as_mut_ptr().cast(), text.len()) };
    if length >= text.len() {
        text.resize(length + 1, 0);
        unsafe { bc::bc_tab_find_open(tab, text.as_mut_ptr().cast(), text.len()) };
    }
    text.truncate(length);
    String::from_utf8_lossy(&text).into_owned()
}

fn find(tab: u64, text: &str, forward: bool) {
    if let Ok(text) = CString::new(text) {
        unsafe { bc::bc_tab_find(tab, text.as_ptr(), forward) };
    }
}

impl FindBar {
    /// Finds the text as the user types it, as Chrome's find bar does, once an input method has
    /// committed it.
    fn edited(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.input.read(cx).committed_change(&self.searched) else {
            return;
        };
        self.searched = text.to_string();
        find(self.tab, &self.searched, true);
    }

    fn find_next(&mut self, forward: bool, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        if !text.is_empty() {
            find(self.tab, text, forward);
        }
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = self.input.read(cx).text().to_string().into();
        self.input
            .update(cx, |input, cx| input.reset(text, window, cx));
    }

    fn close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        self.end_tip(cx);
        unsafe { bc::bc_tab_find_close(self.tab) };
        FINDS.with_borrow_mut(|finds| {
            if let Some(find) = finds.get_mut(&self.tab) {
                find.open = false;
            }
        });
        unsafe { platform::host_hide_find_bar(self.ui_view) };
    }

    /// What the page found for the text, once it has looked: which match of how many, or none.
    fn found(&self) -> Option<(i32, i32)> {
        FINDS.with_borrow(|finds| match finds.get(&self.tab) {
            Some(find) if !find.text.is_empty() && find.matches >= 0 => {
                Some((find.selected, find.matches))
            }
            _ => None,
        })
    }

    /// The pointer has come onto or left `button`, whose tooltip names it and its key, hanging
    /// from it in the browser window, as Peek's buttons' do.
    fn hover(&mut self, button: Named, hovered: bool, cx: &mut Context<Self>) {
        let frame = self
            .frames
            .borrow()
            .get(&button)
            .copied()
            .filter(|_| hovered);
        match (frame, (self.place)(cx)) {
            (Some(frame), Some((origin, window))) => {
                let (name, keystroke) = button.named();
                let keys = keystroke_keys(keystroke)
                    .into_iter()
                    .map(Into::into)
                    .collect();
                let anchor = Anchor::under(Bounds::new(origin + frame.origin, frame.size), window);
                self.tipped = Some(button);
                tooltip::hover(
                    self.browser,
                    Tip {
                        text: name.into(),
                        keys,
                    },
                    anchor,
                    cx,
                );
            }
            // Unless the pointer went from it onto another button, whose tooltip took its place.
            _ if self.tipped == Some(button) => self.end_tip(cx),
            _ => {}
        }
    }

    /// Ends the tooltip of the button it asked for one for, if any: the pointer left, or a click,
    /// a key or the bar going ends it.
    fn end_tip(&mut self, cx: &mut App) {
        if self.tipped.take().is_some() {
            tooltip::dismiss(cx);
        }
    }

    /// Makes `button`, the bar's button that names itself, say so under the pointer, and keeps
    /// where it's drawn for its tooltip.
    fn named(
        &self,
        button: gpui::Stateful<gpui::Div>,
        named: Named,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let frames = self.frames.clone();
        button
            .relative()
            .on_hover(cx.listener(move |bar, hovered: &bool, _, cx| bar.hover(named, *hovered, cx)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|bar, _, _, cx| bar.end_tip(cx)),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        frames.borrow_mut().insert(named, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }

    /// A 28 pt button with `icon`, which dims while it can't be used, and which an assistive app
    /// reads as `name`, its tooltip's, if it's asked for the tree.
    fn button(
        id: &'static str,
        icon: &'static str,
        name: &'static str,
        enabled: bool,
        a11y: bool,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .when(a11y, |button| {
                button
                    .role(Role::Button)
                    .aria_label(name)
                    .a11y_synthetic_children(move |builder| {
                        if !enabled {
                            builder.parent_node().set_disabled();
                        }
                    })
            })
            .flex_none()
            .size(px(BUTTON))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS))
            .when(enabled, |button| {
                button
                    .hover(|style| style.bg(Palette::on_dark(STATE_HOVERED)))
                    .active(|style| style.bg(Palette::on_dark(STATE_PRESSED)))
            })
            .child(
                svg()
                    .path(icon)
                    .size(px(GLYPH))
                    .text_color(rgba(if enabled { GLYPH_COLOR } else { GLYPH_DISABLED })),
            )
    }
}

impl Render for FindBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let a11y = window.is_a11y_active();
        let found = self.found();
        // Arc's count, "1/12", and its previous and next, or "NOT FOUND" in their place.
        let (status, matches) = match found {
            Some((_, 0)) => ("NOT FOUND".to_string(), None),
            Some((selected, matches)) => (format!("{selected}/{matches}"), Some(matches)),
            None => (String::new(), None),
        };
        // check.sh reads these lines.
        log!(
            "browser: find bar drew \"{}\" {status}",
            self.input.read(cx).text()
        );
        let enabled = matches.is_some();
        let not_found = found.is_some() && !enabled;
        let previous = Named::Previous.named().0;
        let previous = Self::button("previous", "icons/chevron-up.svg", previous, enabled, a11y);
        let previous = self
            .named(previous, Named::Previous, cx)
            .when(enabled, |button| {
                button.on_click(cx.listener(|bar, _, _, cx| bar.find_next(false, cx)))
            });
        let next = Named::Next.named().0;
        let next = Self::button("next", "icons/chevron-down.svg", next, enabled, a11y);
        let next = self
            .named(next, Named::Next, cx)
            .ml(px(-ARROW_OVERLAP))
            .when(enabled, |button| {
                button.on_click(cx.listener(|bar, _, _, cx| bar.find_next(true, cx)))
            });
        // What it says, said as it changes after what's typed, wherever VoiceOver's cursor is:
        // "1 of 12" rather than its slash, and "not found" rather than its capitals.
        let said = match found {
            Some((_, 0)) => "Not found".to_string(),
            Some((selected, matches)) => format!("{selected} of {matches}"),
            None => String::new(),
        };
        let status = div()
            .id("matches")
            .when(a11y, |status| {
                status
                    .role(Role::Status)
                    .aria_label("Matches")
                    .aria_value(said)
                    .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
            })
            .flex_none()
            .mr(px(COUNT_GAP))
            .when(not_found, |status| {
                status
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(NOT_FOUND))
            })
            .when(!not_found, |status| {
                status.text_size(px(text::SMALL)).text_color(rgba(COUNT))
            })
            .child(status);
        let divider = div()
            .flex_none()
            .w(px(DIVIDER_GAP))
            .h(px(DIVIDER_HEIGHT))
            .flex()
            .justify_center()
            .child(div().w(px(1.)).h_full().bg(rgba(DIVIDER)));
        let close = Self::button("close", "icons/x.svg", Named::Close.named().0, true, a11y);
        let close = self
            .named(close, Named::Close, cx)
            .on_click(cx.listener(|bar, _, window, cx| bar.close(&Close, window, cx)));
        let search = div()
            .flex_none()
            .size(px(BUTTON))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path("icons/search.svg")
                    .size(px(GLYPH))
                    .text_color(rgba(GLYPH_COLOR)),
            );
        // The browser's shortcuts work here too, below the bar's own keys.
        div()
            .key_context(shell::KEY_CONTEXT)
            .size_full()
            .font_family(browser::face::UI)
            .child(
                div()
                    .key_context("FindBar")
                    // A key ends a button's tooltip, as a click does.
                    .capture_key_down(cx.listener(|bar, _, _, cx| bar.end_tip(cx)))
                    .on_action(cx.listener(|bar, _: &FindNext, _, cx| bar.find_next(true, cx)))
                    .on_action(cx.listener(|bar, _: &FindPrevious, _, cx| bar.find_next(false, cx)))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::close))
                    .size_full()
                    .relative()
                    .flex()
                    .items_center()
                    .px(px(INSET))
                    .rounded(px(RADIUS))
                    .bg(rgb(FILL))
                    .text_color(rgb(TEXT))
                    .text_size(px(text::BODY))
                    .line_height(px(18.))
                    .child(search)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ml(px(FIELD_GAP))
                            .overflow_hidden()
                            .child(self.input.clone()),
                    )
                    .child(status)
                    .when(!not_found, |bar| bar.child(previous).child(next))
                    .child(divider)
                    .child(close)
                    // The popover level's rim, its own layer so the border doesn't move the bar's
                    // parts. Its shadow is the window's, which the host's find bar still takes from
                    // the system.
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .rounded(px(RADIUS))
                            .border(px(0.5))
                            .border_color(Level::Popover.rim()),
                    ),
            )
    }
}
