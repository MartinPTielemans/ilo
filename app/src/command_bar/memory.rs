//! Memory's rows and its one-time ask, as the #2324 spike mocks them for the preview: a page found
//! by the words it said, and, the first time a query finds no page by its title, the question
//! that turns Memory on. Nothing here reads or keeps a page; the rows are made up.

use gpui::{
    AnyElement, App, Context, FontWeight, HighlightStyle, Hsla, Rgba, SharedString, StyledText,
    div, prelude::*, px,
};

use super::{CommandBar, glyph};
use crate::palette::{self, STATE_HOVERED, STATE_PRESSED};
use crate::tokens::command_bar::{
    ASK_BUTTON, ASK_BUTTON_X, ASK_BUTTONS_TOP, ASK_GAP, ASK_HEIGHT, ASK_LINE, ASK_Y, CHIP,
    CHIP_RADIUS, ICON_BOX, ICON_BOX_X, ICON_SIZE, LINE, LIST_X, ROW_END, SECONDARY, TITLE, TITLE_X,
};

/// Lucide's text-search, Find's: words found in a page's text, here in a page read before.
pub const GLYPH: &str = "icons/text-search.svg";

/// The question, then where it's kept and who can't read it, what's kept out by itself, what
/// isn't, and how to keep out the rest, each said no stronger than the site's own signal is.
const QUESTION: &str = "Find pages by what they said?";

fn said() -> String {
    format!(
        "{app} can keep the words of pages you read on this device, encrypted with the key that \
         guards your saved passwords. Agents can't read them. It never keeps private windows, \
         password or payment fields, pages that ask not to be kept, or sites that say they're a \
         bank, insurer or medical site, or are on .bank or .insurance. Other bank and health sites, \
         and health pages on general sites, are kept until you add them in Settings, where you \
         can forget a site or everything at any time.",
        app = env!("APP_NAME"),
    )
}

/// How much taller the panel is with the ask at its foot: its line and the ask.
pub fn ask_height() -> f32 {
    LINE + ASK_HEIGHT
}

/// The words a row was found by, after its title as its host would be: the words typed in
/// `title`'s colour, the rest in `dim`, both laid on `ground`, the row's fill. GPUI lays a
/// highlight's colour over the text's and keeps the text's alpha, so the same ink at two alphas
/// would come out as one; laid on the ground first, each is solid and shows as it is.
pub(super) fn words(
    words: &SharedString,
    matched: &[std::ops::Range<usize>],
    title: Hsla,
    dim: Hsla,
    ground: Rgba,
) -> AnyElement {
    let solid = |color: Hsla| {
        let laid = ground.blend(Rgba::from(color));
        Hsla::from(Rgba { a: 1., ..laid })
    };
    let (title, dim) = (solid(title), solid(dim));
    let lead = "— ";
    let text: SharedString = format!("{lead}{words}").into();
    let highlights = matched.iter().map(|range| {
        let style = HighlightStyle {
            color: Some(title),
            ..HighlightStyle::default()
        };
        (range.start + lead.len()..range.end + lead.len(), style)
    });
    div()
        .min_w_0()
        .flex_1()
        .truncate()
        .text_color(dim)
        .child(StyledText::new(text).with_highlights(highlights))
        .into_any_element()
}

impl CommandBar {
    /// The ask at the panel's foot: Memory's glyph in the rows' icon column, the question and
    /// what it keeps in their title column, and Not Now and Turn On under them at the end, both
    /// quiet, as a yes asked for shouldn't be the easier press.
    pub(super) fn memory_ask(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = palette::current(cx);
        let button = |id: &'static str, label: &'static str| {
            let hovered = palette.state(CHIP, STATE_HOVERED);
            let pressed = palette.state(CHIP, STATE_PRESSED);
            div()
                .id(id)
                .flex_none()
                .h(px(ASK_BUTTON))
                .px(px(ASK_BUTTON_X))
                .flex()
                .items_center()
                .rounded(px(CHIP_RADIUS))
                .bg(palette.plate(CHIP))
                .hover(move |style| style.bg(hovered))
                .active(move |style| style.bg(pressed))
                .text_color(palette.on_overlay(TITLE))
                .child(label)
                .on_click(cx.listener(move |bar, _, _, cx| {
                    crate::log!("browser: memory ask {label}");
                    bar.memory_asking = false;
                    cx.notify();
                }))
        };
        let tile = div()
            .size(px(ICON_BOX))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CHIP_RADIUS))
            .bg(palette.plate(CHIP))
            .child(glyph(GLYPH, ICON_SIZE, palette.on_overlay_glyph(TITLE)));
        let buttons = div()
            .mt(px(ASK_BUTTONS_TOP))
            .flex()
            .justify_end()
            .gap(px(ASK_GAP))
            .child(button("memory-not-now", "Not Now"))
            .child(button("memory-turn-on", "Turn On"));
        let words = div()
            .flex_1()
            .min_w_0()
            .ml(px(TITLE_X - ICON_BOX_X - ICON_BOX))
            .line_height(px(ASK_LINE))
            .child(div().text_color(palette.on_overlay(TITLE)).child(QUESTION))
            .child(
                div()
                    .font_weight(FontWeight::NORMAL)
                    .text_color(palette.on_overlay(SECONDARY))
                    .child(said()),
            )
            .child(buttons);
        div()
            .h(px(ASK_HEIGHT))
            .flex_none()
            .flex()
            .items_start()
            .pt(px(ASK_Y))
            .pl(px(LIST_X + ICON_BOX_X))
            .pr(px(LIST_X + ROW_END))
            .child(tile)
            .child(words)
            .into_any_element()
    }
}

/// Puts the ask at the foot of the bar that's open, as the first query that finds no page by its
/// title would.
pub fn ask(cx: &mut App) {
    let Some(window) = cx.try_global::<super::CommandBarWindow>().cloned() else {
        return;
    };
    window
        .update(cx, |bar, _, cx| {
            bar.memory_asking = true;
            cx.notify();
        })
        .ok();
}

/// Leaves the caret after what's typed in the bar that's open, nothing selected, as typing does.
pub fn typed(cx: &mut App) {
    let Some(window) = cx.try_global::<super::CommandBarWindow>().cloned() else {
        return;
    };
    window
        .update(cx, |bar, window, cx| {
            bar.input.update(cx, |input, cx| {
                let text: SharedString = input.text().to_string().into();
                let caret = text.len();
                input.set_text(text, caret, window, cx)
            })
        })
        .ok();
}

/// Selects the row at `index` of the bar that's open, as ↓ would.
pub fn select(index: usize, cx: &mut App) {
    let Some(window) = cx.try_global::<super::CommandBarWindow>().cloned() else {
        return;
    };
    window
        .update(cx, |bar, window, cx| bar.select(index, window, cx))
        .ok();
}
