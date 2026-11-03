// Adapted from Zed's crates/gpui/examples/input.rs (https://github.com/zed-industries/zed, commit
// b36b283021c48f1712714b8a0e2ad6ffb578920b). Copyright Zed Industries, Inc. Licensed under the
// Apache License, Version 2.0. Changes: a constructor, `text`, `composing`, `selection`, `pasted`,
// `reset`, `complete`, `set_text`, a placeholder colour, `set_placeholder`, key bindings scoped to
// the field, ⌘← and ⌘→ as Home and End, a context menu, `Away`, scrolling to keep the caret in
// view with a fade where text runs past an edge, and no styling of its own.
//
//! A single-line text field, with IME support through GPUI's input handler. GPUI has no text
//! field of its own; Zed's is its editor.

use std::{ops::Range, rc::Rc};

use crate::menu::Closed;
use crate::shell::MenuEntry;
use crate::tokens::palette::{NO_THEME_ACCENT, SELECTION};
use crate::tokens::text_input::PLACEHOLDER;
use gpui::{
    Action, App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Global,
    GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, Role, ShapedLine, SharedString, Style, Subscription,
    TextRun, UTF16Selection, UnderlineStyle, Window, accesskit, actions, div, fill, hsla,
    linear_color_stop, linear_gradient, point, prelude::*, px, relative, rgb, size,
};
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteToStart,
        DeleteWordBack,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        PasteAndMatchStyle,
        Cut,
        Copy,
        Undo,
        Redo,
        PasteAndGo,
        PasteAndSearch,
        OpenMenu,
    ]
);

/// Shows a field's context menu: `items`' titles, dimmed where they aren't enabled and a line for
/// an empty one, at `position` in `window`, with its first item highlighted where it opened from
/// the keyboard, and calls `chosen` once it closes, with how it closed. The app's,
/// `menu::field_menu`, shows it in ilo's own menu.
pub struct FieldMenu(pub Rc<ShowMenu>);

pub type ShowMenu =
    dyn Fn(Vec<(SharedString, bool)>, Point<Pixels>, bool, &Window, Chosen, &mut App);
pub type Chosen = Box<dyn FnOnce(Closed, &mut Window, &mut App)>;

impl Global for FieldMenu {}

/// Calls `listener` once the keyboard leaves the field `focus` for something else in its window.
/// A window that stops being the key one, as while the field's own menu, another window or another
/// app has the keyboard, keeps the field focused for when it's back, so that isn't leaving it.
pub fn on_leave<T: 'static>(
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<T>,
    mut listener: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static,
) -> Subscription {
    let field = focus.clone();
    cx.on_blur(focus, window, move |view, window, cx| {
        if !field.is_focused(window) {
            listener(view, window, cx)
        }
    })
}

/// The user went away from the field with the keyboard still in it: a press outside it, which
/// moves the keyboard only where it lands on something that takes it, or its own menu closed by a
/// press away from both or by the user going to another window or app.
pub struct Away;

impl EventEmitter<Away> for TextInput {}

/// Calls `listener` once the field loses the keyboard, however it goes (owner, 2026-11-02, #1414):
/// to something else in its window, with a press away from it (`Away`), or with its window, to an
/// overlay, another window or another app. Its own menu takes the window's keyboard and gives it
/// back, so it counts only once the user goes away from both.
pub fn on_lose_keyboard<T: 'static>(
    field: &Entity<TextInput>,
    window: &mut Window,
    cx: &mut Context<T>,
    listener: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) -> Vec<Subscription> {
    let listener = Rc::new(listener);
    let (left, away, deactivated) = (listener.clone(), listener.clone(), listener);
    let input = field.downgrade();
    let focus = field.focus_handle(cx);
    vec![
        on_leave(&focus, window, cx, move |view, window, cx| {
            left(view, window, cx)
        }),
        cx.subscribe_in(field, window, move |view, _, _: &Away, window, cx| {
            away(view, window, cx)
        }),
        cx.observe_window_activation(window, move |view, window, cx| {
            let menu = input
                .upgrade()
                .is_some_and(|input| input.read(cx).menu_shown);
            if !window.is_window_active() && !menu {
                deactivated(view, window, cx)
            }
        }),
    ]
}

/// `text` without the `javascript:` schemes it starts with, past any whitespace and control
/// characters, as Chrome's address bar takes a paste: a page that talks the user into pasting
/// script there can't run it. Typed, it runs, as in Chrome.
pub fn without_javascript(text: &str) -> &str {
    const SCHEME: &str = "javascript:";
    let mut rest = text;
    let mut found = false;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c.is_control());
        match rest.get(..SCHEME.len()) {
            Some(scheme) if scheme.eq_ignore_ascii_case(SCHEME) => {
                found = true;
                rest = &rest[SCHEME.len()..];
            }
            _ => return if found { rest } else { text },
        }
    }
}

pub fn bind_keys(cx: &mut App) {
    let context = Some("TextInput");
    let key = |key, action| crate::shell::view_binding(key, action, "TextInput", cx);
    let mac_keys = [
        key("cmd-a", Box::new(SelectAll)),
        key("cmd-v", Box::new(Paste)),
        key("cmd-c", Box::new(Copy)),
        key("cmd-x", Box::new(Cut)),
        // So the Edit menu shows them. The menu's Undo and Redo go to the first responder, which
        // undoes for itself, as a page does.
        key("cmd-z", Box::new(Undo)),
        key("shift-cmd-z", Box::new(Redo)),
        key("shift-cmd-v", Box::new(PasteAndMatchStyle)),
    ];
    cx.bind_keys(mac_keys);
    // A Mac text field's forward delete, which keeps ⌃D from the browser's Developer Mode.
    if !crate::shell::OFF_THE_MAC {
        cx.bind_keys([KeyBinding::new("ctrl-d", Delete, context)]);
    }
    // ⌘⌫ deletes to the field's start and ⌥⌫ the word before the caret, as in any Mac field. Off
    // the Mac Ctrl+Backspace deletes the word, and on Linux Ctrl+Shift+Backspace to the start, as
    // in Chrome's fields there, ahead of Clear Browsing Data while a field has the keys.
    match crate::shell::OFF_THE_MAC {
        false => cx.bind_keys([
            KeyBinding::new("cmd-backspace", DeleteToStart, context),
            KeyBinding::new("alt-backspace", DeleteWordBack, context),
        ]),
        true => {
            cx.bind_keys([KeyBinding::new("ctrl-backspace", DeleteWordBack, context)]);
            if cfg!(target_os = "linux") {
                cx.bind_keys([KeyBinding::new(
                    "ctrl-shift-backspace",
                    DeleteToStart,
                    context,
                )]);
            }
        }
    }
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        // As in any Mac text field, and ahead of the browser's back and forward.
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, context),
        // Full Keyboard Access's, and Windows' and Linux's, way to a field's menu.
        KeyBinding::new("shift-f10", OpenMenu, context),
        KeyBinding::new("menu", OpenMenu, context),
    ]);
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    placeholder_color: Hsla,
    caret_color: Hsla,
    selection_color: Hsla,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    pasted: bool,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// How far the line has scrolled left to keep the caret in view.
    scroll: Pixels,
    /// How wide a fade an edge with more text past it gets, and the ground it fades into.
    fade: Option<(Pixels, Hsla)>,
    is_selecting: bool,
    paste_and_go: bool,
    secure: bool,
    /// The field's own menu shows, with the keyboard its window had.
    menu_shown: bool,
}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: placeholder.into(),
            placeholder_color: hsla(0., 0., 0., PLACEHOLDER),
            caret_color: rgb(NO_THEME_ACCENT).into(),
            selection_color: Hsla::from(rgb(NO_THEME_ACCENT)).opacity(SELECTION),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            pasted: false,
            last_layout: None,
            last_bounds: None,
            scroll: px(0.),
            fade: None,
            is_selecting: false,
            paste_and_go: false,
            secure: false,
            menu_shown: false,
        }
    }

    /// With `secure`, it shows a dot for each character typed and won't copy or cut them, as a
    /// password field. An input method still composes in it.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// What the field shows: its text, or a secure field's dots.
    pub fn shown(&self) -> SharedString {
        match self.secure {
            true => "•".repeat(self.content.chars().count()).into(),
            false => self.content.clone(),
        }
    }

    /// Where `offset` in the text is in what the field shows.
    fn shown_offset(&self, offset: usize) -> usize {
        match self.secure {
            true => self.content[..offset].chars().count() * '•'.len_utf8(),
            false => offset,
        }
    }

    /// Where `offset` in what the field shows is in the text.
    fn text_offset(&self, offset: usize) -> usize {
        match self.secure {
            true => (self.content.char_indices())
                .nth(offset / '•'.len_utf8())
                .map_or(self.content.len(), |(index, _)| index),
            false => offset,
        }
    }

    /// Its menu has Paste and Go and Paste and Search, as an address bar's has, which dispatch
    /// PasteAndGo and PasteAndSearch for the field's owner to take, and a paste loses its leading
    /// `javascript:`, as Chrome's address bar's does.
    pub fn paste_and_go(mut self) -> Self {
        self.paste_and_go = true;
        self
    }

    /// The placeholder's colour, for a field on a dark background.
    pub fn placeholder_color(mut self, color: impl Into<Hsla>) -> Self {
        self.placeholder_color = color.into();
        self
    }

    /// The placeholder's, caret's and selection's colours, for an input whose ground and accent
    /// change with the Space.
    pub fn set_colors(&mut self, placeholder: Hsla, caret: Hsla, selection: Hsla) {
        (
            self.placeholder_color,
            self.caret_color,
            self.selection_color,
        ) = (placeholder, caret, selection);
    }

    /// Fades an edge with more text past it over `width` into `ground`, the opaque fill under the
    /// field, which may change with the Space.
    pub fn set_fade(&mut self, width: Pixels, ground: Hsla) {
        self.fade = Some((width, ground));
    }

    pub fn caret_color(mut self, color: impl Into<Hsla>) -> Self {
        self.caret_color = color.into();
        self
    }

    pub fn selection_color(mut self, color: impl Into<Hsla>) -> Self {
        self.selection_color = color.into();
        self
    }

    /// Starts with `text` in the field, all selected.
    pub fn with_text(mut self, text: SharedString) -> Self {
        self.selected_range = 0..text.len();
        self.content = text;
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// An input method is composing in the field, so its text isn't what the user means yet.
    pub fn composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// The field's own menu shows, which has its window's keyboard until it closes, so the field
    /// hasn't been left.
    pub fn menu_shown(&self) -> bool {
        self.menu_shown
    }

    pub fn selection(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    /// The last change to the text was a paste.
    pub fn pasted(&self) -> bool {
        self.pasted
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_to_start(&mut self, _: &DeleteToStart, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_back_to(0, window, cx);
    }

    fn delete_word_back(
        &mut self,
        _: &DeleteWordBack,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.previous_word_start(self.cursor_offset());
        self.delete_back_to(start, window, cx);
    }

    /// Deletes the selection, or with none, back from the caret to `offset`, as ⌫ does a
    /// character.
    fn delete_back_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        // The input method's while it composes, which has the keys before the field.
        if self.marked_range.is_some() {
            return;
        }
        if self.selected_range.is_empty() {
            if offset == self.cursor_offset() {
                window.play_system_bell();
                return;
            }
            self.select_to(offset, window, cx);
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.request_virtual_keyboard();

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), window, cx)
        }
    }

    /// A right-click shows the field's menu, as any Mac text field has, with the keyboard in the
    /// field. Outside the selection, the caret moves to the click first. Paste stays enabled, as
    /// asking what the clipboard holds before the user pastes can make macOS ask them.
    fn on_right_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = cx.try_global::<FieldMenu>().map(|menu| menu.0.clone()) else {
            return;
        };
        // The click is the field's, not the menu of a row it's in, as when renaming a tab.
        cx.stop_propagation();
        window.focus(&self.focus_handle, cx);
        let at = self.index_for_mouse_position(event.position);
        if at < self.selected_range.start || at > self.selected_range.end {
            self.move_to(at, window, cx);
        }
        self.show_menu(menu, event.position, false, window, cx);
    }

    /// ⇧F10 or the Menu key shows the field's menu under the caret, its first item highlighted.
    fn open_menu(&mut self, _: &OpenMenu, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = cx.try_global::<FieldMenu>().map(|menu| menu.0.clone()) else {
            return;
        };
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let x = (self.last_layout.as_ref())
            .filter(|line| line.text == self.content)
            .map_or(px(0.), |line| line.x_for_index(self.cursor_offset()));
        let at = point(bounds.left() + x - self.scroll, bounds.bottom());
        self.show_menu(menu, at, true, window, cx);
    }

    fn show_menu(
        &mut self,
        menu: Rc<ShowMenu>,
        position: Point<Pixels>,
        from_keys: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = !self.selected_range.is_empty();
        let copies = selected && !self.secure;
        let mut items: Vec<(&str, bool, Option<Box<dyn Action>>)> = vec![
            ("Cut", copies, Some(Box::new(Cut))),
            ("Copy", copies, Some(Box::new(Copy))),
            ("Paste", true, Some(Box::new(Paste))),
        ];
        if self.paste_and_go {
            items.push(("Paste and Go", true, Some(Box::new(PasteAndGo))));
            items.push(("Paste and Search", true, Some(Box::new(PasteAndSearch))));
        }
        items.push(("", false, None));
        items.push((
            "Select All",
            !self.content.is_empty(),
            Some(Box::new(SelectAll)),
        ));
        let titles = (items.iter())
            .map(|&(title, enabled, _)| (MenuEntry::label(title), enabled))
            .collect();
        let mut actions: Vec<Option<Box<dyn Action>>> =
            items.into_iter().map(|(_, _, action)| action).collect();
        let field = cx.weak_entity();
        let chosen: Chosen = Box::new(move |closed, window, cx| {
            let away = field.update(cx, |field, _| {
                field.menu_shown = false;
                match closed {
                    Closed::PressedAt(at) => !field.last_bounds.is_some_and(|it| it.contains(&at)),
                    Closed::Left => true,
                    Closed::Chose(_) | Closed::Otherwise => false,
                }
            });
            if let Some(action) = closed
                .chosen()
                .and_then(|index| actions.get_mut(index)?.take())
            {
                window.dispatch_action(action, cx);
            }
            if away.unwrap_or(false) {
                field.update(cx, |_, cx| cx.emit(Away)).ok();
            }
        });
        self.menu_shown = true;
        menu(titles, position, from_keys, window, chosen, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = text.replace("\n", " ");
            let text = if self.paste_and_go {
                without_javascript(&text)
            } else {
                &text
            };
            self.replace_text_in_range(None, text, window, cx);
            self.pasted = true;
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.secure {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.secure {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        self.text_offset(line.closest_index_for_x(position.x - bounds.left() + self.scroll))
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// Where the word before `offset` starts, past the spaces and punctuation after it, as ⌥⌫
    /// finds it: in "example.com/some/path" from its end, at "path".
    fn previous_word_start(&self, offset: usize) -> usize {
        let before = &self.content[..offset];
        let mut start = offset;
        for (index, text) in before.split_word_bound_indices().rev() {
            start = index;
            if text.chars().any(char::is_alphanumeric) {
                break;
            }
        }
        start
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// Starts over with `text` in the field, all selected.
    pub fn reset(&mut self, text: SharedString, window: &Window, cx: &mut Context<Self>) {
        self.selected_range = 0..text.len();
        self.content = text;
        self.selection_reversed = false;
        self.marked_range = None;
        self.pasted = false;
        self.last_layout = None;
        self.last_bounds = None;
        self.scroll = px(0.);
        self.is_selecting = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    /// Shows `text` with the caret at byte `caret`, nothing selected.
    pub fn set_text(
        &mut self,
        text: SharedString,
        caret: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.content = text;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    pub fn set_placeholder(&mut self, placeholder: SharedString, cx: &mut Context<Self>) {
        self.placeholder = placeholder;
        cx.notify();
    }

    /// Shows `text`, with what follows its first `typed` bytes selected and the caret before it,
    /// as an address bar shows a completion of what was typed.
    pub fn complete(
        &mut self,
        text: SharedString,
        typed: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = typed..text.len();
        self.selection_reversed = true;
        self.content = text;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        self.pasted = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.pasted = false;
        let insertion_start_utf16 = self.offset_to_utf16(range.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;

        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        // An empty field lays out its placeholder, and its caret is at its start. An input method
        // under Wayland composes for a field only once it knows where that is.
        if self.content.is_empty() {
            return Some(Bounds::new(bounds.origin, size(px(0.), bounds.size.height)));
        }
        if last_layout.text != self.shown() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        let left = bounds.left() - self.scroll;
        Some(Bounds::from_corners(
            point(
                left + last_layout.x_for_index(self.shown_offset(range.start)),
                bounds.top(),
            ),
            point(
                left + last_layout.x_for_index(self.shown_offset(range.end)),
                bounds.bottom(),
            ),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range_utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        if last_layout.text != self.shown() {
            return None;
        }
        let utf8_index =
            self.text_offset(last_layout.closest_index_for_x(line_point.x + self.scroll));
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    /// How far the line is scrolled left.
    scroll: Pixels,
    /// The fades over the start and the end, where more text runs past them.
    fades: Vec<PaintQuad>,
}

/// The caret's width.
const CARET: Pixels = px(2.);

/// Where a fade is the ground at full, from its inner side: its outer third is solid, so a glyph
/// the field's edge cuts doesn't show a sliver.
const SOLID: f32 = 2. / 3.;

/// How far a field `width` wide scrolls a line `text` wide so the caret, at `caret`, is in view
/// and `margin` clear of an edge with more text past it, moving no further from `scroll` than it
/// must, so the line stays put while the caret moves within it.
fn scrolled(scroll: Pixels, caret: Pixels, text: Pixels, width: Pixels, margin: Pixels) -> Pixels {
    let end = (text + CARET - width).max(px(0.));
    let margin = margin.min(width / 4.);
    let mut scroll = scroll.min(end);
    if caret + CARET > scroll + width - margin {
        scroll = caret + CARET - width + margin;
    }
    if caret < scroll + margin {
        scroll = caret - margin;
    }
    scroll.clamp(px(0.), end)
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.shown();
        let selected = &input.selected_range;
        let selected_range = input.shown_offset(selected.start)..input.shown_offset(selected.end);
        let cursor = input.shown_offset(input.cursor_offset());
        let style = window.text_style();
        let empty = content.is_empty();
        let all = !empty && selected_range == (0..content.len());

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), input.placeholder_color)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked_range = (input.marked_range.as_ref())
            .map(|marked| input.shown_offset(marked.start)..input.shown_offset(marked.end));
        let runs = if let Some(marked_range) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        // The line follows the caret, except under a selection of all of it, as a field shows when
        // it opens holding a URL: that keeps where it was, from its start.
        let width = bounds.size.width;
        let fade = input.fade.map_or(px(0.), |(width, _)| width);
        let scroll = if empty {
            px(0.)
        } else if all {
            input.scroll.min((line.width - width).max(px(0.)))
        } else {
            scrolled(input.scroll, cursor_pos, line.width, width, fade)
        };
        let left = bounds.left() - scroll;
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(left + cursor_pos, bounds.top()),
                        size(CARET, bounds.bottom() - bounds.top()),
                    ),
                    input.caret_color,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(left + line.x_for_index(selected_range.start), bounds.top()),
                        point(left + line.x_for_index(selected_range.end), bounds.bottom()),
                    ),
                    input.selection_color,
                )),
                None,
            )
        };
        let mut fades = Vec::new();
        if let Some((fade, ground)) = input.fade {
            let (clear, ground) = (
                linear_color_stop(ground.opacity(0.), 0.),
                linear_color_stop(ground, SOLID),
            );
            let band =
                |x: Pixels| Bounds::new(point(x, bounds.top()), size(fade, bounds.size.height));
            if scroll > px(0.) {
                fades.push(fill(
                    band(bounds.left()),
                    linear_gradient(270., clear, ground),
                ));
            }
            if line.width - scroll > width {
                fades.push(fill(
                    band(bounds.right() - fade),
                    linear_gradient(90., clear, ground),
                ));
            }
        }
        self.input.update(cx, |input, _| input.scroll = scroll);
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll,
            fades,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line = prepaint.line.take().unwrap();
        // A line scrolled, or longer than the field, shows only within the field's sides; its
        // glyphs may still reach above and below the line's height.
        let tall = window.line_height();
        let mask = Bounds::from_corners(
            point(bounds.left(), bounds.top() - tall),
            point(bounds.right(), bounds.bottom() + tall),
        );
        let clip = prepaint.scroll > px(0.) || line.width > bounds.size.width;
        window.with_content_mask(clip.then_some(ContentMask { bounds: mask }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection)
            }
            let origin = point(bounds.left() - prepaint.scroll, bounds.top());
            line.paint(origin, tall, gpui::TextAlign::Left, None, window, cx)
                .unwrap();
            for fade in prepaint.fades.drain(..) {
                window.paint_quad(fade);
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("text-input")
            // A text field called what its placeholder asks for, saying what's typed in it, while
            // an assistive app reads the window. Typing still goes through the input handler, so
            // VoiceOver hears what IME commits. The text is a text run too, with the caret in it,
            // as AT-SPI's Text, which Orca reads, has text only from runs. A secure field gives
            // its dots, never what was typed.
            .when(window.is_a11y_active(), |field| {
                let name = self.placeholder.trim_end_matches('…').to_owned();
                let role = if self.secure {
                    Role::PasswordInput
                } else {
                    Role::TextInput
                };
                let shown = self.shown();
                let characters =
                    |offset| shown[..self.shown_offset(offset)].graphemes(true).count();
                let anchor = match self.selection_reversed {
                    true => self.selected_range.end,
                    false => self.selected_range.start,
                };
                let (anchor, focus) = (characters(anchor), characters(self.cursor_offset()));
                field
                    .role(role)
                    .aria_label(name)
                    .aria_value(shown.clone())
                    .a11y_synthetic_children(move |builder| {
                        let mut run = accesskit::Node::new(Role::TextRun);
                        let lengths = shown.graphemes(true).map(|each| each.len().min(255) as u8);
                        run.set_character_lengths(lengths.collect::<Vec<_>>());
                        run.set_value(shown.to_string());
                        let node = builder.synthetic_node_id("text");
                        builder.push_child(node, run);
                        let at = |character_index| accesskit::TextPosition {
                            node,
                            character_index,
                        };
                        builder
                            .parent_node()
                            .set_text_selection(accesskit::TextSelection {
                                anchor: at(anchor),
                                focus: at(focus),
                            });
                    })
            })
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_to_start))
            .on_action(cx.listener(Self::delete_word_back))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::open_menu))
            .on_action(cx.listener(Self::paste))
            // A field's text has no style, so it pastes as Paste does.
            .on_action(cx.listener(|field, _: &PasteAndMatchStyle, window, cx| {
                field.paste(&Paste, window, cx)
            }))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            // A field has no undo yet. Its ⌘Z stops here, so it doesn't undo a Clear.
            .on_action(|_: &Undo, _, _| {})
            .on_action(|_: &Redo, _, _| {})
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_down_out(cx.listener(|field, _, window, cx| {
                if field.focus_handle.is_focused(window) {
                    cx.emit(Away)
                }
            }))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod leave_tests;
#[cfg(test)]
mod tests;
