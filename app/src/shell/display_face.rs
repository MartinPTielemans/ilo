//! ilo's display face, Outfit (OFL, its license in `assets/fonts`), which only the rare screens set:
//! an empty Space's name, the first run's welcome and the Library's heading. Everything else is in
//! the chrome's face (`crate::face`). Each weight is registered the first time a text in it draws,
//! never at launch, since registering a font makes GPUI shape every line again once.

use std::borrow::Cow;
use std::time::Instant;

use gpui::{App, FontWeight, Global, ParentElement, SharedString, Styled, div, px};

const OUTFIT_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Outfit-Medium.ttf");
const OUTFIT_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Outfit-SemiBold.ttf");
const FAMILY: &str = "Outfit";

/// The file of the face's `weight`: medium (500) for the product's name and an empty Space's,
/// semibold (600) for headings. A weight it hasn't is the nearest it has.
fn file(weight: FontWeight) -> &'static [u8] {
    match weight.0 >= 550. {
        true => OUTFIT_SEMIBOLD,
        false => OUTFIT_MEDIUM,
    }
}

/// The face's files registered with the app's text system, or that failed to be.
#[derive(Default)]
struct Registered(Vec<&'static [u8]>);

impl Global for Registered {}

/// Registers the face's file for `weight` the first time a text in it is asked for.
fn register(weight: FontWeight, cx: &mut App) {
    let file = file(weight);
    let registered = cx.default_global::<Registered>();
    if registered.0.iter().any(|done| std::ptr::eq(*done, file)) {
        return;
    }
    registered.0.push(file);
    let start = Instant::now();
    match cx.text_system().add_fonts(vec![Cow::Borrowed(file)]) {
        // check.sh reads this line, for what the face costs the screen that first shows it.
        Ok(()) => log!(
            "browser: display face in {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.
        ),
        Err(error) => log!("couldn't register the display face: {error}"),
    }
}

/// `text` set in the display face at medium, at `size`, a line of its own `line` tall.
pub fn display_text(
    text: impl Into<SharedString>,
    size: f32,
    line: f32,
    cx: &mut App,
) -> gpui::Div {
    display_text_weighted(text, size, line, FontWeight::MEDIUM, cx)
}

/// `text` set in the display face at `weight`, at `size`, a line of its own `line` tall.
pub fn display_text_weighted(
    text: impl Into<SharedString>,
    size: f32,
    line: f32,
    weight: FontWeight,
    cx: &mut App,
) -> gpui::Div {
    register(weight, cx);
    div()
        .font_family(FAMILY)
        .font_weight(weight)
        .text_size(px(size))
        .line_height(px(line))
        .child(text.into())
}

/// The face's family, for a run of a text set in it at medium.
pub(super) fn family(cx: &mut App) -> SharedString {
    register(FontWeight::MEDIUM, cx);
    FAMILY.into()
}

/// How far below a line's top Outfit's cap height starts, set at `size` in a line `line` tall:
/// GPUI centres the face's ascent and descent (1 and 0.26 of the size) in the line, and the caps
/// are 0.7 of the size tall. Lets a heading's caps be put where they're measured.
pub(super) fn cap_top(size: f32, line: f32) -> f32 {
    let (ascent, descent, cap) = (size, 0.26 * size, 0.7 * size);
    (line - ascent - descent) / 2. + ascent - cap
}

#[cfg(test)]
mod tests {
    use gpui::{FontWeight, TestAppContext};

    use super::{OUTFIT_MEDIUM, OUTFIT_SEMIBOLD, Registered, display_text_weighted, file};

    /// The weight class a font file says it is, in its OS/2 table.
    fn weight_class(font: &[u8]) -> u16 {
        let be = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]);
        let tables = be(4) as usize;
        (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&record| &font[record..record + 4] == b"OS/2")
            .map(|record| {
                let at = u32::from_be_bytes(font[record + 8..record + 12].try_into().unwrap());
                be(at as usize + 4)
            })
            .expect("an OS/2 table")
    }

    #[test]
    fn each_weight_is_its_own_file() {
        assert_eq!(weight_class(file(FontWeight::SEMIBOLD)), 600);
        assert_eq!(weight_class(file(FontWeight::MEDIUM)), 500);
        assert_eq!(weight_class(OUTFIT_SEMIBOLD), 600);
        assert_eq!(weight_class(OUTFIT_MEDIUM), 500);
    }

    /// A text at medium registers medium alone; semibold comes with the first text in it.
    #[gpui::test]
    fn a_weight_is_registered_at_its_first_use(cx: &mut TestAppContext) {
        let registered = |cx: &mut TestAppContext| {
            cx.update(|cx| {
                (cx.try_global::<Registered>())
                    .map(|registered| {
                        registered
                            .0
                            .iter()
                            .map(|f| weight_class(f))
                            .collect::<Vec<u16>>()
                    })
                    .unwrap_or_default()
            })
        };
        assert_eq!(registered(cx), Vec::<u16>::new(), "nothing at launch");
        cx.update(|cx| _ = display_text_weighted("Library", 20., 24., FontWeight::MEDIUM, cx));
        assert_eq!(registered(cx), vec![500]);
        cx.update(|cx| _ = display_text_weighted("Library", 20., 24., FontWeight::SEMIBOLD, cx));
        cx.update(|cx| _ = display_text_weighted("Library", 20., 24., FontWeight::SEMIBOLD, cx));
        assert_eq!(registered(cx), vec![500, 600], "once each");
    }
}
