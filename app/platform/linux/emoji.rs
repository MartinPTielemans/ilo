//! Whether the system draws an emoji, as host_can_draw_emoji asks, on Linux and Windows, from
//! GPUI's own text system, which draws the picker: one glyph that takes room, and none missing, is
//! a drawn emoji. A font older than the emoji shapes a sequence it doesn't know as its parts, and
//! an emoji no font has as the missing glyph. Unlike the Mac's emoji.mm, the glyph needn't be the
//! emoji font's: GPUI draws a symbol such as ❤️ from the first font that has it, in the text's
//! colour, with its variation selector as a glyph of no width, and it's still there.

use std::{
    cell::OnceCell,
    ffi::{CStr, c_char},
};

use gpui::{App, TextRun, WindowTextSystem, font, px};

use browser::tokens::icon_picker::emoji::EMOJI_SIZE;

thread_local! {
    static TEXT: OnceCell<WindowTextSystem> = const { OnceCell::new() };
}

/// Keeps the app's text system, as the picker asks while the app is already borrowed.
pub fn keep(cx: &App) {
    TEXT.with(|text| {
        text.get_or_init(|| WindowTextSystem::new(cx.text_system().clone()));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host_can_draw_emoji(emoji: *const c_char) -> bool {
    let emoji = unsafe { CStr::from_ptr(emoji) }.to_string_lossy();
    TEXT.with(|text| {
        // No window has opened, so no picker either.
        let Some(text) = text.get() else {
            return true;
        };
        let run = TextRun { len: emoji.len(), font: font(browser::face::UI), ..Default::default() };
        let line = text.layout_line(&emoji, px(EMOJI_SIZE), &[run], None);
        let glyphs: Vec<_> = line.runs.iter().flat_map(|run| &run.glyphs).collect();
        let ends = glyphs.iter().skip(1).map(|glyph| glyph.position.x).chain([line.width]);
        let wide = glyphs.iter().zip(ends).filter(|(glyph, end)| *end > glyph.position.x);
        wide.count() == 1 && glyphs.iter().all(|glyph| glyph.id.0 != 0)
    })
}
