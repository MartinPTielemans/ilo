//! The UI's reads of the clipboard's text: its fields' paste, Paste and Go, a pasted URL as a tab,
//! and a copied secret's clearing. GPUI reads it, except where the browser gives a reader of its
//! own: under Wayland, where the UI shares Chromium's connection to the compositor, it reads through
//! Chromium's clipboard (bc_clipboard_read_text). There GPUI can't read a selection Chromium holds,
//! as Chromium would answer on the thread that waits, and a compositor may hand the selection to
//! the connection's first data device alone, which is Chromium's, as Hyprland does (#2463).

use std::cell::Cell;

use gpui::App;

/// What a read hands on: the clipboard's text, or None if it holds none.
pub type Done = Box<dyn FnOnce(Option<String>, &mut App)>;

thread_local! {
    static READER: Cell<Option<fn(Done)>> = const { Cell::new(None) };
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
