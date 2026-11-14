//! Point's note (#2352): the panel that opens beside the element the user clicked while pointing
//! for an agent, in the foot bar's material. Its field takes what the user means, and ↩ Add numbers
//! the element with it. Under it, folded, the element's picture and a line that says what goes;
//! opened, "Claude gets exactly this" over the rows the agent gets, each as the answer sends it, so
//! the user reads the payload itself before Send (crate::point). esc or a click outside leaves it,
//! the element not noted.
//!
//! GPUI can't draw over the page's native view, so as the Boost editor is, it's a window of its own
//! that the host shows over the browser window, as its child, with the keyboard, which goes back
//! to the browser window as it hides. Its field is a TextInput, so an input method composes in it.

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, Bounds, Context, Entity, Focusable, Global, Hsla, Image, ImageFormat, KeyBinding, Pixels,
    SharedString, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, accesskit::Role, actions, canvas, div, img, point, prelude::*, px,
    rgb, rgba, size, svg,
};

use crate::elevation::Level;
use crate::palette::Palette;
use crate::point::Element;
use crate::text_input::TextInput;
use crate::tokens::find_bar::FILL;
use crate::tokens::foot_bar::{INK, QUIET};
use crate::tokens::note_panel::{
    ABOVE, FIELD_FILL, FIELD_HEIGHT, FIELD_INSET, FIELD_RADIUS, GAP, INSET, MOST, NAME_WIDTH,
    PICTURE_MOST, PICTURE_RADIUS, RADIUS, ROW_GAP, ROWS_TOP, SMALL, TEXT, THUMB_GAP, THUMB_HEIGHT,
    THUMB_WIDTH, WIDTH, WITHHELD,
};

actions!(note_panel, [Add, Dismiss]);

/// What the panel asks of its window.
pub trait Host {
    /// Shows the panel's window at `bounds` in the content of browser window `window`, with the
    /// keyboard, or moves it there.
    fn show(&self, window: u64, bounds: Bounds<Pixels>);
    /// Makes it `height` tall, keeping its top.
    fn resize(&self, height: f32);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
}

/// The note being written, as its window's shell gives it.
#[derive(Clone)]
pub struct Panel {
    /// The number the element gets once it's added.
    pub number: usize,
    pub agent: SharedString,
    pub element: Element,
    /// The element's box and its page's, in the browser window's content.
    pub at: Bounds<Pixels>,
    pub page: Bounds<Pixels>,
    pub palette: Palette,
    /// ↩ Add, with what the user wrote.
    pub added: Rc<dyn Fn(String, &mut App)>,
    /// esc, or a click outside it: the element isn't noted.
    pub dropped: Rc<dyn Fn(&mut App)>,
}

struct Shown {
    window: u64,
    panel: Panel,
    picture: Option<Arc<Image>>,
    /// Whether it shows the rows that go, rather than the line that says what goes.
    opened: bool,
    bounds: Bounds<Pixels>,
}

pub struct NotePanel {
    host: Rc<dyn Host>,
    input: Entity<TextInput>,
    shown: Option<Shown>,
}

struct NotePanelWindow(WindowHandle<NotePanel>);

impl Global for NotePanelWindow {}

/// Opens the panel's window, hidden, for `host` to show over a browser window.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<NotePanel>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(folded_height())));
    open_at(host, bounds, false, cx)
}

/// Opens it at `bounds`, and on screen if `show`, as the preview does to show it alone.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<NotePanel>> {
    cx.bind_keys([
        KeyBinding::new("enter", Add, Some("NotePanel")),
        KeyBinding::new("escape", Dismiss, Some("NotePanel")),
    ]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: show,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // Its rounded shape is drawn, and the shadow takes it, as the foot bar's does.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        let input = cx.new(|cx| {
            TextInput::new("Say what you mean", cx)
                .placeholder_color(rgba(QUIET))
                .spoken("Note")
        });
        cx.new(|_| NotePanel {
            host,
            input,
            shown: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(NotePanelWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the note panel: {error}");
            None
        }
    }
}

fn handle(cx: &App) -> Option<WindowHandle<NotePanel>> {
    cx.try_global::<NotePanelWindow>().map(|panel| panel.0)
}

/// Shows `panel` over browser window `window`, at its element, with the keyboard. Asked again for
/// the same note, as the page moves, it moves and keeps what the user typed.
pub fn show(window: u64, panel: Panel, cx: &mut App) {
    crate::lazy_window::ready::<NotePanel>(cx);
    let Some(handle) = handle(cx) else {
        return;
    };
    _ = handle.update(cx, |note, gpui_window, cx| {
        let same = note.shown.as_ref().is_some_and(|shown| {
            shown.window == window
                && shown.panel.number == panel.number
                && shown.panel.element == panel.element
        });
        let height = match &note.shown {
            Some(shown) if same => f32::from(shown.bounds.size.height),
            _ => folded_height(),
        };
        let bounds = place(panel.at, panel.page, height);
        if same {
            let shown = note.shown.as_mut().expect("the same note shows");
            shown.panel = panel;
            if shown.bounds != bounds {
                shown.bounds = bounds;
                note.host.show(window, bounds);
            }
            return;
        }
        // check.sh reads this line.
        log!(
            "browser: note panel {} {}",
            panel.number,
            if panel.element.withheld {
                "withheld"
            } else {
                "shown"
            }
        );
        let picture = (panel.element.png.as_ref())
            .map(|png| Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec())));
        // The caret and the selection in the Space's accent, as its marks.
        let accent = panel.palette.accent;
        note.input.update(cx, |input, cx| {
            input.set_colors(
                rgba(QUIET).into(),
                accent.into(),
                Hsla::from(accent).opacity(0.35),
            );
            input.reset("".into(), gpui_window, cx)
        });
        gpui_window.focus(&note.input.focus_handle(cx), cx);
        note.shown = Some(Shown {
            window,
            panel,
            picture,
            opened: false,
            bounds,
        });
        note.host.show(window, bounds);
        cx.notify();
    });
}

/// Hides the panel if it shows over browser window `window`, as its page goes from view: the note
/// isn't dropped, and shows again as the page does.
pub fn hide(window: u64, cx: &mut App) {
    let Some(handle) = handle(cx) else {
        return;
    };
    _ = handle.update(cx, |note, _, cx| {
        if note
            .shown
            .as_ref()
            .is_some_and(|shown| shown.window == window)
        {
            note.shown = None;
            note.host.hide();
            cx.notify();
        }
    });
}

/// The user left the panel with a click outside it, as the host says.
pub fn dismissed(cx: &mut App) {
    let Some(handle) = handle(cx) else {
        return;
    };
    let dropped = handle
        .update(cx, |note, _, cx| {
            let shown = note.shown.take()?;
            cx.notify();
            Some(shown.panel.dropped)
        })
        .ok()
        .flatten();
    if let Some(dropped) = dropped {
        dropped(cx);
    }
}

/// Logs where "Show what's sent" or "Hide what's sent" is, in the browser window's points, when
/// that changes. check.sh reads this line, to open the rows as a user does.
fn fold_drawn(origin: gpui::Point<Pixels>, size: gpui::Size<Pixels>) {
    thread_local! {
        static DRAWN: std::cell::Cell<Option<Bounds<Pixels>>> = const { std::cell::Cell::new(None) };
    }
    let bounds = Bounds::new(origin, size);
    if DRAWN.replace(Some(bounds)) != Some(bounds) {
        log!(
            "browser: note panel fold at {:.0} {:.0} {:.0} {:.0}",
            f32::from(origin.x),
            f32::from(origin.y),
            f32::from(size.width),
            f32::from(size.height)
        );
    }
}

/// How tall it is folded: the field, and the thumbnail's line under it.
fn folded_height() -> f32 {
    INSET * 2. + FIELD_HEIGHT + ROWS_TOP + THUMB_HEIGHT
}

/// Where the panel goes beside the element at `at` on the page at `page`, `height` tall: left of
/// it if there's room in the page, else right, else over the element's left, its top a little over
/// the element's so the field is level with the mark's label, all of it kept in the page.
pub fn place(at: Bounds<Pixels>, page: Bounds<Pixels>, height: f32) -> Bounds<Pixels> {
    let (width, gap) = (px(WIDTH), px(GAP));
    let left = if at.left() - gap - width >= page.left() {
        at.left() - gap - width
    } else if at.right() + gap + width <= page.right() {
        at.right() + gap
    } else {
        at.left()
    };
    let left = left.min(page.right() - width).max(page.left());
    let top = (at.top() - px(ABOVE))
        .min(page.bottom() - px(height))
        .max(page.top());
    Bounds::new(point(left, top), size(width, px(height)))
}

impl NotePanel {
    fn add(&mut self, _: &Add, _: &mut Window, cx: &mut Context<Self>) {
        // ↩ commits what an input method composes, not the note.
        if self.input.read(cx).composing() {
            return;
        }
        let words = self.input.read(cx).text().to_owned();
        let Some(shown) = self.shown.take() else {
            return;
        };
        self.host.hide();
        cx.notify();
        let added = shown.panel.added;
        // After this update, as adding shows the bar again.
        cx.defer(move |cx| added(words, cx));
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.take() else {
            return;
        };
        log!("browser: note panel dismissed");
        self.host.hide();
        cx.notify();
        let dropped = shown.panel.dropped;
        cx.defer(move |cx| dropped(cx));
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if let Some(shown) = self.shown.as_mut() {
            shown.opened = !shown.opened;
            cx.notify();
        }
    }

    /// Its content drew `height` tall: the window takes that height, keeping its top.
    fn measured(&mut self, height: f32, _: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let height = height.min(MOST).ceil();
        if (f32::from(shown.bounds.size.height) - height).abs() < 0.5 {
            return;
        }
        let bounds = place(shown.panel.at, shown.panel.page, height);
        let moved = bounds.origin != shown.bounds.origin;
        shown.bounds = bounds;
        self.host.resize(height);
        if moved {
            self.host.show(shown.window, bounds);
        }
    }
}

impl Render for NotePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context("NotePanel")
            .on_action(cx.listener(Self::add))
            .on_action(cx.listener(Self::dismiss))
            .size_full()
            .font_family(crate::face::UI);
        let Some(shown) = &self.shown else {
            return root;
        };
        let a11y = window.is_a11y_active();
        let element = &shown.panel.element;
        let quiet = rgba(QUIET);
        let chevron = |up: bool| {
            svg()
                .path(match up {
                    true => "icons/chevron-up.svg",
                    false => "icons/chevron-down.svg",
                })
                .size(px(SMALL))
                .text_color(quiet)
        };
        let field = div()
            .h(px(FIELD_HEIGHT))
            .flex_none()
            .px(px(FIELD_INSET))
            .flex()
            .items_center()
            .gap(px(THUMB_GAP))
            .rounded(px(FIELD_RADIUS))
            .bg(rgba(FIELD_FILL))
            .child(div().flex_1().min_w_0().child(self.input.clone()))
            .child(
                div()
                    .id("note-add")
                    .flex_none()
                    .text_size(px(SMALL))
                    .text_color(quiet)
                    .cursor_pointer()
                    .when(a11y, |add| add.role(Role::Button).aria_label("Add"))
                    .on_click(cx.listener(|note, _, window, cx| note.add(&Add, window, cx)))
                    .child("↩ Add"),
            );
        let picture = |width: Option<f32>, most: f32| {
            shown.picture.clone().map(|picture| {
                img(picture)
                    .when_some(width, |picture, width| picture.w(px(width)))
                    .max_h(px(most))
                    .rounded(px(PICTURE_RADIUS))
                    .flex_none()
            })
        };
        let origin = shown.bounds.origin;
        let toggle = |label: SharedString, up: bool| {
            div()
                .id("note-fold")
                .relative()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(SMALL))
                .text_color(quiet)
                .cursor_pointer()
                .when(a11y, |each| each.role(Role::Button))
                .on_click(cx.listener(|note, _, _, cx| note.toggle(cx)))
                .child(label)
                .child(chevron(up))
                .child(
                    canvas(
                        move |bounds, _, _| fold_drawn(origin + bounds.origin, bounds.size),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
        };
        let below = match shown.opened {
            false => div()
                .mt(px(ROWS_TOP))
                .flex()
                .gap(px(THUMB_GAP))
                .children(
                    picture(Some(THUMB_WIDTH), THUMB_HEIGHT).map(|thumb| thumb.h(px(THUMB_HEIGHT))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .text_size(px(SMALL))
                        .child(element.folded())
                        .child(toggle("Show what's sent".into(), false)),
                )
                .into_any_element(),
            true => {
                let rows = element.rows().into_iter().map(|row| {
                    let picture = (row.name == "Picture")
                        .then(|| picture(None, PICTURE_MOST))
                        .flatten();
                    div()
                        .mt(px(ROW_GAP))
                        .flex()
                        .text_size(px(SMALL))
                        .child(
                            div()
                                .w(px(NAME_WIDTH))
                                .flex_none()
                                .text_color(quiet)
                                .child(row.name),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(ROW_GAP))
                                .text_color(match row.withheld {
                                    true => rgba(WITHHELD),
                                    false => rgba(INK),
                                })
                                .child(row.value)
                                .children(picture),
                        )
                });
                div()
                    .mt(px(ROWS_TOP))
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .child(div().flex().justify_between().child(toggle(
                        format!("{} gets exactly this", shown.panel.agent).into(),
                        true,
                    )))
                    .child(
                        div()
                            .id("note-rows")
                            .flex()
                            .flex_col()
                            .min_h_0()
                            .overflow_y_scroll()
                            .children(rows),
                    )
                    .into_any_element()
            }
        };
        let note = cx.weak_entity();
        // The content's own height, which the window takes, a frame after it changes.
        let measure = canvas(
            move |bounds, _, cx| {
                let height = f32::from(bounds.size.height);
                let note = note.clone();
                cx.defer(move |cx| {
                    _ = note.update(cx, |note, cx| note.measured(height, cx));
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        root.child(
            div()
                .id("note-panel")
                .when(a11y, |panel| {
                    panel
                        .role(Role::Dialog)
                        .aria_label(format!("Note {}", shown.panel.number))
                })
                .relative()
                .w_full()
                .max_h(px(MOST))
                .flex()
                .flex_col()
                .p(px(INSET))
                .rounded(px(RADIUS))
                .bg(rgb(FILL))
                .text_size(px(TEXT))
                .text_color(rgba(INK))
                .child(measure)
                .child(field)
                .child(below)
                // The popover level's rim, as the foot bar's, its own layer.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(RADIUS))
                        .border(px(0.5))
                        .border_color(Level::Popover.rim()),
                ),
        )
    }
}

#[cfg(test)]
mod tests;
