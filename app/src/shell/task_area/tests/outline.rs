//! The line round a page a thread shows: the page's own outline, as any page has it, at rest and
//! while the thread drives the page.

use gpui::{Hsla, rgba};

use super::*;
use crate::palette::Tone;
use crate::tokens::shell::{PAGE_OUTLINE, PAGE_OUTLINE_LIGHT, PAGE_OUTLINE_WIDTH};

/// Draws the window as it is in `tone`.
fn toned(shell: &Entity<Shell>, tone: Tone, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, _| shell.force_tone(Some(tone)));
    draw(shell, cx);
    draw(shell, cx);
    assert_eq!(shell.read_with(cx, |shell, _| shell.palette.tone), tone);
}

/// The watched page's card: its part of the page area.
fn page_card(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| {
        let (card, _) = shell.page_frame(shell.viewport);
        watched(shell.page_part(card))
    })
}

/// The card the outline was drawn round in the last frame.
fn line(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Bounds<Pixels>> {
    shell.read_with(cx, |shell, _| {
        shell.task_area.as_ref().unwrap().outlined.get()
    })
}

/// The outline was drawn round `card` as a page's is: outside it by the line's width on all four
/// sides, the one beside the conversation's column too (the owner, #4396), in `tone`'s own ink.
fn outlined(shell: &Entity<Shell>, card: Bounds<Pixels>, tone: Tone, cx: &mut VisualTestContext) {
    let width = px(PAGE_OUTLINE_WIDTH);
    assert_eq!(line(shell, cx), Some(card), "{tone:?}: no outline round it");
    let mut line = shell.read_with(cx, |shell, _| shell.watched_outline(card));
    let style = line.style();
    // Outside the card by its own width all round.
    let at = |edge: Pixels| Some(gpui::Length::from(edge));
    assert!(
        style.inset.left == at(card.left() - width)
            && style.inset.top == at(card.top() - width)
            && style.size.width == at(card.size.width + 2. * width)
            && style.size.height == at(card.size.height + 2. * width),
        "{tone:?}: not outside the card on all four sides"
    );
    let sides = &style.border_widths;
    for side in [sides.left, sides.top, sides.right, sides.bottom] {
        assert!(side == Some(width.into()), "{tone:?}: a side with no line");
    }
    let ink = match tone {
        Tone::Dark => PAGE_OUTLINE,
        Tone::Light => PAGE_OUTLINE_LIGHT,
    };
    assert_eq!(style.border_color, Some(Hsla::from(rgba(ink))), "{tone:?}");
}

/// A page a thread shows has the page's own outline at rest, in Dark and in Light: on its top,
/// right and bottom and none on its left.
#[gpui::test]
fn a_watched_page_has_the_pages_own_outline_in_both_tones(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
    }
}

/// Watched in its tile, the outline is round the page's card, the ground the tile shows on, in
/// both tones.
#[gpui::test]
fn a_page_watched_in_its_tile_has_the_pages_own_outline_in_both_tones(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching_tile(&shell, &host, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
    }
}

/// While the thread drives the page, nothing is drawn on it: the card and the panel beside the
/// page say it is at work, and the page keeps its own outline alone, in both tones.
#[gpui::test]
fn a_driven_page_has_its_own_outline_alone(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
    }
}

/// A page driven in its tile has the outline round its card alone, in both tones.
#[gpui::test]
fn a_page_driven_in_its_tile_has_its_own_outline_alone(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    host.shows_tiles.set(Some(true));
    watching_working(&shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watch_tile_shown()),
        Some(30)
    );
    for tone in [Tone::Dark, Tone::Light] {
        toned(&shell, tone, cx);
        let card = page_card(&shell, cx);
        outlined(&shell, card, tone, cx);
    }
}
