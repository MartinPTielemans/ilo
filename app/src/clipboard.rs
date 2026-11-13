//! The UI's reads of the clipboard's text: its fields' paste, Paste and Go and a pasted URL as a
//! tab. GPUI reads it, except where the browser gives a reader of its own: under Wayland, where the
//! UI shares Chromium's connection to the compositor, it reads through Chromium's clipboard
//! (bc_clipboard_read_text). There GPUI can't read a selection Chromium holds, as Chromium would
//! answer on the thread that waits, and a compositor may hand the selection to the connection's
//! first data device alone, which is Chromium's, as Hyprland does (#2463).
//!
//! A copied secret's clearing asks something else: whether the clipboard still holds what the app
//! put there. GPUI knows that itself, and Chromium's reader can't say: it reads the app's own copy
//! as empty while the app hasn't the keyboard, so the clear never ran.

use std::cell::Cell;

use gpui::App;

/// What a read hands on: the clipboard's text, or None if it holds none.
pub type Done = Box<dyn FnOnce(Option<String>, &mut App)>;

thread_local! {
    static READER: Cell<Option<fn(Done)>> = const { Cell::new(None) };
    static HOLDER: Cell<Option<fn() -> Option<String>>> = const { Cell::new(None) };
}

/// From now on the UI's reads go through `reader`, which hands `done` the text once it has it.
pub fn read_through(reader: fn(Done)) {
    READER.set(Some(reader));
}

/// Hands the clipboard's text, or None if it holds none, to `done`, never during the call: after
/// the update under way from GPUI, or once the browser's reader has it. So a paste lands as
/// Chromium's own fields' does on Linux, where they read the clipboard the same way.
pub fn read_text(cx: &mut App, done: impl FnOnce(Option<String>, &mut App) + 'static) {
    match READER.get() {
        Some(read) => read(Box::new(done)),
        None => {
            let text = cx.read_from_clipboard().and_then(|item| item.text());
            cx.defer(move |cx| done(text, cx));
        }
    }
}

/// From now on, what the app holds the clipboard with comes from `holder`: the text it put there,
/// or None once something else was copied. Under Wayland that's GPUI's own data source, which the
/// compositor cancels on another copy (gpui_linux::held_clipboard_text).
pub fn held_through(holder: fn() -> Option<String>) {
    HOLDER.set(Some(holder));
}

/// Whether the clipboard still holds `text` as the app put it there, for a copied secret's clear.
/// From the holder where there is one, else from GPUI's clipboard, which answers the app's own copy
/// itself. Never through the browser's reader.
pub fn holds_own(cx: &App, text: &str) -> bool {
    match HOLDER.get() {
        Some(held) => held().as_deref() == Some(text),
        None => {
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref()
                == Some(text)
        }
    }
}
