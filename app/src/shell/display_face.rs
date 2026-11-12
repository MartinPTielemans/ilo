//! ilo's display face, Outfit (OFL, its license in `assets/fonts`), which only the rare screens set:
//! an empty Space's name, the first run's welcome and the Library's heading. Everything else is in
//! the chrome's face (`crate::face`). It's registered the first time one of them draws, never at
//! launch, since registering a font makes GPUI shape every line again once.

use std::borrow::Cow;
use web_time::Instant;

use gpui::{App, FontWeight, Global, ParentElement, SharedString, Styled, div, px};

const OUTFIT_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Outfit-Medium.ttf");
const FAMILY: &str = "Outfit";

/// Set once the face is registered with the app's text system, or has failed to be.
struct Registered;

impl Global for Registered {}

/// Registers the face the first time it's asked for.
fn register(cx: &mut App) {
    if cx.has_global::<Registered>() {
        return;
    }
    cx.set_global(Registered);
    let start = Instant::now();
    match cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(OUTFIT_MEDIUM)])
    {
        // check.sh reads this line, for what the face costs the screen that first shows it.
        Ok(()) => log!(
            "browser: display face in {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.
        ),
        Err(error) => log!("couldn't register the display face: {error}"),
    }
}

/// `text` set in the display face at `size`, a line of its own `line` tall.
pub fn display_text(
    text: impl Into<SharedString>,
    size: f32,
    line: f32,
    cx: &mut App,
) -> gpui::Div {
    register(cx);
    div()
        .font_family(FAMILY)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(size))
        .line_height(px(line))
        .child(text.into())
}

/// How far below a line's top Outfit's cap height starts, set at `size` in a line `line` tall:
/// GPUI centres the face's ascent and descent (1 and 0.26 of the size) in the line, and the caps
/// are 0.7 of the size tall. Lets a heading's caps be put where they're measured.
pub(super) fn cap_top(size: f32, line: f32) -> f32 {
    let (ascent, descent, cap) = (size, 0.26 * size, 0.7 * size);
    (line - ascent - descent) / 2. + ascent - cap
}
